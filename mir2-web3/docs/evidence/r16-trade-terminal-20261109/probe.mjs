import assert from 'node:assert/strict';
import { spawn, execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { closeSync, openSync } from 'node:fs';
import { copyFile, mkdir, readFile, readdir, writeFile } from 'node:fs/promises';
import { createRequire } from 'node:module';
import net from 'node:net';
import { join, resolve } from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { pathToFileURL } from 'node:url';

const [project, executable, fixture, output, mode, tickArgument] = process.argv.slice(2).map(String);
assert(project && executable && fixture && output && ['observe', 'verify', 'settle', 'deposit'].includes(mode));
const root = resolve(project), exe = resolve(executable), prior = resolve(fixture), out = resolve(output);
const tickMs = tickArgument ? Number(tickArgument) : null;
assert(tickMs === null || Number.isInteger(tickMs) && tickMs >= 100 && tickMs <= 5000);
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
const sourceAccounts = join(prior, 'data/accounts.json');
const originalStoreHash = await fileSha(sourceAccounts);
await copyFile(sourceAccounts, join(data, 'accounts.json'));
const recovery = await readdir(join(prior, 'data/save-recovery'), { recursive: true });
const recoveryMetadata = new Set(['.mir2-directory-durability', '.mir2-recovery-exclusive.lock', '.mir2-recovery-owner-v1.json', 'quarantine', 'quarantine/.mir2-directory-durability']);
assert(recovery.every((name) => recoveryMetadata.has(name.replaceAll('\\', '/'))), 'Source fixture has unresolved recovery data');
await mkdir(join(data, 'save-recovery'));
const state = await load(join(prior, 'state.local.json'));
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
  ...(tickMs === null ? {} : { MIR2_GATEWAY_RUNTIME_TICK_MS: String(tickMs) }),
};
const baselineHead = execFileSync('git', ['rev-parse', 'HEAD'], { cwd: root, encoding: 'utf8' }).trim();
const binarySha256 = await fileSha(exe);
const gatewayLog = openSync(file('gateway.private.log'), 'wx');
const gateway = spawn(exe, [], { cwd: file('private-cwd'), env, windowsHide: true, stdio: ['pipe', gatewayLog, gatewayLog] });
closeSync(gatewayLog);
gateway.stdin.on('error', () => {});
let launchError;
gateway.on('error', (error) => { launchError = error; });

const proof = { schema: 1, mode, sourceHead: baselineHead, binarySha256, tickMs, passed: false, steps: [] };
const trace = [];
const wires = {};
const clients = {};
let phase = 'boot';
let number = 0;
const baseTime = performance.now();
const gameTradePackets = new Set(['TradeRequest', 'TradeAccept', 'TradeItem', 'TradeGold', 'TradeConfirm', 'TradeCancel', 'DepositTradeItem', 'RetrieveTradeItem', 'GainedItem', 'GainedGold', 'LogOutSuccess', 'StartGame']);
const gameTradeCommands = new Set(['tradeRequest', 'tradeReply', 'tradeGold', 'tradeConfirm', 'tradeCancel', 'depositTradeItem', 'retrieveTradeItem']);

