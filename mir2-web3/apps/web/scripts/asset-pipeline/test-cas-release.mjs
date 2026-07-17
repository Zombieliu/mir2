import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";

import {
  CHANNEL_CACHE_CONTROL,
  createCasRelease,
  loadCasUploadPlan,
  validateCasManifest,
  writeCasReleaseArtifacts,
} from "./cas-release.mjs";

const HASH_A = createHash("sha256").update(Buffer.alloc(10)).digest("hex");
const HASH_B = createHash("sha256").update(Buffer.alloc(20)).digest("hex");

function files() {
  return [
    { relativePath: "sprites/b.png", sha256: HASH_B, size: 20, contentType: "image/png", stagePath: "b.png" },
    { relativePath: "audio/a.wav", sha256: HASH_A, size: 10, contentType: "audio/wav", stagePath: "a.wav" },
  ];
}

test("CAS release and channel are deterministic across input ordering and local metadata", () => {
  const forward = createCasRelease(files(), { prefix: "mir2/cas", channel: "stable" });
  const reverse = createCasRelease(files().reverse().map((file) => ({ ...file, stagePath: `other-${file.stagePath}` })), {
    prefix: "mir2/cas",
    channel: "stable",
  });
  assert.equal(forward.manifestJson, reverse.manifestJson);
  assert.equal(forward.channelJson, reverse.channelJson);
  assert.equal(createHash("sha256").update(forward.manifestJson).digest("hex"), forward.descriptor.manifest.sha256);
  assert.match(forward.descriptor.manifest.objectKey, new RegExp(`${forward.descriptor.manifest.sha256}\\.json$`));
  assert.deepEqual(forward.manifest.files.map((file) => file.path), ["audio/a.wav", "sprites/b.png"]);
  assert.match(forward.manifest.files[0].objectKey, new RegExp(`${HASH_A}$`));
});

test("content changes produce new immutable blob and release keys", () => {
  const baseline = createCasRelease(files());
  const changed = createCasRelease(files().map((file) =>
    file.relativePath === "audio/a.wav" ? { ...file, sha256: "c".repeat(64) } : file));
  assert.notEqual(baseline.descriptor.manifest.sha256, changed.descriptor.manifest.sha256);
  assert.notEqual(baseline.descriptor.manifest.objectKey, changed.descriptor.manifest.objectKey);
  assert.notEqual(baseline.manifest.files[0].objectKey, changed.manifest.files[0].objectKey);
});

test("invalid hashes, duplicate paths, traversal, and manifest tampering are rejected", () => {
  assert.throws(() => createCasRelease([{ ...files()[0], sha256: "bad" }]), /sha256/);
  assert.throws(() => createCasRelease([files()[0], files()[0]]), /Duplicate release file path/);
  assert.throws(() => createCasRelease([{ ...files()[0], relativePath: "../escape.png" }]), /Invalid release-relative path/);

  const plan = createCasRelease(files());
  const corrupt = structuredClone(plan.manifest);
  corrupt.files[0].size += 1;
  assert.throws(
    () => validateCasManifest(corrupt, { prefix: "mir2/cas", expectedHash: plan.descriptor.manifest.sha256 }),
    /contentHash mismatch/,
  );
});

test("materialized upload plan validates artifacts and keeps the mutable channel separate", async () => {
  const directory = await mkdtemp(path.join(os.tmpdir(), "mir2-cas-release-"));
  try {
    await writeFile(path.join(directory, "a.wav"), Buffer.alloc(10));
    await writeFile(path.join(directory, "b.png"), Buffer.alloc(20));
    const sourceFiles = files().map((file) => ({ ...file, stagePath: path.join(directory, file.stagePath) }));
    const descriptor = await writeCasReleaseArtifacts(createCasRelease(sourceFiles, { channel: "candidate" }), directory);
    const uploadPlan = await loadCasUploadPlan({ files: sourceFiles, cas: descriptor });
    assert.equal(uploadPlan.assets.length, 2);
    assert.match(uploadPlan.manifest.objectKey, /releases\/sha256\/[a-f0-9]{64}\.json$/);
    assert.equal(uploadPlan.channel.objectKey, "mir2/cas/channels/candidate.json");
    assert.equal(uploadPlan.channel.cacheControl, CHANNEL_CACHE_CONTROL);
    assert.ok(uploadPlan.channel.size < uploadPlan.manifest.size);

    await writeFile(path.join(directory, "a.wav"), Buffer.alloc(10, 1));
    await assert.rejects(() => loadCasUploadPlan({ files: sourceFiles, cas: descriptor }), /staged asset hash mismatch/);
    await writeFile(path.join(directory, "a.wav"), Buffer.alloc(10));

    const manifest = JSON.parse(await readFile(descriptor.manifest.stagePath, "utf8"));
    manifest.files[0].size += 1;
    await writeFile(descriptor.manifest.stagePath, JSON.stringify(manifest), "utf8");
    await assert.rejects(
      () => loadCasUploadPlan({ files: sourceFiles, cas: descriptor }),
      /artifact size mismatch|contentHash mismatch/,
    );
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});
