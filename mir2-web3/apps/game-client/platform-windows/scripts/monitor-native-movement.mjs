#!/usr/bin/env node

// Read-only native movement listener.
//
// It connects to the Gateway spectator socket, which has no player session and
// rejects gameplay commands structurally. The listener records authoritative
// player position changes only; it never sends movement or controls the native
// window.

import assert from "node:assert/strict";
import fs from "node:fs/promises";
import path from "node:path";

const SCHEMA = "mir2.windows.native-movement-monitor.v2";
const DEFAULT_GATEWAY_WS = "ws://127.0.0.1:7210";
const DEFAULT_GATEWAY_HTTP = "http://127.0.0.1:7210";
const DEFAULT_TOKEN = "local-spectator-director";
const DEFAULT_DURATION_MS = 45_000;
const DEFAULT_STALL_MS = 850;
const MAX_RECORDED_MOVES = 5_000;

function parseArgs(argv) {
  const values = {};
  const optionKey = (value) => value.replace(/-([a-z])/g, (_, letter) => letter.toUpperCase());
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (!argument.startsWith("--")) throw new Error(`unexpected argument: ${argument}`);
    const separator = argument.indexOf("=");
    if (separator >= 0) {
      values[optionKey(argument.slice(2, separator))] = argument.slice(separator + 1);
      continue;
    }
    const key = optionKey(argument.slice(2));
    if (key === "selfTest" || key === "help" || key === "fromStart") {
      values[key] = true;
      continue;
    }
    const next = argv[index + 1];
    if (next === undefined || next.startsWith("--")) throw new Error(`missing value for --${key}`);
    values[key] = next;
    index += 1;
  }
  return values;
}

function printUsage() {
  console.log(`Usage:
  node monitor-native-movement.mjs [options]

Read-only sources (choose one):
  --client-trace <jsonl> Tail MIR2_NATIVE_MOVEMENT_TRACE_PATH (preferred)
  --recording <jsonl>   Tail the Gateway's local spectator recording with no delay
  --gateway-ws <url>    Spectator WebSocket base (default: ws://127.0.0.1:7210)

Options:
  --target <name>       Player name to observe (default: first player)
  --duration-ms <ms>    Sample duration, 1000..1800000 (default: 45000)
  --stall-ms <ms>       Gap classified as a stall (default: 850)
  --map <name>          Map identity; discovered for WebSocket mode
  --token <token>       Optional spectator director token
  --output <json>       Write the complete evidence report
  --from-start          Read an existing client trace before following new lines
  --self-test           Run synthetic analyzer regression
  --help                Show this help

The monitor never sends gameplay commands and never controls the game window.`);
}

function boundedInteger(value, fallback, minimum, maximum, label) {
  if (value === undefined || value === "") return fallback;
  const parsed = Number(value);
  if (!Number.isSafeInteger(parsed) || parsed < minimum || parsed > maximum) {
    throw new Error(`${label} must be an integer in ${minimum}..${maximum}`);
  }
  return parsed;
}

function normalizedBase(value, protocol) {
  const url = new URL(value);
  if (!protocol.includes(url.protocol)) throw new Error(`unsupported gateway protocol: ${url.protocol}`);
  url.pathname = url.pathname.replace(/\/$/, "");
  url.search = "";
  url.hash = "";
  return url.toString().replace(/\/$/, "");
}

function finiteInteger(value) {
  return Number.isSafeInteger(value) ? value : null;
}

