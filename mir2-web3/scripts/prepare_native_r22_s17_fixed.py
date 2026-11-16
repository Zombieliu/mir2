#!/usr/bin/env python3
"""Freeze the root-verified R22/s17 publication, retaining the signed s15 engine.

Offline code generation only: this neither verifies CMS nor signs/publishes.
Root must first execute the pinned Windows CMS and complete delivery-byte gates.
Only new R22/s17 paths are create-only outputs; old channels and readers remain
immutable. Identities, predecessor, origin, limits and transport policy are not
CLI options. Actual stage, promotion, cloud and Windows acceptance stay open.
"""
from __future__ import annotations

import argparse
import ast
import base64
import copy
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import stat
import sys
import subprocess
import types
import time

sys.dont_write_bytecode = True
BASE = Path(__file__).resolve().parents[1]
GAME_SOURCE = '7fea5cdba8cd160d4f2c5536f14fb7ce98c6bfbc'
GAME_CANDIDATE = 'WN-CANDIDATE-20261008-invited-22'
ENGINE_SOURCE = 'b4390ee4c987bbf000ba1d95761151e39fc0d54f'
GAME_DIRECTORY = 'releases/game-' + GAME_CANDIDATE
ENGINE_DIRECTORY = 'releases/updater-9213EC3FE1A992F7-s15'
ENGINE_EXE_SHA = '9213ec3fe1a992f757ea48c53981afa661765b42e637c1ad811e326060b170b8'
SEQUENCE = 17
PREDECESSOR = 16
FIXTURE = 'scripts/fixtures/native-r22-s17/'
WORKER = 'infra/cloudflare/mir2-r2-bulk-upload/'
PLAN_PATH = WORKER + 'src/native-r22-s17-publication-plan.json'
MODULE_PATH = WORKER + 'src/native-r22-s17-publication.mjs'
DRIVER_PATH = 'scripts/native_r22_s17_r2_delivery.py'
MAX_OBJECTS = 100
MAX_METADATA = 32 * 1024 * 1024
MAX_ARTIFACT = 128 * 1024 * 1024
CONTROL_LIMIT = 65536
SHA = re.compile(r'[0-9a-f]{64}\Z')
IMMUTABLE = 'public, max-age=31536000, immutable'

# Exact Git blob bytes from the clean reviewed publication base; no input
# feed/CMS/proof normalization and no dependency on checkout line endings.
TEMPLATE_COMMIT = 'aba54763f797c90596fd8aeea7f24c622fe71bed'
TEMPLATES = {'scripts/native_r20_s16_r2_delivery.py': 'd27d27c7b66e91cf36924b6d375d1af462db71408a826c6b0830d0f243657027', 'scripts/test_native_r20_s16_r2_delivery.py': '5404b8f443cd0d31b0f5e8863accae353573713c663086017377b0097d1eb90a', 'scripts/test_native_r20_s16_interrupt.py': 'bc5af1d2feb722b3f3a04cc6606e9f489ae3a9b198a7425000900814497922dc', 'infra/cloudflare/mir2-r2-bulk-upload/src/native-r20-s16-publication.mjs': 'acd0459e3a33efc11e1519c9a0238e54b5b2d65e8018c1ae721d023fa9f1d5c6', 'infra/cloudflare/mir2-r2-bulk-upload/src/native-r20-s16-publication-plan.json': 'c9b149a3ef6351100eecbc43fc889b1262031824b00ee9169faa4b8d5808c1c9', 'infra/cloudflare/mir2-r2-bulk-upload/test/native-r20-s16-publication.test.mjs': '9b85534194842790da69162251858985148b03a2df15dd541c729bf1edef5eb6', 'scripts/fixtures/native-r20-s16/actual-cms16/PUBLICATION-PLAN.json': '905bc48ea9a210d5cb8ef5d82f748b341d9654cf287f1c3803ce74a1a7ae556d', 'scripts/fixtures/native-r20-s16/actual-cms16/ROOT-VERIFICATION.json': '6a59813e8e3451820a064e1ff1c9f7525f77e0aab6ed85ffa791c8679c3192e8', 'scripts/fixtures/native-r20-s16/preview/artifacts/feed/latest.json': 'fd8109231f6077405c410047fab94ea1a5797e9963e81fd9a80a0cb86cf95dca', 'scripts/fixtures/native-r20-s16/preview/artifacts/feed/latest.p7s': '1f881c23724b892ab9814eb2eb9002a365df159a7220bb5316538a0ea181a5c7'}
# Exact alternate checkout bytes for historical source text only.
# No original CMS/proof/feed input may enter this code-only conversion path.
CHECKOUT_CRLF_PINS = {'scripts/native_r20_s16_r2_delivery.py': '5ef09625286688caba91bfdb34b1663711edccca4bf06b1c5f7c131ec2b94e6c', 'scripts/test_native_r20_s16_r2_delivery.py': 'dba45d708fb42f840f4bd00ddebb7d10844e225051a24a65e99f90a38dfececf', 'scripts/test_native_r20_s16_interrupt.py': '34bd76ee23e560d5f93dc1d8478e677a280ae909a9c70277917d795605d73330', 'infra/cloudflare/mir2-r2-bulk-upload/src/native-r20-s16-publication.mjs': '1eb01cbaab41058910c9209bcbf9d5845a35b863b38719eecffc1b93ba93f043', 'infra/cloudflare/mir2-r2-bulk-upload/src/native-r20-s16-publication-plan.json': '8309026edbee4b3597603523c51ddee745f6fd8aa61882196abdc3ed5600ebac', 'infra/cloudflare/mir2-r2-bulk-upload/test/native-r20-s16-publication.test.mjs': '4992cd1723be60d8be5fbbfa8de0bead293138cbf6b0b2ef0cb00d0003090407'}


