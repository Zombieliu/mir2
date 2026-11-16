#!/usr/bin/env python3
"""Read-only TCP companion; no load guard, packet capture or service control."""
# REMOTE-BEGIN
import base64,hashlib,hmac,ipaddress,json,math,os,re,select,shutil,subprocess,sys,time,urllib.request
from pathlib import Path
HOST='165.154.65.136'; USER='ubuntu'
HOST_KEY='SHA256:6M4ouOk6QvFj1qIEmjj9/LOa8/enGiqZ/n+m7JHo7q8'
SCHEMA='mir2.cloud-tcp-readonly.v1'; MAX_LINE=262144; MAX_SOCKETS=512; MAX_SS=1048576
MAX_TOTAL=268435456; MAX_STDERR=65536
SS_ARGS=['ss','-Htinpe','state','established','( sport = :443 or sport = :7210 or dport = :7210 )']
UNIT_ARGS=['systemctl','show','mir2-playtest.service','--no-pager','--property=MainPID,InvocationID,ActiveState']
COUNTERS={'Tcp':['RetransSegs','InSegs','OutSegs','InErrs','OutRsts','AttemptFails','EstabResets'],
 'TcpExt':['TCPTimeouts','TCPSynRetrans','TCPFastRetrans','TCPSlowStartRetrans','TCPRetransFail','TCPSpuriousRTOs','TCPLostRetransmit','TCPAbortOnTimeout','TCPBacklogDrop','TCPRcvQDrop']}
METRICS={'rto':'rtoMs','bytes_sent':'bytesSent','bytes_acked':'bytesAcked','bytes_received':'bytesReceived','bytes_retrans':'bytesRetransmitted','notsent':'notSentBytes','segs_out':'segmentsOut','segs_in':'segmentsIn','data_segs_out':'dataSegmentsOut','data_segs_in':'dataSegmentsIn'}
METRIC_NAMES=list(METRICS.values())+['rttMs','rttVariationMs','retransOutstanding','retransTotal']
REASONS={'duration_completed','identity_changed','ss_unavailable','read_command_failed','read_timeout','read_bound','proc_unavailable','counter_format','ss_format','socket_bound','projection_bound','remote_read_failed','counter_reversed'}
class DiagnosticError(ValueError): pass
def need(ok,code):
 if not ok:raise DiagnosticError(code)
def integer(v):return type(v) is int and 0<=v<=18446744073709551615
def validate_pins(p):
 need(type(p) is dict and set(p)=={'schema','templateOnly','rootReviewedIdentityPins','target','caddy','provenance'},'identity_changed')
 need(p['schema']=='mir2.cloud-tcp-readonly-pins.v1' and p['templateOnly'] is False and p['rootReviewedIdentityPins'] is True,'identity_changed')
 t=p['target']; c=p['caddy']; v=p['provenance']
 need(type(t) is dict and set(t)=={'pid','startTicks','invocationId','revision'} and type(c) is dict and set(c)=={'pid','startTicks'},'identity_changed')
 need(all(integer(q['pid']) and q['pid']>1 and integer(q['startTicks']) and q['startTicks']>0 for q in [t,c]) and t['pid']!=c['pid'],'identity_changed')
 need(type(t['invocationId']) is str and re.fullmatch('[0-9a-f]{32}',t['invocationId']) and type(t['revision']) is str and re.fullmatch('[0-9a-f]{40}',t['revision']),'identity_changed')
 need(type(v) is dict and set(v)=={'statusReceiptSha256','statusSourceSha256'} and all(type(z) is str and re.fullmatch('[0-9a-f]{64}',z) for z in v.values()),'identity_changed')
 return p

def endpoint(value):
 try:
  host,port=value.rsplit(':',1);address=ipaddress.ip_address(host.strip('[]'));port=int(port)
  if isinstance(address,ipaddress.IPv6Address) and address.ipv4_mapped:address=address.ipv4_mapped
  need(0<port<65536,'ss_format');return str(address),port,address.is_loopback
 except (ValueError,TypeError):raise DiagnosticError('ss_format')

