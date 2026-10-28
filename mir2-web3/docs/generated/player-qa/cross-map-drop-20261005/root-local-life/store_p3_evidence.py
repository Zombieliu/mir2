import hashlib
import json
from pathlib import Path
import re

PROJECT = Path('C:/Users/Administrator/.codex/worktrees/crowded-combat-control/mir2-player-journey/mir2-web3')
BASE = Path('C:/mir2-playtest-releases/20261004-native-r18')
OUT = Path(__file__).resolve().parent
DEST = PROJECT / 'docs/generated/player-qa/cross-map-drop-20261005'

def digest(data):
    return hashlib.sha256(data).hexdigest()

def write_lf(path, text):
    path.write_bytes(text.encode('utf-8'))

def under(root, relative):
    path = (root / relative).resolve()
    path.relative_to(root.resolve())
    assert not path.is_symlink()
    return path

assert not DEST.exists(), 'preserve previous evidence'
prior = json.loads((BASE / 'cross-map-drop-implementation-01/source-manifest.json').read_bytes())
followup = json.loads((BASE / 'p3-local-life-review-fix-01/source-manifest.json').read_bytes())
sources = {row['file']: row for row in prior['files']}
sources.update({row['file']: row for row in followup['files']})
assert len(sources) == 15
for row in sources.values():
    data = under(PROJECT, row['file']).read_bytes()
    assert len(data) == row['bytes'] and digest(data) == row['sha256'], row['file']

successful = [
    ('p3-root-review-01', 'root-gateway-cross-map-02', 9),
    ('p3-root-review-01', 'root-gateway-death18-01', 12),
    ('p3-root-review-01', 'root-simulation-cross-map-01', 19),
    ('p3-root-review-01', 'root-simulation-checkpoint-01', 17),
    ('p3-root-life-review-01', 'root-local-life-and-owner-01', 43),
    ('p3-root-life-review-01', 'root-cross-map-award-after-life-01', 9),
]
for phase, label, expected in successful:
    receipt = json.loads((BASE / phase / (label + '.json')).read_bytes())
    data = (BASE / phase / (label + '.log')).read_bytes()
    assert receipt['exitCode'] == 0 and digest(data) == receipt['logSha256']
    matches = re.findall(rb'test result: ok\. (\d+) passed; (\d+) failed;', data)
    assert sum(int(p) for p, f in matches) == expected
    assert all(int(f) == 0 for p, f in matches)

final_root = {
    'schema': 'mir2.p3a.root-final-source-and-focused-review.v1',
    'rootBaseline': '753ad7895e39da779b1f453ee4a09b9f0a3c011f',
    'files': list(sources.values()),
    'exactFrozenPhysicalBytes': True,
    'rootOriginalSelections': 57,
    'rootLocalLifeAndAdjacentSelections': 52,
    'countsOverlapAndAreNotAdded': True,
    'workerOriginalDistinctChecks': 89,
    'workerNewDistinctLocalLifeChecks': 3,
    'boundedWorkerTotalDistinctChecks': 92,
    'knownPriorEnvironmentFixtureFailureRetained': True,
    'sameTickLocalTypedDeathChecked': True,
    'crossZoneSameBatchLife': 'OPEN',
    'objectIdAdmissionCollision': 'OPEN; source confirmed, no collision execution',
    'liveNetworkOrNativeOrHumanOrNaturalProgression': False,
    'publishedR18IncludesThisSource': False,
    'servicesOrPlayerSavesChanged': False,
}
write_lf(OUT / 'ROOT-FINAL-SOURCE-01.json', json.dumps(final_root, indent=2) + '\n')

index = []
def copy(source, relative, expected=None):
    data = source.read_bytes()
    if expected:
        assert len(data) == expected['bytes'] and digest(data) == expected['sha256'], str(source)
    target = under(DEST, relative)
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_bytes(data)
    index.append({'file': relative, 'bytes': len(data), 'sha256': digest(data)})