class GuardError(Exception):
    pass


def require(value, label):
    if not value:
        raise GuardError(label)


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def canonical(value):
    return (json.dumps(value, sort_keys=True, separators=(',', ':'),
                       ensure_ascii=False, allow_nan=False) + '\n').encode('utf-8')


def plain(path):
    path = Path(path)
    require(path.is_absolute() and '..' not in path.parts, 'absolute_plain_input_required')
    for ancestor in [*reversed(path.parents), path]:
        info = ancestor.lstat()
        require(not stat.S_ISLNK(info.st_mode)
                and not (getattr(info, 'st_file_attributes', 0) & 0x400), 'linked_input_rejected')
        if ancestor == path:
            require(stat.S_ISREG(info.st_mode) and info.st_nlink == 1, 'plain_single_link_file_required')
        else:
            require(stat.S_ISDIR(info.st_mode), 'plain_parent_required')
    return path


def identity(info):
    return info.st_dev, info.st_ino, info.st_size, info.st_mtime_ns, info.st_ctime_ns


def read(path, maximum):
    path = plain(path)
    before = path.lstat()
    require(0 < before.st_size <= maximum, 'bounded_input_required')
    with path.open('rb') as stream:
        opened = os.fstat(stream.fileno())
        require(identity(opened)[:4] == identity(before)[:4], 'input_changed_before_read')
        raw = stream.read(maximum + 1)
        require(len(raw) == before.st_size and identity(os.fstat(stream.fileno())) == identity(opened),
                'input_changed_during_read')
    require(identity(path.lstat()) == identity(before), 'input_replaced_after_read')
    return raw


def hash_file(path, size, expected):
    path = plain(path)
    before = path.lstat()
    require(before.st_size == size, 'source_size_mismatch')
    digest = hashlib.sha256()
    total = 0
    with path.open('rb') as stream:
        opened = os.fstat(stream.fileno())
        require(identity(opened)[:4] == identity(before)[:4], 'source_changed_before_read')
        while block := stream.read(65536):
            total += len(block)
            require(total <= size, 'source_length_changed')
            digest.update(block)
        require(identity(os.fstat(stream.fileno())) == identity(opened), 'source_changed_during_read')
    require(total == size and digest.hexdigest() == expected and identity(path.lstat()) == identity(before),
            'source_full_hash_mismatch')