def numeric(text,key):
 matches=re.findall(r'(?:^|\s)'+re.escape(key)+r':([^\s]+)',text)
 if not matches:return None
 need(len(matches)==1 and re.fullmatch(r'[0-9]+(?:\.[0-9]+)?',matches[0]),'ss_format')
 value=float(matches[0]) if '.' in matches[0] else int(matches[0]);need(math.isfinite(value) and 0<=value<=18446744073709551615,'ss_format');return value

def project_ss(raw,pins,salt):
 need(type(raw) is str and len(raw.encode())<=MAX_SS and type(salt) is bytes and len(salt)==32,'read_bound')
 rows=[];header=None;detail=[];unowned=foreign=0
 def finish():
  nonlocal unowned,foreign
  if header is None:return
  recvq,sendq,local,peer,tail=header;lh,lp,ll=endpoint(local);ph,pp,pl=endpoint(peer);text=' '.join([tail,*detail])
  owners=set(int(v) for v in re.findall(r'\bpid=(\d+)',text));role=None
  if lp==443:
   if owners=={pins['caddy']['pid']}:role='caddyPublic443'
  elif ll and pl and lp==7210:
   if owners=={pins['target']['pid']}:role='targetLoopback7210'
  elif ll and pl and pp==7210:
   if owners=={pins['caddy']['pid']}:role='caddyLoopback7210'
  else:raise DiagnosticError('ss_format')
  if role is None:
   if not owners:unowned+=1
   else:foreign+=1
   return
  inode=re.findall(r'\bino:(\d+)',text);need(len(inode)<=1,'ss_format');ino=inode[0] if inode else 'unavailable'
  l=lh+':'+str(lp);p=ph+':'+str(pp)
  digest=lambda x:hmac.new(salt,x.encode(),hashlib.sha256).hexdigest()
  row={'role':role,'socketId':digest('socket|'+l+'|'+p+'|'+ino),'flowId':digest('flow|'+'|'.join(sorted([l,p]))),'recvQBytes':recvq,'sendQBytes':sendq}
  row.update({out:numeric(text,key) for key,out in METRICS.items()})
  need(all(row[k] is None or integer(row[k]) for k in METRICS.values() if k!='rtoMs'),'ss_format')
  rtt=re.findall(r'(?:^|\s)rtt:([^\s]+)',text);rt=re.findall(r'(?:^|\s)retrans:([^\s]+)',text)
  need(len(rtt)<=1 and len(rt)<=1,'ss_format')
  row.update(rttMs=None,rttVariationMs=None,retransOutstanding=None,retransTotal=None)
  if rtt:
   match=re.fullmatch(r'([0-9]+(?:\.[0-9]+)?)/([0-9]+(?:\.[0-9]+)?)',rtt[0]);need(match is not None,'ss_format')
   row.update(rttMs=float(match[1]),rttVariationMs=float(match[2]))
  if rt:
   match=re.fullmatch(r'(\d+)/(\d+)',rt[0]);need(match is not None,'ss_format');row.update(retransOutstanding=int(match[1]),retransTotal=int(match[2]))
  row['diagnosticComplete']=all(row[k] is not None for k in ['rttMs','rtoMs','retransTotal','bytesSent','bytesReceived']) and bool(inode)
  need(all(v is None or integer(v) or type(v) is float and math.isfinite(v) and 0<=v<=18446744073709551615 for k,v in row.items() if k in METRIC_NAMES),'ss_format')
  rows.append(row);need(len(rows)<=MAX_SOCKETS,'socket_bound')
 for line in raw.splitlines():
  fields=line.split()
  if not fields:continue
  if fields[0] in ['tcp','tcp6']:fields=fields[1:]
  if fields and fields[0]=='ESTAB':fields=fields[1:]
  if len(fields)>=4 and fields[0].isdigit() and fields[1].isdigit():
   finish();header=(int(fields[0]),int(fields[1]),fields[2],fields[3],' '.join(fields[4:]));detail=[]
  else:
   need(header is not None and line[0].isspace(),'ss_format');detail.append(line.strip())
 finish();return {'sockets':rows,'unownedSelectedHeaders':unowned,'foreignOwnerSelectedHeadersExcluded':foreign,'diagnosticComplete':unowned==0 and foreign==0 and all(x['diagnosticComplete'] for x in rows)}

