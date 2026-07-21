import { createHash } from "node:crypto";
import { createReadStream } from "node:fs";
import fs from "node:fs/promises";
import path from "node:path";

const DEFAULT_PUBLIC_ROOT = "/generated/crystal-packs/full";
const SHA256_PATTERN = /^[a-f0-9]{64}$/;

export async function inspectFullPackClosure({
  fullPackRoot,
  publicRoot = DEFAULT_PUBLIC_ROOT,
  expectedContentHash = "",
  verifyPageHashes = true,
  pageHashConcurrency = 4,
  rejectOrphans = true,
} = {}) {
  if (!fullPackRoot) throw new Error("fullPackRoot is required");
  const resolvedRoot = path.resolve(fullPackRoot);
  const normalizedPublicRoot = normalizePublicRoot(publicRoot);
  const indexPath = path.join(resolvedRoot, "index.json");
  const indexBytes = await fs.readFile(indexPath);
  const index = JSON.parse(indexBytes.toString("utf8"));

  if (index?.schemaVersion !== 1 || index?.kind !== "mir2-crystal-full-pack-index") {
    throw new Error(`Invalid full Crystal pack index: ${indexPath}`);
  }
  if (!SHA256_PATTERN.test(String(index.contentHash ?? ""))) {
    throw new Error(`Full Crystal pack index has an invalid contentHash: ${indexPath}`);
  }
  if (expectedContentHash && index.contentHash !== expectedContentHash) {
    throw new Error(`Full Crystal pack content hash mismatch: expected ${expectedContentHash}, found ${index.contentHash}`);
  }

  const libraryRecords = Array.isArray(index.libraries) ? index.libraries : [];
  const expectedLibraryCount = Number(index.summary?.libraryCount ?? libraryRecords.length);
  if (!expectedLibraryCount || libraryRecords.length !== expectedLibraryCount) {
    throw new Error(
      `Full Crystal pack index library count mismatch: expected ${expectedLibraryCount}, found ${libraryRecords.length}`,
    );
  }

  const libraries = new Map();
  const pages = new Map();
  for (const record of libraryRecords) {
    const manifestHash = String(record?.manifestSha256 ?? "").toLowerCase();
    if (!SHA256_PATTERN.test(manifestHash)) {
      throw new Error(`Invalid manifestSha256 for full-pack library ${record?.key ?? "(unknown)"}`);
    }
    if (record?.shardUrl && record.shardUrl !== record.manifestUrl) {
      throw new Error(`Full-pack shardUrl/manifestUrl mismatch for ${record?.key ?? "(unknown)"}`);
    }

    const manifestFile = resolvePackUrl({
      fullPackRoot: resolvedRoot,
      publicRoot: normalizedPublicRoot,
      url: record?.manifestUrl,
      requiredPrefix: "libraries/",
      extension: ".json",
      label: `library ${record?.key ?? "(unknown)"}`,
    });
    if (libraries.has(manifestFile.relativePath)) {
      throw new Error(`Duplicate full-pack library path: ${manifestFile.publicPath}`);
    }

    const manifestBytes = await fs.readFile(manifestFile.absolutePath);
    const actualManifestHash = sha256(manifestBytes);
    if (actualManifestHash !== manifestHash) {
      throw new Error(
        `Full-pack library hash mismatch: ${manifestFile.publicPath} expected ${manifestHash}, found ${actualManifestHash}`,
      );
    }
    const manifest = JSON.parse(manifestBytes.toString("utf8"));
    const manifestKey = String(manifest.libraryKey ?? manifest.key ?? "");
    if (manifestKey !== String(record.key ?? "")) {
      throw new Error(`Full-pack library key mismatch: index=${record.key} manifest=${manifestKey}`);
    }

    const manifestPages = Array.isArray(manifest.pages) ? manifest.pages : [];
    if (Number(record.pageCount ?? manifestPages.length) !== manifestPages.length) {
      throw new Error(
        `Full-pack page count mismatch for ${record.key}: index=${record.pageCount}, manifest=${manifestPages.length}`,
      );
    }
    for (const page of manifestPages) {
      const pageHash = String(page?.sha256 ?? "").toLowerCase();
      if (!SHA256_PATTERN.test(pageHash) || page?.key !== `sha256:${pageHash}`) {
        throw new Error(`Invalid full-pack page hash/key in ${record.key}`);
      }
      const pageFile = resolvePackUrl({
        fullPackRoot: resolvedRoot,
        publicRoot: normalizedPublicRoot,
        url: page?.imageUrl,
        requiredPrefix: "pages/",
        extension: ".png",
        label: `page ${pageHash}`,
      });
      const expectedRelativePath = `pages/${pageHash.slice(0, 2)}/${pageHash}.png`;
      if (pageFile.relativePath !== expectedRelativePath) {
        throw new Error(`Full-pack page path does not match its hash: ${pageFile.publicPath}`);
      }
      const expectedSize = Number(page.networkBytes ?? 0);
      if (!Number.isSafeInteger(expectedSize) || expectedSize <= 0) {
        throw new Error(`Invalid full-pack page size for ${pageFile.publicPath}`);
      }
      const existingPage = pages.get(pageFile.relativePath);
      if (existingPage) {
        if (existingPage.sha256 !== pageHash || existingPage.size !== expectedSize) {
          throw new Error(`Conflicting full-pack page metadata: ${pageFile.publicPath}`);
        }
      } else {
        pages.set(pageFile.relativePath, {
          ...pageFile,
          kind: "page",
          sha256: pageHash,
          size: expectedSize,
        });
      }
    }

    libraries.set(manifestFile.relativePath, {
      ...manifestFile,
      kind: "library",
      sha256: manifestHash,
      size: manifestBytes.byteLength,
    });
  }

  const libraryFiles = [...libraries.values()].sort(compareRelativePath);
  const pageFiles = [...pages.values()].sort(compareRelativePath);
  await validateExactDirectoryFiles({
    directory: path.join(resolvedRoot, "libraries"),
    fullPackRoot: resolvedRoot,
    expected: new Set(libraryFiles.map((file) => file.relativePath)),
    extension: ".json",
    label: "library",
    rejectOrphans,
  });
  await validateExactDirectoryFiles({
    directory: path.join(resolvedRoot, "pages"),
    fullPackRoot: resolvedRoot,
    expected: new Set(pageFiles.map((file) => file.relativePath)),
    extension: ".png",
    label: "page",
    rejectOrphans,
  });

  for (const pageFile of pageFiles) {
    const stats = await fs.stat(pageFile.absolutePath);
    if (!stats.isFile() || stats.size !== pageFile.size) {
      throw new Error(
        `Full-pack page size mismatch: ${pageFile.publicPath} expected ${pageFile.size}, found ${stats.size}`,
      );
    }
  }
  if (verifyPageHashes) {
    await runPool(pageFiles, pageHashConcurrency, async (pageFile) => {
      const actualHash = await sha256File(pageFile.absolutePath);
      if (actualHash !== pageFile.sha256) {
        throw new Error(
          `Full-pack page hash mismatch: ${pageFile.publicPath} expected ${pageFile.sha256}, found ${actualHash}`,
        );
      }
    });
  }

  const indexFile = {
    absolutePath: indexPath,
    relativePath: "index.json",
    publicPath: `${normalizedPublicRoot}/index.json`,
    kind: "index",
    sha256: sha256(indexBytes),
    size: indexBytes.byteLength,
  };
  return {
    index,
    contentHash: index.contentHash,
    sourceContentHash: index.sourceContentHash ?? null,
    libraryCount: libraryFiles.length,
    pageCount: pageFiles.length,
    fileCount: 1 + libraryFiles.length + pageFiles.length,
    indexFile,
    libraryFiles,
    pageFiles,
    files: [indexFile, ...libraryFiles, ...pageFiles],
    pageHashesVerified: verifyPageHashes,
  };
}

