"""Read a CLOSED public legacy-harness case; never poll, launch or qualify it."""
import argparse
from bisect import bisect_left
from collections import Counter,defaultdict,deque
from datetime import datetime,timezone
import hashlib
import json
import math
from pathlib import Path
import re
import statistics
import sys

ROOT=Path(__file__).resolve().parent
REVISION='7403bc5ee5cbb472477db101b8be603136049dcf'
BINARY='26c131881c6c1e9de4ca2179d9f7a05f97d07f7e29269c80dca3dcd04cae1082'
PINS={'sourceSha256':('public_signed_load.mjs','4a70deae9d6790effe1a332cba6ab48ef7b4244809df4e8e7c99aba64f5d3938'),
 'probeSourceSha256':('public_signed_probe.mjs','b119c5dd0d67ff1f4a42bd7f94bf6b12374fba6ae2ebef147a3bb853f0b58c75'),
 'authSourceSha256':('public_sui_wallet_auth.mjs','08a274f05823925a8fc0d6df771a5d4cc5f3ca48f8f5a3046520308e56d4f34c')}


class Refusal(Exception):pass


def need(value,code):
    if not value:raise Refusal(code)


def number(v):return isinstance(v,(int,float)) and not isinstance(v,bool) and math.isfinite(v)


def hash_string(v):return isinstance(v,str) and len(v)==64 and all(c in '0123456789abcdef' for c in v)


def utc(v):
    try:return datetime.fromisoformat(v.replace('Z','+00:00')).timestamp()*1000 if isinstance(v,str) else None
    except (ValueError,OverflowError):return None


def stamp(v):return datetime.fromtimestamp(v/1000,timezone.utc).isoformat() if number(v) else None


def public_path(value):
    p=Path(value).absolute()
    need(p.drive.upper()=='C:' and not any(x.lower()=='private' for x in p.parts),'public_c_input_required')
    for part in [p,*p.parents]:need(not part.is_symlink() and not (hasattr(part,'is_junction') and part.is_junction()),'linked_input_refused')
    return p


def expected_identity(value):
    need(isinstance(value,(tuple,list)) and len(value)==3 and type(value[0]) is int and 1<value[0]<2**31 and
      type(value[1]) is int and 0<value[1]<2**63 and isinstance(value[2],str) and
      re.fullmatch('[0-9a-f]{32}',value[2]) is not None,'explicit_valid_measurement_identity_required')
    return tuple(value)


def sample_identity(row):
    return (row.get('targetProcess',{}).get('pid'),row.get('targetProcess',{}).get('startTicks'),row.get('targetUnit',{}).get('InvocationID'))


def output_path(value):
    need(Path(value).is_absolute(),'absolute_public_output_required')
    p=public_path(value)
    need(not p.exists() and p.parent.is_dir(),'fresh_public_output_required')
    repositories=[a for a in Path(__file__).resolve().parents if (a/'.git').exists()]
    need(not any(p.is_relative_to(a) for a in repositories) and not any((a/'.git').exists() for a in p.parents),
      'outside_repository_output_required')
    return p


def source_digest():
    # Executed canonical source may be on E; only user-selected inputs stay C-only.
    return hashlib.sha256(Path(__file__).read_bytes()).hexdigest()


def initialization_placeholder(row):
    return (isinstance(row,dict) and set(row)=={'schemaVersion','sampledAt','stop','reason'} and
      type(row['schemaVersion']) is int and row['schemaVersion']==1 and type(row['sampledAt']) is int and
      0<row['sampledAt']<2**63 and row['stop'] is True and row['reason']=='monitor starting; not ready for admission')


def digest(p):
    p=public_path(p);info=p.stat();need(p.is_file() and info.st_nlink==1,'regular_input_required')
    with p.open('rb') as f:value=hashlib.file_digest(f,'sha256').hexdigest()
    need((info.st_size,info.st_mtime_ns)==(p.stat().st_size,p.stat().st_mtime_ns),'input_changed_during_read')
    return value


def read_json(p,maximum=16*2**20):
    p=public_path(p);need(p.is_file(),'required_public_file_missing')
    need(0<p.stat().st_size<=maximum,'bounded_json_required')
    info=p.stat();raw=p.read_bytes()
    need((info.st_size,info.st_mtime_ns)==(p.stat().st_size,p.stat().st_mtime_ns),'json_changed_during_read')
    value=json.loads(raw);need(isinstance(value,dict),'json_object_required');return value


