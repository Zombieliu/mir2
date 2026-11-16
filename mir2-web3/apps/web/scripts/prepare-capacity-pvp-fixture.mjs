// Explicit offline server-only fixture. Never ordinary-player or P0 evidence.
import fs from 'node:fs/promises';
import path from 'node:path';
import crypto from 'node:crypto';
import { pathToFileURL } from 'node:url';
import { loadProtocolCollisionMap, protocolMapCellIsWalkable } from './quest-agent/protocol-navigation.mjs';

const PROJECT = path.resolve(import.meta.dirname, '../../..');
const CHECKOUT = path.dirname(PROJECT);
export const PVP_FIXTURE_COUNTS = Object.freeze([2, 10, 25, 50, 100, 250, 500]);
const requireValue = (condition, message) => { if (!condition) throw new Error(`PvP fixture: ${message}`); };
const digest = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const pointKey = p => `${p.x},${p.y}`;
const distance = (a, b) => Math.max(Math.abs(a.x - b.x), Math.abs(a.y - b.y));
const inside = (file, root) => { const r = path.relative(root, file); return r === '' || !r.startsWith(`..${path.sep}`) && r !== '..' && !path.isAbsolute(r); };
async function readPinned(file) {
  file = path.resolve(file);
  const stat = await fs.lstat(file);
  requireValue(stat.isFile() && !stat.isSymbolicLink(), 'input must be a regular file');
  const bytes = await fs.readFile(file);
  return { bytes, pin: { file, bytes: bytes.length, sha256: digest(bytes) } };
}
export function warriorFortyVitals() {
  const f = Math.fround, level = f(40);
  return { level: 40, hp: Math.trunc(f(f(14) + f(f(f(f(level / f(4)) + f(4.5)) + f(level / f(20))) * level))),
    mp: Math.trunc(f(f(11) + f(level * f(3.5)))),
    bagWeight: Math.trunc(f(f(50) + f(f(level / f(3)) * level))),
    handWeight: Math.trunc(f(f(12) + f(f(level / f(13)) * level))),
    wearWeight: Math.trunc(f(f(15) + f(f(level / f(20)) * level))) };
}
export function pvpFixtureGroups(count) {
  requireValue(PVP_FIXTURE_COUNTS.includes(count), 'count must be one of 2,10,25,50,100,250,500');
  const groups = [];
  for (let index = 0; index < count;) {
    const size = index === 22 || index === 25 ? 3 : 2;
    requireValue(index + size <= count, 'prefix cuts a combat group');
    groups.push({ groupIndex: groups.length, type: size === 3 ? 'triangle' : 'pair', actorIndices: Array.from({ length: size }, (_, n) => index + n) });
    index += size;
  }
  return groups;
}
export function selectPvpFixtureLayout(map, mapInfo, npcs, count) {
  const groups = pvpFixtureGroups(count), candidates = [];
  const excluded = p => !protocolMapCellIsWalkable(map, p) ||
    mapInfo.safe_zones.some(z => distance(p, z.location) <= z.size + 2) ||
    npcs.some(n => distance(p, n.location) <= 2) ||
    mapInfo.movements.some(m => distance(p, m.source) <= 2);
  // Every potential group reserves a clear L, even when only its horizontal pair is used.
  for (let y = 8; y + 1 < map.height - 8; y += 24) for (let x = 8; x + 1 < map.width - 8; x += 24) {
    const cells = [{ x, y }, { x: x + 1, y }, { x, y: y + 1 }];
    if (cells.every(p => !excluded(p))) candidates.push(cells);
  }
  requireValue(candidates.length >= groups.length, 'not enough legal dispersed group cells');
  const actors = [];
  for (const group of groups) {
    const selected = candidates[Math.floor((group.groupIndex + .5) * candidates.length / groups.length)];
    group.actorIndices.forEach((poolIndex, n) => actors.push({ poolIndex, groupIndex: group.groupIndex,
      targetIndex: group.actorIndices[(n + 1) % group.actorIndices.length], position: { ...selected[n] } }));
  }
  requireValue(new Set(actors.map(a => pointKey(a.position))).size === count, 'layout cells repeat');
  for (const actor of actors) requireValue(distance(actor.position, actors[actor.targetIndex].position) === 1, 'target must be adjacent');
  return { actors, groups, staticCandidateGroups: candidates.length, groupLatticeSpacing: 24,
    minimumInterGroupChebyshevDistance: 23, allCellsDistinct: true, outsideSafeZones: true,
    npcAndTransferClearance: 2, dynamicMonsterAbsenceProven: false };
}
function metadata(itemIndex) {
  return { item_index: itemIndex, awake_type: 0, awake_values: [], refined_value: 0, refine_added: 0,
    refine_success_chance: 0, wedding_ring: -1, expire_info: null, rental_information: null, sealed_info: null,
    slots: [], is_shop_item: false, gm_made: false, live_socketed_at_capture: true,
    socket_layout_hydrated: true, captured_socket_positions: [], captured_socket_position: null };
}
export function pvpFixtureItems(templates, hpCount, uidStart) {
  const vitals = warriorFortyVitals(), hp = templates.find(t => t.item_index === 662);
  requireValue(hp?.name === '(HP)DrugLarge' && hp.stack_size === 20 && hp.weight === 3 && hp.required_amount === 0 &&
    hp.stats.some(s => s.stat === 12 && s.value === 110), 'LargeHP canonical contract differs');
  requireValue(Number.isInteger(hpCount) && hpCount >= 1 && hpCount * hp.weight <= vitals.bagWeight, 'potion quantity exceeds legal bag weight');
  let uid = uidStart;
  const equipment = [[221, 'weapon', 'WoodenSword'], [317, 'armour', 'BaseDress(M)'],
    [482, 'braceletLeft', 'SharpBracelet'], [482, 'braceletRight', 'SharpBracelet']].map(([index, slot, name]) => {
    const t = templates.find(t => t.item_index === index);
    requireValue(t?.name === name && t.stack_size === 1 && t.required_type === 0 && t.required_amount <= 40 &&
      (t.required_class & 1) && (t.required_gender & 1), 'equipment canonical requirement differs');
    return { key: `crystal-item-${index}`, slot, quantity: 1, name, icon: t.image, shape: t.shape,
      description: '', durability_current: t.durability, durability_max: t.durability, grade: 'common',
      added_attack: 0, added_defence: 0, added_luck: 0, added_stats: [], socketed: [], cursed: false,
      socket_slots: 0, gem_count: 0, awake_type: 0, awake_values: [], user_item_metadata: metadata(index),
      user_item_unique_id: uid++, identified: true, soul_bound_id: null, sealed_expiry_time_binary_datetime: 0,
      sealed_next_time_binary_datetime: 0, rental_binding_flags: 0, rental_owner_name: '',
      rental_expiry_binary_datetime: 0, rental_locked: false, attack: 0, defence: 0 };
  });
  const inventory = [];
  for (let remaining = hpCount; remaining > 0;) {
    const quantity = Math.min(remaining, hp.stack_size), slot = inventory.length;
    inventory.push({ key: 'crystal-item-662', name: hp.name, icon: hp.image, slot, unique_id: uid++, container: 'bag1',
      quantity, description: '', durability_current: null, durability_max: null, weight: hp.weight, equip_slot: null,
      grade: 'none', added_attack: 0, added_defence: 0, added_stats: [], socketed: [], user_item_metadata: metadata(662),
      cursed: false, socket_slots: 0, gem_count: 0, identified: true, soul_bound_id: null,
      sealed_expiry_time_binary_datetime: 0, sealed_next_time_binary_datetime: 0, rental_binding_flags: 0,
      rental_owner_name: '', rental_expiry_binary_datetime: 0, rental_locked: false, attack: 0, defence: 0, heal_hp: 110, heal_mp: 0 });
    remaining -= quantity;
  }
  requireValue(inventory.length <= 40, 'bag cells exceed base inventory46 minus belt6');
  const handWeight = templates.find(t => t.item_index === 221).weight;
  const wearWeight = templates.find(t => t.item_index === 317).weight + 2 * templates.find(t => t.item_index === 482).weight;
  requireValue(handWeight <= vitals.handWeight && wearWeight <= vitals.wearWeight, 'equipment is overweight');
  return { equipment, inventory, bagWeight: hpCount * hp.weight, handWeight, wearWeight, nextUid: uid };
}
function freshSave(character, position, items, vitals) {
  return { revision: 0, character, map_file_name: '0', map_title: 'BichonProvince', position, direction: 'Down',
    bind_point: null, hp: vitals.hp, max_hp: vitals.hp, mp: vitals.mp, max_mp: vitals.mp,
    experience: 0, max_experience: 100, gold: 0, credit: 0, city_currencies: {}, pk_points: 0,
    chat_banned: false, chat_ban_until_ms: null, inventory_capacity: 46,
    inventory_items_json: items.inventory.map(i => JSON.stringify(i)), belt_items_json: [],
    hero_inventory_items_json: [], hero_equipment_items_json: [], hero_inventory_capacity: null,
    hero_inventory_legacy_40: false, hero_vitals: null, hero_registry_attachment: null,
    storage_items_json: [], equipment_items_json: items.equipment.map(i => JSON.stringify(i)), equipment_items_explicit_empty: false,
    quest_states_json: [], skill_states_json: [], buff_states_json: [], npc_flag_states_json: [], npc_saved_values_json: [],
    npc_buy_back_items_json: [], npc_used_goods_items_json: [], item_rental_records_json: [], has_rented_item: false,
    stage5_systems_json: null, player_kill_receipts: {}, npc_purchase_journal: null, personal_production: null };
}
export async function prepareCapacityPvpFixture(options) {
  const count = Number(options.count), groups = pvpFixtureGroups(count), hpCount = Number(options.hpCount ?? 194);
  const batch = options.batch ?? crypto.randomBytes(4).toString('hex');
  requireValue(/^[a-f0-9]{8}$/.test(batch), 'batch must be eight lowercase hexadecimal characters');
  const endpoint = new URL(options.endpoint ?? 'ws://127.0.0.1:17510/ws');
  requireValue(endpoint.protocol === 'ws:' && ['127.0.0.1', 'localhost', '[::1]'].includes(endpoint.hostname) &&
    !endpoint.username && !endpoint.password && !endpoint.search && !endpoint.hash && endpoint.pathname === '/ws', 'endpoint must be explicit local ws /ws');
  const output = path.resolve(options.outputDirectory);
  const realParent = await fs.realpath(path.dirname(output));
  requireValue(!inside(output, CHECKOUT) && !inside(path.join(realParent, path.basename(output)), await fs.realpath(CHECKOUT)), 'output must be outside repository');
  try { await fs.lstat(output); throw new Error('PvP fixture: output already exists'); } catch (e) { if (e.code !== 'ENOENT') throw e; }
  const originalStore = await readPinned(options.sourceStore), originalPool = await readPinned(options.sourcePool);
  const sourceStore = JSON.parse(originalStore.bytes), sourcePool = JSON.parse(originalPool.bytes);
  requireValue(sourceStore.accounts && !Array.isArray(sourceStore.accounts) && Array.isArray(sourcePool.accounts) && sourcePool.accounts.length > 0, 'invalid source store/pool');
  const credentials = sourcePool.accounts.map(a => {
    const stored = sourceStore.accounts[a.accountId];
    requireValue(stored && typeof stored.password === 'string' && stored.password.length > 0 &&
      typeof a.password === 'string' && a.password.length >= 12 && a.password.length <= 20, 'source credential template missing');
    return { hash: stored.password, password: a.password };
  });
  const itemFile = path.join(PROJECT, 'packages/game-data/data/generated/crystal_item_manifest.json');
  const respawnFile = path.join(PROJECT, 'packages/game-data/data/generated/crystal_respawn_manifest.json');
  const npcFile = path.join(PROJECT, 'packages/game-data/data/generated/crystal_npc_info_manifest.json');
  const [itemSource, respawnSource, npcSource] = await Promise.all([readPinned(itemFile), readPinned(respawnFile), readPinned(npcFile)]);
  const templates = JSON.parse(itemSource.bytes).items, mapInfo = JSON.parse(respawnSource.bytes).maps.find(m => m.map_file_name === '0');
  const npcs = JSON.parse(npcSource.bytes).npcs.filter(n => String(n.map_file_name) === '0');
  requireValue(mapInfo && !mapInfo.no_fight, 'Map0 combat contract differs');
  const map = await loadProtocolCollisionMap('0');
  const mapSource = await readPinned(map.sourcePath), layout = selectPvpFixtureLayout(map, mapInfo, npcs, count), vitals = warriorFortyVitals();
  const uidBase = crypto.randomBytes(5).readUIntBE(0, 5) + 1;
  const store = { schemaVersion: sourceStore.schemaVersion, nextCharacterIndex: count + 1,
    gameShopGlobalPurchases: {}, sharedGuilds: {}, sharedConquests: {}, sharedHeroes: {}, heroIdHighWatermark: 0, accounts: {} };
  const pool = { schemaVersion: 1, purpose: 'isolated-load-actors', realm: 'isolated', endpoint: endpoint.href, batch,
    fixtureScope: 'server-only-preconfigured-pvp', ordinaryEvidence: false, accounts: [] };
  const actors = layout.actors.map(actor => {
    const i = actor.poolIndex, name = `L${batch.slice(0, 5)}${i.toString(36).padStart(3, '0')}`, accountId = `lt${batch}${i.toString(36).padStart(3, '0')}`;
    const character = { index: i + 1, name, level: 40, class: 'Warrior', gender: 'Male' };
    requireValue(!sourceStore.accounts[accountId] && !sourcePool.accounts.some(a => a.name === name), 'fixture identity collides with source');
    const items = pvpFixtureItems(templates, hpCount, uidBase + i * 64), credential = credentials[i % credentials.length];
    store.accounts[accountId] = { password: credential.hash, storage_size: 80, has_expanded_storage: false,
      expanded_storage_expiry_time_binary_datetime: 0, storage_password: '', storage_password_last_set_binary_datetime: 0,
      is_banned: false, ban_reason: '', ban_until_ms: null, banned_at_ms: null, gm_level: 0,
      character_last_access_binary_datetimes: {}, characters: [character], saves: { [character.index]: freshSave(character, actor.position, items, vitals) } };
    pool.accounts.push({ accountId, name, password: credential.password, state: 'ready', characterIndex: character.index,
      provisioning: { source: 'offline-server-only-pvp-fixture', preseeded: true, ordinaryEvidence: false } });
    return { ...actor, name, characterIndex: character.index };
  });
  // All validation precedes creation. Existing fixtures and the source bank are never overwritten.
  await fs.mkdir(output);
  const privateDirectory = path.join(output, 'private'), publicDirectory = path.join(output, 'public');
  await fs.mkdir(privateDirectory, { mode: 0o700 }); await fs.mkdir(publicDirectory);
  const storeFile = path.join(privateDirectory, 'accounts.json'), poolFile = path.join(privateDirectory, 'capacity-pool.json');
  await fs.writeFile(storeFile, JSON.stringify(store), { flag: 'wx', mode: 0o600 });
  await fs.writeFile(poolFile, JSON.stringify(pool), { flag: 'wx', mode: 0o600 });
  const [actualStore, actualPool, sourceStoreAfter, sourcePoolAfter] = await Promise.all([readPinned(storeFile), readPinned(poolFile), readPinned(options.sourceStore), readPinned(options.sourcePool)]);
  requireValue(sourceStoreAfter.pin.sha256 === originalStore.pin.sha256 && sourcePoolAfter.pin.sha256 === originalPool.pin.sha256, 'source changed while preparing');
  const manifest = { schemaVersion: 1, scope: 'server-only-preconfigured-pvp', preseeded: true, count, mapFileName: '0',
    endpoint: endpoint.href, batch, actors, groups, layout: { ...layout, actors: undefined, groups: undefined }, vitals,
    equipment: { templates: [221, 317, 482, 482], bonusAccuracy: 4, noAddedStats: true, noImmortality: true },
    hpItem: { templateIndex: 662, name: '(HP)DrugLarge', grid: 'Inventory', firstSlot: 0, perActor: hpCount,
      heal: 110, stackSize: 20, weight: 3, bagWeight: hpCount * 3, bagWeightLimit: vitals.bagWeight, inventoryCapacity: 46 },
    supply: { preconfiguredHealingPerActor: hpCount * 110, theoreticalTwoHourAverageHealingPerSecond: hpCount * 110 / 7200,
      sustainableTwoHoursProven: false, normalPotionTimingUnchanged: true, forceHeal: false },
    source: { store: originalStore.pin, pool: originalPool.pin, unchanged: true, credentialsCopiedOnlyIntoPrivateFiles: true },
    canonicalInputs: [itemSource.pin, respawnSource.pin, npcSource.pin, mapSource.pin],
    privateArtifacts: { store: actualStore.pin, pool: actualPool.pin },
    auth: { requiredGatewayProfile: 'development-loopback-600-v1', profileActivatedByBuilder: false,
      ordinaryLedgerModified: false, oldSecretsOrRecoveryCopied: false },
    requiresGatewayProfile: { developmentLoopbackAuthBudget: 600, authWindowSeconds: 900 },
    ordinaryEvidence: false, p0Accepted: false, capacityAccepted: false, combatAccepted: false,
    dynamicOccupancyProven: false, liveExecutionPerformed: false };
  const manifestFile = path.join(publicDirectory, 'fixture-manifest-v1.json');
  await fs.writeFile(manifestFile, JSON.stringify(manifest, null, 2) + '\n', { flag: 'wx' });
  return { manifestFile, storeFile, poolFile, manifest };
}
if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
  const options = {};
  const names = { 'source-store': 'sourceStore', 'source-pool': 'sourcePool', 'output-directory': 'outputDirectory', count: 'count', endpoint: 'endpoint', 'hp-count': 'hpCount', batch: 'batch' };
  for (const argument of process.argv.slice(2)) {
    const match = /^--([^=]+)=(.+)$/.exec(argument);
    requireValue(match && names[match[1]], 'use --source-store= --source-pool= --output-directory= --count= [--endpoint= --hp-count= --batch=]');
    options[names[match[1]]] = match[2];
  }
  prepareCapacityPvpFixture(options).then(result => console.log(JSON.stringify({ manifestFile: result.manifestFile,
    count: result.manifest.count, groupCount: result.manifest.groups.length, preseeded: true, ordinaryEvidence: false,
    capacityAccepted: false, p0Accepted: false }))).catch(error => { console.error(error.message); process.exitCode = 1; });
}
