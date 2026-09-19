import fs from 'node:fs/promises';
import { completeQuestObjectives } from './protocol-combat.mjs';
import { interactQuest } from './protocol-quest-actions.mjs';
import { createNavigator, reviveInTown, selfPlayer } from './protocol-play.mjs';
import { createMapTraveler } from './protocol-travel.mjs';
import { prepareLoadout, combatAction, meleeCombatAction, useSupplies } from './protocol-loadout.mjs';
import { purchaseV2BasicHpPotion, equipHeldAmulet, moveHeldBeltItemToInventory, restockInVillage } from './protocol-supplies.mjs';
import { expectedItemRewards, verifyItemRewards } from './protocol-rewards.mjs';
import { hasAuthoritativePlayerDeath } from './protocol-observation.mjs';
import { refreshCombatWorldSnapshot } from './protocol-refresh.mjs';
import { cloneConfirmedV2Recoveries } from './newcomer-v2-recovery-ledger.mjs';

const ROOT = new URL('../../../../', import.meta.url);
const V2_CONFIG = new URL('config/quest-guidance/newcomer-journey-v2.json', ROOT);
const NPC_MANIFEST = new URL('packages/game-data/data/generated/crystal_npc_info_manifest.json', ROOT);
const RESPAWN_MANIFEST = new URL('packages/game-data/data/generated/crystal_respawn_manifest.json', ROOT);
const ITEM_MANIFEST = new URL('packages/game-data/data/generated/crystal_item_manifest.json', ROOT);

const CLASS_MASK = Object.freeze({ Warrior: 1, Wizard: 2, Taoist: 4 });
const GENDER_MASK = Object.freeze({ Male: 1, Female: 2 });
const BASIC_HP_ITEM_INDEX = 658; // Crystal item manifest: (HP)DrugSmall.
export const BICHON_SAFE_AREA = Object.freeze({ x: 328, y: 264 });
const TECHNIQUE_SPELL = Object.freeze({ Thrusting: 3, HalfMoon: 4 });
const PRACTICE_SPAWN_CANDIDATE_LIMIT = 3;
const PRACTICE_SPAWN_WAYPOINT_LIMIT = 6;
const PRACTICE_SPAWN_STEP_BUDGET = 120;
const PRACTICE_SPAWN_ATTEMPT_BUDGET = 180;
const PRACTICE_TECHNIQUE_WINDOW_MS = 12_000;
const PRACTICE_TECHNIQUE_MAX_SWINGS = 3;
// V2 quest templates address the live NPC object IDs. Crystal's database
// manifest numbers Board as npc_index 35, while the public world object is
// 24, so never reinterpret these object IDs as database row indexes.
const V2_ENDPOINTS = Object.freeze({
  3: Object.freeze({ scriptKey: 'BichonProvince/BorderVillage/Jane' }),
  24: Object.freeze({ scriptKey: 'BichonProvince/BichonWall/Board' }),
});
const ACTIVE_STAGES = new Set(['available', 'inprogress', 'readytoturnin']);
const normalize = value => String(value ?? '').replace(/[^a-z0-9]/gi, '').toLowerCase();
const stage = value => normalize(value);

/**
 * Load the independently configured V2 cadence and Crystal NPC/respawn
 * manifests. The result is presentation-free runner metadata: it contains no
 * local completion state and cannot grant or finish a quest by itself.
 */
export async function loadNewcomerV2Route({ className, gender, readFile = fs.readFile } = {}) {
  const [configText, npcText, respawnText, itemText] = await Promise.all([
    readFile(V2_CONFIG, 'utf8'),
    readFile(NPC_MANIFEST, 'utf8'),
    readFile(RESPAWN_MANIFEST, 'utf8'),
    readFile(ITEM_MANIFEST, 'utf8'),
  ]);
  return buildNewcomerV2Route({
    config: JSON.parse(configText),
    npcManifest: JSON.parse(npcText),
    respawnManifest: JSON.parse(respawnText),
    itemManifest: JSON.parse(itemText),
    className,
    gender,
  });
}

/** Build the 22 main nodes plus four server-owned growth claims from manifests. */
export function buildNewcomerV2Route({ config, npcManifest, respawnManifest, itemManifest, className, gender }) {
  if (!config || !String(config.profile).match(/^newcomer-v2$/i)) {
    throw new Error('newcomer-v2 runtime configuration is required');
  }
  if (!CLASS_MASK[className] || !GENDER_MASK[gender]) throw new Error('newcomer-v2 route requires a known class and gender');
  const npcById = new Map((npcManifest?.npcs ?? []).map(npc => [Number(npc.npc_index), npc]));
  const itemByName = new Map((itemManifest?.items ?? []).map(item => [String(item.name), item]));
  const respawns = respawnIndex(respawnManifest);
  const quests = (config.quests ?? []).map(definition => buildQuest(definition, npcById, respawns, itemByName, className, gender));
  const growth = (config.growthRewards ?? []).map(definition => buildGrowth(definition, npcById, itemByName, className, gender));
  const ids = [...quests, ...growth].map(quest => quest.questId);
  if (quests.length !== 22 || growth.length !== 4 || new Set(ids).size !== 26) {
    throw new Error('newcomer-v2 must define exactly 22 main quests and 4 growth claims');
  }
  return Object.freeze({
    profile: 'newcomer-v2',
    classMask: Object.freeze({ ...CLASS_MASK }),
    genderMask: Object.freeze({ ...GENDER_MASK }),
    quests: Object.freeze([...quests, ...growth].sort((left, right) => left.order - right.order)),
    mainQuestIds: Object.freeze(quests.map(quest => quest.questId)),
    growthQuestIds: Object.freeze(growth.map(quest => quest.questId)),
  });
}

function buildQuest(definition, npcById, respawns, itemByName, className, gender) {
  const questId = integer(definition?.id ?? definition?.questId, 'quest id');
  const maps = [...new Set((definition?.maps ?? []).map(String).filter(Boolean))];
  if (!maps.length) throw new Error(`q${questId} has no objective maps`);
  const kills = (definition?.kills ?? []).map(kill => buildKill(kill, maps, respawns, questId));
  const flags = (definition?.flags ?? []).map(flag => ({
    number: integer(flag.number, `q${questId} flag number`),
    message: requiredString(flag.message, `q${questId} flag message`),
    kind: requiredString(flag.kind, `q${questId} flag kind`),
    conditions: Object.freeze([
      ...(flag.conditions ?? []).map(String),
    ]),
    requirements: Object.freeze(flag.requirements ?? {}),
  }));
  return Object.freeze({
    questId,
    order: integer(definition.order, `q${questId} order`),
    title: requiredString(definition.title, `q${questId} title`),
    minLevel: integer(definition.minLevel, `q${questId} minimum level`),
    requiredQuestId: Number(definition.requiredQuestId ?? 0),
    afterQuestIds: Object.freeze((definition.afterQuestIds ?? []).map(Number)),
    objectiveMaps: Object.freeze(maps),
    startNpc: endpoint(definition.startNpcId, npcById, questId, 'start'),
    finishNpc: endpoint(definition.finishNpcId, npcById, questId, 'finish'),
    startInDiary: Number(definition.startNpcId) === 0,
    finishInDiary: Number(definition.finishNpcId) === 0,
    objectives: Object.freeze({ kill: Object.freeze(kills), item: Object.freeze([]), flag: Object.freeze(flags) }),
    rewards: Object.freeze({ fixedItems: Object.freeze(resolveRewards(definition.rewards, itemByName, className, gender, questId)), selectableItems: Object.freeze([]) }),
    kind: 'main',
  });
}

function buildGrowth(definition, npcById, itemByName, className, gender) {
  const questId = integer(definition?.id ?? definition?.questId, 'growth quest id');
  const finishNpcId = Number(definition.finishNpcId ?? definition.endpointNpcId ?? 0);
  return Object.freeze({
    questId,
    order: 10_000 + integer(definition.level, `q${questId} level`),
    title: `Level ${definition.level} growth reward`,
    minLevel: integer(definition.level, `q${questId} level`),
    requiredQuestId: Number(definition.afterQuestId ?? definition.afterNode ?? 0),
    afterQuestIds: Object.freeze([]),
    objectiveMaps: Object.freeze([]),
    startNpc: endpoint(finishNpcId, npcById, questId, 'growth start'),
    finishNpc: endpoint(finishNpcId, npcById, questId, 'growth finish'),
    startInDiary: finishNpcId === 0,
    finishInDiary: finishNpcId === 0,
    objectives: Object.freeze({ kill: Object.freeze([]), item: Object.freeze([]), flag: Object.freeze([]) }),
    rewards: Object.freeze({ fixedItems: Object.freeze(resolveRewards(definition.rewards, itemByName, className, gender, questId)), selectableItems: Object.freeze([]) }),
    kind: 'growth',
  });
}

function endpoint(npcId, npcById, questId, phase) {
  const id = Number(npcId ?? 0);
  if (id === 0) return null;
  const endpoint = V2_ENDPOINTS[id];
  const npc = endpoint
    ? [...npcById.values()].find(candidate => candidate.script_key === endpoint.scriptKey)
    : npcById.get(id);
  if (!npc?.location || !npc.map_file_name) throw new Error(`q${questId} ${phase} NPC ${id} is absent from Crystal NPC manifest`);
  return Object.freeze({
    objectId: id,
    name: String(npc.name),
    mapFileName: String(npc.map_file_name),
    position: Object.freeze({ x: integer(npc.location.x, `NPC ${id} x`), y: integer(npc.location.y, `NPC ${id} y`) }),
    scriptKey: String(npc.script_key ?? npc.file_name ?? ''),
  });
}

