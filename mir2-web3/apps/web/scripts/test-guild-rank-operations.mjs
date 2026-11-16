import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {fileURLToPath} from 'node:url';
import test from 'node:test';
import ts from 'typescript';
const cache=new Map();
function load(name){
  if(cache.has(name))return cache.get(name);
  const url=new URL('../lib/'+name+'.ts',import.meta.url);
  const compiled=ts.transpileModule(readFileSync(url,'utf8'),{fileName:fileURLToPath(url),compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.CommonJS,strict:true},reportDiagnostics:true});
  assert.deepEqual((compiled.diagnostics??[]).filter(d=>d.category===ts.DiagnosticCategory.Error),[]);
  const m={exports:{}};cache.set(name,m.exports);
  new Function('module','exports','require',compiled.outputText)(m,m.exports,id=>{
    assert.equal(id,'./social-parity-actions');return load('social-parity-actions');
  });return m.exports;
}
const {GuildRankOperations}=load('guild-rank-operations');
const source=()=>({owner:{socket:{},connectionGeneration:1,sessionGeneration:2,sceneRevision:3,playerObjectId:1000,mapFileName:'0'},
  scope:{},guildName:'Guild',actor:'Leader',ownRankIndex:0,permissions:['CanChangeRank'],
  ranks:[{index:0,name:'Leader',permissions:['CanChangeRank']},{index:2,name:'Members',permissions:['CanRecruit']}]});
const ack=(actor='Leader',index=2,name='Scouts')=>({status:7,name:actor,rankIndex:0,
  ranks:[{index,name,options:2,members:[]}]});
function renamed(ops,s,text='Scouts',now=100){const p=ops.reserveRename(2,text,s,now);assert.ok(p);assert(ops.claim(p,s,p.command,now));return p;}

