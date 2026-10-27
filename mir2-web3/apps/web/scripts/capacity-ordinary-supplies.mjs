import { performance } from 'node:perf_hooks';
import { findProtocolWalkPath, protocolMapCellIsWalkable } from './quest-agent/protocol-navigation.mjs';
import { collectSafeFundingVenison } from './quest-agent/protocol-funding.mjs';
import { clearTravelBlockingMonster, harvestDirection } from './quest-agent/protocol-combat.mjs';
import { NavigationStepGuarded, NavigationStalled, isTerminalNavigationNoPath } from './quest-agent/protocol-play.mjs';
import { hasAuthoritativePlayerDeath, observedPlayerHp } from './quest-agent/protocol-observation.mjs';
import { certifyOrdinaryAi0Packet, isOrdinaryAi0Threat, isProtectedOrdinaryCell, isProtectedOrdinaryPacketCell, ordinaryProtectionSummary } from './capacity-ordinary-protection.mjs';

export const ORDINARY_SUPPLY_COMMANDS = Object.freeze(['harvest', 'interact', 'selectNpcDialog', 'sellItem', 'buyItem']);
const ALLOWED = new Set(['clientVersion', 'keepAlive', 'walk', 'run', 'turn', 'attack', 'attackV2', 'harvest', 'pickUp', ...ORDINARY_SUPPLY_COMMANDS]);
const JOHN = Object.freeze({ name: 'Merchant_John', x: 292, y: 603 });
const RUBEN = Object.freeze({ name: 'Merchant_Ruben', x: 288, y: 608 });
const HP = '(HP)DrugSmall';
class FundingTargetLoss extends Error {
  constructor(objectId, reason) {
    super(`ordinary supplies: only live Deer may be hunted (${objectId}: ${reason})`);
    this.code = 'ORDINARY_SUPPLY_FUNDING_TARGET_LOSS';
    this.reason = reason;
  }
}
// Only numeric protocol IDs and exact canonical decimal IDs can match.
const objectId = value => typeof value === 'number' && Number.isSafeInteger(value) && value > 0 && value <= 0xffff_ffff ? value :
  typeof value === 'string' && /^[1-9]\d*$/.test(value) && Number.isSafeInteger(Number(value)) && Number(value) <= 0xffff_ffff ? Number(value) : null;
const PATH_HAZARD = Symbol('ordinary-supply-path-hazard');
// Mirrors createNavigator's default trail lifetime. Production uses its
// Date.now clock; injected ordinary-navigation fixtures share options.now.
const PLANNER_TRAIL_MS = 20_000, PLANNER_TRAIL_LIMIT = 2048, PLANNER_ID_LIMIT = 1024;
const PATH_REPLAN_LIMIT = 3;
// Same source-bound visible radius as selectPreparationStagingPoint.
const PREPARATION_AOI_RADIUS = 16;
const PLANNER_POSITION_PACKETS = new Set(['ObjectWalk', 'ObjectRun', 'ObjectTurn']);
const PLANNER_IDENTITY_PACKETS = new Set(['ObjectMonster', 'NewMonsterInfo', 'ObjectPlayer', 'ObjectHero', 'ObjectNpc', 'NewNpcInfo',
  'ObjectName', 'ObjectDied', 'ObjectRevived', 'ObjectRemove', 'ObjectTeleportOut', 'Death']);
const DIRECTIONS = Object.freeze({ Up:[0,-1], UpRight:[1,-1], Right:[1,0], DownRight:[1,1], Down:[0,1], DownLeft:[-1,1], Left:[-1,0], UpLeft:[-1,-1] });
class SupplyPathHazard extends Error {
  constructor(hazard) { super('ordinary supplies: unsafe hostile approach'); this.hazard = hazard; }
}
const sleepDefault = ms => new Promise(resolve => setTimeout(resolve, ms));
const distance = (a, b) => Math.max(Math.abs(a.x - b.x), Math.abs(a.y - b.y));
const integer = (value, label, min = 0, max = Number.MAX_SAFE_INTEGER) => {
  if (!Number.isSafeInteger(value) || value < min || value > max) throw new Error(`ordinary supplies: invalid ${label}`);
  return value;
};
const template = item => Number(item?.itemIndex ?? item?.item_index ?? (item?.tooltipSource?.realInfo ?? item?.tooltipSource?.real_info ?? item?.tooltipSource?.info)?.item_index ?? (item?.tooltipSource?.realInfo ?? item?.tooltipSource?.real_info ?? item?.tooltipSource?.info)?.itemIndex);
const owner = snapshot => (snapshot?.entities ?? []).find(entity => String(entity.objectId) === String(snapshot?.playerObjectId));
const quantity = item => integer(Number(item.quantity), 'held quantity', 1);
const isHp = item => template(item) === 658 && item?.name === HP;
const held = snapshot => [...(snapshot?.inventoryItems ?? []), ...(snapshot?.beltItems ?? [])];
const hpStock = snapshot => {
  const ids = new Set();
  return held(snapshot).filter(isHp).reduce((sum, item) => {
    const id = integer(Number(item.uniqueId), 'held HP uniqueId');
    if (ids.has(id)) throw new Error('ordinary supplies: duplicate held HP identity');
    ids.add(id); return sum + quantity(item);
  }, 0);
};
const venison = snapshot => (snapshot?.inventoryItems ?? []).filter(item => item?.name === 'Venison');
const venisonStock = snapshot => venison(snapshot).reduce((sum, item) => sum + quantity(item), 0);
const point = (value, label) => ({ x: integer(value?.x, `${label}.x`, 0, 65535), y: integer(value?.y, `${label}.y`, 0, 65535) });

/** Pure geometry, not an identity certificate or a command authorization. */
export function ordinaryPreparationClearanceGeometry({radius=8,namedRadius=0,qualifiedNormalAi0=false}={}) {
  const clearance=Math.max(8,integer(radius,'geometry radius'),integer(namedRadius,'geometry named radius'));
  return qualifiedNormalAi0===true&&clearance===8 ? {x:8,y:6,rectangle:true} : {x:clearance,y:clearance,rectangle:false};
}

function roomForHp(snapshot) {
  const capacity = Number(snapshot.maxBagSlots ?? Number(snapshot.inventoryCapacity) - 6);
  integer(capacity, 'authoritative bag capacity', 1, 80);
  const occupied = new Set();
  let room = 0;
  for (const item of snapshot.inventoryItems ?? []) {
    const slot = integer(Number(item.slot), 'inventory slot', 0, 39);
    const offset = item.container === 'bag1' ? 0 : item.container === 'bag2' ? 40 : NaN;
    integer(offset, 'inventory container', 0, 40);
    if (slot + offset >= capacity || occupied.has(slot + offset)) throw new Error('ordinary supplies: invalid occupied bag slot');
    occupied.add(slot + offset);
    if (isHp(item)) room += Math.max(0, 20 - quantity(item));
  }
  for (const item of snapshot.beltItems ?? []) if (isHp(item)) room += Math.max(0, 20 - quantity(item));
  return room + (capacity - occupied.size) * 20;
}

/**
 * Explicit, quiescent map-0 preparation. No activation, revival, travel command,
 * measurement resupply or direct transport access. The caller supplies ordinary
 * navigation using the facade passed to navigate, and calls observe AFTER its
 * normal receiver has produced the actual sequence/monotonic receipt.
 */