function buildKill(kill, maps, respawns, questId) {
  const monsterIndex = integer(kill.monsterIndex, `q${questId} monster index`);
  const spawns = (respawns.get(monsterIndex) ?? []).filter(spawn => maps.includes(spawn.mapFileName));
  if (!spawns.length) throw new Error(`q${questId} ${kill.monster} has no Crystal respawn in ${maps.join(',')}`);
  // completeQuestObjectives consumes `spawnCandidates`, rather than the
  // presentation-oriented `spawns` name used by the V2 practice driver.
  // Keep one immutable manifest-backed source list under both names so its
  // normal destination chooser can find every remaining multi-kill target.
  const spawnCandidates = Object.freeze(spawns.map(spawn => Object.freeze({
    ...spawn,
    monsterIndex,
    monsterName: requiredString(kill.monster, `q${questId} monster name`),
  })));
  return Object.freeze({
    monsterName: requiredString(kill.monster, `q${questId} monster name`),
    monsterIndex,
    count: integer(kill.count, `q${questId} kill count`),
    spawnCandidates,
    spawns: spawnCandidates,
  });
}

function respawnIndex(manifest) {
  const index = new Map();
  for (const map of manifest?.maps ?? []) {
    for (const spawn of map?.respawns ?? []) {
      const monsterIndex = Number(spawn?.monster_index);
      if (!Number.isSafeInteger(monsterIndex)) continue;
      const location = spawn.location;
      if (!location || !map.map_file_name) continue;
      const count = Number(spawn.count);
      if (!Number.isSafeInteger(count) || count <= 0) continue;
      const entry = Object.freeze({
        mapFileName: String(map.map_file_name),
        mapTitle: String(map.map_title ?? map.map_file_name),
        position: Object.freeze({ x: integer(location.x, 'respawn x'), y: integer(location.y, 'respawn y') }),
        count,
        spread: Math.max(0, Number(spawn.spread ?? 0)),
        delayMinutes: Math.max(0, Number(spawn.delay_minutes ?? spawn.delayMinutes ?? 0)),
        respawnIndex: Number(spawn.respawn_index ?? -1),
        isBoss: false,
      });
      const entries = index.get(monsterIndex) ?? [];
      entries.push(entry);
      index.set(monsterIndex, entries);
    }
  }
  return index;
}

function resolveRewards(rewards, itemByName, className, gender, questId) {
  const selected = [...(rewards?.common ?? []), ...(rewards?.[className.toLowerCase()] ?? [])];
  return selected.map(reward => {
    const itemName = String(reward?.name ?? (gender === 'Female' ? reward?.femaleItem : reward?.maleItem) ?? '');
    const item = itemByName.get(itemName);
    const count = Number(reward?.count);
    if (!item || !Number.isSafeInteger(Number(item.item_index)) || !Number.isSafeInteger(count) || count <= 0) {
      throw new Error(`q${questId} has an invalid ${className} reward ${itemName}`);
    }
    return Object.freeze({ itemIndex: Number(item.item_index), itemName, count });
  });
}

/** Read only a server-projected V2 flag row; local booleans never satisfy it. */
export function v2FlagState(snapshot, questId, flag) {
  const quest = (snapshot?.questLog ?? []).find(entry => Number(entry?.questId) === Number(questId));
  const objective = (quest?.objectives ?? []).find(entry =>
    Number(entry?.number ?? entry?.objectiveNumber) === Number(flag?.number) ||
    normalize(entry?.label).includes(normalize(flag?.message))
  );
  if (!objective) return Object.freeze({ known: false, complete: false });
  const complete = objective.done === true ||
    (Number(objective.required) > 0 && Number(objective.current) >= Number(objective.required));
  return Object.freeze({ known: true, complete });
}

/** Fail closed until the server snapshot, not a runner-local marker, confirms every flag. */
export function assertV2FlagsConfirmed(snapshot, routeQuest) {
  for (const flag of routeQuest?.objectives?.flag ?? []) {
    const current = v2FlagState(snapshot, routeQuest.questId, flag);
    if (!current.known || !current.complete) throw new Error(`q${routeQuest.questId} flag ${flag.number} lacks server confirmation`);
  }
  return true;
}

/** Growth is offered only when the server has inserted an available quest row. */
export function availableV2Growth(snapshot, route) {
  return route.quests.filter(quest => quest.kind === 'growth' &&
    stage((snapshot?.questLog ?? []).find(entry => Number(entry?.questId) === quest.questId)?.stage) === 'available');
}

/**
 * Run one bounded public-protocol V2 pass. It never writes a quest store or
 * manufactures a flag: acceptance, attacks, purchases and finish receipts
 * must each advance the authoritative snapshot. A blocked condition returns a
 * durable pause record for a later normal resume.
 */
export async function runNewcomerV2Journey({ client, className, gender, report, ordinaryStartedAt, deadlineMs = 120 * 60_000, functionalRecheck = null, checkpoint = null, survival = {}, recovery = {} }) {
  if (!client?.snapshot) throw new Error('V2 runner requires an authoritative bootstrap snapshot');
  const route = await loadNewcomerV2Route({ className, gender });
  const inWorldStartedAt = Date.now();
  const reportStartedAt = Date.parse(ordinaryStartedAt ?? report?.startedAt ?? '');
  // The run report is created before login. Keep that wall clock for every
  // V2 retry in this normal process, while retaining in-world time separately
  // for diagnosing route work from connection/bootstrap cost.
  const startedAt = Number.isFinite(reportStartedAt) ? reportStartedAt : inWorldStartedAt;
  const recheckStartedAt = functionalRecheck ? Date.parse(functionalRecheck.startedAt) : null;
  const recheckDeadline = functionalRecheck ? Date.parse(functionalRecheck.deadlineAt) : null;
  if (functionalRecheck && (!Number.isFinite(recheckStartedAt) || !Number.isFinite(recheckDeadline) || recheckDeadline !== recheckStartedAt + deadlineMs)) {
    throw new Error('invalid functional recheck clock');
  }
  const deadline = functionalRecheck ? recheckDeadline : startedAt + deadlineMs;
  const checkDeadline = () => {
    if (Date.now() >= deadline) throw new V2Pause(functionalRecheck ? 'functionalRecheckDeadline' : 'deadline', null,
      functionalRecheck ? '120-minute functional V2 recheck deadline reached' : '120-minute V2 journey deadline reached');
  };
  // The route helper modules own their loops.  Give each one a client whose
  // sends and waits share this absolute deadline, so an old inner loop cannot
  // continue issuing public commands after the journey's ordinary-run cap.
  const boundedClient = deadlineBoundClient(client, deadline, checkDeadline);
  const navigate = createNavigator(boundedClient, {
    emergencyEscape: typeof survival.emergencyEscape === 'function' ? survival.emergencyEscape : undefined,
    emergencyEscapeHpRatio: Number(survival.emergencyEscapeHpRatio ?? 0),
    maxEmergencyEscapesPerNavigation: Number.isSafeInteger(Number(survival.maxEmergencyEscapesPerNavigation))
      ? Number(survival.maxEmergencyEscapesPerNavigation)
      : undefined,
  });
  const travel = createMapTraveler(boundedClient, navigate);
  const priorRecoveries = Object.hasOwn(recovery, 'priorRecoveries')
    ? recovery.priorRecoveries
    : [];
  const result = {
    profile: route.profile, status: 'running', routeQuestIds: route.quests.map(quest => quest.questId), attempts: [],
    recoveries: cloneConfirmedV2Recoveries(priorRecoveries),
    ordinaryStartedAt: new Date(startedAt).toISOString(), deadlineAt: new Date(startedAt + deadlineMs).toISOString(),
    ...(functionalRecheck ? {
      functionalRecheckStartedAt: functionalRecheck.startedAt,
      functionalRecheckDeadlineAt: functionalRecheck.deadlineAt,
      functionalRecheckElapsedMs: Number(functionalRecheck.elapsedMs ?? 0),
      ordinaryElapsedMs: recovery.ordinaryElapsedMs ?? report?.ordinaryElapsedMs ?? report?.v2?.ordinaryElapsedMs,
    } : {}),
  };
  try {
    await recoverV2Death({
      client: boundedClient, result, report, checkpoint, inWorldStartedAt, startedAt,
      phase: 'bootstrap', recovery,
    });
    // Replace an earlier run's paused/completed QA projection as soon as this
    // ordinary process has entered the world. This remains local evidence;
    // it has no relationship to server quest state.
    await checkpointV2Progress({ checkpoint, report, result, inWorldStartedAt, startedAt });
    for (let step = 0; step < 80; step += 1) {
      checkDeadline();
      const quest = nextServerQuest(boundedClient.snapshot, route);
      if (!quest) {
        const completion = v2CompletionState(boundedClient.snapshot, route);
        if (completion.completed) {
          result.status = 'completed';
          result.completed = true;
        } else if (completion.allQuestsCompleted) {
          throw new V2Pause('awaitingLevel30', null,
            `all 26 V2 quests are server Completed, but authoritative level ${completion.level} is below 30`);
        } else {
          throw new V2Pause('awaitingServerOffer', null, 'no V2 quest is currently offered by the server');
        }
        break;
      }
      const record = { questId: quest.questId, kind: quest.kind, startedAt: new Date().toISOString() };
      result.attempts.push(record);
      // Persist the running attempt before its first public action. A crash
      // during a long combat/search must not leave the last report claiming
      // that an older attempt is still the current state.
      await checkpointV2Progress({ checkpoint, report, result, inWorldStartedAt, startedAt });
      try {
        const before = serverQuest(boundedClient.snapshot, quest.questId);
        if (stage(before?.stage) === 'available') {
          await acceptV2Quest(boundedClient, quest, navigate, travel, checkDeadline);
          await waitForServerStage(boundedClient, quest.questId, ['inprogress', 'readytoturnin'], `q${quest.questId} accept`);
        }
        const current = serverQuest(boundedClient.snapshot, quest.questId);
        if (stage(current?.stage) === 'inprogress') {
          // A resumed character can be alive but one hit from death with no
          // potions. Complete the ordinary departure check before map travel
          // or practice can issue a combat command.
          if (typeof survival.ensureReady === 'function') {
            const readiness = await survival.ensureReady(boundedClient, quest, navigate);
            if (readiness?.status !== 'ready') {
              throw new V2Pause('awaitingSafeSupplies', quest.questId,
                String(readiness?.message ?? `q${quest.questId} needs ordinary travel supplies before combat`));
            }
          }
          await completeV2Objectives(boundedClient, quest, { navigate, travel, className, checkDeadline, survival });
          try {
            assertV2FlagsConfirmed(boundedClient.snapshot, quest);
          } catch {
            throw new V2Pause('awaitingServerFlag', quest.questId, `q${quest.questId} flags are not server-confirmed`);
          }
          await waitForServerStage(boundedClient, quest.questId, ['readytoturnin'], `q${quest.questId} objectives`);
        }
        if (stage(serverQuest(boundedClient.snapshot, quest.questId)?.stage) === 'readytoturnin') {
          const inventoryBefore = structuredClone(boundedClient.snapshot);
          await finishV2Quest(boundedClient, quest, navigate, travel, checkDeadline);
          await waitForServerStage(boundedClient, quest.questId, ['completed'], `q${quest.questId} finish`);
          record.itemRewards = verifyItemRewards(inventoryBefore, boundedClient.snapshot, expectedItemRewards(quest, -1));
        }
        if (stage(serverQuest(boundedClient.snapshot, quest.questId)?.stage) !== 'completed') {
          throw new V2Pause('serverNoProgress', quest.questId, `q${quest.questId} did not reach Completed`);
        }
        record.completedAt = new Date().toISOString();
        // This writes only the local QA report. It preserves the original
        // ordinary-run clock and completed receipts across a process crash; it
        // has no protocol, store, or server-state side effect.
        await checkpointV2Progress({ checkpoint, report, result, inWorldStartedAt, startedAt });
      } catch (error) {
        const recovered = await recoverV2Death({
          client: boundedClient, result, report, checkpoint, inWorldStartedAt, startedAt,
          questId: quest.questId, phase: 'quest', recovery,
        });
        if (!recovered) throw error;
        record.interruptedByDeath = true;
        // Re-select strictly from the new authoritative quest log. Do not
        // infer the old quest's state or replay a local completion marker.
        continue;
      }
    }
  } catch (error) {
    const pause = error instanceof V2Pause
      ? error
      : new V2Pause('operationFailed', result.attempts.at(-1)?.questId ?? null, String(error?.message ?? error));
    result.status = 'paused';
    result.pause = { reason: pause.reason, questId: pause.questId, message: pause.message, at: new Date().toISOString() };
  }
  updateV2TimingReport(report, result, inWorldStartedAt, startedAt);
  return result;
}