def template_inputs():
    raw = {}
    for name, pin in TEMPLATES.items():
        result = subprocess.run(['git', '-C', str(BASE), 'show',
            TEMPLATE_COMMIT + ':mir2-web3/' + name], stdout=subprocess.PIPE,
            stderr=subprocess.PIPE, check=False)
        if result.returncode == 0:
            value = result.stdout
        else:
            # A shallow CI clone may omit the reviewed ancestor object. Accept
            # only explicitly pinned unchanged source variants from checkout.
            value = read(BASE / name, MAX_METADATA)
            if sha(value) != pin:
                require(name in CHECKOUT_CRLF_PINS
                    and sha(value) == CHECKOUT_CRLF_PINS[name], 'reviewed_checkout_template_changed')
                value = value.replace(b'\r\n', b'\n')
        require(0 < len(value) <= MAX_METADATA and sha(value) == pin,
                'reviewed_git_blob_template_changed')
        raw[name] = value
    subject = types.ModuleType('frozen_s16_generator_subject')
    subject.__file__ = str(BASE / 'scripts/native_r20_s16_r2_delivery.py')
    exec(compile(raw['scripts/native_r20_s16_r2_delivery.py'], subject.__file__, 'exec'), subject.__dict__)
    previous = subject.load_plan_bytes(raw[WORKER + 'src/native-r20-s16-publication-plan.json'])
    require(previous['candidate']['sequence'] == PREDECESSOR
            and previous['candidate']['sourceRevision'] == ENGINE_SOURCE, 'exact_predecessor16_required')
    previous_feed = subject.strict_json(raw['scripts/fixtures/native-r20-s16/preview/artifacts/feed/latest.json'], 32768)
    require(previous_feed['sequence'] == PREDECESSOR
            and previous_feed['engine']['directory'] == ENGINE_DIRECTORY, 'exact_s15_engine_directory_required')
    require(sha(base64.b64decode(subject.SOURCE_PLAN_BASE64, validate=True)) == previous['sourcePlanSha256']
            and sha(base64.b64decode(subject.ROOT_VERIFICATION_BASE64, validate=True)) == previous['rootVerificationSha256']
            and raw['scripts/fixtures/native-r20-s16/actual-cms16/PUBLICATION-PLAN.json']
                == base64.b64decode(subject.SOURCE_PLAN_BASE64, validate=True)
            and raw['scripts/fixtures/native-r20-s16/actual-cms16/ROOT-VERIFICATION.json']
                == base64.b64decode(subject.ROOT_VERIFICATION_BASE64, validate=True), 'actual_predecessor_proof_required')
    return raw, previous, previous_feed, subject


def exact(value, fields, label):
    require(isinstance(value, dict) and set(value) == set(fields), label)


def object_limit(path):
    return MAX_METADATA if re.search(r'\.(json|p7s|sig|txt)$', path, re.I) else MAX_ARTIFACT


def closure(objects):
    return [{key: entry[key] for key in ('path', 'size', 'sha256')} for entry in objects]


def relative(path):
    require(isinstance(path, str) and re.fullmatch(r'[A-Za-z0-9._/-]+', path)
            and all(part not in ('', '.', '..') and not part.startswith('.') for part in path.split('/')),
            'unsafe_object_path')


