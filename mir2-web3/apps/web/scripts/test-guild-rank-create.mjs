import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import ts from "typescript";
import { budgetedSendStatements } from "./lib/social-budget-test-runtime.mjs";

const read = name => readFileSync(new URL(name, import.meta.url), "utf8");
function compile(source, fileName, dependencies = {}) {
  const result = ts.transpileModule(source, { fileName, reportDiagnostics: true,
    compilerOptions: {target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.CommonJS,jsx:ts.JsxEmit.ReactJSX,strict:true} });
  assert.deepEqual((result.diagnostics ?? []).filter(row => row.category === ts.DiagnosticCategory.Error), []);
  const module = {exports:{}};
  new Function("require","module","exports",result.outputText)(id => {
    assert(Object.hasOwn(dependencies,id), "Unexpected dependency " + id); return dependencies[id];
  },module,module.exports);
  return module.exports;
}
const actions = compile(read("../lib/social-parity-actions.ts"),"social-parity-actions.ts");
const {GuildRankOperations} = compile(read("../lib/guild-rank-operations.ts"),"guild-rank-operations.ts",{"./social-parity-actions":actions});
const {applyGuildMemberChange} = compile(read("../lib/guild-rank-projection.ts"),"guild-rank-projection.ts",{"./social-parity-actions":actions});
const keys = actions.GUILD_RANK_PERMISSION_KEYS;
const member = (name,id) => ({name,id,online:true});
const rank = (index,name,options,members=[]) => ({index,name,options,members});
const raw = () => [rank(0,"Leader",255,[member("Leader",1)]),rank(1,"Officer",65,[member("Officer",2)]),
  rank(2,"Members",2,[member("Alice",3),member("Bob",4)])];
function source() {
  const value = actions.projectGuildRanks(raw());
  return {owner:{socket:{},connectionGeneration:1,sessionGeneration:2,sceneRevision:3,playerObjectId:1000,mapFileName:"0"},
    scope:{},guildName:"Guild",actor:"Leader",ownRankIndex:0,permissions:[...keys],ranks:value.ranks,
    roster:{revision:10,complete:true,actorIndex:1,members:value.members}};
}
const six = (s,changes={}) => ({name:s.actor,rankIndex:0,status:6,
  ranks:[rank(s.ranks.length > 1 ? s.ranks.length-1 : 1,"Rank 2",0)],...changes});
function resulting(s,newName="Rank 2") {
  const count=s.ranks.length, insertion=count > 1 ? count-1 : 1;
  const ranks=s.ranks.map(row=>rank(row.index>=insertion?row.index+1:row.index,row.name,
    keys.reduce((bits,key,bit)=>row.permissions.includes(key)?bits|(1<<bit):bits,0),
    s.roster.members.filter(m=>m.rankIndex===row.index).map(m=>member(m.name,m.id))));
  ranks.splice(insertion,0,rank(insertion,newName,0));
  return ranks;
}
const readback = (s,changes={}) => ({revision:s.roster.revision+1,complete:true,value:actions.projectGuildRanks(resulting(s)),...changes});
const full = ranks => ({name:"",rankIndex:0,status:255,ranks});
function entered(ops,s,now=0) {
  const proof=ops.reserveCreate(s,now);assert(proof);assert(ops.claim(proof,s,proof.command,now));return proof;
}
const busy = (ops,now=0) => assert.equal(ops.view(now).create,true);
function successor(s,revision=11) {
  const value=actions.projectGuildRanks(resulting(s));
  return {...s,ranks:value.ranks,roster:{...s.roster,revision,members:value.members}};
}

test("create emits exactly Native's ordinary type4 command within 1..254 existing ranks",()=>{
  for(const count of [1,2,3,254]) assert.deepEqual(actions.guildRankCreateCommand(count),
    {type:"editGuildMember",changeType:4,rankIndex:0,name:"",rankName:`Rank-${count-1}`});
  for(const value of [0,255,256,-1,1.5,NaN,Infinity,"3",null,undefined,{}])
    assert.equal(actions.guildRankCreateCommand(value),null);
});

