import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import { certifyOrdinaryAi0Packet, isProtectedOrdinaryPacketCell, loadOrdinaryProtection, isOrdinaryAi0Threat, isProtectedOrdinaryCell, ordinaryProtectionSummary } from './capacity-ordinary-protection.mjs';

const route = { source: { runtimeProfileId: 'platinum_176' } };
const spec = await loadOrdinaryProtection(route);
const monster = { kind: 'monster', name: 'Scarecrow', disposition: 'hostile', ownerName: null, ai: 0 };
const inside = { x: 288, y: 616 };
const protectedCell = (entity = monster, cell = inside) => isProtectedOrdinaryCell(spec, '0', entity, cell);

test('real canonical and web sources independently match the selected safe zones and unambiguous AI0 names', async () => {
  const summary = ordinaryProtectionSummary(spec);
  const projections = [];
  for (const [label, relative] of [
    ['canonical', '../../../packages/game-data/data/generated/crystal_respawn_manifest.json'],
    ['web', '../lib/generated/crystal_respawn_manifest.json'],
  ]) {
    const raw = await readFile(new URL(relative, import.meta.url));
    const manifest = JSON.parse(raw);
    const maps = manifest.maps.filter(map => String(map.map_file_name) === '0');
    assert.equal(maps.length, 1);
    const selected = maps[0];
    const ai0 = [...new Set(selected.respawns.filter(spawn => spawn.monster_ai === 0).map(spawn => spawn.monster_name))].sort();
    for (const name of ai0) assert.ok(selected.respawns.filter(spawn => spawn.monster_name === name).every(spawn => spawn.monster_ai === 0));
    const zones = selected.safe_zones.toSorted((a, b) => a.location.x - b.location.x || a.location.y - b.location.y || a.size - b.size || Number(a.start_point) - Number(b.start_point));
    assert.deepEqual(summary.safeZones, zones);
    assert.deepEqual(summary.ai0MonsterNames, ai0);
    assert.equal(summary.sourceManifests[label].bytes, raw.length);
    assert.equal(summary.sourceManifests[label].sha256, createHash('sha256').update(raw).digest('hex'));
    projections.push({ zones, ai0 });
  }
  assert.deepEqual(projections[0], projections[1]);
  assert.deepEqual(summary.ai0MonsterNames, ['ForestYeti', 'HookingCat', 'Oma', 'RakingCat', 'Scarecrow', 'Yob']);
  assert.ok(summary.safeZones.some(zone => zone.location.x === 288 && zone.location.y === 616 && zone.size === 10 && zone.start_point));
  assert.equal(summary.runtimeProfileId, 'platinum_176');
  assert.equal(summary.mapFileName, '0');
  assert.equal(summary.selectedMapMatches, true);
  assert.equal(summary.normalAi0SafeZoneOnly, true);
  assert.equal(summary.allDamageImmunity, false);
});

test('cache retains only immutable trusted selected data and rechecks profile admission', async () => {
  assert.equal(await loadOrdinaryProtection(route, '0'), spec);
  for (const value of [spec, spec.safeZones, spec.safeZones[0], spec.safeZones[0].location, spec.ai0MonsterNames,
    spec.sourceManifests, spec.sourceManifests.canonical, ordinaryProtectionSummary(spec)]) assert.equal(Object.isFrozen(value), true);
  assert.throws(() => { spec.safeZones[0].size = 999; }, TypeError);
  assert.throws(() => { spec.ai0MonsterNames.push('Deer'); }, TypeError);
  for (const wrong of [null, {}, { source: {} }, { source: { runtimeProfileId: 'Platinum_176' } }, { source: { runtimeProfileId: 'platinum_176 ' } }]) {
    await assert.rejects(loadOrdinaryProtection(wrong, '0'), /requires the platinum_176 route profile/);
  }
  assert.equal('maps' in spec, false);
  assert.equal('respawns' in spec, false);
});

test('missing or noncanonical selected maps fail instead of borrowing Map0 protection', async () => {
  await assert.rejects(loadOrdinaryProtection(route, '__missing_map__'), /selected map must exist exactly once/);
  for (const map of [null, 0, '', ' 0', '0 ']) await assert.rejects(loadOrdinaryProtection(route, map), /invalid map name/);
  assert.equal(await loadOrdinaryProtection(route, '0'), spec);
});

