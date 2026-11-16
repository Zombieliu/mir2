#!/usr/bin/env python3
"""Readonly SSH sampler. A stop flag drains load clients; no service is stopped."""
import argparse
import base64
import getpass
import hashlib
import hmac
import json
import math
import os
from pathlib import Path
import re
import shlex
import socket
import sys
import threading
import time

HOST = '165.154.65.136'
USER = 'ubuntu'
KNOWN_HOST_KEY = 'SHA256:6M4ouOk6QvFj1qIEmjj9/LOa8/enGiqZ/n+m7JHo7q8'
ORIGINAL_SOURCE_SHA256 = 'e9b3945f5d366e34741cbe9750d5b7b07b47bd1259151da97f07bb574c2a1048'
TRANSPORT_SOURCE_SHA256 = '326bfdc54a8c7af6133d6d9ad22d0f0fe3057db5910467e3f5421d420bbb4456'
MAX_LINE_BYTES = 262144
MAX_STDERR_BYTES = 8388608


def repository_root():
    for anchor in (Path(__file__).resolve().parent, Path.cwd().resolve()):
        for candidate in (anchor, *anchor.parents):
            if (candidate/'mir2-web3/scripts/verify_unified_codeline.py').is_file():
                return candidate
    return None


REPOSITORY = repository_root()
CAPACITY_KEYS = ["currentWsConnections", "currentActiveSessions", "currentReconnectLeases",
                 "currentLoginInFlight", "currentNewCharacterInFlight", "currentStartGameInFlight"]