def proc_counters(raw,group):
 lines=raw.splitlines();pairs=[]
 for i,line in enumerate(lines):
  if line.startswith(group+':'):
   if all(re.fullmatch(r'-?\d+',x) for x in line.split()[1:]):continue
   need(i+1<len(lines) and lines[i+1].startswith(group+':'),'counter_format')
   names=line.split()[1:];values=lines[i+1].split()[1:]
   if names and all(re.fullmatch(r'-?\d+',x) for x in names):continue
   need(len(names)==len(values) and len(set(names))==len(names),'counter_format');pairs.append(dict(zip(names,values)))
 need(len(pairs)==1,'counter_format');values=pairs[0];out={}
 for key in COUNTERS[group]:
  v=values.get(key);need(v is None or v.isdigit(),'counter_format');out[key]=int(v) if v is not None else None
 return out

def read_small(path,bound):
 try:
  with open(path,'rb') as f:data=f.read(bound+1)
  need(len(data)<=bound,'read_bound');return data.decode('utf8')
 except (OSError,UnicodeError):raise DiagnosticError('proc_unavailable')

def readonly_command(argv,deadline):
 need(argv in [SS_ARGS,UNIT_ARGS],'read_command_failed');left=min(1.5,deadline-time.monotonic());need(left>0,'read_timeout')
 try:
  p=subprocess.Popen(argv,stdin=subprocess.DEVNULL,stdout=subprocess.PIPE,stderr=subprocess.DEVNULL,bufsize=0)
 except FileNotFoundError:raise DiagnosticError('ss_unavailable' if argv==SS_ARGS else 'read_command_failed')
 data=bytearray();end=time.monotonic()+left;os.set_blocking(p.stdout.fileno(),False)
 try:
  while True:
   need(time.monotonic()<end,'read_timeout')
   if select.select([p.stdout],[],[],min(.05,max(0,end-time.monotonic())))[0]:
    chunk=os.read(p.stdout.fileno(),65536)
    if chunk:data.extend(chunk);need(len(data)<=(MAX_SS if argv==SS_ARGS else 8192),'read_bound')
    elif p.poll() is not None:break
   elif p.poll() is not None:
    chunk=os.read(p.stdout.fileno(),65536)
    if chunk:data.extend(chunk);need(len(data)<=(MAX_SS if argv==SS_ARGS else 8192),'read_bound')
    else:break
  need(p.returncode==0,'read_command_failed');return bytes(data).decode('utf8')
 finally:
  # Cancels only this tool's owned read-only helper on timeout/bound, never a service/client.
  if p.poll() is None:p.kill();p.wait(timeout=1)
  p.stdout.close()

class NoRedirect(urllib.request.HTTPRedirectHandler):
 def redirect_request(self,*args,**kwargs):return None

def identity(pins,deadline):
 text=readonly_command(UNIT_ARGS,deadline);rows=[x.split('=',1) for x in text.splitlines() if '=' in x];need(len(rows)==3 and len({x[0] for x in rows})==3,'identity_changed');u=dict(rows);target=pins['target'];caddy=pins['caddy']
 need(u.get('MainPID')==str(target['pid']) and u.get('InvocationID')==target['invocationId'] and u.get('ActiveState')=='active','identity_changed')
 for value in [target,caddy]:
  stat=read_small('/proc/'+str(value['pid'])+'/stat',8192);fields=stat.rsplit(')',1)[1].split();need(len(fields)>19 and fields[19].isdigit() and int(fields[19])==value['startTicks'],'identity_changed')
 need(read_small('/proc/'+str(caddy['pid'])+'/comm',128).strip()=='caddy','identity_changed')
 remain=min(.75,deadline-time.monotonic());need(remain>0,'read_timeout');opener=urllib.request.build_opener(urllib.request.ProxyHandler({}),NoRedirect())
 try:
  with opener.open('http://127.0.0.1:7210/health',timeout=remain) as response:raw=response.read(524289)
  need(len(raw)<=524288,'read_bound');h=json.loads(raw);need(type(h) is dict and h.get('revision')==target['revision'],'identity_changed')
 except DiagnosticError:raise
 except Exception:raise DiagnosticError('identity_changed')
 return {'target':dict(target),'caddy':dict(caddy)}

