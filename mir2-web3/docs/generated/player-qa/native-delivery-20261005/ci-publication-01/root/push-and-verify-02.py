from pathlib import Path
import datetime,json,subprocess,time
base=Path(__file__).resolve().parent
root=Path('C:/Users/Administrator/.codex/worktrees/crowded-combat-control/mir2-player-journey')
head='bb4679272cb6ae6a677f18a0e636d31dd3d139f3'
ref='codex/crowded-combat-control-20261003'
git=['git','-c','gc.auto=0','-C',str(root)]
assert subprocess.check_output(git+['rev-parse','HEAD']).decode().strip()==head
assert not subprocess.check_output(git+['status','--porcelain']).strip()
start=time.monotonic()
argv=git+['-c','core.sshCommand=ssh -o ServerAliveInterval=15 -o ServerAliveCountMax=3','push','origin',head+':refs/heads/'+ref]
with (base/'PUSH-02.log').open('xb') as out:rc=subprocess.call(argv,stdout=out,stderr=subprocess.STDOUT,timeout=300)
receipt={'head':head,'pushExitCode':rc,'elapsedSeconds':time.monotonic()-start,
         'forcePush':False,'actualRemoteHeadMatches':None,'createdUtc':datetime.datetime.now(datetime.timezone.utc).isoformat()}
with (base/'PUSH-02-RECEIPT.json').open('x',encoding='utf-8') as out:json.dump(receipt,out,indent=2)
api=subprocess.run(['gh','api','repos/Zombieliu/mir2/git/ref/heads/'+ref,'--jq','.object.sha'],cwd=root,stdout=subprocess.PIPE,stderr=subprocess.PIPE,timeout=60)
with (base/'API-REMOTE-REF-02.log').open('xb') as out:out.write(api.stdout+api.stderr)
assert api.returncode==0 and api.stdout.decode().strip()==head
receipt['actualRemoteHeadMatches']=True
with (base/'API-REMOTE-CONFIRMATION-02.json').open('x',encoding='utf-8') as out:json.dump(receipt,out,indent=2)
args=['gh','workflow','run','web-assets-r2-release.yml','--repo','Zombieliu/mir2','--ref',ref,'-f','native_delivery_operation=verify']
event=subprocess.run(args,cwd=root,stdout=subprocess.PIPE,stderr=subprocess.PIPE,timeout=60)
with (base/'VERIFY-DISPATCH-02.log').open('xb') as out:out.write(event.stdout+event.stderr)
assert event.returncode==0
with (base/'VERIFY-DISPATCH-02.json').open('x',encoding='utf-8') as out:json.dump({'argv':args,'head':head,'exitCode':event.returncode,'stagePromoteAndProductionDispatched':False},out,indent=2)
print(json.dumps({'head':head,'apiVerified':True,'pushExitCode':rc,'operation':'verify'}))
