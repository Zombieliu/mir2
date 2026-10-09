import { promises as fs } from "node:fs";
import { constants as fsConstants } from "node:fs";
import { realpathSync } from "node:fs";
import { createRequire } from "node:module";
import { createHash } from "node:crypto";
import path from "node:path";

const ROOT_PUBLIC_FILES = new Set([
  "favicon.ico", "favicon.svg", "mir2-asset-worker.js", "original-asset-manifest.generated.json",
]);
const LOCAL_UI_FALLBACKS = new Set([
  "original-ui/Prguse/2092.png", "original-ui/Prguse/2094.png", "original-ui/Prguse/2095.png",
]);
const LOCAL_DEBUG_FILES = new Set([
  "debug/map-samples/smtile-72.png", "debug/map-samples/smtile-80.png",
]);
const WHOLE_PUBLIC_ROOTS = new Set([
  "bevy-entity-atlases", "bootstrap", "original-effects", "pwa",
]);
const MAP_PUBLIC_ROOTS = new Set(["map-atlas", "original-map-blend"]);
const REMOTE_UI_MEDIA_ROOTS = new Set([
  "AArmour", "AHair", "ARArmour", "ARHair", "ARWeapon", "AWeapon", "CArmour", "CHair",
  "ChrSel", "CWeapon", "Cursors", "Items", "MMap", "MapLinkIcon", "Monster", "NPC",
  "Prguse", "Prguse2", "Sound", "Title",
  // These measured non-actor libraries use the existing original-ui miss proxy.
  "Help", "DNItems", "StateItem", "MagIcon", "MagIcon2", "BuffIcon", "GuildSkill",
  // These measured actor/door roots use that proxy as well.
  "Mount", "Pet", "Gate",
]);
const REQUIRED_MODULES = ["next", "next/dist/server/lib/start-server", "react", "react-dom", "sharp"];

function segmentsOf(relativePath) {
  if (typeof relativePath !== "string" || path.win32.isAbsolute(relativePath) || path.posix.isAbsolute(relativePath) ||
      relativePath.includes("\0") || relativePath.includes(":")) {
    throw new Error(`Unsafe portable path: ${relativePath}`);
  }
  if (!relativePath) return [];
  const segments = relativePath.replaceAll("\\", "/").split("/");
  if (segments.some((segment) => !segment || segment === "." || segment === "..")) {
    throw new Error(`Unsafe portable path: ${relativePath}`);
  }
  return segments;
}

function inside(root, candidate) {
  const relative = path.relative(root, candidate);
  return relative === "" || (relative !== ".." && !relative.startsWith(`..${path.sep}`) && !path.isAbsolute(relative));
}

function requireInside(root, candidate) {
  if (!path.isAbsolute(root) || !path.isAbsolute(candidate) || !inside(root, candidate)) {
    throw new Error(`Portable path escapes its root: ${candidate}`);
  }
}

function isSensitivePath(candidate) {
  return String(candidate).split(/[\\/]/).some((segment) => {
    const name = segment.toLowerCase();
    return name === ".env" || name.startsWith(".env.") || /\.(?:pem|key|p12|pfx)$/.test(name);
  });
}

export function selectThinPublicEntry(relativePath, isDirectory, { runtimeVersion, runtimeFiles, mapAtlasFiles = null, coreVersion = null, coreFiles = new Set() }) {
  const segments = segmentsOf(relativePath);
  if (segments.length === 0) return isDirectory;
  const normalized = segments.join("/");
  if (isSensitivePath(normalized)) return false;
  const [root, second] = segments;
  if (root === "client-core") {
    return isDirectory
      ? segments.length === 1 || ([...coreFiles].some((file) => file.startsWith(`${normalized}/`)))
      : coreFiles.has(normalized);
  }
  if (root === "bevy-runtime-releases") {
    if (isDirectory) {
      return segments.length === 1 || (second === runtimeVersion && [...runtimeFiles].some((file) =>
        file.startsWith(`${normalized}/`)));
    }
    return runtimeFiles.has(normalized);
  }
  if (root === "generated") {
    if (isDirectory && segments.length === 1) return true;
    if (second === "map-atlas" && mapAtlasFiles) {
      return isDirectory
        ? [...mapAtlasFiles].some((file) => file.startsWith(`${normalized}/`))
        : mapAtlasFiles.has(normalized);
    }
    return MAP_PUBLIC_ROOTS.has(second) && (isDirectory || segments.length > 2);
  }
  if (root === "original-ui") {
    if (isDirectory) return true;
    const name = segments.at(-1);
    const supportFile = /^(?:LICENSE(?:\..*)?|COPYING(?:\..*)?|NOTICE(?:\..*)?)$/i.test(name) ||
      /\.(?:otf|ttf|woff2?)$/i.test(name);
    return !REMOTE_UI_MEDIA_ROOTS.has(second) || LOCAL_UI_FALLBACKS.has(normalized) ||
      normalized.endsWith(".json") || supportFile;
  }
  if (root === "debug") {
    return isDirectory ? segments.length <= 2 : LOCAL_DEBUG_FILES.has(normalized);
  }
  if (WHOLE_PUBLIC_ROOTS.has(root)) return isDirectory || segments.length > 1;
  return !isDirectory && segments.length === 1 && ROOT_PUBLIC_FILES.has(root);
}

