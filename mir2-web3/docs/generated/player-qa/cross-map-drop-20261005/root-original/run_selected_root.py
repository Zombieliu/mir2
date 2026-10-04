import datetime
import hashlib
import json
import os
from pathlib import Path
import subprocess
import time

PROJECT = Path('C:/Users/Administrator/.codex/worktrees/crowded-combat-control/mir2-player-journey/mir2-web3')
OUT = Path(__file__).resolve().parent
TARGET = 'C:/mir2-build/gateway-tests-r49'
selections = [
    ('root-gateway-cross-map-01', ['-p', 'mir2-gateway', '--lib', 'cross_map_drop_lifecycle_tests']),
    ('root-gateway-death18-01', ['-p', 'mir2-gateway', '--lib', 'dead_experience_chain_tests']),
    ('root-simulation-cross-map-01', ['-p', 'mir2-simulation', '--test', 'shared_cross_map_drop_lifecycle']),
    ('root-simulation-checkpoint-01', ['-p', 'mir2-simulation', '--lib', 'runtime::zone::runtime::checkpoint::tests']),
]
env = os.environ.copy()
env['CARGO_TARGET_DIR'] = TARGET
for label, args in selections:
    command = ['cargo', '+1.95.0', 'test', '--offline', '--locked', '--jobs', '2', *args]
    print(json.dumps({'starting': label, 'command': command, 'target': TARGET}), flush=True)
    started = datetime.datetime.now(datetime.timezone.utc).isoformat()
    clock = time.monotonic()
    logfile = OUT / (label + '.log')
    assert not logfile.exists(), f'preserve existing run: {logfile}'
    with logfile.open('wb') as log:
        result = subprocess.run(command, cwd=PROJECT, env=env, stdout=log, stderr=subprocess.STDOUT)
    data = logfile.read_bytes()
    receipt = {'command': command, 'cwd': str(PROJECT), 'target': TARGET, 'jobs': 2,
               'startedUtc': started, 'elapsedSeconds': time.monotonic() - clock,
               'exitCode': result.returncode, 'logSha256': hashlib.sha256(data).hexdigest(),
               'networkAcceptance': False, 'overlapsWorkerChecks': True}
    (OUT / (label + '.json')).write_text(json.dumps(receipt, indent=2) + '\n', encoding='utf-8')
    print(json.dumps({'completed': label, 'exitCode': result.returncode, 'elapsedSeconds': receipt['elapsedSeconds']}), flush=True)
    if result.returncode:
        raise SystemExit(result.returncode)