def validate_inputs(publication_raw, root_raw, feed_raw, signature_raw, bootstrap,
                    previous, previous_feed, subject, *, now=None):
    publication = subject.strict_json(publication_raw, CONTROL_LIMIT)
    root = subject.strict_json(root_raw, CONTROL_LIMIT)
    feed = subject.strict_json(feed_raw, 32768)
    exact(publication, ('schema', 'expectedCurrent', 'candidate', 'verificationReceipt', 'objects'),
          'publication_fields_mismatch')
    require(publication['schema'] == 'mir2.windows.r2-publication.v1'
            and publication['expectedCurrent'] == previous['candidate'], 'exact_expected_current16_required')
    candidate = publication['candidate']
    exact(candidate, ('schema', 'channel', 'platform', 'sequence', 'sourceRevision', 'feedSha256', 'signatureSha256'),
          'candidate_fields_mismatch')
    require(candidate['schema'] == 'mir2.windows.r2-channel.v1'
            and candidate['channel'] == 'invited' and candidate['platform'] == 'windows-x64'
            and type(candidate['sequence']) is int and candidate['sequence'] == SEQUENCE
            and candidate['sourceRevision'] == ENGINE_SOURCE
            and candidate['feedSha256'] == sha(feed_raw)
            and candidate['signatureSha256'] == sha(signature_raw), 'exact_candidate17_required')
    exact(root, ('schema', 'passed', 'candidateVerified', 'cmsVerified', 'sourceRevision', 'sequence',
                 'feedSha256', 'signatureSha256', 'objectsSha256'), 'root_fields_mismatch')
    require(root['schema'] == 'mir2.windows.r2-verification.v1' and root['passed'] is True
            and root['candidateVerified'] is True and root['cmsVerified'] is True
            and all(root[key] == candidate[key] for key in ('sourceRevision', 'sequence', 'feedSha256', 'signatureSha256')),
            'actual_root17_receipt_required')
    ref = publication['verificationReceipt']
    exact(ref, ('path', 'sha256'), 'verification_reference_fields_mismatch')
    require(ref['sha256'] == sha(root_raw), 'root_raw_hash_mismatch')
    objects = publication['objects']
    require(isinstance(objects, list) and 6 <= len(objects) <= MAX_OBJECTS, 'publication_object_bound')
    for entry in objects:
        exact(entry, ('path', 'source', 'size', 'sha256'), 'object_fields_mismatch')
        relative(entry['path'])
        require(type(entry['size']) is int and 0 < entry['size'] <= object_limit(entry['path'])
                and isinstance(entry['sha256'], str) and SHA.fullmatch(entry['sha256']), 'object_size_hash_bound')
    require(objects == sorted(objects, key=lambda entry: entry['path'])
            and len({entry['path'].lower() for entry in objects}) == len(objects), 'sorted_unique_closure_required')
    require(root['objectsSha256'] == sha(canonical(closure(objects))), 'actual_root_closure_mismatch')
    exact(feed, ('schema', 'channel', 'platform', 'sequence', 'createdUnix', 'expiresUnix', 'minBootstrap',
                 'protocol', 'content', 'game', 'engine'), 'feed_fields_mismatch')
    seconds = int(time.time()) if now is None else now
    require(feed['schema'] == 'mir2.windows.update-feed.v1' and feed['channel'] == 'invited'
            and feed['platform'] == 'windows-x64' and type(feed['sequence']) is int and feed['sequence'] == SEQUENCE
            and feed['protocol'] == 'crystal-mir2-v1' and feed['content'] == 'mir2.windows.package-manifest.v4'
            and type(feed['minBootstrap']) is int and feed['minBootstrap'] in (0, 1)
            and type(feed['createdUnix']) is int and type(feed['expiresUnix']) is int
            and feed['createdUnix'] <= seconds < feed['expiresUnix']
            and 0 < feed['expiresUnix'] - feed['createdUnix'] <= 31 * 86400, 'current_signed_feed17_required')
    by_path = {entry['path']: entry for entry in objects}
    for name, names in (('game', ('PACKAGE-MANIFEST.json', 'VERSION.json', 'RELEASE-STATEMENT.json', 'RELEASE-STATEMENT.p7s')),
                        ('engine', ('ENGINE.json', 'ENGINE.p7s'))):
        component = feed[name]
        exact(component, ('directory', 'identity', 'metadata'), 'feed_component_fields')
        relative(component['directory'])
        require(component['directory'].startswith('releases/') and isinstance(component['metadata'], list)
                and len(component['metadata']) == len(names), 'component_metadata_bound')
        seen = set()
        for item in component['metadata']:
            exact(item, ('path', 'size', 'sha256'), 'component_metadata_fields')
            require(item['path'] in names and item['path'] not in seen, 'component_metadata_set')
            seen.add(item['path'])
            obj = by_path.get(component['directory'] + '/' + item['path'])
            require(obj is not None and type(item['size']) is int and item['size'] == obj['size']
                    and isinstance(item['sha256'], str) and SHA.fullmatch(item['sha256'].lower())
                    and item['sha256'].lower() == obj['sha256'], 'component_metadata_byte_binding')
    require(feed['game']['directory'] == GAME_DIRECTORY and feed['game']['identity'] == GAME_CANDIDATE,
            'exact_r22_game_identity_required')
    require(feed['engine'] == previous_feed['engine'] and feed['engine']['directory'] == ENGINE_DIRECTORY,
            'exact_s15_engine_component_required')
    expected_engine = closure([entry for entry in previous['objects']
                               if entry['path'].startswith(ENGINE_DIRECTORY + '/')])
    actual_engine = closure([entry for entry in objects if entry['path'].startswith(ENGINE_DIRECTORY + '/')])
    require(actual_engine == expected_engine and len(actual_engine) == 3, 'exact_s15_engine_closure_required')
    feed_directory = 'feeds/s17-' + candidate['feedSha256']
    expected_feed = [feed_directory + '/latest.json', feed_directory + '/latest.p7s']
    require([entry['path'] for entry in objects[:2]] == expected_feed
            and all(objects[index]['size'] == len(raw) and objects[index]['sha256'] == sha(raw)
                    for index, raw in enumerate((feed_raw, signature_raw))), 'exact_immutable_feed_pair_required')
    for entry in objects:
        path = entry['path']
        require(path.startswith(GAME_DIRECTORY + '/') or path.startswith(ENGINE_DIRECTORY + '/')
                or path in expected_feed or re.fullmatch(r'installers/[0-9a-f]{64}/Mir2Setup\.exe', path),
                'r22_closed_object_scope_required')
    for entry in objects:
        hash_file(entry['source'], entry['size'], entry['sha256'])
    engine = subject.strict_json(read(by_path[ENGINE_DIRECTORY + '/ENGINE.json']['source'], 32768), 32768)
    require(engine.get('sourceRevision') == ENGINE_SOURCE
            and engine.get('exeSha256', '').lower() == ENGINE_EXE_SHA
            and by_path[ENGINE_DIRECTORY + '/Mir2Updater.exe']['sha256'] == ENGINE_EXE_SHA,
            'exact_engine_b4390_required')
    version = subject.strict_json(read(by_path[GAME_DIRECTORY + '/VERSION.json']['source'], CONTROL_LIMIT), CONTROL_LIMIT)
    require(version.get('gitRevision') == GAME_SOURCE and version.get('candidate') == GAME_CANDIDATE
            and version.get('worktreeDirty') is False and version.get('clientOnly') is True,
            'exact_game_source7fea5cd_required')
    bootstrap_entry = next((entry for entry in objects if entry['path'].startswith('installers/')), None)
    require(bootstrap_entry is not None
            and sum(entry['path'].startswith('installers/') for entry in objects) == 1, 'one_bootstrap_required')
    bootstrap = plain(bootstrap)
    hash_file(bootstrap, bootstrap_entry['size'], bootstrap_entry['sha256'])
    require(bootstrap_entry['path'] == 'installers/' + bootstrap_entry['sha256'] + '/Mir2Setup.exe',
            'bootstrap_content_address_required')
    with bootstrap.open('rb') as stream:
        require(stream.read(2) == b'MZ', 'bootstrap_executable_required')
    return publication, root, feed, bootstrap_entry


