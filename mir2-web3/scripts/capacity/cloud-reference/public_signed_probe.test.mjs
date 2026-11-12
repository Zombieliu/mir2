import test from 'node:test';
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import net from 'node:net';
import os from 'node:os';
import path from 'node:path';
import { createRequire } from 'node:module';
import { performance } from 'node:perf_hooks';
import { exchangeGuest, guestCookie, SignedClient, parseActorOrdinals, selectProbeActors,
  optionsFrom, probeTransportForActor, ProbeWaitDeadline, completeSignedBootstrap } from './public_signed_probe.mjs';

const WireWebSocket = createRequire(new URL('../../../apps/web/package.json', import.meta.url))('next/dist/compiled/ws');

const origin = 'https://example.invalid';
const cookie = 'mir2_channel_guest=11111111-2222-3333-4444-555555555555';
const proof = { provider: 'directGuest', accountId: 'isolated-player-1', token: 'signed.payload', expiresAt: 9999999999999 };
const response = (body, headers = {}, status = 200) => new Response(JSON.stringify(body), { status, headers });

test('fresh dedicated guest uses public proof exchange without private signing keys or identity headers', async () => {
  const calls = [], secrets = [], actor = {};
  const result = await exchangeGuest(origin, actor, {
    remember: value => secrets.push(value),
    fetchImpl: async (url, options) => {
      calls.push({ url, options });
      return calls.length === 1
        ? response({ provider: 'directGuest', credential: 'guest.proof' }, { 'set-cookie': cookie + '; HttpOnly; Secure; Path=/' })
        : response(proof);
    },
  });
  assert.equal(actor.cookie, cookie); assert.equal(actor.accountId, proof.accountId); assert.equal(result.token, proof.token);
  assert.deepEqual(calls.map(c => c.url), [origin + '/api/channels/guest', origin + '/api/channels/session/exchange']);
  assert.deepEqual(JSON.parse(calls[0].options.body), { provider: 'directGuest' });
  assert.deepEqual(JSON.parse(calls[1].options.body), { provider: 'directGuest', credential: 'guest.proof' });
  for (const call of calls) {
    assert.equal(call.options.redirect, 'error');
    assert.equal(Object.keys(call.options.headers).some(key => /user.agent|forwarded|connecting.ip|authorization/i.test(key)), false);
  }
  assert.deepEqual(secrets, [cookie, 'guest.proof', proof.token]);
});

test('relogin keeps its actual guest cookie and renews the public proof for the same account', async () => {
  const actor = { cookie, accountId: proof.accountId }; let calls = 0;
  const result = await exchangeGuest(origin, actor, { fetchImpl: async (_, options) => {
    if (++calls === 1) { assert.equal(options.headers.cookie, cookie); return response({ provider: 'directGuest', credential: 'renewed.guest' }); }
    return response({ ...proof, token: 'renewed.signed' });
  } });
  assert.equal(calls, 2); assert.equal(result.token, 'renewed.signed'); assert.equal(actor.cookie, cookie);
});

test('an absent durable cookie stops before exchanging or creating a Gateway account', async () => {
  let calls = 0;
  await assert.rejects(exchangeGuest(origin, {}, { fetchImpl: async () => {
    calls++; return response({ provider: 'directGuest', credential: 'proof' });
  } }), /durable dedicated cookie/);
  assert.equal(calls, 1); assert.equal(guestCookie(new Headers()), null);
});

test('changed account identity on renewed cookie fails without account fallback', async () => {
  let calls = 0;
  const actor = { cookie, accountId: 'original-account' };
  await assert.rejects(exchangeGuest(origin, actor, { fetchImpl: async () => ++calls === 1
    ? response({ provider: 'directGuest', credential: 'proof' }) : response(proof) }), /different account/);
  assert.equal(actor.accountId, 'original-account');
});

test('HTTP rejection exposes status and route without echoing sensitive response payload', async () => {
  await assert.rejects(exchangeGuest(origin, {}, { fetchImpl: async () => response({ token: 'must-not-be-echoed' }, {}, 403) }),
    error => error.message.includes('403') && !error.message.includes('must-not-be-echoed'));
});

function stoppedClient(){
  const sent=[],context={connection:0,secrets:[],guard:{stopped:true},assert(){throw new Error('Capacity guard stopped');},emit(){}};
  const c=new SignedClient('wss://example.invalid/ws',{name:'SyntheticDrain'},0,context);c.inGame=true;
  c.ws={readyState:1,send(text){const command=JSON.parse(text);sent.push(command.type);if(command.type==='logOut')c.observe({packet:'LogOutSuccess',payload:{}});},
    close(){this.readyState=3;c.closed=true;},terminate(){this.readyState=3;c.closed=true;}};
  return {c,sent};
}