def emit_remote(row):
 raw=(json.dumps(row,separators=(',',':'),allow_nan=False)+'\n').encode();need(len(raw)<=MAX_LINE,'projection_bound');sys.stdout.buffer.write(raw);sys.stdout.buffer.flush()

def remote_main(config):
 pins=validate_pins(config['pins']);seconds=config['maxSeconds'];need(type(seconds) is int and 1<=seconds<=10800,'read_bound')
 start=time.monotonic();deadline=start+seconds;salt=os.urandom(32);previous=None;index=0;complete=True;first=None
 try:
  need(shutil.which('ss') is not None,'ss_unavailable')
  while time.monotonic()<deadline:
   began=time.monotonic();sample_deadline=min(deadline,began+5);before=identity(pins,sample_deadline)
   totals={'Tcp':proc_counters(read_small('/proc/net/snmp',262144),'Tcp'),'TcpExt':proc_counters(read_small('/proc/net/netstat',262144),'TcpExt')}
   projection=project_ss(readonly_command(SS_ARGS,sample_deadline),pins,salt);after=identity(pins,sample_deadline);need(before==after,'identity_changed')
   deltas={g:{k:None for k in names} for g,names in COUNTERS.items()}
   if previous:
    for group,values in totals.items():
     for key,value in values.items():
      prior=previous[group][key]
      if value is not None and prior is not None:need(value>=prior,'counter_reversed');deltas[group][key]=value-prior
   index+=1;timestamp=round(time.time()*1000);first=timestamp if first is None else first
   ok=projection['diagnosticComplete'] and all(v is not None for group in totals.values() for v in group.values());complete=complete and ok
   emit_remote({'schema':SCHEMA,'kind':'sample','sampleIndex':index,'sampledAt':timestamp,'remoteMonotonicNs':time.monotonic_ns(),'collectionDurationMs':round((time.monotonic()-began)*1000,3),'identityBefore':before,'identityAfter':after,'kernelTotals':totals,'kernelDeltas':deltas,**projection,'diagnosticComplete':ok})
   previous=totals;time.sleep(max(0,min(2-(time.monotonic()-began),deadline-time.monotonic())))
  emit_remote({'schema':SCHEMA,'kind':'terminal','sampledAt':round(time.time()*1000),'reason':'duration_completed','sampleCount':index,'firstSampledAt':first,'diagnosticComplete':complete})
 except BrokenPipeError:return
 except Exception as e:
  code=str(e) if type(e) is DiagnosticError and str(e) in REASONS else 'remote_read_failed'
  try:emit_remote({'schema':SCHEMA,'kind':'terminal','sampledAt':round(time.time()*1000),'reason':code,'sampleCount':index,'firstSampledAt':first,'diagnosticComplete':False})
  except BrokenPipeError:pass
  raise SystemExit(2)
# REMOTE-END

import argparse,getpass,socket,threading
SELF_TEXT=Path(__file__).read_text(encoding='utf8')
REMOTE=SELF_TEXT.split('# REMOTE-BEGIN\n',1)[1].split('# REMOTE-END\n',1)[0]+'\nremote_main(json.loads(base64.b64decode(sys.argv[1])))\n'

def verify_host_key(key):
 need(key.get_name()=='ssh-ed25519','identity_changed');fingerprint='SHA256:'+base64.b64encode(hashlib.sha256(key.asbytes()).digest()).decode().rstrip('=');need(hmac.compare_digest(fingerprint,HOST_KEY),'identity_changed')

def authenticate_pinned(transport,password):
 verify_host_key(transport.get_remote_server_key());transport.auth_password(USER,password,fallback=False);need(transport.is_authenticated(),'identity_changed')

