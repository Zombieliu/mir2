"""Update the closed F-drive installation with the verified native HTTPS engine.

Keeps its first-game-launch health obligation pending; no login or game starts.
"""
import datetime, hashlib, json, os, pathlib, subprocess

TASK = pathlib.Path(__file__).resolve().parent
INSTALL = pathlib.Path('F:/mir2/Mir2Invite')
NEW = pathlib.Path('C:/Users/Administrator/.codex/worktrees/r9/mir2-player-journey/mir2-web3/dist/r9')
SOURCE = '180b01d46f35a5e97f4c60ee2ae033c7b53717e5'
CANDIDATE = 'WN-CANDIDATE-20261001-invited-09'
SHELL = pathlib.Path(subprocess.check_output(['where.exe', 'pwsh'], text=True).splitlines()[0])

def read(path):
    return json.loads(path.read_text(encoding='utf-8-sig'))

def sha(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest().upper()

def require(ok, message):
    if not ok:
        raise RuntimeError(message)

def regular(path):
    require(path.is_file() and not path.is_symlink() and not path.is_junction(), 'Plain native file required')
    for parent in path.parents:
        require(not parent.is_symlink() and not parent.is_junction(), 'Native path redirects rejected')

def events():
    path = INSTALL / '.update/last-run.jsonl'
    return [json.loads(line) for line in path.read_text(encoding='utf-8').splitlines()] if path.exists() else []

def hidden_run(exe, arg, timeout):
    startup = subprocess.STARTUPINFO()
    startup.dwFlags |= subprocess.STARTF_USESHOWWINDOW
    startup.wShowWindow = subprocess.SW_HIDE
    result = subprocess.run([str(exe), arg], cwd=INSTALL, timeout=timeout,
                            creationflags=subprocess.CREATE_NO_WINDOW, startupinfo=startup)
    require(result.returncode == 0, f'Native {arg} failed ({result.returncode})')

def scratch_engine(engine_id):
    require(len(engine_id) == 64 and all(c in '0123456789ABCDEF' for c in engine_id), 'Native engine pointer invalid')
    engine = INSTALL / 'updater/engines' / engine_id / 'Mir2Updater.exe'
    scratch = INSTALL / '.update' / f'engine-{engine_id}.exe'
    regular(engine)
    require(sha(engine) == engine_id, 'Native active engine does not match signed pointer')
    if not scratch.exists():
        with scratch.open('xb') as stream:
            stream.write(engine.read_bytes())
    regular(scratch)
    require(sha(scratch) == engine_id, 'Native scratch engine identity differs')
    return scratch

def preferences():
    result = {}
    for variable in ('APPDATA', 'LOCALAPPDATA'):
        directory = pathlib.Path(os.environ[variable]) / 'mir2-web3'
        if directory.is_dir():
            for path in directory.iterdir():
                if path.is_file() and path.suffix.lower() in ('.json', '.bak'):
                    regular(path)
                    result[str(path)] = sha(path)
    return result

def main():
    report_path = TASK / 'actual-native-https-update-report.json'
    require(not report_path.exists(), 'Fresh native update report required')
    published = read(TASK / 'publish-process.json')
    require(published.get('complete') and published.get('success') and published.get('sequence') == 5 and
            published.get('source') == SOURCE, 'Exact sequence-five publication required')
    busy = subprocess.check_output([str(SHELL), '-NoProfile', '-Command',
        "@(Get-CimInstance Win32_Process | Where-Object {$_.ExecutablePath -and $_.ExecutablePath.StartsWith('F:\\mir2\\Mir2Invite\\',[StringComparison]::OrdinalIgnoreCase)}).Count"], text=True).strip()
    require(busy == '0', 'User installation is busy; preserve its normal exit')
    launcher = INSTALL / 'Mir2Launcher.exe'
    regular(launcher)
    before = read(INSTALL / 'game/VERSION.json')
    require(before.get('candidate') == 'WN-CANDIDATE-20261001-invited-08', 'Expected installed r8 required')
    require(sha(INSTALL / 'game/mir2-platform-windows.exe') == before['exeSha256'], 'Installed r8 identity changed')
    old_manifest = {f['path']: f for f in read(INSTALL / 'game/PACKAGE-MANIFEST.json')['files']}
    new_manifest = read(NEW / 'PACKAGE-MANIFEST.json')['files']
    changed = [f['path'] for f in new_manifest if f['path'] not in old_manifest or
               f['sha256'] != old_manifest[f['path']]['sha256']]
    prefs = preferences()
    hidden_run(launcher, '--verify-only', 120)
    engine_id = (INSTALL / 'updater/active.txt').read_text(encoding='ascii').strip()
    scratch = scratch_engine(engine_id)
    update_count = sum(e['event'] == 'update' for e in events())
    hidden_run(scratch, '--check-only', 900)
    first_events = events()
    updates = [e['detail'] for e in first_events if e['event'] == 'update']
    require(len(updates) == update_count + 1, 'Native first update receipt missing')
    first = updates[-1]
    require(first['candidate'] == CANDIDATE and first['sequence'] == 5 and first['activated'] and
            first['changedGameFiles'] == len(changed), 'Native delta activation differs')
    hidden_run(launcher, '--verify-only', 120)
    version = read(INSTALL / 'game/VERSION.json')
    require(version['candidate'] == CANDIDATE and version['gitRevision'] == SOURCE, 'Actual installed source differs')
    for name in changed + ['VERSION.json', 'PACKAGE-MANIFEST.json', 'RELEASE-STATEMENT.json', 'RELEASE-STATEMENT.p7s']:
        require(sha(INSTALL / 'game' / name) == sha(NEW / name), 'Installed delta byte mismatch: ' + name)
    current_engine = (INSTALL / 'updater/active.txt').read_text(encoding='ascii').strip()
    verified_bundle = read(TASK / 'updater-bundle-verification.json')
    require(verified_bundle['passed'] and verified_bundle['sourceRevision'] == SOURCE and
            current_engine == verified_bundle['engineSha256'], 'Installed engine differs from verified r9 bundle')
    require(first['downloadedFiles'] == len(changed) + int(current_engine != engine_id), 'Native download count differs')
    scratch = scratch_engine(current_engine)
    hidden_run(scratch, '--check-only', 900)
    final_events = events()
    final_updates = [e['detail'] for e in final_events if e['event'] == 'update']
    require(len(final_updates) == update_count + 2, 'Native repeated-check receipt missing')
    second = final_updates[-1]
    require(second['candidate'] == CANDIDATE and second['sequence'] == 5 and
            second['downloadedBytes'] == 0 and second['downloadedFiles'] == 0, 'Repeat check downloaded payload')
    hidden_run(launcher, '--verify-only', 120)
    require(preferences() == prefs, 'Per-user language/display/control preferences changed')
    report = dict(schema='mir2.installed-native-https-update.v1', passed=True, candidate=CANDIDATE,
        sourceRevision=SOURCE, sequence=5, actualInstallation=str(INSTALL), firstUpdate=first,
        secondUpdate=second, changedPayload=changed, installedExeSha256=sha(INSTALL/'game/mir2-platform-windows.exe'),
        nativeEngineVerified=True, nativeLauncherVerification=True, realNativeHttps=True,
        engineChanged=(current_engine != engine_id), preferenceFilesVerified=len(prefs), preferencesPreserved=True,
        gameStarted=False, authenticatedGuiAccepted=False, firstLaunchHealthObligation='pending',
        timestampUtc=datetime.datetime.now(datetime.timezone.utc).isoformat())
    with report_path.open('x', encoding='utf-8') as stream:
        json.dump(report, stream, indent=2)
        stream.write('\n')
    print(json.dumps(report))

if __name__ == '__main__':
    main()
