from pathlib import Path
import datetime,json,subprocess,time
base=Path(__file__).resolve().parent
root=Path('C:/Users/Administrator/.codex/worktrees/crowded-combat-control/mir2-player-journey')
head='8e1acedbb2926ca395750d2b34d641627c24642a'
git=['git','-c','gc.auto=0','-C',str(root)]
assert subprocess.check_output(git+['rev-parse','HEAD']).decode().strip()==head
assert not subprocess.check_output(git+['status','--porcelain']).strip()
network=git+['-c','core.sshCommand=ssh -o ServerAliveInterval=15 -o ServerAliveCountMax=3']
argv=network+['push','origin',head+':refs/heads/codex/crowded-combat-control-20261003']
clock=time.monotonic();start=datetime.datetime.now(datetime.timezone.utc).isoformat()
with (base/'PUSH-01.log').open('xb') as out:
    rc=subprocess.call(argv,stdout=out,stderr=subprocess.STDOUT,timeout=300)
receipt={'head':head,'exitCode':rc,'startedUtc':start,'elapsedSeconds':time.monotonic()-clock,
         'forcePush':False,'actualProductionDispatch':False}
with (base/'PUSH-RECEIPT-01.json').open('x',encoding='utf-8') as out:json.dump(receipt,out,indent=2)
remote=subprocess.run(network+['ls-remote','origin','refs/heads/codex/crowded-combat-control-20261003'],stdout=subprocess.PIPE,stderr=subprocess.PIPE,timeout=90)
with (base/'REMOTE-REF-01.log').open('xb') as out:out.write(remote.stdout+remote.stderr)
receipt['actualRemoteHeadMatches']=remote.returncode==0 and remote.stdout.decode().split()[0]==head
with (base/'REMOTE-CONFIRMATION-01.json').open('x',encoding='utf-8') as out:json.dump(receipt,out,indent=2)
print(json.dumps(receipt));assert receipt['actualRemoteHeadMatches']