test("create requires complete contiguous real roster, own identity, rights and capacity",()=>{
  const base=source();
  const changes=[{roster:undefined},{roster:{...base.roster,complete:false}},
    {roster:{...base.roster,revision:0}},{roster:{...base.roster,actorIndex:99}},
    {permissions:[]},{actor:"Other"},{ownRankIndex:2},
    {ranks:base.ranks.filter(r=>r.index!==1)},
    {ranks:[...base.ranks,base.ranks[2]]},
    {roster:{...base.roster,members:[...base.roster.members,{...base.roster.members[0],id:99}]}},
    {ranks:Array.from({length:255},(_,index)=>({index,name:"R"+index,permissions:index===0?[...keys]:[]}))}];
  for(const change of changes) assert.equal(new GuildRankOperations().reserveCreate({...base,...change},0),null);
  const s=source(),ops=new GuildRankOperations(),before=structuredClone({ranks:s.ranks,roster:s.roster});
  const proof=ops.reserveCreate(s,0);assert(proof);assert.equal(proof.kind,"create");
  assert(Object.isFrozen(proof));assert(Object.isFrozen(proof.command));
  assert.deepEqual({ranks:s.ranks,roster:s.roster},before);
});

test("only one create is reserved; definitely-unsent cancellation permits a fresh ordinary attempt",()=>{
  const s=source(),ops=new GuildRankOperations(),proof=ops.reserveCreate(s,0);assert(proof);busy(ops);
  assert.equal(ops.reserveCreate(s,0),null);assert.equal(ops.outcomeUnknown(proof),false);
  assert(ops.cancelDefinitelyUnsent(proof));assert.equal(ops.claim(proof,s,proof.command,0),false);
  assert(ops.reserveCreate(s,0));
});

test("final create claim rechecks role/scope/roster version, data and exact physical scene",()=>{
  const s=source(),ops=new GuildRankOperations(),proof=ops.reserveCreate(s,0);assert(proof);
  for(const next of [{...s,scope:{}},{...s,guildName:"Other"},{...s,actor:"Other"},
    {...s,permissions:[]},{...s,owner:{...s.owner,socket:{}}},
    {...s,owner:{...s.owner,sceneRevision:4}}, {...s,owner:{...s.owner,mapFileName:"D001"}},
    {...s,roster:{...s.roster,revision:11}}, {...s,roster:{...s.roster,actorIndex:2}},
    {...s,ranks:s.ranks.map(r=>({...r,name:r.name+" changed"}))}])
    assert.equal(ops.claim(proof,next,proof.command,0),false);
  assert.equal(ops.claim(proof,s,{...proof.command,rankName:"Forged"},0),false);
  assert.equal(ops.claim(proof,s,{...proof.command,extra:true},0),false);
  let reads=0;const accessor={...proof.command};Object.defineProperty(accessor,"rankName",{get(){reads++;return proof.command.rankName;},enumerable:true});
  assert.equal(ops.claim(proof,s,accessor,0),false);assert.equal(reads,0);
  assert.equal(ops.claim(Object.freeze({...proof}),s,proof.command,0),false);
  assert(ops.claim(proof,s,proof.command,0));assert.equal(ops.claim(proof,s,proof.command,0),false);
});

test("synchronous DTO reentry cannot claim a retired creation",()=>{
  const s=source(),ops=new GuildRankOperations(),proof=ops.reserveCreate(s,0);assert(proof);
  const dto=new Proxy({...proof.command},{ownKeys(target){ops.retire();return Reflect.ownKeys(target);}});
  assert.equal(ops.claim(proof,s,dto,0),false);assert.equal(ops.outcomeUnknown(proof),false);
});

test("entered unknown create never times out or retries at the five-second input threshold",()=>{
  const s=source(),ops=new GuildRankOperations(),proof=entered(ops,s);
  assert.equal(ops.nextWake(0),5000);assert.equal(ops.cancelDefinitelyUnsent(proof),false);
  ops.expire(5001);busy(ops,5001);assert(ops.outcomeUnknown(proof));assert.equal(ops.reserveCreate(s,5001),null);
  assert.equal(ops.nextWake(5001),null,"no endless polling timer or automatic mutation retry");
});

