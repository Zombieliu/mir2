import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";

import { resolveCurrentContentAddressedManifest } from "./build-map-atlas-release.mjs";

function contentPath(root, bytes) {
  const hash = createHash("sha256").update(bytes).digest("hex");
  return path.join(root, `manifest.${hash}.json`);
}

test("release resolves the verified current manifest while retaining prior immutable versions", async () => {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), "mir2-map-atlas-release-"));
  try {
    const previous = Buffer.from('{"version":"old"}\n');
    const current = Buffer.from('{"version":"current"}\n');
    await fs.writeFile(contentPath(root, previous), previous);
    const expected = contentPath(root, current);
    await fs.writeFile(expected, current);
    await fs.writeFile(path.join(root, "manifest.json"), current);

    assert.equal(await resolveCurrentContentAddressedManifest(root), expected);
    await fs.access(contentPath(root, previous));
  } finally {
    await fs.rm(root, { recursive: true, force: true });
  }
});

test("release rejects a current pointer without matching immutable bytes", async () => {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), "mir2-map-atlas-release-"));
  try {
    const current = Buffer.from('{"version":"current"}\n');
    await fs.writeFile(path.join(root, "manifest.json"), current);
    await assert.rejects(
      () => resolveCurrentContentAddressedManifest(root),
      /does not match|ENOENT/,
    );
  } finally {
    await fs.rm(root, { recursive: true, force: true });
  }
});
