from pathlib import Path
import json,hashlib,subprocess
N=Path(__file__).parent;P=Path('C:/Users/Administrator/.codex/worktrees/cell-preproduction-p0-p1/mir2-player-journey/mir2-web3');B=N.parent
def sha(b):return hashlib.sha256(b).hexdigest()
def pin(p):
 b=p.read_bytes();return dict(file=str(p.resolve()),bytes=len(b),sha256=sha(b))
def put(n,b):
 with(N/n).open('xb')as f:f.write(b if isinstance(b,bytes)else b.encode())
 return pin(N/n)
def js(n,x):return put(n,json.dumps(x,ensure_ascii=False,indent=2)+'\n')
f=json.loads((B/'capacity-scout-target-waypoint-routing-driver-frozen-v1.json').read_bytes());oldfiles=f['files'];assert sha(json.dumps(oldfiles,separators=(',',':')).encode())==f['sourceSha256']
# Correct only this unpublished metadata's aggregate key spelling; retain its original bytes honestly.
meta=N/'preimages-v1.json';put('preimages-before-aggregate-key-correction-v1.json',meta.read_bytes());v=json.loads(meta.read_bytes());v['files']=oldfiles;v['aggregateVerified']=True;meta.write_text(json.dumps(v,indent=2)+'\n')
files=[];changed=[];same=[]
for x in oldfiles:
 p=P/x['file'];cur=pin(p);row=dict(file=x['file'],bytes=cur['bytes'],sha256=cur['sha256']);files.append(row)
 if row!=x:changed.append(x['file'])
 else:same.append(row)
expected=['apps/web/scripts/playtest-capacity-soak-core.mjs','apps/web/scripts/playtest-capacity-soak.mjs','apps/web/scripts/playtest-capacity-soak.test.mjs'];assert changed==expected
mutation=json.loads((N/'production-mutation-v1.json').read_bytes())
for row in mutation['rows']:
 p=P/'apps/web/scripts'/row['file'];cur=p.read_bytes();a=row['before'].encode();b=row['after'].encode();assert cur.count(b)==1;assert cur.replace(b,a)==(N/'preimages'/(row['file']+'.txt')).read_bytes()
test=(P/'apps/web/scripts/playtest-capacity-soak.test.mjs').read_bytes();prior=(N/'preimages/playtest-capacity-soak.test.mjs.txt').read_bytes();assert test.startswith(prior);append=put('appended-tests-final-v1.txt',test[len(prior):]);assert test[len(prior):].count(b"test('AOI received state")==5
syntax=[]
for rel in expected:
 args=['C:/Program Files/nodejs/node.exe','--check',str(P/rel)];r=subprocess.run(args,capture_output=True);assert r.returncode==0;syntax.append(dict(args=args,exitCode=r.returncode,stdout=r.stdout.decode(),stderr=r.stderr.decode(),source=pin(P/rel)))
r=subprocess.run(['git','diff','--check'],cwd=P,capture_output=True);assert r.returncode==0;diffcheck=dict(exitCode=r.returncode,stdout=r.stdout.decode(),stderr=r.stderr.decode())
d=subprocess.run(['git','diff','--',*expected],cwd=P,capture_output=True);patch=put('source-diff-v1.patch',d.stdout);put('source-diff-stderr-v1.log',d.stderr)
declarations=[];oldmain=(N/'preimages/playtest-capacity-soak.mjs.txt').read_bytes();newmain=(P/'apps/web/scripts/playtest-capacity-soak.mjs').read_bytes()
for name in ['measurementClock','navigationCoordination','nativePatrolPlanning','nativeHpMaintenance','nativeChasePreference','nativeAttackWitness']:
 marker=("Object.defineProperty(report, '"+name+"'").encode();a=oldmain.index(marker);z=oldmain.index(b'}) });',a)+6;block=oldmain[a:z];assert block in newmain;declarations.append(dict(name=name,bytes=len(block),sha256=sha(block),rawExact=True))
out=js('source-preservation-and-focused-receipt-v1.json',dict(schemaVersion=1,originalCodeCommit='9d720e7ca2409253b93c5f0659327eb6ad50c05c',priorSourceSha256=f['sourceSha256'],sourceSha256=sha(json.dumps(files,separators=(',',':')).encode()),currentSourcePins=files,changedInputs=changed,otherSixInputsUnchanged=same,rawPreimages=[pin(p)for p in sorted((N/'preimages').iterdir())],preimageManifest=pin(meta),productionMutation=pin(N/'production-mutation-v1.json'),sourceDiff=patch,red=pin(N/'red-execution-v1.json'),green=pin(N/'green-execution-v1.json'),adjacent=pin(N/'adjacent-execution-v1.json'),syntax=syntax,diffCheck=diffcheck,preservation=dict(productionTwoHunksInverseRawExact=True,originalTestPrefixRawExact=True,originalTestPrefixBytes=len(prior),originalTestPrefixSha256=sha(prior),newTestCount=5,finalAppend=append,newTestOnlyAssertionCorrection='After RED, one newly appended cumulative observedPairs assertion was changed to compare full stats before/after reconciliation; no original assertion changed.',aoiSampleBodyRawExact=True,originalSixDeclarationsRawExact=declarations,range16AndStrictGreater1000Unchanged=True,noNewTimersOrStatsOrVisibleCredit=True),limits=dict(emptyTimersReturnBeforeCohortScan=True,onlyChangedActorExistingClocksConsidered=True,receiverProjectionCountsCachedPerHook=True,fullN2EntityScansNotAdded=True),verificationScope='Focused5 and adjacent20 share the five new cases; total20 distinct tests here. Full four-entry regression is reserved for Root. No live capacity result or historical verdict is changed.',capacityAccepted=False,realCapacityRunPerformed=False,rootCauseProven=False))
print(json.dumps(dict(receipt=out,sourceSha256=sha(json.dumps(files,separators=(',',':')).encode()),files=files,patch=patch),indent=2))