function finiteNumber(value) {
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

function percentile(sorted, ratio) {
  if (sorted.length === 0) return null;
  const index = Math.min(sorted.length - 1, Math.max(0, Math.ceil(sorted.length * ratio) - 1));
  return sorted[index];
}

function rounded(value, digits = 2) {
  if (!Number.isFinite(value)) return null;
  const scale = 10 ** digits;
  return Math.round(value * scale) / scale;
}

function numericStats(values) {
  const finite = values.filter((value) => Number.isFinite(value) && value >= 0);
  const sorted = [...finite].sort((left, right) => left - right);
  return {
    count: sorted.length,
    min: sorted.at(0) ?? null,
    median: percentile(sorted, 0.5),
    p95: percentile(sorted, 0.95),
    max: sorted.at(-1) ?? null,
    average: sorted.length > 0
      ? rounded(sorted.reduce((sum, value) => sum + value, 0) / sorted.length)
      : null,
  };
}

function moveIdentity(event) {
  return [
    event.atMs,
    event.objectId,
    event.payload?.from?.x,
    event.payload?.from?.y,
    event.payload?.to?.x,
    event.payload?.to?.y,
  ].join(":");
}

class MovementAnalyzer {
  constructor({ targetName = null, stallMs = DEFAULT_STALL_MS } = {}) {
    this.requestedTargetName = targetName;
    this.targetName = null;
    this.targetObjectId = null;
    this.stallMs = stallMs;
    this.sequences = new Set();
    this.eventKeys = new Set();
    this.moves = [];
    this.worldFrameCount = 0;
    this.firstCapturedAtMs = null;
    this.lastCapturedAtMs = null;
    this.spectatorDelayMs = null;
    this.directorAuthorized = null;
  }

  selectTarget(targets) {
    if (!Array.isArray(targets) || targets.length === 0) return false;
    const requested = this.requestedTargetName?.trim().toLocaleLowerCase("en-US");
    const target = requested
      ? targets.find((candidate) => String(candidate?.name ?? "").toLocaleLowerCase("en-US") === requested)
      : targets[0];
    if (!target) return false;
    const objectId = finiteInteger(target.objectId);
    if (objectId === null || typeof target.name !== "string" || target.name.trim() === "") return false;
    this.targetObjectId = objectId;
    this.targetName = target.name;
    return true;
  }

  observeStatus(payload) {
    if (!payload || typeof payload !== "object") return [];
    if (this.targetObjectId === null) this.selectTarget(payload.targets);
    const sequence = finiteInteger(payload.sequence);
    if (sequence !== null && this.sequences.has(sequence)) return [];
    if (sequence !== null) this.sequences.add(sequence);
    const capturedAtMs = finiteInteger(payload.capturedAtMs);
    if (capturedAtMs !== null) {
      this.firstCapturedAtMs ??= capturedAtMs;
      this.lastCapturedAtMs = capturedAtMs;
      this.worldFrameCount += 1;
    }
    const added = [];
    for (const event of Array.isArray(payload.events) ? payload.events : []) {
      if (event?.kind !== "move") continue;
      const objectId = finiteInteger(event.objectId);
      const nameMatches = this.targetName !== null
        && typeof event.name === "string"
        && event.name.toLocaleLowerCase("en-US") === this.targetName.toLocaleLowerCase("en-US");
      if (this.targetObjectId !== null && objectId !== this.targetObjectId && !nameMatches) continue;
      const fromX = finiteInteger(event.payload?.from?.x);
      const fromY = finiteInteger(event.payload?.from?.y);
      const toX = finiteInteger(event.payload?.to?.x);
      const toY = finiteInteger(event.payload?.to?.y);
      const atMs = finiteInteger(event.atMs);
      if ([fromX, fromY, toX, toY, atMs].some((value) => value === null)) continue;
      const key = moveIdentity(event);
      if (this.eventKeys.has(key)) continue;
      this.eventKeys.add(key);
      const previous = this.moves.at(-1);
      const gapMs = previous ? atMs - previous.atMs : null;
      const stepTiles = Math.max(Math.abs(toX - fromX), Math.abs(toY - fromY));
      const move = {
        atMs,
        from: { x: fromX, y: fromY },
        to: { x: toX, y: toY },
        stepTiles,
        gapMs,
        tilesPerSecond: gapMs && gapMs > 0 ? rounded(stepTiles * 1_000 / gapMs) : null,
        stalled: gapMs !== null && gapMs > this.stallMs,
      };
      this.moves.push(move);
      if (this.moves.length > MAX_RECORDED_MOVES) this.moves.shift();
      added.push(move);
    }
    return added;
  }

  observeRecordingFrame(frame) {
    const targets = Array.isArray(frame?.world?.entities)
      ? frame.world.entities
        .filter((entity) => entity?.kind === "selfPlayer" || entity?.kind === "player")
        .map((entity) => ({ objectId: entity.objectId, name: entity.name }))
      : [];
    return this.observeStatus({
      sequence: frame?.sequence,
      capturedAtMs: frame?.capturedAtMs,
      targets,
      events: frame?.events,
    });
  }

  report(config) {
    const gaps = this.moves.map((move) => move.gapMs).filter((value) => value !== null && value >= 0);
    const sortedGaps = [...gaps].sort((left, right) => left - right);
    const stalls = this.moves.filter((move) => move.stalled);
    const stepHistogram = {};
    for (const move of this.moves) {
      const key = String(move.stepTiles);
      stepHistogram[key] = (stepHistogram[key] ?? 0) + 1;
    }
    const cadence = this.moves.length < 4
      ? "insufficient-data"
      : stalls.length > 0
        ? "authoritative-stall-observed"
        : "authoritative-cadence-continuous";
    return {
      schema: SCHEMA,
      generatedAt: new Date().toISOString(),
      readOnly: true,
      sampleKind: "gateway-authoritative-move-stream",
      sourceMode: config.sourceMode,
      recording: config.recording ?? null,
      gateway: config.gatewayWs,
      spectatorDelayMs: this.spectatorDelayMs,
      directorAuthorized: this.directorAuthorized,
      map: config.map,
      requestedTarget: this.requestedTargetName,
      target: this.targetName,
      targetObjectId: this.targetObjectId,
      configuredDurationMs: config.durationMs,
      stallThresholdMs: this.stallMs,
      observedSpanMs: this.firstCapturedAtMs !== null && this.lastCapturedAtMs !== null
        ? this.lastCapturedAtMs - this.firstCapturedAtMs
        : 0,
      worldFrameCount: this.worldFrameCount,
      moveCount: this.moves.length,
      stepHistogram,
      gapMs: {
        count: gaps.length,
        min: sortedGaps.at(0) ?? null,
        median: percentile(sortedGaps, 0.5),
        p95: percentile(sortedGaps, 0.95),
        max: sortedGaps.at(-1) ?? null,
        average: gaps.length > 0 ? rounded(gaps.reduce((sum, value) => sum + value, 0) / gaps.length) : null,
      },
      stallCount: stalls.length,
      stalls,
      cadence,
      interpretation: cadence === "authoritative-cadence-continuous"
        ? "The sampled Gateway/Zone position cadence stayed below the stall threshold; investigate native presentation/frame pacing next."
        : cadence === "authoritative-stall-observed"
          ? "The authoritative position stream itself exceeded the stall threshold; inspect command/ACK timing and Zone movement scheduling."
          : "Hold movement longer or select the correct player target before drawing a cadence conclusion.",
      moves: this.moves,
    };
  }
}

class ClientTraceAnalyzer {
  constructor({ stallMs = DEFAULT_STALL_MS } = {}) {
    this.stallMs = stallMs;
    this.commands = [];
    this.acks = [];
    this.holds = [];
    this.planBlockedEvents = [];
    this.currentHold = null;
    this.pendingCommand = null;
    this.lastCommandInHold = null;
    this.holdOrdinal = 0;
    this.firstAtMs = null;
    this.lastAtMs = null;
    this.sessionProcessId = null;
    this.traceDropCount = 0;
    this.firstCapturedAtUnixMs = null;
    this.lastCapturedAtUnixMs = null;
  }

  observe(event) {
    if (!event || typeof event !== "object") return null;
    const capturedAtUnixMs = finiteInteger(event.capturedAtUnixMs);
    if (capturedAtUnixMs !== null) {
      this.firstCapturedAtUnixMs ??= capturedAtUnixMs;
      this.lastCapturedAtUnixMs = capturedAtUnixMs;
    }
    const droppedBefore = finiteInteger(event.droppedBefore);
    if (droppedBefore !== null && droppedBefore > 0) this.traceDropCount += droppedBefore;
    if (event.type === "sessionStarted") {
      this.sessionProcessId = finiteInteger(event.processId);
      return { kind: "session", processId: this.sessionProcessId };
    }
    const atMs = finiteNumber(event.atMs);
    if (atMs === null) return null;
    this.firstAtMs ??= atMs;
    this.lastAtMs = atMs;

    if (event.type === "movementHoldStarted") {
      if (this.currentHold) this.finishHold(atMs, "superseded");
      this.holdOrdinal += 1;
      this.currentHold = {
        id: this.holdOrdinal,
        mode: String(event.mode ?? "unknown"),
        startedAtMs: atMs,
        stoppedAtMs: null,
        durationMs: null,
        stopReason: null,
        commandCount: 0,
        ackCount: 0,
        terminalSilenceMs: null,
      };
      this.lastCommandInHold = null;
      return { kind: "hold-start", ...this.currentHold };
    }

    if (event.type === "movementHoldStopped") {
      const hold = this.finishHold(atMs, String(event.reason ?? "unknown"));
      return hold ? { kind: "hold-stop", ...hold } : null;
    }

    if (event.type === "movementControllerReset") {
      const hold = this.finishHold(atMs, String(event.reason ?? "controllerReset"));
      return hold ? { kind: "hold-stop", ...hold } : null;
    }

    if (event.type === "movementPlanBlocked") {
      const blocked = {
        atMs,
        capturedAtUnixMs,
        holdId: this.currentHold?.id ?? null,
        mode: String(event.mode ?? "unknown"),
        direction: String(event.direction ?? "unknown"),
        origin: { x: finiteInteger(event.originX), y: finiteInteger(event.originY) },
      };
      this.planBlockedEvents.push(blocked);
      return { kind: "plan-blocked", ...blocked };
    }

    if (event.type === "commandSent") {
      if (this.pendingCommand && this.pendingCommand.ackAtMs === null) {
        this.pendingCommand.unacknowledgedReason = "nextCommandSent";
      }
      const previous = this.currentHold ? this.lastCommandInHold : null;
      const command = {
        index: this.commands.length + 1,
        atMs,
        mode: String(event.mode ?? "unknown"),
        direction: String(event.direction ?? "unknown"),
        from: { x: finiteInteger(event.fromX), y: finiteInteger(event.fromY) },
        to: { x: finiteInteger(event.toX), y: finiteInteger(event.toY) },
        holdId: this.currentHold?.id ?? null,
        commandGapMs: previous ? rounded(atMs - previous.atMs) : null,
        postAckToNextCommandMs: previous?.ackAtMs !== null && previous?.ackAtMs !== undefined
          ? rounded(atMs - previous.ackAtMs)
          : null,
        ackAtMs: null,
        ackLatencyMs: null,
        disposition: null,
        progressed: null,
        matchedDestination: null,
        samePositionAck: null,
        unacknowledgedReason: null,
        stalled: previous ? atMs - previous.atMs > this.stallMs : false,
      };
      this.commands.push(command);
      this.pendingCommand = command;
      if (this.currentHold) {
        this.currentHold.commandCount += 1;
        this.lastCommandInHold = command;
      }
      return { kind: "command", ...command };
    }

    if (event.type === "authoritative") {
      const command = this.pendingCommand;
      const x = finiteInteger(event.x);
      const y = finiteInteger(event.y);
      const disposition = String(event.tsDisposition ?? "unknown");
      const ack = {
        index: this.acks.length + 1,
        atMs,
        packet: String(event.packet ?? "unknown"),
        x,
        y,
        disposition,
        commandIndex: command?.index ?? null,
        latencyMs: command ? rounded(atMs - command.atMs) : null,
        progressed: command && x !== null && y !== null
          ? x !== command.from.x || y !== command.from.y
          : null,
        matchedDestination: command && x !== null && y !== null
          ? x === command.to.x && y === command.to.y
          : null,
        samePositionAck: command && x !== null && y !== null
          ? x === command.from.x && y === command.from.y
          : null,
        stalled: command ? atMs - command.atMs > this.stallMs : false,
      };
      this.acks.push(ack);
      if (command) {
        command.ackAtMs = atMs;
        command.ackLatencyMs = ack.latencyMs;
        command.disposition = disposition;
        command.progressed = ack.progressed;
        command.matchedDestination = ack.matchedDestination;
        command.samePositionAck = ack.samePositionAck;
        this.pendingCommand = null;
        if (this.currentHold?.id === command.holdId) this.currentHold.ackCount += 1;
      }
      return { kind: "ack", ...ack };
    }

    if (event.type === "clear" && this.pendingCommand) {
      this.pendingCommand.unacknowledgedReason = "movementShadowCleared";
      this.pendingCommand = null;
    }
    return null;
  }

  finishHold(atMs, reason) {
    if (!this.currentHold) return null;
    const hold = this.currentHold;
    hold.stoppedAtMs = atMs;
    hold.durationMs = rounded(Math.max(0, atMs - hold.startedAtMs));
    hold.stopReason = reason;
    hold.terminalSilenceMs = this.lastCommandInHold
      ? rounded(Math.max(0, atMs - this.lastCommandInHold.atMs))
      : hold.durationMs;
    this.holds.push(hold);
    this.currentHold = null;
    this.lastCommandInHold = null;
    return hold;
  }

  report(config) {
    const reportAtUnixMs = Date.now();
    const commandGaps = this.commands.map((command) => command.commandGapMs).filter((value) => value !== null);
    const ackLatencies = this.acks.map((ack) => ack.latencyMs).filter((value) => value !== null);
    const postAckSchedule = this.commands
      .map((command) => command.postAckToNextCommandMs)
      .filter((value) => value !== null);
    const commandCadenceStalls = this.commands.filter(
      (command) => command.commandGapMs !== null && command.commandGapMs > this.stallMs,
    );
    const ackLatencyStalls = this.acks.filter(
      (ack) => ack.latencyMs !== null && ack.latencyMs > this.stallMs,
    );
    const corrections = this.acks.filter(
      (ack) => ack.disposition.toLocaleLowerCase("en-US") === "correction",
    );
    const samePositionAcks = this.acks.filter((ack) => ack.samePositionAck === true);
    const unacknowledged = this.commands.filter((command) => command.ackAtMs === null);
    const activeHold = this.currentHold
      ? {
          ...this.currentHold,
          observedUntilMs: this.lastAtMs,
          observedDurationMs: this.lastAtMs === null
            ? null
            : rounded(Math.max(0, this.lastAtMs - this.currentHold.startedAtMs)),
        }
      : null;
    const activeHoldSilenceMs = activeHold && this.lastCapturedAtUnixMs !== null
      ? Math.max(0, reportAtUnixMs - this.lastCapturedAtUnixMs)
      : null;
    const activeHoldSilenceStall = activeHoldSilenceMs !== null
      && activeHoldSilenceMs > this.stallMs;
    let cadence;
    if (corrections.length > 0 || samePositionAcks.length > 0) {
      cadence = "authoritative-correction-observed";
    } else if (ackLatencyStalls.length > 0) {
      cadence = "command-ack-stall-observed";
    } else if (commandCadenceStalls.length > 0) {
      cadence = "held-command-cadence-stall-observed";
    } else if (activeHoldSilenceStall) {
      cadence = "active-held-input-silence-observed";
    } else if (this.planBlockedEvents.length > 0) {
      cadence = "local-route-blocked-observed";
    } else if (this.commands.length < 4) {
      cadence = "insufficient-data";
    } else {
      cadence = "held-command-cadence-continuous";
    }
    let interpretation;
    if (cadence === "authoritative-correction-observed") {
      interpretation = "The server corrected at least one in-flight movement command; inspect collision/occupancy routing for those exact coordinates.";
    } else if (cadence === "command-ack-stall-observed") {
      interpretation = "At least one command waited beyond the threshold for an authoritative ACK; inspect Gateway/Zone scheduling and transport timing.";
    } else if (cadence === "held-command-cadence-stall-observed") {
      interpretation = "ACKs were timely but the client left an over-threshold gap between commands during one continuous movement hold; inspect native pacing/input/planning.";
    } else if (cadence === "active-held-input-silence-observed") {
      interpretation = "The client reported an active movement hold, then stopped producing trace events beyond the threshold; inspect the native main loop or a fully stalled input path.";
    } else if (cadence === "local-route-blocked-observed") {
      interpretation = "The local Crystal-style preflight found direct and adjacent routes blocked while movement remained held; compare the recorded tile with map and live occupancy.";
    } else if (cadence === "held-command-cadence-continuous") {
      interpretation = "Command cadence and command-to-ACK latency stayed below the threshold during continuous movement holds; investigate presentation/frame pacing if a visual flash remains.";
    } else {
      interpretation = "Hold movement through at least four commands before drawing a cadence conclusion.";
    }
    return {
      schema: SCHEMA,
      generatedAt: new Date().toISOString(),
      readOnly: true,
      sampleKind: "native-client-command-ack",
      sourceMode: config.sourceMode,
      clientTrace: config.clientTrace,
      processId: this.sessionProcessId,
      configuredDurationMs: config.durationMs,
      stallThresholdMs: this.stallMs,
      observedSpanMs: this.firstAtMs !== null && this.lastAtMs !== null
        ? rounded(this.lastAtMs - this.firstAtMs)
        : 0,
      traceDropCount: this.traceDropCount,
      holdCount: this.holds.length + (activeHold ? 1 : 0),
      completedHolds: this.holds,
      activeHold,
      activeHoldSilenceMs,
      activeHoldSilenceStall,
      planBlockedCount: this.planBlockedEvents.length,
      planBlockedEvents: this.planBlockedEvents,
      commandCount: this.commands.length,
      ackCount: this.acks.length,
      unacknowledgedCount: unacknowledged.length,
      correctionCount: corrections.length,
      samePositionAckCount: samePositionAcks.length,
      commandGapMs: numericStats(commandGaps),
      ackLatencyMs: numericStats(ackLatencies),
      postAckToNextCommandMs: numericStats(postAckSchedule),
      commandCadenceStallCount: commandCadenceStalls.length,
      ackLatencyStallCount: ackLatencyStalls.length,
      commandCadenceStalls,
      ackLatencyStalls,
      cadence,
      interpretation,
      commands: this.commands,
      acks: this.acks,
    };
  }
}

async function discoverMap(gatewayHttp, token) {
  const url = new URL(`${gatewayHttp}/spectator/matches`);
  url.searchParams.set("token", token);
  const response = await fetch(url);
  if (!response.ok) throw new Error(`spectator match discovery failed: HTTP ${response.status}`);
  const payload = await response.json();
  const match = payload.matches?.[0];
  if (!match?.mapFileName) throw new Error("spectator match discovery found no active map");
  return String(match.mapFileName);
}

function openSpectator(url, analyzer, durationMs) {
  return new Promise((resolve, reject) => {
    const socket = new WebSocket(url);
    let settled = false;
    let authorized = false;
    const finish = (reason) => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      try { socket.close(); } catch {}
      resolve(reason);
    };
    const timer = setTimeout(() => finish("duration"), durationMs);
    const interrupt = () => finish("interrupt");
    process.once("SIGINT", interrupt);
    socket.addEventListener("open", () => {
      console.log(`[movement-monitor] connected read-only; sampling ${durationMs} ms`);
    });
    socket.addEventListener("message", (message) => {
      let decoded;
      try {
        decoded = JSON.parse(String(message.data));
      } catch {
        return;
      }
      if (decoded.type === "error") {
        if (!settled) reject(new Error(`spectator error: ${decoded.message ?? "unknown"}`));
        settled = true;
        clearTimeout(timer);
        return;
      }
      if (decoded.type !== "spectatorStatus") return;
      const payload = decoded.payload;
      if (!authorized) {
        if (payload?.readOnly !== true) {
          settled = true;
          clearTimeout(timer);
          socket.close();
          reject(new Error("spectator transport did not prove read-only mode"));
          return;
        }
        analyzer.spectatorDelayMs = finiteInteger(payload.delayMs);
        analyzer.directorAuthorized = payload.directorAuthorized === true;
        console.log(`[movement-monitor] spectator delay=${analyzer.spectatorDelayMs ?? "unknown"}ms director=${analyzer.directorAuthorized}`);
        authorized = true;
      }
      const priorTarget = analyzer.targetObjectId;
      const moves = analyzer.observeStatus(payload);
      if (priorTarget === null && analyzer.targetObjectId !== null) {
        console.log(`[movement-monitor] target=${analyzer.targetName} objectId=${analyzer.targetObjectId}`);
      }
      for (const move of moves) {
        const gap = move.gapMs === null ? "first" : `${move.gapMs}ms`;
        const alert = move.stalled ? " STALL" : "";
        console.log(`[movement-monitor] ${move.from.x},${move.from.y} -> ${move.to.x},${move.to.y} step=${move.stepTiles} gap=${gap}${alert}`);
      }
    });
    socket.addEventListener("error", () => {
      if (!settled) {
        settled = true;
        clearTimeout(timer);
        reject(new Error(`WebSocket connection failed: ${url.origin}`));
      }
    });
    socket.addEventListener("close", () => {
      process.removeListener("SIGINT", interrupt);
      if (!settled) {
        settled = true;
        clearTimeout(timer);
        reject(new Error("spectator socket closed before the sample completed"));
      }
    });
  });
}

