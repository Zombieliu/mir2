import fs from 'node:fs/promises';
import path from 'node:path';
import crypto from 'node:crypto';
import { pathToFileURL } from 'node:url';

const [repository, className, portText] = process.argv.slice(2);
if (!repository || !['Wizard', 'Taoist'].includes(className)) throw new Error('Owned caster repository and class required');
const root = path.dirname(new URL(import.meta.url).pathname).replace(/^\/(\w:)/, '$1');
const directory = path.join(root, className.toLowerCase());
const cohort = JSON.parse(await fs.readFile(path.join(directory, className + '.report.json'), 'utf8'));
if (cohort.completed !== true || cohort.v2?.status !== 'completed') throw new Error('Normal saved full route required before extra supply checks');
const privateStore = path.join(directory, 'accounts.json');
const preservedStore = path.join(directory, 'full-route-accounts.private.json');
try { await fs.copyFile(privateStore, preservedStore, fs.constants.COPYFILE_EXCL); }
catch (error) { if (error.code !== 'EEXIST') throw error; }
const savedStoreSha256 = crypto.createHash('sha256').update(await fs.readFile(preservedStore)).digest('hex');
const credentials = JSON.parse(await fs.readFile(path.join(directory, className + '.private.json'), 'utf8'));
const moduleUrl = name => pathToFileURL(path.join(repository, 'apps/web/scripts/quest-agent', name)).href;
const { ProtocolClient, startGameBootstrapEvidence } = await import(moduleUrl('protocol-client.mjs'));
const { createNavigator, selfPlayer } = await import(moduleUrl('protocol-play.mjs'));
const { createMapTraveler } = await import(moduleUrl('protocol-travel.mjs'));
const { restockV2Supplies } = await import(moduleUrl('protocol-supplies.mjs'));
const { v2SupplyStock, v2SupplyItem, V2_SUPPLY_ITEMS } = await import(moduleUrl('newcomer-v2-supply-policy.mjs'));
const runId = new Date().toISOString().replace(/[:.]/g, '-');
const traceFile = path.join(root, className + '.post-route-supplies.' + runId + '.trace.jsonl');
const reportFile = path.join(root, className + '.post-route-supplies.report.json');
const startedAt = new Date().toISOString();
const until = Date.now() + 600_000;
const client = new ProtocolClient('ws://127.0.0.1:' + Number(portText) + '/ws', traceFile);
const report = { className, runId, startedAt, traceFile, transport: 'normal-local-websocket', visualAccepted: false,
  ordinaryRouteRunId: cohort.runId, preservedFullRouteStoreSha256: savedStoreSha256,
  ordinaryClockNotExtended: true, postRouteOnly: true, purchases: [], completed: false };