def monitor_completion(latest,stop_file_exists,identity):
    identity=expected_identity(identity)
    readback=latest.get('monitorReadback',{})
    pin=readback.get('expectedIdentity',{})
    need((pin.get('pid'),pin.get('startTicks'),pin.get('invocationId'))==identity and readback.get('expectedRevision')==REVISION,
      'monitor_identity_pin_mismatch')
    if 'targetProcess' in latest or 'targetUnit' in latest:need(sample_identity(latest)==identity,'monitor_identity_pin_mismatch')
    need(latest.get('schemaVersion')==1 and latest.get('monitorTerminal') is True and latest.get('stop') is True and
      latest.get('outcome')=='completed' and latest.get('servicesStopped') is False and stop_file_exists and
      readback.get('completedBy')=='localStopFile' and readback.get('streamEndReason')=='localStopFile','monitor_not_closed')
    received=readback.get('remoteExitStatusReceived');remote_exit=readback.get('remoteExitStatus')
    method_return=readback.get('exitStatusMethodReturn')
    # A locally closed SSH channel can return the -1 method sentinel without
    # receiving a remote exit status. Preserve it without inventing exit 0.
    known_method=method_return is None or type(method_return) is int and method_return==0
    unknown_method=method_return is None or type(method_return) is int and method_return==-1
    need(type(received) is bool and (received and type(remote_exit) is int and remote_exit==0 and known_method or
      not received and remote_exit is None and unknown_method),'inconsistent_remote_monitor_exit_scope')
    return {'localReaderTerminalCompleted':True,'completedBy':readback['completedBy'],'streamEndReason':readback['streamEndReason'],
      'servicesStopped':False,'localProcessActualExitCodeInTerminal':None,
      'localProcessExitEvidence':'not present in terminal projection; root CLI exit receipt is separate',
      'remoteExitStatusReceived':received,'remoteExitStatus':remote_exit,
      'exitStatusMethodReturn':method_return,
      'remoteTerminalSampleObserved':readback.get('remoteTerminalSampleObserved'),
      'remoteExitScope':'observed remote exit0' if received else 'unknown: owned SSH channel closed after local stop-file completion'}


def closed_gates(case,identity):
    # These checks precede ALL report discovery, large event/host reads.
    job=read_json(case/'client-job/job-result.json')
    need(job.get('schema')=='mir2.cloud-client-job-result.v4' and job.get('allOwnedMembersExited') is True and
      job.get('jobClose',{}).get('closed') is True and job['jobClose'].get('allMembersExited') is True and
      job['jobClose'].get('killAppliedByClose') is False and job.get('forceKillUsed') is False,'owned_job_not_normally_closed')
    need(job.get('jobClose',{}).get('failedLaunches')==[] and job.get('finalJobSample',{}).get('jobActiveProcesses')==0,
      'owned_job_members_or_failed_launches_remain')
    latest=read_json(case/'host.latest.json')
    monitor_completion(latest,(case/'monitor.stop').is_file(),identity)
    before=read_json(case/'preflight-before.json');after=read_json(case/'preflight-after.json')
    for value,phase in [(before,'before'),(after,'after')]:
        need(value.get('schema')=='mir2.existing-public-pool-preflight.v3' and value.get('phase')==phase and value.get('passed') is True and
          value.get('readOnlyPrivateInputs') is True and value.get('identitiesGenerated') is False and value.get('ledgerWrittenOrReset') is False and
          value.get('pool',{}).get('actorCount')==100 and value.get('sourcePolicy',{}).get('passed') is True,'passed_closed_preflight_chain_required')
        need(all(hash_string(value['pool'].get(k)) for k in ['identityFingerprintSha256','canonicalActorArraySha256']),
          'complete_pool_identity_commitments_required')
    need(before.get('sourceLockSha256')==after.get('sourceLockSha256')==job.get('sourceLockSha256') and
      before['pool'].get('identityFingerprintSha256')==after['pool'].get('identityFingerprintSha256') and
      before['pool'].get('canonicalActorArraySha256')==after['pool'].get('canonicalActorArraySha256') and
      after.get('comparison',{}).get('baselineReceiptSha256')==digest(case/'preflight-before.json') and
      after['comparison'].get('actorIdentityUnchanged') is True and after['comparison'].get('entireCanonicalActorArrayUnchanged') is True and
      after['comparison'].get('allUnexpiredLedgerAttemptsRetained') is True,'closed_pool_identity_source_or_ledger_chain_mismatch')
    spec=read_json(case/'client-job-spec.json');need(job.get('specSha256')==digest(case/'client-job-spec.json'),'job_spec_hash_mismatch')
    lock_path=public_path(spec['sourceLock']['path']);need(digest(lock_path)==before['sourceLockSha256']==spec['sourceLock']['sha256'],'source_lock_hash_mismatch')
    lock=read_json(lock_path)
    need(job.get('clientRuntimeSourcePolicyPassed') is True and
      all(job.get('runtimeSourceObservations',{}).get(k,{}).get('passed') is True for k in ['beforeLaunch','afterExit']),'runtime_source_policy_failed')
    return job,latest,before,after,spec,lock


def stage_bounds(stage):
    start,end=stage.get('startedAtMs'),stage.get('endedAtMs')
    anchors=[utc(p.get('atUtc'))-p['elapsedMs'] for p in stage.get('population',[]) if utc(p.get('atUtc')) is not None and number(p.get('elapsedMs'))]
    if not anchors or not number(start) or not number(end) or end<start:return None
    origin=statistics.median(anchors)
    return {'startUnixMs':origin,'endUnixMs':origin+end-start,'startedAtUtc':stamp(origin),'endedAtUtc':stamp(origin+end-start),
      'nodeMonoToUnixOffsetMs':origin-start,'anchorCount':len(anchors),'anchorSpreadMs':max(anchors)-min(anchors),
      'mappingReliable':max(anchors)-min(anchors)<=1000,'method':'median(population.atUtc-elapsedMs); no cross-process monotonic comparison'}


def percentile(values):
    values=sorted(v for v in values if number(v));return values[math.ceil(len(values)*.95)-1] if values else None


def worst(actors,key,minimum=False):
    if not actors or any(not number(a.get(key)) for a in actors):return None
    return (min if minimum else max)(a[key] for a in actors)


