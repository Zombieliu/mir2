#!/usr/bin/env python3
"""Pure validation/mock tests everywhere; Linux filesystem tests own /tmp fixtures only.

No systemctl, network, database, service changes or shell deletion. Linux positive
fixtures inject a future clock so real ctime need not be forged or aged for days.
"""
import copy
import datetime
import importlib.util
import io
import json
import os
from pathlib import Path
import stat
import sys
import tempfile
import time
import types
import unittest
from unittest import mock

sys.dont_write_bytecode = True
SPEC = importlib.util.spec_from_file_location('retention_cleanup', Path(__file__).with_name('server_retention_cleanup.py'))
TOOL = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(TOOL)
NOW = 1900000000.0


def snapshot(path='/srv/.mir2-origin-fixture'):
    old = int((NOW - 72 * 3600) * 1000000000)
    return {'path': path, 'dev': 8, 'ino': 123, 'mtimeNs': old, 'ctimeNs': old, 'mntId': 42,
            'inventorySha256': 'a' * 64, 'entryCount': 3, 'totalBytes': 12, 'newestChangeNs': old}


def plan(*candidates):
    return {'schemaVersion': 1, 'createdAt': datetime.datetime.fromtimestamp(NOW - 1, datetime.timezone.utc).isoformat(),
            'referenceAuditSha256': 'b' * 64, 'allReferencesClear': True, 'ageMinHours': 48,
            'candidates': list(candidates or [snapshot()])}


def encoded(value):
    return json.dumps(value, separators=(',', ':')).encode()


class FakeFS:
    def __init__(self, *items):
        self.items = {item['path']: copy.deepcopy(item) for item in items}
        self.scans = []
        self.deletions = []
        self.on_inspect = None
        self.on_delete = None

    def inspect(self, path):
        self.scans.append(path)
        if self.on_inspect:
            self.on_inspect(path)
        return copy.deepcopy(self.items[path])

    def delete(self, item, progress, before_mutation):
        before_mutation()
        self.deletions.append(item['path'])
        if self.on_delete:
            return self.on_delete(item, progress)
        progress['entriesRemoved'] += item['entryCount']
        progress['logicalBytesUnlinked'] += item['totalBytes']