/** Persist a completed-node receipt through the runner's local QA callback. */
export async function checkpointV2Progress({ checkpoint, report, result, inWorldStartedAt, startedAt }) {
  updateV2TimingReport(report, result, inWorldStartedAt, startedAt);
  if (typeof checkpoint !== 'function') return false;
  await checkpoint(report);
  return true;
}

function updateV2TimingReport(report, result, inWorldStartedAt, startedAt) {
  result.elapsedMs = Date.now() - inWorldStartedAt;
  const functionalRecheck = result.functionalRecheckStartedAt;
  if (functionalRecheck) {
    result.functionalRecheckElapsedMs = Date.now() - Date.parse(functionalRecheck);
  } else {
    result.ordinaryElapsedMs = Date.now() - startedAt;
  }
  if (!report) return;
  report.v2 = result;
  report.inWorldElapsedMs = result.elapsedMs;
  if (functionalRecheck) {
    report.functionalRecheckElapsedMs = result.functionalRecheckElapsedMs;
    report.ordinaryElapsedMs = result.ordinaryElapsedMs;
  } else report.ordinaryElapsedMs = result.ordinaryElapsedMs;
}

/**
 * Recover only an already-authoritative death. ReviveInTown itself proves the
 * public Revived/world snapshot receipt; this wrapper records both lifecycle
 * edges locally and never edits quest progress. A recovered pass always
 * reselects its work from the server quest log.
 */
export async function recoverV2Death({
  client,
  result,
  report,
  checkpoint,
  inWorldStartedAt,
  startedAt,
  questId = null,
  phase = 'quest',
  recovery = {},
}) {
  const isDead = typeof recovery.isDead === 'function' ? recovery.isDead : hasAuthoritativePlayerDeath;
  if (!isDead(client?.snapshot)) return false;
  const maxRecoveries = Number.isSafeInteger(Number(recovery.maxRecoveries))
    ? Math.max(0, Number(recovery.maxRecoveries))
    : 3;
  const recoveries = result.recoveries ??= [];
  if (recoveries.length >= maxRecoveries) {
    throw new V2Pause('deathRecoveryLimit', questId,
      `q${questId ?? 'bootstrap'} exceeded the ${maxRecoveries} authoritative V2 recovery limit`);
  }
  const before = selfPlayer(client);
  const entry = {
    phase,
    questId,
    deathAt: new Date().toISOString(),
    before: before ? {
      mapFileName: String(client?.snapshot?.mapFileName ?? ''),
      objectId: Number(before.objectId), x: Number(before.x), y: Number(before.y), hp: Number(before.hp ?? client?.snapshot?.playerHp),
    } : null,
  };
  recoveries.push(entry);
  await checkpointV2Progress({ checkpoint, report, result, inWorldStartedAt, startedAt });
  const revive = typeof recovery.revive === 'function' ? recovery.revive : reviveInTown;
  let receipt;
  try {
    receipt = await revive(client);
  } catch (error) {
    throw new V2Pause('reviveUnconfirmed', questId,
      `q${questId ?? 'bootstrap'} authoritative death could not be revived: ${String(error?.message ?? error)}`);
  }
  if (!receipt || isDead(client?.snapshot)) {
    throw new V2Pause('reviveUnconfirmed', questId,
      `q${questId ?? 'bootstrap'} lacked an authoritative town-revive receipt`);
  }
  const after = selfPlayer(client);
  entry.revive = receipt;
  entry.revivedAt = new Date().toISOString();
  entry.after = after ? {
    mapFileName: String(client?.snapshot?.mapFileName ?? ''),
    objectId: Number(after.objectId), x: Number(after.x), y: Number(after.y), hp: Number(after.hp ?? client?.snapshot?.playerHp),
  } : null;
  await checkpointV2Progress({ checkpoint, report, result, inWorldStartedAt, startedAt });
  return true;
}

class V2Pause extends Error {
  constructor(reason, questId, message) {
    super(message);
    this.reason = reason;
    this.questId = questId;
  }
}

function deadlineBoundClient(client, deadline, checkDeadline) {
  const remaining = () => {
    checkDeadline();
    return Math.max(1, deadline - Date.now());
  };
  const boundedTimeout = timeout => {
    const requested = Number(timeout);
    return Math.min(remaining(), Number.isFinite(requested) && requested > 0 ? requested : 20_000);
  };
  return new Proxy(client, {
    get(target, property, receiver) {
      if (property === 'send') return command => {
        checkDeadline();
        return target.send(command);
      };
      if (property === 'request') return (command, label, timeout) => {
        checkDeadline();
        return target.request(command, label, boundedTimeout(timeout));
      };
      if (property === 'wait') return (predicate, label, timeout) =>
        target.wait(predicate, label, boundedTimeout(timeout));
      const value = Reflect.get(target, property, receiver);
      return typeof value === 'function' ? value.bind(target) : value;
    },
  });
}

function nextServerQuest(snapshot, route) {
  const entries = new Map((snapshot?.questLog ?? []).map(entry => [Number(entry?.questId), entry]));
  return route.quests.find(quest => ['readytoturnin', 'inprogress', 'available'].includes(stage(entries.get(quest.questId)?.stage))) ?? null;
}

/** Completion is server-owned: all 26 V2 rows must be Completed and level is at least 30. */
export function v2CompletionState(snapshot, route) {
  const requiredIds = [...new Set((route?.quests ?? []).map(quest => Number(quest?.questId)).filter(Number.isSafeInteger))];
  const completedQuestIds = requiredIds.filter(questId => stage(serverQuest(snapshot, questId)?.stage) === 'completed');
  const level = Number(selfPlayer({ snapshot })?.level ?? snapshot?.playerLevel ?? 0);
  const allQuestsCompleted = requiredIds.length === 26 && completedQuestIds.length === requiredIds.length;
  return Object.freeze({
    completedQuestIds: Object.freeze(completedQuestIds),
    allQuestsCompleted,
    level: Number.isFinite(level) ? level : 0,
    completed: allQuestsCompleted && level >= 30,
  });
}

function serverQuest(snapshot, questId) {
  return (snapshot?.questLog ?? []).find(entry => Number(entry?.questId) === Number(questId)) ?? null;
}

async function acceptV2Quest(client, quest, navigate, travel, checkDeadline) {
  checkDeadline();
  if (quest.startNpc) await travel(quest.startNpc.mapFileName);
  checkDeadline();
  await interactQuest(client, quest, { type: 'accept' }, navigate);
}

async function finishV2Quest(client, quest, navigate, travel, checkDeadline) {
  checkDeadline();
  if (quest.finishNpc) await travel(quest.finishNpc.mapFileName);
  checkDeadline();
  await interactQuest(client, quest, { type: 'finish', selectedItemIndex: -1 }, navigate);
}

