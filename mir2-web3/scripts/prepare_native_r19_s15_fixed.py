#!/usr/bin/env python3
"""Freeze the root-verified R19/s15 publication into a bounded new channel.

This is an offline code generator, not a CMS verifier or publisher. Root must
first execute the actual pinned Windows CMS and complete delivery byte gates.
Only those reviewed raw receipts are inputs. No credentials, private keys,
HTTP, signing, cloud mutation, old-channel edits or arbitrary live plan are
used. All outputs are restricted to the new R19/s15 owned paths below.
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
import time

sys.dont_write_bytecode = True
BASE = Path(__file__).resolve().parents[1]
GAME_SOURCE = '2b04e82c4f42b22f895f5f95b5bd79a8147eacdf'
GAME_CANDIDATE = 'WN-CANDIDATE-20261007-invited-19'
ENGINE_SOURCE = 'b4390ee4c987bbf000ba1d95761151e39fc0d54f'
GAME_DIRECTORY = 'releases/game-' + GAME_CANDIDATE
MAX_OBJECTS = 100
MAX_METADATA = 32 * 1024 * 1024
MAX_ARTIFACT = 128 * 1024 * 1024
CONTROL_LIMIT = 65536
SHA = re.compile(r'[0-9a-f]{64}\Z')
IMMUTABLE = 'public, max-age=31536000, immutable'
TEMPLATES = {
    'scripts/native_r17_s14_r2_delivery.py': 'e39df7ccbd7fbd60802db8462169fa09f4b58673928e176d9cb3919badbe0bbc',
    'scripts/test_native_r17_s14_r2_delivery.py': 'bb1a0597bd846e95f0ee663906cb853771cd371b9cffb684d19b04ed4b45dca6',
    'scripts/test_native_r17_s14_interrupt.py': '77365c3bad603f0d1bc857d2e6ffbd945b54c294f22514fae850ae3ee9cada14',
    'infra/cloudflare/mir2-r2-bulk-upload/src/native-r17-s14-publication.mjs': '3f95a78aa1f71ff82e3bb8b89299345e32382e2322a658bbf9c00a7248a20aa0',
    'infra/cloudflare/mir2-r2-bulk-upload/src/native-r17-s14-publication-plan.json': '2bb21fd0a2387de39641e37a3a11663b62610bcc32d8f35e8207e842184a1eb1',
    'infra/cloudflare/mir2-r2-bulk-upload/test/native-r17-s14-publication.test.mjs': 'c2ab0e1bf9967940dd5aec5abb3f21ad442deb4fe0ac2c78994e3aaa4720b36b',
}
LF_TEMPLATE_SHA = {
    'scripts/native_r17_s14_r2_delivery.py': '26efee2476eab7e1da1f6fbcb3827bfb88de8cde55795e13f74b209b8329ae83',
    'scripts/test_native_r17_s14_r2_delivery.py': 'dfc46238280d27c4fa93c038ded5ecec2b97673c5e33ee8f3cfb3430bb57464f',
    'scripts/test_native_r17_s14_interrupt.py': 'aa9bc76c4a7242b82449ad3c7fd11892c0a6f5e5bc44aa5c7d13d09473e1428f',
}


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
        # Windows path/handle stat can give different meanings to ctime. Match
        # device/inode/size/mtime across APIs, then compare each API's full stamp
        # against its own initial observation before and after the read.
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
    raw = {name: read(BASE / name, MAX_METADATA) for name in TEMPLATES}
    # Only the explicit reviewed Windows CRLF and Git/Linux LF byte versions
    # are accepted. Do not broadly normalize or admit an edited old template.
    require(all(sha(raw[name]) in (pin, LF_TEMPLATE_SHA.get(name, pin))
                for name, pin in TEMPLATES.items()), 'reviewed_template_changed')
    spec = importlib.util.spec_from_file_location('frozen_s14_generator_subject',
        BASE / 'scripts/native_r17_s14_r2_delivery.py')
    subject = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(subject)
    previous = subject.load_plan_bytes(raw['infra/cloudflare/mir2-r2-bulk-upload/src/native-r17-s14-publication-plan.json'])
    require(previous['candidate']['sequence'] == 14
            and previous['candidate']['sourceRevision'] == ENGINE_SOURCE, 'exact_predecessor14_required')
    return raw, previous, subject


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
                    previous, subject, *, now=None):
    publication = subject.strict_json(publication_raw, CONTROL_LIMIT)
    root = subject.strict_json(root_raw, CONTROL_LIMIT)
    feed = subject.strict_json(feed_raw, 32768)
    exact(publication, ('schema', 'expectedCurrent', 'candidate', 'verificationReceipt', 'objects'),
          'publication_fields_mismatch')
    require(publication['schema'] == 'mir2.windows.r2-publication.v1'
            and publication['expectedCurrent'] is None, 'reviewed_immutable_plan_required')
    candidate = publication['candidate']
    exact(candidate, ('schema', 'channel', 'platform', 'sequence', 'sourceRevision', 'feedSha256', 'signatureSha256'),
          'candidate_fields_mismatch')
    require(candidate['schema'] == 'mir2.windows.r2-channel.v1'
            and candidate['channel'] == 'invited' and candidate['platform'] == 'windows-x64'
            and type(candidate['sequence']) is int and candidate['sequence'] == 15
            and candidate['sourceRevision'] == ENGINE_SOURCE
            and candidate['feedSha256'] == sha(feed_raw)
            and candidate['signatureSha256'] == sha(signature_raw), 'exact_candidate15_required')
    exact(root, ('schema', 'passed', 'candidateVerified', 'cmsVerified', 'sourceRevision', 'sequence',
                 'feedSha256', 'signatureSha256', 'objectsSha256'), 'root_fields_mismatch')
    require(root['schema'] == 'mir2.windows.r2-verification.v1' and root['passed'] is True
            and root['candidateVerified'] is True and root['cmsVerified'] is True
            and all(root[key] == candidate[key] for key in ('sourceRevision', 'sequence', 'feedSha256', 'signatureSha256')),
            'actual_root15_receipt_required')
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
            and feed['platform'] == 'windows-x64' and type(feed['sequence']) is int and feed['sequence'] == 15
            and feed['protocol'] == 'crystal-mir2-v1' and feed['content'] == 'mir2.windows.package-manifest.v4'
            and type(feed['minBootstrap']) is int and feed['minBootstrap'] in (0, 1)
            and type(feed['createdUnix']) is int and type(feed['expiresUnix']) is int
            and feed['createdUnix'] <= seconds < feed['expiresUnix']
            and 0 < feed['expiresUnix'] - feed['createdUnix'] <= 31 * 86400, 'current_signed_feed15_required')
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
            'exact_r19_game_identity_required')
    engine_directory = feed['engine']['directory']
    require(engine_directory.startswith('releases/updater-') and engine_directory != GAME_DIRECTORY,
            'bounded_engine_directory_required')
    feed_directory = 'feeds/s15-' + candidate['feedSha256']
    expected_feed = [feed_directory + '/latest.json', feed_directory + '/latest.p7s']
    require([entry['path'] for entry in objects[:2]] == expected_feed
            and all(objects[index]['size'] == len(raw) and objects[index]['sha256'] == sha(raw)
                    for index, raw in enumerate((feed_raw, signature_raw))), 'exact_immutable_feed_pair_required')
    for entry in objects:
        path = entry['path']
        require(path.startswith(GAME_DIRECTORY + '/') or path.startswith(engine_directory + '/')
                or path in expected_feed or re.fullmatch(r'installers/[0-9a-f]{64}/Mir2Setup\.exe', path),
                'r19_closed_object_scope_required')
    for entry in objects:
        hash_file(entry['source'], entry['size'], entry['sha256'])
    engine = subject.strict_json(read(by_path[engine_directory + '/ENGINE.json']['source'], 32768), 32768)
    require(engine.get('sourceRevision') == ENGINE_SOURCE, 'exact_engine_b4390_required')
    version = subject.strict_json(read(by_path[GAME_DIRECTORY + '/VERSION.json']['source'], CONTROL_LIMIT), CONTROL_LIMIT)
    require(version.get('gitRevision') == GAME_SOURCE and version.get('candidate') == GAME_CANDIDATE,
            'exact_game_source2b04_required')
    require(set(entry['path'] for entry in objects if entry['path'].startswith(engine_directory + '/'))
            == {engine_directory + '/' + name for name in ('ENGINE.json', 'ENGINE.p7s', 'Mir2Updater.exe')},
            'exact_three_engine_files_required')
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


def replace_function(text, name, next_name, replacement):
    start = text.index('def ' + name + '(')
    end = text.index('def ' + next_name + '(', start)
    return text[:start] + replacement.rstrip() + '\n\n\n' + text[end:]


def make_outputs(publication_raw, root_raw, feed_raw, signature_raw, bootstrap):
    raw, previous, subject = template_inputs()
    publication, proof, feed, installer = validate_inputs(publication_raw, root_raw, feed_raw, signature_raw,
                                                         bootstrap, previous, subject)
    definition = copy.deepcopy(previous)
    definition.update(schema='mir2.native.r19.fixed-origin-plan.v1', sourceRevision=ENGINE_SOURCE,
        sourcePlanSha256=sha(publication_raw), rootVerificationSha256=sha(root_raw),
        candidate=publication['candidate'], objectsSha256=proof['objectsSha256'], knownPrevious=previous['candidate'],
        predecessorProof={key: previous[key] for key in ('sourcePlanSha256', 'rootVerificationSha256')})
    # Frozen proofs live in code, not in control requests. Their raw hashes and
    # exact decoded closure checks remain mandatory. This retains the64KiB limit
    # for a genuinely different complete table of up to100 objects.
    definition.pop('sourcePlanBase64')
    definition.pop('rootVerificationBase64')
    definition['preparation'].update(requiredGameSourceRevision=GAME_SOURCE, requiredGameCandidate=GAME_CANDIDATE,
        gameManifestSha256=next(obj['sha256'] for obj in publication['objects'] if obj['path'] == GAME_DIRECTORY + '/PACKAGE-MANIFEST.json'),
        gameVersionSha256=next(obj['sha256'] for obj in publication['objects'] if obj['path'] == GAME_DIRECTORY + '/VERSION.json'),
        verifierReceiptSource='fixtures/actual-cms15/ROOT-VERIFICATION.json', verifierExecutedThisPhase=True,
        requiredVerifier='Actual pinned Windows CMS; exact R19/source2b04; unchanged engine b4390ee4')
    definition['installerOriginEvidence'].update(expectedSha256=installer['sha256'], expectedSize=installer['size'],
        requestedOriginRelativePath=installer['path'], sourceFixture=str(bootstrap),
        actualPublicBuildReceipt='root-owned R19 Bootstrap receipt; independently full-hashed by generator')
    definition['objects'] = []
    for index, entry in enumerate(publication['objects']):
        path = entry['path']
        obj = {key: entry[key] for key in ('path', 'size', 'sha256')}
        obj.update(index=index, r2Key=definition['r2Prefix'] + path, originRelativePath=path,
            contentType='application/json' if path.endswith('.json') else 'application/pkcs7-signature' if path.endswith('.p7s')
            else 'text/plain; charset=utf-8' if path.endswith('.txt') else 'application/octet-stream', cacheControl=IMMUTABLE)
        definition['objects'].append(obj)
    count = len(definition['objects'])
    game_count = sum(obj['path'].startswith(GAME_DIRECTORY + '/') for obj in definition['objects'])
    total = sum(obj['size'] for obj in definition['objects'])
    literal = (json.dumps(definition, indent=2, ensure_ascii=False) + '\n').encode('utf-8')
    require(len(literal) <= CONTROL_LIMIT, 'fixed_plan_exceeds_unchanged_control_limit')
    native_hash = sha(canonical(definition))
    blobs = dict(SOURCE_PLAN_BASE64=base64.b64encode(publication_raw).decode('ascii'),
                 ROOT_VERIFICATION_BASE64=base64.b64encode(root_raw).decode('ascii'),
                 PREDECESSOR_PLAN_BASE64=previous['sourcePlanBase64'],
                 PREDECESSOR_ROOT_BASE64=previous['rootVerificationBase64'])
    script = raw['scripts/native_r17_s14_r2_delivery.py'].decode('utf-8').replace('\r\n', '\n')
    script = script.replace('Fixed R17 CI', 'Fixed R19/s15 CI').replace('/upload/native-r17-s14', '/upload/native-r19-s15')
    script = script.replace('native-r17-s14-publication-plan.json', 'native-r19-s15-publication-plan.json')
    for name, value in dict(SOURCE_PLAN_SHA=sha(publication_raw), ROOT_SHA=sha(root_raw), NATIVE_SHA=native_hash,
                            LITERAL_SHA=sha(literal), OBJECTS_SHA=proof['objectsSha256']).items():
        script, matches = re.subn(r'^' + name + r" = '[0-9a-f]{64}'$", name + ' = ' + repr(value), script, flags=re.M)
        require(matches == 1, 'cli_hash_anchor_hunk_changed')
    script = replace_once(script, 'CHUNK = 65536', '\n'.join(name + ' = ' + repr(value) for name, value in blobs.items())
        + '\nOBJECT_COUNT = ' + str(count) + '\nCHUNK = 65536')
    script = replace_once(script, 'len(raw) == 63692', 'len(raw) == ' + str(len(literal)))
    script = script.replace('mir2.native.r17.fixed-origin-plan.v1', 'mir2.native.r19.fixed-origin-plan.v1')
    script = script.replace("plan['candidate']['sequence'] == 14", "plan['candidate']['sequence'] == 15")
    script = script.replace("plan['knownPrevious']['sequence'] == 13", "plan['knownPrevious']['sequence'] == 14")
    script = script.replace("'6032ef8b3e27dd97bad0b20c8676ef9f185db64b'", repr(GAME_SOURCE))
    script = script.replace("proof['sourcePlanBase64']", 'PREDECESSOR_PLAN_BASE64').replace("proof['rootVerificationBase64']", 'PREDECESSOR_ROOT_BASE64')
    script = script.replace("plan['sourcePlanBase64']", 'SOURCE_PLAN_BASE64').replace("plan['rootVerificationBase64']", 'ROOT_VERIFICATION_BASE64')
    start = script.index('        old_game = sorted(')
    end = script.index('        operation = ', start)
    script = script[:start] + """        game_prefix = 'releases/game-WN-CANDIDATE-20261007-invited-19/'
        game_objects = [entry for entry in plan['objects'] if entry['path'].startswith('releases/game-')]
        require(plan['preparation']['requiredGameCandidate'] == 'WN-CANDIDATE-20261007-invited-19'
                and game_objects and all(entry['path'].startswith(game_prefix) for entry in game_objects)
                and digest(canonical(closure(plan))) == OBJECTS_SHA, 'plan_invalid')