export async function sha256File(filePath) {
  const hash = createHash("sha256");
  const stream = createReadStream(filePath);
  for await (const chunk of stream) hash.update(chunk);
  return hash.digest("hex");
}

function resolvePackUrl({ fullPackRoot, publicRoot, url, requiredPrefix, extension, label }) {
  const value = String(url ?? "");
  if (!value.startsWith(`${publicRoot}/`) || value.includes("?") || value.includes("#") || value.includes("\\")) {
    throw new Error(`Invalid full-pack URL for ${label}: ${value}`);
  }
  const relativePath = value.slice(publicRoot.length + 1);
  const segments = relativePath.split("/");
  if (
    !relativePath.startsWith(requiredPrefix) ||
    path.posix.extname(relativePath).toLowerCase() !== extension ||
    segments.some((segment) => !segment || segment === "." || segment === "..")
  ) {
    throw new Error(`Invalid full-pack path for ${label}: ${value}`);
  }
  const absolutePath = path.resolve(fullPackRoot, ...segments);
  if (!isPathInside(absolutePath, fullPackRoot)) {
    throw new Error(`Full-pack path escapes its root for ${label}: ${value}`);
  }
  return { absolutePath, relativePath, publicPath: value };
}

async function validateExactDirectoryFiles({ directory, fullPackRoot, expected, extension, label, rejectOrphans }) {
  const actualPaths = await listRegularFiles(directory, fullPackRoot, extension);
  const actual = new Set(actualPaths);
  const missing = [...expected].filter((value) => !actual.has(value));
  const orphaned = rejectOrphans ? actualPaths.filter((value) => !expected.has(value)) : [];
  if (missing.length || orphaned.length) {
    throw new Error(
      `Full-pack ${label} closure mismatch: missing=${missing.slice(0, 3).join(",") || "none"} ` +
        `orphaned=${orphaned.slice(0, 3).join(",") || "none"}`,
    );
  }
}

