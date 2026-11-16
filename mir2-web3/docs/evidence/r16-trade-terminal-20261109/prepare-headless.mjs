import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const repo = resolve(process.argv[2]);
const root = dirname(fileURLToPath(import.meta.url));
const out = join(root, 'headless');
const archived = join(repo, 'docs/evidence/r16-social-budget-20261109/ui');
const baseline = '8cd766e53eede54e8c79c93fbb2ec860fb7fab90';
const fixed = execFileSync('git', ['rev-parse', 'HEAD'], { cwd: repo, encoding: 'utf8' }).trim();
assert.equal(fixed, 'ac6817dbf95ab1e5d1e98f78a9dd88b6885a8731');
const exe = join(root, 'fixed-gateway.exe');
const sha = createHash('sha256').update(await readFile(exe)).digest('hex');
const wireProof = JSON.parse(await readFile(join(root, 'fixed-default/proof.json'), 'utf8'));
assert.equal(wireProof.sourceHead, baseline);
assert.equal(wireProof.binarySha256, sha);
assert(wireProof.passed && wireProof.sameConnectionReopen);

function replaceOnce(source, pattern, replacement) {
  const matches = typeof pattern === 'string' ? source.split(pattern).length - 1 : [...source.matchAll(new RegExp(pattern.source, 'g'))].length;
  assert.equal(matches, 1, `Unexpected archived script shape for ${String(pattern)}`);
  return source.replace(pattern, replacement);
}

let launch = await readFile(join(archived, 'launch-isolated.mjs'), 'utf8');
launch = replaceOnce(launch, /,repo='[^']+';/, `,repo=${JSON.stringify(repo)};`);
launch = replaceOnce(launch, /const exe='[^']+',exeSha=sha\(await readFile\(exe\)\);assert\.equal\(exeSha,'[^']+'\);/, `const exe=join(root,'../fixed-gateway.exe'),exeSha=sha(await readFile(exe));assert.equal(exeSha,'${sha}');`);
launch = replaceOnce(launch,
  "const priv=join(root,'private'),data=join(oldPrivate,'data'),cwd=join(priv,'gateway-cwd');await mkdir(cwd,{recursive:true,mode:0o700});",
  "const priv=join(root,'private'),data=join(priv,'data'),cwd=join(priv,'gateway-cwd');await mkdir(cwd,{recursive:true,mode:0o700});\nawait mkdir(join(data,'save-recovery'),{recursive:true,mode:0o700});\nawait copyFile(join(oldPrivate,'data/accounts.json'),join(data,'accounts.json'));");
launch = replaceOnce(launch, "gatewayBuildBase:'c404bde8a5190534f6bc83ec8a4fe78fffc8cb50',currentGatewaySourceExact:false,reusedOwnFileStore:true", `gatewayBuildBase:'${baseline}',gatewaySourceCommit:'${fixed}',currentGatewaySourceExact:true,reusedOwnFileStore:false,clonedOwnFileStore:true`);
launch = launch.replaceAll('19481', '19581').replaceAll('19482', '19582').replaceAll('19483', '19583').replaceAll('.next-social-budget', '.next-trade-terminal');

let ui = await readFile(join(archived, 'social-ui.mjs'), 'utf8');
ui = replaceOnce(ui, /,repo='[^']+';/, `,repo=${JSON.stringify(repo)};`);
ui = replaceOnce(ui, "'--remote-debugging-port=19484','--remote-debugging-address=127.0.0.1',", '');
ui = replaceOnce(ui, / if\(!proof\.passed\)\{console\.log\('Owned headless instance retained; send close to release\.'\);[^\n]+\}\r?\n/, '');
ui = replaceOnce(ui, 'headlessStillOpen:!proof.passed', 'headlessStillOpen:false');
ui = replaceOnce(ui, "import readline from 'node:readline';", '');

await mkdir(out);
await writeFile(join(out, 'launch-isolated.mjs'), launch, { flag: 'wx' });
await writeFile(join(out, 'social-ui.mjs'), ui, { flag: 'wx' });
console.log(JSON.stringify({ gatewaySourceCommit: fixed, gatewaySha256: sha, fromArchivedUi: true, output: out }));
