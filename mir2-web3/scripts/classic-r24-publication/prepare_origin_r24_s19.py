"""Offline fixed R24/s19 origin preparation; consumes genuine root-issued proof.

No networking, SSH, credential access, subprocesses, or public/server writes.
All outputs are exclusive files in this generator's fixed operators directory.
The parent must review before executing any emitted Linux command.
"""
import argparse
import ast
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import pprint
import re
import stat

OUT = Path('C:/mir2-playtest-releases/20261010-shop-mail-r24/operators-01')
OLD = Path('C:/mir2-playtest-releases/20261009-target-r23')
HERE = Path(__file__).resolve().parent
GAME = '87fd85a0d5a819e5b38c90d192421f9239f65678'
ENGINE = 'b4390ee4c987bbf000ba1d95761151e39fc0d54f'
CANDIDATE = 'WN-CANDIDATE-20261010-invited-24'
USER = '/tmp/mir2-origin-r24-s19-20261011-01'
ROOT = '/srv/.mir2-origin-r24-s19-20261011-01'
PUBLIC = '/srv/mir2-client-updates'
OLD_WRITER_SHA = '46fa356c3a1c83944c0e73c90a475d1a8507e58476bf810f4630c529fcd1c0f8'
GUARDS_SHA = '10693760a24973f896caa3f5ac6b1f59e02a32f6e9977cea0dbaa65adbaa0811'
OLD_PROOF_SHA = '6465c2919c92a31930350d0f147bac76d7a235f1544575bae4ed725b6b133dbf'
OLD_PLAN_SHA = 'a6bbad7431913438fb50d378ec76eeaedf96a6c5dc077e6625e9247860c026a0'
CADDY_SHA = '4f54f9ca961771de96efff9b24d983815cfbbf5c69369bed9da5e9c61dec4632'
FEED18 = {
    'latest.json': {'size': 1159, 'sha256': '2d87c59edb2aff2b55237dad1fcc418eac6ba03b6514720982ac16c1215f20d2'},
    'latest.p7s': {'size': 1614, 'sha256': '1f94346017807b0d6a2c24a3f4cdd3226461cc9618ed6dc0daa43460e7c0f8f4'},
}
ENGINE_EXE_SHA = '9213ec3fe1a992f757ea48c53981afa661765b42e637c1ad811e326060b170b8'
TEMPLATE_PINS = {'origin_r24_tail.py.in': '96aa297918fa1d401c02218a67d6e5fea49e18a0c8c7f66263ea208b8e3090cd', 'private_modes_r24.py.in': '6e0ad12528f3987c9e1b994279a5c4eded7493528d13cc9363d5397d0122a281', 'seal_r24.py.in': '3d4efef8fda7a640f147d3c7fca54b89937db701962f802124782f3182538f6f', 'feed_promote_r24.py.in': 'c6ac64a5a9f185fef02df5a3526494ef80de8537e40802f98f78c32ad37d9d1a'}
MAX_OBJECTS = 100
MAX_METADATA = 32 * 1024 * 1024
MAX_ARTIFACT = 128 * 1024 * 1024
OUTPUT_NAMES = (
    'APPEND-INPUTS.prepared.json', 'ROOT-VERIFICATION.json', 'PUBLICATION-PLAN.json',
    'PREVIOUS-ROOT-VERIFICATION.json', 'PREVIOUS-PUBLICATION-PLAN.json', 'guards.py',
    'origin_r24_s19.py', 'UPLOAD-BATCH-01.json', 'SEAL-INPUTS-01.json',
    'CREATE-STAGE-01.command', 'PRIVATE-MODES-01.command', 'SEAL-STAGE-01.command',
    'PREFLIGHT-01.command', 'APPEND-01.command', 'FEED-PROMOTE-01.command',
    'OFFLINE-PREPARATION-01.json',
)


def need(condition, label):
    if not condition:
        raise RuntimeError(label)


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def pairs(values):
    result = {}
    for key, value in values:
        need(key not in result, 'duplicate-json-key')
        result[key] = value
    return result