export async function completeV2Objectives(client, quest, { navigate, travel, className, checkDeadline, survival = {} }) {
  checkDeadline();
  let enteredObjectiveMap = false;
  const mapEntryAfter = Number(client.sequence);
  if (quest.objectiveMaps.length && !quest.objectiveMaps.includes(String(client.snapshot?.mapFileName ?? ''))) {
    await travel(quest.objectiveMaps[0]);
    enteredObjectiveMap = true;
  }
  checkDeadline();
  if (enteredObjectiveMap && questHasMapEntryFlag(quest)) {
    // D001, D401, and D021 entry flags often share an AND display group with
    // learning and loadout conditions. Do not wait for that aggregate flag
    // before those prerequisites run. A fresh map packet plus the landing
    // position proves this transfer; the flag is still checked after all
    // preparation and objective work below.
    await waitForMapEntryReceipt(client, quest, mapEntryAfter);
  }
  await prepareLoadout(client);
  await learnV2Skills(client, requiredV2Skills(quest, className));
  if (className === 'Taoist') await equipHeldAmulet(client);
  if (Number(quest?.questId) === 2110007 && className === 'Taoist') {
    const readiness = await restockInVillage(client, navigate, {
      ensureTaoistForestWeapon: true,
    });
    if (readiness.status !== 'restocked' && readiness.status !== 'sufficient') {
      throw new V2Pause(
        readiness.status === 'needsMandatorySupplies' ? 'awaitingForestSupplies' : 'awaitingForestWeapon',
        quest.questId,
        readiness.status === 'needsMandatorySupplies'
          ? 'q2110007 Taoist requires held HP, MP, and transport supplies before an ordinary melee upgrade'
          : 'q2110007 Taoist requires a live, affordable Blacksmith melee upgrade',
      );
    }
  }
  await driveServerEventFlags(client, quest, navigate, checkDeadline);
  if (needsPotionPurchase(quest)) {
    await purchaseRequiredBasicPotion(client, navigate, quest, checkDeadline);
  }
  // Practice first while the manifest-backed spawn is still available. The
  // normal objective loop may consume the final nearby target before a later
  // server-event condition has a lawful target to observe.
  if (quest.objectives.flag.length && !flagsComplete(client.snapshot, quest)) {
    await driveV2Practice(client, quest, navigate, className, checkDeadline);
  }
  if (quest.objectives.kill.length) {
    await completeQuestObjectives(client, quest, navigate, {
      travel,
      action: (owner, target) => combatAction(owner, target),
      prepare: async owner => {
        checkDeadline();
        return v2Sustain(owner, survival, checkDeadline, { hpThreshold: 0.6, mpThreshold: 0.35 });
      },
      // `prepare` runs once per engagement; sustain is also called from the
      // ordinary attack/search/retreat cadence so stocked restoratives are not
      // stranded while a live objective needs several public actions.
      sustain: owner => v2Sustain(owner, survival, checkDeadline, { hpThreshold: 0.75, mpThreshold: 0.35 }),
      recoverAfterUnsafeRetreat: async (owner, navigateNear) => {
        checkDeadline();
        if (typeof survival.recoverAfterUnsafeRetreat === 'function') {
          return survival.recoverAfterUnsafeRetreat(owner, navigateNear);
        }
        return v2Sustain(owner, survival, checkDeadline, { hpThreshold: 0.85, mpThreshold: 0.35 });
      },
      emergencyEscape: typeof survival.emergencyEscape === 'function' ? survival.emergencyEscape : undefined,
      // The two Taoist cave Skeleton steps can trap a low-armour caster in
      // four adjacent hostiles before the generic 35% escape threshold. Use
      // the already-held public RandomTeleport while it is still survivable;
      // the existing one-scroll cap and authoritative relocation still apply.
      emergencyEscapeHpRatio: className === 'Taoist' && [2110010, 2110011].includes(Number(quest.questId))
        ? Math.max(0.65, Number(survival.emergencyEscapeHpRatio ?? 0))
        : Number(survival.emergencyEscapeHpRatio ?? 0),
      // Each inner bounded loop also receives the deadline-guarded client.
      // Keep its own budgets deliberately small so a later ordinary resume is
      // preferred over speculative spawn searching.
      maxEngagements: 48,
      maxAttackAttempts: 20,
      maxSpawnSearches: 4,
      maxSpawnWaypoints: 120,
      spawnSearchTimeoutMs: 30_000,
      spawnRespawnWaitMs: 5_000,
      maxSpawnRespawnWaits: 1,
      questSettleTimeout: 12_000,
      preferredObjectiveMaps: quest.objectiveMaps,
      // At the Taoist Skeleton steps, a nearby BoneFighter can outlast the
      // fixed attack budget even though it is not a quest target. Keep the
      // objective in focus and retreat from an unsafe pull instead. At
      // Warrior N16, a full-health adjacent Zombie3 can similarly exhaust
      // the cap before a wounded objective Zombie3 is reached.
      focusTargetThroughAggressors:
        (className === 'Taoist' && [2110010, 2110011].includes(Number(quest.questId))) ||
        (className === 'Warrior' && Number(quest.questId) === 2110016),
      refreshWhileWaiting: refreshCombatWorldSnapshot,
      retryUnclaimedSnapshotCorpse: true,
      retrySpawnSearchTimeout: true,
      // V2 keeps only live AOI target hints ahead of the certified field. A
      // historical same-map packet can be minutes old and must not spend this
      // route's fixed 30-second coverage window before its nearest waypoint.
      allowHistoricalSpawnHints: false,
    });
  }
}

async function v2Sustain(owner, survival, checkDeadline, thresholds) {
  checkDeadline();
  if (typeof survival.sustain === 'function') return survival.sustain(owner, thresholds);
  return useSupplies(owner, thresholds);
}

function needsPotionPurchase(quest) {
  return quest.objectives.flag.some(flag => flag.conditions.includes('ordinaryBasicPotionBought'));
}

function questHasMapEntryFlag(quest) {
  return quest.objectives.flag.some(flag => (flag.conditions ?? []).some(condition =>
    /(?:caveentryreached|mineentryreached|entrancereached)/i.test(String(condition))));
}

function flagsComplete(snapshot, quest) {
  try { return assertV2FlagsConfirmed(snapshot, quest); } catch { return false; }
}

async function driveServerEventFlags(client, quest, navigate, checkDeadline) {
  const conditions = quest.objectives.flag.flatMap(flag => flag.conditions ?? []).map(String);
  if (conditions.includes('bichonSafeArrival')) {
    // q5 stays on map 0, so map travel would be a no-op. Walk to the actual
    // Bichon safe-area receipt near Jane instead.
    if (String(client.snapshot?.mapFileName) !== '0') {
      throw new V2Pause('safeArrivalWrongMap', quest.questId, 'Bichon safe-arrival must be reached by ordinary walking on map 0');
    }
    const before = practiceFingerprint(client.snapshot, quest);
    checkDeadline();
    await navigate(BICHON_SAFE_AREA, 4, () => false, { maxSuccessfulSteps: 520, maxAttempts: 640, detectPositionCycles: true });
    await waitForPracticeReceipt(client, quest, before, { kind: 'bichonSafeArrival' });
  }
  if (conditions.includes('expeditionNpcReport') || conditions.includes('expeditionFinalReport')) {
    const before = practiceFingerprint(client.snapshot, quest);
    await publicNpcReport(client, quest.finishNpc ?? quest.startNpc, navigate, checkDeadline);
    await waitForPracticeReceipt(client, quest, before, { kind: 'npcReport' });
  }
}

async function publicNpcReport(client, endpoint, navigate, checkDeadline) {
  if (!endpoint) throw new Error('server report lacks a configured NPC endpoint');
  checkDeadline();
  await navigate(endpoint.position, 16, () => false, { maxSuccessfulSteps: 180, maxAttempts: 240, detectPositionCycles: true });
  const npc = await client.wait(() => (client.snapshot?.entities ?? []).find(entity =>
    entity?.kind === 'npc' && Number(entity?.objectId) === Number(endpoint.objectId)), `live report NPC ${endpoint.objectId}`, 20_000);
  const after = client.sequence;
  client.send({ type: 'interact', objectId: Number(npc.objectId) });
  await client.wait(() => client.sequence > after &&
    String(client.snapshot?.activeNpcDialog?.npcObjectId) === String(npc.objectId), `report NPC ${endpoint.objectId} dialog`, 20_000);
}

async function driveV2Practice(client, quest, navigate, className, checkDeadline) {
  const plan = practicePlan(client.snapshot, quest, className);
  if (!plan.length) return;
  return executeV2PracticePlan({ client, quest, navigate, checkDeadline, plan });
}

/**
 * Execute a supplied plan through public packets. Exported for protocol
 * contract tests; the runner always obtains `plan` from `practicePlan`.
 */
