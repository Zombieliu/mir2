// Verify every valid frame and route from the whole-map audit, optionally against source Lib RGBA.
import fs from 'node:fs';import path from 'node:path';import sharp from 'sharp';
import {parseLibrary,decodeFrameRgba} from './crystal-library.mjs';
const args=Object.fromEntries(process.argv.slice(2).map(a=>{const[k,...v]=a.replace(/^--/,'').split('=');return[k,v.join('=')];}));
if(!args.report)throw Error('Use --report=<audit JSON> [--data=<Crystal Data>] [--atlas=<manifest>] [--keyed=<manifest>] [--result=<JSON>]');
const root=path.resolve(import.meta.dirname,'../public'),report=JSON.parse(fs.readFileSync(args.report));
if(!report.valid?.length||report.missingLibraries.length||report.unsupportedRoutes.length)throw Error('Incomplete/empty audit report');
const atlasPath=args.atlas??path.join(root,'generated/map-atlas/manifest.json'),keyedPath=args.keyed??path.join(root,'generated/native-map-keyed/manifest.json');
const atlas=JSON.parse(fs.readFileSync(atlasPath)),keyed=JSON.parse(fs.readFileSync(keyedPath));
const rects=new Map(),keys=new Map();for(const p of atlas.pages)for(const r of p.r){const k=p.l+'#'+r[0];if(rects.has(k))throw Error('Duplicate atlas key '+k);rects.set(k,{p,r});}for(const e of keyed.entries){if(keys.has(e.key))throw Error('Duplicate standalone '+e.key);keys.set(e.key,e);}
const cache=new Map(),errors=[],libCache=new Map();const decode=async p=>sharp(p).ensureAlpha().raw().toBuffer({resolveWithObject:true});
let raw=0,standalone=0,libCompared=0;
for(const ref of report.valid){const srcPath=path.join(root,'original-map',ref.library,ref.frame+'.png');const src=await decode(srcPath);
 if(src.info.width!==ref.width||src.info.height!==ref.height)errors.push({key:ref.key,error:'source dimensions'});
 if(args.data){if(!libCache.has(ref.library)){libCache.clear();libCache.set(ref.library,parseLibrary(fs.readFileSync(path.join(args.data,'Map',ref.library)+'.Lib')));}const lib=libCache.get(ref.library),f=lib.frames[ref.frame];if(!f||!Buffer.from(decodeFrameRgba(lib,f)).equals(src.data))errors.push({key:ref.key,error:'source Lib RGBA mismatch'});libCompared++;}
 for(const route of ref.routes){if(route==='raw'){const item=rects.get(ref.key);if(!item){errors.push({key:ref.key,error:'missing raw'});continue;}const{p,r}=item;
 if(!cache.has(p.u))cache.set(p.u,await decode(path.join(root,p.u)));const page=cache.get(p.u);
 if(page.info.width!==p.w||page.info.height!==p.h||r[1]<0||r[2]<0||r[3]!==src.info.width||r[4]!==src.info.height||r[1]+r[3]>p.w||r[2]+r[4]>p.h){errors.push({key:ref.key,error:'atlas geometry'});continue;}
 let same=true;for(let y=0;y<r[4];y++){const o=((r[2]+y)*p.w+r[1])*4;if(!page.data.subarray(o,o+r[3]*4).equals(src.data.subarray(y*r[3]*4,(y+1)*r[3]*4)))same=false;}if(!same)errors.push({key:ref.key,error:'atlas RGBA'});raw++;
 }else if(route==='standalone'){const e=keys.get(ref.key);if(!e){errors.push({key:ref.key,error:'missing keyed'});continue;}
 const file=args.keyed?path.join(path.dirname(keyedPath),'pages',path.basename(e.imageUrl)):path.join(root,e.imageUrl);
 const image=await decode(file);if(e.width!==src.info.width||e.height!==src.info.height||image.info.width!==e.width||image.info.height!==e.height||!image.data.equals(src.data))errors.push({key:ref.key,error:'keyed dimensions/RGBA'});standalone++;
 }else throw Error('Unknown route '+route);}
}
const validKeys=new Set(report.valid.map(r=>r.key)),noDraw=new Set(report.noDraw.map(r=>r.key));
const sourceIncompleteFamilies=report.families.filter(f=>f.some(k=>noDraw.has(k)));
const familiesWithUnclassifiedFrame=report.families.filter(f=>f.some(k=>!noDraw.has(k)&&!validKeys.has(k)));
if(familiesWithUnclassifiedFrame.length)errors.push({error:'unclassified animation frames',count:familiesWithUnclassifiedFrame.length});
const result={valid:report.valid.length,raw,standalone,libCompared,noDraw:report.noDraw.length,animationFamilies:report.families.length,sourceIncompleteFamilies:sourceIncompleteFamilies.length,errors};
console.log(JSON.stringify(result,null,2));if(args.result)fs.writeFileSync(args.result,JSON.stringify(result,null,2));if(errors.length)process.exitCode=1;
