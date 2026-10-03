// Frozen-source extraction evidence, not Windows-device or live-chat acceptance.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {execFileSync} from 'node:child_process';
const [qa] = process.argv.slice(2);
assert(qa, 'Provide the exact evidence directory');
const frozen = '3f5e61533235921369bc13a7760b4a56b0e467e5';
const hostPath = 'mir2-web3/apps/game-client/platform-windows/src/gateway.rs';
const sharedPath = 'mir2-web3/apps/game-client/client-bevy/src/native_chat_ingress.rs';
const frozenHost = execFileSync('git', ['show', `${frozen}:${hostPath}`], {encoding:'utf8',maxBuffer:4*1024*1024});
const currentHost = fs.readFileSync(hostPath, 'utf8');
const shared = fs.readFileSync(sharedPath, 'utf8');
function functionBody(text) {
  const start = text.indexOf('fn transform_chat_line(');
  assert(start >= 0);
  const begin = text.indexOf('{',start);
  let end = begin + 1, depth = 1;
  for (;end < text.length && depth;end++) {
    if (text[end] === '{') depth++;
    if (text[end] === '}') depth--;
  }
  assert.equal(depth,0);
  return text.slice(start,end);
}
const normalize = text => text.replaceAll('crate::chat::','mir2_client_bevy::chat::')
  .replace(/\s+/g,'').replace(/,([\)\]\}])/g,'$1');
assert.equal(normalize(functionBody(frozenHost)),normalize(functionBody(shared)));
assert(!currentHost.includes('fn transform_chat_line('));
assert(currentHost.includes('use mir2_client_bevy::native_chat_ingress::transform_chat_line;'));
assert(currentHost.includes('transform_chat_line(packet, payload)'));
const result = {frozenWindows:frozen,function:'transform_chat_line',bodyAndSignatureEquivalent:true,
  permittedDifferences:['public visibility','chat type namespace','formatting'],
  windowsHostUsesSharedImplementation:true,windowsDeviceAccepted:false,authenticatedAndroidAccepted:false};
fs.writeFileSync(path.join(qa,'source-equivalence.json'),JSON.stringify(result,null,2)+'\n');
console.log(JSON.stringify(result,null,2));