test('a stopped capacity guard does not poison heartbeats and drain only sends real logout cleanup',async()=>{
  const {c,sent}=stoppedClient();c.heartbeat();assert.equal(c.failure,undefined);assert.deepEqual(sent,[]);
  assert.throws(()=>c.send({type:'walk',direction:'Up'}),/Capacity guard stopped/);
  c.draining=true;
  for(const type of ['walk','chat','attack','passkeyLogin','clientVersion','startGame','newCharacter','qa.giveItem'])
    assert.throws(()=>c.send({type}),/Only keepAlive\/logOut/);
  c.send({type:'keepAlive',time:123});await c.close();
  assert.deepEqual(sent,['keepAlive','logOut']);assert.equal(c.logoutReceipt.packet,'LogOutSuccess');
  assert.equal(c.inGame,false);assert.equal(c.closed,true);assert.equal(c.failure,undefined);
});

test('drain keeps genuine transport failures and does not turn a failed close into normal logout',async()=>{
  const {c}=stoppedClient(),failure=new Error('actual transport failed');c.failure=failure;
  await assert.rejects(c.close(),/actual transport failed/);assert.equal(c.failure,failure);
  const second=stoppedClient().c;second.draining=true;second.ws.send=()=>{throw failure;};second.heartbeat();
  assert.equal(second.failure,failure);
});

function requestClient(){
  const context={connection:0,secrets:[],guard:{stopped:false},assert(){if(this.guard.stopped)throw new Error('request guard stopped');},emit(){}};
  const c=new SignedClient('wss://example.invalid/ws',{name:'SyntheticRequest'},0,context);
  c.ws={readyState:1,send(){}};return c;
}
const busyFor=milliseconds=>{const end=performance.now()+milliseconds;while(performance.now()<end){}};

test('request accepts a timely cached receipt when native batch work delays the final poll beyond the unchanged deadline',async t=>{
  const c=requestClient();let timer;
  t.after(()=>clearTimeout(timer));
  c.ws.send=()=>{timer=setTimeout(()=>{c.observe({packet:'StartGame',payload:{result:4}});busyFor(120);},10);};
  const response=await c.request({type:'startGame',characterIndex:1},'StartGame',100);
  assert.equal(response.payload.result,4);assert.ok(c.startGameTiming.ackElapsedMs<100);
  assert.equal(c.startGameTiming.budgetMs,100);assert.ok(c.startGameTiming.pollObservedAtMs>c.startGameTiming.deadlineAtMs);
  assert.equal(c.startGameTiming.ackElapsedMs,response.monotonicMs-c.startGameTiming.sentAtMs);
});

test('request rejects a truly late cached receipt instead of treating the final poll as extra response budget',async t=>{
  const c=requestClient();let timer;
  t.after(()=>clearTimeout(timer));
  c.ws.send=()=>{timer=setTimeout(()=>{busyFor(120);c.observe({packet:'StartGame',payload:{result:4}});},10);};
  await assert.rejects(c.request({type:'startGame',characterIndex:1},'StartGame',100),/timed out waiting for StartGame/);
  const response=c.events.find(e=>e.packet==='StartGame');assert.ok(response.monotonicMs>c.startGameTiming.deadlineAtMs);
  assert.equal(c.startGameTiming.ackElapsedMs,undefined);
});

test('request preserves received/cursor matching and gateway rejection, guard, transport and closed errors',async()=>{
  for(const kind of ['rejection','guard','transport','closed']){
    const c=requestClient();c.record('received',{packet:'ClientVersion',payload:{result:1}});
    c.ws.send=()=>{
      if(kind==='rejection')c.observe({type:'error',message:'ordinary denial'});
      if(kind==='guard'){c.observe({packet:'ClientVersion',payload:{result:1}});c.context.guard.stopped=true;}
      if(kind==='transport'){c.observe({packet:'ClientVersion',payload:{result:1}});c.failure=new Error('real socket failed');}
      if(kind==='closed')c.closed=true;
    };
    const expected={rejection:/Gateway rejected clientVersion: ordinary denial/,guard:/request guard stopped/,transport:/real socket failed/,closed:/disconnected during ClientVersion/}[kind];
    await assert.rejects(c.request({type:'clientVersion'},'ClientVersion',100),expected);
  }
  const c=requestClient();c.record('received',{packet:'ClientVersion',payload:{old:true}});
  c.ws.send=()=>{c.record('sent',{packet:'ClientVersion',payload:{wrongDirection:true}});c.observe({packet:'KeepAlive',payload:{}});c.observe({packet:'ClientVersion',payload:{fresh:true}});};
  const response=await c.request({type:'clientVersion'},'ClientVersion',100);assert.equal(response.payload.fresh,true);
});

