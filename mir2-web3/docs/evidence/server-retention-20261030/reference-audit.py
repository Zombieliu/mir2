import datetime,fnmatch,hashlib,json,os,pathlib,re,stat,subprocess,time,urllib.request
P=pathlib.Path
START=time.time(); candidates=[]; errors=[]; transient=[]; refs={}; coverage={}; texts=[]; links=0; link_cycles=[]
def failure(kind,error):errors.append({'category':kind,'type':type(error).__name__})
def command(args,timeout=20):
 p=subprocess.run(args,capture_output=True,text=True,timeout=timeout)
 if p.returncode:raise RuntimeError('commandFailed')
 return p.stdout
def metadata(p):
 s=p.lstat();return {'dev':s.st_dev,'ino':s.st_ino,'mode':s.st_mode,'uid':s.st_uid,'mtimeNs':s.st_mtime_ns,'ctimeNs':s.st_ctime_ns,'ageHours':(START-max(s.st_mtime,s.st_ctime))/3600}
for parent,pattern in [('/srv','.mir2-*'),('/tmp','mir2-origin-*')]:
 for p in P(parent).iterdir():
  if fnmatch.fnmatchcase(p.name,pattern):
   s=p.lstat()
   if stat.S_ISDIR(s.st_mode) and not stat.S_ISLNK(s.st_mode):
    candidates.append({'path':str(p),'identity':metadata(p),'references':[]})
    refs[str(p)]=[]
paths=sorted(refs,key=len,reverse=True)
def match(value,source,ancestor=False):
 if not isinstance(value,str):return
 value=value.replace('\\x20',' ')
 for c in candidates:
  p=c['path'];hit=p in value
  if ancestor and value.startswith('/'):
   raw=value.removesuffix(' (deleted)')
   try:hit=hit or P(raw)==P(p) or P(raw) in P(p).parents
   except ValueError:pass
  if hit:
   r={'source':source}
   if r not in c['references']:c['references'].append(r)
def text(path,category,limit=4*1024*1024):
 p=P(path)
 try:
  if not p.is_file():return
  with p.open('rb') as f:magic=f.read(4)
  if magic==b'\x7fELF':
   # Executables are references, not text configs. Inspect their dynamic search
   # paths without executing them or reading their complete binary contents.
   raw=command(['/usr/bin/readelf','-d',str(p)],timeout=5)
   match(raw,'elfDynamic:'+str(p))
   for encoded in re.findall(r'(?:RUNPATH|RPATH).*?\[([^\]]+)\]',raw):
    for entry in encoded.split(':'):
     value=entry.replace('${ORIGIN}',str(p.parent)).replace('$ORIGIN',str(p.parent))
     if value.startswith(('/','.')):match(os.path.normpath(str(p.parent/value) if not value.startswith('/') else value),'elfSearchPath:'+str(p),ancestor=True)
   texts.append({'category':'elfDynamic','path':str(p),'sha256':hashlib.sha256(raw.encode()).hexdigest()});return raw
  if p.stat().st_size>limit:raise RuntimeError('configurationTooLarge')
  raw=p.read_bytes();s=raw.decode('utf8','replace');texts.append({'category':category,'path':str(p),'sha256':hashlib.sha256(raw).hexdigest()})
  match(s,category+':'+str(p))
  # Existing absolute files referenced by launch scripts/env are inspected once
  # below. Only paths are emitted; raw source/env contents never leave memory.
  return s
 except FileNotFoundError:transient.append({'category':category,'path':str(p)})
 except (OSError,RuntimeError,subprocess.SubprocessError) as e:failure(category+':'+str(p),e)
def resolved_link(path):
 # Observe every edge, including aliases outside the initial walk roots.
 # A fully enumerated cycle is recorded; it has no further undiscovered edge.
 pending=list(P(path).parts[1:]);done=P('/');seen=set();steps=0
 while pending:
  part=pending.pop(0)
  if part in ('','.'):continue
  if part=='..':done=done.parent;continue
  cur=done/part;match(str(cur),'symlink-chain:'+str(path))
  try:s=cur.lstat()
  except FileNotFoundError:return str(cur.joinpath(*pending))
  if stat.S_ISLNK(s.st_mode):
   state=(str(cur),tuple(pending))
   if state in seen:
    link_cycles.append(str(path));return str(cur)
   seen.add(state);steps+=1
   if steps>256:raise RuntimeError('symlinkChainBound')
   target=os.readlink(cur)
   absolute=os.path.normpath(target if target.startswith('/') else str(done/target))
   if absolute not in ('/','/srv','/tmp'):match(absolute,'symlink-edge:'+str(path),ancestor=True)
   if target.startswith('/'):done=P('/')
   pending=list(P(target).parts[1:] if target.startswith('/') else P(target).parts)+pending
  else:done=cur
 return str(done)