export async function executeV2PracticePlan({ client, quest, navigate, checkDeadline = () => {}, plan = [] }) {
  // q21 is an AND sequence. Reacquire a live target for every damaging step
  // and prefer a different AOI object when one exists, while retaining a
  // one-monster fallback for sparse legitimate respawns.
  const usedTargetIds = new Set();
  const acquireTarget = async (desiredDistance, options = {}) => {
    const target = await approachPracticeTarget(client, quest, navigate, desiredDistance, checkDeadline, {
      ...options,
      excludeObjectIds: usedTargetIds,
    });
    usedTargetIds.add(Number(target.objectId));
    return target;
  };
  for (const step of plan) {
    checkDeadline();
    if (step.kind === 'poison') {
      const target = await acquireTarget(6);
      await equipHeldPoison(client, quest.questId);
      const cast = await castV2Spell(client, target, 'Poisoning', checkDeadline, quest.questId);
      await waitForPoisonEvidence(client, cast.after, cast.target, cast.poisonBefore, quest.questId);
    } else if (step.kind === 'normal') {
      const target = await acquireTarget(1);
      const after = client.sequence;
      const hp = Number(target.hp);
      await meleeCombatAction(client, target);
      await waitForTargetDamage(client, after, target, hp, quest.questId, 'normal attack');
    } else if (step.kind === 'technique') {
      const range = step.spell === 'Thrusting' ? 2 : 1;
      let firstSwingAt = null;
      for (let swing = 0; swing < PRACTICE_TECHNIQUE_MAX_SWINGS; swing += 1) {
        checkDeadline();
        if (firstSwingAt != null && Date.now() - firstSwingAt >= PRACTICE_TECHNIQUE_WINDOW_MS) {
          throw new V2Pause('awaitingPracticeDamage', quest.questId, `${step.spell} missed within the original receipt window`);
        }
        // Crystal weapon techniques are stateful toggles. Re-arm and choose a
        // fresh live ray target after a lawful accuracy miss; a mere attack
        // animation is not evidence of positive damage.
        await armWeaponTechnique(client, quest.questId, step.spell);
        const target = await acquireTarget(range, step.spell === 'Thrusting'
          ? { directionalRay: { minRange: 1, maxRange: 2 } }
          : {});
        const after = client.sequence;
        const hp = Number(target.hp);
        await attackDirectionTechnique(client, target, step.spell);
        firstSwingAt ??= Date.now();
        const remaining = Math.max(1, PRACTICE_TECHNIQUE_WINDOW_MS - (Date.now() - firstSwingAt));
        const receiptTimeout = swing + 1 < PRACTICE_TECHNIQUE_MAX_SWINGS ? Math.min(2_500, remaining) : remaining;
        try {
          await waitForTargetDamage(client, after, target, hp, quest.questId, step.spell, TECHNIQUE_SPELL[step.spell], receiptTimeout);
          break;
        } catch (error) {
          const acceptedSwing = receivedAfter(client, after, 'ObjectAttack', payload =>
            Number(payload?.objectId) === Number(selfPlayer(client)?.objectId) &&
            Number(payload?.spell) === TECHNIQUE_SPELL[step.spell]);
          if (error?.reason !== 'awaitingPracticeDamage' || !acceptedSwing ||
              swing + 1 >= PRACTICE_TECHNIQUE_MAX_SWINGS) throw error;
          usedTargetIds.delete(Number(target.objectId));
        }
      }
    } else if (step.kind === 'healing') {
      // This is deliberately not gated on damage or HP: q4 requires the
      // accepted self-cast even when the new character is still at full HP.
      const cast = await castV2Spell(client, selfPlayer(client), 'Healing', checkDeadline, quest.questId);
      await waitForSelfMagic(client, cast.after, 'Healing', quest.questId);
    } else if (step.kind === 'summon') {
      const target = await acquireTarget(6);
      await equipHeldAmulet(client);
      const cast = await castV2Spell(client, target, 'SummonSkeleton', checkDeadline, quest.questId);
      await client.wait(() => ownedBoneFamiliar(client.snapshot), `q${quest.questId} owned BoneFamiliar`, 12_000)
        .catch(() => { throw new V2Pause('awaitingOwnedPet', quest.questId, 'SummonSkeleton lacked an authoritative owned-pet receipt'); });
      const pet = ownedBoneFamiliar(client.snapshot);
      await waitForPetDamage(client, cast.after, pet, cast.target, quest.questId);
    } else if (step.kind === 'spell') {
      // Lightning is a six-tile, directional Zone ray. Select a live target
      // on that ray, preferring a distant aligned tile if movement is needed;
      // the self-routed client packet alone does not make an off-axis target
      // hittable.
      const target = await acquireTarget(step.spell === 'Lightning' ? 6 : 8,
        step.spell === 'Lightning'
          ? { directionalRay: { minRange: 1, maxRange: 6, preferDistant: true } }
          : {});
      if (step.spell === 'SoulFireBall') await equipHeldAmulet(client);
      const cast = await castV2Spell(client, target, step.spell, checkDeadline, quest.questId);
      await waitForSpellDamage(client, cast.after, cast.target, cast.beforeHp, step.spell, quest.questId);
    } else if (step.kind === 'reposition') {
      const target = await acquireTarget(8);
      const actor = selfPlayer(client);
      const destination = legalReposition(actor, target);
      await navigate(destination, 0, () => false, { maxSuccessfulSteps: 12, maxAttempts: 20, detectPositionCycles: true });
      if (distance(selfPlayer(client), actor) < 1) throw new V2Pause('repositionRejected', quest.questId, 'wizard reposition had no authoritative movement receipt');
    }
  }
  // A display group can be 0/1 for an AND-chain, so it is valid for its first
  // two actions to leave the group at zero. Verify the authoritative group
  // only after every independently receipted action above has completed.
  await client.wait(() => flagsComplete(client.snapshot, quest), `q${quest.questId} final practice flags`, 12_000)
    .catch(() => { throw new V2Pause('awaitingServerFlag', quest.questId, `q${quest.questId} final practice flags were not server-confirmed`); });
}

/** One real action for every unresolved class condition, in stable order. */
export function practicePlan(snapshot, quest, className) {
  const conditions = classConditions(quest, className);
  const includes = fragment => conditions.some(condition => normalize(condition).includes(normalize(fragment)));
  const steps = [];
  if (className === 'Warrior') {
    if (includes('Fencing') || includes('Slaying')) steps.push(Object.freeze({ kind: 'normal' }));
    for (const spell of ['HalfMoon', 'Thrusting']) if (includes(spell) && includes(`${spell} attack damage`)) steps.push(Object.freeze({ kind: 'technique', spell }));
  } else if (className === 'Wizard') {
    const spellSteps = ['FireBall', 'GreatFireBall', 'Lightning', 'FireWall']
      .filter(spell => includes(spell) && (includes(`${spell} damage`) || (spell === 'FireWall' && includes('owned FireWall damage'))));
    if (includes('spell damage followed') && spellSteps.length === 0) {
      const known = new Set((snapshot?.knownSkills ?? []).map(skill => String(skill?.spell)));
      const spell = ['Lightning', 'GreatFireBall', 'FireBall'].find(candidate => known.has(candidate));
      if (spell) spellSteps.push(spell);
    }
    if (includes('reposition')) {
      // q21 records the movement only when it occurs between two legal spell
      // damages. Keep FireWall after the walk rather than sending both casts
      // before movement.
      const beforeMove = spellSteps.find(spell => spell !== 'FireWall');
      if (beforeMove) steps.push(Object.freeze({ kind: 'spell', spell: beforeMove }));
      steps.push(Object.freeze({ kind: 'reposition' }));
      for (const spell of spellSteps) if (spell !== beforeMove) steps.push(Object.freeze({ kind: 'spell', spell }));
    } else {
      for (const spell of spellSteps) steps.push(Object.freeze({ kind: 'spell', spell }));
    }
  } else if (className === 'Taoist') {
    if (includes('Healing cast accepted on self')) steps.push(Object.freeze({ kind: 'healing' }));
    if (includes('SpiritSword')) steps.push(Object.freeze({ kind: 'normal' }));
    // Poison occupies the Amulet slot. Cast it first, then restore Amulet
    // before SoulFireBall or SummonSkeleton in the same bounded practice pass.
    if (includes('poison effect')) steps.push(Object.freeze({ kind: 'poison' }));
    // N18 requires that the spell is learned and the legal loadout prepared,
    // but has no configured monster or owned-pet damage objective. Only an
    // explicit owned-skeleton receipt may start the combat practice sequence.
    if (includes('owned skeleton')) steps.push(Object.freeze({ kind: 'summon' }));
    if (includes('SoulFireBall damage')) steps.push(Object.freeze({ kind: 'spell', spell: 'SoulFireBall' }));
  }
  return Object.freeze(steps);
}

export function requiredV2Skills(quest, className) {
  const knownNames = ['Fencing', 'Slaying', 'HalfMoon', 'Thrusting', 'FireBall', 'GreatFireBall', 'Lightning', 'FireWall', 'Healing', 'SpiritSword', 'SoulFireBall', 'SummonSkeleton', 'Poisoning'];
  const conditions = classConditions(quest, className);
  return Object.freeze(knownNames.filter(name => conditions.some(condition =>
    referencesSkill(condition, name) ||
    (name === 'SummonSkeleton' && /owned skeleton/i.test(condition)) ||
    (name === 'Poisoning' && /poison effect/i.test(condition))
  )));
}

function referencesSkill(condition, spell) {
  const text = String(condition ?? '').toLowerCase();
  const wanted = String(spell).toLowerCase();
  if (!text.includes(wanted)) return false;
  // Camel-cased Crystal skill names contain their shorter counterpart. A
  // SoulFireBall condition must never make the runner wait for FireBall.
  if (wanted === 'fireball' && /(?:great|soul)fireball/i.test(text)) return false;
  return true;
}

function classConditions(quest, className) {
  return quest.objectives.flag.flatMap(flag => [
    ...(flag.requirements?.[className.toLowerCase()] ?? []),
    ...(flag.conditions ?? []),
  ]).map(String);
}

async function learnV2Skills(client, spells) {
  for (const spell of spells) {
    if ((client.snapshot?.knownSkills ?? []).some(skill => String(skill?.spell) === spell)) continue;
    const book = (client.snapshot?.inventoryItems ?? []).find(item =>
      normalize(item?.name) === normalize(spell) && validUniqueId(item?.uniqueId) && itemIsSkillBook(item));
    if (!book) throw new V2Pause('awaitingSkillBook', null, `${spell} is required but no authoritative usable book is held`);
    const ack = await client.request({ type: 'useItem', grid: 'inventory', uniqueId: book.uniqueId }, 'UseItem', 12_000);
    if (ack?.payload?.success !== true) throw new V2Pause('skillBookRejected', null, `${spell} book use was rejected`);
    await client.wait(() => (client.snapshot?.knownSkills ?? []).some(skill => String(skill?.spell) === spell), `learned ${spell}`, 12_000);
  }
}

export async function purchaseRequiredBasicPotion(client, navigate, quest, checkDeadline) {
  const before = basicPotionCount(client.snapshot);
  checkDeadline();
  await purchaseV2BasicHpPotion(client, navigate);
  if (basicPotionCount(client.snapshot) !== before + 1) {
    throw new V2Pause('potionPurchaseUnconfirmed', quest.questId, 'ordinary Basic Potion purchase lacked an authoritative receipt');
  }
}