test('explicit pool selection retains original wallet references/order and maps selected actors to dense local ordinals',()=>{
  const pool={actors:Array.from({length:100},(_,ordinal)=>({ordinal,name:'Synthetic'+ordinal,walletIdentity:{kind:'synthetic-ed25519-wallet',secretKey:'offline-synthetic',
    address:'offline-address-'+ordinal,accountId:'offline-account-'+ordinal,characterIndex:ordinal}}))};
  const original=JSON.stringify(pool),indices=parseActorOrdinals('75,0',2),selected=selectProbeActors(pool,2,indices);
  assert.deepEqual(selected.map(({localOrdinal,originalOrdinal})=>({localOrdinal,originalOrdinal})),[{localOrdinal:0,originalOrdinal:75},{localOrdinal:1,originalOrdinal:0}]);
  assert.equal(selected[0].actor,pool.actors[75]);assert.equal(selected[1].actor,pool.actors[0]);assert.equal(JSON.stringify(pool),original);
  assert.throws(()=>selectProbeActors(pool,2),/explicit --actor-ordinals/);
  const small={actors:pool.actors.slice(0,2)};assert.deepEqual(selectProbeActors(small,2).map(x=>x.originalOrdinal),[0,1]);
});

test('selector rejects wrong counts, duplicates, invalid indices and uninitialized identities rather than creating a new actor',()=>{
  for(const value of ['75','75,0,1','75,75','-1,0','x,0','1.5,0','75,'])assert.throws(()=>parseActorOrdinals(value,2),/Actor ordinals/);
  const pool={actors:[{ordinal:0,walletIdentity:{kind:'synthetic-ed25519-wallet',secretKey:'offline',address:'offline',accountId:'offline',characterIndex:0}},{ordinal:1}]};
  assert.throws(()=>selectProbeActors(pool,2,[0,1]),/existing dedicated wallet and character/);
  assert.throws(()=>selectProbeActors(pool,2,[75,0]),/missing or inconsistent/);
  assert.equal(pool.actors.length,2);assert.equal(pool.actors[1].walletIdentity,undefined);
});

test('event wait accepts an on-time cached receipt after polling wakes past its deadline',async t=>{
  const c=requestClient();let receipt,timer;
  t.after(()=>clearTimeout(timer));
  timer=setTimeout(()=>{receipt={packet:'LogOutSuccess',monotonicMs:performance.now()};busyFor(120);},10);
  const began=performance.now(),result=await c.wait(()=>receipt,'timed cleanup receipt',100);
  assert.equal(result,receipt);assert.ok(receipt.monotonicMs-began<100);assert.ok(performance.now()-began>100);
});

test('event wait rejects a late receipt even if it is cached before the delayed poll resumes',async t=>{
  const c=requestClient();let receipt,timer;
  t.after(()=>clearTimeout(timer));
  timer=setTimeout(()=>{busyFor(120);receipt={packet:'LogOutSuccess',monotonicMs:performance.now()};},10);
  await assert.rejects(c.wait(()=>receipt,'late cleanup receipt',100),/timed out waiting for late cleanup receipt/);
});

test('expired boolean and un-timestamped state predicates have no evidence for a late success',async()=>{
  const c=requestClient();
  await assert.rejects(c.wait(()=>true,'unknown boolean',-1),/timed out waiting for unknown boolean/);
  await assert.rejects(c.wait(()=>({ready:true}),'unknown state',-1),/timed out waiting for unknown state/);
  const now=performance.now();
  assert.equal((await c.wait(()=>({monotonicMs:now-200}),'on-time cached negative remainder',-100)).monotonicMs,now-200);
  await assert.rejects(c.wait(()=>({monotonicMs:performance.now()}),'truly late negative remainder',-100),/timed out waiting/);
});

