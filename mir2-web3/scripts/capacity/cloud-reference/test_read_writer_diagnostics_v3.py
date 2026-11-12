"""Offline only: no repository, credentials, public network or real SSH."""
import base64
import ast
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

import read_writer_diagnostics_v3 as reader

HERE = Path(__file__).resolve().parent
CONFIG = reader.make_config('7','9','1'*32,'2'*40,'2026-11-12T12:00:00Z','2026-11-12T12:00:20Z')
IDENTITY = {'pid':7,'processStartTicks':9,'invocationId':'1'*32,'activeState':'active'}
LEAF = '[gateway-slow-stage] stage=personal_tick.leaves duration_ms=8 total_us=8000 billing_us=1000 gift_us=1000 checkpoint_begin_us=1000 personal_timers_us=1000 source_dispatch_us=1000 finalize_us=1000 finish_us=1000 leaf_mask=127 outcome=ok'
RESPONSE = '[gateway-response-write] messages=1 bytes=90 flushes=1 serialization_us=50 lock_wait_us=2000000 feed_us=100 flush_us=1500 elapsed_us=2002000'
BLOCK = '[gateway-zone-outbound-block] stage=send registration=17 batchID=2 batchIndex=0 kind=UserLocation lane=owner normalDepth=2 ownerDepth=1 elapsedUs=2002000 recordedUnixMs=1794484800000'
TRACE = '[gateway-zone-delivery-trace] stage=sent serverTraceID=4 registration=17 recipientZoneID=55 kind=UserLocation targetObjectID=55 hasPosition=1 x=-100 y=54 producerUnixMs=1794484800000 elapsedUs=2002000 queueAgeUs=100 gateWaitUs=200'


def row(message=LEAF, **changes):
    value = {'MESSAGE':message,'_PID':'7','_SYSTEMD_INVOCATION_ID':'1'*32,
             '__REALTIME_TIMESTAMP':str(CONFIG['sinceUs']+1)}
    value.update(changes)
    return json.dumps(value)


def namespace():
    space = {'__name__':'offline_fixture'}
    exec(reader.build_remote(CONFIG), space)
    space['identity'] = lambda unused: dict(IDENTITY)
    space['health'] = lambda unused: {'ok':True,'revision':CONFIG['revision']}
    space['diagnostic_flags'] = lambda unused: {'readComplete':True,'zoneDeliveryTraceLiteral1':False,'slowStagePositiveU64':True}
    return space


def result(lines):
    space = namespace()
    space['read_journal'] = lambda unused: (lines, {'exitStatus':0,'stdoutBytes':sum(len(v.encode())+1 for v in lines),
        'stderrBytes':0,'stderrSha256':hashlib.sha256(b'').hexdigest()})
    return space['collect']()


def frame(value):
    return reader.MARKER + json.dumps(value,separators=(',',':')).encode() + b'\n'


class Key:
    def get_name(self): return 'ssh-ed25519'
    def asbytes(self): return b'offline public key fixture'


class Channel:
    def __init__(self, data, status):
        self.data=data; self.status=status; self.stderr=b'PRIVATE raw stderr fixture'; self.closed=False
        self.stdin_closed=False; self.command=None
    def exec_command(self, command): self.command=command
    def shutdown_write(self): self.stdin_closed=True
    def settimeout(self, value): pass
    def recv_ready(self): return bool(self.data)
    def recv_stderr_ready(self): return bool(self.stderr)
    def recv(self, maximum):
        chunk,self.data=self.data[:maximum],self.data[maximum:]; return chunk
    def recv_stderr(self, maximum):
        chunk,self.stderr=self.stderr[:maximum],self.stderr[maximum:]; return chunk
    def exit_status_ready(self): return not self.data and not self.stderr
    def recv_exit_status(self): return self.status
    def close(self): self.closed=True