export async function approachPracticeTarget(client, quest, navigate, desiredDistance, checkDeadline = () => {}, options = {}) {
  const selectTarget = () => matchingPracticeTarget(client.snapshot, quest, options.excludeObjectIds) ??
    matchingPracticeTarget(client.snapshot, quest);
  let target = selectTarget();
  if (!target) {
    const waypoints = practiceSpawnWaypoints(client.snapshot, quest);
    let remainingSteps = PRACTICE_SPAWN_STEP_BUDGET;
    let remainingAttempts = PRACTICE_SPAWN_ATTEMPT_BUDGET;
    for (let index = 0; index < waypoints.length; index += 1) {
      const waypoint = waypoints[index];
      if (target || remainingSteps <= 0 || remainingAttempts <= 0) break;
      checkDeadline();
      // A target can enter AOI while walking a legitimate spawn field. Stop
      // immediately and retarget it rather than spending the remaining field
      // budget marching toward a stale manifest center.
      const waypointAttempts = practiceWaypointAttemptReservation(
        selfPlayer(client), waypoint, remainingAttempts, waypoints.length - index,
      );
      let progress;
      try {
        progress = await navigate(waypoint, 6, () => Boolean(selectTarget()), {
          maxSuccessfulSteps: remainingSteps,
          maxAttempts: waypointAttempts,
          detectPositionCycles: true,
        });
      } catch (error) {
        // A manifest spread may cover an unreachable collision component.
        // Skip only its exact no-path result or this waypoint's exact, fully
        // accounted attempt cap; every other navigation failure stays terminal.
        // The fixed candidate list remains bounded by its existing budgets.
        const noWalkPath = /^No walk path/.test(String(error?.message ?? ''));
        const attemptBudgetExhausted = new RegExp(`^Navigation attempt budget exceeded \\(${waypointAttempts}\\)$`)
          .test(String(error?.message ?? ''));
        if (!noWalkPath && !attemptBudgetExhausted) throw error;
        const settled = settlePracticeNavigationBudget(error, remainingSteps, remainingAttempts, waypointAttempts);
        if (!settled) throw error;
        remainingSteps = settled.remainingSteps;
        remainingAttempts = settled.remainingAttempts;
        // The final accepted move can reveal an authoritative target at the
        // same moment its per-waypoint attempt cap is exhausted. Preserve
        // that receipt before the shared ledger decides whether another field
        // waypoint may start.
        target = selectTarget();
        if (typeof client?.record === 'function') {
          client.record('diagnostic', {
            type: noWalkPath ? 'practiceSpawnWaypointUnreachable' : 'practiceSpawnWaypointAttemptBudgetExhausted',
            questId: Number(quest.questId),
            mapFileName: String(client.snapshot?.mapFileName ?? ''),
            waypoint: { x: Number(waypoint.x), y: Number(waypoint.y) },
          });
        }
        continue;
      }
      const settled = settlePracticeNavigationBudget(progress, remainingSteps, remainingAttempts, waypointAttempts);
      if (!settled) throw new Error('practice navigation lacked trustworthy budget accounting');
      remainingSteps = settled.remainingSteps;
      remainingAttempts = settled.remainingAttempts;
      target = selectTarget();
    }
  }
  if (!target) throw new V2Pause('awaitingObjectiveTarget', quest.questId, `q${quest.questId} has no live configured objective monster`);
  checkDeadline();
  if (options.directionalRay) {
    await alignDirectionalPracticeTarget(client, quest, target, navigate, options.directionalRay, checkDeadline);
  } else {
    await navigate(target, desiredDistance, () => false, { maxSuccessfulSteps: 120, maxAttempts: 180, detectPositionCycles: true });
  }
  checkDeadline();
  const refreshed = (client.snapshot?.entities ?? []).find(entity => Number(entity?.objectId) === Number(target.objectId));
  const directionalRange = options.directionalRay && directionalRayDistance(selfPlayer(client), refreshed);
  const directionalInRange = !options.directionalRay || (directionalRange != null &&
    directionalRange >= Number(options.directionalRay.minRange) && directionalRange <= Number(options.directionalRay.maxRange));
  if (!refreshed || refreshed.dead === true || Number(refreshed.hp ?? 1) <= 0 ||
      distance(selfPlayer(client), refreshed) > desiredDistance || !directionalInRange) {
    throw new V2Pause('targetOutOfRange', quest.questId, 'configured practice target lost its authoritative in-range receipt');
  }
  return refreshed;
}

function practiceWaypointAttemptReservation(actor, waypoint, remainingAttempts, remainingWaypoints) {
  const fairShare = Math.max(1, Math.floor(remainingAttempts / Math.max(1, remainingWaypoints)));
  const directDistance = distance(actor, waypoint);
  // Retain the 180-attempt total, but do not strand the closest real field
  // behind an arbitrary equal sixth of it.  The modest route allowance covers
  // a static detour while the later candidates retain the remainder.
  const travelReserve = Number.isFinite(directDistance)
    ? Math.ceil(directDistance * 1.25)
    : fairShare;
  return Math.max(1, Math.min(remainingAttempts, Math.max(fairShare, travelReserve)));
}

function settlePracticeNavigationBudget(progress, remainingSteps, remainingAttempts, allocatedAttempts) {
  const successfulSteps = progress?.successfulSteps;
  const attempts = progress?.attempts;
  if (!Number.isSafeInteger(successfulSteps) || successfulSteps < 0 || successfulSteps > remainingSteps ||
      !Number.isSafeInteger(attempts) || attempts < 0 || attempts > allocatedAttempts || attempts > remainingAttempts) {
    return null;
  }
  return {
    remainingSteps: remainingSteps - successfulSteps,
    remainingAttempts: remainingAttempts - attempts,
  };
}

/**
 * Crystal directional actions hit exact tiles, not an arbitrary Chebyshev
 * radius. Keep the existing 120/180 final-approach envelope, but share it
 * across at most three manifest-free alignment points.
 */
async function alignDirectionalPracticeTarget(client, quest, target, navigate, ray, checkDeadline) {
  const points = directionalApproachPoints(selfPlayer(client), target, ray).slice(0, 3);
  if (!points.length) throw new V2Pause('targetOutOfRange', quest.questId, 'configured practice target has no legal directional ray');
  let remainingSteps = 120;
  let remainingAttempts = 180;
  let lastNavigationError = null;
  for (let index = 0; index < points.length && remainingSteps > 0 && remainingAttempts > 0; index += 1) {
    checkDeadline();
    const slots = points.length - index;
    // Reserve each remaining candidate's share before navigation. A rejected
    // first point must not grant its unused allowance to every fallback and
    // quietly turn the existing 120/180 cap into an expanded search.
    const stepAllowance = Math.max(1, Math.floor(remainingSteps / slots));
    const attemptAllowance = Math.max(1, Math.floor(remainingAttempts / slots));
    try {
      await navigate(points[index], 0, () => false, {
        maxSuccessfulSteps: stepAllowance,
        maxAttempts: attemptAllowance,
        detectPositionCycles: true,
      });
    } catch (error) {
      // A collision-blocked ray endpoint is ordinary movement failure, not
      // proof that the live target is invalid. Spend only its reserved share
      // and let the next bounded alignment endpoint try once.
      lastNavigationError = error;
    }
    remainingSteps -= stepAllowance;
    remainingAttempts -= attemptAllowance;
    const current = (client.snapshot?.entities ?? []).find(entity => Number(entity?.objectId) === Number(target.objectId));
    const rayDistance = directionalRayDistance(selfPlayer(client), current);
    if (current?.dead !== true && Number(current?.hp ?? 1) > 0 && rayDistance != null &&
      rayDistance >= Number(ray.minRange) && rayDistance <= Number(ray.maxRange)) return;
  }
  if (lastNavigationError) throw lastNavigationError;
}

/** Return the exact line distance for Crystal's eight-direction rays. */
export function directionalRayDistance(from, to) {
  const dx = Math.abs(Number(to?.x) - Number(from?.x));
  const dy = Math.abs(Number(to?.y) - Number(from?.y));
  if (!Number.isFinite(dx) || !Number.isFinite(dy) || (dx === 0 && dy === 0)) return null;
  return dx === 0 || dy === 0 || dx === dy ? Math.max(dx, dy) : null;
}

/**
 * Candidate standing tiles that place a known live target on an exact ray.
 * If the actor is already aligned, retain that verified location. Otherwise
 * Lightning ranks longer (4–6) lanes before close lanes, avoiding an
 * unnecessary adjacent cast while keeping the route search bounded.
 */
export function directionalApproachPoints(actor, target, { minRange = 1, maxRange, preferDistant = false } = {}) {
  const minimum = Math.max(1, Number(minRange));
  const maximum = Math.max(minimum, Number(maxRange));
  if (!validPracticePoint(actor) || !validPracticePoint(target) || !Number.isFinite(maximum)) return Object.freeze([]);
  const currentRange = directionalRayDistance(actor, target);
  if (currentRange != null && currentRange >= minimum && currentRange <= maximum &&
      (!preferDistant || currentRange >= 4)) {
    return Object.freeze([Object.freeze({ x: Number(actor.x), y: Number(actor.y), range: currentRange })]);
  }
  const directions = [
    [-1, -1], [0, -1], [1, -1], [-1, 0], [1, 0], [-1, 1], [0, 1], [1, 1],
  ];
  const ranges = Array.from({ length: maximum - minimum + 1 }, (_, index) => minimum + index);
  const candidates = directions.flatMap(([dx, dy]) => ranges.map(range => ({
    x: Number(target.x) - dx * range,
    y: Number(target.y) - dy * range,
    range,
  })));
  return Object.freeze(candidates.sort((left, right) => {
    const leftSafety = preferDistant && left.range >= 4 ? 0 : 1;
    const rightSafety = preferDistant && right.range >= 4 ? 0 : 1;
    return leftSafety - rightSafety || distance(actor, left) - distance(actor, right) ||
      (preferDistant ? right.range - left.range : left.range - right.range) || left.y - right.y || left.x - right.x;
  }).map(point => Object.freeze(point)));
}