test('functional transport option defaults to native gzip and accepts only the three explicit profiles without widening cohort bounds',()=>{
  const args=['--private-directory',path.join(os.tmpdir(),'mir2-offline-options-private'),
    '--output',path.join(os.tmpdir(),'mir2-offline-options-public'),'--monitor',path.join(os.tmpdir(),'mir2-offline-monitor.json')];
  const defaults=optionsFrom(args);
  assert.equal(defaults.transportProfile,'native-gzip');assert.equal(defaults.count,3);
  assert.equal(probeTransportForActor(undefined,29),'native-gzip');
  for(const profile of ['native-gzip','plain-Web','mixed'])
    assert.equal(optionsFrom([...args,'--transport-profile',profile]).transportProfile,profile);
  for(const profile of ['web','native','Mixed','',null])assert.throws(()=>probeTransportForActor(profile,0),/Transport profile/);
  assert.throws(()=>optionsFrom([...args,'--transport-profile','invalid']),/Transport profile/);
  assert.throws(()=>optionsFrom([...args,'--actors','1']),/bounded to 2\.\.3/);
  assert.throws(()=>optionsFrom([...args,'--actors','4']),/bounded to 2\.\.3/);
});

test('mixed functional selection follows original actor parity across selected order and relogin with unchanged Web/native setup budgets',()=>{
  const pool={actors:Array.from({length:30},(_,ordinal)=>({ordinal,name:'Synthetic'+ordinal,walletIdentity:{kind:'synthetic-ed25519-wallet',
    secretKey:'offline-only',address:'offline-address',accountId:'offline-account',characterIndex:ordinal}}))};
  for(const ordinals of [[0,29],[29,0]]){
    const selected=selectProbeActors(pool,2,ordinals),context={connection:0,
      transportForActor:ordinal=>probeTransportForActor('mixed',selected[ordinal].originalOrdinal)};
    for(let connection=0;connection<2;connection++)for(const {actor,localOrdinal,originalOrdinal}of selected){
      const c=new SignedClient('wss://example.invalid/ws',actor,localOrdinal,context);
      assert.equal(c.transportProfile,originalOrdinal===0?'plain-Web':'native-gzip');
      assert.equal(c.setupBudgetMs,originalOrdinal===0?60000:20000);assert.equal(c.actor,pool.actors[originalOrdinal]);
    }
  }
  assert.equal(probeTransportForActor('plain-Web',29),'plain-Web');assert.equal(probeTransportForActor('native-gzip',0),'native-gzip');
  for(const ordinal of [-1,1.5,NaN,undefined])assert.throws(()=>probeTransportForActor('mixed',ordinal),/Original actor ordinal/);
});

// Raw loopback upgrade peer: it writes a real validated Close frame immediately,
// then deliberately withholds its TCP FIN on an allowHalfOpen socket.
async function closingFixture(t,{finDelayMs=250,failDiagnosticObserver=false,profile='native-gzip'}={}){
  const rows=[],commands=[],sockets=new Set(),timers=new Set();let client,receivedClose;
  const validCloseReceived=new Promise(resolve=>{receivedClose=resolve;});
  const reasonMarker='close-reason-never-recorded';
  const frame=(opcode,payload)=>Buffer.concat([Buffer.from([0x80|opcode,payload.length]),payload]);
  const sendPacket=(socket,packet)=>socket.write(frame(1,Buffer.from(JSON.stringify({packet,payload:{}}))));
  const server=net.createServer({allowHalfOpen:true},socket=>{
    sockets.add(socket);socket.on('close',()=>sockets.delete(socket));socket.on('error',()=>{});
    const receiver=new WireWebSocket.Receiver({isServer:true,maxPayload:4096});let headers=Buffer.alloc(0),upgraded=false;
    receiver.on('error',()=>socket.destroy());
    receiver.on('message',data=>{
      const command=JSON.parse(data.toString());commands.push(command.type);
      if(command.type==='clientVersion'){sendPacket(socket,'ClientVersion');sendPacket(socket,'Connected');}
      if(command.type==='logOut')sendPacket(socket,'LogOutSuccess');
    });
    receiver.once('conclude',()=>{
      const payload=Buffer.concat([Buffer.from([0x03,0xe8]),Buffer.from(reasonMarker)]);
      socket.write(frame(8,payload));
      if(finDelayMs!==null){const timer=setTimeout(()=>socket.end(),finDelayMs);timers.add(timer);}
    });
    socket.on('data',chunk=>{
      if(upgraded){receiver.write(chunk);return;}
      headers=Buffer.concat([headers,chunk]);const end=headers.indexOf('\r\n\r\n');if(end<0)return;
      const key=/\r\nsec-websocket-key:\s*([^\r\n]+)/i.exec(headers.subarray(0,end).toString())?.[1];
      const accept=crypto.createHash('sha1').update(key+'258EAFA5-E914-47DA-95CA-C5AB0DC85B11').digest('base64');
      socket.write('HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: '+accept+'\r\n\r\n');
      upgraded=true;if(headers.length>end+4)receiver.write(headers.subarray(end+4));headers=null;
    });
  });
  t.after(async()=>{
    clearInterval(client?.timer);client?.ws?.terminate();for(const timer of timers)clearTimeout(timer);
    for(const socket of sockets)socket.destroy();await new Promise(resolve=>server.close(resolve));
  });
  await new Promise((resolve,reject)=>{server.once('error',reject);server.listen(0,'127.0.0.1',resolve);});
  const context={connection:0,secrets:[],webBase:origin,transportProfile:profile,assert(){},emit(row){
    rows.push(row);if(row.edge==='validCloseFrameReceived')receivedClose(row);
    if(failDiagnosticObserver&&row.kind==='webSocketClosing')throw new Error('offline observer failure');
  }};
  client=new SignedClient('ws://127.0.0.1:'+server.address().port+'/ws',{name:'SyntheticLoopbackClose'},29,context);
  await client.connect();client.inGame=true;
  return {client,rows,commands,validCloseReceived,reasonMarker};
}

