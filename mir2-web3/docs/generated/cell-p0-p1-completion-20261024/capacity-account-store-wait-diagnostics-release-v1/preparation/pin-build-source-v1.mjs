import fs from 'node:fs/promises';
import { options, captureSource, MANIFEST, pin, absent } from './source-lib-v1.mjs';
const selected = options(); await absent([MANIFEST]);
const current = await captureSource(selected);
const manifest = { schemaVersion: 1, capturedAt: new Date().toISOString(), ...current,
  scope: 'Fresh account-store wait diagnostic backend candidate: exact directd240/44 baseline expands to50 inputs, with only oldlib.rs changed plus six explicitly named diagnostics paths. Other43 raw/GitLF entries and original four-None fixture proofs retained. New22Rust/fmt proof is separate from unchanged d240 Node9/210 historical evidence. No build, lifecycle or capacity acceptance is implied.' };
await fs.writeFile(MANIFEST, JSON.stringify(manifest, null, 2) + '\n', { flag: 'wx' });
console.log(JSON.stringify({ manifest: await pin(MANIFEST), sourceCommit: current.sourceCommit, files: current.files.length, sourceManifestSha256: current.sourceManifestSha256 }));