for phase, target_name in (
    ('cross-map-drop-implementation-01', 'original'),
    ('p3-local-life-review-fix-01', 'local-life'),
):
    phase_path = BASE / phase
    inventory = json.loads((phase_path / 'artifact-inventory.json').read_bytes())
    for row in inventory['files']:
        copy(under(phase_path, row['path']), target_name + '/' + row['path'], row)
    copy(phase_path / 'artifact-inventory.json', target_name + '/artifact-inventory.json')

for phase, target_name, names in (
    ('p3-root-review-01', 'root-original', [
        'ROOT-PREFLIGHT-01.json', 'ROOT-INTEGRATION-01.json', 'apply-check.log',
        'integrated-diff-check.log', 'verify_frozen_p3.py', 'apply_frozen_p3.py',
        'run_selected_root.py', 'run_selected_root_v2.py',
        'root-gateway-cross-map-01.log', 'root-gateway-cross-map-01.json',
        'root-gateway-cross-map-02.log', 'root-gateway-cross-map-02.json',
        'root-gateway-death18-01.log', 'root-gateway-death18-01.json',
        'root-simulation-cross-map-01.log', 'root-simulation-cross-map-01.json',
        'root-simulation-checkpoint-01.log', 'root-simulation-checkpoint-01.json',
    ]),
    ('p3-root-life-review-01', 'root-local-life', [
        'ROOT-LOCAL-LIFE-INTEGRATION-01.json', 'ROOT-FINAL-SOURCE-01.json',
        'forward-check.log', 'reverse-check.log', 'diff-check.log',
        'integrate_local_life.py', 'run_root_life.py', 'store_p3_evidence.py',
        'root-local-life-and-owner-01.log', 'root-local-life-and-owner-01.json',
        'root-cross-map-award-after-life-01.log', 'root-cross-map-award-after-life-01.json',
    ]),
):
    for name in names:
        copy(BASE / phase / name, target_name + '/' + name)

write_lf(DEST / '.gitattributes', '* -text -diff\n**/* -text -diff\nREADME.md text eol=lf diff\n.gitattributes text eol=lf diff\n')
write_lf(DEST / 'INDEX.json', json.dumps({
    'schema': 'mir2.p3a.root-durable-evidence.v1', 'files': index,
    'allRawByteExact': True, 'privateFixtureStoresOrKeysIncluded': False,
    'boundedDistinctWorkerChecks': 92, 'rootChecksOverlapWorker': True,
    'crossZoneSameBatchLife': 'OPEN', 'objectIdAdmissionCollision': 'OPEN',
    'networkOrNativeOrHumanAcceptance': False,
}, indent=2) + '\n')
write_lf(DEST / 'README.md', '''# Cross-map solo ownership and same-tick local life

Root imports the original frozen fourteen-file P3a delivery and the reviewed
two-file follow-up. One production file overlaps: the combined source scope is
fifteen files. Exact physical source hashes are retained before root-authored
documentation updates. Source, original Crystal references and test fixtures
are distinguished from live/native/human acceptance.

The bounded implementation preserves one-manager online identity across map
transfer, revokes true leave/fresh admission/cold restore, binds native awards
to source issuance, and retains strict original 60-second monster drop clocks,
absolute object TTL and claim custody. Cold checkpoint roots validate before
clearing live authority. The same-tick follow-up reads the identity-matched
local player's current typed Dead after earlier damage in the tick. Positive
HP after LevelUp is not substituted for life state.

The original genuine Manager RED is 0 passed/6 failed. Its 89 distinct final
checks pass. The new public Manager life test is genuine RED 2 passed/1 failed;
the correction passes its 3 cases and 40 overlapping ownership cases. Thus the
bounded worker union is 92, not 89+43. Root independently passes original
selected 57 checks, then the 43 life/owner checks and 9 Gateway award checks;
these repeat worker checks and overlap one another, so their totals are not
added as new product coverage.

The first root Gateway attempt compiled and passed 8 tests but failed one
isolated fault fixture because MIR2_P3_RECEIPT_DIR was missing. Its raw output
remains. Only the task-specific test receipt environment was added for the
passing rerun; no product change or private fixture-store content is exported.
All worker compiler/tooling/fixture failures and exact approved fixture
adaptations remain in the original frozen evidence.

The local life fixture uses actual public Manager commands, source ArcherGuard
scalars and Hugger behavior, a bounded prepared source-ID branch, real green
poison and actual earlier same-tick player damage. It is a prepared mechanical
case rather than natural progression or a live network run.

Two root review gates remain open: cross-Zone same-batch life visibility and
requested/actual object-ID mismatch during Manager admission or transfer.
The collision finding is source-confirmed and has no executed collision case
or live occurrence claim. Group/party, Boss/PK LastHitter, cross-process/ZoneId,
SQL/recovery, retained resume, natural Boss/gear/books and native/human journeys
remain separate open gates. This does not close full P3 or full P1.

INDEX.json binds all exact raw evidence. Original proof is in original/;
the local-life follow-up is in local-life/; independent root logs, integration
guards and final source review are in root-original/ and root-local-life/.
Private fault stores, account saves, credentials and keys are excluded.
No service, current player save, generated resource or R18 frozen build/package
was changed; R18/source321316 does not contain this later source.
''')

