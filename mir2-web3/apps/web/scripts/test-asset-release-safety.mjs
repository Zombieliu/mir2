import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { createHash } from "node:crypto";
import fs from "node:fs/promises";
import http from "node:http";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { constants as zlibConstants, gzipSync } from "node:zlib";

import { createCasRelease, writeCasReleaseArtifacts } from "./asset-pipeline/cas-release.mjs";

const SCRIPT_DIR = path.dirname(fileURLToPath(import.meta.url));
const BUILD_SCRIPT = path.join(SCRIPT_DIR, "build-remote-asset-release.mjs");
const UPLOAD_SCRIPT = path.join(SCRIPT_DIR, "upload-r2-assets.mjs");
const CAS_RELEASE_MODULE = path.join(SCRIPT_DIR, "asset-pipeline", "cas-release.mjs");
const FULL_PACK_CLOSURE_MODULE = path.join(SCRIPT_DIR, "asset-pipeline", "full-pack-closure.mjs");

await test("map-atlas pages accompany a referenced map-atlas manifest", async () => {
  await withTempDir(async (root) => {
    const fixtureScript = path.join(root, "apps", "web", "scripts", path.basename(BUILD_SCRIPT));
    const publicRoot = path.join(root, "apps", "web", "public");
    const atlasManifestPath = path.join(publicRoot, "generated", "map-atlas", "manifest.json");
    await fs.mkdir(path.dirname(fixtureScript), { recursive: true });
    await fs.mkdir(path.join(path.dirname(fixtureScript), "asset-pipeline"), { recursive: true });
    await fs.mkdir(path.dirname(atlasManifestPath), { recursive: true });
    await fs.mkdir(path.join(publicRoot, "original-map"), { recursive: true });
    await fs.copyFile(BUILD_SCRIPT, fixtureScript);
    await fs.copyFile(CAS_RELEASE_MODULE, path.join(path.dirname(fixtureScript), "asset-pipeline", "cas-release.mjs"));
    await fs.copyFile(
      FULL_PACK_CLOSURE_MODULE,
      path.join(path.dirname(fixtureScript), "asset-pipeline", "full-pack-closure.mjs"),
    );
    await fs.writeFile(path.join(publicRoot, "original-map", "fixture.png"), "original");
    await fs.writeFile(
      path.join(publicRoot, "original-asset-manifest.generated.json"),
      JSON.stringify({ schemaVersion: 1, assets: { "/original-map/fixture.png": {} } }),
    );
    await fs.writeFile(
      atlasManifestPath,
      JSON.stringify({
        schemaVersion: 1,
        kind: "mir2-map-atlas-manifest",
        atlases: [
          { imageUrl: "/generated/map-atlas/library/p0.png" },
          { imageUrl: "/generated/map-atlas/library/p1.png" },
        ],
      }),
    );
    await fs.mkdir(path.join(publicRoot, "generated", "map-atlas", "library"), { recursive: true });
    await fs.writeFile(path.join(publicRoot, "generated", "map-atlas", "library", "p0.png"), "page-0");
    await fs.writeFile(path.join(publicRoot, "generated", "map-atlas", "library", "p1.png"), "page-1");

    const server = await listen((request, response) => {
      if (request.url === "/api/asset-manifest") {
        response.setHeader("content-type", "application/json");
        response.end(JSON.stringify({
          version: "test-atlas-release",
          resourcePacks: [{
            name: "map",
            priority: 1,
            urls: ["/generated/map-atlas/manifest.json"],
          }],
        }));
        return;
      }
      response.statusCode = 404;
      response.end();
    });

    try {
      const outputDir = path.join(root, "release-output");
      await runNode(fixtureScript, [
        "--baseUrl", server.url,
        "--outDir", outputDir,
        "--stageDir", path.join(root, "stage"),
        "--stageFileMode", "reference",
        "--hashMode", "skip",
        "--cas", "false",
        "--includeSceneSprites", "false",
        "--includePublicAssetRoots", "false",
        "--allowMissing", "true",
      ]);
      const release = JSON.parse(await fs.readFile(path.join(outputDir, "remote-asset-release.json"), "utf8"));
      const files = new Map(release.files.map((file) => [file.path, file]));
      assert.ok(files.has("/generated/map-atlas/manifest.json"));
      assert.deepEqual(
        ["/generated/map-atlas/library/p0.png", "/generated/map-atlas/library/p1.png"].filter((item) => files.has(item)),
        ["/generated/map-atlas/library/p0.png", "/generated/map-atlas/library/p1.png"],
      );
      assert.deepEqual(files.get("/generated/map-atlas/library/p0.png").sources, ["map-atlas-manifest"]);
    } finally {
      await server.close();
    }
  });
});

