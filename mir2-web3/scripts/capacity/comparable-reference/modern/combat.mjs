// Ordinary directional melee extension. Dense v2 and the server are unchanged.
import { performance } from 'node:perf_hooks';
import { MAP_SHA, hash, canonical, requireThat as check, owner, percentile, RetainedReceiveCursor, equipmentFingerprint, snapshotProjection, savedState } from './contract.mjs';
import { ModernClient, observedSharedId, offerAction, observeActions, carriedState } from './driver.mjs';

export const COMBAT_CONTRACT_SHA256 = '8b373b5d03a71c468934d3591c6d06ad65ce6fe2eeb3208acca39ee0ae25a25b';
export const POTION_POLL_MS = 25;
const DIRECTIONS = ['Up', 'UpRight', 'Right', 'DownRight', 'Down', 'DownLeft', 'Left', 'UpLeft'];
const DELTAS = [[0, -1], [1, -1], [1, 0], [1, 1], [0, 1], [-1, 1], [-1, 0], [-1, -1]];
const same = (a, b) => JSON.stringify(canonical(a)) === JSON.stringify(canonical(b));
const pos = p => p?.location ?? p;
const victimId = p => Number(p?.objectId ?? p?.object_id);
const attackerId = p => Number(p?.attackerId ?? p?.attacker_id);
const isPotion = item => item?.key === 'crystal-item-662' || Number(item?.tooltipSource?.userItem?.itemIndex) === 662;
const stock = snapshot => (snapshot?.inventoryItems ?? []).filter(isPotion).map(i => ({ uniqueId: Number(i.uniqueId),
  grid: 'Inventory', slot: i.slot, count: Number(i.quantity) })).sort((a, b) => a.uniqueId - b.uniqueId);
const total = rows => rows.reduce((n, i) => n + i.count, 0);

export function combatReceipt(client, event) {
  return { observerOrdinal: client.ordinal, connection: client.connection, receiveSeq: event.receiveSeq,
    atMs: event.atMs, receiveAtUtc: event.atUtc, packet: event.packet ?? event.type,
    payload: event.type === 'worldSnapshot' ? { ...snapshotProjection(event.payload), playerMaxHp: event.payload.playerMaxHp,
      potionStock: stock(event.payload) } : event.payload ? structuredClone(event.payload) : null };
}

