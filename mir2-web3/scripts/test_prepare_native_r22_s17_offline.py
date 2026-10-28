"""Offline guards only. Synthetic render data never becomes admission/proof."""
import ast
import copy
import hashlib
import importlib.util
import json
from pathlib import Path
import shutil
import subprocess
import sys
import unittest
from unittest.mock import patch
from types import SimpleNamespace

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


G = load('r22_generator', HERE / 'prepare_native_r22_s17_fixed.py')
O = load('r22_origin', HERE / 'classic-r22-publication/prepare_origin_r22_s17.py')


class OfflineGuards(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.raw, cls.previous, cls.feed, cls.subject = G.template_inputs()
        cls.plan_raw = cls.raw['scripts/fixtures/native-r20-s16/actual-cms16/PUBLICATION-PLAN.json']
        cls.proof_raw = cls.raw['scripts/fixtures/native-r20-s16/actual-cms16/ROOT-VERIFICATION.json']
        cls.feed_raw = cls.raw['scripts/fixtures/native-r20-s16/preview/artifacts/feed/latest.json']
        cls.cms_raw = cls.raw['scripts/fixtures/native-r20-s16/preview/artifacts/feed/latest.p7s']

    def test_current16_is_exact_genuine_predecessor(self):
        self.assertEqual(self.previous['candidate']['sequence'], 16)
        self.assertEqual(G.sha(self.plan_raw), O.OLD_PLAN_SHA)
        self.assertEqual(G.sha(self.proof_raw), O.OLD_PROOF_SHA)
        self.assertEqual(G.sha(self.feed_raw), O.FEED16['latest.json']['sha256'])
        self.assertEqual(G.sha(self.cms_raw), O.FEED16['latest.p7s']['sha256'])
        rows = O.validate_admission(json.loads(self.plan_raw), json.loads(self.proof_raw),
                                    O.OLD_PLAN_SHA, O.OLD_PROOF_SHA, 16)
        self.assertEqual(len(rows), 41)

    def test_shallow_clone_accepts_only_exact_pinned_checkout_variants(self):
        with patch.object(G.subprocess, 'run', return_value=SimpleNamespace(returncode=1)):
            raw, previous, feed, _ = G.template_inputs()
        self.assertEqual(raw, self.raw)
        self.assertEqual(previous, self.previous)
        self.assertEqual(feed, self.feed)

    def test_old16_plan_cannot_admit17(self):
        with self.assertRaisesRegex(G.GuardError, 'exact_expected_current16_required'):
            G.validate_inputs(self.plan_raw, self.proof_raw, self.feed_raw, self.cms_raw,
                              Path(__file__), self.previous, self.feed, self.subject)
        with self.assertRaisesRegex(RuntimeError, 'root-issued-admission-binding'):
            O.validate_admission(json.loads(self.plan_raw), json.loads(self.proof_raw),
                                 O.OLD_PLAN_SHA, O.OLD_PROOF_SHA, 17)

    def test_tampered_predecessor_rejected_before_sources(self):
        for mutation in (None, {**self.previous['candidate'], 'sequence': 17},
                         {**self.previous['candidate'], 'feedSha256': '0' * 64}):
            plan = json.loads(self.plan_raw)
            plan['expectedCurrent'] = mutation
            with self.assertRaisesRegex(G.GuardError, 'exact_expected_current16_required'):
                G.validate_inputs(G.canonical(plan), self.proof_raw, self.feed_raw, self.cms_raw,
                                  Path(__file__), self.previous, self.feed, self.subject)

    def test_proof_requires_cms_and_exact_closure(self):
        plan = json.loads(self.plan_raw)
        proof = json.loads(self.proof_raw)
        proof['cmsVerified'] = False
        with self.assertRaisesRegex(RuntimeError, 'root-issued-admission-binding'):
            O.validate_admission(plan, proof, O.OLD_PLAN_SHA, O.OLD_PROOF_SHA, 16)
        proof = json.loads(self.proof_raw)
        plan['objects'][3]['sha256'] = '0' * 64
        with self.assertRaisesRegex(RuntimeError, 'root-issued-closed-object-digest'):
            O.validate_admission(plan, proof, O.OLD_PLAN_SHA, O.OLD_PROOF_SHA, 16)

    def test_bounds_paths_and_new_only_output(self):
        plan = json.loads(self.plan_raw)
        for path in ('../escape', 'releases/a/../b', '/absolute', 'releases/a//b'):
            mutated = copy.deepcopy(plan)
            mutated['objects'][0]['path'] = path
            with self.assertRaisesRegex(RuntimeError, 'public-object-boundary'):
                O.object_rows(mutated)
        plan['objects'][0]['size'] = O.MAX_METADATA + 1
        with self.assertRaisesRegex(RuntimeError, 'object-size-bound'):
            O.object_rows(plan)
        for path in ('scripts/native_r20_s16_r2_delivery.py',
                     G.WORKER + 'src/index.ts', '../escape'):
            with self.assertRaises(G.GuardError):
                G.owned_output(path)

    def test_origin_writer_retains_root_permissions_and_rejects_template_drift(self):
        # Check the original pinned bytes in this checkout on both CI platforms.
        # The production generator still consumes its fixed local release input.
        writer = O.read(HERE / 'fixtures/native-r22-s17/templates/origin_r20_s16.py',
                        O.OLD_WRITER_SHA).decode()
        tail = O.read(O.HERE / 'origin_r22_tail.py.in', O.TEMPLATE_PINS['origin_r22_tail.py.in']).decode()
        # In-memory rendering only: no output directory and no false receipt.
        rendered = O.render_writer({'unitTestOnly': True}, writer, tail)
        ast.parse(rendered)
        self.assertIn("STAGE = Path('/srv/.mir2-origin-r22-s17-20261008-01')", rendered)
        self.assertIn("GAME = '" + G.GAME_SOURCE + "'", rendered)
        for guard in ('linux-root-required', 'root-stage-not-private', 'guard-open-race',
                      'service-pid-or-state-changed', 'os.O_EXCL', 'os.O_NOFOLLOW'):
            self.assertIn(guard, rendered)
        self.assertIn("value['sequence'] == 16", rendered)
        self.assertIn("feed['sequence'] == 17", rendered)
        with self.assertRaisesRegex(RuntimeError, 'reviewed-writer-template-pin'):
            O.render_writer({}, writer + '\n', tail)

    def test_templates_are_fixed_and_parse_without_execution(self):
        for name, pin in O.TEMPLATE_PINS.items():
            value = O.read(O.HERE / name, pin)
            ast.parse(value, filename=name)
        self.assertEqual(G.sha(O.read(O.HERE / 'guards.py.in')), O.GUARDS_SHA)

    def test_runtime_render_changes_identity_preserves_cas_and_limits(self):
        # Deliberately synthetic, not admitted: tests source transformation only.
        definition = copy.deepcopy(self.previous)
        definition['schema'] = 'mir2.native.r22.fixed-origin-plan.v1'
        definition['candidate']['sequence'] = 17
        definition['knownPrevious'] = self.previous['candidate']
        rendered = G.render_runtime(self.raw, definition, b'unit-test-plan', b'unit-test-proof')
        driver = rendered[G.DRIVER_PATH].decode()
        ast.parse(driver)
        self.assertIn("plan['candidate']['sequence'] == 17", driver)
        self.assertIn("plan['knownPrevious']['sequence'] == 16", driver)
        self.assertIn("and original['expectedCurrent'] == plan['knownPrevious']", driver)
        self.assertIn('65536', driver)
        module = rendered[G.MODULE_PATH].decode()
        self.assertIn('candidate.candidate.sequence === 17', module)
        self.assertIn('candidate.knownPrevious.sequence === 16', module)
        self.assertIn('same(original.expectedCurrent, candidate.knownPrevious)', module)
        for marker in ('onlyIf', 'OPERATION_POLICY', 'fixedLengthStream', 'noProgressMs'):
            self.assertTrue(marker.lower() in module.lower(), marker + ' missing')
        node = shutil.which('node')
        self.assertIsNotNone(node, 'Node is required for the original module syntax check')
        result = subprocess.run([node, '--check', '--input-type=module'], input=module.encode(),
                                capture_output=True)
        self.assertEqual(result.returncode, 0, result.stderr.decode())
        tests = G.render_tests(self.raw, 41, 35, 1077405420, 111160320)
        for path, value in tests.items():
            if path.endswith('.py'):
                ast.parse(value, filename=path)


if __name__ == '__main__':
    unittest.main()
