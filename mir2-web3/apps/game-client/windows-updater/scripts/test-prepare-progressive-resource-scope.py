"""Small metadata fixtures only; no pack builds, downloads, or closure claims."""
import copy
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

SCRIPT = Path(__file__).with_name('prepare-progressive-resource-scope.py')
spec = importlib.util.spec_from_file_location('progressive_scope', SCRIPT)
p = importlib.util.module_from_spec(spec)
spec.loader.exec_module(p)


class ScopePreparationTests(unittest.TestCase):
    def setUp(self):
        self.catalog = {'schema': p.CATALOG_SCHEMA, 'resources': {}, 'packs': [
            {'sha256': 'A' * 64, 'size': 104}, {'sha256': 'B' * 64, 'size': 104}]}
        self.catalog['resources'] = {
            'a.png': {'sha256': 'C' * 64, 'size': 8, 'packSha256': 'A' * 64, 'offset': 96, 'storedSize': 8, 'codec': 'stored'},
            'alias.png': {'sha256': 'C' * 64, 'size': 8, 'packSha256': 'A' * 64, 'offset': 96, 'storedSize': 8, 'codec': 'stored'},
            'later.png': {'sha256': 'D' * 64, 'size': 8, 'packSha256': 'B' * 64, 'offset': 96, 'storedSize': 8, 'codec': 'stored'}}

    def prepare(self, paths, catalog=None, scope_id='observed-login'):
        cb = p.canonical(catalog if catalog is not None else self.catalog)
        pb = p.canonical(paths)
        return p.prepare(cb, p.sha(cb), pb, p.sha(pb), scope_id)

    def test_deterministic_sorted_scope_aliases_and_preserving_repack_plan(self):
        first = self.prepare(['alias.png', 'a.png'])
        second = self.prepare(['a.png', 'alias.png'])
        self.assertEqual(first['required-scope.json'], second['required-scope.json'])
        self.assertEqual(first['REPACK-PLAN.json'], second['REPACK-PLAN.json'])
        scope = json.loads(first['required-scope.json'])
        self.assertEqual(scope['requiredPaths'], ['a.png', 'alias.png'])
        receipt = json.loads(first['PREPARED-SCOPE.json'])
        self.assertEqual(receipt['uniquePayloadCount'], 1)
        self.assertEqual(receipt['logicalBytes'], 16)
        self.assertEqual(receipt['requiredObjectBytes'], 104)
        self.assertFalse(receipt['bootstrapAuthorized'])
        self.assertFalse(receipt['firstPlayVerified'])
        self.assertFalse(receipt['packPayloadsVerifiedHere'])
        self.assertEqual(json.loads(first['REPACK-PLAN.json'])['groups'][0]['paths'], scope['requiredPaths'])
        self.assertEqual(receipt['catalog']['resourceCount'], 3)  # Deferred path survives.

    def test_invalid_lists_and_ids_are_rejected(self):
        for paths in [[], ['missing.png'], ['../a.png'], ['AUX.png'], ['code.exe'], ['a.png', 'a.png'], ['a.png', 'A.png'], [False], {'resources': ['a.png']}]:
            with self.subTest(paths=paths), self.assertRaises(ValueError):
                self.prepare(paths)
        for name in ['', 'x' * 65, '中文', 'line\nbreak', 'space id']:
            with self.subTest(name=name), self.assertRaises(ValueError):
                self.prepare(['a.png'], scope_id=name)

    def test_both_inputs_must_match_pins_and_output_scope_is_bounded(self):
        cb = p.canonical(self.catalog); pb = p.canonical(['a.png'])
        for csha, psha in [('0' * 64, p.sha(pb)), (p.sha(cb), '0' * 64)]:
            with self.assertRaises(ValueError):
                p.prepare(cb, csha, pb, psha, 'id')
        with self.assertRaises(ValueError):
            p.prepare(cb, p.sha(cb), b' ' * (p.MAX_SCOPE + 1), '0' * 64, 'id')
        with self.assertRaises(ValueError):
            p.prepare(cb, p.sha(cb), pb, p.sha(pb), 'id', 95)
        with self.assertRaises(ValueError):
            p.prepare(cb, p.sha(cb), pb, p.sha(pb), 'id', p.MAX_PACK + 1)

    def test_catalog_extent_alias_closure_case_and_types_are_strict(self):
        cases = []
        c = copy.deepcopy(self.catalog); c['resources']['later.png']['offset'] = 100; cases.append(c)
        c = copy.deepcopy(self.catalog); c['resources']['alias.png']['offset'] = 95; cases.append(c)
        c = copy.deepcopy(self.catalog); c['packs'].append({'sha256': 'E' * 64, 'size': 32}); cases.append(c)
        c = copy.deepcopy(self.catalog); c['resources']['A.png'] = c['resources']['a.png']; cases.append(c)
        c = copy.deepcopy(self.catalog); c['resources']['a.png']['size'] = True; cases.append(c)
        c = copy.deepcopy(self.catalog); c['resources']['a.png']['codec'] = 'compressed'; cases.append(c)
        c = copy.deepcopy(self.catalog); c['resources']['a.png']['extra'] = 'authority'; cases.append(c)
        for i, catalog in enumerate(cases):
            with self.subTest(i=i), self.assertRaises(ValueError):
                self.prepare(['a.png'], catalog)
        with self.assertRaises(ValueError):
            p.decoded(b'{"schema":1,"schema":2}')

    def test_create_only_outputs_do_not_replace_prior_evidence(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp); output = root / 'fresh'
            artifacts = self.prepare(['a.png']); p.emit(output, artifacts)
            before = {x.name: x.read_bytes() for x in output.iterdir()}
            with self.assertRaises(FileExistsError):
                p.emit(output, artifacts)
            self.assertEqual(before, {x.name: x.read_bytes() for x in output.iterdir()})
            empty = root / 'empty'; empty.mkdir()
            with self.assertRaises(FileExistsError):
                p.emit(empty, artifacts)
            self.assertEqual(list(empty.iterdir()), [])

    def test_actual_cli_rejects_bad_pin_without_creating_output(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp); cp = root / 'catalog.json'; pp = root / 'paths.json'
            cp.write_bytes(p.canonical(self.catalog)); pp.write_bytes(p.canonical(['a.png']))
            args = [sys.executable, '-I', '-S', '-B', str(SCRIPT), '--catalog', str(cp), '--catalog-sha256', p.sha(cp.read_bytes()),
                    '--paths-json', str(pp), '--paths-sha256', p.sha(pp.read_bytes()), '--id', 'observed', '--output', str(root / 'out')]
            result = subprocess.run(args, capture_output=True)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertFalse(json.loads(result.stdout)['bootstrapAuthorized'])
            args[args.index('--paths-sha256') + 1] = '0' * 64
            args[-1] = str(root / 'bad')
            self.assertNotEqual(subprocess.run(args, capture_output=True).returncode, 0)
            self.assertFalse((root / 'bad').exists())


if __name__ == '__main__':
    unittest.main(verbosity=2)
