import fsSync from "node:fs";
import fs from "node:fs/promises";
import path from "node:path";
import sharp from "sharp";

const args = parseArgs(process.argv.slice(2));
const outputDir = path.resolve(args.output ?? path.join("docs", "generated", "player-qa", "movement-jitter"));
const prefix = args.prefix ?? `movement-temporal-${Date.now()}`;
const frameAnalysisOptions = {
  enabled: booleanArg(args.analyzeFrames ?? "true", true),
  width: numberArg(args.frameDiffWidth, 256),
  meanDeltaThreshold: numberArg(args.frameDiffMeanThreshold, 0.75),
  pixelDeltaThreshold: numberArg(args.frameDiffPixelThreshold, 12),
  changedPixelRatioThreshold: numberArg(args.frameDiffChangedRatioThreshold, 0.003),
  alignActions: booleanArg(args.alignActions ?? "true", true),
  preActionMs: numberArg(args.preActionMs, 300),
  postActionMs: numberArg(args.postActionMs, 1200),
};

if (!args.original || !args.web) {
  throw new Error("Usage: node report-movement-temporal-parity.mjs --original <json> --web <json> [--webBichon <json>] [--output <dir>] [--prefix <name>]");
}

const originalPath = path.resolve(args.original);
const webPath = path.resolve(args.web);
const webBichonPath = args.webBichon ? path.resolve(args.webBichon) : null;

const original = await readJson(originalPath);
const web = await readJson(webPath);
const webBichon = webBichonPath ? await readJson(webBichonPath) : null;
const originalSummary = await summarizeOriginal(original, originalPath, frameAnalysisOptions);
const webSummary = await summarizeWeb(web, webPath, frameAnalysisOptions);
const webBichonSummary = webBichon
  ? await summarizeWeb(webBichon, webBichonPath, frameAnalysisOptions)
  : null;
const actionAlignmentOk =
  !frameAnalysisOptions.alignActions ||
  (originalSummary.frameCadence?.actionAlignment?.applied === true &&
    webSummary.frameCadence?.actionAlignment?.applied === true);

const report = {
  ok: Boolean(originalSummary.ok) && Boolean(webSummary.ok) && actionAlignmentOk,
  generatedAt: new Date().toISOString(),
  frameAnalysisOptions,
  actionAlignmentOk,
  original: originalSummary,
  web: webSummary,
  webBichon: webBichonSummary,
  interpretation: buildInterpretation(originalSummary, webSummary, webBichonSummary),
};

await fs.mkdir(outputDir, { recursive: true });
const jsonPath = path.join(outputDir, `${prefix}.json`);
const mdPath = path.join(outputDir, `${prefix}.md`);
await fs.writeFile(jsonPath, `${JSON.stringify(report, null, 2)}\n`, "utf8");
await fs.writeFile(mdPath, renderMarkdown(report), "utf8");
console.log(JSON.stringify({ ok: report.ok, jsonPath, mdPath }, null, 2));

async function readJson(filePath) {
  const raw = await fs.readFile(filePath, "utf8");
  return JSON.parse(raw.replace(/^\uFEFF/, ""));
}

async function summarizeOriginal(report, filePath, analysisOptions) {
  const samples = Array.isArray(report.samples) ? report.samples : [];
  const actionTimeline = extractActionTimeline(report, "original");
  return {
    path: filePath,
    ok: Boolean(report.ok),
    window: report.window ?? null,
    sampleMs: report.sampleMs ?? null,
    holdMs: report.holdMs ?? null,
    stepWaitMs: report.stepWaitMs ?? null,
    sampleCount: samples.length,
    actionCount: actionTimeline.count,
    actionTimeline,
    labels: summarizeOriginalLabels(samples),
    frameCadence: analysisOptions.enabled
      ? await analyzeFrameCadence(
          samples,
          filePath,
          report,
          "original",
          analysisOptions,
          actionTimeline,
        )
      : null,
  };
}

function summarizeOriginalLabels(samples) {
  const groups = new Map();
  for (const sample of samples) {
    const label = sample?.label ?? "unknown";
    const group = groups.get(label) ?? [];
    group.push(sample);
    groups.set(label, group);
  }
  return Array.from(groups, ([label, group]) => {
    const elapsed = group.map((sample) => Number(sample.elapsedMs)).filter(Number.isFinite);
    return {
      label,
      count: group.length,
      firstElapsedMs: elapsed[0] ?? null,
      lastElapsedMs: elapsed[elapsed.length - 1] ?? null,
      averageDeltaMs: averageDelta(elapsed),
    };
  });
}

