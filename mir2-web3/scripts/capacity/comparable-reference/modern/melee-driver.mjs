// Explicit opt-in entrypoint; the ordinary dense CLI never enables combat.
import fs from 'node:fs/promises';
import { comparisonMain } from './driver.mjs';
import { createMeleeEngine } from './combat.mjs';

comparisonMain({ extraOptions: ['--combat-contract'], engineFactory: async options =>
  createMeleeEngine(await fs.readFile(options['--combat-contract']))
}).catch(error => { console.error(JSON.stringify({ ok: false, error: error.message })); process.exitCode = 1; });
