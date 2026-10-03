import assert from 'node:assert/strict';
import {execFileSync} from 'node:child_process';
import {readFileSync,writeFileSync,mkdirSync} from 'node:fs';
import {createHash} from 'node:crypto';
import {dirname,join} from 'node:path';
import {fileURLToPath} from 'node:url';
const qa=dirname(fileURLToPath(import.meta.url)), out=join(qa,'device');
mkdirSync(out,{recursive:true});
const adb='/Users/henryliu/Library/Android/sdk/platform-tools/adb',serial='emulator-5554';
const expected='7b62eda33cbf6c10a41c22516f1e6400dfe92202';
const proof=JSON.parse(readFileSync(join(qa,'package-v20.json')));
assert.equal(proof.source,expected);
const commands=[];
const record=()=>writeFileSync(join(out,'commands.json'),JSON.stringify({source:expected,versionCode:20,
 offlineOnly:true,actualJavaJniQuestAccepted:false,physicalDeviceAccepted:false,commands},null,2)+'\n');
function run(name,args,timeout=30000){
 const entry={name,args,started:new Date().toISOString()};
 commands.push(entry);
 try{
  const bytes=execFileSync(adb,['-s',serial,...args],{timeout,maxBuffer:20*1024*1024});
  writeFileSync(join(out,name),bytes,{flag:'wx'});
  entry.status=0;
  return bytes;
 }catch(error){
  entry.status=error.status;
  entry.error=error.message;
  writeFileSync(join(out,name+'.failed-stdout'),error.stdout??Buffer.alloc(0),{flag:'wx'});
  writeFileSync(join(out,name+'.failed-stderr'),error.stderr??Buffer.alloc(0),{flag:'wx'});
  throw error;
 }finally{entry.finished=new Date().toISOString();record();}
}
const textRun=(name,args,timeout)=>run(name,args,timeout).toString();
const delay=ms=>new Promise(resolve=>setTimeout(resolve,ms));
try{
 assert.match(textRun('avd.txt',['emu','avd','name']),/^Mir2_API_31_ARM64/);
 textRun('os.txt',['shell','getprop','ro.build.fingerprint']);
 assert.equal(textRun('api.txt',['shell','getprop','ro.build.version.sdk']).trim(),'31');
 textRun('display.txt',['shell','wm','size']);
 textRun('density.txt',['shell','wm','density']);
 for(const artifact of proof.artifacts){
  assert.equal(createHash('sha256').update(readFileSync(artifact.path)).digest('hex'),artifact.sha256);
  const app=artifact.variant==='debug'?'com.mir2.web3':'com.mir2.web3.uipreview';
  textRun(artifact.variant+'-stop-before.txt',['shell','am','force-stop',app]);
  assert.match(textRun(artifact.variant+'-install.txt',['install','-r',artifact.path],60000),/Success/);
  assert.match(textRun(artifact.variant+'-package.txt',['shell','dumpsys','package',app]),/versionCode=20\b/);
  const installed=textRun(artifact.variant+'-installed-path.txt',['shell','pm','path',app]).trim();
  assert.match(installed,/^package:\/data\/app\/[A-Za-z0-9_+~.\-=/]+\/base\.apk$/);
  const digest=textRun(artifact.variant+'-installed-sha.txt',['shell','sha256sum',installed.slice('package:'.length)]);
  assert.equal(digest.split(/\s+/)[0],artifact.sha256);
 }
 for(const [name,app,scene] of [
  ['debug-login','com.mir2.web3',null],
  ['quests-ingress','com.mir2.web3.uipreview','quests-ingress'],
  ['npc-ingress','com.mir2.web3.uipreview','npc-ingress'],
 ]){
  textRun(name+'-stop.txt',['shell','am','force-stop',app]);
  const args=['shell','am','start','-W','-n',app+'/com.mir2.web3.MainActivity'];
  if(scene)args.push('--es','ui_scene',scene);
  assert.match(textRun(name+'-start.txt',args),/Status: ok/);
  await delay(4000);
  const pid=textRun(name+'-pid.txt',['shell','pidof',app]).trim();
  assert.match(pid,/^\d+$/);
  const image=run(name+'.png',['exec-out','screencap','-p']);
  assert.equal(image.subarray(0,8).toString('hex'),'89504e470d0a1a0a');
  assert.equal(image.readUInt32BE(16),2340);
  assert.equal(image.readUInt32BE(20),1080);
  const log=textRun(name+'-pid.log',['logcat','-d','--pid='+pid,'-v','threadtime']);
  if(scene){
   assert(log.includes('ANDROID_UI_PREVIEW_READY scene='+scene),'Missing actual scene marker');
   assert(log.includes('ANDROID_QUEST_PREVIEW_RECEIVED_MODELS_APPLIED_NOT_LIVE'),'Missing received-model producer');
   assert(!log.includes('ANDROID_QUEST_PREVIEW_RECEIVED_MODELS_FAILED_NOT_LIVE'));
  }
  textRun(name+'-ime.txt',['shell','dumpsys','input_method']);
  textRun(name+'-resumed.txt',['shell','dumpsys','activity','activities']);
  textRun(name+'-stop-after.txt',['shell','am','force-stop',app]);
  console.log(JSON.stringify({name,pid,scene,source:expected,originalFrame:'2340x1080',live:false}));
 }
}finally{
 for(const [name,app] of [['debug','com.mir2.web3'],['preview','com.mir2.web3.uipreview']]){
  try{textRun(name+'-final-stop.txt',['shell','am','force-stop',app]);}catch(error){console.error(error.message);}
 }
 record();
}
