#!/usr/bin/env node

import { spawn } from "node:child_process";
import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

import {
  isTransientQuestAgentExit,
  replaceCliOption,
  restartDelayMs,
  sanitizeAttemptSummary,
  signalExitCode,
  stripSupervisorOptions,
} from "./supervisor-policy.mjs";

// Full-route entrypoint. Keep the bounded q1-q9 command as the fast regression
// certificate while this entrypoint opts into every authoritative quest and
// the level-50 grind target. A multi-day physical-client run must also survive
// a browser/CDP process disappearing without treating game state as failed.
// Each retry starts a fresh normal client, waits beyond the gateway reconnect
// grace period, and relies only on the already persisted server state.
if (!process.argv.includes("--maxQuestId")) {
  process.argv.push("--maxQuestId", String(Number.MAX_SAFE_INTEGER));
}
if (!process.argv.includes("--targetLevel")) {
  process.argv.push("--targetLevel", "50");
}

const rawArgs = process.argv.slice(2);
const parsed = parseArgs(rawArgs);
const maxRestarts = Math.max(0, finiteInteger(parsed.maxRestarts, 20));
const baseRestartDelayMs = Math.max(15_000, finiteInteger(parsed.restartDelayMs, 20_000));
const totalRuntimeMs = Math.max(1_000, finiteInteger(parsed.maxRuntimeMs, 120 * 60_000));
const startedAt = Date.now();
const outputRoot = path.resolve(
  parsed.output ?? path.join(
    process.cwd(),
    "output",
    "quest-agent",
    `${new Date().toISOString().replace(/[:.]/g, "-")}-${process.pid}`,
  ),
);
const runnerPath = fileURLToPath(new URL("./run-q1-q5.mjs", import.meta.url));
const attemptSummaries = [];
let childArgs = stripSupervisorOptions(rawArgs);
let activeChild = null;
let requestedSignal = null;
let resolveStopRequest;
const stopRequested = new Promise((resolve) => {
  resolveStopRequest = resolve;
});

for (const signal of ["SIGINT", "SIGTERM"]) {
  process.on(signal, () => {
    if (requestedSignal != null) return;
    requestedSignal = signal;
    console.warn(`quest-agent supervisor forwarding graceful shutdown (${signal})`);
    resolveStopRequest(signal);
    if (activeChild && activeChild.exitCode === null && activeChild.signalCode === null) {
      activeChild.kill(signal);
    }
  });
}

await fs.mkdir(path.join(outputRoot, "attempts"), { recursive: true });

for (let attempt = 1; attempt <= maxRestarts + 1; attempt += 1) {
  if (requestedSignal != null) break;
  const elapsedMs = Date.now() - startedAt;
  const remainingRuntimeMs = totalRuntimeMs - elapsedMs;
  if (remainingRuntimeMs < 1_000) {
    console.error("quest-agent supervisor exhausted the total runtime budget before a new attempt");
    break;
  }

  const attemptLabel = String(attempt).padStart(3, "0");
  const attemptDir = path.join(outputRoot, "attempts", `attempt-${attemptLabel}`);
  let attemptArgs = replaceCliOption(childArgs, "output", attemptDir);
  attemptArgs = replaceCliOption(attemptArgs, "maxRuntimeMs", remainingRuntimeMs);
  if (parsed.runId) {
    attemptArgs = replaceCliOption(attemptArgs, "runId", `${parsed.runId}-attempt-${attemptLabel}`);
  }

  console.log(
    `quest-agent supervisor attempt=${attempt}/${maxRestarts + 1} ` +
    `remainingRuntimeMs=${remainingRuntimeMs}`,
  );
  const result = await runChild(runnerPath, attemptArgs);
  const summary = await readJson(path.join(attemptDir, "summary.json"));
  const attemptReport = path.join(attemptDir, "report.json");
  const sanitized = sanitizeAttemptSummary(
    attempt,
    result.exitCode,
    result.signal,
    summary,
  );
  attemptSummaries.push(sanitized);
  await writeSupervisorSummary(outputRoot, startedAt, totalRuntimeMs, attemptSummaries);

  if (requestedSignal != null) {
    await publishLatestAttempt(attemptDir, outputRoot, attemptLabel);
    break;
  }

  const cleanCompletion = result.exitCode === 0 && summary?.completed === true;
  if (cleanCompletion) {
    await publishLatestAttempt(attemptDir, outputRoot, attemptLabel);
    process.exitCode = 0;
    break;
  }

  const transient = isTransientQuestAgentExit({
    exitCode: result.exitCode,
    signal: result.signal,
    summary,
  });
  const retryAvailable = transient && attempt <= maxRestarts;
  if (!retryAvailable) {
    await publishLatestAttempt(attemptDir, outputRoot, attemptLabel);
    process.exitCode = 1;
    break;
  }

  // A fresh runner process otherwise invents a new default identity and also
  // forgets combat-risk observations made immediately before a transient CDP
  // or browser failure. Resume from the report we just wrote: the server is
  // still authoritative for character state, while the report supplies only
  // the same identity and bounded policy memory to the next normal client.
  if (await fileExists(attemptReport)) {
    childArgs = replaceCliOption(childArgs, "resumeReport", attemptReport);
  }

  const delayMs = Math.min(
    restartDelayMs(attempt, baseRestartDelayMs),
    Math.max(0, totalRuntimeMs - (Date.now() - startedAt) - 1_000),
  );
  console.warn(
    `quest-agent supervisor transient browser failure; retrying after ${delayMs}ms`,
  );
  if (delayMs > 0) await Promise.race([delay(delayMs), stopRequested]);
}