await test("verified full Crystal pack files are included only when explicitly requested", async () => {
  await withTempDir(async (root) => {
    const fixtureScript = path.join(root, "apps", "web", "scripts", path.basename(BUILD_SCRIPT));
    const publicRoot = path.join(root, "apps", "web", "public");
    const fullRoot = path.join(publicRoot, "generated", "crystal-packs", "full");
    const coveragePath = path.join(
      root,
      "docs",
      "generated",
      "assets",
      "crystal-full-pack-coverage.generated.json",
    );
    await fs.mkdir(path.dirname(fixtureScript), { recursive: true });
    await fs.mkdir(path.join(path.dirname(fixtureScript), "asset-pipeline"), { recursive: true });
    await fs.mkdir(path.join(publicRoot, "original-map"), { recursive: true });
    await fs.mkdir(path.join(fullRoot, "libraries", "entities"), { recursive: true });
    await fs.mkdir(path.join(fullRoot, "libraries", "ui"), { recursive: true });
    await fs.mkdir(path.dirname(coveragePath), { recursive: true });
    await fs.copyFile(BUILD_SCRIPT, fixtureScript);
    await fs.copyFile(CAS_RELEASE_MODULE, path.join(path.dirname(fixtureScript), "asset-pipeline", "cas-release.mjs"));
    await fs.copyFile(
      FULL_PACK_CLOSURE_MODULE,
      path.join(path.dirname(fixtureScript), "asset-pipeline", "full-pack-closure.mjs"),
    );
    await fs.writeFile(path.join(publicRoot, "original-map", "fixture.png"), "original");
    await fs.writeFile(
      path.join(publicRoot, "original-asset-manifest.generated.json"),
      JSON.stringify({ schemaVersion: 1, assets: { "/original-map/fixture.png": {} } }),
    );
    const contentHash = "f".repeat(64);
    const sourceContentHash = "e".repeat(64);
    const pageABytes = Buffer.from("page-a");
    const pageBBytes = Buffer.from("page-b");
    const pageAHash = createHash("sha256").update(pageABytes).digest("hex");
    const pageBHash = createHash("sha256").update(pageBBytes).digest("hex");
    const pageAUrl = `/generated/crystal-packs/full/pages/${pageAHash.slice(0, 2)}/${pageAHash}.png`;
    const pageBUrl = `/generated/crystal-packs/full/pages/${pageBHash.slice(0, 2)}/${pageBHash}.png`;
    const npcUrl = "/generated/crystal-packs/full/libraries/entities/npc.json";
    const prguseUrl = "/generated/crystal-packs/full/libraries/ui/prguse.json";
    const npcManifest = JSON.stringify({
      libraryKey: "NPC/00",
      pages: [{
        key: `sha256:${pageAHash}`,
        sha256: pageAHash,
        imageUrl: pageAUrl,
        networkBytes: pageABytes.byteLength,
      }],
    });
    const prguseManifest = JSON.stringify({
      libraryKey: "Prguse",
      pages: [{
        key: `sha256:${pageBHash}`,
        sha256: pageBHash,
        imageUrl: pageBUrl,
        networkBytes: pageBBytes.byteLength,
      }],
    });
    const npcManifestHash = createHash("sha256").update(npcManifest).digest("hex");
    const prguseManifestHash = createHash("sha256").update(prguseManifest).digest("hex");
    await fs.writeFile(path.join(fullRoot, "index.json"), JSON.stringify({
      schemaVersion: 1,
      kind: "mir2-crystal-full-pack-index",
      contentHash,
      sourceContentHash,
      summary: { libraryCount: 2 },
      libraries: [
        { key: "NPC/00", pageCount: 1, manifestUrl: npcUrl, shardUrl: npcUrl, manifestSha256: npcManifestHash },
        { key: "Prguse", pageCount: 1, manifestUrl: prguseUrl, shardUrl: prguseUrl, manifestSha256: prguseManifestHash },
      ],
    }));
    await fs.writeFile(
      coveragePath,
      JSON.stringify({
        schemaVersion: 1,
        kind: "mir2-crystal-full-pack-coverage",
        mode: "verify",
        evidence: {
          contentHash,
          pageHashesVerified: true,
          verifiedLibraryCount: 2,
          verifiedUniquePageCount: 2,
        },
      }),
    );
    await fs.writeFile(path.join(fullRoot, "libraries", "entities", "npc.json"), npcManifest);
    await fs.writeFile(path.join(fullRoot, "libraries", "ui", "prguse.json"), prguseManifest);
    await fs.mkdir(path.join(fullRoot, "pages", pageAHash.slice(0, 2)), { recursive: true });
    await fs.mkdir(path.join(fullRoot, "pages", pageBHash.slice(0, 2)), { recursive: true });
    await fs.writeFile(path.join(fullRoot, pageAUrl.split("/full/")[1]), pageABytes);
    await fs.writeFile(path.join(fullRoot, pageBUrl.split("/full/")[1]), pageBBytes);

    const outputDir = path.join(root, "release-output");
    await runNode(fixtureScript, [
      "--offlineManifest", "true",
      "--assetVersion", "full-pack-fixture",
      "--outDir", outputDir,
      "--stageDir", path.join(root, "stage"),
      "--stageFileMode", "reference",
      "--hashMode", "sha256",
      "--cas", "false",
      "--includeSceneSprites", "false",
      "--includePublicAssetRoots", "false",
      "--includeFullCrystalPack", "true",
      "--allowMissing", "true",
    ]);

    const release = JSON.parse(await fs.readFile(path.join(outputDir, "remote-asset-release.json"), "utf8"));
    const files = new Map(release.files.map((file) => [file.path, file]));
    assert.deepEqual(release.fullCrystalPack, {
      enabled: true,
      verified: true,
      path: "/generated/crystal-packs/full/index.json",
      contentHash,
      sourceContentHash,
      libraryCount: 2,
      pageCount: 2,
      fileCount: 5,
      jsonContentEncoding: "gzip",
    });
    assert.ok(files.has("/generated/crystal-packs/full/index.json"));
    assert.ok(files.has("/generated/crystal-packs/full/libraries/entities/npc.json"));
    assert.ok(files.has("/generated/crystal-packs/full/libraries/ui/prguse.json"));
    assert.ok(files.has(pageAUrl));
    assert.ok(files.has(pageBUrl));
    for (const jsonPath of [
      "/generated/crystal-packs/full/index.json",
      "/generated/crystal-packs/full/libraries/entities/npc.json",
      "/generated/crystal-packs/full/libraries/ui/prguse.json",
    ]) {
      const file = files.get(jsonPath);
      assert.equal(file.contentEncoding, "gzip");
      assert.ok(file.encodedSize > 0);
      assert.match(file.encodedSha256, /^[a-f0-9]{64}$/);
    }
    assert.equal(release.stats.encodedFileCount, 3);
    assert.ok(release.stats.storageBytes < release.stats.totalBytes);
    assert.ok(release.stats.storageSavingsBytes > 0);
    assert.deepEqual(
      files.get(pageAUrl).sources,
      ["full-crystal-pack:page"],
    );

    const orphanDir = path.join(fullRoot, "pages", "ff");
    await fs.mkdir(orphanDir, { recursive: true });
    await fs.writeFile(path.join(orphanDir, "orphan.png"), "orphan");
    await assert.rejects(
      runNode(fixtureScript, [
        "--offlineManifest", "true",
        "--assetVersion", "full-pack-orphan-fixture",
        "--outDir", path.join(root, "release-output-orphan"),
        "--stageDir", path.join(root, "stage-orphan"),
        "--stageFileMode", "reference",
        "--hashMode", "sha256",
        "--cas", "false",
        "--includeSceneSprites", "false",
        "--includePublicAssetRoots", "false",
        "--includeFullCrystalPack", "true",
        "--allowMissing", "true",
      ]),
      /closure mismatch/,
    );
  });
});

