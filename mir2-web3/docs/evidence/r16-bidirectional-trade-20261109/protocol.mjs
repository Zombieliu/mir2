import assert from 'node:assert/strict';
import { spawn, execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { closeSync, openSync } from 'node:fs';
import { copyFile, mkdir, readFile, writeFile } from 'node:fs/promises';
import { createRequire } from 'node:module';
import net from 'node:net';
import { join, resolve } from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { pathToFileURL } from 'node:url';

const [project, executable, fixture, output] = process.argv.slice(2);
assert(project && executable && fixture && output);
const root = resolve(project), exe = resolve(executable), prior = resolve(fixture), out = resolve(output);
const file = (name) => join(out, name);
const sha = (bytes) => createHash('sha256').update(bytes).digest('hex');
const fileSha = async (path) => sha(await readFile(path));
const load = async (path) => JSON.parse(await readFile(path, 'utf8'));
const importProject = (path) => import(pathToFileURL(join(root, path)).href);
const { GatewayClient, DIRECTIONS } = await importProject('apps/ai-player/src/gateway-client.mjs');
const { parseGatewayJson } = await importProject('apps/ai-player/src/lossless-json.mjs');
const { playableGatewayEnvironment } = await importProject('apps/web/scripts/r16-playable-service-config.mjs');
const Ws = createRequire(join(root, 'apps/web/package.json'))('next/dist/compiled/ws');

async function freePort() {
  const listener = net.createServer();
  await new Promise((ok, fail) => {
    listener.once('error', fail);
    listener.listen(0, '127.0.0.1', ok);
  });
  const { port } = listener.address();
  await new Promise((ok, fail) => listener.close((error) => error ? fail(error) : ok()));
  return port;
}

await mkdir(out);
const data = file('private-data');
await mkdir(data);
await mkdir(file('private-cwd'));
const sourceAccounts = join(prior, 'accounts.qualified-snapshot.json');
const originalStoreHash = await fileSha(sourceAccounts);
await copyFile(sourceAccounts, join(data, 'accounts.json'));
await mkdir(join(data, 'save-recovery'));
const state = await load(join(prior, 'realm-state.local.json'));
const identities = await load(join(prior, 'ui-identities.local.json'));
const [httpPort, tcpPort] = await Promise.all([freePort(), freePort()]);
assert.notEqual(httpPort, tcpPort);
const origin = `http://127.0.0.1:${httpPort}`;
const { MIR2_PASSKEY_AUTH_SECRET, MIR2_PASSKEY_ALLOWED_ORIGINS, ...localEnv } = playableGatewayEnvironment(state, data);
const env = { ...localEnv,
  MIR2_GATEWAY_WEB_ADDR: `127.0.0.1:${httpPort}`,
  MIR2_GATEWAY_TCP_ADDR: `127.0.0.1:${tcpPort}`,
  MIR2_ALLOWED_WEB_ORIGINS: origin,
  MIR2_SPECTATOR_ENABLED: 'false',
  MIR2_SPECTATOR_RECORDING_ENABLED: 'false',
};
const baselineHead = execFileSync('git', ['rev-parse', 'HEAD'], { cwd: root, encoding: 'utf8' }).trim();
const binarySha256 = await fileSha(exe);
const gatewayLog = openSync(file('gateway.private.log'), 'wx');
const gateway = spawn(exe, [], { cwd: file('private-cwd'), env, windowsHide: true, stdio: ['pipe', gatewayLog, gatewayLog] });
closeSync(gatewayLog);
gateway.stdin.on('error', () => {});
let launchError;
gateway.on('error', (error) => { launchError = error; });

const proof = { schema: 1, scope:'two ordinary WebSockets, isolated File', sourceHead: baselineHead, binarySha256, gatewayPid:gateway.pid, httpPort, tcpPort, passed: false, steps: [] };
const trace = [];
const wires = {};
const clients = {};
let phase = 'boot';
let number = 0;
const baseTime = performance.now();
const gameTradePackets = new Set(['TradeRequest', 'TradeAccept', 'TradeItem', 'TradeGold', 'TradeConfirm', 'TradeCancel', 'DepositTradeItem', 'RetrieveTradeItem', 'GainedItem', 'GainedGold', 'LoseGold', 'LogOutSuccess', 'StartGame']);
const gameTradeCommands = new Set(['tradeRequest', 'tradeReply', 'tradeGold', 'tradeConfirm', 'tradeCancel', 'depositTradeItem', 'retrieveTradeItem']);

function record(role, direction, raw) {
  if (phase === 'boot') return;
  const frame = parseGatewayJson(String(raw));
  let relevant;
  if (direction === 'sent' && gameTradeCommands.has(frame.type)) {
    relevant = { command: frame };
  } else if (direction === 'received' && frame.type === 'packet' && gameTradePackets.has(frame.packet)) {
    const payload = frame.payload ?? {};
    const { unlock, from, to, success, amount, gold, result } = payload;
    relevant = { packet: frame.packet, unlock, from, to, success, amount, gold, result,
      ...(frame.packet === 'TradeItem' ? { items: payload.tradeItems?.filter(Boolean).map((item) => ({ uidHash: sha(String(item.unique_id)), count: item.count })) } : {}) };
    if (frame.packet === 'GainedItem') {
      const { unique_id: uid, item_index: itemIndex, count } = payload.item;
      relevant.item = { uidHash: sha(String(uid)), itemIndex, count };
    }
    // Keep the original bytes of the small trade lifecycle replies. TradeItem
    // is represented by its raw hash and exact UID/count projection.
    if (['TradeAccept', 'TradeCancel', 'TradeConfirm', 'TradeGold', 'LoseGold', 'GainedGold', 'DepositTradeItem', 'RetrieveTradeItem'].includes(frame.packet)) relevant.wire = String(raw);
  } else if (direction === 'received' && frame.type === 'worldSnapshot') {
    const p = frame.payload;
    const trade = p.stage5Systems?.trade;
    relevant = { snapshot: true, tick: p.tick,
      trade: trade === null ? null : trade === undefined ? 'missing' : {
        nonceHash: sha(String(trade.settlementNonce)), slots: trade.offeredSlots,
        uids: Object.fromEntries(Object.entries(trade.offeredUniqueIds ?? {}).map(([slot, uid]) => [slot, sha(String(uid))])),
        locked: trade.locked, completed: trade.completed, prepared: trade.escrowPrepared, offeredGold: trade.offeredGold,
      }, gold: p.gold, economyHash: sha(JSON.stringify(economy(p))) };
  } else {
    return;
  }
  trace.push({ order: ++number, role, phase, direction, elapsedMs: Math.round(performance.now() - baseTime), sha256: sha(String(raw)), ...relevant });
}

function openClient(role) {
  class TraceSocket extends Ws {
    constructor(url) {
      super(url, { origin });
      wires[role] = this;
      this.addEventListener('message', ({ data: raw }) => record(role, 'received', raw));
    }
    send(raw, ...args) {
      record(role, 'sent', raw);
      return super.send(raw, ...args);
    }
  }
  return new GatewayClient({ url: `ws://127.0.0.1:${httpPort}/ws`, realmId: state.world.realmId, WebSocketImpl: TraceSocket, timeoutMs: 30000 });
}

const own = (role) => clients[role].snapshot();
const all = () => ({ primary: economy(own('primary')), peer: economy(own('peer')) });
const cells = (item) => item.slot + (item.container === 'bag2' ? 40 : 0);
const normalize = (items) => (items ?? []).map((item) => ({
  slot: item.slot, uidHash: sha(String(item.uniqueId)), quantity: item.quantity, container: item.container, key: item.key,
})).sort((a, b) => String(a.container).localeCompare(String(b.container)) || String(a.slot).localeCompare(String(b.slot)) || a.uidHash.localeCompare(b.uidHash));
function economy(s) {
  return { gold: s.gold, bag: normalize(s.inventoryItems), belt: normalize(s.beltItems), equipment: normalize(s.equipmentItems) };
}

async function poll(sample, accept, message, timeoutMs = 30000) {
  const end = performance.now() + timeoutMs;
  while (performance.now() < end) {
    const value = sample();
    if (accept(value)) return value;
    await delay(15);
  }
  throw new Error(message);
}

function send(role, command) {
  assert(gameTradeCommands.has(command.type) || command.type === 'keepAlive');
  const after = clients[role].sequence;
  wires[role].send(JSON.stringify(command));
  return after;
}
const packet = (role, name, after, check = () => true) => clients[role].waitFor((event) => event.type === 'packet' && event.packet === name && check(event.payload), { after });
const snapshot = (role, after, check) => clients[role].waitFor((event) => event.type === 'worldSnapshot' && check(event.payload), { after });

async function enter(role) {
  const client = clients[role] = openClient(role);
  const identity = identities[role];
  await client.connect();
  await client.login(identity.accountId, identity.password);
  const world = await client.startGame(identity.characterIndex);
  assert.equal(world.player.name, identity.characterName);
  assert.equal(world.mapFileName, '0');
}

const offsets = [[0,-1],[1,-1],[1,0],[1,1],[0,1],[-1,1],[-1,0],[-1,-1]];
async function align() {
  const a = own('primary').player;
  const direction = DIRECTIONS.indexOf(a.direction);
  assert(direction >= 0);
  const target = { x: a.x + offsets[direction][0], y: a.y + offsets[direction][1] };
  for (let i = 0; i < 8; i++) {
    const b = own('peer').player;
    if (b.x === target.x && b.y === target.y) break;
    const step = offsets.findIndex(([x, y]) => x === Math.sign(target.x - b.x) && y === Math.sign(target.y - b.y));
    assert(step >= 0);
    await clients.peer.walk(DIRECTIONS[step]);
    await delay(420);
  }
  assert.deepEqual([own('peer').player.x, own('peer').player.y], [target.x, target.y]);
  await clients.peer.turn(DIRECTIONS[(direction + 4) % 8]);
  await poll(() => own('peer').entities.some((e) => e.name === a.name && e.kind === 'player' && e.x === a.x && e.y === a.y && e.direction === a.direction), Boolean, 'Mutual facing missing');
  await poll(() => own('peer').player.direction, (d) => d === DIRECTIONS[(direction + 4) % 8], 'Peer facing missing');
  proof.steps.push({ adjacent: true, mutualFacing: true });
}

async function openTrade() {
  const invitationAfter = clients.primary.sequence;
  await clients.peer.requestPlayerTrade();
  await packet('primary', 'TradeRequest', invitationAfter, (payload) => payload.partnerName === identities.peer.characterName);
  const after = send('primary', { type: 'tradeReply', acceptInvite: true });
  await packet('primary', 'TradeAccept', after, (payload) => payload.partnerName === identities.peer.characterName);
  await snapshot('primary', after, (p) => p.stage5Systems?.trade?.partner === identities.peer.characterName);
}

const roles = ['primary','peer'];
const names = {primary:'Candle', peer:'WoodenSword'};
const amounts = {primary:137, peer:59};
const partnerOf = role => role === 'primary' ? 'peer' : 'primary';
const packets = (role,name) => trace.filter(row => row.phase === 'settlement' && row.role === role && row.packet === name);
const delivery = role => packets(role,'GainedGold').length + packets(role,'GainedItem').length;
const sortBag = bag => bag.sort((a,b) => String(a.container).localeCompare(String(b.container)) || String(a.slot).localeCompare(String(b.slot)) || a.uidHash.localeCompare(b.uidHash));

async function transferToOffer(role) {
  const item = own(role).inventoryItems.find(entry => entry.name === names[role]);
  assert(item && item.quantity === 1 && item.container === 'bag1');
  const from = cells(item), to = 0;
  const after = send(role, {type:'depositTradeItem',from,to});
  const ack = await packet(role,'DepositTradeItem',after,p=>p.from===from&&p.to===to);
  assert.equal(ack.payload.success,true);
  await snapshot(role,after,p=>p.stage5Systems?.trade?.offeredSlots?.[to]===from && String(p.stage5Systems?.trade?.offeredUniqueIds?.[to])===String(item.uniqueId));
  return item;
}

function exactSettlement(before, items) {
  const expected = structuredClone(before), received = {};
  for (const role of roles) {
    const other = partnerOf(role);
    const sentItem = items[role], incoming = items[other];
    const terminal = packets(role,'TradeConfirm');
    const credit = packets(role,'GainedGold'), debit = packets(role,'LoseGold');
    const goods = packets(role,'GainedItem');
    assert.equal(terminal.length,1,role+' terminal');
    assert.equal(credit.length,1,role+' incoming gold');
    assert.equal(debit.length,1,role+' outgoing gold');
    assert.equal(goods.length,1,role+' incoming item');
    assert.equal(credit[0].gold,amounts[other]);
    assert.equal(debit[0].gold,amounts[role]);
    assert(credit[0].order < terminal[0].order && goods[0].order < terminal[0].order);
    assert.equal(incoming.key,'crystal-item-'+goods[0].item.itemIndex);
    assert.equal(goods[0].item.count,incoming.quantity);
    const ownUid=sha(String(sentItem.uniqueId)), deliveryUid=goods[0].item.uidHash, incomingUid=sha(String(incoming.uniqueId));
    const outgoing=before[role].bag.find(i=>i.uidHash===ownUid);
    const source=before[other].bag.find(i=>i.uidHash===incomingUid);
    assert(outgoing&&source);
    expected[role].bag=expected[role].bag.filter(i=>i.uidHash!==ownUid);
    const existing=[...expected[role].bag,...before[role].belt,...before[role].equipment];
    assert(!existing.some(i=>i.uidHash===deliveryUid),'receiver delivery collides with existing UID');
    const collision=existing.some(i=>i.uidHash===incomingUid);
    assert(collision?deliveryUid!==incomingUid:deliveryUid===incomingUid);
    const taken=new Set(expected[role].bag.filter(i=>i.container==='bag1').map(i=>i.slot));
    const target=taken.has(source.slot)?Array.from({length:40},(_,i)=>i).find(i=>!taken.has(i)):source.slot;
    expected[role].bag.push({...source,uidHash:deliveryUid,slot:target}); sortBag(expected[role].bag);
    expected[role].gold+=amounts[other]-amounts[role];
    received[role]={itemIndex:goods[0].item.itemIndex,count:incoming.quantity,fromUidHash:incomingUid,toUidHash:deliveryUid,bagSlot:target,
      normalizedCollision:collision,debit:debit[0].gold,credit:credit[0].gold,creditOrder:credit[0].order,itemOrder:goods[0].order,terminalOrder:terminal[0].order};
  }
  return {expected,received};
}

async function receiveSnapshot(role, target = null, afterOrder = 0) {
  send(role, { type: 'keepAlive', time: Date.now() });
  return poll(() => trace.findLast((row) => row.phase === phase && row.role === role && row.snapshot && row.order > afterOrder && (target === 'open' ? typeof row.trade === 'object' && row.trade !== null : row.trade === null)), Boolean, `Missing ${target ?? 'closed'} snapshot for ${role}`);
}

async function relog(expected) {
  phase = 'relogin';
  for (const role of ['primary', 'peer']) {
    await clients[role].logout();
    await clients[role].close();
    await enter(role);
    assert.deepEqual(economy(own(role)), expected[role]);
    assert.equal(own(role).trade.active, false);
    assert.equal(own(role).trade.uncertain, false);
    await clients[role].logout();
  }
  proof.sameCharacterReadback = true;
}

try {
  for (let attempt = 0; ; attempt++) {
    if (launchError) throw launchError;
    assert.equal(gateway.exitCode, null, 'Owned gateway exited before readiness');
    const healthy = await fetch(`${origin}/health`, { signal: AbortSignal.timeout(600) }).then((r) => r.ok, () => false);
    if (healthy) break;
    assert(attempt < 180, 'Owned gateway never became healthy');
    await delay(100);
  }
  await enter('primary');
  await enter('peer');
  await align();
  const before=all();
  assert.deepEqual({primary:before.primary.gold,peer:before.peer.gold},{primary:1000,peer:700});
  proof.assetsBefore=before;
  phase='settlement';
  await openTrade();
  await receiveSnapshot('peer','open');
  const items={};
  for(const role of roles)items[role]=await transferToOffer(role);
  for(const role of roles) {
    const gold=await clients[role].offerTradeGold(amounts[role]);
    assert.equal(gold.offeredGold,amounts[role]);
    assert.equal(gold.walletGold,before[role].gold-amounts[role]);
  }
  for(const role of roles) {
    send(role,{type:'keepAlive',time:Date.now()});
    await poll(()=>packets(role,'TradeGold'),rows=>rows.some(p=>p.amount===amounts[partnerOf(role)]),'Remote gold offer missing for '+role,30000);
  }
  const firstAfter=send('peer',{type:'tradeConfirm',locked:true});
  await snapshot('peer',firstAfter,p=>p.stage5Systems?.trade?.escrowPrepared===true && p.stage5Systems.trade.locked===true);
  assert.equal(own('peer').gold,700-59);
  assert(!own('peer').inventoryItems.some(i=>String(i.uniqueId)===String(items.peer.uniqueId)));
  assert(roles.every(role=>packets(role,'TradeConfirm').length===0&&delivery(role)===0));
  proof.firstPrepare={role:'peer',firstOfferHeld:true,deliveriesOrTerminal:0};
  const secondAfter=send('primary',{type:'tradeConfirm',locked:true});
  await packet('primary','TradeConfirm',secondAfter);
  await snapshot('primary',secondAfter,p=>p.stage5Systems?.trade===null);
  for(const role of roles)send(role,{type:'keepAlive',time:Date.now()});
  const peerTerminal=await poll(()=>packets('peer','TradeConfirm'),rows=>rows.length===1,'Missing peer terminal',30000);
  await receiveSnapshot('peer',null,peerTerminal[0].order);
  assert(roles.every(role=>!own(role).trade.active&&!own(role).trade.uncertain));
  const {expected,received}=exactSettlement(before,items);
  proof.assetsAfter=all();
  assert.deepEqual(proof.assetsAfter,expected);
  assert.equal(expected.primary.gold+expected.peer.gold,1700);
  proof.received=received; proof.exactAssets=true; proof.totalGold=1700;
  await relog(expected);
  proof.passed=true;
} catch (error) {
  proof.failure = { phase, type: error.name, message: String(error.message).split('\n')[0].slice(0, 140) };
} finally {
  await Promise.allSettled(Object.values(clients).map((client) => client.close()));
  if (gateway.exitCode === null && gateway.signalCode === null && !launchError) {
    gateway.stdin.end('shutdown\n');
    await poll(() => gateway.exitCode !== null || gateway.signalCode !== null, Boolean, 'Owned gateway did not stop', 12000).catch(() => {});
    if (gateway.exitCode === null && gateway.signalCode === null) gateway.kill();
  }
  proof.ownedGatewayStopped = gateway.exitCode !== null || gateway.signalCode !== null;
  proof.sourceFixtureUnchanged = await fileSha(sourceAccounts) === originalStoreHash;
  await writeFile(file('wire-trace.json'), JSON.stringify(trace, null, 2) + '\n', { flag: 'wx' });
  await writeFile(file('proof.json'), JSON.stringify(proof, null, 2) + '\n', { flag: 'wx' });
  console.log(JSON.stringify({passed:proof.passed,phase,rows:trace.length,relog:proof.sameCharacterReadback,stopped:proof.ownedGatewayStopped,failure:proof.failure}));
  if (!proof.passed || !proof.ownedGatewayStopped || !proof.sourceFixtureUnchanged) process.exitCode = 1;
}
