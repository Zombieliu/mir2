import assert from 'node:assert/strict';
import test from 'node:test';
import { verifyNumeronSourceClosure } from './numeron-source-closure.mjs';

const sha = value => value.toString(16).padStart(40, '0');
const file = (filename, value, status = 'modified') => ({ filename, sha: sha(value), status });
const check = (condition, message) => assert(condition, message);
function fixture() {
  const first = { baseSource: sha(1), source: sha(3), sourceCommits: [sha(2), sha(3)],
    changedFiles: [file('existing.js', 11), file('new.js', 12, 'added')] };
  const last = { baseSource: sha(3), source: sha(4), sourceCommits: [sha(4)],
    changedFiles: [file('new.js', 13), file('later.js', 14, 'added')] };
  const release = { source: sha(4), baseSource: sha(1), sourceCommits: [sha(2), sha(3), sha(4)],
    changedFiles: [file('existing.js', 11), file('new.js', 13, 'added'), file('later.js', 14, 'added')],
    sourceCheckpoints: [first, last] };
  const calls = [];
  const read = async (base, source) => {
    calls.push({ base, source });
    const segment = release.sourceCheckpoints.find(segment => segment.baseSource === base && segment.source === source);
    assert(segment);
    return { status: 'ahead', ahead_by: segment.sourceCommits.length, behind_by: 0,
      merge_base_commit: { sha: base }, commits: segment.sourceCommits.map(sha => ({ sha })).reverse(),
      files: structuredClone(segment.changedFiles).reverse() };
  };
  return { release, calls, read, verify: (reader = read) => verifyNumeronSourceClosure(release, release.source, reader, check) };
}

test('a complete chain composes an added path later modified and checks every segment', async () => {
  const f = fixture(), receipt = await f.verify();
  assert.deepEqual(receipt.changedFiles, [file('existing.js', 11), file('new.js', 13, 'added'), file('later.js', 14, 'added')]);
  assert.deepEqual(f.calls, [{ base: sha(1), source: sha(3) }, { base: sha(3), source: sha(4) }]);
  assert.equal(receipt.base, sha(1));assert.equal(receipt.checkpoints.length, 2);
});

test('more than300 total paths are verified without accepting a truncated comparison', async () => {
  const f = fixture(), rows = Array.from({ length: 299 }, (_, n) => file(`first-${n}.js`, n + 100, 'added'));
  f.release.sourceCheckpoints[0].changedFiles = rows;
  f.release.sourceCheckpoints[1].changedFiles = [file('last-a.js', 501, 'added'), file('last-b.js', 502, 'added')];
  f.release.changedFiles = [...rows, ...f.release.sourceCheckpoints[1].changedFiles];
  const receipt = await f.verify();assert.equal(receipt.changedFiles.length, 301);
  await assert.rejects(f.verify(async (base, source) => {
    const result = await f.read(base, source);if (source === sha(3)) result.files.pop();return result;
  }), /qrSourceClosureMismatch/);
});

for (const index of [0, 1]) for (const [name, mutate] of [
  ['missing file', c => { c.files.pop(); }],
  ['extra file', c => { c.files.push(file('unreviewed.js', 999)); }],
  ['duplicate path', c => { c.files[1] = c.files[0]; }],
  ['changed blob', c => { c.files[0].sha = sha(999); }],
  ['changed status', c => { c.files[0].status = 'removed'; }],
  ['missing commit', c => { c.commits.pop(); c.ahead_by--; }],
  ['unreviewed commit', c => { c.commits[0].sha = sha(999); }],
  ['wrong merge base', c => { c.merge_base_commit.sha = sha(999); }],
  ['malformed files', c => { c.files = null; }],
]) test(`segment${index+1} rejects ${name}`, async () => {
  const f = fixture();await assert.rejects(f.verify(async (base, source) => {
    const result = await f.read(base, source);if (source === f.release.sourceCheckpoints[index].source) mutate(result);return result;
  }), /qrSourceClosureMismatch/);
});

for (const [name, mutate] of [
  ['gap', r => { r.sourceCheckpoints[1].baseSource = sha(2); }],
  ['early final source', r => { r.source = sha(3); }],
  ['unaccounted final path', r => { r.changedFiles.push(file('hidden.js', 999)); }],
  ['wrong composed add status', r => { r.changedFiles[1].status = 'modified'; }],
  ['repeated commit across segments', r => { r.sourceCheckpoints[1].sourceCommits.push(sha(2)); r.sourceCommits.push(sha(2)); }],
  ['ambiguous300-file segment', r => { r.sourceCheckpoints[0].changedFiles = Array.from({length:300},(_,n)=>file(`f${n}`,n+100)); }],
  ['unsupported checkpoint removal', r => { r.sourceCheckpoints[1].changedFiles[1].status = 'removed'; }],
  ['duplicate add across checkpoints', r => { r.sourceCheckpoints[1].changedFiles[0].status = 'added'; }],
]) test(`configuration rejects ${name}`, async () => {
  const f = fixture();mutate(f.release);await assert.rejects(f.verify(), /qrSourceClosureMismatch/);
});

test('a single legacy closure retains the original exact comparison contract', async () => {
  const f = fixture(), segment = f.release.sourceCheckpoints[0];
  const legacy = { source: segment.source, ...segment };
  const receipt = await verifyNumeronSourceClosure(legacy, legacy.source, f.read, check);
  assert.equal(receipt.base, sha(1));assert.equal(receipt.checkpoints, undefined);
  assert.deepEqual(receipt.changedFiles, segment.changedFiles);
});
