import fs from 'node:fs/promises';
import path from 'node:path';
import crypto from 'node:crypto';
import {fileURLToPath} from 'node:url';

const here=path.dirname(fileURLToPath(import.meta.url));
const project=path.resolve(here,'../../..');
const sha=b=>crypto.createHash('sha256').update(b).digest('hex');
const sources=[
  'apps/simulation/src/sui_usdc_billing.rs','apps/simulation/src/sui_usdc_billing_tests.rs',
  'apps/simulation/src/billing.rs','apps/simulation/src/config.rs','apps/simulation/src/db_projection.rs',
  'apps/simulation/src/lib.rs','apps/simulation/src/runtime/billing.rs','apps/simulation/src/procurement.rs',
  'apps/simulation/src/procurement/tests.rs','infra/postgres/migrations/0019_sui_usdc_billing_authority.sql',
  'apps/gateway/src/sui_usdc_billing.rs','apps/gateway/src/sui_usdc_billing_tests.rs',
  'apps/gateway/src/web_sui_usdc.rs','apps/gateway/src/sui_usdc_checkout.html','apps/gateway/src/sui_usdc_checkout.js',
  'apps/gateway/src/web_billing.rs','apps/gateway/src/web.rs','apps/gateway/src/lib.rs','apps/gateway/src/main.rs',
  'apps/game-client/client-bevy/src/native_billing.rs','apps/game-client/client-bevy/src/native_i18n.rs',
  'apps/game-client/platform-windows/src/billing_browser.rs','apps/game-client/platform-windows/src/gateway.rs',
  'apps/game-client/platform-windows/src/main.rs','apps/game-client/platform-windows/src/native_protocol.rs',
  'apps/game-client/platform-windows/src/shell_bridge.rs',
  'onchain/usdc-recharge/package.json','onchain/usdc-recharge/package-lock.json',
  'onchain/usdc-recharge/service/cli.mjs','onchain/usdc-recharge/service/driver.mjs','onchain/usdc-recharge/service/model.mjs',
  'onchain/usdc-recharge/test/driver.test.mjs',
  'onchain/usdc-recharge/src/mir2_usdc_recharge/Move.toml','onchain/usdc-recharge/src/mir2_usdc_recharge/sources/recharge.move',
  'onchain/usdc-recharge/UPSTREAM.json','docs/SUI-USDC-RECHARGE-V1.zh-CN.md',
].sort();
async function walk(dir){let found=[];for(const e of await fs.readdir(dir,{withFileTypes:true})){const p=path.join(dir,e.name);if(e.isSymbolicLink())throw Error('symlink forbidden');if(e.isDirectory())found.push(...await walk(p));else found.push(p);}return found;}
const artifacts=[];
for(const file of (await walk(here)).sort()){
  const relative=path.relative(here,file).replaceAll('\\','/');
  if(relative==='manifest.json')continue;
  if(/(^|\/)(private[^/]*|node_modules|target|\.env[^/]*)(\/|$)/i.test(relative)||/\.(exe|dll|keystore|key|pem)$/i.test(relative))throw Error('forbidden artifact path: '+relative);
  const bytes=await fs.readFile(file);
  if(!/\.(png|jpg|jpeg)$/i.test(relative)){
    const content=bytes.toString('utf8');
    if(/suiprivkey1[0-9a-z]{15,}|-----BEGIN [^-]*PRIVATE KEY-----|Bearer usdc_[a-f0-9]{64}\.[A-Za-z0-9_-]{43}|\/pay#usdc_[a-f0-9]{64}\.[A-Za-z0-9_-]{43}/.test(content))throw Error('sensitive content in '+relative);
  }
  artifacts.push({path:relative,bytes:bytes.length,sha256:sha(bytes)});
}
const sourcePins=[];
for(const source of sources){const bytes=Buffer.from((await fs.readFile(path.join(project,source),'utf8')).replaceAll('\r\n','\n'),'utf8');sourcePins.push({path:source,normalization:'UTF-8 CRLF to LF',bytes:bytes.length,sha256:sha(bytes)});}
const manifest={formatVersion:1,artifactHashing:'raw bytes; this topic has scoped * -text',sourceHashing:'UTF-8 CRLF to LF',artifactCount:artifacts.length,artifactBytes:artifacts.reduce((n,a)=>n+a.bytes,0),artifacts,sourcePins};
if(process.argv.includes('--verify')){
  const existing=JSON.parse(await fs.readFile(path.join(here,'manifest.json'),'utf8'));
  if(JSON.stringify(existing)!==JSON.stringify(manifest))throw Error('manifest or source pins stale; regenerate after final changes');
  console.log(JSON.stringify({verified:true,artifacts:manifest.artifactCount,bytes:manifest.artifactBytes,sourcePins:sourcePins.length}));
}else{await fs.writeFile(path.join(here,'manifest.json'),JSON.stringify(manifest,null,2)+'\n');console.log(JSON.stringify({written:true,artifacts:manifest.artifactCount,bytes:manifest.artifactBytes,sourcePins:sourcePins.length}));}
