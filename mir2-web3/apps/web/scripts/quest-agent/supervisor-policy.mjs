export function isTransientQuestAgentFatal(fatal) {
  const message = String(fatal ?? "").toLowerCase();
  if (!message) return false;
  return [
    "cdp socket closed",
    "browser process exited",
    "browser websocket closed",
    "target closed",
    "session closed",
    "connection closed while waiting for cdp",
    "character is already online or route lease is unavailable",
  ].some((fragment) => message.includes(fragment));
}

export function isTransientQuestAgentExit({ exitCode, signal, summary } = {}) {
  if (signal != null) return true;
  if (isTransientQuestAgentFatal(summary?.fatal)) return true;
  // Node uses 13 when an unsettled top-level await is the only remaining
  // work. A killed headless Chrome can close its debugging transport without
  // dispatching the WebSocket close callback which would let the runner write
  // summary.json; restrict this fallback to that exact no-report signature.
  return Number(exitCode) === 13 && summary == null;
}

export function restartDelayMs(attempt, baseDelayMs = 20_000) {
  const base = Math.max(15_000, Number(baseDelayMs) || 20_000);
  return Math.min(60_000, base * Math.max(1, Number(attempt) || 1));
}

export function signalExitCode(signal) {
  if (String(signal) === "SIGINT") return 130;
  if (String(signal) === "SIGTERM") return 143;
  return 1;
}

export function replaceCliOption(argv, name, value) {
  const option = `--${name}`;
  const output = [];
  for (let index = 0; index < argv.length; index += 1) {
    if (argv[index] !== option) {
      output.push(argv[index]);
      continue;
    }
    if (argv[index + 1] && !argv[index + 1].startsWith("--")) index += 1;
  }
  if (value !== undefined && value !== null) output.push(option, String(value));
  return output;
}

export function stripSupervisorOptions(argv) {
  return ["maxRestarts", "restartDelayMs"].reduce(
    (current, name) => replaceCliOption(current, name, null),
    [...argv],
  );
}

export function sanitizeAttemptSummary(attempt, exitCode, signal, summary = null) {
  const reportAvailable = summary != null;
  const measured = (value) => reportAvailable ? Number(value ?? 0) : null;
  return {
    attempt,
    exitCode: Number.isInteger(exitCode) ? exitCode : null,
    signal: signal == null ? null : String(signal),
    reportAvailable,
    completed: summary?.completed === true,
    fatal: summary?.fatal == null ? null : String(summary.fatal),
    runtimeMs: measured(summary?.runtimeMs),
    goals: measured(summary?.goals),
    goalsOk: measured(summary?.goalsOk),
    kills: measured(summary?.kills),
    deaths: measured(summary?.deaths),
    revives: measured(summary?.revives),
    shortcutViolations: measured(summary?.shortcutAudit?.violations?.length),
    criticalConsoleErrorCount: measured(summary?.criticalConsoleErrorCount),
    criticalNetworkFailureCount: measured(summary?.criticalNetworkFailureCount),
  };
}
