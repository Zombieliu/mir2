"""Append four exact R23 resources. Never replace a published file or promote a feed."""
import argparse
import fcntl
import hashlib
import json
import os
from pathlib import Path
import pwd
import stat
import subprocess
import uuid
from datetime import datetime, timezone

ROOT = Path('/srv/mir2-client-updates')
RELEASE = ROOT / 'releases/game-WN-CANDIDATE-20261009-invited-23'
STAGE = Path('/home/ubuntu/mir2-r23-static-repair-20261010-0d7f9b4068ba4b388e69111d3786d2c8')
LOCK = Path('/var/backups/mir2-client-updates/.publisher.lock')
FILES = [
    ('mir2-assets/generated/native-map-keyed/coverage-audit.json', 723520, '014c18e1e49666877ebcda9e21935c22b46d8950aa2bd1b35a67824946cae42b'),
    ('mir2-assets/generated/native-map-keyed/resource-input-provenance.json', 32275, 'e8ca68605b5c27e925476cae6e055cceb9552157075d6e45b8aa9d4cb76f05a8'),
    ('mir2-assets/original-ui/Prguse/1002.png', 15671, '72f73b42bc7b95a91144fe0f135ede2ad343fbfed488c75095f98ed427caf497'),
    ('mir2-assets/original-ui/Title/18.png', 697, '3b8a6097b20b32fc1bb8d371efdefffe564af7bf1c75bc098bfd10be097c02ad'),
]
PINS = [
    (RELEASE / 'PACKAGE-MANIFEST.json', 31538315, '2012c0163c8ec418ed09ed9d3953337db1a246bb1b2d3191abe6b0ba485eb03e'),
    (RELEASE / 'VERSION.json', 1221, 'd8c2e7d380c4b37e90695c825e89e1c75ecf6738ce04e67780b4439a002957ee'),
    (RELEASE / 'BUILD-ATTESTATION.json', 2416, '62254955d4376366dfdbe2976c93bcaf4a688cca8e6b283ac1ed61b5ab85ff90'),
    (RELEASE / 'README-START.txt', 49328, '5bc26223994015306280ee17952cafe2d30d61b1649f8a0ff7f1de3cc40b8bbd'),
    (ROOT / 'feed-current/latest.json', 1159, '2d87c59edb2aff2b55237dad1fcc418eac6ba03b6514720982ac16c1215f20d2'),
    (ROOT / 'feed-current/latest.p7s', None, '1f94346017807b0d6a2c24a3f4cdd3226461cc9618ed6dc0daa43460e7c0f8f4'),
    (Path('/etc/caddy/Caddyfile'), 4717, 'df63f51c991fe1f74c3c9415dd19f794e19ec00a9c46a1021cb9e02dfc9d161d'),
]

def protected_directory(path):
    s = path.lstat()
    if not stat.S_ISDIR(s.st_mode) or s.st_uid != 0 or s.st_mode & 0o022:
        raise RuntimeError('Unprotected directory: ' + str(path))

def protected_parents(path):
    for p in reversed(path.parents):
        protected_directory(p)

def read_regular(path, limit, owner=0, dir_fd=None):
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK, dir_fd=dir_fd)
    try:
        s = os.fstat(fd)
        if not stat.S_ISREG(s.st_mode) or s.st_uid != owner or s.st_mode & 0o022 or s.st_size > limit:
            raise RuntimeError('Not a protected bounded ordinary file: ' + str(path))
        with os.fdopen(fd, 'rb', closefd=False) as f:
            b = f.read(limit + 1)
        if len(b) != s.st_size or len(b) > limit:
            raise RuntimeError('File changed while reading: ' + str(path))
        return b
    finally:
        os.close(fd)

def checked_bytes(path, length, sha, owner=0, dir_fd=None):
    b = read_regular(path, length if length is not None else 1000000, owner, dir_fd)
    if (length is not None and len(b) != length) or hashlib.sha256(b).hexdigest() != sha:
        raise RuntimeError('Published/input byte pin changed: ' + str(path))
    return b

def immutable_pins():
    result = []
    for p, length, sha in PINS:
        protected_parents(p)
        b = checked_bytes(p, length, sha)
        result.append({'path': str(p), 'size': len(b), 'sha256': sha})
    manifest = json.loads(checked_bytes(*PINS[0]))
    for rel, length, sha in FILES:
        matches = [f for f in manifest['files'] if f['path'] == rel]
        if len(matches) != 1 or matches[0]['size'] != length or matches[0]['sha256'].lower() != sha:
            raise RuntimeError('Resource differs from pinned package manifest: ' + rel)
    return result

