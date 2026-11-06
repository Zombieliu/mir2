from pathlib import Path
import hashlib, json, os, stat

BASE=Path(__file__).resolve().parent
ROOT=Path('C:/Users/Administrator/.codex/worktrees/crowded-combat-control/mir2-player-journey/mir2-web3')
WORKER=Path('C:/mir2-playtest-releases/20261004-native-r18/native-r17-ci-delivery-worker-01')
freeze_raw=(WORKER/'FROZEN-DELIVERY.json').read_bytes()
assert hashlib.sha256(freeze_raw).hexdigest()=='2b4486122140eef2fc05c69fdfc6d1a7706c0e6ea1f1fa40ceb00716dd2df8d9'
freeze=json.loads(freeze_raw)
counts={'regular':0,'fixture-link-or-reparse':0}
for entry in freeze['artifacts']:
    p=WORKER/entry['path']; info=p.lstat()
    if entry['type']=='regular':
        assert stat.S_ISREG(info.st_mode) and not getattr(info,'st_file_attributes',0)&0x400
        raw=p.read_bytes()
        assert len(raw)==entry['size'] and hashlib.sha256(raw).hexdigest()==entry['sha256']
        assert info.st_nlink==entry['nlink']
    else:
        assert entry['type']=='fixture-link-or-reparse' and entry['followed'] is False
        assert os.readlink(p)==entry['target']
    counts[entry['type']]+=1
assert counts=={'regular':192,'fixture-link-or-reparse':18}
selected=[]
for entry in freeze['productionFiles']:
    dest=ROOT/entry['path']; assert not dest.exists() and not dest.is_symlink()
    raw=(WORKER/entry['path']).read_bytes()
    with dest.open('xb') as out: out.write(raw)
    assert dest.read_bytes()==raw
    selected.append(entry)
receipt={'schema':'mir2.native-r17-ci-root-integration.v1','passed':True,
         'workerFreezeSha256':hashlib.sha256(freeze_raw).hexdigest(),
         'regularArtifactsRehashed':counts['regular'],'negativeFixtureLinksRecheckedWithoutFollowing':counts['fixture-link-or-reparse'],
         'productionFiles':selected,'actualNetworkRequests':0,'credentialsRead':False,'deployed':False}
with (BASE/'INTEGRATION-RECEIPT-01.json').open('x',encoding='utf-8',newline='\n') as out:
    json.dump(receipt,out,indent=2);out.write('\n')
print(json.dumps({'passed':True,'files':len(selected),'rehashedRegular':counts['regular'],'linksFollowed':0}))
