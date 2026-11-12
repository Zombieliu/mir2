"""PREP ONLY: explicitly pinned TEST journal readback; importing does no I/O."""
import argparse
import base64
import datetime as dt
import getpass
import hashlib
import hmac
import json
import re
import shlex
import socket
import sys
import threading
import time
from pathlib import Path

PIN = 'SHA256:6M4ouOk6QvFj1qIEmjj9/LOa8/enGiqZ/n+m7JHo7q8'
TRANSPORT_SOURCE_SHA256 = '2e2e5514f499c3bb635065bbd952bf48a428ebf8fe5c3317aaf8a76b6f07c795'
BASE_SOURCE_SHA256 = '60f4ca8fabc2341c803dca254281bba86b277ac0fd7eca722810ff27a3cac127'
PARENT_READER_SHA256 = 'c477d83fdc11c781b3e003e89b24ff54f21128712e178892b3a05b2e20de5dd7'
MARKER = b'MIR2_WRITER_DIAGNOSTICS_JSON_V1 '
SCHEMA = 'mir2.writer-diagnostics-readonly.v1'
MAX_BYTES = 8_388_608
MAX_ROWS = 12_000

# The exact same static projection executes locally and remotely. No user text
# enters this source; build_remote embeds only validated pins/canonical times.
COMMON = r'''

STAGES = frozenset({
 'personal_tick.leaves','owner_health.prepare.zone_lock_wait','owner_health.prepare.zone_lock_held',
 'owner_health.commit.zone_lock_wait','owner_health.commit.zone_lock_held','runtime_tick.execute',
 'runtime_tick.total','serial_gate.zone_outbound_wait','socket_input.queue_residence','shared_session.inner_execute',
 'session_action.execute','session_action.flush','flush.responses','flush.external_state','flush.save',
 'runtime_tick.flush','runtime_tick.save_checkpoint','runtime_tick.serial_gate_wait',
 'serial_gate.input_wait','socket_turn.input','socket_turn.input_owner_heartbeat','snapshot.build_and_send',
})
LEAVES=('billing_us','gift_us','checkpoint_begin_us','personal_timers_us','source_dispatch_us','finalize_us','finish_us')
SLOW='[gateway-slow-stage]'
RESPONSE='[gateway-response-write]'
BLOCK='[gateway-zone-outbound-block]'
TRACE='[gateway-zone-delivery-trace]'
BLOCK_STAGES=frozenset({'read_gate','health_schedule','health_run','health_await','send','completion_to_pop'})
TRACE_STAGES=frozenset({'enqueue','full','closed','dequeue','gate','send','sent','stale','health-empty','health-error','overload','send-error'})
PREFIXES=frozenset({SLOW,RESPONSE,BLOCK,TRACE})
CONTEXT=('duration_ms','connection','phase_elapsed_ms','queued_phase','input_class')
RESPONSE_FIELDS=('messages','bytes','flushes','serialization_us','lock_wait_us','feed_us','flush_us','elapsed_us')
BLOCK_FIELDS=('registration','batchID','batchIndex','normalDepth','ownerDepth','elapsedUs','recordedUnixMs','kind','lane')
TRACE_FIELDS=('serverTraceID','registration','producerUnixMs','elapsedUs','queueAgeUs','gateWaitUs','kind')
REDACTED_TRACE_FIELDS=frozenset({'recipientZoneID','targetObjectID','hasPosition','x','y'})
ENUMS={
 'outcome':frozenset({'ok','error'}),
 'queued_phase':frozenset({'select_wait','input','input_gate','input_owner_heartbeat','chat','injection',
  'spectator','ai_live','runtime_tick','identity_cache','identity_database','resume_credential','owner_heartbeat','tick_gate','preflight'}),
 'input_class':frozenset({'move','attack_v2','other'}),
 'kind':frozenset({'OwnerHealth','UserLocation','ObjectPlayer','ObjectRemove','Other'}),
 'lane':frozenset({'owner','normal'}),
}
NUMBERS=('duration_ms','total_us')+LEAVES+('leaf_mask','connection','phase_elapsed_ms')+RESPONSE_FIELDS+tuple(k for k in BLOCK_FIELDS+TRACE_FIELDS if k not in ENUMS)
FIELDS=tuple(dict.fromkeys(NUMBERS+tuple(ENUMS)))
BASE_KEYS=frozenset({'atUnixUs','pid','invocationId','prefix','stage','fieldStatus','invalidFields',
                     'missingFields','ignoredTokenCount','redactedTokenCount'})
REMOTE_ERRORS=frozenset({'remoteBudget','identityReadFailed','identityMismatch','startTicksMismatch',
 'procStatInvalid','healthReadFailed','healthRevisionMismatch','journalReadFailed','journalByteBound',
 'journalRowsBound','journalUtf8Invalid','projectedOutputBound','unexpectedRemoteError'})

class ReadFailure(Exception):
 def __init__(self,code):self.code=code

def uint(value,maximum=18446744073709551615):
 if type(value) is int:return value if 0<=value<=maximum else None
 if not isinstance(value,str) or len(value)>20 or not re.fullmatch(r'[0-9]+',value):return None
 number=int(value)
 return number if number<=maximum else None

def no_duplicates(pairs):
 result={}
 for key,value in pairs:
  if key in result:raise ValueError('duplicateJsonKey')
  result[key]=value
 return result

def strict_json(raw):
 def reject_constant(unused):raise ValueError('nonFiniteJson')
 return json.loads(raw,object_pairs_hook=no_duplicates,parse_constant=reject_constant)

def fields_for(prefix,stage):
 if prefix==SLOW:return CONTEXT+(('total_us',)+LEAVES+('leaf_mask','outcome') if stage=='personal_tick.leaves' else ())
 if prefix==RESPONSE:return RESPONSE_FIELDS
 if prefix==BLOCK:return BLOCK_FIELDS
 if prefix==TRACE:return TRACE_FIELDS
 return ()

def required_for(prefix,stage):
 if prefix==SLOW:return ('duration_ms','total_us')+LEAVES+('leaf_mask','outcome') if stage=='personal_tick.leaves' else ('duration_ms',)
 return fields_for(prefix,stage)

def stage_valid(prefix,stage):
 return ((prefix==RESPONSE and stage is None) or
         prefix==SLOW and type(stage) is str and stage in STAGES or
         prefix==BLOCK and type(stage) is str and stage in BLOCK_STAGES or
         prefix==TRACE and type(stage) is str and stage in TRACE_STAGES)

def aggregate(events):
 counts={};maxima={};prefix_counts={};staged={};elapsed={}
 for event in events:
  prefix,stage=event['prefix'],event['stage']
  prefix_counts[prefix]=prefix_counts.get(prefix,0)+1
  if stage is not None:
   by_stage=staged.setdefault(prefix,{})
   by_stage[stage]=by_stage.get(stage,0)+1
  if prefix==SLOW:
   counts[stage]=counts.get(stage,0)+1
   if event.get('duration_ms') is not None:maxima[stage]=max(maxima.get(stage,0),event['duration_ms'])
  value=event.get('elapsed_us') if prefix==RESPONSE else event.get('elapsedUs') if prefix in {BLOCK,TRACE} else None
  if value is not None:elapsed[prefix]=max(elapsed.get(prefix,0),value)
 return {'stageCounts':counts,'maximumDurationMsByStage':maxima,'prefixCounts':prefix_counts,
         'stageCountsByPrefix':staged,'maximumElapsedUsByPrefix':elapsed}

def project(lines,config):
 events=[];filtered=0;malformed=0
 for line in lines:
  try:row=strict_json(line)
  except (ValueError,TypeError,RecursionError):malformed+=1;continue
  if not isinstance(row,dict) or uint(row.get('_PID'))!=config['pid'] or row.get('_SYSTEMD_INVOCATION_ID')!=config['invocationId']:
   filtered+=1;continue
  message=row.get('MESSAGE');at=uint(row.get('__REALTIME_TIMESTAMP'))
  if at is None:malformed+=1;continue
  if not config['sinceUs']<=at<=config['untilUs'] or not isinstance(message,str):filtered+=1;continue
  head=message.split(None,2)
  prefix=head[0] if head else None
  stage=None if prefix==RESPONSE else head[1][6:] if len(head)>1 and head[1].startswith('stage=') else None
  if prefix not in PREFIXES or not stage_valid(prefix,stage):filtered+=1;continue
  fields=fields_for(prefix,stage)
  event=dict.fromkeys(fields)
  event.update(atUnixUs=at,pid=config['pid'],invocationId=config['invocationId'],prefix=prefix,stage=stage,
               invalidFields=[],missingFields=[],ignoredTokenCount=0,redactedTokenCount=0)
  seen=set()
  tokens=message.split()[1 if prefix==RESPONSE else 2:] if len(message)<=8192 else ['unrecognizedToken']
  for token in tokens:
   key,sep,value=token.partition('=')
   if sep and prefix==TRACE and key in REDACTED_TRACE_FIELDS:
    event['redactedTokenCount']+=1;continue
   if not sep or key not in fields:event['ignoredTokenCount']+=1;continue
   if key in seen:
    event[key]=None
    if key not in event['invalidFields']:event['invalidFields'].append(key)
    continue
   seen.add(key)
   if key in ENUMS:
    allowed=ENUMS[key]
    if prefix==TRACE and key=='kind':allowed=allowed-{'OwnerHealth','Other'}
    event[key]=value if value in allowed else None
   else:
    event[key]=uint(value,127 if key=='leaf_mask' else 18446744073709551615)
    if key=='connection' and event[key]==0:event[key]=None
   if event[key] is None:event['invalidFields'].append(key)
  event['missingFields']=[key for key in required_for(prefix,stage) if event[key] is None]
  event['fieldStatus']='invalid' if event['invalidFields'] or event['ignoredTokenCount'] else ('partial' if event['missingFields'] else 'valid')
  events.append(event)
 values=aggregate(events)
 stats={'filteredRows':filtered,'malformedJsonOrTimestampRows':malformed,
        'invalidMetricRows':sum(e['fieldStatus']=='invalid' for e in events),
        'partialMetricRows':sum(e['fieldStatus']=='partial' for e in events)}
 return events,values['stageCounts'],values['maximumDurationMsByStage'],stats

def valid_event(event,config):
 if not isinstance(config,dict) or not isinstance(event,dict):return False
 prefix,stage=event.get('prefix'),event.get('stage')
 if type(prefix) is not str or prefix not in PREFIXES or not stage_valid(prefix,stage):return False
 fields=fields_for(prefix,stage)
 if set(event)!=BASE_KEYS|set(fields):return False
 if event['pid']!=config['pid'] or type(event['pid']) is not int or event['invocationId']!=config['invocationId']:return False
 at=event['atUnixUs']
 if type(at) is not int or not config['sinceUs']<=at<=config['untilUs']:return False
 for key in fields:
  value=event[key]
  if value is None:continue
  if key in ENUMS:
   allowed=ENUMS[key]-{'OwnerHealth','Other'} if prefix==TRACE and key=='kind' else ENUMS[key]
   if type(value) is not str or value not in allowed:return False
  elif type(value) is not int or uint(value,127 if key=='leaf_mask' else 18446744073709551615) is None or (key=='connection' and value==0):return False
 for key in ('invalidFields','missingFields'):
  if not isinstance(event[key],list) or len(event[key])>len(fields) or any(type(v) is not str or v not in fields for v in event[key]) or len(set(event[key]))!=len(event[key]):return False
 for key in ('ignoredTokenCount','redactedTokenCount'):
  if type(event[key]) is not int or not 0<=event[key]<=8192:return False
 if prefix!=TRACE and event['redactedTokenCount']!=0:return False
 if event['missingFields']!=[key for key in required_for(prefix,stage) if event[key] is None]:return False
 if any(event[key] is not None for key in event['invalidFields']):return False
 expected='invalid' if event['invalidFields'] or event['ignoredTokenCount'] else ('partial' if event['missingFields'] else 'valid')
 return event['fieldStatus']==expected

def typed_equal(left,right):
 if type(left) is not type(right):return False
 if isinstance(left,dict):return left.keys()==right.keys() and all(typed_equal(left[k],right[k]) for k in left)
 if isinstance(left,list):return len(left)==len(right) and all(typed_equal(a,b) for a,b in zip(left,right))
 return left==right

def flags_from_environ(raw):
 unknown={'readComplete':False,'zoneDeliveryTraceLiteral1':None,'slowStagePositiveU64':None}
 if type(raw) is not bytes or len(raw)>262144:return unknown
 selected={}
 for token in raw.split(b'\0'):
  key,sep,value=token.partition(b'=')
  if sep and key in {b'MIR2_GATEWAY_ZONE_DELIVERY_TRACE',b'MIR2_GATEWAY_SLOW_STAGE_MS'}:
   if key in selected:return unknown
   selected[key]=value
 try:
  threshold=selected.get(b'MIR2_GATEWAY_SLOW_STAGE_MS',b'').decode('utf-8').strip()
 except UnicodeDecodeError:threshold=''
 valid=False
 if re.fullmatch(r'\+?[0-9]+',threshold):
  canonical=threshold.lstrip('+').lstrip('0') or '0'
  value=uint(canonical)
  valid=value is not None and value>0
 return {'readComplete':True,'zoneDeliveryTraceLiteral1':selected.get(b'MIR2_GATEWAY_ZONE_DELIVERY_TRACE')==b'1',
         'slowStagePositiveU64':valid}

'''
exec(COMMON)

