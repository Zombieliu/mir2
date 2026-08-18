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
const BUILD_OVERLAY_SCRIPT = path.join(SCRIPT_DIR, "build-r2-overlay-release.mjs");
const UPLOAD_SCRIPT = path.join(SCRIPT_DIR, "upload-r2-assets.mjs");
const VERIFY_MONSTER_FRAME_CLOSURE_SCRIPT = path.join(
  SCRIPT_DIR,
  "verify-monster-frame-closure.mjs",
);
const CAS_RELEASE_MODULE = path.join(SCRIPT_DIR, "asset-pipeline", "cas-release.mjs");
const FULL_PACK_CLOSURE_MODULE = path.join(SCRIPT_DIR, "asset-pipeline", "full-pack-closure.mjs");
const QUEST_ITEM_ICON_CLOSURE_MODULE = path.join(
  SCRIPT_DIR,
  "asset-pipeline",
  "quest-item-icon-closure.mjs",
);
const R2_RELEASE_WORKFLOW = path.resolve(
  SCRIPT_DIR,
  "..",
  "..",
  "..",
  "..",
  ".github",
  "workflows",
  "web-assets-r2-release.yml",
);
let passedTestCount = 0;

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
    await fs.copyFile(
      VERIFY_MONSTER_FRAME_CLOSURE_SCRIPT,
      path.join(path.dirname(fixtureScript), path.basename(VERIFY_MONSTER_FRAME_CLOSURE_SCRIPT)),
    );
    await fs.copyFile(CAS_RELEASE_MODULE, path.join(path.dirname(fixtureScript), "asset-pipeline", "cas-release.mjs"));
    await fs.copyFile(
      FULL_PACK_CLOSURE_MODULE,
      path.join(path.dirname(fixtureScript), "asset-pipeline", "full-pack-closure.mjs"),
    );
    await installQuestItemIconClosureFixture(root, fixtureScript, publicRoot);
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
    await fs.copyFile(
      VERIFY_MONSTER_FRAME_CLOSURE_SCRIPT,
      path.join(path.dirname(fixtureScript), path.basename(VERIFY_MONSTER_FRAME_CLOSURE_SCRIPT)),
    );
    await fs.copyFile(CAS_RELEASE_MODULE, path.join(path.dirname(fixtureScript), "asset-pipeline", "cas-release.mjs"));
    await fs.copyFile(
      FULL_PACK_CLOSURE_MODULE,
      path.join(path.dirname(fixtureScript), "asset-pipeline", "full-pack-closure.mjs"),
    );
    await installQuestItemIconClosureFixture(root, fixtureScript, publicRoot);
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

await test("original asset verification retries transient fetch failures", async () => {
  await withTempDir(async (root) => {
    let headRequests = 0;
    const server = await listen((request, response) => {
      if (request.method === "PUT") {
        request.resume();
        request.on("end", () => response.end("ok"));
        return;
      }
      if (request.method === "HEAD" && request.url === "/assets/original-ui/Items/412.png") {
        headRequests += 1;
        if (headRequests <= 3) {
          request.socket.destroy();
          return;
        }
        response.statusCode = 200;
        response.end();
        return;
      }
      response.statusCode = 404;
      response.end();
    });

    try {
      const stagePath = path.join(root, "412.png");
      const manifestPath = path.join(root, "release.json");
      await fs.writeFile(stagePath, "quest-item-icon");
      await fs.writeFile(manifestPath, JSON.stringify({
        assetBaseUrl: `${server.url}/assets`,
        objectPrefix: "mir2/v/verify-retry-fixture",
        publishReleaseManifest: false,
        files: [{
          relativePath: "original-ui/Items/412.png",
          stagePath,
          size: 15,
          contentType: "image/png",
          sources: ["original-asset-manifest"],
        }],
      }));

      const result = await runNode(UPLOAD_SCRIPT, [
        "--manifest", manifestPath,
        "--bucket", "fixture",
        "--driver", "worker",
        "--workerUrl", server.url,
        "--includeReleaseManifest", "false",
        "--verifyOriginalAssets", "true",
        "--concurrency", "1",
        "--verifyOriginalAssetConcurrency", "1",
        "--maxAttempts", "2",
        "--verifyOriginalAssetAttempts", "4",
      ], { MIR2_R2_UPLOAD_SECRET: "fixture-secret" });

      assert.equal(headRequests, 4);
      assert.match(result.stderr, /retry original asset verification 4\/4/);
    } finally {
      await server.close();
    }
  });
});