class PureTests(unittest.TestCase):
    def operate(self, value=None, fs=None, execute=False, sha=None, clock=lambda: NOW):
        value = plan() if value is None else value
        fs = FakeFS(*value['candidates']) if fs is None else fs
        data = encoded(value)
        with mock.patch.object(TOOL.sys, 'platform', 'linux'):
            return TOOL.operate(data, execute=execute, expected_sha256=sha,
                                now=clock, inspector=fs.inspect, deleter=fs.delete), fs

    def test_exact_allowlisted_immediate_children(self):
        for path in ['/srv/.mir2-origin-abc', '/srv/.mir2-client-update-stage-a', '/srv/.mir2-packed-stage-x',
                     '/srv/.mir2-engine-only-origin-a', '/srv/.mir2-v4-engine-feed-stage-a', '/tmp/mir2-origin-fixture']:
            TOOL.candidate_path(path)

    def test_path_escape_roots_backups_and_nested_paths_rejected(self):
        for path in ['/', '/srv', '/tmp', '/home/x/.mir2-origin-a', '/opt/.mir2-origin-a', '/srv/backup',
                     '/srv/.mir2-origin-a/sub', '/srv/../tmp/mir2-origin-a', '/srv//.mir2-origin-a',
                     'srv/.mir2-origin-a', '/srv/.mir2-origin-', '/srv/.mir2-origin-a\n', '/tmp/mir2-origin-a/..']:
            with self.assertRaises(TOOL.CleanupError, msg=path):
                TOOL.candidate_path(path)

    def test_duplicate_keys_at_every_level_rejected(self):
        for data in [b'{"x":1,"x":2}', b'{"x":{"secret":"A","secret":"B"}}']:
            with self.assertRaises(TOOL.CleanupError): TOOL.parse_plan(data, NOW)

    def test_oversize_malformed_and_nonfinite_json_rejected(self):
        for data in [b' ' * (TOOL.MAX_PLAN_BYTES + 1), b'{"SECRET": malformed}', b'NaN', b'Infinity', b'\xff']:
            with self.assertRaises(TOOL.CleanupError): TOOL.parse_plan(data, NOW)

    def test_reference_evidence_must_be_explicit_nonplaceholder(self):
        for key, bad in [('allReferencesClear', False), ('allReferencesClear', 1), ('referenceAuditSha256', '0'*64),
                         ('referenceAuditSha256', 'private-not-a-digest'), ('ageMinHours', 47), ('ageMinHours', True)]:
            value = plan(); value[key] = bad
            result, fs = self.operate(value)
            self.assertFalse(result['passed']); self.assertEqual(fs.deletions, [])

    def test_old_future_and_timezone_naive_plan_rejected(self):
        for seconds in [-121, 1]:
            value = plan(); value['createdAt'] = datetime.datetime.fromtimestamp(NOW + seconds, datetime.timezone.utc).isoformat()
            result, fs = self.operate(value); self.assertFalse(result['passed']); self.assertEqual(fs.scans, [])
        value = plan(); value['createdAt'] = '2030-01-01T00:00:00'
        self.assertFalse(self.operate(value)[0]['passed'])

    def test_default_check_never_deletes_or_claims_reference_proof(self):
        result, fs = self.operate()
        self.assertTrue(result['passed']); self.assertEqual(result['mode'], 'check'); self.assertEqual(fs.deletions, [])
        self.assertFalse(result['globalReferencesIndependentlyVerified']); self.assertFalse(result['atomic'])

    def test_execute_needs_exact_raw_plan_sha_before_any_scan(self):
        for expected in [None, 'c'*64, 'not-a-sha']:
            result, fs = self.operate(execute=True, sha=expected)
            self.assertFalse(result['passed']); self.assertEqual(fs.scans, []); self.assertEqual(fs.deletions, [])
        value = plan(); data = encoded(value)
        result, fs = self.operate(value, execute=True, sha=TOOL.digest(data))
        self.assertTrue(result['passed']); self.assertEqual(len(fs.deletions), 1)

    def test_execute_is_linux_only_even_with_injected_backend(self):
        value = plan(); fs = FakeFS(*value['candidates']); data = encoded(value)
        with mock.patch.object(TOOL.sys, 'platform', 'win32'):
            result = TOOL.operate(data, True, TOOL.digest(data), now=lambda: NOW, inspector=fs.inspect, deleter=fs.delete)
        self.assertFalse(result['passed']); self.assertEqual(fs.scans, [])

    def test_truthy_non_boolean_execute_cannot_mutate(self):
        result, fs = self.operate(execute='false', sha=TOOL.digest(encoded(plan())))
        self.assertEqual(result['error'], 'invalid_execute_flag'); self.assertEqual(fs.scans, [])

    def test_duplicate_candidate_rejected(self):
        result, fs = self.operate(plan(snapshot(), snapshot()))
        self.assertFalse(result['passed']); self.assertEqual(fs.scans, [])

    def test_all_candidates_checked_before_any_mutation(self):
        first, second = snapshot(), snapshot('/tmp/mir2-origin-second')
        value = plan(first, second); fs = FakeFS(first, second); fs.items[second['path']]['ino'] += 1
        result, fs = self.operate(value, fs, True, TOOL.digest(encoded(value)))
        self.assertFalse(result['passed']); self.assertEqual(fs.deletions, [])

    def test_root_identity_and_tree_change_both_fail_closed(self):
        for key in ['dev', 'ino', 'mtimeNs', 'ctimeNs', 'mntId', 'inventorySha256', 'entryCount']:
            value = plan(); fs = FakeFS(*value['candidates'])
            fs.items[value['candidates'][0]['path']][key] = 'c'*64 if key == 'inventorySha256' else fs.items[value['candidates'][0]['path']][key] + 1
            result, fs = self.operate(value, fs, True, TOOL.digest(encoded(value)))
            self.assertFalse(result['passed'], key); self.assertEqual(fs.deletions, [])

    def test_age_checks_latest_child_ctime_not_only_root_mtime(self):
        value = plan(); value['candidates'][0]['newestChangeNs'] = int((NOW - 1)*1000000000)
        result, fs = self.operate(value)
        self.assertEqual(result['error'], 'candidate_not_old_enough'); self.assertEqual(fs.deletions, [])

    def test_candidate_rescanned_before_deletion(self):
        value = plan(); fs = FakeFS(*value['candidates'])
        def changed(_):
            if len(fs.scans) == 2: fs.items[value['candidates'][0]['path']]['ino'] += 1
        fs.on_inspect = changed
        result, fs = self.operate(value, fs, True, TOOL.digest(encoded(value)))
        self.assertEqual(result['error'], 'candidate_changed_before_delete'); self.assertEqual(fs.deletions, [])

    def test_changed_second_candidate_retains_first_partial_receipt(self):
        first, second = snapshot(), snapshot('/tmp/mir2-origin-second'); value = plan(first, second); fs = FakeFS(first, second)
        def change_after_first(item, progress):
            progress['entriesRemoved'] += 3; progress['logicalBytesUnlinked'] += 12
            fs.items[second['path']]['inventorySha256'] = 'c'*64
        fs.on_delete = change_after_first
        result, fs = self.operate(value, fs, True, TOOL.digest(encoded(value)))
        self.assertFalse(result['passed']); self.assertTrue(result['partial']); self.assertEqual(result['entriesRemoved'], 3)
        self.assertEqual(fs.deletions, [first['path']]); self.assertEqual(result['units'][0]['status'], 'deleted')
        self.assertEqual(result['units'][1]['status'], 'failed_before_mutation')

    def test_mid_delete_failure_retains_nonatomic_progress_and_stops_batch(self):
        first, second = snapshot(), snapshot('/tmp/mir2-origin-second'); value = plan(first, second); fs = FakeFS(first, second)
        def fail_midway(item, progress):
            progress['entriesRemoved'] += 1; progress['logicalBytesUnlinked'] += 4
            raise TOOL.CleanupError('tree_changed_during_delete')
        fs.on_delete = fail_midway
        result, fs = self.operate(value, fs, True, TOOL.digest(encoded(value)))
        self.assertFalse(result['passed']); self.assertTrue(result['partial']); self.assertEqual(result['entriesRemoved'], 1)
        self.assertEqual(len(fs.deletions), 1); self.assertFalse(result['atomic'])
        self.assertEqual(result['units'][0]['status'], 'failed_partial')

    def test_plan_expiring_after_batch_check_prevents_first_delete(self):
        value = plan(); times = iter([NOW, NOW, NOW + 121]); fs = FakeFS(*value['candidates'])
        result, fs = self.operate(value, fs, True, TOOL.digest(encoded(value)), clock=lambda: next(times))
        self.assertEqual(result['error'], 'plan_not_fresh'); self.assertEqual(fs.deletions, [])

    def test_operator_interrupt_retains_partial_receipt(self):
        value = plan(); fs = FakeFS(*value['candidates'])
        def interrupted(item, progress):
            progress['entriesRemoved'] += 1
            raise KeyboardInterrupt()
        fs.on_delete = interrupted
        result, fs = self.operate(value, fs, True, TOOL.digest(encoded(value)))
        self.assertEqual(result['error'], 'operator_interrupted'); self.assertTrue(result['partial'])
        self.assertEqual(result['units'][0]['status'], 'failed_partial')

    def test_plan_expiring_during_final_scan_prevents_first_mutation(self):
        value = plan(); times = iter([NOW, NOW, NOW, NOW, NOW + 121]); fs = FakeFS(*value['candidates'])
        result, fs = self.operate(value, fs, True, TOOL.digest(encoded(value)), clock=lambda: next(times))
        self.assertEqual(result['error'], 'plan_not_fresh'); self.assertEqual(fs.deletions, [])
        self.assertEqual(result['units'][0]['status'], 'failed_before_mutation')

    def test_mount_device_or_bind_mount_id_mismatch_rejected(self):
        item = types.SimpleNamespace(st_dev=8)
        for device, mount in [(9,42),(8,43)]:
            item.st_dev = device
            with self.assertRaises(TOOL.CleanupError): TOOL._mount_guard(item, mount, 8, 42)

    def test_special_files_and_hardlinks_refused_but_child_symlink_is_leaf(self):
        for mode, links in [(stat.S_IFIFO,1),(stat.S_IFSOCK,1),(stat.S_IFCHR,1),(stat.S_IFREG,2),(stat.S_IFLNK,2)]:
            with self.assertRaises(TOOL.CleanupError): TOOL._kind(types.SimpleNamespace(st_mode=mode, st_nlink=links))
        self.assertEqual(TOOL._kind(types.SimpleNamespace(st_mode=stat.S_IFLNK, st_nlink=1)), 'symlink')

    def test_root_symlink_rejected_before_opening_root(self):
        parent = types.SimpleNamespace(st_mode=stat.S_IFDIR, st_dev=8, st_ino=1)
        link = types.SimpleNamespace(st_mode=stat.S_IFLNK, st_dev=8, st_ino=2)
        with mock.patch.object(TOOL.sys, 'platform', 'linux'), mock.patch.object(TOOL.os, 'O_NOFOLLOW', 1, create=True), \
             mock.patch.object(TOOL.os, 'O_DIRECTORY', 2, create=True), mock.patch.object(TOOL.os, 'O_PATH', 4, create=True), \
             mock.patch.object(TOOL.os, 'open', return_value=10) as opened, mock.patch.object(TOOL.os, 'close'), \
             mock.patch.object(TOOL.os, 'fstat', return_value=parent), mock.patch.object(TOOL.os, 'stat', return_value=link), \
             mock.patch.object(TOOL, '_mount_id', return_value=42):
            with self.assertRaises(TOOL.CleanupError): TOOL.LinuxTree('/tmp/mir2-origin-rootlink')
            self.assertEqual(opened.call_count, 1)

    def test_cli_error_redacts_malformed_private_json_and_os_errors(self):
        for content in [b'{"SECRET_PRIVATE_SENTINEL": malformed}', b' '*(TOOL.MAX_PLAN_BYTES+1)]:
            output = io.StringIO()
            with mock.patch('builtins.open', return_value=io.BytesIO(content)), mock.patch.object(TOOL.sys, 'stdout', output):
                self.assertEqual(TOOL.main(['private-plan.json']), 1)
            self.assertNotIn('SECRET_PRIVATE_SENTINEL', output.getvalue()); self.assertNotIn('private-plan.json', output.getvalue())


