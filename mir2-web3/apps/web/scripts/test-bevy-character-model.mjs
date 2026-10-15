import assert from "node:assert/strict";import test from "node:test";import {readFileSync} from "node:fs";import ts from "typescript";
function load(url){const m={exports:{}};const code=ts.transpileModule(readFileSync(url,"utf8"),{compilerOptions:{module:ts.ModuleKind.CommonJS,target:ts.ScriptTarget.ES2022}}).outputText;
 new Function("exports","module","require",code)(m.exports,m,p=>load(new URL(`${p}.ts`,url)));return m.exports;}
const {parseStateItemMetadata:parse,projectCharacterModel:project}=load(new URL("../lib/bevy-character-model.ts",import.meta.url));
const item=(id,stateImage,slot=0)=>({uniqueId:id,key:"x",name:"x",quantity:1,slot,container:2,icon:1,description:"",stateImage});
const model=items=>({capacity:46,gold:0,items});
test("partial StateItem version3 uses actual frame30; legacy Bag object stays unchanged",()=>{
 const raw={version:3,count:5192,frames:[{index:30,x:75,y:186,width:28,height:57}]};const meta=parse(raw);
 const bag=model([item(0,30)]);const before=JSON.stringify(bag);const out=project(bag,meta);
 assert.equal(JSON.stringify(bag),before);assert.equal(out.items[0].uniqueId,0);
 assert.deepEqual([out.items[0].stateImageX,out.items[0].stateImageY,out.items[0].stateImageWidth,out.items[0].stateImageHeight],[75,186,28,57]);
});
test("replacement frame and missing frame never inherit old cell geometry",()=>{
 const meta=parse({version:4,frames:[{index:30,x:-20,y:0,width:3,height:4}]});
 assert.equal(project(model([item(1,30)]),meta).items[0].stateImageX,-20);
 assert.equal(project(model([item(2,31)]),meta).items[0].stateImageWidth,0);
 assert.equal(project(model([item(1,30)]),null).items[0].stateImageWidth,0);
});
test("bounded signed geometry and unique slots are required",()=>{
 for(const bad of [{index:30,x:2147483648,y:0,width:3,height:4},{index:30,x:0,y:0,width:65536,height:4}])assert.equal(parse({version:3,frames:[bad]}),null);
 assert.equal(parse({version:3,frames:[{index:30,x:0,y:0,width:0,height:0}]}).frames.get(30).width,0);
 assert.equal(project(model([item(1,30),item(2,30)]),null),null);
});
