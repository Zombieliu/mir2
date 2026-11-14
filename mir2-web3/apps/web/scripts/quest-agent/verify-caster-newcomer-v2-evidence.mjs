import fs from 'node:fs/promises';
import { createReadStream } from 'node:fs';
import readline from 'node:readline';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import { isDeepStrictEqual } from 'node:util';
import { cloneConfirmedV2Recoveries } from './newcomer-v2-recovery-ledger.mjs';

// This adapter audits public receipts and private, independently supplied stores.
// It never changes a store, runs a journey, or exports login/account information.
const CLASSES = ['Wizard', 'Taoist'];
const DURATION_MS = 120 * 60_000;
const RUN_ID = /^\d{4}-\d{2}-\d{2}T\d{2}-\d{2}-\d{2}-\d{3}Z$/;
const NORMAL_COMMANDS = new Set(['clientVersion', 'newAccount', 'login', 'newCharacter', 'startGame',
  'townRevive', 'keepAlive', 'walk', 'run', 'turn', 'attack', 'attackDirection', 'magic', 'spellToggle',
  'harvest', 'interact', 'selectNpcDialog', 'acceptQuest', 'finishQuest', 'pickUp', 'pickUpTile',
  'equipItem', 'moveItem', 'useItem', 'buyItem', 'sellItem', 'logOut']);
const instant = value => typeof value === 'string' && Number.isFinite(Date.parse(value)) &&
  new Date(Date.parse(value)).toISOString() === value;
const completed = value => String(value ?? '').toLowerCase() === 'completed';
const number = value => typeof value === 'number' && Number.isFinite(value);
const integer = value => Number.isSafeInteger(value);
const ref = event => event ? { sequence: event.sequence, at: event.at } : null;
const reason = (list, code, detail = {}) => {
  if (!list.some(row => row.code === code)) list.push({ code, ...detail });
};
const samePath = (left, right) => process.platform === 'win32'
  ? path.resolve(left).toLowerCase() === path.resolve(right).toLowerCase()
  : path.resolve(left) === path.resolve(right);

function runInstant(runId) {
  return RUN_ID.test(runId ?? '')
    ? `${runId.slice(0, 13)}:${runId.slice(14, 16)}:${runId.slice(17, 19)}.${runId.slice(20, 23)}Z`
    : null;
}

async function expectedQuestIds() {
  const config = JSON.parse(await fs.readFile(new URL('../../../../config/quest-guidance/newcomer-journey-v2.json', import.meta.url), 'utf8'));
  const ids = [...config.quests, ...config.growthRewards].map(row => Number(row.id));
  if (ids.length !== 26 || new Set(ids).size !== 26 || ids.some(id => !integer(id))) {
    throw new Error('Invalid 26-node newcomer V2 contract');
  }
  return ids;
}

function questIds(rows, field, expected, problems) {
  if (!Array.isArray(rows)) { reason(problems, 'missing-quest-state'); return []; }
  const seen = new Set();
  const result = new Set();
  for (const row of rows) {
    const id = Number(row?.[field]);
    if (!expected.includes(id)) continue;
    if (seen.has(id)) reason(problems, 'duplicate-quest-state', { questId: id });
    seen.add(id);
    if (completed(row.stage)) result.add(id);
  }
  return expected.filter(id => result.has(id));
}

// A cleanup snapshot has no owner/map and cannot replace an authoritative frame.
function frame(snapshot, className, name, expected) {
  if (!snapshot || typeof snapshot.mapFileName !== 'string' || !snapshot.mapFileName ||
      !integer(snapshot.playerObjectId) || !Array.isArray(snapshot.entities) || !Array.isArray(snapshot.questLog)) return null;
  const owners = snapshot.entities.filter(row => row.objectId === snapshot.playerObjectId);
  if (owners.length !== 1) return null;
  const actor = owners[0];
  if (actor.kind !== 'selfPlayer' || actor.name !== name || actor.class !== className ||
      !integer(actor.level) || !integer(actor.x) || !integer(actor.y) ||
      typeof actor.direction !== 'string' || !actor.direction ||
      !number(snapshot.gold) || !number(snapshot.playerExperience)) return null;
  const problems = [];
  const ids = questIds(snapshot.questLog, 'questId', expected, problems);
  return { objectId: actor.objectId, name, className, level: actor.level,
    mapFileName: snapshot.mapFileName, x: actor.x, y: actor.y, direction: actor.direction,
    gold: snapshot.gold, experience: snapshot.playerExperience, completedIds: ids, problems };
}

