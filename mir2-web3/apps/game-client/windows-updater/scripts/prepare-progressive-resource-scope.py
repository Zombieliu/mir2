"""Prepare unsigned scope bytes from a pinned full catalog + explicit path list.

No profile inference, network, signing, pack reads/builds, or bootstrap authority.
The caller authenticates the catalog digest and separately proves any gameplay
closure. A generated scope is only the exact prospective list supplied here.
"""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import re

MAX_CATALOG = 128 * 1024 * 1024
MAX_SCOPE = 4 * 1024 * 1024
MAX_RESOURCES = 300_000
MAX_PACKS = 256
MAX_PACK = 64 * 1024 * 1024
MAX_TOTAL = 4 * 1024 * 1024 * 1024
SCOPE_SCHEMA = 'mir2.native.required-resource-scope.v1'
CATALOG_SCHEMA = 'mir2.native.resource-catalog.v1'
FORBIDDEN = {'exe', 'dll', 'com', 'scr', 'cpl', 'msi', 'msp', 'bat', 'cmd', 'ps1', 'psm1', 'psd1',
             'vbs', 'vbe', 'js', 'jse', 'wsf', 'wsh', 'hta', 'reg', 'lnk', 'chm', 'jar', 'py', 'pyw',
             'sh', 'bash', 'zsh', 'fish'}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def digest(value):
    require(isinstance(value, str) and re.fullmatch('[0-9A-Fa-f]{64}', value) is not None, 'invalid SHA256')
    return value.upper()


def sha(data):
    return hashlib.sha256(data).hexdigest().upper()


def canonical(value):
    return (json.dumps(value, ensure_ascii=True, separators=(',', ':'), allow_nan=False) + '\n').encode('ascii')


def unique_pairs(pairs):
    out = {}
    for key, value in pairs:
        require(key not in out, 'duplicate JSON field')
        out[key] = value
    return out


def decoded(data):
    return json.loads(data.decode('utf-8'), object_pairs_hook=unique_pairs,
                      parse_constant=lambda _: require(False, 'nonfinite JSON number'))


def exact(value, keys, label):
    require(isinstance(value, dict) and set(value) == set(keys), label + ' fields')


def integer(value, low, high, label):
    require(type(value) is int and low <= value <= high, label + ' bound')
    return value


def logical(path):
    require(isinstance(path, str) and 0 < len(path) <= 230 and path.isascii(), 'logical path length/encoding')
    require(not any(ord(ch) < 32 or ch in '\\:\"<>|?*' for ch in path), 'unsafe logical path')
    for segment in path.split('/'):
        require(segment not in ('', '.', '..') and not segment.endswith(('.', ' ')), 'unsafe path segment')
        stem = segment.split('.')[0].upper()
        require(stem not in {'CON', 'PRN', 'AUX', 'NUL', 'CONIN$', 'CONOUT$'} and
                not re.fullmatch(r'(COM|LPT)[1-9]', stem), 'reserved logical path')
        require(not (set(segment.lower().split('.')[1:]) & FORBIDDEN), 'executable/script resource')
    return path


def plain_path(path):
    require(path.is_absolute(), 'absolute local path required')
    if path.drive:
        require(re.fullmatch('[A-Za-z]:', path.drive) is not None, 'only local drive paths supported')
    for item in [path, *path.parents]:
        require(not item.is_symlink() and not getattr(item, 'is_junction', lambda: False)(), 'symlink/junction rejected')


def read_pin(path, expected, limit):
    plain_path(path)
    require(path.is_file() and path.stat().st_size <= limit, 'input byte/type bound')
    with path.open('rb') as stream:
        data = stream.read(limit + 1)
    require(len(data) <= limit and sha(data) == digest(expected), 'input SHA/byte bound mismatch')
    return data


def catalog_model(data, expected_sha):
    require(len(data) <= MAX_CATALOG and sha(data) == digest(expected_sha), 'catalog SHA/byte bound')
    value = decoded(data)
    exact(value, {'schema', 'resources', 'packs'}, 'catalog')
    require(value['schema'] == CATALOG_SCHEMA and isinstance(value['resources'], dict) and
            len(value['resources']) <= MAX_RESOURCES and isinstance(value['packs'], list) and
            len(value['packs']) <= MAX_PACKS, 'catalog schema/count')
    packs = {}
    for pack in value['packs']:
        exact(pack, {'sha256', 'size'}, 'pack')
        h = digest(pack['sha256'])
        require(h not in packs, 'duplicate pack')
        packs[h] = {'sha256': h, 'size': integer(pack['size'], 32, MAX_PACK, 'pack size')}
    require(sum(p['size'] for p in packs.values()) <= MAX_TOTAL, 'physical total bound')
    resources, names, used, contents = {}, set(), set(), {}
    logical_total = 0
    for path, item in value['resources'].items():
        logical(path)
        require(path.lower() not in names, 'case-colliding catalog paths')
        names.add(path.lower())
        exact(item, {'sha256', 'size', 'packSha256', 'offset', 'storedSize', 'codec'}, 'resource')
        item = dict(item, sha256=digest(item['sha256']), packSha256=digest(item['packSha256']))
        require(item['packSha256'] in packs, 'undeclared resource pack')
        size = integer(item['size'], 0, MAX_PACK, 'resource size')
        integer(item['storedSize'], 0, MAX_PACK, 'stored size')
        integer(item['offset'], 32, MAX_PACK, 'resource offset')
        require(item['codec'] == 'stored' and item['storedSize'] == size and
                item['offset'] + size <= packs[item['packSha256']]['size'], 'resource codec/extent')
        if item['sha256'] in contents:
            require(contents[item['sha256']] == item, 'content alias locator differs')
        contents[item['sha256']] = item
        used.add(item['packSha256'])
        logical_total += size
        resources[path] = item
    require(used == set(packs) and logical_total <= MAX_TOTAL, 'catalog exact closure/logical total')
    for path in names:
        parts = path.split('/')
        require(not any('/'.join(parts[:i]) in names for i in range(1, len(parts))), 'file/directory collision')
    return resources, packs


