"""Build only this adapter against immutable original DLLs, wholly offline."""
from pathlib import Path
import argparse
import hashlib
import json
import os
import subprocess
import tempfile
import datetime
import shutil


def sha(path):
    with path.open('rb') as source:
        return hashlib.file_digest(source, 'sha256').hexdigest()


def main():
    parser = argparse.ArgumentParser(__doc__)
    parser.add_argument('--frozen-runtime', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    base = (Path(tempfile.gettempdir()) / 'mir2-comparable-capacity-20261113').resolve()
    output = args.output.resolve()
    if not output.is_relative_to(base) or output == base or output.exists():
        raise ValueError('fresh_output_below_assigned_C_root_required')
    source = Path(__file__).resolve().parent
    runtime = args.frozen_runtime.resolve(strict=True)
    expected = {'Shared.dll': 'fb90224b1ddd836b0bcfc959ddd29a715cc28f8b14552224e5f5a93f87936c25',
                'Server.Library.dll': '8104a1cb313efbe58e68fa2229472c9b3335f2ee75e107b435309a116a7bce62'}
    for name, digest in expected.items():
        if sha(runtime / name) != digest:
            raise ValueError('frozen_runtime_hash_mismatch')
    output.mkdir(parents=True, exist_ok=False)
    source_copy = output / 'source'
    frozen_copy = output / 'frozen-runtime'
    source_copy.mkdir(); frozen_copy.mkdir()
    files = sorted([p for p in source.iterdir() if p.is_file() and p.suffix in {'.cs', '.csproj', '.py', '.Config'}], key=lambda p: p.name)
    for path in files:
        if path.is_symlink():
            raise ValueError('source_symlink_forbidden')
        before = sha(path); shutil.copyfile(path, source_copy / path.name)
        if sha(source_copy / path.name) != before:
            raise ValueError('source_copy_changed')
    for path in sorted(runtime.glob('*.dll')):
        if path.is_symlink():
            raise ValueError('runtime_symlink_forbidden')
        before = sha(path); shutil.copyfile(path, frozen_copy / path.name)
        if sha(frozen_copy / path.name) != before:
            raise ValueError('runtime_copy_changed')
    files = sorted(source_copy.iterdir(), key=lambda p: p.name)
    lock = {'schema': 'mir2.comparable.crystal-source-lock.v1',
            'files': [{'path': p.name, 'bytes': p.stat().st_size, 'sha256': sha(p)} for p in files],
            'runtimeDlls': [{'name': p.name, 'bytes': p.stat().st_size, 'sha256': sha(p)} for p in sorted(frozen_copy.glob('*.dll'))],
            'compileUsesFrozenSourceCopy': True, 'gameSourceCompiled': False}
    lock_path = output / 'source-lock.json'
    lock_path.write_bytes((json.dumps(lock, indent=2) + '\n').encode())
    lock_sha = sha(lock_path)
    env = os.environ.copy()
    for key, child in [('TEMP', 'tmp'), ('TMP', 'tmp'), ('DOTNET_CLI_HOME', 'dotnet-home'), ('NUGET_PACKAGES', 'nuget')]:
        target = output / child; target.mkdir(exist_ok=True); env[key] = str(target)
    env['DOTNET_CLI_TELEMETRY_OPTOUT'] = '1'; env['DOTNET_SKIP_FIRST_TIME_EXPERIENCE'] = '1'
    project = source_copy / 'ComparableCrystal.csproj'
    common = [str(project), '--artifacts-path', str(output / 'artifacts'),
              '-p:FrozenRuntimeDirectory=' + str(frozen_copy), '-p:SourceLockSha256=' + lock_sha]
    commands = [['dotnet', 'restore', *common, '--configfile', str(source_copy / 'NuGet.Config')],
                ['dotnet', 'build', *common, '--no-restore', '--configuration', 'Release']]
    exits = []
    for index, command in enumerate(commands):
        result = subprocess.run(command, cwd=source_copy, env=env, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, check=False)
        (output / f'build-{index}.log').write_bytes(result.stdout); exits.append(result.returncode)
        if result.returncode:
            print(json.dumps({'exitCodes': exits, 'log': str(output / f'build-{index}.log')})); return result.returncode
    binary = output / 'artifacts/bin/ComparableCrystal/release/ComparableCrystal.dll'
    if not binary.is_file():
        raise ValueError('expected_adapter_output_missing')
    for item in lock['files']:
        if sha(source_copy / item['path']) != item['sha256']:
            raise ValueError('frozen_source_changed_during_compile')
    for item in lock['runtimeDlls']:
        if sha(frozen_copy / item['name']) != item['sha256']:
            raise ValueError('frozen_runtime_changed_during_compile')
    if any(sha(p) != expected[p.name] for p in binary.parent.iterdir() if p.name in expected):
        raise ValueError('copied_runtime_dll_changed')
    receipt = {'schema': 'mir2.comparable.crystal-build.v1', 'atUtc': datetime.datetime.now(datetime.timezone.utc).isoformat(),
               'exitCodes': exits, 'sourceLock': str(lock_path), 'sourceLockSha256': lock_sha,
               'adapter': str(binary), 'adapterSha256': sha(binary), 'sourceSnapshot': str(source_copy),
               'frozenRuntimeSnapshot': str(frozen_copy), 'sourceAndRuntimePostBuildHashCheckPassed': True,
               'gameSourceCompiled': False, 'networkPackageSources': []}
    (output / 'build-receipt.json').write_bytes((json.dumps(receipt, indent=2) + '\n').encode())
    print(json.dumps(receipt)); return 0


if __name__ == '__main__':
    raise SystemExit(main())
