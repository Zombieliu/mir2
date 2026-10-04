python3 -B - <<'MIR2_PUBLISHER_SEAL_EOF'
import hashlib,json,os,pathlib,stat,pwd
stage=pathlib.Path('/tmp/mir2-origin-input-20261005-01');items=[{'uploadName': '14-origin-r17-append.py', 'path': 'tools/origin-r17-append.py', 'size': 47951, 'sha256': '10693760a24973f896caa3f5ac6b1f59e02a32f6e9977cea0dbaa65adbaa0811'}, {'uploadName': '15-test_origin_r17_append.py', 'path': 'tools/test_origin_r17_append.py', 'size': 38105, 'sha256': '4ddffa5d88b603a653f5e3135cdba6ec66ce1264ae55b4a4aea4f8e755c8cb18'}]
assert os.getuid()==pwd.getpwnam("ubuntu").pw_uid
i=stage.lstat();assert stat.S_ISDIR(i.st_mode) and i.st_uid==os.getuid() and stat.S_IMODE(i.st_mode)==0o700
records=[]
for item in items:
    p=stage/item["uploadName"];i=p.lstat()
    assert stat.S_ISREG(i.st_mode) and i.st_uid==os.getuid() and i.st_nlink==1 and i.st_size==item["size"]
    fd=os.open(p,os.O_RDONLY|os.O_NOFOLLOW)
    with os.fdopen(fd,"rb") as f:
        assert os.fstat(f.fileno()).st_ino==i.st_ino and hashlib.file_digest(f,"sha256").hexdigest()==item["sha256"]
        os.fchmod(f.fileno(),0o600)
    records.append(item["sha256"])
print(json.dumps({"passed":True,"privatePublisherInputsSealed":records,"publicFilesWritten":False}))
MIR2_PUBLISHER_SEAL_EOF