function printObservedMoves(analyzer, priorTarget, moves) {
  if (priorTarget === null && analyzer.targetObjectId !== null) {
    console.log(`[movement-monitor] target=${analyzer.targetName} objectId=${analyzer.targetObjectId}`);
  }
  for (const move of moves) {
    const gap = move.gapMs === null ? "first" : `${move.gapMs}ms`;
    const alert = move.stalled ? " STALL" : "";
    console.log(`[movement-monitor] ${move.from.x},${move.from.y} -> ${move.to.x},${move.to.y} step=${move.stepTiles} gap=${gap}${alert}`);
  }
}

async function tailRecording(recording, analyzer, durationMs) {
  const absolute = path.resolve(recording);
  const initial = await fs.stat(absolute);
  if (!initial.isFile()) throw new Error(`spectator recording is not a file: ${absolute}`);
  let offset = initial.size;
  let carry = "";
  let interrupted = false;
  const interrupt = () => { interrupted = true; };
  process.once("SIGINT", interrupt);
  analyzer.spectatorDelayMs = 0;
  analyzer.directorAuthorized = true;
  console.log(`[movement-monitor] tailing local read-only recording=${absolute}`);
  console.log(`[movement-monitor] sampling ${durationMs} ms from byte ${offset}`);
  const deadline = Date.now() + durationMs;
  try {
    while (!interrupted && Date.now() < deadline) {
      const current = await fs.stat(absolute);
      if (current.size < offset) {
        offset = 0;
        carry = "";
      }
      if (current.size > offset) {
        const handle = await fs.open(absolute, "r");
        try {
          while (offset < current.size) {
            const length = Math.min(current.size - offset, 1024 * 1024);
            const buffer = Buffer.allocUnsafe(length);
            const { bytesRead } = await handle.read(buffer, 0, length, offset);
            if (bytesRead <= 0) break;
            offset += bytesRead;
            carry += buffer.subarray(0, bytesRead).toString("utf8");
            const lines = carry.split("\n");
            carry = lines.pop() ?? "";
            for (const line of lines) {
              if (line.trim() === "") continue;
              let frame;
              try {
                frame = JSON.parse(line);
              } catch {
                continue;
              }
              const priorTarget = analyzer.targetObjectId;
              const moves = analyzer.observeRecordingFrame(frame);
              printObservedMoves(analyzer, priorTarget, moves);
            }
          }
        } finally {
          await handle.close();
        }
      }
      await new Promise((resolve) => setTimeout(resolve, 100));
    }
  } finally {
    process.removeListener("SIGINT", interrupt);
  }
  return absolute;
}