test('wire Close receipt is traced before delayed peer TCP FIN and does not complete the existing ws close wait',{timeout:5000},async t=>{
  const {client,rows,commands,validCloseReceived,reasonMarker}=await closingFixture(t);let complete=false;
  const closing=client.close().then(()=>{complete=true;});
  const frame=await validCloseReceived;
  assert.equal(frame.closeFrameReceived,true);assert.equal(frame.closeFrameSent,true);
  assert.equal(frame.socketEnded,false);assert.equal(frame.socketDestroyed,false);
  assert.equal(frame.readyState,WireWebSocket.CLOSING);assert.equal(client.closed,false);assert.equal(complete,false);
  await closing;
  const trace=rows.filter(row=>row.kind==='webSocketClosing'),end=trace.find(row=>row.edge==='socketReadableEnd'),closed=trace.find(row=>row.edge==='webSocketClose');
  assert.ok(end.monotonicMs-frame.monotonicMs>=150);assert.ok(closed.monotonicMs>=end.monotonicMs);
  assert.equal(client.closed,true);assert.equal(client.inGame,false);assert.equal(client.failure,undefined);
  assert.deepEqual(commands,['clientCapabilities','clientVersion','logOut']);
  assert.equal(new Set(trace.map(row=>row.edge)).size,trace.length);assert.ok(trace.length<=7);
  for(const row of trace){assert.equal(row.actorOrdinal,29);assert.equal(row.connection,1);assert.match(row.atUtc,/^\d{4}-\d{2}-\d{2}T/);assert.ok(Number.isFinite(row.monotonicMs));}
  for(const privateValue of [reasonMarker,client.actor.name,'127.0.0.1','example.invalid'])assert.equal(JSON.stringify(trace).includes(privateValue),false);
  t.diagnostic(JSON.stringify({loopbackCloseTrace:trace}));
});

test('a real Close frame without peer FIN still fails the unchanged five-second close-event deadline with bounded flags',{timeout:10000},async t=>{
  const {client,rows,commands,validCloseReceived}=await closingFixture(t,{finDelayMs:null});
  const closing=client.close(),rejected=assert.rejects(closing,/WebSocket close was not acknowledged within 5 seconds/);
  await validCloseReceived;await rejected;
  const timeout=rows.find(row=>row.edge==='closeEventDeadlineExceeded');
  assert.ok(timeout);assert.equal(timeout.deadlineMs,5000);assert.equal(timeout.afterForcedTerminate,true);
  assert.equal(timeout.closeFrameReceived,true);assert.equal(timeout.closeFrameSent,true);
  assert.equal(timeout.socketEnded,false);assert.equal(timeout.socketDestroyed,true);assert.equal(timeout.readyState,WireWebSocket.CLOSING);
  assert.equal(client.logoutReceipt.packet,'LogOutSuccess');assert.equal(client.inGame,false);
  assert.deepEqual(commands,['clientCapabilities','clientVersion','logOut']);
  t.diagnostic(JSON.stringify({loopbackCloseTimeout:timeout}));
});

test('throwing close diagnostic observers cannot change ordinary logout or transport lifecycle',{timeout:5000},async t=>{
  const {client,rows,commands}=await closingFixture(t,{failDiagnosticObserver:true,profile:'plain-Web'});
  await client.close();
  assert.equal(client.closed,true);assert.equal(client.failure,undefined);assert.equal(client.inGame,false);
  assert.deepEqual(commands,['clientVersion','logOut']);
  assert.ok(rows.some(row=>row.edge==='validCloseFrameReceived'));assert.ok(rows.some(row=>row.edge==='webSocketClose'));
});

