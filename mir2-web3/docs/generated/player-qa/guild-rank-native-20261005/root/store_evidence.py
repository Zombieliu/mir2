import gzip
import hashlib
import io
import json
from pathlib import Path
import re
import tarfile

PROJECT = Path('C:/Users/Administrator/.codex/worktrees/crowded-combat-control/mir2-player-journey/mir2-web3')
BASE = Path('C:/mir2-playtest-releases/20261004-native-r18')
PHASE = BASE / 'guild-rank-native-implementation-01'
OUT = Path(__file__).resolve().parent
DEST = PROJECT / 'docs/generated/player-qa/guild-rank-native-20261005'

def digest(data):
    return hashlib.sha256(data).hexdigest()

def write(path, text):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(text.encode())

def safe(root, relative):
    path = (root / relative).resolve()
    path.relative_to(root.resolve())
    assert not path.is_symlink()
    return path

assert not DEST.exists(), 'preserve previous evidence'
frozen_data = (PHASE / 'FROZEN-DELIVERY.json').read_bytes()
assert digest(frozen_data) == 'b43c6f12369fb9912541b8aeea83f0055defa7587c86feb829e4cf1a323e5a2f'
frozen = json.loads(frozen_data)
integration = json.loads((OUT / 'ROOT-INTEGRATION-01.json').read_bytes())
for row in frozen['files']:
    current = safe(PROJECT, row['path']).read_bytes()
    expected = integration['combinedRoutingSha256'] if row['path'] == 'apps/gateway/src/routing.rs' else row['sha256']
    assert digest(current) == expected, row['path']
for relative, expected in frozen['protected_source_and_lock_sha256'].items():
    assert digest(safe(PROJECT, relative).read_bytes()) == expected, relative

selections = [('root-client-guild-01', 83, 1), ('root-wire-rank-01', 4, 0),
              ('root-fifo-rank-01', 4, 0), ('root-gateway-rank-terminal-01', 3, 0),
              ('root-gateway-cross-map-after-rank-01', 9, 0)]
for label, expected, ignored in selections:
    receipt = json.loads((OUT / (label + '.json')).read_bytes())
    log = (OUT / (label + '.log')).read_bytes()
    assert receipt['exitCode'] == 0 and digest(log) == receipt['logSha256']
    results = re.findall(rb'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;', log)
    assert sum(int(p) for p, f, i in results) == expected
    assert all(int(f) == 0 for p, f, i in results)
    assert sum(int(i) for p, f, i in results) == ignored
final = {
    'schema': 'mir2.p5a.native-rank.root-final-review.v1',
    'rootBase': integration['rootBase'], 'frozenWorkerReceipt': digest(frozen_data),
    'exactTenImportedFilesAndCombinedRoutingRetained': True,
    'protectedSourceAndLocksUnchanged': True,
    'rootSelections': [{'name': label, 'passed': passed, 'ignored': ignored} for label, passed, ignored in selections],
    'countsOverlapWorkerAndAreNotParityPercent': True,
    'knownRedAndHarnessFailuresRetained': True,
    'nativeClicksOrScreenshotsOrHumanAcceptance': False,
    'pairedNetworkOrRolloutAccepted': False,
    'open': frozen['open_gates'],
    'r18FrozenSourceDoesNotContainThisLaterSlice': True,
    'servicesOrPlayerSavesChanged': False,
}
write(OUT / 'ROOT-FINAL-REVIEW-01.json', json.dumps(final, indent=2) + '\n')

index = []
def copy(source, relative):
    data = source.read_bytes()
    target = safe(DEST, relative)
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_bytes(data)
    index.append({'file': relative, 'bytes': len(data), 'sha256': digest(data)})