REMOTE = r'''
import base64,json,os,pathlib,re,subprocess,sys,time,urllib.request
c=json.loads(base64.b64decode(sys.argv[1])); P=pathlib.Path
keys=['currentWsConnections','currentActiveSessions','currentReconnectLeases','currentLoginInFlight','currentNewCharacterInFlight','currentStartGameInFlight']
limits=['maxWsConnections','maxActiveSessions','maxReconnectLeases','maxLoginInFlight','maxNewCharacterInFlight','maxStartGameInFlight']
props=['MainPID','InvocationID','ActiveState','ControlGroup','MemoryCurrent','MemoryMax','MemoryHigh','CPUUsageNSec','TasksCurrent']
def command(args):
 r=subprocess.run(args,stdin=subprocess.DEVNULL,stdout=subprocess.PIPE,stderr=subprocess.DEVNULL,text=True,timeout=2,check=True)
 if len(r.stdout)>32768: raise ValueError('oversize metadata')
 return r.stdout
def unit(name):
 d=dict(x.split('=',1) for x in command(['systemctl','show',name,'--no-pager','--property='+','.join(props)]).splitlines() if '=' in x)
 for k in ['MainPID','MemoryCurrent','MemoryMax','MemoryHigh','CPUUsageNSec','TasksCurrent']:
  d[k]=int(d[k]) if d.get(k,'').isdigit() else None
 return d
def pressure(path):
 try:
  return {row.split()[0]:{k:float(v) if k!='total' else int(v) for k,v in (x.split('=') for x in row.split()[1:])} for row in P(path).read_text().splitlines()}
 except FileNotFoundError:return None
def health(port):
 try:
  with urllib.request.urlopen('http://127.0.0.1:'+str(port)+'/health',timeout=1.5) as r:
   raw=r.read(524289)
  if len(raw)>524288: raise ValueError('oversize health')
  d=json.loads(raw); cap=d.get('capacity',{}); cache=d.get('session_cache',{})
  out={'ok':d.get('ok') is True,'http':d.get('http'),'ws':d.get('ws'),'sessionCacheHealthy':cache.get('healthy'),'revision':str(d.get('revision',''))[:80]}
  out.update({k:cap.get(k) for k in keys+limits})
  z=d.get('zone_metrics',d.get('zoneMetrics',{}))
  if isinstance(z,dict):out['zoneMetricScalars']={k:v for k,v in sorted(z.items()) if isinstance(v,(int,float,bool))}
  if len(out.get('zoneMetricScalars',{}))>32:out['zoneMetricScalars']=dict(list(out['zoneMetricScalars'].items())[:32])
  return out
 except Exception as e:return {'ok':False,'errorClass':type(e).__name__}
def process(pid):
 root=P('/proc')/str(pid); fields=(root/'stat').read_text().rsplit(')',1)[1].split()
 status={x.split(':',1)[0]:x.split(':',1)[1].strip() for x in (root/'status').read_text().splitlines() if ':' in x}
 out={'pid':pid,'startTicks':int(fields[19]),'cpuTicks':int(fields[11])+int(fields[12]),'threads':int(status['Threads'])}
 for k in ['VmRSS','VmHWM','VmSwap']:out[k+'Bytes']=int(status.get(k,'0 kB').split()[0])*1024
 out['io']={k:int(v) for k,v in (x.split(':') for x in (root/'io').read_text().splitlines())}
 return out
def cgroup(group):
 if not isinstance(group,str) or not group.startswith('/') or '..' in group.split('/'):raise ValueError('invalid cgroup')
 root=P('/sys/fs/cgroup')/group.lstrip('/')
 out={}
 for name in ['memory.events','cpu.stat']:
  out[name]={k:int(v) for k,v in (x.split() for x in (root/name).read_text().splitlines())}
 out['memory.current']=int((root/'memory.current').read_text())
 out['memory.pressure']=pressure(root/'memory.pressure')
 try:out['io.stat']=(root/'io.stat').read_text()[:8192]
 except FileNotFoundError:out['io.stat']=None
 return out
def expected_identity():
 return {'pid':c['expectedPid'],'startTicks':c['expectedStartTicks'],'invocationId':c['expectedInvocationId'],'activeState':'active'}
def target_identity():
 target=unit(c['unit']); start_ticks=None
 if target.get('MainPID')==c['expectedPid']:
  fields=(P('/proc')/str(c['expectedPid'])/'stat').read_text().rsplit(')',1)[1].split()
  start_ticks=int(fields[19])
 return {'pid':target.get('MainPID'),'startTicks':start_ticks,'invocationId':target.get('InvocationID'),'activeState':target.get('ActiveState')}
def initialization_identity():
 observed=target_identity()
 if observed!=expected_identity():raise ValueError('target activation differs from pinned identity')
 if health(c['healthPort']).get('revision')!=c['expectedRevision']:raise ValueError('target revision differs from pinned release')
 return observed
def sample(nic):
 mem={x.split(':')[0]:int(x.split()[1])*1024 for x in P('/proc/meminfo').read_text().splitlines() if x.split(':')[0] in ['MemTotal','MemAvailable','SwapTotal','SwapFree']}
 target=unit(c['unit']); old=unit(c['oldUnit'])
 out={'schemaVersion':1,'sampledAt':round(time.time()*1000),'remoteMonotonicNs':time.monotonic_ns(),'stop':False,
      'host':{'memory':mem,'logicalCpus':os.cpu_count(),'cpuTicks':list(map(int,P('/proc/stat').read_text().splitlines()[0].split()[1:9])),
              'pressure':{k:pressure('/proc/pressure/'+k) for k in ['cpu','memory','io']}},
      'targetUnit':target,'oldGatewayUnit':old,'health':health(c['healthPort']),'oldGatewayHealth':health(c['oldHealthPort']),
      'network':{'primaryNic':nic,'rxBytes':int(P('/sys/class/net',nic,'statistics/rx_bytes').read_text()),'txBytes':int(P('/sys/class/net',nic,'statistics/tx_bytes').read_text()),'scope':'one host primary NIC, including cohost traffic; no VPN/virtual-interface summation'}}
 if target['MainPID'] and target['MainPID']>1:
  out['targetProcess']=process(target['MainPID']);out['targetCgroup']=cgroup(target['ControlGroup'])
 if old['MainPID'] and old['MainPID']>1:out['oldGatewayProcess']=process(old['MainPID'])
 out['monitorIdentity']={'expected':expected_identity(),'initializationBefore':initialization_before,'initializationAfter':initialization_after,
  'sampleBefore':{'pid':target.get('MainPID'),'startTicks':out.get('targetProcess',{}).get('startTicks'),'invocationId':target.get('InvocationID'),'activeState':target.get('ActiveState')},
  'sampleAfter':target_identity()}
 return out
start=time.monotonic(); previous=None; initial=None
try:
 initialization_before=initialization_identity()
 nic=c.get('primaryNic')
 if not nic:
  routes=json.loads(command(['ip','-j','route','show','default']))
  routes=sorted([r for r in routes if r.get('dev') and r['dev']!='lo'],key=lambda r:r.get('metric',0))
  if not routes:raise ValueError('no primary route')
  nic=routes[0]['dev']
 if not re.fullmatch(r'[A-Za-z0-9][A-Za-z0-9_.:-]{0,31}',nic):raise ValueError('invalid NIC')
 initialization_after=initialization_identity()
 if initialization_after!=initialization_before:raise ValueError('target activation changed during initialization')
 while True:
  begin=time.monotonic(); s=sample(nic); reasons=[]; t=s['targetUnit']; o=s['oldGatewayUnit']; cg=s.get('targetCgroup',{})
  if t['MainPID']!=c['expectedPid'] or t['ActiveState']!='active':reasons.append('target PID or active state changed')
  if any(s['monitorIdentity'][key]!=expected_identity() for key in ['sampleBefore','sampleAfter']):reasons.append('target pinned activation identity changed')
  if not o['MainPID'] or o['ActiveState']!='active':reasons.append('old Gateway is not active')
  if s['host']['memory'].get('MemAvailable',0)<c['minHostMemoryBytes']:reasons.append('host available memory below reserve')
  maximum=t['MemoryMax']; current=t['MemoryCurrent']
  if not maximum or current is None:reasons.append('finite target memory accounting unavailable')
  elif current>maximum*c['maxTargetMemoryRatio']:reasons.append('target memory above frozen fraction of MemoryMax')
  for h in ['health','oldGatewayHealth']:
   v=s[h]
   if v.get('ok') is not True or v.get('http')!='ready' or v.get('ws')!='ready' or v.get('sessionCacheHealthy') is not True or not all(isinstance(v.get(k),int) and not isinstance(v.get(k),bool) and v[k]>=0 for k in keys):reasons.append(h+' failed')
  if c.get('expectedRevision') and s['health'].get('revision')!=c['expectedRevision']:reasons.append('target revision differs from pinned release')
  if initial:
   if t['InvocationID']!=initial['targetUnit']['InvocationID'] or s.get('targetProcess',{}).get('startTicks')!=initial.get('targetProcess',{}).get('startTicks'):reasons.append('target activation identity changed')
   if o['MainPID']!=initial['oldGatewayUnit']['MainPID'] or o['InvocationID']!=initial['oldGatewayUnit']['InvocationID']:reasons.append('old Gateway activation changed')
   if t['MemoryMax']!=initial['targetUnit']['MemoryMax']:reasons.append('target MemoryMax changed')
   if cg.get('memory.events',{}).get('oom_kill',0)>initial.get('targetCgroup',{}).get('memory.events',{}).get('oom_kill',0):reasons.append('target cgroup OOM kill counter increased')
  else:initial=s
  if previous:
   dt=(s['remoteMonotonicNs']-previous['remoteMonotonicNs'])/1e9
   cpu=[b-a for a,b in zip(previous['host']['cpuTicks'],s['host']['cpuTicks'])]
   if dt<=0 or any(x<0 for x in cpu):raise ValueError('CPU or monotonic counter reversal')
   rates={'hostCpuBusyPercent':100*(sum(cpu)-cpu[3]-cpu[4])/sum(cpu) if sum(cpu)>0 else None}
   for k in ['rxBytes','txBytes']:
    delta=s['network'][k]-previous['network'][k]
    if delta<0:raise ValueError('NIC counter reversal')
    rates[k+'PerSecond']=delta/dt
   if 'targetProcess' in s and 'targetProcess' in previous:
    rates['targetCpuCores']=(s['targetProcess']['cpuTicks']-previous['targetProcess']['cpuTicks'])/os.sysconf('SC_CLK_TCK')/dt
   s['rates']=rates
  if time.monotonic()-start>=c['maxSeconds']:reasons.append('finite monitor duration completed')
  if reasons:s.update(stop=True,reason='; '.join(reasons))
  print(json.dumps(s,separators=(',',':')),flush=True)
  if s['stop']:break
  previous=s;time.sleep(max(0,c['intervalSeconds']-(time.monotonic()-begin)))
except BrokenPipeError:pass
except Exception as e:
 try:print(json.dumps({'schemaVersion':1,'sampledAt':round(time.time()*1000),'stop':True,'reason':'readonly remote sampler failed','errorClass':type(e).__name__}),flush=True)
 except BrokenPipeError:pass
'''