test('adapter source type-checks with its real dependencies',()=>{
  const f=fileURLToPath(new URL('../lib/guild-rank-operations.ts',import.meta.url));
  const p=ts.createProgram([f],{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.ESNext,moduleResolution:ts.ModuleResolutionKind.Bundler,strict:true,noEmit:true,skipLibCheck:true,types:[]});
  assert.deepEqual(ts.getPreEmitDiagnostics(p).map(d=>ts.flattenDiagnosticMessageText(d.messageText,'\n')),[]);
});
test('rename sends exactly one ordinary type3 with untrimmed text',()=>{
  const ops=new GuildRankOperations(),s=source(),p=ops.reserveRename(2,'  abc  ',s,0);assert.ok(p);
  assert.deepEqual(p.command,{type:'editGuildMember',changeType:3,rankIndex:2,name:'',rankName:'  abc  '});
  assert.deepEqual(ops.view(0).renameRankIndices,[2]);assert.equal(ops.outcomeUnknown(p),false);
});
test('permission requests one explicit bit, not a name change or eight-item batch',()=>{
  const ops=new GuildRankOperations(),s=source(),p=ops.reservePermission(2,'CanKick',true,s,0);assert.ok(p);
  assert.deepEqual(p.command,{type:'editGuildMember',changeType:5,rankIndex:2,name:'true',rankName:'2'});
  assert.equal(ops.reservePermission(2,'CanRecruit',true,s,0),null);
});
test('Native own-rank hierarchy, rights and real target are required',()=>{
  for(const change of [{ownRankIndex:3},{permissions:[]},{ranks:[]},{ownRankIndex:-1},{ownRankIndex:256}]){
    const ops=new GuildRankOperations(),s={...source(),...change};assert.equal(ops.reserveRename(2,'Scouts',s,0),null);
    assert.equal(ops.reservePermission(2,'CanKick',true,s,0),null);
  }
});
test('only actual final claim consumes this proof and definitely-unsent retains retry',()=>{
  const ops=new GuildRankOperations(),s=source(),p=ops.reserveRename(2,'Scouts',s,0);assert.ok(p);
  assert.equal(ops.cancelDefinitelyUnsent(p),true);assert.equal(ops.claim(p,s,p.command,0),false);
  const q=renamed(ops,s);assert.equal(ops.cancelDefinitelyUnsent(q),false);assert.equal(ops.claim(q,s,q.command,100),false);
  assert.equal(ops.outcomeUnknown(q),true);
});
test('source/guild/scope/socket/scene/rights changes cannot enter stale commands',()=>{
  const ops=new GuildRankOperations(),s=source(),p=ops.reserveRename(2,'Scouts',s,0);assert.ok(p);
  for(const next of [{...s,scope:{}},{...s,guildName:'Other'},{...s,permissions:[]},
    {...s,owner:{...s.owner,socket:{}}},{...s,owner:{...s.owner,sceneRevision:4}},
    {...s,ranks:s.ranks.map(r=>({...r,name:'Changed'}))}])assert.equal(ops.claim(p,next,p.command,0),false);
  assert.equal(ops.claim(p,s,{...p.command,rankName:'Another'},0),false);assert.equal(ops.claim(p,s,p.command,0),true);
});
test('exact real status7 actor and nested rank identity confirm rename, not top-level rankIndex',()=>{
  const ops=new GuildRankOperations(),s=source();renamed(ops,s);
  for(const value of [ack('Peer'),ack('Leader',0),ack('Leader',2,'Other'),{...ack(),status:255},
    {status:7,name:'Leader',ranks:[{index:2,name:'Scouts'}]}])assert.equal(ops.receipt(value,s.owner,s.scope),false);
  assert.equal(ops.receipt(ack(),{...s.owner,socket:{}},s.scope),false);
  assert.equal(ops.receipt(ack(),s.owner,{}),false);
  assert.equal(ops.receipt(ack(),s.owner,s.scope),true);assert.deepEqual(ops.view(100).renameRankIndices,[]);
});
test('map change preserves entered work and same-physical-owner ACK can finish it',()=>{
  const ops=new GuildRankOperations(),s=source();renamed(ops,s);
  assert.equal(ops.receipt(ack(),{...s.owner,sceneRevision:4,mapFileName:'D001'},s.scope),true);
});
test('rename retry opens strictly after five seconds and never declares success',()=>{
  const ops=new GuildRankOperations(),s=source();renamed(ops,s,'Scouts',0);
  assert.equal(ops.reserveRename(2,'Scouts',s,5000),null);assert.deepEqual(ops.view(5000).renameRankIndices,[2]);
  assert.ok(ops.reserveRename(2,'Scouts',s,5001));
});
test('permission cooldown is global300ms and does not optimistically mutate server ranks',()=>{
  const ops=new GuildRankOperations(),s=source(),before=structuredClone(s.ranks),p=ops.reservePermission(2,'CanKick',true,s,0);
  assert.ok(p);assert.equal(ops.claim(p,s,p.command,0),true);ops.finishPermissionAttempt(p);
  assert.equal(ops.reservePermission(0,'CanKick',true,s,299),null);
  assert.ok(ops.reservePermission(0,'CanKick',true,s,300));assert.deepEqual(s.ranks,before);
});
test('permission300ms is input cooldown, not a new deadline for delayed valid send',()=>{
  const ops=new GuildRankOperations(),s=source(),p=ops.reservePermission(2,'CanKick',true,s,0);assert.ok(p);
  assert.equal(ops.claim(p,s,p.command,1000),true);assert.equal(ops.view(1299).permission,true);assert.equal(ops.view(1300).permission,false);
});
test('retirement releases local controls without fabricating server state',()=>{
  const ops=new GuildRankOperations(),s=source(),p=renamed(ops,s);ops.retire();
  assert.equal(ops.outcomeUnknown(p),false);assert.deepEqual(ops.view(101),{renameRankIndices:[],permission:false});
  assert.equal(ops.receipt(ack(),s.owner,s.scope),false);
});
