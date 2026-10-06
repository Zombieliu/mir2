// Lexical extraction check: only visibility, namespace and formatting may differ.
import fs from 'node:fs';
import { execFileSync } from 'node:child_process';
import assert from 'node:assert/strict';
const qa = process.argv[2];
assert(qa, 'provide exact evidence output directory');
const source = '3f5e61533235921369bc13a7760b4a56b0e467e5';
const file = 'mir2-web3/apps/game-client/platform-windows/src/gateway.rs';
const old = execFileSync('git', ['show', `${source}:${file}`], { encoding:'utf8', maxBuffer:4*1024*1024 });
const shared = fs.readFileSync('mir2-web3/apps/game-client/client-bevy/src/native_player_ingress.rs', 'utf8');
const host = fs.readFileSync(file, 'utf8');
const normalize = text => text.replace(/\bpub /g,'').replaceAll('crate::read_model::','mir2_client_bevy::read_model::').replace(/\s+/g,'').replace(/,([\)\]\}])/g,'$1');
const begin = text => text.indexOf('#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]');
const blockEnd = old.indexOf('\nfn crystal_tooltip_viewer(', begin(old));
const sharedEnd = shared.indexOf('\npub fn update_wallet_from_snapshot(', begin(shared));
assert.equal(normalize(old.slice(begin(old),blockEnd)), normalize(shared.slice(begin(shared),sharedEnd)));
const names = ['update_wallet_from_snapshot','merge_wallet_into_world','merge_wallet_into_payload',
  'apply_wallet_delta','wallet_value','value_u32','value_i32','value_i64','value_string'];
function body(text,name) {
  const start=text.indexOf(`fn ${name}(`); assert(start>=0,name);
  const begin=text.indexOf('{',start); let end=begin+1,depth=1;
  for(;end<text.length&&depth;end++){if(text[end]==='{')depth++;if(text[end]==='}')depth--;}
  assert.equal(depth,0); return text.slice(start,end);
}
for (const name of names) assert.equal(normalize(body(old,name)),normalize(body(shared,name)),name);
assert(!host.includes('struct NativeUiPlayerCursor'));
assert(!host.includes('fn apply_wallet_delta('));
assert(host.includes('use mir2_client_bevy::native_player_ingress::{'));
const result={source, cursorAndWalletFields:true, fiveCursorMethods:true,
  helperBodies:names, permittedDifferences:['visibility','read_model namespace','formatting including trailing commas'],
  windowsHostUsesSharedImplementation:true, windowsDeviceAccepted:false, authenticatedAndroidAccepted:false};
fs.writeFileSync(`${qa}/source-equivalence.json`,JSON.stringify(result,null,2)+'\n');
console.log(JSON.stringify(result,null,2));
