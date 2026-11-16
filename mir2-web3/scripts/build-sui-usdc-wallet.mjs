import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';
import { writeFile, readFile } from 'node:fs/promises';
import { createHash } from 'node:crypto';

const root = new URL('../apps/gateway/', import.meta.url);
const require = createRequire(new URL('package.json', root));
const { build } = require('esbuild');
const entry = fileURLToPath(new URL('src/sui_usdc_dubhe_wallet_entry.js', root));
const output = fileURLToPath(new URL('src/sui_usdc_dubhe_wallet.bundle.js', root));
const built = await build({ entryPoints: [entry], outfile: output, bundle: true, minify: true,
  platform: 'browser', target: ['es2022'], format: 'iife', legalComments: 'eof',
  define: { 'process.env.NODE_ENV': '"production"' }, metafile: true });
const generated = await readFile(output, 'utf8');
const legalOffset = generated.lastIndexOf('\n/*! Bundled license information:');
// Preserve executable code byte-for-byte; normalize only the generated license
// comment so repository whitespace checks also pass for vendored notices.
const normalized = legalOffset < 0 ? generated : generated.slice(0, legalOffset) + generated.slice(legalOffset).replace(/[\t ]+$/gm, '');
await writeFile(output, normalized);
const bytes = Buffer.from(normalized);
const receipt = { schema: 'mir2.checkout-wallet-bundle.v1',
  bundleSha256: createHash('sha256').update(bytes).digest('hex'), bytes: bytes.length,
  lockSha256: createHash('sha256').update(await readFile(new URL('package-lock.json', root))).digest('hex'),
  inputs: Object.keys(built.metafile.inputs).length, externalImports: Object.values(built.metafile.outputs).flatMap(o => o.imports.filter(i => i.external)) };
if (receipt.externalImports.length) throw Error('Checkout wallet bundle must be self-contained');
await writeFile(new URL('src/sui_usdc_dubhe_wallet.bundle.json', root), JSON.stringify(receipt, null, 2) + '\n');
console.log(JSON.stringify(receipt));