test("matching own fresh status6 plus exact complete result releases create while the cooldown remains",()=>{
  const s=source(),ops=new GuildRankOperations(),proof=entered(ops,s);
  const packet=six(s);assert.equal(packet.rankIndex,0);assert.equal(packet.ranks[0].index,2);assert(!Object.hasOwn(packet,"changeType"));
  assert(ops.receipt(packet,s.owner,s.scope,readback(s)));assert.equal(ops.outcomeUnknown(proof),false);
  busy(ops,4999);assert.equal(ops.reserveCreate(successor(s),4999),null);
  assert.equal(ops.view(5000).create,undefined);assert(ops.reserveCreate(successor(s),5000));
});

test("one-rank and last available slot follow the real insertion rule without dropping old member identities",()=>{
  for(const count of [1,2,254]) {
    const s=source(),wire=Array.from({length:count},(_,index)=>rank(index,"R"+index,index===0?255:0,
      index===0?[member("Leader",1)]:index===count-1?[member("Last",2)]:[]));
    const value=actions.projectGuildRanks(wire);s.ranks=value.ranks;s.roster.members=value.members;
    const ops=new GuildRankOperations();entered(ops,s);
    const insertion=count>1?count-1:1,packet={...six(s),ranks:[rank(insertion,"Server Name",0)]};
    assert(ops.receipt(packet,s.owner,s.scope,{...readback(s),value:actions.projectGuildRanks(resulting(s,"Server Name"))}));
    assert.equal(ops.view(5000).create,undefined);
  }
});

test("a real non-leader CanChangeRank actor may create; the dummy DTO rankIndex0 is not a hierarchy target",()=>{
  const s=source();s.actor="Officer";s.ownRankIndex=1;s.permissions=[...s.ranks[1].permissions];s.roster.actorIndex=2;
  assert(s.permissions.includes("CanChangeRank"));
  const ops=new GuildRankOperations(),proof=entered(ops,s);
  assert.equal(proof.command.rankIndex,0);assert(ops.receipt(six(s),s.owner,s.scope,readback(s)));
});

test("wrong actor/index/options/member payload or stale version cannot acknowledge creation",()=>{
  for(const packet of [six(source(),{name:"Officer"}),six(source(),{rankIndex:2}),
    six(source(),{ranks:[rank(0,"Rank 2",0)]}),six(source(),{ranks:[rank(2,"Rank 2",1)]}),
    six(source(),{ranks:[rank(2,"Rank 2",0,[member("New",10)])]}),six(source(),{status:7}),
    six(source(),{ranks:[]})]) {
    const s=source(),ops=new GuildRankOperations();entered(ops,s);
    assert.equal(ops.receipt(packet,s.owner,s.scope,readback(s)),false);busy(ops);
    assert.equal(ops.takeCreateReadbackRequest(s.owner,s.scope),false);
  }
  const s=source(),ops=new GuildRankOperations();entered(ops,s);
  assert.equal(ops.receipt(six(s),s.owner,s.scope,readback(s,{revision:10})),false);
  assert.equal(ops.takeCreateReadbackRequest(s.owner,s.scope),false);
});

test("255 alone cannot resolve create even when its complete shape looks exactly right",()=>{
  const s=source(),ops=new GuildRankOperations();entered(ops,s);
  assert.equal(ops.receipt(full(resulting(s)),s.owner,s.scope,readback(s)),false);busy(ops);
  assert.equal(ops.takeCreateReadbackRequest(s.owner,s.scope),false);
});

test("a real own6 candidate may request only one read, then needs a later matching full wire readback",()=>{
  const s=source(),ops=new GuildRankOperations(),proof=entered(ops,s);
  assert.equal(ops.receipt(six(s),s.owner,s.scope,readback(s,{complete:false})),false);busy(ops);
  assert.equal(ops.takeCreateReadbackRequest({...s.owner,socket:{}},s.scope),false);
  assert(ops.takeCreateReadbackRequest(s.owner,s.scope));assert.equal(ops.takeCreateReadbackRequest(s.owner,s.scope),false);
  assert.equal(ops.receipt(full(resulting(s)),s.owner,s.scope,readback(s)),false,"same version is not the later read");
  assert.equal(ops.receipt(full([]),s.owner,s.scope,readback(s,{revision:12})),false,"raw full packet must agree with projected readback");
  assert(ops.receipt(full(resulting(s)),s.owner,s.scope,readback(s,{revision:12})));
  assert.equal(ops.outcomeUnknown(proof),false);
});