def outside_repository(value):
    result = Path(value).expanduser().resolve()
    if REPOSITORY is None:
        raise ValueError("Run from the canonical workspace so repository output exclusion can be verified")
    if result == REPOSITORY or REPOSITORY in result.parents:
        raise ValueError("Monitor output/control paths must be outside the repository")
    return result


def atomic_write(path, sample):
    temporary = path.with_name(path.name + ".next")
    with temporary.open("w", encoding="utf-8") as stream:
        json.dump(sample, stream, separators=(",", ":")); stream.write("\n")
        stream.flush(); os.fsync(stream.fileno())
    for attempt in range(11):
        try:
            os.replace(temporary, path); return
        except PermissionError:
            if attempt == 10: raise
            time.sleep(.05)


def parse_arguments(argv=None):
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--host", choices=[HOST], default=HOST)
    p.add_argument("--user", choices=[USER], default=USER)
    p.add_argument("--ssh-port", type=int, choices=[22], default=22)
    p.add_argument("--paramiko-runtime", default=str(Path.home()/".codex/tmp/mir2-deploy-ssh-runtime"))
    p.add_argument("--expected-pid", type=int, required=True)
    p.add_argument("--expected-revision", required=True, help="Exact release commit; a mismatch drains clients without controlling services")
    p.add_argument("--expected-invocation-id", required=True)
    p.add_argument("--expected-start-ticks", type=int, required=True)
    p.add_argument("--unit", choices=["mir2-playtest.service"], default="mir2-playtest.service")
    p.add_argument("--old-unit", choices=["mir2-gateway.service", "mir2-windows-gateway.service"], default="mir2-gateway.service")
    p.add_argument("--health-port", type=int, choices=[7210], default=7210)
    p.add_argument("--old-health-port", type=int, choices=[7110], default=7110)
    p.add_argument("--primary-nic")
    p.add_argument("--interval-seconds", type=float, default=2)
    p.add_argument("--max-seconds", type=int, default=1800)
    p.add_argument("--stale-seconds", type=float, default=12)
    p.add_argument("--min-host-memory-bytes", type=int, default=512*1024*1024)
    p.add_argument("--max-target-memory-ratio", type=float, default=.85)
    p.add_argument("--output", required=True)
    p.add_argument("--history", required=True)
    p.add_argument("--stop-file", required=True)
    a = p.parse_args(argv)
    if not (a.expected_pid>1 and 1<=a.max_seconds<=10800 and 1<=a.interval_seconds<=3 and a.interval_seconds*2<a.stale_seconds<15):
        p.error("Require pinned PID, finite <=10800 seconds, 1..3 second interval and a stale budget below 15 seconds")
    if a.min_host_memory_bytes<512*1024*1024 or not (0<a.max_target_memory_ratio<=.85):
        p.error("Protection thresholds may only be tightened")
    if not re.fullmatch(r"[0-9a-f]{40}",a.expected_revision): p.error("Expected revision must be the exact lowercase full Git commit")
    if not re.fullmatch(r"[0-9a-f]{32}",a.expected_invocation_id) or not 0<a.expected_start_ticks<=18446744073709551615:
        p.error("Require exact lowercase invocation ID and positive process start ticks")
    if not all(1<=v<=65535 for v in [a.ssh_port,a.health_port,a.old_health_port]): p.error("Invalid port")
    if a.primary_nic and not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9_.:-]{0,31}",a.primary_nic): p.error("Invalid primary NIC")
    try: a.output,a.history,a.stop_file=map(outside_repository,[a.output,a.history,a.stop_file])
    except ValueError: p.error("Monitor output/control paths must be verifiably outside the canonical workspace")
    if len({a.output,a.history,a.stop_file})!=3: p.error("Output/history/control paths must differ")
    return a


