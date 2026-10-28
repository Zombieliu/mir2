"""Freeze the bounded eleven-file delivery only after every scoped run finishes."""
from pathlib import Path
import hashlib
import json
import os
import re
import subprocess
from datetime import datetime, timezone

OUT = Path(__file__).resolve().parent
ROOT = Path('C:/Users/Administrator/.codex/worktrees/gn/mir2-player-journey/mir2-web3')
BASE = '0e22935261beea4240b2dc32e7faf5a75cda7a08'
MODIFIED = [
    'apps/game-client/client-bevy/src/social.rs',
    'apps/game-client/client-bevy/src/crystal_ui/guild_panel.rs',
    'apps/game-client/client-bevy/src/crystal_ui/overlays.rs',
    'apps/game-client/platform-windows/src/gameplay_bridge.rs',
    'apps/game-client/runtime/src/lib.rs',
    'apps/gateway/src/routing.rs',
]
ADDED = [
    'apps/game-client/client-bevy/src/social_guild_rank_tests.rs',
    'apps/game-client/client-bevy/src/crystal_ui/guild_rank_flow_tests.rs',
    'apps/game-client/platform-windows/src/guild_rank_wire_tests.rs',
    'apps/game-client/runtime/src/native_data_path_tests/guild_rank_ingest_tests.rs',
    'apps/gateway/tests/shared_guild_rank_terminal.rs',
]
RUNS = ['client-red-01','gateway-red-01','client-green-01','wire-red-01',
    'client-red-02','client-green-02','wire-green-01','fifo-green-01',
    'gateway-green-01','client-social-adjacent-01','wire-adjacent-01',
    'gateway-guild-adjacent-01']
env = dict(os.environ, GIT_OPTIONAL_LOCKS='0')

def sha(raw):
    return hashlib.sha256(raw).hexdigest()

def git(*arguments):
    return subprocess.check_output(['git','-c','gc.auto=0','-c','status.relativePaths=true',
        *arguments],cwd=ROOT,env=env)

assert not (OUT / 'FROZEN-DELIVERY.json').exists(), 'Never replace a previous freeze.'
assert git('rev-parse','HEAD').decode().strip() == BASE
status = git('status','--porcelain=v1','--untracked-files=all').decode()
repository_root = Path(git('rev-parse','--show-toplevel').decode().strip())
project_prefix = ROOT.relative_to(repository_root).as_posix()+'/'
repository_paths = {line[3:] for line in status.splitlines() if line}
assert all(path.startswith(project_prefix) for path in repository_paths), 'No writes outside the primary project.'
paths = {path[len(project_prefix):] for path in repository_paths}
assert paths == set(MODIFIED + ADDED), ('Unexpected source write set', paths)
git('diff','--check')
baseline = json.loads((OUT / 'baseline-index.json').read_text(encoding='utf-8'))
protected = {}
for item in baseline['files']:
    if item['path'] in MODIFIED:
        continue
    current = (ROOT / item['path']).read_bytes()
    assert sha(current) == item['sha256'], ('Protected source/lock changed',item['path'])
    protected[item['path']] = item['sha256']

old_lib = (OUT / 'baseline-source/mir2-web3/apps/game-client/runtime/src/lib.rs').read_bytes()
new_lib = (ROOT / 'apps/game-client/runtime/src/lib.rs').read_bytes()
declaration = b'\n    #[cfg(test)]\n    mod guild_rank_ingest_tests;\n'
assert new_lib.replace(declaration,b'',1) == old_lib, 'Runtime lib must change only the cfg(test) module declaration.'