function extractActionTimeline(report, kind) {
  const entries = [];
  const append = (entry, elapsedMs, source) => {
    const normalizedElapsedMs = Number(elapsedMs);
    if (!Number.isFinite(normalizedElapsedMs)) return;
    entries.push({
      label: entry?.label ?? `${kind}-action-${entries.length + 1}`,
      type: entry?.type ?? entry?.button ?? null,
      elapsedMs: normalizedElapsedMs,
      scheduledAtMs: Number.isFinite(Number(entry?.scheduledAtMs))
        ? Number(entry.scheduledAtMs)
        : Number.isFinite(Number(entry?.atMs))
          ? Number(entry.atMs)
          : null,
      source,
    });
  };

  if (kind === "original") {
    for (const action of Array.isArray(report?.actions) ? report.actions : []) {
      append(
        action,
        action?.performedAtCaptureMs ?? action?.performedAtMs ?? action?.atMs,
        "native-action",
      );
    }
  } else {
    for (const action of Array.isArray(report?.actions) ? report.actions : []) {
      const clicks = Array.isArray(action?.dispatch?.clicks) ? action.dispatch.clicks : [];
      for (const click of clicks) {
        const captureStartedAtMs = Number(action?.dispatch?.captureStartedAtMs);
        const sequenceStartedAtMs = Number(action?.dispatch?.sequenceStartedAtMs);
        const performedAtMs = Number(click?.performedAtMs);
        const derivedElapsedMs =
          Number.isFinite(captureStartedAtMs) &&
          Number.isFinite(sequenceStartedAtMs) &&
          Number.isFinite(performedAtMs)
            ? sequenceStartedAtMs + performedAtMs - captureStartedAtMs
            : null;
        append(
          click,
          click?.performedAtCaptureMs ?? derivedElapsedMs,
          "web-click",
        );
      }
    }

    if (entries.length === 0 && Number.isFinite(Number(report?.frameImageCaptureStartedAtMs))) {
      const captureStartedAtMs = Number(report.frameImageCaptureStartedAtMs);
      for (const frame of Array.isArray(report?.movementWebSocketFramesSent)
        ? report.movementWebSocketFramesSent
        : []) {
        const payload = parsePayload(frame);
        if (!["walk", "run", "moveTo"].includes(payload?.type)) continue;
        append(
          { label: payload.type, type: payload.type },
          Number(frame?.at) - captureStartedAtMs,
          "web-command-fallback",
        );
      }
    }
  }

  entries.sort((left, right) => left.elapsedMs - right.elapsedMs);
  const firstActionMs = entries[0]?.elapsedMs ?? null;
  const lastActionMs = entries.at(-1)?.elapsedMs ?? null;
  return {
    count: entries.length,
    firstActionMs,
    lastActionMs,
    spanMs:
      Number.isFinite(firstActionMs) && Number.isFinite(lastActionMs)
        ? lastActionMs - firstActionMs
        : null,
    entries,
  };
}

async function summarizeWeb(report, filePath, analysisOptions) {
  const sent = Array.isArray(report.movementWebSocketFramesSent) ? report.movementWebSocketFramesSent : [];
  const received = Array.isArray(report.movementWebSocketFramesReceived) ? report.movementWebSocketFramesReceived : [];
  const selfAckLatencies = matchSelfAckLatencies(sent, received);
  const failedAssertions = Array.isArray(report.assertions)
    ? report.assertions.filter((assertion) => assertion?.pass === false).map((assertion) => assertion.name)
    : [];
  const actionTimeline = extractActionTimeline(report, "web");

  return {
    path: filePath,
    ok: Boolean(report.ok),
    interaction: report.interaction ?? null,
    map: report.finalState?.mapFileName ?? report.startTarget?.map ?? null,
    mapTitle: report.finalState?.mapTitle ?? null,
    startTarget: report.startTarget ?? null,
    finalPlayer: report.finalState?.player ?? null,
    sampleMs: report.sampleMs ?? null,
    routeStepMs: report.routeStepMs ?? null,
    clickHoldMs: report.clickHoldMs ?? null,
    sampleCount: report.sampleCount ?? null,
    frameImageDir: report.frameImageDir ?? null,
    frameImageCount: report.frameImageCount ?? 0,
    actionTimeline,
    movementCommandCounts: countMovementCommands(sent),
    selfAckLatency: summarizeLatencies(selfAckLatencies),
    failedAssertions,
    interactionPollution: summarizeInteractionPollution(report.interactionPollution, report.webSocketFramesSentTail),
    feelMetrics: report.feelMetrics ?? null,
    frameCadence: analysisOptions.enabled
      ? await analyzeFrameCadence(
          Array.isArray(report.samples) ? report.samples : [],
          filePath,
          report,
          "web",
          analysisOptions,
          actionTimeline,
        )
      : null,
    nonFaviconNetwork404s: Array.isArray(report.nonFaviconNetwork404s)
      ? report.nonFaviconNetwork404s.length
      : null,
    criticalConsoleErrors: Array.isArray(report.criticalConsoleErrors) ? report.criticalConsoleErrors.length : null,
  };
}

function matchSelfAckLatencies(sentFrames, receivedFrames) {
  const latencies = [];
  let receiveIndex = 0;
  for (const sent of sentFrames) {
    const sentPayload = parsePayload(sent);
    if (!["walk", "run", "moveTo"].includes(sentPayload?.type) || !Number.isFinite(sent?.at)) {
      continue;
    }

    let received = null;
    while (receiveIndex < receivedFrames.length) {
      const candidate = receivedFrames[receiveIndex];
      receiveIndex += 1;
      const receivedPayload = parsePayload(candidate);
      if (Number.isFinite(candidate?.at) && candidate.at >= sent.at && isSelfMovementAck(receivedPayload)) {
        received = candidate;
        break;
      }
    }

    latencies.push({
      command: sentPayload,
      sentAt: sent.at,
      receivedAt: received?.at ?? null,
      latencyMs: received ? received.at - sent.at : null,
      receivedPacket: received ? parsePayload(received) : null,
    });
  }
  return latencies;
}

