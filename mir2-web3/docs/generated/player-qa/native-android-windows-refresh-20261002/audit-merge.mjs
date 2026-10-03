import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';

const parent = '9c2a5428d77c6fd2a146b920474eba4b839fdab1';
const upstream = '3d735745f1117d42a7859e87604a106351dca935';
const base = '3f5e61533235921369bc13a7760b4a56b0e467e5';
const sha256 = value => createHash('sha256').update(value).digest('hex');
function run(command, args, input) {
  const result = spawnSync(command, args, { input, maxBuffer: 32 * 1024 * 1024 });
  if (result.error || result.status !== 0) {
    throw new Error(`${command} ${args.join(' ')} failed: ${result.error ?? result.stderr}`);
  }
  return result.stdout;
}
const git = (...args) => run('git', args).toString().trim();
const normalize = value => run(process.env.MIR2_AUDIT_RUSTFMT ?? 'rustfmt',
  [`+${process.env.MIR2_AUDIT_TOOLCHAIN ?? '1.89.0'}`, '--edition', '2021',
    '--config', 'skip_children=true', '--emit', 'stdout'], value);
// Without an argument inspect the resolved pending merge. With an argument
// inspect immutable Git source, so later Android work cannot rebind this audit.
const selected = process.argv[2] ? git('rev-parse', process.argv[2]) : null;
const files = [
  'mir2-web3/apps/gateway/src/routing.rs',
  'mir2-web3/apps/simulation/src/runtime/zone/manager.rs',
  'mir2-web3/apps/simulation/src/runtime/zone/runtime.rs',
];
const server = files.map(file => {
  const expected = run('git', ['show', `${upstream}:${file}`]);
  const actual = selected ? run('git', ['show', `${selected}:${file}`]) : readFileSync(file);
  const normalizedExpected = normalize(expected);
  const normalizedActual = normalize(actual);
  return { file, upstreamBlob: git('rev-parse', `${upstream}:${file}`),
    upstreamSha256: sha256(expected), actualSha256: sha256(actual),
    normalizedUpstreamSha256: sha256(normalizedExpected),
    normalizedActualSha256: sha256(normalizedActual),
    normalizedEqual: normalizedExpected.equals(normalizedActual) };
});
const current = selected ? git('rev-parse', `${selected}^1`) : git('rev-parse', 'HEAD');
const merging = selected ? git('rev-parse', `${selected}^2`) : git('rev-parse', '-q', '--verify', 'MERGE_HEAD');
const compared = selected ? [parent, selected] : [parent];
const androidChanges = git('diff', '--name-only', ...compared, '--',
  'mir2-web3/apps/game-client/platform-android');
const oldEvidenceChanges = git('diff', '--name-only', ...compared, '--',
  'mir2-web3/docs/generated/player-qa/native-android-chat-editor-20261002');
const unresolved = git('ls-files', '--unmerged');
const commonAncestor = git('merge-base', parent, upstream);
const upstreamCommitCount = Number(git('rev-list', '--count', `${base}..${upstream}`));
const report = {
  scope: 'Exact-source merge audit; not APK, online or device acceptance',
  selected, parent, upstream, base, current, merging, commonAncestor, upstreamCommitCount,
  server, androidSourceUnchanged: androidChanges === '',
  oldV16EvidenceUnchanged: oldEvidenceChanges === '', noUnresolvedIndex: unresolved === '',
  expectedMergeParents: current === parent && merging === upstream,
};
report.pass = report.expectedMergeParents && commonAncestor === base && upstreamCommitCount === 14
  && report.androidSourceUnchanged && report.oldV16EvidenceUnchanged
  && report.noUnresolvedIndex && server.every(file => file.normalizedEqual);
process.stdout.write(`${JSON.stringify(report, null, 2)}\n`);
if (!report.pass) process.exitCode = 1;
