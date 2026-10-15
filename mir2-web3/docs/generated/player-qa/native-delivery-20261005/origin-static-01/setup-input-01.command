python3 -B - <<'MIR2_PRIVATE_INPUT_EOF'
import json, os, pathlib, pwd, stat
p=pathlib.Path('/tmp/mir2-origin-input-20261005-01')
assert pwd.getpwuid(os.getuid()).pw_name == "ubuntu"
assert not os.path.lexists(p)
p.mkdir(mode=0o700)
i=p.lstat()
assert stat.S_ISDIR(i.st_mode) and i.st_uid==os.getuid() and stat.S_IMODE(i.st_mode)==0o700
print(json.dumps({"createdPrivateUploadStage":str(p),"uid":i.st_uid,"mode":oct(stat.S_IMODE(i.st_mode)),"publicFilesWritten":False}))
MIR2_PRIVATE_INPUT_EOF
