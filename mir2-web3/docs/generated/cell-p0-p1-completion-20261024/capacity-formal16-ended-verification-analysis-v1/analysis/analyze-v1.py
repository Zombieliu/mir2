import json,hashlib,sys,pathlib,collections,datetime,math
B=pathlib.Path('C:/mir2-build/p01-completion-20261024');R=B/'release-native-v2-scout-selection-diagnostics-ladder-16';O=B/'formal16-supply-failure-analysis-v1';RUN='1792981760951-0079fde7'
P=pathlib.Path('C:/Users/Administrator/.codex/worktrees/cell-preproduction-p0-p1/mir2-player-journey/mir2-web3')
MODE=sys.argv[1:]
if MODE not in [['--analyze-closed-formal16'],['--analyze-selected-formal16']]:raise SystemExit('Explicit closed formal16 or saved selected-only analysis required')
def read(p):return json.loads(pathlib.Path(p).read_text(encoding='utf-8'))
def sha(b):return hashlib.sha256(b).hexdigest()
def pin(p):
 p=pathlib.Path(p);b=p.read_bytes();return {'file':str(p.resolve()),'bytes':len(b),'sha256':sha(b)}
def write(n,v):
 b=(json.dumps(v,ensure_ascii=False,indent=2)+'\n').encode();assert len(b)<=2*1024*1024
 with (O/n).open('xb') as f:f.write(b)
def counts(xs,key):return dict(collections.Counter(key(x) for x in xs))
def dist(a,b):
 if not all(isinstance(v,dict) and isinstance(v.get('x'),(int,float)) and isinstance(v.get('y'),(int,float)) for v in [a,b]):return None
 return max(abs(a['x']-b['x']),abs(a['y']-b['y']))
def selected(row,ln):
 keys=['type','actor','phase','at','kind','status','scheduledAtMs','sentAtMs','latencyMs','plannedIntervalMs','cadence','targetId','chaseTargetId','scouting','scoutHomeIndex','plannedTarget','actual','movedCells','targetPosition','declaredHome','hunting','attackResult','attackResults','attackRequestSent','requestId','damage','sharedHealthVerified','nativeOpportunity','initialCombatTransit','navigationCoordinationConflict','attackWitnessUnavailable','reason','homeIndex','confirmedMoves','elapsedMs','fallbackHomeIndex','selectedHomeIndex','pathSearches','hintPathSearches','hintScanTruncated','hintOrderChangedByActiveScoutPreference','hintTargetId','hintTarget','actualObserver','potentialAttackTile','hintCount','activeOtherWaypointCount','sharedHintCount','unsharedHintCount','hintPreferenceReason','truncated','rowLimit']
 return {'line':ln,**{k:row[k] for k in keys if k in row}}