export function validateCombatPlan(plan, externalContractBytes) {
  check(Buffer.isBuffer(externalContractBytes) && hash(externalContractBytes) === COMBAT_CONTRACT_SHA256,
    'Pinned external combat-contract bytes required');
  const contract = JSON.parse(externalContractBytes.toString('utf8'));
  check(plan?.combatContractSha256 === COMBAT_CONTRACT_SHA256 && same(plan.combatContract, contract) &&
    plan.combatContractCanonicalSha256 === hash(contract), 'Embedded/external combat contract mismatch');
  check(plan.schema === 'mir2.comparable.plan.v1' && plan.qualificationLayer === 'game-core-local' &&
    plan.scenario === 'dispersed-melee' && plan.capacityClaimAllowed === false, 'Only explicit game-core-local melee supported');
  check(plan.definitionSha256 === '9172f5786dd1fad2bcbd3d0fc173c4070c738322710707128b9466e245a4e6e5' &&
    plan.scheduling?.version === 2 && plan.scheduling.chatPhaseRule === 'same-primary-phase-with-midpoint-offset' &&
    plan.scheduling.primaryIntervalMs === 1200 && plan.scheduling.minimumSameActorOfferGapMs === 600 &&
    plan.scheduling.chatOffsetWithinPrimaryIntervalMs === 600, 'Corrected common v2 combat scheduling required');
  check((plan.purpose === 'functional' && plan.population === 2 && plan.durationMs === 60000) ||
    (plan.purpose === 'capacity' && [25, 50, 100].includes(plan.population) && plan.durationMs === 300000), 'Combat population/duration mismatch');
  check(plan.map?.index === 440 && plan.map.fileName === 'DogYoHyun' && plan.map.width === 800 && plan.map.height === 800 &&
    plan.map.rawSha256 === MAP_SHA && /^[0-9a-f]{64}$/.test(plan.navigationSha256), 'Pinned active Map440/navigation required');
  check(plan.actorTemplate?.class === 'Warrior' && plan.actorTemplate.level === 40 && plan.actorTemplate.gender === 'Male' &&
    /^[0-9a-f]{20}$/.test(plan.planId) && /^[0-9a-f]{64}$/.test(plan.generatorSourceSha256), 'Pinned L40 fixture/generator required');
  check(plan.assessment?.windowMs === 60000 && plan.assessment.perActor === true && plan.assessment.allCompleteWindowsRequired === true &&
    plan.assessment.failedActionCanBecomeCompleted === false && plan.assessment.notSentIsTimeout === false &&
    plan.assessment.notSentIsCompleted === false && plan.assessment.safetyOrEvidenceFailureIsHardFailure === true, 'Per-actor minute assessment drift');
  check(plan.causality?.maximumUnresolvedActionsPerActor === 1 && plan.causality.absoluteDeadlineFromPlannedTime === true &&
    plan.causality.lateReceiptCanRestoreSuccess === false && plan.causality.newSendBeforePreviousEvidenceTerminal === false,
  'Unique pending/absolute deadline contract required');
  const limits = { minimumCompletionRatio: .95, maximumTimeoutRatioExclusive: .01, attackP95Ms: 750, chatP95Ms: 1000,
    scheduleP95Ms: 250, attackDeadlineMs: 2500, chatDeadlineMs: 2500, equipmentAckDeadlineMs: 15000,
    equipmentStateDeadlineMs: 15000, logoutAckDeadlineMs: 5000, transportCloseDeadlineMs: 5000 };
  check(Object.entries(limits).every(([k, v]) => plan.thresholds?.[k] === v), 'Common combat thresholds drift');
  check(plan.resourceContract?.logicalCpus === 4 && plan.resourceContract.memoryCommitBytes === 8589934592 &&
    plan.resourceContract.budgetScope === 'all-declared-server-processes', 'Total shared resource budget required');
  check(same(plan.equipmentActorOrdinals, plan.purpose === 'functional' ? [0, 1] : [0, 1, 2]) && plan.equipmentRoundsPerActor === 2,
    'Common equipment phase required');
  check(Array.isArray(plan.actors) && plan.actors.length === plan.population && Array.isArray(plan.actions), 'Complete actors/actions required');
  const cells = new Set();
  for (const [ordinal, actor] of plan.actors.entries()) {
    check(actor.ordinal === ordinal && Number.isSafeInteger(actor.group), 'Invalid combat actor namespace');
    const group = plan.actors.filter(a => a.group === actor.group), peers = group.filter(a => a.ordinal !== ordinal).map(a => a.ordinal);
    check([2, 3].includes(group.length) && same(actor.peerOrdinals, peers) && peers.includes(actor.attackTargetOrdinal), 'Declared combat peer/target group mismatch');
    check(actor.lane?.length === 2 && actor.lane.every(p => same(p, actor.start)) && Number.isInteger(actor.start?.x) &&
      Number.isInteger(actor.start.y) && actor.start.x > 0 && actor.start.x < 800 && actor.start.y > 0 && actor.start.y < 800,
    'Melee actors must keep their declared stationary cell');
    const key = `${actor.start.x},${actor.start.y}`; check(!cells.has(key), 'Combat actors overlap'); cells.add(key);
    const target = plan.actors[actor.attackTargetOrdinal];
    check(Math.max(Math.abs(target.start.x - actor.start.x), Math.abs(target.start.y - actor.start.y)) === 1,
      'Planned melee target must be adjacent');
    check(plan.actors.filter(a => a.attackTargetOrdinal === ordinal).length === 1, 'Competing planned incoming attackers are ambiguous');
    for (const other of plan.actors.filter(a => a.group !== actor.group)) check(Math.max(Math.abs(actor.start.x - other.start.x),
      Math.abs(actor.start.y - other.start.y)) >= 40, 'Combat groups must remain dispersed');
    const expected = [], phase = Math.floor(ordinal * 1200 / plan.population);
    for (let dueMs = phase; dueMs < plan.durationMs; dueMs += 1200) expected.push({ kind: 'attack', dueMs });
    // 15000 mod 1200 is 600: every actor's chat is at its own attack midpoint.
    for (let dueMs = 15000 + phase; dueMs < plan.durationMs; dueMs += 30000) expected.push({ kind: 'chat', dueMs });
    expected.sort((a, b) => a.dueMs - b.dueMs || a.kind.localeCompare(b.kind));
    const actual = plan.actions.filter(a => a.actorOrdinal === ordinal);
    check(actual.length === expected.length && actual.every((a, i) => a.ticketSequence === i + 1 && a.kind === expected[i].kind &&
      a.dueMs === expected[i].dueMs && a.deadlineMs === a.dueMs + plan.thresholds[`${a.kind}DeadlineMs`] &&
      (a.kind === 'attack' ? a.targetActorOrdinal === actor.attackTargetOrdinal : a.nonce === `m2cmp:${plan.planId}:${ordinal}:${i + 1}` &&
        a.nonceSha256 === hash(a.nonce))), 'Complete fixed combat offers and midpoint chats required');
  }
  check(plan.actions.every((a, i) => Number.isSafeInteger(a.actorOrdinal) && a.actorOrdinal >= 0 && a.actorOrdinal < plan.population &&
    (!i || a.dueMs > plan.actions[i - 1].dueMs || (a.dueMs === plan.actions[i - 1].dueMs && a.actorOrdinal >= plan.actions[i - 1].actorOrdinal))),
  'Globally ordered fixed combat offers required');
  return plan;
}