def replace_once(text, old, new):
    require(text.count(old) == 1, 'reviewed_template_hunk_changed')
    return text.replace(old, new, 1)


def replace_assignment(text, name, value, *, javascript=False):
    prefix = 'const ' if javascript else ''
    end = ';' if javascript else ''
    replacement = prefix + name + ' = ' + (json.dumps(value) if javascript else repr(value)) + end
    result, count = re.subn(r'^' + re.escape(prefix + name) + r' = [^\r\n]*' + r'$',
                            lambda _match: replacement, text, flags=re.M)
    require(count == 1, 'reviewed_assignment_hunk_changed')
    return result


def freeze_definition(publication, proof, installer, bootstrap, previous):
    definition = copy.deepcopy(previous)
    definition.update(schema='mir2.native.r22.fixed-origin-plan.v1', sourceRevision=ENGINE_SOURCE,
        sourcePlanSha256=sha(canonical(publication)), rootVerificationSha256=sha(canonical(proof)),
        candidate=publication['candidate'], expectedCurrent=publication['expectedCurrent'],
        objectsSha256=proof['objectsSha256'], knownPrevious=previous['candidate'],
        predecessorProof={key: previous[key] for key in ('sourcePlanSha256', 'rootVerificationSha256')})
    definition['preparation'].update(requiredGameSourceRevision=GAME_SOURCE, requiredGameCandidate=GAME_CANDIDATE,
        gameManifestSha256=next(obj['sha256'] for obj in publication['objects'] if obj['path'] == GAME_DIRECTORY + '/PACKAGE-MANIFEST.json'),
        gameVersionSha256=next(obj['sha256'] for obj in publication['objects'] if obj['path'] == GAME_DIRECTORY + '/VERSION.json'),
        verifierReceiptSource='fixtures/actual-cms17/ROOT-VERIFICATION.json', verifierExecutedThisPhase=True,
        requiredVerifier='Actual pinned Windows CMS; exact R22/source7fea5cd; unchanged signed s15 engine b4390ee4')
    definition['installerOriginEvidence'].update(expectedSha256=installer['sha256'], expectedSize=installer['size'],
        requestedOriginRelativePath=installer['path'], sourceFixture=str(bootstrap),
        actualPublicBuildReceipt='root-reviewed reused R19 Bootstrap artifact; independently full-hashed by generator')
    definition['objects'] = []
    for index, entry in enumerate(publication['objects']):
        path = entry['path']
        obj = {key: entry[key] for key in ('path', 'size', 'sha256')}
        obj.update(index=index, r2Key=definition['r2Prefix'] + path, originRelativePath=path,
            contentType='application/json' if path.endswith('.json') else 'application/pkcs7-signature' if path.endswith('.p7s')
            else 'text/plain; charset=utf-8' if path.endswith('.txt') else 'application/octet-stream', cacheControl=IMMUTABLE)
        definition['objects'].append(obj)
    return definition


