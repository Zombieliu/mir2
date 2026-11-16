import json,hashlib,sys,pathlib,collections,datetime,math
if sys.argv[1:]!=['--analyze-closed-formal15']:raise SystemExit('Explicit closed-run analysis argument required')
B=pathlib.Path(r'C:/mir2-build/p01-completion-20261024');R=B/'release-native-v2-scout-anti-convergence-ladder-15';O=B/'formal15-supply-failure-analysis-v1';RUN='1792974410829-a5288349'
P=pathlib.Path(r'C:/Users/Administrator/.codex/worktrees/cell-preproduction-p0-p1/mir2-player-journey/mir2-web3')
def read(p):return json.loads(pathlib.Path(p).read_text(encoding='utf-8'))
def pin(p):
 p=pathlib.Path(p);b=p.read_bytes();return {'file':str(p.resolve()),'bytes':len(b),'sha256':hashlib.sha256(b).hexdigest()}
def write(n,v):
 with (O/n).open('x',encoding='utf-8',newline='\n') as f:f.write(json.dumps(v,ensure_ascii=False,indent=2)+'\n')
def selected(row,ln):
 keys=['type','actor','phase','at','kind','status','scheduledAtMs','sentAtMs','latencyMs','plannedIntervalMs','cadence','targetId','chaseTargetId','scouting','scoutHomeIndex','plannedTarget','actual','movedCells','targetPosition','hunting','attackResult','attackRequestSent','requestId','damage','sharedHealthVerified','nativeOpportunity','initialCombatTransit','navigationCoordinationConflict','attackWitnessUnavailable','reason','homeIndex','confirmedMoves','elapsedMs','fallbackHomeIndex','selectedHomeIndex','pathSearches','hintPathSearches','hintScanTruncated','hintOrderChangedByActiveScoutPreference','hintTargetId','hintTarget','actualObserver','potentialAttackTile','truncated','rowLimit']
 return {'line':ln,**{k:row[k] for k in keys if k in row}}
reportFile=R/'reports'/f'{RUN}.report.json'; report=read(reportFile); assert report['runId']==RUN and not report['ok']
stage=next(x for x in report['stages'] if x['name']=='stage-25');w=stage['rollingWindows'][3];A={x['actor'] for x in w['activity'] if x['role']=='combat'};assert len(A)==2 and not w['passed'];start,end=w['startedAtMs'],w['endedAtMs']
rowsByType=collections.Counter();actions=collections.Counter();phaseActions=collections.Counter();stageRows=[];allDiag=[];advances=[];wireConnections=collections.Counter();schemas={};rows=0;digest=hashlib.sha256();raw=R/'reports'/f'{RUN}.samples.jsonl';st=raw.stat();size=0
with raw.open('rb') as f:
 for ln,line in enumerate(f,1):
  rows=ln;size+=len(line);digest.update(line);assert len(line)<=64*1024*1024
  q=json.loads(line);typ=q.get('type');rowsByType[typ]+=1
  if typ not in schemas:schemas[typ]=list(q)
  if typ=='action':
   actions[(q.get('kind'),q.get('status'))]+=1
   if q.get('phase')=='stage-25':phaseActions[(q.get('kind'),q.get('status'))]+=1
   if q.get('actor') in A and q.get('phase')=='stage-25':stageRows.append(selected(q,ln));assert len(stageRows)<=2000
  elif typ=='combatScoutSelection' and q.get('actor') in A:allDiag.append(selected(q,ln));assert len(allDiag)<=256
  elif typ=='combatScoutAdvanced' and q.get('actor') in A and q.get('phase')=='stage-25':advances.append(selected(q,ln));assert len(advances)<=256
  if typ=='compactNavigationWire':wireConnections[q.get('connection')]+=1
