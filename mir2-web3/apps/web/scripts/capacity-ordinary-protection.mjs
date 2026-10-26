import { readFile } from 'node:fs/promises';
import { createHash } from 'node:crypto';

const PROFILE = 'platinum_176';
const INPUTS = Object.freeze({
  canonical: Object.freeze({ path: 'packages/game-data/data/generated/crystal_respawn_manifest.json',
    url: new URL('../../../packages/game-data/data/generated/crystal_respawn_manifest.json', import.meta.url) }),
  web: Object.freeze({ path: 'apps/web/lib/generated/crystal_respawn_manifest.json',
    url: new URL('../lib/generated/crystal_respawn_manifest.json', import.meta.url) }),
});
const cachedSpecs = new Map();
const trustedSpecs = new WeakSet();
const summaries = new WeakMap();
const int32 = value => Number.isSafeInteger(value) && value >= -0x8000_0000 && value <= 0x7fff_ffff;
const assert = (condition, message) => { if (!condition) throw new Error(`ordinary protection: ${message}`); };

function selectedData(manifest, mapFileName) {
  assert(Array.isArray(manifest?.maps), 'missing map manifest');
  const maps = manifest.maps.filter(map => String(map?.map_file_name) === mapFileName);
  assert(maps.length === 1, 'selected map must exist exactly once');
  const map = maps[0];
  assert(Array.isArray(map.safe_zones) && Array.isArray(map.respawns), 'missing authored safe zones or respawns');
  const safeZones = map.safe_zones.map(zone => {
    assert(int32(zone?.location?.x) && int32(zone?.location?.y) && Number.isInteger(zone.size) && zone.size >= 0 &&
      zone.size <= 0xffff && typeof zone.start_point === 'boolean', 'invalid authored safe zone');
    return { location: { x: zone.location.x, y: zone.location.y }, size: zone.size, start_point: zone.start_point };
  }).sort((a, b) => a.location.x - b.location.x || a.location.y - b.location.y || a.size - b.size || Number(a.start_point) - Number(b.start_point));
  const nameAis = new Map();
  for (const spawn of map.respawns) {
    assert(typeof spawn?.monster_name === 'string' && spawn.monster_name.length > 0 && spawn.monster_name.length <= 128 &&
      Number.isInteger(spawn.monster_ai) && spawn.monster_ai >= 0 && spawn.monster_ai <= 0xff, 'invalid authored monster AI');
    const ais = nameAis.get(spawn.monster_name) ?? new Set();
    ais.add(spawn.monster_ai);
    nameAis.set(spawn.monster_name, ais);
  }
  // A name shared by different AIs cannot establish an AI when the DTO omits it.
  const ai0MonsterNames = [...nameAis].filter(([, ais]) => ais.size === 1 && ais.has(0)).map(([name]) => name).sort();
  return { safeZones, ai0MonsterNames };
}

async function readSpec(mapFileName) {
  const entries = await Promise.all(Object.entries(INPUTS).map(async ([label, input]) => {
    const bytes = await readFile(input.url);
    const selected = selectedData(JSON.parse(bytes.toString('utf8')), mapFileName);
    return { label, selected, evidence: Object.freeze({ path: input.path, bytes: bytes.length,
      sha256: createHash('sha256').update(bytes).digest('hex') }) };
  }));
  assert(JSON.stringify(entries[0].selected) === JSON.stringify(entries[1].selected), 'canonical/web selected map protection differs');
  const { safeZones, ai0MonsterNames } = entries[0].selected;
  for (const zone of safeZones) { Object.freeze(zone.location); Object.freeze(zone); }
  Object.freeze(safeZones);
  Object.freeze(ai0MonsterNames);
  const sourceManifests = Object.freeze(Object.fromEntries(entries.map(entry => [entry.label, entry.evidence])));
  const fields = { runtimeProfileId: PROFILE, mapFileName, safeZones, ai0MonsterNames, sourceManifests,
    selectedMapMatches: true, normalAi0SafeZoneOnly: true, allDamageImmunity: false };
  const spec = Object.freeze({ ...fields });
  trustedSpecs.add(spec);
  summaries.set(spec, Object.freeze({ ...fields }));
  return spec;
}

/** Read fixed, source-bound manifests only. Route profile admission is checked even on a cache hit. */
export async function loadOrdinaryProtection(route, mapFileName = '0') {
  assert(route?.source?.runtimeProfileId === PROFILE, 'requires the platinum_176 route profile');
  assert(typeof mapFileName === 'string' && mapFileName.length > 0 && mapFileName.length <= 128 && mapFileName === mapFileName.trim(), 'invalid map name');
  let pending = cachedSpecs.get(mapFileName);
  if (!pending) {
    pending = readSpec(mapFileName);
    cachedSpecs.set(mapFileName, pending);
  }
  try { return await pending; }
  catch (error) { if (cachedSpecs.get(mapFileName) === pending) cachedSpecs.delete(mapFileName); throw error; }
}

/** Classify an ordinary threat without implying that any particular physical cell is protected. */
export function isOrdinaryAi0Threat(spec, mapName, entity) {
  return trustedSpecs.has(spec) && mapName === spec.mapFileName && entity?.kind === 'monster' &&
    entity.disposition === 'hostile' && entity.ownerName == null &&
    (entity.ai == null || entity.ai === 0) && spec.ai0MonsterNames.includes(entity.name);
}

/** Static protection from normal AI0 attacks only; poison and already released effects remain separate risks. */
export function isProtectedOrdinaryCell(spec, mapName, entity, cell) {
  if (!isOrdinaryAi0Threat(spec, mapName, entity) || !int32(cell?.x) || !int32(cell?.y)) return false;
  // Mirrors Rust runtime/map.rs and zone/runtime.rs: inclusive location +/- size.
  // This does not authorize occupying a monster's physical cell.
  return spec.safeZones.some(zone => cell.x >= zone.location.x - zone.size && cell.x <= zone.location.x + zone.size &&
    cell.y >= zone.location.y - zone.size && cell.y <= zone.location.y + zone.size);
}

/** Evidence contains bounded selected data and input hashes, never the complete manifests. */
export function ordinaryProtectionSummary(spec) {
  return trustedSpecs.has(spec) ? summaries.get(spec) : null;
}
