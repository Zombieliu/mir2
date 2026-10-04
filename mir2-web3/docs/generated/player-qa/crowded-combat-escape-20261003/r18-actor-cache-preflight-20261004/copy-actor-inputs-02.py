import hashlib, json, os, pathlib, subprocess

ROOT = pathlib.Path('C:/Users/Administrator/.codex/worktrees/r18/mir2-player-journey/mir2-web3')
PHASE = pathlib.Path('C:/mir2-playtest-releases/20261004-native-r17/map-implementation/phase2-208-runtime-209-resources')
SOURCE = PHASE / 'actor-input/original-ui'
DEST = ROOT / 'apps/web/public/original-ui'
OUT = pathlib.Path('C:/mir2-playtest-releases/20261004-native-r18/actor-input-copy-02.json')
REVISION = '57729ba8c1473ca920a8ea8b3d6e11105d9d4d93'
sha = lambda data: hashlib.sha256(data).hexdigest()
git = lambda *args, **kw: subprocess.run(['git', '-c', 'gc.auto=0', *args], cwd=ROOT, check=True, capture_output=True, **kw).stdout
assert not OUT.exists()
assert git('rev-parse', 'HEAD').decode().strip() == REVISION
assert not git('status', '--porcelain=v1', '--untracked-files=all').strip()
receipt = json.loads((PHASE / 'implementation-receipt.json').read_text(encoding='utf-8-sig'))
for item in [receipt['actualResources']['frameSetCatalog'], receipt['actualResources']['actor049']['metadata']]:
    data = pathlib.Path(item['path']).read_bytes()
    assert len(data) == item['bytes'] and sha(data) == item['sha256']
# This historical aggregate exporter index is tracked, differs only in the
# newer full export, and is neither consumed nor shipped by this packager.
# Do not replace tracked source to populate a generated resource cache.
excluded = SOURCE / 'manifest.generated.json'
package_script = ROOT / 'apps/game-client/platform-windows/scripts/package-windows-candidate.ps1'
assert 'manifest.generated.json' not in package_script.read_text(encoding='utf-8-sig')
assert sha((DEST / 'frame-sets.generated.json').read_bytes()) == '280d4fee9de0864218611f54014b4bf3487c4478a39133c25ebe211d223e62e9'
excluded_identity = {'relative': excluded.name, 'sourceSha256': sha(excluded.read_bytes()), 'retainedTrackedSha256': sha((DEST / excluded.name).read_bytes()), 'reason': 'not a Candidate input or shipped file; retain tracked source'}
files = [p for p in SOURCE.rglob('*') if p.is_file() and p != excluded]
rows = []
for p in files:
    assert not p.is_symlink() and not os.path.isjunction(p)
    rel = p.relative_to(SOURCE); target = DEST / rel
    assert target.resolve().is_relative_to(DEST.resolve())
    cursor = target.parent
    while cursor != ROOT.parent:
        assert not cursor.is_symlink() and not os.path.isjunction(cursor)
        cursor = cursor.parent
    data = p.read_bytes()
    rows.append((target, data, {'relative': rel.as_posix(), 'bytes': len(data), 'sha256': sha(data)}))
paths = [target.relative_to(ROOT).as_posix() for target, _, _ in rows]
result = subprocess.run(['git', '-c', 'gc.auto=0', 'check-ignore', '--stdin', '-z'], cwd=ROOT, input=('\0'.join(paths) + '\0').encode(), capture_output=True)
assert result.returncode in (0, 1)
ignored = set(result.stdout.decode().rstrip('\0').split('\0'))
for target, data, _ in rows:
    if target.relative_to(ROOT).as_posix() not in ignored:
        assert target.is_file() and target.read_bytes() == data, 'Refuse changed non-ignored actor input: ' + str(target)
changed = 0
for target, data, _ in rows:
    if not target.exists() or target.read_bytes() != data:
        target.parent.mkdir(parents=True, exist_ok=True); target.write_bytes(data); changed += 1
    assert sha(target.read_bytes()) == sha(data)
assert not git('status', '--porcelain=v1', '--untracked-files=all').strip()
payload = {'schema': 'mir2.r18.physical-source-actor-input-copy.v2', 'sourceReceiptSha256': sha((PHASE / 'implementation-receipt.json').read_bytes()), 'exactSourceRevision': REVISION, 'actorSource': str(SOURCE), 'target': str(DEST), 'files': [row for _, _, row in rows], 'fileCount': len(rows), 'bytes': sum(row['bytes'] for _, _, row in rows), 'filesAddedOrReplaced': changed, 'excluded': [excluded_identity], 'trackedSourceClean': True, 'protectedInstallationChanged': False, 'priorAttempt': {'helper': 'copy-actor-inputs-01.py', 'exitCode': 1, 'phase': 'preflight before any copies', 'reason': 'refused aggregate tracked exporter manifest difference'}}
OUT.write_text(json.dumps(payload, indent=2) + '\n', encoding='utf-8')
print(json.dumps({k: payload[k] for k in ['fileCount', 'bytes', 'filesAddedOrReplaced', 'trackedSourceClean']}))