if (requestedSignal != null) {
  process.exitCode = signalExitCode(requestedSignal);
} else if (attemptSummaries.length === 0 || attemptSummaries.at(-1)?.completed !== true) {
  process.exitCode = 1;
}

function runChild(scriptPath, args) {
  return new Promise((resolve, reject) => {
    const child = spawn(process.execPath, [scriptPath, ...args], {
      cwd: process.cwd(),
      env: process.env,
      stdio: "inherit",
      // Keep the runner and its Chrome child out of the supervisor terminal's
      // foreground process group. Ctrl-C reaches the supervisor once; it then
      // forwards one typed graceful signal to the runner, which can finalize
      // evidence before closing Chrome. Windows keeps its existing console
      // behavior because detached children create a separate visible window.
      detached: process.platform !== "win32",
    });
    activeChild = child;
    child.once("error", (error) => {
      if (activeChild === child) activeChild = null;
      reject(error);
    });
    child.once("exit", (exitCode, signal) => {
      if (activeChild === child) activeChild = null;
      resolve({ exitCode, signal });
    });
  });
}

async function publishLatestAttempt(attemptDir, outputDir, attemptLabel) {
  const artifacts = [
    "summary.json",
    "report.json",
    "report.md",
    "action-trail.jsonl",
    "browser-diagnostics.json",
  ];
  for (const artifact of artifacts) {
    await fs.copyFile(path.join(attemptDir, artifact), path.join(outputDir, artifact)).catch(() => {});
  }
  await fs.cp(path.join(attemptDir, "frames"), path.join(outputDir, "frames"), {
    recursive: true,
    force: true,
  }).catch(() => {});
  await fs.writeFile(
    path.join(outputDir, "latest-attempt.json"),
    JSON.stringify({ attempt: Number(attemptLabel), directory: `attempts/attempt-${attemptLabel}` }, null, 2),
  );
}

async function writeSupervisorSummary(outputDir, runStartedAt, runtimeBudgetMs, attempts) {
  await fs.writeFile(
    path.join(outputDir, "supervisor-summary.json"),
    JSON.stringify({
      schema: "mir2-quest-agent-supervisor/1",
      startedAt: runStartedAt,
      runtimeBudgetMs,
      elapsedMs: Date.now() - runStartedAt,
      attempts,
    }, null, 2),
  );
}

async function readJson(file) {
  try {
    return JSON.parse(await fs.readFile(file, "utf8"));
  } catch {
    return null;
  }
}

async function fileExists(file) {
  try {
    await fs.access(file);
    return true;
  } catch {
    return false;
  }
}

function parseArgs(argv) {
  const parsedArgs = {};
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (!arg.startsWith("--")) continue;
    const key = arg.slice(2);
    const next = argv[index + 1];
    if (!next || next.startsWith("--")) parsedArgs[key] = "true";
    else {
      parsedArgs[key] = next;
      index += 1;
    }
  }
  return parsedArgs;
}

function finiteInteger(value, fallback) {
  const number = Number(value);
  return Number.isFinite(number) ? Math.trunc(number) : fallback;
}

function delay(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}