def decode(raw):
    return json.loads(raw, object_pairs_hook=pairs)


def encoded(value):
    return (json.dumps(value, indent=2, ensure_ascii=True) + '\n').encode()


def source_identity(info):
    # NTFS path/fd mode and ctime reporting differs. Do not relax Linux operators.
    return (info.st_dev, info.st_ino, stat.S_IFMT(info.st_mode), info.st_nlink, info.st_size, info.st_mtime_ns)


def ordinary(path):
    path = Path(path)
    need(path.is_absolute() and '..' not in path.parts, 'absolute-local-source-required')
    for parent in [*reversed(path.parent.parents), path.parent]:
        i = parent.lstat()
        need(stat.S_ISDIR(i.st_mode) and not getattr(i, 'st_file_attributes', 0) & 0x400, 'unsafe-local-parent')
    i = path.lstat()
    need(stat.S_ISREG(i.st_mode) and i.st_nlink == 1 and not getattr(i, 'st_file_attributes', 0) & 0x400, 'unsafe-local-file')
    return i


def read(path, expected_sha=None, maximum=MAX_METADATA):
    path = Path(path)
    before = ordinary(path)
    need(before.st_size <= maximum, 'local-read-bound')
    fd = os.open(path, os.O_RDONLY | getattr(os, 'O_NOFOLLOW', 0))
    with os.fdopen(fd, 'rb') as stream:
        need(source_identity(os.fstat(stream.fileno())) == source_identity(before), 'local-open-race')
        raw = stream.read(maximum + 1)
        need(source_identity(os.fstat(stream.fileno())) == source_identity(before), 'local-read-race')
    need(source_identity(ordinary(path)) == source_identity(before) and len(raw) == before.st_size, 'local-path-race')
    if expected_sha is not None:
        need(sha(raw) == expected_sha, 'local-source-sha:' + path.name)
    return raw


def match_local(obj):
    path = Path(obj['source'])
    before = ordinary(path)
    need(before.st_size == obj['size'], 'local-object-size:' + obj['path'])
    fd = os.open(path, os.O_RDONLY | getattr(os, 'O_NOFOLLOW', 0))
    count, h = 0, hashlib.sha256()
    with os.fdopen(fd, 'rb') as stream:
        need(source_identity(os.fstat(stream.fileno())) == source_identity(before), 'local-object-open-race')
        while chunk := stream.read(1024 * 1024):
            count += len(chunk)
            need(count <= obj['size'], 'local-object-overrun')
            h.update(chunk)
        need(source_identity(os.fstat(stream.fileno())) == source_identity(before), 'local-object-read-race')
    need(source_identity(ordinary(path)) == source_identity(before) and count == obj['size']
         and h.hexdigest() == obj['sha256'], 'local-object-pin:' + obj['path'])


def object_rows(plan):
    need(plan['schema'] == 'mir2.windows.r2-publication.v1' and isinstance(plan['objects'], list), 'publication-schema')
    need(2 <= len(plan['objects']) <= MAX_OBJECTS, 'object-count-bound')
    result = []
    for index, row in enumerate(plan['objects']):
        need(set(row) == {'path', 'source', 'size', 'sha256'}, 'publication-row-fields')
        path = PurePosixPath(row['path'])
        need(not path.is_absolute() and len(path.parts) >= 2 and path.parts[0] in ('releases', 'feeds', 'installers')
             and '..' not in path.parts and all(re.fullmatch(r'[A-Za-z0-9_.-]+', p) and p not in ('.', '..') for p in path.parts)
             and str(path) == row['path'], 'public-object-boundary')
        maximum = MAX_METADATA if path.suffix.lower() in ('.json', '.p7s') else MAX_ARTIFACT
        need(type(row['size']) is int and 0 < row['size'] <= maximum, 'object-size-bound')
        need(re.fullmatch('[0-9a-f]{64}', row['sha256']) and isinstance(row['source'], str), 'object-sha-source')
        result.append({**row, 'index': index})
    need(len(result) == len({o['path'] for o in result}), 'duplicate-public-path')
    return result


