#!/usr/bin/env python3
"""Reviewed root-only static release publisher. Does not touch game services/saves.

Inputs must have passed the workstation's trusted CMS/attestation verifier.
All uploaded inputs and the verification helper are additionally SHA256 pinned.
Keep this file root-owned before execution; deploy-prepare-plan.py emits a guard.
"""
import argparse
import ctypes
import fcntl
import hashlib
import http.client
import importlib.util
import json
import os
from pathlib import Path
import pwd
import re
import socket
import ssl
import stat
import subprocess
import sys
import tarfile
import time
import uuid

sys.dont_write_bytecode = True
ROOT = Path('/srv/mir2-client-updates')
CADDY = Path('/etc/caddy/Caddyfile')
BACKUPS = Path('/var/backups/mir2-client-updates')
HOST = '165.154.65.136.sslip.io'
ANCHOR = b'relay-hk.obelisk.build, 165.154.65.136.sslip.io {'
MAX_TOTAL = 4 * 1024**3
MAX_ARCHIVE = 5 * 1024**3
MAX_FILES = 200_064
BLOCK = b'''
    # BEGIN MIR2 NATIVE CLIENT UPDATES V1
    @mir2_native_updates path /client-updates /client-updates/*
    handle @mir2_native_updates {
        route {
            @mir2_native_read method GET HEAD
            handle @mir2_native_read {
                route {
                    @mir2_native_discovery path /client-updates/latest.json /client-updates/latest.p7s
                    handle @mir2_native_discovery {
                        uri strip_prefix /client-updates
                        root * /srv/mir2-client-updates/feed-current
                        header Cache-Control "no-store"
                        header X-Content-Type-Options "nosniff"
                        file_server
                    }
                    @mir2_native_payload path /client-updates/releases/*
                    handle @mir2_native_payload {
                        uri strip_prefix /client-updates
                        root * /srv/mir2-client-updates
                        header Cache-Control "public, max-age=31536000, immutable"
                        header X-Content-Type-Options "nosniff"
                        file_server
                    }
                    respond "Not found" 404
                }
            }
            respond "Method not allowed" 405
        }
    }
    # END MIR2 NATIVE CLIENT UPDATES V1
'''


def need(ok, message):
    if not ok:
        raise ValueError(message)


def digest(data):
    return hashlib.sha256(data).hexdigest().upper()


def plain(path, directory=False, root_owned=False):
    path = Path(path)
    info = path.lstat()
    need(not stat.S_ISLNK(info.st_mode) and (stat.S_ISDIR(info.st_mode) if directory else stat.S_ISREG(info.st_mode)), 'linked/special/wrong-type path rejected')
    if not directory:
        need(info.st_nlink == 1, 'hardlinked input rejected')
    if root_owned:
        need(info.st_uid == 0 and not info.st_mode & 0o022, 'root-owned non-shared path required')
    return info


def ancestors(path, root_owned=False):
    path = Path(os.path.abspath(path))
    for item in reversed((path, *path.parents)):
        plain(item, directory=True, root_owned=root_owned)
    return path


def read_pinned(path, expected, limit):
    need(re.fullmatch(r'[0-9A-F]{64}', expected) is not None, 'uppercase expected SHA256 required')
    ancestors(path.parent)
    initial = plain(path)
    need(initial.st_size <= limit, 'input exceeds bound')
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW)
    with os.fdopen(fd, 'rb') as source:
        opened = os.fstat(source.fileno())
        need((initial.st_dev, initial.st_ino) == (opened.st_dev, opened.st_ino) and opened.st_nlink == 1, 'input changed during opening')
        data = source.read(limit + 1)
    need(len(data) <= limit and digest(data) == expected, 'input SHA256 mismatch')
    return data


