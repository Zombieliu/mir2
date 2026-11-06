from pathlib import Path
from datetime import datetime, timezone
import hashlib
import json
import subprocess
import os

OUT = Path(__file__).resolve().parent
ROOT = Path('C:/Users/Administrator/.codex/worktrees/gn/mir2-player-journey/mir2-web3')
BASE = '0e22935261beea4240b2dc32e7faf5a75cda7a08'
files = [
    'apps/game-client/client-bevy/src/social.rs',
    'apps/game-client/client-bevy/src/crystal_ui/guild_panel.rs',
    'apps/game-client/client-bevy/src/crystal_ui/overlays.rs',
    'apps/game-client/platform-windows/src/gameplay_bridge.rs',
    'apps/game-client/runtime/src/lib.rs',
    'apps/game-client/runtime/src/native_ingest.rs',
    'apps/gateway/src/routing.rs',
    'apps/gateway/src/shared_guilds.rs',
    'apps/simulation/src/runtime/shared_guilds.rs',
    'apps/simulation/src/runtime/shared_guild_management.rs',
    'apps/gateway/tests/shared_guild_rank_management.rs',
    'apps/simulation/tests/shared_guild_rank_management.rs',
    'docs/SHARED-GUILD-RANK-RENAME-20261004.md',
    'apps/game-client/client-bevy/Cargo.toml',
    'apps/game-client/client-bevy/Cargo.lock',
    'apps/game-client/runtime/Cargo.toml',
    'apps/game-client/runtime/Cargo.lock',
    'apps/game-client/platform-windows/Cargo.toml',
    'apps/game-client/platform-windows/Cargo.lock',
    'Cargo.toml', 'Cargo.lock',
]
assert not (OUT / 'baseline-index.json').exists()
env = dict(os.environ, GIT_OPTIONAL_LOCKS='0')
head = subprocess.check_output(['git', '-c', 'gc.auto=0', 'rev-parse', 'HEAD'], cwd=ROOT, env=env).decode().strip()
status = subprocess.check_output(['git', '-c', 'gc.auto=0', 'status', '--short'], cwd=ROOT, env=env).decode().strip()
assert head == BASE and not status, (head, status)
entries = []
for relative in files:
    data = (ROOT / relative).read_bytes()
    copy = OUT / 'baseline-source/mir2-web3' / relative
    copy.parent.mkdir(parents=True, exist_ok=True)
    copy.write_bytes(data)
    entries.append({'path': relative, 'bytes': len(data), 'sha256': hashlib.sha256(data).hexdigest()})
record = {'created_at_utc': datetime.now(timezone.utc).isoformat(), 'root': ROOT.as_posix(),
    'base': BASE, 'read_only_git_status': status, 'files': entries,
    'scope': 'Four client production files plus cfg(test) module declaration; routing edit awaits explicit handoff.',
    'old_plan_sha256': hashlib.sha256(Path('C:/mir2-playtest-releases/20261004-native-r18/guild-rank-native-plan-01/PLAN.md').read_bytes()).hexdigest()}
(OUT / 'baseline-index.json').write_text(json.dumps(record, indent=2) + '\n', encoding='utf-8', newline='\n')
print(json.dumps({'base': head, 'clean': not bool(status), 'baseline_files': len(entries)}))
