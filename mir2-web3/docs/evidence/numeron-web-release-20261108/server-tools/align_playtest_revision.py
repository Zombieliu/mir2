"""Align one owned TEST release label; no executable, key or data edits."""
import os, json, stat, hashlib, time, subprocess, urllib.request, runpy

m = runpy.run_path('/usr/local/libexec/mir2-r16-qa/r4/cutover_playtest_r16.py', run_name='revision_alignment')
assert os.geteuid() == 0
b = '/root/r16-playtest-revision-d53-20261109'
envfile, revision = m['ENV'], m['SOURCE']
def run(argv, **kw):
    return subprocess.run(argv, check=True, capture_output=True, timeout=12, **kw).stdout
def props():
    text = run(['/usr/bin/systemctl', 'show', 'mir2-playtest.service', '--property=ActiveState,MainPID,ExecStart'], text=True)
    return dict(line.split('=', 1) for line in text.splitlines())
def health():
    with urllib.request.urlopen('http://127.0.0.1:7210/health', timeout=3) as r:
        return json.load(r)
def save(name, data):
    with open(b + '/' + name, 'xb') as f:
        f.write(data); f.flush(); os.fsync(f.fileno())
    os.chmod(b + '/' + name, 0o600)
def read_owned():
    fd = os.open(envfile, os.O_RDONLY | os.O_NOFOLLOW)
    try:
        st = os.fstat(fd)
        assert stat.S_ISREG(st.st_mode) and st.st_uid == st.st_gid == 0 and st.st_nlink == 1 and stat.S_IMODE(st.st_mode) == 0o600
        data = os.read(fd, 16385); assert len(data) == st.st_size < 16384
        return data, (st.st_dev, st.st_ino)
    finally: os.close(fd)

before = props(); pid = before['MainPID']
assert before['ActiveState'] == 'active' and os.readlink('/proc/' + pid + '/exe') == m['NEW']
h = health()
assert h['ok'] and h['revision'] == 'e9117b6691e3f7ec238ff1c1eb77b699b0646c2f'
assert all(value == 0 for key, value in h['capacity'].items() if key.startswith('current'))
old, inode = read_owned()
assert b'MIR2_DEPLOY_REVISION=' not in old and old.endswith(b'\n')
os.mkdir(b, 0o700)
save('previous-environment.private', old)
save('proc-environment.private', open('/proc/' + pid + '/environ', 'rb').read())
save('before.public.json', json.dumps({'pid': int(pid), 'idle': True, 'oldRevision': h['revision'], 'binary': m['NEW']}).encode())
current = health()
assert current['ok'] and all(value == 0 for key, value in current['capacity'].items() if key.startswith('current')) and props() == before
run(['/usr/bin/systemctl', 'stop', '--no-block', 'mir2-playtest.service'])
deadline = time.monotonic() + 40
while props()['ActiveState'] == 'deactivating' and time.monotonic() < deadline:
    time.sleep(.25)
assert props()['ActiveState'] == 'inactive' and props()['MainPID'] == '0'
with open(b + '/final.pg_dump', 'xb') as f:
    subprocess.run(['/usr/sbin/runuser', '-u', 'postgres', '--', '/usr/bin/pg_dump', '--no-password', '-Fc', '--dbname=mir2_playtest'], stdout=f, stderr=subprocess.PIPE, check=True, timeout=12)
os.chmod(b + '/final.pg_dump', 0o600)
assert read_owned() == (old, inode)
new = old + ('MIR2_DEPLOY_REVISION=' + json.dumps(revision) + '\n').encode()
temporary = envfile + '.revision-r5-tmp'
fd = os.open(temporary, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
with os.fdopen(fd, 'wb') as f:
    f.write(new); f.flush(); os.fsync(f.fileno())
assert read_owned() == (old, inode)
os.replace(temporary, envfile)
parent = os.open(os.path.dirname(envfile), os.O_RDONLY | os.O_DIRECTORY)
try: os.fsync(parent)
finally: os.close(parent)
assert read_owned()[0] == new
run(['/usr/bin/systemctl', 'start', '--no-block', 'mir2-playtest.service'])
deadline = time.monotonic() + 25
after = None
while time.monotonic() < deadline:
    try:
        after = health()
        if after.get('ok') and after.get('revision') == revision: break
    except Exception: pass
    time.sleep(.3)
assert after and after.get('ok') and after.get('revision') == revision
result = {'schema': 1, 'kind': 'r16-test-revision-alignment', 'passed': True, 'revision': revision, 'binaryUnchanged': os.readlink('/proc/' + props()['MainPID'] + '/exe') == m['NEW'], 'onlyAddedEnvironmentKey': 'MIR2_DEPLOY_REVISION', 'idleBeforeStop': True, 'normalStop': True, 'snapshotBytes': os.stat(b + '/final.pg_dump').st_size, 'secretsRecorded': False}
save('verification.public.json', json.dumps(result, indent=2).encode())
print(json.dumps(result))