test("ambiguous concurrent rename, changed permissions or member movement remains unknown",()=>{
  for(const mutate of [v=>{v.ranks[0].name="Other";},v=>{v.ranks[1].permissions=[];},
    v=>{v.members.find(m=>m.name==="Bob").rankIndex=1;},v=>{v.members.pop();}]) {
    const s=source(),ops=new GuildRankOperations();entered(ops,s);const next=readback(s);mutate(next.value);
    assert.equal(ops.receipt(six(s),s.owner,s.scope,next),false);busy(ops);
  }
});

test("candidate cannot borrow somebody else's6; same-owner map-change ACK is still valid",()=>{
  const s=source(),ops=new GuildRankOperations();entered(ops,s);
  assert.equal(ops.receipt(six(s),{...s.owner,socket:{}},s.scope,readback(s)),false);
  assert.equal(ops.receipt(six(s),s.owner,{},readback(s)),false);
  assert.equal(ops.receipt(six(s),s.owner,s.scope,readback(s,{complete:false})),false);
  assert.equal(ops.receipt(six(s,{name:"Officer"}),s.owner,s.scope,readback(s,{revision:12})),false);
  assert(ops.receipt(full(resulting(s)),{...s.owner,mapFileName:"D001",sceneRevision:4},s.scope,readback(s,{revision:13})));
});

test("create structural proof is independent of online cosmetics and never mutates caller data",()=>{
  const s=source(),ops=new GuildRankOperations(),before=structuredClone({ranks:s.ranks,roster:s.roster});entered(ops,s);
  const next=readback(s);next.value.members.forEach(m=>{m.online=false;});
  assert(ops.receipt(six(s),s.owner,s.scope,next));assert.deepEqual({ranks:s.ranks,roster:s.roster},before);
});

test("create serializes structural edits until resolved; retire clears local state without accepting old scope",()=>{
  const s=source(),ops=new GuildRankOperations();entered(ops,s);
  assert.equal(ops.reserveRename(2,"Veterans",s,100),null);
  assert.equal(ops.reservePermission(2,"CanKick",true,s,100),null);
  ops.retire();const next={...s,scope:{}};const proof=entered(ops,next,100);
  assert.equal(ops.receipt(six(s),s.owner,s.scope,readback(s)),false);assert(ops.outcomeUnknown(proof));
});

// Execute production Page callbacks and its exact final rank guard + socket try.
const pageText=read("../app/page.tsx"),pageAst=ts.createSourceFile("page.tsx",pageText,ts.ScriptTarget.Latest,true,ts.ScriptKind.TSX);
const functions=new Map();let memberCase;
const visit=node=>{
  if(ts.isFunctionDeclaration(node)&&node.name)functions.set(node.name.text,node);
  if(ts.isCaseClause(node)&&ts.isStringLiteral(node.expression)&&node.expression.text==="GuildMemberChange")memberCase=node;
  ts.forEachChild(node,visit);
};visit(pageAst);
const sendBody=budgetedSendStatements(ts,functions.get("sendRaw"),pageAst);
const early=sendBody.find(node=>ts.isIfStatement(node)&&node.expression.getText(pageAst).includes('command.type === "editGuildMember"')
  &&node.expression.getText(pageAst).includes('options?.socialRosterProof?.rankProof'));
const final=sendBody.find(node=>ts.isIfStatement(node)&&node.expression.getText(pageAst).includes('options?.socialRosterProof?.rankProof')
  &&node.expression.getText(pageAst).includes('guildRankOperationsRef.current.claim'));
const readEarly=sendBody.find(node=>ts.isIfStatement(node)&&node.expression.getText(pageAst).includes('command.type === "requestGuildInfo"')
  &&node.expression.getText(pageAst).includes('options?.socialRosterProof?.rankReadProof'));