await test("resume mode verifies existing assets and uploads only release metadata", async () => {
  await withTempDir(async (root) => {
    const putKeys = [];
    let headRequests = 0;
    const server = await listen((request, response) => {
      if (request.method === "PUT") {
        putKeys.push(new URL(request.url, server.url).searchParams.get("key"));
        request.resume();
        request.on("end", () => response.end("ok"));
        return;
      }
      if (request.method === "HEAD" && request.url === "/assets/original-ui/Items/412.png") {
        headRequests += 1;
        response.statusCode = 200;
        response.end();
        return;
      }
      response.statusCode = 404;
      response.end();
    });

    try {
      const stagePath = path.join(root, "412.png");
      const manifestPath = path.join(root, "release.json");
      await fs.writeFile(stagePath, "quest-item-icon");
      await fs.writeFile(manifestPath, JSON.stringify({
        assetBaseUrl: `${server.url}/assets`,
        objectPrefix: "mir2/v/resume-fixture",
        files: [{
          relativePath: "original-ui/Items/412.png",
          stagePath,
          size: 15,
          contentType: "image/png",
          sources: ["original-asset-manifest"],
        }],
      }));

      const result = await runNode(UPLOAD_SCRIPT, [
        "--manifest", manifestPath,
        "--bucket", "fixture",
        "--driver", "worker",
        "--workerUrl", server.url,
        "--verifyOriginalAssets", "true",
        "--resumeExistingAssets", "true",
        "--concurrency", "1",
        "--verifyOriginalAssetConcurrency", "1",
        "--maxAttempts", "2",
      ], { MIR2_R2_UPLOAD_SECRET: "fixture-secret" });

      assert.equal(headRequests, 1);
      assert.deepEqual(putKeys, ["mir2/v/resume-fixture/remote-asset-release.json"]);
      assert.match(result.stdout, /resuming existing asset prefix; verifying 1 assets/);
    } finally {
      await server.close();
    }
  });
});

await test("resume mode refuses to skip uploads without full original asset verification", async () => {
  await withTempDir(async (root) => {
    const stagePath = path.join(root, "412.png");
    const manifestPath = path.join(root, "release.json");
    await fs.writeFile(stagePath, "quest-item-icon");
    await fs.writeFile(manifestPath, JSON.stringify({
      assetBaseUrl: "https://assets.invalid/mir2/v/resume-guard-fixture",
      objectPrefix: "mir2/v/resume-guard-fixture",
      files: [{
        relativePath: "original-ui/Items/412.png",
        stagePath,
        size: 15,
        contentType: "image/png",
        sources: ["original-asset-manifest"],
      }],
    }));

    await assert.rejects(
      runNode(UPLOAD_SCRIPT, [
        "--manifest", manifestPath,
        "--bucket", "fixture",
        "--driver", "worker",
        "--workerUrl", "https://upload.invalid",
        "--resumeExistingAssets", "true",
      ], { MIR2_R2_UPLOAD_SECRET: "fixture-secret" }),
      /requires MIR2_R2_VERIFY_ORIGINAL_ASSETS=1/,
    );
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

await test("Cloudflare OAuth API streams deterministic gzip bytes with HTTP metadata", async () => {
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
        objectPrefix: "mir2/v/oauth-api-fixture",
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
        "--driver", "api",
        "--accountId", "fixture-account",
        "--apiBaseUrl", server.url,
        "--includeReleaseManifest", "false",
        "--maxAttempts", "1",
      ], { CLOUDFLARE_API_TOKEN: "fixture-oauth-token" });

      assert.equal(requests.length, 1);
      assert.equal(requests[0].method, "PUT");
      assert.match(
        requests[0].url,
        /^\/accounts\/fixture-account\/r2\/buckets\/fixture\/objects\/mir2\/v\/oauth-api-fixture\//,
      );
      assert.equal(requests[0].headers.authorization, "Bearer fixture-oauth-token");
      assert.equal(requests[0].headers["content-encoding"], "gzip");
      assert.equal(Number(requests[0].headers["content-length"]), encoded.byteLength);
      assert.deepEqual(requests[0].body, encoded);
    } finally {
      await server.close();
    }
  });
});

