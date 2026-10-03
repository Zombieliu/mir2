import assert from 'node:assert/strict';
import {execFileSync} from 'node:child_process';
import {readFileSync,writeFileSync,mkdirSync} from 'node:fs';
import {createHash} from 'node:crypto';
import {dirname,join} from 'node:path';
import {fileURLToPath} from 'node:url';

const taskDir=dirname(fileURLToPath(import.meta.url));
const [phase,proofPath]=process.argv.slice(2);
assert.match(phase??'',/^(baseline|probe|fixed)(-[a-z0-9]+)?$/);
const proof=JSON.parse(readFileSync(proofPath));
const out=join(taskDir,phase);mkdirSync(out,{recursive:true});
const adb='/Users/henryliu/Library/Android/sdk/platform-tools/adb';
const commands=[],scenes=[];
const report={installedSource:proof.source,versionCode:proof.versionCode,
  actualDevice:'emulator-5554',offlineOnly:true,realNetworkAccepted:false,
  physicalDeviceAccepted:false,commands,scenes};
const save=()=>writeFileSync(join(out,'commands.json'),JSON.stringify(report,null,2)+'\n');
function run(name,args,timeout=30000){
 const row={name,args,started:new Date().toISOString()};commands.push(row);
 try{
  const bytes=execFileSync(adb,['-s','emulator-5554',...args],{timeout,maxBuffer:32*1024*1024});
  writeFileSync(join(out,name),bytes,{flag:'wx'});row.exitCode=0;return bytes;
 }catch(error){
  row.exitCode=error.status;row.error=error.message;
  writeFileSync(join(out,name+'.failed-stdout'),error.stdout??Buffer.alloc(0),{flag:'wx'});
  writeFileSync(join(out,name+'.failed-stderr'),error.stderr??Buffer.alloc(0),{flag:'wx'});throw error;
 }finally{row.finished=new Date().toISOString();save();}
}
const text=(name,args)=>run(name,args).toString();
const delay=ms=>new Promise(resolve=>setTimeout(resolve,ms));
function png(name){
 const bytes=run(name+'.png',['exec-out','screencap','-p']);
 assert.equal(bytes.subarray(0,8).toString('hex'),'89504e470d0a1a0a');
 assert.equal(bytes.readUInt32BE(16),2340);assert.equal(bytes.readUInt32BE(20),1080);
}
function binding(artifact,suffix){
 const app=artifact.variant==='debug'?'com.mir2.web3':'com.mir2.web3.uipreview';
 assert.equal(createHash('sha256').update(readFileSync(artifact.path)).digest('hex'),artifact.sha256);
 const path=text(artifact.variant+'-installed-path-'+suffix+'.txt',['shell','pm','path',app]).trim().replace(/^package:/,'');
 assert.match(path,/^\/data\/app\/[A-Za-z0-9_+~.\-=/]+\/base\.apk$/);
 const sha=text(artifact.variant+'-installed-sha-'+suffix+'.txt',['shell','sha256sum',path]).trim().split(/\s+/)[0];
 assert.equal(sha,artifact.sha256);
 const pkg=text(artifact.variant+'-package-'+suffix+'.txt',['shell','dumpsys','package',app]);
 assert.match(pkg,new RegExp('versionCode='+proof.versionCode+'\\b'));
}
function totals(log){
 return {gl506:(log.match(/E emuglGLESv2_enc:.*GL error 0x506/g)??[]).length,
  uninitializedColor:(log.match(/D eglCodecCommon:.*rbo not color renderable\. format: 0x0/g)??[]).length,
  fatalOrPanic:(log.match(/FATAL EXCEPTION|thread '.*' panicked|Fatal signal/g)??[]).length};
}
try{
 assert.match(text('avd.txt',['emu','avd','name']),/^Mir2_API_31_ARM64/);
 assert.equal(text('api.txt',['shell','getprop','ro.build.version.sdk']).trim(),'31');
 text('os.txt',['shell','getprop','ro.build.fingerprint']);
 text('display.txt',['shell','wm','size']);text('density.txt',['shell','wm','density']);
 for(const artifact of proof.artifacts)binding(artifact,'before');
 const cases=[['login','com.mir2.web3',null],['world','com.mir2.web3.uipreview','world-render'],
  ['quests','com.mir2.web3.uipreview','quests-ingress']];
 for(const [name,app,scene] of cases){
  if(!proof.artifacts.some(a=>(a.variant==='debug'?'com.mir2.web3':'com.mir2.web3.uipreview')===app))continue;
  text(name+'-stop-before.txt',['shell','am','force-stop',app]);
  const launch=['shell','am','start','-W','-n',app+'/com.mir2.web3.MainActivity'];
  if(scene)launch.push('--es','ui_scene',scene);
  assert.match(text(name+'-start.txt',launch),/Status: ok/);
  const pid=text(name+'-pid.txt',['shell','pidof',app]).trim();assert.match(pid,/^\d+$/);
  await delay(4000);png(name+'-cold');
  const cold=text(name+'-cold-pid.log',['logcat','-d','--pid='+pid,'-v','threadtime']);
  text(name+'-home.txt',['shell','input','keyevent','KEYCODE_HOME']);await delay(1000);
  assert.match(text(name+'-resume.txt',launch),/Status: ok/);await delay(4000);
  assert.equal(text(name+'-resume-pid.txt',['shell','pidof',app]).trim(),pid);
  png(name+'-resumed');
  const complete=text(name+'-latest-pid.log',['logcat','-d','--pid='+pid,'-v','threadtime']);
  const row={name,scene,pid,cold:totals(cold),latestCumulative:totals(complete),
   originalFrames:'2340x1080',sameProcessResume:true};scenes.push(row);save();
  text(name+'-stop-after.txt',['shell','am','force-stop',app]);console.log(JSON.stringify(row));
 }
 for(const artifact of proof.artifacts)binding(artifact,'after');
 report.canonicalTotal=scenes.reduce((n,s)=>n+s.latestCumulative.gl506,0);
 report.zeroErrorAccepted=report.canonicalTotal===0;save();
}finally{
 for(const app of ['com.mir2.web3','com.mir2.web3.uipreview']){
  try{text(app+'-final-stop.txt',['shell','am','force-stop',app]);}catch(error){console.error(error.message);}
 }
 save();
}
