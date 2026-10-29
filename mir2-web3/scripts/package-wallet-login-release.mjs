import { createHash } from "node:crypto";
import { spawnSync } from "node:child_process";
import { readdirSync } from "node:fs";
import { cp, lstat, mkdir, mkdtemp, readFile, readdir, rename, rm, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const projectRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const appRoot = path.join(projectRoot, "apps/wallet-login");
const outputRoot = path.join(projectRoot, "dist/wallet-login-releases");
const serverEntry = "mir2-web3/apps/wallet-login/server.js";

function command(executable, args, options = {}) {
  const result = spawnSync(executable, args, {
    cwd: projectRoot, encoding: "utf8", windowsHide: true, ...options,
  });
  if (result.error || result.status !== 0) {
    throw new Error(`wallet-login command failed: ${path.basename(executable)} (exit ${result.status ?? "unavailable"})`);
  }
  return result.stdout?.trim() ?? "";
}

export function requireChild(root, candidate) {
  const relative = path.relative(path.resolve(root), path.resolve(candidate));
  if (!relative || relative.startsWith(`..${path.sep}`) || relative === ".." || path.isAbsolute(relative)) {
    throw new Error("package path escapes its intended directory");
  }
}

export function buildEnvironment(environment, revision) {
  const allowed = /^(PATH|SYSTEMROOT|WINDIR|COMSPEC|PATHEXT|TEMP|TMP|TMPDIR|HOME|USERPROFILE|LOCALAPPDATA|APPDATA|LANG|LC_ALL|TERM)$/i;
  const result = Object.fromEntries(Object.entries(environment).filter(([name]) => allowed.test(name)));
  return {
    ...result,
    NODE_ENV: "production",
    NEXT_TELEMETRY_DISABLED: "1",
    MIR2_WALLET_LOGIN_BUILD_REVISION: revision,
  };
}

export async function inspectPayload(root) {
  const files = [];
  async function visit(directory) {
    for (const entry of (await readdir(directory)).sort()) {
      const absolute = path.join(directory, entry);
      requireChild(root, absolute);
      const relative = path.relative(root, absolute).split(path.sep).join("/");
      const metadata = await lstat(absolute);
      if (metadata.isSymbolicLink() || (!metadata.isDirectory() && !metadata.isFile())) {
        throw new Error("standalone payload contains a link or special file");
      }
      if (/^\.env(?:\.|$)/i.test(entry)
        || /(^|\/)\.git(\/|$)/.test(relative)
        || /(^|\/)apps\/web\/public(\/|$)/.test(relative)) {
        throw new Error("standalone payload contains private environment or game assets");
      }
      if (metadata.isDirectory()) await visit(absolute);
      else {
        const bytes = await readFile(absolute);
        files.push({ path: relative, sizeBytes: bytes.length, sha256: createHash("sha256").update(bytes).digest("hex") });
      }
    }
  }
  await visit(root);
  if (files.length > 20_000 || files.reduce((sum, file) => sum + file.sizeBytes, 0) > 400 * 1024 * 1024) {
    throw new Error("standalone payload exceeds the wallet service size boundary");
  }
  return files;
}

export function verifySourceRevision(revision) {
  if (!/^[a-f0-9]{40}$/.test(revision ?? "")) throw new Error("--revision requires a full lowercase source SHA");
  const actual = command("git", ["rev-parse", "HEAD"]);
  if (actual !== revision) throw new Error("requested release revision differs from checked-out HEAD");
  if (command("git", ["status", "--porcelain=v1", "--untracked-files=no"])) {
    throw new Error("wallet-login release requires a clean tracked checkout");
  }
  const required = [
    "apps/wallet-login/package.json", "apps/wallet-login/package-lock.json", "apps/wallet-login/next.config.ts",
    "scripts/package-wallet-login-release.mjs", "scripts/test-wallet-login-service.mjs",
    "apps/web/lib/server/dubhe-connect-auth.ts", "apps/web/lib/server/passkey-auth.ts",
    "apps/web/app/api/channels/session/exchange/route.ts",
  ];
  command("git", ["ls-files", "--error-unmatch", "--", ...required]);
  if (command("git", ["status", "--porcelain=v1", "--untracked-files=all", "--", "apps/wallet-login"])) {
    throw new Error("wallet-login source contains uncommitted or untracked inputs");
  }
  if (readdirSync(appRoot).some((name) => /^\.env(?:\.|$)/i.test(name))) {
    throw new Error("wallet-login release refuses local environment files before build");
  }
}

export async function packageRelease(revision) {
  verifySourceRevision(revision);
  if (Number(process.versions.node.split(".")[0]) < 22) throw new Error("wallet-login packaging requires Node 22 or newer");
  const lock = JSON.parse(await readFile(path.join(appRoot, "package-lock.json"), "utf8"));
  const webLock = JSON.parse(await readFile(path.join(projectRoot, "apps/web/package-lock.json"), "utf8"));
  for (const dependency of ["@mysten/sui", "next", "react", "react-dom", "typescript"]) {
    if (lock.packages[`node_modules/${dependency}`]?.version !== webLock.packages[`node_modules/${dependency}`]?.version) {
      throw new Error(`wallet-login dependency differs from shared Web lock: ${dependency}`);
    }
  }
  command(process.execPath, [path.join(appRoot, "node_modules/next/dist/bin/next"), "build", "--webpack"], {
    cwd: appRoot, env: buildEnvironment(process.env, revision), stdio: "inherit",
  });
  verifySourceRevision(revision);
  await mkdir(outputRoot, { recursive: true });
  const stageRoot = await mkdtemp(path.join(outputRoot, ".wallet-login-stage-"));
  requireChild(outputRoot, stageRoot);
  // cp(errorOnExist) needs a fresh destination on Linux as well as Windows.
  const stage = path.join(stageRoot, "payload");
  const archiveName = `mir2-wallet-login-${revision}.tar.gz`;
  const archivePath = path.join(outputRoot, archiveName);
  const temporaryArchive = `${stageRoot}.tar.gz`;
  try {
    const standaloneRoot = path.join(appRoot, ".next/standalone");
    // Validate the build output before copying so source links cannot be followed.
    await inspectPayload(standaloneRoot);
    await cp(standaloneRoot, stage, { recursive: true, force: false, errorOnExist: true });
    const staticRoot = path.join(appRoot, ".next/static");
    await cp(staticRoot, path.join(stage, "mir2-web3/apps/wallet-login/.next/static"), { recursive: true });
    const files = await inspectPayload(stage);
    if (!files.some((file) => file.path === serverEntry)) throw new Error("standalone server entry is absent");
    const metadata = {
      schema: "mir2.wallet-login.release.v1",
      sourceRevision: revision,
      revisionSource: "git rev-parse HEAD; clean tracked tree plus tracked wallet source verified before and after build",
      nodeMajor: Number(process.versions.node.split(".")[0]),
      buildCommand: "next build --webpack",
      serverEntry,
      listen: { hostname: "127.0.0.1", configuredBy: ["HOSTNAME", "PORT"] },
      lockSha256: createHash("sha256").update(await readFile(path.join(appRoot, "package-lock.json"))).digest("hex"),
      files,
    };
    await writeFile(path.join(stage, "RELEASE.json"), `${JSON.stringify(metadata, null, 2)}\n`, { flag: "wx" });
    command("tar", ["-czf", temporaryArchive, "-C", stage, "."]);
    const archiveBytes = await readFile(temporaryArchive);
    const archiveSha256 = createHash("sha256").update(archiveBytes).digest("hex");
    await rename(temporaryArchive, archivePath);
    await writeFile(`${archivePath}.sha256`, `${archiveSha256}  ${archiveName}\n`);
    const latest = {
      schema: metadata.schema, sourceRevision: revision, archive: archiveName,
      sizeBytes: archiveBytes.length, sha256: archiveSha256, serverEntry,
    };
    await writeFile(path.join(outputRoot, "latest-wallet-login-release.json"), `${JSON.stringify(latest, null, 2)}\n`);
    console.log(JSON.stringify(latest));
    return latest;
  } finally {
    requireChild(outputRoot, stageRoot);
    await rm(stageRoot, { recursive: true, force: true });
    requireChild(outputRoot, temporaryArchive);
    await rm(temporaryArchive, { force: true });
  }
}

if (process.argv[1] && pathToFileURL(path.resolve(process.argv[1])).href === import.meta.url) {
  try {
    if (process.argv.length !== 4 || process.argv[2] !== "--revision") throw new Error("usage: node scripts/package-wallet-login-release.mjs --revision <full-source-SHA>");
    await packageRelease(process.argv[3]);
  } catch (error) {
    console.error(error instanceof Error ? error.message : "wallet-login packaging failed");
    process.exitCode = 1;
  }
}
