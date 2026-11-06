#!/usr/bin/env node

import { access, readdir, readFile } from "node:fs/promises";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const webRoot = path.resolve(scriptDir, "..");
const defaultAssetRoot = path.join(webRoot, "public", "original-ui");
const defaultLibraries = [
  "Monster/000",
  "Monster/003",
  "Monster/004",
  "Monster/005",
  "Monster/007",
  "Monster/010",
  "Monster/012",
];
const requiredActionsByLibrary = Object.freeze({
  "Monster/000": ["Standing", "Walking", "Attack1"],
  "Monster/003": ["Standing", "Walking", "Attack1", "Struck", "Die", "Dead"],
  "Monster/004": ["Standing", "Walking", "Attack1", "Struck", "Die", "Dead"],
  "Monster/005": ["Standing", "Walking", "Attack1", "Struck", "Die", "Dead", "Revive"],
  "Monster/007": ["Standing", "Walking", "Attack1", "Struck", "Die", "Dead"],
  "Monster/010": ["Standing", "Attack1", "Struck", "Die", "Dead"],
  "Monster/012": ["Standing", "Walking", "Attack1", "Struck", "Die", "Dead"],
});

export async function verifyMonsterFrameClosure({
  assetRoot = defaultAssetRoot,
  libraries = defaultLibraries,
} = {}) {
  const failures = [];
  const results = [];

  for (const library of libraries) {
    const libraryDir = path.join(assetRoot, ...library.split("/"));
    const metaPath = path.join(libraryDir, "meta.json");
    try {
      await access(metaPath);
    } catch {
      failures.push(`${library}: missing meta.json`);
      continue;
    }

    const meta = JSON.parse(await readFile(metaPath, "utf8"));
    const entries = await readdir(libraryDir);
    const pngIndices = entries
      .map((entry) => /^(\d+)\.png$/u.exec(entry)?.[1])
      .filter(Boolean)
      .map(Number)
      .sort((left, right) => left - right);
    const expectedCount = Number(meta.count);
    const frameArrayCount = Array.isArray(meta.frames) ? meta.frames.length : 0;
    const actions = Array.isArray(meta.frameSet?.actions) ? meta.frameSet.actions : [];
    const missingIndices = Number.isInteger(expectedCount)
      ? Array.from({ length: expectedCount }, (_, index) => index).filter(
          (index) => pngIndices[index] !== index,
        )
      : [];

    if (!Number.isInteger(expectedCount) || expectedCount <= 0) {
      failures.push(`${library}: invalid meta.count=${String(meta.count)}`);
    }
    if (pngIndices.length !== expectedCount) {
      failures.push(
        `${library}: PNG count ${pngIndices.length} does not match meta.count ${expectedCount}`,
      );
    }
    if (frameArrayCount !== expectedCount) {
      failures.push(
        `${library}: frames[] count ${frameArrayCount} does not match meta.count ${expectedCount}`,
      );
    }
    if (missingIndices.length > 0) {
      failures.push(
        `${library}: missing frame indices ${missingIndices.slice(0, 12).join(",")}${
          missingIndices.length > 12 ? ",…" : ""
        }`,
      );
    }
    if (actions.length === 0) {
      failures.push(`${library}: frameSet.actions is empty`);
    }

    const requiredActions = requiredActionsByLibrary[library] ?? ["Standing", "Die", "Dead"];
    for (const requiredAction of requiredActions) {
      if (!actions.some((action) => action.actionName === requiredAction)) {
        failures.push(`${library}: missing ${requiredAction} action`);
      }
    }

    for (const action of actions) {
      const start = Number(action.start);
      const count = Number(action.count);
      if (
        !Number.isInteger(start) ||
        !Number.isInteger(count) ||
        start < 0 ||
        count <= 0 ||
        start + count > expectedCount
      ) {
        failures.push(
          `${library}: ${String(action.actionName)} range ${start}+${count} exceeds ${expectedCount}`,
        );
      }
    }

    results.push({
      library,
      frameCount: pngIndices.length,
      actionCount: actions.length,
    });
  }

  if (failures.length > 0) {
    throw new Error(`Monster frame closure failed:\n- ${failures.join("\n- ")}`);
  }

  return results;
}

function parseArguments(argv) {
  const options = {};
  for (let index = 0; index < argv.length; index += 1) {
    if (argv[index] === "--asset-root") {
      options.assetRoot = path.resolve(argv[index + 1]);
      index += 1;
    } else if (argv[index] === "--libraries") {
      options.libraries = argv[index + 1]
        .split(",")
        .map((value) => value.trim())
        .filter(Boolean);
      index += 1;
    }
  }
  return options;
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    const results = await verifyMonsterFrameClosure(parseArguments(process.argv.slice(2)));
    for (const result of results) {
      console.log(
        `[monster-frame-closure] ${result.library}: ${result.frameCount} frames, ${result.actionCount} actions`,
      );
    }
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 1;
  }
}