async function listRegularFiles(root, fullPackRoot, extension) {
  const files = [];
  const stack = [root];
  while (stack.length) {
    const current = stack.pop();
    const entries = await fs.readdir(current, { withFileTypes: true });
    for (const entry of entries) {
      const entryPath = path.join(current, entry.name);
      if (entry.isSymbolicLink()) throw new Error(`Symbolic links are not allowed in full pack: ${entryPath}`);
      if (entry.isDirectory()) {
        stack.push(entryPath);
      } else if (entry.isFile() && path.extname(entry.name).toLowerCase() === extension) {
        files.push(path.relative(fullPackRoot, entryPath).split(path.sep).join("/"));
      } else if (!entry.isFile()) {
        throw new Error(`Special files are not allowed in full pack: ${entryPath}`);
      }
    }
  }
  return files.sort((left, right) => left.localeCompare(right));
}

async function runPool(items, concurrency, worker) {
  let nextIndex = 0;
  async function next() {
    while (nextIndex < items.length) {
      const index = nextIndex;
      nextIndex += 1;
      await worker(items[index], index);
    }
  }
  await Promise.all(Array.from({ length: Math.min(Math.max(1, concurrency), items.length || 1) }, next));
}

function normalizePublicRoot(value) {
  const normalized = String(value ?? "").trim().replace(/\/+$/, "");
  if (!normalized.startsWith("/") || normalized.includes("..") || normalized.includes("\\")) {
    throw new Error(`Invalid full-pack public root: ${value}`);
  }
  return normalized;
}

function isPathInside(candidate, root) {
  const relative = path.relative(root, candidate);
  return relative === "" || (!!relative && !relative.startsWith("..") && !path.isAbsolute(relative));
}

function compareRelativePath(left, right) {
  return left.relativePath.localeCompare(right.relativePath);
}

function sha256(value) {
  return createHash("sha256").update(value).digest("hex");
}