top_names = [
    'FROZEN-DELIVERY.json', 'DELIVERY.md', 'FINAL-VALIDATION.json', 'baseline-index.json',
    'guild-rank-native-scoped.patch', 'routing-guild-rank-terminal.patch',
    'gateway-terminal-fixture-origin.json', 'freeze-harness-prefix-red-01.log',
    'freeze-harness-prefix-red-01.json', 'frozen-patch-reverse-check.log',
    'freeze_delivery.py', 'run_cargo.py', 'snapshot_inputs.py', 'setup_evidence.py',
    'write_gateway_red_test.py', 'guild_rank_ingest_tests.prepared.rs',
]
for run in frozen['scoped_runs']:
    for suffix, key in (('.log', 'log_sha256'), ('-input.json', 'input_sha256'), ('-receipt.json', 'receipt_sha256')):
        name = run['name'] + suffix
        assert digest((PHASE / name).read_bytes()) == run[key], name
        top_names.append(name)
for name in top_names:
    copy(PHASE / name, 'raw/' + name)

source_rows = []
archive = OUT / 'SOURCE-SNAPSHOTS-01.tar.gz'
assert not archive.exists(), 'preserve completed archive'
with archive.open('wb') as raw:
    with gzip.GzipFile(filename='', fileobj=raw, mode='wb', mtime=0) as zipped:
        with tarfile.open(fileobj=zipped, mode='w|', format=tarfile.PAX_FORMAT) as tar:
            for directory in ('baseline-source', 'client-red-01-source', 'client-red-02-source',
                              'gateway-red-01-source', 'wire-red-01-source', 'final-source'):
                source_root = PHASE / directory
                assert source_root.is_dir()
                for source in sorted(source_root.rglob('*')):
                    if not source.is_file():
                        continue
                    assert not source.is_symlink()
                    relative = source.relative_to(PHASE).as_posix()
                    assert source.suffix == '.rs' or source.name in ('Cargo.toml', 'Cargo.lock', 'SHARED-GUILD-RANK-RENAME-20261004.md'), relative
                    data = source.read_bytes()
                    info = tarfile.TarInfo(relative)
                    info.size, info.mode, info.mtime = len(data), 0o644, 0
                    tar.addfile(info, io.BytesIO(data))
                    source_rows.append({'path': relative, 'bytes': len(data), 'sha256': digest(data)})
with tarfile.open(archive, 'r:gz') as tar:
    for row in source_rows:
        data = tar.extractfile(row['path']).read()
        assert len(data) == row['bytes'] and digest(data) == row['sha256']
copy(archive, 'raw/SOURCE-SNAPSHOTS-01.tar.gz')
write(DEST / 'SOURCE-SNAPSHOTS-INDEX.json', json.dumps({'schema': 'mir2.p5a.raw-source-snapshots.v1',
    'archive': 'raw/SOURCE-SNAPSHOTS-01.tar.gz', 'files': source_rows,
    'allMembersIndividuallyReverified': True, 'privateStoresOrKeysIncluded': False}, indent=2) + '\n')

for name in ('ROOT-INTEGRATION-01.json', 'ROOT-FINAL-REVIEW-01.json',
             'whole-forward-check.log', 'routing-forward-check.log', 'routing-apply.log',
             'whole-reverse-check.log', 'diff-check.log',
             'integrate_frozen_rank.py', 'run_root_rank.py', 'store_evidence.py'):
    copy(OUT / name, 'root/' + name)
for label, passed, ignored in selections:
    copy(OUT / (label + '.log'), 'root/' + label + '.log')
    copy(OUT / (label + '.json'), 'root/' + label + '.json')
write(DEST / '.gitattributes', '* -text -diff\n**/* -text -diff\nREADME.md text eol=lf diff\n.gitattributes text eol=lf diff\n')
write(DEST / 'INDEX.json', json.dumps({'schema': 'mir2.p5a.root-durable-evidence.v1', 'files': index,
    'sourceArchiveMemberIndex': 'SOURCE-SNAPSHOTS-INDEX.json',
    'allRawBytesExact': True, 'privateStoresOrKeysIncluded': False,
    'nativeOrNetworkOrHumanAcceptance': False}, indent=2) + '\n')
