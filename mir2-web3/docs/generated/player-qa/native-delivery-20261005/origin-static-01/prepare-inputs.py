"""Prepare hash-pinned small public inputs, no credentials or upload yet."""
import hashlib
import json
from pathlib import Path
import shlex

root = Path(__file__).absolute().parent
base = root.parent
plan = base / "origin-download-compatibility-plan-01"
tools = plan / "source/apps/game-client/windows-updater/scripts"
delivery = base / "r17-download-acceleration-02"
publication = base / "r17-r2-publication-01"
user_stage = "/tmp/mir2-origin-input-20261005-01"
private_stage = "/srv/.mir2-origin-r17-acceleration-20261005-01"
sources = {
    "tools/prepare-native-delivery.py": tools / "prepare-native-delivery.py",
    "tools/archive-update-release.py": tools / "archive-update-release.py",
    "tools/remote-preflight.py": root / "remote-preflight.py",
    "tools/guarded-generate.py": root / "guarded-generate.py",
    "EXPECTED.json": root / "EXPECTED-02.json",
    "BASELINE.json": root / "preflight-02.json",
    "inputs/DELIVERY.json": delivery / "DELIVERY.json",
    "inputs/DELIVERY.p7s": delivery / "DELIVERY.p7s",
    "inputs/SIGN-DELIVERY.json": delivery / "SIGN-DELIVERY.json",
    "inputs/ORIGIN-ADDITIONS.json": plan / "ORIGIN-ADDITIONS.json",
    "inputs/PUBLICATION-PLAN.json": publication / "PUBLICATION-PLAN.json",
    "inputs/ROOT-VERIFICATION.json": publication / "ROOT-VERIFICATION.json",
    "inputs/CMS-RECHECK.json": root / "CMS-VERIFICATION-01.json",
}
items = []
uploads = []
for number, (name, path) in enumerate(sources.items()):
    data = path.read_bytes()
    flat = "%02d-" % number + Path(name).name
    items.append({"path": name, "uploadName": flat, "size": len(data),
                  "sha256": hashlib.sha256(data).hexdigest()})
    uploads.append([str(path), user_stage + "/" + flat])
proof = {"schema": "mir2.origin.hash-pinned-upload-inputs.v1", "files": items,
         "containsCredentials": False, "publicFilesModified": False,
         "userStage": user_stage, "rootStage": private_stage}
encoded = json.dumps(proof, sort_keys=True, indent=2).encode() + b"\n"
with (root / "UPLOAD-INPUTS.json").open("xb") as stream:
    stream.write(encoded)
uploads.append([str(root / "UPLOAD-INPUTS.json"), user_stage + "/UPLOAD-INPUTS.json"])
with (root / "upload-01.json").open("x", newline="\n") as stream:
    json.dump(uploads, stream, sort_keys=True, indent=2)
setup = '''import json, os, pathlib, pwd, stat
p=pathlib.Path(%r)
assert pwd.getpwuid(os.getuid()).pw_name == "ubuntu"
assert not os.path.lexists(p)
p.mkdir(mode=0o700)
i=p.lstat()
assert stat.S_ISDIR(i.st_mode) and i.st_uid==os.getuid() and stat.S_IMODE(i.st_mode)==0o700
print(json.dumps({"createdPrivateUploadStage":str(p),"uid":i.st_uid,"mode":oct(stat.S_IMODE(i.st_mode)),"publicFilesWritten":False}))
''' % user_stage
with (root / "setup-input-01.command").open("x", newline="\n") as stream:
    stream.write("python3 -B - <<'MIR2_PRIVATE_INPUT_EOF'\n" + setup + "MIR2_PRIVATE_INPUT_EOF\n")
copy_script = '''import hashlib, json, os, pathlib, pwd, stat
user=pathlib.Path(%r);stage=pathlib.Path(%r)
expected=%r
assert os.geteuid()==0
assert not os.path.lexists(stage)
assert stat.S_ISDIR(pathlib.Path("/srv").lstat().st_mode)
assert pathlib.Path("/srv").lstat().st_uid==0 and pathlib.Path("/srv").lstat().st_mode&0o022==0
u=user.lstat()
assert stat.S_ISDIR(u.st_mode) and u.st_uid==pwd.getpwnam("ubuntu").pw_uid and stat.S_IMODE(u.st_mode)==0o700
def read_input(p,n,sha):
    info=p.lstat();assert stat.S_ISREG(info.st_mode) and info.st_nlink==1 and info.st_uid==u.st_uid
    assert info.st_mode&0o022==0 and info.st_size==n
    fd=os.open(p,os.O_RDONLY|os.O_NOFOLLOW)
    with os.fdopen(fd,"rb") as f:
        opened=os.fstat(f.fileno());assert (info.st_dev,info.st_ino,info.st_size,info.st_mtime_ns)==(opened.st_dev,opened.st_ino,opened.st_size,opened.st_mtime_ns)
        value=f.read(n+1);end=os.fstat(f.fileno())
    assert (info.st_dev,info.st_ino,info.st_size,info.st_mtime_ns)==(end.st_dev,end.st_ino,end.st_size,end.st_mtime_ns)
    assert len(value)==n and hashlib.sha256(value).hexdigest()==sha
    return value
inventory=read_input(user/"UPLOAD-INPUTS.json",%d,%r)
assert json.loads(inventory)==expected
verified=[(i,read_input(user/i["uploadName"],i["size"],i["sha256"])) for i in expected["files"]]
assert not os.path.lexists(stage)
os.umask(0o077);stage.mkdir(mode=0o700)
(stage/"tools").mkdir(mode=0o700);(stage/"inputs").mkdir(mode=0o700)
assert stage.lstat().st_dev==pathlib.Path("/srv/mir2-client-updates").lstat().st_dev
for item,value in verified:
    target=stage/item["path"]
    with target.open("xb") as f:f.write(value);f.flush();os.fsync(f.fileno())
with (stage/"UPLOAD-INPUTS.json").open("xb") as f:f.write(inventory);f.flush();os.fsync(f.fileno())
print(json.dumps({"passed":True,"privateRootStage":str(stage),"inputsCopied":len(verified),"inputBytes":sum(len(v) for _,v in verified),"allFilesSHA256Verified":True,"publicFilesWritten":False,"servicesRestarted":False}))
''' % (user_stage, private_stage, proof, len(encoded), hashlib.sha256(encoded).hexdigest())
with (root / "root-copy-01.command").open("x", newline="\n") as stream:
    stream.write("sudo -n python3 -B - <<'MIR2_PRIVATE_COPY_EOF'\n" + copy_script + "MIR2_PRIVATE_COPY_EOF\n")
with (root / "generate-01.command").open("x", newline="\n") as stream:
    stream.write("sudo -n nice -n 10 python3 -B " + shlex.quote(private_stage + "/tools/guarded-generate.py") + "\n")
print(json.dumps({"passed": True, "files": len(items), "bytes": sum(item["size"] for item in items),
                  "uploadManifestSha256": hashlib.sha256(encoded).hexdigest()}))
