"""Freeze only the authorized two-file follow-up against the prior P3 bytes."""
from pathlib import Path
import hashlib
import json
import re
import shutil
import subprocess
from datetime import datetime, timezone

PHASE = Path(__file__).parent
REPO = Path('C:/Users/Administrator/.codex/worktrees/p3/mir2-player-journey/mir2-web3')
PRIOR = PHASE.parent / 'cross-map-drop-implementation-01'
PROD = 'apps/simulation/src/runtime/zone/runtime/experience_ownership.rs'
TEST = 'apps/simulation/tests/shared_experience_owner_tick_life.rs'

def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()

def read_json(path):
    return json.loads(Path(path).read_text(encoding='utf-8-sig'))

def write_json(name, value):
    (PHASE / name).write_text(json.dumps(value, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')

def git_run(args, cwd):
    return subprocess.run(['git', '-c', 'core.autocrlf=false', *args], cwd=cwd, capture_output=True)

manifest = read_json(PRIOR / 'source-manifest.json')
remaining = []
for entry in manifest['files']:
    actual = digest(REPO / entry['file'])
    expected = 'e8602ca303ea9b8a963602cd4a53901e323106b9f3c8f1068879f1d28c330f14' if entry['file'] == PROD else entry['sha256']
    remaining.append({'file': entry['file'], 'frozenSha256': entry['sha256'], 'actualSha256': actual,
                      'unchanged': actual == entry['sha256'], 'authorizedFollowUp': entry['file'] == PROD})
    assert actual == expected, ('unauthorized source change', entry['file'])
assert sum(item['unchanged'] for item in remaining) == 13
write_json('prior-13-source-unchanged.json', remaining)

artifact_checks = []
for item in read_json(PRIOR / 'artifact-inventory.json')['files']:
    actual = digest(PRIOR / item['path'])
    artifact_checks.append({'path': item['path'], 'sha256': actual, 'matchesFrozen': actual == item['sha256']})
    assert actual == item['sha256'], ('prior frozen artifact changed', item['path'])
assert len(artifact_checks) == 100
assert digest(PRIOR / 'artifact-inventory.json') == '7a22bc310d7137ae43523bcd4d3d184d45fb6f2c45ec475e34d4504636d7ef3c'
write_json('prior-100-artifacts-unchanged.json', artifact_checks)

originals = []
for item in read_json(PRIOR / 'original-source-recheck.json')['sources']:
    actual = digest(item['originalPath'])
    assert actual == item['sha256'], ('Crystal source changed', item['originalPath'])
    originals.append({'file': item['originalPath'], 'sha256': actual, 'matchesFrozen': True})
write_json('original-nine-source-unchanged.json', originals)

source_files = []
for relative in (PROD, TEST):
    destination = PHASE / 'final-source' / relative
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(REPO / relative, destination)
    source_files.append({'file': relative, 'sha256': digest(destination), 'bytes': destination.stat().st_size,
                         'new': relative == TEST})
write_json('source-manifest.json', {'baseline': 'frozen P3a fourteen-source phase; HEAD57729', 'files': source_files})

# Diff without Git's checkout newline conversion: byte-exact CRLF sources are
# part of the receipt. Git is used only for read-only diff and external checks.
diff = git_run(['diff', '--no-index', '--no-renames', '--', str(PHASE / 'experience_ownership.frozen.rs'), str(REPO / PROD)], REPO)
assert diff.returncode == 1, diff.stderr.decode(errors='replace')
prod_path = ('mir2-web3/' + PROD).encode()
lines = diff.stdout.splitlines(keepends=True)
lines[0] = b'diff --git a/' + prod_path + b' b/' + prod_path + b'\n'
for index, line in enumerate(lines):
    if line.startswith(b'--- '):
        lines[index] = b'--- a/' + prod_path + b'\n'
    elif line.startswith(b'+++ '):
        lines[index] = b'+++ b/' + prod_path + b'\n'
prod_patch = b''.join(lines)
test_path = ('mir2-web3/' + TEST).encode()
test_bytes = (REPO / TEST).read_bytes()
test_lines = test_bytes.splitlines(keepends=True)
test_patch = b'diff --git a/' + test_path + b' b/' + test_path + b'\nnew file mode 100644\n--- /dev/null\n+++ b/' + test_path + b'\n'
test_patch += f'@@ -0,0 +1,{len(test_lines)} @@\n'.encode()
test_patch += b''.join(b'+' + line for line in test_lines)
patch = PHASE / 'p3-local-life-follow-up.patch'
patch.write_bytes(prod_patch + test_patch)

check_root = PHASE / 'patch-check-forward-01'
assert not check_root.exists(), 'Use a new numbered physical check for any retry'
baseline_file = check_root / 'mir2-web3' / PROD
baseline_file.parent.mkdir(parents=True)
shutil.copyfile(PHASE / 'experience_ownership.frozen.rs', baseline_file)
patch_checks = []
for name, args in [('forward-check', ['apply', '--check', str(patch)]),
                   ('forward-apply', ['apply', str(patch)]),
                   ('reverse-check', ['apply', '--check', '--reverse', str(patch)])]:
    result = git_run(args, check_root)
    (PHASE / f'patch-{name}-attempt-01.log').write_bytes(result.stdout + result.stderr)
    patch_checks.append({'name': name, 'exitCode': result.returncode,
                         'cwd': str(check_root), 'core.autocrlf': False})
    write_json('patch-verification.json', patch_checks)
    assert result.returncode == 0, (name, result.stderr.decode(errors='replace'))
for entry in source_files:
    assert digest(check_root / 'mir2-web3' / entry['file']) == entry['sha256'], ('applied byte mismatch', entry['file'])
write_json('patch-applied-byte-verification.json', [{'file': entry['file'], 'sha256': entry['sha256'], 'exact': True} for entry in source_files])

red = read_json(PHASE / 'red-attempt-01-result.json')
green = read_json(PHASE / 'green-and-owner-adjacent-attempt-01-result.json')
assert red['exitCode'] == 101 and red['productionSha256'] == 'c3b12a1a0685266554b911727b9c1d7c9dbc0ba1ded4818c667951f530ed5ec5'
assert green['exitCode'] == 0
green_text = (PHASE / 'green-and-owner-adjacent-attempt-01.log').read_text(encoding='utf-8-sig', errors='replace')
passed = [int(value) for value in re.findall(r'test result: ok\. (\d+) passed', green_text)]
assert passed == [19, 3, 21], passed
write_json('focused-tests.json', {
    'command': 'cargo test -p mir2-simulation --test shared_experience_owner_tick_life --test shared_monster_ownership --test shared_cross_map_drop_lifecycle -j 2 -- --nocapture',
    'target': 'C:/mir2-build/gateway-tests-r50', 'jobs': 2,
    'red': {'beforeProductionEdit': True, 'passed': 2, 'failed': 1,
            'failedCase': 'owner_dies_earlier_in_same_tick_rival_poison_claims_current_dead_owner',
            'actual': 'owner', 'expected': 'rival', 'rawLog': 'red-attempt-01.log'},
    'green': {'newPassed': 3, 'sameMapOwnerPassed': 21, 'crossMapPassed': 19, 'totalPassed': 43, 'failed': 0,
              'rawLog': 'green-and-owner-adjacent-attempt-01.log'},
    'testFormatting': {'redSha256': red['testSha256'], 'greenSha256': green['testSha256'],
                       'change': 'rustfmt --edition 2021 applied only to the new test after RED; no assertion or fixture logic change'},
    'preparedTickAcceptance': True, 'networkAcceptance': False,
})

finding_sources = [
    ('apps/simulation/src/runtime/zone/manager.rs', [56, 519, 524, 543, 566, 598, 680]),
    ('apps/simulation/src/runtime/zone/runtime.rs', [671, 2478, 2483, 3714, 12682, 14298, 14314]),
    ('apps/simulation/src/runtime/zone/online_identity.rs', [21, 27, 81]),
    ('apps/gateway/src/routing.rs', [2883, 3363, 3380, 9636, 9644, 11658]),
    ('apps/simulation/src/runtime/monsters.rs', [183, 2121, 2126, 2129]),
    ('apps/simulation/src/runtime/crystal_compat.rs', [5]),
]
write_json('object-id-collision-readonly.json', {
    'status': 'source-confirmed public Manager defect; no collision test or live run executed in this phase',
    'distinguishedFrom': 'same-tick local typed Dead follow-up',
    'sources': [{'file': relative, 'sha256': digest(REPO / relative), 'lines': lines} for relative, lines in finding_sources],
    'original': [{'file': 'E:/mir2/Crystal/Server/MirObjects/MapObject.cs', 'sha256': digest('E:/mir2/Crystal/Server/MirObjects/MapObject.cs'), 'line': 21},
                 {'file': 'E:/mir2/Crystal/Server/MirEnvir/Envir.cs', 'sha256': digest('E:/mir2/Crystal/Server/MirEnvir/Envir.cs'), 'line': 92}],
    'freshManager': 'admit(requested ID) precedes unique_object_id actual remapping. Zero or an occupied player/monster/object/ground/claimed ID yields player-world membership but no matching online presence and no native ownership.',
    'crossMapManager': 'destination collision remaps actual object while retained same-epoch proof keeps the original ID. Refresh excludes the transferred player and source/other zones clear old claims/protection. The book proof may still validate while no presence matches.',
    'standalone': 'safe for this mismatch: ZoneRuntime admits local book against actual remapped ID.',
    'ordinaryGateway': 'client cannot pick server ID. Shared counter starts50000, personal-drop projection shares it, but configured/native objects and Zone allocator do not. Initial fifty-to-one-hundred players are not proven to collide: native dynamic starts1000000; classic respawn derives from200000; legacy starter monster60000. Counter growth/restore has no cross-domain availability guard.',
    'recommendedNextPolicy': 'Preflight destination identity before revoke/admit/detach; reject zero or another occupied ID and preserve original membership, proof, award/custody references, and clocks. Return a visible failure to Gateway. Allocate checked fresh globally unused IDs for new online players. Cross-map retain the same global Node ID; do not remint/rewrite all old ownership on collision.',
    'proposedMinimumFiles': ['apps/simulation/src/runtime/zone/runtime.rs', 'apps/simulation/src/runtime/zone/manager.rs', 'apps/gateway/src/routing.rs', 'apps/simulation/tests/shared_online_object_identity.rs (NEW)', 'apps/gateway/src/routing/tests/online_object_identity_tests.rs (NEW)'],
    'minimumTests': ['fresh requested0 failclosed', 'occupied player/monster/NPC/live-drop/held-drop destination IDs', 'fresh admission has matching actual proof and one source-issued award', 'cross-map collision leaves original source membership and proof/claims/clocks unchanged', 'ordinary Gateway NewAccount/Login/NewCharacter/StartGame with prepared allocation collision', 'counter exhaustion never recycles and never detaches old membership'],
    'filesChangedForThisFinding': [],
})

(PHASE / 'REVIEW.md').write_text('''# Bounded local at-impact Dead follow-up

The exact prior P3a delivery remains immutable: fourteen-source manifest, original complete patch, one hundred artifact bytes and nine original Crystal source bytes were rechecked. Only `experience_ownership.rs` is a later authorized source change; the other thirteen files remain byte-identical. No prior receipt was rewritten.

Original `MonsterObject.cs` (SHA256 2cc4e673c589ad57452b4682c3bcb155b4b6b3027b1d3ee52a18a5ae1b7df98e) lines1493–1509 and2627–2633 read current `EXPOwner.Dead` when applying poison/direct damage. Batch-start presence was insufficient when the owner died earlier in the same local tick. A matching online proof and all stored identity fields are checked first. A matching local player then supplies typed `dead`; no HP inference is used. Only an owner whose presence names another Zone uses immutable global snapshot `dead`.

The new prepared Manager regression uses real Wizard FireBall damage, a real Hugger delayed blast and its chance/resist-selected green poison, and a rival Taoist poison pulse. At2600 the first owner's actual poison receipt145→0 precedes the target's three-damage fatal pulse. Frozen production incorrectly selected the owner; fixed production selects the rival and issues one correctly bound source award/drop. Controls745→490 surviving owner and full-HP typed Dead both pass. These trusted prepared commands and scalar fixtures do not constitute a normal-network or human route acceptance result.

RED had two controls passing and the actual same-tick case failing. The sole formatting step was rustfmt of the new test between RED/GREEN; fixture/assertion logic remained unchanged. GREEN includes all three new cases, twenty-one same-map ownership cases and nineteen cross-map/drop cases:43 passed, zero failed. No broad Gateway/death test rerun was needed because their production inputs did not change.

Patch paths start at `mir2-web3/` and apply after the original P3 fourteen-file phase. Physical forward check, forward apply with SHA256-equal final two source bytes, and reverse check use `git -c core.autocrlf=false apply` on the separately created external source-only directory. No Git commit or index mutation was performed in the real worktree.

Remote ownership life changing earlier/later in another Zone of the same sequential or parallel tick batch remains OPEN. There is no redesign of Zone phases, no claim of complete global mutable Node parity, no protocol change, and no custody/cold-restore/TTL changes.

The independently requested object-ID finding is source-backed and remains read-only. Manager proof admission precedes managed Join remapping; matching presence can then disappear. The public Manager zero/collision path follows directly from code, but no collision test/network run was executed. Gateway uses a counter starting50000 without a destination/native/global-domain collision scan; initial fifty-to-one-hundred players are not claimed to have collided. See `object-id-collision-readonly.json` for exact source hashes/line references, original global readonly ObjectID evidence, the five-file proposed next write set, and fail-closed admission recommendation.

No service, network, player-save, generated-resource, package, release-build, publish, commit, push, runtime tick-phase, Manager, routing, or global-doc changes occurred in this follow-up. This phase is frozen for parent review.
''', encoding='utf-8')

receipt = {
    'schema': 'mir2.p3a.local-at-impact-life.follow-up.v1', 'createdAt': datetime.now(timezone.utc).isoformat(),
    'status': 'frozen-for-parent-review', 'worktree': str(REPO), 'relativeTo': 'exact original frozen P3a 14-file delivery',
    'files': 2, 'productionFiles': [PROD], 'newTests': [TEST], 'patch': patch.name, 'patchSha256': digest(patch),
    'finalSourceManifestSha256': digest(PHASE / 'source-manifest.json'),
    'trueRedBeforeProductionEdit': True, 'redPassed': 2, 'redFailed': 1, 'greenPassed': 43, 'greenFailed': 0,
    'prior13SourceFilesUnchanged': True, 'prior14FrozenCopiesAndPhaseUnchanged': True,
    'prior100FrozenArtifactsUnchanged': True, 'nineCrystalSourcesUnchanged': True,
    'exactPhysicalForwardApply': True, 'networkAcceptance': False, 'crossZoneSameBatchLife': 'OPEN',
    'objectIdCollision': 'separate read-only finding and proposed next write set; not fixed or run',
    'noChangesTo': ['runtime.rs', 'manager.rs', 'routing.rs', 'types.rs', 'cold custody', 'TTL/protection clocks', 'global docs', 'services', 'saves', 'resources', 'Git commits', 'release/publication'],
}
write_json('final-receipt.json', receipt)
inventory = [{'path': str(path.relative_to(PHASE)).replace('\\', '/'), 'bytes': path.stat().st_size,
              'sha256': digest(path)} for path in sorted(PHASE.rglob('*')) if path.is_file() and path.name != 'artifact-inventory.json']
write_json('artifact-inventory.json', {'schema': 'mir2.p3a.local-life.frozen-artifacts.v1', 'files': inventory})
print(json.dumps({'patchSha256': digest(patch), 'receiptSha256': digest(PHASE / 'final-receipt.json'),
                  'sourceManifestSha256': digest(PHASE / 'source-manifest.json'), 'artifactCount': len(inventory),
                  'artifactInventorySha256': digest(PHASE / 'artifact-inventory.json'), 'passed': 43}, indent=2))
