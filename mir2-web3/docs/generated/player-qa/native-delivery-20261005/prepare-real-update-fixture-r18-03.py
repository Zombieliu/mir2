import hashlib, json
from pathlib import Path

root = Path(__file__).parent
prior = Path('C:/mir2-playtest-releases/20261004-native-r17/updater')
old = Path('C:/Users/Administrator/.codex/worktrees/r17/mir2-player-journey/mir2-web3/dist/r17')
new = Path('C:/Users/Administrator/.codex/worktrees/r18/mir2-player-journey/mir2-web3/dist/r18')
assert json.loads((new / 'VERSION.json').read_bytes())['gitRevision'] == '321316b791120a6fe549e4006623e63333a5543f'
old_manifest = json.loads((old / 'PACKAGE-MANIFEST.json').read_bytes())
new_manifest = json.loads((new / 'PACKAGE-MANIFEST.json').read_bytes())
old_entries = {entry['path']: entry for entry in old_manifest['files']}
changed = [entry for entry in new_manifest['files'] if entry['path'] not in old_entries or entry['sha256'] != old_entries[entry['path']]['sha256']]
assert any(entry['path'].startswith('mir2-assets/') for entry in changed), 'R18 includes the verified 209-map and Monster049 resource delta'
feed = json.loads((root / 'updater/feed/latest.json').read_bytes())
source_map = json.loads((root / 'updater/source-map.json').read_bytes())
assert feed['sequence'] == 13 and feed['game']['identity'] == 'WN-CANDIDATE-20261004-invited-18'
engine_root = Path(source_map[feed['engine']['directory']])
assert hashlib.sha256((engine_root / 'Mir2Updater.exe').read_bytes()).hexdigest() == 'fb3505d6d84d997cdbcef17f986a39b930c3fce5d108ca9831d90c5f69b16734'
config = {
    'oldPackage': str(old), 'newPackage': str(new),
    'bundle': str(prior / 'bundle'), 'feed': str(root / 'updater/feed'),
    'engine': str(engine_root),
    'oldSeed': str(prior / 'feed/latest.json'), 'oldSeedSignature': str(prior / 'feed/latest.p7s'),
    'output': str(root / 'real-update-fixture-03'),
    'expectedChangedPayload': [entry['path'] for entry in changed],
    'expectedEngineChange': False,
    'expectedRemovedPayload': sorted(set(old_entries) - {entry['path'] for entry in new_manifest['files']}),
    'expectedUnchangedPayload': len(new_manifest['files']) - len(changed),
    'expectedOldCandidate': 'WN-CANDIDATE-20261004-invited-17',
}
with (root / 'real-update-fixture-inputs-03.json').open('x', encoding='utf-8') as f:
    json.dump(config, f, indent=2)
with (root / 'signed-resource-delta-03.json').open('x', encoding='utf-8') as f:
    json.dump({'changedPayload': changed, 'unchangedPayloadCount': config['expectedUnchangedPayload'], 'changedPayloadBytes': sum(entry['size'] for entry in changed), 'engineUnchanged': True, 'newMetadataBytes': sum(e['size'] for e in feed['game']['metadata']) + sum(e['size'] for e in feed['engine']['metadata']), 'fileLevelDelta': True, 'binaryDifferencePatch': False}, f, indent=2)
print(json.dumps({'changedPayloadCount': len(changed), 'changedPayloadBytes': sum(entry['size'] for entry in changed), 'unchangedPayloadCount': config['expectedUnchangedPayload'], 'engineUnchanged': True}))