write(DEST / 'README.md', '''# Native Guild rank rename: root integration and review

Root imports the eleven-file frozen worker delivery into P3/source513eb83.
Only the two reviewed Guild terminal hunks change the already-integrated
Gateway routing file. Ten other files retain exact frozen bytes; protected
authority/native-ingest/manifest/lock inputs remain unchanged. ROOT-INTEGRATION
records the combined routing hash and both forward/reverse applicability.

Source status7 is a one-rank nested delta and a command3 terminal, distinct
from status255 metadata. The client preserves other ranks and members, retains
legal raw UTF16/space names, and completes only current local requester/index/
text/scope context after a new source arrival. Hidden strict >5000ms timeout
allows retry without declaring success. Known stale, expired or unsent local
requests retire only their exact context. Gateway preserves the committed
actor source7 before canonical255; server permissions/durability remain the
authority. The real native FIFO and reset consumer are tested unchanged.

Root passes83 broad client Guild checks (one existing GPU case remains ignored),
4 send-boundary,4 FIFO/reset,3 ordinary Gateway terminal and9 P3 award regression
checks. These overlap the worker's retained final83/4/4/3 and adjacent35/99/16;
they are not additional unique coverage or a parity percentage. Genuine worker
RED7/9 client,1/3 send layer and1/2 Gateway remain. Broad RED79/4/1 includes two
new defects and two declared legacy fixture corrections. The external freeze
harness prefix failure is also preserved; no source/data result was erased.

All public raw runs, inputs and receipts are in raw/. SOURCE-SNAPSHOTS-01.tar.gz
preserves the baseline, RED inputs and final source once, with every member
individually byte-verified by SOURCE-SNAPSHOTS-INDEX.json. Root exact integration,
final checks and raw commands/logs are in root/. INDEX.json binds all copied
raw evidence. Private account stores, fault-store contents and keys are excluded.

Actual native clicks/screenshots/fonts/assets and human acceptance remain open.
Self UiReadModel/AOI rank labels and other rank commands, kick/leave, ordinary
war and Guild kill-EXP remain open. The ordinary wire has no nonce: delayed old
same-content A/B/A replies cannot be uniquely correlated. No new wire or durable
schema is invented. Paired TCP/WSS/build/installer/rollout are not accepted.
R18/source321316 remains its separate immutable pair and does not contain this
later source. No service, current player save or installed game is modified.
''')

scope = '''# Native Guild rank rename Candidate — 2026-10-05

The source/typed consumer/editor and ordinary Gateway terminal slice is
integrated after P3/source513eb83. It preserves partial rank metadata and
matches the actual committed source7 reply to current local editor context.
Hidden strict five-second timeout permits retry; local dispatch failure cleans
only the unsent request. Ordinary wire command3/status7 and durable authority
schemas are unchanged. Full source limits, genuine failed cases, test counts
and exact byte receipts are in the [durable evidence](generated/player-qa/guild-rank-native-20261005/README.md).

Root selected83 client (one existing GPU case ignored),4 send,4 actual FIFO,
3 ordinary Gateway terminal and9 P3 award cases pass. Counts overlap worker
checks and do not measure full parity. Existing notice, bank/lifecycle and
server rank adjacency pass in the worker's frozen sixteen-case selection.

Actual native clicks/rendered labels/screenshots, paired real-network builds
and rollout remain open. Self/AOI rank labels, no-nonce A/B/A ambiguity,
other rank commands, kick/leave, ordinary war and Guild kill-EXP remain separate.
R18/source321316 is unchanged and does not contain this later integration.
'''
write(PROJECT / 'docs/NATIVE-GUILD-RANK-RENAME-20261005.md', scope)
note = '''> 2026-10-05 root integrates the eleven-file native Guild rank-rename slice
> after P3/source513eb83, applying only two Gateway terminal hunks. Source7
> merges its actual nested rank, retires exact current editor context, and is
> preserved before canonical255; hidden strict5s timeout and unsent/stale cleanup
> are checked. Root83 client/4 wire/4 FIFO/3 terminal/9 P3 checks pass, overlapping
> retained worker checks; one existing GPU case stays ignored. Native clicks,
> self/AOI labels, no-nonce ABA, paired network/build/rollout and fullP5 stay open.
> Protected authority/native queue/schemas/locks and R18 artifacts are unchanged.
> [Scope](NATIVE-GUILD-RANK-RENAME-20261005.md),
> [root evidence](generated/player-qa/guild-rank-native-20261005/README.md).

'''
for name in ('AGENT-ORCHESTRATION.md', 'AGENT-TASK-QUEUE.md', 'CRYSTAL-1TO1-ROADMAP.md',
             'BACKEND-1TO1-PROGRESS.md', 'CRYSTAL-SERVER-PARITY.md',
             'CLASSIC-GAMEPLAY-GOAL-20261004.md', 'NATIVE-WINDOWS-PLAYER-QA.md'):
    path = PROJECT / 'docs' / name
    before = path.read_bytes()
    assert note.splitlines()[0].encode() not in before
    split = before.index(b'\n') + 1
    ending = b'\r\n' if b'\r\n' in before else b'\n'
    path.write_bytes(before[:split] + ending + note.encode().replace(b'\n', ending) + before[split:])