export class ModernCombatClient extends ModernClient {
  send(command) {
    if (!['attackDirection', 'changeAMode', 'useItem'].includes(command?.type)) return super.send(command);
    const keys = Object.keys(command).sort();
    if (command.type === 'attackDirection') check(same(keys, ['direction', 'spell', 'type']) && DIRECTIONS.includes(command.direction) && command.spell === 0,
      'Only ordinary directional Spell.None melee allowed');
    else if (command.type === 'changeAMode') check(same(keys, ['mode', 'type']) && command.mode === 5, 'Only ordinary all attack mode allowed');
    else check(same(keys, ['grid', 'type', 'uniqueId']) && command.grid === 'inventory' && Number.isSafeInteger(command.uniqueId) && command.uniqueId > 0,
      'Ordinary inventory UID potion reference required');
    check(this.ws?.readyState === 1 && !this.closed, 'Original socket is not open');
    const encoded = JSON.stringify(command), sentAtMs = this.now(), bytes = Buffer.byteLength(encoded);
    this.emit({ event: 'sent', actorOrdinal: this.ordinal, connection: this.connection, atMs: sentAtMs,
      atUtc: new Date().toISOString(), wireBytes: bytes, command: structuredClone(command) });
    this.sentBytes += bytes; this.countSteadyBytes('sent', bytes, sentAtMs); this.ws.send(encoded); return sentAtMs;
  }
  observe(message, wireBytes = 0) {
    const event = super.observe(message, wireBytes), state = this.combat;
    if (!state || state.ended) return event;
    if (event.type === 'worldSnapshot' && owner(this.latestSnapshotEvent.payload)?.name === this.actor.name) {
      state.live = this.snapshot; state.ownedSource = this.latestSnapshotEvent; state.patches = [];
    }
    if (event.packet === 'HealthChanged') {
      const hp = event.payload?.hp, mp = event.payload?.mp;
      check(Number.isSafeInteger(hp) && Number.isSafeInteger(mp) && hp >= 0, 'Invalid ordinary HP operation');
      state.health.push({ ...combatReceipt(this, event), hpBefore: state.hp, hpAfter: hp, mpBefore: state.mp, mpAfter: mp });
      state.hp = hp; state.mp = mp;
      if (state.health.length > 1024) state.health.splice(0, 128);
      if (state.live) { state.live.playerHp = hp; state.live.playerMp = mp; }
      state.patches.push(combatReceipt(this, event));
    }
    if (['DuraChanged', 'DeleteItem'].includes(event.packet)) applyOwnedItemPacket(this, event);
    if (event.packet === 'Death' || (event.packet === 'HealthChanged' && state.hp === 0) ||
      (event.type === 'worldSnapshot' && (event.payload.playerHp <= 0 || event.payload.owner?.dead === true))) {
      failEvidence(this, 'ordinaryOwnerDeath', event);
    }
    if (event.packet === 'LogOutSuccess') {
      const source = { snapshot: combatReceipt(this, state.ownedSource), patches: structuredClone(state.patches),
        logOutSuccess: combatReceipt(this, event), basis: 'same-connection owner state frozen during LogOutSuccess observation' };
      this.logoutOwnedState = { state: savedState(state.live), carried: structuredClone(carriedState(state.live)), source };
      state.ended = true;
      this.emit({ event: 'logoutOwnedState', actorOrdinal: this.ordinal, connection: this.connection, ...structuredClone(this.logoutOwnedState) });
    }
    if (state.patches.length > 2048) failEvidence(this, 'ownedStatePatchEvidenceLimit');
    return event;
  }
}

function applyOwnedItemPacket(client, event) {
  const state = client.combat, live = state.live, uid = Number(event.payload?.uniqueId);
  const grid = ['inventoryItems', 'beltItems', 'equipmentItems'].find(k => live[k]?.some(i => Number(i.uniqueId) === uid));
  if (!grid) { failEvidence(client, 'ordinaryOwnedItemPatchUnknownUid', event); return; }
  // Current full source remains immutable; only the owned live view is patched.
  live[grid] = structuredClone(live[grid]);
  const item = live[grid].find(i => Number(i.uniqueId) === uid), user = item.tooltipSource?.userItem;
  if (event.packet === 'DuraChanged') {
    const dura = event.payload.currentDura;
    if (!Number.isInteger(dura) || dura < 0 || dura > item.durabilityMax) { failEvidence(client, 'invalidOrdinaryDuraPatch', event); return; }
    item.durabilityCurrent = dura;
    if (user) { if ('current_dura' in user) user.current_dura = dura; else user.currentDura = dura; }
  } else {
    const count = event.payload.count;
    if (!Number.isInteger(count) || count <= 0 || count > item.quantity) { failEvidence(client, 'invalidOrdinaryStockPatch', event); return; }
    item.quantity -= count;
    if (user) user.count = item.quantity;
    live[grid] = live[grid].filter(i => Number(i.quantity) > 0);
  }
  state.patches.push(combatReceipt(client, event));
}