function summarizeLatencies(latencies) {
  const values = latencies.map((entry) => entry.latencyMs).filter(Number.isFinite);
  return {
    count: latencies.length,
    missingCount: latencies.filter((entry) => entry.latencyMs === null).length,
    maxMs: values.length ? Math.max(...values) : null,
    averageMs: values.length ? Math.round((values.reduce((sum, value) => sum + value, 0) / values.length) * 100) / 100 : null,
    entries: latencies,
  };
}

function countMovementCommands(frames) {
  const counts = { walk: 0, run: 0, moveTo: 0, other: 0 };
  for (const frame of frames) {
    const payload = parsePayload(frame);
    if (payload?.type in counts) {
      counts[payload.type] += 1;
    } else {
      counts.other += 1;
    }
  }
  return counts;
}

function buildInterpretation(original, web, webBichon) {
  const notes = [];
  notes.push("Native Crystal temporal capture is automated; current native evidence contains window frame images but no packet telemetry.");
  if (
    original.frameCadence?.actionAlignment?.applied === true &&
    web.frameCadence?.actionAlignment?.applied === true
  ) {
    notes.push(
      `Native and Web frame streams are clipped to the same first-action-relative window (${original.frameCadence.actionAlignment.clipStartMs}ms to ${original.frameCadence.actionAlignment.clipEndMs}ms native; ${web.frameCadence.actionAlignment.clipStartMs}ms to ${web.frameCadence.actionAlignment.clipEndMs}ms Web).`,
    );
  } else {
    notes.push("Native/Web action alignment is unavailable; frame cadence must not be treated as exact movement-time evidence.");
  }
  if (web.ok) {
    notes.push(`The Web ${web.interaction ?? "movement"} capture is strict-green, with responsive self ACKs and no failed harness assertions.`);
  }
  const nativeActionSpanMs = Number(original.actionTimeline?.spanMs);
  const webActionSpanMs = Number(web.actionTimeline?.spanMs);
  if (Number.isFinite(nativeActionSpanMs) && Number.isFinite(webActionSpanMs)) {
    notes.push(
      `The aligned action spans are Crystal ${nativeActionSpanMs}ms and Web ${webActionSpanMs}ms (absolute delta ${Math.abs(webActionSpanMs - nativeActionSpanMs)}ms).`,
    );
  }
  const movementCommandCount =
    (web.movementCommandCounts?.walk ?? 0) + (web.movementCommandCounts?.run ?? 0);
  const expectedSixPhasePairs = movementCommandCount * 6;
  if (
    expectedSixPhasePairs > 0 &&
    web.frameCadence?.activePairCount === expectedSixPhasePairs
  ) {
    notes.push(
      `The aligned Web stream contains ${web.frameCadence.activePairCount} active frame pairs, numerically matching ${movementCommandCount} movement commands x six Crystal movement phases; this supports complete phase activity but is not an actor-isolated pixel proof.`,
    );
  }
  if (webBichon && !webBichon.ok) {
    const pollutedByEntityHit = (webBichon.interactionPollution?.entityHitClickCount ?? 0) > 0;
    const pollutedByGameplayAction = Object.entries(webBichon.interactionPollution?.nonMovementGameplayFrameTypes ?? {})
      .some(([type, count]) => type !== "transferMap" && Number(count) > 0);
    if (pollutedByEntityHit || pollutedByGameplayAction) {
      notes.push("The Web Bichon click-route is non-green but polluted by entity/gameplay action frames, so it should not be treated as a clean movement-only gap.");
    } else {
      notes.push("The Web Bichon click-route remains non-green and records the remaining crowded-AOI / blocked-route feel gap.");
    }
  }
  if (webBichon) {
    const runCount = webBichon.movementCommandCounts?.run ?? 0;
    if (runCount > 0) {
      notes.push("The Bichon failure sample includes a run command, so the previous right-click-to-walk input-semantics gap is closed.");
    }
  }
  notes.push(...compareFrameCadence(original.frameCadence, web.frameCadence, "Web movement capture"));
  if (webBichon?.frameCadence) {
    notes.push(...compareFrameCadence(original.frameCadence, webBichon.frameCadence, "Web Bichon route"));
  }
  return notes;
}

