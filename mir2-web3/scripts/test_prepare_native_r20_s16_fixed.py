"""Offline s16 generator checks before root supplies signed R20 inputs.

Real frozen s15 bytes are pinned/read. Mutated in-memory policy inputs are
negative cases only: no fake CMS admission, signed feed, public-stage success
or generated publication is written. Pure rendering uses unadmitted data and
checks source transformations; it never invokes make_outputs or emission.
"""
from __future__ import annotations

import ast
import copy
import importlib.util
import io
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time
import unittest
from unittest.mock import patch

sys.dont_write_bytecode = True
BASE = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location('s16_offline_generator',
    BASE / 'scripts/prepare_native_r20_s16_fixed.py')
G = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(G)
RAW, PREVIOUS, PREVIOUS_FEED, P = G.template_inputs()
FIXTURE = BASE / G.FIXTURE / 'qa-runs/generator-policy'


def negative_inputs():
    """Deliberately unsigned policy input, never a verifier-produced receipt."""
    source = json.loads(RAW['scripts/fixtures/native-r19-s15/actual-cms15/PUBLICATION-PLAN.json'])
    root = json.loads(RAW['scripts/fixtures/native-r19-s15/actual-cms15/ROOT-VERIFICATION.json'])
    feed = copy.deepcopy(PREVIOUS_FEED)
    feed['sequence'] = 16
    feed['game']['directory'] = G.GAME_DIRECTORY
    feed['game']['identity'] = G.GAME_CANDIDATE
    feed_raw = G.canonical(feed)
    # This explicit marker cannot be a real signed CMS object.
    signature = b'UNSIGNED_IN_MEMORY_NEGATIVE_POLICY_INPUT_ONLY'
    source['expectedCurrent'] = copy.deepcopy(PREVIOUS['candidate'])
    source['candidate'].update(sequence=16, feedSha256=G.sha(feed_raw), signatureSha256=G.sha(signature))
    old_game = PREVIOUS_FEED['game']['directory']
    for entry in source['objects']:
        if entry['path'].startswith(old_game + '/'):
            entry['path'] = G.GAME_DIRECTORY + entry['path'][len(old_game):]
        elif entry['path'].startswith('feeds/'):
            suffix = entry['path'].rsplit('/', 1)[1]
            entry['path'] = 'feeds/s16-' + G.sha(feed_raw) + '/' + suffix
            raw = feed_raw if suffix == 'latest.json' else signature
            entry.update(size=len(raw), sha256=G.sha(raw))
    source['objects'].sort(key=lambda entry: entry['path'])
    root.update(sequence=16, feedSha256=G.sha(feed_raw), signatureSha256=G.sha(signature))
    root['objectsSha256'] = G.sha(G.canonical(G.closure(source['objects'])))
    return source, root, feed, signature


