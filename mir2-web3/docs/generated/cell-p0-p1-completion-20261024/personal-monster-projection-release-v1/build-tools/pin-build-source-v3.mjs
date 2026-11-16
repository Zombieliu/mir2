import fs from 'node:fs/promises';
import { options, captureSource, MANIFEST, pin, absent } from './source-lib-v3.mjs';
const selected = options(); await absent([MANIFEST]);
const current = await captureSource(selected);
const manifest = { schemaVersion: 1, capturedAt: new Date().toISOString(), ...current,
  scope: 'Fresh personal/shared-monster projection backend candidate: preserved468/41 certificate plus only routing.rs, the existing personal-monster-projection, live-AOI-order and pending-zone-AOI tests, and the readonly Zone ownership API delta. Forty-four explicit inputs, all39 other original raw/Git inputs unchanged, the original four-None fixture inverse retained. No build or gameplay acceptance is implied.' };
await fs.writeFile(MANIFEST, JSON.stringify(manifest, null, 2) + '\n', { flag: 'wx' });
console.log(JSON.stringify({ manifest: await pin(MANIFEST), sourceCommit: current.sourceCommit, files: current.files.length, sourceManifestSha256: current.sourceManifestSha256 }));