def action_metrics(rows):
    completed=[r for r in rows if r.get('outcome',{}).get('completed') is True]
    counts=Counter(r['action'] for r in rows);done=Counter(r['action'] for r in completed)
    return {'offered':len(rows),'completed':len(completed),'completionRatio':len(completed)/len(rows) if rows else None,
      'moves':done['walk'],'moveOffered':counts['walk'],'chats':done['chat'],'chatOffered':counts['chat'],
      'timeouts':sum(r.get('outcome',{}).get('timeout') is True for r in rows),
      'notSent':sum(r.get('sent') is None for r in rows),'missingOutcome':sum('outcome' not in r for r in rows),
      'movementP95Ms':percentile([r['outcome'].get('latencyMs') for r in completed if r['action']=='walk']),
      'chatP95Ms':percentile([r['outcome'].get('latencyMs') for r in completed if r['action']=='chat']),
      'scheduleP95Ms':percentile([r.get('outcome',{}).get('latenessMs',r.get('latenessMs')) for r in rows]),
      'reasons':dict(Counter(r.get('outcome',{}).get('reason') for r in rows if r.get('outcome',{}).get('reason')))}


def scan_events(path,report):
    need(path.is_file() and path.stat().st_size<=4*2**30,'bounded_complete_events_required')
    initial=path.stat();hasher=hashlib.sha256();count=0
    stage_rows=[[] for _ in report['stages']];active={};connections={};receipts={};rings=defaultdict(deque)
    logouts=[];logout_requests=[];closes=defaultdict(list);admitted=set();socket_activity=Counter();walk_pairs=Counter();binding_failures=[];ambiguous=0
    all_counts=Counter();all_reasons=Counter();timeout_actions=Counter();failure_examples=[]
    with path.open('rb') as stream:
        for line in stream:
            hasher.update(line);need(len(line)<=2**20 and line.endswith(b'\n'),'truncated_or_oversized_event_line')
            if not line.strip():continue
            count+=1;need(count<=8000000,'event_row_bound_exceeded');e=json.loads(line)
            a,c=e.get('actorOrdinal'),e.get('connection');kind=e.get('kind')
            if isinstance(a,int) and isinstance(c,int):connections[a]=c
            if isinstance(a,int) and isinstance(c,int) and e.get('direction') in ['sent','received']:socket_activity[(a,c)]+=1
            if kind=='admitted':admitted.add((a,c))
            if e.get('direction')=='received' and kind in ['UserLocation','ObjectWalk']:
                key=(a,c,e.get('sequence'));receipts[key]=e;rings[(a,c)].append(key)
                if len(rings[(a,c)])>512:receipts.pop(rings[(a,c)].popleft(),None)
            if kind=='logoutReceipt':logouts.append(e)
            if kind=='logOut' and e.get('direction')=='sent':logout_requests.append(e)
            if kind=='webSocketClosing':closes[(a,c)].append(e)
            if kind=='offered':
                all_counts['offered']+=1
                key=(a,e.get('ticketSequence'));need(key not in active,'duplicate_unclosed_offer_key')
                row={'actorOrdinal':a,'connection':connections.get(a),'ticket':key[1],'action':e.get('action'),
                  'plannedAtMs':e.get('plannedAtMs'),'latenessMs':e.get('latenessMs'),'sent':None}
                active[key]=row
                for index,stage in enumerate(report['stages']):
                    if number(row['plannedAtMs']) and stage['startedAtMs']<=row['plannedAtMs']<stage['endedAtMs']:stage_rows[index].append(row)
            elif e.get('direction')=='sent' and kind in ['walk','chat']:
                pending=[r for (actor,_),r in active.items() if actor==a and r['action']==kind and r['sent'] is None]
                if len(pending)==1:pending[0]['sent']=e
                elif len(pending)>1:ambiguous+=1
            elif kind=='actionOutcome':
                row=active.pop((a,e.get('ticketSequence')),None)
                need(row is not None,'outcome_without_offer');row['outcome']=e
                all_counts['actionOutcomes']+=1
                all_counts['completed']+=e.get('completed') is True
                all_counts['timeouts']+=e.get('timeout') is True
                all_counts['notSent']+=row['sent'] is None
                if e.get('reason'):all_reasons[e['reason']]+=1
                if e.get('timeout') is True:timeout_actions[e.get('action',row['action'])]+=1
                if e.get('completed') is not True:
                    all_counts['incompleteOutcomes']+=1
                    if len(failure_examples)<100:failure_examples.append({'actorOrdinal':a,'ticketSequence':e.get('ticketSequence'),
                      'action':row['action'],'plannedAtMs':row['plannedAtMs'],'atUtc':e.get('atUtc'),'monotonicMs':e.get('monotonicMs'),
                      'timeout':e.get('timeout') is True,'notSent':row['sent'] is None,'reason':e.get('reason')})
                if row['action']=='walk' and e.get('completed') is True:
                    owner=receipts.get((a,row['connection'],e.get('ownerReceipt')))
                    peer=receipts.get((e.get('peerActorOrdinal'),connections.get(e.get('peerActorOrdinal')),e.get('peerReceipt')))
                    sent=row['sent'];valid=bool(owner and peer and sent)
                    if valid:valid=(owner['kind']=='UserLocation' and peer['kind']=='ObjectWalk' and
                      number(owner.get('x')) and (owner.get('x'),owner.get('y'))==(peer.get('x'),peer.get('y')) and
                      owner.get('monotonicMs')==e.get('ownerReceiptAtMs') and peer.get('monotonicMs')==e.get('peerReceiptAtMs') and
                      sent['monotonicMs']<=min(owner['monotonicMs'],peer['monotonicMs']) and
                      max(owner['monotonicMs'],peer['monotonicMs'])<=row['plannedAtMs']+2000)
                    walk_pairs['checked']+=1;walk_pairs['matched' if valid else 'notIndependentlyBound']+=1
                    if not valid and len(binding_failures)<100:binding_failures.append({'actorOrdinal':a,'ticket':row['ticket'],'ownerReceipt':e.get('ownerReceipt'),'peerReceipt':e.get('peerReceipt')})
            need(len(active)<=1000,'pending_offer_bound_exceeded')
    need((initial.st_size,initial.st_mtime_ns)==(path.stat().st_size,path.stat().st_mtime_ns),'events_changed_during_read')
    need(hasher.hexdigest()==report.get('eventsSha256') and count==report.get('rows'),'final_events_hash_or_rows_mismatch')
    return {'sha256':hasher.hexdigest(),'rows':count,'stageRows':stage_rows,'logouts':logouts,'closes':closes,
      'admitted':admitted,'socketActivity':dict(socket_activity),'logoutRequests':logout_requests,
      'allRawActionOutcomes':{'scope':'ALL raw offered/actionOutcome events, including admission and auth-window waits; no cleanup/report actor inference',
        **{k:all_counts[k] for k in ['offered','actionOutcomes','completed','timeouts','notSent','incompleteOutcomes']},
        'reasonCounts':dict(all_reasons),'timeoutsByAction':dict(timeout_actions),'failureExamples':failure_examples,
        'failureExamplesTruncated':all_counts['incompleteOutcomes']>len(failure_examples)},
      'walkPairs':dict(walk_pairs),'walkBindingFailures':binding_failures,'sendAssociationAmbiguities':ambiguous}