function printClientTraceObservation(observation) {
  if (!observation) return;
  if (observation.kind === "hold-start") {
    console.log(`[movement-monitor] hold#${observation.id} ${observation.mode} started`);
  } else if (observation.kind === "hold-stop") {
    console.log(`[movement-monitor] hold#${observation.id} stopped reason=${observation.stopReason} duration=${observation.durationMs}ms commands=${observation.commandCount}`);
  } else if (observation.kind === "command") {
    const gap = observation.commandGapMs === null ? "first" : `${observation.commandGapMs}ms`;
    console.log(`[movement-monitor] command#${observation.index} ${observation.mode} ${observation.from.x},${observation.from.y} -> ${observation.to.x},${observation.to.y} gap=${gap}`);
  } else if (observation.kind === "ack") {
    const latency = observation.latencyMs === null ? "unpaired" : `${observation.latencyMs}ms`;
    const alert = observation.stalled ? " STALL" : "";
    console.log(`[movement-monitor] ack#${observation.index} command=${observation.commandIndex ?? "none"} position=${observation.x},${observation.y} latency=${latency} disposition=${observation.disposition}${alert}`);
  } else if (observation.kind === "plan-blocked") {
    console.log(`[movement-monitor] route blocked hold=${observation.holdId ?? "none"} origin=${observation.origin.x},${observation.origin.y} direction=${observation.direction} mode=${observation.mode}`);
  }
}