await test("Cloudflare OAuth API honors Retry-After on rate limits", async () => {
  await withTempDir(async (root) => {
    const requests = [];
    const server = await listen((request, response) => {
      const chunks = [];
      request.on("data", (chunk) => chunks.push(chunk));
      request.on("end", () => {
        requests.push({
          method: request.method,
          url: request.url,
          body: Buffer.concat(chunks),
        });
        response.setHeader("content-type", "application/json");
        if (requests.length === 1) {
          response.statusCode = 429;
          response.setHeader("retry-after", "0");
          response.end(JSON.stringify({ success: false, errors: [{ message: "rate limited" }] }));
          return;
        }
        response.statusCode = 200;
        response.end(JSON.stringify({ success: true }));
      });
    });

    try {
      const stagePath = path.join(root, "runtime.js");
      const manifestPath = path.join(root, "release.json");
      const raw = Buffer.from("export const runtime = true;\n");
      await fs.writeFile(stagePath, raw);
      await fs.writeFile(manifestPath, JSON.stringify({
        objectPrefix: "mir2/v/oauth-api-rate-limit-fixture",
        publishReleaseManifest: false,
        files: [{
          relativePath: "bevy-runtime/v/bevy-fixture/pkg/runtime.js",
          stagePath,
          size: raw.byteLength,
          sha256: createHash("sha256").update(raw).digest("hex"),
          contentType: "text/javascript; charset=utf-8",
        }],
      }));

      const result = await runNode(UPLOAD_SCRIPT, [
        "--manifest", manifestPath,
        "--bucket", "fixture",
        "--driver", "api",
        "--accountId", "fixture-account",
        "--apiBaseUrl", server.url,
        "--includeReleaseManifest", "false",
        "--concurrency", "1",
        "--maxAttempts", "2",
      ], { CLOUDFLARE_API_TOKEN: "fixture-oauth-token" });

      assert.equal(requests.length, 2);
      assert.deepEqual(requests[0].body, raw);
      assert.deepEqual(requests[1].body, raw);
      assert.match(result.stderr, /retry 2\/2 in 1000ms/);
    } finally {
      await server.close();
    }
  });
});

await test("immutable runtime workflow serializes and retries R2 uploads", async () => {
  const workflow = await fs.readFile(R2_RELEASE_WORKFLOW, "utf8");
  assert.match(workflow, /MIR2_R2_UPLOAD_CONCURRENCY:\s*"1"/);
  assert.match(workflow, /MIR2_R2_UPLOAD_ATTEMPTS:\s*"6"/);
  assert.match(workflow, /upload_driver:[\s\S]*?default:\s*worker/);
  assert.equal((workflow.match(/MIR2_R2_UPLOAD_WORKER_URL:/g) ?? []).length, 2);
  assert.equal((workflow.match(/MIR2_R2_UPLOAD_SECRET:/g) ?? []).length, 2);
});

