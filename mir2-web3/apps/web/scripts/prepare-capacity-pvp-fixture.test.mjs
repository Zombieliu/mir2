import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import path from 'node:path';
import os from 'node:os';
import crypto from 'node:crypto';
import { warriorFortyVitals, pvpFixtureGroups, selectPvpFixtureLayout, pvpFixtureItems,
  prepareCapacityPvpFixture, PVP_FIXTURE_COUNTS } from './prepare-capacity-pvp-fixture.mjs';
import { loadProtocolCollisionMap } from './quest-agent/protocol-navigation.mjs';

const PROJECT = path.resolve(import.meta.dirname, '../../..');
const sha = b => crypto.createHash('sha256').update(b).digest('hex');
const items = JSON.parse(await fs.readFile(path.join(PROJECT, 'packages/game-data/data/generated/crystal_item_manifest.json'), 'utf8')).items;
const mapInfo = JSON.parse(await fs.readFile(path.join(PROJECT, 'packages/game-data/data/generated/crystal_respawn_manifest.json'), 'utf8')).maps.find(m => m.map_file_name === '0');
const npcs = JSON.parse(await fs.readFile(path.join(PROJECT, 'packages/game-data/data/generated/crystal_npc_info_manifest.json'), 'utf8')).npcs.filter(n => String(n.map_file_name) === '0');
const distance = (a, b) => Math.max(Math.abs(a.x - b.x), Math.abs(a.y - b.y));

test('PvP fixture uses the actual Warrior40 f32 vitals and legal canonical equipment and HP stacks', () => {
  assert.deepEqual(warriorFortyVitals(), { level: 40, hp: 674, mp: 151, bagWeight: 583, handWeight: 135, wearWeight: 95 });
  const state = pvpFixtureItems(items, 194, 1000);
  assert.equal(state.bagWeight, 582);
  assert.equal(state.inventory.length, 10);
  assert.deepEqual(state.inventory.map(i => i.quantity), [20, 20, 20, 20, 20, 20, 20, 20, 20, 14]);
  assert.deepEqual(state.equipment.map(i => [i.key, i.slot]), [['crystal-item-221', 'weapon'],
    ['crystal-item-317', 'armour'], ['crystal-item-482', 'braceletLeft'], ['crystal-item-482', 'braceletRight']]);
  assert.equal(state.handWeight, 4); assert.equal(state.wearWeight, 7);
  assert(state.inventory.every(i => i.heal_hp === 110 && i.quantity <= 20 && i.container === 'bag1' && i.user_item_metadata.gm_made === false));
  const uids = [...state.equipment.map(i => i.user_item_unique_id), ...state.inventory.map(i => i.unique_id)];
  assert.equal(new Set(uids).size, 14);
  assert.throws(() => pvpFixtureItems(items, 195, 1), /legal bag weight/);
  assert.throws(() => pvpFixtureItems(items, 0, 1), /legal bag weight/);
  const altered = structuredClone(items); altered.find(i => i.item_index === 662).stats[0].value = 9999;
  assert.throws(() => pvpFixtureItems(altered, 194, 1), /canonical contract/);
});

test('PvP fixture exact25 and every frozen prefix close all pairs and both adjacent target cycles', async () => {
  const map = await loadProtocolCollisionMap('0');
  for (const count of PVP_FIXTURE_COUNTS) {
    const layout = selectPvpFixtureLayout(map, mapInfo, npcs, count);
    assert.equal(layout.actors.length, count);
    assert.equal(new Set(layout.actors.map(a => `${a.position.x},${a.position.y}`)).size, count);
    assert(layout.staticCandidateGroups >= 249);
    const incoming = new Map();
    for (const actor of layout.actors) {
      assert.equal(distance(actor.position, layout.actors[actor.targetIndex].position), 1);
      incoming.set(actor.targetIndex, (incoming.get(actor.targetIndex) ?? 0) + 1);
      assert(!mapInfo.safe_zones.some(z => distance(actor.position, z.location) <= z.size + 2));
      assert(!npcs.some(n => distance(actor.position, n.location) <= 2));
      assert(!mapInfo.movements.some(m => distance(actor.position, m.source) <= 2));
    }
    assert.equal(incoming.size, count); assert([...incoming.values()].every(n => n === 1));
    if (count >= 25) assert.deepEqual(layout.groups.find(g => g.actorIndices[0] === 22).actorIndices, [22, 23, 24]);
    if (count >= 50) assert.deepEqual(layout.groups.find(g => g.actorIndices[0] === 25).actorIndices, [25, 26, 27]);
    if (count === 500) {
      assert.equal(layout.groups.filter(g => g.type === 'pair').length, 247);
      assert.equal(layout.groups.filter(g => g.type === 'triangle').length, 2);
    }
  }
  for (const bad of [0, 1, 3, 24, 26, 501, NaN]) assert.throws(() => pvpFixtureGroups(bad), /count must/);
  assert.throws(() => selectPvpFixtureLayout({ width: 16, height: 16, blocked: new Uint8Array(256) }, mapInfo, [], 500), /not enough/);
});