await test("release manifest upload starts after every referenced asset upload completes", async () => {
  await withTempDir(async (root) => {
    const events = [];
    const server = await listen((request, response) => {
      const key = new URL(request.url, server.url).searchParams.get("key");
      events.push({ type: "start", key });
      request.resume();
      request.on("end", () => {
        const delay = key.endsWith("asset-a.bin") ? 80 : key.endsWith("asset-b.bin") ? 30 : 0;
        setTimeout(() => {
          events.push({ type: "complete", key });
          response.end("ok");
        }, delay);
      });
    });

    try {
      const assetA = path.join(root, "asset-a.bin");
      const assetB = path.join(root, "asset-b.bin");
      const manifestPath = path.join(root, "release.json");
      await fs.writeFile(assetA, "a");
      await fs.writeFile(assetB, "b");
      const files = [
        {
          relativePath: "asset-a.bin",
          stagePath: assetA,
          size: 1,
          sha256: createHash("sha256").update("a").digest("hex"),
          contentType: "application/octet-stream",
        },
        {
          relativePath: "asset-b.bin",
          stagePath: assetB,
          size: 1,
          sha256: createHash("sha256").update("b").digest("hex"),
          contentType: "application/octet-stream",
        },
      ];
      const cas = await writeCasReleaseArtifacts(
        createCasRelease(files, { prefix: "release/cas", channel: "candidate" }),
        root,
      );
      await fs.writeFile(manifestPath, JSON.stringify({
        objectPrefix: "release/v1",
        files,
        cas,
      }));

      await runNode(UPLOAD_SCRIPT, [
        "--manifest", manifestPath,
        "--bucket", "fixture",
        "--driver", "worker",
        "--workerUrl", server.url,
        "--concurrency", "2",
        "--maxAttempts", "1",
      ], { MIR2_R2_UPLOAD_SECRET: "fixture-secret" });

      const manifestKey = "release/v1/remote-asset-release.json";
      const manifestStart = events.findIndex((event) => event.type === "start" && event.key === manifestKey);
      assert.ok(manifestStart >= 0, "release manifest was uploaded");
      for (const assetKey of ["release/v1/asset-a.bin", "release/v1/asset-b.bin"]) {
        const completion = events.findIndex((event) => event.type === "complete" && event.key === assetKey);
        assert.ok(completion >= 0, `${assetKey} completed`);
        assert.ok(completion < manifestStart, `${assetKey} completed before release manifest upload started`);
      }

      const casManifestKey = cas.manifest.objectKey;
      const channelKey = cas.channel.objectKey;
      const casManifestStart = events.findIndex((event) => event.type === "start" && event.key === casManifestKey);
      const channelStart = events.findIndex((event) => event.type === "start" && event.key === channelKey);
      assert.ok(casManifestStart > 0, "immutable CAS manifest was uploaded");
      assert.ok(channelStart > casManifestStart, "mutable channel upload started after immutable CAS manifest");
      for (const key of [casManifestKey, manifestKey]) {
        const completion = events.findIndex((event) => event.type === "complete" && event.key === key);
        assert.ok(completion >= 0, `${key} completed`);
        assert.ok(completion < channelStart, `${key} completed before mutable channel upload started`);
      }
    } finally {
      await server.close();
    }
  });
});

