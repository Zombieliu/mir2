#!/usr/bin/env python3
"""Pure report tests plus Linux-only, owned temporary audit-store fixtures.

No SSH, real systemctl, service, journal, database, or protected-root writes.
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
import types
import unittest
from unittest import mock


sys.dont_write_bytecode = True
SPEC = importlib.util.spec_from_file_location('retention_daily_audit', Path(__file__).with_name('daily_audit.py'))
AUDIT = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(AUDIT)
PRODUCER = 'mir2-retention-daily-audit'
LIMIT = 5 * 1024 * 1024


def report(at='2026-10-30T01:02:03Z', status='ok'):
    return {'producer': PRODUCER, 'schemaVersion': 1, 'createdAt': at, 'status': status}


def filename(at):
    dt = datetime.datetime.fromisoformat(at.replace('Z', '+00:00'))
    return dt.strftime('audit-%Y%m%dT%H%M%SZ.json')


class EncodeTests(unittest.TestCase):
    def test_minimal_report_roundtrips_as_plain_utf8_json(self):
        value = report()
        encoded = AUDIT.encode_report(value)
        self.assertIsInstance(encoded, bytes)
        self.assertLessEqual(len(encoded), LIMIT)
        self.assertEqual(json.loads(encoded.decode('utf-8')), value)

    def test_required_producer_and_schema_are_not_truthy_substitutes(self):
        for key, bad in [('producer', 'another-tool'), ('producer', None),
                         ('schemaVersion', 2), ('schemaVersion', True), ('schemaVersion', '1')]:
            value = report(); value[key] = bad
            with self.assertRaises(AUDIT.AuditError, msg=(key, bad)):
                AUDIT.encode_report(value)
        for key in ('producer', 'schemaVersion', 'createdAt', 'status'):
            value = report(); value.pop(key)
            with self.assertRaises(AUDIT.AuditError, msg=key):
                AUDIT.encode_report(value)

    def test_status_enum_is_explicit(self):
        for status in ('ok', 'warning', 'critical', 'error'):
            self.assertEqual(json.loads(AUDIT.encode_report(report(status=status)))['status'], status)
        for status in ('passed', '', True, None, 'ok\n'):
            with self.assertRaises(AUDIT.AuditError):
                AUDIT.encode_report(report(status=status))

    def test_timestamp_must_be_real_utc_not_filename_or_path_input(self):
        for at in ('2026-02-30T01:02:03Z', '../audit.json', '2026-10-30',
                   '2026-10-30T01:02:03', None):
            with self.assertRaises(AUDIT.AuditError, msg=at):
                AUDIT.encode_report(report(at=at))

    def test_nonfinite_nested_numbers_and_non_json_values_reject(self):
        for bad in (float('nan'), float('inf'), -float('inf'), {1, 2}, Path('/private'), object()):
            value = report(); value['observations'] = [{'nested': bad}]
            with self.assertRaises(AUDIT.AuditError):
                AUDIT.encode_report(value)

    def test_limit_is_utf8_bytes_not_unicode_character_count(self):
        value = report(); value['observations'] = '\u4e2d' * (LIMIT // 3 + 1)
        with self.assertRaises(AUDIT.AuditError):
            AUDIT.encode_report(value)
        value['observations'] = 'a' * LIMIT
        with self.assertRaises(AUDIT.AuditError):
            AUDIT.encode_report(value)

    def test_encoder_does_not_mutate_original_report(self):
        value = report(); before = copy.deepcopy(value)
        AUDIT.encode_report(value)
        self.assertEqual(value, before)

    def test_encoded_report_roundtrips_through_owned_temporary_file(self):
        with tempfile.TemporaryDirectory(prefix='mir2-audit-json-test-') as folder:
            target = Path(folder)/filename(report()['createdAt'])
            target.write_bytes(AUDIT.encode_report(report()))
            self.assertEqual(json.loads(target.read_bytes()), report())
            self.assertLessEqual(target.stat().st_size, LIMIT)


class CollectionTests(unittest.TestCase):
    def collect(self, total=100*1024**3, available=30*1024**3, run=None, feeds=True):
        self.commands = []
        def runner(command, **kwargs):
            self.commands.append((command, kwargs))
            if run:
                return run(command, **kwargs)
            return types.SimpleNamespace(returncode=0, stdout=b'active\n')
        self.lstat = mock.Mock(return_value=types.SimpleNamespace(st_mode=stat.S_IFREG, st_size=12,
            st_mtime_ns=1234, st_dev=8, st_ino=100))
        self.stat = mock.Mock(return_value=types.SimpleNamespace(st_mode=stat.S_IFDIR))
        return AUDIT.collect_report(now=datetime.datetime(2026,10,30,tzinfo=datetime.timezone.utc),
            statvfs_fn=lambda path: types.SimpleNamespace(f_blocks=total//4096,f_frsize=4096,f_bavail=available//4096),
            stat_fn=self.stat,lstat_fn=self.lstat,run_fn=runner,include_feeds=feeds)

    def test_only_fixed_units_roots_and_metadata_are_collected(self):
        value=self.collect()
        self.assertEqual(value['status'],'ok');self.assertTrue(value['readOnlyCollection'])
        self.assertEqual([call[0] for call in self.commands],
                         [['/usr/bin/systemctl','is-active','--',unit] for unit in AUDIT.LIVE_UNITS])
        self.assertEqual(self.stat.call_args_list,[mock.call(path) for path in AUDIT.PROTECTED_ROOTS])
        self.assertEqual(self.lstat.call_args_list,[mock.call(path) for path in AUDIT.FEED_FILES])
        self.assertEqual(len(value['liveUnits']),4);self.assertEqual(len(value['feedMetadata']),6)
        self.assertNotIn('contents',json.dumps(value));self.assertNotIn('environment',json.dumps(value))

    def test_disk_percent_or_absolute_thresholds_and_critical_precedence(self):
        cases=[(100,30,'ok'),(100,20,'warning'),(40,10,'warning'),(20,5,'critical'),(100,10,'critical')]
        for total,available,expected in cases:
            with self.subTest(total=total,available=available):
                value=self.collect(total*1024**3,available*1024**3)
                self.assertEqual(value['rootFilesystem']['status'],expected)
                self.assertEqual(value['status'],expected)
        self.assertEqual(self.collect(100*1024**3,20*1024**3+4096)['status'],'ok')

    def test_optional_feed_metadata_omission_performs_no_file_metadata_reads(self):
        value=self.collect(feeds=False)
        self.assertEqual(value['feedMetadata'],[]);self.lstat.assert_not_called()
        self.assertEqual(value['status'],'ok')

    def test_unit_raw_stdout_and_private_exception_text_never_enter_report(self):
        def inactive(*args,**kwargs):
            return types.SimpleNamespace(returncode=3,stdout=b'SECRET_ENV_SENTINEL')
        value=self.collect(run=inactive)
        self.assertEqual(value['status'],'error');self.assertNotIn('SECRET_ENV_SENTINEL',json.dumps(value))
        def forbidden(*args,**kwargs):
            raise PermissionError('SECRET_ENV_SENTINEL')
        value=self.collect(run=forbidden)
        self.assertEqual(value['status'],'error');self.assertNotIn('SECRET_ENV_SENTINEL',json.dumps(value))

    def test_missing_protected_root_is_error_without_raw_exception(self):
        with mock.patch.object(AUDIT,'PROTECTED_ROOTS',('/protected',)):
            value=AUDIT.collect_report(statvfs_fn=lambda path:types.SimpleNamespace(f_blocks=100,f_frsize=1024**3,f_bavail=30),
                stat_fn=mock.Mock(side_effect=PermissionError('SECRET_ENV_SENTINEL')),
                run_fn=lambda *a,**k:types.SimpleNamespace(returncode=0,stdout=b'active\n'),include_feeds=False)
        self.assertEqual(value['status'],'error');self.assertFalse(value['protectedRoots'][0]['exists'])
        self.assertNotIn('SECRET_ENV_SENTINEL',json.dumps(value))

    def test_warning_critical_exit_zero_but_collection_error_exits_nonzero(self):
        for status,expected in [('warning',0),('critical',0),('error',1)]:
            value={**report(status=status),'errors':[] if status!='error' else ['live_unit_not_active']}
            context=mock.MagicMock();store=context.__enter__.return_value
            store.publish.return_value=filename(value['createdAt']);store.prune.return_value=0
            output=io.StringIO()
            with mock.patch.object(AUDIT.sys,'platform','linux'),mock.patch.object(AUDIT.os,'geteuid',return_value=0,create=True), \
                 mock.patch.object(AUDIT,'AuditStore',return_value=context),mock.patch.object(AUDIT,'collect_report',return_value=value), \
                 mock.patch.object(AUDIT.sys,'stdout',output):
                self.assertEqual(AUDIT.main([]),expected)
            self.assertEqual(json.loads(output.getvalue())['status'],status)

    def test_cli_invalid_arguments_do_not_echo_secret_or_traceback(self):
        output=io.StringIO()
        with mock.patch.object(AUDIT.sys,'stdout',output),mock.patch.object(AUDIT.sys,'stderr',output):
            self.assertEqual(AUDIT.main(['--SECRET_ENV_SENTINEL']),1)
        self.assertNotIn('SECRET_ENV_SENTINEL',output.getvalue());self.assertNotIn('Traceback',output.getvalue())

    def test_partial_prune_error_keeps_published_name_and_removed_count(self):
        context=mock.MagicMock();store=context.__enter__.return_value
        store.publish.return_value=filename(report()['createdAt']);store.removed_count=1
        store.prune.side_effect=PermissionError('SECRET_ENV_SENTINEL')
        output=io.StringIO()
        with mock.patch.object(AUDIT.sys,'platform','linux'),mock.patch.object(AUDIT.os,'geteuid',return_value=0,create=True), \
             mock.patch.object(AUDIT,'AuditStore',return_value=context), \
             mock.patch.object(AUDIT,'collect_report',return_value={**report(),'errors':[]}), \
             mock.patch.object(AUDIT.sys,'stdout',output):
            self.assertEqual(AUDIT.main([]),1)
        result=json.loads(output.getvalue())
        self.assertEqual(result['metadataFilesRemoved'],1);self.assertIsNotNone(result['auditFile'])
        self.assertNotIn('SECRET_ENV_SENTINEL',output.getvalue())


@unittest.skipUnless(sys.platform.startswith('linux'), 'Real no-follow/dirfd audit-store fixtures require Linux')
class LinuxStoreTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='mir2-retention-audit-test-')
        self.parent = Path(self.temp.name)
        self.root = self.parent / 'reports'
        self.uid = os.geteuid()

    def tearDown(self):
        self.temp.cleanup()

    def make_store(self):
        return AUDIT.AuditStore(str(self.root), owner=self.uid)

    def write_known(self, value, name=None, mode=0o600):
        self.root.mkdir(mode=0o700, exist_ok=True)
        p = self.root / (name or filename(value['createdAt']))
        p.write_bytes(AUDIT.encode_report(value))
        p.chmod(mode)
        return p

    def test_creates_only_own_missing_directory_with_private_permissions(self):
        with self.make_store():
            s = self.root.stat()
            self.assertEqual(stat.S_IMODE(s.st_mode), 0o700)
            self.assertEqual(s.st_uid, self.uid)
        self.assertEqual(set(p.name for p in self.parent.iterdir()), {'reports'})

    def test_publish_matches_created_at_and_does_not_overwrite(self):
        value = report()
        with self.make_store() as store:
            name = store.publish(value)
            self.assertEqual(name, filename(value['createdAt']))
            saved = (self.root / name).read_bytes()
            self.assertEqual(json.loads(saved), value)
            s = (self.root / name).stat()
            self.assertEqual((stat.S_IMODE(s.st_mode), s.st_uid, s.st_nlink), (0o600, self.uid, 1))
            with self.assertRaises(AUDIT.AuditError):
                store.publish({**value, 'status': 'critical'})
            self.assertEqual((self.root / name).read_bytes(), saved)

    def test_keep_fourteen_by_validated_record_time_not_mtime(self):
        for day in range(1, 17):
            p = self.write_known(report(at=f'2026-10-{day:02d}T01:02:03Z'))
            os.utime(p, (1000 + 17 - day, 1000 + 17 - day))
        with self.make_store() as store:
            self.assertEqual(store.prune(), 2)
            self.assertEqual(store.removed_count, 2)
        remaining = sorted(p.name for p in self.root.glob('audit-*.json'))
        self.assertEqual(remaining, [filename(f'2026-10-{day:02d}T01:02:03Z') for day in range(3, 17)])

    def test_unknown_regular_file_preserved_byte_for_byte(self):
        self.root.mkdir(mode=0o700)
        unknown = self.root / 'operator-evidence.json'
        unknown.write_bytes(b'UNRELATED-EVIDENCE')
        for day in range(1, 17):
            self.write_known(report(at=f'2026-10-{day:02d}T01:02:03Z'))
        with self.make_store() as store:
            self.assertEqual(store.prune(), 2)
        self.assertEqual(unknown.read_bytes(), b'UNRELATED-EVIDENCE')

    def test_any_symlink_is_refused_and_external_target_survives(self):
        self.root.mkdir(mode=0o700)
        outside = self.parent / 'keep'
        outside.write_bytes(b'KEEP')
        (self.root / 'unrelated-name').symlink_to(outside)
        with self.assertRaises(AUDIT.AuditError):
            with self.make_store() as store:
                store.prune()
        self.assertEqual(outside.read_bytes(), b'KEEP')

    def test_subdirectory_is_refused_not_recursively_pruned(self):
        self.root.mkdir(mode=0o700)
        folder = self.root / 'nested'
        folder.mkdir(); (folder / 'keep').write_bytes(b'KEEP')
        with self.assertRaises(AUDIT.AuditError):
            with self.make_store() as store:
                store.prune()
        self.assertEqual((folder / 'keep').read_bytes(), b'KEEP')

    def test_root_symlink_and_permissive_root_modes_refused(self):
        self.root.symlink_to(self.parent, target_is_directory=True)
        with self.assertRaises((AUDIT.AuditError,OSError)):
            with self.make_store():
                pass
        self.root.unlink()
        self.root.mkdir(mode=0o755)
        with self.assertRaises(AUDIT.AuditError):
            with self.make_store():
                pass

    def test_wrong_expected_owner_is_refused(self):
        self.root.mkdir(mode=0o700)
        with self.assertRaises(AUDIT.AuditError):
            with AUDIT.AuditStore(str(self.root), owner=self.uid + 1):
                pass

    def test_known_record_needs_exact_mode_single_link_and_size_bound(self):
        p = self.write_known(report(), mode=0o644)
        with self.assertRaises(AUDIT.AuditError):
            with self.make_store() as store:
                store.prune()
        p.chmod(0o600)
        os.link(p, self.parent / 'outside-hardlink')
        with self.assertRaises(AUDIT.AuditError):
            with self.make_store() as store:
                store.prune()

    def test_known_filename_with_wrong_producer_or_timestamp_cannot_be_deleted(self):
        for wrong in ({**report(), 'producer': 'another-tool'}, report(at='2026-10-29T01:02:03Z')):
            p = self.root / filename(report()['createdAt'])
            self.root.mkdir(mode=0o700, exist_ok=True)
            p.write_text(json.dumps(wrong), encoding='utf-8'); p.chmod(0o600)
            with self.assertRaises(AUDIT.AuditError):
                with self.make_store() as store:
                    store.prune()
            self.assertTrue(p.exists())
            p.unlink()

    def test_store_lock_prevents_overlapping_publisher(self):
        with self.make_store():
            with self.assertRaises(AUDIT.AuditError):
                with self.make_store():
                    pass

    def test_oversize_owned_record_is_preserved_and_refused(self):
        p=self.write_known(report())
        with p.open('r+b') as stream:
            stream.truncate(LIMIT+1)
        with self.assertRaises(AUDIT.AuditError):
            with self.make_store():
                pass
        self.assertEqual(p.stat().st_size,LIMIT+1)

    def test_failed_write_never_publishes_partial_json_or_leaves_own_temp(self):
        with self.make_store() as store:
            with mock.patch.object(AUDIT.os,'write',side_effect=OSError('fixture-failure')):
                with self.assertRaises(OSError):store.publish(report())
        self.assertEqual({p.name for p in self.root.iterdir()},{'.audit.lock'})

    def test_directory_identity_change_stops_publication(self):
        with self.make_store() as store:
            moved=self.parent/'moved-original'
            self.root.rename(moved);self.root.mkdir(mode=0o700)
            with self.assertRaises(AUDIT.AuditError):store.publish(report())
        self.assertFalse(any(self.root.iterdir()))
        self.assertEqual({p.name for p in moved.iterdir()},{'.audit.lock'})

    def test_record_changed_after_inventory_is_not_pruned(self):
        for day in range(1,16):self.write_known(report(at=f'2026-10-{day:02d}T01:02:03Z'))
        with self.make_store() as store:
            original=store.inventory
            def changed():
                records=original()
                (self.root/min(records)).write_bytes(b'CHANGED')
                return records
            with mock.patch.object(store,'inventory',side_effect=changed):
                with self.assertRaises(AUDIT.AuditError):store.prune()
            self.assertEqual(store.removed_count,0)
        self.assertEqual(len(list(self.root.glob('audit-*.json'))),15)


if __name__ == '__main__':
    unittest.main()
