from pathlib import Path
import datetime,json,subprocess,time
base=Path(__file__).resolve().parent
root=Path('C:/Users/Administrator/.codex/worktrees/crowded-combat-control/mir2-player-journey')
head='8e1acedbb2926ca395750d2b34d641627c24642a'
ref='codex/crowded-combat-control-20261003'
def run(name,args):
    start=time.monotonic(); result=subprocess.run(args,cwd=root,stdout=subprocess.PIPE,stderr=subprocess.PIPE,timeout=60)
    with (base/(name+'.stdout.log')).open('xb') as out:out.write(result.stdout)
    with (base/(name+'.stderr.log')).open('xb') as out:out.write(result.stderr)
    with (base/(name+'.command.json')).open('x',encoding='utf-8') as out:
        json.dump({'argv':args,'exitCode':result.returncode,'elapsedSeconds':time.monotonic()-start,
                   'createdUtc':datetime.datetime.now(datetime.timezone.utc).isoformat()},out,indent=2)
    assert result.returncode==0,name
    return result.stdout
raw=run('API-REMOTE-REF-01',['gh','api','repos/Zombieliu/mir2/git/ref/heads/'+ref,'--jq','.object.sha'])
assert raw.decode().strip()==head
with (base/'API-REMOTE-CONFIRMATION-01.json').open('x',encoding='utf-8') as out:
    json.dump({'actualRemoteHead':head,'actualRemoteHeadMatches':True,
        'source':'authenticated GitHub Git ref API','previousPushExit128Preserved':True,
        'previousSSHReadFailureDoesNotProveRemoteRefMismatch':True,'repeatedPushPerformed':False},out,indent=2)
run('VERIFY-DISPATCH-01',['gh','workflow','run','web-assets-r2-release.yml','--repo','Zombieliu/mir2',
    '--ref',ref,'-f','native_delivery_operation=verify'])
print(json.dumps({'sourcePushedAndApiVerified':True,'head':head,'dispatchedOperation':'verify',
                  'stageOrPromoteDispatched':False,'cloudflareCredentialAccess':False}))
