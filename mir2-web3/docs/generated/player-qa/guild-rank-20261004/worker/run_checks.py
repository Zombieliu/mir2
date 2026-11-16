"""Scoped offline Cargo checks, retaining complete raw output and actual input byte hashes."""
from pathlib import Path
from datetime import datetime, timezone
import hashlib
import json
import os
import subprocess
import sys

sys.stdout.reconfigure(encoding='utf-8', errors='replace')

HERE = Path(__file__).resolve().parent
ROOT = Path('C:/Users/Administrator/.codex/worktrees/p5/mir2-player-journey/mir2-web3')
TARGET = 'C:/mir2-build/gateway-tests-r51'
PHASE = sys.argv[1]
ALLOWED = {
    'gateway-red-01': ['-p', 'mir2-gateway', '--test', 'shared_guild_rank_management', '--', '--nocapture', '--test-threads=1'],
    'gateway-green-01': ['-p', 'mir2-gateway', '--test', 'shared_guild_rank_management', '--', '--nocapture', '--test-threads=1'],
    'simulation-green-01': ['-p', 'mir2-simulation', '--features', 'test-support', '--test', 'shared_guild_rank_management', '--', '--nocapture', '--test-threads=1'],
    'guild-adjacent-01': ['-p', 'mir2-gateway', '--test', 'shared_guild_management', '--test', 'guild_shared_lifecycle', '--', '--nocapture', '--test-threads=1'],
}
if PHASE not in ALLOWED:
    raise SystemExit('Unknown focused check')
command = ['cargo', '+1.95.0', 'test', '--offline', '--locked'] + ALLOWED[PHASE]
env = os.environ.copy()
env['CARGO_TARGET_DIR'] = TARGET
env['CARGO_BUILD_JOBS'] = '2'
watched = [
    'apps/simulation/src/runtime/shared_guilds.rs',
    'apps/simulation/src/runtime/shared_guild_management.rs',
    'apps/gateway/src/shared_guilds.rs',
    'apps/simulation/tests/shared_guild_rank_management.rs',
    'apps/gateway/tests/shared_guild_rank_management.rs',
]
inputs = {rel: {'sha256': hashlib.sha256((ROOT/rel).read_bytes()).hexdigest(), 'bytes': (ROOT/rel).stat().st_size} for rel in watched if (ROOT/rel).exists()}
if PHASE == 'gateway-red-01':
    (HERE/'gateway-red-input.rs').write_bytes((ROOT/'apps/gateway/tests/shared_guild_rank_management.rs').read_bytes())
record = {'phase': PHASE, 'command': command, 'cwd': ROOT.as_posix(), 'target': TARGET, 'jobs': 2, 'started_utc': datetime.now(timezone.utc).isoformat(), 'inputs': inputs}
receipt = HERE/f'{PHASE}.json'
receipt.write_text(json.dumps(record,indent=2)+'\n',encoding='utf-8')
print(json.dumps({'phase': PHASE, 'status': 'running', 'log': (HERE/f'{PHASE}.log').as_posix()},indent=2), flush=True)
with (HERE/f'{PHASE}.log').open('wb') as log:
    result = subprocess.run(command, cwd=ROOT, env=env, stdout=log, stderr=subprocess.STDOUT)
record.update({'exit_code': result.returncode, 'ended_utc': datetime.now(timezone.utc).isoformat(), 'log_sha256': hashlib.sha256((HERE/f'{PHASE}.log').read_bytes()).hexdigest(), 'source_unchanged_during_run': all((ROOT/rel).exists() and hashlib.sha256((ROOT/rel).read_bytes()).hexdigest()==value['sha256'] for rel,value in inputs.items())})
receipt.write_text(json.dumps(record,indent=2)+'\n',encoding='utf-8')
print(json.dumps(record,indent=2),flush=True)
print((HERE/f'{PHASE}.log').read_text(encoding='utf-8',errors='replace')[-12000:], flush=True)
raise SystemExit(result.returncode)
