#!/usr/bin/env python3
"""Metadata-bound deletion of exact old staging directories; never a reference scanner.

Default PLAN.json checks only. --execute requires Linux and --expected-sha256
of the complete input bytes. --inspect PATH... emits metadata fingerprints/counts,
not file contents. Allowed targets are immediate named children of /srv or /tmp.
An operator-owned fresh reference audit is mandatory but is NOT independently
proved here. No shell, services, feeds, databases, backups or journal operations.
Deletion is not atomic: concurrent changes stop the batch and retain a partial
receipt. The operator must exclude concurrent publishers/writers; this is not a
transaction against another privileged process changing the filesystem.
"""
import argparse
import datetime
import hashlib
import json
import os
import re
import stat
import sys
import time

MAX_PLAN_BYTES = 1024 * 1024
MAX_TARGETS = 32
MAX_ENTRIES = 100000
MAX_DEPTH = 64
SHA = re.compile(r'[0-9a-f]{64}\Z')
TARGET = re.compile(r'(?:/srv/\.mir2-(?:origin|client-update-stage|packed-stage|engine-only-origin|v4-engine-feed-stage)|/tmp/mir2-origin)-[A-Za-z0-9][A-Za-z0-9._-]{0,180}\Z', re.ASCII)
SNAPSHOT_KEYS = {'path', 'dev', 'ino', 'mtimeNs', 'ctimeNs', 'mntId', 'inventorySha256', 'entryCount', 'totalBytes', 'newestChangeNs'}


class CleanupError(Exception):
    def __init__(self, code):
        super().__init__(code)
        self.code = code


def require(condition, code):
    if not condition:
        raise CleanupError(code)


def candidate_path(value):
    require(isinstance(value, str) and TARGET.fullmatch(value) is not None, 'candidate_path_not_allowed')
    return value.rsplit('/', 1)


def _pairs(items):
    result = {}
    for key, value in items:
        require(key not in result, 'duplicate_json_key')
        result[key] = value
    return result


def digest(data):
    return hashlib.sha256(data).hexdigest()


def fresh_plan(plan, now):
    try:
        created = datetime.datetime.fromisoformat(plan['createdAt'].replace('Z', '+00:00'))
        require(created.tzinfo is not None, 'invalid_created_at')
        age = now - created.timestamp()
    except (ValueError, TypeError, AttributeError, OverflowError):
        raise CleanupError('invalid_created_at') from None
    require(0 <= age <= 120, 'plan_not_fresh')


def validate_snapshot(value):
    require(isinstance(value, dict) and set(value) == SNAPSHOT_KEYS, 'invalid_candidate_snapshot')
    candidate_path(value['path'])
    for key in SNAPSHOT_KEYS - {'path', 'inventorySha256'}:
        require(type(value[key]) is int and value[key] >= 0, 'invalid_candidate_number')
    require(value['ino'] > 0 and value['mntId'] > 0 and 1 <= value['entryCount'] <= MAX_ENTRIES, 'invalid_candidate_number')
    require(value['newestChangeNs'] >= max(value['mtimeNs'], value['ctimeNs']), 'invalid_candidate_age')
    require(isinstance(value['inventorySha256'], str) and SHA.fullmatch(value['inventorySha256']), 'invalid_inventory_sha256')


def parse_plan(data, now=None):
    require(isinstance(data, bytes) and len(data) <= MAX_PLAN_BYTES, 'plan_size_invalid')
    try:
        plan = json.loads(data.decode('utf-8'), object_pairs_hook=_pairs,
                          parse_constant=lambda _: (_ for _ in ()).throw(CleanupError('invalid_json_constant')))
    except (ValueError, UnicodeError, RecursionError):
        raise CleanupError('invalid_private_plan') from None
    require(isinstance(plan, dict) and set(plan) == {'schemaVersion', 'createdAt', 'referenceAuditSha256', 'allReferencesClear', 'ageMinHours', 'candidates'}, 'invalid_plan_schema')
    require(type(plan['schemaVersion']) is int and plan['schemaVersion'] == 1, 'invalid_plan_schema')
    require(plan['allReferencesClear'] is True and isinstance(plan['referenceAuditSha256'], str)
            and SHA.fullmatch(plan['referenceAuditSha256']) and plan['referenceAuditSha256'] != '0' * 64, 'invalid_reference_evidence')
    require(type(plan['ageMinHours']) is int and 48 <= plan['ageMinHours'] <= 87600, 'invalid_minimum_age')
    require(isinstance(plan['candidates'], list) and 1 <= len(plan['candidates']) <= MAX_TARGETS, 'invalid_candidate_count')
    paths = set()
    for item in plan['candidates']:
        validate_snapshot(item)
        require(item['path'] not in paths, 'duplicate_candidate')
        paths.add(item['path'])
    fresh_plan(plan, time.time() if now is None else now)
    return plan