export function verifyCombatFixture(client, plan) {
  const source = client.latestSnapshotEvent, snapshot = source?.payload, self = owner(snapshot);
  const ui = client.events.find(e => e.receiveSeq === client.bootstrapProof?.user?.receiveSeq && e.packet === 'UserInformation');
  check(ui && ui.receiveSeq < source?.receiveSeq && ui.payload?.name === client.actor.name && ui.payload.hasHero === false &&
    ui.payload.magicCount === 0 && ui.payload.intelligentCreatureCount === 0 && ui.payload.creatureSummoned === false,
  'Fresh ordinary UserInformation must prove no hero, magic or summoned creature');
  check(self?.name === client.actor.name && self.class === 'Warrior' && self.level === 40 && self.gender === 'Male' &&
    self.x === plan.actors[client.ordinal].start.x && self.y === plan.actors[client.ordinal].start.y && snapshot.inSafeZone === false &&
    snapshot.playerHp === 674 && snapshot.playerMaxHp === 674 && snapshot.playerMp === 151,
  'Fresh full-HP L40 stationary combat owner required');
  check(Array.isArray(snapshot.knownSkills) && snapshot.knownSkills.length === 0 && Array.isArray(snapshot.activeBuffs) && snapshot.activeBuffs.length === 0 &&
    snapshot.heroVitals === null && snapshot.heroInventoryItems?.length === 0 && snapshot.heroEquipmentItems?.length === 0 &&
    snapshot.stage5Systems?.hero === null && snapshot.stage5Systems.intelligentCreatures?.length === 0 &&
    snapshot.stage5Systems.summonedIntelligentCreatureType === 99 &&
    !snapshot.entities.some(e => ['hero', 'pet'].includes(e.kind) || Number(e.masterObjectId ?? 0) > 0),
  'Fresh fixture has skills, buffs, hero or pets; no runtime clearing allowed');
  const expected = { weapon: [221, [[4, 2], [5, 4]]], armour: [317, [[0, 2], [1, 2], [3, 1]]],
    braceletLeft: [482, [[10, 2]]], braceletRight: [482, [[10, 2]]] };
  check(snapshot.equipmentItems?.length === 4 && snapshot.equipmentItems.every(item => {
    const allowed = expected[item.slot], info = item.tooltipSource?.info, user = item.tooltipSource?.userItem;
    return allowed && item.key === `crystal-item-${allowed[0]}` && Number(user?.item_index ?? user?.itemIndex) === allowed[0] &&
      info?.item_index === allowed[0] && info.item_set === 0 && info.unique === 0 &&
      same(info.stats?.map(s => [s.stat, s.value]).sort((a, b) => a[0] - b[0]), allowed[1]) &&
      (user.added_stats ?? user.addedStats)?.length === 0 && (user.awake_values ?? user.awakeValues)?.length === 0 &&
      user.slots?.length === 0 && Number(user.refined_value ?? user.refinedValue) === 0 &&
      Number(user.awake_type ?? user.awakeType) === 0 && Number(user.refine_added ?? user.refineAdded) === 0 &&
      Number(user.gem_count ?? user.gemCount) === 0 &&
      item.addedAttack === 0 && item.addedDefence === 0 && item.addedLuck === 0;
  }), 'Only exact ordinary221/317/482x2 gear without bonus effects is comparable');
  client.emit({ event: 'combatBaseline', actorOrdinal: client.ordinal, connection: client.connection, sourceReceipt: combatReceipt(client, source),
    userInformation: combatReceipt(client, ui), hp: snapshot.playerHp, maxHp: snapshot.playerMaxHp, knownSkills: snapshot.knownSkills,
    activeBuffs: snapshot.activeBuffs, heroVitals: snapshot.heroVitals, hero: snapshot.stage5Systems.hero,
    intelligentCreatures: snapshot.stage5Systems.intelligentCreatures, summonedIntelligentCreatureType: snapshot.stage5Systems.summonedIntelligentCreatureType,
    gear: structuredClone(snapshot.equipmentItems), fingerprint: equipmentFingerprint(snapshot) });
}

export function beginCombatTracking(client) {
  const source = client.latestSnapshotEvent, snapshot = source?.payload, self = owner(snapshot);
  check(self?.name === client.actor.name && snapshot.playerHp > 0 && Number.isSafeInteger(snapshot.playerMaxHp) && snapshot.playerMaxHp > 0,
    'Fresh complete owner vitals required before combat');
  equipmentFingerprint(snapshot);
  client.combat = { hp: snapshot.playerHp, mp: snapshot.playerMp, maxHp: snapshot.playerMaxHp,
    baseline: combatReceipt(client, source), health: [], nextPotionPollAtMs: -Infinity, lastPotionSentAtMs: -Infinity,
    potionSequence: 0, hardFailure: false, potionDisabled: false, active: null, live: client.snapshot, ownedSource: source, patches: [], ended: false };
  return client.combat;
}

