import fs from 'node:fs/promises';
import { createHash } from 'node:crypto';

// Source-exact v27 resource baseline. This controller is a Candidate gameplay
// diagnostic, not a new balance profile, original-C# parity or visual approval.
export const sha256 = value => createHash('sha256').update(value).digest('hex');
export const SOURCE_HASHES = Object.freeze({
  profile: 'f50ef1b78e2544df4fd3a65902f6e72f4ec3cbc7693e0ee373d6ee899581e7a9',
  npcScripts: '7ff29f221c39e5d16b7ebdc7efcde5090daf7ff3daacf6b2b1552b5b36429e93',
  npcs: 'dcc31e181b1f329c6da3658846ff18adb13731b18d899f8914b7628d54074105',
  maps: '5303f8093be7f15ddd9f860e6db1787ac96606508db3086bab38bdc4c6353dd3',
  mapEvents: 'd68823dfff2a1cfc00afc7bf4d369bb20a025ad9af5acb89a75d571f4709ba29',
  monsters: '5961fe9f220cb8440833af746f4c53fd1c728803979b19dbd867f10fc3027cec',
  drops: 'c10f5b1ac39cefe1c5b7c0ec5779547cf5b0901eaece680b7049d9dda56c1cf7',
  items: 'b0d48ada978d8a062e993fe0425b6a724672aeea354a3b7b4508fb4c161658b7',
  quests: '13d6f3fd94fd9e865ab60c9f2ea50585053bd2050bff021a0379f1ed0948e0d6',
  magics: 'ccbb31513c006860790a4c374b3f788665902cfa3fffe810695199882860153d',
});
const DATA = new URL('../../../../packages/game-data/data/', import.meta.url);
export const SOURCE_URLS = Object.freeze({
  profile: new URL('content_profiles/platinum_176.json', DATA),
  ...Object.fromEntries(Object.entries({ npcScripts: 'npc', npcs: 'npc_info', maps: 'respawn',
    mapEvents: 'map_event', monsters: 'monster', drops: 'drop', items: 'item', quests: 'quest_packet', magics: 'magic' })
    .map(([key, stem]) => [key, new URL(`generated/crystal_${stem}_manifest.json`, DATA)])),
});
export const CLASSES = Object.freeze(['Warrior', 'Wizard', 'Taoist']);
export const COMMANDS = new Set(['clientVersion', 'login', 'startGame', 'keepAlive', 'walk', 'run', 'turn',
  'attack', 'attackDirection', 'magic', 'spellToggle', 'interact', 'selectNpcDialog', 'acceptQuest',
  'finishQuest', 'pickUp', 'equipItem', 'moveItem', 'useItem', 'buyItem', 'logOut']);
