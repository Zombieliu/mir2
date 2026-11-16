// Publish verified staging packs without deleting or overwriting immutable PNGs.
import fs from 'node:fs';import path from 'node:path';import crypto from 'node:crypto';
const args=Object.fromEntries(process.argv.slice(2).map(a=>{const[k,...v]=a.replace(/^--/,'').split('=');return[k,v.join('=')];}));
if(!args.atlas||!args.keyed||!args.backup)throw Error('Use --atlas=<staged manifest> --keyed=<staged manifest> --backup=<new evidence directory> after verification');
const root=path.resolve(import.meta.dirname,'../public'),sha=b=>crypto.createHash('sha256').update(b).digest('hex');
const read=p=>JSON.parse(fs.readFileSync(p));
const immutable=(dest,bytes)=>{if(fs.existsSync(dest)){if(!fs.readFileSync(dest).equals(bytes))throw Error('Immutable collision '+dest);return;}fs.mkdirSync(path.dirname(dest),{recursive:true});fs.writeFileSync(dest,bytes,{flag:'wx'});};
const atomic=(dest,bytes)=>{const temp=dest+'.verified-resource-repair.tmp';fs.writeFileSync(temp,bytes,{flag:'wx'});fs.renameSync(temp,dest);};
fs.mkdirSync(args.backup,{recursive:true});
const atlas=read(args.atlas),keyed=read(args.keyed),atlasDest=path.join(root,'generated/map-atlas'),keyedDest=path.join(root,'generated/native-map-keyed');
const oldAtlas=read(path.join(atlasDest,'manifest.json')),oldKeyed=read(path.join(keyedDest,'manifest.json'));
const rawKeys=new Set(atlas.pages.flatMap(p=>p.r.map(r=>p.l+'#'+r[0]))),keys=new Set(keyed.entries.map(e=>e.key));
for(const p of oldAtlas.pages)for(const r of p.r)if(!rawKeys.has(p.l+'#'+r[0]))throw Error('Lost old atlas key');
for(const e of oldKeyed.entries)if(!keys.has(e.key))throw Error('Lost old standalone key');
for(const p of atlas.pages){const source=path.resolve(root,'.'+p.u);if(!source.startsWith(root+path.sep))throw Error('Invalid page path');const bytes=fs.readFileSync(source),name=path.basename(p.u);if(!name.endsWith('.'+sha(bytes).slice(0,16)+'.png'))throw Error('Page hash mismatch');const folder=p.l.split('/').join('-');p.u='/generated/map-atlas/'+folder+'/'+name;immutable(path.join(atlasDest,folder,name),bytes);}
for(const e of keyed.entries){const bytes=fs.readFileSync(path.join(path.dirname(args.keyed),'pages',path.basename(e.imageUrl)));if(path.basename(e.imageUrl)!==sha(bytes)+'.png')throw Error('Keyed hash mismatch');immutable(path.join(keyedDest,'pages',path.basename(e.imageUrl)),bytes);}
for(const [name,dir,next]of [['map-atlas',atlasDest,atlas],['native-map-keyed',keyedDest,keyed]]){const dest=path.join(dir,'manifest.json'),old=fs.readFileSync(dest);immutable(path.join(args.backup,name+'-before.json'),old);const bytes=Buffer.from(JSON.stringify(next)+'\n');immutable(path.join(dir,'manifest.'+sha(bytes)+'.json'),bytes);atomic(dest,bytes);console.log(name+' '+sha(bytes));}