REMOTE_BODY = r'''
import os,selectors,subprocess,urllib.request

def left(deadline,cap=20):
 value=deadline-time.monotonic()
 if value<=0: raise ReadFailure('remoteBudget')
 return min(value,cap)

def identity(deadline):
 try:
  done=subprocess.run(['systemctl','show','mir2-playtest.service','--property=MainPID','--property=InvocationID','--property=ActiveState'],stdin=subprocess.DEVNULL,capture_output=True,text=True,timeout=left(deadline),check=True)
  if len(done.stdout)>65536: raise ReadFailure('identityReadFailed')
  unit=dict(line.split('=',1) for line in done.stdout.splitlines() if '=' in line)
  if uint(unit.get('MainPID'))!=CONFIG['pid'] or unit.get('InvocationID')!=CONFIG['invocationId'] or unit.get('ActiveState')!='active': raise ReadFailure('identityMismatch')
  with Path('/proc',str(CONFIG['pid']),'stat').open() as stream: raw=stream.read(65537)
  if len(raw)>65536 or ')' not in raw: raise ReadFailure('procStatInvalid')
  fields=raw[raw.rfind(')')+2:].split()
  if len(fields)<20 or uint(fields[19])!=CONFIG['processStartTicks']: raise ReadFailure('startTicksMismatch')
 except ReadFailure: raise
 except Exception: raise ReadFailure('identityReadFailed') from None
 return {'pid':CONFIG['pid'],'processStartTicks':CONFIG['processStartTicks'],'invocationId':CONFIG['invocationId'],'activeState':'active'}

def health(deadline):
 try:
  opener=urllib.request.build_opener(urllib.request.ProxyHandler({}))
  with opener.open('http://127.0.0.1:7210/health',timeout=left(deadline,5)) as response: raw=response.read(1048577)
  if len(raw)>1048576: raise ReadFailure('healthReadFailed')
  value=strict_json(raw)
  if not isinstance(value,dict) or value.get('revision')!=CONFIG['revision'] or value.get('ok') is not True: raise ReadFailure('healthRevisionMismatch')
 except ReadFailure: raise
 except Exception: raise ReadFailure('healthReadFailed') from None
 return {'ok':True,'revision':CONFIG['revision']}

def journal_args():
 staged=lambda prefix,values:re.escape(prefix)+r' stage=(?:'+'|'.join(re.escape(stage) for stage in sorted(values))+r')(?:\s|$)'
 grep=r'^(?:'+staged(SLOW,STAGES)+'|'+re.escape(RESPONSE)+r'(?:\s|$)|'+staged(BLOCK,BLOCK_STAGES)+'|'+staged(TRACE,TRACE_STAGES)+')'
 return ['journalctl','-u','mir2-playtest.service','_PID='+str(CONFIG['pid']),'_SYSTEMD_INVOCATION_ID='+CONFIG['invocationId'],'--since',CONFIG['queryUtc'][0],'--until',CONFIG['queryUtc'][1],'-n',str(MAX_ROWS),'--grep',grep,'--no-pager','-o','json','--output-fields=MESSAGE,__REALTIME_TIMESTAMP,_PID,_SYSTEMD_INVOCATION_ID']

def diagnostic_flags(deadline):
 left(deadline)
 try:
  with Path('/proc',str(CONFIG['pid']),'environ').open('rb') as stream:raw=stream.read(262145)
 except Exception:
  return flags_from_environ(None)
 left(deadline)
 return flags_from_environ(raw)

def read_journal(deadline):
 process=subprocess.Popen(journal_args(),stdin=subprocess.DEVNULL,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
 output=[]; count=0; stderr_bytes=0; stderr_hash=hashlib.sha256(); stdout_bytes=0; selector=selectors.DefaultSelector()
 try:
  for stream in (process.stdout,process.stderr):
   os.set_blocking(stream.fileno(),False); selector.register(stream,selectors.EVENT_READ)
  while selector.get_map():
   for key,unused in selector.select(timeout=left(deadline,.2)):
    chunk=os.read(key.fileobj.fileno(),32768)
    if not chunk: selector.unregister(key.fileobj); continue
    count+=len(chunk)
    if count>MAX_BYTES: raise ReadFailure('journalByteBound')
    if key.fileobj is process.stdout: output.append(chunk); stdout_bytes+=len(chunk)
    else: stderr_bytes+=len(chunk); stderr_hash.update(chunk)
  status=process.wait(timeout=left(deadline))
  if status!=0: raise ReadFailure('journalReadFailed')
  try: lines=b''.join(output).decode('utf-8').splitlines()
  except UnicodeDecodeError: raise ReadFailure('journalUtf8Invalid') from None
  if len(lines)>MAX_ROWS: raise ReadFailure('journalRowsBound')
  return lines,{'exitStatus':status,'stdoutBytes':stdout_bytes,'stderrBytes':stderr_bytes,'stderrSha256':stderr_hash.hexdigest()}
 finally:
  selector.close()
  if process.poll() is None: process.kill()
  process.stdout.close(); process.stderr.close()

def collect():
 deadline=time.monotonic()+20; clock_before=time.time_ns()//1000
 before=identity(deadline); health_before=health(deadline)
 flags=diagnostic_flags(deadline)
 lines,journal=read_journal(deadline)
 events,counts,maxima,stats=project(lines,CONFIG)
 health_after=health(deadline); after=identity(deadline); left(deadline)
 if before!=after: raise ReadFailure('identityMismatch')
 reached=len(lines)>=MAX_ROWS
 projection_complete=not any(stats.values())
 return {'schema':SCHEMA,'ok':True,'targetIdentityBefore':before,'targetIdentityAfter':after,
  'identityStableAcrossRead':True,'backendRevision':CONFIG['revision'],'healthBefore':health_before,'healthAfter':health_after,
  'diagnosticEvents':events,**aggregate(events),'diagnosticFlags':flags,
  'journalWindowUtc':CONFIG['windowUtc'],'journalRowsBound':MAX_ROWS,'journalRowsRead':len(lines),
  'rowBoundReached':reached,'projectionComplete':projection_complete,'boundedWindowExportComplete':not reached and projection_complete,
  'projectionStats':stats,'journalRead':journal,'remoteClockBeforeUnixUs':clock_before,'remoteClockAfterUnixUs':time.time_ns()//1000,
  'remoteQueryMaxSeconds':20,'serviceOperations':[],'networkOrVpnOperations':[]}

if __name__=='__main__':
 try:
  result=collect(); data=json.dumps(result,separators=(',',':'),allow_nan=False)
  if len(MARKER)+len(data.encode())+1>MAX_BYTES: raise ReadFailure('projectedOutputBound')
  print(MARKER.decode()+data,flush=True)
 except BaseException as error:
  category=error.code if isinstance(error,ReadFailure) and error.code in REMOTE_ERRORS else 'unexpectedRemoteError'
  print(MARKER.decode()+json.dumps({'schema':SCHEMA,'ok':False,'errorClass':category,'boundedWindowExportComplete':False,'serviceOperations':[],'networkOrVpnOperations':[]}),flush=True)
  raise SystemExit(1)
'''