export function classifyPortableStandalonePath(relativePath, appRelativePath, isDirectory, publicSelection) {
  const segments = segmentsOf(relativePath);
  const app = segmentsOf(appRelativePath);
  if (app.every((segment, index) => segments[index] === segment) && segments.length > app.length) {
    const tail = segments.slice(app.length);
    if (tail[0] === "node_modules") return { copy: false, reason: "external dependency junction" };
    if (tail[0] === ".next" && tail[1] === "cache" && tail[2] === "mir2-scene-blueprints") {
      return { copy: false, reason: "scene cache seed" };
    }
    if (tail.at(-1).endsWith(".nft.json")) return { copy: false, reason: "copied NFT metadata" };
    if (tail[0] === "public") {
      const publicRelative = tail.slice(1).join("/");
      if (!publicSelection(publicRelative, isDirectory)) return { copy: false, reason: "thin public selection" };
    }
  }
  if (segments.some((segment) => isSensitivePath(segment))) {
    return { copy: false, reason: "private build input" };
  }
  return { copy: true, reason: null };
}

export async function copyPortableStandalone({ sourceRoot, destinationRoot, appRelativePath, publicSelection }) {
  if (!path.isAbsolute(sourceRoot) || !path.isAbsolute(destinationRoot)) throw new Error("Portable copy roots must be absolute");
  if (inside(sourceRoot, destinationRoot) || inside(destinationRoot, sourceRoot)) {
    throw new Error("Portable copy roots must not overlap");
  }
  if ((await fs.lstat(sourceRoot)).isSymbolicLink()) throw new Error("Portable standalone source root is a link");
  if ((await fs.lstat(sourceRoot)).isDirectory() !== true) throw new Error("Portable standalone source root is not a directory");
  await assertAbsent(destinationRoot);
  await fs.mkdir(destinationRoot);
  const result = { files: 0, bytes: 0, sourceBytes: 0, skipped: {}, jsonCompactions: [] };
  async function visit(sourceDirectory, destinationDirectory, relativeDirectory) {
    for (const entry of await fs.readdir(sourceDirectory, { withFileTypes: true })) {
      const relative = relativeDirectory ? `${relativeDirectory}/${entry.name}` : entry.name;
      const source = path.resolve(sourceRoot, ...segmentsOf(relative));
      const destination = path.resolve(destinationRoot, ...segmentsOf(relative));
      requireInside(sourceRoot, source);
      requireInside(destinationRoot, destination);
      const stat = await fs.lstat(source);
      const decision = classifyPortableStandalonePath(relative, appRelativePath, stat.isDirectory(), publicSelection);
      if (!decision.copy) {
        result.skipped[decision.reason] = (result.skipped[decision.reason] ?? 0) + 1;
        continue;
      }
      if (stat.isSymbolicLink()) throw new Error(`Unclassified standalone link: ${relative}`);
      if (stat.isDirectory()) {
        await fs.mkdir(destination);
        await visit(source, destination, relative);
      } else if (stat.isFile()) {
        const compact = relative.slice(appRelativePath.length + 1).replaceAll("\\", "/");
        const outcome = await copySelectedFile(source, destination,
          compact.startsWith("public/") ? compact.slice("public/".length) : null);
        if (outcome.compaction) result.jsonCompactions.push(outcome.compaction);
        result.files += 1;
        result.bytes += outcome.outputBytes;
        result.sourceBytes += stat.size;
      } else {
        throw new Error(`Unsupported standalone entry: ${relative}`);
      }
    }
  }
  await visit(sourceRoot, destinationRoot, "");
  return result;
}