def linkwalk(root,skip_candidates=True,max_entries=3000000):
 global links
 count=0
 if not P(root).exists():return
 stack=[root]
 while stack:
  d=stack.pop()
  try:
   with os.scandir(d) as it:
    for entry in it:
     count+=1
     if count>max_entries:failure('symlinkWalkBound:'+root,RuntimeError());return
     p=entry.path
     try:
      if entry.is_symlink():
       links+=1;target=resolved_link(p);match(target,'symlink:'+p,ancestor=True)
      elif entry.is_dir(follow_symlinks=False) and not(skip_candidates and p in refs):stack.append(p)
     except FileNotFoundError:pass
     except (OSError,RuntimeError) as e:failure('symlinkWalk:'+root,e)
  except FileNotFoundError:pass
  except OSError as e:failure('symlinkWalk:'+root,e)
 coverage['symlinks:'+root]=count
try:
 loaded=command(['systemctl','list-units','--all','--no-legend','--plain','--no-pager']).splitlines()
 installed=command(['systemctl','list-unit-files','--no-legend','--no-pager']).splitlines()
 # Bare @. templates are not instantiated units and systemctl show rejects them;
 # their persistent definitions are scanned below, including vendor templates.
 names=sorted({x.split()[0] for x in loaded+installed if x.split() and '@.' not in x.split()[0] and re.fullmatch(r'[A-Za-z0-9_@.\\:-]+\.(service|socket|timer|path|mount|automount)',x.split()[0])})
 properties='Id,LoadState,ActiveState,MainPID,ExecStart,ExecStartPre,ExecStartPost,ExecStop,ExecStopPost,ExecReload,WorkingDirectory,EnvironmentFiles,RootDirectory,RootImage,BindPaths,BindReadOnlyPaths,ReadWritePaths,FragmentPath,DropInPaths'
 raw=command(['systemctl','show','--property='+properties,'--',*names],timeout=30)
 match(raw,'effectiveSystemdProperties')
 envfiles=[];launchfiles=[];units=[]
 for block in raw.split('\n\n'):
  row=dict(line.split('=',1) for line in block.splitlines() if '=' in line)
  if not row:continue
  units.append({k:row.get(k) for k in ('Id','LoadState','ActiveState','MainPID')})
  for key in ('WorkingDirectory','RootDirectory','BindPaths','BindReadOnlyPaths'):
   for token in row.get(key,'').split():
    value=token.removeprefix('-')
    # Generic host '/' is not ownership of every historical directory.
    if value!='/':match(value,'systemd:'+str(row.get('Id'))+':'+key,ancestor=True)
  for key in ('EnvironmentFiles','FragmentPath','DropInPaths'):
   for path in re.findall(r'/(?:etc|opt|home|var|srv|usr|lib|run)/[^\s;{}()\x00]+',row.get(key,'')):
    envfiles.append((path,'systemdFile'))
  for key in ('ExecStart','ExecStartPre','ExecStartPost','ExecStop','ExecStopPost','ExecReload'):
   for path in re.findall(r'/(?:etc|opt|home|var|srv|usr/local)/[^\s;{}()\x00"\']+',row.get(key,'')):
    launchfiles.append((path,'launchFile'))
 coverage['systemdUnits']=len(units)
 for p,cat in sorted(set(envfiles+launchfiles)):text(p,cat)
except (OSError,RuntimeError,subprocess.SubprocessError) as e:failure('systemdInventory',e);units=[]
for root in ('/etc/systemd/system','/run/systemd/system','/usr/lib/systemd/system','/lib/systemd/system','/etc/caddy','/etc/cron.d','/etc/cron.daily','/etc/cron.hourly','/etc/cron.weekly','/var/spool/cron/crontabs'):
 if P(root).exists():
  for p in P(root).rglob('*'):
   if p.is_file():text(p,'persistentConfig')
for p in ('/etc/crontab','/etc/systemd/journald.conf','/etc/logrotate.conf'):text(p,'persistentConfig')
try:
 with urllib.request.urlopen('http://127.0.0.1:2019/config/',timeout=4) as f:active=f.read(4*1024*1024+1)
 if len(active)>4*1024*1024:raise RuntimeError('caddyConfigTooLarge')
 match(active.decode('utf8'),'caddyRunningConfig');coverage['caddyRunningConfig']=True