class FixedGeneratorPolicyTests(unittest.TestCase):
    def rejected(self, label, mutate, *, update_closure=False, rebind_feed=False):
        source, root, feed, signature = negative_inputs()
        mutate(source, root, feed)
        feed_raw = G.canonical(feed)
        if rebind_feed:
            source['candidate']['feedSha256'] = root['feedSha256'] = G.sha(feed_raw)
            for entry in source['objects']:
                if entry['path'].startswith('feeds/'):
                    suffix = entry['path'].rsplit('/', 1)[1]
                    entry['path'] = 'feeds/s16-' + G.sha(feed_raw) + '/' + suffix
                    if suffix == 'latest.json':
                        entry.update(size=len(feed_raw), sha256=G.sha(feed_raw))
            update_closure = True
        if update_closure:
            root['objectsSha256'] = G.sha(G.canonical(G.closure(source['objects'])))
        root_raw = G.canonical(root)
        source['verificationReceipt']['sha256'] = G.sha(root_raw)
        with patch.object(G, 'hash_file', side_effect=AssertionError('Negative case must reject before source I/O')) as full_hash:
            with self.assertRaisesRegex(G.GuardError, '^' + label + '$'):
                G.validate_inputs(G.canonical(source), root_raw, feed_raw, signature,
                    Path(__file__).resolve(), PREVIOUS, PREVIOUS_FEED, P,
                    now=PREVIOUS_FEED['createdUnix'] + 100)
        full_hash.assert_not_called()

    def test_frozen_predecessor_and_engine_are_pinned_full_bytes(self):
        self.assertEqual(len(RAW), 10)
        self.assertEqual(PREVIOUS['candidate']['sequence'], 15)
        self.assertEqual(PREVIOUS_FEED['engine']['directory'], G.ENGINE_DIRECTORY)
        for name, expected in G.TEMPLATES.items():
            self.assertEqual(G.sha(RAW[name]), expected)

    def test_old_actual_s15_cannot_admit_s16(self):
        with patch.object(G, 'hash_file') as full_hash:
            with self.assertRaisesRegex(G.GuardError, '^exact_expected_current15_required$'):
                G.validate_inputs(RAW['scripts/fixtures/native-r19-s15/actual-cms15/PUBLICATION-PLAN.json'],
                    RAW['scripts/fixtures/native-r19-s15/actual-cms15/ROOT-VERIFICATION.json'],
                    RAW['scripts/fixtures/native-r19-s15/preview/artifacts/feed/latest.json'],
                    RAW['scripts/fixtures/native-r19-s15/preview/artifacts/feed/latest.p7s'],
                    Path(__file__).resolve(), PREVIOUS, PREVIOUS_FEED, P,
                    now=PREVIOUS_FEED['createdUnix'] + 100)
        full_hash.assert_not_called()

    def test_expected_current_requires_exact_frozen_actual15_not_none_or_tampered_fields(self):
        self.rejected('exact_expected_current15_required', lambda s, r, f: s.update(expectedCurrent=None))
        for key, value in (('schema', 'wrong'), ('sequence', 14), ('sequence', 16),
                           ('sourceRevision', G.GAME_SOURCE), ('feedSha256', '0' * 64),
                           ('signatureSha256', '0' * 64), ('channel', 'other'), ('platform', 'other')):
            self.rejected('exact_expected_current15_required',
                lambda s, r, f: s['expectedCurrent'].update({key: value}))
        self.rejected('exact_expected_current15_required',
            lambda s, r, f: s['expectedCurrent'].update(extra='unknown field'))

    def test_identity_and_signature_rebinding_are_not_cli_admission(self):
        for mutation in (
            lambda s, r, f: s['candidate'].update(sequence=15),
            lambda s, r, f: s['candidate'].update(sequence=True),
            lambda s, r, f: s['candidate'].update(sourceRevision=G.GAME_SOURCE),
            lambda s, r, f: s['candidate'].update(signatureSha256='0' * 64),
            lambda s, r, f: s['candidate'].update(feedSha256='0' * 64),
        ):
            self.rejected('exact_candidate16_required', mutation)

    def test_root_receipt_true_and_exact_candidate_are_mandatory(self):
        for mutation in (
            lambda s, r, f: r.update(cmsVerified=False),
            lambda s, r, f: r.update(candidateVerified=False),
            lambda s, r, f: r.update(passed=1),
            lambda s, r, f: r.update(sequence=15),
            lambda s, r, f: r.update(sourceRevision=G.GAME_SOURCE),
        ):
            self.rejected('actual_root16_receipt_required', mutation)
        self.rejected('actual_root_closure_mismatch', lambda s, r, f: r.update(objectsSha256='0' * 64))

    def test_duplicates_unknown_fields_and_101_objects_reject_before_source_read(self):
        self.rejected('publication_object_bound',
            lambda s, r, f: s.update(objects=[copy.deepcopy(s['objects'][0]) for _ in range(101)]))
        self.rejected('sorted_unique_closure_required', lambda s, r, f: s['objects'].append(copy.deepcopy(s['objects'][0])))
        self.rejected('object_fields_mismatch', lambda s, r, f: s['objects'][0].update(unreviewed=True))
        self.rejected('publication_fields_mismatch', lambda s, r, f: s.update(sourceOverride=G.GAME_SOURCE))

    def test_metadata_and_artifact_limits_are_unchanged(self):
        for suffix, maximum in (('/PACKAGE-MANIFEST.json', 32 * 1024 * 1024),
                                ('/mir2-platform-windows.exe', 128 * 1024 * 1024)):
            self.rejected('object_size_hash_bound',
                lambda s, r, f: next(e for e in s['objects'] if e['path'].endswith(suffix)).update(size=maximum + 1))

    def test_engine_executable_changed_hash_rejects_rebound_root(self):
        self.rejected('exact_s15_engine_closure_required',
            lambda s, r, f: next(e for e in s['objects'] if e['path'].endswith('/Mir2Updater.exe')).update(sha256='0' * 64),
            update_closure=True)

    def test_engine_executable_changed_size_rejects_rebound_root(self):
        self.rejected('exact_s15_engine_closure_required',
            lambda s, r, f: next(e for e in s['objects'] if e['path'].endswith('/Mir2Updater.exe')).update(size=1952257),
            update_closure=True)

    def test_engine_extra_object_cannot_extend_original_directory(self):
        def mutation(source, root, feed):
            item = copy.deepcopy(next(e for e in source['objects'] if e['path'].endswith('/Mir2Updater.exe')))
            item['path'] = G.ENGINE_DIRECTORY + '/extra.exe'
            source['objects'].append(item)
            source['objects'].sort(key=lambda e: e['path'])
        self.rejected('exact_s15_engine_closure_required', mutation, update_closure=True)

    def test_engine_directory_cannot_be_renamed_even_with_rebound_feed_and_root(self):
        def mutation(source, root, feed):
            feed['engine']['directory'] = 'releases/updater-9213EC3FE1A992F7-s16'
            for item in source['objects']:
                if item['path'].startswith(G.ENGINE_DIRECTORY + '/'):
                    item['path'] = feed['engine']['directory'] + item['path'][len(G.ENGINE_DIRECTORY):]
        self.rejected('exact_s15_engine_component_required', mutation, rebind_feed=True)

    def test_engine_identity_cannot_change_even_with_rebound_feed_and_root(self):
        self.rejected('exact_s15_engine_component_required',
            lambda s, r, f: f['engine'].update(identity='2'), rebind_feed=True)

    def test_engine_metadata_changed_hash_cannot_change_even_with_all_bindings_rebound(self):
        def mutation(source, root, feed):
            next(e for e in source['objects'] if e['path'].endswith('/ENGINE.json'))['sha256'] = '0' * 64
            next(e for e in feed['engine']['metadata'] if e['path'] == 'ENGINE.json')['sha256'] = '0' * 64
        self.rejected('exact_s15_engine_component_required', mutation, rebind_feed=True)

    def test_feed_cannot_widen_validity_or_use_bool_sequence(self):
        for mutation in (
            lambda s, r, f: f.update(expiresUnix=f['createdUnix'] + 32 * 86400),
            lambda s, r, f: f.update(expiresUnix=f['createdUnix'] + 10),
            lambda s, r, f: f.update(sequence=True),
            lambda s, r, f: f.update(minBootstrap=True),
        ):
            self.rejected('current_signed_feed16_required', mutation, rebind_feed=True)

    def test_r20_game_identity_cannot_change_even_with_rebound_feed_and_root(self):
        self.rejected('exact_r20_game_identity_required',
            lambda s, r, f: f['game'].update(identity='WN-CANDIDATE-20261007-invited-19'), rebind_feed=True)

    def test_unknown_or_old_game_artifact_scope_rejects_rebound_root(self):
        for directory in ('releases/unreviewed', PREVIOUS_FEED['game']['directory']):
            def mutation(source, root, feed):
                item = next(e for e in source['objects'] if e['path'].endswith('/mir2-platform-windows.exe'))
                item['path'] = directory + '/mir2-platform-windows.exe'
                source['objects'].sort(key=lambda e: e['path'])
            self.rejected('r20_closed_object_scope_required', mutation, update_closure=True)

    def test_missing_manifest_rejects_false_complete_root(self):
        self.rejected('component_metadata_byte_binding',
            lambda s, r, f: s.update(objects=[e for e in s['objects'] if not e['path'].endswith('/PACKAGE-MANIFEST.json')]),
            update_closure=True)

    def test_duplicate_json_and_bom_are_rejected(self):
        for raw in (b'{"schema":1,"schema":2}', b'\xef\xbb\xbf{}'):
            with self.assertRaises(P.Fault):
                P.strict_json(raw)

    def test_raw_hashes_reject_changed_source_bytes_and_retain_failure(self):
        FIXTURE.mkdir(parents=True, exist_ok=True)
        directory = Path(tempfile.mkdtemp(prefix='source-full-hash-', dir=FIXTURE))
        path = directory / 'source.bin'
        path.write_bytes(b'changed bytes')
        with self.assertRaisesRegex(G.GuardError, '^source_full_hash_mismatch$'):
            G.hash_file(path, path.stat().st_size, G.sha(b'original bytes'))
        self.assertEqual(path.read_bytes(), b'changed bytes')

    def test_emit_preflights_all_destinations_before_any_write(self):
        FIXTURE.mkdir(parents=True, exist_ok=True)
        directory = Path(tempfile.mkdtemp(prefix='conflict-', dir=FIXTURE))
        destination = directory / G.FIXTURE / 'proof.txt'
        destination.parent.mkdir(parents=True)
        destination.write_bytes(b'retained failed proof')
        with patch.object(G, 'BASE', directory):
            with self.assertRaisesRegex(G.GuardError, '^output_conflict_retained$'):
                G.emit({G.FIXTURE + 'first.txt': b'new', G.FIXTURE + 'proof.txt': b'changed'})
        self.assertFalse(destination.with_name('first.txt').exists())
        self.assertEqual(destination.read_bytes(), b'retained failed proof')

    def test_emit_identical_repeat_is_create_only_and_idempotent(self):
        FIXTURE.mkdir(parents=True, exist_ok=True)
        directory = Path(tempfile.mkdtemp(prefix='idempotent-', dir=FIXTURE))
        output = {G.FIXTURE + 'proof.txt': b'exact original proof'}
        with patch.object(G, 'BASE', directory):
            G.emit(output)
            destination = directory / next(iter(output))
            before = destination.stat()
            G.emit(output)
        self.assertEqual(destination.stat().st_ino, before.st_ino)
        self.assertEqual(destination.stat().st_mtime_ns, before.st_mtime_ns)

    def test_emit_cannot_write_old_channel_or_arbitrary_path(self):
        for name in ('scripts/native_r19_s15_r2_delivery.py', '../outside.txt',
                     'scripts/fixtures/native-r19-s15/proof.txt', '.github/workflows/web-assets-r2-release.yml'):
            with self.assertRaises(G.GuardError):
                G.emit({name: b'forbidden'})