reportFile=R/'reports'/f'{RUN}.report.json';report=read(reportFile);assert report['runId']==RUN and report['ok'] is False
stage=next(x for x in report['stages'] if x['name']=='stage-25');failed=[(i,x) for i,x in enumerate(stage['rollingWindows']) if not x['passed']];assert len(failed)==1
wi,w=failed[0];assert wi==4 and w['seconds']==60
A={x['actor'] for x in w['activity'] if x['role']=='combat'};assert A=={'Le984da05','Le984da0f'}
start,end=w['startedAtMs'],w['endedAtMs'];contextStart=min(start-60000,stage['rollingWindows'][wi-1]['startedAtMs'])
catalogFile=B/'collect-formal-native-v2-scout-selection-diagnostics-ladder-16-public-artifact-catalog.json';catalog=read(catalogFile)
raw=R/'reports'/f'{RUN}.samples.jsonl';selectedFile=O/'selected-evidence-v1.json'
if MODE==['--analyze-closed-formal16']:
 assert not selectedFile.exists();assert not raw.is_symlink()
 st=raw.stat();digest=hashlib.sha256();size=0;rows=0;rowTypes=collections.Counter();schemas={};actions=collections.Counter();phaseActions=collections.Counter();wireConnections=collections.Counter();stageRows=[];diags=[];advances=[];blocked=[];measuredAt=collections.Counter()
 for ln,line in enumerate(raw.open('rb'),1):
  rows=ln;size+=len(line);digest.update(line);assert len(line)<=64*1024*1024
  q=json.loads(line);typ=q.get('type');rowTypes[typ]+=1
  if typ not in schemas:schemas[typ]=list(q)
  if typ=='action':
   actions[(q.get('kind'),q.get('status'))]+=1
   if q.get('kind')=='attack':phaseActions[(q.get('phase'),q.get('status'))]+=1
   at=q.get('at',0)
   if q.get('kind')=='attack' and q.get('status')=='success':
    matches=[s['name'] for s in report['stages'] if s['startedAtMs']<at<=s['endedAtMs']]
    measuredAt[matches[0] if matches else 'outside-recorded-measured-time-intervals']+=1
   if q.get('actor') in A and q.get('phase')=='stage-25':stageRows.append(selected(q,ln));assert len(stageRows)<=2000
  elif typ=='combatScoutSelection':diags.append(selected(q,ln));assert len(diags)<=4096
  elif typ=='combatScoutAdvanced' and q.get('actor') in A and q.get('phase')=='stage-25':advances.append(selected(q,ln));assert len(advances)<=4096
  elif typ=='navigationBlocked' and q.get('actor') in A and contextStart<=q.get('at',0)<=end:
   if len(blocked)<16:blocked.append({'line':ln,**{k:q[k] for k in ['type','actor','phase','at','self','home','targetCursor','attemptCount','attemptsTruncated','nearbyOccupantCount','occupantsTruncated'] if k in q},'nearbyOccupants':q.get('nearbyOccupants',[])[:16]})
  if typ=='compactNavigationWire':wireConnections[q.get('connection')]+=1
 afterStat=raw.stat();assert (st.st_size,st.st_mtime_ns,st.st_ino)==(size,afterStat.st_mtime_ns,afterStat.st_ino) and size==afterStat.st_size
 rawPin={'file':str(raw.resolve()),'bytes':size,'rows':rows,'sha256':digest.hexdigest()};expected=next(p for p in catalog['externalArchives'] if p['kind']=='capacity-samples-jsonl');assert expected['bytes']==size and expected['sha256']==rawPin['sha256']
 # Keep only the requested two-minute context, plus one adjacent attack per edge for gap accounting.
 contextRows=[x for x in stageRows if contextStart<=x.get('at',0)<=end];edges=[]
 for actor in sorted(A):
  attacks=[x for x in stageRows if x['actor']==actor and x.get('kind')=='attack' and x.get('status')=='success'];before=[x for x in attacks if x['at']<contextStart];after=[x for x in attacks if x['at']>end]
  edges+=before[-1:]+after[:1]
 saved={'schemaVersion':1,'runId':RUN,'scope':'One authentic raw stream; only scalar actions for both W5 fighters in preceding60s plus failed window, adjacent attack edges, complete stage25 scout selection rows/sentinels, and bounded navigation diagnostics. No packet payload or snapshot dump.','raw':rawPin,'rawStatBefore':{'bytes':st.st_size,'mtimeNs':st.st_mtime_ns},'rawStatAfter':{'bytes':afterStat.st_size,'mtimeNs':afterStat.st_mtime_ns},'rawSourceMatchesCatalog':True,'catalog':pin(catalogFile),'window':{'index':wi+1,'startedAtMs':start,'endedAtMs':end,'seconds':w['seconds'],'contextStartedAtMs':contextStart},'rowTypes':dict(rowTypes),'rowTypeKeys':schemas,'wholeActions':[{'kind':k[0],'status':k[1],'count':v} for k,v in actions.items()],'attackRowsByPhaseAndStatus':[{'phase':k[0],'status':k[1],'count':v} for k,v in phaseActions.items()],'successfulAttackRowsByAccountingAtInterval':dict(measuredAt),'wireRowsByConnection':dict(wireConnections),'contextActions':contextRows,'adjacentAttackEdges':edges,'completeStage25ScoutSelectionRows':[x for x in diags if x.get('phase')=='stage-25'],'allRunScoutSelectionCountsByActorPhase':dict(collections.Counter(x['actor']+'/'+x.get('phase','') for x in diags)),'scoutAdvancesInContext':[x for x in advances if contextStart<=x.get('at',0)<=end],'allStage25ScoutAdvanceReasonCounts':counts(advances,lambda x:x.get('reason')),'boundedNavigationBlocked':blocked,'totalTwoFighterStage25ActionCount':len(stageRows)}
 write('selected-evidence-v1.json',saved)