export async function copySelectedPublic({ sourceRoot, destinationRoot, selection }) {
  if (!path.isAbsolute(sourceRoot) || !path.isAbsolute(destinationRoot)) throw new Error("Public copy roots must be absolute");
  if (inside(sourceRoot, destinationRoot) || inside(destinationRoot, sourceRoot)) {
    throw new Error("Public copy roots must not overlap");
  }
  if ((await fs.lstat(sourceRoot)).isSymbolicLink()) throw new Error("Source public root is a link");
  await fs.mkdir(destinationRoot, { recursive: true });
  let files = 0;
  let collisions = 0;
  const jsonCompactions = [];
  async function visit(sourceDirectory, destinationDirectory, relativeDirectory) {
    for (const entry of await fs.readdir(sourceDirectory, { withFileTypes: true })) {
      const relative = relativeDirectory ? `${relativeDirectory}/${entry.name}` : entry.name;
      const source = path.resolve(sourceRoot, ...segmentsOf(relative));
      const destination = path.resolve(destinationRoot, ...segmentsOf(relative));
      requireInside(sourceRoot, source);
      requireInside(destinationRoot, destination);
      const stat = await fs.lstat(source);
      if (!selection(relative, stat.isDirectory())) continue;
      if (stat.isSymbolicLink()) throw new Error(`Selected public asset is a link: ${relative}`);
      if (stat.isDirectory()) {
        await fs.mkdir(destination, { recursive: true });
        await visit(source, destination, relative);
      } else if (stat.isFile()) {
        await fs.mkdir(path.dirname(destination), { recursive: true });
        const outcome = await copySelectedFile(source, destination, relative);
        if (outcome.collision) collisions += 1;
        if (outcome.compaction) jsonCompactions.push(outcome.compaction);
        files += 1;
      } else {
        throw new Error(`Unsupported public entry: ${relative}`);
      }
    }
  }
  await visit(sourceRoot, destinationRoot, "");
  return { files, collisions, jsonCompactions };
}

/** Remove only JSON grammar whitespace outside strings, leaving every token byte intact. */
export function compactJsonWhitespace(source) {
  const input = Buffer.isBuffer(source) ? source : Buffer.from(source);
  const output = Buffer.allocUnsafe(input.length);
  let length = 0;
  let inString = false;
  let escaped = false;
  for (const byte of input) {
    if (inString) {
      output[length++] = byte;
      if (escaped) escaped = false;
      else if (byte === 0x5c) escaped = true;
      else if (byte === 0x22) inString = false;
    } else if (byte === 0x22) {
      inString = true;
      output[length++] = byte;
    } else if (byte !== 0x20 && byte !== 0x09 && byte !== 0x0a && byte !== 0x0d) {
      output[length++] = byte;
    }
  }
  if (inString) throw new Error("Unterminated JSON string while compacting public metadata");
  JSON.parse(input.toString("utf8"));
  JSON.parse(output.subarray(0, length).toString("utf8"));
  return output.subarray(0, length);
}

async function copySelectedFile(source, destination, publicRelative) {
  const compact = publicRelative === "original-asset-manifest.generated.json" ||
    publicRelative === "bevy-entity-atlases/manifest.json" ||
    (publicRelative?.startsWith("original-ui/") && publicRelative.endsWith(".json"));
  const sourceBytes = compact ? await fs.readFile(source) : null;
  const outputBytes = compact ? compactJsonWhitespace(sourceBytes) : null;
  const compaction = compact ? {
    path: publicRelative,
    sourceBytes: sourceBytes.length,
    outputBytes: outputBytes.length,
    sourceSha256: createHash("sha256").update(sourceBytes).digest("hex"),
    outputSha256: createHash("sha256").update(outputBytes).digest("hex"),
  } : null;
  let collision = false;
  try {
    const existing = await fs.lstat(destination);
    if (!existing.isFile() || existing.isSymbolicLink()) throw new Error(`Selected destination is not a regular file: ${destination}`);
    const actual = await fs.readFile(destination);
    const expected = outputBytes ?? await fs.readFile(source);
    if (!actual.equals(expected)) throw new Error(`Standalone/source public collision differs: ${publicRelative ?? destination}`);
    collision = true;
  } catch (error) {
    if (error?.code !== "ENOENT") throw error;
    if (outputBytes) await fs.writeFile(destination, outputBytes, { flag: "wx" });
    else await fs.copyFile(source, destination, fsConstants.COPYFILE_EXCL);
  }
  return { collision, compaction, outputBytes: outputBytes?.length ?? (await fs.stat(source)).size };
}