def copy_archive(source, target, expected):
    ancestors(source.parent)
    info = plain(source)
    need(info.st_size <= MAX_ARCHIVE, 'archive exceeds bound')
    fd = os.open(source, os.O_RDONLY | os.O_NOFOLLOW)
    h, total = hashlib.sha256(), 0
    with os.fdopen(fd, 'rb') as incoming, target.open('xb') as saved:
        opened = os.fstat(incoming.fileno())
        need((info.st_dev, info.st_ino) == (opened.st_dev, opened.st_ino) and opened.st_nlink == 1, 'archive changed during opening')
        for chunk in iter(lambda: incoming.read(1024**2), b''):
            total += len(chunk)
            need(total <= MAX_ARCHIVE, 'archive grew beyond bound')
            h.update(chunk)
            saved.write(chunk)
        saved.flush()
        os.fsync(saved.fileno())
    need(h.hexdigest().upper() == expected, 'archive SHA256 mismatch')
    return total


def inject_route(old):
    if b'# BEGIN MIR2 NATIVE CLIENT UPDATES V1' in old:
        need(old.count(BLOCK) == 1 and old.count(b'# BEGIN MIR2 NATIVE CLIENT UPDATES V1') == 1, 'existing update route differs from reviewed fragment')
        return old
    need(old.count(ANCHOR) == 1 and b'/client-updates' not in old, 'unexpected Caddy site/route; manual review required')
    position = old.index(ANCHOR) + len(ANCHOR)
    new = old[:position] + BLOCK + old[position:]
    need(new.replace(BLOCK, b'', 1) == old, 'existing Caddy content changed')
    return new


def health_idle():
    import urllib.request
    result = []
    for port in [7110, 7210]:
        with urllib.request.urlopen(f'http://127.0.0.1:{port}/health', timeout=15) as response:
            health = json.load(response)
        need(health.get('ok') is True and health.get('http') == 'ready' and health.get('ws') == 'ready', 'existing Gateway health is not ready')
        capacity = health.get('capacity', {})
        need(all(type(capacity.get(k)) is int and capacity[k] == 0 for k in ['currentWsConnections', 'currentActiveSessions', 'currentReconnectLeases']), 'Gateway is in use; Caddy reload postponed')
        result.append({'port': port, 'revision': health.get('revision'), 'capacity': capacity})
    return result


def filter_validation_environment(config, live_bytes, inherited):
    """Pass through only the existing config's service references; never log values."""
    names = set(re.findall(r'\{\$([A-Za-z_][A-Za-z_0-9]*)[^}]*\}', config.decode('utf-8')))
    needed = {name.encode('ascii'): name for name in names}
    selected = {}
    for entry in live_bytes.split(b'\0'):
        key, separator, value = entry.partition(b'=')
        if separator and key in needed:
            need(key not in selected and bool(value), 'Required Caddy environment value is empty/ambiguous')
            try:
                selected[key] = value.decode('utf-8')
            except UnicodeError:
                raise ValueError('Required Caddy environment value has invalid encoding') from None
    need(set(selected) == set(needed), 'Required Caddy service environment value is absent')
    result = inherited.copy()
    result.update((needed[key], value) for key, value in selected.items())
    return result


def caddy_validation_environment(config):
    pid = int(subprocess.check_output(['systemctl', 'show', 'caddy', '-p', 'MainPID', '--value']))
    need(pid > 1 and Path(f'/proc/{pid}/comm').read_text().strip() == 'caddy', 'Current Caddy process is unavailable')
    with Path(f'/proc/{pid}/environ').open('rb') as source:
        live_bytes = source.read(1_048_577)
    need(len(live_bytes) <= 1_048_576, 'Caddy process environment exceeds bound')
    return filter_validation_environment(config, live_bytes, os.environ)


def command_checked(command, log, environment=None):
    result = subprocess.run(command, capture_output=True, env=environment)
    with log.open('xb') as saved:
        saved.write(result.stdout + result.stderr)
    os.chmod(log, 0o600)
    need(result.returncode == 0, 'Caddy operation failed; private diagnostics retained')