def utc(value):
    if not isinstance(value, str) or not re.fullmatch(r'\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d(?:\.\d{1,6})?(?:Z|\+00:00)', value):
        raise ValueError('Explicit UTC ISO timestamp required')
    parsed = dt.datetime.fromisoformat(value.replace('Z', '+00:00'))
    delta = parsed - dt.datetime(1970, 1, 1, tzinfo=dt.timezone.utc)
    micros = (delta.days * 86400 + delta.seconds) * 1_000_000 + delta.microseconds
    if micros < 0:
        raise ValueError('UTC timestamp precedes epoch')
    return parsed, micros


def make_config(pid, start_ticks, invocation, revision, since, until):
    if not isinstance(pid, str) or not re.fullmatch(r'[1-9][0-9]{0,9}', pid) or int(pid) > 2147483647:
        raise ValueError('Positive numeric PID required')
    if not isinstance(start_ticks, str) or not re.fullmatch(r'[1-9][0-9]{0,19}', start_ticks) or int(start_ticks) > 18446744073709551615:
        raise ValueError('Positive process start ticks required')
    if not isinstance(invocation, str) or not re.fullmatch(r'[0-9a-f]{32}', invocation):
        raise ValueError('Exact lowercase 32-hex invocation required')
    if not isinstance(revision, str) or not re.fullmatch(r'[0-9a-f]{40}', revision):
        raise ValueError('Exact lowercase 40-hex revision required')
    first, first_us = utc(since)
    last, last_us = utc(until)
    if not 0 < last_us - first_us <= 60_000_000:
        raise ValueError('UTC window must be positive and at most 60 seconds')
    return {'pid': int(pid), 'processStartTicks': int(start_ticks), 'invocationId': invocation,
            'revision': revision, 'sinceUs': first_us, 'untilUs': last_us,
            'windowUtc': [point.isoformat().replace('+00:00', 'Z') for point in (first, last)],
            'queryUtc': [point.strftime('%Y-%m-%d %H:%M:%S.%f UTC') for point in (first, last)]}