for (const [name, point] of [
  ['west edge', { x: 278, y: 616 }], ['east edge', { x: 298, y: 616 }],
  ['north edge', { x: 288, y: 606 }], ['south edge', { x: 288, y: 626 }],
  ['northwest corner', { x: 278, y: 606 }], ['northeast corner', { x: 298, y: 606 }],
  ['southwest corner', { x: 278, y: 626 }], ['southeast corner', { x: 298, y: 626 }],
]) test(`village inclusive rectangle protects ${name}`, () => assert.equal(protectedCell(monster, point), true));

for (const [name, point] of [
  ['west', { x: 277, y: 616 }], ['east', { x: 299, y: 616 }],
  ['north', { x: 288, y: 605 }], ['south', { x: 288, y: 627 }],
]) test(`one physical cell outside village ${name} remains unprotected`, () => assert.equal(protectedCell(monster, point), false));

test('every actual physical cell must independently satisfy the geometry', () => {
  const run = [{ x: 298, y: 616 }, { x: 299, y: 616 }];
  assert.deepEqual(run.map(cell => protectedCell(monster, cell)), [true, false]);
  assert.equal(protectedCell(monster, { x: 650, y: 629 }), true, 'other actual Map0 authored safe zone');
  for (const cell of [null, {}, { x: '288', y: 616 }, { x: 288.5, y: 616 }, { x: 288, y: Infinity }]) assert.equal(protectedCell(monster, cell), false);
});

test('only trusted loader specs on the exact selected map can protect', () => {
  for (const fake of [null, {}, { ...spec }, JSON.parse(JSON.stringify(spec)), ordinaryProtectionSummary(spec)]) {
    assert.equal(isProtectedOrdinaryCell(fake, '0', monster, inside), false);
    assert.equal(ordinaryProtectionSummary(fake), null);
  }
  for (const map of ['1', 0, null, '00']) assert.equal(isProtectedOrdinaryCell(spec, map, monster, inside), false);
});

test('known normal AI0 names allow absent optional AI but reject conflicting or malformed actual AI', () => {
  for (const name of spec.ai0MonsterNames) {
    assert.equal(protectedCell({ ...monster, name }), true);
    assert.equal(protectedCell({ ...monster, name, ai: null }), true);
    const withoutAi = { ...monster, name }; delete withoutAi.ai;
    assert.equal(protectedCell(withoutAi), true);
  }
  for (const ai of [1, 16, '0', false, NaN, {}]) assert.equal(protectedCell({ ...monster, ai }), false);
  for (const name of ['Deer', 'VampireSpider', 'scarecrow', 'Scarecrow ', '', null]) assert.equal(protectedCell({ ...monster, name }), false);
});

test('only canonical hostile unowned monster DTOs qualify', () => {
  for (const disposition of [undefined, null, 'Hostile', 'friendly', 'neutral', 'unknown']) assert.equal(protectedCell({ ...monster, disposition }), false);
  for (const kind of [undefined, null, 'Monster', 'player', 'npc']) assert.equal(protectedCell({ ...monster, kind }), false);
  for (const ownerName of ['owner', '', ' ', 0, false, {}]) assert.equal(protectedCell({ ...monster, ownerName }), false);
  const unowned = { ...monster }; delete unowned.ownerName;
  assert.equal(protectedCell(unowned), true);
  assert.equal(protectedCell(null), false);
});

test('ordinary AI0 classification authenticates all identity gates without granting cell protection', () => {
  assert.equal(isOrdinaryAi0Threat(spec, '0', { ...monster, x: 288, y: 600 }), true);
  assert.equal(protectedCell(monster, { x: 288, y: 600 }), false);
  for (const fake of [null, {}, { ...spec }, ordinaryProtectionSummary(spec)]) assert.equal(isOrdinaryAi0Threat(fake, '0', monster), false);
  for (const map of ['1', 0, null]) assert.equal(isOrdinaryAi0Threat(spec, map, monster), false);
  for (const changed of [{ disposition: null }, { disposition: 'Hostile' }, { kind: 'player' }, { name: 'Deer' },
    { ownerName: 'owner' }, { ownerName: '' }, { ai: 1 }, { ai: '0' }]) assert.equal(isOrdinaryAi0Threat(spec, '0', { ...monster, ...changed }), false);
  assert.equal(isOrdinaryAi0Threat(spec, '0', null), false);
  assert.equal(isOrdinaryAi0Threat(spec, '0', { ...monster, ai: null }), true);
});