saved=read(selectedFile);rawPin=saved['raw'];stageRows=sorted(saved['contextActions']+saved['adjacentAttackEdges'],key=lambda x:x['line']);diags=saved['completeStage25ScoutSelectionRows']
scenario=read(R/'scenario.json');anchors=report['scenario']['anchors'];assert [(p['x'],p['y']) for p in anchors]==[(p['x'],p['y']) for p in scenario['anchors']]
cluster=next(c for c in scenario['combatClusters'] if 5 in c['combatActorIndices']);homes=[anchors[i] for i in cluster['movementActorIndices']]
def home(index):return homes[index] if isinstance(index,int) and 0<=index<len(homes) else None
REASONS={'insufficient-hints','no-active-other-waypoints','all-hints-shared','all-hints-unshared','reordered'};FIELDS=['hintCount','activeOtherWaypointCount','sharedHintCount','unsharedHintCount','hintPreferenceReason']
def contract(x):
 if x.get('truncated'):return 'sentinel'
 if not all(k in x for k in FIELDS):return 'missing-fields'
 h,o,s,u,r=[x[k] for k in FIELDS]
 integer=lambda z,lo,hi:isinstance(z,int) and not isinstance(z,bool) and lo<=z<=hi
 if not integer(h,0,4) or r not in REASONS:return 'invalid-value-or-reason'
 if r=='insufficient-hints':ok=h<2 and o is None and s is None and u is None and x.get('hintOrderChangedByActiveScoutPreference') is False
 else:
  ok=integer(o,0,100) and integer(s,0,4) and integer(u,0,4) and h>=2 and s+u==h
  if r=='no-active-other-waypoints':ok=ok and o==0 and s==0 and u==h and x.get('hintOrderChangedByActiveScoutPreference') is False
  elif r=='all-hints-shared':ok=ok and o>0 and s==h and u==0 and x.get('hintOrderChangedByActiveScoutPreference') is False
  elif r=='all-hints-unshared':ok=ok and o>0 and s==0 and u==h and x.get('hintOrderChangedByActiveScoutPreference') is False
  elif r=='reordered':ok=ok and o>0 and s>0 and u>0 and x.get('hintOrderChangedByActiveScoutPreference') is True
 return 'valid' if ok else 'branch-inconsistent'
def diagSummary(xs):
 return {'rows':len(xs),'sentinels':sum(bool(x.get('truncated')) for x in xs),'fieldContracts':counts(xs,contract),'preferenceReasons':counts([x for x in xs if not x.get('truncated')],lambda x:x.get('hintPreferenceReason')),'hintCounts':counts([x for x in xs if not x.get('truncated')],lambda x:str(x.get('hintCount'))),'activeOtherWaypointCounts':counts([x for x in xs if not x.get('truncated')],lambda x:str(x.get('activeOtherWaypointCount'))),'sharedUnsharedTuples':counts([x for x in xs if not x.get('truncated')],lambda x:f"{x.get('sharedHintCount')}/{x.get('unsharedHintCount')}"),'stablePartitionApplied':sum(x.get('hintOrderChangedByActiveScoutPreference') is True for x in xs)}