function bootstrapFixture(t,{profile='native-gzip',observe=false,ackDelay=10,readyDelay=20,pollDelay=readyDelay??ackDelay??0}={}){
  const descriptor=Object.getOwnPropertyDescriptor(performance,'now'),clock={now:100000};
  Object.defineProperty(performance,'now',{configurable:true,value:()=>clock.now});
  t.after(()=>descriptor?Object.defineProperty(performance,'now',descriptor):delete performance.now);
  const h={clock,base:clock.now,rows:[],commands:[],waits:[]};
  const actor={name:'SyntheticBootstrap',walletIdentity:{kind:'synthetic-ed25519-wallet',secretKey:'offline-only-secret',
    address:'offline-only-address',accountId:'offline-only-account',characterIndex:7}};
  const context={connection:0,secrets:['offline-only-secret'],transportProfile:profile,campaignMode:'explore',observeLateBootstrap:observe,
    firstAttemptAdmissionOk:true,allSloPassed:true,assert(){if(h.guardFailed)throw new Error('synthetic guard stopped');},emit(row){
      h.rows.push(structuredClone(row));if(row.kind==='bootstrapDiagnosticObservationStarted')h.onObservationStart?.();
    }};
  const c=new SignedClient('wss://example.invalid/ws',actor,29,context);Object.assign(h,{c,context});
  const owner=()=>({objectId:1000,name:actor.name,kind:'selfPlayer',x:290,y:616,dead:false});
  c.sequence=4;c.events=[{direction:'received',sequence:1,packet:'StartGame',payload:{result:4},monotonicMs:h.base-100},
    {direction:'received',sequence:2,packet:'UserInformation',payload:{objectId:1000,name:actor.name,hp:18,location:{x:290,y:616}},monotonicMs:h.base-100},
    {direction:'received',sequence:3,packet:'MapInformation',payload:{fileName:'0'},monotonicMs:h.base-100},
    {direction:'received',sequence:4,type:'worldSnapshot',ownerNameMatches:true,monotonicMs:h.base-100}];
  c.snapshot={mapFileName:'0',playerObjectId:1000,playerHp:18,entities:[owner()]};
  h.start=(delay,result=4)=>{clock.now=h.base+delay;c.observe({packet:'StartGame',payload:{result}});};
  h.complete=(delay,name=actor.name)=>{
    clock.now=h.base+delay;c.observe({packet:'UserInformation',payload:{objectId:1000,name,hp:18,location:{x:290,y:616}}});
    c.observe({packet:'MapInformation',payload:{fileName:'0'}});
    c.observe({type:'worldSnapshot',payload:{mapFileName:'0',playerObjectId:1000,playerHp:18,entities:[{...owner(),name}],
      inventoryItems:[],equipmentItems:[],beltItems:[]}});
  };
  c.ws={readyState:1,send(text){const command=JSON.parse(text);h.commands.push(command);assert.equal(command.type,'startGame');
    if(ackDelay!==null)h.start(ackDelay);if(readyDelay!==null)h.complete(readyDelay);clock.now=h.base+pollDelay;h.onSend?.();}};
  const actualWait=c.wait.bind(c);
  c.wait=(predicate,label,budget)=>{
    const deadline=clock.now+budget;h.waits.push({label,budget,deadline});
    // Advance an empty synthetic wait to its real deadline; retain the actual
    // SignedClient wait/request code and its unchanged20/60s product budgets.
    return actualWait(()=>{const result=predicate();if(!result)clock.now=Math.max(clock.now,deadline+1);return result;},label,budget);
  };
  return h;
}

test('default-off bootstrap still stops at the original native20s/Web60s post-ACK deadline without using old cached owner data',async t=>{
  for(const profile of ['native-gzip','plain-Web'])await t.test(profile,async t=>{
    const h=bootstrapFixture(t,{profile,readyDelay:null});
    await assert.rejects(completeSignedBootstrap(h.c,h.context,'admission',7),error=>error instanceof ProbeWaitDeadline&&/complete authoritative owner snapshot/.test(error.message));
    const miss=h.context.admissionSloMisses[0],budget=profile==='plain-Web'?60000:20000;
    assert.equal(h.c.setupBudgetMs,budget);assert.equal(miss.phase,'postAckCompleteBootstrap');assert.equal(miss.originalBudgetMs,budget);
    assert.equal(miss.originalDeadlineAtMs,h.base+10+budget);assert.equal(h.c.startGameTiming.budgetMs,20000);
    assert.equal(h.context.firstAttemptAdmissionOk,false);assert.equal(h.context.allSloPassed,false);
    assert.equal(h.c.bootstrapProof,undefined);assert.equal(h.context.diagnosticObserveUsed,undefined);assert.equal(h.c.snapshot,null);
    assert.deepEqual(h.commands,[{type:'startGame',characterIndex:7}]);assert.equal(h.c.bootstrapCursor,4);
  });
});