def _mount_id(fd):
    # st_dev does not distinguish a bind mount of the same filesystem.
    try:
        with open('/proc/self/fdinfo/' + str(fd), 'r', encoding='ascii') as stream:
            contents = stream.read(8193)
        require(len(contents) <= 8192, 'mount_identity_unavailable')
        lines = contents.splitlines()
        matches = [line.split(':', 1)[1].strip() for line in lines if line.startswith('mnt_id:')]
        require(len(matches) == 1 and matches[0].isdigit() and int(matches[0]) > 0, 'mount_identity_unavailable')
        return int(matches[0])
    except (OSError, ValueError, UnicodeError):
        raise CleanupError('mount_identity_unavailable') from None


def _kind(item):
    if stat.S_ISDIR(item.st_mode):
        return 'directory'
    if stat.S_ISREG(item.st_mode):
        require(item.st_nlink == 1, 'hardlink_refused')
        return 'file'
    if stat.S_ISLNK(item.st_mode):
        require(item.st_nlink == 1, 'hardlink_refused')
        return 'symlink'
    raise CleanupError('special_file_refused')


def _metadata(item, mount_id):
    return {'type': _kind(item), 'dev': item.st_dev, 'ino': item.st_ino, 'size': item.st_size,
            'mtimeNs': item.st_mtime_ns, 'ctimeNs': item.st_ctime_ns, 'nlink': item.st_nlink,
            'mode': stat.S_IMODE(item.st_mode), 'uid': item.st_uid, 'gid': item.st_gid, 'mntId': mount_id}


def _same_inode(left, right):
    return (left.st_dev, left.st_ino, stat.S_IFMT(left.st_mode)) == (right.st_dev, right.st_ino, stat.S_IFMT(right.st_mode))


def _mount_guard(item, mount_id, root_dev, root_mount):
    require(item.st_dev == root_dev and mount_id == root_mount, 'mount_boundary_refused')


