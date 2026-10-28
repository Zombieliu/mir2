import hashlib
import json
from pathlib import Path
import shlex

root=Path(__file__).absolute().parent
worker=root.parent/"origin-acceleration-publisher-worker-01"
stage="/srv/.mir2-origin-r17-acceleration-20261005-01"
upload="/tmp/mir2-origin-input-20261005-01"
code_pins={"origin-r17-append.py":"10693760a24973f896caa3f5ac6b1f59e02a32f6e9977cea0dbaa65adbaa0811",
           "test_origin_r17_append.py":"4ddffa5d88b603a653f5e3135cdba6ec66ce1264ae55b4a4aea4f8e755c8cb18"}
items=[]
uploads=[]
for number,(name,pin) in enumerate(code_pins.items(),14):
    value=(worker/name).read_bytes()
    assert hashlib.sha256(value).hexdigest()==pin
    target="%02d-"%number+name
    uploads.append([str(worker/name),upload+"/"+target])
    items.append({"uploadName":target,"path":"tools/"+name,"size":len(value),"sha256":pin})
expected=(root/"EXPECTED-02.json").read_bytes()
assert hashlib.sha256(expected).hexdigest()=="0e68a80cb1f77118d239a908f5392daccf13f649810f1e311adf65988990e92b"
cms=(root/"CMS-VERIFICATION-01.json").read_bytes()
cms_sha=hashlib.sha256(cms).hexdigest()
assert cms_sha=="f88f248c69f3d4b9bf3624dec171fc23fd2640e093d339f07e4c65a465c158ec"
test_log=(worker/"local-tests-05.log").read_bytes()
assert hashlib.sha256(test_log).hexdigest()=="6df6f916b8acffda7149097934f5902e86504e0b8b4f9c4462854dc6fe50585f"
with (root/"upload-publisher-01.json").open("x",newline="\n") as stream:
    json.dump(uploads,stream,indent=2)
seal='''import hashlib,json,os,pathlib,stat,pwd
stage=pathlib.Path(%r);items=%r
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
'''%(upload,items)
with (root/"seal-publisher-01.command").open("x",newline="\n") as stream:
    stream.write("python3 -B - <<'MIR2_PUBLISHER_SEAL_EOF'\n"+seal+"MIR2_PUBLISHER_SEAL_EOF\n")
copy='''import hashlib,json,os,pathlib,stat,pwd
stage=pathlib.Path(%r);upload=pathlib.Path(%r);items=%r
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
'''%(stage,upload,items)
with (root/"copy-publisher-01.command").open("x",newline="\n") as stream:
    stream.write("sudo -n python3 -B - <<'MIR2_PUBLISHER_COPY_EOF'\n"+copy+"MIR2_PUBLISHER_COPY_EOF\n")
with (root/"linux-negative-tests-01.command").open("x",newline="\n") as stream:
    stream.write("sudo -n nice -n 10 python3 -B "+shlex.quote(stage+"/tools/test_origin_r17_append.py")+"\n")
for mode,label in (("preflight","preflight-01"),("apply","apply-01")):
    command=["sudo","-n","nice","-n","10","python3","-B",stage+"/tools/origin-r17-append.py",mode,
             "--inputs",stage+"/inputs","--generated",stage+"/generated","--run-dir",stage+"/run-"+label,
             "--cms-receipt-sha256",cms_sha]
    with (root/("publisher-"+label+".command")).open("x",newline="\n") as stream:
        stream.write(shlex.join(command)+"\n")
receipt={"passed":True,"reviewedPublisherSHA256":code_pins,"localTests":31,
         "localTestsLogSha256":hashlib.sha256(test_log).hexdigest(),"cmsReceiptSha256":cms_sha,
         "allRemoteCommandsPreparedOnly":True,"credentialsIncluded":False}
with (root/"PUBLISHER-ROOT-REVIEW-01.json").open("x",newline="\n") as stream:
    json.dump(receipt,stream,indent=2)
print(json.dumps(receipt))