await test("r2-s3 uploads deterministic gzip bytes with matching metadata", async () => {
  await withTempDir(async (root) => {
    const requests = [];
    const server = await listen((request, response) => {
      const chunks = [];
      request.on("data", (chunk) => chunks.push(chunk));
      request.on("end", () => {
        requests.push({
          method: request.method,
          url: request.url,
          headers: request.headers,
          body: Buffer.concat(chunks),
        });
        response.statusCode = 200;
        response.setHeader("etag", '"fixture"');
        response.end();
      });
    });

    try {
      const stagePath = path.join(root, "index.json");
      const manifestPath = path.join(root, "release.json");
      const raw = Buffer.from(JSON.stringify({
        kind: "mir2-crystal-full-pack-index",
        libraries: Array.from({ length: 100 }, (_, index) => ({ key: `Library/${index}` })),
      }));
      const encoded = gzipSync(raw, {
        level: zlibConstants.Z_BEST_COMPRESSION,
        mtime: 0,
      });
      await fs.writeFile(stagePath, raw);
      await fs.writeFile(manifestPath, JSON.stringify({
        objectPrefix: "mir2/v/gzip-fixture",
        fullCrystalPack: { enabled: true },
        files: [{
          relativePath: "generated/crystal-packs/full/index.json",
          stagePath,
          size: raw.byteLength,
          sha256: createHash("sha256").update(raw).digest("hex"),
          contentType: "application/json; charset=utf-8",
          contentEncoding: "gzip",
          encodedSize: encoded.byteLength,
          encodedSha256: createHash("sha256").update(encoded).digest("hex"),
        }],
      }));

      await runNode(UPLOAD_SCRIPT, [
        "--manifest", manifestPath,
        "--bucket", "fixture",
        "--driver", "r2-s3",
        "--s3Endpoint", server.url,
        "--s3AccessKeyId", "fixture-access",
        "--s3SecretAccessKey", "fixture-secret",
        "--includeReleaseManifest", "false",
        "--maxAttempts", "1",
      ]);

      assert.equal(requests.length, 1);
      assert.equal(requests[0].method, "PUT");
      assert.equal(requests[0].headers["content-encoding"], "gzip");
      assert.equal(Number(requests[0].headers["content-length"]), encoded.byteLength);
      assert.deepEqual(requests[0].body, encoded);
    } finally {
      await server.close();
    }
  });
});

