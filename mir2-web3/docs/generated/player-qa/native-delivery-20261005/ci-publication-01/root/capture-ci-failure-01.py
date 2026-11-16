from pathlib import Path
import datetime,hashlib,json,subprocess
base=Path(__file__).resolve().parent
out=base/'actual-linux-ci-01';out.mkdir()
root=Path('C:/Users/Administrator/.codex/worktrees/crowded-combat-control/mir2-player-journey')
run_id='37250860949'
for name,args in [('run',['gh','run','view',run_id,'--repo','Zombieliu/mir2','--json','databaseId,headSha,status,conclusion,url,jobs']),
                  ('failed-log',['gh','run','view',run_id,'--repo','Zombieliu/mir2','--log-failed'])]:
    value=subprocess.run(args,cwd=root,stdout=subprocess.PIPE,stderr=subprocess.PIPE,timeout=60)
    assert value.returncode==0
    with (out/(name+('.json' if name=='run' else '.log'))).open('xb') as target:target.write(value.stdout)
run=json.loads((out/'run.json').read_bytes())
assert run['headSha']=='8e1acedbb2926ca395750d2b34d641627c24642a' and run['conclusion']=='failure'
logs=(out/'failed-log.log').read_text('utf-8-sig')
assert 'FAILED (errors=9)' in logs and 'FileNotFoundError' in logs
assert 'mir2-web3/evidence/fixtures' in logs
assert all(j['conclusion']=='skipped' for j in run['jobs'] if j['name']!='native-download-tooling-check')
receipt={'schema':'mir2.native-linux-ci-mechanical-failure.v1','runId':int(run_id),'head':run['headSha'],
    'failed':True,'testsAttempted':46,'passedMethods':37,'harnessErrors':9,'productAssertionFailures':0,
    'cause':'test fixture mkdir did not create its evidence parent on a clean checkout',
    'stagePromoteAndWebAndCredentialJobsSkipped':True,'missingRawTestReceiptAndArtifactDueSameParentError':True,
    'repair':'one test-factory mkdir parents=True; no assertions or production source changed',
    'logs':[{ 'path':p.name,'bytes':p.stat().st_size,'sha256':hashlib.sha256(p.read_bytes()).hexdigest()} for p in out.iterdir()],
    'createdUtc':datetime.datetime.now(datetime.timezone.utc).isoformat()}
with (out/'FAILURE-REVIEW-01.json').open('x',encoding='utf-8',newline='\n') as target:json.dump(receipt,target,indent=2);target.write('\n')
body=(base/'run-tests.py').read_text('utf-8')
body=body.replace("BASE/'fixtures'","BASE/'fixtures-02'").replace('ROOT-TESTS-01.log','ROOT-TESTS-02.log').replace('ROOT-TESTS-RECEIPT-01.json','ROOT-TESTS-RECEIPT-02.json')
with (base/'run-tests-02.py').open('x',encoding='utf-8',newline='\n') as target:target.write(body)
print(json.dumps({'retainedLinuxHarnessErrors':9,'productionSourceChanged':False,'oldRawEvidenceRewritten':False}))