probe = BASE / 'download-parallel-probe-20261005-01'
probe_dest = PROJECT / 'docs/generated/player-qa/download-parallel-20261005'
assert not probe_dest.exists()
probe_receipt = json.loads((probe / 'RESULT-01.json').read_bytes())
assert all(row['pass'] for row in probe_receipt['attempts'])
assert len({row['parts'][0]['sha256'] for row in probe_receipt['attempts']}) == 1
probe_index = []
for name in ('probe.py', 'RESULT-01.json', 'connections-1-01.json', 'connections-2-01.json', 'connections-4-01.json'):
    data = (probe / name).read_bytes()
    target = probe_dest / name
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_bytes(data)
    probe_index.append({'file': name, 'bytes': len(data), 'sha256': digest(data)})
write(probe_dest / '.gitattributes', '* -text -diff\nREADME.md text eol=lf diff\n.gitattributes text eol=lf diff\n')
write(probe_dest / 'INDEX.json', json.dumps({'schema': 'mir2.download.root-durable-parallel-evidence.v1', 'files': probe_index,
    'r2OrUpdaterSpeedAcceptance': False, 'serverChanges': False}, indent=2) + '\n')
write(probe_dest / 'README.md', '''# Bounded parallel origin download diagnostic

The exact user r16 Bootstrap URL honors verified-TLS direct HTTP1.1 Range.
Only7MiB total was requested, with at most4 concurrent connections; no full
installer download, proxy use or server/service/save change occurred. Actual
current-development-host aggregates are0.11145MiB/s (1 connection,1MiB,8.973s),
0.09857MiB/s (2,2MiB,20.289s), and0.20438MiB/s (4,4MiB,19.571s). The same first
range hash matches across modes. Short windows vary; these do not establish a
constant per-connection limit, a hosting bandwidth plan, or the other laptop's
speed. They are not actual updater or R2/CDN acceptance.

At the observed four-connection aggregate,620MB would still take roughly48
minutes ignoring overhead. The frozen existing updater uses serial artifacts,
so that estimate is not a claim it reaches the parallel probe rate. Signed
bundle publication and real old-Bootstrap use need separate verification;
Cloudflare/R2 deployment remains dependent on real account authentication.
INDEX.json binds the exact script and all raw outcomes.
''')
print(json.dumps({'exactPublicBlobs': len(index), 'sourceSnapshotMembers': len(source_rows),
                  'sourceArchiveBytes': archive.stat().st_size, 'globalDocs': 7,
                  'probeBlobs': len(probe_index), 'nativeOrHumanAcceptance': False}), flush=True)