function recoverySummary(entry) {
  const owner = value => value ? Object.fromEntries(['objectId', 'mapFileName', 'x', 'y', 'hp']
    .filter(key => key in value).map(key => [key, value[key]])) : null;
  return { phase: entry.phase, questId: entry.questId, deathAt: entry.deathAt,
    revivedAt: entry.revivedAt ?? null, before: owner(entry.before), after: owner(entry.after) };
}

function timingAndDeaths(reports, checkedAt, problems) {
  const first = reports[0];
  const last = reports.at(-1);
  const ordinaryStart = first?.v2?.ordinaryStartedAt;
  const ordinaryDeadline = instant(ordinaryStart) ? new Date(Date.parse(ordinaryStart) + DURATION_MS).toISOString() : null;
  let functionalStart = null;
  let functionalDeadline = null;
  let priorRecoveries = [];
  let ordinaryRecoveries = [];
  let retainedRecoveries = [];
  for (const report of reports) {
    const v2 = report.v2 ?? {};
    const ledger = report.v2TimingLedger;
    if (!instant(report.startedAt) || !instant(runInstant(report.runId)) ||
        Math.abs(Date.parse(report.startedAt) - Date.parse(runInstant(report.runId))) > 5_000 ||
        !ordinaryDeadline || v2.ordinaryStartedAt !== ordinaryStart || v2.deadlineAt !== ordinaryDeadline ||
        (report !== first && ledger?.ordinaryStartedAt !== ordinaryStart) ||
        (ledger != null && ledger.ordinaryStartedAt !== ordinaryStart)) reason(problems, 'ordinary-clock-changed');
    if (report === first && Math.abs(Date.parse(report.startedAt) - Date.parse(ordinaryStart)) > 5_000) {
      reason(problems, 'missing-original-ordinary-run');
    }
    const hasFunctional = v2.functionalRecheckStartedAt != null || v2.functionalRecheckDeadlineAt != null ||
      ledger?.functionalRecheck === true;
    if (hasFunctional) {
      const start = v2.functionalRecheckStartedAt;
      const deadline = v2.functionalRecheckDeadlineAt;
      if (!instant(start) || !instant(deadline) || Date.parse(start) < Date.parse(ordinaryDeadline) ||
          Date.parse(deadline) !== Date.parse(start) + DURATION_MS ||
          Date.parse(report.startedAt) < Date.parse(start) || ledger?.functionalRecheck !== true ||
          ledger.functionalRecheckStartedAt !== start || ledger.functionalRecheckDeadlineAt !== deadline ||
          (functionalStart !== null && (functionalStart !== start || functionalDeadline !== deadline))) {
        reason(problems, 'functional-clock-changed');
      }
      functionalStart ??= start;
      functionalDeadline ??= deadline;
    } else if (functionalStart !== null) reason(problems, 'functional-clock-dropped');
    try {
      // The first fresh runner has no resume ledger. Subsequent runs must
      // retain the previous result in their explicit startup ledger.
      const before = cloneConfirmedV2Recoveries(report === first && ledger == null ? [] : ledger?.recoveries);
      const after = cloneConfirmedV2Recoveries(v2.recoveries);
      if (!isDeepStrictEqual(before.slice(0, priorRecoveries.length), priorRecoveries) ||
          !isDeepStrictEqual(after.slice(0, before.length), before)) reason(problems, 'death-ledger-reset');
      priorRecoveries = after;
      retainedRecoveries = after;
      if (!hasFunctional) ordinaryRecoveries = after;
    } catch { reason(problems, 'invalid-death-ledger'); }
  }
  return {
    ordinary: { startedAt: ordinaryStart ?? null, deadlineAt: ordinaryDeadline, durationMs: DURATION_MS,
      expiredAtAudit: !!ordinaryDeadline && Date.parse(checkedAt) >= Date.parse(ordinaryDeadline),
      completedWithinDeadline: false, elapsedToFinalLogoutMs: null },
    functional: functionalStart === null ? null : { startedAt: functionalStart, deadlineAt: functionalDeadline,
      durationMs: DURATION_MS, expiredAtAudit: instant(functionalDeadline) && Date.parse(checkedAt) >= Date.parse(functionalDeadline),
      inheritedCompletedIds: [], newlyReceiptedIds: [], completedWithinDeadline: false },
    freshTimedCompletion: false,
    deaths: { maxRevives: 3, originalOrdinaryLedger: ordinaryRecoveries.map(recoverySummary),
      retainedLedger: retainedRecoveries.map(recoverySummary), confirmedRecoveries: retainedRecoveries.length,
      preserved: !problems.some(row => ['invalid-death-ledger', 'death-ledger-reset'].includes(row.code)) },
    lastReport: last,
  };
}

