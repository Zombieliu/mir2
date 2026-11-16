import fs from 'node:fs/promises';
import path from 'node:path';
import crypto from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
export const DIRECTORY = path.dirname(fileURLToPath(import.meta.url));
export const BASE = path.dirname(DIRECTORY);
export const BUILD_ROOT = 'C:/mir2-build/cell-preproduction-p0-p1-20261024';
export const GIT_BASE = '68f810b442321ae66360a2be18e31dc309efdaf7';
export const OLD_COMMIT = '46880bd1ef415131c4cf85aeb4dd2fa4f6fd4f9b';
export const OLD_MANIFEST_SHA = '4da9d4dac73773bae041c98b641874a9ad4427217c1b36bb453f1a014ad7b058';
export const OLD_MANIFEST = path.join(BASE, 'backend-source-socket-diagnostics-v1.json');
export const OLD_BINARY_SHAS = Object.freeze(['855b4ba3c80ccb99b4128a9ee1170bc56ad51fcbaf022d5269f0b6662986c61f', 'a43078b05e70b480f948fee505e23d2757ce1ae20fdc1f22b2348df46ee7125d']);
export const EXPECTED_INPUT_COUNT = 44;
export const ALLOWED_DELTAS = Object.freeze([
  'mir2-web3/apps/gateway/src/routing.rs',
  'mir2-web3/apps/gateway/src/routing/tests/personal_monster_projection_tests.rs',
  'mir2-web3/apps/gateway/src/routing/tests/live_aoi_order_tests.rs',
  'mir2-web3/apps/gateway/src/routing/tests/pending_zone_aoi_tests.rs',
  'mir2-web3/apps/simulation/src/runtime/zone/runtime.rs',
]);
export const FIXTURE_BASELINE = '0998105c30f35502b8b569b82fb7942a5d356c89';
export const FIXTURE_INSERTIONS = new Map([
  ['mir2-web3/apps/gateway/src/web_live_overflow_tests.rs', ['failure_tx,', 'watch::channel(0).0,', '"overflow-transport-fixture".to_owned(),']],
  ['mir2-web3/apps/gateway/src/web_transport_close_tests.rs', ['"close-reader-fixture".to_owned(),']],
]);
export const MANIFEST = path.join(BASE, 'backend-source-personal-monster-projection-v1.json');
export const RECEIPT = path.join(BASE, 'personal-monster-projection-v1-build-receipt.json');
export const RELEASE = path.join(BASE, 'release-personal-monster-projection-v1.json');
export const BUILD_LOG = path.join(BUILD_ROOT, 'p0-personal-monster-projection-v1-release-build.log');
export const TARGET = path.join(BUILD_ROOT, 'target');
export const BINARY = path.join(TARGET, 'release/mir2-gateway.exe');
export const IMMUTABLE_BINARY = path.join(DIRECTORY, 'bin/mir2-gateway.exe');
export const BUILD_ARGS = ['+1.95.0', 'build', '--locked', '--offline', '--release', '--jobs', '4', '-p', 'mir2-gateway', '--bin', 'mir2-gateway'];
export const BINDING_FILE = path.join(DIRECTORY, 'final-source-binding-v3.json');
export const sha = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
export const pin = async name => { const bytes = await fs.readFile(name); return { path: name, bytes: bytes.length, sha256: sha(bytes) }; };
export const read = async name => JSON.parse(await fs.readFile(name, 'utf8'));
export const sourceInput = name => /^mir2-web3\/(?:.*\.rs|(?:.*\/)?Cargo\.(?:toml|lock)|\.cargo\/.*)$/.test(name);
const fixed = ['mir2-web3/Cargo.toml', 'mir2-web3/Cargo.lock', 'mir2-web3/apps/gateway/Cargo.toml', 'mir2-web3/apps/simulation/Cargo.toml', 'mir2-web3/.cargo/config.toml'];
const isHex = text => typeof text === 'string' && /^[a-f0-9]{40}$/.test(text), isSha = text => typeof text === 'string' && /^[a-f0-9]{64}$/.test(text);
export function options(args = process.argv.slice(2)) {
  const result = {};
  for (let i = 0; i < args.length; i += 2) {
    if (!['--repository', '--expected-head'].includes(args[i]) || !args[i + 1] || result[args[i]]) throw Error('Only one --repository and one --expected-head are permitted');
    result[args[i]] = args[i + 1];
  }
  if (!result['--repository'] || !isHex(result['--expected-head']) || [OLD_COMMIT, 'b84d5d02a8299b6ac36c5e56cb7006f6bc80d3bd'].includes(result['--expected-head'])) throw Error('The real new post-fix committed40hex HEAD and repository directory are required');
  return { repositoryDirectory: path.resolve(result['--repository']), expectedHead: result['--expected-head'] };
}
export const git = (cwd, args, input) => execFileSync('git.exe', args, { cwd, input, windowsHide: true, env: { ...process.env, GIT_NO_LAZY_FETCH: '1' }, maxBuffer: 32 * 1024 * 1024 });
const paths = bytes => bytes.toString('utf8').split('\0').filter(Boolean);
function safeRelative(relative) { if (!sourceInput(relative) || path.isAbsolute(relative) || relative.split('/').some(p => !p || p === '.' || p === '..') || /[:\\]/.test(relative)) throw Error('Unsafe backend source path'); return relative; }
function fixtureOnlyNone(repository, relative, blob) {
  const baseline = git(repository, ['cat-file', 'blob', FIXTURE_BASELINE + ':' + relative]); let restored = blob.toString('utf8');
  for (const anchor of FIXTURE_INSERTIONS.get(relative)) {
    const escaped = anchor.replace(/[.*+?^${}()|[\]\\]/g, '\\$&'), expression = new RegExp(escaped + '\n[ \\t]*None,(?=\n)', 'g');
    if ([...restored.matchAll(expression)].length !== 1) throw Error('The four approved fixture None insertions no longer match'); restored = restored.replace(expression, anchor);
  }
  if (!Buffer.from(restored).equals(baseline)) throw Error('Existing diagnostic fixtures changed beyond the preserved four None arguments');
  return { path: relative, baselineCommit: FIXTURE_BASELINE, baselineGitLfSha256: sha(baseline), addedNoneArguments: FIXTURE_INSERTIONS.get(relative).length, inverseTransformRestoresBaselineGitLf: true };
}
async function finalBinding(expectedHead) {
  const binding = await read(BINDING_FILE);
  if (binding.schemaVersion !== 1 || binding.state !== 'FINAL_COMMITTED_BACKEND_SOURCE' || binding.sourceCommit !== expectedHead || !isHex(binding.sourceCommit) ||
      !Array.isArray(binding.sourcePins) || binding.sourcePins.length !== ALLOWED_DELTAS.length) throw Error('PENDING_FINAL_COMMITTED_BACKEND_SOURCE: root must supply the actual post-fix commit, exact five raw source pins and real validation binding; no capture/build is permitted');
  const wanted = [...ALLOWED_DELTAS].sort();
  if (JSON.stringify(binding.sourcePins.map(p => p.path).sort()) !== JSON.stringify(wanted) || binding.sourcePins.some(p => !Number.isSafeInteger(p.bytes) || p.bytes <= 0 || !isSha(p.sha256))) throw Error('Final five-path source pin schema differs');
  const reference = binding.rootFunctionalValidation;
  if (!reference || typeof reference !== 'object' || Array.isArray(reference) || JSON.stringify(Object.keys(reference).sort()) !== JSON.stringify(['bytes','path','sha256']) ||
      typeof reference.path !== 'string' || !path.isAbsolute(reference.path) || !Number.isSafeInteger(reference.bytes) || reference.bytes <= 0 || !isSha(reference.sha256)) throw Error('rootFunctionalValidation must be an actual exact {path,bytes,sha256} JSON receipt reference');
  if (!/\.json$/i.test(reference.path) || reference.path.split(/[\\/]/).some(part => part === '..' || /^(private|store|recordings|bin|secret|secrets)$/i.test(part))) throw Error('Functional validation must reference a non-private external JSON receipt');
  const stat = await fs.lstat(reference.path), real = await fs.realpath(reference.path), roots = await Promise.all([BASE, BUILD_ROOT].map(root => fs.realpath(root)));
  if (!stat.isFile() || stat.isSymbolicLink() || !roots.some(root => { const relative = path.relative(root, real); return relative && !path.isAbsolute(relative) && relative !== '..' && !relative.startsWith('..' + path.sep); })) throw Error('Functional receipt must resolve to a regular file within the existing external evidence/build roots');
  const receiptBytes = await fs.readFile(real);
  if (receiptBytes.length !== reference.bytes || sha(receiptBytes) !== reference.sha256) throw Error('Functional receipt raw size or SHA differs');
  const receipt = JSON.parse(receiptBytes.toString('utf8'));
  const canonicalPins = pins => pins.map(p => ({path:p.path,bytes:p.bytes,sha256:p.sha256})).sort((a,b) => a.path.localeCompare(b.path));
  if (receipt.schemaVersion !== 1 || !Array.isArray(receipt.sourcePins) || receipt.sourcePins.length !== ALLOWED_DELTAS.length ||
      receipt.sourcePins.some(p => !p || typeof p.path !== 'string' || !Number.isSafeInteger(p.bytes) || p.bytes <= 0 || !isSha(p.sha256)) ||
      JSON.stringify(canonicalPins(receipt.sourcePins)) !== JSON.stringify(canonicalPins(binding.sourcePins)) ||
      receipt.checks?.rustFocusedPassed !== true || receipt.checks?.adjacentPassed !== true || receipt.checks?.rustfmtPassed !== true) throw Error('Functional receipt must bind these exact five raw source pins and the actual focused Rust, adjacent and rustfmt passing summary');
  return binding;
}
export async function captureSource({ repositoryDirectory, expectedHead }) {
  // Deliberately first: no Git source capture or build can run in the prepared state.
  const binding = await finalBinding(expectedHead);
  const repository = await fs.realpath(git(repositoryDirectory, ['rev-parse', '--show-toplevel']).toString('utf8').trim()), sourceCommit = git(repository, ['rev-parse', 'HEAD']).toString('utf8').trim();
  if (sourceCommit !== expectedHead || git(repository, ['status', '--porcelain', '--untracked-files=no']).toString('utf8').trim()) throw Error('Final HEAD must match and all tracked files must be committed/clean');
  git(repository, ['merge-base', '--is-ancestor', OLD_COMMIT, sourceCommit]);
  const untracked = paths(git(repository, ['ls-files', '--others', '-z', '--', 'mir2-web3'])).filter(sourceInput); if (untracked.length) throw Error('Ignored-inclusive untracked backend inputs are forbidden');
  const oldBytes = await fs.readFile(OLD_MANIFEST), old = JSON.parse(oldBytes);
  if (oldBytes.length !== 14000 || sha(oldBytes) !== OLD_MANIFEST_SHA || old.sourceCommit !== OLD_COMMIT || old.gitBase !== GIT_BASE || old.files?.length !== 41 || sha(JSON.stringify(old.files)) !== old.sourceManifestSha256) throw Error('The original468/41 source certificate differs');
  const changed = paths(git(repository, ['diff', '--no-ext-diff', '--no-textconv', '--name-only', '-z', GIT_BASE, sourceCommit, '--', 'mir2-web3'])).filter(sourceInput);
  const expected = [...new Set([...changed, ...fixed])].sort(), allowed = [...new Set([...old.files.map(f => f.path), ...ALLOWED_DELTAS])].sort();
  if (expected.length !== EXPECTED_INPUT_COUNT || JSON.stringify(expected) !== JSON.stringify(allowed)) throw Error('The closure must be the preserved41 plus exactly the existing personal-monster-projection, live-AOI-order and pending-zone-AOI test files (44 total)');
  const delta = paths(git(repository, ['diff', '--no-ext-diff', '--no-textconv', '--name-only', '-z', OLD_COMMIT, sourceCommit, '--', 'mir2-web3'])).filter(sourceInput).sort();
  if (JSON.stringify(delta) !== JSON.stringify([...ALLOWED_DELTAS].sort())) throw Error('Only the five explicitly approved backend paths may differ from468');
  const files = [], fixtureProofs = [], gitSourceBindings = [];
  for (const relative of expected) {
    safeRelative(relative); const full = path.join(repository, relative), stat = await fs.lstat(full), real = await fs.realpath(full), rel = path.relative(repository, real);
    if (!stat.isFile() || stat.isSymbolicLink() || path.isAbsolute(rel) || rel === '..' || rel.startsWith('..' + path.sep)) throw Error('Backend source must be a regular committed file inside the checkout');
    const bytes = await fs.readFile(real), blob = git(repository, ['cat-file', 'blob', sourceCommit + ':' + relative]), filteredObject = git(repository, ['hash-object', '--path=' + relative, '--stdin'], bytes).toString('utf8').trim(), committedObject = git(repository, ['rev-parse', sourceCommit + ':' + relative]).toString('utf8').trim();
    if (filteredObject !== committedObject) throw Error('Raw source filtered through its actual Git attributes differs from the committed code object');
    const entry = { path: relative, bytes: bytes.length, sha256: sha(bytes), gitBlobLfSha256: sha(blob) }, prior = old.files.find(f => f.path === relative);
    if (!ALLOWED_DELTAS.includes(relative) && (!prior || entry.bytes !== prior.bytes || entry.sha256 !== prior.sha256 || entry.gitBlobLfSha256 !== prior.gitBlobLfSha256)) throw Error('An original41 source outside the five approved routing/test/ownership-API paths changed');
    const authorized = binding.sourcePins.find(p => p.path === relative); if (authorized && (authorized.bytes !== entry.bytes || authorized.sha256 !== entry.sha256)) throw Error('Root final source raw pin differs');
    if (FIXTURE_INSERTIONS.has(relative)) fixtureProofs.push(fixtureOnlyNone(repository, relative, blob));
    files.push(entry); gitSourceBindings.push({ path: relative, rawBytes: bytes.length, rawSha256: entry.sha256, gitAttributeFilteredObject: filteredObject, committedGitObject: committedObject, rawWorktreeRewritten: false });
  }
  if (git(repository, ['rev-parse', 'HEAD']).toString('utf8').trim() !== sourceCommit || git(repository, ['status', '--porcelain', '--untracked-files=no']).toString('utf8').trim() || paths(git(repository, ['ls-files', '--others', '-z', '--', 'mir2-web3'])).some(sourceInput)) throw Error('Source tree changed during capture');
  return { repository, sourceCommit, gitBase: GIT_BASE, trackedTreeClean: true, ignoredInclusiveUntrackedBackendCount: 0, files, fixtureProofs, gitSourceBindings,
    sourceInputCount: EXPECTED_INPUT_COUNT, allowedDeltas: ALLOWED_DELTAS, otherOriginalInputsUnchanged: 39,
    closureExpansionReason: 'The existing personal_monster_projection_tests.rs, live_aoi_order_tests.rs and pending_zone_aoi_tests.rs are now explicit deltas but were unchanged from68f to468, so they were not in the old41. Add exactly those three test files:44 inputs,39 Rust files plus the original five Cargo/.cargo inputs. Only routing.rs and zone/runtime.rs change within the old41; the other39 original raw/Git inputs remain unchanged. The two old diagnostic fixtures still invert exactly to099 after removing their four approved None arguments.',
    sourceManifestSha256: sha(JSON.stringify(files)), previousBackendSourceManifest: { ...(await pin(OLD_MANIFEST)), sourceCommit: OLD_COMMIT },
    finalSourceBinding: await pin(BINDING_FILE), rootFunctionalValidation: binding.rootFunctionalValidation, buildArguments: BUILD_ARGS };
}
export async function requireFrozen(selected) {
  const current = await captureSource(selected), manifestBytes = await fs.readFile(MANIFEST), manifest = JSON.parse(manifestBytes);
  if (manifest.schemaVersion !== 1 || manifest.sourceCommit !== selected.expectedHead || manifest.gitBase !== GIT_BASE || manifest.sourceInputCount !== EXPECTED_INPUT_COUNT || manifest.files?.length !== EXPECTED_INPUT_COUNT ||
      manifest.sourceManifestSha256 !== current.sourceManifestSha256 || JSON.stringify(manifest.files) !== JSON.stringify(current.files) || JSON.stringify(manifest.fixtureProofs) !== JSON.stringify(current.fixtureProofs) ||
      JSON.stringify(manifest.gitSourceBindings) !== JSON.stringify(current.gitSourceBindings) || JSON.stringify(manifest.finalSourceBinding) !== JSON.stringify(current.finalSourceBinding) || JSON.stringify(manifest.rootFunctionalValidation) !== JSON.stringify(current.rootFunctionalValidation) || JSON.stringify(manifest.buildArguments) !== JSON.stringify(BUILD_ARGS)) throw Error('Frozen44-input source and exact committed raw/Git source differ');
  return { manifest, manifestBytes, current };
}
export function samePin(a, b) { return a?.bytes === b?.bytes && a?.sha256 === b?.sha256; }
export async function absent(names) { for (const name of names) { try { await fs.access(name); } catch (error) { if (error.code === 'ENOENT') continue; throw error; } throw Error('Refusing to overwrite an existing build artifact'); } }