assert st.st_size==size==raw.stat().st_size and st.st_mtime_ns==raw.stat().st_mtime_ns
rawPin={'file':str(raw),'bytes':size,'rows':rows,'sha256':digest.hexdigest()};catalogFile=B/'collect-formal-native-v2-scout-anti-convergence-ladder-15-public-artifact-catalog.json';catalog=read(catalogFile);expected=next(p for p in catalog['externalArchives'] if p['kind']=='capacity-samples-jsonl');assert expected['bytes']==size and expected['sha256']==rawPin['sha256']
# Wall-clock action timestamps are completion/accounting times, not exact send or claim-lifetime boundaries.
windowRows=[x for x in stageRows if start<x.get('at',0)<=end];gaps=[];summaries=[]
for actor in sorted(A):
 actorRows=[x for x in stageRows if x['actor']==actor];win=[x for x in windowRows if x['actor']==actor];attack=[x for x in actorRows if x.get('kind')=='attack' and x.get('status')=='success']
 actorGaps=[]
 for left,right in zip(attack,attack[1:]):
  if right['at']<=start or left['at']>=end:continue
  middle=[x for x in actorRows if left['line']<x['line']<right['line'] and x.get('kind') in ['walk','run','turn','attack']]
  actorGaps.append({'actor':actor,'durationMs':right['at']-left['at'],'leftAttack':left,'rightAttack':right,'interveningCounts':dict(collections.Counter('scout' if x.get('scouting') else 'chase' if x.get('chaseTargetId') else x.get('kind') for x in middle)),'interveningActions':middle,'intersectsFailedWindow':True})
 gaps+=sorted(actorGaps,key=lambda x:x['durationMs'],reverse=True)[:3]
 s=next(x for x in w['activity'] if x['actor']==actor);summaries.append({'actor':actor,'reportActivity':s,'rawWindowKinds':dict(collections.Counter(x.get('kind') for x in win)),'rawWindowStatus':dict(collections.Counter(x.get('status') for x in win)),'scoutMoves':sum(x.get('scouting',False) and x.get('kind') in ['walk','run'] for x in win),'chaseMoves':sum(bool(x.get('chaseTargetId')) and x.get('kind') in ['walk','run'] for x in win),'scoutHomeIndices':dict(collections.Counter(x['scoutHomeIndex'] for x in win if x.get('scouting'))),'chaseTargetIds':dict(collections.Counter(x['chaseTargetId'] for x in win if x.get('chaseTargetId'))),'attackTargetIds':dict(collections.Counter(x['targetId'] for x in win if x.get('kind')=='attack')),'maximumActionLatencyMs':max(x.get('latencyMs',0) for x in win),'maximumPlannedIntervalMs':max(x.get('plannedIntervalMs',0) for x in win)})
# Conservative same-target chase ACK runs: break on any different non-chat action; no unrecorded pursuit lifetime is inferred.
segments=[]
for actor in sorted(A):
 current=None
 for x in [x for x in stageRows if x['actor']==actor]:
  if x.get('kind')=='chat':continue
  eligible=x.get('status')=='success' and x.get('kind') in ['walk','run'] and x.get('chaseTargetId')
  if eligible:
   if current and current['targetId']==x['chaseTargetId']:current['lastAt']=x['at'];current['lines'].append(x['line']);current['actions'].append(x)
   else:
    if current:segments.append(current)
    current={'actor':actor,'targetId':x['chaseTargetId'],'firstAt':x['at'],'lastAt':x['at'],'lines':[x['line']],'actions':[x]}
  else:
   if current:segments.append(current);current=None
 if current:segments.append(current)
overlaps=[]
for i,l in enumerate(segments):
 for r in segments[i+1:]:
  lo=max(l['firstAt'],r['firstAt'],start);hi=min(l['lastAt'],r['lastAt'],end)
  if l['actor']!=r['actor'] and l['targetId']==r['targetId'] and hi>lo:overlaps.append({'targetId':l['targetId'],'start':lo,'end':hi,'durationMs':hi-lo,'left':l,'right':r})
def dataset(name,statusName,store=False):
 file=R/name; data=[]
 for ln,line in enumerate(file.read_text(encoding='utf-8').splitlines(),1):
  v=json.loads(line);data.append({'line':ln,**v})
 status=read(R/statusName);assert len(data)==status['rowsQueued'] and file.stat().st_size==status['bytesQueued']
 current=[v for v in data if start<=v['observedAtMs']<=end]
 if store:
  protocol=read(B/'account-store-slow-stage-diagnostics-v1/protocol-contract-v1.json');assert all(v['stage'] in protocol['wire']['stages'] and v['operation'] in protocol['wire']['operations'] for v in data)
  crossed=[v for v in data if v['startedUnixMs']+v['maxStartUs']/1000<=end and v['startedUnixMs']+v['maxEndUs']/1000>=start]
 else:crossed=[v for v in data if v['observedAtMs']-v['durationMs']<=end and v['observedAtMs']>=start]
 return {'data':pin(file),'status':pin(R/statusName),'captureComplete':status['complete'],'rowCount':len(data),'maximumCompletedMs':max((v['durationMs'] for v in data),default=None),'largestCompletedRows':sorted(data,key=lambda x:x['durationMs'],reverse=True)[:5],'receivedWithinFailedWindow':current,'completedSpanIntersectsFailedWindow':crossed,'windowMaximumCompletedMs':max((v['durationMs'] for v in current),default=None),'countsByStageInWindow':dict(collections.Counter(v['stage'] for v in current)),'completedOnly':True,'emissionClockNotActionIdentity':True}
