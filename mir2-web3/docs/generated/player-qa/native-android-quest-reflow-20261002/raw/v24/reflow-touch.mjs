import assert from 'node:assert/strict';
import {execFileSync} from 'node:child_process';
import {readFileSync,writeFileSync,mkdirSync} from 'node:fs';
import {createHash} from 'node:crypto';
import {dirname,join} from 'node:path';
import {fileURLToPath} from 'node:url';
const qa=dirname(fileURLToPath(import.meta.url));
const [source,label,stepsJson]=process.argv.slice(2);
assert.match(source??'',/^[a-f0-9]{40}$/);
assert(['wide-touch','wide-touch-retry'].includes(label));
const steps=JSON.parse(stepsJson);assert(Array.isArray(steps)&&steps.length>0&&steps.length<=12);
for(const step of steps){
 assert.match(step.name,/^[a-z][a-z0-9-]+$/);
 assert(['tap','swipe'].includes(step.kind));
 const count=step.kind==='tap'?2:4;assert.equal(step.coordinates.length,count);
 for(let i=0;i<count;i++)assert(Number.isInteger(step.coordinates[i])&&step.coordinates[i]>=0&&step.coordinates[i]<(i%2===0?2340:1080));
}
const out=join(qa,label);mkdirSync(out,{recursive:false});
const proof=JSON.parse(readFileSync(join(qa,'package-v24.json')));assert.equal(proof.source,source);
const artifact=proof.artifacts.find(entry=>entry.variant==='preview');
assert.equal(createHash('sha256').update(readFileSync(artifact.path)).digest('hex'),artifact.sha256);
const adb='/Users/henryliu/Library/Android/sdk/platform-tools/adb',serial='emulator-5554',app='com.mir2.web3.uipreview';
const commands=[],frames=[];
const record=()=>writeFileSync(join(out,'commands.json'),JSON.stringify({source,versionCode:24,apkSha256:artifact.sha256,offlineOnly:true,
 noTransactionButtonTapped:true,physicalDeviceAccepted:false,steps,frames,commands},null,2)+'\n');
function run(name,args,timeout=30000){
 const entry={name,args,started:new Date().toISOString()};commands.push(entry);
 try{const bytes=execFileSync(adb,['-s',serial,...args],{timeout,maxBuffer:32*1024*1024});writeFileSync(join(out,name),bytes,{flag:'wx'});entry.status=0;return bytes;}
 catch(error){entry.status=error.status;writeFileSync(join(out,name+'.failed-stdout'),error.stdout??Buffer.alloc(0),{flag:'wx'});writeFileSync(join(out,name+'.failed-stderr'),error.stderr??Buffer.alloc(0),{flag:'wx'});throw error;}
 finally{entry.finished=new Date().toISOString();record();}
}
const text=(name,args)=>run(name,args).toString();
const delay=ms=>new Promise(resolve=>setTimeout(resolve,ms));
function installed(edge){
 assert.match(text(edge+'-package.txt',['shell','dumpsys','package',app]),/versionCode=24\b/);
 const path=text(edge+'-path.txt',['shell','pm','path',app]).trim().replace(/^package:/,'');
 assert.match(path,/^\/data\/app\/[A-Za-z0-9_+~.\-=/]+\/base\.apk$/);
 assert.equal(text(edge+'-sha.txt',['shell','sha256sum',path]).trim().split(/\s+/)[0],artifact.sha256);
}
let pid;
function frame(name){
 const bytes=run(name+'.png',['exec-out','screencap','-p']);
 assert.equal(bytes.readUInt32BE(16),2340);assert.equal(bytes.readUInt32BE(20),1080);
 const log=text(name+'-pid.log',['logcat','-d','--pid='+pid,'-v','threadtime']);
 assert(log.includes('ANDROID_QUEST_PREVIEW_RECEIVED_MODELS_APPLIED_NOT_LIVE'));
 frames.push({name,pid:Number(pid),originalFrame:name+'.png',log:name+'-pid.log'});record();
}
try{
 assert.match(text('avd.txt',['emu','avd','name']),/^Mir2_API_31_ARM64/);
 assert.equal(text('api.txt',['shell','getprop','ro.build.version.sdk']).trim(),'31');
 text('os.txt',['shell','getprop','ro.build.fingerprint']);text('display.txt',['shell','wm','size']);text('density.txt',['shell','wm','density']);
 installed('before');text('stop-before.txt',['shell','am','force-stop',app]);
 assert.match(text('start.txt',['shell','am','start','-W','-n',app+'/com.mir2.web3.MainActivity','--es','ui_scene','quests-ingress']),/Status: ok/);
 await delay(4000);pid=text('pid.txt',['shell','pidof',app]).trim();assert.match(pid,/^\d+$/);frame('diary');
 for(const step of steps){
  text(step.name+'-input.txt',['shell','input',step.kind,...step.coordinates.map(String),...(step.kind==='swipe'?['700']:[])]);
  await delay(2000);frame(step.name);
 }
 installed('after');text('ime.txt',['shell','dumpsys','input_method']);text('resumed.txt',['shell','dumpsys','activity','activities']);
 console.log(JSON.stringify({source,pid,frames,offlineOnly:true}));
}finally{try{text('stop-after.txt',['shell','am','force-stop',app]);}finally{record();}}