class LinuxTree:
    def __init__(self, path):
        require(sys.platform.startswith('linux'), 'linux_filesystem_required')
        self.parent_path, self.name = candidate_path(path)
        require(all(getattr(os, key, 0) for key in ('O_NOFOLLOW', 'O_DIRECTORY', 'O_PATH')), 'required_dirfd_support_unavailable')
        self.path, self.parent, self.root = path, None, None
        self.dir_flags = os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | getattr(os, 'O_CLOEXEC', 0)
        self.leaf_flags = os.O_PATH | os.O_NOFOLLOW | getattr(os, 'O_CLOEXEC', 0)
        try:
            self.parent = os.open(self.parent_path, self.dir_flags)
            self.parent_stat, self.parent_mount = os.fstat(self.parent), _mount_id(self.parent)
            require(stat.S_ISDIR(self.parent_stat.st_mode), 'parent_not_directory')
            before = os.stat(self.name, dir_fd=self.parent, follow_symlinks=False)
            require(stat.S_ISDIR(before.st_mode), 'candidate_not_real_directory')
            self.root = os.open(self.name, self.dir_flags, dir_fd=self.parent)
            held = os.fstat(self.root)
            require(_same_inode(before, held), 'candidate_identity_changed')
            self.root_dev, self.root_mount = held.st_dev, _mount_id(self.root)
            _mount_guard(held, self.root_mount, self.parent_stat.st_dev, self.parent_mount)
            self.chain = [(self.parent, self.name, self.root)]
            self.binding(self.chain)
        except Exception:
            self.close()
            raise

    def close(self):
        for key in ('root', 'parent'):
            fd = getattr(self, key, None)
            if fd is not None:
                os.close(fd)
                setattr(self, key, None)

    def __enter__(self):
        return self

    def __exit__(self, *_):
        self.close()

    def binding(self, chain):
        fresh = os.open(self.parent_path, self.dir_flags)
        try:
            require(_same_inode(os.fstat(fresh), self.parent_stat) and _mount_id(fresh) == self.parent_mount, 'parent_identity_changed')
        finally:
            os.close(fresh)
        for parent_fd, name, child_fd in chain:
            current = os.stat(name, dir_fd=parent_fd, follow_symlinks=False)
            held = os.fstat(child_fd)
            require(_same_inode(current, held) and stat.S_ISDIR(held.st_mode), 'directory_identity_changed')
            _mount_guard(held, _mount_id(child_fd), self.root_dev, self.root_mount)

    def node(self, directory_fd, name):
        before = os.stat(name, dir_fd=directory_fd, follow_symlinks=False)
        kind = _kind(before)
        fd = os.open(name, self.dir_flags if kind == 'directory' else self.leaf_flags, dir_fd=directory_fd)
        try:
            after, mount_id = os.fstat(fd), _mount_id(fd)
            require(_metadata(before, mount_id) == _metadata(after, mount_id), 'entry_identity_changed')
            _mount_guard(after, mount_id, self.root_dev, self.root_mount)
            return fd, _metadata(after, mount_id)
        except Exception:
            os.close(fd)
            raise

    def scan(self):
        entries = {}
        def visit(fd, relative, depth, chain):
            require(depth <= MAX_DEPTH and len(entries) < MAX_ENTRIES, 'inventory_limit_exceeded')
            self.binding(chain)
            start = _metadata(os.fstat(fd), _mount_id(fd))
            entries[relative] = start
            names = sorted(os.listdir(fd))
            for name in names:
                require(name not in ('', '.', '..') and '/' not in name and '\x00' not in name, 'invalid_entry_name')
                child, metadata = self.node(fd, name)
                child_relative = name if relative == '.' else relative + '/' + name
                try:
                    if metadata['type'] == 'directory':
                        visit(child, child_relative, depth + 1, chain + [(fd, name, child)])
                    else:
                        require(len(entries) < MAX_ENTRIES, 'inventory_limit_exceeded')
                        entries[child_relative] = metadata
                finally:
                    os.close(child)
            require(names == sorted(os.listdir(fd)) and start == _metadata(os.fstat(fd), _mount_id(fd)), 'tree_changed_during_scan')
        visit(self.root, '.', 0, self.chain)
        self.binding(self.chain)
        serialized = json.dumps([[key, entries[key]] for key in sorted(entries)], sort_keys=True, separators=(',', ':'), ensure_ascii=True).encode('ascii')
        root = entries['.']
        public = {'path': self.path, 'dev': root['dev'], 'ino': root['ino'], 'mtimeNs': root['mtimeNs'], 'ctimeNs': root['ctimeNs'], 'mntId': root['mntId'],
                  'inventorySha256': digest(serialized), 'entryCount': len(entries), 'totalBytes': sum(e['size'] for e in entries.values() if e['type'] != 'directory'),
                  'newestChangeNs': max(max(e['mtimeNs'], e['ctimeNs']) for e in entries.values())}
        return public, entries

    def delete(self, entries, progress):
        children = {key: [] for key, value in entries.items() if value['type'] == 'directory'}
        for key in entries:
            if key != '.':
                parent, _, name = key.rpartition('/')
                children[parent or '.'].append(name)
        def visit(fd, relative, chain):
            self.binding(chain)
            require(_metadata(os.fstat(fd), _mount_id(fd)) == entries[relative], 'directory_changed_before_delete')
            prefix = '' if relative == '.' else relative + '/'
            names = sorted(children[relative])
            require(sorted(os.listdir(fd)) == names, 'tree_changed_during_delete')
            for name in names:
                key = prefix + name
                child, current = self.node(fd, name)
                try:
                    require(current == entries[key], 'entry_changed_during_delete')
                    if current['type'] == 'directory':
                        child_chain = chain + [(fd, name, child)]
                        visit(child, key, child_chain)
                        self.binding(child_chain)
                        require(not os.listdir(child), 'tree_changed_during_delete')
                        os.rmdir(name, dir_fd=fd)
                        progress['entriesRemoved'] += 1
                    else:
                        self.binding(chain)
                        require(_metadata(os.stat(name, dir_fd=fd, follow_symlinks=False), _mount_id(child)) == current, 'entry_changed_during_delete')
                        os.unlink(name, dir_fd=fd)  # A symlink itself is removed; its target is never followed.
                        progress['entriesRemoved'] += 1
                        progress['logicalBytesUnlinked'] += current['size']
                    require(os.fstat(child).st_nlink == 0, 'removed_entry_identity_changed')
                finally:
                    os.close(child)
            require(not os.listdir(fd), 'tree_changed_during_delete')
        visit(self.root, '.', self.chain)
        self.binding(self.chain)
        os.rmdir(self.name, dir_fd=self.parent)
        progress['entriesRemoved'] += 1
        require(os.fstat(self.root).st_nlink == 0, 'removed_entry_identity_changed')
        try:
            os.stat(self.name, dir_fd=self.parent, follow_symlinks=False)
        except FileNotFoundError:
            return
        raise CleanupError('candidate_replaced_after_delete')


def inspect_candidate(path):
    with LinuxTree(path) as tree:
        return tree.scan()[0]


def _old_enough(snapshot, hours, now):
    require(now * 1000000000 - snapshot['newestChangeNs'] >= hours * 3600 * 1000000000, 'candidate_not_old_enough')


def delete_candidate(expected, progress, before_mutation):
    with LinuxTree(expected['path']) as tree:
        observed, entries = tree.scan()
        require(observed == expected, 'candidate_changed_before_delete')
        before_mutation()
        tree.delete(entries, progress)


