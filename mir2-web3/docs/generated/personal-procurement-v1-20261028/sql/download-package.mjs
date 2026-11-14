// Bounded range download of the same official, exact package. No install scripts.
import { spawn } from 'node:child_process';
import { readFile, writeFile, stat } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
const root = dirname(fileURLToPath(import.meta.url));
const url = 'https://registry.npmjs.org/@electric-sql/pglite/-/pglite-0.5.13.tgz';
const length = 9320802; // Official CDN Content-Range: bytes 0-0/9320802.
const integrity = 'sha512-4rZuxK1k07w6sSpJzjMri0B9+HuXuC7V9pxVB51pR0T9KPqXg6TD0mdaEwBA9rkcL4jdht4tGDf/wYzAiGhT7g==';
const parts = 32;
let next = 0;
async function one(index) {
  const start = Math.floor(length * index / parts), end = Math.floor(length * (index + 1) / parts) - 1;
  const path = join(root, `package-range-${String(index).padStart(2, '0')}.bin`);
  let prefix; try { prefix = await readFile(path); } catch { prefix = Buffer.alloc(0); }
  if (prefix.length > end - start + 1) throw Error(`range ${index} oversized existing part`);
  for (let attempt = 0; prefix.length < end - start + 1 && attempt < 12; attempt++) {
    const tail = `${path}.tail-${attempt}-${Date.now()}`;
    const code = await new Promise((ok, bad) => {
      const child = spawn('curl.exe', ['--location', '--fail', '--silent', '--show-error', '--max-time', '20',
        '--retry', '0', '--range', `${start + prefix.length}-${end}`, '--output', tail, url], { windowsHide: true, stdio: ['ignore', 'ignore', 'ignore'] });
      child.on('error', bad); child.on('close', ok);
    });
    if (![0, 28].includes(code)) throw Error(`range ${index}: exit ${code}`);
    let received; try { received = await readFile(tail); } catch { received = Buffer.alloc(0); }
    if (prefix.length + received.length > end - start + 1) throw Error(`range ${index} returned too many bytes`);
    prefix = Buffer.concat([prefix, received]);
    await writeFile(path, prefix);
  }
  if ((await stat(path)).size !== end - start + 1) throw Error(`range ${index} length mismatch`);
  console.log(`range ${index + 1}/${parts} verified`);
}
await Promise.all(Array.from({ length: 16 }, async () => { while (next < parts) await one(next++); }));
const chunks = await Promise.all(Array.from({ length: parts }, (_, index) => readFile(join(root, `package-range-${String(index).padStart(2, '0')}.bin`))));
const bytes = Buffer.concat(chunks);
if (bytes.length !== length || `sha512-${createHash('sha512').update(bytes).digest('base64')}` !== integrity) throw Error('Official npm integrity mismatch');
await writeFile(join(root, 'pglite-0.5.13-verified.tgz'), bytes);
await writeFile(join(root, 'download-integrity.json'), `${JSON.stringify({ url, length, integrity, verified: true, version: '0.5.13' }, null, 2)}\n`);
console.log('Exact official package integrity verified');