def validate_admission(plan, proof, plan_sha, proof_sha, sequence):
    need(proof['schema'] == 'mir2.windows.r2-verification.v1' and proof['passed'] is True
         and proof['candidateVerified'] is True and proof['cmsVerified'] is True
         and proof['sequence'] == sequence and proof['sourceRevision'] == ENGINE, 'root-issued-admission-binding')
    need(plan['verificationReceipt']['sha256'] == proof_sha, 'plan-root-proof-sha')
    candidate = plan['candidate']
    need(candidate['schema'] == 'mir2.windows.r2-channel.v1' and candidate['channel'] == 'invited'
         and candidate['platform'] == 'windows-x64' and candidate['sequence'] == sequence
         and candidate['sourceRevision'] == ENGINE and candidate['feedSha256'] == proof['feedSha256']
         and candidate['signatureSha256'] == proof['signatureSha256'], 'candidate-channel-binding')
    objects = object_rows(plan)
    closure = sorted([{k: o[k] for k in ('path', 'size', 'sha256')} for o in objects], key=lambda o: o['path'])
    raw = (json.dumps(closure, sort_keys=True, separators=(',', ':'), ensure_ascii=True) + '\n').encode()
    need(sha(raw) == proof['objectsSha256'], 'root-issued-closed-object-digest')
    prefix = f"feeds/s{sequence}-{proof['feedSha256']}/"
    need(objects[0]['path'] == prefix + 'latest.json' and objects[0]['sha256'] == proof['feedSha256']
         and objects[1]['path'] == prefix + 'latest.p7s' and objects[1]['sha256'] == proof['signatureSha256'], 'fixed-feed-order')
    need(objects[0]['size'] <= 32768 and objects[1]['size'] <= 32768, 'fixed-feed-size-bound')
    return objects


def frozen_pin(row):
    return {k: row[k] for k in ('index', 'path', 'size', 'sha256')}


def write(name, raw):
    need(name in OUTPUT_NAMES and Path(name).name == name and name not in ('.', '..'), 'output-leaf-only')
    with (OUT / name).open('xb') as stream:
        stream.write(raw)
    return {'path': str(OUT / name), 'size': len(raw), 'sha256': sha(raw)}


def command(source):
    ast.parse(source)
    return ("sudo -n python3 -B - <<'PY'\n" + source + '\nPY\n').encode()


def require_fresh_outputs():
    for name in OUTPUT_NAMES:
        path = OUT / name
        need(not path.exists() and not path.is_symlink(), 'fresh-output-required:' + name)


def render_writer(pins, old_writer, tail):
    need(sha(old_writer.encode('utf-8')) == OLD_WRITER_SHA, 'reviewed-writer-template-pin')
    tree = ast.parse(old_writer)
    functions = {node.name: ast.get_source_segment(old_writer, node) for node in tree.body if isinstance(node, ast.FunctionDef)}
    assignments = [node for node in tree.body if isinstance(node, ast.Assign)
        and any(isinstance(target, ast.Name) and target.id == 'PINS' for target in node.targets)]
    need(len(assignments) == 1, 'reviewed-pins-template-form')
    # Exact pinned R22 writer contains multiline PINS; take only code before it.
    base = '\n'.join(old_writer.splitlines()[:assignments[0].lineno - 1])
    base = base.replace('/srv/.mir2-origin-r23-s18-20261009-02', ROOT)
    base = base.replace("GAME = 'faefb69671272843958f68781b4f8ca2f4014522'", 'GAME = ' + repr(GAME))
    base = base.replace('R23/s18', 'R24/s19')
    writer = (base + '\n\nPINS = ' + pprint.pformat(pins, width=110, sort_dicts=True) + '\n\n'
        + functions['ensure_parent'] + '\n\n' + functions['check_target'] + '\n\n' + tail)
    ast.parse(writer)
    need(len(writer.encode('utf-8')) <= 262144, 'fixed-writer-size-bound')
    return writer