console.log("asset release safety tests passed (4/4)");

async function test(name, fn) {
  try {
    await fn();
    console.log(`ok - ${name}`);
  } catch (error) {
    console.error(`not ok - ${name}`);
    throw error;
  }
}

async function withTempDir(fn) {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), "mir2-asset-release-safety-"));
  try {
    await fn(root);
  } finally {
    await fs.rm(root, { recursive: true, force: true });
  }
}

async function listen(handler) {
  const server = http.createServer(handler);
  await new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", resolve);
  });
  const address = server.address();
  return {
    url: `http://127.0.0.1:${address.port}`,
    close: () => new Promise((resolve, reject) => server.close((error) => error ? reject(error) : resolve())),
  };
}

function runNode(script, args, env = {}) {
  return new Promise((resolve, reject) => {
    const child = spawn(process.execPath, [script, ...args], {
      cwd: path.dirname(script),
      env: { ...process.env, ...env },
      stdio: ["ignore", "pipe", "pipe"],
    });
    let stdout = "";
    let stderr = "";
    child.stdout.on("data", (chunk) => { stdout += chunk; });
    child.stderr.on("data", (chunk) => { stderr += chunk; });
    child.on("error", reject);
    child.on("close", (code) => {
      if (code === 0) resolve({ stdout, stderr });
      else reject(new Error(`${path.basename(script)} exited ${code}\n${stdout}\n${stderr}`));
    });
  });
}