function failEvidence(client, reason, event) {
  const state = client.combat; if (!state || state.hardFailure) return;
  state.hardFailure = true;
  client.emit({ event: 'combatEvidenceFailure', actorOrdinal: client.ordinal, connection: client.connection,
    reason, hardEvidence: true, passed: false, atMs: client.now(), receipt: event ? combatReceipt(client, event) : null });
}
function directionBetween(a, b) {
  const dx = b.x - a.x, dy = b.y - a.y, index = DELTAS.findIndex(([x, y]) => x === dx && y === dy);
  check(index >= 0, 'Current authoritative target is not adjacent'); return DIRECTIONS[index];
}
// Each attack/potion context owns incremental views of the existing retained
// receiver histories. Keep these outside serializable action evidence.
const combatReceipts = new WeakMap();
function receiptWindow(context, key, client, floor, predicate, events = client.events) {
  let readers = combatReceipts.get(context);
  if (!readers) { readers = new Map(); combatReceipts.set(context, readers); }
  let reader = readers.get(key);
  if (!reader || reader.client !== client || reader.connection !== client.connection || reader.floor !== floor.receiveSeq) {
    reader = { client, connection: client.connection, floor: floor.receiveSeq, cursor: new RetainedReceiveCursor(floor.receiveSeq, predicate) };
    readers.set(key, reader);
  }
  return reader.cursor.matches(events);
}
function floor(client) { return { observerOrdinal: client.ordinal, actorOrdinal: client.ordinal, connection: client.connection, receiveSeq: client.sequence }; }

export function offerCombatAction(client, clients, plan, action, epoch, now = performance.now()) {
  check(client.combat, 'Combat tracking has not started');
  if (client.combat.hardFailure) {
    client.emit({ event: 'notSent', ...action, actorOrdinal: client.ordinal, deadlineAtMs: epoch + action.deadlineMs,
      dueAtMs: epoch + action.dueMs, atMs: now, reason: 'combatHardEvidenceFailure', passed: false, timedOut: false, hardEvidence: true });
    return null;
  }
  if (action.kind === 'chat') return offerAction(client, clients, plan, action, epoch, now);
  check(action.kind === 'attack', 'Only attack/chat primary offers allowed');
  const peers = plan.actors[client.ordinal].peerOrdinals.map(i => clients[i]);
  const spec = { ...action, actorOrdinal: client.ordinal, deadlineAtMs: epoch + action.deadlineMs, dueAtMs: epoch + action.dueMs,
    offeredAtMs: now, offeredDelayMs: Math.max(0, now - epoch - action.dueMs), expectedPeerOrdinals: peers.map(c => c.ordinal), nonceHash: null };
  if (!client.gate.start(spec, now)) return null;
  let attempted = false;
  try {
    check(now < spec.deadlineAtMs && !client.pending && !client.failure && !client.closed && !client.combat.hardFailure,
      'Combat client unavailable or deadline expired');
    const target = clients[action.targetActorOrdinal], before = owner(client.snapshot), atTarget = owner(target?.snapshot);
    check(target?.combat && !target.closed && !target.combat.hardFailure && target.combat.hp > 0 && before && atTarget,
      'Living exact target owner unavailable');
    check(same({ x: before.x, y: before.y }, plan.actors[client.ordinal].start) && same({ x: atTarget.x, y: atTarget.y }, plan.actors[target.ordinal].start),
      'Authoritative combat cells differ from the fixed plan');
    const direction = directionBetween(before, atTarget), targetSharedId = observedSharedId(client, target.actor);
    const observers = [...new Set([client, ...peers, target])], floors = new Map(observers.map(c => [c.ordinal, floor(c)]));
    const expectedSharedIds = peers.map(c => ({ observerOrdinal: c.ordinal, connection: c.connection, objectId: observedSharedId(c, client.actor) }));
    const targetAttackerId = observedSharedId(target, client.actor), targetLocalId = Number(target.snapshot.playerObjectId);
    const actualTarget = { actorOrdinal: target.ordinal, connection: target.connection, objectId: targetSharedId,
      ownerObjectId: targetLocalId, x: atTarget.x, y: atTarget.y };
    const context = { kind: 'attack', target, peers, floors, before: { x: before.x, y: before.y }, direction, targetAttackerId, targetLocalId,
      targetHealthFloor: target.sequence, expectedSharedIds, actualTarget };
    client.combat.active = context;
    Object.assign(client.gate.pending, { direction, actualTarget, expectedSharedIds, receiveSequenceFloors: [...floors.values()] });
    attempted = true; const sentAtMs = client.send({ type: 'attackDirection', direction, spell: 0 });
    Object.assign(client.gate.pending, { sentAtMs, scheduleDelayMs: Math.max(0, sentAtMs - spec.dueAtMs) });
    client.emit({ event: 'actionSent', ...client.gate.pending });
  } catch (error) {
    if (attempted) { Object.assign(client.gate.pending, { sendFailed: true, sentAtMs: client.now() }); failEvidence(client, 'uncertainOrdinaryAttackSend'); }
    else { client.gate.pending = null; client.combat.active = null;
      client.emit({ event: 'notSent', ...spec, reason: 'preconditionFailed', failure: error.message, passed: false, timedOut: false, hardEvidence: true }); }
  }
  return null;
}

