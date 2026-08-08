import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";

import sharp from "sharp";

const root = await fs.mkdtemp(path.join(os.tmpdir(), "mir2-visual-report-test-"));

try {
  await sharp(Buffer.from([0, 0, 0, 0, 0, 0]), {
    raw: { width: 2, height: 1, channels: 3 },
  })
    .png()
    .toFile(path.join(root, "threshold-original.png"));
  await sharp(Buffer.from([12, 12, 12, 11, 11, 11]), {
    raw: { width: 2, height: 1, channels: 3 },
  })
    .png()
    .toFile(path.join(root, "threshold-web.png"));
  await fs.writeFile(
    path.join(root, "threshold-web-state.json"),
    `${JSON.stringify({ screen: "game", mapFileName: "0", player: { x: 1, y: 1 } })}\n`,
  );

  await run(process.execPath, [
    path.resolve("apps/web/scripts/report-crystal-visual-parity.mjs"),
    "--input",
    root,
    "--output",
    root,
    "--prefix",
    "threshold-report",
    "--maxSamples",
    "1",
    "--pixelDeltaThreshold",
    "12",
  ]);
  const report = JSON.parse(await fs.readFile(path.join(root, "threshold-report.json"), "utf8"));
  const full = report.samples[0].regionMetrics.full;
  assert.equal(report.pixelDeltaThreshold, 12);
  assert.equal(full.pixelCount, 2);
  assert.equal(full.changedPixelCount, 1);
  assert.equal(full.changedPixelRatio, 0.5);
  assert.equal(full.pixelDeltaThreshold, 12);
  console.log("crystal visual parity report tests: ok");
} finally {
  await fs.rm(root, { recursive: true, force: true });
}

async function run(command, args) {
  const child = spawn(command, args, { stdio: ["ignore", "pipe", "pipe"], windowsHide: true });
  let stdout = "";
  let stderr = "";
  child.stdout.on("data", (chunk) => (stdout += chunk));
  child.stderr.on("data", (chunk) => (stderr += chunk));
  const code = await new Promise((resolve, reject) => {
    child.on("error", reject);
    child.on("close", resolve);
  });
  if (code !== 0) throw new Error(`${command} failed with ${code}\n${stdout}\n${stderr}`);
}
