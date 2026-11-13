"""Synthetic boundary checks retained under this C evidence directory only."""
import copy
import contextlib
import io
from datetime import datetime,timezone
import hashlib
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

TEST_IDENTITY=(1077951,1549561055,'f0889eecc5f44b9db77fc02e2ba8bc41')
NEW_IDENTITY=(1092834,1549795188,'f5d9c02fa6ff464fb6e7612cf65768d0')
FIXTURE_PATHS=[]
ROOT=Path(__file__).parent
spec=importlib.util.spec_from_file_location('closed_summary_under_test',ROOT/'summarize_closed.py')
m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)


def fixture():
    parent=Path(tempfile.gettempdir()).resolve();assert parent.drive.upper()=='C:'
    assert not any((a/'.git').exists() for a in [parent,*parent.parents])
    root=Path(tempfile.mkdtemp(prefix='mir2-summary-v4-case-',dir=parent)).resolve();assert root.parent==parent
    FIXTURE_PATHS.append(str(root));return root


def write(p,j):
    p.parent.mkdir(parents=True,exist_ok=True);p.write_text(json.dumps(j),encoding='utf8')


def terminal_v3(identity=TEST_IDENTITY):
    row=json.loads((ROOT/'actual-v3-terminal.sanitized.json').read_bytes())
    row['monitorReadback']['expectedIdentity'].update(pid=identity[0],startTicks=identity[1],invocationId=identity[2])
    return row


def sample(at,identity=TEST_IDENTITY):
    return {'sampledAt':at,'targetProcess':{'pid':identity[0],'startTicks':identity[1],'VmRSSBytes':1000},
      'targetUnit':{'InvocationID':identity[2],'MemoryMax':1600*2**20},'health':{'revision':m.REVISION},
      'targetCgroup':{'cpu.stat':{'usage_usec':at*2000},'memory.events':{'oom_kill':0},'memory.current':2000},
      'host':{'memory':{'MemAvailable':3000}},'network':{'primaryNic':'eth0','rxBytes':at*1000,'txBytes':at*2000}}


def bounds(lo,hi):return {'startUnixMs':lo,'endUnixMs':hi,'mappingReliable':True}


def close_prefix(root,identity=TEST_IDENTITY):
    lock=root/'lock.json';write(lock,{'runtimeFiles':[{'path':'scripts/'+name,'sha256':sha} for name,sha in m.PINS.values()]})
    locksha=m.digest(lock)
    spec={'sourceLock':{'path':str(lock),'sha256':locksha},'command':['node','entry','--stages','2,25,50,100','--stage-seconds','300','--top-seconds','0']}
    write(root/'client-job-spec.json',spec)
    before={'schema':'mir2.existing-public-pool-preflight.v3','phase':'before','passed':True,'readOnlyPrivateInputs':True,
      'identitiesGenerated':False,'ledgerWrittenOrReset':False,'sourceLockSha256':locksha,'pool':{'actorCount':100,
      'identityFingerprintSha256':'a'*64,'canonicalActorArraySha256':'b'*64},'sourcePolicy':{'passed':True}}
    write(root/'preflight-before.json',before);after=copy.deepcopy(before);after['phase']='after';after['comparison']={
      'baselineReceiptSha256':m.digest(root/'preflight-before.json'),'actorIdentityUnchanged':True,'entireCanonicalActorArrayUnchanged':True,
      'allUnexpiredLedgerAttemptsRetained':True,'continuousRuntimeLedgerNonResetProven':False}
    write(root/'preflight-after.json',after)
    job={'schema':'mir2.cloud-client-job-result.v4','allOwnedMembersExited':True,'forceKillUsed':False,'actualExitCode':1,
      'jobClose':{'closed':True,'allMembersExited':True,'killAppliedByClose':False,'failedLaunches':[]},'finalJobSample':{'jobActiveProcesses':0},
      'sourceLockSha256':locksha,'specSha256':m.digest(root/'client-job-spec.json'),'clientRuntimeSourcePolicyPassed':True,
      'runtimeSourceObservations':{'beforeLaunch':{'passed':True},'afterExit':{'passed':True}},'processEnvelopePassed':False,
      'processCreationToExitResourceEvidenceComplete':False,'allLiveResourceSamplesQualified':False,'startupResourceGapCount':1}
    write(root/'client-job/job-result.json',job);write(root/'host.latest.json',terminal_v3(identity))
    (root/'monitor.stop').write_bytes(b'')
    return job


