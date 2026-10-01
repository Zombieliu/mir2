#!/usr/bin/env python3
"""Offline fixtures only: validates deployment extraction/config preservation.

Windows fixture extraction bypasses POSIX ownership enforcement only for its
temporary files. Separate synthetic checks test that logic. Actual Linux file
ownership, systemd reload, renameat2 and TLS must pass on the deployment host.
"""
import importlib.util
import io
import json
import os
from pathlib import Path
import sys
import subprocess
import tarfile
import tempfile
import time
import types
import unittest
from unittest import mock

sys.dont_write_bytecode = True
if os.name == 'nt':
    sys.modules['fcntl'] = types.SimpleNamespace()
    sys.modules['pwd'] = types.SimpleNamespace()


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


QA = Path(__file__).parent
server = load('deploy_server_test', QA / 'deploy-server-release.py')
fixtures = load('deploy_archive_fixtures', QA / 'test-archive-update-release.py')
helper = fixtures.archive


class DeployTests(unittest.TestCase):
    def setUp(self):
        self.original_plain = server.plain
        self.plain_patch = None
        if os.name == 'nt':
            self.plain_patch = mock.patch.object(server, 'plain', side_effect=lambda path, directory=False, root_owned=False: self.original_plain(path, directory=directory, root_owned=False))
            self.plain_patch.start()
        self.temp = tempfile.TemporaryDirectory(prefix='mir2-deploy-fixture-')
        self.root = Path(self.temp.name)
        self.candidate = fixtures.fixture_candidate(self.root / 'candidate')
        self.engine = fixtures.fixture_engine(self.root / 'engine')
        self.archive = self.root / 'fixture.tar.gz'
        self.mapping = {'releases/game-r6': str(self.candidate), 'releases/updater-v1': str(self.engine)}
        self.receipt = helper.build_archive(self.mapping, self.archive)
        def component(directory, root, identity, names):
            return {'directory': directory, 'identity': identity, 'metadata': [{'path': name, 'size': (root / name).stat().st_size, 'sha256': helper.sha256((root / name).read_bytes())} for name in names]}
        self.feed = {'schema': 'mir2.windows.update-feed.v1', 'channel': 'invited', 'platform': 'windows-x64', 'sequence': 3,
                     'createdUnix': int(time.time()), 'expiresUnix': int(time.time()) + 86400, 'minBootstrap': 1,
                     'protocol': 'crystal-mir2-v1', 'content': 'mir2.windows.package-manifest.v4',
                     'game': component('releases/game-r6', self.candidate, 'WN-CANDIDATE-fixture-06', helper.META),
                     'engine': component('releases/updater-v1', self.engine, '1', ['ENGINE.json', 'ENGINE.p7s'])}
        self.stage = self.root / 'stage'
        self.stage.mkdir()

    def tearDown(self):
        if self.plain_patch:
            self.plain_patch.stop()
        assert self.root.is_absolute() and self.root.parent.resolve() == Path(tempfile.gettempdir()).resolve()
        assert self.root.name.startswith('mir2-deploy-fixture-')
        self.temp.cleanup()

    def extract(self, archive=None):
        return server.extract_checked(archive or self.archive, self.receipt, self.feed, self.stage, helper)

    def bad_tar(self, member, data=b''):
        output = self.root / 'adversarial.tar.gz'
        with tarfile.open(output, 'w:gz', format=tarfile.USTAR_FORMAT) as packed:
            member.uid = member.gid = 0
            member.uname = member.gname = ''
            if member.isfile():
                member.size = len(data)
                packed.addfile(member, io.BytesIO(data))
            else:
                packed.addfile(member)
        return output

    def test_exact_archive_extracts_and_binds_both_components(self):
        result = self.extract()
        self.assertEqual(sum(len(x) for x in result.values()), self.receipt['fileCount'])
        for directory, root in self.mapping.items():
            for path in Path(root).rglob('*'):
                if path.is_file():
                    self.assertEqual((self.stage / directory / path.relative_to(root)).read_bytes(), path.read_bytes())

    def test_tar_traversal_and_unlisted_paths_rejected(self):
        for name in ['../../outside', '/absolute', 'releases/game-r6/../bad', 'releases/game-r6/private.env', 'releases/game-r6/ads:stream']:
            with self.subTest(path=name), self.assertRaises(ValueError):
                self.extract(self.bad_tar(tarfile.TarInfo(name), b'bad'))
        self.assertFalse((self.root / 'outside').exists())

    def test_tar_links_and_special_types_rejected(self):
        for kind in [tarfile.SYMTYPE, tarfile.LNKTYPE, tarfile.FIFOTYPE, tarfile.CHRTYPE]:
            member = tarfile.TarInfo('releases/game-r6/README-START.txt')
            member.type, member.linkname = kind, '../../outside'
            with self.subTest(kind=kind), self.assertRaisesRegex(ValueError, 'links/special'):
                self.extract(self.bad_tar(member))

    def test_missing_files_rejected(self):
        member = tarfile.TarInfo('releases')
        member.type = tarfile.DIRTYPE
        with self.assertRaisesRegex(ValueError, 'omitted receipt'):
            self.extract(self.bad_tar(member))

    def test_extracted_hash_mismatch_rejected(self):
        member = tarfile.TarInfo('releases/game-r6/README-START.txt')
        original = (self.candidate / 'README-START.txt').read_bytes()
        with self.assertRaisesRegex(ValueError, 'SHA256 mismatch'):
            self.extract(self.bad_tar(member, b'X' * len(original)))

    def test_feed_identity_and_metadata_binding_rejected(self):
        self.feed['game']['identity'] = 'WN-CANDIDATE-wrong'
        with self.assertRaisesRegex(ValueError, 'component identity'):
            self.extract()

    def test_receipt_alias_and_byte_count_rejected(self):
        self.receipt['files'].append({**self.receipt['files'][0], 'path': self.receipt['files'][0]['path'].upper()})
        self.receipt['fileCount'] += 1
        with self.assertRaises(ValueError):
            self.extract()

    def test_caddy_addition_preserves_every_original_byte(self):
        old = server.ANCHOR + b'\n handle_path /playtest/* { reverse_proxy 127.0.0.1:7210 }\n handle { reverse_proxy 127.0.0.1:7110 }\n}\n'
        new = server.inject_route(old)
        self.assertEqual(new.replace(server.BLOCK, b'', 1), old)
        self.assertIn(b'header Cache-Control "no-store"', new)
        self.assertIn(b'max-age=31536000, immutable', new)
        self.assertEqual(server.inject_route(new), new)

    def test_unreviewed_caddy_configuration_rejected(self):
        for data in [b'changed.site { }', server.ANCHOR + b'\n# /client-updates\n}', server.ANCHOR + b'\n# BEGIN MIR2 NATIVE CLIENT UPDATES V1\n}']:
            with self.subTest(config=data), self.assertRaises(ValueError):
                server.inject_route(data)

    def test_root_ownership_guard_rejects_shared_or_unowned_paths(self):
        for uid, mode in [(1000, 0o100644), (0, 0o100666)]:
            fake = types.SimpleNamespace(st_mode=mode, st_nlink=1, st_uid=uid)
            with self.subTest(uid=uid, mode=mode), mock.patch.object(Path, 'lstat', return_value=fake), self.assertRaisesRegex(ValueError, 'root-owned'):
                self.original_plain(self.root / 'owned.txt', root_owned=True)

    def test_validation_environment_uses_only_referenced_service_names(self):
        # Synthetic values; never inspect live service secrets in this fixture.
        result = server.filter_validation_environment(b'header X-Test {$NEEDED}', b'NEEDED=synthetic-value\0UNUSED=must-not-copy\0', {'PATH': 'synthetic-path'})
        self.assertEqual(result, {'PATH': 'synthetic-path', 'NEEDED': 'synthetic-value'})
        self.assertNotIn('UNUSED', result)

    def test_validation_environment_missing_empty_or_duplicate_stops(self):
        for data in [b'OTHER=synthetic\0', b'NEEDED=\0', b'NEEDED=one\0NEEDED=two\0']:
            with self.subTest(environment=data), self.assertRaises(ValueError):
                server.filter_validation_environment(b'header X-Test {$NEEDED}', data, {})

    def test_network_verifier_covers_head_method_and_directory_isolation(self):
        feed_bytes, signature = b'fixture discovery', b'fixture signature'
        responses = {('GET', '/client-updates/latest.json'): (200, feed_bytes, {'Cache-Control': 'no-store'}),
                     ('GET', '/client-updates/latest.p7s'): (200, signature, {'Cache-Control': 'no-store'})}
        for component in [self.feed['game'], self.feed['engine']]:
            for entry in component['metadata']:
                responses[('HEAD', '/client-updates/' + component['directory'] + '/' + entry['path'])] = (200, b'', {'Content-Length': str(entry['size']), 'Cache-Control': 'public, immutable'})
        for path in ['/client-updates/', '/client-updates/releases/', '/client-updates/feed-current/latest.json',
                     '/client-updates/releases/game-r6/', '/client-updates/releases/updater-v1/']:
            responses[('GET', path)] = (404, b'', {})
        responses[('POST', '/client-updates/latest.json')] = (405, b'', {})
        calls = []

        class FakeConnection:
            def __init__(self, *args, **kwargs):
                pass
            def request(self, method, path, headers):
                self.key = (method, path)
                calls.append(self.key)
            def getresponse(self):
                status, body, headers = responses[self.key]
                return types.SimpleNamespace(status=status, read=lambda limit: body[:limit], getheader=lambda name, default=None: headers.get(name, default))
            def close(self):
                pass
        with mock.patch.object(server, 'LocalTLS', FakeConnection):
            server.verify_serving(feed_bytes, signature, self.feed)
            self.assertEqual(set(calls), set(responses))
            responses[('GET', '/client-updates/releases/')] = (200, b'unwanted directory listing', {})
            with self.assertRaisesRegex(ValueError, 'isolation check'):
                server.verify_serving(feed_bytes, signature, self.feed)

    def plan_command(self, output):
        feed_root = self.root / 'fixture-feed'
        feed_root.mkdir()
        (feed_root / 'latest.json').write_bytes(fixtures.dumped(self.feed))
        (feed_root / 'latest.p7s').write_bytes(b'FAKE signature fixture; never published')
        return [sys.executable, '-B', str(QA / 'deploy-prepare-plan.py'), '--archive', str(self.archive),
                '--feed-root', str(feed_root), '--output-root', str(output), '--sequence', '3',
                '--expected-caddy-sha256', 'D' * 64, '--externally-verified-signatures']

    def test_plan_cli_emits_sha_pinned_external_fresh_output_without_connection(self):
        output = self.root / 'external-plan'
        result = subprocess.run(self.plan_command(output), capture_output=True, text=True, encoding='utf-8')
        self.assertEqual(result.returncode, 0, result.stderr)
        plan = json.loads(result.stdout)
        self.assertFalse(plan['serverMutated'])
        self.assertEqual(plan['sequence'], 3)
        self.assertEqual(len(json.loads((output / 'deploy-upload.json').read_bytes())), 6)
        self.assertIn('Privileged script identity/path guard', (output / 'deploy-apply.sh').read_text())
        self.assertEqual(plan['inputs']['release.tar.gz']['sha256'], helper.sha256(self.archive.read_bytes()))
        preparer = load('plan_fixture_module', QA / 'deploy-prepare-plan.py')
        compile(preparer.BOOTSTRAP, '<SHA-pinned-root-bootstrap>', 'exec')

    def test_plan_cli_preserves_existing_output(self):
        output = self.root / 'existing-plan'
        output.mkdir()
        marker = output / 'preserve.txt'
        marker.write_bytes(b'existing output')
        result = subprocess.run(self.plan_command(output), capture_output=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(marker.read_bytes(), b'existing output')
        self.assertEqual(list(output.iterdir()), [marker])

    def test_plan_cli_rejects_linked_output_parent(self):
        actual = self.root / 'actual-output-parent'
        actual.mkdir()
        linked = self.root / 'linked-output-parent'
        try:
            linked.symlink_to(actual, target_is_directory=True)
        except OSError as exc:
            self.skipTest('Symlink privilege unavailable: ' + str(exc))
        result = subprocess.run(self.plan_command(linked / 'unsafe-plan'), capture_output=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse((actual / 'unsafe-plan').exists())


if __name__ == '__main__':
    unittest.main(verbosity=2)