// HP is an operation stream: modern shared PvP sends the health operation
// before ObjectStruck/DamageIndicator. Positive potion/regen operations remain
// visible and must not be folded into the hit's before/after pair.
function targetOutcome(client, context) {
  const { target, targetAttackerId: expectedAttacker, targetLocalId: localId, floors } = context;
  const events = receiptWindow(context, 'targetOutcome', target, floors.get(target.ordinal), e =>
    ['ObjectAttack', 'Struck', 'ObjectStruck', 'DamageIndicator'].includes(e.packet));
  const competing = events.find(e => {
    if (e.packet !== 'ObjectAttack' || victimId(e.payload) === expectedAttacker) return false;
    const delta = DELTAS[DIRECTIONS.indexOf(e.payload.direction)], from = pos(e.payload), at = context.actualTarget;
    return delta && from?.x + delta[0] === at.x && from?.y + delta[1] === at.y;
  });
  if (competing) return { error: 'competingAnimationTargetsTheSameVictim' };
  const struck = events.filter(e => (e.packet === 'Struck' || (e.packet === 'ObjectStruck' && victimId(e.payload) === localId)));
  if (struck.some(e => attackerId(e.payload) !== expectedAttacker)) return { error: 'competingTargetStruckAttacker' };
  const damage = events.filter(e => e.packet === 'DamageIndicator' && victimId(e.payload) === localId);
  if (damage.length > 1) return { error: 'multipleUnpairedTargetDamageIndicators' };
  const d = damage[0]; if (!d) return null;
  const amount = d.payload.damage, type = d.payload.damage_type ?? d.payload.damageType;
  if (!Number.isSafeInteger(amount) || amount < 0 || ![0, 1].includes(type) || (type === 1 && amount !== 0)) return { error: 'invalidModernDamageOutcome' };
  const bound = struck.find(e => e.receiveSeq < d.receiveSeq);
  const deltas = receiptWindow(context, 'targetHealth', target, { receiveSeq: context.targetHealthFloor },
    e => e.hpAfter < e.hpBefore, target.combat.health);
  if (deltas.length > 1) return { error: 'competingTargetHpDamageOperations' };
  if (type === 1) {
    if (deltas.length) return { error: 'missConflictsWithTargetHpDamage' };
    return { kind: 'explicitMiss', damageMagnitude: 0, receipts: [combatReceipt(target, d)] };
  }
  if (!bound) return null;
  if (amount === 0) {
    if (deltas.length) return { error: 'zeroBlockConflictsWithTargetHpDamage' };
    return { kind: 'explicitZeroBlock', damageMagnitude: 0, receipts: [combatReceipt(target, bound), combatReceipt(target, d)] };
  }
  if (!deltas.length) return null;
  const delta = deltas[0];
  if (delta.hpBefore - delta.hpAfter !== amount) return { error: 'targetHpDeltaDoesNotEqualBoundDamage' };
  return { kind: 'hit', damageMagnitude: amount, hpBefore: delta.hpBefore, hpAfter: delta.hpAfter,
    receipts: [combatReceipt(target, bound), combatReceipt(target, d), delta] };
}

export function observeCombatActions(clients, now = performance.now()) {
  observeActions(clients); // Chat retains the frozen nonce/namespace predicate.
  for (const client of clients) {
    const state = client.combat, pending = client.gate.pending, context = state?.active;
    if (!pending || !state) continue;
    if (pending.kind === 'potion') { observePotion(client); }
    else if (pending.kind === 'attack' && context && !state.hardFailure) {
      if ([client, ...context.peers, context.target].some(c => clients[c.ordinal]?.connection !== context.floors.get(c.ordinal)?.connection)) {
        failEvidence(client, 'attackObserverConnectionChanged'); continue;
      }
      const location = receiptWindow(context, 'ownerLocation', client, context.floors.get(client.ordinal), e => e.packet === 'UserLocation')[0];
      if (location && (pos(location.payload).x !== context.before.x || pos(location.payload).y !== context.before.y || location.payload.direction !== context.direction)) {
        client.gate.close([combatReceipt(client, location)], location.atMs, false); state.active = null; continue;
      }
      const animations = context.peers.map(peer => receiptWindow(context, `animation:${peer.ordinal}`, peer, context.floors.get(peer.ordinal), e => e.packet === 'ObjectAttack' &&
        victimId(e.payload) === context.expectedSharedIds.find(s => s.observerOrdinal === peer.ordinal).objectId &&
        pos(e.payload).x === context.before.x && pos(e.payload).y === context.before.y && e.payload.direction === context.direction && e.payload.spell === 0)[0]);
      const outcome = targetOutcome(client, context);
      if (outcome?.error) { failEvidence(client, outcome.error); continue; }
      if (location && animations.every(Boolean) && outcome) {
        Object.assign(pending, { outcome: { ...outcome, receipts: undefined } });
        const receipts = [combatReceipt(client, location), ...animations.map((e, i) => combatReceipt(context.peers[i], e)), ...outcome.receipts];
        client.gate.close(receipts, Math.max(...receipts.map(r => r.atMs))); state.active = null;
      }
    }
    client.gate.expire(now);
    if (client.gate.pending?.kind === 'potion' && client.gate.pending.timedOut) {
      state.potionDisabled = true; failEvidence(client, 'issuedPotionEvidenceTimeout');
    }
  }
}

