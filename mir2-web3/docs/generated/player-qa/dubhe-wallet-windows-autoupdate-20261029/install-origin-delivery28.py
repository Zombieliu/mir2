#!/usr/bin/env python3
"""One-time, fixed Candidate28 delivery copy. No aliases or service mutations."""
import concurrent.futures, datetime, fcntl, hashlib, json, os, pathlib, re, stat, urllib.request, uuid

PLAN = pathlib.Path('/home/ubuntu/mir2-native-upload-63c2254bb18c4af09ae6caaff0bb87c7/PUBLICATION-PLAN.json')
PLAN_SHA = '5473cfe052e8715ef3f9aa9521239fd94a0d1ad343dd119c1d47a35795869311'
COMPONENT = 'releases/game-WN-CANDIDATE-20261029-wallet-28/'
TARGET = pathlib.Path('/srv/mir2-client-updates/five-class') / COMPONENT.rstrip('/')
BASE = 'https://assets.mir2.obelisk.build/client-updates/five-class/'
MANIFEST_SHA = 'bca71514a99e158eb583f48d3e81095219c81c0c122cb1ae498b081460f822c0'

def need(ok, message):
    if not ok:
        raise RuntimeError(message)

def directory(path):
    for parent in reversed((path, *path.parents)):
        value = parent.lstat()
        need(stat.S_ISDIR(value.st_mode) and not stat.S_ISLNK(value.st_mode) and value.st_uid == 0 and not value.st_mode & 0o022, 'unsafe directory: '+str(parent))

def read_file(path):
    value = path.lstat()
    need(stat.S_ISREG(value.st_mode) and not stat.S_ISLNK(value.st_mode) and value.st_nlink == 1 and not value.st_mode & 0o022, 'unsafe file: '+str(path))
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW)
    opened = os.fstat(fd)
    need((value.st_dev,value.st_ino,value.st_size,value.st_mtime_ns) == (opened.st_dev,opened.st_ino,opened.st_size,opened.st_mtime_ns), 'file changed during open')
    return os.fdopen(fd,'rb')

def file_identity(path):
    with read_file(path) as stream:
        size = os.fstat(stream.fileno()).st_size
        return size, hashlib.file_digest(stream,'sha256').hexdigest()

class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self,*args,**kwargs):
        return None

def fetch(entry, stage):
    relative = entry['path'][len(COMPONENT):]
    saved = stage / relative
    if saved.parent != stage:
        saved.parent.mkdir(mode=0o700,exist_ok=True)
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}),NoRedirect())
    request = urllib.request.Request(BASE+entry['path'],headers={'Accept-Encoding':'identity'})
    with opener.open(request,timeout=60) as response:
        need(response.status == 200 and response.geturl() == request.full_url, 'unexpected download URL/status')
        need(response.headers.get('Content-Encoding','identity') == 'identity', 'unexpected encoding')
        need(response.headers.get('Content-Length') == str(entry['size']), 'download length header differs')
        fd = os.open(saved, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW,0o600)
        digest, count = hashlib.sha256(), 0
        with os.fdopen(fd,'wb') as stream:
            while chunk := response.read(1024**2):
                count += len(chunk)
                need(count <= entry['size'], 'download exceeds bound')
                digest.update(chunk)
                stream.write(chunk)
            stream.flush()
            os.fsync(stream.fileno())
        need((count,digest.hexdigest()) == (entry['size'],entry['sha256']), 'download SHA256 differs')
    need(file_identity(saved) == (entry['size'],entry['sha256']), 'staged readback differs')
    return relative