""" + script[end:]
    script = script.replace("len(plan['objects']) == 36", "len(plan['objects']) == OBJECT_COUNT and 6 <= OBJECT_COUNT <= 100")
    script = script.replace("0 < entry['size'] <= 128 * 1024 * 1024", "0 < entry['size'] <= (32 if re.search(r'\\.(json|p7s|sig|txt)$', entry['path'], re.I) else 128) * 1024 * 1024")
    script = script.replace('for index in range(36):', "for index in range(len(plan['objects'])):")
    script = script.replace("0 <= strict_json(body)['index'] < 36", "0 <= strict_json(body)['index'] < len(self.plan['objects'])")
    script = replace_once(script,
        "or re.fullmatch(re.escape(PRIVATE_PREFIX) + r'/objects/(?:[0-9]|[12][0-9]|3[0-5])', path))",
        "or path in {PRIVATE_PREFIX + '/objects/' + str(entry['index']) for entry in self.plan['objects']})")
    script = script.replace("receipt['verifiedObjectCount'] == 36", "receipt['verifiedObjectCount'] == len(plan['objects'])")
    script = script.replace("stage['verifiedObjectCount'] == 36", "stage['verifiedObjectCount'] == len(plan['objects'])")
    script = script.replace('CMS/closed36 proof', 'CMS/complete R19 proof')
    script = script.replace('current-origin13', 'current-origin15')
    script = script.replace('\r\n', '\n')
    module = raw['infra/cloudflare/mir2-r2-bulk-upload/src/native-r17-s14-publication.mjs'].decode('utf-8')
    module = module.replace('native-r17-s14-publication-plan.json', 'native-r19-s15-publication-plan.json')
    module = module.replace('/upload/native-r17-s14', '/upload/native-r19-s15')
    module = module.replace('NativeR17S14', 'NativeR19S15').replace('nativeR17S14', 'nativeR19S15')
    module = module.replace('preview14', 'R19/s15').replace('current origin13', 'current origin14')
    module = replace_once(module, "export const NATIVE_PLAN_SHA256 = '" + subject.NATIVE_SHA + "';",
        "export const NATIVE_PLAN_SHA256 = '" + native_hash + "';\nconst OBJECT_COUNT = " + str(count) + ';\n'
        + '\n'.join('const ' + name + ' = ' + json.dumps(value) + ';' for name, value in blobs.items()))
    module = module.replace('candidate.candidate.sequence === 14', 'candidate.candidate.sequence === 15')
    module = module.replace('candidate.knownPrevious.sequence === 13', 'candidate.knownPrevious.sequence === 14')
    module = module.replace('6032ef8b3e27dd97bad0b20c8676ef9f185db64b', GAME_SOURCE)
    module = module.replace('candidate.predecessorProof.sourcePlanBase64', 'PREDECESSOR_PLAN_BASE64')
    module = module.replace('candidate.predecessorProof.rootVerificationBase64', 'PREDECESSOR_ROOT_BASE64')
    module = module.replace('candidate.sourcePlanBase64', 'SOURCE_PLAN_BASE64').replace('candidate.rootVerificationBase64', 'ROOT_VERIFICATION_BASE64')
    start = module.index('  const oldGame = ')
    end = module.index('  for (const [index, entry]', start)
    module = module[:start] + """  const gameObjects = candidate.objects.filter(entry => entry.path.startsWith('releases/game-'));
  requireValue(candidate.schema === 'mir2.native.r19.fixed-origin-plan.v1'
    && candidate.preparation.requiredGameCandidate === 'WN-CANDIDATE-20261007-invited-19'
    && gameObjects.length > 0 && gameObjects.every(entry => entry.path.startsWith('releases/game-WN-CANDIDATE-20261007-invited-19/'))
    && OBJECT_COUNT >= 6 && OBJECT_COUNT <= 100 && candidate.objects.length === OBJECT_COUNT
    && await digest(ENCODER.encode(canonical(closure(candidate.objects))), cryptoApi) === candidate.objectsSha256,
    'prepared_closure_mismatch', 503);
