#!/usr/bin/env python3
"""Read-only host metadata audit; only this tool's private JSON reports are pruned.

No environment, file contents, database queries or service mutations are collected.
Disk warning/critical alone exits zero; collection or storage errors exit nonzero.
"""
import argparse
import datetime
import json
import os
import re
import stat
import subprocess
import sys
import uuid

PRODUCER = 'mir2-retention-daily-audit'
OUTPUT_DIR = '/var/lib/mir2-retention-audit'
MAX_REPORT_BYTES = 5 * 1024 * 1024
KEEP_REPORTS = 14
MAX_DIRECTORY_ENTRIES = 256
RECORD_NAME = re.compile(r'audit-([0-9]{8}T[0-9]{6}Z)\.json\Z', re.ASCII)
LIVE_UNITS = ('mir2-gateway.service', 'mir2-playtest.service', 'caddy.service', 'mir2-playtest-wallet-login.service')
PROTECTED_ROOTS = ('/srv/mir2-client-updates', '/var/backups', '/var/lib/mir2', '/var/lib/mir2-playtest', '/var/lib/postgresql', '/opt')
FEED_FILES = tuple('/srv/mir2-client-updates/' + prefix + 'feed-current/latest.' + suffix
                   for prefix in ('packed/', 'five-class/', '') for suffix in ('json', 'p7s'))


class AuditError(Exception):
    def __init__(self, code):
        super().__init__(code)
        self.code = code


def require(value, code):
    if not value:
        raise AuditError(code)


def _unique_pairs(items):
    result = {}
    for key, value in items:
        require(key not in result, 'duplicate_owned_audit_key')
        result[key] = value
    return result


def _utc(value):
    require(isinstance(value, datetime.datetime) and value.tzinfo is not None, 'invalid_audit_time')
    return value.astimezone(datetime.timezone.utc).replace(microsecond=0)


def record_name(created_at):
    try:
        value = datetime.datetime.strptime(created_at, '%Y-%m-%dT%H:%M:%SZ')
    except (ValueError, TypeError):
        raise AuditError('invalid_audit_time') from None
    require(value.strftime('%Y-%m-%dT%H:%M:%SZ') == created_at, 'invalid_audit_time')
    return value.strftime('audit-%Y%m%dT%H%M%SZ.json')


def encode_report(report):
    require(isinstance(report, dict) and report.get('producer') == PRODUCER
            and type(report.get('schemaVersion')) is int and report['schemaVersion'] == 1
            and report.get('status') in ('ok', 'warning', 'critical', 'error'), 'invalid_report_schema')
    record_name(report.get('createdAt'))
    try:
        data = (json.dumps(report, sort_keys=True, ensure_ascii=True, allow_nan=False, separators=(',', ':')) + '\n').encode('ascii')
    except (TypeError, ValueError, RecursionError):
        raise AuditError('invalid_report_json') from None
    require(len(data) <= MAX_REPORT_BYTES, 'report_too_large')
    return data


def collect_report(now=None, statvfs_fn=None, lstat_fn=os.lstat, stat_fn=os.stat, run_fn=subprocess.run,
                   include_feeds=True):
    now = _utc(now or datetime.datetime.now(datetime.timezone.utc))
    report = {'schemaVersion': 1, 'producer': PRODUCER, 'createdAt': now.strftime('%Y-%m-%dT%H:%M:%SZ'),
              'status': 'ok', 'readOnlyCollection': True, 'rootFilesystem': {}, 'protectedRoots': [],
              'liveUnits': [], 'feedMetadata': [], 'errors': []}
    try:
        space = (statvfs_fn or os.statvfs)('/')
        total, available = space.f_blocks * space.f_frsize, space.f_bavail * space.f_frsize
        require(total > 0 and 0 <= available <= total, 'invalid_statvfs')
        percent = available * 100 / total
        level = 'critical' if percent <= 10 or available <= 5 * 1024**3 else 'warning' if percent <= 20 or available <= 10 * 1024**3 else 'ok'
        report['rootFilesystem'] = {'path': '/', 'totalBytes': total, 'availableBytes': available,
                                    'availablePercent': round(percent, 4), 'status': level}
        report['status'] = level
    except (OSError, AttributeError, ValueError, AuditError):
        report['errors'].append('root_filesystem_collection_failed')
    for path in PROTECTED_ROOTS:
        item = {'path': path, 'exists': False, 'isDirectory': False}
        try:
            found = stat_fn(path)
            item.update(exists=True, isDirectory=stat.S_ISDIR(found.st_mode))
            if not item['isDirectory']:
                report['errors'].append('protected_root_not_directory')
        except OSError:
            report['errors'].append('protected_root_unavailable')
        report['protectedRoots'].append(item)
    for unit in LIVE_UNITS:
        item = {'unit': unit, 'active': False}
        try:
            result = run_fn(['/usr/bin/systemctl', 'is-active', '--', unit], stdin=subprocess.DEVNULL,
                            stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, timeout=4, check=False)
            item['active'] = result.returncode == 0 and result.stdout.strip() == b'active'
            if not item['active']:
                report['errors'].append('live_unit_not_active')
        except (OSError, subprocess.SubprocessError):
            report['errors'].append('live_unit_collection_failed')
        report['liveUnits'].append(item)
    if include_feeds:
        for path in FEED_FILES:
            item = {'path': path, 'available': False}
            try:
                found = lstat_fn(path)
                require(stat.S_ISREG(found.st_mode), 'feed_not_regular')
                item.update(available=True, size=found.st_size, mtimeNs=found.st_mtime_ns,
                            dev=found.st_dev, ino=found.st_ino)
            except (OSError, AuditError):
                report['errors'].append('feed_metadata_unavailable')
            report['feedMetadata'].append(item)
    if report['errors']:
        report['status'] = 'error'
    return report


