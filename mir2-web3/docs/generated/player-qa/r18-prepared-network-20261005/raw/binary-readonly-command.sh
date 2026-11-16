python3 - <<'PYREMOTE'
import pathlib,hashlib,json,os,stat
p=pathlib.Path('/home/ubuntu/mir2-playtest-preflight-20260929/capacity-verified-321316b791120a6fe549e4006623e63333a5543f')
rows=[]
for child in sorted(p.iterdir()):
    s=child.lstat();r={'name':child.name,'regularFile':stat.S_ISREG(s.st_mode),'symlink':stat.S_ISLNK(s.st_mode),'bytes':s.st_size,'mode':oct(stat.S_IMODE(s.st_mode)),'uid':s.st_uid,'executableByCurrentUid':os.access(child,os.X_OK)}
    if child.name=='mir2-gateway' and r['regularFile'] and not r['symlink']:
        with child.open('rb') as h:r['sha256']=hashlib.file_digest(h,'sha256').hexdigest()
    rows.append(r)
print(json.dumps({'schema':'mir2.r18-artifact-file-readonly.v1','directory':str(p),'files':rows,'currentUid':os.getuid(),'gatewayStarted':False,'artifactModified':False}))
PYREMOTE