await test("Vercel release can verify a pre-deployed Worker", async () => {
  const workflow = await fs.readFile(R2_RELEASE_WORKFLOW, "utf8");
  assert.doesNotMatch(workflow, /deploy_vercel requires deploy_worker/);
  assert.doesNotMatch(workflow, /remote release version mismatch/);
  assert.match(workflow, /remote release objectPrefix mismatch/);
  assert.match(workflow, /remote release assetBaseUrl mismatch/);
  assert.match(
    workflow,
    /name: Deploy Player Web to Vercel\n\s+if: \$\{\{ \(inputs\.publish_r2 \|\| inputs\.use_existing_release\) && inputs\.deploy_vercel \}\}/,
  );
  assert.match(
    workflow,
    /name: Smoke current same-origin original assets\n\s+if: \$\{\{ \(inputs\.publish_r2 \|\| inputs\.use_existing_release\) && \(inputs\.deploy_worker \|\| inputs\.deploy_vercel\) \}\}/,
  );
  assert.match(
    workflow,
    /name: Verify full-pack closure through same-origin Worker\n\s+if: \$\{\{ inputs\.use_existing_release && \(inputs\.deploy_worker \|\| inputs\.deploy_vercel\)/,
  );
  assert.match(
    workflow,
    /name: Deploy same-origin asset Worker proxy\n\s+if: \$\{\{ \(inputs\.publish_r2 \|\| inputs\.use_existing_release\) && inputs\.deploy_worker \}\}/,
  );
  assert.match(
    workflow,
    /--build-env "MIR2_REUSE_ORIGINAL_ASSET_MANIFEST=1"/,
  );
  assert.match(
    workflow,
    /sync_vercel_production_env "MIR2_ASSET_VERSION" "\$MIR2_ASSET_VERSION"/,
  );
  assert.match(
    workflow,
    /sync_vercel_production_env "MIR2_ORIGINAL_ASSET_REMOTE_RELEASE" "\$MIR2_ORIGINAL_ASSET_REMOTE_RELEASE"/,
  );
  assert.match(
    workflow,
    /npx vercel@56\.4\.1 env add "\$1" production[\s\S]*?--force[\s\S]*?--value "\$2"/,
  );
});

await test("new R2 releases bootstrap the original-asset manifest locally before upload", async () => {
  const workflow = await fs.readFile(R2_RELEASE_WORKFLOW, "utf8");
  assert.match(
    workflow,
    /name: Build Web for asset manifest staging[\s\S]*?MIR2_ORIGINAL_ASSET_MANIFEST_MODE:\s*"filesystem"[\s\S]*?run: npm run build/,
  );
  assert.match(
    workflow,
    /name: Deploy Player Web to Vercel[\s\S]*?MIR2_ORIGINAL_ASSET_MANIFEST_MODE="remote-release"/,
  );
});

await test("existing overlay releases copy the pinned runtime before publishing the new prefix", async () => {
  const workflow = await fs.readFile(R2_RELEASE_WORKFLOW, "utf8");
  assert.match(
    workflow,
    /name: Prepare pinned Bevy runtime for an existing overlay release[\s\S]*?MIR2_FALLBACK_OBJECT_PREFIX[\s\S]*?npm run runtime:fetch:prebuilt/,
  );
  assert.match(
    workflow,
    /name: Stage immutable Bevy runtime release[\s\S]*?npm run runtime:r2:build/,
  );
});

await test("overlay releases publish only changed objects while preserving the verified full pack", async () => {
  await withTempDir(async (root) => {
    const fixtureScript = path.join(root, "apps", "web", "scripts", path.basename(BUILD_OVERLAY_SCRIPT));
    const publicRoot = path.join(root, "apps", "web", "public");
    const monsterRoot = path.join(publicRoot, "original-ui", "Monster", "000");
    const npcRoot = path.join(publicRoot, "original-ui", "NPC", "000");
    const baseManifestPath = path.join(root, "base-release.json");
    const originalAssetManifestPath = path.join(root, "original-assets.json");
    const outputPath = path.join(root, "overlay-release.json");
    const uploadPlanPath = path.join(root, "overlay-upload-plan.json");
    const manifestUploadPlanPath = path.join(root, "overlay-manifest-upload-plan.json");
    const fallbackObjectPrefix = "mir2/v/full-fixture";
    const overlayObjectPrefix = "mir2/v/overlay-fixture";
    const oldMonsterBytes = Buffer.from("old-monster-frame");
    const newMonsterBytes = Buffer.from("new-monster-frame");
    const npcBytes = Buffer.from("unchanged-npc-frame");
    const fullPackBytes = Buffer.from("verified-full-pack-index");
    const metaBytes = Buffer.from('{"frames":[{"index":0}],"frameSet":{"Attack":[0]}}\n');
    const sha256 = (bytes) => createHash("sha256").update(bytes).digest("hex");

    await fs.mkdir(path.dirname(fixtureScript), { recursive: true });
    await fs.mkdir(monsterRoot, { recursive: true });
    await fs.mkdir(npcRoot, { recursive: true });
    await fs.copyFile(BUILD_OVERLAY_SCRIPT, fixtureScript);
    await fs.writeFile(path.join(monsterRoot, "0.png"), newMonsterBytes);
    await fs.writeFile(path.join(monsterRoot, "meta.json"), metaBytes);
    await fs.writeFile(path.join(npcRoot, "0.png"), npcBytes);
    await fs.writeFile(baseManifestPath, JSON.stringify({
      schemaVersion: 2,
      version: "full-fixture",
      objectPrefix: fallbackObjectPrefix,
      assetBaseUrl: `https://assets.example/${fallbackObjectPrefix}`,
      cacheControl: "public, max-age=31536000, immutable",
      stats: {
        fileCount: 3,
        missingCount: 0,
        sceneSpriteFileCount: 1,
        publicAssetFileCount: 2,
      },
      fullCrystalPack: {
        enabled: true,
        verified: true,
        fileCount: 1,
        assetHash: sha256(fullPackBytes),
      },
      originalAssetManifest: {
        schemaVersion: 1,
        assetHash: "old-original-asset-hash",
        assetCount: 2,
        originalMapPngCount: 0,
        originalUiPngCount: 2,
      },
      sceneSpriteRoots: [{ root: "Monster", fileCount: 1 }],
      publicAssetRoots: [
        { root: "original-ui", fileCount: 2 },
        { root: "bevy-entity-atlases", fileCount: 0 },
      ],
      files: [
        {
          p: "generated/crystal-packs/full/index.json",
          s: fullPackBytes.length,
          h: sha256(fullPackBytes),
          c: "application/json; charset=utf-8",
        },
        {
          p: "original-ui/Monster/000/0.png",
          s: oldMonsterBytes.length,
          h: sha256(oldMonsterBytes),
          c: "image/png",
        },
        {
          p: "original-ui/NPC/000/0.png",
          s: npcBytes.length,
          h: sha256(npcBytes),
          c: "image/png",
        },
      ],
    }));
    await fs.writeFile(originalAssetManifestPath, JSON.stringify({
      schemaVersion: 1,
      assetHash: "current-original-asset-hash",
      stats: {
        assetCount: 2,
        originalMapPngCount: 0,
        originalUiPngCount: 2,
      },
      assets: {
        "/original-ui/Monster/000/0.png": {
          size: newMonsterBytes.length,
          sha256: sha256(newMonsterBytes),
        },
        "/original-ui/NPC/000/0.png": {
          size: npcBytes.length,
          sha256: sha256(npcBytes),
        },
      },
    }));

    await runNode(fixtureScript, [
      "--baseManifest", baseManifestPath,
      "--originalAssetManifest", originalAssetManifestPath,
      "--version", "overlay-fixture",
      "--objectPrefix", overlayObjectPrefix,
      "--fallbackObjectPrefix", fallbackObjectPrefix,
      "--assetBaseUrl", `https://assets.example/${overlayObjectPrefix}`,
      "--overlayRoots", "original-ui/Monster/000",
      "--output", outputPath,
      "--uploadPlan", uploadPlanPath,
      "--manifestUploadPlan", manifestUploadPlanPath,
    ]);

    const release = JSON.parse(await fs.readFile(outputPath, "utf8"));
    const uploadPlan = JSON.parse(await fs.readFile(uploadPlanPath, "utf8"));
    const manifestUploadPlan = JSON.parse(await fs.readFile(manifestUploadPlanPath, "utf8"));
    const logicalFiles = new Map(release.files.map((file) => [file.p, file]));
    const uploadPaths = uploadPlan.files.map((file) => file.p).sort();

    assert.equal(release.objectPrefix, overlayObjectPrefix);
    assert.equal(release.fallbackObjectPrefix, fallbackObjectPrefix);
    assert.equal(release.fullCrystalPack.verified, true);
    assert.equal(release.fullCrystalPack.assetHash, sha256(fullPackBytes));
    assert.equal(logicalFiles.get("generated/crystal-packs/full/index.json").h, sha256(fullPackBytes));
    assert.equal(logicalFiles.get("original-ui/NPC/000/0.png").h, sha256(npcBytes));
    assert.equal(logicalFiles.get("original-ui/Monster/000/0.png").h, sha256(newMonsterBytes));
    assert.deepEqual(uploadPaths, [
      "original-ui/Monster/000/0.png",
      "original-ui/Monster/000/meta.json",
    ]);
    assert.equal(uploadPlan.publishReleaseManifest, false);
    assert.equal(manifestUploadPlan.files.length, 1);
    assert.equal(manifestUploadPlan.files[0].p, "remote-asset-release.json");
    assert.equal(manifestUploadPlan.files[0].stagePath, outputPath);
  });
});

await test("Cloudflare OAuth API fails fast on authentication errors", async () => {
  await withTempDir(async (root) => {
    let requestCount = 0;
    const server = await listen((request, response) => {
      requestCount += 1;
      request.resume();
      request.on("end", () => {
        response.statusCode = 401;
        response.setHeader("content-type", "application/json");
        response.end(JSON.stringify({ success: false, errors: [{ message: "authentication error" }] }));
      });
    });

    try {
      const stagePath = path.join(root, "runtime.js");
      const manifestPath = path.join(root, "release.json");
      const raw = Buffer.from("export const runtime = true;\n");
      await fs.writeFile(stagePath, raw);
      await fs.writeFile(manifestPath, JSON.stringify({
        objectPrefix: "mir2/v/oauth-api-auth-fixture",
        publishReleaseManifest: false,
        files: [{
          relativePath: "bevy-runtime/v/bevy-fixture/pkg/runtime.js",
          stagePath,
          size: raw.byteLength,
          sha256: createHash("sha256").update(raw).digest("hex"),
          contentType: "text/javascript; charset=utf-8",
        }],
      }));

      await assert.rejects(
        runNode(UPLOAD_SCRIPT, [
          "--manifest", manifestPath,
          "--bucket", "fixture",
          "--driver", "api",
          "--accountId", "fixture-account",
          "--apiBaseUrl", server.url,
          "--includeReleaseManifest", "false",
          "--maxAttempts", "6",
        ], { CLOUDFLARE_API_TOKEN: "invalid-fixture-token" }),
        /HTTP 401 authentication error/,
      );
      assert.equal(requestCount, 1);
    } finally {
      await server.close();
    }
  });
});

await test("upload Worker streams deterministic gzip bytes with integrity headers", async () => {
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
        response.end(JSON.stringify({ ok: true }));
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
      const rawSha256 = createHash("sha256").update(raw).digest("hex");
      const encodedSha256 = createHash("sha256").update(encoded).digest("hex");
      await fs.writeFile(stagePath, raw);
      await fs.writeFile(manifestPath, JSON.stringify({
        objectPrefix: "mir2/v/worker-fixture",
        fullCrystalPack: { enabled: true },
        files: [{
          relativePath: "generated/crystal-packs/full/index.json",
          stagePath,
          size: raw.byteLength,
          sha256: rawSha256,
          contentType: "application/json; charset=utf-8",
          contentEncoding: "gzip",
          encodedSize: encoded.byteLength,
          encodedSha256,
        }],
      }));

      await runNode(UPLOAD_SCRIPT, [
        "--manifest", manifestPath,
        "--bucket", "fixture",
        "--driver", "worker",
        "--workerUrl", server.url,
        "--includeReleaseManifest", "false",
        "--maxAttempts", "1",
      ], { MIR2_R2_UPLOAD_SECRET: "fixture-worker-token" });

      assert.equal(requests.length, 1);
      assert.equal(requests[0].method, "PUT");
      assert.match(requests[0].url, /^\/upload\?key=mir2%2Fv%2Fworker-fixture%2F/);
      assert.equal(requests[0].headers.authorization, "Bearer fixture-worker-token");
      assert.equal(requests[0].headers["content-encoding"], undefined);
      assert.equal(requests[0].headers["x-mir2-content-encoding"], "gzip");
      assert.equal(requests[0].headers["x-mir2-sha256"], rawSha256);
      assert.equal(requests[0].headers["x-mir2-encoded-sha256"], encodedSha256);
      assert.equal(Number(requests[0].headers["content-length"]), encoded.byteLength);
      assert.deepEqual(requests[0].body, encoded);
    } finally {
      await server.close();
    }
  });
});

