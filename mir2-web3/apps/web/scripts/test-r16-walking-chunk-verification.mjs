import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import test from 'node:test';
import {verifyWalkingPartScript,verifyWalkingChunkGroup} from './lib/verify-r16-walking-chunks.mjs';
const manifest=JSON.parse(readFileSync(new URL('../lib/generated/r16-entry-walking-parts.json',import.meta.url)));
const values=manifest.parts.map(part=>JSON.parse(readFileSync(new URL('../public'+part.path,import.meta.url))));
const script=value=>Buffer.from('self.webpackChunk.push([[1],{1:function(m){m.exports=JSON.parse('+JSON.stringify(JSON.stringify(value))+');}}]);');
const bodies=manifest.parts.map((part,index)=>({libraryKey:part.libraryKey,body:script(values[index])}));
test('all five compiled script literals reconstruct and decode the exact original 240 PNGs',()=>{
 const proof=verifyWalkingChunkGroup(bodies.toReversed(),manifest);
 assert(proof.receipt.passed);assert.equal(proof.receipt.payloadBytes,764724);assert.equal(proof.receipt.pngs,240);
 assert.equal(proof.receipt.payloadSha256,'41fcaae2757ad0351da2e3840bf54008952b5c2714441530bb64c21f5c9fd64f');
});
test('script verification never evaluates unrelated module code',()=>{
 const bytes=Buffer.concat([bodies[0].body,Buffer.from(';globalThis.__walkingVerifierExecuted=true;')]);
 assert(verifyWalkingPartScript(bytes,manifest.parts[0]).receipt.passed);assert.equal(globalThis.__walkingVerifierExecuted,undefined);
});
test('missing, duplicate or incomplete script bodies cannot qualify the group',()=>{
 for(const input of [bodies.slice(1),[...bodies.slice(0,4),bodies[0]],bodies.map((entry,index)=>index===0?{...entry,body:entry.body.subarray(0,100)}:entry)])
  assert.throws(()=>verifyWalkingChunkGroup(input,manifest));
});
test('a changed PNG or a different aggregate pin is rejected',()=>{
 const value=structuredClone(values[0]);value.images[Object.keys(value.images)[0]]+='A';
 assert.throws(()=>verifyWalkingChunkGroup([{libraryKey:manifest.parts[0].libraryKey,body:script(value)},...bodies.slice(1)],manifest));
 assert.throws(()=>verifyWalkingChunkGroup(bodies,{...manifest,sourceSha256:'0'.repeat(64)}),/whole walking pack digest mismatch/);
});
test('two matching payload literals in one response are ambiguous and rejected',()=>{
 assert.throws(()=>verifyWalkingPartScript(Buffer.concat([bodies[0].body,bodies[0].body]),manifest.parts[0]),/must be unique/);
});
