import hashlib
import json
import os
from pathlib import Path
import tempfile
import unittest

from adapter_source_binding import adapter_sources_proof


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


@unittest.skipUnless(os.name == 'nt', 'Explicit Windows C-only source binding')
class AdapterSourceBindingTests(unittest.TestCase):
    def setUp(self):
        parent = Path(tempfile.gettempdir()) / 'mir2-comparable-capacity-20261113' / 'controller-source-binding-tests'
        parent.mkdir(parents=True, exist_ok=True)
        self.temporary = tempfile.TemporaryDirectory(dir=parent)
        self.root = Path(self.temporary.name).resolve()
        assert self.root.is_relative_to(parent.resolve()) and self.root.drive.upper() == 'C:'
        self.addCleanup(self.temporary.cleanup)
        (self.root / 'driver.mjs').write_text("import './contract.mjs';\n", encoding='utf-8')
        (self.root / 'contract.mjs').write_text('export const contract = 1;\n', encoding='utf-8')
        (self.root / 'ws.js').write_text('module.exports = {};\n', encoding='utf-8')
        (self.root / 'server.exe').write_bytes(b'fixture-only; never executed')
        self.manifest_path = self.root / 'adapter-sources.json'
        self.runtime_path = self.root / 'runtime.json'
        self.spec = {'engine': 'modern', 'server': {'command': [str(self.root / 'server.exe')]},
            'driver': {'adapterSources': str(self.manifest_path), 'command': ['node.exe',
                str(self.root / 'driver.mjs'), '--adapter-sources', str(self.manifest_path),
                '--ws-module', str(self.root / 'ws.js'), '--runtime-receipt', str(self.runtime_path)]}}
        self.seal()

    def seal(self):
        ws = self.root / 'ws.js'
        self.manifest = {'schema': 'mir2.comparable.modern-adapter-sources.v1', 'entrypoint': 'driver.mjs',
            'files': [{'path': p.name, 'bytes': p.stat().st_size, 'sha256': sha(p)}
                      for p in sorted(self.root.glob('*.mjs'))],
            'wsModule': {'path': str(ws), 'bytes': ws.stat().st_size, 'sha256': sha(ws)}}
        self.manifest_path.write_text(json.dumps(self.manifest), encoding='utf-8')
        self.runtime = {'adapterSourceManifestSha256': sha(self.manifest_path),
            'adapterEntrypointSha256': sha(self.root / 'driver.mjs'), 'binarySha256': sha(self.root / 'server.exe')}
        self.runtime_path.write_text(json.dumps(self.runtime), encoding='utf-8')

    def test_valid_bundle_binds_actual_entry_ws_runtime_and_server(self):
        proof = adapter_sources_proof(self.spec)
        self.assertEqual(proof['adapterSourceManifestSha256'], sha(self.manifest_path))
        self.assertEqual(proof['driverEntrypointSha256'], sha(self.root / 'driver.mjs'))
        self.assertEqual(proof['serverBinarySha256'], sha(self.root / 'server.exe'))
        self.assertTrue(proof['adapterSourceProof']['actualCommandEntrypointMatched'])
        self.assertEqual(proof, adapter_sources_proof(self.spec))

    def test_source_mutation_is_rejected(self):
        (self.root / 'contract.mjs').write_text('export const contract = 2;\n', encoding='utf-8')
        with self.assertRaisesRegex(ValueError, 'source bytes'):
            adapter_sources_proof(self.spec)

    def test_actual_entrypoint_cannot_be_swapped_for_another_listed_file(self):
        self.spec['driver']['command'][1] = str(self.root / 'contract.mjs')
        with self.assertRaisesRegex(ValueError, 'entrypoint differs'):
            adapter_sources_proof(self.spec)

    def test_declared_sources_must_close_actual_local_imports(self):
        (self.root / 'driver.mjs').write_text("import './contract.mjs'; import './missing.mjs';\n", encoding='utf-8')
        self.seal()
        with self.assertRaisesRegex(ValueError, 'dependency is not closed'):
            adapter_sources_proof(self.spec)

    def test_manifest_rejects_path_escape_and_duplicates(self):
        for invalid in ('../driver.mjs', 'DRIVER.MJS'):
            self.seal()
            self.manifest['files'].append(dict(self.manifest['files'][0], path=invalid))
            self.manifest_path.write_text(json.dumps(self.manifest), encoding='utf-8')
            with self.assertRaises(ValueError):
                adapter_sources_proof(self.spec)

    def test_actual_ws_path_is_bound_even_when_bytes_match(self):
        alternative = self.root / 'alternative-ws.js'
        alternative.write_bytes((self.root / 'ws.js').read_bytes())
        command = self.spec['driver']['command']
        command[command.index('--ws-module') + 1] = str(alternative)
        with self.assertRaisesRegex(ValueError, 'WS module differs'):
            adapter_sources_proof(self.spec)

    def test_runtime_cannot_claim_another_adapter_or_server(self):
        for field in ('adapterSourceManifestSha256', 'adapterEntrypointSha256', 'binarySha256'):
            self.seal()
            self.runtime[field] = '0' * 64
            self.runtime_path.write_text(json.dumps(self.runtime), encoding='utf-8')
            with self.assertRaises(ValueError):
                adapter_sources_proof(self.spec)

    def test_build_manifest_cannot_claim_another_binary(self):
        path = self.root / 'build.json'
        path.write_text(json.dumps({'binarySha256': '0' * 64}), encoding='utf-8')
        self.runtime['buildManifestSha256'] = sha(path)
        self.runtime_path.write_text(json.dumps(self.runtime), encoding='utf-8')
        self.spec['driver']['command'] += ['--build-manifest', str(path)]
        with self.assertRaisesRegex(ValueError, 'Build manifest'):
            adapter_sources_proof(self.spec)

    def test_legacy_stage_remains_opt_in(self):
        self.assertIsNone(adapter_sources_proof({'engine': 'crystal', 'driver': {}}))


if __name__ == '__main__':
    unittest.main()