async function analyzeFrameCadence(
  samples,
  reportPath,
  report,
  kind,
  options,
  actionTimeline,
) {
  const firstActionMs = Number(actionTimeline?.firstActionMs);
  const lastActionMs = Number(actionTimeline?.lastActionMs);
  const alignmentApplied =
    options.alignActions &&
    Number(actionTimeline?.count) > 0 &&
    Number.isFinite(firstActionMs) &&
    Number.isFinite(lastActionMs);
  const clipStartMs = alignmentApplied ? -Math.max(0, options.preActionMs) : null;
  const clipEndMs = alignmentApplied
    ? lastActionMs - firstActionMs + Math.max(0, options.postActionMs)
    : null;
  const groups = new Map();
  let sourceFrameCount = 0;
  let excludedFrameCount = 0;
  for (const sample of samples) {
    const framePath = resolveSampleFramePath(sample, reportPath, report, kind);
    if (!framePath) {
      continue;
    }
    sourceFrameCount += 1;
    const captureElapsedMs = sampleElapsedMs(sample);
    const elapsedMs =
      alignmentApplied && Number.isFinite(captureElapsedMs)
        ? captureElapsedMs - firstActionMs
        : captureElapsedMs;
    if (
      alignmentApplied &&
      (!Number.isFinite(elapsedMs) || elapsedMs < clipStartMs || elapsedMs > clipEndMs)
    ) {
      excludedFrameCount += 1;
      continue;
    }
    const label = sample?.label ?? "unknown";
    const group = groups.get(label) ?? [];
    group.push({
      label,
      path: framePath,
      elapsedMs,
      captureElapsedMs,
      index: Number.isFinite(Number(sample?.index)) ? Number(sample.index) : null,
      capturedAt: Number.isFinite(Number(sample?.capturedAt)) ? Number(sample.capturedAt) : null,
    });
    groups.set(label, group);
  }

  const labels = [];
  for (const [label, frames] of groups) {
    frames.sort(compareFrameSamples);
    const pairs = [];
    for (let index = 1; index < frames.length; index += 1) {
      const previous = frames[index - 1];
      const current = frames[index];
      try {
        const diff = await diffFramePair(previous.path, current.path, options);
        const elapsedDeltaMs =
          Number.isFinite(previous.elapsedMs) && Number.isFinite(current.elapsedMs)
            ? current.elapsedMs - previous.elapsedMs
            : null;
        pairs.push({
          fromIndex: previous.index,
          toIndex: current.index,
          fromElapsedMs: previous.elapsedMs,
          toElapsedMs: current.elapsedMs,
          fromCaptureElapsedMs: previous.captureElapsedMs,
          toCaptureElapsedMs: current.captureElapsedMs,
          elapsedDeltaMs,
          meanAbsDelta: roundMetric(diff.meanAbsDelta),
          changedPixelRatio: roundMetric(diff.changedPixelRatio, 6),
          active:
            diff.meanAbsDelta >= options.meanDeltaThreshold ||
            diff.changedPixelRatio >= options.changedPixelRatioThreshold,
        });
      } catch (error) {
        pairs.push({
          fromIndex: previous.index,
          toIndex: current.index,
          fromElapsedMs: previous.elapsedMs,
          toElapsedMs: current.elapsedMs,
          fromCaptureElapsedMs: previous.captureElapsedMs,
          toCaptureElapsedMs: current.captureElapsedMs,
          elapsedDeltaMs: null,
          error: error instanceof Error ? error.message : String(error),
          active: false,
        });
      }
    }
    labels.push(summarizeFrameCadenceLabel(label, frames, pairs));
  }

  return {
    enabled: true,
    kind,
    diffWidth: options.width,
    thresholds: {
      meanDelta: options.meanDeltaThreshold,
      pixelDelta: options.pixelDeltaThreshold,
      changedPixelRatio: options.changedPixelRatioThreshold,
    },
    actionAlignment: {
      requested: options.alignActions,
      applied: alignmentApplied,
      firstActionMs: alignmentApplied ? firstActionMs : null,
      lastActionMs: alignmentApplied ? lastActionMs : null,
      actionSpanMs: alignmentApplied ? lastActionMs - firstActionMs : null,
      preActionMs: alignmentApplied ? Math.max(0, options.preActionMs) : null,
      postActionMs: alignmentApplied ? Math.max(0, options.postActionMs) : null,
      clipStartMs,
      clipEndMs,
      sourceFrameCount,
      includedFrameCount: sourceFrameCount - excludedFrameCount,
      excludedFrameCount,
      actions: alignmentApplied
        ? actionTimeline.entries.map((entry) => ({
            ...entry,
            actionRelativeMs: entry.elapsedMs - firstActionMs,
          }))
        : [],
    },
    labelCount: labels.length,
    frameCount: labels.reduce((sum, label) => sum + label.frameCount, 0),
    pairCount: labels.reduce((sum, label) => sum + label.pairCount, 0),
    activePairCount: labels.reduce((sum, label) => sum + label.activePairCount, 0),
    labels,
  };
}