""" + module[end:]
    module = module.replace('candidate.objects.length === 36', 'candidate.objects.length === OBJECT_COUNT')
    module = module.replace('entry.size <= 128 * 1024 * 1024', "entry.size <= (/\\.(json|p7s|sig|txt)$/i.test(entry.path) ? 32 : 128) * 1024 * 1024")
    require('oldGame.length === 30' not in module and 'range(36)' not in script
            and "len(plan['objects']) == 36" not in script, 'old_game_or_object_count_assumption_retained')
    outputs = {
        'infra/cloudflare/mir2-r2-bulk-upload/src/native-r19-s15-publication-plan.json': literal,
        'infra/cloudflare/mir2-r2-bulk-upload/src/native-r19-s15-publication.mjs': module.encode('utf-8'),
        'scripts/native_r19_s15_r2_delivery.py': script.encode('utf-8'),
        'scripts/fixtures/native-r19-s15/actual-cms15/PUBLICATION-PLAN.json': publication_raw,
        'scripts/fixtures/native-r19-s15/actual-cms15/ROOT-VERIFICATION.json': root_raw,
        'scripts/fixtures/native-r19-s15/preview/artifacts/feed/latest.json': feed_raw,
        'scripts/fixtures/native-r19-s15/preview/artifacts/feed/latest.p7s': signature_raw,
        'scripts/fixtures/native-r19-s15/predecessor14/latest.json': read(BASE / 'scripts/fixtures/native-r17-s14/preview/artifacts/feed/latest.json', 32768),
        'scripts/fixtures/native-r19-s15/predecessor14/latest.p7s': read(BASE / 'scripts/fixtures/native-r17-s14/preview/artifacts/feed/latest.p7s', 32768),
        'scripts/fixtures/native-r19-s15/predecessor14/PUBLICATION-PLAN.json': base64.b64decode(previous['sourcePlanBase64'], validate=True),
        'scripts/fixtures/native-r19-s15/predecessor14/ROOT-VERIFICATION.json': base64.b64decode(previous['rootVerificationBase64'], validate=True),
    }
    require(sha(outputs['scripts/fixtures/native-r19-s15/predecessor14/latest.json']) == previous['candidate']['feedSha256']
            and sha(outputs['scripts/fixtures/native-r19-s15/predecessor14/latest.p7s']) == previous['candidate']['signatureSha256'],
            'real_predecessor_fixture_changed')
    outputs.update(make_tests(raw, count, game_count, total, max(obj['size'] for obj in definition['objects'])))
    for name, data in outputs.items():
        if name.endswith('.py'):
            ast.parse(data, filename=name)
    report = dict(schema='mir2.r19.s15.fixed-generator.v1', passed=True, candidate=definition['candidate'],
        gameSourceRevision=GAME_SOURCE, gameCandidate=GAME_CANDIDATE, knownPrevious=definition['knownPrevious'],
        sourcePlanSha256=sha(publication_raw), rootVerificationSha256=sha(root_raw), nativePlanSha256=native_hash,
        literalSha256=sha(literal), literalBytes=len(literal), objectsSha256=proof['objectsSha256'],
        objects=count, gameObjects=game_count, totalBytes=total,
        templates={name: sha(data) for name, data in raw.items()}, templatePins=TEMPLATES,
        linuxTemplatePins=LF_TEMPLATE_SHA,
        rootCmsReverifiedByGenerator=False, rootIssuedReceiptRequired=True, cloudWrites=False,
        oldChannelFilesModified=False, publicStateChanged=False,
        outputs=[dict(path=name, size=len(data), sha256=sha(data)) for name, data in sorted(outputs.items())])
    outputs['scripts/fixtures/native-r19-s15/GENERATOR-REPORT.json'] = canonical(report)
    return outputs, report


GENERATOR_TESTS = r'''
# Generator negative cases do not mint CMS admission. They use the genuine raw
# new receipt, then deliberately change its binding before any source-byte I/O.
# The one actual full-closure generation is witnessed by GENERATOR-REPORT.json;
# its Windows source paths are not assumed present on a clean Linux CI runner.
GEN_SPEC = importlib.util.spec_from_file_location('native_r19_fixed_generator',
    BASE / 'scripts/prepare_native_r19_s15_fixed.py')
G = importlib.util.module_from_spec(GEN_SPEC)
GEN_SPEC.loader.exec_module(G)


class FixedGeneratorTests(unittest.TestCase):
    def inputs(self):
        source = json.loads((FIXTURE_ROOT / 'actual-cms15/PUBLICATION-PLAN.json').read_bytes())
        root = json.loads((FIXTURE_ROOT / 'actual-cms15/ROOT-VERIFICATION.json').read_bytes())
        return source, root

    def rejected(self, label, mutation, feed=FEED, cms=CMS):
        source, root = self.inputs()
        mutation(source, root)
        root_raw = G.canonical(root)
        source['verificationReceipt']['sha256'] = G.sha(root_raw)
        with patch.object(G, 'hash_file') as full_hash:
            with self.assertRaises(G.GuardError) as failure:
                G.validate_inputs(G.canonical(source), root_raw, feed, cms,
                    Path(__file__).resolve(), REAL_PLAN, P, now=json.loads(FEED)['createdUnix'] + 100)
        self.assertEqual(str(failure.exception), label)
        full_hash.assert_not_called()

    def test_generator_old_candidate_and_old_signature_cannot_admit15(self):
        self.rejected('exact_candidate15_required', lambda s, r: s['candidate'].update(sequence=14))
        old_cms = (FIXTURE_ROOT / 'predecessor14/latest.p7s').read_bytes()
        self.rejected('exact_candidate15_required', lambda s, r: None, cms=old_cms)

    def test_generator_false_root_and_changed_root_closure_rejected(self):
        self.rejected('actual_root15_receipt_required', lambda s, r: r.update(cmsVerified=False))
        self.rejected('actual_root_closure_mismatch', lambda s, r: r.update(objectsSha256='0' * 64))

    def test_generator_duplicate_and_101_objects_rejected_without_source_read(self):
        self.rejected('publication_object_bound',
            lambda s, r: s.update(objects=[copy.deepcopy(s['objects'][0]) for _ in range(101)]))
        self.rejected('sorted_unique_closure_required',
            lambda s, r: s['objects'].append(copy.deepcopy(s['objects'][0])))

    def test_generator_metadata_and_artifact_limits_are_not_widened(self):
        def oversize(source, root, suffix, limit):
            item = next(e for e in source['objects'] if e['path'].endswith(suffix))
            item['size'] = limit + 1
        self.rejected('object_size_hash_bound',
            lambda s, r: oversize(s, r, '/PACKAGE-MANIFEST.json', 32 * 1024 * 1024))
        self.rejected('object_size_hash_bound',
            lambda s, r: oversize(s, r, '/mir2-platform-windows.exe', 128 * 1024 * 1024))

    def test_generator_unknown_scope_rejected_even_with_rebound_closure(self):
        def mutate(source, root):
            item = next(e for e in source['objects'] if e['path'].endswith('/mir2-platform-windows.exe'))
            item['path'] = 'releases/unreviewed/mir2-platform-windows.exe'
            source['objects'].sort(key=lambda e: e['path'])
            root['objectsSha256'] = G.sha(G.canonical(G.closure(source['objects'])))
        self.rejected('r19_closed_object_scope_required', mutate)

    def test_generator_missing_manifest_cannot_use_false_complete_root(self):
        def mutate(source, root):
            source['objects'] = [e for e in source['objects'] if not e['path'].endswith('/PACKAGE-MANIFEST.json')]
            root['objectsSha256'] = G.sha(G.canonical(G.closure(source['objects'])))
        self.rejected('component_metadata_byte_binding', mutate)

    def test_generator_hash_rejects_changed_bytes_and_retains_failure(self):
        FIXTURES.mkdir(parents=True, exist_ok=True)
        directory = Path(tempfile.mkdtemp(prefix='generator-hash-', dir=FIXTURES))
        path = directory / 'source.bin'
        path.write_bytes(b'changed source bytes')
        with self.assertRaises(G.GuardError) as failure:
            G.hash_file(path, path.stat().st_size, G.sha(b'original source bytes'))
        self.assertEqual(str(failure.exception), 'source_full_hash_mismatch')
        self.assertEqual(path.read_bytes(), b'changed source bytes')

    def test_generator_conflict_preserves_every_existing_output_before_writing(self):
        FIXTURES.mkdir(parents=True, exist_ok=True)
        directory = Path(tempfile.mkdtemp(prefix='generator-conflict-', dir=FIXTURES))
        destination = directory / 'scripts/fixtures/native-r19-s15/proof.txt'
        destination.parent.mkdir(parents=True)
        destination.write_bytes(b'retained failed proof')
        with patch.object(G, 'BASE', directory):
            with self.assertRaises(G.GuardError) as failure:
                G.emit({'scripts/fixtures/native-r19-s15/first.txt': b'new',
                        'scripts/fixtures/native-r19-s15/proof.txt': b'changed'})
        self.assertEqual(str(failure.exception), 'output_conflict_retained')
        self.assertFalse(destination.with_name('first.txt').exists())
        self.assertEqual(destination.read_bytes(), b'retained failed proof')

    def test_generator_identical_repeat_is_create_only_and_idempotent(self):
        FIXTURES.mkdir(parents=True, exist_ok=True)
        directory = Path(tempfile.mkdtemp(prefix='generator-idempotent-', dir=FIXTURES))
        output = {'scripts/fixtures/native-r19-s15/proof.txt': b'original exact proof'}
        with patch.object(G, 'BASE', directory):
            G.emit(output)
            destination = directory / next(iter(output))
            before = destination.stat()
            G.emit(output)
        self.assertEqual(destination.stat().st_ino, before.st_ino)
        self.assertEqual(destination.stat().st_mtime_ns, before.st_mtime_ns)
        self.assertEqual(destination.read_bytes(), b'original exact proof')
'''


def make_tests(raw, count, game_count, total, largest=111013376):
    # Adapt the already-reviewed transport/stream/CAS suite, retaining the
    # immutable prepared13 negative fixture and every rejection/interruption.
    node = raw['infra/cloudflare/mir2-r2-bulk-upload/test/native-r17-s14-publication.test.mjs'].decode('utf-8')
    node = node.replace('../src/native-r17-s14-publication', '../src/native-r19-s15-publication')
    node = node.replace('NativeR17S14', 'NativeR19S15').replace('native-r17-s14/preview/', 'native-r19-s15/preview/')
    node = node.replace('native-r17-s14/predecessor13/', 'native-r19-s15/predecessor14/').replace('/upload/native-r17-s14', '/upload/native-r19-s15')
    node = node.replace('definition.candidate.sequence, 14', 'definition.candidate.sequence, 15')
    node = node.replace('6032ef8b3e27dd97bad0b20c8676ef9f185db64b', GAME_SOURCE)
    node = node.replace('e.candidate.sequence = 13', 'e.candidate.sequence = 14')
    node = node.replace('definition.objects.length, 36', 'definition.objects.length, ' + str(count))
    node = node.replace("definition.objects.filter(e => e.path.startsWith('releases/game-')).length, 30", "definition.objects.filter(e => e.path.startsWith('releases/game-')).length, " + str(game_count))
    node = node.replace('817841942', str(total)).replace('p.objects[35]', 'p.objects.at(-1)')
    node = replace_once(node,
        "JSON.parse(Buffer.from(definition.predecessorProof.sourcePlanBase64, 'base64'))",
        "JSON.parse(readFileSync(new URL('../../../../scripts/fixtures/native-r19-s15/predecessor14/PUBLICATION-PLAN.json', import.meta.url)))")
    node = node.replace('p.predecessorProof.rootVerificationBase64',
        "readFileSync(new URL('../../../../scripts/fixtures/native-r19-s15/predecessor14/ROOT-VERIFICATION.json', import.meta.url)).toString('base64')")
    node = node.replace('{length: 1158}', '{length: FEED_BYTES.byteLength + 1}')
    node = node.replace('f.bucket.bytesRead, 111013376', 'f.bucket.bytesRead, entry.size')
    node = node.replace('f.stats.fixedSizes, [111013376]', 'f.stats.fixedSizes, [entry.size]')
    node = node.replace('largest, 111013376', 'largest, ' + str(largest))
    node = node.replace('f.bucket.records.size, 37', 'f.bucket.records.size, definition.objects.length + 1')
    node = node.replace('111MB artifact', 'actual game executable artifact')
    node = node.replace('immutable13 preview cannot promote when current origin latest pair is still12',
        'immutable15 preview cannot promote while current origin latest pair remains14')
    node = node.replace('{index: 36}', '{index: definition.objects.length}')
    node = node.replace("'/objects/36'", "'/objects/' + definition.objects.length")
    node = node.replace(".filter(call => call.operation === 'head').length, 36", ".filter(call => call.operation === 'head').length, definition.objects.length")
    node = node.replace('fixed14 closure and previous proof bind30 original game objects plus6 new objects', 'fixed15 closure binds exact new R19 and genuine predecessor14')
    node = node.replace('preview14', 'R19/s15').replace('only after 36 checks', 'only after all fixed object checks')
    python = raw['scripts/test_native_r17_s14_r2_delivery.py'].decode('utf-8')
    python = python.replace("BASE / 'scripts/native_r17_s14_r2_delivery.py'", "BASE / 'scripts/native_r19_s15_r2_delivery.py'")
    python = python.replace("FIXTURE_ROOT = BASE / 'scripts/fixtures/native-r17-s14'", "FIXTURE_ROOT = BASE / 'scripts/fixtures/native-r19-s15'")
    python = python.replace("PLAN['candidate']['sequence'], 14", "PLAN['candidate']['sequence'], 15")
    python = python.replace('6032ef8b3e27dd97bad0b20c8676ef9f185db64b', GAME_SOURCE)
    python = python.replace("PLAN['predecessorProof']['rootVerificationBase64']", 'P.PREDECESSOR_ROOT_BASE64')
    python = python.replace("changed['objects'][35]", "changed['objects'][-1]")
    python = python.replace("headers['x-mir2-sequence'] = '13'", "headers['x-mir2-sequence'] = '14'")
    python = python.replace("verifiedObjectCount=36", "verifiedObjectCount=len(PLAN['objects'])")
    python = python.replace("len(PLAN['objects']), 36", "len(PLAN['objects']), " + str(count))
    python = python.replace("len(REAL_PLAN['objects']), 36", "len(REAL_PLAN['objects']), " + str(count))
    python = python.replace("sum(e['path'].startswith('releases/game-') for e in PLAN['objects']), 30", "sum(e['path'].startswith('releases/game-') for e in PLAN['objects']), " + str(game_count))
    python = python.replace('817841942', str(total))
    python = python.replace('self.assertEqual(len(gets), 36)', "self.assertEqual(len(gets), len(REAL_PLAN['objects']))")
    python = python.replace('transport.stored.update(range(36))', "transport.stored.update(range(len(REAL_PLAN['objects'])))")
    python = python.replace('self.assertGreaterEqual(len(heads), 36)', "self.assertGreaterEqual(len(heads), len(REAL_PLAN['objects']))")
    python = python.replace('), 36)', "), len(REAL_PLAN['objects']))")
    python = python.replace("('verifiedObjectCount', 35)", "('verifiedObjectCount', len(REAL_PLAN['objects']) - 1)")
    python = python.replace("PLAN_RAW.replace(b'\"sequence\": 14', b'\"sequence\": 15', 1)", "PLAN_RAW.replace(b'\"sequence\": 15', b'\"sequence\": 16', 1)")
    python = python.replace("entry = transport.plan['objects'][35]", "entry = transport.plan['objects'][-1]")
    python = python.replace("entry = REAL_PLAN['objects'][32]",
        "entry = next(item for item in REAL_PLAN['objects'] if item['path'].endswith('/mir2-platform-windows.exe'))")
    python = python.replace("call['receivedBytes'], 111013376", "call['receivedBytes'], entry['size']")
    python = python.replace('test_111mb_synthetic_stream', 'test_actual_exe_synthetic_stream')
    python = python.replace("FIXTURES = BASE / 'evidence/fixtures'", "FIXTURES = FIXTURE_ROOT / 'qa-runs/fixtures'")
    python = python.replace("'evidence/TEST-RECEIPT-'", "'scripts/fixtures/native-r19-s15/qa-runs/TEST-RECEIPT-'")
    python = python.replace("'source36Accepted'", "'sourceClosureAccepted'").replace("'public36Accepted'", "'publicClosureAccepted'")
    python = python.replace('toy remaining34; synthetic111MB', 'toy remaining objects; synthetic exact-size executable')
    python = python.replace('literal14 table', 'literal15 R19 table').replace('literal14', 'literal15').replace('preview14', 'R19/s15')
    python = python.replace('test_exact13_table_real_feed_and_original30_game_hashes', 'test_exact15_table_real_feed_and_closed_r19_game_hashes')
    python = python.replace('test_new_alias_sequence_headers_require13', 'test_new_alias_sequence_headers_require15')
    python = python.replace('test_partial_actual_feed_streams_do_not_mint36_object_pass', 'test_partial_actual_feed_streams_do_not_mint_complete_object_pass')
    python = python.replace('No whole36 or CMS success', 'No complete closure or CMS success')
    python = python.replace('source36 publication', 'complete source publication')
    python = python.replace('\r\n', '\n')
    python = replace_once(python, "if __name__ == '__main__':", GENERATOR_TESTS + "\n\nif __name__ == '__main__':")
    interrupt = raw['scripts/test_native_r17_s14_interrupt.py'].decode('utf-8').replace('scripts/native_r17_s14_r2_delivery.py', 'scripts/native_r19_s15_r2_delivery.py')
    interrupt = interrupt.replace("'evidence/interrupt-fixtures'", "'scripts/fixtures/native-r19-s15/qa-runs/interrupt-fixtures'")
    interrupt = interrupt.replace('\r\n', '\n')
    return {
        'infra/cloudflare/mir2-r2-bulk-upload/test/native-r19-s15-publication.test.mjs': node.encode('utf-8'),
        'scripts/test_native_r19_s15_r2_delivery.py': python.encode('utf-8'),
        'scripts/test_native_r19_s15_interrupt.py': interrupt.encode('utf-8'),
    }


def output_parent(path):
    for ancestor in [*reversed(path.parent.parents), path.parent]:
        if not ancestor.exists() and not ancestor.is_symlink():
            continue
        info = ancestor.lstat()
        require(stat.S_ISDIR(info.st_mode) and not stat.S_ISLNK(info.st_mode)
                and not (getattr(info, 'st_file_attributes', 0) & 0x400), 'linked_output_parent_rejected')


def emit(outputs):
    # Check every destination before the first write. Re-running with identical
    # inputs is permitted; conflicts never overwrite a retained prior attempt.
    for name, data in outputs.items():
        path = BASE / name
        output_parent(path)
        if path.exists() or path.is_symlink():
            require(read(path, max(MAX_METADATA, len(data))) == data, 'output_conflict_retained')
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
