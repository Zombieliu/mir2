#!/usr/bin/env python3
"""Emit reviewed local SSH upload/apply plans; never connect or mutate a server."""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shlex
import sys
import uuid

sys.dont_write_bytecode = True
DEFAULT_HELPER = Path(__file__).with_name('archive-update-release.py')
BOOTSTRAP = '''import hashlib,os,pathlib,stat,sys
base=pathlib.Path('/var/lib/mir2-client-publisher')
def need(ok):
 if not ok:raise SystemExit('Privileged script identity/path guard failed')
def checked_parent(p):
 for x in reversed((p,*p.parents)):
  s=x.lstat();need(stat.S_ISDIR(s.st_mode) and not stat.S_ISLNK(s.st_mode))
def copy_code(source,expected):
 source=pathlib.Path(source);checked_parent(source.parent);s=source.lstat();need(stat.S_ISREG(s.st_mode) and s.st_nlink==1 and s.st_size<=65536)
 fd=os.open(source,os.O_RDONLY|os.O_NOFOLLOW)
 with os.fdopen(fd,'rb') as f:
  t=os.fstat(f.fileno());need((s.st_dev,s.st_ino)==(t.st_dev,t.st_ino) and t.st_nlink==1);data=f.read(65537)
 need(len(data)<=65536 and hashlib.sha256(data).hexdigest().upper()==expected)
 dest=base/(expected+'.py')
 if os.path.lexists(dest):
  t=dest.lstat();need(stat.S_ISREG(t.st_mode) and t.st_nlink==1 and t.st_uid==0 and not t.st_mode&0o022 and dest.read_bytes()==data)
 else:
  fd=os.open(dest,os.O_WRONLY|os.O_CREAT|os.O_EXCL|os.O_NOFOLLOW,0o500)
  with os.fdopen(fd,'wb') as f:f.write(data);f.flush();os.fsync(f.fileno())
 return str(dest)
need(os.geteuid()==0)
checked_parent(base.parent)
if not base.exists():base.mkdir(mode=0o700)
checked_parent(base)
s=base.stat();need(s.st_uid==0 and not s.st_mode&0o077)
script=copy_code(sys.argv[1],sys.argv[2]);helper=copy_code(sys.argv[3],sys.argv[4])
os.execv(sys.executable,[sys.executable,script,'--archive-helper',helper,'--archive-helper-sha256',sys.argv[4],*sys.argv[5:]])
'''