export function pollPotions(clients, now = performance.now()) {
  for (const client of clients) {
    const state = client.combat; if (!state || now < state.nextPotionPollAtMs) continue;
    state.nextPotionPollAtMs = now + POTION_POLL_MS;
    if (state.hardFailure || state.potionDisabled || client.pending || client.gate.pending || client.failure || client.closed ||
      state.hp <= 0 || state.hp / state.maxHp > .7 || now - state.lastPotionSentAtMs < 2000) continue;
    const source = client.latestSnapshotEvent, inventory = stock(source?.payload), item = inventory.find(i => i.count > 0);
    if (!item) continue; // No restock or fabricated stock; ordinary death remains a hard stop.
    check(owner(source.payload)?.name === client.actor.name && inventory.every(i => Number.isSafeInteger(i.count) && i.count > 0), 'Exact potion source inventory required');
    const spec = { kind: 'potion', actorOrdinal: client.ordinal, connection: client.connection, potionSequence: ++state.potionSequence,
      uniqueId: item.uniqueId, grid: 'Inventory', beforeStock: inventory, beforeTotal: total(inventory),
      hpAtIssue: state.hp, maxHpAtIssue: state.maxHp, sourceReceipt: combatReceipt(client, source),
      deadlineAtMs: now + 2500, dueAtMs: now, receiveSequenceFloors: [floor(client)] };
    if (!client.gate.start(spec, now)) continue;
    state.active = { kind: 'potion', floor: floor(client) }; state.lastPotionSentAtMs = now;
    try {
      client.emit({ event: 'potionSourceState', actorOrdinal: client.ordinal, connection: client.connection, potionSequence: spec.potionSequence,
        sourceReceipt: spec.sourceReceipt, fingerprint: equipmentFingerprint(source.payload), state: structuredClone(carriedState(source.payload)) });
      const sentAtMs = client.send({ type: 'useItem', uniqueId: item.uniqueId, grid: 'inventory' });
      Object.assign(client.gate.pending, { sentAtMs, deadlineAtMs: sentAtMs + 2500 }); state.lastPotionSentAtMs = sentAtMs;
      client.emit({ event: 'potionSent', ...client.gate.pending });
    } catch { Object.assign(client.gate.pending, { sendFailed: true }); failEvidence(client, 'uncertainOrdinaryPotionSend'); }
  }
}

function observePotion(client) {
  const pending = client.gate.pending, state = client.combat, context = state.active;
  const ack = receiptWindow(context, 'potionAck', client, context.floor,
    e => e.packet === 'UseItem' && Number(e.payload?.uniqueId) === pending.uniqueId && e.payload.grid === 'Inventory')[0];
  if (!ack) return;
  if (ack.payload.success !== true) {
    client.gate.close([combatReceipt(client, ack)], ack.atMs, false); state.active = null;
    state.potionDisabled = true; failEvidence(client, 'issuedPotionRejected', ack); return;
  }
  const snapshot = client.latestSnapshotEvent;
  if (!snapshot || snapshot.receiveSeq <= ack.receiveSeq) return;
  if (owner(snapshot.payload)?.name !== client.actor.name || Number(snapshot.payload.playerObjectId) !== Number(client.snapshot?.playerObjectId)) {
    failEvidence(client, 'issuedPotionStockOwnerMismatch', snapshot); return;
  }
  const after = stock(snapshot.payload), expected = pending.beforeStock.map(i => i.uniqueId === pending.uniqueId ? { ...i, count: i.count - 1 } : i).filter(i => i.count > 0);
  if (!same(after, expected) || total(after) !== pending.beforeTotal - 1) { state.potionDisabled = true; failEvidence(client, 'issuedPotionStockNotExactlyOne', snapshot); return; }
  const proof = { ack: combatReceipt(client, ack), stock: combatReceipt(client, snapshot), before: pending.beforeStock, after,
    totalBefore: pending.beforeTotal, totalAfter: total(after), inventoryFingerprint: hash(after), ordinaryIndependentStockObserved: true };
  client.emit({ event: 'potionStateEvidence', actorOrdinal: client.ordinal, connection: client.connection, potionSequence: pending.potionSequence,
    sourceReceipt: proof.stock, fingerprint: equipmentFingerprint(snapshot.payload), state: structuredClone(carriedState(snapshot.payload)) });
  const receipts = [proof.ack, proof.stock];
  const result = client.gate.close(receipts, Math.max(...receipts.map(r => r.atMs))); state.active = null;
  client.emit({ event: 'potionTerminal', actorOrdinal: client.ordinal, connection: client.connection, potionSequence: pending.potionSequence,
    sentAtMs: pending.sentAtMs, deadlineAtMs: pending.deadlineAtMs, passed: result.passed, timedOut: result.timedOut, proof });
  if (!result.passed) { state.potionDisabled = true; failEvidence(client, 'issuedPotionEvidenceLate'); }
}

