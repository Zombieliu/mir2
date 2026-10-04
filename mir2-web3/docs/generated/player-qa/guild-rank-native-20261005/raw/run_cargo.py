"""Bounded offline Cargo runner; retain raw bytes and per-run source hashes."""
from pathlib import Path
from datetime import datetime, timezone
import hashlib
import json
import os
import subprocess
import sys
import time
import re

OUT = Path(__file__).resolve().parent
ROOT = Path('C:/Users/Administrator/.codex/worktrees/gn/mir2-player-journey/mir2-web3')
name, manifest, *arguments = sys.argv[1:]
assert re.fullmatch(r'[a-z0-9-]+', name)
assert manifest in ['Cargo.toml', 'apps/game-client/client-bevy/Cargo.toml',
    'apps/game-client/runtime/Cargo.toml', 'apps/game-client/platform-windows/Cargo.toml']
assert not (OUT / (name + '.log')).exists(), 'Never overwrite a previous failure or result.'
command = ['cargo', '+1.95.0', 'test', '--manifest-path', manifest,
    '--offline', '--locked', '--jobs', '2', *arguments]
tracked = [
    'apps/game-client/client-bevy/src/social.rs',
    'apps/game-client/client-bevy/src/social_guild_rank_tests.rs',
    'apps/game-client/client-bevy/src/crystal_ui/guild_panel.rs',
    'apps/game-client/client-bevy/src/crystal_ui/guild_rank_flow_tests.rs',
    'apps/game-client/client-bevy/src/crystal_ui/overlays.rs',
    'apps/game-client/platform-windows/src/gameplay_bridge.rs',
    'apps/game-client/platform-windows/src/guild_rank_wire_tests.rs',
    'apps/game-client/runtime/src/lib.rs',
    'apps/game-client/runtime/src/native_data_path_tests/guild_rank_ingest_tests.rs',
    'apps/game-client/runtime/src/native_ingest.rs',
    'apps/gateway/src/routing.rs',
    'apps/gateway/tests/shared_guild_rank_terminal.rs',
    'apps/gateway/src/shared_guilds.rs',
    'apps/simulation/src/runtime/shared_guilds.rs',
    'apps/simulation/src/runtime/shared_guild_management.rs',
]
def source_index():
    return {relative: hashlib.sha256((ROOT / relative).read_bytes()).hexdigest()
        for relative in tracked if (ROOT / relative).is_file()}
before = source_index()
run_record = {'name': name, 'root': ROOT.as_posix(), 'command': command,
    'target_dir': 'C:/mir2-build/gateway-tests-r51', 'source_before_sha256': before,
    'started_utc': datetime.now(timezone.utc).isoformat()}
(OUT / (name + '-input.json')).write_text(json.dumps(run_record, indent=2) + '\n', encoding='utf-8')
env = dict(os.environ, CARGO_TARGET_DIR=run_record['target_dir'], CARGO_BUILD_JOBS='2',
    CARGO_NET_OFFLINE='true', GIT_OPTIONAL_LOCKS='0')
started = time.monotonic()
print(json.dumps({'run': name, 'command': command, 'target_dir': run_record['target_dir']}), flush=True)
with (OUT / (name + '.log')).open('wb') as log:
    result = subprocess.run(command, cwd=ROOT, env=env, stdout=log, stderr=subprocess.STDOUT)
raw = (OUT / (name + '.log')).read_bytes()
text = raw.decode('utf-8', errors='replace')
after = source_index()
run_record.update({'finished_utc': datetime.now(timezone.utc).isoformat(),
    'exit_code': result.returncode, 'duration_seconds': round(time.monotonic() - started, 3),
    'log_sha256': hashlib.sha256(raw).hexdigest(), 'source_after_sha256': after,
    'sources_unchanged_during_run': before == after,
    'test_results': re.findall(r'test result: [^\r\n]+', text),
    'failures': sorted(set(re.findall(r'^---- (.+) stdout ----$', text, re.MULTILINE)
        + re.findall(r"^thread '([^']+)' [^\r\n]*panicked", text, re.MULTILINE)))})
(OUT / (name + '-receipt.json')).write_text(json.dumps(run_record, indent=2) + '\n', encoding='utf-8')
print(json.dumps({key: run_record[key] for key in ['name','exit_code','duration_seconds',
    'test_results','failures','sources_unchanged_during_run','log_sha256']}, ensure_ascii=True), flush=True)
sys.exit(result.returncode)
