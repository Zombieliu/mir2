set -eu
exec sudo -n python3 -c 'import hashlib,os,pathlib,stat,sys
base=pathlib.Path('"'"'/var/lib/mir2-client-publisher'"'"')
def need(ok):
 if not ok:raise SystemExit('"'"'Privileged script identity/path guard failed'"'"')
def checked_parent(p):
 for x in reversed((p,*p.parents)):
  s=x.lstat();need(stat.S_ISDIR(s.st_mode) and not stat.S_ISLNK(s.st_mode))
def copy_code(source,expected):
 source=pathlib.Path(source);checked_parent(source.parent);s=source.lstat();need(stat.S_ISREG(s.st_mode) and s.st_nlink==1 and s.st_size<=65536)
 fd=os.open(source,os.O_RDONLY|os.O_NOFOLLOW)
 with os.fdopen(fd,'"'"'rb'"'"') as f:
  t=os.fstat(f.fileno());need((s.st_dev,s.st_ino)==(t.st_dev,t.st_ino) and t.st_nlink==1);data=f.read(65537)
 need(len(data)<=65536 and hashlib.sha256(data).hexdigest().upper()==expected)
 dest=base/(expected+'"'"'.py'"'"')
 if os.path.lexists(dest):
  t=dest.lstat();need(stat.S_ISREG(t.st_mode) and t.st_nlink==1 and t.st_uid==0 and not t.st_mode&0o022 and dest.read_bytes()==data)
 else:
  fd=os.open(dest,os.O_WRONLY|os.O_CREAT|os.O_EXCL|os.O_NOFOLLOW,0o500)
  with os.fdopen(fd,'"'"'wb'"'"') as f:f.write(data);f.flush();os.fsync(f.fileno())
 return str(dest)
need(os.geteuid()==0)
checked_parent(base.parent)
if not base.exists():base.mkdir(mode=0o700)
checked_parent(base)
s=base.stat();need(s.st_uid==0 and not s.st_mode&0o077)
script=copy_code(sys.argv[1],sys.argv[2]);helper=copy_code(sys.argv[3],sys.argv[4])
os.execv(sys.executable,[sys.executable,script,'"'"'--archive-helper'"'"',helper,'"'"'--archive-helper-sha256'"'"',sys.argv[4],*sys.argv[5:]])
' /home/ubuntu/mir2-native-upload-0f74c17b589e41f697be17b448bd403e/deploy-server-release.py 34D36D30CBE0A85092AC877E4C5A6C78026FB0D2F5EDC767E8E93F26B2B2216B /home/ubuntu/mir2-native-upload-0f74c17b589e41f697be17b448bd403e/archive-update-release.py 2FB281283D42C232307EA7E682DA8E9C4A46078DE396ABA233CC4037FAF68A3B --staging /home/ubuntu/mir2-native-upload-0f74c17b589e41f697be17b448bd403e --archive-sha256 FB5697A89B2A652E793A790FA6DCCE0A88B4C1D057DD86DEADBB07CF6127AF52 --receipt-sha256 F8CD66058B7D8190B80B68EF7114FE96594BBA4B0DDCA757F1F0530E2B368F09 --latest-sha256 BEBFB5193DE19DD4107C1E5CC4BAC89C092B83094D86FE49F4A10DD3A291E487 --signature-sha256 58507D93FDB2E2D903D01DB8072CBB6C4221FE5C6691B6DBD4388B5129E2E919 --sequence 5 --expected-caddy-sha256 C9EB1149DD23BED11B94B921E812FF111822410E487C4EF61B3C9BB351C649BD --externally-verified-signatures