def expected_identity(a):
    return {"pid":a.expected_pid,"startTicks":a.expected_start_ticks,"invocationId":a.expected_invocation_id,"activeState":"active"}


def identity_matches(value, a):
    return isinstance(value,dict) and value==expected_identity(a) and type(value.get("pid")) is int and type(value.get("startTicks")) is int


def validate_sample(sample, a):
    if not isinstance(sample,dict) or type(sample.get("stop")) is not bool:
        raise ValueError("invalid monitor sample")
    at=sample.get("sampledAt")
    age=round(time.time()*1000)-at if type(at) in (int,float) and math.isfinite(at) else float("inf")
    if age>a.stale_seconds*1000 or age< -5000:
        raise ValueError("remote sample timestamp/schema invalid or stale")
    if not sample["stop"] or sample.get("reason")=="finite monitor duration completed":
        identity=sample.get("monitorIdentity")
        if not isinstance(identity,dict) or set(identity)!={"expected","initializationBefore","initializationAfter","sampleBefore","sampleAfter"} or not all(identity_matches(value,a) for value in identity.values()):
            raise ValueError("unverified activation identity")
        target=sample.get("targetUnit"); process=sample.get("targetProcess"); health=sample.get("health")
        if not isinstance(target,dict) or not isinstance(process,dict) or not isinstance(health,dict) or target.get("MainPID")!=a.expected_pid or target.get("InvocationID")!=a.expected_invocation_id or process.get("startTicks")!=a.expected_start_ticks or health.get("revision")!=a.expected_revision:
            raise ValueError("sample activation fields mismatch")


