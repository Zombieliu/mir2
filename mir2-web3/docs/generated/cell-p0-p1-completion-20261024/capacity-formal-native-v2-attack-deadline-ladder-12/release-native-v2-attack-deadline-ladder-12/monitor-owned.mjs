import {assertFormal12Configured} from './formal-template-contract.mjs';
assertFormal12Configured();
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import { root, project, read, assertPin } from './input-lib.mjs';
if (process.argv.slice(2).join(' ') !== '--owned-monitor') throw new Error('Only the owned launcher may start this monitor');
const pins = await read(path.join(root, 'source-pins.json'));
for (const entry of pins.files) await assertPin(entry);
const options = await read(path.join(root, 'monitor-options.json'));
let requested = false;
process.on('message', value => { if (value?.type === 'stop-owned-monitor' && Object.keys(value).length === 1) requested = true; });
const relay = setInterval(() => { if (requested && process.listenerCount('SIGTERM')) { requested = false; process.emit('SIGTERM'); } }, 100);
try {
  const { runMonitor } = await import(pathToFileURL(path.join(project, 'apps/web/scripts/playtest-capacity-monitor.mjs')));
  const result = await runMonitor(options);
  console.log(JSON.stringify({ phase: 'owned-monitor-finished', ...result }));
} finally { clearInterval(relay); if (process.connected) process.disconnect(); }
