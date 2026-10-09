"""R23 preparation contracts only; no CMS admission or actual publication.

All future render values are explicitly unadmitted, in-memory test models.
Retained disk fixtures are bounded UNIT-ONLY files, never actual-cms18 proofs.
"""
import ast
import copy
import importlib.util
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest
from types import SimpleNamespace
from unittest.mock import patch

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
BASE = HERE.parent
UNIT_ROOT = HERE / 'fixtures/native-r23-s18/qa-runs/offline-unit-only'


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


G = load('r23_preparation_subject', HERE / 'prepare_native_r23_s18_fixed.py')
R22 = load('immutable_r22_generator', HERE / 'prepare_native_r22_s17_fixed.py')


class PreparationContracts(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.raw, cls.previous, cls.feed, cls.subject = G.template_inputs()
        prefix = 'scripts/fixtures/native-r22-s17/'
        cls.plan_raw = cls.raw[prefix + 'actual-cms17/PUBLICATION-PLAN.json']
        cls.proof_raw = cls.raw[prefix + 'actual-cms17/ROOT-VERIFICATION.json']
        cls.feed_raw = cls.raw[prefix + 'preview/artifacts/feed/latest.json']
        cls.cms_raw = cls.raw[prefix + 'preview/artifacts/feed/latest.p7s']
        cls.before = {name: G.sha((BASE / name).read_bytes()) for name in G.TEMPLATES}

    @classmethod
    def tearDownClass(cls):
        if hasattr(cls, 'before'):
            after = {name: G.sha((BASE / name).read_bytes()) for name in G.TEMPLATES}
            if after != cls.before:
                raise AssertionError('Original immutable R22 files changed during preparation tests')

    def retained_unit_directory(self, label):
        UNIT_ROOT.mkdir(parents=True, exist_ok=True)
        directory = Path(tempfile.mkdtemp(prefix=label + '-', dir=UNIT_ROOT))
        (directory / 'UNIT-ONLY.txt').write_text(
            'Offline contract fixture; never CMS/admission/publication evidence.\n', encoding='utf-8')
        return directory

    def render_model(self):
        # This unadmitted object is used only to exercise deterministic source rendering.
        definition = copy.deepcopy(self.previous)
        definition.update(schema='mir2.native.r23.fixed-origin-plan.v1', admission='unit-test-unadmitted')
        definition['candidate'] = {**self.previous['candidate'], 'sequence': 18}
        definition['knownPrevious'] = self.previous['candidate']
        definition['expectedCurrent'] = self.previous['candidate']
        definition['preparation']['requiredGameSourceRevision'] = G.GAME_SOURCE
        definition['preparation']['requiredGameCandidate'] = G.GAME_CANDIDATE
        return G.render_runtime(self.raw, definition,
            b'unit-test-unadmitted-plan-not-a-root-receipt', b'unit-test-unadmitted-proof-not-cms'), definition

    def test_exact_genuine_predecessor17_raw_bytes(self):
        self.assertEqual(G.TEMPLATE_COMMIT, 'e7850e9aef9ea038907bee621f331f3286c1ac9b')
        self.assertEqual(self.previous['candidate']['sequence'], 17)
        self.assertEqual(self.previous['candidate']['sourceRevision'], G.ENGINE_SOURCE)
        self.assertEqual(G.sha(self.plan_raw), 'ed5e6d25c508e2c9f5bccac7a1e2d258617364926f186fbde7eec9b753e330ee')
        self.assertEqual(G.sha(self.proof_raw), '8ec73ca7d1e7c500a46a9c58af41789307087f6cd7d3b74a77dc5e056981cabf')
        self.assertEqual(G.sha(self.feed_raw), 'e7efebcb6b897e15f3b93b8a7a8eeb354d0eb01ccf799bb155c26c394f6efc4f')
        self.assertEqual(G.sha(self.cms_raw), '84145fbabc752cd648d81415a5d45aba464ee67ddd0a7d2a1a9f296c31844276')
        self.assertEqual(len(self.previous['objects']), 40)
        self.assertEqual(self.feed['engine']['directory'], G.ENGINE_DIRECTORY)

    def test_shallow_clone_exact_checkout_is_the_same_subject(self):
        with patch.object(G.subprocess, 'run', return_value=SimpleNamespace(returncode=1)):
            raw, previous, feed, _ = G.template_inputs()
        self.assertEqual(raw, self.raw)
        self.assertEqual(previous, self.previous)
        self.assertEqual(feed, self.feed)

    def test_only_pinned_code_crlf_can_normalize(self):
        code = 'scripts/native_r22_s17_r2_delivery.py'
        real_read = G.read
        def crlf_code(path, maximum):
            if Path(path) == BASE / code:
                return self.raw[code].replace(b'\n', b'\r\n')
            return real_read(path, maximum)
        with patch.object(G.subprocess, 'run', return_value=SimpleNamespace(returncode=1)), patch.object(G, 'read', side_effect=crlf_code):
            raw, _, _, _ = G.template_inputs()
        self.assertEqual(raw, self.raw)
        protected = ('actual-cms17/PUBLICATION-PLAN.json', 'actual-cms17/ROOT-VERIFICATION.json',
                     'preview/artifacts/feed/latest.json', 'preview/artifacts/feed/latest.p7s')
        for name in protected:
            key = 'scripts/fixtures/native-r22-s17/' + name
            def changed_protected(path, maximum):
                if Path(path) == BASE / key:
                    return self.raw[key] + b'\r\n'
                return real_read(path, maximum)
            with self.subTest(name=name), patch.object(G.subprocess, 'run', return_value=SimpleNamespace(returncode=1)), patch.object(G, 'read', side_effect=changed_protected):
                with self.assertRaisesRegex(G.GuardError, 'reviewed_checkout_template_changed'):
                    G.template_inputs()

    def test_git_template_drift_fails_before_subject_execution(self):
        with patch.object(G.subprocess, 'run', return_value=SimpleNamespace(returncode=0, stdout=b'changed code')):
            with self.assertRaisesRegex(G.GuardError, 'reviewed_git_blob_template_changed'):
                G.template_inputs()

    def test_original17_plan_does_not_admit18_or_read_sources(self):
        with patch.object(G, 'hash_file') as full_hash:
            with self.assertRaisesRegex(G.GuardError, 'exact_expected_current17_required'):
                G.validate_inputs(self.plan_raw, self.proof_raw, self.feed_raw, self.cms_raw,
                    Path(__file__).resolve(), self.previous, self.feed, self.subject)
        full_hash.assert_not_called()

    def test_exact_pointer_rebinding_still_cannot_reuse_candidate17_or_cms17(self):
        plan = json.loads(self.plan_raw)
        plan['expectedCurrent'] = self.previous['candidate']
        with patch.object(G, 'hash_file') as full_hash:
            with self.assertRaisesRegex(G.GuardError, 'exact_candidate18_required'):
                G.validate_inputs(G.canonical(plan), self.proof_raw, self.feed_raw, self.cms_raw,
                    Path(__file__).resolve(), self.previous, self.feed, self.subject)
        full_hash.assert_not_called()

    def test_wrong_previous_pointer_fails_before_source_bytes(self):
        for changed in (None, {**self.previous['candidate'], 'sequence': 18},
                        {**self.previous['candidate'], 'sourceRevision': G.GAME_SOURCE},
                        {**self.previous['candidate'], 'feedSha256': '0' * 64}):
            plan = json.loads(self.plan_raw)
            plan['expectedCurrent'] = changed
            with self.subTest(changed=changed), patch.object(G, 'hash_file') as full_hash:
                with self.assertRaisesRegex(G.GuardError, 'exact_expected_current17_required'):
                    G.validate_inputs(G.canonical(plan), self.proof_raw, self.feed_raw, self.cms_raw,
                        Path(__file__).resolve(), self.previous, self.feed, self.subject)
                full_hash.assert_not_called()

    def test_limits_and_path_guards_match_original(self):
        for name in ('MAX_OBJECTS', 'MAX_METADATA', 'MAX_ARTIFACT', 'CONTROL_LIMIT', 'IMMUTABLE'):
            self.assertEqual(getattr(G, name), getattr(R22, name))
        for path in ('../escape', 'releases/a/../b', '/absolute', 'releases/a//b', 'releases/.hidden/a'):
            with self.subTest(path=path), self.assertRaises(G.GuardError):
                G.relative(path)
        self.assertEqual(G.object_limit('a/PACKAGE-MANIFEST.json'), 32 * 1024 * 1024)
        self.assertEqual(G.object_limit('a/client.exe'), 128 * 1024 * 1024)
        for path in ('scripts/native_r22_s17_r2_delivery.py', G.WORKER + 'src/index.ts', '../escape',
                     G.WORKER + 'src/.gitattributes'):
            with self.subTest(path=path), self.assertRaises(G.GuardError):
                G.owned_output(path)

    def test_metadata_and_full_hash_io_functions_are_ast_identical(self):
        def functions(module):
            tree = ast.parse(Path(module.__file__).read_bytes())
            return {n.name: ast.dump(n, include_attributes=False) for n in tree.body if isinstance(n, ast.FunctionDef)}
        before, after = functions(R22), functions(G)
        for name in ('require', 'sha', 'canonical', 'plain', 'identity', 'read', 'hash_file',
                     'exact', 'object_limit', 'closure', 'relative', 'replace_once', 'replace_assignment',
                     'output_parent', 'emit', 'main'):
            self.assertEqual(before[name], after[name], name + ' must retain original policy')

    def test_clock_and_root_cms_guard_ast_only_change_reviewed_labels(self):
        def target(module):
            tree = ast.parse(Path(module.__file__).read_bytes())
            return next(n for n in tree.body if isinstance(n, ast.FunctionDef) and n.name == 'validate_inputs')
        old = target(R22)
        labels = {'exact_expected_current16_required':'exact_expected_current17_required',
                  'exact_candidate17_required':'exact_candidate18_required',
                  'actual_root17_receipt_required':'actual_root18_receipt_required',
                  'current_signed_feed17_required':'current_signed_feed18_required',
                  'exact_r22_game_identity_required':'exact_r23_game_identity_required',
                  'feeds/s17-':'feeds/s18-', 'r22_closed_object_scope_required':'r23_closed_object_scope_required',
                  'exact_game_source7fea5cd_required':'exact_game_sourcefaefb696_required'}
        class ReviewedLabels(ast.NodeTransformer):
            def visit_Constant(self, node):
                if isinstance(node.value, str) and node.value in labels:
                    node.value = labels[node.value]
                return node
        self.assertEqual(ast.dump(ReviewedLabels().visit(old), include_attributes=False),
                         ast.dump(target(G), include_attributes=False))

    def test_render_changes_only_admission_functions_transport_functions_are_identical(self):
        rendered, definition = self.render_model()
        self.assertEqual(definition['admission'], 'unit-test-unadmitted')
        driver = rendered[G.DRIVER_PATH].decode()
        original = self.raw['scripts/native_r22_s17_r2_delivery.py'].decode()
        def top_level_functions(text):
            tree = ast.parse(text)
            return {n.name: ast.dump(n, include_attributes=False) for n in tree.body
                    if isinstance(n, (ast.FunctionDef, ast.ClassDef))}
        old_functions = top_level_functions(G.advance_source(original))
        new_functions = top_level_functions(driver)
        changed = {n for n in old_functions if old_functions[n] != new_functions[n]}
        self.assertEqual(changed, {'read_prepared_plan_bytes'})
        self.assertIn("plan['candidate']['sequence'] == 18", driver)
        self.assertIn("plan['knownPrevious']['sequence'] == 17", driver)
        self.assertIn("and original['expectedCurrent'] == plan['knownPrevious']", driver)
        module = rendered[G.MODULE_PATH].decode()
        self.assertIn('candidate.candidate.sequence === 18', module)
        self.assertIn('candidate.knownPrevious.sequence === 17', module)
        self.assertIn('same(original.expectedCurrent, candidate.knownPrevious)', module)
        self.assertIn('onlyIf: new Headers(condition)', module)
        self.assertIn('onlyIf: new Headers({\'if-none-match\': \'*\'})', module)
        self.assertIn('pointer_write_outcome_unknown', module)
        node = shutil.which('node')
        self.assertIsNotNone(node, 'Node required for module syntax check')
        result = subprocess.run([node, '--check', '--input-type=module'], input=module.encode(), capture_output=True)
        self.assertEqual(result.returncode, 0, result.stderr.decode())

    def test_renderer_rejects_changed_exact_identity_hunk(self):
        raw = dict(self.raw)
        key = 'scripts/native_r22_s17_r2_delivery.py'
        raw[key] = raw[key].replace(b"plan['candidate']['sequence'] == 17", b"plan['candidate']['sequence'] != 17")
        _, definition = self.render_model()
        with self.assertRaisesRegex(G.GuardError, 'reviewed_template_hunk_changed'):
            G.render_runtime(raw, definition, b'unit-only', b'unit-only')

    def test_derived_tests_preserve_negative_versions_and_bounded_stream_offsets(self):
        tests = G.render_tests(self.raw, 43, 37, 1200000000, 112000000)
        py = tests['scripts/test_native_r23_s18_r2_delivery.py'].decode()
        ast.parse(py)
        self.assertIn("PLAN_RAW.replace(b'\"sequence\": 18', b'\"sequence\": 19', 1)", py)
        self.assertIn("('sequence', 16), ('sequence', 18)", py)
        self.assertIn('update(sequence=17)', py)
        self.assertIn('self.offset = 16', py)
        self.assertIn('return FEED[:16]', py)
        self.assertIn('len(REAL_PLAN[\'objects\']), 43', py)
        self.assertIn('predecessor17/latest.p7s', py)
        node = tests[G.WORKER + 'test/native-r23-s18-publication.test.mjs'].decode()
        self.assertIn('definition.candidate.sequence, 18', node)
        self.assertIn('e.candidate.sequence = 17', node)
        self.assertIn('remains17', node)
        self.assertIn('largest, 112000000', node)
        self.assertIn('FEED_BYTES.slice(0, 16)', node)
        self.assertEqual(node.count('definition.objects.length, 43'), 2)
        for name, data in tests.items():
            if name.endswith('.py'):
                ast.parse(data, filename=name)

    def test_conflict_preflight_preserves_failure_before_any_other_output(self):
        directory = self.retained_unit_directory('conflict')
        prefix = G.FIXTURE + 'unit-fixture/'
        destination = directory / (prefix + 'proof.txt')
        destination.parent.mkdir(parents=True)
        destination.write_bytes(b'unit-only retained earlier failure')
        with patch.object(G, 'BASE', directory):
            with self.assertRaisesRegex(G.GuardError, 'output_conflict_retained'):
                G.emit({prefix + 'first.txt': b'unit-only new', prefix + 'proof.txt': b'changed'})
        self.assertFalse(destination.with_name('first.txt').exists())
        self.assertEqual(destination.read_bytes(), b'unit-only retained earlier failure')

    def test_identical_emission_never_replaces_inode_or_timestamps(self):
        directory = self.retained_unit_directory('idempotent')
        output = {G.FIXTURE + 'unit-fixture/proof.txt': b'unit-only original exact fixture'}
        with patch.object(G, 'BASE', directory):
            G.emit(output)
            destination = directory / next(iter(output))
            before = G.identity(destination.stat())
            G.emit(output)
        self.assertEqual(G.identity(destination.stat()), before)

    def test_source_hash_rejects_changed_bytes_and_keeps_the_fixture(self):
        directory = self.retained_unit_directory('source-hash')
        path = directory / 'source.bin'
        path.write_bytes(b'unit-only changed source')
        with self.assertRaisesRegex(G.GuardError, 'source_full_hash_mismatch'):
            G.hash_file(path, path.stat().st_size, G.sha(b'unit-only original source'))
        self.assertEqual(path.read_bytes(), b'unit-only changed source')


if __name__ == '__main__':
    unittest.main(verbosity=2)