@unittest.skipUnless(sys.platform.startswith('linux'), 'Real dirfd/mount-id tests are Linux-only; never services')
class LinuxFilesystemTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='mir2-origin-fixture-', dir='/tmp')
        self.root = Path(self.temp.name)
        (self.root/'sub').mkdir(); (self.root/'a').write_bytes(b'alpha'); (self.root/'sub'/'b').write_bytes(b'beta')
        self.future = time.time() + 72*3600

    def tearDown(self):
        self.temp.cleanup()

    def prepared(self):
        value = plan(TOOL.inspect_candidate(str(self.root)))
        value['createdAt'] = datetime.datetime.fromtimestamp(self.future-1, datetime.timezone.utc).isoformat()
        return value, encoded(value)

    def test_real_check_preserves_tree_and_execute_deletes_exact_fixture(self):
        value, data = self.prepared()
        checked = TOOL.operate(data, now=lambda:self.future)
        self.assertTrue(checked['passed'], checked); self.assertTrue((self.root/'a').exists())
        removed = TOOL.operate(data, True, TOOL.digest(data), now=lambda:self.future)
        self.assertTrue(removed['passed'], removed); self.assertFalse(self.root.exists()); self.assertEqual(removed['entriesRemoved'], 4)

    def test_child_symlink_target_survives_real_deletion(self):
        with tempfile.TemporaryDirectory(prefix='retention-target-', dir='/tmp') as outside:
            target=Path(outside)/'keep';target.write_bytes(b'KEEP')
            (self.root/'external').symlink_to(outside, target_is_directory=True)
            value,data=self.prepared();result=TOOL.operate(data,True,TOOL.digest(data),now=lambda:self.future)
            self.assertTrue(result['passed'],result);self.assertEqual(target.read_bytes(),b'KEEP')

    def test_real_root_symlink_rejected(self):
        alias=Path('/tmp')/('mir2-origin-rootlink-'+self.root.name)
        alias.symlink_to(self.root, target_is_directory=True)
        try:
            with self.assertRaises(TOOL.CleanupError):TOOL.inspect_candidate(str(alias))
            self.assertTrue((self.root/'a').exists())
        finally:alias.unlink()

    def test_real_tree_change_and_hardlink_refused(self):
        value,data=self.prepared();(self.root/'new').write_bytes(b'new')
        result=TOOL.operate(data,True,TOOL.digest(data),now=lambda:self.future)
        self.assertFalse(result['passed']);self.assertTrue((self.root/'a').exists())
        os.link(self.root/'a',self.root/'hard')
        with self.assertRaises(TOOL.CleanupError):TOOL.inspect_candidate(str(self.root))

    def test_directory_metadata_change_after_scan_stops_before_mutation(self):
        value, data = self.prepared()
        original = TOOL.LinuxTree.delete
        def changed(tree, entries, progress):
            os.chmod(tree.root, 0o700 if stat.S_IMODE(os.fstat(tree.root).st_mode) != 0o700 else 0o755)
            return original(tree, entries, progress)
        with mock.patch.object(TOOL.LinuxTree, 'delete', changed):
            result = TOOL.operate(data, True, TOOL.digest(data), now=lambda:self.future)
        self.assertEqual(result['error'], 'directory_changed_before_delete'); self.assertFalse(result['partial'])
        self.assertTrue((self.root/'a').exists())

    def test_real_mid_delete_addition_reports_partial_without_deleting_new_entry(self):
        value,data=self.prepared();unlink=TOOL.os.unlink;injected=[False]
        def race(name, *args, **kwargs):
            result=unlink(name,*args,**kwargs)
            if not injected[0]:
                injected[0]=True;(self.root/'unexpected').write_bytes(b'NEW')
            return result
        with mock.patch.object(TOOL.os,'unlink',side_effect=race):
            result=TOOL.operate(data,True,TOOL.digest(data),now=lambda:self.future)
        self.assertFalse(result['passed']);self.assertTrue(result['partial']);self.assertTrue((self.root/'unexpected').exists())

    def test_leaf_replacement_at_unlink_is_detected_as_partial(self):
        value, data = self.prepared(); unlink = TOOL.os.unlink; injected = [False]
        def race(name, *args, **kwargs):
            if not injected[0]:
                injected[0] = True
                os.rename(name, 'moved-original', src_dir_fd=kwargs['dir_fd'], dst_dir_fd=kwargs['dir_fd'])
                (self.root/name).write_bytes(b'NEW')
            return unlink(name, *args, **kwargs)
        with mock.patch.object(TOOL.os, 'unlink', side_effect=race):
            result = TOOL.operate(data, True, TOOL.digest(data), now=lambda:self.future)
        self.assertEqual(result['error'], 'removed_entry_identity_changed'); self.assertTrue(result['partial'])
        self.assertEqual((self.root/'moved-original').read_bytes(), b'alpha')


if __name__ == '__main__':
    unittest.main()