def sha_file(path, helper):
    helper.absolute_directory(str(path.parent))
    helper._plain(path, directory=False)
    with path.open('rb') as source:
        return hashlib.file_digest(source, 'sha256').hexdigest().upper()


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--archive', required=True, type=Path)
    p.add_argument('--feed-root', required=True, type=Path)
    p.add_argument('--output-root', required=True, type=Path)
    p.add_argument('--sequence', type=int, required=True)
    p.add_argument('--expected-caddy-sha256', required=True)
    p.add_argument('--archive-helper', type=Path, default=DEFAULT_HELPER)
    p.add_argument('--externally-verified-signatures', action='store_true', required=True)
    a = p.parse_args()
    helper_path = a.archive_helper.absolute()
    specification = importlib.util.spec_from_file_location('mir2_plan_archive_helper', helper_path)
    helper = importlib.util.module_from_spec(specification)
    sys.modules[specification.name] = helper
    specification.loader.exec_module(helper)
    helper.hash_value(a.expected_caddy_sha256.upper(), 'Caddy baseline')
    helper.require(a.sequence > 0, 'positive sequence required')
    inputs = {'release.tar.gz': a.archive.absolute(), 'release.tar.gz.receipt.json': Path(str(a.archive.absolute()) + '.receipt.json'),
              'latest.json': (a.feed_root / 'latest.json').absolute(), 'latest.p7s': (a.feed_root / 'latest.p7s').absolute(),
              'deploy-server-release.py': Path(__file__).with_name('deploy-server-release.py').absolute(), 'archive-update-release.py': helper_path}
    hashes = {name: sha_file(path, helper) for name, path in inputs.items()}
    receipt = helper.json_bytes(inputs['release.tar.gz.receipt.json'].read_bytes(), 'receipt', 64 * 1024**2)
    feed = helper.json_bytes(inputs['latest.json'].read_bytes(), 'feed', 32768)
    helper.require(receipt.get('archiveSha256') == hashes['release.tar.gz'] and receipt.get('archiveSizeBytes') == inputs['release.tar.gz'].stat().st_size, 'archive receipt differs from actual archive')
    helper.require(feed.get('sequence') == a.sequence and feed.get('schema') == 'mir2.windows.update-feed.v1', 'feed/sequence mismatch')
    helper.require(a.output_root.is_absolute(), 'absolute local plan output required')
    output = Path(os.path.abspath(a.output_root))
    helper.absolute_directory(str(output.parent))
    helper.require(not os.path.lexists(output), 'fresh local plan output required')
    output.mkdir()
    stage = '/home/ubuntu/mir2-native-upload-' + uuid.uuid4().hex
    quoted_stage = shlex.quote(stage)
    create = 'set -eu\numask 077\ntest ! -e ' + quoted_stage + '\nmkdir -m 700 -- ' + quoted_stage + '\nstat -c "%a %U:%G %n" -- ' + quoted_stage + '\n'
    upload = [[str(path), stage + '/' + name] for name, path in inputs.items()]
    server_args = ['--staging', stage, '--archive-sha256', hashes['release.tar.gz'], '--receipt-sha256', hashes['release.tar.gz.receipt.json'],
                   '--latest-sha256', hashes['latest.json'], '--signature-sha256', hashes['latest.p7s'],
                   '--sequence', str(a.sequence), '--expected-caddy-sha256', a.expected_caddy_sha256.upper(), '--externally-verified-signatures']
    command = ['sudo', '-n', 'python3', '-c', BOOTSTRAP, stage + '/deploy-server-release.py', hashes['deploy-server-release.py'],
               stage + '/archive-update-release.py', hashes['archive-update-release.py'], *server_args]
    apply = 'set -eu\nexec ' + ' '.join(map(shlex.quote, command)) + '\n'
    preflight = '''set -eu
sudo -n sha256sum /etc/caddy/Caddyfile
df -h /srv
for port in 7110 7210; do
 curl -fsS --max-time 15 http://127.0.0.1:$port/health | python3 -c 'import json,sys; h=json.load(sys.stdin); print(json.dumps({k:h.get(k) for k in ["ok","http","ws","revision","capacity"]}))'
 printf '\n'
done
'''
    for name, text in [('deploy-create-stage.sh', create), ('deploy-apply.sh', apply), ('deploy-preflight.sh', preflight)]:
        (output / name).write_text(text, encoding='utf-8', newline='\n')
    (output / 'deploy-upload.json').write_text(json.dumps(upload, indent=2), encoding='utf-8')
    result = {'schema': 'mir2.static-update-upload-plan.v1', 'serverMutated': False, 'sequence': a.sequence,
              'remoteStaging': stage, 'inputs': {name: {'path': str(path), 'sha256': hashes[name]} for name, path in inputs.items()},
              'expectedCaddySha256': a.expected_caddy_sha256.upper(), 'externalCmsGateRequired': True,
              'commands': ['remote.py --command-file deploy-preflight.sh', 'remote.py --command-file deploy-create-stage.sh',
                           'remote.py --upload-batch deploy-upload.json', 'remote.py --command-file deploy-apply.sh --timeout 1800']}
    (output / 'deploy-plan.json').write_text(json.dumps(result, indent=2) + '\n', encoding='utf-8')
    print(json.dumps(result))


if __name__ == '__main__':
    main()
