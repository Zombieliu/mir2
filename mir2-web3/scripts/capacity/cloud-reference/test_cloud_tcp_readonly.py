import contextlib,copy,hashlib,base64,io,json,sys,tempfile,time,types,unittest
from pathlib import Path
from unittest import mock
import cloud_tcp_readonly as m
HERE=Path(tempfile.gettempdir())/"mir2-cloud-tcp-readonly-offline"
HERE.mkdir(exist_ok=True)
SECRET='SYNTHETIC_ONLY_DO_NOT_EXPORT'
def pins():return {'schema':'mir2.cloud-tcp-readonly-pins.v1','templateOnly':False,'rootReviewedIdentityPins':True,'target':{'pid':41,'startTicks':100,'invocationId':'a'*32,'revision':'b'*40},'caddy':{'pid':42,'startTicks':200},'provenance':{'statusReceiptSha256':'c'*64,'statusSourceSha256':'d'*64}}
def ss(local='192.0.2.1:443',peer='198.51.100.3:50001',pid=42,detail='rto:204 rtt:1.25/0.5 retrans:0/0 bytes_sent:0 bytes_received:0 bytes_acked:0 bytes_retrans:0'):
 return f'ESTAB 0 5 {local} {peer} users:(("caddy",pid={pid},fd=8)) ino:77\n\t cubic {detail}\n'
def sample():
 p=pins();z=m.project_ss(ss(),p,b'x'*32)
 return {'schema':m.SCHEMA,'kind':'sample','sampleIndex':1,'sampledAt':round(time.time()*1000),'remoteMonotonicNs':1,'collectionDurationMs':3,'identityBefore':{'target':p['target'],'caddy':p['caddy']},'identityAfter':{'target':p['target'],'caddy':p['caddy']},'kernelTotals':{g:{k:0 for k in ks} for g,ks in m.COUNTERS.items()},'kernelDeltas':{g:{k:None for k in ks} for g,ks in m.COUNTERS.items()},**z}
class Key:
 def __init__(self,name='ssh-ed25519'):self.name=name
 def get_name(self):return self.name
 def asbytes(self):return b'synthetic-ed25519-public-key'
class Channel:
 def __init__(self,exit_status=0,wire=None):
  x=sample();end={'schema':m.SCHEMA,'kind':'terminal','sampledAt':round(time.time()*1000),'reason':'duration_completed','sampleCount':1,'firstSampledAt':x['sampledAt'],'diagnosticComplete':True};self.data=wire if wire is not None else (json.dumps(x)+'\n'+json.dumps(end)+'\n').encode();self.closed=False;self.exit=exit_status;self.stdin_closed=False;self.command=None
 def settimeout(self,n):pass
 def exec_command(self,s):self.command=s;assert SECRET not in s and s.startswith('sudo -n -- python3 -B -u -c ')
 def shutdown_write(self):self.stdin_closed=True
 def recv_stderr_ready(self):return False
 def recv_ready(self):return bool(self.data)
 def recv(self,n):d,self.data=self.data[:n],self.data[n:];return d
 def exit_status_ready(self):return not self.data
 def recv_exit_status(self):return self.exit
 def close(self):self.closed=True
 def send(self,*a):raise AssertionError('No stdin bytes may be sent')
 def sendall(self,*a):raise AssertionError('No stdin bytes may be sent')
class Transport:
 def __init__(self,c,key):self.c=c;self.key=key;self.auth_calls=[];self.options=types.SimpleNamespace(key_types=None)
 def get_security_options(self):return self.options
 def start_client(self,timeout):pass
 def get_remote_server_key(self):return self.key
 def auth_password(self,user,password,fallback):self.auth_calls.append((user,password,fallback))
 def is_authenticated(self):return bool(self.auth_calls)
 def open_session(self,timeout):return self.c
 def is_active(self):return True
 def close(self):pass
