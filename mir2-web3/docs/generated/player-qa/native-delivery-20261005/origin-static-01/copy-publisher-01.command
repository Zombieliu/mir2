sudo -n python3 -B - <<'MIR2_PUBLISHER_COPY_EOF'
import hashlib,json,os,pathlib,stat,pwd
stage=pathlib.Path('/srv/.mir2-origin-r17-acceleration-20261005-01');upload=pathlib.Path('/tmp/mir2-origin-input-20261005-01');items=[{'uploadName': '14-origin-r17-append.py', 'path': 'tools/origin-r17-append.py', 'size': 47951, 'sha256': '10693760a24973f896caa3f5ac6b1f59e02a32f6e9977cea0dbaa65adbaa0811'}, {'uploadName': '15-test_origin_r17_append.py', 'path': 'tools/test_origin_r17_append.py', 'size': 38105, 'sha256': '4ddffa5d88b603a653f5e3135cdba6ec66ce1264ae55b4a4aea4f8e755c8cb18'}]
assert os.geteuid()==0
for p in (stage,stage/"tools",stage/"inputs"):
    i=p.lstat();assert stat.S_ISDIR(i.st_mode) and i.st_uid==0 and stat.S_IMODE(i.st_mode)==0o700
u=upload.lstat();assert stat.S_ISDIR(u.st_mode) and u.st_uid==pwd.getpwnam("ubuntu").pw_uid and stat.S_IMODE(u.st_mode)==0o700
copied=[]
for item in items:
    p=upload/item["uploadName"];i=p.lstat()
    assert stat.S_ISREG(i.st_mode) and i.st_uid==u.st_uid and i.st_nlink==1 and stat.S_IMODE(i.st_mode)==0o600 and i.st_size==item["size"]
    fd=os.open(p,os.O_RDONLY|os.O_NOFOLLOW)
    with os.fdopen(fd,"rb") as f:
        opened=os.fstat(f.fileno());assert (i.st_dev,i.st_ino,i.st_size,i.st_mtime_ns)==(opened.st_dev,opened.st_ino,opened.st_size,opened.st_mtime_ns)
        value=f.read(item["size"]+1);end=os.fstat(f.fileno())
    assert (i.st_dev,i.st_ino,i.st_size,i.st_mtime_ns)==(end.st_dev,end.st_ino,end.st_size,end.st_mtime_ns)
    assert len(value)==item["size"] and hashlib.sha256(value).hexdigest()==item["sha256"]
    target=stage/item["path"];assert not os.path.lexists(target)
    fd=os.open(target,os.O_CREAT|os.O_EXCL|os.O_WRONLY|os.O_NOFOLLOW,0o600)
    with os.fdopen(fd,"wb") as f:f.write(value);f.flush();os.fsync(f.fileno())
    copied.append(item["sha256"])
value=(stage/"EXPECTED.json").read_bytes()
assert hashlib.sha256(value).hexdigest()=="0e68a80cb1f77118d239a908f5392daccf13f649810f1e311adf65988990e92b"
with (stage/"inputs/EXPECTED.json").open("xb") as f:f.write(value);f.flush();os.fsync(f.fileno())
for name in ("run-preflight-01","run-apply-01"):
    p=stage/name;assert not os.path.lexists(p);p.mkdir(mode=0o700)
print(json.dumps({"passed":True,"publisherFilesCopied":copied,"freshRunDirectoriesCreated":True,"publicFilesWritten":False,"servicesRestarted":False}))
MIR2_PUBLISHER_COPY_EOF
