from pathlib import Path
import datetime, hashlib, json, subprocess

ROOT = Path('C:/Users/Administrator/.codex/worktrees/crowded-combat-control/mir2-player-journey')
ISSUED = Path('C:/Users/Administrator/.codex/worktrees/r2-client-delivery/mir2-player-journey')
WORKER = Path('C:/mir2-playtest-releases/20261004-native-r18/native-updater-progress-worker-01')
BASE = Path(__file__).resolve().parent
REL = Path('mir2-web3/apps/game-client/windows-updater')
REV = '4c60c323aed11829abae6ee5ce6e13c675a3c1c3'
COMMON = 'f3c30036e36441b8e306aaac9b5e8d35488d8f85'
ROOT_REV = 'b86a71c6a6924d8d4ef4296cf4fdc06d9e392155'

def sha(b): return hashlib.sha256(b).hexdigest()
def git(where, *args):
    return subprocess.check_output(['git', '-c', 'gc.auto=0', *args], cwd=where)
def write_new(p, b):
    p.parent.mkdir(parents=True, exist_ok=True)
    with p.open('xb') as f: f.write(b)

assert git(ROOT, 'rev-parse', 'HEAD').decode().strip() == ROOT_REV
assert git(ISSUED, 'rev-parse', 'HEAD').decode().strip() == REV
assert git(ROOT, 'diff', COMMON, 'HEAD', '--', REL.as_posix()) == b''
freeze_raw = (WORKER / 'FROZEN-WORKER-01.json').read_bytes()
assert sha(freeze_raw) == 'dd97b3b41ac0744f8ab2401e99c316cc1ff34b6511cde34e34da97529205fb21'
freeze = json.loads(freeze_raw)
for entry in freeze['files']:
    body = (WORKER / entry['path']).read_bytes()
    assert len(body) == entry['bytes'] and sha(body) == entry['sha256'], entry['path']

# Bring only the issued runtime baseline, not tools or ignored build outputs.
baseline = ['Cargo.lock', 'Cargo.toml', 'src/lib.rs', 'src/update.rs', 'src/delivery.rs', 'src/delivery_tests.rs']
overlay = ['src/update.rs', 'src/delivery.rs', 'src/transaction.rs', 'src/ui.rs', 'src/progress_trace_tests.rs']
changes = {}
issued_inputs = []
tracked = git(ISSUED, 'ls-files', '-z', '--', REL.as_posix()).decode().split('\0')
for name in filter(None, tracked):
    body = (ISSUED / name).read_bytes()
    assert body == git(ISSUED, 'show', f'{REV}:{name}'), name
    issued_inputs.append({'path': name, 'bytes': len(body), 'sha256': sha(body)})
assert len(issued_inputs) == 41
for name in baseline:
    body = (ISSUED / REL / name).read_bytes()
    assert body == git(ISSUED, 'show', f'{REV}:{(REL / name).as_posix()}')
    changes[name] = body
for name in overlay:
    body = (WORKER / 'updater' / name).read_bytes()
    changes[name] = body
planned = []
for name, after in changes.items():
    dest = ROOT / REL / name
    exists = dest.exists()
    before = dest.read_bytes() if exists else None
    tracked_here = bool(git(ROOT, 'ls-files', '--', (REL / name).as_posix()).strip())
    if tracked_here:
        assert before == git(ROOT, 'show', f'HEAD:{(REL / name).as_posix()}'), name
    elif exists:
        assert before == after, f'preserve unrelated untracked file: {name}'
    planned.append({'path': (REL / name).as_posix(), 'beforeBytes': len(before) if exists else None,
                    'beforeSha256': sha(before) if exists else None, 'afterBytes': len(after), 'afterSha256': sha(after)})
    if exists: write_new(BASE / 'before' / name, before)
    write_new(BASE / 'selected-source' / name, after)
receipt = {'schema': 'mir2.updater-progress-root-integration.v1', 'rootBefore': ROOT_REV,
           'issuedRuntimeRevision': REV, 'commonAncestor': COMMON, 'rootUpdaterDiffSinceCommonEmpty': True,
           'baselineRuntimeFiles': baseline, 'workerOverlayFiles': overlay,
           'workerFreezeSha256': sha(freeze_raw), 'workerFrozenFilesRehashed': len(freeze['files']),
           'issuedTrackedPublicInputs': issued_inputs, 'plannedChanges': planned,
           'localPlayerInstallOrGameWritten': False, 'publishedEngineWritten': False,
           'createdUtc': datetime.datetime.now(datetime.timezone.utc).isoformat()}
write_new(BASE / 'INTEGRATION-PLAN-01.json', (json.dumps(receipt, indent=2)+'\n').encode())
for name, after in changes.items():
    dest = ROOT / REL / name
    dest.parent.mkdir(parents=True, exist_ok=True)
    dest.write_bytes(after)
    assert dest.read_bytes() == after
receipt['allSelectedSourceBytesReadBack'] = True
write_new(BASE / 'INTEGRATION-RECEIPT-01.json', (json.dumps(receipt, indent=2)+'\n').encode())
print(json.dumps({'passed': True, 'selectedFiles': len(changes), 'frozenWorkerRehashed': len(freeze['files']), 'issuedTrackedRehashed': len(issued_inputs)}))