linked=[]
for d in diags:
 if d.get('truncated'):continue
 hs=home(d.get('selectedHomeIndex'));nxt=next((x for x in stageRows if x['actor']==d['actor'] and x['line']>d['line']),None)
 following=[x for x in stageRows if x['actor']==d['actor'] and x['line']>d['line'] and x.get('at',0)<=min(end,d['at']+20000)]
 step=[]
 for x in following:
  if x.get('kind')=='chat':continue
  if not x.get('scouting') or x.get('scoutHomeIndex')!=d.get('selectedHomeIndex'):break
  if len(step)==12:break
  step.append({'action':x,'actualToDeclaredWaypointCells':dist(x.get('actual'),hs),'actualToHintTargetCells':dist(x.get('actual'),d.get('hintTarget')),'actualToPotentialAttackTileCells':dist(x.get('actual'),d.get('potentialAttackTile'))})
 linked.append({'selection':d,'declaredSelectedWaypoint':hs,'declaredFallbackWaypoint':home(d.get('fallbackHomeIndex')),'observerToDeclaredWaypointCells':dist(d.get('actualObserver'),hs),'hintTargetToDeclaredWaypointCells':dist(d.get('hintTarget'),hs),'attackTileToDeclaredWaypointCells':dist(d.get('potentialAttackTile'),hs),'nextRecordedActorAction':nxt,'nextActionMatchesSelectedScout':bool(nxt and nxt.get('scouting') and nxt.get('scoutHomeIndex')==d.get('selectedHomeIndex')),'boundedFollowingSameWaypointScoutAcks':step,'followingAckCap':12,'windowContextOnly':True})
windowRows=[x for x in stageRows if start<x.get('at',0)<=end];fighters=[];gaps=[];segments=[]
for actor in sorted(A):
 xs=[x for x in stageRows if x['actor']==actor];win=[x for x in windowRows if x['actor']==actor];attacks=[x for x in xs if x.get('kind')=='attack' and x.get('status')=='success'];actorGaps=[]
 for left,right in zip(attacks,attacks[1:]):
  if right['at']<=start or left['at']>=end:continue
  middle=[x for x in xs if left['line']<x['line']<right['line'] and x.get('kind') in ['walk','run','turn','attack']]
  actorGaps.append({'actor':actor,'durationMs':right['at']-left['at'],'leftAttack':left,'rightAttack':right,'interveningCounts':counts(middle,lambda x:'scout' if x.get('scouting') else 'chase' if x.get('chaseTargetId') else x.get('kind')),'interveningActions':middle})
 gaps+=sorted(actorGaps,key=lambda x:x['durationMs'],reverse=True)[:3]
 active=[x for x in win if x.get('kind')=='attack' and x.get('status')=='success'];activity=next(x for x in w['activity'] if x['actor']==actor)
 fighters.append({'actor':actor,'reportActivity':activity,'rawWindowKinds':counts(win,lambda x:x.get('kind')),'rawWindowStatus':counts(win,lambda x:x.get('status')),'scoutMoves':sum(x.get('scouting',False) and x.get('kind') in ['walk','run'] for x in win),'chaseMoves':sum(bool(x.get('chaseTargetId')) and x.get('kind') in ['walk','run'] for x in win),'scoutHomeIndices':counts([x for x in win if x.get('scouting')],lambda x:str(x['scoutHomeIndex'])),'chaseTargetIds':counts([x for x in win if x.get('chaseTargetId')],lambda x:str(x['chaseTargetId'])),'attackTargetIds':counts([x for x in active],lambda x:str(x['targetId'])),'maximumActionLatencyMs':max((x.get('latencyMs',0) for x in win),default=None),'maximumPlannedIntervalMs':max((x.get('plannedIntervalMs',0) for x in win),default=None),'windowEdgeWithoutAttackMs':{'leading':(active[0]['at']-start) if active else end-start,'trailing':(end-active[-1]['at']) if active else end-start},'diagnosticsInFailedWindow':diagSummary([x for x in diags if x['actor']==actor and start<=x.get('at',0)<=end])})
 current=None
 for x in xs:
  if x.get('kind')=='chat':continue
  eligible=x.get('status')=='success' and x.get('kind') in ['walk','run'] and x.get('chaseTargetId')
  if eligible:
   if current and current['targetId']==x['chaseTargetId']:current['lastAt']=x['at'];current['actions'].append(x)
   else:
    if current:segments.append(current)
    current={'actor':actor,'targetId':x['chaseTargetId'],'firstAt':x['at'],'lastAt':x['at'],'actions':[x]}
  elif current:segments.append(current);current=None
 if current:segments.append(current)
