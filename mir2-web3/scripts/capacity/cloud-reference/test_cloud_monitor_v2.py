"""Offline monitor fixtures only. No real sockets, SSH, /proc or services."""
import ast
import base64
import copy
import hashlib
import io
import json
import shlex
import sys
import tempfile
import types
import unittest
from contextlib import redirect_stderr, redirect_stdout
from pathlib import Path
from unittest.mock import patch

import cloud_monitor_v2 as monitor

HERE=Path(__file__).resolve().parent
TEST_ROOT=Path(tempfile.gettempdir()).resolve()
PINS={'pid':7,'startTicks':9,'invocationId':'1'*32,'activeState':'active'}
REV='2'*40


def original_remote():
    data=(HERE/'cloud_monitor.py').read_bytes()
    assert hashlib.sha256(data).hexdigest()==monitor.ORIGINAL_SOURCE_SHA256
    return ast.literal_eval(next(node.value for node in ast.parse(data.decode()).body
        if isinstance(node,ast.Assign) and any(isinstance(target,ast.Name) and target.id=='REMOTE' for target in node.targets)))


class Clock:
    def __init__(self): self.now=100.; self.on_sleep=None
    def time(self): return 1_800_000_000+self.now
    def monotonic(self): return self.now
    def monotonic_ns(self): return int(self.now*1e9)
    def sleep(self, seconds):
        self.now+=seconds
        if self.on_sleep is not None: self.on_sleep()


def config(max_seconds=1):
    return {'unit':'mir2-playtest.service','oldUnit':'mir2-gateway.service','expectedPid':7,
            'expectedRevision':REV,'expectedInvocationId':'1'*32,'expectedStartTicks':9,
            'healthPort':7210,'oldHealthPort':7110,'primaryNic':'eth0','maxSeconds':max_seconds,
            'intervalSeconds':2,'minHostMemoryBytes':512*1024*1024,'maxTargetMemoryRatio':.85}


def health():
    result={'ok':True,'http':'ready','ws':'ready','sessionCacheHealthy':True,'revision':REV}
    result.update({key:0 for key in monitor.CAPACITY_KEYS})
    result.update({key:50 for key in ['maxWsConnections','maxActiveSessions','maxReconnectLeases','maxLoginInFlight','maxNewCharacterInFlight','maxStartGameInFlight']})
    return result


def sample(clock,index=0):
    identity={key:dict(PINS) for key in ['expected','initializationBefore','initializationAfter','sampleBefore','sampleAfter']}
    return {'schemaVersion':1,'sampledAt':round(clock.time()*1000),'remoteMonotonicNs':clock.monotonic_ns(),'stop':False,
        'host':{'memory':{'MemAvailable':1024*1024*1024},'logicalCpus':2,'cpuTicks':[100+index,0,0,100+index,0,0,0,0],'pressure':{}},
        'targetUnit':{'MainPID':7,'InvocationID':'1'*32,'ActiveState':'active','MemoryCurrent':500,'MemoryMax':1000,'MemoryHigh':900,'CPUUsageNSec':10,'TasksCurrent':3,'ControlGroup':'/fixture'},
        'oldGatewayUnit':{'MainPID':8,'InvocationID':'3'*32,'ActiveState':'active'},
        'health':health(),'oldGatewayHealth':health(),
        'targetProcess':{'pid':7,'startTicks':9,'cpuTicks':10+index,'threads':3,'VmRSSBytes':500,'VmHWMBytes':600,'VmSwapBytes':0,'io':{}},
        'oldGatewayProcess':{'pid':8,'startTicks':10,'cpuTicks':1,'threads':1},
        'targetCgroup':{'memory.events':{'oom_kill':0},'memory.current':500,'cpu.stat':{},'memory.pressure':{},'io.stat':''},
        'network':{'primaryNic':'eth0','rxBytes':100+index,'txBytes':200+index,'scope':'one host primary NIC, including cohost traffic; no VPN/virtual-interface summation'},
        'monitorIdentity':identity}


def run_remote(source, mutate=None, max_seconds=1, identity_values=None, health_values=None):
    cfg=config(max_seconds); encoded=base64.b64encode(json.dumps(cfg).encode()).decode()
    prefix,suffix=source.split('start=time.monotonic(); previous=None; initial=None',1)
    space={'__name__':'offline_remote_fixture'}
    with patch.object(sys,'argv',['remote',encoded]): exec(prefix,space)
    clock=Clock(); space['time']=clock; space['os']=types.SimpleNamespace(sysconf=lambda unused:100)
    index=[0]
    def next_sample(unused):
        value=sample(clock,index[0])
        if mutate is not None: mutate(value,index[0])
        index[0]+=1
        return value
    space['sample']=next_sample
    identities=iter(identity_values) if identity_values is not None else None
    healths=iter(health_values) if health_values is not None else None
    space['target_identity']=lambda: next(identities) if identities is not None else dict(PINS)
    space['health']=lambda unused: next(healths) if healths is not None else health()
    output=io.StringIO()
    with redirect_stdout(output): exec('start=time.monotonic(); previous=None; initial=None'+suffix,space)
    return [json.loads(line) for line in output.getvalue().splitlines()]