await test("upload Worker omits optional representation headers for identity assets", async () => {
  await withTempDir(async (root) => {
    const requests = [];
    const server = await listen((request, response) => {
      const chunks = [];
      request.on("data", (chunk) => chunks.push(chunk));
      request.on("end", () => {
        requests.push({
          headers: request.headers,
          body: Buffer.concat(chunks),
        });
        response.statusCode = 200;
        response.end(JSON.stringify({ ok: true }));
      });
    });

    try {
      const stagePath = path.join(root, "mir2_bevy_runtime.js");
      const manifestPath = path.join(root, "runtime-release.json");
      const bytes = Buffer.from("runtime-fixture");
      const sha256 = createHash("sha256").update(bytes).digest("hex");
      await fs.writeFile(stagePath, bytes);
      await fs.writeFile(manifestPath, JSON.stringify({
        schemaVersion: 1,
        kind: "mir2-bevy-runtime-r2-release",
        publishReleaseManifest: false,
        objectPrefix: "mir2/v/runtime-fixture",
        files: [{
          relativePath: "bevy-runtime/pkg-webgpu/mir2_bevy_runtime.js",
          stagePath,
          size: bytes.byteLength,
          sha256,
          contentType: "text/javascript; charset=utf-8",
        }],
      }));

      await runNode(UPLOAD_SCRIPT, [
        "--manifest", manifestPath,
        "--bucket", "fixture",
        "--driver", "worker",
        "--workerUrl", server.url,
        "--includeReleaseManifest", "false",
        "--maxAttempts", "1",
      ], { MIR2_R2_UPLOAD_SECRET: "fixture-worker-token" });

      assert.equal(requests.length, 1);
      assert.equal(requests[0].headers["x-mir2-content-encoding"], undefined);
      assert.equal(requests[0].headers["x-mir2-encoded-sha256"], undefined);
      assert.equal(requests[0].headers["x-mir2-sha256"], sha256);
      assert.deepEqual(requests[0].body, bytes);
    } finally {
      await server.close();
    }
  });
});

