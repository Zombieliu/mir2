// Verify every valid audited floor reference against atlas/keyed PNG pixels.
// Usage: node scripts/verify-map0-floor-coverage.mjs <audit report JSON>
import fs from 'node:fs';import sharp from 'sharp';import {fileURLToPath} from 'node:url';
const reportPath=process.argv[2];if(!reportPath)throw Error('Specify JSON from audit-map0-floor-coverage --report');
const root=fileURLToPath(new URL('../public/',import.meta.url)).replace(/[\\/]$/,''),report=JSON.parse(fs.readFileSync(reportPath));
const atlas=JSON.parse(fs.readFileSync(root+'/generated/map-atlas/manifest.json')),keyed=JSON.parse(fs.readFileSync(root+'/generated/native-map-keyed/manifest.json'));
const rects=new Map(atlas.pages.flatMap(p=>p.r.map(r=>[p.l+'#'+r[0],{p,r}]))),keys=new Map(keyed.entries.map(e=>[e.key,e])),cache=new Map();
let checked=0;const errors=[];
for(const lib of report.raw)for(const f of lib.valid){const key=lib.library+'#'+f,item=rects.get(key);if(!item){errors.push({key,error:'missing rect'});continue;}const{p,r}=item;
 if(!cache.has(p.u))cache.set(p.u,await sharp(root+p.u).ensureAlpha().raw().toBuffer({resolveWithObject:true}));const page=cache.get(p.u),source=await sharp(root+'/original-map/'+lib.library+'/'+f+'.png').ensureAlpha().raw().toBuffer({resolveWithObject:true});
 if(page.info.width!==p.w||page.info.height!==p.h||source.info.width!==r[3]||source.info.height!==r[4])errors.push({key,error:'dimensions'});
 let same=true;for(let y=0;y<r[4];y++){const offset=((r[2]+y)*p.w+r[1])*4;if(!page.data.subarray(offset,offset+r[3]*4).equals(source.data.subarray(y*r[3]*4,(y+1)*r[3]*4)))same=false;}if(!same)errors.push({key,error:'RGBA mismatch'});checked++;}
let standalone=0;for(const lib of report.standalone)for(const f of lib.valid){const key=lib.library+'#'+f,e=keys.get(key);if(!e){errors.push({key,error:'missing standalone'});continue;}const source=await sharp(root+'/original-map/'+lib.library+'/'+f+'.png').ensureAlpha().raw().toBuffer({resolveWithObject:true}),image=await sharp(root+e.imageUrl).ensureAlpha().raw().toBuffer({resolveWithObject:true});if(source.info.width!==e.width||source.info.height!==e.height||!source.data.equals(image.data))errors.push({key,error:'standalone RGBA mismatch'});standalone++;}
const result={rawChecked:checked,standaloneChecked:standalone,validFloorReferences:checked+standalone,noDraw:report.noDraw,errors};console.log(JSON.stringify(result,null,2));if(errors.length)process.exitCode=1;