def repository_root():
 for anchor in [Path(__file__).resolve().parent,Path.cwd().resolve()]:
  for p in [anchor,*anchor.parents]:
   if (p/'mir2-web3/scripts/verify_unified_codeline.py').is_file():return p
 return None

def outside(value):
 p=Path(value).expanduser().resolve();r=repository_root();need(r is not None and p!=r and r not in p.parents,'read_bound')
 need(os.name!='nt' or p.drive.upper()=='C:','read_bound');return p

def validate_row(x,pins):
 need(type(x) is dict and x.get('schema')==SCHEMA and type(x.get('sampledAt')) is int and abs(time.time()*1000-x['sampledAt'])<15000,'ss_format')
 if x.get('kind')=='terminal':
  need(set(x)=={'schema','kind','sampledAt','reason','sampleCount','firstSampledAt','diagnosticComplete'} and x['reason'] in REASONS and integer(x['sampleCount']) and type(x['diagnosticComplete']) is bool and (x['firstSampledAt'] is None or integer(x['firstSampledAt'])),'ss_format');return x
 need(set(x)=={'schema','kind','sampleIndex','sampledAt','remoteMonotonicNs','collectionDurationMs','identityBefore','identityAfter','kernelTotals','kernelDeltas','sockets','unownedSelectedHeaders','foreignOwnerSelectedHeadersExcluded','diagnosticComplete'} and x['kind']=='sample','ss_format')
 expect={'target':pins['target'],'caddy':pins['caddy']};need(x['identityBefore']==expect and x['identityAfter']==expect,'identity_changed')
 need(integer(x['sampleIndex']) and x['sampleIndex']>0 and integer(x['remoteMonotonicNs']) and type(x['collectionDurationMs']) in [int,float] and math.isfinite(x['collectionDurationMs']) and 0<=x['collectionDurationMs']<=6000,'ss_format')
 for field in ['kernelTotals','kernelDeltas']:
  need(type(x[field]) is dict and set(x[field])==set(COUNTERS),'counter_format')
  for group,keys in COUNTERS.items():need(type(x[field][group]) is dict and set(x[field][group])==set(keys) and all(v is None or integer(v) for v in x[field][group].values()),'counter_format')
 need(type(x['sockets']) is list and len(x['sockets'])<=MAX_SOCKETS,'socket_bound')
 keys={'role','socketId','flowId','recvQBytes','sendQBytes','diagnosticComplete',*METRIC_NAMES}
 for row in x['sockets']:
  need(type(row) is dict and set(row)==keys and row['role'] in ['caddyPublic443','caddyLoopback7210','targetLoopback7210'],'ss_format')
  need(all(type(row[k]) is str and re.fullmatch('[0-9a-f]{64}',row[k]) for k in ['socketId','flowId']) and integer(row['recvQBytes']) and integer(row['sendQBytes']) and type(row['diagnosticComplete']) is bool,'ss_format')
  need(all(v is None or integer(v) or type(v) is float and math.isfinite(v) and 0<=v<=18446744073709551615 for k,v in row.items() if k in METRIC_NAMES),'ss_format')
 need(type(x['diagnosticComplete']) is bool and integer(x['unownedSelectedHeaders']) and integer(x['foreignOwnerSelectedHeadersExcluded']),'ss_format')
 if x['diagnosticComplete']:need(x['unownedSelectedHeaders']==x['foreignOwnerSelectedHeadersExcluded']==0 and all(row['diagnosticComplete'] for row in x['sockets']) and all(v is not None for group in x['kernelTotals'].values() for v in group.values()),'ss_format')
 for row in x['sockets']:
  if row['diagnosticComplete']:need(all(row[k] is not None for k in ['rttMs','rtoMs','retransTotal','bytesSent','bytesReceived']),'ss_format')
 return x

