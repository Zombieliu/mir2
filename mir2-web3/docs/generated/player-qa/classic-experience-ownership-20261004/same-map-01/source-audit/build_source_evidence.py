"""Read pinned gameplay source and write only this external audit directory."""
from __future__ import annotations

import hashlib
import json
import subprocess
from datetime import datetime, timezone
from pathlib import Path

OUT = Path(__file__).resolve().parent
PROJECT = Path('C:/Users/Administrator/.codex/worktrees/crowded-combat-control/mir2-player-journey/mir2-web3')
CRYSTAL = Path('E:/mir2/Crystal')
BASE = '6032ef8b3e27dd97bad0b20c8676ef9f185db64b'

# These are gameplay source only; do not read credentials, stores, or service files.
SPANS = {
    'shared': {
        'apps/simulation/src/runtime/zone/types.rs': [(353, 450), (989, 1153), (1182, 1240)],
        'apps/simulation/src/runtime/zone/runtime.rs': [(2484, 2535), (5210, 5240), (9359, 9489), (9754, 9770), (9847, 9867), (9967, 10002), (10162, 10219), (11549, 11561), (11900, 11924), (12004, 12019), (12304, 12537), (12559, 12592), (13507, 13516), (14335, 14361), (15290, 15303), (16691, 16875)],
        'apps/simulation/src/runtime/zone/manager.rs': [(25, 34), (336, 341), (424, 496)],
        'apps/simulation/src/runtime/zone/runtime/checkpoint.rs': [(26, 40), (181, 208), (621, 656), (733, 753), (789, 819)],
        'apps/simulation/src/runtime/drops.rs': [(60, 65), (1595, 1603), (1657, 1665), (3404, 3418), (3535, 3554), (3604, 3620)],
        'apps/simulation/src/runtime/zone/experience.rs': [(1, 103)],
        'apps/simulation/src/runtime/shared_kill_experience.rs': [(28, 177)],
        'apps/simulation/src/runtime/prepared_kill_experience.rs': [(65, 140)],
        'apps/simulation/src/runtime/zone/runtime/spider_ai.rs': [(133, 153), (654, 722)],
        'apps/simulation/src/runtime/zone/runtime/hell_ai.rs': [(109, 140), (364, 405)],
        'apps/simulation/src/runtime/zone/runtime/horned_encounter_ai.rs': [(94, 125), (1108, 1148)],
        'apps/simulation/src/runtime/zone/runtime/hugger_ai.rs': [(73, 90), (322, 365)],
        'apps/simulation/src/runtime/zone/runtime/kirin_snow_ai.rs': [(44, 63)],
        'apps/simulation/src/runtime/zone/runtime/mud_boulder_ai.rs': [(115, 132)],
        'apps/simulation/src/runtime/zone/runtime/thunder_ai.rs': [(158, 184)],
        'apps/simulation/src/runtime/zone/runtime/vampire_ai.rs': [(448, 466), (504, 531)],
        'apps/gateway/src/routing.rs': [(1697, 1730), (4476, 4490), (4742, 4766), (7350, 7356), (10865, 10948), (9440, 9464), (9547, 9559)],
        'apps/simulation/tests/shared_zone.rs': [(10621, 10662), (10878, 10928), (10973, 10995), (11063, 11082)],
    },
    'crystal': {
        'Server/MirObjects/MapObject.cs': [(66, 67), (153, 155), (184, 185), (204, 226), (339, 375), (780, 800)],
        'Server/MirObjects/MonsterObject.cs': [(602, 603), (966, 1015), (1093, 1185), (1192, 1198), (1464, 1509), (1596, 1607), (2573, 2633), (2686, 2737)],
        'Server/MirObjects/HumanObject.cs': [(219, 220), (675, 683), (2365, 2380), (6211, 6219), (7186, 7199), (7285, 7301)],
        'Server/MirObjects/PlayerObject.cs': [(266, 279), (400, 407), (599, 644), (794, 903), (3447, 3458), (4376, 4390), (7519, 7539)],
        'Server/MirObjects/ItemObject.cs': [(129, 142)],
        'Server/MirObjects/Monsters/WoomaTaurus.cs': [(1, 15)],
        'Server/MirObjects/Monsters/ZumaTaurus.cs': [(1, 16)],
        'Server/MirObjects/Monsters/EvilMir.cs': [(152, 181)],
        'Server/Settings.cs': [(16, 17), (94, 99), (112, 113), (228, 230)],
        'Shared/Globals.cs': [(34, 38)],
        'Shared/Functions/Functions.cs': [(71, 75)],
    },
}

records = []
excerpts = []
for source, files in SPANS.items():
    for relative, spans in files.items():
        if source == 'shared':
            blob = subprocess.check_output(['git', '-C', str(PROJECT), 'show', f'{BASE}:mir2-web3/{relative}'])
            path = PROJECT / relative
            acquisition = f'git object {BASE}:mir2-web3/{relative}'
        else:
            path = CRYSTAL / relative
            blob = path.read_bytes()
            acquisition = 'local comparison source, read only'
        lines = blob.decode('utf-8-sig').splitlines()
        for start, end in spans:
            if start < 1 or start > end or end > len(lines):
                raise ValueError(f'Invalid evidence span: {relative}:{start}-{end}')
            excerpts.append(f'\n[{source}] {path.as_posix()}:{start}-{end}\n{acquisition}\n')
            excerpts.extend(f'{number}: {lines[number - 1]}\n' for number in range(start, end + 1))
        records.append({
            'source': source,
            'absolute_path': path.as_posix(),
            'relative_path': relative,
            'acquisition': acquisition,
            'sha256': hashlib.sha256(blob).hexdigest(),
            'byte_length': len(blob),
            'line_count': len(lines),
            'ranges': [{'start': start, 'end': end} for start, end in spans],
        })

index = {
    'kind': 'read-only-source-audit',
    'captured_at_utc': datetime.now(timezone.utc).isoformat(),
    'shared_revision': BASE,
    'shared_source_prefix': 'mir2-web3/',
    'shared_root': PROJECT.as_posix(),
    'crystal_root': CRYSTAL.as_posix(),
    'tests_run': False,
    'builds_run': False,
    'live_state_accessed': False,
    'repository_files_written': False,
    'source_files': records,
}
OUT.joinpath('source-excerpts.txt').write_text(''.join(excerpts), encoding='utf-8')
OUT.joinpath('source-index.json').write_text(json.dumps(index, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
receipt = {
    'kind': 'source-evidence-capture',
    'source_file_count': len(records),
    'source_span_count': sum(len(entry['ranges']) for entry in records),
    'outputs': {},
}
for name in ['README.md', 'build_source_evidence.py', 'source-index.json', 'source-excerpts.txt']:
    blob = OUT.joinpath(name).read_bytes()
    receipt['outputs'][name] = {'sha256': hashlib.sha256(blob).hexdigest(), 'bytes': len(blob)}
OUT.joinpath('evidence-receipt.json').write_text(json.dumps(receipt, indent=2) + '\n', encoding='utf-8')
print(json.dumps({'status': 'captured source evidence only', 'files': len(records), 'spans': receipt['source_span_count'], 'directory': OUT.as_posix()}))