def extract_checked(archive_path, receipt, feed, stage, helper):
    need(receipt.get('schema') == 'mir2.windows.update-archive-receipt.v1' and receipt.get('tarFormat') == 'USTAR', 'unsupported archive receipt')
    components = [feed['game'], feed['engine']]
    prefixes = [c['directory'] for c in components]
    need(len(set(prefixes)) == 2, 'release directories must differ')
    for p in prefixes:
        helper.relative_path(p)
        need(p.startswith('releases/') and p.count('/') == 1, 'flat immutable release directory required')
    entries, total = {}, 0
    need(isinstance(receipt.get('files'), list) and len(receipt['files']) <= MAX_FILES, 'receipt file count outside bound')
    for item in receipt['files']:
        need(isinstance(item, dict) and set(item) == {'path', 'size', 'sha256'}, 'invalid receipt entry')
        path = helper.relative_path(item['path'])
        need(any(path.startswith(p + '/') for p in prefixes) and path.lower() not in entries, 'unexpected/case-colliding release path')
        size = helper.uint(item['size'], helper.MAX_FILE, 'receipt file size')
        helper.hash_value(item['sha256'], 'receipt file')
        entries[path.lower()] = item
        total += size
    need(type(receipt.get('fileCount')) is int and receipt['fileCount'] == len(entries) and type(receipt.get('uncompressedBytes')) is int and receipt['uncompressedBytes'] == total and total <= MAX_TOTAL, 'receipt count/total mismatch')
    seen, names, members = set(), {}, 0
    with tarfile.open(archive_path, 'r|gz') as packed:
        for member in packed:
            members += 1
            need(members <= 400_128 and not member.pax_headers and not packed.pax_headers, 'archive entry/PAX bound rejected')
            name = member.name
            helper.relative_path(name)
            need(member.isfile() or member.isdir(), 'archive links/special files rejected')
            need(member.uid == member.gid == 0 and member.uname == member.gname == '', 'archive ownership extensions rejected')
            need(name.lower() not in names, 'duplicate/case-colliding archive entry')
            names[name.lower()] = 'directory' if member.isdir() else 'file'
            need(name == 'releases' or any(name == p or name.startswith(p + '/') for p in prefixes), 'archive escaped release directories')
            for parent in Path(name).parents:
                if str(parent) != '.':
                    need(names.get(parent.as_posix().lower()) != 'file', 'archive file/directory collision')
            target = stage / name
            need(target.is_relative_to(stage), 'archive target escaped stage')
            if member.isdir():
                target.mkdir(mode=0o700, parents=True, exist_ok=True)
                plain(target, directory=True, root_owned=True)
                continue
            expected = entries.get(name.lower())
            need(expected is not None and expected['path'] == name and member.size == expected['size'], 'archive member differs from receipt')
            target.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
            incoming = packed.extractfile(member)
            h, size = hashlib.sha256(), 0
            with target.open('xb') as saved:
                for chunk in iter(lambda: incoming.read(1024**2), b''):
                    size += len(chunk)
                    need(size <= expected['size'], 'archive member exceeds declared size')
                    h.update(chunk)
                    saved.write(chunk)
            need(size == expected['size'] and h.hexdigest().upper() == expected['sha256'], 'extracted member SHA256 mismatch')
            seen.add(name.lower())
    need(seen == set(entries), 'archive omitted receipt files')
    files, _, _ = helper.scan({p: str(stage / p) for p in prefixes})
    game = helper.validate_candidate(files[prefixes[0]])
    engine = helper.validate_engine(files[prefixes[1]])
    need(game['identity'] == feed['game']['identity'] and engine['identity'] == feed['engine']['identity'], 'feed component identity mismatch')
    for component in components:
        for item in component['metadata']:
            actual = files[component['directory']].get(item['path'])
            need(actual is not None and actual.size == item['size'] and actual.sha256 == item['sha256'], 'feed metadata SHA256 binding mismatch')
    return files


def promote_immutable(stage, component, files, helper):
    target, source = ROOT / component, stage / component
    if target.exists():
        plain(target, directory=True, root_owned=True)
        for folder, dirs, names in os.walk(target, followlinks=False):
            plain(Path(folder), directory=True, root_owned=True)
            for name in names:
                plain(Path(folder) / name, root_owned=True)
        actual, _, _ = helper.scan({component: str(target)})
        wanted = {p: (e.size, e.sha256) for p, e in files.items()}
        found = {p: (e.size, e.sha256) for p, e in actual[component].items()}
        need(found == wanted, 'existing immutable release differs; refusing overwrite')
        return 'verified-existing'
    for folder, dirs, names in os.walk(source, followlinks=False):
        plain(Path(folder), directory=True, root_owned=True)
        os.chmod(folder, 0o755)
        for name in names:
            file = Path(folder) / name
            plain(file, root_owned=True)
            os.chmod(file, 0o644)
    need(not os.path.lexists(target), 'release destination appeared concurrently')
    os.rename(source, target)
    return 'promoted'