overlaps=[]
for i,l in enumerate(segments):
 for rr in segments[i+1:]:
  lo=max(l['firstAt'],rr['firstAt'],start);hi=min(l['lastAt'],rr['lastAt'],end)
  if l['actor']!=rr['actor'] and l['targetId']==rr['targetId'] and hi>lo:overlaps.append({'targetId':l['targetId'],'startedAtMs':lo,'endedAtMs':hi,'durationMs':hi-lo,'left':l,'right':rr})
def dataset(name,statusName,store=False):
 file=R/name;b=file.read_bytes();data=[{'line':i,**json.loads(line)} for i,line in enumerate(b.splitlines(),1)];status=read(R/statusName);assert len(data)==status['rowsQueued'] and len(b)==status['bytesQueued']
 if store:
  protocol=read(B/'account-store-slow-stage-diagnostics-v1/protocol-contract-v1.json');assert all(x['stage'] in protocol['wire']['stages'] and x['operation'] in protocol['wire']['operations'] for x in data)
  endpoints=lambda x:(x['startedUnixMs']+x['maxStartUs']/1000,x['startedUnixMs']+x['maxEndUs']/1000)
 else:endpoints=lambda x:(x['observedAtMs']-x['durationMs'],x['observedAtMs'])
 current=[x for x in data if start<=x['observedAtMs']<=end];context=[x for x in data if contextStart<=x['observedAtMs']<=end];crossed=[x for x in data if endpoints(x)[0]<=end and endpoints(x)[1]>=start]
 return {'data':{'file':str(file.resolve()),'bytes':len(b),'sha256':sha(b)},'status':pin(R/statusName),'captureComplete':status['complete'],'rowCount':len(data),'maximumCompletedMs':max((x['durationMs'] for x in data),default=None),'largestCompletedRows':sorted(data,key=lambda x:x['durationMs'],reverse=True)[:5],'receivedWithinFailedWindowCount':len(current),'receivedWithinFailedWindow':current[:64],'windowRowsTruncated':len(current)>64,'completedSpanIntersectsFailedWindowCount':len(crossed),'completedSpanIntersectsFailedWindow':crossed[:64],'intersectingRowsTruncated':len(crossed)>64,'receivedWithinContextCount':len(context),'contextLargestCompletedRows':sorted(context,key=lambda x:x['durationMs'],reverse=True)[:12],'windowMaximumCompletedMs':max((x['durationMs'] for x in current),default=None),'countsByStageInWindow':counts(current,lambda x:x['stage']),'completedOnly':True,'emissionClockNotActionIdentity':True,'rowProjectionCap':64}
