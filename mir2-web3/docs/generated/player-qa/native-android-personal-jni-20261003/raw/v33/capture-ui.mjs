import assert from 'node:assert/strict';
import {execFileSync} from 'node:child_process';
import {readFileSync,writeFileSync,mkdirSync,existsSync} from 'node:fs';
import {dirname,join} from 'node:path';
import {fileURLToPath} from 'node:url';
const root=dirname(fileURLToPath(import.meta.url));
const [mode,scene,...params]=process.argv.slice(2);
assert(['cold','tap','swipe','home','back','stop','sample'].includes(mode));
assert(['gameshop-jni','storage-jni','storage-locked-jni','normal-intent-isolation'].includes(scene));
const dir=join(root,'ui-'+scene);mkdirSync(dir,{recursive:true});
const proof=JSON.parse(readFileSync(join(root,'package-baseline.json')));
const normal=scene==='normal-intent-isolation',artifact=proof.artifacts.find(a=>a.variant===(normal?'debug':'preview'));
assert.equal(proof.versionCode,33);
const file=join(dir,'commands.json');
const report=existsSync(file)?JSON.parse(readFileSync(file)):{source:proof.source,versionCode:33,scene,actualDevice:'emulator-5554',offlineOnly:true,commands:[],realAuthenticationAccepted:false,realNetworkAccepted:false,physicalDeviceAccepted:false};
assert.equal(report.source,proof.source);
const adb='/Users/henryliu/Library/Android/sdk/platform-tools/adb',app='com.mir2.web3'+(normal?'':'.uipreview');
const save=()=>writeFileSync(file,JSON.stringify(report,null,2)+'\n');
function run(name,args){
 const row={name,args,started:new Date().toISOString()};report.commands.push(row);
 try{const bytes=execFileSync(adb,['-s','emulator-5554',...args],{timeout:30000,maxBuffer:32*1024*1024});
  writeFileSync(join(dir,name),bytes,{flag:'wx'});row.exitCode=0;return bytes;
 }catch(error){row.exitCode=error.status;row.error=error.message;throw error;}
 finally{row.finished=new Date().toISOString();save();}
}
const text=(name,args)=>run(name,args).toString();
const delay=ms=>new Promise(resolve=>setTimeout(resolve,ms));
const suffix=report.commands.length+'-'+mode;
const path=text(suffix+'-installed-path.txt',['shell','pm','path',app]).trim().replace(/^package:/,'');
assert.equal(text(suffix+'-installed-sha.txt',['shell','sha256sum',path]).trim().split(/\s+/)[0],artifact.sha256);
assert.match(text(suffix+'-package.txt',['shell','dumpsys','package',app]),/versionCode=33\b/);
const launch=['shell','am','start','-W','-n',app+'/com.mir2.web3.MainActivity','--es','ui_scene',normal?'gameshop-jni':scene];
if(mode==='cold'){
 assert(!report.pid);
 text('stop-before.txt',['shell','am','force-stop',app]);
 assert.match(text('start.txt',launch),/Status: ok/);
 report.pid=text('pid.txt',['shell','pidof',app]).trim();assert.match(report.pid,/^\d+$/);save();
 await delay(normal?5000:14000);
}else{
 assert.equal(text(suffix+'-pid-before.txt',['shell','pidof',app]).trim(),report.pid);
 if(mode==='tap'){assert.equal(params.length,2);assert(params.every(p=>/^\d+$/.test(p)));text(suffix+'-action.txt',['shell','input','tap',...params]);await delay(1500);}
 if(mode==='swipe'){assert.equal(params.length,5);assert(params.every(p=>/^\d+$/.test(p)));text(suffix+'-action.txt',['shell','input','swipe',...params]);await delay(1500);}
 if(mode==='home'){text(suffix+'-home.txt',['shell','input','keyevent','KEYCODE_HOME']);await delay(1000);assert.match(text(suffix+'-resume.txt',launch),/Status: ok/);await delay(4000);}
 if(mode==='back'){text(suffix+'-back.txt',['shell','input','keyevent','KEYCODE_BACK']);await delay(1500);}
 if(mode==='stop'){text(suffix+'-stop.txt',['shell','am','force-stop',app]);console.log(JSON.stringify({stopped:scene,pid:report.pid}));process.exit(0);}
 assert.equal(text(suffix+'-pid-after.txt',['shell','pidof',app]).trim(),report.pid);
}
const png=run(suffix+'.png',['exec-out','screencap','-p']);
assert.equal(png.subarray(0,8).toString('hex'),'89504e470d0a1a0a');
report.actualScreenshotSize={width:png.readUInt32BE(16),height:png.readUInt32BE(20)};
const log=text(suffix+'-pid.log',['logcat','-d','--pid='+report.pid,'-v','threadtime']);
report.latestLog=suffix+'-pid.log';
report.latestTotals={gl506:(log.match(/E emuglGLESv2_enc:.*GL error 0x506/g)??[]).length,uninitializedColor:(log.match(/D eglCodecCommon:.*rbo not color renderable\. format: 0x0/g)??[]).length,fatalOrPanic:(log.match(/FATAL EXCEPTION|thread '.*' panicked|Fatal signal/g)??[]).length};
report.javaProducerSent=log.includes('PERSONAL_JNI_OFFLINE_SENT');
report.sharedConsumerObserved=log.includes('ANDROID_PERSONAL_JNI_SHARED_CONSUMER_NOT_LIVE');
if(normal){assert(!report.javaProducerSent&&!report.sharedConsumerObserved);assert(!log.includes('ANDROID_UI_PREVIEW_READY'));}
save();console.log(JSON.stringify({scene,mode,pid:report.pid,originalPng:join(dir,suffix+'.png'),size:report.actualScreenshotSize,javaSent:report.javaProducerSent,sharedConsumer:report.sharedConsumerObserved,totals:report.latestTotals}));