def operate(data, execute=False, expected_sha256=None, now=time.time,
            inspector=inspect_candidate, deleter=delete_candidate):
    report = {'schemaVersion': 1, 'mode': 'execute' if execute else 'check', 'passed': False, 'atomic': False,
              'partial': False, 'globalReferencesIndependentlyVerified': False, 'units': [], 'entriesRemoved': 0, 'logicalBytesUnlinked': 0}
    try:
        require(type(execute) is bool, 'invalid_execute_flag')
        plan = parse_plan(data, now())
        observed_sha = digest(data)
        require(expected_sha256 is None or isinstance(expected_sha256, str) and SHA.fullmatch(expected_sha256), 'invalid_expected_sha256')
        require(not execute or expected_sha256 is not None, 'expected_sha256_required')
        require(expected_sha256 is None or expected_sha256 == observed_sha, 'plan_sha256_mismatch')
        if execute:
            require(sys.platform.startswith('linux'), 'linux_required_for_execute')
        report.update(planSha256=observed_sha, referenceAuditSha256=plan['referenceAuditSha256'], allReferencesClearDeclared=True)
        # Whole-batch validation completes before the first unlink/rmdir.
        for expected in plan['candidates']:
            observed = inspector(expected['path'])
            require(observed == expected, 'candidate_inventory_mismatch')
            _old_enough(observed, plan['ageMinHours'], now())
            report['units'].append({'path': expected['path'], 'status': 'checked'})
        if execute:
            for expected, row in zip(plan['candidates'], report['units']):
                row.update(status='rechecking', entriesRemoved=0, logicalBytesUnlinked=0)
                try:
                    fresh_plan(plan, now())
                    # Recheck this exact candidate after other targets, immediately before deleting.
                    observed = inspector(expected['path'])
                    require(observed == expected, 'candidate_changed_before_delete')
                    _old_enough(observed, plan['ageMinHours'], now())
                    row['status'] = 'deleting'
                    deleter(expected, row, lambda: fresh_plan(plan, now()))
                    row['status'] = 'deleted'
                except (Exception, KeyboardInterrupt):
                    row['status'] = 'failed_partial' if row['entriesRemoved'] else 'failed_before_mutation'
                    raise
                finally:
                    report['entriesRemoved'] += row['entriesRemoved']
                    report['logicalBytesUnlinked'] += row['logicalBytesUnlinked']
        report['passed'] = True
    except CleanupError as error:
        report['error'] = error.code
    except KeyboardInterrupt:
        report['error'] = 'operator_interrupted'
    except (OSError, ValueError, TypeError, RecursionError):
        report['error'] = 'filesystem_or_private_input_error'
    except Exception:
        report['error'] = 'unexpected_internal_error'
    report['partial'] = not report['passed'] and report['entriesRemoved'] > 0
    return report


class PrivateParser(argparse.ArgumentParser):
    def error(self, _):
        raise CleanupError('invalid_arguments')


def main(argv=None):
    try:
        parser = PrivateParser(description=__doc__)
        parser.add_argument('plan', nargs='?')
        parser.add_argument('--inspect', nargs='+', metavar='PATH')
        parser.add_argument('--execute', action='store_true')
        parser.add_argument('--expected-sha256')
        args = parser.parse_args(argv)
        if args.inspect:
            require(not args.plan and not args.execute and args.expected_sha256 is None, 'invalid_arguments')
            require(1 <= len(args.inspect) <= MAX_TARGETS and len(set(args.inspect)) == len(args.inspect), 'invalid_candidate_count')
            for path in args.inspect:
                candidate_path(path)
            value = {'schemaVersion': 1, 'mode': 'inspect', 'passed': True, 'metadataOnly': True, 'globalReferencesIndependentlyVerified': False,
                     'candidates': [inspect_candidate(path) for path in args.inspect]}
        else:
            require(args.plan is not None, 'invalid_arguments')
            with open(args.plan, 'rb') as stream:
                data = stream.read(MAX_PLAN_BYTES + 1)
            value = operate(data, args.execute, args.expected_sha256)
    except CleanupError as error:
        value = {'schemaVersion': 1, 'passed': False, 'error': error.code, 'entriesRemoved': 0}
    except KeyboardInterrupt:
        value = {'schemaVersion': 1, 'passed': False, 'error': 'operator_interrupted', 'entriesRemoved': 0}
    except (OSError, ValueError, TypeError, RecursionError):
        value = {'schemaVersion': 1, 'passed': False, 'error': 'private_input_or_inspection_error', 'entriesRemoved': 0}
    except Exception:
        value = {'schemaVersion': 1, 'passed': False, 'error': 'unexpected_internal_error', 'entriesRemoved': 0}
    print(json.dumps(value, sort_keys=True))
    return 0 if value['passed'] else 1


if __name__ == '__main__':
    sys.exit(main())