def advance_source(text):
    return (text.replace('native_r20_s16', 'native_r22_s17')
        .replace('native-r20-s16', 'native-r22-s17')
        .replace('NativeR20S16', 'NativeR22S17').replace('nativeR20S16', 'nativeR22S17')
        .replace('R20/s16', 'R22/s17').replace('R20 proof', 'R22 proof')
        .replace('mir2.native.r20.', 'mir2.native.r22.')
        .replace('8faedd89fbcdda7d4f814c05f7d2f56979ab5bf5', GAME_SOURCE)
        .replace('WN-CANDIDATE-20261007-invited-20', GAME_CANDIDATE))


def render_runtime(raw, definition, publication_raw, root_raw):
    """Advance only exact reviewed identity hunks; preserve transport policy."""
    literal = (json.dumps(definition, indent=2, ensure_ascii=False) + '\n').encode('utf-8')
    require(len(literal) <= CONTROL_LIMIT, 'fixed_plan_exceeds_unchanged_control_limit')
    native_hash = sha(canonical(definition))
    blobs = dict(SOURCE_PLAN_BASE64=base64.b64encode(publication_raw).decode('ascii'),
        ROOT_VERIFICATION_BASE64=base64.b64encode(root_raw).decode('ascii'),
        PREDECESSOR_PLAN_BASE64=base64.b64encode(raw['scripts/fixtures/native-r20-s16/actual-cms16/PUBLICATION-PLAN.json']).decode('ascii'),
        PREDECESSOR_ROOT_BASE64=base64.b64encode(raw['scripts/fixtures/native-r20-s16/actual-cms16/ROOT-VERIFICATION.json']).decode('ascii'))
    script = advance_source(raw['scripts/native_r20_s16_r2_delivery.py'].decode('utf-8'))
    script = replace_once(script, "plan['candidate']['sequence'] == 16", "plan['candidate']['sequence'] == 17")
    script = replace_once(script, "plan['knownPrevious']['sequence'] == 15", "plan['knownPrevious']['sequence'] == 16")
    for name, value in dict(SOURCE_PLAN_SHA=sha(publication_raw), ROOT_SHA=sha(root_raw), NATIVE_SHA=native_hash,
            LITERAL_SHA=sha(literal), OBJECTS_SHA=definition['objectsSha256'], OBJECT_COUNT=len(definition['objects']), **blobs).items():
        script = replace_assignment(script, name, value)
    script, changed = re.subn(r'require\(len\(raw\) == [0-9]+ and',
        'require(len(raw) == ' + str(len(literal)) + ' and', script)
    require(changed == 1, 'reviewed_literal_length_hunk_changed')
    module = advance_source(raw[WORKER + 'src/native-r20-s16-publication.mjs'].decode('utf-8'))
    module = replace_once(module, 'candidate.candidate.sequence === 16', 'candidate.candidate.sequence === 17')
    module = replace_once(module, 'candidate.knownPrevious.sequence === 15', 'candidate.knownPrevious.sequence === 16')
    old_hash = sha(canonical(json.loads(raw[WORKER + 'src/native-r20-s16-publication-plan.json'])))
    module = replace_once(module, "export const NATIVE_PLAN_SHA256 = '" + old_hash + "';",
        "export const NATIVE_PLAN_SHA256 = '" + native_hash + "';")
    for name, value in dict(OBJECT_COUNT=len(definition['objects']), **blobs).items():
        module = replace_assignment(module, name, value, javascript=True)
    return {PLAN_PATH: literal, MODULE_PATH: module.encode('utf-8'), DRIVER_PATH: script.encode('utf-8')}


