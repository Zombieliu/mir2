"""Bind a newly launched modern adapter to its frozen, local source bundle."""
import hashlib
import json
from pathlib import Path
import re


def _file(value, maximum):
    path = Path(value)
    if not path.is_absolute() or path.drive.upper() != 'C:' or '..' in path.parts:
        raise ValueError('Source binding requires an absolute C path')
    for part in (path, *path.parents):
        stat = part.lstat()
        if part.is_symlink() or getattr(stat, 'st_file_attributes', 0) & 0x400:
            raise ValueError('Source binding rejects reparse points')
    if not path.is_file() or not 0 < path.stat().st_size <= maximum:
        raise ValueError('Source binding file size is outside its declared bound')
    return path.resolve(strict=True)


def _sha(path):
    digest = hashlib.sha256()
    with path.open('rb') as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b''):
            digest.update(chunk)
    return digest.hexdigest()


def _option(command, name, required=True):
    indices = [i for i, arg in enumerate(command) if arg == name]
    if not indices and not required:
        return None
    if len(indices) != 1 or indices[0] + 1 >= len(command):
        raise ValueError('Source binding needs one explicit adapter option')
    return command[indices[0] + 1]


def adapter_sources_proof(spec):
    """Return public hashes; legacy stages without this opt-in retain their API."""
    driver = spec['driver']
    if 'adapterSources' not in driver:
        return None
    if spec['engine'] != 'modern':
        raise ValueError('The JS source bundle contract is modern-only')
    command = driver['command']
    manifest_path = _file(driver['adapterSources'], 1024 * 1024)
    if _file(_option(command, '--adapter-sources'), 1024 * 1024) != manifest_path:
        raise ValueError('Adapter command and declared manifest differ')
    manifest = json.loads(manifest_path.read_text(encoding='utf-8'))
    if manifest.get('schema') != 'mir2.comparable.modern-adapter-sources.v1':
        raise ValueError('Unknown modern adapter source manifest')
    entry = manifest.get('entrypoint')
    if entry not in ('driver.mjs', 'melee-driver.mjs'):
        raise ValueError('Unknown modern adapter entrypoint')
    rows = manifest.get('files')
    if not isinstance(rows, list) or not 2 <= len(rows) <= 16:
        raise ValueError('Bounded adapter source file list required')
    seen, files = set(), {}
    for row in rows:
        name = row.get('path')
        if not isinstance(name, str) or Path(name).name != name or '\\' in name or ':' in name or not name.endswith('.mjs') or name.casefold() in seen:
            raise ValueError('Distinct local MJS basenames required')
        seen.add(name.casefold())
        source = _file(manifest_path.parent / name, 2 * 1024 * 1024)
        if type(row.get('bytes')) is not int or source.stat().st_size != row['bytes'] or _sha(source) != row.get('sha256'):
            raise ValueError('Adapter source bytes do not match manifest')
        files[name] = source
    required = {'driver.mjs', 'contract.mjs'}
    if entry == 'melee-driver.mjs':
        required |= {'melee-driver.mjs', 'combat.mjs'}
    if not required.issubset(files):
        raise ValueError('Missing required adapter source dependency')
    for source in files.values():
        text = source.read_text(encoding='utf-8')
        for _quote, relative in re.findall(r'''(?:\bfrom\s*|\bimport\s*\(?\s*)(['"])(\.[^'"]+)\1''', text):
            if not relative.startswith('./') or relative[2:] not in files:
                raise ValueError('Adapter local import dependency is not closed')
    if len(command) < 2 or _file(command[1], 2 * 1024 * 1024) != files[entry]:
        raise ValueError('Actual command entrypoint differs from manifest')
    ws = manifest.get('wsModule', {})
    ws_path = _file(ws.get('path', ''), 2 * 1024 * 1024)
    if _file(_option(command, '--ws-module'), 2 * 1024 * 1024) != ws_path or type(ws.get('bytes')) is not int or ws_path.stat().st_size != ws['bytes'] or _sha(ws_path) != ws.get('sha256'):
        raise ValueError('Actual WS module differs from manifest')
    manifest_sha, entry_sha = _sha(manifest_path), _sha(files[entry])
    runtime_path = _file(_option(command, '--runtime-receipt'), 2 * 1024 * 1024)
    runtime = json.loads(runtime_path.read_text(encoding='utf-8'))
    if runtime.get('adapterSourceManifestSha256') != manifest_sha or runtime.get('adapterEntrypointSha256') != entry_sha:
        raise ValueError('Runtime receipt does not bind the actual adapter')
    server_binary = _file(spec['server']['command'][0], 256 * 1024 * 1024)
    server_sha = _sha(server_binary)
    if runtime.get('binarySha256') != server_sha:
        raise ValueError('Runtime receipt does not bind the actual server binary')
    result = {'adapterSourceManifestSha256': manifest_sha, 'driverEntrypointSha256': entry_sha,
              'runtimeReceiptSha256': _sha(runtime_path), 'serverBinarySha256': server_sha,
              'adapterSourceProof': {'schema': manifest['schema'], 'entrypoint': entry,
                  'files': rows, 'wsModule': ws, 'actualCommandEntrypointMatched': True}}
    build_path = _option(command, '--build-manifest', required=False)
    if build_path is not None:
        build_path = _file(build_path, 1024 * 1024)
        build = json.loads(build_path.read_text(encoding='utf-8'))
        build_sha = _sha(build_path)
        if runtime.get('buildManifestSha256') != build_sha or build.get('binarySha256') != server_sha:
            raise ValueError('Build manifest does not bind the actual server binary')
        result['buildManifestSha256'] = build_sha
    return result
