import difflib
import hashlib
import json
from pathlib import Path

proof = Path(__file__).parent
phase = proof / 'updater-real-test-harness-03'
phase.mkdir(exist_ok=False)
test = Path('C:/Users/Administrator/.codex/worktrees/u18/mir2-player-journey/mir2-web3/apps/game-client/windows-updater/tests/real_packages.rs')
before = test.read_bytes()
(phase / 'before.rs').write_bytes(before)
source = before.decode('utf-8')
replacements = [
    ('    let result = update::check_update(&root, &files, &Quiet).unwrap();\n    let expected: Vec<_>',
     '    assert!(!root.join(".update/downloads").exists(), "fresh content cache");\n    let expected: Vec<_>'),
    ('    assert_eq!(result.changed_game_files, expected.len() + removed.len());\n    let engine_change',
     '''    let expected_paths: BTreeSet<_> = expected.iter().copied().collect();
    assert_eq!(expected_paths.len(), expected.len(), "unique changed paths");
    let signed_changed_paths: BTreeSet<_> = new_manifest.files.iter()
        .filter(|entry| old_map.get(entry.path.as_str())
            .is_none_or(|old| old.sha256 != entry.sha256 || old.size != entry.size))
        .map(|entry| entry.path.as_str()).collect();
    assert_eq!(expected_paths, signed_changed_paths, "exact signed changed path set");
    let expected_content_hashes: BTreeSet<_> = expected_paths.iter()
        .map(|name| new_map.get(*name).unwrap().sha256.as_str()).collect();
    let result = update::check_update(&root, &files, &Quiet).unwrap();
    assert_eq!(result.changed_game_files, expected.len() + removed.len());
    let engine_change'''),
    ('expected.len() + usize::from(engine_change)',
     'expected_content_hashes.len() + usize::from(engine_change)'),
    ('''    let expected_game_requests: BTreeSet<_> = expected
        .iter()
        .map(|name| format!("{game_prefix}{name}"))
        .collect();
    let actual_game_requests: BTreeSet<_> = requested
        .iter()
        .filter(|name| name.starts_with(&game_prefix))
        .cloned()
        .collect();
    // Bind every asset and executable request to the actual signed delta.
    assert_eq!(actual_game_requests, expected_game_requests);''',
     '''    let engine_request = format!("{}/Mir2Updater.exe", files.engine_dir);
    let mut actual_content_hashes = BTreeSet::new();
    for request in &requested {
        if let Some(name) = request.strip_prefix(&game_prefix) {
            assert!(expected_paths.contains(name), "only signed changed payload is fetched");
            let hash = new_map.get(name).unwrap().sha256.as_str();
            assert!(actual_content_hashes.insert(hash), "one transfer per content hash");
        } else {
            assert!(engine_change, "unchanged engine must not be fetched");
            assert_eq!(request, &engine_request, "no unbound payload request");
        }
    }
    // Equal-content signed paths reuse the cache; each is still independently
    // checked below in the complete new-manifest and old rollback hash scans.
    assert_eq!(actual_content_hashes, expected_content_hashes);'''),
    ('"removedPayload":removed,"allOldPayloadHashesRestored":true,',
     '"removedPayload":removed,"allOldPayloadHashesRestored":true,\n        "uniqueChangedPayloadContents":expected_content_hashes.len(),"exactChangedRequestHashesVerified":true,'),
]
for old, new in replacements:
    count = source.count(old)
    required = 2 if old == 'expected.len() + usize::from(engine_change)' else 1
    assert count == required, (old[:80], count, required)
    source = source.replace(old, new)
after = source.encode('utf-8')
test.write_bytes(after)
(phase / 'after.rs').write_bytes(after)
(phase / 'diff.patch').write_text(''.join(difflib.unified_diff(before.decode().splitlines(True), source.splitlines(True), fromfile='harness-02.rs', tofile='harness-03.rs')), encoding='utf-8')
receipt = {'schema': 'mir2.updater.real-package-harness-adaptation.v1',
           'testOnly': True, 'productionUpdaterModified': False,
           'beforeSha256': hashlib.sha256(before).hexdigest(),
           'afterSha256': hashlib.sha256(after).hexdigest(),
           'reason': 'Authenticated changed paths can share content SHA; assert unique fetched hashes, every new target hash, exact removals and every old rollback hash.',
           'priorFailureLogsPreserved': ['real-update-r18-01.log', 'real-update-r18-02.log'],
           'gameLaunched': False, 'protectedInstallationTouched': False}
(phase / 'receipt.json').write_text(json.dumps(receipt, indent=2), encoding='utf-8')
prior_config = proof / 'prepare-real-update-fixture-r18-02.py'
config = prior_config.read_text(encoding='utf-8').replace('real-update-fixture-02', 'real-update-fixture-03').replace('real-update-fixture-inputs-02.json', 'real-update-fixture-inputs-03.json').replace('signed-resource-delta-02.json', 'signed-resource-delta-03.json')
with (proof / 'prepare-real-update-fixture-r18-03.py').open('x', encoding='utf-8') as stream:
    stream.write(config)
print(json.dumps(receipt))