export function scoreCombatWindows(plan, results) {
  const rows = [];
  for (const actor of plan.actors) for (let minute = 0; minute < plan.durationMs / 60000; minute++) {
    const offers = plan.actions.filter(a => a.actorOrdinal === actor.ordinal && Math.floor(a.dueMs / 60000) === minute);
    const actual = offers.map(a => results.find(r => r.actorOrdinal === a.actorOrdinal && r.ticketSequence === a.ticketSequence));
    const completed = actual.filter(r => r?.passed), timeout = actual.filter(r => r?.timedOut && Number.isFinite(r.sentAtMs)).length;
    const p95 = kind => percentile(completed.filter(r => r.kind === kind).map(r => r.completedAtMs - r.sentAtMs));
    const attackP95Ms = p95('attack'), chatP95Ms = p95('chat'), scheduleP95Ms = percentile(actual.filter(r => Number.isFinite(r?.sentAtMs)).map(r => r.scheduleDelayMs));
    const completionRatio = offers.length ? completed.length / offers.length : null;
    rows.push({ actorOrdinal: actor.ordinal, minute, offered: offers.length, completed: completed.length, timeout,
      notSent: actual.filter(r => !Number.isFinite(r?.sentAtMs)).length, rejected: actual.filter(r => r?.accepted === false && Number.isFinite(r?.sentAtMs)).length,
      completionRatio, attackP95Ms, chatP95Ms, scheduleP95Ms,
      passed: completionRatio !== null && completionRatio >= .95 && timeout / offers.length < .01 && attackP95Ms !== null && attackP95Ms <= 750 &&
        chatP95Ms !== null && chatP95Ms <= 1000 && scheduleP95Ms !== null && scheduleP95Ms <= 250 });
  }
  return rows;
}

export function createMeleeEngine(externalContractBytes) {
  const potions = new Map(), failures = [];
  return {
    Client: ModernCombatClient, validatePlan: plan => validateCombatPlan(plan, externalContractBytes),
    offerAction: offerCombatAction, observeActions: observeCombatActions, scoreWindows: scoreCombatWindows, poll: pollPotions,
    reportFields: { combatContractSha256: COMBAT_CONTRACT_SHA256, reactivePotionPollMs: POTION_POLL_MS,
      combatScope: 'ordinary stationary adjacent directional melee plus chat; explicit target outcomes and reactive inventory potions' },
    async setup(clients, plan) {
      for (const c of clients) {
        verifyCombatFixture(c, plan);
        const ack = await c.request({ type: 'changeAMode', mode: 5 }, e => e.packet === 'ChangeAMode' && e.payload?.mode === 5,
          c.now() + 15000, 'ordinaryAllAttackMode');
        c.emit({ event: 'combatMode', actorOrdinal: c.ordinal, connection: c.connection, mode: 5, ack: combatReceipt(c, ack) });
        beginCombatTracking(c);
      }
    },
    checkSafety(clients) { check(!clients.some(c => c.combat?.hardFailure || c.combat?.hp <= 0), 'Combat safety or causal evidence failed'); },
    record(row) {
      if (['potionSent', 'potionTerminal'].includes(row.event)) {
        const key = `${row.actorOrdinal}:${row.connection}:${row.potionSequence}`;
        potions.set(key, { ...(potions.get(key) ?? {}), ...row });
      }
      if (row.event === 'combatEvidenceFailure') failures.push(structuredClone(row));
    },
    evidenceValid(clients) { return !clients.some(c => c.combat?.hardFailure) && [...potions.values()].every(p => p.event === 'potionTerminal' && p.passed); },
    captureSavedState(client, ack) {
      const saved = client.logoutOwnedState;
      check(saved && saved.source.logOutSuccess.receiveSeq === ack.receiveSeq, 'No exact LogOutSuccess-frozen owned state'); return saved;
    },
    captureVerificationState(client) {
      const source = client.latestSnapshotEvent, snapshot = source?.payload;
      const ui = client.events.find(e => e.receiveSeq === client.bootstrapProof?.user?.receiveSeq && e.packet === 'UserInformation');
      check(ui && owner(snapshot)?.name === client.actor.name && Number.isInteger(ui.payload.hp) && Number.isInteger(ui.payload.mp), 'Fresh initial UserInformation/save roster missing');
      const initial = { ...snapshot, playerHp: ui.payload.hp, playerMp: ui.payload.mp };
      return { state: savedState(initial), carried: structuredClone(carriedState(snapshot)),
        source: { userInformation: combatReceipt(client, ui), snapshot: combatReceipt(client, source), basis: 'fresh initial UserInformation pools plus complete fresh owned roster' } };
    },
    finalizeReport(report) { report.reactivePotions = { pollIntervalMs: POTION_POLL_MS, includedInPrimaryOffers: false,
      issued: potions.size, evidence: [...potions.values()], failures, passed: [...potions.values()].every(p => p.event === 'potionTerminal' && p.passed) && failures.length === 0 }; }
  };
}
