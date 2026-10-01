"""Prepare exact r8/r9 fixtures; --run waits for verified release inputs then runs QA.

Writes only this QA task's receipt/config/logs and the separate test-owned tree.
Never launches the game, publishes a feed, or modifies either signed package.
"""
import argparse
import datetime as dt
import hashlib
import json
import os
import re
from pathlib import Path
import subprocess
import sys
import time

BASE = Path('C:/mir2-playtest-releases/20261001-native-r9')
OLD_BASE = Path('C:/mir2-playtest-releases/20261001-native-r8')
OLD_PROJECT = Path('C:/Users/Administrator/.codex/worktrees/capacity-transport/mir2-player-journey/mir2-web3')
NEW_PROJECT = Path('C:/Users/Administrator/.codex/worktrees/r9/mir2-player-journey/mir2-web3')
OLD_SOURCE = '3f5e61533235921369bc13a7760b4a56b0e467e5'
NEW_SOURCE = None
OLD_CANDIDATE = 'WN-CANDIDATE-20261001-invited-08'
NEW_CANDIDATE = 'WN-CANDIDATE-20261001-invited-09'
CONFIG = BASE / 'real-r8-r9-final-fixtures.json'
RECEIPT = BASE / 'real-integration-final-process.json'
OUTPUT = Path('C:/mir2-updater-qa/real-r8-r9-final-v1')
TEST_EXE = Path('C:/mir2-build/windows-updater/debug/deps/real_packages-3894a1e566a733f2.exe')
TEST_NAMES = [
    'real_package_delta_preserves_personal_files_and_first_launch_rollback',
    'real_r6_metadata_matches_native_cms_key_pin_and_rejects_tampering',
]


def require(condition, message):
    if not condition:
        raise RuntimeError(message)


def read_json(path):
    return json.loads(path.read_text(encoding='utf-8-sig'))