const readFinal=sendBody.find(node=>ts.isIfStatement(node)&&node.expression.getText(pageAst)==='wireCommand.type === "requestGuildInfo"');
const socketTry=sendBody.find(node=>ts.isTryStatement(node)&&node.tryBlock.statements.some(row=>row.getText(pageAst).includes('socket.send(serialized)')));
assert(early&&final&&readEarly&&readFinal&&socketTry&&memberCase,"actual Page rank/read guards, socket and receive case");
const needed=["readGuildRankEditSource","submitGuildRankEdit","createGuildRank","requestGuildRankRoster",
  "readSocialRosterSource","socialRosterProofCurrent","sendSocialRosterCommand"];
const pageCode=needed.map(name=>{assert(functions.has(name));return functions.get(name).getText(pageAst);}).join("\n")+`
function boundary(command,options){const wireCommand=command,socket=socketRef.current,serialized=JSON.stringify(command);
${early.getText(pageAst)};${readEarly.getText(pageAst)};beforeBoundary();
${final.getText(pageAst)};${readFinal.getText(pageAst)};${socketTry.getText(pageAst)};return true;}
function receive(payload){const event={packet:"GuildMemberChange"};switch(event.packet){${memberCase.getText(pageAst)}}}
return {${needed.join(",")},boundary,receive};`;
const pageCompiled=ts.transpileModule(`function factory(){${pageCode}}`,{compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.None}}).outputText;
function pageFixture({throws=false}={}) {
  const s=source(),messages=[],ops=new GuildRankOperations();let now=0,callback=()=>{};
  const socket={readyState:1,send(body){messages.push(JSON.parse(body));if(throws)throw new Error("controlled unknown send");}};
  s.owner.socket=socket;
  const world={playerObjectId:1000,entities:[{objectId:1000,name:"Leader"}],stage5Systems:{guild:{name:"Guild",members:s.roster.members.map(m=>m.name)}}};
  const scope={guildRankOperationsRef:{current:ops},guildRankScopeRef:{current:{owner:s.owner,guildName:s.guildName,key:s.scope}},
    guildPermissionsRef:{current:{owner:s.owner,guildName:s.guildName,permissions:s.permissions,rankIndex:0}},
    guildRanksRef:{current:{owner:s.owner,guildName:s.guildName,value:actions.projectGuildRanks(raw()),complete:true,revision:10}},
    worldRef:{current:world},socialCharacterIndexRef:{current:{current:1}},socketRef:{current:socket},
    socialItemWindowsRef:{current:{guild:true}},socialRosterWindowsRef:{current:{friends:false}},
    guildRankReadRef:{current:{scope:s.scope,needed:false,proof:null,entered:false}},guildStorageRawRef:{current:null},
    guildBuffAuthorityRef:{current:{read(){return null;},invalidatePermissions(){}}},applyGuildMemberChange,
    currentSocialReplyOwner:()=>s.owner,currentSocialReceiveOwner:()=>s.owner,
    sameSocialPhysicalOwner:(a,b)=>!!b&&a.socket===b.socket&&a.connectionGeneration===b.connectionGeneration
      &&a.sessionGeneration===b.sessionGeneration&&a.playerObjectId===b.playerObjectId,
    WebSocket:{OPEN:1},performance:{now:()=>now},console:{error(){}},scheduleGuildRankControls(){},
    retireGuildRankControls(){ops.retire();},beforeBoundary:()=>callback(),
    updateWorld(fn){scope.worldRef.current=fn(scope.worldRef.current);}};
  const api=new Function("scope",`with(scope){${pageCompiled};return factory();}`)(scope);
  const render=()=>{scope.socialRosterRenderSources={guild:api.readSocialRosterSource("guild")};};render();
  scope.send=(command,options)=>api.boundary(command,options);
  return {api,scope,s,ops,messages,render,setNow:value=>{now=value;},before:fn=>{callback=fn;}};
}

test("actual Page callback and final claim send one type4; actual6 receiver publishes the new full structure",()=>{
  const f=pageFixture();assert(f.api.createGuildRank());
  assert.deepEqual(f.messages,[{type:"editGuildMember",changeType:4,rankIndex:0,name:"",rankName:"Rank-2"}]);
  assert.equal(f.api.createGuildRank(),false);f.api.receive(six(f.s));
  assert.equal(f.scope.guildRanksRef.current.value.ranks.length,4);
  assert.equal(f.scope.guildRanksRef.current.revision,11);
  assert.equal(f.scope.guildRanksRef.current.value.members.find(m=>m.name==="Bob").rankIndex,3);
  f.setNow(5000);assert.equal(f.ops.view(5000).create,undefined);
});

