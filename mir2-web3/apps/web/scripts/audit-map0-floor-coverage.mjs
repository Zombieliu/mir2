// Audit map 0 against valid original Crystal floor frames. By default read-only.
// --export-missing writes only absent PNGs with exclusive creation; atlas publication is separate.
// Usage: node scripts/audit-map0-floor-coverage.mjs <Crystal Data> [--export-missing] [--report=<path>]
import fs from 'node:fs';import path from 'node:path';import z from 'node:zlib';
import {mapLibraryKeyForIndex,parseType100Map,decodeCrystalMiddleAnimationCount,decodeCrystalFrontAnimationCount} from './build-native-keyed-map-pack.mjs';
import {mapAtlasLibrarySupportsRawUpload} from './build-map-atlas-pack.mjs';
import {parseLibrary,decodeFrameRgba,encodePng} from './crystal-library.mjs';
const dataDir=process.argv[2];if(!dataDir)throw Error('Specify Crystal Data directory');
const apply=process.argv.includes('--export-missing');
const web=path.resolve(import.meta.dirname,'..'),pub=path.join(web,'public');
const map=parseType100Map(z.gunzipSync(fs.readFileSync(path.join(web,'lib/generated/crystal-map-pack/0.map.gz'))));
const grouped=new Map();
for(let x=0;x<map.width;x++)for(let y=0;y<map.height;y++){
 const c=map.cells[x*map.height+y];
 for(const [layer,index,frame,count] of [['back',c.backIndex,(c.backImage&0x1fffffff)-1,0],['middle',c.middleIndex,c.middleImage-1,decodeCrystalMiddleAnimationCount(c.middleAnimationFrame)],['front',c.frontIndex,(c.frontImage&32767)-1,decodeCrystalFrontAnimationCount(c.frontAnimationFrame)]]){
 if(index<0||frame<0||(layer==='back'&&(x%2||y%2)))continue;
 const lib=mapLibraryKeyForIndex(index);if(!grouped.has(lib))grouped.set(lib,new Map());
 const refs=grouped.get(lib);const prior=refs.get(frame);refs.set(frame,{back:layer==='back'||prior?.back,static:count<=1||prior?.static});
 }
}
const atlas=JSON.parse(fs.readFileSync(path.join(pub,'generated/map-atlas/manifest.json')));
const rawKeys=new Set(atlas.pages.flatMap(p=>p.r.map(r=>p.l+'#'+r[0])));
const keyed=JSON.parse(fs.readFileSync(path.join(pub,'generated/native-map-keyed/manifest.json')));
const standaloneKeys=new Set(keyed.entries.map(e=>e.key));
const report={map:'0',raw:[],standalone:[],missingLibraries:[],noDraw:0,exported:0};
for(const [library,refs] of grouped){
 const file=path.join(dataDir,'Map',...library.split('/'))+'.Lib';
 if(!fs.existsSync(file)){report.missingLibraries.push(library);continue;}
 const lib=parseLibrary(fs.readFileSync(file));const result={library,valid:[],missing:[],atlasMissing:[]};
 for(const [frame,ref]of refs){const f=lib.frames[frame];if(!f||f.width<=0||f.height<=0){if(ref.back)report.noDraw++;continue;}
 if(!ref.back&&(!ref.static||!((f.width===48&&f.height===32)||(f.width===96&&f.height===64))))continue;
 result.valid.push(frame);
 if(!(mapAtlasLibrarySupportsRawUpload(library)?rawKeys:standaloneKeys).has(library+'#'+frame))result.atlasMissing.push(frame);
 const out=path.join(pub,'original-map',library,frame+'.png');
 if(!fs.existsSync(out)){result.missing.push(frame);if(apply){const rgba=decodeFrameRgba(lib,f);const png=encodePng(f.width,f.height,rgba,1);fs.mkdirSync(path.dirname(out),{recursive:true});fs.writeFileSync(out,png,{flag:'wx'});report.exported++;}}
 }
 if(result.valid.length)report[mapAtlasLibrarySupportsRawUpload(library)?'raw':'standalone'].push(result);
}
const reportPath=process.argv.find(arg=>arg.startsWith('--report='))?.slice('--report='.length);
if(reportPath)fs.writeFileSync(reportPath,JSON.stringify(report,null,2));
console.log(JSON.stringify({...report,raw:report.raw.map(r=>({library:r.library,valid:r.valid.length,missing:r.missing.length,atlasMissing:r.atlasMissing.length})),standalone:report.standalone.map(r=>({library:r.library,valid:r.valid.length,missing:r.missing.length,atlasMissing:r.atlasMissing.length}))},null,2));

if(!apply && (report.missingLibraries.length || [...report.raw,...report.standalone].some(r=>r.missing.length||r.atlasMissing.length)))process.exitCode=1;