function summarizeFrameCadenceLabel(label, frames, pairs) {
  const validPairs = pairs.filter((pair) => !pair.error);
  const activePairs = validPairs.filter((pair) => pair.active);
  const meanDeltas = validPairs.map((pair) => pair.meanAbsDelta).filter(Number.isFinite);
  const changedRatios = validPairs.map((pair) => pair.changedPixelRatio).filter(Number.isFinite);
  const sampleDeltas = pairs.map((pair) => pair.elapsedDeltaMs).filter(Number.isFinite);
  const activeStart = activePairs.map((pair) => pair.fromElapsedMs).filter(Number.isFinite);
  const activeEnd = activePairs.map((pair) => pair.toElapsedMs).filter(Number.isFinite);
  const peakPair = validPairs.reduce(
    (best, pair) => (!best || pair.meanAbsDelta > best.meanAbsDelta ? pair : best),
    null,
  );
  const averageSampleDeltaMs = averageMetric(sampleDeltas);
  const meanAbsDeltaAverage = averageMetric(meanDeltas);
  const changedPixelRatioAverage = averageMetric(changedRatios, 6);

  return {
    label,
    frameCount: frames.length,
    pairCount: pairs.length,
    analyzedPairCount: validPairs.length,
    errorPairCount: pairs.length - validPairs.length,
    firstElapsedMs: firstFinite(frames.map((frame) => frame.elapsedMs)),
    lastElapsedMs: lastFinite(frames.map((frame) => frame.elapsedMs)),
    averageSampleDeltaMs,
    maxSampleDeltaMs: sampleDeltas.length ? Math.max(...sampleDeltas) : null,
    meanAbsDeltaAverage,
    meanAbsDeltaMax: meanDeltas.length ? roundMetric(Math.max(...meanDeltas)) : null,
    meanAbsDeltaPerSecond:
      Number.isFinite(meanAbsDeltaAverage) && Number.isFinite(averageSampleDeltaMs) && averageSampleDeltaMs > 0
        ? roundMetric((meanAbsDeltaAverage * 1000) / averageSampleDeltaMs, 4)
        : null,
    changedPixelRatioAverage,
    changedPixelRatioMax: changedRatios.length ? roundMetric(Math.max(...changedRatios), 6) : null,
    changedPixelRatioPerSecond:
      Number.isFinite(changedPixelRatioAverage) && Number.isFinite(averageSampleDeltaMs) && averageSampleDeltaMs > 0
        ? roundMetric((changedPixelRatioAverage * 1000) / averageSampleDeltaMs, 6)
        : null,
    activePairCount: activePairs.length,
    activePairRatio: pairs.length ? roundMetric(activePairs.length / pairs.length, 4) : null,
    firstActiveElapsedMs: activeStart.length ? Math.min(...activeStart) : null,
    lastActiveElapsedMs: activeEnd.length ? Math.max(...activeEnd) : null,
    activeWindowMs:
      activeStart.length && activeEnd.length ? Math.max(...activeEnd) - Math.min(...activeStart) : null,
    peakElapsedMs: peakPair?.toElapsedMs ?? null,
    pairs,
  };
}

async function diffFramePair(previousPath, currentPath, options) {
  const previous = await loadFramePixels(previousPath, options.width);
  const current = await loadFramePixels(currentPath, options.width);
  if (previous.info.width !== current.info.width || previous.info.height !== current.info.height) {
    throw new Error(`Frame dimensions differ: ${previous.info.width}x${previous.info.height} vs ${current.info.width}x${current.info.height}`);
  }
  const channels = Math.min(previous.info.channels, current.info.channels, 3);
  const pixelCount = previous.info.width * previous.info.height;
  let sum = 0;
  let changed = 0;
  for (let offset = 0; offset < previous.data.length; offset += previous.info.channels) {
    let pixelDelta = 0;
    for (let channel = 0; channel < channels; channel += 1) {
      pixelDelta += Math.abs(previous.data[offset + channel] - current.data[offset + channel]);
    }
    const meanPixelDelta = pixelDelta / channels;
    sum += meanPixelDelta;
    if (meanPixelDelta >= options.pixelDeltaThreshold) {
      changed += 1;
    }
  }
  return {
    meanAbsDelta: sum / pixelCount,
    changedPixelRatio: changed / pixelCount,
  };
}

async function loadFramePixels(filePath, width) {
  const image = sharp(toSharpPath(filePath)).rotate().resize({ width, withoutEnlargement: true }).toColorspace("srgb").removeAlpha();
  const { data, info } = await image.raw().toBuffer({ resolveWithObject: true });
  return { data, info };
}

function toSharpPath(filePath) {
  if (process.platform !== "win32") {
    return filePath;
  }
  const resolved = path.resolve(filePath);
  if (resolved.startsWith("\\\\?\\")) {
    return resolved;
  }
  if (resolved.startsWith("\\\\")) {
    return `\\\\?\\UNC\\${resolved.slice(2)}`;
  }
  return `\\\\?\\${resolved}`;
}

function resolveSampleFramePath(sample, reportPath, report, kind) {
  const rawPath = kind === "original" ? sample?.capture?.path : sample?.frameImage;
  if (!rawPath || typeof rawPath !== "string") {
    return null;
  }
  const reportDir = path.dirname(reportPath);
  const candidates = [];
  candidates.push(rawPath);
  candidates.push(path.resolve(rawPath));
  candidates.push(path.resolve(reportDir, rawPath));
  candidates.push(path.join(reportDir, path.basename(rawPath)));
  if (kind === "web" && report?.frameImageDir) {
    candidates.push(path.resolve(report.frameImageDir, path.basename(rawPath)));
  }
  for (const candidate of candidates) {
    if (fsSync.existsSync(candidate)) {
      return path.resolve(candidate);
    }
  }
  return null;
}

