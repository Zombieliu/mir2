export const DEFAULT_LOCAL_COMMAND_POSE_LATENCY_BUDGET_MS = 75;

export function analyzeLocalCommandPoseLatency(commands, probe, budgetMs) {
  const normalizedBudget = finiteNonNegativeNumber(budgetMs) ??
    DEFAULT_LOCAL_COMMAND_POSE_LATENCY_BUDGET_MS;
  const movementCommands = (Array.isArray(commands) ? commands : [])
    .map(normalizeCommand)
    .filter(Boolean)
    .sort((left, right) => left.commandAtMs - right.commandAtMs || left.movementSeq - right.movementSeq);
  const sinkEvents = (Array.isArray(probe?.sinkEvents) ? probe.sinkEvents : [])
    .map(normalizeSinkEvent)
    .filter(Boolean)
    .sort((left, right) => left.generatedAtMs - right.generatedAtMs || left.frameId - right.frameId);
  const samples = [];
  const missingCommands = [];

  for (let index = 0; index < movementCommands.length; index += 1) {
    const command = movementCommands[index];
    if (command.type !== "walk" && command.type !== "run") continue;
    const nextCommandAtMs = movementCommands[index + 1]?.commandAtMs ?? Number.POSITIVE_INFINITY;
    const event = sinkEvents.find(
      (candidate) =>
        candidate.generatedAtMs >= command.commandAtMs &&
        candidate.generatedAtMs < nextCommandAtMs,
    );
    if (!event) {
      missingCommands.push(command);
      continue;
    }
    samples.push({
      movementSeq: command.movementSeq,
      type: command.type,
      direction: command.direction,
      commandAtMs: command.commandAtMs,
      frameId: event.frameId,
      generatedAtMs: event.generatedAtMs,
      sinkAtMs: event.sinkAtMs,
      commandToPoseMs: event.generatedAtMs - command.commandAtMs,
      commandToSinkMs: event.sinkAtMs - command.commandAtMs,
      poseToSinkMs: event.sinkAtMs - event.generatedAtMs,
      cameraX: event.cameraX,
      cameraY: event.cameraY,
    });
  }

  const eligibleCommandCount = movementCommands.filter(
    (command) => command.type === "walk" || command.type === "run",
  ).length;
  const maxCommandToPoseMs = maxOrNull(samples.map((sample) => sample.commandToPoseMs));
  const maxCommandToSinkMs = maxOrNull(samples.map((sample) => sample.commandToSinkMs));
  const droppedSinkEventCount = finiteNonNegativeNumber(probe?.droppedSinkEventCount) ?? 0;
  const coverageComplete =
    eligibleCommandCount > 0 &&
    samples.length === eligibleCommandCount &&
    missingCommands.length === 0 &&
    droppedSinkEventCount === 0;

  return {
    version: 1,
    budgetMs: normalizedBudget,
    armedAtMs: finiteNonNegativeNumber(probe?.armedAtMs),
    sinkCallbackCount: finiteNonNegativeNumber(probe?.sinkCallbackCount) ?? 0,
    droppedSinkEventCount,
    eligibleCommandCount,
    matchedCommandCount: samples.length,
    missingCommands,
    samples,
    maxCommandToPoseMs,
    maxCommandToSinkMs,
    coverageComplete,
    responsive:
      coverageComplete &&
      maxCommandToSinkMs !== null &&
      maxCommandToSinkMs <= normalizedBudget,
  };
}

function normalizeCommand(value) {
  if (!value || typeof value !== "object") return null;
  const type = typeof value.type === "string" ? value.type : null;
  if (type !== "walk" && type !== "run" && type !== "turn") return null;
  const commandAtMs = finiteNonNegativeNumber(value.at);
  if (commandAtMs === null) return null;
  return {
    movementSeq: finiteNonNegativeNumber(value.movementSeq) ?? 0,
    type,
    direction: typeof value.direction === "string" ? value.direction : null,
    commandAtMs,
  };
}

function normalizeSinkEvent(value) {
  if (!value || typeof value !== "object") return null;
  if (value.cameraSource !== "localCommand") return null;
  const frameId = finiteNonNegativeNumber(value.frameId);
  const generatedAtMs = finiteNonNegativeNumber(value.generatedAtMs);
  const sinkAtMs = finiteNonNegativeNumber(value.sinkAtMs);
  const cameraX = finiteNumber(value.cameraX);
  const cameraY = finiteNumber(value.cameraY);
  if (
    frameId === null ||
    generatedAtMs === null ||
    sinkAtMs === null ||
    cameraX === null ||
    cameraY === null ||
    (Math.abs(cameraX) <= 0.001 && Math.abs(cameraY) <= 0.001)
  ) {
    return null;
  }
  return { frameId, generatedAtMs, sinkAtMs, cameraX, cameraY };
}

function maxOrNull(values) {
  return values.length > 0 ? Math.max(...values) : null;
}

function finiteNumber(value) {
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

function finiteNonNegativeNumber(value) {
  const number = finiteNumber(value);
  return number !== null && number >= 0 ? number : null;
}
