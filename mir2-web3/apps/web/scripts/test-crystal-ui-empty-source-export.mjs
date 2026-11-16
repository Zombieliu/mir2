import assert from "node:assert/strict";
import { mkdtemp, mkdir, readFile, writeFile, rm } from "node:fs/promises";
import { existsSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { gzipSync } from "node:zlib";
import { spawnSync } from "node:child_process";

const temp = await mkdtemp(path.join(tmpdir(), "mir2-empty-source-export-"));
try {
  const dataDir = path.join(temp, "Data");
  const outputDir = path.join(temp, "output");
  await mkdir(dataDir);
  // V2: first frame is an explicit original zero-size slot; second is 1x1 BGRA.
  const pixels = gzipSync(Buffer.from([3, 2, 1, 255]));
  const bytes = Buffer.alloc(16 + 17 + 17 + pixels.length);
  bytes.writeInt32LE(2, 0);
  bytes.writeInt32LE(2, 4);
  bytes.writeInt32LE(16, 8);
  bytes.writeInt32LE(33, 12);
  bytes.writeInt16LE(1, 33);
  bytes.writeInt16LE(1, 35);
  bytes.writeInt32LE(pixels.length, 46);
  pixels.copy(bytes, 50);
  await writeFile(path.join(dataDir, "Fixture.Lib"), bytes);
  const result = spawnSync(process.execPath, [path.join(import.meta.dirname, "export-crystal-ui.mjs"),
    "--dataDir", dataDir, "--outputDir", outputDir, "--libraries", "Fixture", "--fullLibraries", "Fixture"], { encoding: "utf8" });
  assert.equal(result.status, 0, result.stderr);
  const meta = JSON.parse(await readFile(path.join(outputDir, "Fixture/meta.json")));
  assert.equal(meta.count, 2, "retain source index space");
  assert.deepEqual(meta.frames.map((frame) => frame.index), [1], "omit original zero-size frame");
  assert.equal(existsSync(path.join(outputDir, "Fixture/0.png")), false, "never manufacture an invalid PNG");
  assert.equal(existsSync(path.join(outputDir, "Fixture/1.png")), true);
  console.log("Crystal full-library export preserves empty source slots without fabricated PNGs");
} finally {
  const resolved = path.resolve(temp);
  const tempRoot = path.resolve(tmpdir());
  if (!resolved.startsWith(`${tempRoot}${path.sep}`) || !path.basename(resolved).startsWith("mir2-empty-source-export-")) throw new Error("Unsafe test temporary path");
  await rm(resolved, { recursive: true, force: true });
}
