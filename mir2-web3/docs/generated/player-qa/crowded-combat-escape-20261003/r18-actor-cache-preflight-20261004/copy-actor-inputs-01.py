import hashlib, json, os, pathlib, subprocess

ROOT=pathlib.Path('C:/Users/Administrator/.codex/worktrees/r18/mir2-player-journey/mir2-web3')
PHASE=pathlib.Path('C:/mir2-playtest-releases/20261004-native-r17/map-implementation/phase2-208-runtime-209-resources')
SOURCE=PHASE/'actor-input/original-ui'
DEST=ROOT/'apps/web/public/original-ui'
OUT=pathlib.Path('C:/mir2-playtest-releases/20261004-native-r18/actor-input-copy-01.json')
sha=lambda data:hashlib.sha256(data).hexdigest()
assert not OUT.exists()
receipt=json.loads((PHASE/'implementation-receipt.json').read_text(encoding='utf-8-sig'))
for item in [receipt['actualResources']['frameSetCatalog'],receipt['actualResources']['actor049']['metadata']]:
    data=pathlib.Path(item['path']).read_bytes()
    assert len(data)==item['bytes'] and sha(data)==item['sha256']
files=[p for p in SOURCE.rglob('*') if p.is_file()]
assert files
rows=[]
for p in files:
    assert not p.is_symlink() and not os.path.isjunction(p)
    rel=p.relative_to(SOURCE); target=DEST/rel
    assert target.resolve().is_relative_to(DEST.resolve())
    cursor=target.parent
    while cursor!=ROOT.parent:
        assert not cursor.is_symlink() and not os.path.isjunction(cursor)
        cursor=cursor.parent
    data=p.read_bytes()
    rows.append((p,target,data,{'relative':rel.as_posix(),'bytes':len(data),'sha256':sha(data)}))
paths=[target.relative_to(ROOT).as_posix() for _,target,_,_ in rows]
stdin=('\0'.join(paths)+'\0').encode()
result=subprocess.run(['git','-c','gc.auto=0','check-ignore','--stdin','-z'],cwd=ROOT,input=stdin,capture_output=True)
ignored=set(result.stdout.decode().rstrip('\0').split('\0'))
for _,target,data,_ in rows:
    if target.relative_to(ROOT).as_posix() not in ignored:
        assert target.exists() and target.read_bytes()==data,'Refuse changed non-ignored actor source: '+str(target)
changed=0
for _,target,data,_ in rows:
    if not target.exists() or target.read_bytes()!=data:
        target.parent.mkdir(parents=True,exist_ok=True); target.write_bytes(data); changed+=1
    assert sha(target.read_bytes())==sha(data)
assert not subprocess.check_output(['git','-c','gc.auto=0','status','--porcelain=v1','--untracked-files=all'],cwd=ROOT).strip()
payload={'schema':'mir2.r18.physical-source-actor-input-copy.v1','sourceReceiptSha256':sha((PHASE/'implementation-receipt.json').read_bytes()),'exactSourceRevision':'57729ba8c1473ca920a8ea8b3d6e11105d9d4d93','actorSource':str(SOURCE),'target':str(DEST),'files':[row for _,_,_,row in rows],'fileCount':len(rows),'bytes':sum(row['bytes'] for _,_,_,row in rows),'filesAddedOrReplaced':changed,'trackedSourceClean':True,'protectedInstallationChanged':False}
OUT.write_text(json.dumps(payload,indent=2)+'\n',encoding='utf-8')
print(json.dumps({k:payload[k] for k in ['fileCount','bytes','filesAddedOrReplaced','trackedSourceClean']}))
