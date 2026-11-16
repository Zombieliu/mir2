import fs from 'node:fs/promises';
import { options,captureSource,MANIFEST,pin,absent } from './source-lib.mjs';
const selected=options();
await absent([MANIFEST]);
const current=await captureSource(selected);
const manifest={schemaVersion:1,capturedAt:new Date().toISOString(),...current,
  scope:'Socket queue diagnostics candidate. Exact immutable-base-to-HEAD41 Rust/Cargo/.cargo inputs: original39 with only web.rs changed, unchanged38 other inputs, plus two existing fixture files containing exactly four None constructor arguments. Committed raw/Git-LF equivalence and ignored-inclusive untracked-input rejection remain strict. Node driver/content pinned separately. Build and lifecycle/capacity acceptance are not implied.'};
await fs.writeFile(MANIFEST,JSON.stringify(manifest,null,2)+'\n',{flag:'wx'});
console.log(JSON.stringify({manifest:await pin(MANIFEST),sourceCommit:current.sourceCommit,files:current.files.length,sourceManifestSha256:current.sourceManifestSha256}));