async function tailClientTrace(clientTrace, analyzer, durationMs, fromStart) {
  const absolute = path.resolve(clientTrace);
  const initial = await fs.stat(absolute);
  if (!initial.isFile()) throw new Error(`native movement trace is not a file: ${absolute}`);
  let offset = fromStart ? 0 : initial.size;
  let carry = "";
  let interrupted = false;
  const interrupt = () => { interrupted = true; };
  process.once("SIGINT", interrupt);
  console.log(`[movement-monitor] tailing native client trace=${absolute}`);
  console.log(`[movement-monitor] sampling ${durationMs} ms from byte ${offset}`);
  const deadline = Date.now() + durationMs;
  try {
    while (!interrupted && Date.now() < deadline) {
      const current = await fs.stat(absolute);
      if (current.size < offset) {
        offset = 0;
        carry = "";
      }
      if (current.size > offset) {
        const handle = await fs.open(absolute, "r");
        try {
          while (offset < current.size) {
            const length = Math.min(current.size - offset, 1024 * 1024);
            const buffer = Buffer.allocUnsafe(length);
            const { bytesRead } = await handle.read(buffer, 0, length, offset);
            if (bytesRead <= 0) break;
            offset += bytesRead;
            carry += buffer.subarray(0, bytesRead).toString("utf8");
            const lines = carry.split("\n");
            carry = lines.pop() ?? "";
            for (const line of lines) {
              if (line.trim() === "") continue;
              let event;
              try {
                event = JSON.parse(line);
              } catch {
                continue;
              }
              printClientTraceObservation(analyzer.observe(event));
            }
          }
        } finally {
          await handle.close();
        }
      }
      await new Promise((resolve) => setTimeout(resolve, 50));
    }
  } finally {
    process.removeListener("SIGINT", interrupt);
  }
  return absolute;
}

