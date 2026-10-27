import { protocolMapCellIsWalkable } from './quest-agent/protocol-navigation.mjs';

const assert = (ok, message) => { if (!ok) throw new Error(message); };
export const PREPARATION_STAGING_CANDIDATE_POLICY = 'fresh-unoccupied-distinct-saved-v1';

// This is a planning filter over a frozen ordinary staging catalogue. It never
// moves an entity, changes occupancy or treats offline saved points as Zone state.
export function selectPreparationStagingPoint({ snapshot, receipt, defaultPoint, candidates, savedPoints }) {
  const owner = snapshot?.entities?.find(e => e.objectId === snapshot.playerObjectId);
  assert(owner && receipt?.ownerId === snapshot.playerObjectId && receipt.ownerName === owner.name &&
    receipt.map === '0' && String(snapshot.mapFileName) === '0' && Number.isSafeInteger(owner.x) && Number.isSafeInteger(owner.y) &&
    receipt.x === owner.x && receipt.y === owner.y && Number.isSafeInteger(receipt.sequence) && receipt.sequence > 0 &&
    Number.isFinite(receipt.monotonicMs), 'Fresh same-owner map0 staging snapshot required');
  const occupied = new Set();
  for (const entity of snapshot.entities ?? []) {
    if (entity.objectId === snapshot.playerObjectId || entity.dead === true) continue;
    if (['player', 'selfplayer', 'remoteplayer', 'monster', 'npc'].includes(String(entity.kind).toLowerCase()) &&
        Number.isSafeInteger(entity.x) && Number.isSafeInteger(entity.y)) occupied.add(entity.x + ',' + entity.y);
  }
  const declared = new Set(candidates.map(p => p.x + ',' + p.y));
  const order = [defaultPoint, ...candidates];
  for (const point of order) {
    const key = point.x + ',' + point.y;
    if (!declared.has(key) || savedPoints.has(key) || occupied.has(key) ||
        Math.max(Math.abs(owner.x - point.x), Math.abs(owner.y - point.y)) > 16 ||
        (snapshot.mapTransfers ?? []).some(t => t.bounds && point.x >= t.bounds.minX && point.x <= t.bounds.maxX &&
          point.y >= t.bounds.minY && point.y <= t.bounds.maxY)) continue;
    return { x: point.x, y: point.y };
  }
  throw new Error('No fresh visible unoccupied distinct declared staging candidate');
}


// Shape validation runs before admission. The current map/transfer checks run
// once the ordinary owner has actually entered map0, before the late choice.
export function preparationStagingCatalogue(preparation, points, map, mapTransfers = []) {
  const candidates = preparation.stagingCandidates;
  if (candidates === undefined && preparation.candidatePolicy === undefined) return null;
  assert(preparation.candidatePolicy === PREPARATION_STAGING_CANDIDATE_POLICY &&
    Array.isArray(candidates) && candidates.length === 335,
    'Explicit frozen335 staging candidates and policy are required');
  assert(candidates.every(p => Number.isSafeInteger(p?.x) && Number.isSafeInteger(p?.y) &&
    p.x >= 0 && p.y >= 0 && p.x <= 65535 && p.y <= 65535 &&
    (p.mapFileName == null || String(p.mapFileName) === '0')),
    'Staging candidates must be ordinary map0 cells');
  const declared = new Set(candidates.map(p => p.x + ',' + p.y));
  assert(declared.size === 335 && points.every(p => declared.has(p.x + ',' + p.y)),
    'Preferred points must be contained in the complete distinct frozen catalogue');
  if (map !== undefined) {
    assert(map?.mapFileName === '0' && candidates.every(p => protocolMapCellIsWalkable(map, p)) &&
      candidates.every(p => !mapTransfers.some(t => t.bounds && p.x >= t.bounds.minX && p.x <= t.bounds.maxX &&
        p.y >= t.bounds.minY && p.y <= t.bounds.maxY)),
      'Staging candidates must be ordinary walkable current map0 cells without a transfer');
  }
  return Object.freeze(candidates.map(({ x, y }) => Object.freeze({ x, y })));
}
