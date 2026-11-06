// Pure ordinary entrance selection. No packets, wall edits or authority writes.
import { findProtocolWalkPath } from './vendor/protocol-navigation.mjs';
import { self, dist, liveBlockers } from './dense_observation.mjs';

const point = p => Number.isSafeInteger(p?.x) && Number.isSafeInteger(p?.y) && p.x >= 0 && p.y >= 0;
const samePoint = (a, b) => point(a) && point(b) && a.x === b.x && a.y === b.y;
const signature = t => JSON.stringify([t?.map_index, t?.source?.x, t?.source?.y, t?.destination?.x,
  t?.destination?.y, t?.need_hole, t?.need_move, t?.conquest_index, t?.show_on_big_map, t?.icon, t?.targetMap]);
function liveCanonical(snapshot, metadata, proposed, map) {
  const sourceMap = snapshot?.mapFileName, ownerId = snapshot?.playerObjectId, me = self(snapshot);
  if (typeof sourceMap !== 'string' || map?.mapFileName !== sourceMap || snapshot?.mapSnapshotPending === true
    || !Number.isSafeInteger(ownerId) || ownerId <= 0 || !me || me.dead === true || !point(me)
    || !(snapshot.playerHp > 0) || !Array.isArray(snapshot.mapTransfers)) return null;
  const source = metadata?.maps?.find(m => m.map === sourceMap);
  const candidate = source?.transfers?.find(t => signature(t) === signature(proposed));
  if (!candidate || candidate.need_hole !== false || candidate.need_move !== false
    || candidate.conquest_index !== 0 || !Number.isSafeInteger(candidate.map_index) || candidate.map_index < 0
    || !point(candidate.source) || !point(candidate.destination) || typeof candidate.targetMap !== 'string'
    || candidate.source.x >= map.width || candidate.source.y >= map.height) return null;
  const expectedKey = `crystal-move:${sourceMap}:${candidate.source.x}:${candidate.source.y}:${candidate.map_index}:${candidate.destination.x}:${candidate.destination.y}`;
  const live = snapshot.mapTransfers.find(t => t.key === expectedKey && t.mapFileName === sourceMap
    && t.toMapFileName === candidate.targetMap && samePoint(t.toPosition, candidate.destination)
    && t.bounds?.minX === candidate.source.x && t.bounds?.maxX === candidate.source.x
    && t.bounds?.minY === candidate.source.y && t.bounds?.maxY === candidate.source.y);
  return live ? { candidate, sourceMap, ownerId, liveKey: expectedKey } : null;
}

export function assertOrdinaryTransferCurrent(snapshot, metadata, selection, map) {
  const valid = liveCanonical(snapshot, metadata, selection, map);
  if (!valid || selection.sourceMap !== valid.sourceMap || selection.ownerId !== valid.ownerId
    || selection.liveKey !== valid.liveKey) throw new Error('Stale or non-source-verified ordinary entrance; override refused');
  return valid.candidate;
}

export function selectOrdinaryTransfer({ snapshot, metadata, targetMap, map }) {
  const source = metadata?.maps?.find(m => m.map === snapshot?.mapFileName), me = self(snapshot);
  const choices = (source?.transfers ?? []).filter(t => t.targetMap === targetMap)
    .map(t => structuredClone(t));
  if (me) choices.sort((a, b) => dist(me, a.source) - dist(me, b.source));
  const attempts = [];
  for (const proposed of choices) {
    const valid = liveCanonical(snapshot, metadata, proposed, map);
    if (!valid) { attempts.push({ source: proposed.source, reason: 'not-current-source-verified-ordinary-entrance' }); continue; }
    const route = findProtocolWalkPath({ map, start: me, target: valid.candidate.source,
      dynamicObstacles: liveBlockers(snapshot), staticWalkableOverrides: [valid.candidate.source], maxExpanded: 120000 });
    if (!route || route.length < 2) { attempts.push({ source: proposed.source, reason: 'no-legal-static-occupancy-path' }); continue; }
    attempts.push({ source: proposed.source, reason: 'ordinary-route-found', routeNodes: route.length });
    return { selection: { ...structuredClone(valid.candidate), sourceMap: valid.sourceMap, ownerId: valid.ownerId,
      liveKey: valid.liveKey }, route, attempts };
  }
  return { selection: null, route: null, attempts };
}