export async function selectMapAtlasClosure(publicRoot, { pinnedManifestPath = "", pinnedContentHash = "", pinnedEnabled = false } = {}) {
  if (!path.isAbsolute(publicRoot) || (await fs.lstat(publicRoot)).isSymbolicLink()) {
    throw new Error("Map atlas public root must be an absolute real directory");
  }
  const files = new Set();
  const manifests = [];
  if (pinnedEnabled || pinnedManifestPath || pinnedContentHash) {
    const match = /^\/generated\/map-atlas\/manifest\.([a-f0-9]{64})\.json$/i.exec(pinnedManifestPath);
    if (!match || !pinnedContentHash || match[1].toLowerCase() !== pinnedContentHash.toLowerCase()) {
      throw new Error("Compiled pinned map-atlas path/hash is incomplete or inconsistent");
    }
    manifests.push(pinnedManifestPath.slice(1));
  }
  manifests.unshift("generated/map-atlas/manifest.json");
  for (const relative of manifests) {
    const raw = await readRegularPublicFile(publicRoot, relative);
    if (relative !== "generated/map-atlas/manifest.json") {
      const digest = createHash("sha256").update(raw).digest("hex");
      if (digest !== pinnedContentHash.toLowerCase()) throw new Error("Pinned map-atlas bytes do not match compiled hash");
    }
    let manifest;
    try { manifest = JSON.parse(raw.toString("utf8")); }
    catch (error) { throw new Error(`Malformed map-atlas manifest: ${relative}`, { cause: error }); }
    if (manifest?.schemaVersion !== 2 || manifest.kind !== "mir2-map-atlas-manifest" ||
        !Array.isArray(manifest.pages) || manifest.pages.length === 0) {
      throw new Error(`Invalid map-atlas manifest shape: ${relative}`);
    }
    files.add(relative);
    const pagePaths = new Set();
    const pageKeys = new Set();
    for (const page of manifest.pages) {
      if (!page || typeof page.u !== "string" ||
          !/^\/generated\/map-atlas\/[A-Za-z0-9._/-]+\.png$/.test(page.u) ||
          typeof page.l !== "string" || !page.l || !Number.isInteger(page.p) || page.p < 0) {
        throw new Error(`Invalid map-atlas page in ${relative}`);
      }
      const pageRelative = page.u.slice(1);
      segmentsOf(pageRelative);
      const pageKey = `${page.l}#${page.p}`;
      if (pagePaths.has(pageRelative) || pageKeys.has(pageKey)) {
        throw new Error(`Duplicate map-atlas page in ${relative}: ${pageRelative}`);
      }
      pagePaths.add(pageRelative);
      pageKeys.add(pageKey);
      await readRegularPublicFile(publicRoot, pageRelative, false);
      files.add(pageRelative);
    }
  }
  return { files, manifests, pages: files.size - manifests.length };
}

async function readRegularPublicFile(root, relative, readBytes = true) {
  const segments = segmentsOf(relative);
  let candidate = root;
  for (let index = 0; index < segments.length; index += 1) {
    candidate = path.join(candidate, segments[index]);
    requireInside(root, candidate);
    const stat = await fs.lstat(candidate);
    if (stat.isSymbolicLink() || (index === segments.length - 1 ? !stat.isFile() : !stat.isDirectory())) {
      throw new Error(`Map-atlas entry is not a regular path: ${relative}`);
    }
  }
  return readBytes ? fs.readFile(candidate) : null;
}

