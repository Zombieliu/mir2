"""One bounded read-only cloud inventory; pinned ED25519 before local getpass.

No remote files/settings/services are changed. Only caller-owned SSH is closed.
Public output contains selected numeric/enum facts, never argv or raw environment.
One authentication candidate per invocation; the coordinator supplies it in PTY.
"""
import argparse,base64,getpass,hashlib,hmac,json,shlex,socket,sys,threading,time,warnings
from pathlib import Path

PIN='SHA256:6M4ouOk6QvFj1qIEmjj9/LOa8/enGiqZ/n+m7JHo7q8'
MARKER=b'MIR2_FRESH_READONLY_INVENTORY_V1 '
RUNTIME=Path('C:/Users/Administrator/.codex/tmp/mir2-deploy-ssh-runtime')
REMOTE=r'''
import hashlib,json,math,os,re,subprocess,time,urllib.request
from pathlib import Path
DEADLINE=time.monotonic()+20
UNIT='mir2-playtest.service'
SPEC={
 'MIR2_GATEWAY_MAX_ACTIVE_SESSIONS':'uint','MIR2_GATEWAY_MAX_WS_CONNECTIONS':'uint','MIR2_GATEWAY_MAX_RECONNECT_LEASES':'uint',
 'MIR2_GATEWAY_MAX_LOGIN_IN_FLIGHT':'uint','MIR2_GATEWAY_MAX_NEW_CHARACTER_IN_FLIGHT':'uint','MIR2_GATEWAY_MAX_START_GAME_IN_FLIGHT':'uint',
 'MIR2_GATEWAY_RECONNECT_GRACE_SECONDS':'uint','MIR2_GATEWAY_TOKIO_WORKER_THREADS':'uint','MIR2_GATEWAY_RUNTIME_TICK_MS':'uint',
 'MIR2_GATEWAY_RUNTIME_TICK_INPUT_WAKE_MS':'uint','MIR2_GATEWAY_SLOW_STAGE_MS':'uint','MIR2_GATEWAY_ZONE_DELIVERY_TRACE':'flag',
 'MIR2_ZONE_ROUTING_MODE':{'single','per_map'},'MIR2_ZONE_OWNER_LEASE_BACKEND':{'redis','memory','in-memory','in_memory'},
 'MIR2_ZONE_TOPOLOGY_FILE':'presence','MIR2_ZONE_TOPOLOGY_JSON':'presence','MIR2_GATEWAY_ZONE_OWNER_RESOLVER_CONFIG':'presence',
 'MIR2_ACCOUNT_STORE_BACKEND':{'file','json','postgres','source','source-of-truth','source_of_truth','mirror'},
 'MIR2_ACCOUNT_STORE_PG_POOL_MAX_SIZE':'uint','MIR2_CONTENT_PROFILE':{'platinum_176','platinum_fiveclass','crystal_full'},
}
def left(cap=20):
 value=DEADLINE-time.monotonic()
 if value<=0:raise TimeoutError('remoteQueryBudget')
 return min(cap,value)
def uint(value):
 return int(value) if re.fullmatch(r'[0-9]{1,20}',value) and int(value)<=18446744073709551615 else None
def project_environment(raw):
 out={key:{'present':False,'configuredValue':None,'parseStatus':'absent'} for key in SPEC}
 for line in raw.split(b'\0'):
  key,sep,value=line.partition(b'=')
  try:name=key.decode('ascii')
  except UnicodeDecodeError:continue
  if not sep or name not in SPEC:continue
  row=out[name]
  if row['present']:row.update(configuredValue=None,parseStatus='duplicate');continue
  row['present']=True;row['parseStatus']='invalid';kind=SPEC[name]
  if kind=='presence':row.update(configuredValue=True,parseStatus='valid');continue
  if len(value)>64:continue
  try:token=value.decode('ascii')
  except UnicodeDecodeError:continue
  parsed=None
  if kind=='uint':parsed=uint(token)
  elif kind=='flag' and token.strip().lower() in {'1','true','yes','on','0','false','no','off'}:parsed=token.strip().lower() in {'1','true','yes','on'}
  elif isinstance(kind,set) and token in kind:parsed=token
  if parsed is not None:row.update(configuredValue=parsed,parseStatus='valid')
 return out
def read(path,limit=16384):
 left()
 with Path(path).open('rb') as stream:data=stream.read(limit+1)
 if len(data)>limit:raise ValueError('fileReadBound')
 return data.decode('ascii',errors='strict').strip()
def startticks(pid):
 text=read('/proc/'+str(pid)+'/stat',4096)
 return int(text[text.rfind(')')+2:].split()[19])
def unit():
 props=['MainPID','InvocationID','ActiveState','SubState','ControlGroup','CPUQuotaPerSecUSec','CPUQuotaPeriodUSec','MemoryMax','MemoryHigh','TasksMax']
 result=subprocess.run(['systemctl','show',UNIT]+['--property='+p for p in props],stdin=subprocess.DEVNULL,capture_output=True,text=True,check=True,timeout=left())
 fields=dict(line.split('=',1) for line in result.stdout.splitlines() if '=' in line)
 pid=int(fields.get('MainPID','0'))
 invocation=fields.get('InvocationID','')
 if invocation and not re.fullmatch('[0-9a-f]{32}',invocation):raise ValueError('invocationFormat')
 return {'pid':pid,'invocationId':invocation or None,'activeState':fields.get('ActiveState'),'subState':fields.get('SubState'),
         'processStartTicks':startticks(pid) if pid else None,'controlGroup':fields.get('ControlGroup'),
         'systemdLimits':{p:fields.get(p) if fields.get(p) in {'infinity','[not set]'} or re.fullmatch(r'[0-9]+(?:us|ms|s)?',fields.get(p,'')) else None for p in props[5:]}}
def rss(pid):
 try:
  status=read('/proc/'+str(pid)+'/status',32768);comm=read('/proc/'+str(pid)+'/comm',128)
  match=re.search(r'^VmRSS:\s+([0-9]+) kB$',status,re.M)
  if not re.fullmatch(r'[A-Za-z0-9_.+-]{1,64}',comm):comm='[non-token-name]'
  return {'pid':pid,'comm':comm,'rssBytes':int(match[1])*1024 if match else None}
 except (OSError,UnicodeError):return None
def kernel_scalar(path):
 try:
  text=read(path,4096)
  return 'max' if text=='max' else uint(text)
 except OSError:return None
def pressure(path):
 try:text=read(path,4096)
 except OSError:return None
 out={}
 for line in text.splitlines():
  parts=line.split()
  if not parts or parts[0] not in {'some','full'}:continue
  fields={}
  for token in parts[1:]:
   key,sep,value=token.partition('=')
   if sep and key in {'avg10','avg60','avg300','total'} and re.fullmatch(r'[0-9]+(?:\.[0-9]+)?',value):
    n=float(value) if '.' in value else int(value)
    if math.isfinite(n):fields[key]=n
  out[parts[0]]=fields
 return out
def main():
 before=unit();mem={}
 for line in read('/proc/meminfo',16384).splitlines():
  match=re.fullmatch(r'(MemTotal|MemAvailable|SwapTotal|SwapFree):\s+([0-9]+) kB',line)
  if match:mem[match[1]+'Bytes']=int(match[2])*1024
 processes=[]
 for path in Path('/proc').iterdir():
  left()
  if path.name.isdecimal():
   row=rss(int(path.name))
   if row is not None:processes.append(row)
 protected=[]
 for pid in [3855184,3855181,2107428]:
  row=rss(pid);protected.append(row or {'pid':pid,'exists':False})
 result={'schema':'mir2.cloud-fresh-readonly-inventory.v1','ok':True,'atUnixMs':round(time.time()*1000),'host':{'logicalCpus':os.cpu_count(),**mem,
         'readableProcessCount':len(processes),'topRss':sorted(processes,key=lambda p:p['rssBytes'] if p['rssBytes'] is not None else -1,reverse=True)[:20]},
         'targetBefore':before,'protectedHistoricalPidPresenceOnly':protected,'serviceOperations':[],'configurationChanges':[]}
 if before['pid']:
  pid=before['pid'];env_path=Path('/proc',str(pid),'environ')
  with env_path.open('rb') as stream:raw=stream.read(262145)
  if len(raw)>262144:raise ValueError('environmentReadBound')
  result['configuredWhitelist']=project_environment(raw);del raw
  executable=Path('/proc',str(pid),'exe');exe_path=str(executable.resolve(strict=True));sha=hashlib.sha256();size=0
  with executable.open('rb') as stream:
   while True:
    left();chunk=stream.read(1048576)
    if not chunk:break
    size+=len(chunk)
    if size>268435456:raise ValueError('executableSizeBound')
    sha.update(chunk)
  result['executable']={'path':exe_path,'bytesHashed':size,'sha256':sha.hexdigest()}
  cg=(Path('/sys/fs/cgroup')/before['controlGroup'].lstrip('/')).resolve(strict=True)
  if not cg.is_relative_to(Path('/sys/fs/cgroup')):raise ValueError('cgroupScope')
  cgroup={name:kernel_scalar(cg/name) for name in ['memory.max','memory.high','memory.current','memory.peak','pids.current','pids.max']}
  for name in ['cpuset.cpus.effective','cpu.max']:
   try:
    text=read(cg/name,4096)
    cgroup[name]=text if re.fullmatch(r'[0-9,\-]+|(?:max|[0-9]+) [0-9]+',text) else None
   except OSError:cgroup[name]=None
  try:
   cgroup['memory.events']={key:uint(value) for key,value in (line.split() for line in read(cg/'memory.events',4096).splitlines()) if key in {'low','high','max','oom','oom_kill','oom_group_kill','sock_throttled'}}
  except OSError:cgroup['memory.events']=None
  cgroup['pressure']={name:pressure(cg/(name+'.pressure')) for name in ['cpu','memory','io']};result['cgroup']=cgroup
  try:
   with urllib.request.urlopen('http://127.0.0.1:7210/health',timeout=left(4)) as response:data=response.read(1048577)
   if len(data)>1048576:raise ValueError('healthReadBound')
   health=json.loads(data);rev=health.get('revision')
   result['health']={'revision':rev if isinstance(rev,str) and re.fullmatch('[0-9a-f]{40}',rev) else None,
       **{key:health.get(key) if type(health.get(key)) in [int,bool] else None for key in ['ok','maxActiveSessions','maxWsConnections','maxReconnectLeases','maxLoginInFlight','maxNewCharacterInFlight','maxStartGameInFlight','currentActiveSessions','currentWsConnections','currentReconnectLeases']}}
  except Exception as error:result['health']={'readComplete':False,'errorClass':type(error).__name__}
 after=unit();left();result.update(targetAfter=after,identityStableAcrossRead=before==after)
 if before!=after:result['ok']=False
 print('MIR2_FRESH_READONLY_INVENTORY_V1 '+json.dumps(result),flush=True)
if __name__=='__main__':
 try:main()
 except Exception as error:print('MIR2_FRESH_READONLY_INVENTORY_V1 '+json.dumps({'schema':'mir2.cloud-fresh-readonly-inventory.v1','ok':False,'errorClass':type(error).__name__}),flush=True);raise SystemExit(1)
'''


