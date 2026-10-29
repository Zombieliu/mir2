// Two ordinary sockets only: functional PvP, never a capacity acceptance run.
import fs from 'node:fs/promises';
import path from 'node:path';
import crypto from 'node:crypto';
import { pathToFileURL } from 'node:url';
import { CapacityClient, ordinaryLogin, drain } from './playtest-capacity-soak.mjs';
import { LoginBudget, LOGIN_WINDOW_MS, validateCapacityPool, BoundedEvidence } from './playtest-capacity-soak-core.mjs';
import { outsideRepository, validateEndpoint, observedPlayerId } from './playtest-multiplayer-smoke.mjs';
import { loadProtocolCollisionMap, findProtocolWalkPath, protocolMapCellIsWalkable } from './quest-agent/protocol-navigation.mjs';

const ownErrors = new WeakSet();
const assert = (ok, text) => { if (!ok) { const error=new Error(text);ownErrors.add(error);throw error; } };
const pause = ms => new Promise(resolve => setTimeout(resolve, ms));
const distance = (a, b) => Math.max(Math.abs(a.x - b.x), Math.abs(a.y - b.y));
const owner = client => client.snapshot?.entities?.find(e => e.objectId === client.snapshot.playerObjectId);
const inside = (p, b) => b && p.x >= b.minX && p.x <= b.maxX && p.y >= b.minY && p.y <= b.maxY;
const directions = new Map([["0,-1",'Up'],["1,-1",'UpRight'],["1,0",'Right'],["1,1",'DownRight'],["0,1",'Down'],["-1,1",'DownLeft'],["-1,0",'Left'],["-1,-1",'UpLeft']]);
const integer = (x, lo, hi) => Number.isSafeInteger(x) && x >= lo && x <= hi;

export function pvpProbeOptions(options) {
  assert(options?.realm === 'isolated', 'PvP probe requires realm isolated');
  const durationSeconds = options.durationSeconds === undefined ? 30 : Number(options.durationSeconds);
  assert(integer(durationSeconds, 20, 45), 'PvP durationSeconds must be20..45');
  const indices = typeof options.indices === 'string' ? options.indices.split(',').map(Number) : options.indices ?? [0, 1];
  assert(Array.isArray(indices) && indices.length === 2 && indices.every(i => integer(i, 0, 99)) && indices[0] !== indices[1], 'Exactly two distinct pool indices required');
  const fixture = options.fixture;
  assert(fixture && typeof fixture.mapFileName === 'string' && fixture.mapFileName.length > 0 &&
    Array.isArray(fixture.positions) && fixture.positions.length === 2 && fixture.positions.every(p => integer(p?.x, 0, 65535) && integer(p?.y, 0, 65535)) &&
    distance(...fixture.positions) === 1, 'Fixture requires a map and two distinct adjacent ordinary positions');
  return { ...options, durationSeconds, indices, fixture: { mapFileName: fixture.mapFileName, positions: fixture.positions.map(({x,y}) => ({x,y})) } };
}

// The sole added command is the documented ordinary browser ChangeAMode5.
export class PvpProbeClient extends CapacityClient {
  send(command) {
    if (command?.type !== 'changeAMode') return super.send(command);
    assert(command.mode === 5 && Object.keys(command).sort().join(',') === 'mode,type', 'Only ordinary all-target mode5 is added');
    if (!this.draining) this.context.guard.assert();
    this.budget.reserve(); this.bytes.sent += Buffer.byteLength(JSON.stringify(command));
    this.record('sent', command); this.ws.send(JSON.stringify(command));
  }
  observe(message) {
    super.observe(message);
    if (message.type === 'worldSnapshot' && message.payload?.playerObjectId != null) {
      try { this.probeRaw = pvpRawScalar(message.payload, this.label, this.events.at(-1).sequence); }
      catch (error) { this.probeRawFailure = error; }
    }
  }
}

