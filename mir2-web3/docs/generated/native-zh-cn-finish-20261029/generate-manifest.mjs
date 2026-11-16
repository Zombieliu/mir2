import fs from 'node:fs/promises';
import path from 'node:path';
import crypto from 'node:crypto';
import {fileURLToPath} from 'node:url';
import {execFileSync} from 'node:child_process';

const here=path.dirname(fileURLToPath(import.meta.url));
const project=path.resolve(here,'../../..');
const flags=new Set(process.argv.slice(2));
for(const flag of flags)if(!['--verify','--verify-staged','--require-complete'].includes(flag))throw Error('Unknown flag: '+flag);
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
if(flags.has('--verify')||flags.has('--verify-staged')){
  const existingRaw=await fs.readFile(path.join(here,'manifest.json'));
  const existing=JSON.parse(existingRaw.toString('utf8'));
  if(JSON.stringify(existing)!==JSON.stringify(manifest))throw Error('Artifact or source manifest is stale');
  let stagedVerified=false;
  if(flags.has('--verify-staged')){
    // Read the actual index blobs, not a worktree diff or another checkout.
    const git=(args)=>execFileSync('git',['-C',project,...args],{windowsHide:true,maxBuffer:64*1024*1024,stdio:['ignore','pipe','pipe']});
    const repository=git(['rev-parse','--show-toplevel']).toString('utf8').trim();
    const prefix=path.relative(repository,project).replaceAll('\\','/');
    if(prefix==='..'||prefix.startsWith('../'))throw Error('Project escaped Git repository');
    const repositoryPath=relative=>prefix?prefix+'/'+relative:relative;
    const topic=path.relative(project,here).replaceAll('\\','/');
    const readStaged=relative=>{
      try{return git(['show',':'+repositoryPath(relative)]);}
      catch{throw Error('Missing readable staged blob: '+relative);}
    };
    const expectedTopicFiles=new Set([...artifacts.map(item=>repositoryPath(topic+'/'+item.path)),repositoryPath(topic+'/manifest.json')]);
    const stagedTopicFiles=new Set();
    for(const record of git(['ls-files','--stage','--full-name','-z','--',topic]).toString('utf8').split('\0').filter(Boolean)){
      const match=record.match(/^([0-9]+) ([a-f0-9]+) ([0-3])\t(.+)$/s);
      if(!match||match[3]!=='0'||!['100644','100755'].includes(match[1]))throw Error('Conflicted or non-regular staged evidence file');
      stagedTopicFiles.add(match[4]);
    }
    if(stagedTopicFiles.size!==expectedTopicFiles.size||[...expectedTopicFiles].some(file=>!stagedTopicFiles.has(file)))throw Error('Staged topic file set differs from manifest');
    if(!readStaged(topic+'/manifest.json').equals(existingRaw))throw Error('Staged manifest differs from verified worktree manifest');
    for(const item of artifacts){
      const raw=readStaged(topic+'/'+item.path);
      if(raw.length!==item.bytes||sha(raw)!==item.sha256)throw Error('Staged artifact differs: '+item.path);
    }
    for(const item of sourcePins){
      const raw=readStaged(item.path);
      const content=item.normalization==='raw bytes'?raw:Buffer.from(raw.toString('utf8').replace(/^\uFEFF/,'').replaceAll('\r\n','\n'),'utf8');
      if(content.length!==item.bytes||sha(content)!==item.sha256)throw Error('Staged source differs: '+item.path);
    }
    stagedVerified=true;
  }
  console.log(JSON.stringify({verified:true,stagedVerified,complete:index.complete,artifacts:artifacts.length,sourcePins:sourcePins.length}));
}else{
  await fs.writeFile(path.join(here,'manifest.json'),JSON.stringify(manifest,null,2)+'\n');
  console.log(JSON.stringify({written:true,complete:index.complete,artifacts:artifacts.length,sourcePins:sourcePins.length}));
}