class UnadmittedRenderingTests(unittest.TestCase):
    def rendered(self):
        source, proof, feed, signature = negative_inputs()
        installer = next(e for e in source['objects'] if e['path'].startswith('installers/'))
        definition = G.freeze_definition(source, proof, installer, Path(__file__).resolve(), PREVIOUS)
        # Source generation mechanics are tested without creating CMS admission.
        definition['admission'] = 'prepared'
        source_raw, proof_raw = G.canonical(source), G.canonical(proof)
        runtime = G.render_runtime(RAW, definition, source_raw, proof_raw)
        tests = G.render_tests(RAW, len(definition['objects']),
            sum(e['path'].startswith(G.GAME_DIRECTORY + '/') for e in definition['objects']),
            sum(e['size'] for e in definition['objects']), max(e['size'] for e in definition['objects']))
        return definition, runtime, tests

    def test_pure_rendering_anchors_exact_r20_s16_and_predecessor15(self):
        definition, runtime, tests = self.rendered()
        self.assertEqual(definition['admission'], 'prepared')
        self.assertEqual(definition['knownPrevious'], PREVIOUS['candidate'])
        self.assertEqual(definition['expectedCurrent'], PREVIOUS['candidate'])
        self.assertEqual(definition['operationPolicy'], PREVIOUS['operationPolicy'])
        self.assertEqual(definition['originBase'], PREVIOUS['originBase'])
        self.assertEqual(G.closure([e for e in definition['objects'] if e['path'].startswith(G.ENGINE_DIRECTORY + '/')]),
                         G.closure([e for e in PREVIOUS['objects'] if e['path'].startswith(G.ENGINE_DIRECTORY + '/')]))
        driver = runtime[G.DRIVER_PATH].decode()
        worker = runtime[G.MODULE_PATH].decode()
        self.assertIn("plan['candidate']['sequence'] == 16", driver)
        self.assertIn("plan['knownPrevious']['sequence'] == 15", driver)
        self.assertIn('candidate.candidate.sequence === 16', worker)
        self.assertIn('candidate.knownPrevious.sequence === 15', worker)
        self.assertIn(G.GAME_SOURCE, driver)
        self.assertIn(G.GAME_CANDIDATE, worker)
        self.assertNotIn('source2b04', driver + worker)
        for name, data in {**runtime, **tests}.items():
            G.owned_output(name)
            if name.endswith('.py'):
                ast.parse(data, filename=name)

    def test_delivery_non_identity_functions_are_unchanged_ast(self):
        definition, runtime, _ = self.rendered()
        old = ast.parse(RAW['scripts/native_r19_s15_r2_delivery.py'])
        new = ast.parse(runtime[G.DRIVER_PATH])
        old_functions = {node.name: ast.dump(node, include_attributes=False) for node in old.body
                         if isinstance(node, (ast.FunctionDef, ast.ClassDef))}
        new_functions = {node.name: ast.dump(node, include_attributes=False) for node in new.body
                         if isinstance(node, (ast.FunctionDef, ast.ClassDef))}
        self.assertEqual(set(old_functions), set(new_functions))
        # load_plan_bytes adds only the explicit frozen actual15 expectedCurrent
        # admission check. Runtime observation, CAS and retry functions do not
        # change and remain compared here byte-for-byte in their AST.
        for name in old_functions.keys() - {'read_prepared_plan_bytes', 'load_plan_bytes'}:
            self.assertEqual(old_functions[name], new_functions[name], name)
        old_admission = ast.get_source_segment(RAW['scripts/native_r19_s15_r2_delivery.py'].decode(),
            next(n for n in old.body if isinstance(n, ast.FunctionDef) and n.name == 'load_plan_bytes'))
        expected_admission = old_admission.replace(
            "and plan['expectedCurrent'] is None and original['expectedCurrent'] is None",
            "and plan['expectedCurrent'] == plan['knownPrevious']\n"
            "                and original['expectedCurrent'] == plan['knownPrevious']")
        new_admission = ast.get_source_segment(runtime[G.DRIVER_PATH].decode(),
            next(n for n in new.body if isinstance(n, ast.FunctionDef) and n.name == 'load_plan_bytes'))
        self.assertEqual(ast.dump(ast.parse(expected_admission), include_attributes=False),
                         ast.dump(ast.parse(new_admission), include_attributes=False))

    def test_worker_streaming_cas_deadline_and_heads_are_unchanged_source(self):
        _, runtime, _ = self.rendered()
        old = RAW[G.WORKER + 'src/native-r19-s15-publication.mjs'].decode()
        new = runtime[G.MODULE_PATH].decode()
        start = '// Bounded strict JSON rejects duplicate keys'
        tail = old[old.index(start):].replace('native-r19-s15', 'native-r20-s16')
        tail = tail.replace('NativeR19S15', 'NativeR20S16').replace('nativeR19S15', 'nativeR20S16')
        tail = tail.replace('R19/s15', 'R20/s16')
        self.assertEqual(tail, new[new.index(start):])

    @unittest.skipUnless(shutil.which('node'), 'Node is not available for syntax-only source checks')
    def test_node_parses_worker_and_all_policy_tests_without_loading_or_emitting(self):
        _, runtime, tests = self.rendered()
        for name, data in {**runtime, **tests}.items():
            if name.endswith('.mjs'):
                result = subprocess.run([shutil.which('node'), '--input-type=module', '--check'],
                    input=data, capture_output=True, timeout=20, check=False)
                self.assertEqual(result.returncode, 0, result.stderr.decode(errors='replace'))