note = '''> 2026-10-05 root integrates the bounded one-manager cross-map solo ownership
> and native ground protection slice, plus identity-matched local typed Dead
> at actual same-tick impact. Original Manager RED0/6 becomes89 passing checks;
> the additional genuine life RED2/1 becomes3 passing cases,92 distinct total.
> Root selects57 original then52 related checks, all pass with overlaps retained.
> Award source issuance, online transfer/leave epochs, strict60-second clocks,
> custody/TTL and cold-root validation are checked in prepared local fixtures.
> Cross-Zone same-batch life and object-ID admission collision remain OPEN;
> party/Boss/PK, process handoff, SQL, network/native/human/progression stay open.
> R18/source321316 artifacts and current services/saves are unchanged.
> [Scope](CROSS-MAP-DROP-LIFECYCLE-20261004.md),
> [root evidence](generated/player-qa/cross-map-drop-20261005/README.md).

'''
for name in ('AGENT-ORCHESTRATION.md', 'AGENT-TASK-QUEUE.md', 'CRYSTAL-1TO1-ROADMAP.md',
             'BACKEND-1TO1-PROGRESS.md', 'CRYSTAL-SERVER-PARITY.md',
             'CLASSIC-GAMEPLAY-GOAL-20261004.md'):
    path = PROJECT / 'docs' / name
    before = path.read_bytes()
    assert note.splitlines()[0].encode() not in before
    split = before.index(b'\n') + 1
    ending = b'\r\n' if b'\r\n' in before else b'\n'
    extra = note.encode().replace(b'\n', ending)
    path.write_bytes(before[:split] + ending + extra + before[split:])

scope_path = PROJECT / 'docs/CROSS-MAP-DROP-LIFECYCLE-20261004.md'
before = scope_path.read_bytes()
split = before.index(b'\n') + 1
addition = '''
> Root integration 2026-10-05: original14 files plus one new life test form a
> combined15-file slice. Independent root57 and post-fix52 selected checks pass
> with overlaps. An actual same-tick local death RED2/1 becomes GREEN3/0 by
> reading identity-matched current typed Dead at impact; adjacent40 pass.
> Cross-Zone same-batch life and requested/actual object-ID admission collision
> remain OPEN, including source-confirmed transfer identity/protection loss.
> No collision execution or live occurrence is claimed. Full P3 is not accepted.
> [Durable root/worker evidence](generated/player-qa/cross-map-drop-20261005/README.md).

'''
ending = b'\r\n' if b'\r\n' in before else b'\n'
scope_path.write_bytes(before[:split] + addition.encode().replace(b'\n', ending) + before[split:])
print(json.dumps({'byteExactEvidenceFiles': len(index), 'globalDocsUpdated': 6,
                  'combinedSourceFiles': 15, 'boundedDistinctWorkerChecks': 92,
                  'privateStoresIncluded': False, 'nativeOrHumanAcceptance': False}), flush=True)