class Key:
    def get_name(self): return 'ssh-ed25519'
    def asbytes(self): return b'offline monitor host key fixture'


class Channel:
    def __init__(self,data=b'',status=0,stderr=b'PRIVATE stderr fixture'):
        self.data=data;self.status=status;self.stderr=stderr;self.closed=False;self.stdin_closed=False;self.command=None
    def settimeout(self, unused): pass
    def exec_command(self, command): self.command=command
    def shutdown_write(self): self.stdin_closed=True
    def recv_ready(self): return bool(self.data)
    def recv_stderr_ready(self): return bool(self.stderr)
    def recv(self, maximum):
        assert self.stdin_closed
        chunk,self.data=self.data[:maximum],self.data[maximum:];return chunk
    def recv_stderr(self, maximum):
        assert self.stdin_closed
        chunk,self.stderr=self.stderr[:maximum],self.stderr[maximum:];return chunk
    def exit_status_ready(self): return self.status is not None and not self.data and not self.stderr
    def recv_exit_status(self): return self.status
    def close(self): self.closed=True


class Transport:
    def __init__(self, channel, auth_error=False, setup_error=False):
        self.channel=channel;self.auth_error=auth_error;self.setup_error=setup_error;self.auth_calls=0;self.active=True
    def get_security_options(self): return types.SimpleNamespace(key_types=None)
    def start_client(self, timeout):
        if self.setup_error: raise TimeoutError('PRIVATE setup error')
    def get_remote_server_key(self): return Key()
    def auth_password(self,user,password,fallback):
        self.auth_calls+=1
        assert user=='ubuntu' and password=='PRIVATE password fixture' and fallback is False
        if self.auth_error: raise RuntimeError('PRIVATE authentication detail')
    def is_authenticated(self): return not self.auth_error and self.auth_calls==1
    def open_session(self, timeout): return self.channel
    def is_active(self): return self.active
    def close(self): self.active=False


def cli(directory,extra=()):
    return ['--output',str(directory/'latest.json'),'--history',str(directory/'history.jsonl'),
        '--stop-file',str(directory/'stop.flag'),'--expected-pid','7','--expected-revision',REV,
        '--expected-invocation-id','1'*32,'--expected-start-ticks','9','--max-seconds','30',*extra]