/**
 * Rank only same-map, manifest-backed combat fields by distance to their
 * footprint. This mirrors normal objective selection without teaching the V2
 * practice driver to invent a target or cross maps.
 */
export function practiceSpawnCandidates(snapshot, quest) {
  const actor = selfPlayer({ snapshot });
  const mapFileName = String(snapshot?.mapFileName ?? '');
  return Object.freeze((quest?.objectives?.kill ?? []).flatMap(kill =>
    kill?.spawnCandidates ?? kill?.spawns ?? []).filter(candidate =>
    String(candidate?.mapFileName ?? '') === mapFileName && validPracticePoint(candidate?.position)
  ).sort((left, right) =>
    practiceFootprintDistance(actor, left) - practiceFootprintDistance(actor, right) ||
    Number(right?.count ?? 0) - Number(left?.count ?? 0) ||
    Number(left?.delayMinutes ?? 0) - Number(right?.delayMinutes ?? 0) ||
    Number(left?.respawnIndex ?? 0) - Number(right?.respawnIndex ?? 0)
  ).slice(0, PRACTICE_SPAWN_CANDIDATE_LIMIT));
}

/** Bounded, spread-aware field coverage for a V2 class-practice target. */
export function practiceSpawnWaypoints(snapshot, quest) {
  const actor = selfPlayer({ snapshot });
  const candidates = practiceSpawnCandidates(snapshot, quest);
  const points = candidates.flatMap((candidate, candidateIndex) => practiceFieldCoverage(candidate)
    .map(point => ({ ...point, candidateIndex })));
  return Object.freeze(points.sort((left, right) =>
    distance(actor, left) - distance(actor, right) ||
    left.candidateIndex - right.candidateIndex || left.y - right.y || left.x - right.x
  ).slice(0, PRACTICE_SPAWN_WAYPOINT_LIMIT).map(({ x, y }) => Object.freeze({ x, y })));
}

function practiceFootprintDistance(actor, candidate) {
  return Math.max(0, distance(actor, candidate?.position) - Math.max(0, Number(candidate?.spread ?? 0)));
}

function practiceFieldCoverage(candidate) {
  const center = candidate?.position;
  const spread = Math.max(0, Math.ceil(Number(candidate?.spread ?? 0)));
  if (!validPracticePoint(center)) return [];
  // Crystal respawn spreads are fields, not a six-tile disk around the
  // manifest center. Sample the center and its bounded 20-tile grid so a
  // field visible from an edge is eligible before a remote center is reached.
  const axis = value => {
    const low = Math.ceil(Number(value) - spread);
    const high = Math.floor(Number(value) + spread);
    const values = [];
    for (let current = low; current <= high; current += 20) values.push(current);
    if (!values.includes(Number(value))) values.push(Number(value));
    if (!values.includes(high)) values.push(high);
    return values;
  };
  return axis(center.x).flatMap(x => axis(center.y).map(y => ({ x, y })));
}

function validPracticePoint(point) {
  return Number.isFinite(Number(point?.x)) && Number.isFinite(Number(point?.y));
}

function matchingPracticeTarget(snapshot, quest, excludedObjectIds = null) {
  const names = new Set((quest.objectives.kill ?? []).map(kill => normalize(kill.monsterName)));
  const actor = selfPlayer({ snapshot });
  const excluded = excludedObjectIds instanceof Set ? excludedObjectIds : new Set();
  return (snapshot?.entities ?? []).filter(entity => entity?.kind === 'monster' && entity.dead !== true && Number(entity?.hp ?? 1) > 0 &&
    !excluded.has(Number(entity?.objectId)) && names.has(normalize(entity?.name)) &&
    String(entity?.mapFileName ?? snapshot?.mapFileName ?? '') === String(snapshot?.mapFileName ?? ''))
    .sort((left, right) => distance(actor, left) - distance(actor, right))[0] ?? null;
}

async function waitForV2SpellReady(client, spell, checkDeadline, questId) {
  const until = Date.now() + 20_000;
  for (;;) {
    checkDeadline();
    const skill = (client.snapshot?.knownSkills ?? []).find(entry => String(entry?.spell) === spell);
    if (!skill) throw new V2Pause('awaitingLearnedSkill', questId, `${spell} is not authoritative in knownSkills`);
    if (Number(skill.cooldownRemainingTicks ?? 0) <= 0) return;
    const remaining = until - Date.now();
    if (remaining <= 0) throw new V2Pause('awaitingSkillCooldown', questId, `${spell} stayed on authoritative cooldown`);
    // A prior cast can leave the shared Zone action clock locked even when
    // this spell itself has not been used. A fresh public snapshot, not a
    // local timer guess, must release the next practice cast.
    await new Promise(resolve => setTimeout(resolve, Math.min(200, remaining)));
    checkDeadline();
    await refreshCombatWorldSnapshot(client, { timeoutMs: Math.max(1, until - Date.now()) });
  }
}

async function castV2Spell(client, target, spell, checkDeadline = () => {}, questId = null) {
  await waitForV2SpellReady(client, spell, checkDeadline, questId);
  const actor = selfPlayer(client);
  if (!actor) throw new Error('V2 combat actor is absent');
  if (!target || !Number.isSafeInteger(Number(target.objectId))) throw new Error(`V2 ${spell} needs an authoritative target`);
  const currentTarget = Number(target.objectId) === Number(actor.objectId)
    ? actor
    : (client.snapshot?.entities ?? []).find(entity => Number(entity?.objectId) === Number(target.objectId));
  if (!currentTarget || currentTarget.dead === true || Number(currentTarget.hp ?? 1) <= 0) {
    throw new V2Pause('practiceTargetLost', questId, `${spell} target is no longer live after cooldown`);
  }
  const after = Number(client.sequence ?? 0);
  const beforeHp = Number(currentTarget.hp);
  const poisonBefore = equippedPoisonQuantity(client.snapshot);
  const lightning = spell === 'Lightning';
  const ground = spell === 'FireWall';
  const command = lightning
    ? {
        // Lightning is a self-routed Crystal cast. Its targeting coordinates
        // are the caster's authoritative location, never a stale remote AOI
        // target, and its lock remains false.
        type: 'magic', objectId: Number(actor.objectId), spell,
        // Crystal routes Lightning through the caster, but its direction must
        // still be derived from the current authoritative target rather than
        // reusing the actor's stale facing direction.
        direction: direction(actor, currentTarget),
        targetId: Number(actor.objectId), x: Number(actor.x), y: Number(actor.y), spellTargetLock: false,
      }
    : ground
      ? {
          // The public gateway keeps the actor in objectId, while targetId=0
          // selects the Zone's ground-spell route. The live monster tile is
          // is used for this practice; the target must later receive positive
          // damage before this step can pass.
          type: 'magic', objectId: Number(actor.objectId), spell,
          direction: direction(actor, currentTarget), targetId: 0,
          x: Number(currentTarget.x), y: Number(currentTarget.y), spellTargetLock: false,
        }
    : {
        type: 'magic', objectId: Number(actor.objectId), spell,
        direction: direction(actor, currentTarget), targetId: Number(currentTarget.objectId), x: Number(currentTarget.x), y: Number(currentTarget.y), spellTargetLock: true,
      };
  client.send(command);
  return { kind: 'magic', spell, targetId: Number(currentTarget.objectId), command, after, target: currentTarget, beforeHp, poisonBefore };
}

async function attackDirectionTechnique(client, target, spell) {
  const actor = selfPlayer(client);
  const spellId = TECHNIQUE_SPELL[spell];
  if (!actor || !Number.isSafeInteger(spellId)) throw new Error(`unsupported warrior technique ${spell}`);
  client.send({ type: 'attackDirection', direction: direction(actor, target), spell: spellId });
}

async function armWeaponTechnique(client, questId, spell) {
  const actor = selfPlayer(client);
  if (!actor) throw new V2Pause('awaitingTechniqueArming', questId, `${spell} has no authoritative player`);
  if (!Number.isSafeInteger(TECHNIQUE_SPELL[spell])) throw new Error(`unsupported warrior technique ${spell}`);
  const after = client.sequence;
  client.send({ type: 'spellToggle', spell, canUse: true });
  await client.wait(() => receivedAfter(client, after, 'SpellToggle', payload =>
    Number(payload?.objectId) === Number(actor.objectId) &&
    String(payload?.spell) === spell && payload?.canUse === true,
  ), `q${questId} ${spell} armed`, 12_000).catch(() => {
    throw new V2Pause('awaitingTechniqueArming', questId, `${spell} lacked an authoritative arming receipt`);
  });
}

async function equipHeldPoison(client, questId) {
  const heldPoison = item => /(?:green|red)poison/i.test(String(item?.name)) &&
    validUniqueId(item?.uniqueId) && Number(item?.quantity ?? 1) > 0;
  if ((client.snapshot?.equipmentItems ?? []).some(heldPoison)) return;
  let poison = (client.snapshot?.inventoryItems ?? []).find(heldPoison);
  if (!poison) {
    const beltPoison = (client.snapshot?.beltItems ?? []).find(heldPoison);
    if (beltPoison) poison = await moveHeldBeltItemToInventory(client, beltPoison, 'Poison');
  }
  if (!poison) throw new V2Pause('awaitingPoisonMaterial', questId, 'Poisoning requires an authoritative poison stack in the bag');
  const ack = await client.request({ type: 'equipItem', grid: 'inventory', uniqueId: poison.uniqueId, to: 9 }, 'EquipItem', 12_000);
  if (ack?.payload?.success !== true) throw new V2Pause('poisonEquipRejected', questId, 'Poison material equip was rejected');
  await client.wait(() => (client.snapshot?.equipmentItems ?? []).some(item => Number(item?.uniqueId) === Number(poison.uniqueId) && /poison/i.test(String(item?.name))), `q${questId} poison equipped`, 12_000);
}