def services():
    return subprocess.run(['systemctl', 'show', 'mir2-playtest.service', 'caddy.service',
        '--property=Id,ActiveState,SubState,MainPID,ExecMainStartTimestampMonotonic'],
        check=True, capture_output=True, text=True, timeout=15).stdout

def directory_sync(path):
    fd = os.open(path, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
    try:
        os.fsync(fd)
    finally:
        os.close(fd)

parser = argparse.ArgumentParser()
parser.add_argument('--apply', action='store_true')
args = parser.parse_args()
if os.geteuid() != 0:
    raise RuntimeError('Root operator required')
protected_parents(LOCK)
lock_fd = os.open(LOCK, os.O_RDONLY | os.O_NOFOLLOW)
lock_stat = os.fstat(lock_fd)
if not stat.S_ISREG(lock_stat.st_mode) or lock_stat.st_uid != 0 or lock_stat.st_mode & 0o077:
    raise RuntimeError('Publisher lock is not protected')
fcntl.flock(lock_fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
try:
    before = immutable_pins()
    service_before = services()
    for p in [Path('/'), Path('/srv'), ROOT, ROOT / 'releases', RELEASE]:
        protected_directory(p)
    ubuntu_uid = pwd.getpwnam('ubuntu').pw_uid
    stage_stat = STAGE.lstat()
    if not stat.S_ISDIR(stage_stat.st_mode) or stage_stat.st_uid != ubuntu_uid or stage_stat.st_mode & 0o077:
        raise RuntimeError('Unexpected upload stage')
    stage_fd = os.open(STAGE, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
    try:
        payloads = [checked_bytes('asset-' + str(i) + '.bin', length, sha, ubuntu_uid, stage_fd)
                    for i, (_, length, sha) in enumerate(FILES)]
    finally:
        os.close(stage_fd)
    actions = []
    # Validate every target before creating anything; tolerate only byte-exact retry.
    for rel, length, sha in FILES:
        target = RELEASE / rel
        if not target.is_relative_to(RELEASE) or '..' in target.parts:
            raise RuntimeError('Target escapes fixed release')
        for p in reversed(target.parents):
            try:
                p.lstat()
            except FileNotFoundError:
                continue
            protected_directory(p)
        try:
            target.lstat()
        except FileNotFoundError:
            action = 'append_missing'
        else:
            checked_bytes(target, length, sha)
            action = 'already_exact'
        actions.append({'path': str(target), 'size': length, 'sha256': sha, 'action': action})
    report = {'schema': 'mir2.r23-missing-assets.append.v1',
              'utc': datetime.now(timezone.utc).isoformat(), 'apply': args.apply,
              'assets': actions, 'protectedPinsBefore': before, 'servicesBefore': service_before,
              'noFeedPromotion': True, 'noServiceRestartCalled': True}
    if args.apply:
        for i, entry in enumerate(actions):
            if entry['action'] == 'already_exact':
                continue
            target = Path(entry['path'])
            for p in reversed(target.parents):
                try:
                    p.lstat()
                except FileNotFoundError:
                    p.mkdir(mode=0o755)
                    directory_sync(p.parent)
                protected_directory(p)
            temp = target.parent / ('.repair-' + uuid.uuid4().hex + '.part')
            fd = os.open(temp, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
            try:
                with os.fdopen(fd, 'wb', closefd=False) as f:
                    f.write(payloads[i])
                    f.flush()
                os.fchmod(fd, 0o444)
                os.fsync(fd)
                # Hard-link publication is atomic and cannot replace any target.
                os.link(temp, target, follow_symlinks=False)
                directory_sync(target.parent)
            finally:
                try:
                    os.close(fd)
                finally:
                    temp.unlink()
                    directory_sync(target.parent)
            checked_bytes(target, entry['size'], entry['sha256'])
            entry['action'] = 'appended_verified'
    report['protectedPinsAfter'] = immutable_pins()
    report['servicesAfter'] = services()
    report['serviceProcessesUnchanged'] = report['servicesBefore'] == report['servicesAfter']
    report['protectedBytesUnchanged'] = report['protectedPinsBefore'] == report['protectedPinsAfter']
    print(json.dumps(report, indent=2))
finally:
    os.close(lock_fd)