def main():
    need(os.geteuid() == 0, 'root required')
    # The plan is user-uploaded input; bytes are pinned before parsing or use.
    with read_file(PLAN) as stream:
        raw = stream.read(1024**2)
        need(not stream.read(1), 'oversized plan')
    need(hashlib.sha256(raw).hexdigest() == PLAN_SHA,'publication plan differs')
    plan = json.loads(raw)
    need(plan['downloadScope'] == 'five-class' and plan['candidate']['sequence'] == 22, 'wrong channel')
    entries = []
    for entry in plan['objects']:
        path = entry['path']
        if path in (COMPONENT+'DELIVERY.json',COMPONENT+'DELIVERY.p7s') or re.fullmatch(re.escape(COMPONENT)+r'delivery/[0-9a-f]{64}\.m2[bd]\.gz',path):
            entries.append(entry)
    need(len(entries) == 21 and len({e['path'] for e in entries}) == 21, 'wrong fixed delivery closure')
    need(all(type(e['size']) is int and 0 < e['size'] <= 64*1024**2 and re.fullmatch('[0-9a-f]{64}',e['sha256']) for e in entries),'invalid object identity')
    directory(TARGET)
    need(file_identity(TARGET/'PACKAGE-MANIFEST.json')[1] == MANIFEST_SHA, 'wrong existing candidate')
    directory(pathlib.Path('/var/backups/mir2-client-updates'))
    lock_fd = os.open('/var/backups/mir2-client-updates/.publisher.lock',os.O_RDWR | os.O_NOFOLLOW)
    with os.fdopen(lock_fd,'rb') as lock:
        fcntl.flock(lock,fcntl.LOCK_EX | fcntl.LOCK_NB)
        stage = pathlib.Path('/srv') / ('.mir2-origin-delivery28-'+uuid.uuid4().hex)
        stage.mkdir(mode=0o700)
        with concurrent.futures.ThreadPoolExecutor(max_workers=4) as pool:
            list(pool.map(lambda entry: fetch(entry,stage),entries))
        # All bytes are verified before anything becomes publicly readable.
        for entry in entries:
            destination = TARGET / entry['path'][len(COMPONENT):]
            if destination.parent != TARGET and not destination.parent.exists():
                destination.parent.mkdir(mode=0o755)
            directory(destination.parent)
            if os.path.lexists(destination):
                need(file_identity(destination) == (entry['size'],entry['sha256']), 'existing object differs; refusing overwrite')
        installed, existing = [], []
        # Descriptor is published last, after all signed bundles are available.
        for entry in sorted(entries,key=lambda e: (2 if e['path'].endswith('/DELIVERY.json') else 1 if e['path'].endswith('/DELIVERY.p7s') else 0,e['path'])):
            relative = entry['path'][len(COMPONENT):]
            source, destination = stage/relative, TARGET/relative
            if os.path.lexists(destination):
                need(file_identity(destination) == (entry['size'],entry['sha256']), 'existing object changed; refusing overwrite')
                existing.append(relative)
                continue
            os.chmod(source,0o644)
            os.link(source,destination,follow_symlinks=False)
            source.unlink()
            need(file_identity(destination) == (entry['size'],entry['sha256']), 'published readback differs')
            fd = os.open(destination.parent,os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
            os.fsync(fd)
            os.close(fd)
            installed.append(relative)
        opener = urllib.request.build_opener(urllib.request.ProxyHandler({}),NoRedirect())
        public_base = 'https://165.154.65.136.sslip.io/client-updates/five-class/'
        public_checks = []
        for entry in entries:
            url = public_base+entry['path']
            with opener.open(urllib.request.Request(url,method='HEAD',headers={'Accept-Encoding':'identity'}),timeout=60) as response:
                need(response.status == 200 and response.geturl() == url and response.headers.get('Content-Length') == str(entry['size']), 'origin HEAD differs')
                need(response.headers.get('Content-Encoding','identity') == 'identity' and 'immutable' in response.headers.get('Cache-Control',''), 'origin encoding/cache differs')
            if entry['path'].endswith(('/DELIVERY.json','/DELIVERY.p7s')):
                with opener.open(urllib.request.Request(url,headers={'Accept-Encoding':'identity'}),timeout=60) as response:
                    data = response.read(entry['size']+1)
                    need(response.status == 200 and len(data) == entry['size'] and hashlib.sha256(data).hexdigest() == entry['sha256'], 'origin signed metadata bytes differ')
                public_checks.append({'path':entry['path'],'head':200,'metadataSha256Verified':True})
            else:
                with opener.open(urllib.request.Request(url,headers={'Accept-Encoding':'identity','Range':'bytes=0-63'}),timeout=60) as response:
                    data = response.read(65)
                    need(response.status == 206 and response.headers.get('Content-Range') == 'bytes 0-63/%d'%entry['size'] and response.headers.get('Content-Length') == '64' and len(data) == 64, 'origin bundle Range differs')
                with read_file(TARGET/entry['path'][len(COMPONENT):]) as stream:
                    need(data == stream.read(64), 'origin bundle Range bytes differ')
                public_checks.append({'path':entry['path'],'head':200,'range':206,'prefixMatched':True})
        result = {'schema':'mir2.fixed-candidate28-origin-delivery.v1','passed':True,'checkedUtc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'planSha256':PLAN_SHA,'target':str(TARGET),'installed':installed,'verifiedExisting':existing,'verifiedObjects':[{'path':e['path'],'size':e['size'],'sha256':e['sha256']} for e in entries],'originPublicChecks':public_checks,'backendMutated':False,'databaseWritten':False,'dockerStarted':False,'retainedPrivateStage':str(stage)}
        print(json.dumps(result,separators=(',',':')))

if __name__ == '__main__':
    main()