def summarize_stage(stage,rows):
    seconds=stage.get('seconds');span=stage['endedAtMs']-stage['startedAtMs'];windows=[]
    expected_minutes=math.floor(seconds/60) if number(seconds) else 0
    for window in stage.get('windows',[]):
        minute=window.get('minute');from_ms=stage['startedAtMs']+(minute-1)*60000
        selected=[r for r in rows if from_ms<=r['plannedAtMs']<from_ms+60000]
        actor_rows=[]
        for actor in window.get('actors',[]):
            raw=action_metrics([r for r in selected if r['actorOrdinal']==actor.get('actorOrdinal')])
            keys=['offered','completed','completionRatio','moves','moveOffered','chats','timeouts','movementP95Ms','chatP95Ms','scheduleP95Ms']
            agree=all(raw.get(k)==actor.get(k) for k in keys)
            actor_rows.append({'actorOrdinal':actor.get('actorOrdinal'),'reportedPassed':actor.get('passed'),'rawMetricsAgree':agree,**raw})
        ordinals=[a['actorOrdinal'] for a in actor_rows]
        windows.append({'minute':minute,'reportedPassed':window.get('passed'),'reportedWorkloadAdequate':window.get('workloadAdequate'),
          'actorPopulationComplete':sorted(ordinals)==list(range(stage['players'])),'allReportedActorsPassed':all(a['reportedPassed'] is True for a in actor_rows),
          'allRawMetricsAgree':all(a['rawMetricsAgree'] for a in actor_rows),'actors':actor_rows,
          'worstActorMovementP95Ms':worst(actor_rows,'movementP95Ms'),
          'worstActorChatP95Ms':worst(actor_rows,'chatP95Ms'),
          'worstActorScheduleP95Ms':worst(actor_rows,'scheduleP95Ms'),
          'minimumActorCompletionRatio':worst(actor_rows,'completionRatio',True),**action_metrics(selected)})
    population=stage.get('population',[]);bounds=stage_bounds(stage)
    complete=bool(expected_minutes and span>=seconds*1000 and len(windows)>=expected_minutes)
    return {'name':stage.get('name'),'players':stage.get('players'),'requestedSeconds':seconds,'elapsedSeconds':span/1000,
      'fullyCompletedDuration':complete,'reportedPassed':stage.get('passed'),'reportedFailure':stage.get('failure'),
      'reportedWorkloadAdequate':stage.get('workloadAdequate'),'utcBounds':bounds,
      'populationMatchesStage':bool(population) and all(p.get('controlled')==p.get('serverWs')==p.get('serverActive')==stage['players'] for p in population),
      'allFullWindowsPassed':bool(windows) and all(w['reportedPassed'] is True and w['allReportedActorsPassed'] for w in windows),
      'allRawWindowMetricsAgree':bool(windows) and all(w['allRawMetricsAgree'] and w['actorPopulationComplete'] for w in windows),
      'windows':windows,'actionsOverObservedSpan':action_metrics(rows),'stageApplicationTraffic':stage.get('applicationTraffic')}


