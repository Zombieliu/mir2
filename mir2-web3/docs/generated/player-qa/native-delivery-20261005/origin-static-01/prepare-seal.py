import hashlib
import json
from pathlib import Path

root=Path(__file__).absolute().parent
value=(root/"UPLOAD-INPUTS.json").read_bytes()
inventory=json.loads(value)
files=[{"uploadName":"UPLOAD-INPUTS.json","size":len(value),"sha256":hashlib.sha256(value).hexdigest()},
       *inventory["files"]]
script='''import hashlib,json,os,pathlib,pwd,stat
p=pathlib.Path(%r);items=%r
assert pwd.getpwuid(os.getuid()).pw_name=="ubuntu"
i=p.lstat();assert stat.S_ISDIR(i.st_mode) and i.st_uid==os.getuid() and stat.S_IMODE(i.st_mode)==0o700
records=[]
for item in items:
    path=p/item["uploadName"];before=path.lstat()
    assert stat.S_ISREG(before.st_mode) and before.st_uid==os.getuid() and before.st_nlink==1
    assert before.st_size==item["size"]
    fd=os.open(path,os.O_RDONLY|os.O_NOFOLLOW)
    with os.fdopen(fd,"rb") as stream:
        opened=os.fstat(stream.fileno());assert (before.st_dev,before.st_ino)==(opened.st_dev,opened.st_ino)
        assert hashlib.file_digest(stream,"sha256").hexdigest()==item["sha256"]
        os.fchmod(stream.fileno(),0o600)
        after=os.fstat(stream.fileno());assert stat.S_IMODE(after.st_mode)==0o600
    records.append({"name":item["uploadName"],"beforeMode":oct(stat.S_IMODE(before.st_mode)),"afterMode":"0o600","sha256":item["sha256"]})
print(json.dumps({"passed":True,"privateUploadInputsSealed":records,"publicFilesWritten":False}))
'''%(inventory["userStage"],files)
with (root/"seal-inputs-01.command").open("x",newline="\n") as out:
    out.write("python3 -B - <<'MIR2_SEAL_PRIVATE_INPUTS_EOF'\n"+script+"MIR2_SEAL_PRIVATE_INPUTS_EOF\n")
