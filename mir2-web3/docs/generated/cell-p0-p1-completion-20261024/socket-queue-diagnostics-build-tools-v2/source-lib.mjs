import fs from 'node:fs/promises';
import path from 'node:path';
import crypto from 'node:crypto';
import { execFileSync } from 'node:child_process';

export const BASE = 'C:/mir2-build/p01-completion-20261024';
export const BUILD_ROOT = 'C:/mir2-build/cell-preproduction-p0-p1-20261024';
export const GIT_BASE = '68f810b442321ae66360a2be18e31dc309efdaf7';
export const OLD_COMMIT = '461f1d85eed6309a55a88bd79a0022f7b97febe4';
export const OLD_MANIFEST_SHA = '94c7ee1b121fac0634212989638d7c451e8ce89343b3bf23a9d940d7365f289d';
export const OLD_BINARY_SHA = '855b4ba3c80ccb99b4128a9ee1170bc56ad51fcbaf022d5269f0b6662986c61f';
export const FIXTURE_BASELINE = '0998105c30f35502b8b569b82fb7942a5d356c89';
export const FIXTURE_INSERTIONS = new Map([
  ['mir2-web3/apps/gateway/src/web_live_overflow_tests.rs', ['failure_tx,', 'watch::channel(0).0,', '"overflow-transport-fixture".to_owned(),']],
  ['mir2-web3/apps/gateway/src/web_transport_close_tests.rs', ['"close-reader-fixture".to_owned(),']]
]);
export const MANIFEST = path.join(BASE, 'backend-source-socket-diagnostics-v1.json');
export const RECEIPT = path.join(BASE, 'socket-diagnostics-v1-build-receipt.json');
export const RELEASE = path.join(BASE, 'release-socket-diagnostics-v1.json');
export const BUILD_LOG = path.join(BUILD_ROOT, 'p0-socket-diagnostics-v1-release-build.log');
export const TARGET = path.join(BUILD_ROOT, 'target');
export const BINARY = path.join(TARGET, 'release/mir2-gateway.exe');
export const IMMUTABLE_BINARY = path.join(BASE, 'socket-diagnostics-v1/bin/mir2-gateway.exe');
export const BUILD_ARGS = ['+1.95.0', 'build', '--locked', '--offline', '--release', '-p', 'mir2-gateway', '--bin', 'mir2-gateway'];
export const sha = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
export const pin = async name => { const bytes = await fs.readFile(name); return { path: name, bytes: bytes.length, sha256: sha(bytes) }; };
export const read = async name => JSON.parse(await fs.readFile(name, 'utf8'));
export const sourceInput = name => /^mir2-web3\/(?:.*\.rs|(?:.*\/)?Cargo\.(?:toml|lock)|\.cargo\/.*)$/.test(name);
const fixed = ['mir2-web3/Cargo.toml', 'mir2-web3/Cargo.lock', 'mir2-web3/apps/gateway/Cargo.toml', 'mir2-web3/apps/simulation/Cargo.toml', 'mir2-web3/.cargo/config.toml'];
const isHex = text => typeof text === 'string' && /^[a-f0-9]{40}$/.test(text);
export function options(args = process.argv.slice(2)) {
  const result = {};
  for (let i=0;i<args.length;i+=2) {
    if (!['--repository','--expected-head'].includes(args[i]) || !args[i+1] || result[args[i]]) throw Error('Only one --repository and one --expected-head are permitted');
    result[args[i]] = args[i+1];
  }
  if (!result['--repository'] || !isHex(result['--expected-head']) || result['--expected-head'] === OLD_COMMIT || result['--expected-head'] === '0998105c30f35502b8b569b82fb7942a5d356c89') throw Error('Explicit new full40-character committed HEAD and repository directory are required');
  return { repositoryDirectory: path.resolve(result['--repository']), expectedHead: result['--expected-head'] };
}
export const git = (cwd,args) => execFileSync('git.exe', args, { cwd, windowsHide:true, env:{...process.env,GIT_NO_LAZY_FETCH:'1'}, maxBuffer:32*1024*1024 });
const paths = bytes => bytes.toString('utf8').split('\0').filter(Boolean);
function fixtureOnlyNone(repository,relative,blob) {
  const baseline=git(repository,['cat-file','blob',FIXTURE_BASELINE+':'+relative]);
  let restored=blob.toString('utf8');
  for(const anchor of FIXTURE_INSERTIONS.get(relative)) {
    const escaped=anchor.replace(/[.*+?^${}()|[\]\\]/g,'\\$&');
    const expression=new RegExp(escaped+'\\n[ \\t]*None,(?=\\n)','g');
    if([...restored.matchAll(expression)].length!==1)throw Error('Expected exactly one new None constructor argument at each approved fixture anchor');
    restored=restored.replace(expression,anchor);
  }
  if(!Buffer.from(restored).equals(baseline))throw Error('New fixture input differs from099 beyond the four approved None constructor arguments');
  return {path:relative,baselineCommit:FIXTURE_BASELINE,baselineGitLfSha256:sha(baseline),addedNoneArguments:FIXTURE_INSERTIONS.get(relative).length,
    inverseTransformRestoresBaselineGitLf:true};
}
export async function captureSource({repositoryDirectory,expectedHead}) {
  const repository = (await fs.realpath(git(repositoryDirectory,['rev-parse','--show-toplevel']).toString('utf8').trim()));
  const sourceCommit=git(repository,['rev-parse','HEAD']).toString('utf8').trim();
  if (sourceCommit!==expectedHead) throw Error('Current HEAD is not the explicitly selected new backend commit');
  if (git(repository,['status','--porcelain','--untracked-files=no']).toString('utf8').trim()) throw Error('Tracked source tree must be fully committed and clean');
  // Deliberately omit --exclude-standard: ignored Rust/Cargo/.cargo files must
  // not become invisible extra build inputs. Only names are read here.
  const untracked=paths(git(repository,['ls-files','--others','-z','--','mir2-web3'])).filter(sourceInput);
  if (untracked.length) throw Error('Ignored-inclusive untracked backend inputs are forbidden');
  const changed=paths(git(repository,['diff','--no-ext-diff','--no-textconv','--name-only','-z',GIT_BASE,sourceCommit,'--','mir2-web3'])).filter(sourceInput);
  const expected=[...new Set([...changed,...fixed])].sort();
  const oldBytes=await fs.readFile(path.join(BASE,'backend-source-native-attack-v2.json')), old=JSON.parse(oldBytes);
  if (sha(oldBytes)!==OLD_MANIFEST_SHA || old.sourceCommit!==OLD_COMMIT || old.gitBase!==GIT_BASE || old.files?.length!==39 || sha(JSON.stringify(old.files))!==old.sourceManifestSha256) throw Error('Preserved original39-input source manifest differs');
  const allowed=[...old.files.map(f=>f.path),...FIXTURE_INSERTIONS.keys()].sort();
  if (expected.length!==41 || JSON.stringify(expected)!==JSON.stringify(allowed)) throw Error('New backend closure must be original39 plus exactly the two approved fixture inputs');
  const files=[], fixtureProofs=[];
  for(const relative of expected) {
    if (!sourceInput(relative) || path.isAbsolute(relative) || relative.split('/').some(p=>!p||p==='.'||p==='..') || /[:\\]/.test(relative)) throw Error('Unsafe backend path');
    const full=path.join(repository,relative), resolved=await fs.realpath(full), rel=path.relative(repository,resolved);
    if (path.isAbsolute(rel) || rel.startsWith('..') || (await fs.lstat(full)).isSymbolicLink()) throw Error('Backend input must be a normal file within the committed checkout');
    const bytes=await fs.readFile(full), blob=git(repository,['cat-file','blob',sourceCommit+':'+relative]);
    if (!bytes.equals(blob) && !Buffer.from(bytes.toString('utf8').replaceAll('\r\n','\n')).equals(blob)) throw Error('Current raw backend bytes differ from committed Git LF blob');
    const entry={path:relative,bytes:bytes.length,sha256:sha(bytes),gitBlobLfSha256:sha(blob)};
    const prior=old.files.find(f=>f.path===relative);
    if(FIXTURE_INSERTIONS.has(relative)) fixtureProofs.push(fixtureOnlyNone(repository,relative,blob));
    else if(relative!=='mir2-web3/apps/gateway/src/web.rs' && (entry.bytes!==prior.bytes || entry.sha256!==prior.sha256 || entry.gitBlobLfSha256!==prior.gitBlobLfSha256)) throw Error('Socket diagnostics source changed an input outside web.rs and the two approved fixtures');
    files.push(entry);
  }
  const web=files.find(f=>f.path==='mir2-web3/apps/gateway/src/web.rs'), priorWeb=old.files.find(f=>f.path===web.path);
  if(web.gitBlobLfSha256===priorWeb.gitBlobLfSha256) throw Error('New socket diagnostics web.rs has not been committed');
  const oldToNew=paths(git(repository,['diff','--no-ext-diff','--no-textconv','--name-only','-z',OLD_COMMIT,sourceCommit,'--','mir2-web3'])).filter(sourceInput);
  if(JSON.stringify(oldToNew)!==JSON.stringify(['mir2-web3/apps/gateway/src/web.rs',...FIXTURE_INSERTIONS.keys()].sort())) throw Error('New backend may change only web.rs and the two approved fixtures relative to preserved461f source');
  if(git(repository,['rev-parse','HEAD']).toString('utf8').trim()!==sourceCommit || git(repository,['status','--porcelain','--untracked-files=no']).toString('utf8').trim() ||
    paths(git(repository,['ls-files','--others','-z','--','mir2-web3'])).some(sourceInput)) throw Error('Source tree changed while capturing the backend inputs');
  return {repository,sourceCommit,gitBase:GIT_BASE,trackedTreeClean:true,ignoredInclusiveUntrackedBackendCount:0,files,fixtureProofs,
    sourceInputCount:41,closureExpansionReason:'Focused compile-v1 failed E0061 at four pre-existing test constructor calls after the new diagnostics parameter. Exactly three None arguments in web_live_overflow_tests.rs and one in web_transport_close_tests.rs are added; old assertions and all other fixture bytes restore exactly to099 Git LF after removing those four arguments.',
    sourceManifestSha256:sha(JSON.stringify(files)),previousBackendSourceManifest:{...(await pin(path.join(BASE,'backend-source-native-attack-v2.json'))),sourceCommit:OLD_COMMIT},buildArguments:BUILD_ARGS};
}
export async function requireFrozen(options) {
  const manifestBytes=await fs.readFile(MANIFEST),manifest=JSON.parse(manifestBytes),current=await captureSource(options);
  if(manifest.schemaVersion!==1||manifest.sourceCommit!==options.expectedHead||manifest.gitBase!==GIT_BASE||manifest.sourceInputCount!==41||manifest.files?.length!==41||manifest.sourceManifestSha256!==current.sourceManifestSha256||JSON.stringify(manifest.files)!==JSON.stringify(current.files)||JSON.stringify(manifest.fixtureProofs)!==JSON.stringify(current.fixtureProofs)||JSON.stringify(manifest.buildArguments)!==JSON.stringify(BUILD_ARGS)) throw Error('Frozen manifest and current exact committed41-input source differ');
  return {manifest,manifestBytes,current};
}
export function samePin(a,b) { return a?.bytes===b?.bytes && a?.sha256===b?.sha256; }
export async function absent(names) { for(const name of names) { try { await fs.access(name); } catch(error) { if(error.code==='ENOENT') continue; throw error; } throw Error('Refusing to overwrite an existing build artifact'); } }
