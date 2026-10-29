import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import sharp from 'sharp';
import {validateFloorReport, uniqueIndex, verifyFloorCoverage} from './verify-map0-floor-coverage.mjs';
const library = 'WemadeMir3/Snow/SmObjectsc';
const report = () => ({map:'0',raw:[],standalone:[{library,valid:[2766],missing:[],atlasMissing:[]}],missingLibraries:[],noDraw:0});
test('rejects empty, incomplete, duplicate and unroutable audit reports', () => {
  assert.doesNotThrow(()=>validateFloorReport(report()));
  for(const mutate of [r=>r.standalone=[],r=>r.missingLibraries=['missing'],r=>r.standalone[0].missing=[1],
    r=>r.standalone[0].atlasMissing=[1],r=>r.standalone[0].valid=[2766,2766],r=>r.standalone[0].library='Bogus/SmObjectsbad']) {
    const r=report();mutate(r);assert.throws(()=>validateFloorReport(r));
  }
  assert.throws(()=>uniqueIndex([{key:'x'},{key:'x'}],e=>e.key),/Duplicate/);
});
test('standalone dimensions and raw rectangle bounds reject identical pixel buffers with wrong geometry', async()=>{
  const prefix=path.join(os.tmpdir(),'map-floor-verify-');
  const root=await fs.mkdtemp(prefix);
  const write=async(rel,data)=>{const dest=path.join(root,rel);await fs.mkdir(path.dirname(dest),{recursive:true});await fs.writeFile(dest,data);};
  const png=async(w,h)=>sharp({create:{width:w,height:h,channels:4,background:{r:80,g:40,b:20,alpha:1}}}).png().toBuffer();
  try {
    await write(`original-map/${library}/2766.png`,await png(2,1));
    await write('source.png',await png(1,2));
    await write('generated/map-atlas/manifest.json',JSON.stringify({pages:[]}));
    const entry={key:`${library}#2766`,imageUrl:'/source.png',width:2,height:1};
    await write('generated/native-map-keyed/manifest.json',JSON.stringify({entries:[entry]}));
    assert.deepEqual((await verifyFloorCoverage(root,report())).errors,[{key:entry.key,error:'standalone dimensions'}]);
    await write('source.png',await png(2,1));
    assert.deepEqual((await verifyFloorCoverage(root,report())).errors,[]);
    await write('generated/native-map-keyed/manifest.json',JSON.stringify({entries:[entry,entry]}));
    await assert.rejects(verifyFloorCoverage(root,report()),/Duplicate/);
    await write('generated/native-map-keyed/manifest.json',JSON.stringify({entries:[]}));
    const raw=report();raw.raw=[{...raw.standalone[0],library:'WemadeMir2/Tiles'}];raw.standalone=[];
    await write('original-map/WemadeMir2/Tiles/2766.png',await png(2,1));
    const page={l:'WemadeMir2/Tiles',u:'/source.png',w:2,h:1,r:[[2766,1,0,2,1]]};
    await write('generated/map-atlas/manifest.json',JSON.stringify({pages:[page]}));
    assert.equal((await verifyFloorCoverage(root,raw)).errors[0].error,'dimensions/rect bounds');
    page.r=[[2766,0,0,2,1],[2766,0,0,2,1]];
    await write('generated/map-atlas/manifest.json',JSON.stringify({pages:[page]}));
    await assert.rejects(verifyFloorCoverage(root,raw),/Duplicate/);
  } finally {
    assert.ok(path.resolve(root).startsWith(path.resolve(prefix)));
    await fs.rm(root,{recursive:true,force:true});
  }
});