def sha256(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest().upper()


def record(receipt, phase):
    receipt['phase'] = phase
    RECEIPT.write_text(json.dumps(receipt, indent=2) + '\n', encoding='utf-8')


def wait_verified(deadline_seconds):
    deadline = time.monotonic() + deadline_seconds
    while True:
        process_path = BASE / 'build-package-process.json'
        package_ready = False
        if process_path.exists():
            process = read_json(process_path)
            require(process.get('source') == NEW_SOURCE and process.get('candidateVersion') == NEW_CANDIDATE,
                    'Authoritative package receipt identity mismatch')
            if process.get('complete') is True and process.get('success') is not True:
                raise RuntimeError('Release pipeline failed: ' + str(process.get('error', process.get('phase'))))
            package_ready = process.get('complete') is True and process.get('success') is True
        release_path = BASE / 'release-process.json'
        if release_path.exists():
            release = read_json(release_path)
            require(release.get('source') == NEW_SOURCE, 'Authoritative release receipt source mismatch')
            if release.get('complete') is True and release.get('success') is not True:
                raise RuntimeError('Release pipeline failed: ' + str(release.get('error', release.get('phase'))))
        verification = BASE / 'updater-bundle-verification.json'
        if verification.exists():
            verified = read_json(verification)
            require(verified.get('passed') is True, 'Updater bundle verification did not pass')
            require(verified.get('sourceRevision') == NEW_SOURCE, 'Verified updater source mismatch')
            require(verified.get('gameSourceRevision') == NEW_SOURCE, 'Verified game source mismatch')
            require(verified.get('gameCandidate') == NEW_CANDIDATE, 'Verified game candidate mismatch')
            if package_ready:
                return
        require(time.monotonic() < deadline, 'Exact updater verification is not yet available')
        time.sleep(min(20, deadline - time.monotonic()))


def verified_bundle(base, source, candidate):
    verified = read_json(base / 'updater-bundle-verification.json')
    require(verified.get('passed') is True and verified.get('sourceRevision') == source
            and verified.get('gameSourceRevision') == source
            and verified.get('gameCandidate') == candidate, 'Exact signed updater verification required')
    bundle = base / 'updater/bundle'
    statement = read_json(bundle / 'UPDATER-BUNDLE.json')
    require(statement['sourceRevision'] == source and statement['gameSourceRevision'] == source
            and statement['gameCandidate'] == candidate, 'Bundle identity mismatch')
    for entry in statement['files']:
        path = bundle / entry['path']
        require(path.stat().st_size == entry['size'] and sha256(path) == entry['sha256'],
                'Verified bundle file changed: ' + entry['path'])
    pointer = (bundle / 'updater/active.txt').read_text(encoding='ascii')
    engine = read_json(bundle / 'updater/engines' / pointer / 'ENGINE.json')
    require(pointer == engine['exeSha256'] == verified['engineSha256'], 'Engine descriptor/pointer mismatch')
    require(engine['sourceRevision'] == source, 'Engine source mismatch')
    return bundle, engine


def verified_candidate(package, source, candidate):
    manifest_path = package / 'PACKAGE-MANIFEST.json'
    manifest = read_json(manifest_path)
    version = read_json(package / 'VERSION.json')
    statement = read_json(package / 'RELEASE-STATEMENT.json')
    attestation = read_json(package / 'BUILD-ATTESTATION.json')
    for item in (version, statement):
        require(item['candidate'] == candidate and item['gitRevision'] == source
                and item['worktreeDirty'] is False, 'Candidate/source/clean build mismatch')
        require(item['packageManifestSha256'] == sha256(manifest_path), 'Manifest binding mismatch')
        require(item['packageManifestAggregateSha256'] == manifest['aggregateSha256'], 'Aggregate binding mismatch')
        require(item['buildAttestationSha256'] == sha256(package / 'BUILD-ATTESTATION.json'), 'Attestation binding mismatch')
    require(version.get('clientOnly') is True, 'The r9 hotfix must be client-only')
    require(statement['versionSha256'] == sha256(package / 'VERSION.json'), 'Version binding mismatch')
    require(attestation['gitRevision'] == source and attestation['worktreeDirty'] is False
            and attestation['worktreeStatusLineCount'] == 0, 'Attested clean source mismatch')
    require(attestation['exeSha256'] == version['exeSha256'] == statement['exeSha256'], 'Executable binding mismatch')
    files = manifest['files']
    require(len(files) == manifest['fileCount'], 'Manifest count mismatch')
    require(len({entry['path'].lower() for entry in files}) == len(files), 'Manifest path collision')
    canonical = ''.join(f"{entry['path']}\t{entry['size']}\t{entry['sha256']}\n"
                        for entry in sorted(files, key=lambda entry: entry['path']))
    require(hashlib.sha256(canonical.encode('ascii')).hexdigest().upper() == manifest['aggregateSha256'],
            'Manifest canonical hash mismatch')
    require(sum(entry['size'] for entry in files) == manifest['totalBytes'], 'Manifest byte count mismatch')
    mapped = {entry['path']: entry for entry in files}
    for name in ('BUILD-ATTESTATION.json', version['exeName']):
        require(sha256(package / name) == mapped[name]['sha256'], 'Payload binding mismatch: ' + name)
    executable = package / version['exeName']
    require(sha256(executable) == attestation['exeSha256']
            and executable.stat().st_size == attestation['exeSizeBytes'], 'Actual executable differs from attestation')
    return manifest


def prepare():
    old_bundle, old_engine = verified_bundle(OLD_BASE, OLD_SOURCE, OLD_CANDIDATE)
    _, new_engine = verified_bundle(BASE, NEW_SOURCE, NEW_CANDIDATE)
    old_package, new_package = OLD_PROJECT / 'dist/r8', NEW_PROJECT / 'dist/r9'
    old_manifest = verified_candidate(old_package, OLD_SOURCE, OLD_CANDIDATE)
    new_manifest = verified_candidate(new_package, NEW_SOURCE, NEW_CANDIDATE)
    require(sha256(new_package / 'BUILD-ATTESTATION.json') == sha256(
        NEW_PROJECT / 'target-attested-windows-candidate/BUILD-ATTESTATION.json'), 'Original build attestation differs')
    engine_root = BASE / 'updater/engine-release'
    require(read_json(engine_root / 'ENGINE.json') == new_engine, 'Feed engine descriptor differs')
    require(sha256(engine_root / 'Mir2Updater.exe') == new_engine['exeSha256'], 'Feed engine hash differs')
    feed = read_json(BASE / 'updater/feed/latest.json')
    require(feed['game']['identity'] == NEW_CANDIDATE, 'Feed candidate differs')
    require(feed['sequence'] == 5 and feed['channel'] == 'invited', 'Exact r9 invited sequence 5 required')
    for group, root in (('game', new_package), ('engine', engine_root)):
        for entry in feed[group]['metadata']:
            path = root / entry['path']
            require(path.stat().st_size == entry['size'] and sha256(path) == entry['sha256'],
                    'Signed feed metadata binding differs: ' + entry['path'])
    old_files = {entry['path']: entry for entry in old_manifest['files']}
    changed_entries = [entry for entry in new_manifest['files']
                       if entry['path'] not in old_files or old_files[entry['path']]['sha256'] != entry['sha256']]
    new_paths = {entry['path'] for entry in new_manifest['files']}
    require(not (set(old_files) - new_paths), 'Unexpected removed payload paths in this client-only hotfix')
    engine_change = old_engine['exeSha256'] != new_engine['exeSha256']
    expected_download_bytes = sum(entry['size'] for entry in changed_entries) + (new_engine['exeSize'] if engine_change else 0)
    config = {
        'oldPackage': str(old_package), 'newPackage': str(new_package), 'bundle': str(old_bundle),
        'feed': str(BASE / 'updater/feed'), 'engine': str(engine_root), 'output': str(OUTPUT),
        'oldSeed': str(old_bundle / 'updater/seed-feed.json'),
        'oldSeedSignature': str(old_bundle / 'updater/seed-feed.p7s'),
        'expectedChangedPayload': [entry['path'] for entry in changed_entries],
        'expectedUnchangedPayload': len(new_manifest['files']) - len(changed_entries),
        'expectedOldCandidate': OLD_CANDIDATE,
        'expectedEngineChange': engine_change,
        'evidence': {
            'oldSource': OLD_SOURCE, 'newSource': NEW_SOURCE, 'newCandidate': NEW_CANDIDATE,
            'feedSequence': feed['sequence'],
            'oldManifestSha256': sha256(old_package / 'PACKAGE-MANIFEST.json'),
            'newManifestSha256': sha256(new_package / 'PACKAGE-MANIFEST.json'),
            'oldAggregateSha256': old_manifest['aggregateSha256'],
            'newAggregateSha256': new_manifest['aggregateSha256'],
            'changedPayload': changed_entries,
            'changedPayloadBytes': sum(entry['size'] for entry in changed_entries),
            'expectedDownloadedPayloadBytes': expected_download_bytes,
            'newEngineBytes': new_engine['exeSize'],
            'removedPayloadPaths': [],
            'oldEngineSha256': old_engine['exeSha256'], 'newEngineSha256': new_engine['exeSha256'],
            'oldPayloadFiles': old_manifest['fileCount'], 'newPayloadFiles': new_manifest['fileCount'],
            'oldVerificationSha256': sha256(OLD_BASE / 'updater-bundle-verification.json'),
            'newVerificationSha256': sha256(BASE / 'updater-bundle-verification.json'),
            'tests': TEST_NAMES,
        },
    }
    require(not OUTPUT.exists(), 'Fresh test output required; prior evidence must be retained')
    if CONFIG.exists():
        require(read_json(CONFIG) == config, 'Existing exact fixture config differs; do not overwrite evidence')
    else:
        with CONFIG.open('x', encoding='utf-8') as stream:
            json.dump(config, stream, indent=2)
            stream.write('\n')
    print(json.dumps(config), flush=True)
    return config


def main():
    global NEW_SOURCE
    parser = argparse.ArgumentParser()
    parser.add_argument('--source-sha', required=True)
    parser.add_argument('--run', action='store_true')
    parser.add_argument('--wait-seconds', type=int, default=0)
    args = parser.parse_args()
    require(re.fullmatch(r'[0-9a-f]{40}', args.source_sha) is not None, 'Exact lowercase source SHA required')
    NEW_SOURCE = args.source_sha
    require(not RECEIPT.exists(), 'Retain prior final receipt; do not restart over evidence')
    require(0 <= args.wait_seconds <= 5400, 'Wait budget must be between 0 and 5400 seconds')
    receipt = {'source': NEW_SOURCE, 'candidateVersion': NEW_CANDIDATE,
               'startedUtc': dt.datetime.now(dt.timezone.utc).isoformat(), 'pid': os.getpid(),
               'complete': False, 'success': False, 'testsStarted': False, 'publicPublished': False}
    record(receipt, 'wait-updater-verification')
    try:
        wait_verified(args.wait_seconds)
        record(receipt, 'derive-exact-manifest-delta')
        config = prepare()
        receipt['fixtureConfigSha256'] = sha256(CONFIG)
        receipt['helperSha256'] = sha256(Path(__file__))
        receipt['newPackage'] = config['newPackage']
        receipt['oldPackage'] = config['oldPackage']
        receipt['output'] = config['output']
        receipt['feedSequence'] = config['evidence']['feedSequence']
        receipt['changedPayloadFiles'] = len(config['expectedChangedPayload'])
        receipt['changedPayloadBytes'] = config['evidence']['changedPayloadBytes']
        if args.run:
            require(TEST_EXE.is_file(), 'The assigned real-package test binary is missing')
            require(not (BASE / 'real-package-integration-final.log').exists(), 'Retain the prior real-test log')
            env = os.environ.copy()
            env['MIR2_UPDATER_REAL_FIXTURES'] = str(CONFIG)
            command = [str(TEST_EXE), '--ignored', '--test-threads=1']
            receipt['testExeSha256'] = sha256(TEST_EXE)
            receipt['testsStarted'] = True
            record(receipt, 'signed-package-delta-and-rollback')
            with (BASE / 'real-package-integration-final.log').open('xb') as log:
                completed = subprocess.run(command, env=env, stdout=log, stderr=subprocess.STDOUT,
                                           cwd=BASE, check=False)
            receipt['testExitCode'] = completed.returncode
            require(completed.returncode == 0, 'Actual signed package integration failed; preserve output')
            log_text = (BASE / 'real-package-integration-final.log').read_text(encoding='utf-8')
            require(all(f'test {name} ... ok' in log_text for name in TEST_NAMES)
                    and '2 passed; 0 failed' in log_text, 'Both exact real-package tests must pass')
            receipt['tests'] = [{'name': name, 'passed': True} for name in TEST_NAMES]
            report = read_json(OUTPUT / 'report.json')
            require(report.get('passed') is True, 'Real package report did not pass')
            result = report['result']
            require(result['candidate'] == NEW_CANDIDATE and result['sequence'] == 5
                    and result['activated'] is True, 'Actual activated candidate/sequence differs')
            require(result['downloadedBytes'] == config['evidence']['expectedDownloadedPayloadBytes'],
                    'Actual payload download bytes differ from exact signed manifest delta')
            require(result['changedGameFiles'] == len(config['expectedChangedPayload'])
                    and result['downloadedFiles'] == len(config['expectedChangedPayload']) + int(config['expectedEngineChange']),
                    'Actual payload download file count differs from manifest delta')
            require(report.get('allNewPayloadHashesVerified') is True
                    and report.get('pendingFirstLaunchRetained') is True
                    and report.get('failedReleaseQuarantined') is True
                    and report.get('engineRollbackVerified') is True, 'Actual update/rollback gates incomplete')
            preserved = {}
            for relative, expected in (('game/logs/player.log', b'saved diagnostic'),
                                       ('game/personal-file.txt', b'personal file')):
                path = OUTPUT / 'installation' / relative
                require(path.read_bytes() == expected, 'Rollback changed the owned personal fixture: ' + relative)
                preserved[relative] = {'bytes': len(expected), 'sha256': sha256(path)}
            receipt['rollbackPreservedFiles'] = preserved
            require(report.get('gameLaunched') is False, 'Owned fixture must never launch the game')
            receipt['gameLaunched'] = False
            receipt['realTestReportSha256'] = sha256(OUTPUT / 'report.json')
            receipt['report'] = report
        receipt['success'] = True
        record(receipt, 'complete' if args.run else 'prepared-only')
        return 0
    except Exception as error:
        receipt['error'] = str(error)
        record(receipt, 'stopped-before-test' if not receipt['testsStarted'] else 'failed-test')
        print(str(error), file=sys.stderr, flush=True)
        return 1
    finally:
        receipt['complete'] = True
        receipt['finishedUtc'] = dt.datetime.now(dt.timezone.utc).isoformat()
        record(receipt, receipt['phase'])


if __name__ == '__main__':
    sys.exit(main())