class Guards(unittest.TestCase):
 def test_remote_shared_source_compiles_without_execution(self):compile(m.REMOTE,'remote','exec');self.assertIn('remote_main(',m.REMOTE);self.assertNotIn('auth_password',m.REMOTE)
 def test_parser_zero_is_valid_anonymous_and_salted(self):
  z=m.project_ss(ss(),pins(),b'x'*32);r=z['sockets'][0];self.assertTrue(z['diagnosticComplete']);self.assertEqual(r['retransTotal'],0);self.assertEqual(r['bytesSent'],0);self.assertEqual(r['sendQBytes'],5);self.assertEqual(r['rttMs'],1.25)
  text=json.dumps(z);self.assertNotIn('192.0.2.1',text);self.assertNotIn('50001',text);self.assertNotIn('caddy",pid',text);self.assertNotEqual(r['socketId'],m.project_ss(ss(),pins(),b'y'*32)['sockets'][0]['socketId'])
 def test_loopback_pair_and_ipv6_public_scope(self):
  z=m.project_ss(ss('127.0.0.1:7210','127.0.0.1:45000',41)+ss('127.0.0.1:45000','127.0.0.1:7210',42),pins(),b'x'*32)['sockets'];self.assertEqual(z[0]['flowId'],z[1]['flowId']);self.assertNotEqual(z[0]['socketId'],z[1]['socketId']);self.assertEqual(m.project_ss(ss('[::ffff:192.0.2.1]:443','[2001:db8::3]:45000'),pins(),b'x'*32)['sockets'][0]['role'],'caddyPublic443')
 def test_missing_metrics_remain_null_and_incomplete(self):
  z=m.project_ss(ss(detail='bytes_sent:0'),pins(),b'x'*32);self.assertFalse(z['diagnosticComplete']);self.assertIsNone(z['sockets'][0]['retransTotal']);self.assertIsNone(z['sockets'][0]['bytesReceived'])
 def test_unknown_or_foreign_owner_not_exported_or_falsely_complete(self):
  for text in [ss().replace('users:(("caddy",pid=42,fd=8))',''),ss(pid=90)]:
   z=m.project_ss(text,pins(),b'x'*32);self.assertEqual(z['sockets'],[]);self.assertFalse(z['diagnosticComplete']);self.assertNotIn('pid',json.dumps(z))
 def test_invalid_fields_and_non_loopback7210_reject(self):
  for text in [ss(detail='rto:garbage'),ss(detail='bytes_sent:0.5'),ss('192.0.2.1:7210','127.0.0.1:9',41),'not known output']:
   with self.subTest(text=text):
    with self.assertRaises(m.DiagnosticError):m.project_ss(text,pins(),b'x'*32)
 def test_kernel_header_pairs_and_missing_not_zero(self):
  z=m.proc_counters('Tcp: RetransSegs InSegs\nTcp: 7 20\nUdp: InDatagrams\nUdp: 5\n','Tcp');self.assertEqual(z['RetransSegs'],7);self.assertIsNone(z['OutRsts'])
  with self.assertRaises(m.DiagnosticError):m.proc_counters('Tcp: RetransSegs InSegs\nTcp: 1\n','Tcp')
 def test_pins_unknown_template_and_boolean_pid_refuse(self):
  for field in ['template','boolean','secret']:
   x=pins()
   if field=='template':x['templateOnly']=True
   elif field=='boolean':x['target']['pid']=True
   else:x['password']=SECRET
   with self.assertRaises(m.DiagnosticError):m.validate_pins(x)
 def test_both_identity_snapshots_and_schema_are_guarded(self):
  for field in ['identityBefore','identityAfter']:
   x=sample();x[field]=copy.deepcopy(x[field]);x[field]['target']['startTicks']+=1
   with self.assertRaises(m.DiagnosticError):m.validate_row(x,pins())
  x=sample();x['rawEndpoints']='198.51.100.3:9'
  with self.assertRaises(m.DiagnosticError):m.validate_row(x,pins())
 def test_missing_metric_cannot_claim_diagnostic_complete(self):
  x=sample();x['sockets'][0]['retransTotal']=None
  with self.assertRaises(m.DiagnosticError):m.validate_row(x,pins())
 def test_fixed_read_command_whitelist_rejects_service_control_before_spawn(self):
  with mock.patch.object(m.subprocess,'Popen') as popen:
   with self.assertRaises(m.DiagnosticError):m.readonly_command(['systemctl','stop','mir2-playtest.service'],time.monotonic()+1)
   popen.assert_not_called()
 def test_remote_missing_ss_is_failure_not_zero(self):
  output=[]
  with mock.patch.object(m.shutil,'which',return_value=None),mock.patch.object(m,'emit_remote',side_effect=output.append),mock.patch.object(m,'identity') as identity:
   with self.assertRaises(SystemExit) as e:m.remote_main({'pins':pins(),'maxSeconds':1})
   self.assertEqual(e.exception.code,2);identity.assert_not_called()
  self.assertEqual(output[0]['reason'],'ss_unavailable');self.assertFalse(output[0]['diagnosticComplete']);self.assertNotIn('kernelTotals',output[0])
 def test_remote_changed_after_identity_never_publishes_sample(self):
  expected={'target':pins()['target'],'caddy':pins()['caddy']};changed=copy.deepcopy(expected);changed['caddy']['startTicks']+=1;output=[]
  def raw(path,bound):
   group='TcpExt' if path.endswith('netstat') else 'Tcp';names=m.COUNTERS[group];return group+': '+' '.join(names)+'\n'+group+': '+' '.join('0' for _ in names)+'\n'
  with mock.patch.object(m.shutil,'which',return_value='/usr/bin/ss'),mock.patch.object(m,'identity',side_effect=[expected,changed]),mock.patch.object(m,'read_small',side_effect=raw),mock.patch.object(m,'readonly_command',return_value=ss()),mock.patch.object(m,'emit_remote',side_effect=output.append):
   with self.assertRaises(SystemExit) as e:m.remote_main({'pins':pins(),'maxSeconds':1})
   self.assertEqual(e.exception.code,2)
  self.assertEqual(len(output),1);self.assertEqual(output[0]['reason'],'identity_changed');self.assertFalse(output[0]['diagnosticComplete'])
 def run_fake(self,exit_status=0,wrong_key=False,wire=None):
  t=tempfile.TemporaryDirectory(dir=HERE,prefix='offline-fixture-');folder=Path(t.name);self.assertEqual(folder.parent,HERE)
  with t:
   pp=folder/'pins.json';pp.write_text(json.dumps(pins()),encoding='utf8');ch=Channel(exit_status,wire);key=Key();tr=Transport(ch,key);fake=types.SimpleNamespace(Transport=lambda tcp:tr);fp='SHA256:'+base64.b64encode(hashlib.sha256(key.asbytes()).digest()).decode().rstrip('=')
   with mock.patch.dict(sys.modules,{'paramiko':fake}),mock.patch.object(m.socket,'create_connection',return_value=types.SimpleNamespace(close=lambda:None)),mock.patch.object(m.getpass,'getpass',return_value=SECRET),mock.patch.object(m.sys,'stdin',types.SimpleNamespace(isatty=lambda:True)),mock.patch.object(m,'HOST_KEY',m.HOST_KEY if wrong_key else fp),contextlib.redirect_stdout(io.StringIO()):
    result=m.main(['--pins',str(pp),'--output',str(folder/'latest.json'),'--history',str(folder/'history.jsonl'),'--stop-file',str(folder/'diagnostic.stop'),'--max-seconds','1'])
   raw=(folder/'history.jsonl').read_bytes();self.assertNotIn(SECRET.encode(),raw);last=json.loads((folder/'latest.json').read_bytes());return result,last,ch,tr
 def test_fake_transport_pin_before_auth_and_no_sudo_stdin(self):
  result,last,ch,tr=self.run_fake();self.assertEqual(result,0);self.assertTrue(ch.stdin_closed);self.assertEqual(len(tr.auth_calls),1);self.assertFalse(tr.auth_calls[0][2]);self.assertEqual(last['remoteStdinPasswordBytesSent'],0);self.assertFalse(last['coverageFromCampaignStartClaimed']);self.assertEqual(last['serviceOperations'],[])
 def test_wrong_host_key_prevents_auth_and_command(self):
  result,last,ch,tr=self.run_fake(wrong_key=True);self.assertEqual(result,2);self.assertEqual(tr.auth_calls,[]);self.assertIsNone(ch.command);self.assertFalse(last['hostKeyVerifiedBeforeAuthentication'])
 def test_missing_remote_exit_is_null_never_success(self):
  result,last,ch,tr=self.run_fake(exit_status=-1);self.assertEqual(result,2);self.assertIsNone(last['remoteExitStatus']);self.assertFalse(last['remoteExitStatusReceived']);self.assertFalse(last['diagnosticComplete'])
if __name__=='__main__':unittest.main()