def render_tests(raw, count, game_count, total, largest):
    node = advance_source(raw[WORKER + 'test/native-r20-s16-publication.test.mjs'].decode('utf-8'))
    python = advance_source(raw['scripts/test_native_r20_s16_r2_delivery.py'].decode('utf-8'))
    node = (node.replace('predecessor15/', 'predecessor16/').replace('fixed16', 'fixed17')
        .replace('R20 and genuine predecessor15', 'R22 and genuine predecessor16')
        .replace('immutable16', 'immutable17').replace('remains15', 'remains16')
        .replace('definition.candidate.sequence, 16', 'definition.candidate.sequence, 17')
        .replace('e.candidate.sequence = 15', 'e.candidate.sequence = 16')
        .replace('definition.objects.length, 41', 'definition.objects.length, ' + str(count))
        .replace("startsWith('releases/game-')).length, 35", "startsWith('releases/game-')).length, " + str(game_count))
        .replace('1077405420', str(total)).replace('largest, 111160320', 'largest, ' + str(largest)))
    python = (python.replace('native_r20_fixed_generator', 'native_r22_fixed_generator')
        .replace('prepare_native_r20_s16_fixed', 'prepare_native_r22_s17_fixed')
        .replace('predecessor15/', 'predecessor16/').replace('actual-cms16/', 'actual-cms17/')
        .replace("PLAN['candidate']['sequence'], 16", "PLAN['candidate']['sequence'], 17")
        .replace("len(PLAN['objects']), 41", "len(PLAN['objects']), " + str(count))
        .replace("len(REAL_PLAN['objects']), 41", "len(REAL_PLAN['objects']), " + str(count))
        .replace("sum(e['path'].startswith('releases/game-') for e in PLAN['objects']), 35", "sum(e['path'].startswith('releases/game-') for e in PLAN['objects']), " + str(game_count))
        .replace('1077405420', str(total)).replace("headers['x-mir2-sequence'] = '15'", "headers['x-mir2-sequence'] = '16'")
        .replace("PLAN_RAW.replace(b'\"sequence\": 16', b'\"sequence\": 17', 1)", "PLAN_RAW.replace(b'\"sequence\": 17', b'\"sequence\": 18', 1)")
        .replace('require16', 'require17').replace('exact_candidate16_required', 'exact_candidate17_required')
        .replace('actual_root16_receipt_required', 'actual_root17_receipt_required')
        .replace('r20_closed_object_scope_required', 'r22_closed_object_scope_required')
        .replace('expected_current15', 'expected_current16').replace('cannot_admit16', 'cannot_admit17')
        .replace("('sequence', 14), ('sequence', 16)", "('sequence', 15), ('sequence', 17)")
        .replace('update(sequence=15)', 'update(sequence=16)'))
    interrupt = advance_source(raw['scripts/test_native_r20_s16_interrupt.py'].decode('utf-8'))
    return {WORKER + 'test/native-r22-s17-publication.test.mjs': node.encode('utf-8'),
        'scripts/test_native_r22_s17_r2_delivery.py': python.encode('utf-8'),
        'scripts/test_native_r22_s17_interrupt.py': interrupt.encode('utf-8')}


def make_outputs(publication_raw, root_raw, feed_raw, signature_raw, bootstrap):
    raw, previous, previous_feed, subject = template_inputs()
    publication, proof, feed, installer = validate_inputs(publication_raw, root_raw, feed_raw, signature_raw,
        bootstrap, previous, previous_feed, subject)
    definition = freeze_definition(publication, proof, installer, bootstrap, previous)
    # Bind exact input bytes, never a normalized JSON reserialization.
    definition.update(sourcePlanSha256=sha(publication_raw), rootVerificationSha256=sha(root_raw))
    outputs = render_runtime(raw, definition, publication_raw, root_raw)
    outputs.update({
        FIXTURE + 'actual-cms17/PUBLICATION-PLAN.json': publication_raw,
        FIXTURE + 'actual-cms17/ROOT-VERIFICATION.json': root_raw,
        FIXTURE + 'preview/artifacts/feed/latest.json': feed_raw,
        FIXTURE + 'preview/artifacts/feed/latest.p7s': signature_raw,
        FIXTURE + 'predecessor16/latest.json': raw['scripts/fixtures/native-r20-s16/preview/artifacts/feed/latest.json'],
        FIXTURE + 'predecessor16/latest.p7s': raw['scripts/fixtures/native-r20-s16/preview/artifacts/feed/latest.p7s'],
        FIXTURE + 'predecessor16/PUBLICATION-PLAN.json': raw['scripts/fixtures/native-r20-s16/actual-cms16/PUBLICATION-PLAN.json'],
        FIXTURE + 'predecessor16/ROOT-VERIFICATION.json': raw['scripts/fixtures/native-r20-s16/actual-cms16/ROOT-VERIFICATION.json'],
    })
    count = len(definition['objects'])
    game_count = sum(obj['path'].startswith(GAME_DIRECTORY + '/') for obj in definition['objects'])
    total = sum(obj['size'] for obj in definition['objects'])
    outputs.update(render_tests(raw, count, game_count, total, max(obj['size'] for obj in definition['objects'])))
    for name, data in outputs.items():
        if name.endswith('.py'):
            ast.parse(data, filename=name)
    report = dict(schema='mir2.r22.s17.fixed-generator.v1', passed=True, candidate=definition['candidate'],
        gameSourceRevision=GAME_SOURCE, gameCandidate=GAME_CANDIDATE, knownPrevious=definition['knownPrevious'],
        sourcePlanSha256=sha(publication_raw), rootVerificationSha256=sha(root_raw),
        nativePlanSha256=sha(canonical(definition)), literalSha256=sha(outputs[PLAN_PATH]),
        literalBytes=len(outputs[PLAN_PATH]), objectsSha256=proof['objectsSha256'],
        objects=count, gameObjects=game_count, totalBytes=total, engineDirectory=ENGINE_DIRECTORY,
        unchangedEngine=True, templates={name: sha(data) for name, data in raw.items()}, templatePins=TEMPLATES,
        rootCmsReverifiedByGenerator=False, rootIssuedReceiptRequired=True, cloudWrites=False,
        oldChannelFilesModified=False, publicStateChanged=False, actualStageAccepted=False,
        actualPromoteAccepted=False, actualWindowsAcceptance=False,
        outputs=[dict(path=name, size=len(data), sha256=sha(data)) for name, data in sorted(outputs.items())])
    outputs[FIXTURE + 'GENERATOR-REPORT.json'] = canonical(report)
    return outputs, report