function ownedBoneFamiliar(snapshot) {
  const owner = String(selfPlayer({ snapshot })?.name ?? '').trim();
  if (!owner) return null;
  return (snapshot?.entities ?? []).find(entity =>
    entity?.kind === 'monster' && entity?.dead !== true && String(entity?.name) === 'BoneFamiliar' &&
    String(entity?.ownerName ?? '').trim().toLowerCase() === owner.toLowerCase()) ?? null;
}

function practiceFingerprint(snapshot, quest) {
  return JSON.stringify((quest.objectives.flag ?? []).map(flag => ({ number: flag.number, state: v2FlagState(snapshot, quest.questId, flag) })));
}

/** A map-entry receipt is transport evidence, independent of a 0/1 AND flag. */
export function hasFreshV2MapEntryReceipt(client, afterSequence, objectiveMaps) {
  const wantedMaps = new Set((objectiveMaps ?? []).map(String));
  const snapshot = client?.snapshot;
  if (!wantedMaps.has(String(snapshot?.mapFileName ?? ''))) return false;
  const actor = selfPlayer(client);
  if (!Number.isFinite(Number(actor?.x)) || !Number.isFinite(Number(actor?.y))) return false;
  return (client?.events ?? []).some(event => {
    if (Number(event?.sequence) <= Number(afterSequence) || event?.direction !== 'received') return false;
    if (!['MapChanged', 'MapInformation', 'MapInfo'].includes(String(event?.packet ?? ''))) return false;
    const mapFileName = String(event?.payload?.fileName ?? event?.payload?.mapFileName ?? '');
    return wantedMaps.has(mapFileName);
  });
}

async function waitForMapEntryReceipt(client, quest, afterSequence) {
  await client.wait(() => hasFreshV2MapEntryReceipt(client, afterSequence, quest.objectiveMaps),
    `q${quest.questId} authoritative map entry`, 12_000).catch(() => {
      throw new V2Pause('awaitingMapEntryReceipt', quest.questId,
        `q${quest.questId} objective-map transfer lacked a fresh map and landing-position receipt`);
    });
}

async function waitForPracticeReceipt(client, quest, before, step) {
  await client.wait(() => practiceFingerprint(client.snapshot, quest) !== before || flagsComplete(client.snapshot, quest), `q${quest.questId} ${step.kind} receipt`, 12_000)
    .catch(() => { throw new V2Pause('awaitingServerFlag', quest.questId, `${step.kind} lacked an authoritative practice receipt`); });
}

function legalReposition(actor, target) {
  const dx = Number(actor?.x) <= Number(target?.x) ? -2 : 2;
  const dy = Number(actor?.y) <= Number(target?.y) ? -1 : 1;
  return { x: Number(actor?.x) + dx, y: Number(actor?.y) + dy };
}

function basicPotionCount(snapshot) {
  return ['inventoryItems', 'beltItems', 'equipmentItems'].reduce((total, key) => total + (snapshot?.[key] ?? [])
    .filter(item => itemTemplateIndex(item) === BASIC_HP_ITEM_INDEX)
    .reduce((sum, item) => sum + Math.max(0, Number(item?.quantity ?? 1)), 0), 0);
}

export const countBasicHpPotion = basicPotionCount;

function itemTemplateIndex(item) {
  const source = item?.tooltipSource?.realInfo ?? item?.tooltipSource?.real_info ?? item?.tooltipSource?.info ?? {};
  const value = item?.itemIndex ?? item?.item_index ?? source?.itemIndex ?? source?.item_index;
  return Number.isSafeInteger(Number(value)) ? Number(value) : null;
}

function equippedPoisonQuantity(snapshot) {
  return (snapshot?.equipmentItems ?? []).filter(item => /(?:green|red)poison/i.test(String(item?.name)))
    .reduce((total, item) => total + Math.max(0, Number(item?.quantity ?? 1)), 0);
}

function receivedAfter(client, sequence, packet, predicate = () => true) {
  return (client?.events ?? []).some(event => Number(event?.sequence) > Number(sequence) &&
    event?.direction === 'received' && event?.packet === packet && predicate(event?.payload ?? {}));
}

function targetTookDamage(client, target, beforeHp, after, attackerId = null) {
  const current = (client.snapshot?.entities ?? []).find(entity => Number(entity?.objectId) === Number(target?.objectId));
  const hpDeclined = Number.isFinite(beforeHp) && Number(current?.hp) < beforeHp;
  const struck = receivedAfter(client, after, 'ObjectStruck', payload =>
    Number(payload?.objectId) === Number(target?.objectId) && (attackerId == null || Number(payload?.attackerId) === Number(attackerId)));
  return hpDeclined || struck;
}

function magicAccepted(client, after, spell, actor, expectedTargetId = null) {
  return receivedAfter(client, after, 'ObjectMagic', payload => Number(payload?.objectId) === Number(actor?.objectId) &&
    String(payload?.spell) === spell && payload?.cast === true &&
    (expectedTargetId == null || Number(payload?.targetId) === Number(expectedTargetId))) ||
    receivedAfter(client, after, 'Magic', payload => String(payload?.spell) === spell && payload?.cast === true &&
      (expectedTargetId == null || Number(payload?.targetId) === Number(expectedTargetId)));
}

async function waitForTargetDamage(client, after, target, beforeHp, questId, label, spellId = null, timeoutMs = 12_000) {
  const actor = selfPlayer(client);
  await client.wait(() => {
    const attack = spellId == null || receivedAfter(client, after, 'ObjectAttack', payload =>
      Number(payload?.objectId) === Number(actor?.objectId) && Number(payload?.spell) === spellId);
    return attack && targetTookDamage(client, target, beforeHp, after, actor?.objectId);
  }, `q${questId} ${label} damage`, timeoutMs).catch(() => {
    throw new V2Pause('awaitingPracticeDamage', questId, `${label} lacked a positive post-send target damage receipt`);
  });
}

async function waitForSpellDamage(client, after, target, beforeHp, spell, questId) {
  const actor = selfPlayer(client);
  const expectedTargetId = spell === 'Lightning'
    ? actor?.objectId
    : spell === 'FireWall'
      ? 0
      : target?.objectId;
  await client.wait(() => magicAccepted(client, after, spell, actor, expectedTargetId) &&
    targetTookDamage(client, target, beforeHp, after, actor?.objectId), `q${questId} ${spell} damage`, 12_000).catch(() => {
    throw new V2Pause('awaitingPracticeDamage', questId, `${spell} lacked a positive post-send target damage receipt`);
  });
}

async function waitForSelfMagic(client, after, spell, questId) {
  const actor = selfPlayer(client);
  await client.wait(() => magicAccepted(client, after, spell, actor, actor?.objectId), `q${questId} ${spell} self magic`, 12_000).catch(() => {
    throw new V2Pause('awaitingSelfMagic', questId, `${spell} lacked an accepted self-magic receipt`);
  });
}

async function waitForPoisonEvidence(client, after, target, beforeQuantity, questId) {
  const actor = selfPlayer(client);
  await client.wait(() => magicAccepted(client, after, 'Poisoning', actor, target?.objectId) &&
    equippedPoisonQuantity(client.snapshot) < beforeQuantity &&
    receivedAfter(client, after, 'ObjectPoisoned', payload => Number(payload?.objectId) === Number(target?.objectId)),
  `q${questId} Poisoning material effect`, 12_000).catch(() => {
    throw new V2Pause('awaitingPoisonEffect', questId, 'Poisoning lacked a material-consumption and target-effect receipt');
  });
}

async function waitForPetDamage(client, after, pet, target, questId) {
  const hp = Number(target?.hp);
  await client.wait(() => targetTookDamage(client, target, hp, after, pet?.objectId), `q${questId} BoneFamiliar damage`, 12_000).catch(() => {
    throw new V2Pause('awaitingOwnedPetDamage', questId, 'BoneFamiliar lacked a positive owned-pet damage receipt');
  });
}

function itemIsSkillBook(item) {
  const source = item?.tooltipSource?.realInfo ?? item?.tooltipSource?.real_info ?? item?.tooltipSource?.info ?? {};
  return Number(item?.itemType ?? item?.item_type ?? source?.itemType ?? source?.item_type) === 20;
}

function validUniqueId(value) {
  return Number.isSafeInteger(Number(value)) && Number(value) >= 0;
}

function direction(from, to) {
  const dx = Math.sign(Number(to.x) - Number(from.x));
  const dy = Math.sign(Number(to.y) - Number(from.y));
  return new Map([['0,-1', 'Up'], ['1,-1', 'UpRight'], ['1,0', 'Right'], ['1,1', 'DownRight'], ['0,1', 'Down'], ['-1,1', 'DownLeft'], ['-1,0', 'Left'], ['-1,-1', 'UpLeft']]).get(`${dx},${dy}`) ?? 'Down';
}

function distance(left, right) {
  return Math.max(Math.abs(Number(left?.x) - Number(right?.x)), Math.abs(Number(left?.y) - Number(right?.y)));
}

async function waitForServerStage(client, questId, stages, label) {
  return client.wait(() => stages.includes(stage(serverQuest(client.snapshot, questId)?.stage)), label, 20_000)
    .catch(() => { throw new V2Pause('awaitingServerStage', questId, `${label} lacked a server stage receipt`); });
}

function integer(value, label) {
  const number = Number(value);
  if (!Number.isSafeInteger(number) || number <= 0) throw new Error(`invalid ${label}`);
  return number;
}
function requiredString(value, label) {
  const text = String(value ?? '').trim();
  if (!text) throw new Error(`missing ${label}`);
  return text;
}