export async function auditPortableTree(root) {
  if (!path.isAbsolute(root)) throw new Error("Portable audit root must be absolute");
  const result = { exists: true, bytes: 0, files: 0, directories: 0, links: 0 };
  async function visit(candidate) {
    requireInside(root, candidate);
    const stat = await fs.lstat(candidate);
    if (stat.isSymbolicLink()) {
      result.links += 1;
      throw new Error(`Portable output retains a link: ${candidate}`);
    }
    if (stat.isFile()) {
      result.bytes += stat.size;
      result.files += 1;
    } else if (stat.isDirectory()) {
      result.directories += 1;
      for (const entry of await fs.readdir(candidate)) await visit(path.join(candidate, entry));
    } else {
      throw new Error(`Unsupported portable output entry: ${candidate}`);
    }
  }
  await visit(root);
  return result;
}

async function assertAbsent(candidate) {
  try {
    await fs.lstat(candidate);
  } catch (error) {
    if (error?.code === "ENOENT") return;
    throw error;
  }
  throw new Error(`Refusing to overwrite an existing portable output: ${candidate}`);
}

export async function copyVerifiedRequiredServerFiles({ sourcePath, destinationPath, sourceBytes }) {
  if (!path.isAbsolute(sourcePath) || !path.isAbsolute(destinationPath) || !Buffer.isBuffer(sourceBytes)) {
    throw new Error("Required-server-files copy needs absolute paths and verified source bytes");
  }
  if (path.basename(sourcePath) !== "required-server-files.json" ||
      path.basename(destinationPath) !== "required-server-files.json") {
    throw new Error("Unexpected required-server-files path");
  }
  const sourceStat = await fs.lstat(sourcePath);
  if (!sourceStat.isFile() || sourceStat.isSymbolicLink()) {
    throw new Error("Source required-server-files must be a regular file");
  }
  await fs.mkdir(path.dirname(destinationPath), { recursive: true });
  try {
    const destinationStat = await fs.lstat(destinationPath);
    if (!destinationStat.isFile() || destinationStat.isSymbolicLink()) {
      throw new Error("Standalone required-server-files must be a regular file");
    }
    const existing = await fs.readFile(destinationPath);
    if (!existing.equals(sourceBytes)) throw new Error("Standalone required-server-files differs from the verified source build");
  } catch (error) {
    if (error?.code !== "ENOENT") throw error;
    await fs.copyFile(sourcePath, destinationPath, fsConstants.COPYFILE_EXCL);
  }
  const copied = await fs.readFile(destinationPath);
  if (!copied.equals(sourceBytes)) throw new Error("Copied required-server-files differs from the verified source build");
  return JSON.parse(copied.toString("utf8"));
}

function commonRoot(a, b) {
  const aRoot = path.parse(a).root;
  const bRoot = path.parse(b).root;
  if (aRoot.toLowerCase() !== bRoot.toLowerCase()) {
    throw new Error("NFT dependency and standalone roots must share a filesystem root");
  }
  return aRoot;
}

async function routeEntries(sourceAppRoot, distDir) {
  const serverDirectory = path.join(sourceAppRoot, distDir, "server");
  const result = [path.join(sourceAppRoot, "server.js")];
  async function visit(directory) {
    for (const entry of await fs.readdir(directory, { withFileTypes: true })) {
      const candidate = path.join(directory, entry.name);
      if (entry.isSymbolicLink()) throw new Error(`Linked Next route entry: ${candidate}`);
      if (entry.isDirectory()) await visit(candidate);
      else if (entry.isFile() && /(?:^|[\\/])(?:route|page|middleware)\.js$/.test(candidate)) result.push(candidate);
    }
  }
  await visit(serverDirectory);
  return result;
}