function sampleElapsedMs(sample) {
  const value = sample?.t ?? sample?.elapsedMs;
  return Number.isFinite(Number(value)) ? Number(value) : null;
}

function compareFrameSamples(a, b) {
  for (const key of ["index", "elapsedMs", "capturedAt"]) {
    const left = a[key];
    const right = b[key];
    if (Number.isFinite(left) && Number.isFinite(right) && left !== right) {
      return left - right;
    }
  }
  return a.path.localeCompare(b.path);
}

function compareFrameCadence(originalCadence, webCadence, label) {
  if (!originalCadence || !webCadence) {
    return [];
  }
  const notes = [];
  const originalLabels = new Map(originalCadence.labels.map((entry) => [entry.label, entry]));
  for (const webLabel of webCadence.labels) {
    const originalLabel = originalLabels.get(webLabel.label);
    if (!originalLabel || !Number.isFinite(originalLabel.activePairRatio) || !Number.isFinite(webLabel.activePairRatio)) {
      continue;
    }
    const ratioDelta = roundMetric(webLabel.activePairRatio - originalLabel.activePairRatio, 4);
    const meanDeltaRatio =
      Number.isFinite(originalLabel.meanAbsDeltaAverage) && originalLabel.meanAbsDeltaAverage > 0
        ? roundMetric(webLabel.meanAbsDeltaAverage / originalLabel.meanAbsDeltaAverage, 4)
        : null;
    const meanDeltaPerSecondRatio =
      Number.isFinite(originalLabel.meanAbsDeltaPerSecond) && originalLabel.meanAbsDeltaPerSecond > 0
        ? roundMetric(webLabel.meanAbsDeltaPerSecond / originalLabel.meanAbsDeltaPerSecond, 4)
        : null;
    notes.push(
      `${label} frame cadence '${webLabel.label}': activePairRatio ${webLabel.activePairRatio} vs Crystal ${originalLabel.activePairRatio} (delta ${ratioDelta}), meanDeltaRatio=${formatValue(meanDeltaRatio)}, meanDeltaPerSecondRatio=${formatValue(meanDeltaPerSecondRatio)}.`,
    );
  }
  if (notes.length === 0) {
    const originalAggregate = aggregateCadenceMetrics(originalCadence);
    const webAggregate = aggregateCadenceMetrics(webCadence);
    if (
      Number.isFinite(originalAggregate.meanAbsDeltaAverage) &&
      Number.isFinite(webAggregate.meanAbsDeltaAverage)
    ) {
      const meanDeltaRatio =
        originalAggregate.meanAbsDeltaAverage > 0
          ? roundMetric(webAggregate.meanAbsDeltaAverage / originalAggregate.meanAbsDeltaAverage, 4)
          : null;
      const meanDeltaPerSecondRatio =
        originalAggregate.meanAbsDeltaPerSecond > 0
          ? roundMetric(webAggregate.meanAbsDeltaPerSecond / originalAggregate.meanAbsDeltaPerSecond, 4)
          : null;
      notes.push(
        `${label} frame cadence was analyzed without overlapping labels; the full-window, non-actor-isolated aggregate visual delta is Crystal ${formatValue(originalAggregate.meanAbsDeltaAverage)} vs Web ${formatValue(webAggregate.meanAbsDeltaAverage)} (ratio ${formatValue(meanDeltaRatio)}); normalized delta/sec is Crystal ${formatValue(originalAggregate.meanAbsDeltaPerSecond)} vs Web ${formatValue(webAggregate.meanAbsDeltaPerSecond)} (ratio ${formatValue(meanDeltaPerSecondRatio)}). Different world objects, ambient effects, HUD contents, browser chrome, or capture geometry can change this ratio, so it must not be treated as a standalone movement-completeness score.`,
      );
    } else {
      notes.push(`${label} frame cadence was analyzed, but no labels overlapped with the native Crystal sample.`);
    }
  }
  return notes;
}