class Transport:
    def __init__(self, channel): self.channel=channel; self.active=True; self.auth_called=False
    def get_security_options(self): return types.SimpleNamespace(key_types=None)
    def start_client(self, timeout): pass
    def get_remote_server_key(self): return Key()
    def auth_password(self, user, password, fallback):
        assert user=='ubuntu' and password=='PRIVATE password fixture' and fallback is False
        self.auth_called=True
    def is_authenticated(self): return self.auth_called
    def open_session(self, timeout): return self.channel
    def is_active(self): return self.active
    def close(self): self.active=False


class ReaderTests(unittest.TestCase):
    def test_response_aggregate_keeps_exact_prefix_without_inventing_stage_or_socket(self):
        value=result([row(RESPONSE)])
        event=value['diagnosticEvents'][0]
        self.assertEqual(event['prefix'],'[gateway-response-write]')
        self.assertIsNone(event['stage'])
        self.assertNotIn('connection',event)
        self.assertEqual(event['lock_wait_us'],2000000)
        self.assertEqual(event['elapsed_us'],2002000)
        self.assertNotIn('duration_ms',event)
        self.assertEqual(value['stageCounts'],{})
        self.assertEqual(value['prefixCounts'],{'[gateway-response-write]':1})
        self.assertEqual(value['maximumElapsedUsByPrefix'],{'[gateway-response-write]':2002000})
        self.assertTrue(value['boundedWindowExportComplete'])
        self.assertEqual(reader.validate_frame(frame(value),CONFIG)[1],'validFrame')
        event['stage']='response_write'
        self.assertEqual(reader.validate_frame(frame(value),CONFIG)[1],'unsafeEvent')

    def test_zone_prefixes_keep_backend_context_and_redact_positions_and_object_ids(self):
        value=result([row(BLOCK),row(TRACE.replace('x=-100','x=PRIVATE-COORDINATE'))])
        block,trace=value['diagnosticEvents']
        self.assertEqual(block['registration'],trace['registration'])
        self.assertEqual(block['batchID'],2)
        self.assertEqual(trace['serverTraceID'],4)
        self.assertEqual(trace['redactedTokenCount'],5)
        for key in ['recipientZoneID','targetObjectID','hasPosition','x','y']:
            self.assertNotIn(key,trace)
        self.assertNotIn('PRIVATE',json.dumps(value))
        self.assertTrue(value['boundedWindowExportComplete'])
        self.assertEqual(reader.validate_frame(frame(value),CONFIG)[1],'validFrame')

    def test_all_prefixes_reject_bad_metrics_enums_payload_tokens_and_duplicates(self):
        cases=[RESPONSE.replace('flush_us=1500','flush_us=1.5'),
               RESPONSE+' lock_wait_us=9', BLOCK.replace('lane=owner','lane=PRIVATE'),
               TRACE.replace('kind=UserLocation','kind=Other'), TRACE+' account=PRIVATE-ACCOUNT',
               RESPONSE+' payload=PRIVATE-PACKET']
        for message in cases:
            with self.subTest(message=message):
                value=result([row(message)])
                self.assertEqual(value['diagnosticEvents'][0]['fieldStatus'],'invalid')
                self.assertFalse(value['boundedWindowExportComplete'])
                self.assertNotIn('PRIVATE',json.dumps(value))
                self.assertEqual(reader.validate_frame(frame(value),CONFIG)[1],'validFrame')
        value=result([row(RESPONSE.replace(' flush_us=1500',''))])
        self.assertIsNone(value['diagnosticEvents'][0]['flush_us'])
        self.assertEqual(value['diagnosticEvents'][0]['fieldStatus'],'partial')

    def test_flags_are_only_booleans_and_absent_or_unreadable_stay_distinct(self):
        raw=(b'PASSWORD=PRIVATE-SECRET\0MIR2_GATEWAY_ZONE_DELIVERY_TRACE_RECIPIENTS=PRIVATE-IDS\0'
             b'MIR2_GATEWAY_ZONE_DELIVERY_TRACE=1\0MIR2_GATEWAY_SLOW_STAGE_MS=25\0')
        observed=reader.flags_from_environ(raw)
        self.assertEqual(observed,{'readComplete':True,'zoneDeliveryTraceLiteral1':True,'slowStagePositiveU64':True})
        self.assertNotIn('PRIVATE',json.dumps(observed))
        self.assertEqual(reader.flags_from_environ(b'UNRELATED=PRIVATE\0'),
                         {'readComplete':True,'zoneDeliveryTraceLiteral1':False,'slowStagePositiveU64':False})
        for raw in [None,b'x'*262145,b'MIR2_GATEWAY_ZONE_DELIVERY_TRACE=1\0MIR2_GATEWAY_ZONE_DELIVERY_TRACE=1\0']:
            observed=reader.flags_from_environ(raw)
            self.assertFalse(observed['readComplete'])
            self.assertIsNone(observed['zoneDeliveryTraceLiteral1'])
            self.assertIsNone(observed['slowStagePositiveU64'])
        for text,expected in [('0',False),('-1',False),('bad',False),(' 25 ',True),('+25',True),
                              ('00000000000000000000025',True),('18446744073709551615',True),
                              ('18446744073709551616',False)]:
            raw=('MIR2_GATEWAY_SLOW_STAGE_MS='+text+'\0MIR2_GATEWAY_ZONE_DELIVERY_TRACE= 1\0').encode()
            observed=reader.flags_from_environ(raw)
            self.assertIs(observed['slowStagePositiveU64'],expected)
            self.assertFalse(observed['zoneDeliveryTraceLiteral1'])

    def test_verified_pid_environment_reader_is_bounded_read_only_and_never_persists_raw(self):
        space={'__name__':'offline_fixture'}
        exec(reader.build_remote(CONFIG),space)
        opens=[];reads=[]
        class FakeFile(io.BytesIO):
            def read(self,limit=-1):
                reads.append(limit)
                return super().read(limit)
        class FakePath:
            def __init__(self,*parts):self.parts=parts
            def open(self,mode):
                opens.append((self.parts,mode))
                return FakeFile(b'ACCOUNT=PRIVATE\0MIR2_GATEWAY_ZONE_DELIVERY_TRACE=1\0MIR2_GATEWAY_SLOW_STAGE_MS=25\0')
        space['Path']=FakePath
        observed=space['diagnostic_flags'](reader.time.monotonic()+1)
        self.assertEqual(opens,[(('/proc','7','environ'),'rb')])
        self.assertEqual(reads,[262145])
        self.assertEqual(set(observed),{'readComplete','zoneDeliveryTraceLiteral1','slowStagePositiveU64'})
        self.assertTrue(all(type(value) is bool for value in observed.values()))
        self.assertNotIn('PRIVATE',json.dumps(observed))

    def test_flag_aggregate_and_context_forgery_fail_closed(self):
        original=result([row(RESPONSE),row(BLOCK),row(TRACE)])
        changes=[lambda v:v['diagnosticFlags'].update(rawEnvironment='PRIVATE'),
                 lambda v:v['diagnosticFlags'].update(zoneDeliveryTraceLiteral1=1),
                 lambda v:v['diagnosticFlags'].update(readComplete=False),
                 lambda v:v['prefixCounts'].update({'[gateway-response-write]':True}),
                 lambda v:v['diagnosticEvents'][0].update(connection=7),
                 lambda v:v['diagnosticEvents'][2].update(x=100)]
        for change in changes:
            value=copy.deepcopy(original);change(value)
            safe,category,unused=reader.validate_frame(frame(value),CONFIG)
            self.assertIsNone(safe)
            self.assertNotEqual(category,'validFrame')
        original['diagnosticFlags']=reader.flags_from_environ(None)
        self.assertEqual(reader.validate_frame(frame(original),CONFIG)[1],'validFrame')
        self.assertFalse(original['diagnosticFlags']['readComplete'])

    def test_journal_grep_selects_only_exact_existing_prefixes_and_static_labels(self):
        space=namespace();args=space['journal_args']();pattern=args[args.index('--grep')+1]
        for message in [LEAF,RESPONSE,BLOCK,TRACE,'[gateway-slow-stage] stage=session_action.flush duration_ms=50 connection=7']:
            self.assertIsNotNone(reader.re.search(pattern,message))
        for message in ['[gateway-zone-live] owner_location account=PRIVATE',
                        '[gateway-slow-stage] stage=session_action.flush_extra duration_ms=1',
                        '[gateway-zone-outbound-block] stage=PRIVATE elapsedUs=1']:
            self.assertIsNone(reader.re.search(pattern,message))

    def test_parent_transport_pins_budgets_and_exclusive_writer_are_unchanged(self):
        parent=HERE/'read_tick_diagnostics_v2.py'
        old_text=parent.read_text(encoding='utf-8-sig')
        self.assertEqual(hashlib.sha256(parent.read_bytes()).hexdigest(),reader.PARENT_READER_SHA256)
        old={node.name:node for node in ast.parse(old_text).body if isinstance(node,ast.FunctionDef)}
        new={node.name:node for node in ast.parse(Path(reader.__file__).read_text(encoding='utf-8')).body if isinstance(node,ast.FunctionDef)}
        for name in ['utc','make_config','build_remote','verify_host_key']:
            self.assertEqual(ast.dump(old[name],include_attributes=False),ast.dump(new[name],include_attributes=False),name)
        expected=copy.deepcopy(new['main'])
        for node in ast.walk(expected):
            if isinstance(node,ast.Constant) and node.value=='mir2.writer-diagnostics-readback.v1':
                node.value='mir2.tick-diagnostics-readback.v1'
            if isinstance(node,ast.Dict):
                kept=[(key,value) for key,value in zip(node.keys,node.values)
                      if not (isinstance(key,ast.Constant) and key.value=='parentReaderSourceSha256')]
                node.keys=[key for key,value in kept];node.values=[value for key,value in kept]
        self.assertEqual(ast.dump(old['main'],include_attributes=False),ast.dump(expected,include_attributes=False))
        old_remote=ast.literal_eval(next(node.value for node in ast.parse(old_text).body if isinstance(node,ast.Assign) and any(isinstance(target,ast.Name) and target.id=='REMOTE_BODY' for target in node.targets)))
        old_remote_ast={node.name:node for node in ast.parse(old_remote).body if isinstance(node,ast.FunctionDef)}
        new_remote_ast={node.name:node for node in ast.parse(reader.REMOTE_BODY).body if isinstance(node,ast.FunctionDef)}
        for name in ['left','identity','health','read_journal']:
            self.assertEqual(ast.dump(old_remote_ast[name],include_attributes=False),ast.dump(new_remote_ast[name],include_attributes=False),name)
        self.assertEqual(reader.MAX_BYTES,8_388_608)
        self.assertEqual(reader.MAX_ROWS,12_000)
    def test_explicit_pins_and_utc_window_reject_injection_and_wrong_ranges(self):
        base=['7','9','1'*32,'2'*40,'2026-11-12T12:00:00Z','2026-11-12T12:00:20Z']
        for index,value in [(0,'7;whoami'),(0,'0'),(0,'True'),(1,'9\n1'),(2,'x'*32),(3,'2'*39),
                            (3,'2'*40+"'"),(4,'2026-11-12T12:00:00+08:00'),
                            (5,'2026-11-12T12:01:00.000001Z'),(5,base[4])]:
            with self.subTest(index=index,value=value), self.assertRaises(ValueError):
                args=base.copy(); args[index]=value; reader.make_config(*args)
        self.assertEqual(reader.make_config(*base[:-1],'2026-11-12T12:01:00Z')['untilUs']-CONFIG['sinceUs'],60_000_000)

    def test_generated_remote_compiles_and_journal_scope_is_fixed_test_only(self):
        space=namespace(); args=space['journal_args']()
        self.assertEqual(args[:3],['journalctl','-u','mir2-playtest.service'])
        self.assertIn('_PID=7',args); self.assertIn('_SYSTEMD_INVOCATION_ID='+'1'*32,args)
        self.assertEqual(args[args.index('-n')+1],'12000')
        for stage in reader.STAGES: self.assertIn(reader.re.escape(stage),args[args.index('--grep')+1])
        self.assertNotIn('account',reader.STAGES)
        bad=copy.deepcopy(CONFIG); bad['queryUtc'][0]="'; systemctl stop ignored"
        with self.assertRaises(ValueError): reader.build_remote(bad)

    def test_all_known_stages_and_leaf_indices_are_retained(self):
        lines=[row('[gateway-slow-stage] stage='+stage+' duration_ms=8') for stage in sorted(reader.STAGES-{'personal_tick.leaves'})]
        lines += [row(LEAF.replace('leaf_mask=127','leaf_mask='+str(1<<bit))) for bit in range(7)]
        value=result(lines)
        self.assertEqual(len(value['diagnosticEvents']),len(reader.STAGES)-1+7)
        self.assertEqual([e['leaf_mask'] for e in value['diagnosticEvents'][-7:]],[1<<bit for bit in range(7)])
        self.assertEqual(reader.validate_frame(frame(value),CONFIG)[1],'validFrame')

    def test_identity_window_and_static_stage_filter_reject_foreign_rows(self):
        lines=[row(),row(_PID='8'),row(_PID=True),row(_SYSTEMD_INVOCATION_ID='3'*32),
               row(__REALTIME_TIMESTAMP=str(CONFIG['sinceUs']-1)),row('[gateway-slow-stage] stage=private.account duration_ms=3')]
        value=result(lines)
        self.assertEqual(len(value['diagnosticEvents']),1)
        self.assertEqual(value['projectionStats']['filteredRows'],5)
        self.assertFalse(value['boundedWindowExportComplete'])
        self.assertEqual(reader.validate_frame(frame(value),CONFIG)[1],'validFrame')

    def test_private_tokens_and_extra_raw_fields_are_never_projected(self):
        secrets=['PRIVATE-ACCOUNT','wallet-PRIVATE','cookie-PRIVATE','opaque-error-PRIVATE']
        value=result([row(LEAF+' account='+secrets[0]+' wallet='+secrets[1]+' cookie='+secrets[2]+' '+secrets[3],PRIVATE=secrets)])
        rendered=json.dumps(value)
        for secret in secrets: self.assertNotIn(secret,rendered)
        self.assertEqual(value['diagnosticEvents'][0]['ignoredTokenCount'],4)
        self.assertFalse(value['boundedWindowExportComplete'])
        self.assertEqual(reader.validate_frame(frame(value),CONFIG)[1],'validFrame')

    def test_missing_metrics_stay_null_and_mark_export_incomplete(self):
        value=result([row('[gateway-slow-stage] stage=personal_tick.leaves duration_ms=8')])
        event=value['diagnosticEvents'][0]
        for key in reader.LEAVES+('total_us','leaf_mask','outcome'): self.assertIsNone(event[key])
        self.assertEqual(event['fieldStatus'],'partial')
        self.assertFalse(value['projectionComplete']); self.assertFalse(value['boundedWindowExportComplete'])
        self.assertEqual(reader.validate_frame(frame(value),CONFIG)[1],'validFrame')

    def test_malformed_metrics_duplicate_keys_and_unknown_enums_are_null(self):
        for before,after,key in [('leaf_mask=127','leaf_mask=128','leaf_mask'),('leaf_mask=127','leaf_mask=-1','leaf_mask'),
                                 ('duration_ms=8','duration_ms=1.5','duration_ms'),('outcome=ok','outcome=PRIVATE','outcome'),
                                 ('billing_us=1000','billing_us=1000 billing_us=9','billing_us')]:
            with self.subTest(key=key,after=after):
                value=result([row(LEAF.replace(before,after))]); event=value['diagnosticEvents'][0]
                self.assertIsNone(event[key]); self.assertEqual(event['fieldStatus'],'invalid')
                self.assertFalse(value['boundedWindowExportComplete'])
                self.assertEqual(reader.validate_frame(frame(value),CONFIG)[1],'validFrame')
        value=result([row('[gateway-slow-stage] stage=socket_input.queue_residence duration_ms=9 connection=4 queued_phase=spectator phase_elapsed_ms=2 input_class=attack_v2')])
        self.assertEqual(value['diagnosticEvents'][0]['queued_phase'],'spectator')
        self.assertTrue(value['boundedWindowExportComplete'])

    def test_malformed_json_and_timestamp_are_explicitly_incomplete(self):
        value=result([row(),'{malformed',row(__REALTIME_TIMESTAMP='bad')])
        self.assertEqual(value['projectionStats']['malformedJsonOrTimestampRows'],2)
        self.assertFalse(value['boundedWindowExportComplete'])
        self.assertEqual(reader.validate_frame(frame(value),CONFIG)[1],'validFrame')

    def test_twelve_thousand_rows_fit_byte_bound_but_are_incomplete(self):
        value=result([row('[gateway-slow-stage] stage=runtime_tick.execute duration_ms=2')]*reader.MAX_ROWS)
        raw=frame(value)
        self.assertLess(len(raw),reader.MAX_BYTES)
        self.assertEqual(len(value['diagnosticEvents']),12_000)
        self.assertTrue(value['rowBoundReached']); self.assertFalse(value['boundedWindowExportComplete'])
        self.assertEqual(reader.validate_frame(raw,CONFIG)[1],'validFrame')

    def test_framing_failures_never_return_unparsed_data(self):
        good=frame(result([row()]))
        cases=[(b'','emptyStdout'),(reader.MARKER[:8],'truncatedMarker'),(b'PRIVATE','markerAbsent'),
               (good[:-1],'truncatedFrame'),(good+good,'multipleFrames'),(b'PRIVATE\n'+good,'unframedStdout'),
               (reader.MARKER+b'\xff\n','invalidUtf8Payload'),(reader.MARKER+b'{bad}\n','invalidJsonPayload'),
               (b'x'*(reader.MAX_BYTES+1),'stdoutByteBound')]
        for raw,category in cases:
            with self.subTest(category=category):
                value,actual,unused=reader.validate_frame(raw,CONFIG)
                self.assertIsNone(value); self.assertEqual(actual,category)

    def test_result_pin_type_privacy_and_completion_forgery_fail_closed(self):
        original=result([row()])
        changes=[lambda v:v['targetIdentityAfter'].update(pid=8),lambda v:v.update(backendRevision='3'*40),
                 lambda v:v.update(rowBoundReached=True),lambda v:v.update(PRIVATE='payload'),
                 lambda v:v['diagnosticEvents'][0].update(leaf_mask=True),
                 lambda v:v['diagnosticEvents'][0].update(stage=[]),lambda v:v['diagnosticEvents'][0].update(outcome={}),
                 lambda v:v['diagnosticEvents'][0].update(PRIVATE='payload'),
                 lambda v:v.update(stageCounts={'personal_tick.leaves':True})]
        for change in changes:
            value=copy.deepcopy(original); change(value)
            safe,category,unused=reader.validate_frame(frame(value),CONFIG)
            self.assertIsNone(safe); self.assertNotEqual(category,'validFrame')
        error={'schema':reader.SCHEMA,'ok':False,'errorClass':'identityMismatch','boundedWindowExportComplete':False,'serviceOperations':[],'networkOrVpnOperations':[]}
        self.assertEqual(reader.validate_frame(frame(error),CONFIG)[1],'validFrame')
        error['errorClass']=['PRIVATE']
        self.assertEqual(reader.validate_frame(frame(error),CONFIG)[1],'unsafeErrorFrame')

    def test_identity_before_after_change_cannot_produce_a_success_result(self):
        space=namespace(); calls=[]
        def identity(unused):
            calls.append(1); value=dict(IDENTITY)
            if len(calls)>1: value['pid']=8
            return value
        space['identity']=identity
        space['read_journal']=lambda unused:([],{'exitStatus':0,'stdoutBytes':0,'stderrBytes':0,'stderrSha256':hashlib.sha256(b'').hexdigest()})
        with self.assertRaises(space['ReadFailure']): space['collect']()

    def test_non_dict_identity_and_health_objects_are_typed_frame_rejections(self):
        original=result([row()])
        for key in ('targetIdentityBefore','targetIdentityAfter','healthBefore','healthAfter'):
            for malformed in (None,[],1,'PRIVATE raw scalar',True):
                with self.subTest(field=key,value=malformed):
                    value=copy.deepcopy(original); value[key]=malformed
                    safe,category,unused=reader.validate_frame(frame(value),CONFIG)
                    self.assertIsNone(safe)
                    self.assertEqual(category,'identityMismatch' if key.startswith('targetIdentity') else 'revisionMismatch')
                    self.assertNotIn('PRIVATE',json.dumps({'queryResult':safe,'parseCategory':category}))

    def test_non_dict_expected_pins_and_event_objects_fail_without_exceptions(self):
        original=result([row()]); good=frame(original)
        for malformed in (None,[],1,'PRIVATE raw scalar',True):
            with self.subTest(value=malformed):
                safe,category,unused=reader.validate_frame(good,malformed)
                self.assertIsNone(safe); self.assertEqual(category,'invalidExpectedPins')
                self.assertFalse(reader.valid_event(original['diagnosticEvents'][0],malformed))
                value=copy.deepcopy(original); value['diagnosticEvents']=[malformed]
                safe,category,unused=reader.validate_frame(frame(value),CONFIG)
                self.assertIsNone(safe); self.assertEqual(category,'unsafeEvent')

    def test_host_key_requires_ed25519_and_exact_pin(self):
        with self.assertRaises(reader.ReadFailure): reader.verify_host_key(Key())
        pin='SHA256:'+base64.b64encode(hashlib.sha256(Key().asbytes()).digest()).decode().rstrip('=')
        with patch.object(reader,'PIN',pin):
            reader.verify_host_key(Key())
            bad=Key(); bad.get_name=lambda:'ssh-rsa'
            with self.assertRaises(reader.ReadFailure): reader.verify_host_key(bad)

    def test_host_key_mismatch_never_reaches_authentication(self):
        channel=Channel(frame(result([row()])),0); transport=Transport(channel)
        tcp=types.SimpleNamespace(shutdown=lambda unused:None,close=lambda:None)
        with tempfile.TemporaryDirectory(dir=HERE) as directory:
            self.assertEqual(Path(directory).resolve().parent,HERE)
            target=Path(directory)/'pin-mismatch.json'
            args=['--output',str(target),'--pid','7','--start-ticks','9','--invocation-id','1'*32,'--revision','2'*40,'--since',CONFIG['windowUtc'][0],'--until',CONFIG['windowUtc'][1]]
            with patch.object(reader.sys.stdin,'isatty',return_value=True),patch.object(reader.getpass,'getpass',return_value='PRIVATE password fixture'),patch.object(reader.socket,'create_connection',return_value=tcp),patch.dict(sys.modules,{'paramiko':types.SimpleNamespace(Transport=lambda unused:transport)}),redirect_stdout(io.StringIO()):
                self.assertEqual(reader.main(args),1)
            output=json.loads(target.read_text())
            self.assertFalse(transport.auth_called); self.assertIsNone(channel.command)
            self.assertFalse(output['hostKeyVerifiedBeforeAuthentication'])
            self.assertEqual(output['localErrorClass'],'hostKeyMismatchBeforeAuthentication')
            self.assertNotIn('PRIVATE',target.read_text())

    def test_remote_deadline_and_before_after_revision_checks_are_not_optional(self):
        space=namespace()
        with self.assertRaises(space['ReadFailure']): space['left'](reader.time.monotonic()-1)
        calls=[]
        def health(unused):
            calls.append(1)
            if len(calls)>1: raise space['ReadFailure']('healthRevisionMismatch')
            return {'ok':True,'revision':CONFIG['revision']}
        space['health']=health
        space['read_journal']=lambda unused:([],{'exitStatus':0,'stdoutBytes':0,'stderrBytes':0,'stderrSha256':hashlib.sha256(b'').hexdigest()})
        with self.assertRaises(space['ReadFailure']): space['collect']()
        self.assertEqual(len(calls),2)

    def test_missing_or_injected_cli_pins_fail_before_prompt_or_network(self):
        with patch.object(reader.getpass,'getpass') as password, patch.object(reader.socket,'create_connection') as connect, redirect_stderr(io.StringIO()):
            for args in [[],['--output','unused','--pid','7;PRIVATE','--start-ticks','9','--invocation-id','1'*32,'--revision','2'*40,'--since',CONFIG['windowUtc'][0],'--until',CONFIG['windowUtc'][1]]]:
                with self.assertRaises(SystemExit): reader.main(args)
            password.assert_not_called(); connect.assert_not_called()

    def test_mocked_transport_truthful_exit_status_no_raw_stream_and_exclusive_output(self):
        pin='SHA256:'+base64.b64encode(hashlib.sha256(Key().asbytes()).digest()).decode().rstrip('=')
        with tempfile.TemporaryDirectory(dir=HERE) as directory:
            self.assertEqual(Path(directory).resolve().parent,HERE)
            for status in (0,1,-1):
                channel=Channel(frame(result([row()])),status); transport=Transport(channel)
                tcp=types.SimpleNamespace(shutdown=lambda unused:None,close=lambda:None)
                target=Path(directory)/('status-'+str(status)+'.json')
                args=['--output',str(target),'--pid','7','--start-ticks','9','--invocation-id','1'*32,'--revision','2'*40,'--since',CONFIG['windowUtc'][0],'--until',CONFIG['windowUtc'][1]]
                with patch.object(reader,'PIN',pin),patch.object(reader.sys.stdin,'isatty',return_value=True),patch.object(reader.getpass,'getpass',return_value='PRIVATE password fixture'),patch.object(reader.socket,'create_connection',return_value=tcp),patch.dict(sys.modules,{'paramiko':types.SimpleNamespace(Transport=lambda unused:transport)}),redirect_stdout(io.StringIO()):
                    code=reader.main(args)
                output=json.loads(target.read_text())
                self.assertEqual(code,0 if status==0 else 1)
                self.assertEqual(output['remoteExitStatusReceived'],status>=0)
                self.assertEqual(output['remoteExitStatus'],status if status>=0 else None)
                self.assertEqual(output['exitStatusMethodReturn'],status)
                self.assertEqual(output['ok'],status==0)
                self.assertNotIn('PRIVATE',target.read_text())
                self.assertTrue(channel.stdin_closed)
                command=shlex.split(channel.command)
                self.assertEqual(command[:6],['sudo','-n','--','python3','-u','-c'])
                compile(command[6],'<offline-ssh-command>','exec')
                self.assertFalse(output['clockEvidence']['exactCrossClockLatencyInferred'])
                self.assertIsNotNone(output['clockEvidence']['localRequestUnixUs'])
                before=target.read_bytes()
                with patch.object(reader.getpass,'getpass') as password,patch.object(reader.socket,'create_connection') as connect,redirect_stderr(io.StringIO()),self.assertRaises(SystemExit): reader.main(args)
                password.assert_not_called(); connect.assert_not_called(); self.assertEqual(target.read_bytes(),before)


if __name__ == '__main__': unittest.main(verbosity=2)