async function scanTrace(file, report, identity, expected, problems) {
  const receipts = new Map();
  const pending = new Map();
  let knownCompleted = new Set();
  let loginVerified = false;
  let startSent = false;
  let startAccepted = false;
  let owner = null;
  let lastFrame = null;
  let firstFrame = null;
  let logoutSent = null;
  let logoutSuccess = null;
  let logoutFrame = null;
  let creationSent = false;
  let created = false;
  let newAccountSent = false;
  let lastSequence = 0;
  let lastAt = Date.parse(report.startedAt);
  let deathActive = false;
  const deathReceipts = [];
  let traceStat;
  try { traceStat = await fs.stat(file); } catch { reason(problems, 'unreadable-trace'); return { receipts }; }
  const lines = readline.createInterface({ input: createReadStream(file), crlfDelay: Infinity });
  try {
    for await (const line of lines) {
      if (!line.trim()) continue;
      let event;
      try { event = JSON.parse(line); } catch { reason(problems, 'malformed-trace'); break; }
      if (!integer(event.sequence) || event.sequence <= lastSequence || !instant(event.at) ||
          Date.parse(event.at) < lastAt) { reason(problems, 'invalid-trace-order'); continue; }
      lastSequence = event.sequence;
      lastAt = Date.parse(event.at);
      if (event.direction === 'sent') {
        if (!NORMAL_COMMANDS.has(event.type)) reason(problems, 'non-ordinary-command');
        if (event.type === 'login') {
          loginVerified = event.accountId === identity.accountKey;
          if (!loginVerified) reason(problems, 'trace-account-mismatch');
        } else if (event.type === 'newAccount') newAccountSent = event.accountId === identity.accountKey;
        else if (event.type === 'newCharacter') creationSent = event.name === report.name && event.class === report.className;
        else if (event.type === 'startGame') {
          startSent = loginVerified && event.characterIndex === report.characterIndex;
          startAccepted = false;
          owner = null;
          logoutSent = null;
          logoutSuccess = null;
          logoutFrame = null;
          lastFrame = null;
          pending.clear();
          knownCompleted = new Set();
          if (!startSent) reason(problems, 'trace-character-mismatch');
        } else if (event.type === 'finishQuest' && owner !== null) {
          const id = event.questIndex;
          if (expected.includes(id) && !knownCompleted.has(id)) pending.set(id, { finish: ref(event), change: null });
        } else if (event.type === 'logOut' && owner !== null) logoutSent = ref(event);
        continue;
      }
      if (event.direction !== 'received') continue;
      if (event.packet === 'NewCharacterSuccess') {
        const character = event.payload?.character;
        created = creationSent && character?.name === report.name && character?.class === report.className &&
          character?.index === report.characterIndex && character?.level === 0;
      } else if (event.packet === 'StartGame') startAccepted = startSent && event.payload?.result === 4;
      else if (event.packet === 'UserInformation') {
        const user = event.payload;
        if (startAccepted && user?.name === report.name && user?.class === report.className && integer(user?.objectId)) owner = user.objectId;
        else reason(problems, 'trace-owner-mismatch');
      } else if (event.type === 'worldSnapshot' && owner !== null && !logoutSuccess) {
        const state = frame(event.payload, report.className, report.name, expected);
        if (state && state.objectId === owner) {
          lastFrame = { ...ref(event), state };
          firstFrame ??= lastFrame;
          for (const problem of state.problems) reason(problems, problem.code, { questId: problem.questId });
        }
      } else if (event.packet === 'ChangeQuest' && owner !== null && !logoutSuccess) {
        const id = Number(event.payload?.questId ?? event.payload?.id);
        if (event.payload?.completed === true && pending.has(id)) pending.get(id).change = ref(event);
      } else if (event.packet === 'CompleteQuest' && owner !== null && !logoutSuccess) {
        const ids = event.payload?.completedQuests;
        if (!Array.isArray(ids) || ids.some(id => !integer(id))) { reason(problems, 'invalid-completion-receipt'); continue; }
        for (const id of expected) {
          const request = pending.get(id);
          if (request?.change && ids.includes(id) && !knownCompleted.has(id)) {
            receipts.set(id, { questId: id, runId: report.runId, finish: request.finish,
              change: request.change, complete: ref(event) });
            pending.delete(id);
          }
        }
        knownCompleted = new Set(ids);
      } else if (event.packet === 'LogOutSuccess' && owner !== null && logoutSent && event.sequence > logoutSent.sequence) {
        const matches = (event.payload?.characters ?? []).filter(row => row.name === report.name &&
          row.index === report.characterIndex && row.class === report.className);
        if (matches.length === 1 && lastFrame && matches[0].level === lastFrame.state.level) {
          logoutSuccess = ref(event);
          logoutFrame = lastFrame;
        } else reason(problems, 'logout-character-mismatch');
      }
      if (owner !== null && ['Died', 'ObjectDied', 'Death'].includes(event.packet) && event.payload?.objectId === owner && !deathActive) {
        deathReceipts.push({ runId: report.runId, ...ref(event) });
        deathActive = true;
      }
      if (event.packet === 'Revived' && event.payload?.objectId === owner) deathActive = false;
    }
  } catch { reason(problems, 'unreadable-trace'); }
  try {
    const after = await fs.stat(file);
    if (after.size !== traceStat.size || after.mtimeMs !== traceStat.mtimeMs) reason(problems, 'trace-changed-during-audit');
  } catch { reason(problems, 'trace-changed-during-audit'); }
  return { receipts, firstFrame, logoutSent, logoutSuccess, logoutFrame,
    identityVerified: owner !== null && loginVerified && startAccepted,
    freshCreation: newAccountSent && created && firstFrame?.state.level === 1 && firstFrame.state.completedIds.length === 0,
    deathReceipts };
}