def _identity(found):
    return (found.st_dev, found.st_ino, found.st_mode, found.st_uid, found.st_nlink,
            found.st_size, found.st_mtime_ns, found.st_ctime_ns)


def _regular(found, owner):
    require(stat.S_ISREG(found.st_mode) and found.st_uid == owner and found.st_nlink == 1
            and stat.S_IMODE(found.st_mode) == 0o600 and 0 <= found.st_size <= MAX_REPORT_BYTES,
            'unsafe_audit_file')


def _open_directory(path, create=False):
    require(isinstance(path, str) and path.startswith('/') and path != '/' and all(part not in ('', '.', '..') for part in path[1:].split('/')), 'invalid_audit_directory')
    flags = os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC
    current = os.open('/', flags)
    try:
        parts = path[1:].split('/')
        for index, part in enumerate(parts):
            if create and index == len(parts) - 1:
                try:
                    os.mkdir(part, 0o700, dir_fd=current)
                except FileExistsError:
                    pass
            child = os.open(part, flags, dir_fd=current)
            os.close(current)
            current = child
        return current
    except BaseException:
        os.close(current)
        raise


class AuditStore:
    def __init__(self, path, owner=0):
        require(sys.platform.startswith('linux'), 'linux_required')
        import fcntl
        self.path, self.owner, self.directory, self.lock, self.removed_count = path, owner, None, None, 0
        try:
            self.directory = _open_directory(path, create=True)
            found = os.fstat(self.directory)
            require(found.st_uid == owner and stat.S_IMODE(found.st_mode) == 0o700, 'unsafe_audit_directory')
            self.directory_identity = (found.st_dev, found.st_ino)
            try:
                self.lock = os.open('.audit.lock', os.O_RDWR | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW | os.O_CLOEXEC, 0o600, dir_fd=self.directory)
            except FileExistsError:
                self.lock = os.open('.audit.lock', os.O_RDWR | os.O_NOFOLLOW | os.O_CLOEXEC, dir_fd=self.directory)
            _regular(os.fstat(self.lock), owner)
            require(os.fstat(self.lock).st_size == 0, 'unsafe_audit_lock')
            try:
                fcntl.flock(self.lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
            except BlockingIOError:
                raise AuditError('audit_already_locked') from None
            self._binding()
            self.inventory()
        except BaseException:
            self.close()
            raise

    def close(self):
        for name in ('lock', 'directory'):
            fd = getattr(self, name, None)
            if fd is not None:
                os.close(fd)
                setattr(self, name, None)

    def __enter__(self):
        return self

    def __exit__(self, *_):
        self.close()

    def _binding(self):
        fresh = _open_directory(self.path)
        try:
            found = os.fstat(fresh)
            require((found.st_dev, found.st_ino) == self.directory_identity and found.st_uid == self.owner
                    and stat.S_IMODE(found.st_mode) == 0o700, 'audit_directory_changed')
        finally:
            os.close(fresh)
        current = os.stat('.audit.lock', dir_fd=self.directory, follow_symlinks=False)
        require(_identity(current) == _identity(os.fstat(self.lock)), 'audit_lock_changed')
        _regular(current, self.owner)

    def inventory(self):
        self._binding()
        records = {}
        # A fresh open description avoids reusing scandir's directory offset.
        scan_fd = os.open('.', os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC, dir_fd=self.directory)
        try:
            names = []
            with os.scandir(scan_fd) as stream:
                for index, item in enumerate(stream):
                    require(index < MAX_DIRECTORY_ENTRIES, 'audit_directory_entry_limit')
                    names.append(item.name)
        finally:
            os.close(scan_fd)
        for name in names:
            found = os.stat(name, dir_fd=self.directory, follow_symlinks=False)
            require(not stat.S_ISLNK(found.st_mode) and not stat.S_ISDIR(found.st_mode), 'unsafe_audit_directory_entry')
            if not RECORD_NAME.fullmatch(name):
                continue  # Unknown files are never retention candidates.
            _regular(found, self.owner)
            fd = os.open(name, os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC, dir_fd=self.directory)
            try:
                require(_identity(os.fstat(fd)) == _identity(found), 'audit_file_changed')
                with os.fdopen(os.dup(fd), 'rb') as source:
                    data = source.read(MAX_REPORT_BYTES + 1)
                require(len(data) <= MAX_REPORT_BYTES, 'unsafe_audit_file')
                try:
                    value = json.loads(data, object_pairs_hook=_unique_pairs)
                except (ValueError, UnicodeError, RecursionError):
                    raise AuditError('invalid_owned_audit_json') from None
                encode_report(value)
                require(record_name(value['createdAt']) == name, 'audit_name_mismatch')
                require(_identity(os.fstat(fd)) == _identity(found), 'audit_file_changed')
            finally:
                os.close(fd)
            records[name] = found
        self._binding()
        return records

    def publish(self, report):
        data, name = encode_report(report), record_name(report['createdAt'])
        records = self.inventory()
        require(name not in records, 'audit_name_collision')
        require(not records or name > max(records), 'audit_clock_before_existing_record')
        temporary = '.audit-tmp-' + uuid.uuid4().hex
        self._binding()
        fd = os.open(temporary, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW | os.O_CLOEXEC, 0o600, dir_fd=self.directory)
        held, pending = os.fstat(fd), True
        try:
            view = memoryview(data)
            while view:
                written = os.write(fd, view)
                require(written > 0, 'audit_write_failed')
                view = view[written:]
            os.fsync(fd)
            self._binding()
            current = os.stat(temporary, dir_fd=self.directory, follow_symlinks=False)
            require((current.st_dev, current.st_ino) == (held.st_dev, held.st_ino), 'audit_temporary_changed')
            # link() publishes the fully written file atomically and refuses an existing name.
            os.link(temporary, name, src_dir_fd=self.directory, dst_dir_fd=self.directory, follow_symlinks=False)
            os.unlink(temporary, dir_fd=self.directory)
            pending = False
            _regular(os.fstat(fd), self.owner)
            require(_identity(os.stat(name, dir_fd=self.directory, follow_symlinks=False)) == _identity(os.fstat(fd)), 'audit_file_changed')
            os.fsync(self.directory)
            return name
        finally:
            try:
                if pending:
                    self._binding()
                    current = os.stat(temporary, dir_fd=self.directory, follow_symlinks=False)
                    require(current.st_dev == held.st_dev and current.st_ino == held.st_ino, 'audit_temporary_changed')
                    os.unlink(temporary, dir_fd=self.directory)
            finally:
                os.close(fd)

    def prune(self):
        records = self.inventory()
        for name in sorted(records)[:-KEEP_REPORTS]:
            self._binding()
            fd = os.open(name, os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC, dir_fd=self.directory)
            try:
                current = os.stat(name, dir_fd=self.directory, follow_symlinks=False)
                require(_identity(current) == _identity(records[name]) == _identity(os.fstat(fd)), 'audit_file_changed')
                _regular(current, self.owner)
                os.unlink(name, dir_fd=self.directory)
                self.removed_count += 1
                require(os.fstat(fd).st_nlink == 0, 'removed_audit_identity_changed')
            finally:
                os.close(fd)
        os.fsync(self.directory)
        return self.removed_count


class PrivateParser(argparse.ArgumentParser):
    def error(self, _):
        raise AuditError('invalid_arguments')


def main(argv=None):
    result, store, written = {}, None, None
    try:
        parser = PrivateParser(description=__doc__)
        parser.add_argument('--skip-feed-metadata', action='store_true')
        args = parser.parse_args(argv)
        require(sys.platform.startswith('linux') and os.geteuid() == 0, 'linux_root_required')
        with AuditStore(OUTPUT_DIR) as store:
            report = collect_report(include_feeds=not args.skip_feed_metadata)
            written = store.publish(report)
            removed = store.prune()
            result = {'status': report['status'], 'auditFile': written, 'metadataFilesRemoved': removed,
                      'collectionErrors': report['errors']}
        exit_code = 1 if report['status'] == 'error' else 0
    except (AuditError, OSError, ValueError, TypeError, KeyboardInterrupt) as error:
        result = {'status': 'error', 'error': error.code if isinstance(error, AuditError) else 'audit_operation_failed',
                  'auditFile': written, 'metadataFilesRemoved': store.removed_count if store else 0}
        exit_code = 1
    except Exception:
        result = {'status': 'error', 'error': 'audit_internal_error', 'auditFile': written,
                  'metadataFilesRemoved': store.removed_count if store else 0}
        exit_code = 1
    print(json.dumps(result, sort_keys=True))
    return exit_code


if __name__ == '__main__':
    sys.exit(main())