def output_parent(path):
    for ancestor in [*reversed(path.parent.parents), path.parent]:
        if not ancestor.exists() and not ancestor.is_symlink():
            continue
        info = ancestor.lstat()
        require(stat.S_ISDIR(info.st_mode) and not stat.S_ISLNK(info.st_mode)
                and not (getattr(info, 'st_file_attributes', 0) & 0x400), 'linked_output_parent_rejected')


def owned_output(name):
    relative(name)
    require(name in (PLAN_PATH, MODULE_PATH, DRIVER_PATH,
                    WORKER + 'test/native-r22-s17-publication.test.mjs',
                    'scripts/test_native_r22_s17_r2_delivery.py', 'scripts/test_native_r22_s17_interrupt.py')
            or name.startswith(FIXTURE), 'new_s17_output_scope_required')


def emit(outputs):
    # Complete preflight before the first write. Conflicts preserve all prior
    # attempts. Identical reruns do not replace inode/timestamps or proof bytes.
    for name, data in outputs.items():
        owned_output(name)
        require(isinstance(data, bytes) and 0 < len(data) <= MAX_METADATA, 'bounded_output_required')
        path = BASE / name
        output_parent(path)
        if path.exists() or path.is_symlink():
            require(read(path, MAX_METADATA) == data, 'output_conflict_retained')
    for name, data in outputs.items():
        path = BASE / name
        if path.exists():
            continue
        path.parent.mkdir(parents=True, exist_ok=True)
        output_parent(path)
        with path.open('xb') as stream:
            stream.write(data)
            stream.flush()
            os.fsync(stream.fileno())


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--publication-root', type=Path, required=True)
    parser.add_argument('--feed-root', type=Path, required=True)
    parser.add_argument('--bootstrap-exe', type=Path, required=True)
    args = parser.parse_args()
    try:
        publication_raw = read(args.publication_root / 'PUBLICATION-PLAN.json', CONTROL_LIMIT)
        root_raw = read(args.publication_root / 'ROOT-VERIFICATION.json', CONTROL_LIMIT)
        feed_raw = read(args.feed_root / 'latest.json', 32768)
        signature_raw = read(args.feed_root / 'latest.p7s', 32768)
        outputs, report = make_outputs(publication_raw, root_raw, feed_raw, signature_raw, args.bootstrap_exe)
        emit(outputs)
    except Exception as error:
        label = str(error) if isinstance(error, GuardError) else 'input_or_generation_guard_failed'
        print(json.dumps(dict(passed=False, result=label, cloudWrites=False, oldChannelFilesModified=False)))
        return 2
    print(json.dumps({key: report[key] for key in ('passed', 'candidate', 'objects', 'gameObjects', 'totalBytes',
        'literalBytes', 'nativePlanSha256', 'sourcePlanSha256', 'rootVerificationSha256', 'cloudWrites')}))
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