export function createOrdinarySupplies(client, options = {}) {
  const enabled = options.preparationProfile === 'ordinary-economic-v1' || (options.targetHp ?? 0) > 0;
  const targetHp = integer(options.targetHp ?? (enabled ? 12 : 0), 'targetHp', enabled ? 1 : 0, 40);
  if (options.preparationProfile != null && !['held-only', 'ordinary-economic-v1'].includes(options.preparationProfile)) throw new Error('ordinary supplies: unknown preparation profile');
  if (options.preparationProfile === 'held-only' && targetHp !== 0) throw new Error('ordinary supplies: held-only cannot purchase');
  if (options.resolveStagingPoint != null && typeof options.resolveStagingPoint !== 'function') throw new Error('ordinary supplies: staging resolver must be synchronous function');
  const fundingMap = options.fundingNavigationMap;
  if (fundingMap !== undefined && (!fundingMap || fundingMap.mapFileName !== '0' ||
      !Number.isSafeInteger(fundingMap.width) || !Number.isSafeInteger(fundingMap.height) ||
      fundingMap.width < 1 || fundingMap.height < 1 || fundingMap.width * fundingMap.height > 4_000_000 ||
      !(fundingMap.blocked instanceof Uint8Array) || fundingMap.blocked.length !== fundingMap.width * fundingMap.height)) {
    throw new TypeError('ordinary supplies: fundingNavigationMap must be the current map0 collision bitmap');
  }
  const now = options.now ?? (() => performance.now());
  const sleep = options.sleep ?? sleepDefault;
  const timeoutMs = integer(options.timeoutMs ?? 600_000, 'timeoutMs', 1, 600_000);
  const evidenceMs = integer(options.evidenceMs ?? 12_000, 'evidenceMs', 1, 12_000);
  const maxCommands = integer(options.maxCommands ?? 2048, 'maxCommands', 1, 4096);
  const maxNavigationCommands = integer(options.maxNavigationCommands ?? 2048, 'maxNavigationCommands', 1, 4096);
  const maxHunts = integer(options.maxHunts ?? 8, 'maxHunts', 1, 8);
  const maxEmptySearchRefreshes = integer(options.maxEmptySearchRefreshes ?? 20, 'maxEmptySearchRefreshes', 1, 20);
  const rows = [], proofs = [], commandRows = [], queue = [], fundingTargetRejections = [], fundingCapitalProofs = [];
  let capitalQuoteRequested = false, fundingCapitalProofCount = 0;
  let fundingTargetRejectionCount = 0, pathHazardCount = 0;
  const pathHazards = [], pathReplans = [], movementOrigins = new WeakMap(), unsentPathErrors = new WeakSet();
  let navigationPolicy = null, fundingApproachId = null;
  const fundingApproaches = [], fundingDiscoveries = [], fundingCandidateQueries = [];
  let fundingCandidateQueryCount = 0;
  const plannerNow = options.now ?? Date.now, plannerIdentities = new Map(), plannerTrail = new Map();
  let plannerFallbackCount = 0, plannerFreshSequence = -1, plannerGlobalBlockedUntil = 0, plannerViewAllowed = null;
  let plannerPacketRenewals = 0, lastReceiptSequence = -1, lastReceiptMono = -Infinity;
  const qualificationEvents = [];
  let qualificationEventCount = 0, qualificationRepeatedReads = 0, plannerDiagnosticSource = 'planner-read', plannerDiagnosticInput = null;
  let navigationNoPath = null;
  const navigationFailures=[];let navigationFailureCount=0;
  function plannerObservedScalar(e, permitName = true) {
    if (!e || typeof e !== 'object') return null;
    const p = e.location ?? e, finite = value => Number.isFinite(value) ? value : null;
    return { ...(permitName && typeof e.name === 'string' ? {name:e.name.slice(0,80)} : {}),
      kind:typeof e.kind==='string'?e.kind.slice(0,32):null,x:finite(p.x),y:finite(p.y),
      ai:finite(e.ai),aiPresent:Object.hasOwn(e,'ai'),hp:finite(e.hp),hpPresent:Object.hasOwn(e,'hp'),
      dead:typeof e.dead==='boolean'?e.dead:null,disposition:typeof e.disposition==='string'?e.disposition.slice(0,32):null,
      ownerPresent:Object.hasOwn(e,'ownerName'),ownerIsNull:e.ownerName==null,classPresent:Object.hasOwn(e,'class'),classIsNull:e.class==null,
      masterPresent:Object.hasOwn(e,'masterObjectId'),masterIsZero:e.masterObjectId===0 };
  }
  function recordQualification(id, reason, at, observed = null) {
    try {
      const row = plannerIdentities.get(id), source = plannerDiagnosticSource;
      const input = source==='worldSnapshot' ? observed : source==='planner-read' ? null : plannerDiagnosticInput;
      const observedScalar = plannerObservedScalar(observed,observed?.kind==='monster');
      const inputScalar = plannerObservedScalar(input,source==='ObjectMonster'||(source==='worldSnapshot'&&observed?.kind==='monster'));
      const key = JSON.stringify([reason,source,observedScalar,inputScalar]);
      if (source==='planner-read' && row?.diagnosticKey===key) { qualificationRepeatedReads++; return; }
      if (row) { row.diagnosticKey=key;row.diagnosticReason=reason;row.diagnosticSource=source;row.diagnosticSequence=lastReceiptSequence; }
      qualificationEventCount++;
      qualificationEvents.push({objectId:id,reason,source,sequence:lastReceiptSequence,monotonicMs:lastReceiptMono,
        observedRawSequence:lastObserved,blockedRemainingMs:Math.max(0,(row?.blockedUntil??0)-at),
        trustRemainingMs:Math.max(0,(row?.trustedUntil??0)-at),historyRemainingMs:Math.max(0,(row?.closedHistoryUntil??0)-at),
        positionCount:[...plannerTrail.values()].filter(p=>p.objectId===id).length,identityCount:plannerIdentities.size,
        observed:observedScalar,input:inputScalar});
      if (qualificationEvents.length>32) qualificationEvents.shift();
    } catch { /* Scalar diagnostics cannot alter production qualification. */ }
  }
  function prunePlannerMemory(at) {
    for (const [key, row] of plannerTrail) if (row.expiresAt <= at) plannerTrail.delete(key);
    const retainedIds = new Set([...plannerTrail.values()].map(row => row.objectId));
    for (const [id, row] of plannerIdentities) if (row.trustedUntil <= at && row.blockedUntil <= at && !retainedIds.has(id)) plannerIdentities.delete(id);
  }
  function plannerIdentity(id, at) {
    let row = plannerIdentities.get(id);
    if (!row && plannerIdentities.size < PLANNER_ID_LIMIT) { row = { trustedUntil:0, blockedUntil:0 }; plannerIdentities.set(id,row); }
    if (!row) {
      // An untracked ID may already have an unknown shared trail. Even after
      // slots free up, never exempt it (or another ID) until that trail ages out.
      plannerGlobalBlockedUntil = Math.max(plannerGlobalBlockedUntil, at + PLANNER_TRAIL_MS); plannerFallbackCount++;
      recordQualification(id,'identity-cap-exhausted',at);
    }
    return row;
  }
  function revokePlannerIdentity(id, at, overflow = false, reason = 'not-qualified', observed = null) {
    const row = plannerIdentity(id,at);
    if (row) { row.trustedUntil = 0; row.closedHistoryUntil = 0; row.rawIdentity = null; row.wireCertificate = null; row.live = false; row.departed = false; row.blockedUntil = Math.max(row.blockedUntil, at + PLANNER_TRAIL_MS); }
    for (const [key, point] of plannerTrail) if (point.objectId === id) plannerTrail.delete(key);
    if (overflow) plannerFallbackCount++;
    recordQualification(id,reason,at,observed);
  }
  function plannerGroups(snapshot) {
    const groups = new Map();
    for (const entity of snapshot.entities ?? []) {
      const id = Number(entity?.objectId);
      if (!Number.isSafeInteger(id) || id <= 0 || id > 0xffff_ffff) continue;
      const group = groups.get(id) ?? []; group.push(entity); groups.set(id,group);
    }
    return groups;
  }
  function trustedPlannerEntity(group) {
    const entity = group.length === 1 ? group[0] : null;
    return entity && objectId(entity.objectId) !== null && entity.class == null && entity.dead === false && Number.isFinite(entity.hp) && entity.hp > 0 &&
      (entity.masterObjectId == null || entity.masterObjectId === 0) &&
      isOrdinaryAi0Threat(options.ordinaryProtection, client.snapshot.mapFileName, entity) ? entity : null;
  }
  const plannerSignature = entity => ({ kind:entity.kind, name:entity.name, ai:entity.ai, ownerName:entity.ownerName ?? null, class:entity.class ?? null });
  const samePlannerIdentity = (entity, signature) => signature &&
    Object.keys(signature).every(key => (key === 'ownerName' || key === 'class' ? entity[key] ?? null : entity[key]) === signature[key]);
  function wirePlannerEntity(group, certificate) {
    const e = group?.length === 1 ? group[0] : null;
    return e && certificate && e.objectId === certificate.objectId && e.kind === 'monster' && e.name === certificate.name &&
      e.ai === 0 && e.masterObjectId === 0 && e.dead === false && e.class == null && e.ownerName == null &&
      (e.disposition == null || e.disposition === 'hostile') && (e.hp == null || (Number.isFinite(e.hp) && e.hp > 0)) ? e : null;
  }
  function plannerEntity(group, row) {
    return row?.wireCertificate ? wirePlannerEntity(group,row.wireCertificate) : trustedPlannerEntity(group ?? []);
  }
  function protectedCell(entity, cell, historical = false) {
    if (historical) return entity.wireCertificate ?
      isProtectedOrdinaryPacketCell(options.ordinaryProtection,client.snapshot.mapFileName,entity.wireCertificate,cell) :
      isProtectedOrdinaryCell(options.ordinaryProtection,client.snapshot.mapFileName,entity,cell);
    const row = plannerIdentities.get(objectId(entity.objectId)), at = plannerNow();
    if (!row?.live || row.trustedUntil <= at || row.blockedUntil > at || at < plannerGlobalBlockedUntil) return false;
    return row.wireCertificate ? wirePlannerEntity([entity],row.wireCertificate) !== null &&
      isProtectedOrdinaryPacketCell(options.ordinaryProtection,client.snapshot.mapFileName,row.wireCertificate,cell) :
      trustedPlannerEntity([entity]) !== null && samePlannerIdentity(entity,row.rawIdentity) &&
      isProtectedOrdinaryCell(options.ordinaryProtection,client.snapshot.mapFileName,entity,cell);
  }
  function acquisitionGeometry(entity, clearance, historicalPoint = null) {
    const id=historicalPoint?.objectId ?? objectId(entity.objectId),row=plannerIdentities.get(id),at=plannerNow();
    const qualified=clearance===8 && at>=plannerGlobalBlockedUntil && row && row.blockedUntil<=at &&
      (row.live?row.trustedUntil>at:row.closedHistoryUntil>at) &&
      (historicalPoint===null || historicalPoint.expiresAt>at) &&
      (historicalPoint!==null ? (row.rawIdentity?samePlannerIdentity(entity,row.rawIdentity):row.wireCertificate&&
        samePlannerIdentity(entity,{kind:'monster',name:row.wireCertificate.name,ai:0,ownerName:null,class:null})) :
        (row.live && plannerEntity(plannerGroups(client.snapshot).get(id),row)===entity &&
          (row.rawIdentity?samePlannerIdentity(entity,row.rawIdentity):!!row.wireCertificate)));
    // Current Rust ordinary AI0 NEW target acquisition: |dx|<=8, |dy|<=6
    // (zone/runtime.rs209-210,13968-13969). This is neither disengagement nor
    // damage immunity: forced/retained targets, poison and released hits remain.
    return ordinaryPreparationClearanceGeometry({radius:clearance,qualifiedNormalAi0:!!qualified});
  }
  function entersThreatCell(from,cell,position,geometry) {
    const physical=distance(cell,position);if(physical===0)return true;
    const beforePhysical=distance(from,position);
    const metric=p=>Math.max(Math.abs(p.x-position.x)*geometry.y,Math.abs(p.y-position.y)*geometry.x);
    const boundary=geometry.x*geometry.y,before=metric(from),next=metric(cell);
    // Retreat from an already entered rectangle must preserve BOTH metrics:
    // (8,6)->(7,6) keeps weighted distance48 but approaches physically8->7.
    return before<=boundary ? next<before || physical<beforePhysical : next<=boundary;
  }
  function rememberPlannerPosition(id, entity, at) {
    const position = point(entity,'remembered planning hostile'), key = id + ':' + position.x + ',' + position.y;
    if (!plannerTrail.has(key) && plannerTrail.size >= PLANNER_TRAIL_LIMIT) { revokePlannerIdentity(id,at,true,'trail-overflow',entity); return false; }
    plannerTrail.set(key, { objectId:id, entity:{ kind:entity.kind, name:entity.name, ai:entity.ai, disposition:entity.disposition, ownerName:entity.ownerName,
      wireCertificate:plannerIdentities.get(id)?.wireCertificate ?? null },
      ...position, expiresAt:at + PLANNER_TRAIL_MS });
    return true;
  }
  function closePlannerLive(id, at) {
    const row = plannerIdentities.get(id); if (!row) return;
    if (row.live) {
      // A quiet visible monster's already certified position may outlive the
      // last identity receipt. Departure does not add or renew a position.
      row.closedHistoryUntil = row.trustedUntil > at ? Math.max(0,...[...plannerTrail.values()]
        .filter(point=>point.objectId===id).map(point=>point.expiresAt)) : 0;
    }
    row.live = false; row.departed = true;
    recordQualification(id,'live-closed-history-not-renewed',at);
  }
  function observePlannerTrust(snapshot, receipt) {
    if (!ordinaryProtectionSummary(options.ordinaryProtection) || receipt.sequence <= plannerFreshSequence) return;
    const at = plannerNow(); prunePlannerMemory(at); plannerFreshSequence = receipt.sequence; plannerViewAllowed = null;
    const groups = plannerGroups(snapshot);
    for (const id of plannerIdentities.keys()) if (!groups.has(id)) closePlannerLive(id,at);
    for (const [id, group] of groups) {
      if (!group.some(entity => String(entity.kind).toLowerCase() === 'monster') && !plannerIdentities.has(id)) continue;
      const entity = trustedPlannerEntity(group);
      if (!entity) { revokePlannerIdentity(id,at,false,'raw-not-qualified',group.length===1?group[0]:null); continue; }
      let row = plannerIdentity(id,at);
      if ((row?.rawIdentity && !samePlannerIdentity(entity,row.rawIdentity)) ||
          (row?.wireCertificate && row.wireCertificate.name !== entity.name)) { revokePlannerIdentity(id,at,false,'raw-identity-conflict',entity); row = plannerIdentities.get(id); }
      if (row) { row.rawIdentity = plannerSignature(entity); row.wireCertificate = null; row.live = true; row.departed = false; row.closedHistoryUntil = 0; row.trustedUntil = at + PLANNER_TRAIL_MS; }
      if (row) recordQualification(id,'raw-qualified',at,entity);
    }
  }
  function observePlannerPacket(message, receipt) {
    if (!ordinaryProtectionSummary(options.ordinaryProtection) || message?.type !== 'packet') return;
    const packetName = message.packet;
    if (!PLANNER_POSITION_PACKETS.has(packetName) && !PLANNER_IDENTITY_PACKETS.has(packetName)) return;
    const id = objectId(message.payload?.objectId);
    if (id === null || id === identity) return;
    const at = plannerNow(); prunePlannerMemory(at); plannerViewAllowed = null;
    const value = scalar(client.snapshot);
    if (value.ownerId !== identity || value.ownerName !== identityName || value.map !== '0') {
      observationFault = 'ordinary supplies: received owner/map changed'; return;
    }
    const group = plannerGroups(client.snapshot).get(id) ?? [];
    if (packetName === 'ObjectRemove' || packetName === 'ObjectTeleportOut') {
      // AOI departure closes live renewal, not the already certified history.
      // Keep its original TTL and cell-resolved trail; never extend either here.
      closePlannerLive(id,at);
      return;
    }
    if (packetName === 'ObjectMonster') {
      const certificate = certifyOrdinaryAi0Packet(options.ordinaryProtection,value.map,message);
      const entity = wirePlannerEntity(group,certificate);
      let row = plannerIdentities.get(id);
      if (!entity || entity.x !== certificate.x || entity.y !== certificate.y) { revokePlannerIdentity(id,at,false,'wire-not-qualified',group.length===1?group[0]:null); return; }
      if ((row?.rawIdentity && !samePlannerIdentity(entity,row.rawIdentity)) ||
          (row?.wireCertificate && row.wireCertificate.name !== certificate.name)) revokePlannerIdentity(id,at,false,'wire-identity-conflict',entity);
      row = plannerIdentity(id,at); if (!row) return;
      row.rawIdentity = null; row.wireCertificate = certificate; row.live = true; row.departed = false; row.closedHistoryUntil = 0;
      if (rememberPlannerPosition(id,entity,at)) { row.trustedUntil = at + PLANNER_TRAIL_MS; plannerPacketRenewals++;recordQualification(id,'wire-qualified',at,entity); }
      return;
    }
    if (PLANNER_IDENTITY_PACKETS.has(packetName)) { revokePlannerIdentity(id,at,false,'explicit-lifecycle-or-type',group.length===1?group[0]:null); return; }
    const row = plannerIdentities.get(id);
    if (row?.live === false) { revokePlannerIdentity(id,at,false,'position-after-live-closed',group.length===1?group[0]:null); return; }
    if ((!row?.rawIdentity && !row?.wireCertificate) || row.trustedUntil <= at || row.blockedUntil > at) {
      recordQualification(id,'position-without-current-qualification',at,group.length===1?group[0]:null);return;
    }
    const entity = plannerEntity(group,row), payload = message.payload;
    const location = payload.location ?? payload;
    const samePayload = ['name','ai','ownerName','class','kind','disposition','dead'].every(key =>
      !Object.hasOwn(payload,key) || payload[key] === entity?.[key]);
    if (!entity || (row.rawIdentity && !samePlannerIdentity(entity,row.rawIdentity)) || !samePayload ||
        (Object.hasOwn(payload,'masterObjectId') && payload.masterObjectId !== 0) ||
        !Number.isSafeInteger(location.x) || location.x < 0 || location.x > 65535 ||
        !Number.isSafeInteger(location.y) || location.y < 0 || location.y > 65535 ||
        entity.x !== location.x || entity.y !== location.y) { revokePlannerIdentity(id,at,false,'position-identity-or-shape-conflict',entity); return; }
    // Only a new receipt for this locally lifecycle-bound live identity renews it.
    // The merged position was applied by the ordinary receiver before this hook.
    if (rememberPlannerPosition(id,entity,at)) { row.trustedUntil = at + PLANNER_TRAIL_MS; plannerPacketRenewals++;recordQualification(id,'individual-position-qualified',at,entity); }
  }
  function syncPlannerMemory() {
    guard(); const at = plannerNow(); prunePlannerMemory(at);
    for (const [id, group] of plannerGroups(client.snapshot)) {
      if (!group.some(entity => String(entity.kind).toLowerCase() === 'monster') && !plannerIdentities.has(id)) continue;
      const entity = plannerEntity(group,plannerIdentities.get(id));
      if (!entity) { revokePlannerIdentity(id,at,false,'planner-current-not-qualified',group.length===1?group[0]:null); continue; }
      const row = plannerIdentity(id,at); if (!row) continue; // ID cap: leave shared coarse memory authoritative.
      if (row.rawIdentity && !samePlannerIdentity(entity,row.rawIdentity)) { revokePlannerIdentity(id,at,false,'planner-current-identity-conflict',entity); continue; }
      // Shared navigation refreshes each current position at every planner
      // read, not just at a raw receipt. Mirror that trail clock, but renew
      // TRUST only on a new raw or exact raw-certified individual receipt.
      rememberPlannerPosition(id,entity,at);
    }
    return { at, allowed:new Set([...plannerIdentities].filter(([,row]) => at >= plannerGlobalBlockedUntil &&
      (row.live ? row.trustedUntil > at : row.closedHistoryUntil > at) && row.blockedUntil <= at).map(([id]) => id)) };
  }
  const counts = {};
  let state = enabled ? 'idle' : 'disabled', active = false, used = false, draining = false;
  let started = null, deadline = null, identity = null, identityName = null, before = null, after = null, failure = null;
  let lastObserved = -1, lastObservedMono = -Infinity, commands = 0, movementCommands = 0, pendingSaleId = null, lastSent = null, observationFault = null;
  let funding = null, sold = 0, bought = 0, goldEarned = 0, goldSpent = 0, actualStagingReached = false;
  let resolvedStagingPoint = null;

  function scalar(snapshot) {
    const actor = owner(snapshot);
    if (!actor) throw new Error('ordinary supplies: missing owner');
    if (typeof actor.name !== 'string' || !actor.name.trim() || actor.name.length > 128) throw new Error('ordinary supplies: invalid owner name');
    const hp = observedPlayerHp(snapshot), maxHp = Number(snapshot.playerMaxHp ?? actor.maxHp);
    if (!Number.isFinite(hp) || hp <= 0 || !Number.isFinite(maxHp) || maxHp <= 0 || hp > maxHp || hasAuthoritativePlayerDeath(snapshot)) throw new Error('ordinary supplies: dead or invalid owner health');
    // Poisoned/ObjectPoisoned carry u16; the normal observer retains these
    // packet fields when a WorldSnapshot omits transient poison state.
    const poison = value => value == null ? null : integer(value, 'observed poison', 0, 65535);
    return { ownerId: integer(Number(snapshot.playerObjectId), 'ownerId', 1), ownerName: actor.name, map: String(snapshot.mapFileName), ...point(actor, 'owner'), hp, maxHp,
      inSafeZone: typeof snapshot.inSafeZone === 'boolean' ? snapshot.inSafeZone : null,
      nearbyHostileCount: (snapshot.entities ?? []).filter(entity => String(entity.kind).toLowerCase() === 'monster' &&
        String(entity.disposition).toLowerCase() === 'hostile' && !entity.dead && Number(entity.hp) > 0 && distance(actor, entity) <= 8).length,
      playerPoison: poison(snapshot.playerPoison), ownerPoison: poison(actor.poison), dazed: typeof actor.dazed === 'boolean' ? actor.dazed : null,
      gold: integer(Number(snapshot.gold), 'gold'), hpStock: hpStock(snapshot), venison: venisonStock(snapshot),
      saleRemaining: pendingSaleId == null ? null : venison(snapshot).filter(item => Number(item.uniqueId) === pendingSaleId).reduce((sum, item) => sum + quantity(item), 0),
      npcId: Number(snapshot.activeNpcDialog?.npcObjectId) || null,
      buyLink: (snapshot.activeNpcDialog?.links ?? []).some(link => String(link?.target).toLowerCase() === '@buysell' && link.disabled !== true && link.enabled !== false),
      sellLink: (snapshot.activeNpcDialog?.links ?? []).some(link => ['@sell', '@buysell'].includes(String(link?.target).toLowerCase()) && link.disabled !== true && link.enabled !== false) };
  }
  function guard() {
    if (!active || !options.isPreparation?.()) throw new Error('ordinary supplies: quiescent preparation required');
    if (now() >= deadline) throw new Error('ordinary supplies: preparation wall budget exhausted');
    if (client.failure) throw client.failure;
    if (client.closed) throw new Error('ordinary supplies: transport closed');
    if (observationFault) throw new Error(observationFault);
    const value = scalar(client.snapshot);
    if (value.map !== '0' || value.ownerId !== identity || value.ownerName !== identityName ||
        (client.label != null && client.label !== identityName) || value.hp / value.maxHp < 0.7) throw new Error('ordinary supplies: owner/map/health changed');
    return value;
  }
  function observe(message, receipt) {
    if ((!active && state !== 'ready') || !options.isPreparation?.() || receipt?.direction !== 'received' || !Number.isSafeInteger(receipt.sequence) || !Number.isFinite(receipt.monotonicMs)) return;
    if (receipt.sequence <= lastReceiptSequence || receipt.monotonicMs < lastReceiptMono) return;
    lastReceiptSequence = receipt.sequence; lastReceiptMono = receipt.monotonicMs;
    if (['MapChanged', 'MapInformation'].includes(message?.packet)) observationFault = 'ordinary supplies: map transition during preparation';
    if (message?.type !== 'worldSnapshot') {
      plannerDiagnosticSource=typeof message?.packet==='string'?message.packet.slice(0,80):'non-packet';plannerDiagnosticInput=message?.payload;
      try { observePlannerPacket(message,receipt); } catch { /* Malformed packet metadata never certifies an identity. */ }
      finally { plannerDiagnosticSource='planner-read';plannerDiagnosticInput=null; }
      return;
    }
    if (receipt.sequence <= lastObserved || receipt.monotonicMs < lastObservedMono) return;
    lastObserved = receipt.sequence; lastObservedMono = receipt.monotonicMs;
    plannerDiagnosticSource='worldSnapshot';
    try {
      const value = scalar(message.payload);
      if (value.ownerId !== identity || value.ownerName !== identityName || value.map !== '0') observationFault = 'ordinary supplies: received owner/map changed';
      if (!observationFault) observePlannerTrust(message.payload, receipt);
      // Capture both wallet and item quotes from this single raw receipt,
      // never from a later mutable client.snapshot. Invalid quotes are terminal
      // for the opt-in capital check, not silently treated as zero capital.
      let capitalQuote, capitalQuoteError;
      if (capitalQuoteRequested) {
        try { capitalQuote = fundingCapitalQuote(message.payload, value.hpStock); }
        catch (error) { capitalQuoteError = error; }
      }
      rows.push({ sequence: receipt.sequence, monotonicMs: receipt.monotonicMs, ...value,
        ...(capitalQuoteRequested ? { capitalQuote, capitalQuoteError } : {}) });
      if (rows.length > 64) rows.shift();
    } catch { /* Incomplete/invalid observations cannot satisfy a proof. guard still checks live state. */ }
    finally { plannerDiagnosticSource='planner-read';plannerDiagnosticInput=null; }
  }
  async function pause(ms) {
    await drain(); guard();
    if (ms >= deadline - now()) throw new Error('ordinary supplies: preparation wall budget exhausted');
    await sleep(ms); guard();
  }
  function visibleHostiles() {
    return (client.snapshot.entities ?? []).filter(entity => String(entity.kind).toLowerCase() === 'monster' &&
      entity.dead !== true && !(Number.isFinite(entity.hp) && entity.hp <= 0) &&
      entity.disposition !== 'neutral' && entity.disposition !== 'friendly');
  }
  function blockerScalar(entity) {
    return { objectId: objectId(entity.objectId), name: typeof entity.name === 'string' ? entity.name.slice(0,80) : null,
      ...point(entity, 'visible hostile'), hp: Number.isFinite(entity.hp) ? entity.hp : null,
      ai: Number.isSafeInteger(entity.ai) ? entity.ai : null };
  }
  function recordPathHazard(action, phase, from, physicalCells, blocker, radius, reason) {
    const value = guard(), hazard = { [PATH_HAZARD]: true, reason, action, phase, radius,
      from: { ...from }, physicalCells: physicalCells.map(cell => ({ ...cell })), blocker: blocker ? blockerScalar(blocker) : null,
      snapshotSequence: client.sequence, observedSnapshotSequence: lastObserved, ownerId: value.ownerId, ownerName: value.ownerName, map: value.map };
    pathHazardCount++;
    if (pathHazards.length < 32) { const { [PATH_HAZARD]: _, ...scalar } = hazard; pathHazards.push(scalar); }
    return hazard;
  }
  function movementHazard(action, phase, from, physicalCells, policy = navigationPolicy) {
    guard(); const radius = policy?.radius ?? 8;
    for (const hostile of visibleHostiles()) {
      const position = point(hostile, 'visible hostile');
      const named = policy?.byName?.get(String(hostile.name ?? '').replace(/[^a-z0-9]/gi,'').toLowerCase()) ?? 0;
      const clearance = Math.max(radius, named);
      const geometry=acquisitionGeometry(hostile,clearance);
      for (const cell of physicalCells) {
        const nextDistance = distance(cell, position);
        // Static, source-checked membership applies to this exact server cell
        // and this ordinary AI0 monster, never to an inferred future safe zone.
        // Even a protected cell cannot bypass authoritative entity occupancy.
        if (nextDistance !== 0 && protectedCell(hostile,cell) === true) continue;
        // Entry barrier, never a cage: an already-close actor may keep or grow
        // its distance from every threat, but may not enter an occupied tile.
        if (entersThreatCell(from,cell,position,geometry))
          return recordPathHazard(action, phase, from, physicalCells, hostile, clearance, 'hostile-clearance');
      }
    }
    return null;
  }
  function protectedPlannerObstacles(policy) {
    plannerViewAllowed = syncPlannerMemory().allowed;
    const from = point(owner(client.snapshot), 'planning owner'), obstacles = new Map();
    // Rebuild every retained normal trail even while its ID is quarantined
    // or its raw qualification has expired. Shared coarse memory then adds
    // conservative coverage. This also closes the forbidden-before-allowed
    // clock boundary: granting an ID can never precede its outside halo.
    for (const remembered of plannerTrail.values()) {
      const hostile = remembered.entity, position = { x:remembered.x, y:remembered.y };
      const named = policy.byName.get(String(hostile.name ?? '').replace(/[^a-z0-9]/gi,'').toLowerCase()) ?? 0;
      const clearance = Math.max(policy.radius, named);
      const geometry=acquisitionGeometry(hostile,clearance,remembered);
      for (let y = Math.max(0, position.y-clearance); y <= Math.min(65535, position.y+clearance); y++) {
        for (let x = Math.max(0, position.x-clearance); x <= Math.min(65535, position.x+clearance); x++) {
          const cell = { x, y }, nextDistance = distance(cell, position);
          if (nextDistance !== 0 && protectedCell(hostile,cell,true)) continue;
          if (entersThreatCell(from,cell,position,geometry))
            obstacles.set(`${x},${y}`, cell);
        }
      }
    }
    return [...obstacles.values()];
  }
  function assertMovement(command, origin) {
    const from = point(owner(client.snapshot), 'movement owner');
    if (!origin || from.x !== origin.from.x || from.y !== origin.from.y)
      throw new SupplyPathHazard(recordPathHazard(command.type, 'send', from, [], null, origin?.policy?.radius ?? 8, 'owner-transform-changed'));
    const delta = DIRECTIONS[command.direction];
    if (!delta) throw new Error('ordinary supplies: invalid ordinary movement direction');
    const cells = Array.from({ length: command.type === 'run' ? 2 : 1 }, (_, index) =>
      point({ x:from.x+delta[0]*(index+1), y:from.y+delta[1]*(index+1) }, 'movement cell'));
    const hazard = movementHazard(command.type, 'send', from, cells, origin.policy);
    if (hazard) throw new SupplyPathHazard(hazard);
  }
  function fundingTargetFacts(id) {
    const matches = (client.snapshot.entities ?? []).filter(entity => objectId(entity.objectId) === id);
    const target = matches.length === 1 ? matches[0] : null;
    const blocker = target && visibleHostiles().find(hostile => { const p = point(hostile, 'visible hostile'); return Math.abs(p.x-target.x)<=8 && Math.abs(p.y-target.y)<=6; });
    const reason = !matches.length ? 'missing' : matches.length > 1 ? 'ambiguous-id' :
      String(target.kind).toLowerCase() !== 'monster' || target.name !== 'Deer' ? 'not-deer' :
      target.dead !== false || !Number.isFinite(target.hp) || !(target.hp > 0) ? 'not-confirmed-live' :
      target.disposition !== 'neutral' || target.ownerName ? 'not-neutral-deer' :
      blocker ? 'hostile-near-deer' : null;
    return { matches, target, blocker, reason };
  }
  function fundingTransferBounds() {
    const transfers = client.snapshot.mapTransfers;
    if (!Array.isArray(transfers)) throw new Error('ordinary supplies: complete funding map transfers required');
    return transfers.map(transfer => {
      const b = transfer?.bounds;
      if (!b || !['minX','maxX','minY','maxY'].every(k => Number.isSafeInteger(b[k]) && b[k] >= 0 && b[k] <= 65535) ||
          b.minX > b.maxX || b.minY > b.maxY || (transfer.mapFileName != null && transfer.mapFileName !== '0')) {
        throw new Error('ordinary supplies: invalid funding map transfer');
      }
      return b;
    });
  }
  const inFundingTransfer = (cell,bounds) => bounds.some(b => cell.x >= b.minX && cell.x <= b.maxX && cell.y >= b.minY && cell.y <= b.maxY);
  function fundingStaticRouteExists(from,target,bounds) {
    const left=Math.max(0,target.x-16),top=Math.max(0,target.y-16);
    const right=Math.min(fundingMap.width-1,target.x+16),bottom=Math.min(fundingMap.height-1,target.y+16);
    if (from.x<left || from.x>right || from.y<top || from.y>bottom || target.x<left || target.x>right || target.y<top || target.y>bottom) return false;
    const width=right-left+1,height=bottom-top+1,blocked=new Uint8Array(width*height);
    for(let y=top;y<=bottom;y++)for(let x=left;x<=right;x++)blocked[(y-top)*width+x-left]=
      fundingMap.blocked[y*fundingMap.width+x] || Number(inFundingTransfer({x,y},bounds));
    return findProtocolWalkPath({map:{width,height,blocked},start:{x:from.x-left,y:from.y-top},target:{x:target.x-left,y:target.y-top},desiredDistance:1})!==null;
  }
  function fundingHistoryBlocksGoals(from,target,bounds) {
    const at=plannerNow(),goals=[];
    for(let y=Math.max(0,target.y-1);y<=target.y+1;y++)for(let x=Math.max(0,target.x-1);x<=target.x+1;x++){
      const c={x,y};
      if(protocolMapCellIsWalkable(fundingMap,c) && !inFundingTransfer(c,bounds))goals.push(c);
    }
    if (!goals.length) return false; // Static query already excludes an empty goal set.
    const trails=[...plannerTrail.values()].filter(t => t.expiresAt>at && acquisitionGeometry(t.entity,8,t).rectangle===true);
    return goals.every(cell=>trails.some(t =>
      !(distance(cell,t)!==0 && protectedCell(t.entity,cell,true)) && entersThreatCell(from,cell,t,{x:8,y:6,rectangle:true})));
  }
  function recordFundingCandidateQuery(id,from,target,reason) {
    fundingCandidateQueryCount++;
    const last=fundingCandidateQueries.at(-1);
    if(last && last.objectId===id && last.reason===reason && last.sequence===client.sequence &&
      distance(last.from,from)===0 && distance(last.target,target)===0)return;
    if(fundingCandidateQueries.length<32)fundingCandidateQueries.push({objectId:id,sequence:client.sequence,
      observedSnapshotSequence:lastObserved,from:{x:from.x,y:from.y},target:{...target},reason,
      staticAoiRadius:16,range:1,queryOnly:true,attemptCharged:false,qualificationGranted:false});
  }
  function preferFundingCandidate(candidate) {
    const from=guard(),id=objectId(candidate?.objectId);
    if(id===null || fundingTargetFacts(id).reason!==null)return false;
    if(fundingMap===undefined)return true;
    const target=point(candidate,'funding candidate'),bounds=fundingTransferBounds();
    const reason=!fundingStaticRouteExists(from,target,bounds)?'static-aoi-unreachable':
      fundingHistoryBlocksGoals(from,target,bounds)?'certified-goal-blocked':null;
    guard(); // Synchronous planning time belongs to the same actor/global budget.
    if(reason)recordFundingCandidateQuery(id,from,target,reason);
    return reason===null;
  }
  function fundingAoiBoundary() {
    if(fundingMap===undefined || fundingApproachId===null)return [];
    assertFundingApproach('approach-planning');
    const target=point(fundingTargetFacts(fundingApproachId).target,'funding approach target'),cells=[];
    for(let y=Math.max(0,target.y-17);y<=Math.min(65535,target.y+17);y++)for(let x=Math.max(0,target.x-17);x<=Math.min(65535,target.x+17);x++)
      if(distance({x,y},target)===17)cells.push({x,y});
    return cells;
  }
  function assertFundingAoiCells(cells) {
    if(fundingMap===undefined || fundingApproachId===null)return;
    const target=fundingTargetFacts(fundingApproachId).target;
    if(cells.some(cell=>distance(cell,target)>16))throw new FundingTargetLoss(fundingApproachId,'approach-leaves-aoi');
  }
  function assertFundingTarget(command, phase) {
    const id = integer(command.objectId, 'attack objectId', 1, 0xffff_ffff);
    const { matches, target, blocker, reason } = fundingTargetFacts(id);
    if (!reason) return;
    const value = guard();
    fundingTargetRejectionCount++;
    if (fundingTargetRejections.length < 32) fundingTargetRejections.push({ action: command.type, targetId: id, phase, reason,
      targetKind: typeof target?.kind === 'string' ? target.kind.slice(0, 32) : null,
      targetName: typeof target?.name === 'string' ? target.name.slice(0, 80) : null,
      targetDead: typeof target?.dead === 'boolean' ? target.dead : null,
      targetHp: Number.isFinite(target?.hp) ? target.hp : null,
      targetIdType: target ? typeof target.objectId : null, matches: matches.length,
      blocker: blocker ? blockerScalar(blocker) : null,
      snapshotSequence: client.sequence, observedSnapshotSequence: lastObserved, observedSnapshotMonotonicMs: lastObservedMono,
      ownerId: value.ownerId, ownerName: value.ownerName, map: value.map });
    throw new FundingTargetLoss(id, reason);
  }
  function assertFundingApproach(phase) {
    if (fundingApproachId === null) return;
    guard(); // Safety and actor/global budgets take priority over target loss.
    if (commands >= maxCommands || movementCommands >= maxNavigationCommands) throw new Error('ordinary supplies: command budget exhausted');
    assertFundingTarget({ type:'attack', objectId:fundingApproachId }, phase);
  }
  async function clearFundingMonster(fundingClient, target, navigateNear, settings) {
    const selectedId = integer(Number(target.objectId), 'funding target objectId', 1, 0xffff_ffff);
    const value = guard();
    const record = { objectId:selectedId, sequence:client.sequence, observedSnapshotSequence:lastObserved,
      monotonicMs:now(), from:{x:value.x,y:value.y}, target:point(target,'funding target'),
      ownerId:value.ownerId, ownerName:value.ownerName, map:value.map, navigationCalls:0, status:'selection', reason:null };
    if (fundingApproaches.length < 8) fundingApproaches.push(record);
    const approach = async (destination, range, stop, navOptions) => {
      guard();
      assertFundingTarget({ type:'attack', objectId:selectedId }, 'approach-entry');
      if (fundingApproachId !== null || objectId(destination?.objectId) !== selectedId) throw new Error('ordinary supplies: unexpected funding approach target');
      fundingApproachId = selectedId; record.navigationCalls++; record.status = 'approaching';
      try {
        await navigateNear(destination, range, () => {
          assertFundingApproach('approach-stop');
          return typeof stop === 'function' && stop();
        }, { ...navOptions, detectPositionCycles:true });
      } finally { fundingApproachId = null; }
    };
    try {
      assertFundingTarget({ type:'attack', objectId:selectedId }, 'selection');
      const cleared = await clearTravelBlockingMonster(fundingClient, target, approach, settings);
      record.status = 'completed'; return cleared;
    } catch (error) {
      record.status = 'terminal'; record.reason = 'ordinary-operation-failed';
      if (error instanceof FundingTargetLoss && ['missing','ambiguous-id','not-deer','not-confirmed-live','not-neutral-deer','hostile-near-deer','approach-leaves-aoi'].includes(error.reason)) record.reason = error.reason;
      else if (error instanceof NavigationStalled) record.reason = 'navigation-stalled';
      else {
        try {
          const message = error?.message;
          if (typeof message === 'string') {
            if (message === `target ${selectedId} left the authoritative snapshot before attack` ||
                message === `target ${selectedId} left the authoritative snapshot before death was confirmed`) record.reason = 'raw-target-loss';
            else if (message === `target ${selectedId} has no walk path from the current map region`) record.reason = 'no-walk-path';
            else if (new RegExp(`^target ${selectedId} made no authoritative combat progress after [1-9]\\d* attacks$`).test(message)) record.reason = 'no-authoritative-progress';
          }
        } catch { /* A diagnostic classifier cannot replace the original error. */ }
      }
      if (!(error instanceof FundingTargetLoss)) throw error;
      guard(); // Owner, transport and global budgets remain terminal.
      if (commands >= maxCommands || movementCommands >= maxNavigationCommands) throw new Error('ordinary supplies: command budget exhausted');
      if (queue.length !== 0 || pendingSaleId !== null) throw new Error('ordinary supplies: target loss with pending commands', { cause:error });
      record.status = 'target-lost';
      // The original attempt was already charged. Only a confirmed empty
      // queue can use the existing bounded exclusion path; never replay it.
      const lost = new Error(`target ${selectedId} left the authoritative snapshot before attack`, { cause:error });
      lost.code = error.code; throw lost;
    } finally {
      record.endedSequence = client.sequence; record.elapsedMs = Math.max(0,now()-record.monotonicMs);
    }
  }
  function enqueue(command) {
    guard();
    if (!command || !ALLOWED.has(command.type)) throw new Error('ordinary supplies: forbidden command');
    if (['attack', 'attackV2'].includes(command.type)) assertFundingTarget(command, 'enqueue');
    if (command.type === 'harvest' && !(client.snapshot.entities ?? []).some(entity => entity.name === 'Deer' && entity.dead === true && distance(owner(client.snapshot), entity) === 1 && harvestDirection(owner(client.snapshot), entity) === command.direction)) throw new Error('ordinary supplies: only adjacent Deer corpse may be harvested');
    if (command.type === 'pickUp' && !(client.snapshot.groundDrops ?? []).some(drop => drop.objectId === command.objectId && drop.name === 'Venison' && distance(owner(client.snapshot), drop) === 0)) throw new Error('ordinary supplies: only same-tile visible Venison may be picked up');
    if (queue.length >= 8) throw new Error('ordinary supplies: send queue limit');
    const queued = { ...command };
    if (['walk', 'run'].includes(command.type)) movementOrigins.set(queued, { from: point(owner(client.snapshot), 'queued movement owner'), policy: navigationPolicy });
    queue.push(queued);
  }
  async function drain() {
    if (draining) throw new Error('ordinary supplies: reentrant send drain');
    draining = true;
    try {
      while (queue.length) {
        guard();
        await client.pace(); guard();
        let command = queue.shift();
        if (['walk', 'run'].includes(command.type)) {
          try {
            assertFundingApproach('approach-send');
            const origin=movementOrigins.get(command),delta=DIRECTIONS[command.direction];
            if(origin&&delta)assertFundingAoiCells(Array.from({length:command.type==='run'?2:1},(_,i)=>({x:origin.from.x+delta[0]*(i+1),y:origin.from.y+delta[1]*(i+1)})));
            assertMovement(command, origin);
          }
          catch (error) {
            // This precise movement was dequeued but never dispatched. A
            // retry is possible only with no other queued side effects.
            if (error instanceof SupplyPathHazard && queue.length === 0) unsentPathErrors.add(error);
            throw error;
          }
        }
        if (['attack', 'attackV2'].includes(command.type)) assertFundingTarget(command, 'send');
        if (command.type === 'attack' && client.attackResults === 'v2') {
          if (typeof client.nextAttackV2 !== 'function') throw new Error('ordinary supplies: v2 physical attack unavailable');
          command = client.nextAttackV2(command.objectId);
        }
        if (commands >= maxCommands || (['walk', 'run'].includes(command.type) && movementCommands >= maxNavigationCommands)) throw new Error('ordinary supplies: command budget exhausted');
        const sentAtMs = now(), sendCursor = client.sequence;
        client.send(command); commands++; counts[command.type] = (counts[command.type] ?? 0) + 1;
        lastSent = { cursor: sendCursor, sentAtMs, deadline: Math.min(deadline, sentAtMs + evidenceMs) };
        if (['walk', 'run'].includes(command.type)) movementCommands++;
        if (commandRows.length < 32) commandRows.push({ type: command.type, sendCursor, sentAtMs });
      }
    } finally { draining = false; }
  }
  async function wait(predicate, label, ms = evidenceMs) {
    await drain(); guard();
    const stop = Math.min(deadline, now() + Math.min(ms, evidenceMs));
    const timeout = cause => {
      const error = new Error(`Timeout waiting for ${label}`, cause ? { cause } : undefined);
      error.code = 'CAPACITY_WAIT_TIMEOUT'; error.waitLabel = label; return error;
    };
    try {
      return await client.wait(() => { guard(); if (now() > stop) throw timeout(); return predicate(); }, label, Math.max(1, stop - now()));
    } catch (error) {
      // Only a real deadline for this exact wait may use the older helper's
      // retry convention. Identity, health, transport and global-budget errors
      // must remain terminal even when their text contains "timed out".
      guard();
      if (error?.code === 'CAPACITY_WAIT_TIMEOUT' && error.waitLabel === label) {
        throw error.message === `Timeout waiting for ${label}` ? error : timeout(error);
      }
      throw error;
    }
  }
  async function transmit(command) {
    await drain(); guard(); enqueue(command); await drain(); return { ...lastSent };
  }
  const fresh = (row, mark) => row.sequence > mark.cursor && row.monotonicMs >= mark.sentAtMs && row.monotonicMs <= mark.deadline && row.ownerId === identity && row.ownerName === identityName && row.map === '0';
  async function packet(command, packetName) {
    const mark = await transmit(command);
    return wait(() => (client.events ?? []).find(event => event.direction === 'received' && event.packet === packetName && fresh({ ...event, ownerId: identity, ownerName: identityName, map: '0' }, mark)), packetName);
  }
  async function refresh() {
    const mark = await transmit({ type: 'clientVersion' });
    return wait(() => rows.find(row => fresh(row, mark)), 'fresh ordinary supply snapshot');
  }
  async function refreshFundingSearch(_owner, discovery) {
    if (discovery?.type !== 'fundingCandidateDiscovery') return refresh();
    const record = { objectId:discovery.objectId, triggerSequence:discovery.triggerSequence,
      beforeRefreshSequence:client.sequence, searchPoint:{...discovery.searchPoint},
      refresh:discovery.refresh, cueRefresh:discovery.cueRefresh, limit:discovery.limit,
      status:'refreshing', discoveryOnly:true, liveQualification:false };
    if (fundingDiscoveries.length < maxEmptySearchRefreshes) fundingDiscoveries.push(record);
    try {
      const row = await refresh(); guard();
      record.status = 'refreshed'; record.freshSnapshotSequence = row.sequence; record.monotonicMs = row.monotonicMs;
      return row;
    } catch (error) { record.status = 'terminal'; record.reason = 'ordinary-refresh-failed'; throw error; }
  }
  const facade = new Proxy(client, { get(target, key) {
    if (key === 'send') return enqueue;
    if (key === 'wait') return wait;
    if (key === 'request') return packet;
    if (key === 'pace') return async () => { await drain(); guard(); await client.pace(); guard(); };
    if (key === 'record') return () => {}; // Existing helper diagnostics must not expand retained payloads.
    const value = Reflect.get(target, key, target);
    return typeof value === 'function' ? value.bind(target) : value;
  } });
  async function navigate(target, range, stop, navOptions) {
    await drain(); guard();
    const pickupTile = (client.snapshot.groundDrops ?? []).some(drop => drop.objectId === target?.objectId && drop.name === 'Venison');
    if (navigationPolicy) throw new Error('ordinary supplies: reentrant navigation');
    const radius = Math.max(8, integer(navOptions?.hostileAvoidanceRadius ?? 0, 'navigation hostile radius'));
    const byName = new Map(Object.entries(navOptions?.hostileAvoidanceByName ?? {}).map(([name,value]) =>
      [name.replace(/[^a-z0-9]/gi,'').toLowerCase(), integer(value,'named hostile radius')]));
    navigationPolicy = { radius, byName };
    const safeOptions = { ...navOptions, hostileAvoidanceRadius: radius, allowedHostileObjectIds: [],
      beforeMovement: context => {
        assertFundingApproach('approach-before-movement');
        assertFundingAoiCells(context.physicalCells);
        const hazard = movementHazard(context.movementType, 'before-movement', context.from, context.physicalCells);
        return hazard ?? navOptions?.beforeMovement?.(context);
      } };
    if (ordinaryProtectionSummary(options.ordinaryProtection)) {
      // The shared planner's coarse squares would otherwise block protected
      // village cells before beforeMovement can examine them. Rebuild only
      // authenticated ordinary-AI0 squares at cell resolution. Getters run at
      // each planner read, with the then-current owner/AOI, never a stale start.
      const policy = navigationPolicy;
      Object.defineProperties(safeOptions, {
        allowedHostileObjectIds: { enumerable:true, get:() => {
          const current = syncPlannerMemory().allowed;
          // The shared planner reads forbiddenPoints before allowed IDs. A
          // later clock read may revoke this view, but cannot enlarge it.
          return [...current].filter(id => plannerViewAllowed === null || plannerViewAllowed.has(id));
        } },
        forbiddenPoints: { enumerable:true, get:() => [...(navOptions?.forbiddenPoints ?? []), ...protectedPlannerObstacles(policy), ...fundingAoiBoundary()] },
      });
    } else if (fundingMap!==undefined && fundingApproachId!==null) {
      Object.defineProperty(safeOptions,'forbiddenPoints',{enumerable:true,get:()=>[...(navOptions?.forbiddenPoints??[]),...fundingAoiBoundary()]});
    }
    try {
      for (;;) {
        try {
          await options.navigate(facade, target, pickupTile ? 0 : range, stop, safeOptions);
          await drain(); guard(); return;
        } catch (error) {
          try {
            if (String(error?.message??'').startsWith('No walk path on ')) captureNoPath(error,target,pickupTile?0:range);
          } catch { /* Even diagnostic error detection must preserve the original failure. */ }
          const ownHazard = error instanceof SupplyPathHazard || error instanceof NavigationStepGuarded && error.hazard?.[PATH_HAZARD] === true;
          if (!ownHazard) {
            if (await waitForClosedPlannerHistory(error,target,pickupTile?0:range,stop)) continue;
            throw error;
          }
          guard();
          // Funding retains its original finite target-loss/reselection path.
          if (state === 'funding') throw new Error('No walk path: ordinary supplies hostile clearance', { cause:error });
          const hazard = error.hazard;
          const unsent = error instanceof NavigationStepGuarded ? hazard.phase === 'before-movement' :
            hazard.phase === 'send' && unsentPathErrors.has(error);
          if (!unsent || hazard.reason !== 'hostile-clearance' || queue.length !== 0 ||
              pendingSaleId !== null || pathReplans.length >= PATH_REPLAN_LIMIT) throw error;
          pathReplans.push({ attempt:pathReplans.length + 1, phase:hazard.phase, reason:hazard.reason,
            snapshotSequence:client.sequence, observedSnapshotSequence:lastObserved, commands, movementCommands,
            target:point(target,'replan target'), from:{...hazard.from} });
          // No queue clearing, refresh, dose, target selection or budget reset.
          // A new route is computed against the current authoritative AOI.
        }
      }
    } finally { navigationPolicy = null; }
  }
  async function waitForClosedPlannerHistory(error,target,range,stop) {
    if (state !== 'staging' || !isTerminalNavigationNoPath(error) || queue.length !== 0 ||
        pendingSaleId !== null || pathReplans.length >= PATH_REPLAN_LIMIT) return false;
    const value=guard();
    const at=plannerNow(), view=error.navigationDiagnostics;
    if (at < plannerGlobalBlockedUntil || view?.source !== 'shared-navigator-exact-cache' || !Object.isFrozen(view)) return false;
    if (view.mapFileName !== value.map || view.from?.x !== value.x || view.from?.y !== value.y ||
        view.target?.x !== target.x || view.target?.y !== target.y || view.desiredDistance !== range) return false;
    // An unknown/coarse barrier is not a certified natural-expiry case.
    // Truncation also cannot establish the absence of such a barrier.
    if (view.hostileMemoryTruncated || (view.hostileMemory ?? []).some(p=>p.allowed !== true)) return false;
    const visible=plannerGroups(client.snapshot), matches=[];
    for (const remembered of view.hostileMemory ?? []) {
      if (remembered.currentlyVisible !== false || remembered.allowed !== true || visible.has(remembered.objectId)) continue;
      const row=plannerIdentities.get(remembered.objectId), key=`${remembered.objectId}:${remembered.x},${remembered.y}`;
      const trail=plannerTrail.get(key);
      if (!row || row.live !== false || row.blockedUntil > at || !(row.rawIdentity || row.wireCertificate) ||
          !trail || trail.expiresAt <= at || trail.expiresAt > row.closedHistoryUntil) continue;
      matches.push({key,row,trail,expiresAt:trail.expiresAt});
    }
    if (!matches.length) return false;
    // Only helper-owned original expiries authorize waiting. Diagnostic TTLs
    // are relative to another read and never create, renew or clear memory.
    const until=Math.max(...matches.map(p=>p.expiresAt));
    if (until-at > PLANNER_TRAIL_MS) return false;
    const replan={attempt:pathReplans.length+1,phase:'terminal-no-path',reason:'closed-history-natural-expiry',
      snapshotSequence:client.sequence,observedSnapshotSequence:lastObserved,commands,movementCommands,
      target:point(target,'replan target'),from:{x:value.x,y:value.y},waitMs:until-at,historyPoints:matches.length,
      history:matches.slice(0,128).map(match=>({objectId:match.trail.objectId,x:match.trail.x,y:match.trail.y,remainingMs:match.expiresAt-at})),
      actualPassiveWaitMs:0,waitCompleted:false,naturalExpiryReached:false,freshSnapshotSequence:null,commandsAfterRefresh:null};
    pathReplans.push(replan);
    const passiveStarted=now();
    try {
      while (plannerNow() < until) {
        guard();
        if (typeof stop === 'function' && stop()) throw error;
        const currentAt=plannerNow();
        if (currentAt < plannerGlobalBlockedUntil || queue.length !== 0 || pendingSaleId !== null) throw error;
        for (const match of matches) {
          if (currentAt >= match.expiresAt) continue;
          const row=plannerIdentities.get(match.trail.objectId);
          if (row !== match.row || row.live !== false || row.blockedUntil > currentAt ||
              !(row.rawIdentity || row.wireCertificate) || row.closedHistoryUntil < match.expiresAt ||
              plannerTrail.get(match.key) !== match.trail || match.trail.expiresAt !== match.expiresAt ||
              plannerGroups(client.snapshot).has(match.trail.objectId)) throw error;
        }
        await pause(Math.min(250,until-currentAt));
      }
    } finally {
      replan.actualPassiveWaitMs=Math.max(0,now()-passiveStarted);
      replan.naturalExpiryReached=plannerNow()>=until;
    }
    guard();
    if (typeof stop === 'function' && stop()) throw error;
    const freshSnapshot=await refresh(); guard();
    replan.freshSnapshotSequence=freshSnapshot.sequence;replan.commandsAfterRefresh=commands;replan.waitCompleted=true;
    return true;
  }
  function captureNoPath(error,target,range) {
    try {
      const at=plannerNow(), groups=plannerGroups(client.snapshot), shared=error.navigationDiagnostics;
      const exact=shared?.source==='shared-navigator-exact-cache'&&Object.isFrozen(shared)?shared:null;
      const ids=[...new Set([...(exact?.hostileMemory??[]).map(p=>p.objectId),
        ...[...groups].filter(([,g])=>g.some(e=>e.kind==='monster')).map(([id])=>id)])];
      const states=ids.slice(0,128).map(id=>{
        const row=plannerIdentities.get(id),group=groups.get(id)??[];
        return Object.freeze({objectId:id,live:row?.live===true,qualification:row?.wireCertificate?'wire':row?.rawIdentity?'raw':'none',
          blockedRemainingMs:Math.max(0,(row?.blockedUntil??0)-at),trustRemainingMs:Math.max(0,(row?.trustedUntil??0)-at),
          historyRemainingMs:Math.max(0,(row?.closedHistoryUntil??0)-at),positionCount:[...plannerTrail.values()].filter(p=>p.objectId===id).length,
          lastReason:row?.diagnosticReason??null,lastSource:row?.diagnosticSource??null,lastSourceSequence:row?.diagnosticSequence??null,
          current:group.length===1?Object.freeze(plannerObservedScalar(group[0],group[0]?.kind==='monster')):null,duplicates:group.length});
      });
      navigationNoPath=Object.freeze({source:'ordinary-helper-terminal-navigation-failure',scope:'diagnostic-only-no-measurement-credit',
        state,sequence:client.sequence,observedRawSequence:lastObserved,from:Object.freeze(point(owner(client.snapshot),'diagnostic owner')),
        target:Object.freeze(point(target,'diagnostic target')),range,commands,movementCommands,attempts:error.attempts??null,
        successfulSteps:error.successfulSteps??null,globalQuarantineRemainingMs:Math.max(0,plannerGlobalBlockedUntil-at),
        shared:exact,qualificationTotal:ids.length,qualificationTruncated:ids.length>128,qualifications:Object.freeze(states)});
      const from=point(owner(client.snapshot),'diagnostic owner'),goal=point(target,'diagnostic target');
      const repeated=navigationFailures.some(p=>p.from.x===from.x&&p.from.y===from.y&&p.target.x===goal.x&&p.target.y===goal.y&&p.range===range);
      navigationFailures.push({sequence:client.sequence,from,target:goal,range,repeatedRetainedPosition:repeated});
      navigationFailureCount++;if(navigationFailures.length>32)navigationFailures.shift();
    } catch { /* Evidence failure never replaces or recovers a navigation failure. */ }
  }
  function liveNpc(spec) {
    const actor = owner(client.snapshot);
    return (client.snapshot.entities ?? []).find(entity => String(entity.kind).toLowerCase() === 'npc' && entity.name === spec.name && entity.x === spec.x && entity.y === spec.y && Number.isSafeInteger(entity.objectId) && entity.objectId > 0 && distance(actor, entity) <= 16);
  }
  async function shop(spec, sale = false) {
    await navigate(spec, 16); const npc = liveNpc(spec);
    if (!npc) throw new Error('ordinary supplies: exact live merchant unavailable');
    const npcId = npc.objectId;
    const mark = await transmit({ type: 'interact', objectId: npcId });
    await wait(() => rows.find(row => fresh(row, mark) && row.npcId === npcId && (sale ? row.sellLink : row.buyLink)), 'fresh merchant dialog');
    const link = client.snapshot.activeNpcDialog?.links?.find(item => (sale ? ['@sell', '@buysell'] : ['@buysell']).includes(String(item.target).toLowerCase()) && item.disabled !== true && item.enabled !== false);
    if (!link || liveNpc(spec)?.objectId !== npcId || Number(client.snapshot.activeNpcDialog.npcObjectId) !== npcId) throw new Error('ordinary supplies: merchant dialog changed');
    const result = await packet({ type: 'selectNpcDialog', target: link.target }, sale ? 'NPCSell' : 'NPCGoods');
    // Opening a native service can dismiss only the text dialog. Service
    // authority was established by this fresh matched packet following the
    // exact live NPC's enabled link, not by keeping that text window open.
    guard();
    const remainingDialog = client.snapshot.activeNpcDialog;
    if (liveNpc(spec)?.objectId !== npcId ||
        (remainingDialog != null && Number(remainingDialog.npcObjectId) !== npcId)) throw new Error('ordinary supplies: merchant changed during service');
    if (sale) return null;
    if (Number(result.payload?.panelType) !== 0 || !Array.isArray(result.payload?.list)) throw new Error('ordinary supplies: invalid goods panel');
    const row = result.payload.list.find(item => template(item) === 658 && item.name === HP);
    if (!row) throw new Error('ordinary supplies: missing exact HP goods row');
    const id = row.id ?? row.uniqueId ?? row.unique_id;
    if (!((typeof id === 'number' && Number.isSafeInteger(id) && id >= 0) || (typeof id === 'string' && /^(0|[1-9]\d*)$/.test(id) && BigInt(id) <= 18446744073709551615n))) throw new Error('ordinary supplies: invalid live shop row id');
    return { id, price: integer(Number(row.price), 'live HP price', 1), quoteSequence: result.sequence, quoteMonotonicMs: result.monotonicMs };
  }
  function fundingCapitalQuote(snapshot, stock) {
    if (!Array.isArray(snapshot.inventoryItems) || !Array.isArray(snapshot.beltItems)) throw new Error('ordinary supplies: incomplete capital inventory');
    const ids = new Set(), items = [];
    const gold = integer(snapshot.gold, 'capital gold');
    let capital = gold, venisonItemCount = 0;
    for (const [entries, inventory] of [[snapshot.inventoryItems, true], [snapshot.beltItems, false]]) {
      for (const entry of entries) {
        if (!entry || typeof entry !== 'object' || Array.isArray(entry)) throw new Error('ordinary supplies: invalid capital inventory record');
        const uniqueId = integer(entry.uniqueId, 'capital uniqueId');
        if (ids.has(uniqueId)) throw new Error('ordinary supplies: duplicate capital inventory UID');
        ids.add(uniqueId);
        if (inventory && entry.name === 'Venison') {
          integer(uniqueId, 'capital Venison uniqueId', 1);
          const count = integer(entry.quantity, 'capital Venison quantity', 1);
          const sellValue = integer(entry.sellValue, 'capital Venison sellValue', 1);
          const value = integer(count * sellValue, 'capital Venison value', 1);
          capital = integer(capital + value, 'capital total');
          venisonItemCount++;
          if (items.length < 128) items.push({ uniqueId, quantity: count, sellValue, value });
        }
      }
    }
    return { gold, hpStock: stock, items, venisonItemCount, itemsTruncated: venisonItemCount > items.length, capital };
  }
  async function hasRequiredFundingBudget(initialGoods) {
    capitalQuoteRequested = true;
    try {
      const row = await refresh(); guard();
      if (row.capitalQuoteError) throw row.capitalQuoteError;
      const quote = row.capitalQuote;
      if (!quote) throw new Error('ordinary supplies: fresh capital quote missing');
      const initialHpPrice = initialGoods.price;
      const initialHpQuote = { sequence: integer(initialGoods.quoteSequence, 'initial HP quote sequence'),
        monotonicMs: initialGoods.quoteMonotonicMs, price: initialHpPrice };
      const quoteAgeMs = row.monotonicMs - initialHpQuote.monotonicMs;
      if (!Number.isFinite(initialHpQuote.monotonicMs) || !Number.isFinite(quoteAgeMs) || quoteAgeMs < 0) throw new Error('ordinary supplies: invalid initial HP quote receipt');
      const requiredGold = integer(Math.max(0, targetHp - quote.hpStock) * initialHpPrice, 'capital required gold');
      const satisfied = quote.capital >= requiredGold;
      fundingCapitalProofCount++;
      if (fundingCapitalProofs.length === 32) fundingCapitalProofs.shift();
      fundingCapitalProofs.push({ sequence: row.sequence, monotonicMs: row.monotonicMs, ...quote,
        initialHpPrice, initialHpQuote, quoteAgeMs, requiredGold, satisfied, planningOnly: true, preparationOnly: true, measurementCredit: false });
      return satisfied;
    } finally { capitalQuoteRequested = false; }
  }
  async function sellUntil(requiredGold) {
    if (guard().gold >= requiredGold || !venison(client.snapshot).length) return;
    state = 'selling'; await shop(JOHN, true);
    // Never sell starter equipment, quest items, potions or any other material.
    for (let sale = 0; sale < maxHunts + 2 && guard().gold < requiredGold; sale++) {
      const item = venison(client.snapshot)[0]; if (!item) break;
      const id = integer(Number(item.uniqueId), 'Venison uniqueId', 1), count = quantity(item), price = integer(Number(item.sellValue), 'live Venison sellValue', 1);
      pendingSaleId = id; const initial = guard();
      const mark = await transmit({ type: 'sellItem', uniqueId: id, count });
      const ack = await wait(() => (client.events ?? []).find(event => event.direction === 'received' && event.packet === 'SellItem' && fresh({ ...event, ownerId: identity, ownerName: identityName, map: '0' }, mark) && event.payload?.success === true && Number(event.payload.uniqueId) === id && Number(event.payload.count) === count), 'fresh exact Venison sale ACK');
      const proof = await wait(() => rows.find(row => fresh(row, mark) && row.gold === initial.gold + price * count && row.venison === initial.venison - count && row.saleRemaining === 0), 'fresh exact Venison sale');
      proofs.push({ kind: 'sale', sequence: proof.sequence, monotonicMs: proof.monotonicMs, ackSequence: ack.sequence, quantity: count, gold: price * count });
      sold += count; goldEarned += price * count; pendingSaleId = null;
    }
  }
  function result() {
    return { profile: enabled ? 'ordinary-economic-v1' : 'held-only', status: state, targetHp, before, after, actualStagingReached,
      commands, movementCommands, commandCounts: { ...counts }, commandRows: [...commandRows], commandRowsTruncated: commands > commandRows.length,
      purchased: bought, soldVenison: sold, goldEarned, goldSpent, funding, proofs: [...proofs], elapsedMs: started == null ? 0 : Math.max(0, now() - started), failure,
      commandCountsScope: 'module-owned commands; external keepAlive shares the original client budget',
      stagingSafetyScope: 'Fresh authoritative inSafeZone protects against normal monster targeting and impacts only. Existing poison and released effects may still damage; absent status fields do not prove their absence.',
      measurementCredit: false, sustainableDurationProven: false,
      protectionSummary: ordinaryProtectionSummary(options.ordinaryProtection),
      preparationThreatModel:{id:'ordinary-ai0-new-target-8x6-v1',newTargetAcquisitionOnly:true,
        qualifiedNormalAi0:{x:8,y:6},unknownOrUnqualifiedMinimumSquare:8,explicitLargerRequestsPreserved:true,
        retainedOrForcedTargetsMayExceedAcquisitionRange:true,damageImmunity:false},
      navigationCycleDiagnostics:{scope:'terminal-position-recurrence-only-no-retry-credit',total:navigationFailureCount,
        truncated:navigationFailureCount>navigationFailures.length,views:navigationFailures.map(p=>({...p,from:{...p.from},target:{...p.target}}))},
      plannerThreatMemory: { lifetimeMs:PLANNER_TRAIL_MS, positionLimit:PLANNER_TRAIL_LIMIT, identityLimit:PLANNER_ID_LIMIT, positions:plannerTrail.size, identities:plannerIdentities.size, coarseFallbackCount:plannerFallbackCount, packetRenewals:plannerPacketRenewals, exemptionGloballyQuarantined:plannerNow() < plannerGlobalBlockedUntil },
      qualificationEventCount,qualificationRepeatedReads,qualificationEventsTruncated:qualificationEventCount>qualificationEvents.length,
      qualificationEvents:qualificationEvents.map(row=>({...row,input:row.input?{...row.input}:null,observed:row.observed?{...row.observed}:null})),
      navigationNoPath,
      fundingCandidateQueryCount, fundingCandidateQueriesTruncated:fundingCandidateQueryCount>fundingCandidateQueries.length,
      fundingCandidateQueries:fundingCandidateQueries.map(row=>({...row,from:{...row.from},target:{...row.target}})),
      fundingDiscoveries: fundingDiscoveries.map(row=>({...row,searchPoint:{...row.searchPoint}})),
      fundingApproaches: fundingApproaches.map(row => ({ ...row, from:{...row.from}, target:{...row.target} })),
      fundingTargetRejections: fundingTargetRejections.map(row => ({ ...row })), fundingTargetRejectionCount,
      fundingCapitalProofs: fundingCapitalProofs.map(row => ({ ...row, items: row.items.map(item => ({ ...item })), initialHpQuote: { ...row.initialHpQuote } })),
      fundingCapitalProofCount, fundingCapitalProofsTruncated: fundingCapitalProofCount > fundingCapitalProofs.length,
      fundingTargetRejectionsTruncated: fundingTargetRejectionCount > fundingTargetRejections.length,
      pathHazards: pathHazards.map(row => ({ ...row, from:{...row.from}, physicalCells:row.physicalCells.map(cell=>({...cell})), blocker:row.blocker?{...row.blocker}:null })),
      pathHazardCount, pathHazardsTruncated: pathHazardCount > pathHazards.length,
      pathReplanLimit:PATH_REPLAN_LIMIT, pathReplanCount:pathReplans.length,
      pathReplans:pathReplans.map(row=>({...row,target:{...row.target},from:{...row.from},...(row.history?{history:row.history.map(p=>({...p}))}:{})})),
      ...(options.resolveStagingPoint ? { resolvedStagingPoint: resolvedStagingPoint ? { ...resolvedStagingPoint } : null } : {}) };
  }
  function freshStagingReadiness({ afterSequence = -1, maxAgeMs = evidenceMs } = {}) {
    integer(afterSequence, 'fresh ready cursor', -1); integer(maxAgeMs, 'ready freshness', 1, evidenceMs);
    const value = scalar(client.snapshot), staging = point(resolvedStagingPoint ?? options.stagingPoint, 'explicit stagingPoint');
    // Never fall back to an older true membership after a newer snapshot.
    const receipt = rows.at(-1);
    if (!receipt || receipt.sequence !== lastObserved || receipt.monotonicMs !== lastObservedMono ||
        receipt.sequence <= afterSequence || receipt.monotonicMs > now() || receipt.monotonicMs < now() - maxAgeMs ||
        receipt.ownerId !== identity || receipt.ownerName !== identityName || receipt.map !== '0' || receipt.hpStock < targetHp ||
        receipt.x !== staging.x || receipt.y !== staging.y || receipt.hp / receipt.maxHp < 0.7 ||
        value.ownerId !== identity || value.ownerName !== identityName || (client.label != null && client.label !== identityName) ||
        value.map !== '0' || value.x !== staging.x || value.y !== staging.y || value.hpStock < targetHp || value.hp / value.maxHp < 0.7 ||
        client.closed || client.failure) throw new Error('ordinary supplies: fresh cohort readiness failed');
    if ([receipt, value].some(row => row.playerPoison > 0 || row.ownerPoison > 0 || row.dazed === true)) throw new Error('ordinary supplies: negative status at staging');
    if (receipt.inSafeZone !== true && (receipt.nearbyHostileCount > 0 || value.nearbyHostileCount > 0)) throw new Error('ordinary supplies: hostile near staging');
    return { ...value, inSafeZone: receipt.inSafeZone, nearbyHostileCount: receipt.nearbyHostileCount,
      currentNearbyHostileCount: value.nearbyHostileCount, sequence: receipt.sequence, monotonicMs: receipt.monotonicMs, actualStagingReached: true,
      stagingProtection: receipt.inSafeZone === true ? 'authoritative-safe-zone-normal-monsters' : 'no-observed-nearby-hostile' };
  }
  function assertReady(readinessOptions = {}) {
    if (state !== 'ready' || !options.isPreparation?.() || observationFault) throw new Error('ordinary supplies: cohort readiness unavailable');
    return freshStagingReadiness(readinessOptions);
  }
  async function prepare() {
    if (!enabled) return result();
    if (used) throw new Error('ordinary supplies: preparation is single-use');
    used = true;
    started = now(); deadline = started + timeoutMs; active = true; state = 'refreshing';
    try {
      if (typeof client?.pace !== 'function' || typeof client?.send !== 'function' || typeof client?.wait !== 'function' || typeof options.navigate !== 'function' || typeof options.isPreparation !== 'function') throw new Error('ordinary supplies: required client/navigation/preparation adapters missing');
      let staging = point(options.stagingPoint, 'explicit stagingPoint');
      identity = integer(Number(client.snapshot?.playerObjectId), 'initial ownerId', 1);
      identityName = owner(client.snapshot)?.name;
      before = await refresh(); guard();
      if (before.hpStock < targetHp) {
        let goods = await shop(RUBEN), requiredGold = (targetHp - guard().hpStock) * goods.price;
        await sellUntil(requiredGold);
        if (guard().gold < requiredGold) {
          if (!options.route || !Array.isArray(options.route.quests)) throw new Error('ordinary supplies: authoritative Deer route required');
          if (client.attackResults === 'v2' && (client.attackV2Supported !== true || typeof client.nextAttackV2 !== 'function')) throw new Error('ordinary supplies: v2 physical attack unavailable');
          state = 'funding';
          // 200 is only a bounded hunt-count estimate. Actual sale values and
          // live shop prices decide readiness; insufficient proceeds fail.
          const wanted = venisonStock(client.snapshot) + Math.max(1, Math.ceil((requiredGold - guard().gold) / 200));
          const initialGoods = { ...goods };
          funding = await collectSafeFundingVenison(facade, { route: options.route, requiredCount: wanted, maxHunts, maxEmptySearchRefreshes,
            hasRequiredBudget: () => hasRequiredFundingBudget(initialGoods), preferCandidate: preferFundingCandidate, qualifiedOnlyCandidates:true, refreshUnqualifiedCandidates:true,
            travel: async map => { if (map !== '0') throw new Error('ordinary supplies: map travel forbidden'); guard(); },
            navigate, clearMonster: clearFundingMonster, sleep: pause, refreshWhileWaiting: refreshFundingSearch, sustain: async () => { await drain(); guard(); } });
          await refresh(); await sellUntil(requiredGold);
        }
        goods = await shop(RUBEN); state = 'buying';
        while (guard().hpStock < targetHp) {
          const initial = guard(), count = Math.min(20, targetHp - initial.hpStock);
          if (roomForHp(client.snapshot) < count) throw new Error('ordinary supplies: inventory full');
          if (initial.gold < goods.price * count) throw new Error('ordinary supplies: insufficient earned gold');
          const mark = await transmit({ type: 'buyItem', itemIndex: goods.id, count, panelType: 0 });
          const proof = await wait(() => rows.find(row => fresh(row, mark) && row.gold === initial.gold - goods.price * count && row.hpStock === initial.hpStock + count && row.venison === initial.venison), 'fresh exact HP purchase');
          proofs.push({ kind: 'purchase', sequence: proof.sequence, monotonicMs: proof.monotonicMs, quantity: count, gold: goods.price * count });
          bought += count; goldSpent += goods.price * count;
        }
      }
      if (options.resolveStagingPoint) {
        state = 'staging';
        if (distance(guard(),staging) > PREPARATION_AOI_RADIUS) {
          // The declared point is only a region hint, possibly already saved
          // by another actor. Do not choose or occupy an unseen final point.
          await navigate(staging,PREPARATION_AOI_RADIUS);
          // Position packets alone cannot establish current candidate occupancy.
          // This ordinary refresh shares the same actor wall/command budgets.
          await refresh(); guard();
        }
        let receipt = rows.at(-1);
        const value = guard();
        const reusable = receipt && receipt.sequence === client.sequence && receipt.sequence === lastObserved && receipt.monotonicMs === lastObservedMono &&
          receipt.monotonicMs <= now() && receipt.monotonicMs >= now() - evidenceMs &&
          ['ownerId', 'ownerName', 'map', 'x', 'y', 'hpStock', 'gold'].every(key => receipt[key] === value[key]);
        if (!reusable) receipt = await refresh();
        guard();
        const selected = options.resolveStagingPoint({ snapshot: client.snapshot, receipt: { ...receipt }, defaultPoint: { ...staging } });
        if (selected && typeof selected.then === 'function') throw new Error('ordinary supplies: staging resolver must be synchronous');
        resolvedStagingPoint = point(selected, 'resolved stagingPoint');
        staging = { ...resolvedStagingPoint }; guard();
      }
      state = 'staging'; await navigate(staging, 0); after = await refresh(); guard();
      after = freshStagingReadiness({ afterSequence: after.sequence - 1 });
      actualStagingReached = true; state = 'ready'; return result();
    } catch (error) {
      state = 'failed'; failure = 'ordinary supplies: underlying ordinary operation failed';
      try { const message=String(error?.message??'');if(message.startsWith('ordinary supplies:'))failure=message; }
      catch { /* Failure reporting cannot replace the original operation error. */ }
      throw error;
    } finally { active = false; queue.length = 0; pendingSaleId = null; }
  }
  return Object.freeze({ prepare, observe, assertReady, status: result, dispose() { active = false; rows.length = 0; queue.length = 0; } });
}