receipts = []
for name in RUNS:
    receipt = json.loads((OUT / (name+'-receipt.json')).read_text(encoding='utf-8'))
    raw = (OUT / (name+'.log')).read_bytes()
    assert sha(raw) == receipt['log_sha256']
    assert receipt['sources_unchanged_during_run'], ('Run source changed',name)
    expected_red = name in ['client-red-01','gateway-red-01','wire-red-01','client-red-02']
    assert (receipt['exit_code'] != 0) == expected_red, ('Unexpected test outcome',name)
    assert receipt['test_results'], ('No completed test run',name)
    text = raw.decode('utf-8',errors='replace')
    failures = sorted(set(re.findall(r"^thread '([^']+)' [^\r\n]*panicked",text,re.MULTILINE)))
    passed_names = re.findall(r'^test (\S+) \.\.\. ok\s*$',text,re.MULTILINE)
    receipts.append({'name':name,'test_results':receipt['test_results'],
        'exit_code':receipt['exit_code'],'log_sha256':receipt['log_sha256'],
        'input_sha256':sha((OUT/(name+'-input.json')).read_bytes()),
        'receipt_sha256':sha((OUT/(name+'-receipt.json')).read_bytes()),
        'derived_raw_failures':failures,'derived_passed_test_names':passed_names,
        'duration_seconds':receipt['duration_seconds'],
        'source_sha256_at_run':receipt['source_before_sha256']})

files = []
for relative in MODIFIED + ADDED:
    raw = (ROOT / relative).read_bytes()
    target = OUT / 'final-source/mir2-web3' / relative
    target.parent.mkdir(parents=True,exist_ok=True)
    target.write_bytes(raw)
    previous = next((item for item in baseline['files'] if item['path']==relative),None)
    files.append({'path':relative,'kind':'modified' if relative in MODIFIED else 'new',
        'bytes':len(raw),'sha256':sha(raw),
        'baseline_sha256':previous['sha256'] if previous else None})

patch = git('diff','--no-ext-diff','--',*MODIFIED)
for relative in ADDED:
    raw = (ROOT / relative).read_bytes()
    assert raw.endswith(b'\n') and b'\r' not in raw
    lines = raw.splitlines(keepends=True)
    name = 'mir2-web3/'+relative
    header = (f'diff --git a/{name} b/{name}\nnew file mode 100644\n'
        f'--- /dev/null\n+++ b/{name}\n@@ -0,0 +1,{len(lines)} @@\n').encode()
    patch += header + b''.join(b'+'+line for line in lines)
(OUT/'guild-rank-native-scoped.patch').write_bytes(patch)
routing_patch = git('diff','--no-ext-diff','--','apps/gateway/src/routing.rs')
assert routing_patch == (OUT/'routing-guild-rank-terminal.patch').read_bytes(), 'Routing preview must remain frozen.'
receipt = {'frozen_utc':datetime.now(timezone.utc).isoformat(),'state':'source-frozen',
    'base':BASE,'root':ROOT.as_posix(),'git_status':status,'files':files,
    'source_write_set_count':len(files),'protected_source_and_lock_sha256':protected,
    'runtime_lib_cfg_module_only':True,'native_ingest_production_unchanged':True,
    'raw_red_results_preserved':True,'scoped_runs':receipts,
    'patch_sha256':sha(patch),'routing_patch_sha256':sha(routing_patch),
    'root_integrates_p3_routing_separately':True,
    'open_gates':['native/GPU screenshot or human frontend acceptance',
        'TCP/WSS paired real-server and binary/client rollout',
        'UiReadModel own-character Guild/rank label','AOI ObjectPlayer Guild rank label',
        'other rank commands including status6/status8 and option5 pending',
        'kick/leave/war/Guild kill-EXP closure','protocol has no nonce; old same-content A/B/A replies cannot be uniquely distinguished']}
(OUT/'FROZEN-DELIVERY.json').write_text(json.dumps(receipt,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
print(json.dumps({'state':receipt['state'],'source_write_set_count':len(files),
    'scoped_runs':len(receipts),'patch_sha256':sha(patch),
    'routing_patch_sha256':sha(routing_patch),'receipt_sha256':sha((OUT/'FROZEN-DELIVERY.json').read_bytes())}))