def exchange_directories(a, b):
    ancestors(a.parent, root_owned=True)
    ancestors(b.parent, root_owned=True)
    plain(a, directory=True, root_owned=True)
    plain(b, directory=True, root_owned=True)
    libc = ctypes.CDLL(None, use_errno=True)
    rename = libc.renameat2
    rename.argtypes = [ctypes.c_int, ctypes.c_char_p, ctypes.c_int, ctypes.c_char_p, ctypes.c_uint]
    rename.restype = ctypes.c_int
    need(rename(-100, os.fsencode(a), -100, os.fsencode(b), 2) == 0, 'atomic feed directory exchange failed')


class LocalTLS(http.client.HTTPSConnection):
    def connect(self):
        self.sock = self._context.wrap_socket(socket.create_connection(('127.0.0.1', self.port), self.timeout), server_hostname=self.host)


def verify_serving(feed_bytes, signature, feed):
    for path, expected, cache in [('latest.json', feed_bytes, 'no-store'), ('latest.p7s', signature, 'no-store')]:
        connection = LocalTLS(HOST, timeout=30, context=ssl.create_default_context())
        try:
            connection.request('GET', '/client-updates/' + path, headers={'Host': HOST})
            response = connection.getresponse()
            need(response.status == 200 and response.read(65_537) == expected and cache in response.getheader('Cache-Control', ''), 'TLS discovery/cache check failed')
        finally:
            connection.close()
    for component in [feed['game'], feed['engine']]:
        for entry in component['metadata']:
            connection = LocalTLS(HOST, timeout=30, context=ssl.create_default_context())
            try:
                connection.request('HEAD', '/client-updates/' + component['directory'] + '/' + entry['path'], headers={'Host': HOST})
                response = connection.getresponse()
                need(response.status == 200 and response.getheader('Content-Length') == str(entry['size']) and 'immutable' in response.getheader('Cache-Control', ''), 'TLS immutable metadata check failed')
            finally:
                connection.close()
    checks = [('POST', '/client-updates/latest.json', 405),
              ('GET', '/client-updates/', 404),
              ('GET', '/client-updates/releases/', 404),
              ('GET', '/client-updates/feed-current/latest.json', 404)]
    checks.extend(('GET', '/client-updates/' + component['directory'] + '/', 404) for component in [feed['game'], feed['engine']])
    for method, path, expected_status in checks:
        connection = LocalTLS(HOST, timeout=30, context=ssl.create_default_context())
        try:
            connection.request(method, path, headers={'Host': HOST})
            response = connection.getresponse()
            need(response.status == expected_status, 'Static route method/directory isolation check failed')
            response.read(65_537)
        finally:
            connection.close()


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--staging', required=True, type=Path)
    for name in ['archive', 'receipt', 'latest', 'signature', 'archive-helper', 'expected-caddy']:
        p.add_argument('--' + name + '-sha256', required=True)
    p.add_argument('--archive-helper', required=True, type=Path)
    p.add_argument('--sequence', type=int, required=True)
    p.add_argument('--externally-verified-signatures', action='store_true', required=True)
    a = p.parse_args()
    need(os.geteuid() == 0, 'root execution required after review')
    need(a.sequence > 0, 'positive feed sequence required')
    ancestors(CADDY.parent, root_owned=True)
    old = read_pinned(CADDY, a.expected_caddy_sha256, 1_048_576)
    new = inject_route(old)
    health_before = health_idle()
    upload = ancestors(a.staging)
    need(upload.parent == Path('/home/ubuntu') and upload.name.startswith('mir2-native-upload-') and plain(upload, directory=True).st_uid == pwd.getpwnam('ubuntu').pw_uid and not upload.stat().st_mode & 0o077, 'private authorized upload directory required')
    helper_bytes = read_pinned(a.archive_helper, a.archive_helper_sha256, 65_536)
    plain(a.archive_helper, root_owned=True)
    ancestors(a.archive_helper.parent, root_owned=True)
    specification = importlib.util.spec_from_file_location('mir2_verified_archive', a.archive_helper)
    helper = importlib.util.module_from_spec(specification)
    sys.modules[specification.name] = helper
    specification.loader.exec_module(helper)
    receipt_bytes = read_pinned(upload / 'release.tar.gz.receipt.json', a.receipt_sha256, 64 * 1024**2)
    receipt = helper.json_bytes(receipt_bytes, 'archive receipt', 64 * 1024**2)
    feed_bytes = read_pinned(upload / 'latest.json', a.latest_sha256, 32_768)
    signature = read_pinned(upload / 'latest.p7s', a.signature_sha256, 32_768)
    need(len(signature) > 0, 'discovery signature must not be empty')
    feed = helper.json_bytes(feed_bytes, 'feed', 32_768)
    need(feed.get('schema') == 'mir2.windows.update-feed.v1' and feed.get('channel') == 'invited' and feed.get('platform') == 'windows-x64' and type(feed.get('sequence')) is int and feed['sequence'] == a.sequence, 'feed identity/sequence mismatch')
    need(feed.get('protocol') == 'crystal-mir2-v1' and feed.get('content') == 'mir2.windows.package-manifest.v4', 'feed protocol/content mismatch')
    helper.uint(feed.get('minBootstrap'), 1, 'feed minimum bootstrap')
    for component_name, expected_metadata in [('game', set(helper.META)), ('engine', {'ENGINE.json', 'ENGINE.p7s'})]:
        component = feed.get(component_name)
        need(isinstance(component, dict) and set(component) == {'directory', 'identity', 'metadata'} and isinstance(component['metadata'], list), 'invalid feed component')
        names = []
        for item in component['metadata']:
            need(isinstance(item, dict) and set(item) == {'path', 'size', 'sha256'}, 'invalid feed metadata entry')
            helper.relative_path(item['path'])
            helper.uint(item['size'], helper.MAX_META, 'feed metadata size')
            helper.hash_value(item['sha256'], 'feed metadata')
            names.append(item['path'])
        need(len(names) == len(set(names)) and set(names) == expected_metadata, 'feed metadata set mismatch')
    now = int(time.time())
    need(type(feed.get('createdUnix')) is int and type(feed.get('expiresUnix')) is int and 0 < feed['createdUnix'] <= now + 300 and now < feed['expiresUnix'] <= feed['createdUnix'] + 31 * 86400, 'expired/future feed rejected')
    need(receipt.get('archiveSha256') == a.archive_sha256, 'archive receipt pin mismatch')
    ancestors(Path('/srv'), root_owned=True)
    ancestors(Path('/var/backups'), root_owned=True)
    BACKUPS.mkdir(mode=0o700, exist_ok=True)
    plain(BACKUPS, directory=True, root_owned=True)
    lock_path = BACKUPS / '.publisher.lock'
    fd = os.open(lock_path, os.O_CREAT | os.O_RDWR | os.O_NOFOLLOW, 0o600)
    with os.fdopen(fd, 'wb') as lock:
        fcntl.flock(lock.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)
        stamp = time.strftime('%Y%m%dT%H%M%SZ', time.gmtime()) + '-' + uuid.uuid4().hex[:10]
        backup = BACKUPS / stamp
        backup.mkdir(mode=0o700)
        (backup / 'Caddyfile.before').write_bytes(old)
        (backup / 'feed.json.approved').write_bytes(feed_bytes)
        (backup / 'feed.p7s.approved').write_bytes(signature)
        stage = Path('/srv') / ('.mir2-client-update-stage-' + stamp)
        stage.mkdir(mode=0o700)
        owned_archive = stage / 'approved-release.tar.gz'
        archive_size = copy_archive(upload / 'release.tar.gz', owned_archive, a.archive_sha256)
        need(receipt.get('archiveSizeBytes') == archive_size, 'archive size/receipt mismatch')
        files = extract_checked(owned_archive, receipt, feed, stage, helper)
        ROOT.mkdir(mode=0o755, exist_ok=True)
        plain(ROOT, directory=True, root_owned=True)
        (ROOT / 'releases').mkdir(mode=0o755, exist_ok=True)
        plain(ROOT / 'releases', directory=True, root_owned=True)
        promotions = {c['directory']: promote_immutable(stage, c['directory'], files[c['directory']], helper) for c in [feed['game'], feed['engine']]}
        feed_stage = stage / 'feed-prepared'
        feed_stage.mkdir(mode=0o755)
        for name, data in [('latest.json', feed_bytes), ('latest.p7s', signature)]:
            with (feed_stage / name).open('xb') as saved:
                saved.write(data)
                saved.flush()
                os.fsync(saved.fileno())
            os.chmod(feed_stage / name, 0o644)
        replacement = CADDY.parent / ('.mir2-client-update-' + stamp + '.Caddyfile')
        with replacement.open('xb') as saved:
            saved.write(new)
            saved.flush()
            os.fsync(saved.fileno())
        os.chmod(replacement, stat.S_IMODE(CADDY.stat().st_mode))
        validation_environment = caddy_validation_environment(old)
        command_checked(['caddy', 'validate', '--config', str(replacement), '--adapter', 'caddyfile'], backup / 'validate.log', validation_environment)
        current = ROOT / 'feed-current'
        old_feed_present = current.exists()
        if old_feed_present:
            plain(current, directory=True, root_owned=True)
            previous = helper.json_bytes((current / 'latest.json').read_bytes(), 'previous feed', 32_768)
            need(type(previous.get('sequence')) is int and (previous['sequence'] < a.sequence or previous['sequence'] == a.sequence and (current / 'latest.json').read_bytes() == feed_bytes and (current / 'latest.p7s').read_bytes() == signature), 'server feed downgrade/reused sequence rejected')
        need(read_pinned(CADDY, a.expected_caddy_sha256, 1_048_576) == old, 'Caddy config changed during staging')
        health_idle()
        caddy_changed = published = False
        try:
            if new != old:
                os.replace(replacement, CADDY)
                caddy_changed = True
                command_checked(['systemctl', 'reload', 'caddy'], backup / 'reload.log')
            health_idle()
            if old_feed_present:
                exchange_directories(feed_stage, current)
            else:
                need(not os.path.lexists(current), 'feed target appeared concurrently')
                os.rename(feed_stage, current)
            published = True
            verify_serving(feed_bytes, signature, feed)
            health_after = health_idle()
        except BaseException:
            if published:
                if old_feed_present:
                    exchange_directories(feed_stage, current)
                else:
                    os.rename(current, feed_stage)
            if caddy_changed:
                restore = CADDY.parent / ('.mir2-client-restore-' + stamp + '.Caddyfile')
                with restore.open('xb') as saved:
                    saved.write(old)
                    saved.flush()
                    os.fsync(saved.fileno())
                os.chmod(restore, stat.S_IMODE(CADDY.stat().st_mode))
                command_checked(['caddy', 'validate', '--config', str(restore), '--adapter', 'caddyfile'], backup / 'restore-validate.log', validation_environment)
                os.replace(restore, CADDY)
                command_checked(['systemctl', 'reload', 'caddy'], backup / 'restore-reload.log')
            raise
        result = {'schema': 'mir2.static-update-deployment.v1', 'sequence': a.sequence,
                  'feedSha256': a.latest_sha256, 'signatureSha256': a.signature_sha256,
                  'archiveSha256': a.archive_sha256, 'caddyBeforeSha256': digest(old), 'caddyAfterSha256': digest(new),
                  'signatureValidation': 'externally-verified-before-SHA256-pinned-upload',
                  'immutableReleases': promotions, 'healthBefore': health_before, 'healthAfter': health_after,
                  'backupDirectory': str(backup), 'retainedStageDirectory': str(stage),
                  'latestUrl': 'https://' + HOST + '/client-updates/latest.json'}
        (backup / 'DEPLOYMENT.json').write_text(json.dumps(result, indent=2) + '\n')
        print(json.dumps(result))


if __name__ == '__main__':
    try:
        main()
    except BaseException as exc:
        print('Deployment stopped: ' + str(exc), file=sys.stderr)
        raise SystemExit(1)
