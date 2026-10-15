python3 -B - <<'MIR2_SEAL_PRIVATE_INPUTS_EOF'
import hashlib,json,os,pathlib,pwd,stat
p=pathlib.Path('/tmp/mir2-origin-input-20261005-01');items=[{'uploadName': 'UPLOAD-INPUTS.json', 'size': 2904, 'sha256': '4e01407fe254257a3f86d72c05bc96ec084f1a743522868ee2e912727b36bfb1'}, {'path': 'tools/prepare-native-delivery.py', 'sha256': 'a21b98a6efbe7b78e178ffafa7520779e6246e71c44e9545a98224de31f21950', 'size': 14617, 'uploadName': '00-prepare-native-delivery.py'}, {'path': 'tools/archive-update-release.py', 'sha256': '9d9803cf35462bb781e91088c385adec6af33d74244cebff162162df96d13bac', 'size': 24993, 'uploadName': '01-archive-update-release.py'}, {'path': 'tools/remote-preflight.py', 'sha256': '557c23a03f6a9e47491cdb6de42db04db456a8eec992a13bb762b167533af479', 'size': 7060, 'uploadName': '02-remote-preflight.py'}, {'path': 'tools/guarded-generate.py', 'sha256': '2621098ad7db3855f48d640872f4e536b962d5e04cf5906d6415aa895cf605fe', 'size': 7583, 'uploadName': '03-guarded-generate.py'}, {'path': 'EXPECTED.json', 'sha256': '0e68a80cb1f77118d239a908f5392daccf13f649810f1e311adf65988990e92b', 'size': 5229, 'uploadName': '04-EXPECTED.json'}, {'path': 'BASELINE.json', 'sha256': '08ed8ef90bcca21d13ca1a72bc63337f0864569f6c469e96ce585736b569b045', 'size': 7723, 'uploadName': '05-BASELINE.json'}, {'path': 'inputs/DELIVERY.json', 'sha256': 'a72b61448b342656dda4310ceb7ba5aff6608439025775fde2a8303f6ad62de8', 'size': 7272337, 'uploadName': '06-DELIVERY.json'}, {'path': 'inputs/DELIVERY.p7s', 'sha256': 'c3f245cbff7a2e4545d1723cf56d0498b58d7af6360312d53253d255d60eeb93', 'size': 1614, 'uploadName': '07-DELIVERY.p7s'}, {'path': 'inputs/SIGN-DELIVERY.json', 'sha256': 'b733b8eca497d61a84db1c9bc0d3cce46ab19a291eea47f71e55692bee8588c1', 'size': 600, 'uploadName': '08-SIGN-DELIVERY.json'}, {'path': 'inputs/ORIGIN-ADDITIONS.json', 'sha256': 'c47b91cc9ad01f57256a97bfb9ce56fe3475706fee50ce2c81a5b72a5827cd09', 'size': 6314, 'uploadName': '09-ORIGIN-ADDITIONS.json'}, {'path': 'inputs/PUBLICATION-PLAN.json', 'sha256': '04bdb9d5f891bba38752e4ba4029db2d638dc7f4674d494ce0c6b5ab354f3b87', 'size': 13860, 'uploadName': '10-PUBLICATION-PLAN.json'}, {'path': 'inputs/ROOT-VERIFICATION.json', 'sha256': '6b36134597180ad2b423c2015c05b3c48447fbd766e858c2fb0ce6f45e2565b9', 'size': 424, 'uploadName': '11-ROOT-VERIFICATION.json'}, {'path': 'inputs/CMS-RECHECK.json', 'sha256': 'f88f248c69f3d4b9bf3624dec171fc23fd2640e093d339f07e4c65a465c158ec', 'size': 1578, 'uploadName': '12-CMS-RECHECK.json'}]
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
MIR2_SEAL_PRIVATE_INPUTS_EOF