const wire = payload => ({type:'packet',packet:'ObjectMonster',payload:{objectId:206641,name:'Scarecrow',location:{x:287,y:616},ai:0,masterObjectId:0,dead:false,...payload}});
test('actual normal unowned ObjectMonster carries a separate immutable planner certificate withoutinventing health or disposition',()=>{
  const message=wire(),certificate=certifyOrdinaryAi0Packet(spec,'0',message);
  assert.ok(certificate);assert.equal(Object.isFrozen(certificate),true);assert.equal(certificate.plannerOnly,true);
  assert.equal(Object.hasOwn(certificate,'hp'),false);assert.equal(Object.hasOwn(certificate,'disposition'),false);
  assert.equal(Object.hasOwn(message.payload,'hp'),false);assert.equal(Object.hasOwn(message.payload,'disposition'),false);
  assert.equal(isProtectedOrdinaryPacketCell(spec,'0',certificate,inside),true);
  assert.equal(isProtectedOrdinaryPacketCell(spec,'0',certificate,{x:299,y:616}),false);
  assert.equal(isProtectedOrdinaryCell(spec,'0',message.payload,inside),false,'wire identity never masquerades as a raw entity');
});
test('wire certification rejects missing malformed owned neutral or ambiguous identities and catalog replies',()=>{
  for(const change of [{objectId:0},{objectId:'1'},{objectId:-1},{objectId:0x1_0000_0000},{objectId:1.5},
    {ai:undefined},{ai:null},{ai:'0'},{ai:6},{ai:58},{ai:113},{masterObjectId:undefined},{masterObjectId:'0'},
    {masterObjectId:9},{dead:undefined},{dead:'false'},{dead:0},{dead:true},{name:'Deer'},{name:'UnknownMonster'},
    {ownerName:''},{ownerName:'owner'},{class:'Warrior'},{kind:'npc'},{disposition:'neutral'},{disposition:'friendly'},
    {location:undefined},{location:{x:'287',y:616}},{location:{x:287,y:Infinity}},{location:{x:287.5,y:616}},
    {location:{x:-1,y:616}},{location:{x:287,y:65536}},{hp:0},{hp:'20'}])assert.equal(certifyOrdinaryAi0Packet(spec,'0',wire(change)),null,JSON.stringify(change));
  for(const packet of ['NewMonsterInfo','ObjectPlayer','ObjectWalk'])assert.equal(certifyOrdinaryAi0Packet(spec,'0',{...wire(),packet}),null);
  assert.equal(certifyOrdinaryAi0Packet(spec,'0',{...wire(),type:'worldSnapshot'}),null);
  assert.equal(certifyOrdinaryAi0Packet(spec,'0',null),null);
});
test('wire certificates are bound to the exact loader spec and map and cannot be reconstructed',()=>{
  const certificate=certifyOrdinaryAi0Packet(spec,'0',wire());
  for(const fake of [null,{}, {...spec},ordinaryProtectionSummary(spec)]){
    assert.equal(certifyOrdinaryAi0Packet(fake,'0',wire()),null);
    assert.equal(isProtectedOrdinaryPacketCell(fake,'0',certificate,inside),false);
  }
  for(const fake of [null,{}, {...certificate},JSON.parse(JSON.stringify(certificate))])assert.equal(isProtectedOrdinaryPacketCell(spec,'0',fake,inside),false);
  for(const map of ['1',0,null]){
    assert.equal(certifyOrdinaryAi0Packet(spec,map,wire()),null);
    assert.equal(isProtectedOrdinaryPacketCell(spec,map,certificate,inside),false);
  }
});