except Exception as e:failure('caddyRunningConfig',e)
proc_count=0; namespaces={}; proc_script_paths=[]
for pp in P('/proc').iterdir():
 if not pp.name.isdecimal():continue
 pid=pp.name
 try:
  initial=(pp/'stat').read_text().rsplit(')',1)[1].split()[19]
  cwd=os.readlink(pp/'cwd');exe=os.readlink(pp/'exe');root=os.readlink(pp/'root')
  for label,value in (('cwd',cwd),('exe',exe),('root',root)):
   # '/' is every process's host root; it does not make every file a reference.
   match(value,'proc:'+pid+':'+label,ancestor=(label=='cwd' and value not in ('/','/srv','/tmp')))
  argv=(pp/'cmdline').read_bytes().decode('utf8','replace').split('\x00')
  env=(pp/'environ').read_bytes().decode('utf8','replace')
  match('\n'.join(argv),'proc:'+pid+':argv');match(env,'proc:'+pid+':environment')
  for arg in argv:
   val=arg.split('=',1)[-1] if arg.startswith('-') else arg
   if val and len(val)<4096 and not any(x in val for x in '\x00\n\r'):
    ap=P(val) if val.startswith('/') else P(cwd)/val
    if ap.is_file() and ap.suffix in ('.py','.sh','.js','.mjs','.cjs','.json','.toml','.conf'):
     proc_script_paths.append(str(ap))
     match(str(ap),'proc:'+pid+':resolvedArg')
  for fd in (pp/'fd').iterdir():
   try:match(os.readlink(fd),'proc:'+pid+':fd',ancestor=False)
   except FileNotFoundError:pass
  match((pp/'maps').read_text(),'proc:'+pid+':mappedFiles')
  ns=os.readlink(pp/'ns/mnt')
  if ns not in namespaces:
   mountinfo=(pp/'mountinfo').read_text();namespaces[ns]=mountinfo
   for line in mountinfo.splitlines():
    fields=line.split();
    for val in fields[3:5]:
     # A mounted '/' or ordinary /srv filesystem is not an external bind.
     if val not in ('/','/srv','/tmp'):match(val,'mountNamespace:'+ns,ancestor=True)
  again=(pp/'stat').read_text().rsplit(')',1)[1].split()[19]
  if again!=initial:raise RuntimeError('pidReused')
  proc_count+=1
 except (FileNotFoundError,ProcessLookupError):transient.append({'category':'vanishedProcess','pid':pid})
 except PermissionError as e:
  # Kernel-only processes have no exe/cwd/environ but cannot use client stages.
  try:
   if not (pp/'cmdline').read_bytes():transient.append({'category':'kernelOrExitedProcess','pid':pid});continue
  except FileNotFoundError:continue
  failure('processAccess:'+pid,e)
 except (OSError,RuntimeError) as e:failure('processInventory:'+pid,e)
for path in sorted(set(proc_script_paths)):text(path,'runningScriptOrConfig')
coverage['processes']=proc_count;coverage['mountNamespaces']=len(namespaces)
# Do not invoke Docker. Stopped containers and their bind mounts still protect data.
for root in ('/var/lib/docker/containers','/var/lib/containerd'):
 if not P(root).exists():continue
 if root.endswith('/containers'):
  for cp in P(root).iterdir():
   for name in ('hostconfig.json','config.v2.json'):text(cp/name,'containerMetadata')
 else:coverage['containerdPresentRequiresReview']=True
for root in ('/srv/mir2-client-updates','/etc/caddy','/opt','/var/backups','/var/lib/mir2','/var/lib/mir2-playtest'):
 linkwalk(root)
# References from the retained published/rollback roots are protected wholesale;
# none of those roots may be a deletion candidate in this task.
coverage['publicRootProtectedWholesale']='/srv/mir2-client-updates'
coverage['backupAndStateProtectedWholesale']=True
for c in candidates:
 c['eligibleByReferences']=not c['references'] and not errors
 c['oldEnough48Hours']=c['identity']['ageHours']>=48
free=os.statvfs('/')
out={'schemaVersion':1,'observedAtUtc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'durationSeconds':time.time()-START,'candidates':candidates,'coverage':coverage,'errors':errors,'transientRaces':transient,'scannedTextFiles':texts,'symlinksExamined':links,'fullyEnumeratedLinkCycles':sorted(set(link_cycles)),'services':units,'availableBytes':free.f_bavail*free.f_frsize,'readOnly':True}
print(json.dumps(out,separators=(',',':')))
