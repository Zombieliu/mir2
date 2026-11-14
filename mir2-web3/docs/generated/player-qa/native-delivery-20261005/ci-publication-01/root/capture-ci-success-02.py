from pathlib import Path,PurePosixPath
import datetime,hashlib,io,json,stat,subprocess,zipfile
base=Path(__file__).resolve().parent
out=base/'actual-linux-ci-02';out.mkdir()
root=Path('C:/Users/Administrator/.codex/worktrees/crowded-combat-control/mir2-player-journey')
run_id='37251484195'
def captured(name,argv):
    value=subprocess.run(argv,cwd=root,stdout=subprocess.PIPE,stderr=subprocess.PIPE,timeout=90)
    assert value.returncode==0
    with (out/name).open('xb') as f:f.write(value.stdout)
    return value.stdout
run=json.loads(captured('run.json',['gh','run','view',run_id,'--repo','Zombieliu/mir2','--json','databaseId,headSha,status,conclusion,url,jobs']))
assert run['headSha']=='bb4679272cb6ae6a677f18a0e636d31dd3d139f3' and run['conclusion']=='success'
assert all(j['conclusion']==('success' if j['name']=='native-download-tooling-check' else 'skipped') for j in run['jobs'])
captured('tooling.log',['gh','run','view',run_id,'--repo','Zombieliu/mir2','--log'])
assets=json.loads(captured('artifacts.json',['gh','api','repos/Zombieliu/mir2/actions/runs/'+run_id+'/artifacts']))
items=[a for a in assets['artifacts'] if a['name']=='native-r17-tooling-'+run_id]
assert len(items)==1 and not items[0]['expired']
item=items[0]
raw=captured('tooling-artifact.zip',['gh','api','repos/Zombieliu/mir2/actions/artifacts/'+str(item['id'])+'/zip'])
archive_sha=hashlib.sha256(raw).hexdigest()
if item.get('digest'):assert item['digest']=='sha256:'+archive_sha
with zipfile.ZipFile(io.BytesIO(raw)) as z:
    names=z.namelist();assert len(set(names))==len(names)
    for info in z.infolist():
        p=PurePosixPath(info.filename)
        assert not p.is_absolute() and '..' not in p.parts
        assert stat.S_IFMT(info.external_attr>>16) != stat.S_IFLNK
    receipts=[n for n in names if PurePosixPath(n).name.startswith('TEST-RECEIPT-') and n.endswith('.json')]
    assert len(receipts)==1
    body=z.read(receipts[0]);r=json.loads(body)
    assert r['passed'] is True and r['run']==46 and r['pass']==46 and r['errors']==0 and r['fail']==0 and r['skipped']==0
    assert r['platform']=='linux' and r['actualNetworkRequests']==0 and r['credentialsUsed'] is False
    assert r['sourceSha256']=='8fcb1f581fafe0cf254b256e66394397be436facec03d53d6fdb8d7bf83c601f'
    assert r['testSha256']=='a820e757325bfc81e0a45971cba33151c81181964d962208a74411609a377714'
    assert r['literalPlanSha256']=='4a7eda51fd375b64a250faa8d1709f8b611af8d60172aabf7c1032b864e490fc'
    with (out/'actual-test-receipt.json').open('xb') as f:f.write(body)
v={'schema':'mir2.native-linux-ci-root-success.v1','passed':True,'runId':int(run_id),'sourceHead':run['headSha'],
   'actualLinuxTestsPassed':46,'errors':0,'skipped':0,'posixBranchesExecutedByRealLinux':True,
   'productionAndStageAndWebAndCredentialJobsSkipped':True,'cloudflareCredentialOrNetworkProved':False,
   'artifactId':item['id'],'zipSha256':archive_sha,'zipBytes':len(raw),'zipMemberCount':len(names),
   'zipExtracted':False,'receiptByteSha256':hashlib.sha256(body).hexdigest(),'createdUtc':datetime.datetime.now(datetime.timezone.utc).isoformat()}
with (out/'ROOT-SUCCESS-REVIEW.json').open('x',encoding='utf-8',newline='\n') as f:json.dump(v,f,indent=2);f.write('\n')
print(json.dumps(v))