if __name__ == '__main__':
    output = io.StringIO()
    result = unittest.TextTestRunner(stream=output, verbosity=2).run(
        unittest.defaultTestLoader.loadTestsFromModule(sys.modules[__name__]))
    stamp = str(time.time_ns())
    FIXTURE.mkdir(parents=True, exist_ok=True)
    log = FIXTURE / ('offline-' + stamp + '.txt')
    log.write_text(output.getvalue(), encoding='utf-8')
    report = dict(schema='mir2.r20.s16.offline-generator-policy.v1', passed=result.wasSuccessful(),
        tests=result.testsRun, failures=len(result.failures), errors=len(result.errors), skipped=len(result.skipped),
        log=log.name, logSha256=G.sha(log.read_bytes()), sourceRevision=G.GAME_SOURCE,
        syntheticNegativeInputsOnly=True, pureRenderingUnadmitted=True, rootCmsVerified=False,
        fullR20ClosureVerified=False, actualStageAccepted=False, actualPromoteAccepted=False,
        actualWindowsAccepted=False, cloudWrites=False, oldChannelFilesModified=False)
    (FIXTURE / ('offline-' + stamp + '.json')).write_bytes(G.canonical(report))
    print(output.getvalue(), end='')
    print(json.dumps(report))
    raise SystemExit(0 if result.wasSuccessful() else 1)