def verify_host_key(key):
    fingerprint='SHA256:'+base64.b64encode(hashlib.sha256(key.asbytes()).digest()).decode().rstrip('=')
    if key.get_name()!='ssh-ed25519' or not hmac.compare_digest(fingerprint,KNOWN_HOST_KEY):
        raise ValueError("host key mismatch before authentication")


def main(argv=None):
    a = parse_arguments(argv)
    for path in [a.output,a.history]:
        if path.exists() or path.with_name(path.name+".next").exists(): raise ValueError("Preserve existing monitor records; use fresh paths")
        path.parent.mkdir(parents=True,exist_ok=True)
    if a.stop_file.exists(): raise ValueError("Stop-file already exists; use a fresh run")
    if not sys.stdin.isatty(): raise ValueError("Local PTY required for memory-only getpass")
    history = a.history.open("x",encoding="utf-8")
    transport=channel=tcp=None; password=None; count=0; latest={}
    reason="monitor did not start"; outcome="failure"; completed_by=None
    stdout_bytes=stderr_bytes=0; stdout_hash=hashlib.sha256(); stderr_hash=hashlib.sha256()
    stdout_bound=MAX_LINE_BYTES*(math.ceil(a.max_seconds/a.interval_seconds)+3)
    remote_exit=None; exit_return=None; status_received=False; remote_terminal=False; pin_verified=False
    stream_end="notStarted"; buffer=b""; guard_done=threading.Event(); guard_reason=None
    watch={"streamStart":None,"last":None}; setup_deadline=None
    def persist(sample):
        history.write(json.dumps(sample,separators=(",", ":"),allow_nan=False)+"\n"); history.flush()
        atomic_write(a.output,sample)
    def close_owned():
        if tcp is not None:
            try: tcp.shutdown(socket.SHUT_RDWR)
            except OSError: pass
            try: tcp.close()
            except OSError: pass
        for resource in (transport,channel):
            if resource is not None:
                try: resource.close()
                except Exception: pass
    def read_status():
        nonlocal exit_return,remote_exit,status_received
        if channel is not None and not status_received and channel.exit_status_ready():
            exit_return=channel.recv_exit_status()
            if exit_return>=0: remote_exit=exit_return; status_received=True
    def budget_guard():
        nonlocal guard_reason
        while not guard_done.wait(.1):
            now=time.monotonic()
            if a.stop_file.exists(): guard_reason="local stop-file requested normal monitor completion"
            elif watch["streamStart"] is None:
                if setup_deadline is None or now<setup_deadline: continue
                guard_reason="SSH setup deadline exceeded"
            elif now-watch["streamStart"]>=a.max_seconds: guard_reason="finite monitor duration completed"
            elif now-watch["last"]>a.stale_seconds: guard_reason="remote sample stream stale or interrupted"
            else: continue
            close_owned(); return
    persist({"schemaVersion":1,"sampledAt":round(time.time()*1000),"stop":True,"reason":"monitor starting; not ready for admission"})
    try:
        sys.path.insert(0,str(Path(a.paramiko_runtime).expanduser().resolve()))
        import paramiko
        password=getpass.getpass("SSH password (memory only; never sent to sudo): ")
        setup_deadline=time.monotonic()+20
        threading.Thread(target=budget_guard,name="monitor-ssh-budget",daemon=True).start()
        def remaining(cap=20):
            if guard_reason is not None: raise TimeoutError("owned monitor connection interrupted")
            left=setup_deadline-time.monotonic()
            if left<=0: raise TimeoutError("SSH setup deadline exceeded")
            return min(cap,left)
        tcp=socket.create_connection((HOST,22),timeout=remaining())
        transport=paramiko.Transport(tcp)
        transport.get_security_options().key_types=('ssh-ed25519',)
        transport.banner_timeout=remaining(); transport.auth_timeout=remaining(); transport.start_client(timeout=remaining())
        verify_host_key(transport.get_remote_server_key()); pin_verified=True
        transport.auth_timeout=remaining(); transport.auth_password(USER,password,fallback=False); password=None
        if not transport.is_authenticated(): raise ValueError("SSH authentication failed")
        config={"unit":a.unit,"oldUnit":a.old_unit,"expectedPid":a.expected_pid,"expectedRevision":a.expected_revision,
                "expectedInvocationId":a.expected_invocation_id,"expectedStartTicks":a.expected_start_ticks,
                "healthPort":a.health_port,"oldHealthPort":a.old_health_port,"primaryNic":a.primary_nic,
                "maxSeconds":a.max_seconds,"intervalSeconds":a.interval_seconds,
                "minHostMemoryBytes":a.min_host_memory_bytes,"maxTargetMemoryRatio":a.max_target_memory_ratio}
        encoded=base64.b64encode(json.dumps(config).encode()).decode()
        channel=transport.open_session(timeout=remaining()); channel.settimeout(remaining())
        channel.exec_command("sudo -n -- python3 -u -c "+shlex.quote(REMOTE)+" "+shlex.quote(encoded))
        channel.shutdown_write(); channel.settimeout(0.0)
        start=last=time.monotonic(); watch.update(streamStart=start,last=last); stream_end="sampling"
        while True:
            now=time.monotonic()
            if a.stop_file.exists(): reason="local stop-file requested normal monitor completion";outcome="completed";completed_by="localStopFile";stream_end="localStopFile";break
            if now-start>=a.max_seconds: reason="finite monitor duration completed";outcome="completed";completed_by="localDuration";stream_end="localDuration";break
            if now-last>a.stale_seconds: reason="remote sample stream stale or interrupted";stream_end="localStale";break
            if channel.recv_stderr_ready():
                chunk=channel.recv_stderr(4096); stderr_bytes+=len(chunk); stderr_hash.update(chunk)
                if stderr_bytes>MAX_STDERR_BYTES: reason="remote stderr exceeded finite bound";stream_end="stderrBound";break
            if channel.recv_ready():
                chunk=channel.recv(65536); stdout_bytes+=len(chunk); stdout_hash.update(chunk)
                if stdout_bytes>stdout_bound: reason="remote stdout exceeded finite bound";stream_end="stdoutBound";break
                buffer+=chunk
                if len(buffer)>MAX_LINE_BYTES: reason="remote sample exceeded finite bound";stream_end="lineBound";break
                while b"\n" in buffer:
                    line,buffer=buffer.split(b"\n",1)
                    def reject_constant(unused): raise ValueError("non-finite sample value")
                    sample=json.loads(line,parse_constant=reject_constant)
                    validate_sample(sample,a)
                    last=time.monotonic();watch["last"]=last;latest=sample;count+=1;persist(sample)
                    if sample["stop"]:
                        remote_terminal=True;stream_end="remoteTerminalSample"
                        reason=sample.get("reason","remote guard requested drain")
                        outcome="completed" if reason=="finite monitor duration completed" else "guard-stop"
                        completed_by="remoteFiniteTerminal" if outcome=="completed" else None
                        break
                else:
                    if channel.exit_status_ready() and not channel.recv_ready() and not channel.recv_stderr_ready():
                        read_status();reason="remote producer exited without terminal sample";stream_end="remoteExitWithoutTerminal";break
                    continue
                break
            if channel.exit_status_ready() and not channel.recv_ready() and not channel.recv_stderr_ready():
                read_status();reason="remote producer exited without terminal sample";stream_end="remoteExitWithoutTerminal";break
            if not channel.recv_ready() and not channel.recv_stderr_ready() and (channel.closed or not transport.is_active()):
                reason="remote producer exited without terminal sample";stream_end="closedWithoutTerminal";break
            time.sleep(.05)
    except KeyboardInterrupt:
        reason="local interrupt requested normal monitor completion";outcome="completed";completed_by="localInterrupt";stream_end="localInterrupt"
    except Exception as error:
        if guard_reason is not None:
            reason=guard_reason
            if reason in {"local stop-file requested normal monitor completion","finite monitor duration completed"}:
                outcome="completed";completed_by="localStopFile" if reason.startswith("local stop-file") else "localDuration"
        else: reason="monitor failure: "+type(error).__name__
        stream_end="localException"
    finally:
        password=None;guard_done.set()
        try: read_status()
        except Exception: pass
        if completed_by=="remoteFiniteTerminal" and status_received and remote_exit!=0:
            reason="remote producer exited nonzero after terminal sample";outcome="failure";completed_by=None
        terminal={**latest,"schemaVersion":1,"sampledAt":round(time.time()*1000),"stop":True,"reason":reason,
                  "monitorTerminal":True,"sampleCount":count,"outcome":outcome,"servicesStopped":False,
                  "monitorReadback":{"expectedIdentity":expected_identity(a),"expectedRevision":a.expected_revision,
                   "hostKeyPin":KNOWN_HOST_KEY,"hostKeyVerifiedBeforeAuthentication":pin_verified,
                   "remoteExitStatus":remote_exit,"remoteExitStatusReceived":status_received,"exitStatusMethodReturn":exit_return,
                   "remoteTerminalSampleObserved":remote_terminal,"completedBy":completed_by,"streamEndReason":stream_end,
                   "stdoutBytesReceived":stdout_bytes,"stdoutSha256":stdout_hash.hexdigest(),"stdoutBytesBound":stdout_bound,
                   "stderrBytesReceived":stderr_bytes,"stderrSha256":stderr_hash.hexdigest(),"stderrBytesBound":MAX_STDERR_BYTES,
                   "rawStdoutPersisted":False,"rawStderrPersisted":False,"credentialsPersisted":False,"remoteStdinPasswordBytesSent":0,
                   "originalMonitorSourceSha256":ORIGINAL_SOURCE_SHA256,"transportSourceSha256":TRANSPORT_SOURCE_SHA256}}
        try: persist(terminal)
        finally:
            close_owned();history.close()
    print(json.dumps({"output":str(a.output),"history":str(a.history),"samples":count,"outcome":outcome,"reason":reason,"servicesStopped":False}))
    return 0 if outcome=="completed" else 2


if __name__=="__main__":
    try: sys.exit(main())
    except Exception as error:
        print(json.dumps({"errorClass":type(error).__name__,"servicesStopped":False}),file=sys.stderr)
        sys.exit(2)
