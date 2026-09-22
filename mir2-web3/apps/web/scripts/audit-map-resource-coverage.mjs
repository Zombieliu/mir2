// Whole-map reference audit/export. Empty original Lib slots are reported, never fabricated.
import fs from 'node:fs';import path from 'node:path';import z from 'node:zlib';
import {parsePackagedMap,mapLibraryKeyForIndex,mapAtlasPathRequiresAlphaKey,decodeCrystalMiddleAnimationCount,decodeCrystalFrontAnimationCount,crystalMiddleMapBlendMode,crystalFrontMapBlendMode} from './build-native-keyed-map-pack.mjs';
import {mapAtlasLibrarySupportsRawUpload} from './build-map-atlas-pack.mjs';
import {parseLibrary,decodeFrameRgba,encodePng} from './crystal-library.mjs';
const args=Object.fromEntries(process.argv.slice(2).map(a=>{const [k,...v]=a.replace(/^--/,'').split('=');return[k,v.length?v.join('='):true];}));
if(!args.data||!args.report)throw Error('Use --data=<Crystal Data> --report=<JSON> [--maps=0,d001,d401,1,d021,d022] [--export-missing]');
const web=path.resolve(import.meta.dirname,'..'),pub=path.join(web,'public');
const refs=new Map(),maps=[],families=new Map();
for(const name of String(args.maps??'0,d001,d401,1,d021,d022').split(',')){
 const bytes=z.gunzipSync(fs.readFileSync(path.join(web,'lib/generated/crystal-map-pack',name.toLowerCase()+'.map.gz')));
 const m=parsePackagedMap(bytes);if(!m)throw Error('Unsupported map '+name);
 let invalidMiddle=0,tileAnimations=0;
 const add=(x,y,layer,index,base,count=1,step=1,additive=false)=>{
 if(index<0||base<0)return;const library=mapLibraryKeyForIndex(index),n=Math.max(1,count);
 const family=[];
 for(let phase=0;phase<n;phase++){
 const frame=base+phase*step;if(!Number.isSafeInteger(frame)||frame<0)throw Error('Invalid frame');
 const key=library+'#'+frame,standalone=additive||mapAtlasPathRequiresAlphaKey('/original-map/'+library+'/'+frame+'.png');
 const route=standalone?'standalone':'raw';family.push(key);
 if(!refs.has(key))refs.set(key,{key,library,frame,maps:[],routes:[],samples:[],additive:false});
 const r=refs.get(key);if(!r.maps.includes(name))r.maps.push(name);if(!r.routes.includes(route))r.routes.push(route);r.additive ||= additive;
 if(r.samples.length<3)r.samples.push({map:name,x,y,layer,phase});
 }
 if(n>1)families.set(name+':'+library+':'+base+':'+n+':'+step,family);
 };
 for(let x=0;x<m.width;x++)for(let y=0;y<m.height;y++){
 const c=m.cells[x*m.height+y];if(!(x%2)&&!(y%2))add(x,y,'back',c.backIndex,(c.backImage&0x1fffffff)-1);
 if(c.middleIndex<0&&c.middleImage>0)invalidMiddle++;
 add(x,y,'middle',c.middleIndex,c.middleImage-1,decodeCrystalMiddleAnimationCount(c.middleAnimationFrame),1,crystalMiddleMapBlendMode(c.middleAnimationFrame)==='additive');
 add(x,y,'front',c.frontIndex,(c.frontImage&32767)-1,decodeCrystalFrontAnimationCount(c.frontAnimationFrame),1,crystalFrontMapBlendMode(c.frontAnimationFrame)==='additive');
 if(bytes[2]===0x43&&bytes[3]===0x23){const o=8+(x*m.height+y)*26,base=bytes.readInt16LE(o+20)-1,count=bytes[o+24];if(base>=0&&count>0){tileAnimations++;add(x,y,'tileAnimation',190,base,count,bytes.readInt16LE(o+22)^0x2000);}}
 }
 maps.push({name,width:m.width,height:m.height,invalidMiddle,tileAnimations});
}
const report={maps,valid:[],noDraw:[],missingLibraries:[],unsupportedRoutes:[],exported:0,families:[...families.values()]};
const groups=new Map();for(const r of refs.values()){if(!groups.has(r.library))groups.set(r.library,[]);groups.get(r.library).push(r);}
for(const[library,rs]of groups){const file=path.join(args.data,'Map',...library.split('/'))+'.Lib';if(!fs.existsSync(file)){report.missingLibraries.push({library,references:rs.length});continue;}
 const lib=parseLibrary(fs.readFileSync(file));
 for(const r of rs){const f=lib.frames[r.frame];if(!f||f.width<=0||f.height<=0){report.noDraw.push(r);continue;}
 if(r.routes.includes('raw')&&!mapAtlasLibrarySupportsRawUpload(library))report.unsupportedRoutes.push(r);
 const file=path.join(pub,'original-map',library,r.frame+'.png');r.width=f.width;r.height=f.height;r.sourceMissing=!fs.existsSync(file);
 if(r.sourceMissing&&args['export-missing']){fs.mkdirSync(path.dirname(file),{recursive:true});fs.writeFileSync(file,encodePng(f.width,f.height,decodeFrameRgba(lib,f),1),{flag:'wx'});report.exported++;}
 report.valid.push(r);
 }
}
fs.writeFileSync(args.report,JSON.stringify(report,null,2));
console.log(JSON.stringify({maps,valid:report.valid.length,noDraw:report.noDraw.length,sourceMissing:report.valid.filter(r=>r.sourceMissing).length,exported:report.exported,missingLibraries:report.missingLibraries,unsupportedRoutes:report.unsupportedRoutes.length,families:families.size,byMap:maps.map(m=>({map:m.name,valid:report.valid.filter(r=>r.maps.includes(m.name)).length,missing:report.valid.filter(r=>r.maps.includes(m.name)&&r.sourceMissing).length,noDraw:report.noDraw.filter(r=>r.maps.includes(m.name)).length}))},null,2));
if(report.missingLibraries.length||report.unsupportedRoutes.length)process.exitCode=1;