// Only complete raw snapshots provide exact HP/inventory proof. No percentages.
export function pvpRawScalar(raw, name, sequence) {
  const self = raw?.entities?.find(e => e.objectId === raw.playerObjectId && e.name === name && e.kind === 'selfPlayer');
  assert(self && integer(self.objectId, 1, 0xffffffff) && self.dead === false && integer(self.x, 0, 65535) && integer(self.y, 0, 65535), 'PvP exact owner/life/position absent');
  assert(integer(raw.playerHp, 1, 0x7fffffff) && integer(raw.playerMaxHp, 1, 0x7fffffff) && raw.playerHp <= raw.playerMaxHp && typeof raw.inSafeZone === 'boolean', 'PvP exact raw HP/safe-zone absent');
  assert(Array.isArray(raw.inventoryItems) && Array.isArray(raw.beltItems), 'PvP complete raw inventory and belt required');
  const ids = new Set(), hpItems = [];
  for (const [grid, rows] of [['Inventory', raw.inventoryItems], ['Belt', raw.beltItems]]) for (const item of rows) {
    const uid = typeof item?.uniqueId === 'string' && /^\d+$/.test(item.uniqueId) ? Number(item.uniqueId) : item?.uniqueId;
    assert(item && integer(uid, 0, Number.MAX_SAFE_INTEGER) && !ids.has(uid) && integer(item.quantity, 1, 0xffffffff), 'PvP malformed/duplicate inventory UID or quantity');
    ids.add(uid);
    // WorldItemSnapshot exposes identity in tooltipSource; nullable aliases
    // are absence, never a conflicting template. Present values stay strict.
    const source=item.tooltipSource, resolved=source?.realInfo??source?.info;
    const indices=[item.itemIndex,item.item_index,item.info?.item_index,item.realInfo?.item_index,
      source?.info?.item_index,source?.info?.itemIndex,source?.realInfo?.item_index,source?.realInfo?.itemIndex].filter(x=>x!=null);
    const template=resolved?.item_index??resolved?.itemIndex??item.realInfo?.item_index??item.info?.item_index??item.itemIndex??item.item_index;
    if (item.name === '(HP)DrugSmall' || indices.includes(658)) {
      assert(template === 658 && item.name === '(HP)DrugSmall' && indices.length>0 && indices.every(x=>x===658), 'Ambiguous HP658 identity');
      assert(grid === 'Belt' ? item.container === 'belt' : ['bag1','bag2'].includes(item.container), 'HP658 container is not ordinarily usable');
      hpItems.push({ uniqueId: uid, grid, quantity: item.quantity });
    }
  }
  const stock = hpItems.reduce((n, i) => n + i.quantity, 0);
  assert(Number.isSafeInteger(stock), 'PvP HP stock overflow');
  return { sequence, ownerId: self.objectId, map: raw.mapFileName, x: self.x, y: self.y,
    hp: raw.playerHp, maxHp: raw.playerMaxHp, inSafeZone: raw.inSafeZone, hpStock: stock, hpItems:hpItems.sort((a,b)=>a.uniqueId-b.uniqueId) };
}
const publicScalar = row => ({ sequence: row.sequence, map: row.map, x: row.x, y: row.y, hp: row.hp, maxHp: row.maxHp, hpStock: row.hpStock, inSafeZone: row.inSafeZone });

export function pvpHitEvidence(clients,cursors,targetIds,attackerIds) {
  const rows=clients.map((c,i)=>{
    const find=(packet,predicate)=>c.events.find(e=>e.direction==='received'&&e.sequence>cursors[i]&&e.packet===packet&&predicate(e.payload));
    return {struck:find('ObjectStruck',p=>p?.objectId===targetIds[i]&&p.attackerId===attackerIds[i]),
      health:find('ObjectHealth',p=>p?.objectId===targetIds[i]&&Number.isFinite(p.percent)&&p.percent>0&&p.percent<100),
      damage:find('DamageIndicator',p=>p?.objectId===targetIds[i]&&Number.isFinite(p.damage)&&p.damage>0),
      died:find('ObjectDied',p=>p?.objectId===targetIds[i])};
  });
  if(!rows.every(r=>r.struck&&r.health&&!r.died)||rows[0].health.payload.percent!==rows[1].health.payload.percent||!rows.some(r=>r.damage))return null;
  return {targetIds:[...targetIds],attackerIds:[...attackerIds],died:false,healthPercent:rows[0].health.payload.percent,
    receipts:rows.map(r=>({struck:r.struck.sequence,health:r.health.sequence,...(r.damage?{damage:r.damage.sequence}:{})}))};
}