export const ROUTES = Object.freeze({
  stone: Object.freeze(['0','2','3','D710','D711','D712','D713','D714','D715','D71601','D71621','D71608','D71605','D71625','D716','D717']),
  'stone-return': Object.freeze(['D717','D716','D71625','D71623','D71601','D715','D714','D713','D712','D711','D710','3','2','0']),
  zuma: Object.freeze(['0','2','3','0157','D501','D502','D503','D504','D505','D5061','D5068','D5071','D5072','D5073','D5074','D515']),
  'zuma-return': Object.freeze(['D515','D5074','D5073','D5072','D5071','D5068','D5061','D505','D504','D503','D502','D501','0157','3','2','0']),
  redmoon: Object.freeze(['0','1','11','12','D10011','D1002','D10031','D1004','D10052','D10062']),
  'redmoon-return': Object.freeze(['D10062','D10052','D1004','D10031','D1002','D10011','12','11','1','0']),
});
export const EXPECTED_DIRECTED_HOPS = Object.values(ROUTES).reduce((count, chain) => count + chain.length - 1, 0); //76
export const SOURCE_ONLY_ROOM = 'D71653';
export const PRESERVED_DEFECT = Object.freeze({ from: 'D10051', to: 'D10061', source: {x:178,y:53}, destination:{x:20,y:25}, needMove:true });
export const LEGACY_POLICY = Object.freeze([
  Object.freeze({ key:'MongchonProvince/StoneTemple/Stone', path:'MongchonProvince/StoneTemple/Stone.txt',
    rawSha256:'249ff6c6810c7f06cd52f551c2534a2e24e7cead835d6948b72101146cf154c2',
    executionSha256:'f86465391544e7633c1999aea856636266456d650bbd0d56af5d66322dc39d0d',
    conditions:[{section:'@MAIN',line:4,raw:'CHECKQUEST 135 0',intent:'currentAccepted'},
      {section:'@Check2',line:15,raw:'CHECKQUEST 153 1',intent:'completed'},
      {section:'@stonetomba',line:39,raw:'CHECKITEM StoneHeart 1',intent:'canonicalItem1080'},
      {section:'@stonetomba',line:41,raw:'TAKEITEM StoneHeart 1',intent:'canonicalDebit1080'}] }),
  Object.freeze({ key:'WoomyonWoods/TaoistVillage/BigTaoist', path:'WoomyonWoods/TaoistVillage/BigTaoist.txt',
    rawSha256:'911a5edd4dc638eedf54819d0314cbcf2b438f32ba02d6d951091146ece02ebd',
    executionSha256:'e186e61b616cfb28f48611eeca19aba568cf9d9c6f39362ed932b133bcef3111',
    conditions:[{section:'@Next1',line:15,raw:'CHECKQUEST 146 1',intent:'completed'}] }),
]);
export const BOSSES = Object.freeze({
  WhiteBoar: {map:'D717',respawns:[2797,2798],dropPath:'MongchonProvince/StoneTomb/WhiteBoar'},
  EvilSnake: {map:'D717',respawns:[2799],dropPath:'MongchonProvince/StoneTomb/EvilSnake'},
  ZumaTaurus: {map:'D515',respawns:[3966],dropPath:'MongchonProvince/Zuma/ZumaTaurus'},
  RedMoonEvil: {map:'D10062',respawns:[1688],dropPath:'TaoVillage/TreePath/RedValley/RedMoonEvil'},
});
export const LATE_BOOKS = Object.freeze({Warrior:['CounterAttack'],Wizard:['Blink','MagicBooster'],Taoist:['Plague','PetEnhancer','HealingCircle']});

