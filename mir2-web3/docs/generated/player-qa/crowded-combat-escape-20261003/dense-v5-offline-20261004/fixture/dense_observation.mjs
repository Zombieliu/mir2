// Pure receipt classifiers. They never fabricate packets or mutate authority.
import { protocolMapCellIsWalkable } from './vendor/protocol-navigation.mjs';
import { applyProtocolObservation } from './vendor/protocol-observation.mjs';
export const DELTAS = Object.freeze({ Up: [0, -1], UpRight: [1, -1], Right: [1, 0], DownRight: [1, 1], Down: [0, 1], DownLeft: [-1, 1], Left: [-1, 0], UpLeft: [-1, -1] });
export const MOVE_BLOCK_MASK = 8 | 32 | 256;
export const CAST_BLOCK_MASK = 8 | 16 | 32 | 1024;
export const ACCEPTANCE_FALSE = Object.freeze({ fullP1Accepted: false, nativeInputAccepted: false, nativeVisualAccepted: false, humanAccepted: false, ordinaryProgressionAccepted: false, loadAccepted: false });
export const dist = (a, b) => Math.max(Math.abs(a.x - b.x), Math.abs(a.y - b.y));
export const self = snapshot => snapshot?.entities?.find(e => Number(e.objectId) === Number(snapshot?.playerObjectId));
export const location = snapshot => ({ map: String(snapshot?.mapFileName ?? ''), x: Number(self(snapshot)?.x), y: Number(self(snapshot)?.y), direction: self(snapshot)?.direction });
export const samePoint = (a, b) => a?.map === b?.map && a?.x === b?.x && a?.y === b?.y;
export const facing = (a, b) => Object.keys(DELTAS).find(d => DELTAS[d][0] === Math.sign(b.x - a.x) && DELTAS[d][1] === Math.sign(b.y - a.y));
const absolute = (value) => typeof value === 'number' && Number.isFinite(value) && value >= 0;
const SOURCES = new Set(['worldSnapshot', 'UserInformation', 'HealthChanged', 'Death']);
export function applyDenseObservation(before, message, nowMs) {
  const snapshot = applyProtocolObservation(before, message);
  if (!snapshot || typeof snapshot.playerObjectId !== 'number' || !Number.isSafeInteger(snapshot.playerObjectId) || snapshot.playerObjectId <= 0) return snapshot;
  const ownerId = Number(snapshot.playerObjectId), payload = message.payload ?? {};
  const source = message.type === 'worldSnapshot' ? 'worldSnapshot' : message.packet;
  const previous = before?.denseVitalsProvenance;
  const provenance = message.type === 'worldSnapshot' || previous?.ownerId !== ownerId ? { ownerId, hp: null, mp: null } : structuredClone(previous ?? { ownerId, hp: null, mp: null });
  if (source === 'worldSnapshot') {
    if (absolute(payload.playerHp)) provenance.hp = { ownerId, absolute: true, source, observedMs: nowMs };
    if (absolute(payload.playerMp)) provenance.mp = { ownerId, absolute: true, source, observedMs: nowMs };
  } else if (source === 'HealthChanged' || source === 'UserInformation' && Number(payload.objectId) === ownerId) {
    if (absolute(payload.hp)) provenance.hp = { ownerId, absolute: true, source, observedMs: nowMs };
    if (absolute(payload.mp)) provenance.mp = { ownerId, absolute: true, source, observedMs: nowMs };
  } else if (source === 'ObjectHealth' && Number(payload.objectId) === ownerId) {
    provenance.hp = { ownerId, absolute: false, source, observedMs: nowMs };
  } else if (source === 'ObjectMana' && Number(payload.objectId) === ownerId) {
    provenance.mp = { ownerId, absolute: false, source, observedMs: nowMs };
  } else if (source === 'Death') {
    provenance.hp = { ownerId, absolute: true, source, observedMs: nowMs };
  } else if (source === 'Revived') { provenance.hp = null; provenance.mp = null; }
  snapshot.denseVitalsProvenance = provenance;
  return snapshot;
}
const exactHp = (s, ownerId) => s?.exactHp === true && absolute(s.hp) && s.hpProvenance?.absolute === true && s.hpProvenance.ownerId === ownerId && SOURCES.has(s.hpProvenance.source);
const exactMp = (s, ownerId) => s?.exactMp === true && absolute(s.mp) && s.mpProvenance?.absolute === true && s.mpProvenance.ownerId === ownerId && SOURCES.has(s.mpProvenance.source);
function bounds(startMs, endMs, ownerId) {
  if (!Number.isFinite(startMs) || !Number.isFinite(endMs) || endMs < startMs || !Number.isSafeInteger(ownerId) || ownerId <= 0) throw new TypeError('A finite declared closed trial interval and actual owner ID are required');
}
export function liveBlockers(snapshot) { return (snapshot?.entities ?? []).filter(e => Number(e.objectId) !== Number(snapshot.playerObjectId) && e.dead !== true && !['item', 'grounddrop', 'effect'].includes(String(e.kind).toLowerCase())); }
export function ownerState(snapshot, actorId) {
  const me = self(snapshot), actor = snapshot?.entities?.find(e => Number(e.objectId) === Number(actorId));
  const ownerId = Number(snapshot?.playerObjectId), hpProvenance = snapshot?.denseVitalsProvenance?.hp ?? null, mpProvenance = snapshot?.denseVitalsProvenance?.mp ?? null;
  return { ...location(snapshot), ownerId, hp: snapshot?.playerHp ?? null, maxHp: snapshot?.playerMaxHp ?? null, mp: snapshot?.playerMp ?? null, maxMp: snapshot?.playerMaxMp ?? null, hpProvenance, mpProvenance, exactHp: hpProvenance?.absolute === true && hpProvenance.ownerId === ownerId && absolute(snapshot?.playerHp) && snapshot?.playerHealthObservation !== 'percent', exactMp: mpProvenance?.absolute === true && mpProvenance.ownerId === ownerId && absolute(snapshot?.playerMp), dead: me?.dead === true, poison: Number(snapshot?.playerPoison ?? me?.poison ?? 0), inSafeZone: snapshot?.inSafeZone === true, actor: actor ? { objectId: actor.objectId, kind: actor.kind, name: actor.name, ai: actor.ai, disposition: actor.disposition, x: actor.x, y: actor.y, direction: actor.direction, hp: actor.hp, dead: actor.dead } : null };
}
const publicRows = rows => rows.map(e => ({ sequence: e.sequence, tsMs: e.tsMs, packet: e.packet, payload: e.payload, state: e.state }));
function sourceTileExcluded(snapshot, metadata, point) {
  const inBounds = (p, b) => b && p.x >= b.minX && p.x <= b.maxX && p.y >= b.minY && p.y <= b.maxY;
  if ((snapshot.mapTransfers ?? []).some(t => inBounds(point, t.bounds))) return true;
  if ((metadata?.transfers ?? []).some(t => t.source.x === point.x && t.source.y === point.y)) return true;
  if ((metadata?.safeZones ?? []).some(z => dist(point, z.location) <= Number(z.size))) return true;
  return metadata?.fire === true || metadata?.lightning === true;
}
export function legalExit(snapshot, map, metadata, run = false) {
  const me = self(snapshot), blocked = liveBlockers(snapshot);
  if (!me || me.dead || snapshot.inSafeZone || (Number(snapshot.playerPoison ?? me.poison ?? 0) & MOVE_BLOCK_MASK)) return null;
  // All eight local cells must be static-walkable so a wall opening is never
  // mistaken for the one empty dynamic exit in a seven-actor ring.
  if (!Object.values(DELTAS).every(([dx, dy]) => protocolMapCellIsWalkable(map, { x: me.x + dx, y: me.y + dy }))) return null;
  for (const [direction, [dx, dy]] of Object.entries(DELTAS)) {
    if (dx && dy) continue; // A cardinal three-cell corridor avoids corner ambiguity.
    const corridor = [1, 2, 3].map(n => ({ x: me.x + dx * n, y: me.y + dy * n }));
    if (corridor.every(p => protocolMapCellIsWalkable(map, p, blocked) && !sourceTileExcluded(snapshot, metadata, p))) return { type: run ? 'run' : 'walk', direction, target: corridor[run ? 1 : 0], corridor };
  }
  return null;
}
export function positiveOwnerHits(events, ownerId, fromMs, untilMs) {
  bounds(fromMs, untilMs, ownerId);
  return events.filter(e => e.state?.ownerId === ownerId && e.direction === 'received' && e.packet === 'DamageIndicator' && e.tsMs >= fromMs && e.tsMs <= untilMs && Number(e.payload?.objectId) === Number(ownerId) && Number(e.payload?.damageType) === 0 && Number(e.payload?.damage) > 0);
}
export function qualifySeven(events, snapshot, map, metadata, nowMs, windowMs = 6200) {
  const me = self(snapshot), ownerId = Number(snapshot?.playerObjectId), fromMs = nowMs - windowMs;
  bounds(fromMs, nowMs, ownerId);
  const exit = legalExit(snapshot, map, metadata);
  const candidate = (snapshot?.entities ?? []).filter(e => e.kind === 'monster' && e.name === 'CaveBat' && Number(e.ai) === 0 && e.disposition === 'hostile' && e.dead !== true && me && dist(e, me) === 1);
  const attacks = events.filter(e => {
    if (e.direction !== 'received' || e.packet !== 'ObjectAttack' || e.tsMs < fromMs || e.tsMs > nowMs) return false;
    const a = e.state?.actor, o = e.state, d = DELTAS[e.payload?.direction ?? a?.direction];
    const p = e.payload?.location ?? a;
    return a?.kind === 'monster' && a.name === 'CaveBat' && Number(a.ai) === 0 && a.disposition === 'hostile' && a.dead !== true && Number(o?.ownerId) === ownerId && o.map === 'D021' && d && p && Number(p.x) + d[0] === o.x && Number(p.y) + d[1] === o.y;
  });
  const counts = new Map();
  for (const e of attacks) { const id = Number(e.payload.objectId); const rows = counts.get(id) ?? []; rows.push(e); counts.set(id, rows); }
  const eligible = candidate.filter(a => {
    const rows = counts.get(Number(a.objectId)) ?? [];
    return rows.length >= 2 && rows.at(-1).tsMs - rows[0].tsMs >= 2400;
  });
  const hits = positiveOwnerHits(events, ownerId, fromMs, nowMs);
  const exact = events.filter(e => e.tsMs >= fromMs && e.tsMs <= nowMs && exactHp(e.state, ownerId));
  const hpDeclines = exact.slice(1).filter((e, i) => e.state.hp < exact[i].state.hp).map((e, i) => e.sequence);
  const poison = Number(snapshot?.playerPoison ?? me?.poison ?? 0);
  const reasons = [];
  if (!me || me.dead || !(Number(snapshot.playerHp) > 0)) reasons.push('Owner is not positively alive');
  if (String(snapshot?.mapFileName) !== 'D021') reasons.push('Wrong original map');
  if (eligible.length < 7) reasons.push(`Only ${eligible.length} distinct live adjacent CaveBat completed two aimed attack cycles`);
  if (hits.length < 14) reasons.push(`Only ${hits.length} positive normal owner DamageIndicator receipts; Miss/zero excluded`);
  if (!hpDeclines.length) reasons.push('No corresponding exact owner HP decline observed');
  if (poison & MOVE_BLOCK_MASK) reasons.push('Genuine movement-blocking poison active');
  if (!exit) reasons.push('No fresh empty static-walkable three-cell cardinal exit');
  const hpLedger = exact.filter((e, i) => i === 0 || e.state.hp !== exact[i - 1].state.hp).map(e => ({ sequence: e.sequence, tsMs: e.tsMs, hp: e.state.hp, packet: e.packet ?? e.type }));
  return { status: reasons.length ? 'inconclusive' : 'qualified', reasons, windowMs, evaluatedAtMs: nowMs, owner: ownerState(snapshot), attackerIds: eligible.map(e => e.objectId), aimedAttackCounts: Object.fromEntries(eligible.map(e => [e.objectId, counts.get(Number(e.objectId)).length])), positiveOwnerHitCount: hits.length, positiveOwnerDamage: hits.reduce((n, e) => n + Number(e.payload.damage), 0), exactHpDeclineSequences: hpDeclines, exactHpLedger: hpLedger, healingAndRegenerationNotSubtractedFromDamageReceipts: true, exit, attackReceiptSequences: attacks.map(e => e.sequence), positiveOwnerHitSequences: hits.map(e => e.sequence), attribution: 'DamageIndicator has victim ID only; seven actor IDs are independently position/facing qualified, not per-attacker damage attribution', ...ACCEPTANCE_FALSE };
}
export function escapeProof(events, { qualification, sentSequence, sentMs, endMs, ownerId, before, target, boundMs = 1500, committedKind = 'attack', committedAckSequence }) {
  bounds(sentMs, endMs, ownerId);
  const tail = events.filter(e => e.sequence > sentSequence && e.tsMs >= sentMs && e.tsMs <= endMs && e.state?.ownerId === ownerId);
  const ack = tail.find(e => e.direction === 'received' && e.packet === 'UserLocation' && Number(e.payload?.location?.x ?? e.payload?.x) === target.x && Number(e.payload?.location?.y ?? e.payload?.y) === target.y && e.state?.map === before.map);
  const until = ack?.tsMs ?? Math.min(endMs, sentMs + boundMs);
  const during = positiveOwnerHits(tail, ownerId, sentMs, until);
  const died = tail.find(e => e.packet === 'Death' || e.state?.dead === true || (exactHp(e.state, ownerId) && e.state.hp === 0));
  const late = ack && ack.tsMs - sentMs > boundMs;
  const status = died ? 'failed' : !ack || late ? 'failed' : qualification.status !== 'qualified' || !during.length ? 'inconclusive' : 'passed';
  return { status, reason: died ? 'Actual death during escape; not relabeled a death-case pass' : !ack ? 'No target ACK within collected window' : late ? 'Late ACK retained beyond declared finite-action bound' : qualification.status !== 'qualified' ? 'Seven attacker precondition did not qualify' : !during.length ? 'No positive incoming owner hit during pending escape; continuous-hit deadline not exercised' : null, committedKind, committedAckSequence, sentSequence, window: { sentMs, endMs, ownerId }, ackSequence: ack?.sequence ?? null, latencyMs: ack ? ack.tsMs - sentMs : null, boundMs, baselineSourceActionMs: committedKind === 'attack' ? 550 : 600, sourceGlobalSpellMs: committedKind === 'cast' ? 1800 : null, internalDeadlineExposed: false, positiveOwnerHitsDuringPending: publicRows(during), corrections: publicRows(tail.filter(e => e.packet === 'UserLocation')), deathReceiptSequence: died?.sequence ?? null, qualification, ...ACCEPTANCE_FALSE };
}
export function driftProof(events, { anchorSequence, anchorMs, anchor, endMs, ownerId, durationMs = 5000 }) {
  bounds(anchorMs, endMs, ownerId);
  const rows = events.filter(e => e.sequence > anchorSequence && e.tsMs >= anchorMs && e.tsMs <= endMs && e.state?.ownerId === ownerId);
  const authority = rows.filter(e => e.direction === 'received' && (e.packet === 'UserLocation' || e.type === 'worldSnapshot'));
  const drift = authority.filter(e => Number.isFinite(e.state?.x) && !samePoint(anchor, e.state));
  const input = rows.filter(e => e.direction === 'sent' && ['walk', 'run', 'attack', 'attackDirection', 'magic', 'townRevive'].includes(e.type));
  const snapshots = authority.filter(e => e.type === 'worldSnapshot');
  const duration = endMs - anchorMs;
  const observedToEnd = snapshots.some(e => e.tsMs >= anchorMs + durationMs - 500);
  const died = rows.find(e => e.packet === 'Death');
  const status = drift.length || input.length || died ? 'failed' : duration < durationMs || !observedToEnd || snapshots.length < 4 ? 'inconclusive' : 'passed';
  return { status, durationMs: duration, requiredMs: durationMs, anchor, anchorSequence, window: { anchorMs, endMs, ownerId }, authoritySampleCount: authority.length, snapshotCount: snapshots.length, observedToEnd, extraInputs: publicRows(input), driftReceipts: publicRows(drift), positiveOwnerHits: positiveOwnerHits(rows, ownerId, anchorMs, endMs).length, deathReceiptSequence: died?.sequence ?? null, allLateReceiptsRetained: true, ...ACCEPTANCE_FALSE };
}
export function holdStopProof(events, { stopSequence, stopMs, atStop, endMs, ownerId, tailBoundMs = 1500 }) {
  bounds(stopMs, endMs, ownerId);
  const rows = events.filter(e => e.sequence > stopSequence && e.tsMs >= stopMs && e.tsMs <= endMs && e.state?.ownerId === ownerId && e.direction === 'received' && (e.packet === 'UserLocation' || e.type === 'worldSnapshot'));
  let previous = atStop;
  const changes = [];
  for (const e of rows) if (Number.isFinite(e.state?.x) && !samePoint(previous, e.state)) { changes.push(e); previous = e.state; }
  const extra = changes.length > 1 || changes.some(e => e.tsMs > stopMs + tailBoundMs);
  const anchor = changes.at(-1)?.state ?? atStop, anchorSequence = changes.at(-1)?.sequence ?? stopSequence, anchorMs = changes.at(-1)?.tsMs ?? stopMs;
  const quiet = driftProof(events, { anchorSequence, anchorMs, anchor, endMs, ownerId });
  return { status: extra || quiet.status === 'failed' ? 'failed' : quiet.status, pendingStepsAfterStop: changes.length, maximumAllowedPendingSteps: 1, tailBoundMs, postStopChanges: publicRows(changes), quiet, ...ACCEPTANCE_FALSE };
}
export function eightOccupancy(snapshot, map) {
  const me = self(snapshot);
  if (!me || me.dead) return { status: 'inconclusive', reason: 'No living owner' };
  const cells = Object.entries(DELTAS).map(([direction, [dx, dy]]) => {
    const tile = { x: me.x + dx, y: me.y + dy };
    return { direction, tile, staticWalkable: protocolMapCellIsWalkable(map, tile), blocker: liveBlockers(snapshot).find(e => e.x === tile.x && e.y === tile.y) };
  });
  const qualified = cells.every(c => c.staticWalkable && c.blocker?.kind === 'monster' && c.blocker.disposition === 'hostile' && c.blocker.dead !== true);
  return { status: qualified ? 'qualified' : 'inconclusive', cells: cells.map(c => ({ ...c, blocker: c.blocker ? { objectId: c.blocker.objectId, name: c.blocker.name, ai: c.blocker.ai } : null })), reason: qualified ? null : 'Natural eight-cell hostile surround has not formed; a seven ring is not a substitute' };
}
export function rejectionProof(events, { sentSequence, before, blockerId, ownerId, requiredWindowMs, endMs, sentMs }) {
  bounds(sentMs, endMs, ownerId);
  const rows = events.filter(e => e.sequence > sentSequence && e.tsMs >= sentMs && e.tsMs <= endMs && e.state?.ownerId === ownerId);
  const authority = rows.filter(e => e.packet === 'UserLocation' || e.type === 'worldSnapshot');
  const moved = authority.filter(e => Number.isFinite(e.state?.x) && !samePoint(before, e.state));
  const corrections = authority.filter(e => e.packet === 'UserLocation' && samePoint(before, e.state));
  const blockerDied = rows.some(e => e.packet === 'ObjectDied' && Number(e.payload?.objectId) === Number(blockerId));
  return { status: moved.length ? 'failed' : blockerDied || endMs - sentMs < requiredWindowMs || !corrections.length ? 'inconclusive' : 'passed', rejectionCorrectionSequences: corrections.map(e => e.sequence), illegalMoveReceipts: publicRows(moved), blockerDiedDuringCheck: blockerDied, ...ACCEPTANCE_FALSE };
}
export function deathProof(events, ownerId, { afterSequence = 0, fromMs, endMs }) {
  bounds(fromMs, endMs, ownerId);
  const rows = events.filter(e => e.sequence > afterSequence && e.tsMs >= fromMs && e.tsMs <= endMs && e.state?.ownerId === ownerId), death = rows.find(e => e.direction === 'received' && e.packet === 'Death');
  const positive = positiveOwnerHits(rows, ownerId, fromMs, death?.tsMs ?? endMs);
  const zero = death && rows.find(e => e.sequence >= death.sequence && exactHp(e.state, ownerId) && e.state.hp === 0 && e.state.dead === true);
  return { status: death && positive.length && zero ? 'qualified' : 'inconclusive', deathSequence: death?.sequence ?? null, positiveOwnerHitCount: positive.length, zeroHpSequence: zero?.sequence ?? null, reason: death && positive.length && zero ? null : 'Require actual incoming positive damage, owner Death and exact terminal HP 0; rounded 0% is insufficient', ...ACCEPTANCE_FALSE };
}
// These are the two normally purchased spells used by this declared cohort.
// Eligibility deliberately ignores only owner death: that is the barrier under
// test, not an alternative reason for the server to reject the Magic input.
export function ordinaryMagicEligibility(snapshot, spell, targetId) {
  const ownerId = Number(snapshot?.playerObjectId), me = self(snapshot), state = ownerState(snapshot);
  const skill = (snapshot?.knownSkills ?? []).find(s => s.spell === spell);
  const target = (snapshot?.entities ?? []).find(e => Number(e.objectId) === Number(targetId));
  const mpCostKnown = absolute(skill?.mpCost), enoughMp = mpCostKnown && exactMp(state, ownerId) && state.mp >= skill.mpCost;
  const validTarget = spell === 'Healing'
    ? !!me && Number(targetId) === ownerId && target === me
    : spell === 'FireBall' && !!me && target?.kind === 'monster' && target.disposition === 'hostile' && target.dead !== true && dist(me, target) === 1;
  const castControlClear = !(Number(snapshot?.playerPoison ?? me?.poison ?? 0) & CAST_BLOCK_MASK);
  const eligible = !!skill && validTarget && enoughMp && castControlClear;
  return { status: eligible ? 'eligible' : 'not-applicable', spell, ownerId, knownSkillObserved: !!skill, mpCost: mpCostKnown ? skill.mpCost : null, mpCostKnown, beforeMp: exactMp(state, ownerId) ? state.mp : null, absoluteMpProvenance: state.mpProvenance, enoughMp, validTarget, castControlClear, target: target ? { objectId: target.objectId, kind: target.kind, disposition: target.disposition, x: target.x, y: target.y, dead: target.dead === true } : null, ignoringOwnerDeathOnly: true };
}
export function deadActionProof(events, { deathSequence, commandsEndSequence, fromMs, endMs, before, ownerId, spell = null }) {
  bounds(fromMs, endMs, ownerId);
  const rows = events.filter(e => e.sequence > deathSequence && e.tsMs >= fromMs && e.tsMs <= endMs && e.state?.ownerId === ownerId);
  const ordinaryInputs = rows.filter(e => e.direction === 'sent' && ['walk', 'run', 'attack', 'magic'].includes(e.type));
  const revived = rows.find(e => e.packet === 'Revived' || e.packet === 'MapChanged' || e.packet === 'MapInformation');
  const snapshots = rows.filter(e => e.type === 'worldSnapshot' && e.sequence > commandsEndSequence);
  const deadSnapshots = snapshots.filter(e => absoluteDeadHpReceipt(e, ownerId) && e.state.hp === 0 && e.state.dead === true && self(e.payload)?.dead === true);
  const magicInput = spell && ordinaryInputs.find(e => e.sequence <= commandsEndSequence && e.type === 'magic' && e.spell === spell && e.objectId === ownerId);
  const baseline = magicInput && rows.findLast(e => e.direction === 'received' && e.type === 'worldSnapshot' && e.sequence < magicInput.sequence && e.tsMs <= magicInput.tsMs);
  const eligibility = baseline && ordinaryMagicEligibility({ ...baseline.payload, denseVitalsProvenance: { ownerId, hp: baseline.state.hpProvenance, mp: baseline.state.mpProvenance } }, spell, magicInput.targetId);
  const matchingTargetInput = eligibility?.validTarget === true && magicInput?.x === eligibility.target.x && magicInput?.y === eligibility.target.y;
  const baselineExact = !!baseline && baseline.payload?.playerObjectId === ownerId && absoluteMpReceipt(baseline, ownerId) && absoluteDeadHpReceipt(baseline, ownerId) && baseline.state.hp === 0 && baseline.state.dead === true && self(baseline.payload)?.dead === true;
  const eligibleInput = !!magicInput && eligibility?.status === 'eligible' && matchingTargetInput && baselineExact;
  // Absence of animation ACKs is not rejection proof: the exact release can
  // debit MP and execute an effect while deliberately suppressing dead poses.
  const afterInput = eligibleInput ? rows.filter(e => e.direction === 'received' && e.sequence > magicInput.sequence && e.tsMs >= magicInput.tsMs) : [];
  const mpRows = afterInput.filter(e => absoluteMpReceipt(e, ownerId) && (e.type !== 'worldSnapshot' || e.payload?.playerObjectId === ownerId));
  let lastMp = baselineExact ? baseline.state.mp : null;
  const absoluteMpSpendReceipts = [];
  for (const e of mpRows) { if (e.state.mp < lastMp) absoluteMpSpendReceipts.push(e); lastMp = e.state.mp; }
  const mpSnapshots = afterInput.filter(e => e.type === 'worldSnapshot');
  const completeMpCoverage = mpSnapshots.length > 0 && mpSnapshots.every(e => e.payload?.playerObjectId === ownerId && absoluteMpReceipt(e, ownerId));
  const baseMaterials = baseline && deadMaterialLedger(baseline.payload);
  const materialObservationUnknown = [];
  const materialConsumptionReceipts = [];
  let materialLedger = baseMaterials?.values;
  if (eligibleInput && baseMaterials?.status === 'unknown') materialObservationUnknown.push(baseline);
  if (eligibleInput && baseMaterials?.status === 'complete') for (const e of mpSnapshots) {
    const ledger = e.payload?.playerObjectId === ownerId ? deadMaterialLedger(e.payload) : { status: 'unknown' };
    if (ledger.status !== 'complete') { materialObservationUnknown.push(e); continue; }
    const changes = [...materialLedger].filter(([uid, quantity]) => (ledger.values.get(uid) ?? 0) < quantity).map(([uid, quantity]) => ({ uniqueId: uid, beforeQuantity: quantity, afterQuantity: ledger.values.get(uid) ?? 0 }));
    if (changes.length) materialConsumptionReceipts.push({ ...e, consumedItems: changes });
    materialLedger = ledger.values;
  }
  const ownerLifeMutationReceipts = rows.filter(e => e.direction === 'received' && (e.packet === 'UserLocation' || e.type === 'worldSnapshot' || absoluteDeadHpReceipt(e, ownerId))).filter(e => {
    const authoritativeOwner = e.type === 'worldSnapshot' ? e.payload?.playerObjectId === ownerId && self(e.payload) : e.packet === 'UserLocation';
    const loc = e.type === 'worldSnapshot' ? location(e.payload) : { map: e.state.map, ...(e.payload?.location ?? e.payload) };
    const life = absoluteDeadHpReceipt(e, ownerId) && e.state.hp > 0 || e.type === 'worldSnapshot' && e.payload?.playerObjectId === ownerId && self(e.payload)?.dead === false;
    return life || authoritativeOwner && (Number.isFinite(loc.x) && Number.isFinite(loc.y) && !samePoint(before, loc) || Object.hasOwn(DELTAS, before.direction) && Object.hasOwn(DELTAS, loc.direction) && loc.direction !== before.direction);
  });
  const matchingCastReceipts = afterInput.filter(e => matchingCastReceipt(e, spell, ownerId));
  const excludedCastReceipts = afterInput.filter(e => ['MagicCast', 'ObjectMagic'].includes(e.packet) && !matchingCastReceipt(e, spell, ownerId) && e.payload?.cast !== false);
  const preparatoryMagicReceipts = rows.filter(e => e.packet === 'ObjectMagic' && e.payload?.objectId === ownerId && e.payload?.cast === false);
  // DamageIndicator has only a victim ID. A paired living pre-death projectile
  // or another actor's cast can account for a late hit: retain that ambiguity,
  // never attribute it to the new dead Magic merely by timestamp.
  const prior = events.filter(e => e.sequence < deathSequence && e.tsMs < fromMs && e.state?.ownerId === ownerId);
  const priorProjectile = eligibleInput && prior.some(input => input.direction === 'sent' && input.type === 'magic' && input.objectId === ownerId && input.spell === 'FireBall' && input.targetId === magicInput.targetId && exactHp(input.state, ownerId) && input.state.hp > 0 && input.state.dead === false && ['MagicCast', 'ObjectMagic'].every(packet => prior.some(e => e.direction === 'received' && e.sequence > input.sequence && e.tsMs >= input.tsMs && e.packet === packet && matchingCastReceipt(e, input.spell, ownerId))));
  const context = eligibleInput ? events.filter(e => e.state?.ownerId === ownerId && e.direction === 'received' && e.tsMs <= endMs) : [];
  const foreignEffectSource = context.some(e => e.packet === 'ObjectMagic' && Number.isSafeInteger(e.payload?.objectId) && e.payload.objectId !== ownerId && e.payload?.cast === true && (!Array.isArray(e.payload?.targetIds) || e.payload.targetIds.includes(magicInput.targetId)));
  const targetedDamage = afterInput.filter(e => e.packet === 'DamageIndicator' && e.payload?.objectId === magicInput.targetId && absolute(e.payload?.damage) && e.payload.damage > 0);
  const priorProjectileEffectCandidates = priorProjectile ? targetedDamage : [];
  const unattributedTargetDamageReceipts = foreignEffectSource && !priorProjectile ? targetedDamage : [];
  const targetDamageReceipts = priorProjectile || foreignEffectSource ? [] : targetedDamage;
  const uncertainCast = excludedCastReceipts.some(e => e.packet === 'MagicCast' || e.payload?.objectId === ownerId) || priorProjectileEffectCandidates.length > 0 || unattributedTargetDamageReceipts.length > 0;
  const genericLiving = rows.filter(e => e.direction === 'received' && (e.packet === 'UserLocation' && Number.isFinite(e.state?.x) && !samePoint(before, e.state) || e.packet === 'ObjectAttack' && e.payload?.objectId === ownerId || e.packet === 'ObjectMagic' && e.payload?.objectId === ownerId && e.payload?.cast === true));
  const living = [...new Map([...genericLiving, ...ownerLifeMutationReceipts, ...matchingCastReceipts, ...targetDamageReceipts].map(e => [e.sequence, e])).values()];
  const castLife = eligibleInput ? ownerLifeMutationReceipts.filter(e => e.sequence > magicInput.sequence && e.tsMs >= magicInput.tsMs) : [];
  const castFailure = eligibleInput && (matchingCastReceipts.length > 0 || absoluteMpSpendReceipts.length > 0 || materialConsumptionReceipts.length > 0 || targetDamageReceipts.length > 0 || castLife.length > 0 || !!revived);
  const castObserved = deadSnapshots.length > 0 && completeMpCoverage && !materialObservationUnknown.length && !uncertainCast && !genericLiving.length && !revived;
  const reason = !spell ? 'No active caster spell in this class cohort' : !magicInput ? 'No actual matching ordinary Magic input' : !eligibleInput ? 'Actual matching input, exact dead baseline, valid learned spell/cost/target and sufficient absolute MP were not all proven' : castFailure ? 'Post-death matching cast/effect, absolute MP spend, material consumption or owner life/facing mutation occurred; no animation ACK is required for failure' : !completeMpCoverage ? 'Post-input closed-window MP observations are absent, partial or not explicitly absolute for this owner' : materialObservationUnknown.length ? 'Complete before/after item UID and quantity observations were not retained' : uncertainCast || genericLiving.length ? 'Wrong-spell, other committed dead actions or prior/foreign effect ambiguity cannot prove rejection of this requested dead Magic' : !deadSnapshots.length ? 'No exact terminal dead snapshot after commands' : null;
  const castBarrier = { status: castFailure ? 'failed' : !spell || !eligibleInput ? 'not-applicable' : castObserved ? 'passed' : 'inconclusive', spell, actualMagicInputSequence: magicInput?.sequence ?? null, baselineSnapshotSequence: baseline?.sequence ?? null, eligibility: eligibility ?? null, matchingTargetInput, baselineExact, completeMpCoverage, absoluteMpObservationSequences: mpRows.map(e => e.sequence), materialObservation: baseMaterials?.status ?? 'not-observed', reason };
  castBarrier.verified = castBarrier.status === 'passed';
  const ordinaryDenied = deadSnapshots.length && ordinaryInputs.some(e => e.type === 'walk') && ordinaryInputs.some(e => e.type === 'run') && ordinaryInputs.some(e => e.type === 'attack');
  return { status: living.length || revived || castFailure ? 'failed' : !ordinaryDenied || spell && !castBarrier.verified ? 'inconclusive' : 'passed', window: { fromMs, endMs, ownerId }, castBarrier, castBarrierVerified: castBarrier.verified, attemptedCommands: ordinaryInputs.map(e => ({ sequence: e.sequence, type: e.type, spell: e.spell ?? null, targetId: e.targetId ?? e.objectId ?? null })), illegalLivingReceipts: publicRows(living), absoluteMpSpendReceipts: publicRows(absoluteMpSpendReceipts), materialConsumptionReceipts: materialConsumptionReceipts.map(e => ({ ...publicRows([e])[0], consumedItems: e.consumedItems })), materialObservationUnknown: publicRows(materialObservationUnknown), ownerLifeMutationReceipts: publicRows(ownerLifeMutationReceipts), targetDamageReceipts: publicRows(targetDamageReceipts), priorProjectileEffectCandidates: publicRows(priorProjectileEffectCandidates), unattributedTargetDamageReceipts: publicRows(unattributedTargetDamageReceipts), targetDamageAttribution: 'DamageIndicator identifies victim only; targeted post-death effects are not per-spell attribution, and observed prior projectiles/foreign casts remain ambiguous', matchingCastReceipts: publicRows(matchingCastReceipts), excludedCastReceipts: publicRows(excludedCastReceipts), preparatoryMagicReceipts: publicRows(preparatoryMagicReceipts), revivalBeforeExplicitRequest: revived?.sequence ?? null, snapshotsAfterCommands: snapshots.length, exactDeadSnapshotsAfterCommands: deadSnapshots.length, ...ACCEPTANCE_FALSE };
}
function absoluteDeadHpReceipt(e, ownerId) {
  if (!exactHp(e.state, ownerId)) return false;
  if (e.type === 'worldSnapshot' && e.payload?.playerObjectId !== ownerId) return false;
  if (e.packet === 'HealthChanged' && e.payload?.objectId !== undefined && e.payload.objectId !== ownerId) return false;
  const value = e.type === 'worldSnapshot' ? e.payload?.playerHp : e.packet === 'HealthChanged' || e.packet === 'UserInformation' && e.payload?.objectId === ownerId ? e.payload?.hp : null;
  return absolute(value) && value === e.state.hp && e.state.hpProvenance.source === (e.type === 'worldSnapshot' ? e.type : e.packet);
}
function deadMaterialLedger(snapshot) {
  const keys = ['inventoryItems', 'beltItems', 'equipmentItems'];
  if (keys.every(k => !Object.hasOwn(snapshot ?? {}, k))) return { status: 'not-observed' };
  if (!keys.every(k => Array.isArray(snapshot?.[k]))) return { status: 'unknown' };
  const values = new Map();
  for (const item of keys.flatMap(k => snapshot[k])) {
    if (!item || !Number.isSafeInteger(item.uniqueId) || item.uniqueId < 0 || !Number.isSafeInteger(item.quantity) || item.quantity < 0 || values.has(item.uniqueId)) return { status: 'unknown' };
    values.set(item.uniqueId, item.quantity);
  }
  return { status: 'complete', values };
}
function matchingCastReceipt(e, spell, ownerId) {
  return typeof spell === 'string' && spell.length > 0 && e.payload?.spell === spell && (e.packet === 'MagicCast' || e.packet === 'ObjectMagic' && Number(e.payload?.objectId) === ownerId && e.payload?.cast === true);
}
function absoluteMpReceipt(e, ownerId) {
  if (!exactMp(e.state, ownerId)) return false;
  const value = e.type === 'worldSnapshot' ? e.payload?.playerMp : e.packet === 'HealthChanged' || e.packet === 'UserInformation' && Number(e.payload?.objectId) === ownerId ? e.payload?.mp : null;
  const source = e.type === 'worldSnapshot' ? e.type : e.packet;
  return absolute(value) && value === e.state.mp && e.state.mpProvenance.source === source;
}
export function castCommitProof(events, { ownerId, castSentSequence, castSentMs, spell, beforeMp, beforeState, escapeSentSequence, untilMs, commitEndMs = untilMs }) {
  bounds(castSentMs, untilMs, ownerId);
  bounds(castSentMs, commitEndMs, ownerId);
  if (commitEndMs > untilMs) throw new TypeError('Cast commitment window must close within the declared trial interval');
  const actualInput = events.find(e => e.sequence === castSentSequence && e.tsMs >= castSentMs && e.tsMs <= commitEndMs && e.state?.ownerId === ownerId && e.direction === 'sent' && e.type === 'magic' && e.spell === spell && Number(e.objectId) === ownerId);
  const rows = events.filter(e => e.sequence > castSentSequence && e.tsMs >= castSentMs && e.tsMs <= untilMs && e.state?.ownerId === ownerId && e.direction === 'received');
  const firstRows = rows.filter(e => e.tsMs <= commitEndMs);
  const castAck = firstRows.find(e => e.packet === 'MagicCast' && matchingCastReceipt(e, spell, ownerId));
  const ownerCast = firstRows.find(e => e.packet === 'ObjectMagic' && matchingCastReceipt(e, spell, ownerId));
  const baselineExact = exactMp(beforeState, ownerId) && beforeState.mp === beforeMp;
  const mp = baselineExact && firstRows.find(e => absoluteMpReceipt(e, ownerId) && e.state.mp < beforeMp);
  const effects = rows.filter(e => ['DamageIndicator', 'ObjectEffect', 'BuffAdd', 'MagicDelay', 'HealthChanged'].includes(e.packet));
  const paired = !!castAck && !!ownerCast;
  return { status: actualInput && paired && mp ? 'qualified' : 'inconclusive', spell, castSentMs, actualMagicInputSequence: actualInput?.sequence ?? null, commitmentWindow: { castSentSequence, castSentMs, commitEndMs, ownerId }, effectWindowEndsMs: untilMs, baselineExact, firstPairedCastReceiptsAlreadyObserved: paired, castAckSequence: castAck?.sequence ?? null, ownerCastSequence: ownerCast?.sequence ?? null, actualMpSpend: mp ? beforeMp - mp.state.mp : null, mpReceiptSequence: mp?.sequence ?? null, excludedCastReceipts: publicRows(rows.filter(e => ['MagicCast', 'ObjectMagic'].includes(e.packet) && !matchingCastReceipt(e, spell, ownerId))), effectReceipts: publicRows(effects), effectsAfterEscape: effects.filter(e => e.sequence > escapeSentSequence).map(e => e.sequence), sourceMagicMovementMs: 600, sourceGlobalSpellMs: 1800, perSkillClockInvented: false, ...ACCEPTANCE_FALSE };
}
export function prematureCastProof(events, { ownerId, spell, firstSentSequence, firstSentMs, beforeFirstMp, beforeFirstState, secondSentMs, secondSentSequence, beforeSecondMp, beforeSecondState, endMs }) {
  bounds(firstSentMs, secondSentMs, ownerId);
  bounds(secondSentMs, endMs, ownerId);
  const first = castCommitProof(events.filter(e => e.sequence < secondSentSequence), { ownerId, castSentSequence: firstSentSequence, castSentMs: firstSentMs, spell, beforeMp: beforeFirstMp, beforeState: beforeFirstState, untilMs: secondSentMs });
  const actualSecondInput = events.find(e => e.sequence === secondSentSequence && e.tsMs >= secondSentMs && e.tsMs <= endMs && e.state?.ownerId === ownerId && e.direction === 'sent' && e.type === 'magic' && e.spell === spell && Number(e.objectId) === ownerId);
  const rows = events.filter(e => e.sequence > secondSentSequence && e.tsMs >= secondSentMs && e.tsMs <= endMs && e.state?.ownerId === ownerId && e.direction === 'received');
  const accepted = rows.filter(e => matchingCastReceipt(e, spell, ownerId));
  const mpRows = rows.filter(e => absoluteMpReceipt(e, ownerId));
  const baselineExact = exactMp(beforeSecondState, ownerId) && beforeSecondState.mp === beforeSecondMp;
  const spent = baselineExact ? mpRows.filter(e => e.state.mp < beforeSecondMp) : [];
  const died = rows.find(e => e.packet === 'Death' || exactHp(e.state, ownerId) && e.state.hp === 0);
  const prematurelySent = secondSentMs - firstSentMs >= 0 && secondSentMs - firstSentMs <= 1200;
  const collected = endMs - secondSentMs >= 900 && mpRows.filter(e => e.type === 'worldSnapshot').length >= 2;
  return { status: died || accepted.length || spent.length ? 'failed' : !prematurelySent || !collected || first.status !== 'qualified' || !baselineExact || !actualSecondInput ? 'inconclusive' : 'passed', reason: died ? 'Owner Death or exact HP 0 during the premature-cast wait is a trial failure, not cooldown rejection' : accepted.length || spent.length ? 'Actual requested second spell or additional absolute MP spend occurred in the closed wait window' : first.status !== 'qualified' ? 'First actual spell pairing and absolute MP spend did not both qualify inside the first closed cast window' : !baselineExact ? 'No explicit absolute second-cast MP baseline' : !actualSecondInput ? 'No actual matching second ordinary Magic input' : !prematurelySent || !collected ? 'Early send or sufficient closed wait observations were not proven' : null, spell, window: { firstSentSequence, firstSentMs, secondSentSequence, secondSentMs, endMs, ownerId }, actualSecondMagicInputSequence: actualSecondInput?.sequence ?? null, sourceGlobalSpellMs: 1800, sendGapMs: secondSentMs - firstSentMs, deliberatelyEarlierByAtLeastMs: 600, firstCast: first, firstPairedCastReceiptsAlreadyObserved: first.firstPairedCastReceiptsAlreadyObserved, firstMpSpendObservedInWindow: first.mpReceiptSequence != null, acceptedSecondCastReceipts: publicRows(accepted), excludedSecondCastReceipts: publicRows(rows.filter(e => ['MagicCast', 'ObjectMagic'].includes(e.packet) && !matchingCastReceipt(e, spell, ownerId))), additionalManaSpendReceipts: publicRows(spent), exactMpSnapshotSequences: mpRows.filter(e => e.type === 'worldSnapshot').map(e => e.sequence), deathReceiptSequence: died?.sequence ?? null, rawEffectsRetained: publicRows(rows.filter(e => ['DamageIndicator', 'ObjectEffect', 'BuffAdd', 'HealthChanged', 'MagicDelay'].includes(e.packet))), healingEffectAttributionAccepted: false, privateServerDeadlineExposed: false, ...ACCEPTANCE_FALSE };
}