export async function exercisePvpPair(clients, options, services = {}) {
  const now = services.now ?? Date.now, sleep = services.sleep ?? pause, emit = services.emit ?? (() => {});
  const map = await (services.loadMap ?? loadProtocolCollisionMap)(options.fixture.mapFileName);
  const exerciseStartedAt=now(), deadline = exerciseStartedAt + options.durationSeconds * 1000;
  const identities = clients.map(c => ({ name: c.label, id: owner(c)?.objectId }));
  function check() {
    assert(now() < deadline, 'PvP functional window exhausted');
    clients.forEach((c, i) => {
      c.context?.guard.assert(); if (c.failure) throw c.failure;
      const p = owner(c);
      assert(!c.closed && p && p.name === identities[i].name && p.objectId === identities[i].id && p.dead === false && c.snapshot.mapFileName === options.fixture.mapFileName, 'PvP owner/map/life/transport changed');
      assert(c.snapshot.inSafeZone === false, 'PvP requires actual non-safe-zone actors');
    });
  }
  async function fresh(c) {
    check(); const cursor = c.sequence; await c.refresh(); check();
    if (c.probeRawFailure) throw c.probeRawFailure;
    const raw = c.probeRaw;
    assert(raw && raw.sequence > cursor && raw.ownerId === identities[clients.indexOf(c)].id && raw.map === options.fixture.mapFileName, 'Fresh exact PvP owner snapshot missing');
    return structuredClone(raw);
  }
  function mutual() {
    clients.forEach((c, i) => {
      const peerId = observedPlayerId(c, identities[1-i].name);
      const peer = c.snapshot.entities.find(e => e.objectId === peerId && e.name === identities[1-i].name && e.kind === 'player' && e.dead === false);
      assert(peer && distance(peer, owner(c)) <= 16, 'PvP peer is not mutually visible');
    });
  }
  async function move(c, target) {
    let moves = 0;
    while (distance(owner(c), target)) {
      check(); assert(moves < 32, 'PvP bounded ordinary movement exhausted');
      const from = { ...owner(c) }, occupied = c.snapshot.entities.filter(e => e.objectId !== from.objectId && e.dead !== true);
      const route = findProtocolWalkPath({ map, start: from, target, dynamicObstacles: occupied, maxExpanded: 2048 });
      assert(route?.length >= 2, 'PvP fixture has no ordinary walk path');
      const to = route[1];
      assert(protocolMapCellIsWalkable(map, to) && !(c.snapshot.mapTransfers ?? []).some(t => inside(to, t.bounds)), 'PvP movement would cross collision/transfer');
      await sleep(650); await c.pace(); check();
      assert(distance(owner(c), from) === 0 && !(c.snapshot.mapTransfers??[]).some(t=>inside(to,t.bounds)) && !c.snapshot.entities.some(e => e.objectId !== from.objectId && e.dead !== true && distance(e,to) === 0), 'PvP movement changed or occupied');
      const cursor = c.sequence; c.send({ type:'walk', direction: directions.get((to.x-from.x)+','+(to.y-from.y)) });
      await c.wait(() => c.events.find(e => e.direction === 'received' && e.sequence > cursor && e.packet === 'UserLocation' && e.payload?.x === to.x && e.payload?.y === to.y), 'PvP ordinary owner movement', 2000);
      moves++; check();
    }
    await fresh(c); return moves;
  }
  const before = [];
  for (let i = 0; i < 2; i++) {
    before.push(await fresh(clients[i]));
    assert(distance(before[i], options.fixture.positions[i]) <= 4, 'PvP saved start is too far from the declared fixture');
    await move(clients[i], options.fixture.positions[i]);
  }
  mutual();
  // Real outward-and-return ordinary movement, even when saved homes match.
  const from = { x: owner(clients[0]).x, y: owner(clients[0]).y };
  const excursion = [...directions.keys()].map(k => { const [x,y]=k.split(',').map(Number);return{x:from.x+x,y:from.y+y}; }).find(p =>
    protocolMapCellIsWalkable(map,p) && !(clients[0].snapshot.mapTransfers??[]).some(t=>inside(p,t.bounds)) &&
    !clients[0].snapshot.entities.some(e=>e.objectId!==identities[0].id&&e.dead!==true&&distance(e,p)===0));
  assert(excursion, 'PvP fixture needs a free ordinary adjacent movement tile');
  const movementSteps = await move(clients[0], excursion) + await move(clients[0], from);
  for (const c of clients) {
    const ack = await c.request({ type:'changeAMode', mode:5 }, 'ChangeAMode', 2000); check();
    assert(ack.payload?.mode === 5, 'Ordinary all-target attack mode was not acknowledged');
  }
  await fresh(clients[0]); const victimBefore = await fresh(clients[1]); mutual();
  assert(victimBefore.hp / victimBefore.maxHp >= .8 && victimBefore.hpItems.length > 0, 'PvP victim needs healthy ordinary heldHP658');
  const cursors = clients.map(c => c.sequence); let hit = null, afterDamage = null, attackRequests = 0;
  while (!afterDamage && attackRequests < 16) {
    check(); mutual();
    assert(distance(owner(clients[0]), owner(clients[1])) === 1, 'PvP target moved out of ordinary melee range');
    assert(clients[1].probeRaw.hp / clients[1].probeRaw.maxHp > .5, 'PvP victim reached the no-more-attacks threshold');
    await clients[0].pace(); check(); mutual();
    const attackPeer=clients[0].snapshot.entities.find(e=>e.objectId===observedPlayerId(clients[0],identities[1].name)&&e.name===identities[1].name);
    assert(attackPeer&&attackPeer.dead===false&&distance(owner(clients[0]),attackPeer)===1&&Number.isSafeInteger(clients[1].snapshot.playerHp)&&clients[1].snapshot.playerHp/clients[1].snapshot.playerMaxHp>.5, 'PvP target changed during attack cadence');
    const targets=clients.map(c=>observedPlayerId(c,identities[1].name)),attackers=clients.map(c=>observedPlayerId(c,identities[0].name));
    assert(targets.every(id=>integer(id,1,0xffffffff))&&attackers.every(id=>integer(id,1,0xffffffff)), 'PvP receiver-local identity missing');
    clients[0].send({ type:'attack', objectId: targets[0] }); attackRequests++;
    await sleep(900 + (attackRequests % 3) * 37); check();
    const exact = await fresh(clients[1]);
    hit = pvpHitEvidence(clients, cursors, targets, attackers);
    if (exact.hp < victimBefore.hp && hit && hit.died === false) afterDamage = exact;
  }
  assert(afterDamage && hit, 'No dual-client PvP hit and exact victim HP loss proved');
  emit({ type:'pvpDamage', attackResults:'legacy', attackRequests, before:publicScalar(victimBefore),after:publicScalar(afterDamage),hit });
  const victim = clients[1], doseBefore = await fresh(victim), item = doseBefore.hpItems[0];
  assert(item && doseBefore.hp < doseBefore.maxHp, 'PvP single held dose is not applicable');
  const ack = await victim.request({type:'useItem',uniqueId:item.uniqueId,grid:item.grid},'UseItem',2000); check();
  assert(ack.payload?.success === true && Number(ack.payload.uniqueId) === item.uniqueId && ack.payload.grid === item.grid, 'PvP held dose acknowledgement differs');
  let afterHeal = null; const healEnd = Math.min(deadline, now()+12000);
  while (now() < healEnd) {
    const raw = await fresh(victim), quantity = raw.hpItems.find(i=>i.uniqueId===item.uniqueId)?.quantity ?? 0;
    assert(quantity === item.quantity || quantity === item.quantity-1, 'PvP held dose quantity changed unexpectedly');
    if (quantity === item.quantity-1 && raw.hp > doseBefore.hp) { afterHeal=raw;break; }
    await sleep(250); check();
  }
  assert(afterHeal, 'PvP healing needs actual held UID consumption and fresh exact HP increase');
  emit({ type:'pvpHealing',doses:1,uniqueId:item.uniqueId,grid:item.grid,ackSequence:ack.sequence,quantityBefore:item.quantity,quantityAfter:item.quantity-1,before:publicScalar(doseBefore),after:publicScalar(afterHeal),proofWindowMs:12000,capacityCredit:false });
  mutual();
  return { mutualVisibility:true, movementSteps, exerciseElapsedMs:now()-exerciseStartedAt,configuredMaximumExerciseSeconds:options.durationSeconds,fullDurationSustainedCombatPerformed:false,attackResults:'legacy', attackRequests, launchedAttackCount:null,
    damage: { exactHpLoss:victimBefore.hp-afterDamage.hp,dualClientHit:hit },
    healing: { doses:1,uniqueId:item.uniqueId,grid:item.grid,quantityBefore:item.quantity,quantityAfter:item.quantity-1,exactHpIncrease:afterHeal.hp-doseBefore.hp },
    before:before.map(publicScalar),after:[publicScalar(await fresh(clients[0])),publicScalar(await fresh(clients[1]))] };
}