function runSelfTest() {
  const analyzer = new MovementAnalyzer({ targetName: "Scout", stallMs: 850 });
  const status = (sequence, capturedAtMs, event) => ({
    sequence,
    capturedAtMs,
    targets: [{ objectId: 42, name: "Scout" }],
    events: [event],
  });
  const move = (atMs, fromX, toX) => ({
    kind: "move",
    atMs,
    objectId: 42,
    name: "Scout",
    payload: { from: { x: fromX, y: 10 }, to: { x: toX, y: 10 } },
  });
  analyzer.observeStatus(status(1, 1_000, move(1_000, 10, 11)));
  analyzer.observeStatus(status(2, 1_600, move(1_600, 11, 13)));
  analyzer.observeStatus(status(3, 2_200, move(2_200, 13, 15)));
  analyzer.observeStatus(status(3, 2_200, move(2_200, 13, 15)));
  let report = analyzer.report({ gatewayWs: DEFAULT_GATEWAY_WS, map: "0", durationMs: 2_000, sourceMode: "self-test" });
  assert.equal(report.moveCount, 3);
  assert.equal(report.stallCount, 0);
  assert.deepEqual(report.stepHistogram, { 1: 1, 2: 2 });
  analyzer.observeStatus(status(4, 3_201, move(3_201, 15, 17)));
  report = analyzer.report({ gatewayWs: DEFAULT_GATEWAY_WS, map: "0", durationMs: 3_000, sourceMode: "self-test" });
  assert.equal(report.stallCount, 1);
  assert.equal(report.cadence, "authoritative-stall-observed");

  const client = new ClientTraceAnalyzer({ stallMs: 850 });
  client.observe({ type: "sessionStarted", processId: 123 });
  client.observe({ type: "movementHoldStarted", atMs: 100, mode: "run" });
  client.observe({ type: "commandSent", atMs: 110, mode: "walk", direction: "Right", fromX: 1, fromY: 1, toX: 2, toY: 1 });
  client.observe({ type: "authoritative", atMs: 210, packet: "UserLocation", x: 2, y: 1, tsDisposition: "confirmed" });
  client.observe({ type: "commandSent", atMs: 710, mode: "run", direction: "Right", fromX: 2, fromY: 1, toX: 4, toY: 1 });
  client.observe({ type: "authoritative", atMs: 800, packet: "UserLocation", x: 4, y: 1, tsDisposition: "confirmed" });
  client.observe({ type: "commandSent", atMs: 1_800, mode: "run", direction: "Right", fromX: 4, fromY: 1, toX: 6, toY: 1 });
  client.observe({ type: "authoritative", atMs: 2_800, packet: "UserLocation", x: 4, y: 1, tsDisposition: "correction" });
  client.observe({ type: "commandSent", atMs: 2_900, mode: "run", direction: "Down", fromX: 4, fromY: 1, toX: 4, toY: 3 });
  client.observe({ type: "authoritative", atMs: 3_000, packet: "UserLocation", x: 4, y: 3, tsDisposition: "confirmed" });
  client.observe({ type: "movementHoldStopped", atMs: 3_100, mode: "run", reason: "buttonReleased" });
  client.observe({ type: "movementHoldStarted", atMs: 10_000, mode: "run" });
  client.observe({ type: "commandSent", atMs: 10_010, mode: "walk", direction: "Left", fromX: 4, fromY: 3, toX: 3, toY: 3 });
  client.observe({ type: "authoritative", atMs: 10_110, packet: "UserLocation", x: 3, y: 3, tsDisposition: "confirmed" });
  client.observe({ type: "movementHoldStopped", atMs: 10_200, mode: "run", reason: "buttonReleased" });
  const clientReport = client.report({ sourceMode: "self-test", clientTrace: "synthetic.jsonl", durationMs: 3_000 });
  assert.equal(clientReport.commandCount, 5);
  assert.equal(clientReport.ackCount, 5);
  assert.equal(clientReport.commandCadenceStallCount, 2);
  assert.equal(clientReport.ackLatencyStallCount, 1);
  assert.equal(clientReport.correctionCount, 1);
  assert.equal(clientReport.samePositionAckCount, 1);
  assert.equal(clientReport.traceDropCount, 0);
  assert.equal(clientReport.commands[4].commandGapMs, null, "pause between holds must be excluded");
  assert.equal(clientReport.cadence, "authoritative-correction-observed");
  console.log("monitor-native-movement self-test passed");
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  if (args.help) {
    printUsage();
    return;
  }
  if (args.selfTest) {
    runSelfTest();
    return;
  }
  const gatewayWs = normalizedBase(args.gatewayWs ?? process.env.MIR2_MOVEMENT_MONITOR_WS ?? DEFAULT_GATEWAY_WS, ["ws:", "wss:"]);
  const gatewayHttp = normalizedBase(args.gatewayHttp ?? process.env.MIR2_MOVEMENT_MONITOR_HTTP ?? DEFAULT_GATEWAY_HTTP, ["http:", "https:"]);
  const token = args.token ?? process.env.MIR2_MOVEMENT_MONITOR_TOKEN ?? DEFAULT_TOKEN;
  const durationMs = boundedInteger(args.durationMs ?? process.env.MIR2_MOVEMENT_MONITOR_DURATION_MS, DEFAULT_DURATION_MS, 1_000, 30 * 60_000, "durationMs");
  const stallMs = boundedInteger(args.stallMs ?? process.env.MIR2_MOVEMENT_MONITOR_STALL_MS, DEFAULT_STALL_MS, 100, 60_000, "stallMs");
  const targetName = args.target ?? process.env.MIR2_MOVEMENT_MONITOR_TARGET ?? null;
  const output = args.output ?? process.env.MIR2_MOVEMENT_MONITOR_OUTPUT ?? null;
  const requestedRecording = args.recording ?? process.env.MIR2_MOVEMENT_MONITOR_RECORDING ?? null;
  const requestedClientTrace = args.clientTrace ?? process.env.MIR2_MOVEMENT_MONITOR_CLIENT_TRACE ?? null;
  if (requestedRecording && requestedClientTrace) {
    throw new Error("choose either --client-trace or --recording, not both");
  }
  const map = args.map
    ?? process.env.MIR2_MOVEMENT_MONITOR_MAP
    ?? (requestedClientTrace ? "client-trace" : requestedRecording ? "recording" : await discoverMap(gatewayHttp, token));
  const analyzer = requestedClientTrace
    ? new ClientTraceAnalyzer({ stallMs })
    : new MovementAnalyzer({ targetName, stallMs });
  let recording = null;
  let clientTrace = null;
  let sourceMode;
  if (requestedClientTrace) {
    sourceMode = "native-client-trace-tail";
    clientTrace = await tailClientTrace(requestedClientTrace, analyzer, durationMs, args.fromStart === true);
  } else if (requestedRecording) {
    sourceMode = "local-recording-tail";
    recording = await tailRecording(requestedRecording, analyzer, durationMs);
  } else {
    if (typeof WebSocket !== "function") {
      throw new Error("spectator WebSocket mode requires a Node runtime with global WebSocket support");
    }
    sourceMode = "spectator-websocket";
    const url = new URL(`${gatewayWs}/spectator/ws`);
    url.searchParams.set("map", map);
    url.searchParams.set("delayMs", "0");
    url.searchParams.set("mode", "director");
    url.searchParams.set("token", token);
    await openSpectator(url, analyzer, durationMs);
  }
  const report = requestedClientTrace
    ? analyzer.report({ durationMs, sourceMode, clientTrace })
    : analyzer.report({ gatewayWs, map, durationMs, sourceMode, recording });
  if (output) {
    const absolute = path.resolve(output);
    await fs.mkdir(path.dirname(absolute), { recursive: true });
    await fs.writeFile(absolute, `${JSON.stringify(report, null, 2)}\n`, "utf8");
    report.output = absolute;
  }
  console.log(JSON.stringify(report, null, 2));
  const observationCount = requestedClientTrace ? report.commandCount : report.moveCount;
  if (observationCount < 4) process.exitCode = 2;
}

main().catch((error) => {
  console.error(`[movement-monitor] ${error.stack ?? error.message ?? error}`);
  process.exitCode = 1;
});