def interpolate(rows,time,extract):
    times=[r['sampledAt'] for r in rows];i=bisect_left(times,time)
    if i<len(rows) and times[i]==time:a=b=rows[i]
    elif i==0 or i==len(rows):return None
    else:a,b=rows[i-1],rows[i]
    av,bv=extract(a),extract(b)
    if not number(av) or not number(bv):return None
    fraction=(time-a['sampledAt'])/(b['sampledAt']-a['sampledAt']) if a is not b else 0
    return {'value':av+(bv-av)*fraction,'leftAtMs':a['sampledAt'],'rightAtMs':b['sampledAt'],'fraction':fraction,
      'leftCounter':av,'rightCounter':bv,'bracketGapMs':b['sampledAt']-a['sampledAt']}


def counter_metric(rows,lo,hi,extract,factor):
    start,end=interpolate(rows,lo,extract),interpolate(rows,hi,extract)
    intervals=[(a,b) for a,b in zip(rows,rows[1:]) if a['sampledAt']<hi and b['sampledAt']>lo]
    invalid=not start or not end or not intervals or any(not number(extract(a)) or not number(extract(b)) or extract(b)<extract(a) for a,b in intervals)
    rates=[(extract(b)-extract(a))/(b['sampledAt']-a['sampledAt'])*factor for a,b in intervals
      if number(extract(a)) and number(extract(b)) and b['sampledAt']>a['sampledAt'] and extract(b)>=extract(a)]
    return {'mean':None if invalid else (end['value']-start['value'])/(hi-lo)*factor,
      'peakObservedIntervalRate':max(rates) if rates else None,'valid':not invalid,'startInterpolation':start,'endInterpolation':end}


def host_statistics(rows,bounds,identity):
    identity=expected_identity(identity)
    if not bounds or not bounds['mappingReliable']:return {'valid':False,'reason':'stage_utc_mapping_unavailable'}
    rows=[r for r in rows if not r.get('monitorTerminal') and number(r.get('sampledAt'))]
    if len(rows)<2 or any(b['sampledAt']<=a['sampledAt'] for a,b in zip(rows,rows[1:])):return {'valid':False,'reason':'monitor_time_coverage_invalid'}
    lo,hi=bounds['startUnixMs'],bounds['endUnixMs'];need(hi>lo,'positive_stage_span_required')
    intervals=[(a,b) for a,b in zip(rows,rows[1:]) if a['sampledAt']<hi and b['sampledAt']>lo]
    relevant=list({id(r):r for pair in intervals for r in pair}.values())
    nics={r.get('network',{}).get('primaryNic') for r in relevant}
    identity_ok=all(sample_identity(r)==identity and
      r.get('health',{}).get('revision')==REVISION and r.get('targetUnit',{}).get('MemoryMax')==1600*2**20 for r in relevant)
    if not identity_ok or len(nics)!=1 or not all(isinstance(n,str) and n for n in nics):return {'valid':False,'reason':'target_identity_revision_budget_or_primary_nic_changed'}
    cpu=counter_metric(rows,lo,hi,lambda r:r.get('targetCgroup',{}).get('cpu.stat',{}).get('usage_usec'),.001)
    rx=counter_metric(rows,lo,hi,lambda r:r.get('network',{}).get('rxBytes'),.008)
    tx=counter_metric(rows,lo,hi,lambda r:r.get('network',{}).get('txBytes'),.008)
    gaps=[b['sampledAt']-a['sampledAt'] for a,b in intervals]
    inside=[r for r in rows if lo<=r['sampledAt']<=hi]
    def extreme(extract,minimum=False):
        values=[extract(r) for r in inside if number(extract(r))]
        return (min if minimum else max)(values) if values else None
    oom=lambda r:r.get('targetCgroup',{}).get('memory.events',{}).get('oom_kill')
    a,b=interpolate(rows,lo,oom),interpolate(rows,hi,oom)
    oom_bounds=None
    if a and b:
        low=max(0,b['leftCounter']-a['rightCounter']);high=b['rightCounter']-a['leftCounter']
        if high>=low:oom_bounds={'delta':low if low==high else None,'lowerBound':low,'upperBound':high,'integerCounterBoundaryAmbiguity':low!=high}
    return {'valid':all(v['valid'] for v in [cpu,rx,tx]),'samplingReliable':bool(gaps) and max(gaps)<=6000,
      'targetCpuCores':cpu,'primaryNicRxMbps':rx,'primaryNicTxMbps':tx,'maximumSampleGapMs':max(gaps) if gaps else None,
      'sampleGapsOver6Seconds':sum(g>6000 for g in gaps),'observedSamplesInside':len(inside),
      'primaryNic':next(iter(nics)),
      'peakObservedCgroupMemoryBytes':extreme(lambda r:r.get('targetCgroup',{}).get('memory.current')),
      'peakObservedRssBytes':extreme(lambda r:r.get('targetProcess',{}).get('VmRSSBytes')),
      'minimumObservedHostAvailableBytes':extreme(lambda r:r.get('host',{}).get('memory',{}).get('MemAvailable'),True),
      'oomKillDeltaBounds':oom_bounds,'meanMethod':'linear counter interpolation at UTC boundaries; adjacent counter differences, no rate-field averaging or extrapolation',
      'clockBoundary':'uses reported injector UTC and host sampledAt UTC; independent host/injector clock-offset calibration is unavailable',
      'peakMethod':'sample-interval rate / observed memory points only; unobserved peaks unknown',
      'networkScope':'one primary host NIC, including VPN-related and colocated traffic; not game-exclusive; no virtual-interface summation'}