function record(role, direction, raw) {
  if (phase === 'boot') return;
  const frame = parseGatewayJson(String(raw));
  let relevant;
  if (direction === 'sent' && gameTradeCommands.has(frame.type)) {
    relevant = { command: frame };
  } else if (direction === 'received' && frame.type === 'packet' && gameTradePackets.has(frame.packet)) {
    const payload = frame.payload ?? {};
    const { unlock, from, to, success, amount, result } = payload;
    relevant = { packet: frame.packet, unlock, from, to, success, amount, result,
      ...(frame.packet === 'TradeItem' ? { items: payload.tradeItems?.filter(Boolean).map((item) => ({ uidHash: sha(String(item.unique_id)), count: item.count })) } : {}) };
    if (frame.packet === 'GainedItem') {
      const { unique_id: uid, item_index: itemIndex, count } = payload.item;
      relevant.item = { uidHash: sha(String(uid)), itemIndex, count };
    }
    // Keep the original bytes of the small trade lifecycle replies. TradeItem
    // is represented by its raw hash and exact UID/count projection.
    if (['TradeAccept', 'TradeCancel', 'TradeConfirm', 'DepositTradeItem', 'RetrieveTradeItem'].includes(frame.packet)) relevant.wire = String(raw);
  } else if (direction === 'received' && frame.type === 'worldSnapshot') {
    const p = frame.payload;
    const trade = p.stage5Systems?.trade;
    relevant = { snapshot: true, tick: p.tick,
      trade: trade === null ? null : trade === undefined ? 'missing' : {
        nonceHash: sha(String(trade.settlementNonce)), slots: trade.offeredSlots,
        uids: Object.fromEntries(Object.entries(trade.offeredUniqueIds ?? {}).map(([slot, uid]) => [slot, sha(String(uid))])),
        locked: trade.locked, completed: trade.completed, prepared: trade.escrowPrepared, offeredGold: trade.offeredGold,
      }, economyHash: sha(JSON.stringify(economy(p))) };
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
  return new GatewayClient({ url: `ws://127.0.0.1:${httpPort}/ws`, realmId: state.world.realmId, WebSocketImpl: TraceSocket, timeoutMs: 12000 });
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

async function poll(sample, accept, message, timeoutMs = 12000) {
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

async function transferToOffer() {
  const item = own('primary').inventoryItems.find((entry) => entry.name === 'Candle');
  assert(item && item.quantity === 1 && item.container === 'bag1', 'Ordinary starter Candle missing');
  const from = cells(item), to = 0;
  const after = send('primary', { type: 'depositTradeItem', from, to });
  const ack = await packet('primary', 'DepositTradeItem', after, (p) => p.from === from && p.to === to);
  assert.equal(ack.payload.success, true);
  await snapshot('primary', after, (p) => p.stage5Systems?.trade?.offeredSlots?.[to] === from && String(p.stage5Systems?.trade?.offeredUniqueIds?.[to]) === String(item.uniqueId));
  return item;
}

async function receiveSnapshot(role, target = null, afterOrder = 0) {
  send(role, { type: 'keepAlive', time: Date.now() });
  return poll(() => trace.findLast((row) => row.phase === phase && row.role === role && row.snapshot && row.order > afterOrder && (target === 'open' ? typeof row.trade === 'object' && row.trade !== null : row.trade === null)), Boolean, `Missing ${target ?? 'closed'} snapshot for ${role}`);
}

async function peerTerminal(name, afterOrder = 0) {
  send('peer', { type: 'keepAlive', time: Date.now() });
  return poll(() => trace.find((row) => row.phase === phase && row.role === 'peer' && row.order > afterOrder && row.packet === name && (name !== 'TradeCancel' || row.unlock === false)), Boolean, `Missing peer ${name}`);
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
  const before = all();
  const expected = structuredClone(before);
  proof.assetsBefore = before;
  phase = mode === 'settle' ? 'settlement' : 'cancel';
  await openTrade();
  const item = await transferToOffer();
  if (mode === 'settle') {
    await receiveSnapshot('peer', 'open');
    const firstAfter = send('primary', { type: 'tradeConfirm', locked: true });
    await snapshot('primary', firstAfter, (p) => p.stage5Systems?.trade?.escrowPrepared === true);
    assert(!trace.some((row) => row.phase === 'settlement' && row.packet === 'TradeConfirm'), 'Prepare leaked a terminal receipt');
    const secondAfter = send('peer', { type: 'tradeConfirm', locked: true });
    await packet('peer', 'TradeConfirm', secondAfter);
    await snapshot('peer', secondAfter, (p) => p.stage5Systems?.trade === null);
    send('primary', { type: 'keepAlive', time: Date.now() });
    const delivered = await poll(() => trace.find((row) => row.phase === phase && row.role === 'primary' && row.packet === 'TradeConfirm'), Boolean, 'Missing sender final settlement receipt');
    await receiveSnapshot('primary', null, delivered.order);
    const uidHash = sha(String(item.uniqueId));
    const deliveredItems = trace.filter((row) => row.phase === 'settlement' && row.role === 'peer' && row.packet === 'GainedItem');
    assert.equal(deliveredItems.length, 1, 'Expected one actual delivery receipt');
    const receipt = deliveredItems[0];
    assert(receipt.order < trace.find((row) => row.role === 'peer' && row.packet === 'TradeConfirm').order);
    assert.equal(item.key, `crystal-item-${receipt.item.itemIndex}`);
    assert.equal(receipt.item.count, item.quantity);
    const priorPeerUids = new Set([...before.peer.bag, ...before.peer.belt, ...before.peer.equipment].map((row) => row.uidHash));
    assert(!priorPeerUids.has(receipt.item.uidHash), 'Delivery reused a receiver UID');
    if (!priorPeerUids.has(uidHash)) assert.equal(receipt.item.uidHash, uidHash);
    const source = before.primary.bag.filter((row) => row.uidHash === uidHash);
    assert.equal(source.length, 1);
    expected.primary.bag = expected.primary.bag.filter((row) => row.uidHash !== uidHash);
    const receiverCells = new Set(before.peer.bag.filter((row) => row.container === 'bag1').map((row) => row.slot));
    const targetSlot = receiverCells.has(item.slot)
      ? Array.from({ length: 40 }, (_, i) => i).find((i) => !receiverCells.has(i)) : item.slot;
    assert(Number.isInteger(targetSlot));
    expected.peer.bag.push({ ...source[0], uidHash: receipt.item.uidHash, slot: targetSlot });
    expected.peer.bag.sort((a, b) => String(a.container).localeCompare(String(b.container)) || String(a.slot).localeCompare(String(b.slot)) || a.uidHash.localeCompare(b.uidHash));
    proof.delivered = { fromUidHash: uidHash, toUidHash: receipt.item.uidHash, oldReceiverUidCollision: priorPeerUids.has(uidHash), itemIndex: receipt.item.itemIndex, count: item.quantity, toBagSlot: targetSlot, gainedItemBeforeTerminal: true };
    for (const role of ['primary', 'peer']) {
      assert.equal(trace.filter((row) => row.role === role && row.packet === 'TradeConfirm').length, 1, `Terminal confirmations on ${role}`);
    }
  } else {
    if (mode !== 'deposit') {
      const occupied = new Set(own('primary').inventoryItems.map(cells));
      const to = Array.from({ length: 40 }, (_, i) => i).find((i) => !occupied.has(i));
      assert(Number.isInteger(to));
      const after = send('primary', { type: 'retrieveTradeItem', from: 0, to });
      const ack = await packet('primary', 'RetrieveTradeItem', after, (p) => p.from === 0 && p.to === to);
      assert.equal(ack.payload.success, true);
      const uidHash = sha(String(item.uniqueId));
      const beforeItem = expected.primary.bag.find((row) => row.uidHash === uidHash);
      assert(beforeItem && beforeItem.slot === item.slot && beforeItem.quantity === item.quantity);
      beforeItem.slot = to;
      expected.primary.bag.sort((a, b) => String(a.container).localeCompare(String(b.container)) || String(a.slot).localeCompare(String(b.slot)) || a.uidHash.localeCompare(b.uidHash));
      proof.retrieved = { uidHash, fromBagSlot: item.slot, toBagSlot: to, count: item.quantity };
    }
    proof.peerHadOfferBeforeCancel = trace.some((row) => row.role === 'peer' && row.snapshot && row.trade && row.trade !== 'missing');
    const after = send('primary', { type: 'tradeCancel' });
    const cancelOrder = number;
    await packet('primary', 'TradeCancel', after, (p) => p.unlock === false);
    await snapshot('primary', after, (p) => p.stage5Systems?.trade === null);
    const closeBoundary = mode === 'observe'
      ? cancelOrder : (await peerTerminal('TradeCancel', cancelOrder)).order;
    await receiveSnapshot('peer', null, closeBoundary);
    proof.peerState = { active: own('peer').trade.active, uncertain: own('peer').trade.uncertain };
    const peerAccepts = trace.filter((row) => row.role === 'peer' && row.packet === 'TradeAccept');
    const peerTerminals = trace.filter((row) => row.role === 'peer' && row.packet === 'TradeCancel' && row.unlock === false);
    proof.peerAcceptOrders = peerAccepts.map((row) => row.order);
    proof.peerTerminalOrders = peerTerminals.map((row) => row.order);
    proof.peerNullSnapshotOrders = trace.filter((row) => row.role === 'peer' && row.snapshot && row.trade === null).map((row) => row.order);
    if (mode !== 'observe') {
      assert.equal(peerAccepts.length, 1);
      assert.equal(peerTerminals.length, 1);
      assert(peerAccepts[0].order < peerTerminals[0].order);
      assert(proof.peerNullSnapshotOrders.some((order) => order > peerTerminals[0].order));
      assert.deepEqual(proof.peerState, { active: false, uncertain: false });
      // Reuse the very same sessions after the terminal receipt. An ordinary
      // fresh invite may open again after Crystal's request cadence permits it.
      phase = 'reopen';
      await delay(1250);
      await openTrade();
      await receiveSnapshot('peer', 'open');
      const closeAfter = send('primary', { type: 'tradeCancel' });
      const secondCancelOrder = number;
      await packet('primary', 'TradeCancel', closeAfter, (p) => p.unlock === false);
      await snapshot('primary', closeAfter, (p) => p.stage5Systems?.trade === null);
      const closed = await peerTerminal('TradeCancel', secondCancelOrder);
      await receiveSnapshot('peer', null, closed.order);
      assert.deepEqual({ active: own('peer').trade.active, uncertain: own('peer').trade.uncertain }, { active: false, uncertain: false });
      proof.sameConnectionReopen = true;
    }
  }
  proof.assetsAfter = all();
  assert.deepEqual(proof.assetsAfter, expected);
  proof.exactAssets = true;
  await relog(expected);
  proof.passed = mode !== 'observe' || proof.peerTerminalOrders.length === 1 && proof.peerState.uncertain === false;
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
  console.log(JSON.stringify({ passed: proof.passed, phase, rows: trace.length, peer: proof.peerState, accepts: proof.peerAcceptOrders, terminals: proof.peerTerminalOrders, relog: proof.sameCharacterReadback, stopped: proof.ownedGatewayStopped, failure: proof.failure }));
  if (!proof.passed || !proof.ownedGatewayStopped || !proof.sourceFixtureUnchanged) process.exitCode = 1;
}