let actions = 0;
let inWorld = false;
const guard = () => {
  if (Date.now() >= until || actions >= 600) throw new Error('Post-route supply smoke budget exhausted');
  if (inWorld && Number(client.snapshot?.playerHp) <= 0) throw new Error('Post-route supply smoke stopped on death');
};
const bounded = new Proxy(client, { get(target, key) {
  if (key === 'send') return command => { guard(); actions += 1; return target.send(command); };
  if (key === 'wait') return (predicate, label, timeout = 20_000) => {
    guard(); return target.wait(() => { guard(); return predicate(); }, label, Math.max(1, Math.min(timeout, until - Date.now())));
  };
  const value = Reflect.get(target, key, target);
  return typeof value === 'function' ? value.bind(target) : value;
} });
const totalItem = (snapshot, index) => {
  const seen = new Set(); let count = 0;
  for (const item of [...(snapshot?.inventoryItems ?? []), ...(snapshot?.beltItems ?? []), ...(snapshot?.equipmentItems ?? [])]) {
    if (v2SupplyItem(item)?.itemIndex !== index || seen.has(String(item.uniqueId))) continue;
    seen.add(String(item.uniqueId)); count += Number(item.quantity ?? 0);
  }
  return count;
};
try {
  await client.connect();
  const login = await client.request({ type: 'login', accountId: credentials.accountId, password: credentials.password }, 'LoginSuccess');
  const character = login.payload.characters.find(value => value.name === credentials.name);
  if (!character || Number(character.level) < 30) throw new Error('Completed ordinary level-30 character required');
  const afterStart = client.sequence;
  bounded.send({ type: 'startGame', characterIndex: character.index });
  await bounded.wait(() => startGameBootstrapEvidence(client, afterStart, credentials.name), 'post-route bootstrap', 60_000);
  await bounded.wait(() => selfPlayer(client)?.name === credentials.name, 'post-route owner snapshot');
  if (!(Number(selfPlayer(client)?.level) >= 30)) throw new Error('Live level-30 owner required');
  inWorld = true;
  const rawNavigate = createNavigator(bounded);
  const navigate = (point, distance = 1, stop = () => false, options = {}) => rawNavigate(point, distance, stop,
    { ...options, maxSuccessfulSteps: Math.min(520, options.maxSuccessfulSteps ?? 520),
      maxAttempts: Math.min(640, options.maxAttempts ?? 640), detectPositionCycles: true });
  const travel = createMapTraveler(bounded, navigate, { maxDisconnectedRegionRelocations: 1, maxBlockingMonsterClears: 1 });
  const indices = className === 'Taoist' ? [663, 717, 710, 711] : [663, 717];
  for (const index of indices) {
    guard();
    const supply = V2_SUPPLY_ITEMS.find(value => value.itemIndex === index);
    const stock = v2SupplyStock(client.snapshot);
    const policy = { minimumHpRatio: 0.35 };
    for (const kind of ['hp', 'mp', 'amulet', 'poison', 'townTeleport', 'randomTeleport']) {
      policy[kind] = { minimum: 0, target: 0, itemIndex: ({hp:662,mp:663,amulet:712,poison:710,townTeleport:719,randomTeleport:717})[kind] };
    }
    policy[supply.kind] = { minimum: stock[supply.kind] + 1, target: stock[supply.kind] + 1, itemIndex: index };
    const before = { gold: client.snapshot.gold, quantity: totalItem(client.snapshot, index), sequence: client.sequence };
    const result = await restockV2Supplies(bounded, navigate, { policy, travel });
    const after = { gold: client.snapshot.gold, quantity: totalItem(client.snapshot, index), sequence: client.sequence };
    const purchase = result.purchases?.find(value => value.itemIndex === index && value.quantity === 1);
    const receipts = client.events.filter(event => event.direction === 'received' && event.sequence > before.sequence &&
      ['LoseGold', 'GainedItem'].includes(event.packet)).map(event => ({ sequence: event.sequence, at: event.at, packet: event.packet }));
    if (result.status !== 'restocked' || !purchase || after.quantity !== before.quantity + 1 ||
        after.gold !== before.gold - purchase.cost || !receipts.some(value => value.packet === 'LoseGold') ||
        !receipts.some(value => value.packet === 'GainedItem')) throw new Error('Unconfirmed ordinary one-item purchase of ' + supply.name + ': ' + (result.reason ?? result.status));
    report.purchases.push({ ...purchase, before, after, receipts });
    await fs.writeFile(reportFile, JSON.stringify(report, null, 2));
  }
  report.completed = true;
} catch (error) {
  report.error = String(error.message);
  process.exitCode = 1;
} finally {
  await client.close();
  report.finishedAt = new Date().toISOString();
  report.actions = actions;
  report.normalLogout = client.events.filter(event => event.direction === 'received' && event.packet === 'LogOutSuccess')
    .map(event => ({ sequence: event.sequence, at: event.at })).at(-1) ?? null;
  await fs.writeFile(reportFile, JSON.stringify(report, null, 2));
  console.log(JSON.stringify(report));
}