def classify_failure(report,stage_summaries,all_raw=None):
    complete=[s for s in stage_summaries if s.get('fullyCompletedDuration') is True]
    stage_reasons=Counter()
    for s in complete:stage_reasons.update(s['actionsOverObservedSpan']['reasons'])
    completed_counts={key:sum(s['actionsOverObservedSpan'].get(key,0) for s in complete) for key in ['offered','completed','timeouts','notSent']}
    completed_counts.update(scope='only fully elapsed reported measurement-stage spans',reasonCounts=dict(stage_reasons))
    reasons=Counter(all_raw['reasonCounts']) if all_raw is not None else None
    text=' '.join([str(report.get('failure','')),str(report.get('resourceStopReason',''))])
    lower=text.lower()
    return {'reportedFailure':report.get('failure'),'actualReportedFailure':bool(report.get('failure')),'resourceStopReason':report.get('resourceStopReason'),
      'explicitActionTimeouts':all_raw.get('timeouts') if all_raw is not None else None,
      'fixtureNoLegalStep':sum(v for k,v in reasons.items() if 'no legal' in k.lower()) if reasons is not None else None,
      'fixtureAoiOrPeer':sum(v for k,v in reasons.items() if any(x in k.lower() for x in ['aoi','witness absent','no actual peer','no real','shared identity'])) if reasons is not None else None,
      'fixtureDeadOrLivingOwner':sum(v for k,v in reasons.items() if any(x in k.lower() for x in ['not a real living','dead','living authenticated'])) if reasons is not None else None,
      'completedStageCounts':completed_counts,'allRawActionOutcomeCounts':all_raw,
      'countScope':'whole-trace counts come from raw actionOutcome events, not only recorded measurement stages',
      'fixtureSignalsInAnyReportedFailure':{'noLegalStep':any(x in lower for x in ['no legal','no_legal']),
        'aoiOrPeer':any(x in lower for x in ['aoi','peer absent','no actual peer','no real']),
        'deadOrOwner':any(x in lower for x in ['dead','not a real living','living authenticated'])},
      'reportedAdmissionSloMisses':report.get('admissionSloMisses',[]),
      'activitySloStop':any(x in lower for x in ['strict slo','activity/latency','activityslo']),
      'possibleResourceOrSafetyStop':any(x in lower for x in ['rss','memory','resource ceiling','cpu protection','monitor stopped','finite load wall','safety stop','evidence hard row']),
      'reasonCounts':dict(reasons) if reasons is not None else None,
      'attributionBoundary':'client-observed timeouts do not alone prove server acceptance, CPU saturation or a network/server root cause; reason presence is not an additional SLO veto'}


def cleanup_summary(report,event_info):
    out=[]
    for row in report.get('cleanup',[]):
        a=row.get('actorOrdinal');c=row.get('connection')
        candidates=[e for e in event_info['logouts'] if e.get('actorOrdinal')==a and (c is None or e.get('connection')==c)]
        if c is None and candidates:c=min(e['connection'] for e in candidates);candidates=[e for e in candidates if e['connection']==c]
        if c is None:
            observed=sorted(conn for (actor,conn) in event_info.get('socketActivity',{}) if actor==a)
            if observed:c=observed[0]
        logout=candidates[0] if len(candidates)==1 else None
        requests=[e for e in event_info.get('logoutRequests',[]) if e.get('actorOrdinal')==a and e.get('connection')==c]
        socket_observed=bool(event_info.get('socketActivity',{}).get((a,c)) or logout)
        admitted=(a,c) in event_info.get('admitted',set())
        business_expected=bool(requests or logout or admitted)
        rows=event_info['closes'].get((a,c),[]);close=next((e for e in rows if e.get('edge')=='webSocketClose'),None)
        frame=next((e for e in rows if e.get('edge')=='validCloseFrameReceived'),None)
        ack=bool(logout and logout.get('packet')=='LogOutSuccess' and number(logout.get('elapsedMs')) and logout['elapsedMs']<=5000 and logout.get('observedAfter5000Ms') is False)
        wire=bool(logout and close and frame and all(e.get('closeFrameSent') is True and e.get('closeFrameReceived') is True for e in [frame,close]) and
          0<=frame['monotonicMs']-logout['monotonicMs']<=5000 and frame['monotonicMs']<=close['monotonicMs']<=logout['monotonicMs']+5000 and
          not any(e.get('edge')=='closeEventDeadlineExceeded' for e in rows))
        category='business-logout' if requests or logout else 'admitted-without-logout-request' if admitted else 'pre-world-socket-close' if socket_observed else 'unestablished-no-op-close'
        out.append({'actorOrdinal':a,'connection':c,'phase':row.get('phase','primary'),'reportedNormalLogout':row.get('normalLogout'),
          'lifecycleCategory':category,'rawSocketEstablishedEvidence':socket_observed,'rawWorldAdmittedEvidence':admitted,
          'rawLogOutRequestCount':len(requests),'rawLogOutSuccessCount':sum(e.get('packet')=='LogOutSuccess' for e in candidates),
          'businessLogoutExpected':business_expected,'logoutAckWithin5s':ack if business_expected else None,
          'logoutElapsedMs':logout.get('elapsedMs') if logout else None,'wireCloseWithin5sAfterLogout':wire if logout else None,
          'closeCodeObserved':None,'captureFailure':row.get('captureFailure'),'closeFailure':row.get('closeFailure',row.get('message'))})
    business=[r for r in out if r['businessLogoutExpected']];wire=[r for r in out if r['wireCloseWithin5sAfterLogout'] is not None]
    return {'rows':out,'allReportedNormal':bool(out) and all(r['reportedNormalLogout'] is True for r in out),
      'rawBusinessLogOutRequestCount':sum(r['rawLogOutRequestCount'] for r in out),'rawLogOutSuccessCount':sum(r['rawLogOutSuccessCount'] for r in out),
      'unestablishedNoOpCloseCount':sum(r['lifecycleCategory']=='unestablished-no-op-close' for r in out),
      'wireCloseEvidenceCheckedCount':len(wire),
      'establishedSocketsWithoutIndependentCloseBudget':sum(r['rawSocketEstablishedEvidence'] and r['wireCloseWithin5sAfterLogout'] is None for r in out),
      'allLogoutAcksWithin5s':all(r['logoutAckWithin5s'] for r in business) if business else None,
      'allWireCloseEvidenceWithin5s':all(r['wireCloseWithin5sAfterLogout'] for r in wire) if wire else None,
      'savedVerification':report.get('savedVerification',[]),'savedInventoryIndependentRawProof':'not reconstructed: legacy persistent scalar packets omit full inventory and chat nonce text'}