def build_remote(config):
    checked = make_config(str(config['pid']), str(config['processStartTicks']), config['invocationId'],
                          config['revision'], *config['windowUtc'])
    if config != checked:
        raise ValueError('Noncanonical query configuration')
    prelude = 'import json,re,time,hashlib\nfrom pathlib import Path\n'
    for name, value in [('CONFIG', config), ('MARKER', MARKER), ('SCHEMA', SCHEMA),
                        ('MAX_BYTES', MAX_BYTES), ('MAX_ROWS', MAX_ROWS)]:
        prelude += name + '=' + repr(value) + '\n'
    remote = prelude + COMMON + REMOTE_BODY
    compile(remote, '<pinned-tick-diagnostics-remote>', 'exec')
    return remote


def validate_frame(raw, config):
    lines = raw.splitlines(keepends=True)
    frames = [line for line in lines if line.startswith(MARKER)]
    meta = {'frameCount': len(frames), 'unframedStdoutBytes': len(raw) - sum(map(len, frames))}
    def reject(category): return None, category, meta
    if not isinstance(config, dict): return reject('invalidExpectedPins')
    if len(raw) > MAX_BYTES: return reject('stdoutByteBound')
    if not raw: return reject('emptyStdout')
    if not frames: return reject('truncatedMarker' if MARKER.startswith(raw) else 'markerAbsent')
    if len(frames) != 1: return reject('multipleFrames')
    if not frames[0].endswith(b'\n'): return reject('truncatedFrame')
    if meta['unframedStdoutBytes']: return reject('unframedStdout')
    try: value = strict_json(frames[0][len(MARKER):])
    except UnicodeDecodeError: return reject('invalidUtf8Payload')
    except (ValueError, TypeError, RecursionError): return reject('invalidJsonPayload')
    if not isinstance(value, dict) or value.get('schema') != SCHEMA or type(value.get('ok')) is not bool:
        return reject('schemaMismatch')
    if not value['ok']:
        allowed = {'schema', 'ok', 'errorClass', 'boundedWindowExportComplete', 'serviceOperations', 'networkOrVpnOperations'}
        if set(value) != allowed or type(value['errorClass']) is not str or value['errorClass'] not in REMOTE_ERRORS or value['boundedWindowExportComplete'] is not False or value['serviceOperations'] != [] or value['networkOrVpnOperations'] != []:
            return reject('unsafeErrorFrame')
        return value, 'validFrame', meta
    allowed = {'schema','ok','targetIdentityBefore','targetIdentityAfter','identityStableAcrossRead','backendRevision','healthBefore','healthAfter','diagnosticEvents','stageCounts','maximumDurationMsByStage','prefixCounts','stageCountsByPrefix','maximumElapsedUsByPrefix','diagnosticFlags','journalWindowUtc','journalRowsBound','journalRowsRead','rowBoundReached','projectionComplete','boundedWindowExportComplete','projectionStats','journalRead','remoteClockBeforeUnixUs','remoteClockAfterUnixUs','remoteQueryMaxSeconds','serviceOperations','networkOrVpnOperations'}
    if set(value) != allowed: return reject('unsafeResultFields')
    identity = {'pid':config['pid'],'processStartTicks':config['processStartTicks'],'invocationId':config['invocationId'],'activeState':'active'}
    for key in ('targetIdentityBefore','targetIdentityAfter'):
        observed = value[key]
        if not isinstance(observed, dict): return reject('identityMismatch')
        if observed != identity or type(observed.get('pid')) is not int or type(observed.get('processStartTicks')) is not int:
            return reject('identityMismatch')
    if value['identityStableAcrossRead'] is not True:
        return reject('identityMismatch')
    if value['backendRevision'] != config['revision']: return reject('revisionMismatch')
    for key in ('healthBefore','healthAfter'):
        observed = value[key]
        if not isinstance(observed, dict): return reject('revisionMismatch')
        if observed != {'ok':True,'revision':config['revision']} or observed.get('ok') is not True:
            return reject('revisionMismatch')
    if value['journalWindowUtc'] != config['windowUtc']: return reject('windowMismatch')
    rows = value['journalRowsRead']
    if value['journalRowsBound'] != MAX_ROWS or type(value['journalRowsBound']) is not int or type(rows) is not int or not 0 <= rows <= MAX_ROWS:
        return reject('rowBoundMetadataInvalid')
    events = value['diagnosticEvents']; stats = value['projectionStats']
    if not isinstance(events, list) or len(events) > rows or any(not valid_event(event, config) for event in events): return reject('unsafeEvent')
    stat_keys = {'filteredRows','malformedJsonOrTimestampRows','invalidMetricRows','partialMetricRows'}
    if not isinstance(stats, dict) or set(stats) != stat_keys or any(type(n) is not int or not 0 <= n <= rows for n in stats.values()): return reject('projectionStatsInvalid')
    if len(events)+stats['filteredRows']+stats['malformedJsonOrTimestampRows'] != rows or stats['invalidMetricRows'] != sum(e['fieldStatus']=='invalid' for e in events) or stats['partialMetricRows'] != sum(e['fieldStatus']=='partial' for e in events): return reject('projectionStatsInconsistent')
    expected_aggregates = aggregate(events)
    for key, expected in expected_aggregates.items():
        if not typed_equal(value[key], expected): return reject('aggregateMismatch')
    flags = value['diagnosticFlags']
    if not isinstance(flags, dict) or set(flags) != {'readComplete','zoneDeliveryTraceLiteral1','slowStagePositiveU64'} or type(flags['readComplete']) is not bool:
        return reject('unsafeDiagnosticFlags')
    if flags['readComplete']:
        if any(type(flags[key]) is not bool for key in ['zoneDeliveryTraceLiteral1','slowStagePositiveU64']): return reject('unsafeDiagnosticFlags')
    elif any(flags[key] is not None for key in ['zoneDeliveryTraceLiteral1','slowStagePositiveU64']):
        return reject('unsafeDiagnosticFlags')
    complete = not any(stats.values())
    if value['rowBoundReached'] is not (rows >= MAX_ROWS) or value['projectionComplete'] is not complete or value['boundedWindowExportComplete'] is not (rows < MAX_ROWS and complete): return reject('completionMetadataInconsistent')
    journal = value['journalRead']
    if not isinstance(journal, dict) or set(journal) != {'exitStatus','stdoutBytes','stderrBytes','stderrSha256'} or type(journal['exitStatus']) is not int or journal['exitStatus'] != 0 or any(type(journal[key]) is not int or not 0 <= journal[key] <= MAX_BYTES for key in ('stdoutBytes','stderrBytes')) or journal['stdoutBytes']+journal['stderrBytes'] > MAX_BYTES or not isinstance(journal['stderrSha256'], str) or not re.fullmatch(r'[0-9a-f]{64}', journal['stderrSha256']): return reject('journalStatusInvalid')
    if any(type(value[key]) is not int or uint(value[key]) is None for key in ('remoteClockBeforeUnixUs','remoteClockAfterUnixUs')) or value['remoteClockAfterUnixUs'] < value['remoteClockBeforeUnixUs']: return reject('remoteClockInvalid')
    if value['remoteQueryMaxSeconds'] != 20 or type(value['remoteQueryMaxSeconds']) is not int or value['serviceOperations'] != [] or value['networkOrVpnOperations'] != []: return reject('queryScopeInvalid')
    return value, 'validFrame', meta


