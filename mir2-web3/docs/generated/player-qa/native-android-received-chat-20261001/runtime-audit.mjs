// Integrity of the actual installed diagnostic captures, plus bounded manual QA.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {createHash} from 'node:crypto';
const [qa]=process.argv.slice(2);
assert(qa,'Provide exact v14 QA target');
const read=name=>fs.readFileSync(path.join(qa,name),'utf8');
const proof=JSON.parse(read('package-v14.json'));
assert.equal(proof.source,'253b6682f73715b896745ca85a6923be98d0b892');
assert.equal(read('device-api.txt').trim(),'31');
assert.match(read('device-density.txt'),/Physical density: 440/);
for(const variant of ['debug','preview']) {
  assert.match(read(`${variant}-install.log`),/Success/);
  assert.match(read(`${variant}-installed-version.txt`),/versionCode=14/);
  assert.match(read(`${variant}-installed-version.txt`),/versionName=0.1.11-received-chat/);
}
const captures=['debug-login','preview-chat','preview-chat-settings'].map(name=>{
  const bytes=fs.readFileSync(path.join(qa,`${name}.png`));
  assert.equal(bytes.subarray(0,8).toString('hex'),'89504e470d0a1a0a');
  assert.deepEqual([bytes.readUInt32BE(16),bytes.readUInt32BE(20)],[2340,1080]);
  assert.match(read(`${name}-resumed.txt`),new RegExp(`com.mir2.web3${name.startsWith('preview')?'\\.uipreview':''}/`));
  return {name:`${name}.png`,width:2340,height:1080,bytes:bytes.length,
    sha256:createHash('sha256').update(bytes).digest('hex')};
});
const logs=fs.readdirSync(qa).filter(name=>/^(debug-login|preview-chat|preview-chat-settings)-pid\d+\.log$/.test(name)).map(name=>{
  const text=read(name),lines=text.split('\n');
  if(name.startsWith('preview')) {
    assert(text.includes('ANDROID_OFFLINE_RECEIVED_CHAT_QUEUED_NOT_LIVE'));
    assert.match(text,/queued = 2/);
  }
  const errors=lines.filter(line=>/\sE\s|ERROR\b|panicked|FATAL EXCEPTION|Fatal signal|SIGSEGV|SIGABRT/.test(line)).length;
  assert.equal(errors,0,name);
  return {name,capturedLineEntries:lines.length,errorFatalPanicSignalEntries:errors,
    warningsNotErased:true};
});
assert.equal(logs.length,3);
const starts=['start-debug.log','start-preview-chat.log','start-preview-chat-settings.log'].map(name=>{
  const text=read(name),status=text.match(/Status: (\S+)/)?.[1];
  assert.equal(status,'ok');
  return {name,status,waitTimeMs:Number(text.match(/WaitTime: (\d+)/)?.[1])};
});
const result={source:proof.source,versionCode:14,deviceApi:31,density:440,
  fingerprint:read('device-fingerprint.txt').trim(),captures,logs,starts,
  observationMethod:'Manually inspected original SDK pixel captures of diagnostic Intent scenes; no new taps/filter/settings/apply/typing/send/drag/soak claim.',
  observed:{ordinaryEmptyLoginFailClosed:true,offlineSystemChatVisible:true,
    offlinePeerChatVisible:false,actualSoftKeyboardVisible:true,
    offlineChatSettingsOriginalFrameVisible:true,fullReceivedChatVisualAccepted:false,
    fullPhoneUiAccepted:false,startupStabilityAccepted:false,authenticatedGameplayAccepted:false,physicalDeviceAccepted:false},
  retainedFailure:'Actual focused chat shows OFFLINE system chat but clips OFFLINE neighbor: hello. Phone chat focused height includes rows+2*48+12, but row placement needs at least rows+2*48+16: newest row exceeds lines_bottom by4 logical pixels. This source geometry diagnosis still needs a failure-first phone regression and a new exact APK/screenshot before it is accepted as fixed.',
  sharedRuntimeChatEvictionUnchangedAndOpen:true,
  priorV13TimeoutAndEmulatorHostCrashNotErased:true};
fs.writeFileSync(path.join(qa,'runtime-v14.json'),JSON.stringify(result,null,2)+'\n');
console.log(JSON.stringify(result,null,2));