def summarize(case,identity):
    identity=expected_identity(identity)
    case=public_path(case);job,latest,before,after,spec,lock=closed_gates(case,identity)
    reports=list((case/'client').glob('*.report.json'));need(len(reports)==1,'exactly_one_final_report_required')
    report=read_json(reports[0],64*2**20)
    need(report.get('schema')=='mir2.cloud-reference-public-signed-load.v1' and report.get('evidenceComplete') is True and report.get('evidenceLimitExceeded') is False and
      utc(report.get('startedAtUtc')) is not None and utc(report.get('completedAtUtc')) is not None and utc(report['completedAtUtc'])>=utc(report['startedAtUtc']),
      'complete_final_report_required')
    need(report.get('profile')=='movement-chat','movement_chat_profile_required')
    for key,(name,expected) in PINS.items():
        need(report.get(key)==expected and any(r.get('path','').endswith('/'+name) and r.get('sha256')==expected for r in lock['runtimeFiles']),'frozen_load_probe_auth_source_mismatch')
    inputs=read_json(case/'inputs.json');need(inputs.get('sourceRevision')==REVISION and inputs.get('binarySha256')==BINARY,'installed_candidate_public_input_mismatch')
    events=public_path(report['eventsFile']);need(events.parent==case/'client' and events.name==reports[0].name.replace('.report.json','.events.jsonl'),'report_event_path_mismatch')
    info=scan_events(events,report)
    history=case/'host.history.jsonl';need(history.is_file() and history.stat().st_size<=512*2**20,'closed_host_history_required')
    host_rows=[];hasher=hashlib.sha256();initialization=None;first_identity_sample_at=None;history_records=0
    with history.open('rb') as stream:
        for line_index,line in enumerate(stream):
            hasher.update(line);need(len(line)<=2**20 and line.endswith(b'\n'),'truncated_monitor_line')
            if line.strip():
                row=json.loads(line)
                need(isinstance(row,dict),'monitor_history_object_required')
                history_records+=1;need(history_records<=100000,'monitor_row_bound_exceeded')
                if initialization_placeholder(row):
                    need(line_index==0 and initialization is None,'initialization_placeholder_not_first')
                    initialization=row;continue
                if row.get('monitorTerminal'):monitor_completion(row,True,identity)
                else:
                    need(row.get('stop') is False,'unknown_nonterminal_monitor_stop_row')
                    need(sample_identity(row)==identity and row.get('health',{}).get('revision')==REVISION,'history_identity_pin_mismatch')
                    if initialization is not None and first_identity_sample_at is None:
                        need(number(row.get('sampledAt')) and row['sampledAt']>=initialization['sampledAt'],'initialization_timestamp_not_ordered')
                        first_identity_sample_at=row['sampledAt']
                host_rows.append(row);need(len(host_rows)<=100000,'monitor_row_bound_exceeded')
    need(initialization is None or first_identity_sample_at is not None,'initialization_missing_identity_sample')
    stages=[]
    for stage,rows in zip(report['stages'],info['stageRows']):
        s=summarize_stage(stage,rows)
        s['serverResources']=host_statistics(host_rows,s['utcBounds'],identity) if s['fullyCompletedDuration'] else {'valid':False,'reason':'partial_stage_not_used_for_full_stage_resource_mean'}
        stages.append(s)
    options=dict(zip(spec['command'][2::2],spec['command'][3::2]))
    return {'schema':'mir2.closed-cloud-progressive-movement-summary.v6','sourceRevision':REVISION,'declaredInstalledBinarySha256':BINARY,
      'monitorTargetIdentity':{'pid':identity[0],'startTicks':identity[1],'invocationId':identity[2]},
      'runCompleted':True,'reportOk':report.get('ok'),'campaignCompleted':report.get('campaignCompleted'),'allSloPassed':report.get('allSloPassed'),
      'measurementOk':report.get('measurementOk'),'steadyStageSloPassed':report.get('steadyStageSloPassed'),
      'requestedStages':options.get('--stages'),'requestedStageSeconds':options.get('--stage-seconds'),'requestedTopSeconds':options.get('--top-seconds'),
      'attemptedAdmissionActors':report.get('attemptedAdmissionActors'),'completedAdmissionActors':report.get('completedAdmissionActors'),
      'rawAdmittedConnections':len(info['admitted']),'stages':stages,'failureClassification':classify_failure(report,stages,info['allRawActionOutcomes']),
      'cleanup':cleanup_summary(report,info),'walkOwnerPeerReceiptChecks':info['walkPairs'],'walkBindingFailures':info['walkBindingFailures'],
      'finalFreshSaveVerificationCount':len(report.get('savedVerification',[])),
      'finalFreshSaveVerificationActorOrdinals':[r.get('actorOrdinal') for r in report.get('savedVerification',[])],
      'allActorSaveStateIndependentlyProven':False,
      'observedUnsentReasonsAreNotAnAdditionalSloVeto':True,
      'temporaryCapacityConfigRemovalVerifiedByThisTool':False,
      'sendAssociationAmbiguities':info['sendAssociationAmbiguities'],
      'causalEvidenceLimits':'Walk receipt seq/position/time pairs can be checked; expected shared-ID mapping/floors and chat nonce text are not exported. No independent full chat/tri-client/combat qualification.',
      'equipmentRegression':report.get('equipmentRegression',[]),'bootstrapApplicationTraffic':report.get('bootstrapApplicationTraffic',[]),
      'transportProfiles':report.get('transports',[]),'admissionTransportRecovery':report.get('admissionTransportRecovery',[]),'firstAttemptAdmissionOk':report.get('firstAttemptAdmissionOk'),
      'originalQualificationFlags':{k:report.get(k) for k in ['capacityEvidence','ccuQualified','combatTested','fullCombatCapacityQualified','physicalTriClientTested']},
      'capacityQualified':False,'capacityQualified100':False,'fullCombatCapacityQualified':False,'physicalTriClientTested':False,
      'scope':'actual public legacy movement/chat harness; distinct from local common-plan game-core comparison',
      'resourceContract':{'hostLogicalCpus':4,'hostMemoryGiB':8,'provisionedMbps':None,
        'userProvidedBandwidthText':'30MBPS','bandwidthContractVerified':False,'testCgroupMemoryMaxMiB':1600,
        'sharedHost':True,'networkExclusiveToGame':False,'hostAvailableReserveIsObservedNotAnAllocation':True},
      'historyInitialization':{'excludedInitializationRowCount':1 if initialization else 0,'initializationControlRow':initialization,
        'firstValidatedIdentitySampleAtUnixMs':first_identity_sample_at,'includedAsResourceSample':False,
        'scope':'only exact four-key first-line initialization placeholder excluded; no zero counter or memory sample fabricated'},
      'observedMonitorRevisions':sorted({r.get('health',{}).get('revision') for r in host_rows if r.get('health',{}).get('revision')}),
      'closedEvidence':{'ownedJobActualExitCode':job.get('actualExitCode'),'allOwnedExited':True,'noKill':True,'monitorTerminal':True,
        'monitorCompletion':monitor_completion(latest,True,identity),
        'processEnvelopePassed':job.get('processEnvelopePassed'),'processCreationToExitResourceEvidenceComplete':job.get('processCreationToExitResourceEvidenceComplete'),
        'allLiveResourceSamplesQualified':job.get('allLiveResourceSamplesQualified'),'startupResourceGapCount':job.get('startupResourceGapCount'),
        'preflightComparison':after.get('comparison'),'sourceLockSha256':before.get('sourceLockSha256')},
      'inputHashes':{'report':digest(reports[0]),'events':info['sha256'],'hostHistory':hasher.hexdigest(),'before':digest(case/'preflight-before.json'),
        'after':digest(case/'preflight-after.json'),'job':digest(case/'client-job/job-result.json'),'monitorTerminal':digest(case/'host.latest.json')},
      'summarizerSourceSha256':source_digest()}


def main(argv=None):
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--case-dir',required=True);parser.add_argument('--output')
    parser.add_argument('--expected-pid',type=int,required=True);parser.add_argument('--expected-start-ticks',type=int,required=True)
    parser.add_argument('--expected-invocation-id',required=True)
    args=parser.parse_args(argv);identity=expected_identity((args.expected_pid,args.expected_start_ticks,args.expected_invocation_id))
    destination=output_path(args.output) if args.output else None
    summary=summarize(Path(args.case_dir),identity)
    text=json.dumps(summary,indent=2)+'\n'
    if args.output:
        p=destination
        with p.open('x',encoding='utf8') as f:f.write(text)
        print(json.dumps({'written':str(p),'sha256':digest(p),'runCompleted':True,'reportOk':summary['reportOk'],'capacityQualified':False}))
    else:print(text,end='')
    return 0


if __name__=='__main__':
    try:raise SystemExit(main())
    except (Refusal,KeyError,ValueError,OSError) as error:
        print(json.dumps({'schema':'mir2.closed-cloud-summary-refusal.v1','error':str(error) if isinstance(error,Refusal) else type(error).__name__,
          'summaryWritten':False,'capacityQualified':False}));raise SystemExit(2)
