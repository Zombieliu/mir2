sudo -n python3 -B - <<'MIR2_PRIVATE_COPY_EOF'
import hashlib, json, os, pathlib, pwd, stat
user=pathlib.Path('/tmp/mir2-origin-input-20261005-01');stage=pathlib.Path('/srv/.mir2-origin-r17-acceleration-20261005-01')
expected={'schema': 'mir2.origin.hash-pinned-upload-inputs.v1', 'files': [{'path': 'tools/prepare-native-delivery.py', 'uploadName': '00-prepare-native-delivery.py', 'size': 14617, 'sha256': 'a21b98a6efbe7b78e178ffafa7520779e6246e71c44e9545a98224de31f21950'}, {'path': 'tools/archive-update-release.py', 'uploadName': '01-archive-update-release.py', 'size': 24993, 'sha256': '9d9803cf35462bb781e91088c385adec6af33d74244cebff162162df96d13bac'}, {'path': 'tools/remote-preflight.py', 'uploadName': '02-remote-preflight.py', 'size': 7060, 'sha256': '557c23a03f6a9e47491cdb6de42db04db456a8eec992a13bb762b167533af479'}, {'path': 'tools/guarded-generate.py', 'uploadName': '03-guarded-generate.py', 'size': 7583, 'sha256': '2621098ad7db3855f48d640872f4e536b962d5e04cf5906d6415aa895cf605fe'}, {'path': 'EXPECTED.json', 'uploadName': '04-EXPECTED.json', 'size': 5229, 'sha256': '0e68a80cb1f77118d239a908f5392daccf13f649810f1e311adf65988990e92b'}, {'path': 'BASELINE.json', 'uploadName': '05-BASELINE.json', 'size': 7723, 'sha256': '08ed8ef90bcca21d13ca1a72bc63337f0864569f6c469e96ce585736b569b045'}, {'path': 'inputs/DELIVERY.json', 'uploadName': '06-DELIVERY.json', 'size': 7272337, 'sha256': 'a72b61448b342656dda4310ceb7ba5aff6608439025775fde2a8303f6ad62de8'}, {'path': 'inputs/DELIVERY.p7s', 'uploadName': '07-DELIVERY.p7s', 'size': 1614, 'sha256': 'c3f245cbff7a2e4545d1723cf56d0498b58d7af6360312d53253d255d60eeb93'}, {'path': 'inputs/SIGN-DELIVERY.json', 'uploadName': '08-SIGN-DELIVERY.json', 'size': 600, 'sha256': 'b733b8eca497d61a84db1c9bc0d3cce46ab19a291eea47f71e55692bee8588c1'}, {'path': 'inputs/ORIGIN-ADDITIONS.json', 'uploadName': '09-ORIGIN-ADDITIONS.json', 'size': 6314, 'sha256': 'c47b91cc9ad01f57256a97bfb9ce56fe3475706fee50ce2c81a5b72a5827cd09'}, {'path': 'inputs/PUBLICATION-PLAN.json', 'uploadName': '10-PUBLICATION-PLAN.json', 'size': 13860, 'sha256': '04bdb9d5f891bba38752e4ba4029db2d638dc7f4674d494ce0c6b5ab354f3b87'}, {'path': 'inputs/ROOT-VERIFICATION.json', 'uploadName': '11-ROOT-VERIFICATION.json', 'size': 424, 'sha256': '6b36134597180ad2b423c2015c05b3c48447fbd766e858c2fb0ce6f45e2565b9'}, {'path': 'inputs/CMS-RECHECK.json', 'uploadName': '12-CMS-RECHECK.json', 'size': 1578, 'sha256': 'f88f248c69f3d4b9bf3624dec171fc23fd2640e093d339f07e4c65a465c158ec'}], 'containsCredentials': False, 'publicFilesModified': False, 'userStage': '/tmp/mir2-origin-input-20261005-01', 'rootStage': '/srv/.mir2-origin-r17-acceleration-20261005-01'}
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
inventory=read_input(user/"UPLOAD-INPUTS.json",2904,'4e01407fe254257a3f86d72c05bc96ec084f1a743522868ee2e912727b36bfb1')
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
MIR2_PRIVATE_COPY_EOF