function aggregateCadenceMetrics(cadence) {
  const labels = Array.isArray(cadence?.labels) ? cadence.labels : [];
  const totals = labels.reduce(
    (acc, label) => {
      const pairs = Number.isFinite(label.pairCount) ? label.pairCount : 0;
      if (pairs <= 0) return acc;
      if (Number.isFinite(label.meanAbsDeltaAverage)) {
        acc.meanAbsDeltaTotal += label.meanAbsDeltaAverage * pairs;
        acc.meanAbsDeltaWeight += pairs;
      }
      if (Number.isFinite(label.meanAbsDeltaPerSecond)) {
        acc.meanAbsDeltaPerSecondTotal += label.meanAbsDeltaPerSecond * pairs;
        acc.meanAbsDeltaPerSecondWeight += pairs;
      }
      if (Number.isFinite(label.changedPixelRatioAverage)) {
        acc.changedPixelRatioTotal += label.changedPixelRatioAverage * pairs;
        acc.changedPixelRatioWeight += pairs;
      }
      if (Number.isFinite(label.changedPixelRatioPerSecond)) {
        acc.changedPixelRatioPerSecondTotal += label.changedPixelRatioPerSecond * pairs;
        acc.changedPixelRatioPerSecondWeight += pairs;
      }
      return acc;
    },
    {
      meanAbsDeltaTotal: 0,
      meanAbsDeltaWeight: 0,
      meanAbsDeltaPerSecondTotal: 0,
      meanAbsDeltaPerSecondWeight: 0,
      changedPixelRatioTotal: 0,
      changedPixelRatioWeight: 0,
      changedPixelRatioPerSecondTotal: 0,
      changedPixelRatioPerSecondWeight: 0,
    },
  );
  return {
    meanAbsDeltaAverage:
      totals.meanAbsDeltaWeight > 0 ? roundMetric(totals.meanAbsDeltaTotal / totals.meanAbsDeltaWeight, 4) : null,
    meanAbsDeltaPerSecond:
      totals.meanAbsDeltaPerSecondWeight > 0
        ? roundMetric(totals.meanAbsDeltaPerSecondTotal / totals.meanAbsDeltaPerSecondWeight, 4)
        : null,
    changedPixelRatioAverage:
      totals.changedPixelRatioWeight > 0
        ? roundMetric(totals.changedPixelRatioTotal / totals.changedPixelRatioWeight, 6)
        : null,
    changedPixelRatioPerSecond:
      totals.changedPixelRatioPerSecondWeight > 0
        ? roundMetric(totals.changedPixelRatioPerSecondTotal / totals.changedPixelRatioPerSecondWeight, 6)
        : null,
  };
}

function summarizeInteractionPollution(pollution, sentFramesTail = []) {
  const fallbackFrames = Array.isArray(sentFramesTail)
    ? sentFramesTail
        .map((frame) => ({ frame, payload: parsePayload(frame) }))
        .filter(({ payload }) => isGameplayActionPayload(payload))
    : [];
  if (!pollution && fallbackFrames.length === 0) {
    return null;
  }
  const fallbackTypes = {};
  for (const { payload } of fallbackFrames) {
    fallbackTypes[payload.type] = (fallbackTypes[payload.type] ?? 0) + 1;
  }
  return {
    entityHitClickCount: pollution?.entityHitClickCount ?? 0,
    nonMovementGameplayFrameCount: pollution?.nonMovementGameplayFrameCount ?? fallbackFrames.length,
    nonMovementGameplayFrameTypes: pollution?.nonMovementGameplayFrameTypes ?? fallbackTypes,
    entityHitClicks: Array.isArray(pollution?.entityHitClicks) ? pollution.entityHitClicks.slice(0, 12) : [],
  };
}

function renderMarkdown(report) {
  const lines = [
    "# Movement Temporal Parity Report",
    "",
    `Generated: ${report.generatedAt}`,
    "",
    "## Summary",
    "",
    `- Overall automated status: ${report.ok ? "ok" : "needs follow-up"}`,
    `- Action alignment: ${report.actionAlignmentOk ? "ok" : "missing"}`,
    ...report.interpretation.map((note) => `- ${note}`),
    "",
    "## Original Crystal",
    "",
    `- Path: \`${report.original.path}\``,
    `- Samples: ${report.original.sampleCount}, actions: ${report.original.actionCount}`,
    `- Action timeline: first=${formatValue(report.original.actionTimeline?.firstActionMs)}ms, last=${formatValue(report.original.actionTimeline?.lastActionMs)}ms, span=${formatValue(report.original.actionTimeline?.spanMs)}ms`,
    `- Timing: sampleMs=${report.original.sampleMs}, holdMs=${report.original.holdMs}, stepWaitMs=${report.original.stepWaitMs}`,
    "",
    "| Label | Frames | First ms | Last ms | Avg delta ms |",
    "| --- | ---: | ---: | ---: | ---: |",
    ...report.original.labels.map(
      (label) =>
        `| ${label.label} | ${label.count} | ${formatValue(label.firstElapsedMs)} | ${formatValue(label.lastElapsedMs)} | ${formatValue(label.averageDeltaMs)} |`,
    ),
    "",
    ...renderFrameCadence(report.original.frameCadence),
    "",
    "## Web Movement Capture",
    "",
    ...renderWebSection(report.web),
  ];

  if (report.webBichon) {
    lines.push("", "## Web Bichon Movement Capture", "", ...renderWebSection(report.webBichon));
  }

  lines.push("");
  return `${lines.join("\n")}\n`;
}

