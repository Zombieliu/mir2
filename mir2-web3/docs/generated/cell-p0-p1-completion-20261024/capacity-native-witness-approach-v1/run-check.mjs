import fs from 'node:fs/promises';
import path from 'node:path';
import crypto from 'node:crypto';
import { spawn } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const directory = path.dirname(fileURLToPath(import.meta.url));
const project = 'C:/Users/Administrator/.codex/worktrees/cell-preproduction-p0-p1/mir2-player-journey/mir2-web3';
const [name, mode] = process.argv.slice(2);
if (!/^(?:focused|adjacent|full)-v[1-9][0-9]*$/.test(name ?? '') || !['focused', 'adjacent', 'full'].includes(mode)) throw Error('Explicit bounded run name and mode required');
const baseline = JSON.parse(await fs.readFile(path.join(directory, 'prior-regression/final-driver-frozen-v1.json'), 'utf8'));
const sha = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const pin = async file => { const bytes = await fs.readFile(file); return { file, bytes: bytes.length, sha256: sha(bytes) }; };
const sourcePins = async () => Promise.all(baseline.files.map(async ({ file }) => ({ ...await pin(path.join(project, file)), file })));
const before = await sourcePins(), sourceSha256 = sha(JSON.stringify(before));
const snapshotDirectory = path.join(directory, name + '-source-preimage'); await fs.mkdir(snapshotDirectory);
for (const { file } of before) await fs.writeFile(path.join(snapshotDirectory, path.basename(file) + '.txt'), await fs.readFile(path.join(project, file)), { flag: 'wx' });
const sourceManifestFile = path.join(directory, name + '-source-pins.json');
await fs.writeFile(sourceManifestFile, JSON.stringify({ schemaVersion: 1, sourceSha256, files: before }, null, 2) + '\n', { flag: 'wx' });
const focused = '^native witnessed approach';
const adjacent = '^native hunt|^native scout|^temporary observer|^active observer|^navigation|^native fast|^native primed|^native fallback|^pending attack|^planned disconnect|^drain includes|^fanout latency|^real hit|^shared combat|^V2 admission|^attackResultV2|^native timing|^native attack pacing|^native calendar|^native evidence|^native combat walks|^native pending evidence|^native post-strike|^native attack witness|^native initial no-witness|^native HP|^native witnessed approach';
const args = ['--test', ...(mode === 'full' ? [] : ['--test-name-pattern=' + (mode === 'focused' ? focused : adjacent)]),
  'apps/web/scripts/playtest-capacity-soak.test.mjs', ...(mode === 'full' ? ['apps/web/scripts/playtest-capacity-observability.test.mjs', 'apps/web/scripts/playtest-capacity-monitor.test.mjs', 'apps/web/scripts/playtest-capacity-telemetry.test.mjs'] : [])];
const logFile = path.join(directory, name + '.log'), log = await fs.open(logFile, 'wx'), startedAtMs = Date.now();
const child = spawn(process.execPath, args, { cwd: project, stdio: ['ignore', 'pipe', 'pipe'] });
let write = Promise.resolve(), error = null;
const append = bytes => { write = write.then(() => log.writeFile(bytes)); };
child.stdout.on('data', append); child.stderr.on('data', append); child.on('error', value => { error = value.message; });
const result = await new Promise(resolve => child.on('close', (exitCode, signal) => resolve({ exitCode, signal })));
await write; await log.close();
const after = await sourcePins(), sourceSha256After = sha(JSON.stringify(after)), sourcePinsUnchangedAfterRun = JSON.stringify(before) === JSON.stringify(after);
const output = await fs.readFile(logFile, 'utf8'), counts = Object.fromEntries(['tests', 'pass', 'fail', 'cancelled', 'skipped'].map(key => [key, Number(output.match(new RegExp('^# ' + key + ' (\\d+)\\r?$', 'm'))?.[1] ?? NaN)]));
const receipt = { schemaVersion: 1, args, node: process.version, startedAtMs, elapsedMs: Date.now() - startedAtMs, ...result, error,
  sourceSha256, sourceSha256After, sourcePinsUnchangedAfterRun, sourcePins: before.filter(p => /\/playtest-capacity-soak(?:\.test)?\.mjs$/.test(p.file)), exactSourceManifest: await pin(sourceManifestFile), log: await pin(logFile), counts };
const receiptFile = path.join(directory, name + '-receipt.json'); await fs.writeFile(receiptFile, JSON.stringify(receipt, null, 2) + '\n', { flag: 'wx' });
console.log(JSON.stringify({ ...await pin(receiptFile), sourceSha256, sourcePinsUnchangedAfterRun, ...result, counts }));
process.exitCode = result.exitCode || error || !sourcePinsUnchangedAfterRun ? 1 : 0;