test("actual Page permits delegated CanChangeRank without treating type4 rankIndex0 as a leader edit",()=>{
  const f=pageFixture();f.scope.worldRef.current.entities[0].name="Officer";
  f.scope.guildPermissionsRef.current.rankIndex=1;
  f.scope.guildPermissionsRef.current.permissions=[...f.scope.guildRanksRef.current.value.ranks[1].permissions];
  f.scope.socialCharacterIndexRef.current.current=2;f.render();
  assert(f.api.createGuildRank());assert.equal(f.messages[0].rankIndex,0);
  f.api.receive(six({...f.s,actor:"Officer"}));f.setNow(5000);
  assert.equal(f.ops.view(5000).create,undefined);
  assert.equal(f.scope.guildRanksRef.current.value.members.find(m=>m.id===2).rankIndex,1);
});

test("actual Page source and final boundary refuse missing stable role, source changes and naked type4",()=>{
  const f=pageFixture();f.scope.socialCharacterIndexRef.current.current=null;
  assert.equal(f.api.createGuildRank(),false);assert.deepEqual(f.messages,[]);
  f.scope.socialCharacterIndexRef.current.current=1;
  f.before(()=>{f.scope.guildRanksRef.current.revision++;});
  assert.equal(f.api.createGuildRank(),false);assert.deepEqual(f.messages,[]);assert.equal(f.ops.view(0).create,undefined);
  f.before(()=>{});assert(f.api.createGuildRank());assert.equal(f.messages.length,1);
  assert.equal(f.api.boundary({type:"editGuildMember",changeType:4,rankIndex:0,name:"",rankName:"Rank-2"},{}),false);
  assert.equal(f.messages.length,1);
});

test("actual Page create respects closed guild UI, incomplete roster, actor rights and guild scope",()=>{
  for(const change of [f=>{f.scope.socialItemWindowsRef.current.guild=false;},
    f=>{f.scope.guildRanksRef.current.complete=false;},
    f=>{f.scope.guildPermissionsRef.current.permissions=[];},
    f=>{f.scope.worldRef.current.stage5Systems.guild.name="Other";},
    f=>{f.scope.currentSocialReplyOwner=()=>null;}]) {
    const f=pageFixture();change(f);
    assert.equal(f.api.createGuildRank(),false);assert.deepEqual(f.messages,[]);
    assert.equal(f.ops.view(0).create,undefined,"known-unsent admission cannot consume create cooldown");
  }
});

test("actual Page socket-entry exception remains unknown and cannot create a duplicate after five seconds",()=>{
  const f=pageFixture({throws:true});assert(f.api.createGuildRank());assert.equal(f.messages.length,1);
  f.setNow(5001);assert.equal(f.api.createGuildRank(),false);assert.equal(f.messages.length,1);busy(f.ops,5001);
});

test("actual Page readback path preserves candidate, requests one ordinary refresh and resolves only the real full",()=>{
  const f=pageFixture();assert(f.api.createGuildRank());
  f.scope.guildRanksRef.current.complete=false;
  f.api.receive(six(f.s));busy(f.ops);assert.equal(f.scope.guildRankReadRef.current.needed,true);
  f.render();assert(f.api.requestGuildRankRoster());
  assert.deepEqual(f.messages,[{type:"editGuildMember",changeType:4,rankIndex:0,name:"",rankName:"Rank-2"},
    {type:"requestGuildInfo",infoType:1}]);
  assert.equal(f.api.requestGuildRankRoster(),false,"one read may enter; no automatic repeat or mutation replay");
  f.api.receive(full(resulting(f.s)));f.setNow(5000);
  assert.equal(f.ops.view(5000).create,undefined);
  assert.equal(f.scope.guildRankReadRef.current.needed,false);
  assert.equal(f.messages.length,2,"readback processing never sends another mutation");
  assert.equal(f.api.requestGuildRankRoster(),false,"a completed read is not polled");
});