def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--output',type=Path,required=True);args=parser.parse_args()
    if not sys.stdin.isatty():parser.error('PTY required; credential echo fallback forbidden')
    warnings.simplefilter('error',getpass.GetPassWarning)
    target=args.output.resolve()
    if target.exists() or not target.parent.is_dir():parser.error('Exclusive new output in existing directory required')
    sys.path.insert(0,str(RUNTIME));import paramiko
    transport=channel=tcp=None;password=None;pin_ok=False;auth_ok=False;status=None;error_class=None;started=time.monotonic();timer=None;stage='connect'
    out=bytearray();err_hash=hashlib.sha256();err_size=0
    def close_owned():
        for owned in (channel,transport,tcp):
            if owned is not None:
                try:owned.close()
                except Exception:pass
    try:
        tcp=socket.create_connection(('165.154.65.136',22),timeout=12);transport=paramiko.Transport(tcp)
        transport.get_security_options().key_types=('ssh-ed25519',);transport.banner_timeout=12;transport.auth_timeout=12;transport.start_client(timeout=12)
        stage='hostKeyPin';key=transport.get_remote_server_key();fingerprint='SHA256:'+base64.b64encode(hashlib.sha256(key.asbytes()).digest()).decode().rstrip('=')
        if key.get_name()!='ssh-ed25519' or not hmac.compare_digest(fingerprint,PIN):raise ValueError('hostKeyPinMismatchBeforeCredentialInput')
        pin_ok=True;print('Pinned ED25519 host verified before credential input.',flush=True)
        stage='credentialInput';password=getpass.getpass('SSH credential (memory-only, no sudo input): ')
        stage='authentication'
        transport.auth_password('ubuntu',password,fallback=False);password=None;auth_ok=transport.is_authenticated()
        if not auth_ok:raise ValueError('authenticationNotConfirmed')
        deadline=time.monotonic()+60
        stage='readonlyQuery';timer=threading.Timer(60,close_owned);timer.daemon=True;timer.start()
        channel=transport.open_session(timeout=12);channel.exec_command('sudo -n -- python3 -u -c '+shlex.quote(REMOTE));channel.shutdown_write();channel.settimeout(0)
        while time.monotonic()<deadline:
            for unused in range(4):
                if not channel.recv_stderr_ready():break
                chunk=channel.recv_stderr(32768);err_size+=len(chunk);err_hash.update(chunk)
            for unused in range(4):
                if not channel.recv_ready():break
                out.extend(channel.recv(32768))
            if len(out)>262144 or err_size>262144:raise ValueError('diagnosticOutputBound')
            if channel.exit_status_ready() and not channel.recv_ready() and not channel.recv_stderr_ready():
                result=channel.recv_exit_status();status=result if result>=0 else None;break
            if channel.closed or not transport.is_active():break
            time.sleep(.02)
        else:raise TimeoutError('localQueryBudget')
    except BaseException as error:error_class=type(error).__name__
    finally:
        password=None
        if timer is not None:timer.cancel()
        close_owned()
    frames=[line for line in bytes(out).splitlines(keepends=True) if line.startswith(MARKER)]
    parsed=None;parse='incompleteOrMissingFrame'
    if len(frames)==1 and frames[0].endswith(b'\n'):
        try:
            parsed=json.loads(frames[0][len(MARKER):]);parse='validFrame' if parsed.get('schema')=='mir2.cloud-fresh-readonly-inventory.v1' else 'schemaMismatch'
        except (ValueError,UnicodeError,AttributeError):parse='invalidJson'
    ok=pin_ok and auth_ok and error_class is None and status==0 and parse=='validFrame' and parsed.get('ok') is True
    record={'schema':'mir2.cloud-readonly-inventory-readback.v1','ok':ok,'queryResult':parsed,'parseCategory':parse,
        'remoteExitStatus':status,'remoteExitStatusReceived':status is not None,'localErrorClass':error_class,
        'lastStage':stage,
        'hostKeyPin':PIN,'hostKeyVerifiedBeforeCredentialInput':pin_ok,'authenticationConfirmed':auth_ok,
        'stdoutBytes':len(out),'stdoutSha256':hashlib.sha256(out).hexdigest(),'stderrBytes':err_size,'stderrSha256':err_hash.hexdigest(),
        'unparsedStdoutPersisted':False,'rawStderrPersisted':False,'credentialsPersisted':False,'remoteStdinBytes':0,
        'remoteQueryMaxSeconds':20,'elapsedWallMs':round((time.monotonic()-started)*1000,3),'remoteCodeSha256':hashlib.sha256(REMOTE.encode()).hexdigest(),
        'readerSourceSha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),'serviceOperations':[],'configurationChanges':[]}
    with target.open('x',encoding='utf-8') as stream:json.dump(record,stream,indent=2);stream.write('\n')
    print(json.dumps({'ok':ok,'localErrorClass':error_class,'remoteExitStatus':status,'outputSha256':hashlib.sha256(target.read_bytes()).hexdigest()}),flush=True)
    return 0 if ok else 1
if __name__=='__main__':raise SystemExit(main())