def prepare(catalog_data, catalog_sha, paths_data, paths_sha, scope_id, target_pack_bytes=32 * 1024 * 1024):
    """Pure metadata preparation; all validation completes before output writes."""
    resources, packs = catalog_model(catalog_data, catalog_sha)
    require(len(paths_data) <= MAX_SCOPE and sha(paths_data) == digest(paths_sha), 'path-list SHA/byte bound')
    paths = decoded(paths_data)
    require(isinstance(paths, list) and 0 < len(paths) <= MAX_RESOURCES, 'explicit path-list count')
    seen = set()
    for path in paths:
        logical(path)
        require(path.lower() not in seen, 'duplicate/case-colliding explicit paths')
        require(path in resources, 'explicit path absent from full catalog: ' + path)
        seen.add(path.lower())
    paths = sorted(paths)
    require(isinstance(scope_id, str) and 1 <= len(scope_id) <= 64 and
            all(0x21 <= ord(ch) <= 0x7e for ch in scope_id), 'scope id must be printable ASCII1..64')
    integer(target_pack_bytes, 96, MAX_PACK, 'target pack bytes')
    scope = {'schema': SCOPE_SCHEMA, 'catalogSha256': digest(catalog_sha), 'id': scope_id, 'requiredPaths': paths}
    scope_data = canonical(scope)
    require(len(scope_data) <= MAX_SCOPE, 'emitted scope byte bound')
    ids = {resources[path]['packSha256'] for path in paths}
    unique = {resources[path]['sha256']: resources[path] for path in paths}
    plan = {'schema': 'mir2.native.resource-repack-plan.v1', 'sourceCatalogSha256': digest(catalog_sha),
            'targetPackBytes': target_pack_bytes, 'groups': [{'name': 'required-scope', 'paths': paths}]}
    plan_data = canonical(plan)
    receipt = {'schema': 'mir2.native.progressive-scope-preparation.v1',
               'catalog': {'sha256': digest(catalog_sha), 'sizeBytes': len(catalog_data), 'resourceCount': len(resources)},
               'explicitPaths': {'sha256': digest(paths_sha), 'sizeBytes': len(paths_data), 'count': len(paths)},
               'scope': {'path': 'required-scope.json', 'sha256': sha(scope_data), 'sizeBytes': len(scope_data)},
               'repackPlan': {'path': 'REPACK-PLAN.json', 'sha256': sha(plan_data), 'sizeBytes': len(plan_data),
                              'unspecifiedLogicalPaths': 'preserved in default group by full-inventory repack tool'},
               'requiredObjects': [packs[h] for h in sorted(ids)],
               'requiredObjectBytes': sum(packs[h]['size'] for h in ids),
               'logicalBytes': sum(resources[path]['size'] for path in paths),
               'uniquePayloadCount': len(unique), 'uniquePayloadBytes': sum(item['size'] for item in unique.values()),
               'catalogBytesShaVerified': True, 'packPayloadsVerifiedHere': False,
               'catalogAuthentication': 'caller must supply SHA from its independently verified release flow',
               'coverage': 'exact explicit prospective list; not a proved complete first-play closure',
               'signed': False, 'bootstrapAuthorized': False, 'closureComplete': False, 'firstPlayVerified': False}
    return {'required-scope.json': scope_data, 'REPACK-PLAN.json': plan_data, 'PREPARED-SCOPE.json': canonical(receipt)}


def emit(output, artifacts):
    plain_path(output)
    require(output.parent.is_dir(), 'output parent must already exist')
    output.mkdir()  # Refuse even an existing empty directory; retain partial failures.
    for name, data in artifacts.items():
        with (output / name).open('xb') as stream:
            stream.write(data)
        require((output / name).read_bytes() == data, 'output readback mismatch')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--catalog', type=Path, required=True)
    parser.add_argument('--catalog-sha256', required=True)
    parser.add_argument('--paths-json', type=Path, required=True, help='Explicit JSON array, not an inferred profile closure')
    parser.add_argument('--paths-sha256', required=True)
    parser.add_argument('--id', required=True)
    parser.add_argument('--target-pack-bytes', type=int, default=32 * 1024 * 1024)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    artifacts = prepare(read_pin(args.catalog, args.catalog_sha256, MAX_CATALOG), args.catalog_sha256,
                        read_pin(args.paths_json, args.paths_sha256, MAX_SCOPE), args.paths_sha256,
                        args.id, args.target_pack_bytes)
    emit(args.output, artifacts)
    print(artifacts['PREPARED-SCOPE.json'].decode('ascii'), end='')


if __name__ == '__main__':
    main()