def run_local(kind='finite',status=0,pin_good=True,auth_error=False,setup_error=False,stderr_bound=None):
    clock=Clock()
    first=sample(clock); last=sample(clock,1);last.update(stop=True,reason='finite monitor duration completed')
    if kind=='guard': last['reason']='host available memory below reserve'
    data=(json.dumps(first)+'\n'+json.dumps(last)+'\n').encode() if kind in {'finite','guard'} else (json.dumps(first)+'\n').encode() if kind=='eof' else b''
    if kind=='oversize': data=b'PRIVATE'*(monitor.MAX_LINE_BYTES//7+2)
    if kind=='staleClock':
        first['sampledAt']-=12001;data=(json.dumps(first)+'\n').encode()
    channel=Channel(data,status if kind not in {'stale','duration','stop'} else None)
    transport=Transport(channel,auth_error,setup_error)
    tcp=types.SimpleNamespace(shutdown=lambda unused:None,close=lambda:None)
    pin='SHA256:'+base64.b64encode(hashlib.sha256(Key().asbytes()).digest()).decode().rstrip('=') if pin_good else monitor.KNOWN_HOST_KEY
    with tempfile.TemporaryDirectory(dir=TEST_ROOT) as name:
        directory=Path(name);assert directory.resolve().parent==TEST_ROOT
        if kind=='stop': clock.on_sleep=lambda:(directory/'stop.flag').write_text('offline normal stop')
        extra=['--max-seconds','1'] if kind=='duration' else []
        with patch.object(monitor,'REPOSITORY',directory/'fake-repository'),patch.object(monitor,'time',clock),patch.object(monitor,'KNOWN_HOST_KEY',pin),patch.object(monitor.sys.stdin,'isatty',return_value=True),patch.object(monitor.getpass,'getpass',return_value='PRIVATE password fixture'),patch.object(monitor.socket,'create_connection',return_value=tcp),patch.dict(sys.modules,{'paramiko':types.SimpleNamespace(Transport=lambda unused:transport)}),redirect_stdout(io.StringIO()):
            if stderr_bound is None: code=monitor.main(cli(directory,extra))
            else:
                with patch.object(monitor,'MAX_STDERR_BYTES',stderr_bound): code=monitor.main(cli(directory,extra))
        latest=json.loads((directory/'latest.json').read_text())
        history=[json.loads(line) for line in (directory/'history.jsonl').read_text().splitlines()]
        raw=(directory/'latest.json').read_text()+(directory/'history.jsonl').read_text()
        assert 'PRIVATE' not in raw
        return code,latest,history,transport,channel


class MonitorTests(unittest.TestCase):
    def test_remote_original_resource_guards_and_data_ast_are_unchanged(self):
        class StripIdentity(ast.NodeTransformer):
            def visit_FunctionDef(self,node):
                if node.name in {'expected_identity','target_identity','initialization_identity'}: return None
                return self.generic_visit(node)
            def visit_Assign(self,node):
                if any(isinstance(target,ast.Name) and target.id in {'initialization_before','initialization_after'} for target in node.targets): return None
                if any(isinstance(target,ast.Subscript) and isinstance(target.value,ast.Name) and target.value.id=='out' and isinstance(target.slice,ast.Constant) and target.slice.value=='monitorIdentity' for target in node.targets): return None
                return self.generic_visit(node)
            def visit_If(self,node):
                dump=ast.dump(node.test)
                if 'initialization_after' in dump or 'monitorIdentity' in dump: return None
                return self.generic_visit(node)
        expected=ast.dump(ast.parse(original_remote()),include_attributes=False)
        observed=ast.dump(StripIdentity().visit(ast.parse(monitor.REMOTE)),include_attributes=False)
        self.assertEqual(observed,expected)

    def test_cli_guard_thresholds_and_finite_budgets_unchanged(self):
        with tempfile.TemporaryDirectory(dir=TEST_ROOT) as name,patch.object(monitor,'REPOSITORY',HERE/'fake-repository'):
            directory=Path(name);assert directory.resolve().parent==TEST_ROOT
            args=monitor.parse_arguments(cli(directory))
            self.assertEqual((args.interval_seconds,args.stale_seconds,args.min_host_memory_bytes,args.max_target_memory_ratio),(2,12,512*1024*1024,.85))
            self.assertEqual(monitor.parse_arguments(cli(directory,['--max-seconds','10800'])).max_seconds,10800)
            for extra in [['--max-seconds','10801'],['--interval-seconds','0.9'],['--interval-seconds','3.1'],['--stale-seconds','15'],['--stale-seconds','4'],['--min-host-memory-bytes',str(512*1024*1024-1)],['--max-target-memory-ratio','.851']]:
                with self.subTest(extra=extra),redirect_stderr(io.StringIO()),self.assertRaises(SystemExit):monitor.parse_arguments(cli(directory,extra))

    def test_cli_requires_exact_activation_and_fixed_host_service(self):
        with tempfile.TemporaryDirectory(dir=TEST_ROOT) as name,patch.object(monitor,'REPOSITORY',HERE/'fake-repository'):
            directory=Path(name);assert directory.resolve().parent==TEST_ROOT
            for extra in [['--host','example.invalid'],['--user','root'],['--ssh-port','23'],['--unit','other.service'],['--health-port','9999'],['--expected-revision','2'*39],['--expected-invocation-id','x'*32],['--expected-start-ticks','0']]:
                with self.subTest(extra=extra),redirect_stderr(io.StringIO()),self.assertRaises(SystemExit):monitor.parse_arguments(cli(directory,extra))
            with redirect_stderr(io.StringIO()),self.assertRaises(SystemExit):monitor.parse_arguments(['--output','x','--history','y','--stop-file','z'])

    def test_outputs_cannot_be_inside_repository_or_reused(self):
        with tempfile.TemporaryDirectory(dir=TEST_ROOT) as name:
            directory=Path(name);assert directory.resolve().parent==TEST_ROOT
            with patch.object(monitor,'REPOSITORY',directory),redirect_stderr(io.StringIO()),self.assertRaises(SystemExit):monitor.parse_arguments(cli(directory))
            target=directory/'latest.json';target.write_text('frozen existing')
            with patch.object(monitor,'REPOSITORY',directory/'fake-repository'),patch.object(monitor.getpass,'getpass') as password,patch.object(monitor.socket,'create_connection') as connect,self.assertRaises(ValueError):monitor.main(cli(directory))
            password.assert_not_called();connect.assert_not_called();self.assertEqual(target.read_text(),'frozen existing')

    def test_local_pty_is_required_before_password_or_connection(self):
        with tempfile.TemporaryDirectory(dir=TEST_ROOT) as name:
            directory=Path(name);assert directory.resolve().parent==TEST_ROOT
            with patch.object(monitor,'REPOSITORY',directory/'fake-repository'),patch.object(monitor.sys.stdin,'isatty',return_value=False),patch.object(monitor.getpass,'getpass') as password,patch.object(monitor.socket,'create_connection') as connect,self.assertRaises(ValueError):monitor.main(cli(directory))
            password.assert_not_called();connect.assert_not_called()
            self.assertFalse((directory/'history.jsonl').exists())

    def test_sample_clock_stale_and_future_boundaries_are_unchanged(self):
        clock=Clock()
        with tempfile.TemporaryDirectory(dir=TEST_ROOT) as name,patch.object(monitor,'REPOSITORY',HERE/'fake-repository'),patch.object(monitor,'time',clock):
            directory=Path(name);assert directory.resolve().parent==TEST_ROOT
            args=monitor.parse_arguments(cli(directory))
            for age in (12000,-5000):
                value=sample(clock);value['sampledAt']-=age;monitor.validate_sample(value,args)
            for age in (12001,-5001):
                value=sample(clock);value['sampledAt']-=age
                with self.assertRaises(ValueError):monitor.validate_sample(value,args)

    def test_low_memory_target_limit_old_health_and_oom_guards_match_baseline(self):
        def low_host(s,i):s['host']['memory']['MemAvailable']=512*1024*1024-1
        def high_target(s,i):s['targetUnit']['MemoryCurrent']=851
        def old_health(s,i):s['oldGatewayHealth']['sessionCacheHealthy']=False
        def max_changed(s,i):s['targetUnit']['MemoryMax']=1000+i
        def oom(s,i):s['targetCgroup']['memory.events']['oom_kill']=i
        def cpu_reversed(s,i):s['host']['cpuTicks'][0]=100-i
        def nic_reversed(s,i):s['network']['rxBytes']=100-i
        for mutate in [low_host,high_target,old_health,max_changed,oom,cpu_reversed,nic_reversed]:
            with self.subTest(case=mutate.__name__):
                old=run_remote(original_remote(),mutate,max_seconds=10)
                new=run_remote(monitor.REMOTE,mutate,max_seconds=10)
                self.assertTrue(old[-1]['stop']);self.assertTrue(new[-1]['stop'])
                self.assertEqual(new[-1].get('reason'),old[-1].get('reason'))
                self.assertEqual(new[-1].get('errorClass'),old[-1].get('errorClass'))

    def test_healthy_remote_finishes_with_identical_rates_and_resource_fields(self):
        old=run_remote(original_remote());new=run_remote(monitor.REMOTE)
        self.assertEqual(new,old)
        self.assertEqual(new[-1]['reason'],'finite monitor duration completed')
        self.assertIn('targetCpuCores',new[-1]['rates']);self.assertIn('rxBytesPerSecond',new[-1]['rates'])

    def test_remote_initialization_checks_expected_identity_before_and_after(self):
        bad={**PINS,'startTicks':10}
        for identities in [[bad],[dict(PINS),bad]]:
            with self.subTest(identities=identities):
                rows=run_remote(monitor.REMOTE,identity_values=identities)
                self.assertEqual(len(rows),1);self.assertTrue(rows[0]['stop'])
                self.assertEqual(rows[0]['reason'],'readonly remote sampler failed')
        wrong={**health(),'revision':'3'*40}
        rows=run_remote(monitor.REMOTE,health_values=[health(),wrong])
        self.assertTrue(rows[-1]['stop']);self.assertEqual(rows[-1]['reason'],'readonly remote sampler failed')

    def test_per_sample_pin_change_requests_guard_stop(self):
        def mutate(s,i):s['monitorIdentity']['sampleAfter']['invocationId']='4'*32
        rows=run_remote(monitor.REMOTE,mutate,max_seconds=10)
        self.assertEqual(len(rows),1);self.assertTrue(rows[0]['stop'])
        self.assertIn('target pinned activation identity changed',rows[0]['reason'])

    def test_pin_is_verified_before_the_only_password_auth_attempt(self):
        code,latest,unused,transport,channel=run_local(pin_good=False)
        self.assertEqual(code,2);self.assertEqual(transport.auth_calls,0);self.assertIsNone(channel.command)
        self.assertFalse(latest['monitorReadback']['hostKeyVerifiedBeforeAuthentication'])
        code,latest,unused,transport,channel=run_local(auth_error=True)
        self.assertEqual(code,2);self.assertEqual(transport.auth_calls,1);self.assertIsNone(channel.command)
        self.assertTrue(latest['monitorReadback']['hostKeyVerifiedBeforeAuthentication'])

    def test_nopasswd_sudo_stdin_closed_and_no_credential_or_stderr_persistence(self):
        code,latest,history,transport,channel=run_local()
        self.assertEqual(code,0);self.assertEqual(transport.auth_calls,1);self.assertTrue(channel.stdin_closed)
        command=shlex.split(channel.command)
        self.assertEqual(command[:6],['sudo','-n','--','python3','-u','-c'])
        self.assertEqual(command[6],monitor.REMOTE);self.assertNotIn('PRIVATE',channel.command)
        cfg=json.loads(base64.b64decode(command[7]))
        self.assertEqual((cfg['expectedPid'],cfg['expectedStartTicks'],cfg['expectedInvocationId'],cfg['expectedRevision']),(7,9,'1'*32,REV))
        self.assertEqual(latest['monitorReadback']['remoteStdinPasswordBytesSent'],0)
        self.assertEqual(latest['monitorReadback']['stderrBytesReceived'],len(b'PRIVATE stderr fixture'))
        self.assertFalse(latest['monitorReadback']['rawStderrPersisted'])
        self.assertEqual(latest['sampleCount'],2);self.assertTrue(history[0]['stop'])

    def test_stream_exit_without_terminal_fails_even_on_exit_zero(self):
        for status in (0,1,-1):
            with self.subTest(status=status):
                code,latest,unused,unused_transport,unused_channel=run_local('eof',status)
                self.assertEqual(code,2);self.assertEqual(latest['outcome'],'failure')
                meta=latest['monitorReadback'];self.assertFalse(meta['remoteTerminalSampleObserved'])
                self.assertEqual(meta['remoteExitStatusReceived'],status>=0)
                self.assertEqual(meta['remoteExitStatus'],status if status>=0 else None)
                self.assertEqual(meta['exitStatusMethodReturn'],status)

    def test_valid_finite_terminal_does_not_turn_missing_exit_status_into_zero(self):
        code,latest,unused,unused_transport,unused_channel=run_local(status=-1)
        self.assertEqual(code,0);self.assertEqual(latest['monitorReadback']['completedBy'],'remoteFiniteTerminal')
        self.assertTrue(latest['monitorReadback']['remoteTerminalSampleObserved'])
        self.assertFalse(latest['monitorReadback']['remoteExitStatusReceived'])
        self.assertIsNone(latest['monitorReadback']['remoteExitStatus'])
        self.assertEqual(latest['monitorReadback']['exitStatusMethodReturn'],-1)
        code,latest,unused,unused_transport,unused_channel=run_local(status=1)
        self.assertEqual(code,2);self.assertEqual(latest['outcome'],'failure')

    def test_guard_stop_staleness_clock_and_setup_failures_remain_nonzero(self):
        for kwargs in [{'kind':'guard'},{'kind':'stale'},{'kind':'staleClock'},{'setup_error':True}]:
            with self.subTest(kwargs=kwargs):
                code,latest,unused,unused_transport,unused_channel=run_local(**kwargs)
                self.assertEqual(code,2);self.assertTrue(latest['stop']);self.assertTrue(latest['monitorTerminal'])
        code,latest,unused,unused_transport,unused_channel=run_local('stale')
        self.assertEqual(latest['reason'],'remote sample stream stale or interrupted')

    def test_local_stop_and_duration_only_close_owned_transport(self):
        for kind in ('stop','duration'):
            with self.subTest(kind=kind):
                code,latest,unused,transport,channel=run_local(kind)
                self.assertEqual(code,0);self.assertEqual(latest['outcome'],'completed')
                self.assertEqual(latest['monitorReadback']['completedBy'],'localStopFile' if kind=='stop' else 'localDuration')
                self.assertFalse(latest['servicesStopped']);self.assertFalse(transport.active);self.assertTrue(channel.closed)

    def test_stdout_line_and_stderr_bounds_fail_without_raw_persistence(self):
        code,latest,unused,unused_transport,unused_channel=run_local('oversize')
        self.assertEqual(code,2);self.assertEqual(latest['reason'],'remote sample exceeded finite bound')
        code,latest,unused,unused_transport,unused_channel=run_local(stderr_bound=8)
        self.assertEqual(code,2);self.assertEqual(latest['reason'],'remote stderr exceeded finite bound')


if __name__=='__main__':unittest.main(verbosity=2)
