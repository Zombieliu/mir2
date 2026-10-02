import assert from 'node:assert/strict';
import {execFileSync} from 'node:child_process';
import {readFileSync,writeFileSync,mkdirSync} from 'node:fs';
import {createHash} from 'node:crypto';
import {dirname,join} from 'node:path';
import {fileURLToPath} from 'node:url';
const qa=dirname(fileURLToPath(import.meta.url)),out=join(qa,'detail-device');
mkdirSync(out,{recursive:false});
const source=process.argv[2];assert.match(source??'',/^[a-f0-9]{40}$/);
const proof=JSON.parse(readFileSync(join(qa,'package-v23.json')));assert.equal(proof.source,source);
const artifact=proof.artifacts.find(a=>a.variant==='preview');
assert.equal(createHash('sha256').update(readFileSync(artifact.path)).digest('hex'),artifact.sha256);
const adb='/Users/henryliu/Library/Android/sdk/platform-tools/adb',serial='emulator-5554',app='com.mir2.web3.uipreview';
const commands=[],stages=[];
const record=()=>writeFileSync(join(out,'commands.json'),JSON.stringify({source,versionCode:23,apkSha256:artifact.sha256,
 offlineOnly:true,noTransactionButtonTapped:true,physicalDeviceAccepted:false,stages,commands},null,2)+'\n');
function run(name,args){
 const entry={name,args,started:new Date().toISOString()};commands.push(entry);
 try{const bytes=execFileSync(adb,['-s',serial,...args],{timeout:30000,maxBuffer:24*1024*1024});writeFileSync(join(out,name),bytes,{flag:'wx'});entry.status=0;return bytes;}
 catch(error){entry.status=error.status;writeFileSync(join(out,name+'.failed-stdout'),error.stdout??Buffer.alloc(0),{flag:'wx'});writeFileSync(join(out,name+'.failed-stderr'),error.stderr??Buffer.alloc(0),{flag:'wx'});throw error;}
 finally{entry.finished=new Date().toISOString();record();}
}
const text=(name,args)=>run(name,args).toString();
const delay=ms=>new Promise(resolve=>setTimeout(resolve,ms));
try{
 assert.match(text('avd.txt',['emu','avd','name']),/^Mir2_API_31_ARM64/);
 assert.equal(text('api.txt',['shell','getprop','ro.build.version.sdk']).trim(),'31');
 text('display.txt',['shell','wm','size']);text('density.txt',['shell','wm','density']);
 assert.match(text('package.txt',['shell','dumpsys','package',app]),/versionCode=23\b/);
 const installed=text('package-path.txt',['shell','pm','path',app]).trim().replace(/^package:/,'');
 assert.match(installed,/^\/data\/app\/[A-Za-z0-9_+~.\-=/]+\/base\.apk$/);
 assert.equal(text('installed-sha.txt',['shell','sha256sum',installed]).trim().split(/\s+/)[0],artifact.sha256);
 text('stop-before.txt',['shell','am','force-stop',app]);
 assert.match(text('start.txt',['shell','am','start','-W','-n',app+'/com.mir2.web3.MainActivity','--es','ui_scene','quests-ingress']),/Status: ok/);
 await delay(3000);
 const pid=text('pid.txt',['shell','pidof',app]).trim();assert.match(pid,/^\d+$/);
 for(const [name,tap] of [['diary',[0,0]],['pair',[1350,170]],['detail-only',[1197,909]]]){
  if(tap[0]){text(name+'-tap.txt',['shell','input','tap',String(tap[0]),String(tap[1])]);await delay(2000);}
  const frame=run(name+'.png',['exec-out','screencap','-p']);
  assert.equal(frame.subarray(0,8).toString('hex'),'89504e470d0a1a0a');
  assert.equal(frame.readUInt32BE(16),2340);assert.equal(frame.readUInt32BE(20),1080);
  const log=text(name+'-pid.log',['logcat','-d','--pid='+pid,'-v','threadtime']);
  assert(log.includes('ANDROID_QUEST_PREVIEW_RECEIVED_MODELS_APPLIED_NOT_LIVE'));
  stages.push({name,pid:Number(pid),tap:tap[0]?{x:tap[0],y:tap[1]}:null,originalFrame:name+'.png',log:name+'-pid.log'});record();
 }
 text('ime.txt',['shell','dumpsys','input_method']);text('resumed.txt',['shell','dumpsys','activity','activities']);
 console.log(JSON.stringify({source,pid,stages,live:false}));
}finally{try{text('stop-after.txt',['shell','am','force-stop',app]);}finally{record();}}
