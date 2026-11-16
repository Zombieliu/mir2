import datetime
import hashlib
import json
import os
from pathlib import Path
import subprocess
import time

PROJECT = Path('C:/Users/Administrator/.codex/worktrees/crowded-combat-control/mir2-player-journey/mir2-web3')
OUT = Path(__file__).resolve().parent
selections = [
    ('root-client-guild-01', 'C:/mir2-build/gateway-tests-r51', ['--manifest-path', 'apps/game-client/client-bevy/Cargo.toml', '--features', 'native-ui', '--lib', 'guild_']),
    ('root-wire-rank-01', 'C:/mir2-build/gateway-tests-r51', ['--manifest-path', 'apps/game-client/platform-windows/Cargo.toml', '--bin', 'mir2-platform-windows', 'guild_rank_wire_tests']),
    ('root-fifo-rank-01', 'C:/mir2-build/gateway-tests-r51', ['--manifest-path', 'apps/game-client/runtime/Cargo.toml', '--lib', 'guild_rank_ingest_tests']),
    ('root-gateway-rank-terminal-01', 'C:/mir2-build/gateway-tests-r49', ['-p', 'mir2-gateway', '--test', 'shared_guild_rank_terminal']),
    ('root-gateway-cross-map-after-rank-01', 'C:/mir2-build/gateway-tests-r49', ['-p', 'mir2-gateway', '--lib', 'cross_map_drop_lifecycle_tests']),
]
for label, target, args in selections:
    env = os.environ.copy()
    env['CARGO_TARGET_DIR'] = target
    env['MIR2_P3_RECEIPT_DIR'] = str(OUT)
    command = ['cargo', '+1.95.0', 'test', '--offline', '--locked', '--jobs', '2', *args]
    logfile = OUT / (label + '.log')
    assert not logfile.exists(), f'preserve previous attempt: {logfile}'
    print(json.dumps({'starting': label, 'command': command, 'target': target}), flush=True)
    started = datetime.datetime.now(datetime.timezone.utc).isoformat()
    clock = time.monotonic()
    with logfile.open('wb') as log:
        result = subprocess.run(command, cwd=PROJECT, env=env, stdout=log, stderr=subprocess.STDOUT)
    data = logfile.read_bytes()
    receipt = {'command': command, 'cwd': str(PROJECT), 'target': target, 'jobs': 2,
               'startedUtc': started, 'elapsedSeconds': time.monotonic() - clock,
               'exitCode': result.returncode, 'logSha256': hashlib.sha256(data).hexdigest(),
               'isolatedFaultReceiptDir': str(OUT), 'privateFixtureContentsExported': False,
               'networkOrNativeAcceptance': False, 'overlapsWorkerChecks': True}
    (OUT / (label + '.json')).write_bytes((json.dumps(receipt, indent=2) + '\n').encode())
    print(json.dumps({'completed': label, 'exitCode': result.returncode,
                      'elapsedSeconds': receipt['elapsedSeconds']}), flush=True)
    if result.returncode:
        raise SystemExit(result.returncode)
