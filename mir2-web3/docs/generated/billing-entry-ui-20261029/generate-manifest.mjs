import fs from 'node:fs/promises';
import path from 'node:path';
import crypto from 'node:crypto';
import {fileURLToPath} from 'node:url';

const here=path.dirname(fileURLToPath(import.meta.url));
const project=path.resolve(here,'../../..');
const flags=new Set(process.argv.slice(2));
for(const flag of flags)if(!['--verify','--require-complete'].includes(flag))throw Error('Unknown flag: '+flag);
const sha=data=>crypto.createHash('sha256').update(data).digest('hex');
const index=JSON.parse(await fs.readFile(path.join(here,'evidence-index.json'),'utf8'));
if(flags.has('--require-complete')&&(!index.complete||index.checks.some(c=>c.status!=='passed'||!c.path)))throw Error('Required actual Windows or packaging evidence remains pending');
for(const check of index.checks.filter(c=>c.status==='passed')){
  if(!check.path||path.isAbsolute(check.path))throw Error('Unsafe evidence reference');
  const resolved=path.resolve(here,check.path);
  if(!resolved.startsWith(path.join(project,'docs','generated')+path.sep))throw Error('Evidence reference escaped generated topics');
  const report=JSON.parse(await fs.readFile(resolved,'utf8'));
  if(!report.passed)throw Error('Passed index row has no passing receipt: '+check.name);
}

async function walk(directory){
  const found=[];
  for(const entry of await fs.readdir(directory,{withFileTypes:true})){
    if(entry.isSymbolicLink())throw Error('Symlink artifact forbidden');
    const target=path.join(directory,entry.name);
    if(entry.isDirectory())found.push(...await walk(target));else found.push(target);
  }
  return found;
}
const sourcePins=[];
const allowedSourcePaths=new Set();
for(const spec of [...index.sourceFiles].sort((a,b)=>a.path.localeCompare(b.path))){
  if(path.isAbsolute(spec.path)||spec.path.split(/[\\/]/).includes('..')||allowedSourcePaths.has(spec.path))throw Error('Unsafe or duplicate source path');
  allowedSourcePaths.add(spec.path);
  const raw=await fs.readFile(path.join(project,spec.path));
  const content=spec.raw?raw:Buffer.from(raw.toString('utf8').replace(/^\uFEFF/,'').replaceAll('\r\n','\n'),'utf8');
  const pin={path:spec.path,normalization:spec.raw?'raw bytes':'UTF-8 CRLF to LF',bytes:content.length,sha256:sha(content)};
  if(spec.expectedSha256&&spec.expectedSha256!==pin.sha256)throw Error('Immutable font/source pin differs: '+spec.path);
  sourcePins.push(pin);
}

const artifacts=[];
for(const file of (await walk(here)).sort()){
  const relative=path.relative(here,file).replaceAll('\\','/');
  if(relative==='manifest.json')continue;
  if(/(^|\/)(private[^/]*|accounts?|appdata|userprefs|node_modules|target|harness[^/]*|\.env[^/]*)(\/|$)/i.test(relative)||/\.(exe|dll|keystore|key|pem|log|ttf|otf|eot|whl)$/i.test(relative))throw Error('Forbidden artifact: '+relative);
  if(!/\.(json|md|png|jpg|jpeg|webp)$/i.test(relative)&&!['.gitattributes','.gitignore','generate-manifest.mjs'].includes(relative))throw Error('Unexpected artifact type: '+relative);
  const raw=await fs.readFile(file);
  if(!/\.(png|jpg|jpeg|webp)$/i.test(relative)){
    const text=raw.toString('utf8');
    if(/suiprivkey1[0-9a-z]{15,}|-----BEGIN [^-]*PRIVATE KEY-----|Bearer\s+\S{16,}|\/pay#usdc_[a-f0-9]{64}\.[A-Za-z0-9_-]{43}/.test(text))throw Error('Sensitive content: '+relative);
    if(relative.endsWith('.json')){
      const report=JSON.parse(text);
      for(const pin of report.sourcePins??[]){
        if(!allowedSourcePaths.has(pin.path))throw Error('Undeclared recorded source: '+pin.path);
        const source=await fs.readFile(path.join(project,pin.path));
        const content=pin.normalization==='raw bytes'?source:Buffer.from(source.toString('utf8').replace(/^\uFEFF/,'').replaceAll('\r\n','\n'),'utf8');
        if(sha(content)!==pin.sha256)throw Error('Recorded receipt source is stale: '+pin.path);
      }
    }
  }
  artifacts.push({path:relative,bytes:raw.length,sha256:sha(raw)});
}
const manifest={formatVersion:2,topic:index.topic,evidenceStatus:index.status,complete:index.complete,
  artifactHashing:'raw bytes; topic-scoped * -text -whitespace',
  sourceHashing:'UTF-8 CRLF to LF for Rust/JSON/text; raw bytes for font/source artifacts explicitly marked -text',
  artifactCount:artifacts.length,artifactBytes:artifacts.reduce((sum,item)=>sum+item.bytes,0),artifacts,sourcePins};
if(flags.has('--verify')){
  const existing=JSON.parse(await fs.readFile(path.join(here,'manifest.json'),'utf8'));
  if(JSON.stringify(existing)!==JSON.stringify(manifest))throw Error('Artifact or source manifest is stale');
  console.log(JSON.stringify({verified:true,complete:index.complete,artifacts:artifacts.length,sourcePins:sourcePins.length}));
}else{
  await fs.writeFile(path.join(here,'manifest.json'),JSON.stringify(manifest,null,2)+'\n');
  console.log(JSON.stringify({written:true,complete:index.complete,artifacts:artifacts.length,sourcePins:sourcePins.length}));
}