test('opt-in late StartGame observation reuses the same request/connection and records an unqualified real bootstrap',async t=>{
  const h=bootstrapFixture(t,{observe:true,ackDelay:null,readyDelay:null});
  h.onObservationStart=()=>{h.start(20010);h.complete(20020);};
  await completeSignedBootstrap(h.c,h.context,'admission',7);
  const miss=h.context.admissionSloMisses[0],proof=h.c.bootstrapProof;
  assert.equal(miss.phase,'startGameAck');assert.equal(miss.originalBudgetMs,20000);assert.equal(miss.originalDeadlineAtMs,h.base+20000);
  assert.equal(miss.observationDeadlineAtMs,h.base+80000);assert.equal(miss.bootstrapCursor,4);
  assert.equal(miss.startGameRequestSequence,5);assert.equal(miss.startGameSentAtMs,h.base);
  assert.equal(proof.ready,true);assert.equal(proof.strictSetupDeadlinePassed,false);assert.equal(proof.diagnosticAcceptedLate,true);
  assert.equal(proof.startGameAckBudgetMs,20000);assert.equal(proof.postAckBootstrapBudgetMs,20000);assert.equal(proof.startGameAckElapsedMs,20010);
  assert.equal(h.context.firstAttemptAdmissionOk,false);assert.equal(h.context.allSloPassed,false);assert.equal(h.context.diagnosticObserveUsed,true);
  assert.equal(h.c.connection,1);assert.equal(h.c.bootstrapCursor,4);assert.equal(h.c.inGame,true);
  assert.deepEqual(h.commands,[{type:'startGame',characterIndex:7}]);assert.equal(h.context.admissionSloMisses.length,1);
  assert.equal(h.rows.filter(r=>r.kind==='bootstrapDiagnosticLateCompletion').length,1);
  assert.ok(h.waits.filter(w=>w.label.startsWith('late original')).every(w=>w.deadline===h.base+80000));
  for(const secret of [h.c.actor.name,...Object.values(h.c.actor.walletIdentity).filter(v=>typeof v==='string')])
    assert.equal(JSON.stringify(h.context.admissionSloMisses).includes(secret),false);
  t.diagnostic(JSON.stringify({admissionSloMiss:miss,lateCompletion:h.rows.find(r=>r.kind==='bootstrapDiagnosticLateCompletion')}));
});

test('on-time StartGame plus late full bootstrap uses the existing profile deadline and a single absolute diagnostic grace',async t=>{
  for(const profile of ['native-gzip','plain-Web'])await t.test(profile,async t=>{
    const h=bootstrapFixture(t,{observe:true,profile,readyDelay:null}),budget=h.c.setupBudgetMs;
    h.onObservationStart=()=>h.complete(10+budget+10);
    await completeSignedBootstrap(h.c,h.context,'admission',7);
    const miss=h.context.admissionSloMisses[0];assert.equal(miss.phase,'postAckCompleteBootstrap');
    assert.equal(miss.originalBudgetMs,budget);assert.equal(miss.originalDeadlineAtMs,h.base+10+budget);
    assert.ok(h.waits.filter(w=>w.label.startsWith('late original')).every(w=>w.deadline===h.base+10+budget+60000));
    assert.equal(h.c.bootstrapProof.strictSetupDeadlinePassed,false);assert.equal(h.c.bootstrapProof.diagnosticAcceptedLate,true);
    assert.equal(h.c.startGameTiming.pollObservedAtMs,h.base+10);
    assert.equal(h.context.firstAttemptAdmissionOk,false);assert.equal(h.context.allSloPassed,false);assert.equal(h.commands.length,1);
  });
});

test('on-time cached bootstrap receipts remain strict passes when final polling resumes after both original deadlines',async t=>{
  const h=bootstrapFixture(t,{observe:true,ackDelay:10,readyDelay:20,pollDelay:100000});
  await completeSignedBootstrap(h.c,h.context,'admission',7);
  assert.equal(h.c.bootstrapProof.strictSetupDeadlinePassed,true);assert.equal(h.c.bootstrapProof.diagnosticAcceptedLate,false);
  assert.equal(h.c.bootstrapProof.startSequence,6);assert.ok(h.c.bootstrapProof.userSequence>h.c.bootstrapCursor);
  assert.equal(h.c.startGameTiming.ackElapsedMs,10);assert.ok(h.c.startGameTiming.pollObservedAtMs>h.c.startGameTiming.deadlineAtMs);
  assert.equal(h.context.admissionSloMisses,undefined);assert.equal(h.context.allSloPassed,true);assert.equal(h.commands.length,1);
});