def parse_args(argv=None):
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--pins',required=True);p.add_argument('--output',required=True);p.add_argument('--history',required=True);p.add_argument('--stop-file',required=True);p.add_argument('--max-seconds',type=int,default=1800);p.add_argument('--paramiko-runtime',default=str(Path.home()/'.codex/tmp/mir2-deploy-ssh-runtime'));a=p.parse_args(argv)
 need(1<=a.max_seconds<=10800,'read_bound');a.pins=outside(a.pins);a.output=outside(a.output);a.history=outside(a.history);a.stop_file=outside(a.stop_file);need(len({a.pins,a.output,a.history,a.stop_file})==4,'read_bound');return a

def atomic(path,row):
 raw=(json.dumps(row,separators=(',',':'),allow_nan=False)+'\n').encode();temporary=path.with_name(path.name+'.next')
 with temporary.open('xb') as f:f.write(raw);f.flush();os.fsync(f.fileno())
 for n in range(11):
  try:os.replace(temporary,path);break
  except PermissionError:
   if n==10:raise
   time.sleep(.05)

def main(argv=None):
 a=parse_args(argv);pins_raw=a.pins.read_bytes();need(len(pins_raw)<=8192,'read_bound');pins=validate_pins(json.loads(pins_raw))
 for q in [a.output,a.history,a.stop_file,a.output.with_name(a.output.name+'.next')]:need(not q.exists(),'read_bound')
 need(sys.stdin.isatty(),'read_bound');a.output.parent.mkdir(parents=True,exist_ok=True);a.history.parent.mkdir(parents=True,exist_ok=True)
 history=a.history.open('xb');tcp=transport=channel=None;password=None;guard_done=threading.Event();watch={'setup':None,'start':None,'last':None,'reason':None};remote_exit=None;status_received=False;pin_ok=False;count=0;first=None;last=None;all_complete=True;remote_terminal=None;reason='remote_read_failed';written=0;stderr_bytes=0;stderr_sha=hashlib.sha256();buffer=b''
 def close_owned():
  for q in [channel,transport,tcp]:
   if q is not None:
    try:q.close()
    except Exception:pass
 def guard():
  while not guard_done.wait(.1):
   now=time.monotonic();code=None
   if a.stop_file.exists():code='local_stop_file'
   elif watch['start'] is None and watch['setup'] is not None and now>=watch['setup']:code='setup_timeout'
   elif watch['start'] is not None and now-watch['start']>a.max_seconds+6:code='local_duration_bound'
   elif watch['last'] is not None and now-watch['last']>12:code='stream_stale'
   if code:watch['reason']=code;close_owned();return
 def persist(row):
  nonlocal written
  raw=(json.dumps(row,separators=(',',':'),allow_nan=False)+'\n').encode();need(len(raw)<=MAX_LINE and written+len(raw)<=(MAX_TOTAL if row.get('kind')=='localTerminal' else MAX_TOTAL-MAX_LINE),'read_bound');history.write(raw);history.flush();written+=len(raw);atomic(a.output,row)
 try:
  sys.path.insert(0,str(Path(a.paramiko_runtime).expanduser().resolve()));import paramiko
  password=getpass.getpass('SSH password (memory only; never sent to sudo): ');watch['setup']=time.monotonic()+20;threading.Thread(target=guard,daemon=True).start()
  def remaining():
   need(watch['reason'] is None,'read_timeout');left=watch['setup']-time.monotonic();need(left>0,'read_timeout');return left
  tcp=socket.create_connection((HOST,22),timeout=remaining());transport=paramiko.Transport(tcp);transport.get_security_options().key_types=('ssh-ed25519',);transport.banner_timeout=remaining();transport.auth_timeout=remaining();transport.start_client(timeout=remaining());authenticate_pinned(transport,password);pin_ok=True;password=None
  channel=transport.open_session(timeout=remaining());channel.settimeout(remaining());encoded=base64.b64encode(json.dumps({'pins':pins,'maxSeconds':a.max_seconds}).encode()).decode()
  import shlex
  channel.exec_command('sudo -n -- python3 -B -u -c '+shlex.quote(REMOTE)+' '+shlex.quote(encoded));channel.shutdown_write();channel.settimeout(0.0);watch['start']=watch['last']=time.monotonic()
  while True:
   if a.stop_file.exists():reason='local_stop_file';break
   if watch['reason']:reason=watch['reason'];break
   if channel.recv_stderr_ready():
    data=channel.recv_stderr(4096);stderr_bytes+=len(data);stderr_sha.update(data);need(stderr_bytes<=MAX_STDERR,'read_bound')
   if channel.recv_ready():
    buffer+=channel.recv(65536);need(len(buffer)<=MAX_LINE+65536,'read_bound')
    while b'\n' in buffer:
     line,buffer=buffer.split(b'\n',1);need(len(line)<=MAX_LINE,'read_bound');x=json.loads(line,parse_constant=lambda v:(_ for _ in ()).throw(DiagnosticError('ss_format')));x=validate_row(x,pins);watch['last']=time.monotonic()
     if x['kind']=='terminal':
      need(x['sampleCount']==count and x['firstSampledAt']==first,'ss_format');remote_terminal=x;reason=x['reason'];break
     need(x['sampleIndex']==count+1,'ss_format');count+=1;first=x['sampledAt'] if first is None else first;last=x['sampledAt'];all_complete=all_complete and x['diagnosticComplete'];persist({**x,'localObservedAt':round(time.time()*1000)})
    if remote_terminal is not None:break
   if channel.exit_status_ready() and not channel.recv_ready() and not channel.recv_stderr_ready():reason='remote_without_terminal';break
   if channel.closed or not transport.is_active():reason='remote_without_terminal';break
   time.sleep(.05)
 except KeyboardInterrupt:reason='local_interrupt'
 except Exception:reason=watch['reason'] or 'local_or_remote_failure';all_complete=False
 finally:
  password=None;guard_done.set()
  if remote_terminal is not None and channel is not None:
   until=time.monotonic()+1
   while not channel.exit_status_ready() and time.monotonic()<until:time.sleep(.02)
  if channel is not None:
   try:
    if channel.exit_status_ready():
     status=channel.recv_exit_status()
     if status>=0:remote_exit=status;status_received=True
   except Exception:pass
  complete=all_complete and count>0 and reason in ['duration_completed','local_stop_file','local_interrupt'] and (reason!='duration_completed' or remote_exit==0)
  terminal={'schema':SCHEMA,'kind':'localTerminal','sampledAt':round(time.time()*1000),'reason':reason,'sampleCount':count,'coverageFirstSampledAt':first,'coverageLastSampledAt':last,'coverageFromCampaignStartClaimed':False,'diagnosticComplete':complete,'remoteExitStatus':remote_exit,'remoteExitStatusReceived':status_received,'remoteTerminalObserved':remote_terminal is not None,'remoteTerminalReason':remote_terminal['reason'] if remote_terminal else None,'hostKeyVerifiedBeforeAuthentication':pin_ok,'hostKeyPin':HOST_KEY,'pinsSha256':hashlib.sha256(pins_raw).hexdigest(),'collectorSourceSha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),'remoteSourceSha256':hashlib.sha256(REMOTE.encode()).hexdigest(),'stderrBytes':stderr_bytes,'stderrSha256':stderr_sha.hexdigest(),'rawStderrExported':False,'credentialsPersisted':False,'remoteStdinPasswordBytesSent':0,'remoteFilesWritten':False,'serviceOperations':[],'networkSettingsChanged':False,'clientFuseOutput':False,'samplingIntervalSeconds':2,'maxSeconds':a.max_seconds}
  try:persist(terminal)
  finally:close_owned();history.close()
 print(json.dumps({'output':str(a.output),'history':str(a.history),'samples':count,'diagnosticComplete':complete,'reason':reason,'remoteExitStatus':remote_exit,'servicesControlled':False}));return 0 if complete else 2
if __name__=='__main__':
 try:sys.exit(main())
 except Exception:print(json.dumps({'schema':SCHEMA,'kind':'entryFailure','diagnosticComplete':False,'reason':'entry_rejected','servicesControlled':False}),file=sys.stderr);sys.exit(2)