function requireValue(condition, message) { if (!condition) throw new Error(message); }
export function assertCommand(command) {
  requireValue(COMMANDS.has(command?.type), `Forbidden classic route command: ${command?.type}`);
  for (const value of Object.values(command)) requireValue(typeof value !== 'string' || !/^(?:crystal:|qa\.|event\.)/.test(value), 'Forbidden private/debug argument');
}
export function validateEndpoint(url) {
  const endpoint = new URL(url);
  requireValue(!endpoint.username && !endpoint.password && !endpoint.search && !endpoint.hash, 'Gateway URL must not contain credentials/query/fragment');
  requireValue(endpoint.protocol === 'wss:' || (endpoint.protocol === 'ws:' && ['localhost','127.0.0.1','[::1]'].includes(endpoint.hostname)), 'Use WSS or explicit loopback WS');
  return endpoint;
}
export function assertLegacySource(script) {
  const proof = LEGACY_POLICY.find(candidate => candidate.key === script?.script_key);
  requireValue(proof && script.relative_path === proof.path && sha256(script.raw_text) === proof.rawSha256
    && sha256(JSON.stringify(script)) === proof.executionSha256, 'Unknown/changed legacy route source proof');
  for (const condition of proof.conditions) {
    requireValue(script.raw_text.split('\n')[condition.line - 1] === condition.raw, 'Changed complete reviewed raw condition');
    requireValue(script.sections.some(section => section.label === condition.section && section.lines.includes(condition.raw)), 'Changed parsed route condition');
  }
  return proof;
}
export async function loadClassicSources(urls = SOURCE_URLS) {
  const entries = await Promise.all(Object.entries(SOURCE_HASHES).map(async ([key, expected]) => {
    const bytes = await fs.readFile(urls[key]);
    requireValue(sha256(bytes) === expected, `Classic source hash mismatch: ${key}`);
    return [key, JSON.parse(bytes)];
  }));
  const sources = Object.fromEntries(entries);
  requireValue(sources.profile.profileId === 'platinum_176' && sources.profile.version === 27
    && sources.profile.mapWhitelist.length === 208 && !sources.profile.mapWhitelist.includes(SOURCE_ONLY_ROOM), 'Wrong classic runtime profile identity/scope');
  for (const proof of LEGACY_POLICY) assertLegacySource(sources.npcScripts.scripts.find(script => script.script_key === proof.key));
  sources.hashes = {...SOURCE_HASHES};
  sources.portals = Object.fromEntries(Object.entries(ROUTES).map(([id, chain]) => [id, chain.slice(0,-1).map((from,index) => {
    const map = sources.maps.maps.find(map => map.map_file_name === from);
    const to = chain[index+1];
    const destination = sources.maps.maps.find(map => map.map_file_name === to);
    const portals = (map?.movements ?? []).filter(move => move.map_index === destination?.map_index && !move.need_hole && !move.need_move);
    requireValue(portals.length > 0, `Missing ordinary source route ${from}->${to}`);
    return {from,to,portals};
  })]));
  return sources;
}
export function validateScenario(input) {
  requireValue(CLASSES.includes(input?.className), 'Use a declared Warrior/Wizard/Taoist account');
  requireValue(typeof input.accountId === 'string' && input.accountId.length && typeof input.password === 'string' && input.password.length, 'Authenticated account credentials required');
  requireValue(typeof input.characterName === 'string' && input.characterName.length, 'Exact character name required');
  requireValue(Number.isSafeInteger(input.characterIndex) && input.characterIndex >= 0, 'Character index required');
  validateEndpoint(input.gatewayUrl);
  requireValue(input.startingState?.preparedLevel === true || input.startingState?.preparedLevel === false, 'Declare starting level provenance');
  requireValue(Number.isInteger(input.startingState?.level) && input.startingState.level > 0 && input.startingState.level <= 255, 'Declare exact starting level');
  requireValue(['main-round-trips','stone-entry','great-tao-defect'].includes(input.lane), 'Unknown bounded classic route lane');
  const timeoutMs = input.timeoutMs ?? 120*60_000;
  requireValue(Number.isInteger(timeoutMs) && timeoutMs >= 1000 && timeoutMs <= 120*60_000, 'Route deadline must stay within120 minutes');
  const maxKillsPerTarget = input.maxKillsPerTarget ?? 0;
  requireValue(Number.isInteger(maxKillsPerTarget) && maxKillsPerTarget >= 0 && maxKillsPerTarget <= 3, 'Natural kills must be0-3 per target');
  const spawnWaitMs = input.spawnWaitMs ?? 60*60_000;
  requireValue(Number.isInteger(spawnWaitMs) && spawnWaitMs >= 0 && spawnWaitMs <= 150*60_000, 'Natural spawn wait exceeds150 minutes');
  requireValue(!input.qa && !input.debug && !input.relocation && input.dropMultiplier == null, 'No runtime QA, relocation or drop overrides');
  return {...input, timeoutMs, maxKillsPerTarget, spawnWaitMs};
}
export function assertPair(pair, health, scenario) {
  requireValue(pair?.schema === 'mir2.classic-paired-gateway-attestation.v1', 'Root-issued paired Gateway receipt required');
  requireValue(/^[a-f0-9]{40}$/.test(pair.sourceRevision) && /^[a-f0-9]{64}$/.test(pair.gatewayExecutableSha256), 'Missing source/executable pair identity');
  requireValue(health?.ok === true && health.ws === 'ready' && health.revision === pair.sourceRevision, 'Live health revision differs from paired deployment receipt');
  requireValue(pair.gatewayUrl === scenario.gatewayUrl && pair.profileId === 'platinum_176' && pair.profileVersion === 27, 'Paired endpoint/profile identity mismatch');
  for (const [key, hash] of Object.entries(SOURCE_HASHES)) requireValue(pair.sourceHashes?.[key] === hash, `Paired source hash mismatch: ${key}`);
  requireValue(pair.qaNaturalKillDropMultiplier === 1 && pair.profileDropMultiplier === 1 && pair.debugWorldMutationEnabled === false, 'Unverified natural-drop/QA configuration');
  requireValue(pair.namedLegacyPolicy === 'source-bound-stone-big-taoist-v1', 'Pair predates reviewed named legacy repair');
  // Health revision is observed. Executable/config identity remains an explicit
  // root deployment attestation, never inferred from a caller's revision label.
  return {observedHealthRevision:health.revision, executableIdentitySource:'root-deployment-attestation', gatewayExecutableSha256:pair.gatewayExecutableSha256};
}
export function freshSnapshot(events, after, map) {
  return events.find(event => event.direction === 'received' && event.sequence > after && event.type === 'worldSnapshot'
    && event.payload?.mapFileName === map && !event.payload?.mapSnapshotPending) ?? null;
}
export function exactNpc(snapshot, template) {
  return (snapshot?.entities ?? []).find(entity => String(entity.kind).toLowerCase() === 'npc'
    && Number(entity.objectId) === Number(template.loaded_object_id) && entity.name === template.name
    && Number(entity.x) === template.location.x && Number(entity.y) === template.location.y
    && snapshot.mapFileName === template.map_file_name && !snapshot.mapSnapshotPending) ?? null;
}
export function npcLink(snapshot, id, target) {
  const dialog = snapshot?.activeNpcDialog;
  requireValue(!snapshot?.mapSnapshotPending && Number(dialog?.npcObjectId) === Number(id), 'Stale or wrong NPC dialog');
  const link = dialog.links?.find(link => String(link.target).toLowerCase() === String(target).toLowerCase());
  requireValue(link && Array.isArray(dialog.body), `Live dialog does not expose ${target}`);
  return link;
}
export function bagCount(snapshot, uid) {
  return (snapshot?.inventoryItems ?? []).filter(item => String(item.uniqueId) === String(uid)).reduce((n,item) => n+Number(item.quantity),0);
}
export function redactEvidence(value, secrets = []) {
  const visit = (value, key='') => {
    if (/password|secret|token|accountId/i.test(key)) return '[redacted]';
    if (typeof value === 'string') return secrets.filter(Boolean).reduce((safe, secret) => safe.split(String(secret)).join('[redacted]'),value);
    if (Array.isArray(value)) return value.map(entry=>visit(entry));
    if (value && typeof value === 'object') return Object.fromEntries(Object.entries(value).map(([k,v])=>[k,visit(v,k)]));
    return value;
  };
  return visit(value);
}
export function bossCatalog(sources, name) {
  const policy = BOSSES[name]; requireValue(policy, 'Unknown bounded natural Boss');
  const monster = sources.monsters.monsters.find(monster=>monster.name===name);
  requireValue(monster && monster.drop_path.replaceAll('\\','/')===policy.dropPath, 'Wrong monster DropPath');
  const table = sources.drops.tables.find(table=>table.table_key===policy.dropPath);
  requireValue(table?.total_entries > 0, 'Boss uses actual nested DropPath, not empty same-name placeholder');
  const map = sources.maps.maps.find(map=>map.map_file_name===policy.map);
  const spawns = map.respawns.filter(spawn=>policy.respawns.includes(spawn.respawn_index) && spawn.monster_name===name);
  requireValue(spawns.length===policy.respawns.length, 'Missing original source spawn');
  return {policy,monster,table,spawns};
}