await test("runtime-only releases do not overwrite the full release manifest by default", async () => {
  await withTempDir(async (root) => {
    const stagePath = path.join(root, "mir2_bevy_runtime.js");
    const manifestPath = path.join(root, "runtime-release.json");
    const bytes = Buffer.from("runtime-fixture");
    await fs.writeFile(stagePath, bytes);
    await fs.writeFile(manifestPath, JSON.stringify({
      schemaVersion: 1,
      kind: "mir2-bevy-runtime-r2-release",
      publishReleaseManifest: false,
      objectPrefix: "mir2/v/runtime-fixture",
      files: [{
        path: "/bevy-runtime/pkg-webgpu/mir2_bevy_runtime.js",
        relativePath: "bevy-runtime/pkg-webgpu/mir2_bevy_runtime.js",
        stagePath,
        size: bytes.byteLength,
        sha256: createHash("sha256").update(bytes).digest("hex"),
        contentType: "text/javascript; charset=utf-8",
      }],
    }));

    const { stdout } = await runNode(UPLOAD_SCRIPT, [
      "--manifest", manifestPath,
      "--dryRun", "true",
    ]);
    const report = JSON.parse(stdout);
    assert.equal(report.uploadCount, 1);
    assert.equal(report.publishOrder.legacyReleaseManifest, 0);
    assert.equal(
      report.sample.some((entry) => entry.objectKey.endsWith("/remote-asset-release.json")),
      false,
    );
  });
});

console.log(`asset release safety tests passed (${passedTestCount} total)`);

async function test(name, fn) {
  try {
    await fn();
    passedTestCount += 1;
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

async function installQuestItemIconClosureFixture(root, fixtureScript, publicRoot) {
  const generatedDataRoot = path.join(root, "packages", "game-data", "data", "generated");
  const itemIconRoot = path.join(publicRoot, "original-ui", "Items");
  await fs.copyFile(
    QUEST_ITEM_ICON_CLOSURE_MODULE,
    path.join(path.dirname(fixtureScript), "asset-pipeline", "quest-item-icon-closure.mjs"),
  );
  await fs.mkdir(generatedDataRoot, { recursive: true });
  await fs.mkdir(itemIconRoot, { recursive: true });
  await fs.writeFile(
    path.join(generatedDataRoot, "crystal_quest_packet_manifest.json"),
    JSON.stringify({ quests: [] }),
  );
  await fs.writeFile(
    path.join(generatedDataRoot, "crystal_item_manifest.json"),
    JSON.stringify({ items: [] }),
  );
  await fs.writeFile(path.join(itemIconRoot, "meta.json"), JSON.stringify({ frames: [] }));
}
