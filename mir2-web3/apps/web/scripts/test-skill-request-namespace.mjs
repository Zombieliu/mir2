import assert from 'node:assert/strict';
import test from 'node:test';
import {readFileSync} from 'node:fs';
import ts from 'typescript';
const source=new URL('../lib/bevy-spells-ui.ts',import.meta.url);
const code=ts.transpileModule(readFileSync(source,'utf8'),{compilerOptions:{module:ts.ModuleKind.CommonJS,target:ts.ScriptTarget.ES2022}}).outputText;
function load(realm){const m={exports:{}};const factory=new Function('exports','module','globalThis',`${code};return createSpellsRunAllocator;`)(m.exports,m,realm);return{ui:m.exports,factory};}
const realm={},first=load(realm),ui=first.ui;
const key='__mir2SpellsRequestNamespaceV1',BLOCK=2**22,MAX_RUN=2**31-1;
test('fresh controllers burn independent HMR-safe run; all numeric boundaries are JS-safe',()=>{
 const a=ui.burnSpellsRun(),b=ui.burnSpellsRun();assert.equal(b,a+1);assert.equal(ui.requestRunOf(a*BLOCK+1),a);assert.equal(ui.requestRunOf(b*BLOCK+1),b);
 assert.equal(ui.requestRunOf(Number.MAX_SAFE_INTEGER),MAX_RUN);
 for(const id of [0,1,BLOCK,BLOCK*2,Number.MAX_SAFE_INTEGER+1,NaN,Infinity,-1,1.5])assert.equal(ui.requestRunOf(id),null);
});
test('actual module reload retains immutable slot and closure highwater despite rollback/replacement/deletion',()=>{
 const before=realm[key].last,reloaded=load(realm).ui;assert.equal(reloaded.burnSpellsRun(),before+1);
 assert.equal(Object.isFrozen(realm[key]),true);assert.equal(Reflect.set(realm[key],'last',0),false);assert.equal(Reflect.set(realm,key,{version:1,last:0,poisoned:false}),false);assert.equal(Reflect.deleteProperty(realm,key),false);assert.throws(()=>Object.defineProperty(realm,key,{value:{}}));
 assert.equal(ui.burnSpellsRun(),before+2);assert.equal(reloaded.burnSpellsRun(),before+3);
});
test('checked closure exhaustion poisons permanently and writable legacy/corrupt slots fail closed',()=>{
 const exhausted={},module=load(exhausted),state=module.factory(MAX_RUN-1);Object.defineProperty(exhausted,key,{value:state,writable:false,configurable:false});assert.equal(module.ui.burnSpellsRun(),MAX_RUN);assert.throws(()=>module.ui.burnSpellsRun());assert.equal(state.last,MAX_RUN);assert.equal(state.poisoned,true);assert.throws(()=>module.ui.burnSpellsRun());
 for(const bad of [{version:1,last:0,poisoned:false},{version:2,last:3,poisoned:false},{version:1,last:NaN,poisoned:false},{version:1,last:-1,poisoned:false},{version:1,last:3,poisoned:true}]){const broken={[key]:bad},api=load(broken).ui;assert.throws(()=>api.burnSpellsRun());assert.throws(()=>api.burnSpellsRun());assert.equal(broken[key],bad);}
 const wrong={},api=load(wrong).ui;Object.defineProperty(wrong,key,{value:Object.freeze({version:1,last:0,poisoned:false,allocate:()=>0}),writable:false,configurable:false});assert.throws(()=>api.burnSpellsRun());
});
