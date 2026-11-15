import assert from 'node:assert/strict';
import {readFile,readdir} from 'node:fs/promises';
import {resolve,join} from 'node:path';
import {verifyWalkingChunkGroup} from './lib/verify-r16-walking-chunks.mjs';
const [dist,...extra]=process.argv.slice(2);assert(dist&&!extra.length,'Provide one existing build output directory');
const directory=join(resolve(dist),'static/chunks'),files=await readdir(directory);
const manifest=JSON.parse(await readFile(new URL('../lib/generated/r16-entry-walking-parts.json',import.meta.url)));
const bodies=[];
for(const part of manifest.parts){
 const prefix='r16-entry-monster-walking-'+part.libraryKey.slice(-3)+'-fallback.';
 const matches=files.filter(file=>file.startsWith(prefix)&&file.endsWith('.js'));assert.equal(matches.length,1,part.libraryKey);
 bodies.push({libraryKey:part.libraryKey,body:await readFile(join(directory,matches[0]))});
}
console.log(JSON.stringify(verifyWalkingChunkGroup(bodies,manifest).receipt));