async function auditClass({ evidenceRoot, className, storePath, expected, checkedAt, owners }) {
  const problems = [];
  const result = { className, status: 'partial', completed: false, completedUnits: 0,
    snapshotCompletedIds: [], savedCompletedIds: [], receiptedCompletedIds: [], verifiedCompletedIds: [],
    logoutCommitted: false, transformMatches: false, visualAccepted: false, reasons: problems };
  const directory = path.join(evidenceRoot, className.toLowerCase());
  const canonicalPath = path.join(directory, `${className}.report.json`);
  const snapshotPath = path.join(directory, `${className}.snapshot.json`);
  let canonicalText;
  let storeText;
  let snapshotText;
  let report;
  let store;
  try {
    [canonicalText, storeText] = await Promise.all([fs.readFile(canonicalPath, 'utf8'), fs.readFile(storePath, 'utf8')]);
    report = JSON.parse(canonicalText);
    store = JSON.parse(storeText);
  } catch { reason(problems, 'missing-or-invalid-report-or-store'); return result; }
  result.name = typeof report.name === 'string' ? report.name : null;
  result.runId = RUN_ID.test(report.runId ?? '') ? report.runId : null;
  if (report.className !== className || report.v2?.profile !== 'newcomer-v2' ||
      !integer(report.characterIndex) || report.characterIndex < 0 || !report.name) {
    reason(problems, 'invalid-report-identity'); return result;
  }
  const matches = Object.entries(store.accounts ?? {}).flatMap(([accountKey, account]) =>
    (account.characters ?? []).filter(character => character.name === report.name && character.index === report.characterIndex)
      .map(character => ({ accountKey, account, character })));
  if (matches.length !== 1) { reason(problems, 'non-unique-store-character'); return result; }
  const identity = matches[0];
  owners.set(className, identity.accountKey);
  const save = identity.account.saves?.[String(report.characterIndex)];
  if (identity.character.class !== className || save?.character?.class !== className ||
      save?.character?.name !== report.name || save?.character?.index !== report.characterIndex) {
    reason(problems, 'store-character-mismatch'); return result;
  }
  let savedQuests;
  try {
    savedQuests = typeof save.quest_states_json === 'string' ? JSON.parse(save.quest_states_json) : save.quest_states_json;
    if (Array.isArray(savedQuests)) savedQuests = savedQuests.map(row => typeof row === 'string' ? JSON.parse(row) : row);
  } catch { reason(problems, 'invalid-saved-quests'); }
  result.savedCompletedIds = questIds(savedQuests, 'quest_id', expected, problems);
  result.saveRevision = save.revision ?? null;
  const reportByRun = new Map();
  try {
    for (const file of await fs.readdir(directory)) {
      const prefix = `${className}.`;
      if (!file.startsWith(prefix) || !file.endsWith('.report.json')) continue;
      const runId = file.slice(prefix.length, -'.report.json'.length);
      if (!RUN_ID.test(runId)) continue;
      const row = JSON.parse(await fs.readFile(path.join(directory, file), 'utf8'));
      if (row.runId !== runId) reason(problems, 'report-run-id-mismatch');
      reportByRun.set(runId, row);
    }
  } catch { reason(problems, 'invalid-archived-report'); }
  if (reportByRun.has(report.runId) && !isDeepStrictEqual(reportByRun.get(report.runId), report)) reason(problems, 'canonical-report-mismatch');
  reportByRun.set(report.runId, report);
  const reports = [...reportByRun.values()].sort((left, right) => Date.parse(left.startedAt) - Date.parse(right.startedAt));
  if (reports.at(-1)?.runId !== report.runId) reason(problems, 'canonical-report-is-not-latest');
  const timing = timingAndDeaths(reports, checkedAt, problems);
  const allReceipts = new Map();
  const deathReceipts = [];
  let latest = null;
  let first = null;
  for (const row of reports) {
    const trace = path.join(directory, `${className}.${row.runId}.trace.jsonl`);
    if (row.name !== report.name || row.characterIndex !== report.characterIndex || row.className !== className ||
        row.v2?.profile !== 'newcomer-v2' || row.transport !== 'normal-local-websocket' ||
        typeof row.traceFile !== 'string' || !samePath(row.traceFile, trace)) {
      reason(problems, 'resume-identity-or-trace-mismatch'); continue;
    }
    const scan = await scanTrace(trace, row, identity, expected, problems);
    if (!scan.identityVerified) reason(problems, 'missing-owned-bootstrap');
    first ??= scan;
    for (const [id, receipt] of scan.receipts) {
      if (!allReceipts.has(id)) allReceipts.set(id, receipt);
      else reason(problems, 'duplicate-finish-receipt', { questId: id });
    }
    deathReceipts.push(...(scan.deathReceipts ?? []));
    if (row.runId === report.runId) latest = scan;
  }
  if (!first?.freshCreation) reason(problems, 'missing-fresh-normal-creation');
  result.identityVerified = !!latest?.identityVerified;
  result.logoutCommitted = !!latest?.logoutSuccess;
  result.logoutSent = latest?.logoutSent ?? null;
  result.logoutSuccess = latest?.logoutSuccess ?? null;
  result.rawFinalSnapshot = latest?.logoutFrame ? { runId: report.runId, ...ref(latest.logoutFrame),
    selection: latest.logoutFrame.sequence > latest.logoutSent.sequence ? 'post-logout-complete-frame' : 'pre-logout-complete-frame' } : null;
  if (!result.logoutCommitted) reason(problems, 'missing-normal-logout-success');
  if (!latest?.logoutFrame) reason(problems, 'missing-complete-authoritative-final-frame');
  const state = latest?.logoutFrame?.state;
  result.snapshotCompletedIds = state?.completedIds ?? [];
  result.level = state?.level ?? null;
  result.transformMatches = !!state && state.name === save.character.name && state.className === save.character.class &&
    state.level === save.character.level && state.level === identity.character.level && state.mapFileName === save.map_file_name &&
    state.x === save.position?.x && state.y === save.position?.y && state.direction === save.direction &&
    state.gold === save.gold && state.experience === save.experience;
  if (!result.transformMatches) reason(problems, 'authoritative-save-mismatch');
  try {
    snapshotText = await fs.readFile(snapshotPath, 'utf8');
    const runnerFrame = frame(JSON.parse(snapshotText), className, report.name, expected);
    result.runnerSnapshotMatches = !!state && isDeepStrictEqual(runnerFrame, state);
  } catch { result.runnerSnapshotMatches = false; }
  if (!result.runnerSnapshotMatches) reason(problems, 'runner-final-frame-mismatch');
  result.receiptedCompletedIds = expected.filter(id => allReceipts.has(id));
  result.receipts = expected.filter(id => allReceipts.has(id)).map(id => allReceipts.get(id));
  result.persistedCandidateIds = expected.filter(id => result.savedCompletedIds.includes(id) && result.snapshotCompletedIds.includes(id));
  timing.deaths.observedDeathReceipts = deathReceipts;
  if (deathReceipts.length > timing.deaths.confirmedRecoveries) reason(problems, 'unledgered-owner-death');
  timing.deaths.preserved = timing.deaths.preserved && !problems.some(row => row.code === 'unledgered-owner-death');
  if (latest?.logoutSuccess && timing.ordinary.startedAt) {
    timing.ordinary.elapsedToFinalLogoutMs = Date.parse(latest.logoutSuccess.at) - Date.parse(timing.ordinary.startedAt);
  }
  if (timing.functional) {
    timing.functional.inheritedCompletedIds = expected.filter(id => allReceipts.has(id) &&
      Date.parse(allReceipts.get(id).complete.at) < Date.parse(timing.functional.startedAt));
    timing.functional.newlyReceiptedIds = expected.filter(id => allReceipts.has(id) &&
      Date.parse(allReceipts.get(id).complete.at) >= Date.parse(timing.functional.startedAt) &&
      Date.parse(allReceipts.get(id).complete.at) <= Date.parse(timing.functional.deadlineAt));
  }
  delete timing.lastReport;
  result.timing = timing;
  result.pause = report.v2?.pause ? { reason: report.v2.pause.reason, questId: report.v2.pause.questId ?? null,
    at: report.v2.pause.at ?? null } : null;
  try {
    if (await fs.readFile(canonicalPath, 'utf8') !== canonicalText || await fs.readFile(storePath, 'utf8') !== storeText ||
        (snapshotText != null && await fs.readFile(snapshotPath, 'utf8') !== snapshotText)) reason(problems, 'inputs-changed-during-audit');
  } catch { reason(problems, 'inputs-changed-during-audit'); }
  // Validation problems invalidate certification, but candidate/receipt lists
  // above retain precise partial progress without pretending it was committed.
  if (problems.length === 0) result.verifiedCompletedIds = result.persistedCandidateIds.filter(id => allReceipts.has(id));
  result.completedUnits = result.verifiedCompletedIds.length;
  const missing = expected.filter(id => !result.verifiedCompletedIds.includes(id));
  if (missing.length) reason(problems, 'unverified-nodes', { questIds: missing });
  if (!(result.level >= 30)) reason(problems, 'below-level-30');
  if (report.completed !== true || report.v2?.status !== 'completed') reason(problems, 'runner-not-completed');
  result.completed = result.completedUnits === 26 && result.level >= 30 && report.completed === true &&
    report.v2?.status === 'completed' && problems.length === 0;
  result.status = result.completed ? 'completed' : 'partial';
  timing.ordinary.completedWithinDeadline = result.completed && latest.logoutSuccess.at <= timing.ordinary.deadlineAt;
  if (timing.functional) timing.functional.completedWithinDeadline = result.completed &&
    latest.logoutSuccess.at <= timing.functional.deadlineAt && allReceipts.size === 26 &&
    expected.every(id => allReceipts.get(id).complete.at <= timing.functional.deadlineAt);
  return result;
}

