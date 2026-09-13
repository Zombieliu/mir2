import { createReadStream } from 'node:fs';
import { readFile } from 'node:fs/promises';
import readline from 'node:readline';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

import { applyProtocolObservation } from './protocol-observation.mjs';

const CLASSES = Object.freeze(['Warrior', 'Wizard', 'Taoist']);
const GROWTH_MILESTONE_IDS = Object.freeze([15, 20, 25, 30].map(level => 2_100_000 + level));
const DEFAULT_PROFILE = new URL('../../../../config/quest-guidance/newcomer-journey-v1.json', import.meta.url);

function normalizedStage(value) {
  return String(value ?? '').replace(/[^a-z]/gi, '').toLowerCase();
}

function finiteNumber(value) {
  const number = Number(value);
  return Number.isFinite(number) ? number : null;
}

function selfPlayer(snapshot) {
  const objectId = snapshot?.playerObjectId;
  return (snapshot?.entities ?? []).find(entity =>
    String(entity?.objectId) === String(objectId) ||
    String(entity?.kind ?? '').toLowerCase() === 'selfplayer'
  ) ?? null;
}

function mandatoryIds(profile, className) {
  return [...new Set((profile?.chapters ?? []).flatMap(chapter => [
    ...(chapter?.questIds ?? []),
    ...(chapter?.classQuestIds?.[className] ?? []),
  ]).map(Number).filter(Number.isSafeInteger))];
}

function conciseQuestProgress(entry) {
  const progress = {
    questId: Number(entry.questId),
    title: String(entry.title ?? ''),
    stage: String(entry.stage ?? ''),
    current: finiteNumber(entry.current),
    required: finiteNumber(entry.required),
  };
  if (Array.isArray(entry.objectives) && entry.objectives.length) {
    progress.objectives = entry.objectives.map(objective => ({
      label: String(objective?.label ?? ''),
      current: finiteNumber(objective?.current),
      required: finiteNumber(objective?.required),
      done: objective?.done === true,
    }));
  }
  return progress;
}

function isoTime(value) {
  const milliseconds = Date.parse(value);
  return Number.isFinite(milliseconds) ? milliseconds : null;
}

async function relevantFailureReport(outputRoot, className, sessionStartAt, traceLastAt) {
  let report;
  try {
    report = JSON.parse(await readFile(path.join(outputRoot, `${className}.report.json`), 'utf8'));
  } catch (error) {
    if (error?.code === 'ENOENT' || error instanceof SyntaxError) return null;
    throw error;
  }
  if (typeof report?.error !== 'string' || report.error.trim() === '') return null;
  const reportStarted = isoTime(report.startedAt);
  const reportFinished = isoTime(report.finishedAt);
  const sessionStarted = isoTime(sessionStartAt);
  const traceLast = isoTime(traceLastAt);
  if (reportStarted == null || traceLast == null || reportStarted > traceLast) return null;
  if (sessionStarted != null && reportFinished != null && reportFinished < sessionStarted) return null;
  if (sessionStarted != null && reportFinished == null && reportStarted < sessionStarted) return null;
  return {
    runId: String(report.runId ?? ''),
    startedAt: report.startedAt ?? null,
    finishedAt: report.finishedAt ?? null,
    error: report.error,
  };
}