def prepare_output_directory():
    for path in [*reversed(OUT.parents), OUT]:
        if path.exists() or path.is_symlink():
            info = path.lstat()
            need(stat.S_ISDIR(info.st_mode) and not stat.S_ISLNK(info.st_mode)
                and not getattr(info, 'st_file_attributes', 0) & 0x400, 'unsafe-output-parent')
    OUT.mkdir(parents=True, exist_ok=True)
    require_fresh_outputs()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--publication-root', type=Path, required=True)
    parser.add_argument('--plan-sha256', required=True)
    parser.add_argument('--root-proof-sha256', required=True)
    args = parser.parse_args()
    require_fresh_outputs()
    for value in (args.plan_sha256, args.root_proof_sha256):
        need(re.fullmatch('[0-9a-f]{64}', value), 'expected-parent-sha-required')
    plan_raw = read(args.publication_root / 'PUBLICATION-PLAN.json', args.plan_sha256)
    proof_raw = read(args.publication_root / 'ROOT-VERIFICATION.json', args.root_proof_sha256)
    plan, proof = decode(plan_raw), decode(proof_raw)
    objects = validate_admission(plan, proof, args.plan_sha256, args.root_proof_sha256, 19)
    old_plan_raw = read(OLD / 'publication-18-01/PUBLICATION-PLAN.json', OLD_PLAN_SHA)
    old_proof_raw = read(OLD / 'publication-18-01/ROOT-VERIFICATION.json', OLD_PROOF_SHA)
    old_plan, old_proof = decode(old_plan_raw), decode(old_proof_raw)
    old_objects = validate_admission(old_plan, old_proof, OLD_PLAN_SHA, OLD_PROOF_SHA, 18)
    need(len(old_objects) == 40 and old_objects[0]['sha256'] == FEED18['latest.json']['sha256']
         and old_objects[1]['sha256'] == FEED18['latest.p7s']['sha256'], 'known-r23-closure-required')
    need(plan['expectedCurrent'] == old_plan['candidate'], 'exact-s18-predecessor-required')
    for obj in objects:
        match_local(obj)
    feed = decode(read(objects[0]['source'], objects[0]['sha256'], 32768))
    game_directory = 'releases/game-' + CANDIDATE
    need(feed['schema'] == 'mir2.windows.update-feed.v1' and feed['sequence'] == 19 and feed['channel'] == 'invited'
         and feed['platform'] == 'windows-x64' and feed['game']['directory'] == game_directory
         and feed['game']['identity'] == CANDIDATE, 'actual-feed19-game-binding')
    need(feed['engine']['directory'] == 'releases/updater-9213EC3FE1A992F7-s15' and feed['engine']['identity'] == '1', 'fixed-engine-component')
    for obj in objects:
        need(obj['path'].startswith(game_directory + '/') or
             obj['path'].startswith(feed['engine']['directory'] + '/') or
             obj['path'] in (objects[0]['path'], objects[1]['path']) or
             re.fullmatch(r'installers/[0-9a-f]{64}/Mir2Setup\.exe', obj['path']), 'fixed-r24-closed-scope')
    by_path = {o['path']: o for o in objects}
    statement_row = by_path[game_directory + '/RELEASE-STATEMENT.json']
    statement = decode(read(statement_row['source'], statement_row['sha256'], 32768))
    need(statement['gitRevision'] == GAME and statement['candidate'] == CANDIDATE and statement['worktreeDirty'] is False,
         'actual-clean-r24-statement-required')
    engine_row = by_path[feed['engine']['directory'] + '/ENGINE.json']
    engine = decode(read(engine_row['source'], engine_row['sha256'], 32768))
    engine_exe = by_path[feed['engine']['directory'] + '/Mir2Updater.exe']
    need(engine['sourceRevision'] == ENGINE and engine['exeSha256'].lower() == ENGINE_EXE_SHA
         and engine_exe['sha256'] == ENGINE_EXE_SHA, 'exact-reused-engine-required')
    old_by_content = {}
    for obj in old_objects:
        old_by_content.setdefault((obj['size'], obj['sha256']), obj)
    reuse = {}
    for obj in objects:
        old = old_by_content.get((obj['size'], obj['sha256']))
        if old is not None:
            reuse[obj['index']] = old['path']
    source_names = {o['index']: f"{o['index']:02}-{PurePosixPath(o['path']).name}" for o in objects}
    prepared = {'schema': 'mir2.origin.r24.fixed-input-plan.v1', 'sequence': 19, 'gameSource': GAME, 'engineSource': ENGINE,
                'candidate': CANDIDATE, 'rootProofSha256': args.root_proof_sha256, 'publicationSha256': args.plan_sha256,
                'previousRootProofSha256': OLD_PROOF_SHA, 'previousPublicationSha256': OLD_PLAN_SHA,
                'objects': [frozen_pin(o) for o in objects], 'publicBytes': sum(o['size'] for o in objects),
                'reusedPublicObjects': [{'newIndex': index, 'previousPath': path} for index, path in sorted(reuse.items())],
                'reusedPublicBytes': sum(objects[i]['size'] for i in reuse), 'publicBoundary': PUBLIC,
                'userStage': USER, 'rootStage': ROOT, 'sourceNames': source_names,
                'privateCopiesOnly': True, 'gatewayChanges': False}
    prepared_raw = encoded(prepared)
    guards_raw = read(HERE.parent / 'classic-r22-publication/guards.py.in', GUARDS_SHA)
    old_writer_raw = read(OLD / 'operators-02/origin_r23_s18.py', OLD_WRITER_SHA)
    old_writer = old_writer_raw.decode('utf-8')
    pins = {'guards': GUARDS_SHA, 'proof': args.root_proof_sha256, 'publication': args.plan_sha256,
            'prepared': sha(prepared_raw), 'previousProof': OLD_PROOF_SHA, 'previousPublication': OLD_PLAN_SHA,
            'config': CADDY_SHA, 'previousFeed': FEED18, 'previousGameDirectory': 'releases/game-WN-CANDIDATE-20261010-invited-24',
            'gameDirectory': game_directory, 'candidate': CANDIDATE, 'engineExeSha256': ENGINE_EXE_SHA,
            'new': [frozen_pin(o) for o in objects], 'previousObjects': [frozen_pin(o) for o in old_objects],
            'sourceNames': source_names}
    writer = render_writer(pins, old_writer, read(HERE / 'origin_r24_tail.py.in', TEMPLATE_PINS['origin_r24_tail.py.in'], maximum=131072).decode('utf-8'))
    writer_raw = writer.encode()
    need(len(writer_raw) <= 262144, 'fixed-writer-size-bound')
    files = {'APPEND-INPUTS.prepared.json': prepared_raw, 'ROOT-VERIFICATION.json': proof_raw,
             'PUBLICATION-PLAN.json': plan_raw, 'PREVIOUS-ROOT-VERIFICATION.json': old_proof_raw,
             'PREVIOUS-PUBLICATION-PLAN.json': old_plan_raw, 'guards.py': guards_raw, 'origin_r24_s19.py': writer_raw}
    rows = [{'name': name, 'size': len(raw), 'sha256': sha(raw), 'kind': 'user', 'source': USER + '/' + name}
            for name, raw in files.items()]
    uploads = [[str(OUT / name), USER + '/' + name] for name in files]
    for obj in objects:
        name = source_names[obj['index']]
        reused = obj['index'] in reuse
        source = PUBLIC + '/' + reuse[obj['index']] if reused else USER + '/' + name
        rows.append({'name': name, 'size': obj['size'], 'sha256': obj['sha256'], 'kind': 'public' if reused else 'user', 'source': source})
        if not reused:
            uploads.append([obj['source'], USER + '/' + name])
    # Nothing is written until all genuine source/admission/hash checks above have passed.
    prepare_output_directory()
    output_records = [write(name, raw) for name, raw in files.items()]
    output_records.append(write('UPLOAD-BATCH-01.json', encoded(uploads)))
    output_records.append(write('SEAL-INPUTS-01.json', encoded(rows)))
    create = """import json, os, pathlib, stat
def need(condition, label):
    if not condition:
        raise RuntimeError(label)
path = pathlib.Path('/tmp/mir2-origin-r24-s19-20261011-01')
need(os.geteuid() == 1000 and not path.exists() and not path.is_symlink(), 'fresh-user-stage-required')
i = path.parent.lstat()
need(stat.S_ISDIR(i.st_mode) and i.st_uid == 0, 'unsafe-tmp-parent')
path.mkdir(mode=0o700)
i = path.lstat()
need(stat.S_ISDIR(i.st_mode) and i.st_uid == 1000 and stat.S_IMODE(i.st_mode) == 0o700, 'private-created-stage')
print(json.dumps({'schema': 'mir2.private-upload-stage.v1', 'created': True, 'path': str(path)}))
"""
    ast.parse(create)
    output_records.append(write('CREATE-STAGE-01.command', ("python3 -B - <<'PY'\n" + create + 'PY\n').encode()))
    user_rows = [r for r in rows if r['kind'] == 'user']
    private_source = read(HERE / 'private_modes_r24.py.in', TEMPLATE_PINS['private_modes_r24.py.in'], maximum=131072).decode('utf-8')
    private_source = 'ROWS = ' + pprint.pformat(user_rows, width=110, sort_dicts=True) + '\n' + private_source
    ast.parse(private_source)
    output_records.append(write('PRIVATE-MODES-01.command', ("python3 -B - <<'PY'\n" + private_source + '\nPY\n').encode()))
    seal = ('import pathlib\nUSER = pathlib.Path(' + repr(USER) + ')\nROOT = pathlib.Path(' + repr(ROOT) +
            ')\nPUBLIC = pathlib.Path(' + repr(PUBLIC) + ')\nROWS = ' + pprint.pformat(rows, width=110, sort_dicts=True) + '\n' +
            read(HERE / 'seal_r24.py.in', TEMPLATE_PINS['seal_r24.py.in'], maximum=131072).decode('utf-8'))
    output_records.append(write('SEAL-STAGE-01.command', command(seal)))
    for mode in ('preflight', 'append'):
        output_records.append(write(mode.upper() + '-01.command',
                                    f'sudo -n python3 -B {ROOT}/origin_r24_s19.py {mode}\n'.encode()))
    promote = 'WRITER_SHA256 = ' + repr(sha(writer_raw)) + '\n' + read(HERE / 'feed_promote_r24.py.in', TEMPLATE_PINS['feed_promote_r24.py.in'], maximum=131072).decode('utf-8')
    output_records.append(write('FEED-PROMOTE-01.command', command(promote)))
    receipt = {'schema': 'mir2.origin.r24.offline-preparation.v1', 'sequence': 19, 'gameSource': GAME, 'engineSource': ENGINE,
               'rootProofSha256': args.root_proof_sha256, 'publicationSha256': args.plan_sha256,
               'objects': len(objects), 'sealedFiles': len(rows), 'uploadedFiles': len(uploads),
               'reusedPublicFiles': len(reuse), 'reusedPublicBytes': sum(objects[i]['size'] for i in reuse),
               'uploadedObjectBytes': sum(o['size'] for o in objects if o['index'] not in reuse),
               'cmsReverifiedByThisGenerator': False, 'proofIssuerUnchanged': True, 'publicMutations': False,
               'networkCalls': 0, 'gatewayChanges': False, 'outputs': output_records}
    write('OFFLINE-PREPARATION-01.json', encoded(receipt))
    print(json.dumps({k: v for k, v in receipt.items() if k != 'outputs'}, sort_keys=True))


if __name__ == '__main__':
    main()