slow=dataset('slow-stage-public.jsonl','slow-stage-public-status.json');store=dataset('store-stage-public.jsonl','store-stage-public-status.json',True)
analysis={'schemaVersion':1,'createdAt':datetime.datetime.now(datetime.timezone.utc).isoformat(),'runId':RUN,'scope':'Bounded closed formal16 W5 individual supply diagnosis derived from the formal15 one-pass scalar analyzer. One raw stream only, not independent acceptance replay or causal root cause.','analyzer':pin(__file__),'derivedFrom':pin(B/'formal15-supply-failure-analysis-v1/analyze-v1.py'),'raw':rawPin,'rawParsePasses':1,'rawSourceMatchesCatalog':True,'rawStatBefore':saved['rawStatBefore'],'rawStatAfter':saved['rawStatAfter'],'catalog':pin(catalogFile),'report':pin(reportFile),'sourceCode':pin(P/'apps/web/scripts/playtest-capacity-soak.mjs'),'driverFreeze':pin(B/'capacity-scout-selection-diagnostics-driver-frozen-v1.json'),'formalFreeze':pin(R/'launcher-source-frozen-v1.json'),'rootFacts':pin(B/'formal16-root-actual-terminal-and-consumer-facts-v1.json'),'selectedEvidence':pin(selectedFile),'reportSummary':{'ok':False,'phases':[(x['name'],x['passed'],x['measuredSeconds']) for x in report['stages']],'stage25RollingPass':[x['passed'] for x in stage['rollingWindows']],'stage25AttackActions':sum(x['attacks'] for x in stage['activity']),'wholeSuccessfulAttacks':next(x['count'] for x in saved['wholeActions'] if x['kind']=='attack' and x['status']=='success'),'reportMeasuredAttackCount':413,'outsideMeasuredReportCount':74,'outsideMeasuredReportClassification':'Not independently classified; raw.phase and accounting-at interval counters are separate projections, not replacement ledgers.','rawAttackRowsByPhaseAndStatus':saved['attackRowsByPhaseAndStatus'],'rawSuccessfulAttackRowsByAccountingAtInterval':saved['successfulAttackRowsByAccountingAtInterval'],'continuous':{'passed':report['continuousAcceptance']['passed'],'failureCount':report['continuousAcceptance']['failureCount'],'assessedWindows':report['continuousAcceptance']['assessedWindows']},'failedWindow':saved['window'],'jointW5AttackActions':sum(x['attacks'] for x in w['activity'] if x['role']=='combat'),'jointTwoFighterMinimum':36,'individualMinimum':18,'aggregateAboveMinimumDoesNotPassIndividualGate':True},'fighters':fighters,'scoutSelectionDiagnostics':{'completeStage25':diagSummary(diags),'preceding60sPlusW5':diagSummary([x for x in diags if contextStart<=x.get('at',0)<=end]),'W5':diagSummary([x for x in diags if start<=x.get('at',0)<=end]),'allStage25Rows':diags,'selectedWaypointAndAckLinks':linked,'hintCountMeansWaypointHintsNotDistinctObjectIds':True,'nullOtherCountsMeanUnevaluatedNotZero':True,'reorderedMeansMixedStablePartitionAppliedNotArrivalOrGuaranteedSequenceChange':True},'longestAttackGaps':gaps,'sameTargetAckSpanOverlapsInW5':overlaps,'slowStage':slow,'storeStage':store,'rowTypes':saved['rowTypes'],'wireRowsByConnection':saved['wireRowsByConnection'],'limitations':['Selection diagnostic rows exist only on new-scout hint paths with >=1 hint; existing scout, empty hints, failed fallback, exhausted path budget and32-row cap can leave no detailed row.','HintCount is a waypoint-hint count. Different observers can see the same objectId; only the chosen hint ID is recorded, so distinct candidate monster counts cannot be reconstructed.','activeOtherWaypointCount is a count of distinct eligible declared same-map waypoints after active/alive/20s/24ACK filters. Nullable unevaluated counts do not establish absence of other scouts or fighters.','Hint selection-to-action links use bounded next recorded rows and declared scenario coordinates. They do not reconstruct unrecorded snapshots, failed/deferred targets, targetClaims or actual internal pursuit lifetimes.','The available wire rows have only their reported connection counters; this diagnosis does not reconstruct either fighter owner/peer candidate snapshots or perform per-request900ms dual-packet replay.','Base and Store completed rows are50ms left-censored and may be nested; do not add durations. No row cannot exclude <50ms or unfinished waits; traceId and receiver time are not account/action/request identities.','Action wall timestamps are accounting/completion time. Raw phase and interval projections are not substitutes for report counters or independent acceptance ledgers.','W5 total38>=36 does not prove total scene monster shortage or fair/available distribution; the unchanged individual05 gate still fails at15<18. No business source, threshold, runtime, geometry or history is modified.'],'gameOrSourceModified':False,'capacityAccepted':False,'P0Accepted':False,'P2Started':False}
write('analysis-v1.json',analysis)
summary={'schemaVersion':1,'runId':RUN,'analysis':pin(O/'analysis-v1.json'),'selected':pin(selectedFile),'raw':rawPin,'failedWindow':saved['window'],'fighters':[{k:v for k,v in x.items() if k not in ['reportActivity']} for x in fighters],'stage25Diagnostics':diagSummary(diags),'W5Diagnostics':diagSummary([x for x in diags if start<=x.get('at',0)<=end]),'longestGaps':[{'actor':x['actor'],'durationMs':x['durationMs'],'leftAt':x['leftAttack']['at'],'rightAt':x['rightAttack']['at'],'interveningCounts':x['interveningCounts']} for x in gaps],'sameTargetAckOverlapCount':len(overlaps),'slowW5CompletedRows':slow['receivedWithinFailedWindowCount'],'slowW5MaximumCompletedMs':slow['windowMaximumCompletedMs'],'storeW5CompletedRows':store['receivedWithinFailedWindowCount'],'storeW5MaximumCompletedMs':store['windowMaximumCompletedMs'],'hintsAndHomes':[{'line':x['selection']['line'],'actor':x['selection']['actor'],'at':x['selection']['at'],'reason':x['selection'].get('hintPreferenceReason'),'hintCount':x['selection'].get('hintCount'),'other':x['selection'].get('activeOtherWaypointCount'),'shared':x['selection'].get('sharedHintCount'),'unshared':x['selection'].get('unsharedHintCount'),'selectedHomeIndex':x['selection'].get('selectedHomeIndex'),'hintTargetId':x['selection'].get('hintTargetId'),'declaredWaypoint':x['declaredSelectedWaypoint'],'actualObserver':x['selection'].get('actualObserver'),'hintTarget':x['selection'].get('hintTarget'),'potentialAttackTile':x['selection'].get('potentialAttackTile'),'observerToHome':x['observerToDeclaredWaypointCells'],'targetToHome':x['hintTargetToDeclaredWaypointCells'],'attackTileToHome':x['attackTileToDeclaredWaypointCells'],'nextAction':x['nextRecordedActorAction'],'followingScoutAcks':x['boundedFollowingSameWaypointScoutAcks']} for x in linked if contextStart<=x['selection']['at']<=end],'capacityAccepted':False,'P0Accepted':False}
write('summary-v1.json',summary)
print(json.dumps({'analysis':pin(O/'analysis-v1.json'),'summary':pin(O/'summary-v1.json'),'selected':pin(selectedFile),'raw':rawPin,'stage25Diagnostics':summary['stage25Diagnostics'],'W5Diagnostics':summary['W5Diagnostics'],'fighters':summary['fighters'],'longestGaps':summary['longestGaps'],'sameTargetAckOverlapCount':len(overlaps),'slowW5':[slow['receivedWithinFailedWindowCount'],slow['windowMaximumCompletedMs']],'storeW5':[store['receivedWithinFailedWindowCount'],store['windowMaximumCompletedMs']],'hintLinkCount':len(summary['hintsAndHomes'])},ensure_ascii=False))
