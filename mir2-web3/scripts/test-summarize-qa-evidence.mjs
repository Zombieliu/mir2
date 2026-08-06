#!/usr/bin/env node

import assert from "node:assert/strict";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { summarizeEvidence } from "./summarize-qa-evidence.mjs";

test("writes a deterministic inventory digest for QA evidence", async (context) => {
  const repoRoot = await fs.mkdtemp(path.join(os.tmpdir(), "mir2-evidence-summary-"));
  context.after(() => fs.rm(repoRoot, { recursive: true, force: true }));
  await fs.mkdir(path.join(repoRoot, "docs", "generated", "player-qa"), { recursive: true });
  await fs.mkdir(path.join(repoRoot, "docs", "stage5-screenshots"), { recursive: true });
  await fs.writeFile(path.join(repoRoot, "docs", "generated", "player-qa", "report.json"), "{\"ok\":true}\n");
  await fs.writeFile(path.join(repoRoot, "docs", "stage5-screenshots", "frame.png"), Buffer.from([1, 2, 3]));

  const first = await summarizeEvidence({
    repoRoot,
    sources: ["docs/generated/player-qa", "docs/stage5-screenshots", "docs/missing"],
  });
  const second = await summarizeEvidence({
    repoRoot,
    sources: ["docs/generated/player-qa", "docs/stage5-screenshots", "docs/missing"],
    output: "artifacts/qa-evidence/second.json",
  });

  assert.equal(first.summary.fileCount, 2);
  assert.equal(first.summary.bytes, 15);
  assert.equal(first.summary.sha256, second.summary.sha256);
  assert.deepEqual(first.summary.files.map((file) => file.path), [
    "docs/generated/player-qa/report.json",
    "docs/stage5-screenshots/frame.png",
  ]);
  assert.equal(JSON.parse(await fs.readFile(first.outputPath, "utf8")).kind, "mir2-qa-evidence-summary");
});

test("refuses evidence sources outside the repository", async () => {
  await assert.rejects(
    summarizeEvidence({ repoRoot: "/tmp/mir2-evidence-root", sources: ["../outside"] }),
    /outside the repository/,
  );
});