slow=dataset('slow-stage-public.jsonl','slow-stage-public-status.json');store=dataset('store-stage-public.jsonl','store-stage-public-status.json',True)
selectionStage=[x for x in allDiag if x.get('phase')=='stage-25'];selectionW=[x for x in selectionStage if start<=x['at']<=end]
selectionSummary={'allRunRows':len(allDiag),'stage25Rows':len(selectionStage),'failedWindowRows':len(selectionW),'allRunReorderTrueRows':sum(x.get('hintOrderChangedByActiveScoutPreference') is True for x in allDiag),'stage25ReorderTrueRows':sum(x.get('hintOrderChangedByActiveScoutPreference') is True for x in selectionStage),'failedWindowReorderTrueRows':sum(x.get('hintOrderChangedByActiveScoutPreference') is True for x in selectionW),'capSentinelRows':[x for x in allDiag if x.get('truncated')],'allStage25Rows':selectionStage,'failedWindowRowsProjection':selectionW}
selectedEvidence={'schemaVersion':1,'runId':RUN,'source':rawPin,'scope':'Only scalar action/scout projections and bounded longest gap paths; no gameplay packet payloads, snapshots, recordings or credentials.','failedWindow':{'index':4,'startedAtMs':start,'endedAtMs':end,'seconds':w['seconds']},'failedWindowActions':windowRows,'allStage25ScoutSelections':selectionStage,'scoutAdvancesStage25':advances,'longestAttackGaps':gaps,'overlappingSameTargetChaseAckSpansInFailedWindow':overlaps}
write('selected-evidence-v1.json',selectedEvidence)
analysis={'schemaVersion':1,'createdAt':datetime.datetime.now(datetime.timezone.utc).isoformat(),'runId':RUN,'scope':'Bounded closed-run supply diagnosis; one raw stream, completed diagnostics only. Not independent acceptance replay or a causal proof.','source':rawPin,'catalog':pin(catalogFile),'report':pin(reportFile),'sourceCode':pin(P/'apps/web/scripts/playtest-capacity-soak.mjs'),'currentDriverFreeze':pin(B/'capacity-scout-anti-convergence-driver-frozen-v1.json'),'formalFreeze':pin(R/'launcher-source-frozen-v1.json'),'rawParsePasses':1,'rawSourceMatchesActualCatalog':True,'rowTypes':dict(rowsByType),'rowTypeKeys':schemas,'wireRowsByConnection':dict(wireConnections),'report':{'pin':pin(reportFile),'ok':report['ok'],'passedStages':[(x['name'],x['passed'],x['measuredSeconds']) for x in report['stages']],'stage25RollingPass':[x['passed'] for x in stage['rollingWindows']],'stage25AttackActions':phaseActions[('attack','success')],'wholeSuccessfulAttacks':actions[('attack','success')],'wholeAttackStatus':{k[1]:v for k,v in actions.items() if k[0]=='attack'},'continuous':{'passed':report['continuousAcceptance']['passed'],'failureCount':report['continuousAcceptance']['failureCount'],'assessedWindows':report['continuousAcceptance']['assessedWindows']},'failedWindow':{'index':4,'startedAtMs':start,'endedAtMs':end,'seconds':w['seconds'],'passed':False}},'fighters':summaries,'scoutSelectionDiagnostics':selectionSummary,'longestAttackGapScalars':[{k:v for k,v in x.items() if k!='interveningActions'} for x in gaps],'sameTargetAckSpanOverlapCountInFailedWindow':len(overlaps),'slowStage':slow,'storeStage':store,'selectedEvidence':pin(O/'selected-evidence-v1.json'),'limitations':['Scout-selection rows are emitted only when hints exist and at most31 rows plus a cap sentinel per actor/phase. Absent rows do not establish absent hints or an inactive preference.','Active scout preference sorts at most4 existing hint waypoints only while making a new scout; it does not reset existing scout state, make exclusive claims, prove movement or provide owner AOI.','The raw wire diagnostic only observes connections1/2, so the combat actors05/0f owner/peer candidate snapshots, exact failed/deferred filters and targetClaims states cannot be reconstructed from it.','Completed Store and slow spans can be nested and must not be added. Threshold50ms omits shorter or unfinished work; no emitted row does not exclude a stall. traceId is not an account/action/request identity.','Action row wall timestamps are accounting/completion times; chase ACK-span overlap is conservative observed span evidence, not a complete internal pursuit lifetime or exact authority simultaneity.','No per-request900ms dual-packet replay, server holder attribution, map spawn guarantee, causal improvement claim or new acceptance is made.'],'gameOrSourceModified':False,'capacityAccepted':False,'P0Accepted':False,'P2Started':False}
write('analysis-v1.json',analysis)
print(json.dumps({'analysis':pin(O/'analysis-v1.json'),'selected':pin(O/'selected-evidence-v1.json'),'raw':rawPin,'rows':rows,'actions':len(stageRows),'scout':selectionSummary,'fighters':summaries,'overlapCount':len(overlaps),'slowWindowRows':len(slow['receivedWithinFailedWindow']),'slowWindowMax':slow['windowMaximumCompletedMs'],'storeWindowRows':len(store['receivedWithinFailedWindow']),'storeWindowMax':store['windowMaximumCompletedMs'],'gaps':[(x['actor'],x['durationMs'],x['interveningCounts']) for x in gaps]},ensure_ascii=False))