/** Stream one public trace and summarize only its observed, server-authored state. */
export async function summarizeProtocolClass({ outputRoot, className, profile = null }) {
  if (!CLASSES.includes(className)) throw new Error(`Invalid class: ${className}`);
  const resolvedRoot = path.resolve(outputRoot);
  const activeProfile = profile ?? JSON.parse(await readFile(DEFAULT_PROFILE, 'utf8'));
  const ids = mandatoryIds(activeProfile, className);
  const mandatory = new Set(ids);
  const tracePath = path.join(resolvedRoot, `${className}.trace.jsonl`);
  let snapshot = null;
  let firstAt = null;
  let lastAt = null;
  let sessionStartAt = null;
  let authoritativeSnapshotAt = null;
  let deathCount = 0;
  let townReviveCount = 0;
  let malformedLineCount = 0;

  const input = createReadStream(tracePath, { encoding: 'utf8' });
  const lines = readline.createInterface({ input, crlfDelay: Infinity });
  for await (const line of lines) {
    if (line.trim() === '') continue;
    let event;
    try {
      event = JSON.parse(line);
    } catch {
      malformedLineCount += 1;
      continue;
    }
    if (typeof event?.at === 'string') {
      firstAt ??= event.at;
      lastAt = event.at;
    }
    if (event?.direction === 'lifecycle' && event?.type === 'connection') {
      snapshot = null;
      sessionStartAt = event.at ?? lastAt;
      authoritativeSnapshotAt = null;
      continue;
    }
    if (event?.direction === 'received') {
      snapshot = applyProtocolObservation(snapshot, event);
      if (event.type === 'worldSnapshot') authoritativeSnapshotAt = event.at ?? lastAt;
      if (event.packet === 'Death') deathCount += 1;
    } else if (event?.direction === 'sent' && event?.type === 'townRevive') {
      townReviveCount += 1;
    }
  }

  const self = selfPlayer(snapshot);
  const questLog = Array.isArray(snapshot?.questLog) ? snapshot.questLog : [];
  const completed = snapshot == null
    ? null
    : ids.filter(id => questLog.some(entry =>
        Number(entry?.questId) === id && normalizedStage(entry?.stage) === 'completed'
      )).length;
  const completedGrowthMilestones = snapshot == null
    ? null
    : GROWTH_MILESTONE_IDS.filter(id => questLog.some(entry =>
        Number(entry?.questId) === id && normalizedStage(entry?.stage) === 'completed'
      )).length;
  const activeQuests = snapshot == null ? [] : questLog
    .filter(entry => ['inprogress', 'readytoturnin'].includes(normalizedStage(entry?.stage)))
    .map(entry => ({
      ...conciseQuestProgress(entry),
      mandatory: mandatory.has(Number(entry?.questId)),
    }));

  return {
    className,
    level: finiteNumber(self?.level),
    map: snapshot?.mapFileName == null ? null : String(snapshot.mapFileName),
    hp: {
      current: finiteNumber(snapshot?.playerHp ?? self?.hp),
      maximum: finiteNumber(snapshot?.playerMaxHp ?? self?.maxHp),
    },
    selfPosition: self == null ? null : {
      x: finiteNumber(self.x),
      y: finiteNumber(self.y),
    },
    mandatory: { completed, total: ids.length },
    growthMilestones: { completed: completedGrowthMilestones, total: GROWTH_MILESTONE_IDS.length },
    activeQuests,
    deathCount,
    townReviveCount,
    firstTimestamp: firstAt,
    lastTimestamp: lastAt,
    authoritativeSnapshotAt,
    latestFailure: await relevantFailureReport(resolvedRoot, className, sessionStartAt, lastAt),
    malformedLineCount,
  };
}

export async function summarizeProtocolJourneys({
  outputRoot = process.env.MIR2_JOURNEY_OUTPUT ?? 'output/protocol-journey',
  className = process.env.MIR2_JOURNEY_CLASS,
} = {}) {
  const classNames = className == null || String(className).trim() === ''
    ? CLASSES
    : [String(className).trim()];
  const profile = JSON.parse(await readFile(DEFAULT_PROFILE, 'utf8'));
  const summaries = {};
  for (const currentClass of classNames) {
    summaries[currentClass] = await summarizeProtocolClass({ outputRoot, className: currentClass, profile });
  }
  return { classes: summaries };
}

const invokedPath = process.argv[1] ? pathToFileURL(path.resolve(process.argv[1])).href : null;
if (invokedPath === import.meta.url) {
  console.log(JSON.stringify(await summarizeProtocolJourneys(), null, 2));
}
