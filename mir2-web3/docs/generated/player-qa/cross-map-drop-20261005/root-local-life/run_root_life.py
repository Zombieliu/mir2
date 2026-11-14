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
    ('root-local-life-and-owner-01', ['-p', 'mir2-simulation', '--test', 'shared_experience_owner_tick_life', '--test', 'shared_monster_ownership', '--test', 'shared_cross_map_drop_lifecycle']),
    ('root-cross-map-award-after-life-01', ['-p', 'mir2-gateway', '--lib', 'cross_map_drop_lifecycle_tests']),
]
env = os.environ.copy()
env['CARGO_TARGET_DIR'] = TARGET
env['MIR2_P3_RECEIPT_DIR'] = str(OUT)
for label, args in selections:
    command = ['cargo', '+1.95.0', 'test', '--offline', '--locked', '--jobs', '2', *args]
    logfile = OUT / (label + '.log')
    assert not logfile.exists(), f'preserve previous attempt: {logfile}'
    print(json.dumps({'starting': label, 'command': command}), flush=True)
    started = datetime.datetime.now(datetime.timezone.utc).isoformat()
    clock = time.monotonic()
    with logfile.open('wb') as log:
        result = subprocess.run(command, cwd=PROJECT, env=env, stdout=log, stderr=subprocess.STDOUT)
    data = logfile.read_bytes()
    receipt = {'command': command, 'cwd': str(PROJECT), 'target': TARGET, 'jobs': 2,
               'startedUtc': started, 'elapsedSeconds': time.monotonic() - clock,
               'exitCode': result.returncode, 'logSha256': hashlib.sha256(data).hexdigest(),
               'isolatedFaultReceiptDir': str(OUT), 'privateFixtureContentsExported': False,
               'networkOrNativeAcceptance': False, 'overlapsWorkerChecks': True}
    (OUT / (label + '.json')).write_text(json.dumps(receipt, indent=2) + '\n', encoding='utf-8')
    print(json.dumps({'completed': label, 'exitCode': result.returncode,
                      'elapsedSeconds': receipt['elapsedSeconds']}), flush=True)
    if result.returncode:
        raise SystemExit(result.returncode)