async function privateFixture(t) {
  const directory = await fs.mkdtemp(path.join(os.tmpdir(), 'mir2-pvp-fixture-test-'));
  t.after(async () => {
    assert.equal(path.dirname(directory), os.tmpdir());
    assert(path.basename(directory).startsWith('mir2-pvp-fixture-test-'));
    await fs.rm(directory, { recursive: true, force: true });
  });
  const sourceStore = path.join(directory, 'source-accounts.json'), sourcePool = path.join(directory, 'source-pool.json');
  // Opaque credential placeholders test byte copying only, never live authentication.
  const password = 'private-fixture-password', hash = '$argon2id$opaque-unit-fixture';
  const store = { schemaVersion: 1, nextCharacterIndex: 99, sharedGuilds: { mustNotInherit: true },
    accounts: { ltsource000: { password: hash, gm_level: 10, storage_password: 'must-not-inherit',
      activities: { unsafe: true }, characters: [{ index: 98, name: 'OldName' }], saves: { 98: { gold: 999999,
        npc_purchase_journal: { unsafe: true }, player_kill_receipts: { unsafe: true }, stage5_systems_json: '{"mail":["unsafe"]}' } } } } };
  const pool = { accounts: [{ accountId: 'ltsource000', password, name: 'OldName' }] };
  // Normal pool plaintext limits are12..20; the unit marker is deliberately a legal20-byte string.
  pool.accounts[0].password = 'fixture-private-pass';
  await fs.writeFile(sourceStore, JSON.stringify(store)); await fs.writeFile(sourcePool, JSON.stringify(pool));
  return { directory, sourceStore, sourcePool, hash, password: pool.accounts[0].password,
    outputDirectory: path.join(directory, 'new-fixture'), count: 2, endpoint: 'ws://127.0.0.1:17510/ws', batch: '1a2b3c4d' };
}

test('PvP fixture writes only a fresh private bank and public scalar manifest, preserving source bytes', async t => {
  const options = await privateFixture(t);
  const before = await Promise.all([fs.readFile(options.sourceStore), fs.readFile(options.sourcePool)]);
  const result = await prepareCapacityPvpFixture(options);
  const after = await Promise.all([fs.readFile(options.sourceStore), fs.readFile(options.sourcePool)]);
  assert.deepEqual(after.map(sha), before.map(sha));
  const store = JSON.parse(await fs.readFile(result.storeFile)), pool = JSON.parse(await fs.readFile(result.poolFile));
  assert.equal(Object.keys(store.accounts).length, 2); assert.equal(store.nextCharacterIndex, 3);
  assert.deepEqual(store.sharedGuilds, {}); assert.equal(store.sealedStones, undefined); assert.equal(store.guildClock, undefined);
  const uids = [];
  for (const actor of result.manifest.actors) {
    const p = pool.accounts[actor.poolIndex], account = store.accounts[p.accountId], save = account.saves[p.characterIndex];
    assert.match(p.accountId, /^lt[a-z0-9]{8,16}$/); assert.match(p.name, /^L[a-z0-9]{8}$/);
    assert.equal(p.password, options.password); assert.equal(account.password, options.hash);
    assert.equal(account.gm_level, 0); assert.equal(account.activities, undefined); assert.equal(account.storage_password, '');
    assert.deepEqual(save.position, actor.position); assert.equal(save.character.index, actor.characterIndex);
    assert.equal(save.hp, 674); assert.equal(save.mp, 151); assert.equal(save.gold, 0); assert.equal(save.stage5_systems_json, null);
    assert.deepEqual(save.player_kill_receipts, {}); assert.equal(save.npc_purchase_journal, null);
    assert.equal(save.inventory_items_json.map(JSON.parse).reduce((s, i) => s + i.quantity, 0), 194);
    assert.deepEqual(save.buff_states_json, []); assert.deepEqual(save.npc_used_goods_items_json, []);
    uids.push(...save.inventory_items_json.map(JSON.parse).map(i => i.unique_id), ...save.equipment_items_json.map(JSON.parse).map(i => i.user_item_unique_id));
  }
  assert.equal(new Set(uids).size, 28);
  const publicText = await fs.readFile(result.manifestFile, 'utf8');
  assert(!publicText.includes(options.password)); assert(!publicText.includes(options.hash)); assert(!publicText.includes('must-not-inherit'));
  assert.equal(result.manifest.ordinaryEvidence, false); assert.equal(result.manifest.capacityAccepted, false);
  assert.equal(result.manifest.p0Accepted, false); assert.equal(result.manifest.supply.sustainableTwoHoursProven, false);
  assert.equal(result.manifest.auth.profileActivatedByBuilder, false);
  assert.deepEqual(result.manifest.requiresGatewayProfile, { developmentLoopbackAuthBudget: 600, authWindowSeconds: 900 });
  assert.deepEqual((await fs.readdir(path.dirname(result.storeFile))).sort(), ['accounts.json', 'capacity-pool.json']);
  await assert.rejects(prepareCapacityPvpFixture(options), /output already exists/);
});

test('PvP fixture rejects illegal supplies, remote endpoints and repository output without creating files', async t => {
  const options = await privateFixture(t);
  for (const override of [{ hpCount: 195 }, { endpoint: 'ws://example.com/ws' },
    { outputDirectory: path.join(PROJECT, 'must-not-create-pvp-fixture') }]) {
    await assert.rejects(prepareCapacityPvpFixture({ ...options, ...override }));
    await assert.rejects(fs.access(options.outputDirectory), { code: 'ENOENT' });
  }
});
