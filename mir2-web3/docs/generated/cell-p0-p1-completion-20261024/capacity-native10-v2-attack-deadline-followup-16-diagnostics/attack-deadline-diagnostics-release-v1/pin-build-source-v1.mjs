import fs from 'node:fs/promises';
import { options, captureSource, MANIFEST, pin, absent } from './source-lib-v1.mjs';
const selected = options(); await absent([MANIFEST]);
const current = await captureSource(selected);
const manifest = { schemaVersion: 1, capturedAt: new Date().toISOString(), ...current,
  scope: 'Fresh attack deadline diagnostic backend candidate: exact direct5c2/44 baseline with only web.rs changed; all other43 raw/GitLF source entries and original four-None fixture proofs retained. New Node9 and actual8Rust/fmt/210Node receipts are separately bound. No build, lifecycle or capacity acceptance is implied.' };
await fs.writeFile(MANIFEST, JSON.stringify(manifest, null, 2) + '\n', { flag: 'wx' });
console.log(JSON.stringify({ manifest: await pin(MANIFEST), sourceCommit: current.sourceCommit, files: current.files.length, sourceManifestSha256: current.sourceManifestSha256 }));