/** Sharp's runtimePlatformArch: platform + non-glibc Linux family + architecture. */
export async function collectSharpNativePackages(dependencyRoot) {
  if (!path.isAbsolute(dependencyRoot) || (await fs.lstat(dependencyRoot)).isSymbolicLink()) {
    throw new Error("Sharp dependency root must be an absolute real directory");
  }
  const sharpRoot = path.join(dependencyRoot, "sharp");
  const sharpMetadataPath = path.join(sharpRoot, "package.json");
  if ((await fs.lstat(sharpRoot)).isSymbolicLink() || (await fs.lstat(sharpMetadataPath)).isSymbolicLink()) {
    throw new Error("Linked Sharp source metadata");
  }
  requireInside(dependencyRoot, await fs.realpath(sharpMetadataPath));
  const sharpRequire = createRequire(sharpMetadataPath);
  const sharpMetadata = JSON.parse(await fs.readFile(sharpMetadataPath, "utf8"));
  let libc = "";
  if (process.platform === "linux") {
    const detectRoot = path.join(dependencyRoot, "detect-libc");
    if ((await fs.lstat(detectRoot)).isSymbolicLink()) throw new Error("Linked detect-libc package");
    const detectPath = sharpRequire.resolve(detectRoot);
    requireInside(dependencyRoot, await fs.realpath(detectPath));
    const detectLibc = sharpRequire(detectPath);
    libc = detectLibc.isNonGlibcLinuxSync() ? detectLibc.familySync() : "";
  }
  const targetPlatform = `${process.platform}${libc}-${process.arch}`;
  if (!/^[a-z0-9-]+$/.test(targetPlatform)) throw new Error("Unsafe Sharp target platform");
  const binaryName = `@img/sharp-${targetPlatform}`;
  const companionName = `@img/sharp-libvips-${targetPlatform}`;
  const optional = sharpMetadata.optionalDependencies ?? {};
  if (typeof optional[binaryName] !== "string") {
    throw new Error(`Installed Sharp does not declare target binary ${binaryName}`);
  }
  const names = [binaryName];
  if (typeof optional[companionName] === "string") names.push(companionName);
  const scope = path.join(dependencyRoot, "@img");
  const scopeStat = await fs.lstat(scope);
  if (!scopeStat.isDirectory() || scopeStat.isSymbolicLink()) throw new Error("Linked Sharp scope directory");
  const files = new Set();
  const packages = [];
  for (const name of names) {
    const packageRelative = name.replace("/", path.sep);
    const packageRoot = path.join(dependencyRoot, packageRelative);
    let packageStat;
    try { packageStat = await fs.lstat(packageRoot); }
    catch (error) {
      if (error?.code === "ENOENT") throw new Error(`Missing target Sharp native package: ${name}`, { cause: error });
      throw error;
    }
    if (!packageStat.isDirectory() || packageStat.isSymbolicLink()) {
      throw new Error(`Sharp native package is not a real directory: ${name}`);
    }
    requireInside(dependencyRoot, await fs.realpath(packageRoot));
    let count = 0;
    let bytes = 0;
    const included = new Set();
    async function visit(directory) {
      for (const entry of await fs.readdir(directory, { withFileTypes: true })) {
        const candidate = path.join(directory, entry.name);
        requireInside(packageRoot, candidate);
        const stat = await fs.lstat(candidate);
        if (stat.isSymbolicLink()) throw new Error(`Linked Sharp native entry: ${name}`);
        requireInside(dependencyRoot, await fs.realpath(candidate));
        if (stat.isDirectory()) await visit(candidate);
        else if (stat.isFile()) {
          const relative = path.relative(dependencyRoot, candidate);
          files.add(relative);
          included.add(path.relative(packageRoot, candidate).split(path.sep).join("/"));
          count += 1;
          bytes += stat.size;
        } else throw new Error(`Unsupported Sharp native entry: ${name}`);
      }
    }
    await visit(packageRoot);
    const metadata = JSON.parse(await fs.readFile(path.join(packageRoot, "package.json"), "utf8"));
    if (metadata.name !== name || metadata.version !== optional[name] ||
        !included.has("package.json") || !included.has("LICENSE")) {
      throw new Error(`Sharp native package metadata/license mismatch: ${name}`);
    }
    if (name === binaryName) {
      const exportedBinary = metadata.exports?.["./sharp.node"];
      if (typeof exportedBinary !== "string" || !exportedBinary.startsWith("./") ||
          !included.has(segmentsOf(exportedBinary.slice(2)).join("/")) ||
          !included.has("versions.json")) {
        throw new Error(`Sharp target binary or version metadata missing: ${name}`);
      }
      let nativeBinary = exportedBinary;
      if (exportedBinary.endsWith(".cjs")) {
        const bootstrapPath = path.join(packageRoot, ...segmentsOf(exportedBinary.slice(2)));
        if (exportedBinary !== "./index.cjs" || (await fs.lstat(bootstrapPath)).size > 4096) {
          throw new Error(`Unrecognized Sharp native bootstrap: ${name}`);
        }
        const bootstrap = await fs.readFile(bootstrapPath, "utf8");
        const match = /^\s*module\.exports\s*=\s*require\((["'])(\.\/[A-Za-z0-9_./-]+\.node)\1\);?\s*$/.exec(bootstrap);
        if (!match) throw new Error(`Unrecognized Sharp native bootstrap: ${name}`);
        nativeBinary = match[2];
      }
      if (!nativeBinary.endsWith(".node") ||
          !included.has(segmentsOf(nativeBinary.slice(2)).join("/"))) {
        throw new Error(`Sharp target binary or version metadata missing: ${name}`);
      }
      if (process.platform === "win32" && ![...included].some((file) => file.endsWith(".dll"))) {
        throw new Error(`Sharp Windows package lacks libvips DLLs: ${name}`);
      }
    }
    packages.push({ name, files: count, bytes, destination: `node_modules/${name}` });
  }
  return { targetPlatform, packages, files };
}

export async function materializeStandaloneDependencies({
  sourceAppRoot, destinationAppRoot, sourceStandaloneRoot, sourceDependencyRoot, distDir, trace,
}) {
  for (const value of [sourceAppRoot, destinationAppRoot, sourceStandaloneRoot, sourceDependencyRoot]) {
    if (!path.isAbsolute(value)) throw new Error("Dependency roots must be absolute");
  }
  requireInside(sourceStandaloneRoot, sourceAppRoot);
  const dependencyRoot = await fs.realpath(sourceDependencyRoot);
  if (!(await fs.lstat(dependencyRoot)).isDirectory()) throw new Error("Dependency realpath is not a directory");
  const appDependencyRoot = await fs.realpath(path.join(sourceAppRoot, "node_modules"));
  if (appDependencyRoot !== dependencyRoot) throw new Error("Standalone dependency realpath differs from the verified root");
  const base = commonRoot(sourceAppRoot, dependencyRoot);
  const sourceRequire = createRequire(path.join(sourceAppRoot, "server.js"));
  const requiredEntries = REQUIRED_MODULES.map((moduleId) => sourceRequire.resolve(moduleId));
  const entries = [...new Set([...(await routeEntries(sourceAppRoot, distDir)), ...requiredEntries])];
  const allowed = (candidate) => !isSensitivePath(candidate) &&
    (inside(sourceStandaloneRoot, candidate) || inside(dependencyRoot, candidate));
  const guardedReal = async (candidate) => {
    const absolute = path.resolve(base, candidate);
    if (!allowed(absolute)) return null;
    try {
      const real = await fs.realpath(absolute);
      return allowed(real) ? real : null;
    } catch (error) {
      if (error?.code === "ENOENT" || error?.code === "ENOTDIR") return null;
      throw error;
    }
  };
  const guardedRead = async (candidate) => {
    const real = await guardedReal(candidate);
    if (!real) return null;
    try {
      return await fs.readFile(real);
    } catch (error) {
      if (error?.code === "ENOENT" || error?.code === "EISDIR") return null;
      throw error;
    }
  };
  const nodeFileTrace = trace ?? createRequire(path.join(dependencyRoot, "next", "package.json"))("next/dist/compiled/@vercel/nft").nodeFileTrace;
  const traced = await nodeFileTrace(entries, {
    base, processCwd: sourceAppRoot, mixedModules: true,
    readFile: guardedRead,
    stat: async (candidate) => {
      const real = await guardedReal(candidate);
      return real ? fs.stat(real) : null;
    },
    readlink: async (candidate) => {
      const real = await guardedReal(candidate);
      return real ? fs.readlink(path.resolve(base, candidate)).catch(() => null) : null;
    },
    ignore: (relative) => {
      const candidate = path.resolve(base, relative);
      if (!allowed(candidate)) return true;
      try { return !allowed(realpathSync.native(candidate)); }
      catch { return true; }
    },
  });
  const files = new Set();
  let directoryRecords = 0;
  for (const relative of traced.fileList) {
    const candidate = path.resolve(base, relative);
    if (!inside(sourceStandaloneRoot, candidate) && !inside(dependencyRoot, candidate)) {
      throw new Error(`NFT traced outside the allowed roots: ${candidate}`);
    }
    if (isSensitivePath(candidate)) throw new Error("NFT listed private dependency input");
    let real;
    try { real = await fs.realpath(candidate); }
    catch (error) { throw new Error(`NFT listed a missing dependency: ${relative}`, { cause: error }); }
    if (!inside(dependencyRoot, real)) continue;
    const stat = await fs.stat(real);
    if (stat.isDirectory()) { directoryRecords += 1; continue; }
    if (!stat.isFile()) throw new Error(`NFT listed an unsupported dependency: ${relative}`);
    files.add(path.relative(dependencyRoot, real));
  }
  for (const required of requiredEntries) {
    const real = await fs.realpath(required);
    requireInside(dependencyRoot, real);
    files.add(path.relative(dependencyRoot, real));
  }
  for (const relative of [...files]) {
    const segments = segmentsOf(relative);
    const packageSegments = segments[0]?.startsWith("@") ? segments.slice(0, 2) : segments.slice(0, 1);
    const metadata = path.join(dependencyRoot, ...packageSegments, "package.json");
    try { if ((await fs.lstat(metadata)).isFile()) files.add(path.relative(dependencyRoot, metadata)); }
    catch (error) { if (error?.code !== "ENOENT") throw error; }
  }
  const nativeClosure = await collectSharpNativePackages(dependencyRoot);
  for (const relative of nativeClosure.files) files.add(relative);
  const destinationDependencyRoot = path.join(destinationAppRoot, "node_modules");
  await assertAbsent(destinationDependencyRoot);
  await fs.mkdir(destinationDependencyRoot);
  let bytes = 0;
  let nativeBindings = 0;
  for (const relative of [...files].sort()) {
    const source = path.resolve(dependencyRoot, ...segmentsOf(relative));
    const destination = path.resolve(destinationDependencyRoot, ...segmentsOf(relative));
    requireInside(dependencyRoot, source);
    requireInside(destinationDependencyRoot, destination);
    const real = await fs.realpath(source);
    requireInside(dependencyRoot, real);
    const stat = await fs.stat(real);
    if (!stat.isFile()) throw new Error(`Dependency is not a regular file: ${relative}`);
    await fs.mkdir(path.dirname(destination), { recursive: true });
    await fs.copyFile(real, destination, fsConstants.COPYFILE_EXCL);
    bytes += stat.size;
    if (relative.endsWith(".node")) nativeBindings += 1;
  }
  const destinationRequire = createRequire(path.join(destinationAppRoot, "server.js"));
  for (const moduleId of REQUIRED_MODULES) {
    const resolved = destinationRequire.resolve(moduleId);
    requireInside(destinationDependencyRoot, resolved);
  }
  const nativeBinaryName = `@img/sharp-${nativeClosure.targetPlatform}`;
  for (const moduleId of [`${nativeBinaryName}/sharp.node`, `${nativeBinaryName}/versions`]) {
    const resolved = destinationRequire.resolve(moduleId);
    requireInside(destinationDependencyRoot, resolved);
  }
  for (const nativePackage of nativeClosure.packages) {
    const metadata = path.join(destinationDependencyRoot, ...nativePackage.name.split("/"), "package.json");
    requireInside(destinationDependencyRoot, metadata);
    if (!(await fs.lstat(metadata)).isFile()) throw new Error(`Copied Sharp package metadata missing: ${nativePackage.name}`);
  }
  const warningCodes = Object.create(null);
  const warningDetails = [];
  for (const warning of traced.warnings ?? []) {
    const code = String(warning?.code ?? warning?.name ?? "unknown");
    const message = String(warning?.message ?? "");
    warningCodes[code] = (warningCodes[code] ?? 0) + 1;
    // Keep trace diagnostics without serializing Error stacks or unbounded messages.
    warningDetails.push({ code, message: message.slice(0, 8192), messageTruncated: message.length > 8192 });
  }
  return { entries: entries.length, files: files.size, bytes, nativeBindings, directoryRecords,
    targetPlatform: nativeClosure.targetPlatform, nativePackages: nativeClosure.packages,
    nativePackageFiles: nativeClosure.packages.reduce((sum, entry) => sum + entry.files, 0),
    nativePackageBytes: nativeClosure.packages.reduce((sum, entry) => sum + entry.bytes, 0),
    warningCount: warningDetails.length, warningCodes, warningDetails };
}