export async function auditCasterNewcomerV2Evidence({ evidenceRoot, wizardStorePath, taoistStorePath,
  checkedAt = new Date().toISOString() }) {
  if (!evidenceRoot || !wizardStorePath || !taoistStorePath || !instant(checkedAt)) {
    throw new Error('An evidence root, two independent stores, and an ISO audit time are required');
  }
  const expected = await expectedQuestIds();
  const stores = [wizardStorePath, taoistStorePath];
  let independent = !samePath(...stores);
  try { independent &&= !samePath(...await Promise.all(stores.map(file => fs.realpath(file)))); } catch { /* Per-class audit records missing input. */ }
  const classes = [];
  const owners = new Map();
  for (let index = 0; index < CLASSES.length; index += 1) {
    const row = await auditClass({ evidenceRoot, className: CLASSES[index], storePath: stores[index], expected, checkedAt, owners });
    classes.push(row);
  }
  if (owners.size === 2 && owners.get('Wizard') === owners.get('Taoist')) independent = false;
  for (const row of classes) {
    if (!independent) {
      reason(row.reasons, 'stores-are-not-independent');
      row.verifiedCompletedIds = [];
      row.completedUnits = 0;
      row.completed = false;
      row.status = 'partial';
    }
  }
  const allClassesCompleted = classes.every(row => row.completed);
  return { schema: 1, profile: 'newcomer-v2', checkedAt, scopeClasses: [...CLASSES], denominator: 52,
    status: allClassesCompleted ? 'complete' : 'partial', independentStores: independent,
    completedUnits: classes.reduce((sum, row) => sum + row.completedUnits, 0), allClassesCompleted,
    functionalCompletionWithinDeadline: allClassesCompleted && classes.every(row => row.timing?.functional?.completedWithinDeadline === true),
    freshTimedCompletion: false, measuredTime: false, visualAccepted: false, globalParityPercent: null, classes };
}

if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
  const [evidenceRoot, wizardStorePath, taoistStorePath, ...extra] = process.argv.slice(2);
  if (!evidenceRoot || !wizardStorePath || !taoistStorePath || extra.length) {
    console.error('Usage: node verify-caster-newcomer-v2-evidence.mjs <evidence-root> <Wizard-accounts.json> <Taoist-accounts.json>');
    process.exitCode = 2;
  } else {
    try { console.log(JSON.stringify(await auditCasterNewcomerV2Evidence({ evidenceRoot, wizardStorePath, taoistStorePath }), null, 2)); }
    catch { console.error('Caster evidence audit could not read the required evidence contract.'); process.exitCode = 2; }
  }
}
