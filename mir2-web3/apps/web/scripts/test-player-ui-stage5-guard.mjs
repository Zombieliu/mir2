import assert from "node:assert/strict";
import { readdir, readFile } from "node:fs/promises";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const scriptDirectory = path.dirname(fileURLToPath(import.meta.url));
const appDirectory = path.resolve(scriptDirectory, "..", "app");
const forbiddenPlayerCommand = /type\s*:\s*["']stage5Command["']/;
const retiredPlayerWiring = /\bonRunStage5Command\b|\bstage5CommandForSocialAction\b/;
const retiredDebugTransferUi =
  /\bQUICK_TRANSFER_OPTIONS\b|\bsystem-menu-qa-transfer\b|\bTransfer controls\b|\bQuick Jump\b/;

async function normalPlayerSourceFiles(directory) {
  const entries = await readdir(directory, { withFileTypes: true });
  const files = [];
  for (const entry of entries) {
    const entryPath = path.join(directory, entry.name);
    if (entry.isDirectory()) {
      // QA routes are intentionally isolated from normal player navigation and
      // may exercise privileged commands under their own explicit test policy.
      if (entry.name === "qa") continue;
      files.push(...(await normalPlayerSourceFiles(entryPath)));
    } else if (/\.(?:ts|tsx)$/.test(entry.name)) {
      files.push(entryPath);
    }
  }
  return files;
}

test("normal player React sources cannot wire generic stage5 commands or debug-transfer UI", async () => {
  const offenders = [];
  for (const file of await normalPlayerSourceFiles(appDirectory)) {
    const source = await readFile(file, "utf8");
    if (
      forbiddenPlayerCommand.test(source) ||
      retiredPlayerWiring.test(source) ||
      retiredDebugTransferUi.test(source)
    ) {
      offenders.push(path.relative(appDirectory, file));
    }
  }
  assert.deepEqual(offenders, []);
});
