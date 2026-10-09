import assert from "node:assert/strict";
import test from "node:test";
import {readFileSync} from "node:fs";
import ts from "typescript";
const m={exports:{}};new Function("exports","module",ts.transpileModule(readFileSync(new URL("../lib/world-model/character-appearance.ts",import.meta.url),"utf8"),{compilerOptions:{module:ts.ModuleKind.CommonJS,target:ts.ScriptTarget.ES2022}}).outputText)(m.exports,m);
const {projectSelfAppearance:project,sameSelfAppearance}=m.exports;
test("same owner retains missing and invalid optional u8; explicit zero clears",()=>{
 const prior={owner:"a",hair:7,wingEffect:2};
 assert.deepEqual(project(prior,"a",[],1),prior);
 assert.deepEqual(project(prior,"a",[{objectId:1,hair:null,wingEffect:256}],1),prior);
 assert.deepEqual(project(prior,"a",[{objectId:1,hair:0,wing_effect:0}],1),{owner:"a",hair:0,wingEffect:0});
});
test("new owner loses prior appearance and null id cannot select remote",()=>{
 assert.deepEqual(project({owner:"a",hair:7,wingEffect:2},"b",[{objectId:null,kind:"player",hair:5}],null),{owner:"b"});
 assert.deepEqual(project(null,"b",[{objectId:3,kind:"selfPlayer",hair:4,wingEffect:1}],null),{owner:"b",hair:4,wingEffect:1});
 assert.deepEqual(project(null,"b",[{objectId:2,kind:"player",hair:5},{objectId:1,kind:"player",hair:0}],1),{owner:"b",hair:0});
});
test("movement comparison distinguishes unknown, zero and authoritative appearance change",()=>{
 assert.equal(sameSelfAppearance({}, {wingEffect:0}),false);
 assert.equal(sameSelfAppearance({hair:1},{hair:2}),false);
 assert.equal(sameSelfAppearance({hair:0},{hair:0}),true);
});