class ClosedSummaryBoundaries(unittest.TestCase):
    def test_cli_requires_complete_valid_explicit_identity_before_case_read(self):
        base=['--case-dir','C:/synthetic-not-read']
        pins=['--expected-pid',str(NEW_IDENTITY[0]),'--expected-start-ticks',str(NEW_IDENTITY[1]),'--expected-invocation-id',NEW_IDENTITY[2]]
        for args in [base,base+pins[:2],base+pins[:-1],base+['--expected-pid','not-int']+pins[2:]]:
            with patch.object(m,'summarize') as called,contextlib.redirect_stderr(io.StringIO()),self.assertRaises(SystemExit):m.main(args)
            called.assert_not_called()
        for identity in [(True,1,'a'*32),(1,1,'a'*32),(3,0,'a'*32),(3,2,'A'*32),(3,2,'a'*31),(3,2**63,'a'*32)]:
            with self.subTest(identity=identity),self.assertRaises(m.Refusal):m.expected_identity(identity)

    def test_new_epoch_is_caller_bound_in_terminal_and_resources(self):
        latest=terminal_v3(NEW_IDENTITY)
        self.assertTrue(m.monitor_completion(latest,True,NEW_IDENTITY)['localReaderTerminalCompleted'])
        with self.assertRaisesRegex(m.Refusal,'monitor_identity_pin_mismatch'):m.monitor_completion(latest,True,TEST_IDENTITY)
        rows=[sample(at,NEW_IDENTITY) for at in [0,10000,20000]]
        self.assertEqual(m.host_statistics(rows,bounds(5000,15000),NEW_IDENTITY)['targetCpuCores']['mean'],2)
        self.assertFalse(m.host_statistics(rows,bounds(5000,15000),TEST_IDENTITY)['valid'])

    def test_public_output_outside_source_is_fresh_and_repo_private_links_refuse(self):
        root=fixture();output=root/'summary.json'
        self.assertEqual(m.output_path(output),output);self.assertFalse(output.is_relative_to(ROOT.resolve()))
        output.write_bytes(b'keep')
        with self.assertRaisesRegex(m.Refusal,'fresh_public_output_required'):m.output_path(output)
        self.assertEqual(output.read_bytes(),b'keep')
        private=root/'private';private.mkdir()
        with self.assertRaises(m.Refusal):m.output_path(private/'new.json')
        repo=root/'synthetic-repository';repo.mkdir();(repo/'.git').mkdir()
        with self.assertRaisesRegex(m.Refusal,'outside_repository_output_required'):m.output_path(repo/'new.json')
        linked=root/'pretend-linked';linked.mkdir();original=Path.is_symlink
        with patch.object(Path,'is_symlink',lambda p:True if p==linked else original(p)),self.assertRaises(m.Refusal):m.output_path(linked/'new.json')
        with patch.object(m,'public_path',side_effect=AssertionError('self source hashing must not require C input path')):
            with patch.object(m,'__file__',str(ROOT/'test_summarize_closed.py')):
                self.assertEqual(m.source_digest(),hashlib.sha256((ROOT/'test_summarize_closed.py').read_bytes()).hexdigest())

    def test_cli_plumbs_new_identity_and_writes_fresh_public_C_output(self):
        root=fixture();output=root/'explicit-epoch-summary.json'
        args=['--case-dir',str(root),'--expected-pid',str(NEW_IDENTITY[0]),'--expected-start-ticks',str(NEW_IDENTITY[1]),
          '--expected-invocation-id',NEW_IDENTITY[2],'--output',str(output)]
        with patch.object(m,'summarize',return_value={'reportOk':False,'capacityQualified':False}) as called,contextlib.redirect_stdout(io.StringIO()):
            self.assertEqual(m.main(args),0)
        called.assert_called_once_with(root,NEW_IDENTITY)
        self.assertFalse(json.loads(output.read_bytes())['reportOk'])

    def test_actual_v3_local_stop_schema_keeps_remote_exit_unknown(self):
        latest=terminal_v3();result=m.monitor_completion(latest,True,TEST_IDENTITY)
        self.assertTrue(result['localReaderTerminalCompleted']);self.assertFalse(result['servicesStopped'])
        self.assertFalse(result['remoteExitStatusReceived']);self.assertIsNone(result['remoteExitStatus'])
        self.assertIsNone(result['localProcessActualExitCodeInTerminal'])
        self.assertEqual(result['completedBy'],'localStopFile')

    def test_v3_missing_completion_or_inconsistent_remote_exit_is_refused(self):
        for change in ['oldArray','missingReadback','missingMarker','remoteNone','inventedZero','failedOutcome']:
            latest=terminal_v3();marker=True
            if change=='oldArray':latest['servicesStopped']=[]
            elif change=='missingReadback':latest.pop('monitorReadback')
            elif change=='missingMarker':marker=False
            elif change=='remoteNone':latest['monitorReadback']['remoteExitStatusReceived']=None
            elif change=='inventedZero':latest['monitorReadback']['remoteExitStatus']=0
            else:latest['outcome']='failed'
            with self.subTest(change=change),self.assertRaises(m.Refusal):m.monitor_completion(latest,marker,TEST_IDENTITY)

    def test_unestablished_cleanup_flag_is_not_an_extra_business_logout(self):
        report={'cleanup':[{'actorOrdinal':0,'normalLogout':True},{'actorOrdinal':1,'normalLogout':True}]}
        info={'logouts':[{'actorOrdinal':0,'connection':1,'packet':'LogOutSuccess','elapsedMs':100,'observedAfter5000Ms':False,'monotonicMs':1000}],
          'logoutRequests':[{'actorOrdinal':0,'connection':1}], 'socketActivity':{(0,1):10},'admitted':{(0,1)},
          'closes':{(0,1):[{'edge':edge,'monotonicMs':1100+i,'closeFrameSent':True,'closeFrameReceived':True}
            for i,edge in enumerate(['validCloseFrameReceived','webSocketClose'])]}}
        result=m.cleanup_summary(report,info)
        self.assertEqual(result['rawBusinessLogOutRequestCount'],1);self.assertEqual(result['rawLogOutSuccessCount'],1)
        self.assertEqual(result['unestablishedNoOpCloseCount'],1);self.assertEqual(result['wireCloseEvidenceCheckedCount'],1)
        self.assertTrue(result['allReportedNormal']);self.assertTrue(result['allLogoutAcksWithin5s'])
        row=result['rows'][1];self.assertEqual(row['lifecycleCategory'],'unestablished-no-op-close')
        self.assertIsNone(row['logoutAckWithin5s']);self.assertIsNone(row['wireCloseWithin5sAfterLogout'])

    def test_missing_job_refuses_before_any_event_scan(self):
        root=fixture()
        with patch.object(m,'scan_events') as scan,self.assertRaises(m.Refusal):m.summarize(root,TEST_IDENTITY)
        scan.assert_not_called()

    def test_kill_or_owned_members_and_live_monitor_refuse(self):
        for field in ['kill','active','monitor']:
            root=fixture();job=close_prefix(root)
            if field=='kill':job['jobClose']['killAppliedByClose']=True
            elif field=='active':job['finalJobSample']['jobActiveProcesses']=1
            else:write(root/'host.latest.json',{'monitorTerminal':False,'stop':False})
            write(root/'client-job/job-result.json',job)
            with self.subTest(field=field),self.assertRaises(m.Refusal):m.closed_gates(root,TEST_IDENTITY)

    def test_missing_identity_commitment_and_changed_after_chain_refuse(self):
        for field in ['identity','after']:
            root=fixture();close_prefix(root);p=root/'preflight-after.json';after=m.read_json(p)
            if field=='identity':after['pool'].pop('identityFingerprintSha256')
            else:after['comparison']['baselineReceiptSha256']='0'*64
            write(p,after)
            with self.subTest(field=field),self.assertRaises(m.Refusal):m.closed_gates(root,TEST_IDENTITY)

    def test_stage_utc_anchor_maps_same_node_clock_only(self):
        stage={'startedAtMs':1000,'endedAtMs':61000,'population':[{'atUtc':'2026-01-01T00:00:01Z','elapsedMs':1000},
          {'atUtc':'2026-01-01T00:00:11Z','elapsedMs':11000}]}
        result=m.stage_bounds(stage)
        self.assertEqual(result['endUnixMs']-result['startUnixMs'],60000);self.assertEqual(result['anchorSpreadMs'],0)
        self.assertEqual(result['startedAtUtc'],'2026-01-01T00:00:00+00:00')

    def test_not_sent_is_in_denominator_without_becoming_timeout(self):
        rows=[{'action':'walk','sent':{},'outcome':{'completed':True,'latencyMs':100,'latenessMs':0}} for _ in range(31)]
        rows.append({'action':'walk','sent':None,'outcome':{'completed':False,'reason':'previousActionPending','latenessMs':0}})
        result=m.action_metrics(rows)
        self.assertEqual(result['offered'],32);self.assertEqual(result['completed'],31);self.assertEqual(result['notSent'],1)
        self.assertEqual(result['timeouts'],0);self.assertGreaterEqual(result['completionRatio'],.95)

    def test_worst_missing_metrics_stay_null(self):
        self.assertIsNone(m.worst([{'p':3},{'p':None}],'p'));self.assertIsNone(m.percentile([]))

    def test_interpolated_cgroup_and_nic_delta_rates_exact(self):
        result=m.host_statistics([sample(0),sample(10000),sample(20000)],bounds(5000,15000),TEST_IDENTITY)
        self.assertEqual(result['targetCpuCores']['mean'],2);self.assertEqual(result['primaryNicRxMbps']['mean'],8)
        self.assertEqual(result['primaryNicTxMbps']['mean'],16);self.assertEqual(result['targetCpuCores']['startInterpolation']['fraction'],.5)

    def test_missing_boundary_coverage_never_extrapolates_or_zero_fills(self):
        result=m.host_statistics([sample(0),sample(10000)],bounds(-100,9000),TEST_IDENTITY)
        self.assertFalse(result['valid']);self.assertIsNone(result['targetCpuCores']['mean'])

    def test_counter_reset_identity_or_nic_change_invalidates(self):
        for change in ['counter','pid','nic']:
            rows=[sample(0),sample(10000),sample(20000)]
            if change=='counter':rows[2]['targetCgroup']['cpu.stat']['usage_usec']=1
            elif change=='pid':rows[1]['targetProcess']['pid']+=1
            else:rows[1]['network']['primaryNic']='other'
            with self.subTest(change=change):self.assertFalse(m.host_statistics(rows,bounds(5000,15000),TEST_IDENTITY)['valid'])

    def test_long_gap_estimate_is_labelled_unreliable(self):
        result=m.host_statistics([sample(0),sample(20000)],bounds(1000,19000),TEST_IDENTITY)
        self.assertEqual(result['targetCpuCores']['mean'],2);self.assertFalse(result['samplingReliable'])
        self.assertEqual(result['maximumSampleGapMs'],20000)

    def test_oom_discrete_boundary_ambiguity_not_fractional_zero(self):
        rows=[sample(0),sample(10000),sample(20000)];rows[1]['targetCgroup']['memory.events']['oom_kill']=1;rows[2]['targetCgroup']['memory.events']['oom_kill']=1
        result=m.host_statistics(rows,bounds(5000,15000),TEST_IDENTITY)['oomKillDeltaBounds']
        self.assertIsNone(result['delta']);self.assertEqual((result['lowerBound'],result['upperBound']),(0,1))

    def test_logout_and_wire_close_are_separate_not_combined_span(self):
        report={'cleanup':[{'actorOrdinal':0,'normalLogout':True}]}
        info={'logouts':[{'actorOrdinal':0,'connection':1,'packet':'LogOutSuccess','elapsedMs':4900,'observedAfter5000Ms':False,'monotonicMs':4900}],
          'closes':{(0,1):[{'edge':'clientCloseStarted','monotonicMs':0},
            {'edge':'validCloseFrameReceived','monotonicMs':5100,'closeFrameSent':True,'closeFrameReceived':True},
            {'edge':'webSocketClose','monotonicMs':5200,'closeFrameSent':True,'closeFrameReceived':True}]}}
        result=m.cleanup_summary(report,info);self.assertTrue(result['allLogoutAcksWithin5s']);self.assertTrue(result['allWireCloseEvidenceWithin5s'])
        info['closes'][(0,1)].append({'edge':'closeEventDeadlineExceeded'})
        self.assertFalse(m.cleanup_summary(report,info)['allWireCloseEvidenceWithin5s'])

    def test_single_pass_receipt_references_and_final_hash(self):
        root=fixture();path=root/'events.jsonl';rows=[{'kind':'offered','actorOrdinal':0,'ticketSequence':1,'action':'walk','plannedAtMs':100,'monotonicMs':100},
          {'kind':'walk','actorOrdinal':0,'connection':1,'direction':'sent','sequence':1,'monotonicMs':110},
          {'kind':'UserLocation','actorOrdinal':0,'connection':1,'direction':'received','sequence':2,'monotonicMs':130,'x':1,'y':2},
          {'kind':'ObjectWalk','actorOrdinal':1,'connection':2,'direction':'received','sequence':3,'monotonicMs':140,'x':1,'y':2,'objectId':50001},
          {'kind':'actionOutcome','actorOrdinal':0,'ticketSequence':1,'completed':True,'ownerReceipt':2,'peerReceipt':3,'peerActorOrdinal':1,'ownerReceiptAtMs':130,'peerReceiptAtMs':140}]
        # An actual offered packet follows this connection's earlier handshake.
        rows.insert(0,{'kind':'admitted','actorOrdinal':0,'connection':1})
        raw=b''.join((json.dumps(r)+'\n').encode() for r in rows);path.write_bytes(raw)
        report={'stages':[{'startedAtMs':100,'endedAtMs':200}],'eventsSha256':hashlib.sha256(raw).hexdigest(),'rows':len(rows)}
        result=m.scan_events(path,report);self.assertEqual(result['walkPairs']['matched'],1)
        path.write_bytes(raw[:-1])
        with self.assertRaisesRegex(m.Refusal,'truncated_or_oversized_event_line'):m.scan_events(path,report)

    def test_failure_classes_keep_fixture_vs_observed_timeout_distinct(self):
        stage={'actionsOverObservedSpan':{'reasons':{'No legal unoccupied ordinary step':2,'actual owner ACK timed out':1},'timeouts':1}}
        all_raw={'timeouts':1,'reasonCounts':stage['actionsOverObservedSpan']['reasons']}
        result=m.classify_failure({'failure':{'message':'No real AOI movement witness'},'resourceStopReason':'Continuous strict SLO failed'},[stage],all_raw)
        self.assertEqual(result['explicitActionTimeouts'],1);self.assertEqual(result['fixtureNoLegalStep'],2)
        self.assertTrue(result['activitySloStop']);self.assertFalse(result['possibleResourceOrSafetyStop'])
        allowed=m.classify_failure({'ok':True,'failure':None,'resourceStopReason':None},[stage],all_raw)
        self.assertFalse(allowed['actualReportedFailure']);self.assertFalse(allowed['activitySloStop'])
        self.assertFalse(allowed['fixtureSignalsInAnyReportedFailure']['noLegalStep'])

    def test_passed_completed_stage_does_not_hide_four_later_admission_timeouts(self):
        root=fixture();path=root/'events.jsonl';events=[];by_actor={}
        for actor in [0,1]:
            events.append({'kind':'admitted','actorOrdinal':actor,'connection':actor+1,'monotonicMs':0})
            plans=[('walk',2000+j*2000+actor*15) for j in range(30)]+[('chat',16500+actor*15),('chat',46500+actor*15)]
            for ticket,(action,planned) in enumerate(sorted(plans,key=lambda p:p[1]),1):
                events.extend([{'kind':'offered','action':action,'actorOrdinal':actor,'ticketSequence':ticket,'plannedAtMs':planned,'monotonicMs':planned,'latenessMs':0},
                  {'kind':action,'direction':'sent','actorOrdinal':actor,'connection':actor+1,'sequence':ticket,'monotonicMs':planned+.1},
                  {'kind':'actionOutcome','action':action,'actorOrdinal':actor,'ticketSequence':ticket,'completed':True,'timeout':False,'reason':'',
                    'latencyMs':100,'latenessMs':0,'monotonicMs':planned+100}])
        for i in range(8):
            planned=62000+i*3000;reason='timed out waiting for actual one-step peer witness' if i<4 else 'No legal unoccupied ordinary step' if i<7 else 'previousActionPending'
            events.append({'kind':'offered','action':'walk','actorOrdinal':21,'ticketSequence':i+1,'plannedAtMs':planned,'monotonicMs':planned,'latenessMs':0})
            if i<4:events.append({'kind':'walk','direction':'sent','actorOrdinal':21,'connection':22,'sequence':i+1,'monotonicMs':planned+1})
            events.append({'kind':'actionOutcome','action':'walk','actorOrdinal':21,'ticketSequence':i+1,'completed':False,'timeout':i<4,
              'reason':reason,'latenessMs':0,'monotonicMs':planned+2100})
        # A no-op cleanup is not an actionOutcome and must not affect counts.
        events.append({'kind':'webSocketClosing','edge':'clientCloseStarted','actorOrdinal':30,'connection':31,'monotonicMs':99999})
        events.sort(key=lambda e:e['monotonicMs'])
        raw=b''.join((json.dumps(e)+'\n').encode() for e in events);path.write_bytes(raw)
        stage={'name':'stage-2','players':2,'seconds':60,'startedAtMs':1000,'endedAtMs':61000,'passed':True,'failure':None,
          'population':[{'atUtc':'2026-01-01T00:00:00Z','elapsedMs':0,'controlled':2,'serverWs':2,'serverActive':2}]}
        report={'stages':[stage],'eventsSha256':hashlib.sha256(raw).hexdigest(),'rows':len(events),'ok':False,
          'failure':{'phase':'admission-50','message':'Continuous strict SLO failed actor21'},'attemptedAdmissionActors':31,'completedAdmissionActors':30,
          'cleanup':[{'actorOrdinal':i,'normalLogout':True} for i in range(31)]}
        info=m.scan_events(path,report);rows=info['stageRows'][0]
        stage['windows']=[{'minute':1,'passed':True,'actors':[{'actorOrdinal':a,'passed':True,**m.action_metrics([r for r in rows if r['actorOrdinal']==a])} for a in [0,1]]}]
        summary=m.summarize_stage(stage,rows)
        self.assertTrue(summary['reportedPassed']);self.assertTrue(summary['allFullWindowsPassed']);self.assertTrue(summary['allRawWindowMetricsAgree'])
        result=m.classify_failure(report,[summary],info['allRawActionOutcomes'])
        self.assertTrue(result['actualReportedFailure']);self.assertFalse(report['ok'])
        self.assertEqual(result['completedStageCounts']['timeouts'],0);self.assertEqual(result['completedStageCounts']['reasonCounts'],{})
        self.assertEqual(result['explicitActionTimeouts'],4);self.assertEqual(result['fixtureNoLegalStep'],3)
        self.assertEqual(result['reasonCounts']['previousActionPending'],1)
        self.assertEqual(result['allRawActionOutcomeCounts']['offered'],72)
        self.assertEqual(result['allRawActionOutcomeCounts']['actionOutcomes'],72)
        self.assertEqual(result['allRawActionOutcomeCounts']['notSent'],4)

    def test_closed_failed_report_is_summarized_without_100_or_combat_upgrade(self):
        root=fixture();close_prefix(root);client=root/'client';client.mkdir();events=client/'run.events.jsonl';events.write_bytes(b'')
        report={'schema':'mir2.cloud-reference-public-signed-load.v1','evidenceComplete':True,'evidenceLimitExceeded':False,'startedAtUtc':'2026-01-01T00:00:00Z',
          'completedAtUtc':'2026-01-01T00:00:01Z','profile':'movement-chat','stages':[],'eventsFile':str(events),'eventsSha256':hashlib.sha256(b'').hexdigest(),
          'rows':0,'ok':False,'campaignCompleted':False,'failure':{'message':'No legal unoccupied ordinary step'},'cleanup':[],
          'capacityEvidence':False,'ccuQualified':False,'combatTested':False,'fullCombatCapacityQualified':False,'physicalTriClientTested':False,
          **{key:value[1] for key,value in m.PINS.items()}}
        write(client/'run.report.json',report);write(root/'inputs.json',{'sourceRevision':m.REVISION,'binarySha256':m.BINARY})
        (root/'host.history.jsonl').write_text(json.dumps(terminal_v3())+'\n',encoding='utf8')
        summary=m.summarize(root,TEST_IDENTITY)
        self.assertTrue(summary['runCompleted']);self.assertFalse(summary['reportOk']);self.assertFalse(summary['capacityQualified100'])
        self.assertFalse(summary['fullCombatCapacityQualified']);self.assertEqual(summary['stages'],[])
        self.assertFalse(summary['closedEvidence']['processEnvelopePassed'])
        self.assertTrue(summary['failureClassification']['fixtureSignalsInAnyReportedFailure']['noLegalStep'])
        # Even with no completed resource stages, ALL history rows must match
        # the explicitly selected new measurement epoch.
        close_prefix(root,NEW_IDENTITY)
        history=[sample(0,NEW_IDENTITY),terminal_v3(NEW_IDENTITY)]
        (root/'host.history.jsonl').write_text(''.join(json.dumps(row)+'\n' for row in history),encoding='utf8')
        newer=m.summarize(root,NEW_IDENTITY)
        self.assertEqual(newer['monitorTargetIdentity']['pid'],NEW_IDENTITY[0])
        history[0]=sample(0,TEST_IDENTITY)
        (root/'host.history.jsonl').write_text(''.join(json.dumps(row)+'\n' for row in history),encoding='utf8')
        with self.assertRaisesRegex(m.Refusal,'history_identity_pin_mismatch'):m.summarize(root,NEW_IDENTITY)


if __name__=='__main__':unittest.main(verbosity=2)