test('a late ACK cannot restart the absolute60s grace for a later full bootstrap, even when that full proof is valid',async t=>{
  const h=bootstrapFixture(t,{observe:true,ackDelay:20010,readyDelay:85000});
  await assert.rejects(completeSignedBootstrap(h.c,h.context,'admission',7),/timed out waiting for StartGame/);
  assert.equal(h.context.admissionSloMisses.length,1);assert.equal(h.context.admissionSloMisses[0].observationDeadlineAtMs,h.base+80000);
  assert.equal(h.c.bootstrapProof,undefined);assert.equal(h.commands.length,1);
  assert.equal(h.rows.filter(r=>r.kind==='bootstrapDiagnosticObservationStarted').length,1);
  assert.equal(h.rows.filter(r=>r.kind==='bootstrapDiagnosticObservationAborted').length,1);
  assert.ok(h.waits.filter(w=>w.label.startsWith('late original')).every(w=>w.deadline===h.base+80000));
});

test('diagnostic grace cannot accept an old snapshot or an incomplete fresh bootstrap when its absolute deadline expires',async t=>{
  const h=bootstrapFixture(t,{observe:true,readyDelay:null});
  await assert.rejects(completeSignedBootstrap(h.c,h.context,'admission',7),/complete authoritative owner snapshot/);
  assert.equal(h.c.snapshot,null);assert.equal(h.c.bootstrapProof,undefined);assert.equal(h.commands.length,1);
  assert.equal(h.context.admissionSloMisses.length,1);assert.equal(h.context.allSloPassed,false);
});

test('non-deadline rejection, generic errors, transport failure, closed socket and stopped guard never enter observation',async t=>{
  for(const fault of ['rejection','generic','transport','closed','guard'])await t.test(fault,async t=>{
    const h=bootstrapFixture(t,{observe:true,ackDelay:null,readyDelay:null});
    h.onSend=()=>{
      if(fault==='rejection')h.c.observe({type:'error',message:'ordinary rejection'});
      if(fault==='transport')h.c.failure=new Error('actual socket failed');
      if(fault==='closed'){h.c.closed=true;h.c.ws.readyState=3;}
      if(fault==='guard')h.guardFailed=true;
    };
    if(fault==='generic')h.c.wait=async()=>{throw new Error(h.c.label+' timed out waiting for StartGame');};
    await assert.rejects(completeSignedBootstrap(h.c,h.context,'admission',7));
    assert.equal(h.context.admissionSloMisses,undefined);assert.equal(h.context.diagnosticObserveUsed,undefined);
    assert.equal(h.c.bootstrapProof,undefined);assert.equal(h.commands.length,1);
  });
});

test('wrong/malformed owner, late rejection, replaced identity/socket/cursor and failures during grace abort the original miss',async t=>{
  for(const fault of ['owner','malformed','rejectedStart','identity','socket','cursor','transport','closed','guard'])await t.test(fault,async t=>{
    const h=bootstrapFixture(t,{observe:true,ackDelay:null,readyDelay:null});
    h.onObservationStart=()=>{
      h.start(20010,fault==='rejectedStart'?1:4);h.complete(20020,fault==='owner'?'WrongSynthetic':h.c.actor.name);
      if(fault==='malformed')h.c.events.find(e=>e.packet==='UserInformation'&&e.sequence>4).payload.objectId=-1;
      if(fault==='identity')h.c.actor.walletIdentity.accountId='changed-offline-account';
      if(fault==='socket')h.c.ws={readyState:1};
      if(fault==='cursor')h.c.bootstrapCursor++;
      if(fault==='transport')h.c.failure=new Error('actual grace transport failed');
      if(fault==='closed'){h.c.closed=true;h.c.ws.readyState=3;}
      if(fault==='guard')h.guardFailed=true;
    };
    await assert.rejects(completeSignedBootstrap(h.c,h.context,'admission',7),/timed out waiting for StartGame/);
    assert.equal(h.c.bootstrapProof,undefined);assert.equal(h.commands.length,1);assert.equal(h.context.admissionSloMisses.length,1);
    assert.equal(h.context.allSloPassed,false);assert.equal(h.rows.filter(r=>r.kind==='bootstrapDiagnosticLateCompletion').length,0);
  });
});