function renderWebSection(web) {
  return [
    `- Path: \`${web.path}\``,
    `- Status: ${web.ok ? "ok" : "needs follow-up"}`,
    `- Map: ${web.mapTitle ?? web.map ?? "unknown"} (${web.map ?? "?"})`,
    `- Final player: ${web.finalPlayer ? `${web.finalPlayer.x},${web.finalPlayer.y}` : "unknown"}`,
    `- Frames: ${web.frameImageCount} in \`${web.frameImageDir ?? ""}\``,
    `- Action timeline: first=${formatValue(web.actionTimeline?.firstActionMs)}ms, last=${formatValue(web.actionTimeline?.lastActionMs)}ms, span=${formatValue(web.actionTimeline?.spanMs)}ms`,
    `- Commands: walk=${web.movementCommandCounts.walk}, run=${web.movementCommandCounts.run}, moveTo=${web.movementCommandCounts.moveTo}`,
    `- Self ACK latency: count=${web.selfAckLatency.count}, missing=${web.selfAckLatency.missingCount}, avg=${formatValue(web.selfAckLatency.averageMs)}ms, max=${formatValue(web.selfAckLatency.maxMs)}ms`,
    `- Failed assertions: ${web.failedAssertions.length ? web.failedAssertions.join(", ") : "none"}`,
    `- Interaction pollution: entityHitClicks=${formatValue(web.interactionPollution?.entityHitClickCount)}, nonMovementGameplayFrames=${formatValue(web.interactionPollution?.nonMovementGameplayFrameCount)}, types=${formatFrameTypes(web.interactionPollution?.nonMovementGameplayFrameTypes)}`,
    `- Console/network: criticalConsoleErrors=${formatValue(web.criticalConsoleErrors)}, nonFavicon404=${formatValue(web.nonFaviconNetwork404s)}`,
    "",
    ...renderFrameCadence(web.frameCadence),
  ];
}

function renderFrameCadence(cadence) {
  if (!cadence) {
    return ["- Frame cadence: not analyzed"];
  }
  const alignment = cadence.actionAlignment;
  return [
    `- Frame cadence: labels=${cadence.labelCount}, frames=${cadence.frameCount}, pairs=${cadence.pairCount}, activePairs=${cadence.activePairCount}, diffWidth=${cadence.diffWidth}`,
    `- Action alignment: ${alignment?.applied ? `applied, clip=${alignment.clipStartMs}..${alignment.clipEndMs}ms, frames=${alignment.includedFrameCount}/${alignment.sourceFrameCount}` : "not applied"}`,
    "",
    "| Label | Frames | Pairs | Avg sample ms | Active pairs | Active window ms | Mean delta avg/max/sec | Changed px avg/max/sec |",
    "| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |",
    ...cadence.labels.map(
      (label) =>
        `| ${label.label} | ${label.frameCount} | ${label.pairCount} | ${formatValue(label.averageSampleDeltaMs)} | ${label.activePairCount}/${label.pairCount} (${formatValue(label.activePairRatio)}) | ${formatValue(label.activeWindowMs)} | ${formatValue(label.meanAbsDeltaAverage)}/${formatValue(label.meanAbsDeltaMax)}/${formatValue(label.meanAbsDeltaPerSecond)} | ${formatValue(label.changedPixelRatioAverage)}/${formatValue(label.changedPixelRatioMax)}/${formatValue(label.changedPixelRatioPerSecond)} |`,
    ),
  ];
}

function formatFrameTypes(types) {
  if (!types || typeof types !== "object" || Object.keys(types).length === 0) {
    return "none";
  }
  return Object.entries(types)
    .map(([key, value]) => `${key}:${value}`)
    .join(", ");
}

function parsePayload(frame) {
  try {
    return JSON.parse(frame?.payloadData ?? "null");
  } catch {
    return null;
  }
}

function isSelfMovementAck(payload) {
  return [
    "UserLocation",
    "Pushed",
    "UserDash",
    "UserDashFail",
    "UserDashAttack",
    "UserAttackMove",
  ].includes(payload?.packet);
}

function isGameplayActionPayload(payload) {
  if (!payload || typeof payload.type !== "string") return false;
  return [
    "attack",
    "rangeAttack",
    "attackDirection",
    "castSkill",
    "harvest",
    "interact",
    "pickGroundDrop",
    "pickUpTile",
    "transferMap",
    "selectNpcDialog",
    "submitNpcInput",
  ].includes(payload.type);
}

function averageDelta(values) {
  if (values.length < 2) return null;
  const deltas = [];
  for (let index = 1; index < values.length; index += 1) {
    deltas.push(values[index] - values[index - 1]);
  }
  return Math.round((deltas.reduce((sum, value) => sum + value, 0) / deltas.length) * 100) / 100;
}

function averageMetric(values, digits = 2) {
  if (values.length === 0) return null;
  return roundMetric(values.reduce((sum, value) => sum + value, 0) / values.length, digits);
}

function roundMetric(value, digits = 2) {
  if (!Number.isFinite(value)) return null;
  const scale = 10 ** digits;
  return Math.round(value * scale) / scale;
}

function firstFinite(values) {
  return values.find(Number.isFinite) ?? null;
}

function lastFinite(values) {
  for (let index = values.length - 1; index >= 0; index -= 1) {
    if (Number.isFinite(values[index])) {
      return values[index];
    }
  }
  return null;
}

function formatValue(value) {
  return value === null || value === undefined ? "n/a" : String(value);
}

function numberArg(value, fallback) {
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : fallback;
}

function booleanArg(value, fallback) {
  if (value === undefined || value === null) return fallback;
  if (typeof value === "boolean") return value;
  return !["0", "false", "no", "off"].includes(String(value).toLowerCase());
}

function parseArgs(argv) {
  const parsed = {};
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (!arg.startsWith("--")) continue;
    const key = arg.slice(2);
    const next = argv[index + 1];
    if (!next || next.startsWith("--")) {
      parsed[key] = "true";
      continue;
    }
    parsed[key] = next;
    index += 1;
  }
  return parsed;
}