def verify_host_key(key):
    fingerprint = 'SHA256:' + base64.b64encode(hashlib.sha256(key.asbytes()).digest()).decode().rstrip('=')
    if key.get_name() != 'ssh-ed25519' or not hmac.compare_digest(fingerprint, PIN):
        raise ReadFailure('hostKeyMismatchBeforeAuthentication')


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('output','pid','start-ticks','invocation-id','revision','since','until'):
        parser.add_argument('--'+name, required=True)
    args = parser.parse_args(argv)
    try:
        config = make_config(args.pid,args.start_ticks,args.invocation_id,args.revision,args.since,args.until)
        remote = build_remote(config)
    except (ValueError, KeyError): parser.error('Invalid candidate pins or UTC window (maximum 60 seconds).')
    dest = Path(args.output).resolve()
    if dest.exists() or not dest.parent.is_dir(): parser.error('Use a new output in an existing directory.')
    if not sys.stdin.isatty(): parser.error('Local PTY required for memory-only getpass.')
    runtime = Path.home()/'.codex/tmp/mir2-deploy-ssh-runtime'
    if runtime.is_dir(): sys.path.insert(0,str(runtime))
    import paramiko
    password = getpass.getpass('SSH password (memory only; never sent to sudo): ')
    transport=channel=tcp=None; done=threading.Event(); expired=threading.Event()
    started=time.monotonic(); deadline=started+60
    stdout_hash=hashlib.sha256(); stderr_hash=hashlib.sha256(); stdout_bytes=stderr_bytes=0; stdout_parts=[]
    remote_exit=None; exit_return=None; remote_status_received=False; local_error=None; stream_end='notStarted'; pin_verified=False
    channel_closed=None; transport_active=None; request_utc=request_mono=response_utc=response_mono=None
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
    def guard():
        if not done.wait(max(0,deadline-time.monotonic())): expired.set(); close_owned()
    def left():
        value=deadline-time.monotonic()
        if value<=0 or expired.is_set(): raise ReadFailure('localWallBudget')
        return min(20,value)
    with dest.open('x',encoding='utf-8',newline='\n') as writer:
        threading.Thread(target=guard,name='tick-diagnostics-budget',daemon=True).start()
        try:
            tcp=socket.create_connection(('165.154.65.136',22),timeout=left()); transport=paramiko.Transport(tcp)
            transport.get_security_options().key_types=('ssh-ed25519',); transport.banner_timeout=left(); transport.auth_timeout=left(); transport.start_client(timeout=left())
            verify_host_key(transport.get_remote_server_key()); pin_verified=True
            transport.auth_timeout=left(); transport.auth_password('ubuntu',password,fallback=False); password=None
            if not transport.is_authenticated(): raise ReadFailure('authenticationFailed')
            channel=transport.open_session(timeout=left())
            request_utc=time.time_ns()//1000; request_mono=time.monotonic_ns()
            channel.exec_command('sudo -n -- python3 -u -c '+shlex.quote(remote)); channel.shutdown_write(); channel.settimeout(0.0)
            while True:
                left()
                for unused in range(4):
                    if not channel.recv_stderr_ready(): break
                    chunk=channel.recv_stderr(32768); stderr_hash.update(chunk); stderr_bytes+=len(chunk)
                    if stdout_bytes+stderr_bytes>MAX_BYTES: raise ReadFailure('streamByteBound')
                for unused in range(4):
                    if not channel.recv_ready(): break
                    chunk=channel.recv(32768); stdout_hash.update(chunk); stdout_bytes+=len(chunk)
                    if stdout_bytes+stderr_bytes>MAX_BYTES: raise ReadFailure('streamByteBound')
                    stdout_parts.append(chunk)
                pending=channel.recv_ready() or channel.recv_stderr_ready()
                if channel.exit_status_ready() and not pending:
                    exit_return=channel.recv_exit_status()
                    if exit_return>=0:
                        remote_exit=exit_return; remote_status_received=True; stream_end='remoteExitStatusReceived'
                    else: stream_end='channelEndedWithoutRemoteExitStatus'
                    break
                if not pending and (channel.closed or not transport.is_active()):
                    stream_end='transportOrChannelClosedWithoutRemoteExitStatus'; break
                time.sleep(.03)
        except BaseException as error:
            local_error=error.code if isinstance(error,ReadFailure) else 'authenticationOrTransportFailure'
            stream_end='localWallBudget' if expired.is_set() else 'localReadException'
        finally:
            password=None
            if channel is not None and not remote_status_received:
                try:
                    if channel.exit_status_ready():
                        exit_return=channel.recv_exit_status()
                        if exit_return>=0: remote_exit=exit_return; remote_status_received=True
                except Exception: pass
            if request_utc is not None: response_utc=time.time_ns()//1000; response_mono=time.monotonic_ns()
            channel_closed=channel.closed if channel is not None else None
            transport_active=transport.is_active() if transport is not None else None
            done.set(); close_owned()
        parsed,category,framing=validate_frame(b''.join(stdout_parts),config)
        ok=local_error is None and remote_status_received and remote_exit==0 and category=='validFrame' and parsed.get('ok') is True
        output={'schema':'mir2.writer-diagnostics-readback.v1','candidatePins':config,'ok':ok,
                'boundedWindowExportComplete':bool(ok and parsed.get('boundedWindowExportComplete')),
                'queryResult':parsed,'parseCategory':category,'framing':framing,
                'remoteExitStatus':remote_exit,'remoteExitStatusReceived':remote_status_received,'exitStatusMethodReturn':exit_return,
                'streamEndReason':stream_end,'localErrorClass':local_error,'localBudgetExpired':expired.is_set(),
                'channelClosedAtReadEnd':channel_closed,'transportActiveAtReadEnd':transport_active,
                'stdoutBytesReceived':stdout_bytes,'stdoutSha256':stdout_hash.hexdigest(),
                'stderrBytesReceived':stderr_bytes,'stderrSha256':stderr_hash.hexdigest(),
                'unparsedStdoutPersisted':False,'rawStderrPersisted':False,'localWallMaxSeconds':60,'remoteQueryMaxSeconds':20,
                'elapsedWallMs':round((time.monotonic()-started)*1000,3),'hostKeyPin':PIN,'hostKeyVerifiedBeforeAuthentication':pin_verified,
                'remoteStdinPasswordBytesSent':0,'transportSourceSha256':TRANSPORT_SOURCE_SHA256,'baseSourceSha256':BASE_SOURCE_SHA256,'parentReaderSourceSha256':PARENT_READER_SHA256,
                'clockEvidence':{'localRequestUnixUs':request_utc,'localResponseUnixUs':response_utc,
                    'localRequestMonotonicNs':request_mono,'localResponseMonotonicNs':response_mono,
                    'exactCrossClockLatencyInferred':False},'serviceOperations':[],'networkOrVpnOperations':[]}
        json.dump(output,writer,indent=2,allow_nan=False); writer.write('\n')
    print(json.dumps({'output':str(dest),'ok':ok,'remoteExitStatus':remote_exit,'remoteExitStatusReceived':remote_status_received,
                      'stdoutBytesReceived':stdout_bytes,'stderrBytesReceived':stderr_bytes,'parseCategory':category,
                      'boundedWindowExportComplete':output['boundedWindowExportComplete']}))
    return 0 if ok else 1


if __name__ == '__main__':
    raise SystemExit(main())