export function verifyPvpRelogin(saved,raw,original,name) {
  assert(raw&&original&&raw.map===original.map&&raw.x===original.x&&raw.y===original.y&&JSON.stringify(raw.hpItems)===JSON.stringify(original.hpItems),'Ordinary relogin position/held inventory differs from normal save');
  assert(saved?.state?.name===name&&saved.state.mapFileName===raw.map&&saved.state.x===raw.x&&saved.state.y===raw.y,'Final drain identity/position differs');
  return true;
}

async function jsonFile(file) {
  assert(typeof file === 'string' && path.isAbsolute(file), 'Absolute JSON input required');
  const stat = await fs.lstat(file); assert(stat.isFile() && !stat.isSymbolicLink(), 'JSON input must be a regular non-symlink file');
  return JSON.parse(await fs.readFile(file,'utf8'));
}
export async function runPvpProbe(options) {
  const fixture = options.fixture ?? await jsonFile(options.fixtureFile);
  const config = pvpProbeOptions({ ...options, fixture }), endpoint = validateEndpoint(config.endpoint);
  const accountFile = outsideRepository(config.poolFile), output = outsideRepository(config.output);
  assert(path.basename(path.dirname(accountFile)) === 'private', 'Dedicated private pool directory required');
  await fs.mkdir(output,{recursive:true});
  const runId=Date.now()+'-'+crypto.randomBytes(4).toString('hex'),reportPath=path.join(output,runId+'.pvp-report.json'),samplesPath=path.join(output,runId+'.pvp-samples.jsonl');
  const writer = new BoundedEvidence(samplesPath), report = { schemaVersion:1,runId,reportPath,samplesPath,startedAt:new Date().toISOString(),
    mode:'two-client-ordinary-pvp-functional-probe',durationSeconds:config.durationSeconds,durationIsMaximumBudget:true,indices:config.indices,attackResults:'legacy',
    capacityAccepted:false,p0Accepted:false,cleanupErrors:[],checks:{} };
  const allClients=[];let lock,wallTimer,stopped=false;
  const stop=()=>{stopped=true},guard={assert(){assert(!stopped,'PvP finite wall/operator stop');if(writer.failure)throw writer.failure;}};
  const context={endpoint,policy:{activityMode:'native',actionTimeoutMs:2000},attackResults:'legacy',phase:'pvp-probe',
    writer:{add(){}},guard,accountFile,ledgerFile:path.join(path.dirname(accountFile),'capacity-auth-budget.json'),clients:[],allClients,
    cleanupErrors:report.cleanupErrors,add(){}};
  const emit=row=>writer.add({at:Date.now(),...row});
  async function close(c) {
    const cursor=c.sequence,errors=report.cleanupErrors.length;
    const saved=await drain(c,context);
    assert(saved&&c.closed&&!c.inGame&&report.cleanupErrors.length===errors&&c.events.some(e=>e.direction==='received'&&e.sequence>cursor&&e.packet==='LogOutSuccess'),'Normal PvP logout/save acknowledgement incomplete');
    return saved;
  }
  process.on('SIGINT',stop);process.on('SIGTERM',stop);
  try {
    lock=await fs.open(path.join(path.dirname(accountFile),'capacity-run.lock'),'wx',0o600);
    await lock.writeFile(JSON.stringify({runId,pid:process.pid,mode:report.mode}));
    context.pool=validateCapacityPool(await jsonFile(accountFile),endpoint);
    assert(config.indices.every(i=>context.pool.accounts[i]?.state==='ready'),'PvP uses two existing ready saved characters only');
    let ledger;try{ledger=await jsonFile(context.ledgerFile)}catch(e){if(e.code!=='ENOENT')throw e;}
    if(ledger){assert(ledger.schemaVersion===1&&ledger.purpose==='capacity-auth-budget'&&ledger.endpoint===endpoint,'Auth ledger endpoint/schema differs');assert(config.loginNotBefore===undefined,'Existing auth ledger cannot be overridden');}
    const notBefore=ledger?0:config.loginNotBefore===undefined?Date.now()+LOGIN_WINDOW_MS:Date.parse(config.loginNotBefore);
    assert(Number.isFinite(notBefore),'Invalid loginNotBefore');context.loginBudget=new LoginBudget(ledger?.attempts??[],notBefore);
    // Includes auth queue plus bounded setup, exercise and ordinary relogin.
    wallTimer=setTimeout(stop,LOGIN_WINDOW_MS+180000);
    for(const i of config.indices){const c=new PvpProbeClient(endpoint,context.pool.accounts[i],context);context.clients.push(c);allClients.push(c);await ordinaryLogin(c,context);}
    report.checks.session=await exercisePvpPair(context.clients,config,{emit});
    const finals=context.clients.map(c=>structuredClone(c.probeRaw)),saved=[];
    for(const c of context.clients)saved.push(await close(c));
    report.checks.normalLogout=true;
    report.checks.relogin=[];
    for(let i=0;i<2;i++){
      const c=new PvpProbeClient(endpoint,context.pool.accounts[config.indices[i]],context);allClients.push(c);await ordinaryLogin(c,context,'verification');
      await c.refresh();if(c.probeRawFailure)throw c.probeRawFailure;const raw=c.probeRaw,original=finals[i];
      verifyPvpRelogin(saved[i],raw,original,c.label);
      report.checks.relogin.push({actor:i+1,positionAndHpStockPersisted:true,after:publicScalar(raw)});await close(c);
    }
    report.ok=true;
  } catch(error) { report.ok=false;report.error=ownErrors.has(error)?error.message:'Ordinary PvP functional probe failed';emit({type:'probeFailure',category:'ordinary-operation-failed'}); }
  finally {
    clearTimeout(wallTimer);process.off('SIGINT',stop);process.off('SIGTERM',stop);
    for(const c of allClients)if(!c.closed){try{await close(c)}catch{report.cleanupErrors.push({actor:config.indices.indexOf(context.pool?.accounts.indexOf(c.account))+1,step:'normal-drain-incomplete'});}}
    if(lock){try{await lock.close();await fs.unlink(path.join(path.dirname(accountFile),'capacity-run.lock'));}catch{report.cleanupErrors.push({step:'lock-release-failed'});}}
    try{await writer.close()}catch{report.cleanupErrors.push({step:'evidence-close-failed'});}
  }
  report.ok=report.ok===true&&!report.cleanupErrors.length;report.finishedAt=new Date().toISOString();
  report.exerciseElapsedMs=report.checks.session?.exerciseElapsedMs??null;report.totalElapsedMs=Date.parse(report.finishedAt)-Date.parse(report.startedAt);
  await fs.writeFile(reportPath,JSON.stringify(report,null,2)+'\n',{flag:'wx'});return report;
}

if(process.argv[1]&&import.meta.url===pathToFileURL(path.resolve(process.argv[1])).href){
  const args=process.argv.slice(2),options={};
  for(let i=0;i<args.length;i+=2){assert(/^--[a-zA-Z]+$/.test(args[i])&&args[i+1]!==undefined,'Named CLI pairs required');options[args[i].slice(2)]=args[i+1];}
  const report=await runPvpProbe(options);console.log(JSON.stringify({ok:report.ok,reportPath:report.reportPath,samplesPath:report.samplesPath,capacityAccepted:false,p0Accepted:false}));process.exitCode=report.ok?0:1;
}