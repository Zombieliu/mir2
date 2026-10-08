import datetime
import hashlib
import json
from pathlib import Path
import zipfile

out = Path('C:/Users/Administrator/.codex/worktrees/classic-gateway-budget-20261008/mir2-player-journey/mir2-web3/docs/generated/player-qa/classic-20261008/delivery-checkpoint-05')

def digest(data):
    return hashlib.sha256(data).hexdigest()

def write_new(path, data):
    with path.open('xb') as stream:
        stream.write(data)

initial_proof_bytes = (out/'ROOT-FINAL-05.json').read_bytes()
proof = json.loads(initial_proof_bytes)
assert digest(initial_proof_bytes) == 'b2d8931d5c2ef36ab79257bbe27a0d6279fcabc3b83256d80b04893a8e0a4693'
with zipfile.ZipFile(out/'initial-archive-before-review-corrections.zip') as initial:
    assert initial.testzip() is None and len(initial.namelist()) == 33
    initial_inventory = json.loads(initial.read('ARCHIVE.json'))
    for item in initial_inventory['files']:
        data = initial.read(item['file'])
        assert len(data) == item['bytes'] and digest(data) == item['sha256']
    assert initial.read('ROOT-FINAL-05.json') == initial_proof_bytes
for item in proof['originalEvidence']:
    data = (out/item['file']).read_bytes()
    assert data == Path(item['original']).read_bytes()
    assert len(data) == item['bytes'] and digest(data) == item['sha256']
result = json.loads((out/'09-RESULT.json').read_bytes())
assert result['passed'] and result['movementRelations'] == result['reloginAppearanceRelations'] == 8
assert [row['phase'] for row in result['normalLogouts'] if row['phase'].endswith('-relogin')] == ['warrior-relogin', 'female-taoist-relogin']
proof['schema'] = 'mir2.r22.actual-paired-rollout-root-06.v2'
proof['correctedAtUtc'] = datetime.datetime.now(datetime.timezone.utc).isoformat()
proof['initialRootProofSha256'] = digest(initial_proof_bytes)
proof['supersedesSummaryOnly'] = True
proof['ordinaryPublicRelationChecks'] = 44
proof['observerMovementRelations'] = 8
proof['observerReloginAppearanceRelations'] = 8
proof['actualAcknowledgedMovementAndSavedReloginStories'] = 2
proof['nativeLaunch'].pop('manualLogin')
proof['nativeLaunch'].update({'autoLogin': False, 'manualLoginRequired': True, 'nativeLoginObserved': False})
proof['reviewCorrections'] = [
    'Eight movement/relogin relation rows come from two actual actor stories with four observers each, not eight actor stories.',
    'The initial manualLogin field meant the launch required manual login; no actual native login is observed.',
    'Original receipts and the initial33-file archive are preserved; no release or smoke action was rerun.',
]
write_new(out/'ROOT-FINAL-06.json', (json.dumps(proof, indent=2)+'\n').encode('utf-8'))
review = '''Root transcript of second read-only review /root/health_zero_review_01.
This is not a reviewer-authored disk artifact. Findings precede the root correction.

字节与计数核对通过：33个文件，ARCHIVE列32个且只排除自身；26份原始复制、smoke ZIP的11个条目和promotion ZIP的4个条目，均与原文件及记录的长度/SHA一致。-text -diff 覆盖正确。

有两处需收窄措辞：
- README 和 CLASSIC 的8 actual/acknowledged walks、8 saved-position relogins实际是两个移动保存重登故事产生的8条观察者移动关系和8条重登外观关系。7份共有状态块的eight logout/relogin lifecycles也应区分8次正常logout与2个实际relogin故事。
- ROOT-FINAL 的manualLogin=true、共有状态块的with manual login容易被理解为已人工登录。建议写launched for manual login，并明确autoLogin=false/nativeLoginObserved=false。

其余边界清楚：40对象全字节验证属于stage，promotion不重验CMS；人工验收开放，b235未部署，原12小时期限已错过且未重置。未发现发布行为或原始证据的新阻断。
'''
write_new(out/'READONLY-DOC-REVIEW.txt', review.encode('utf-8'))
write_new(out/'finalize-r22-review-corrections-root-06.py', Path(__file__).read_bytes())
files = []
for path in sorted(out.iterdir()):
    assert path.is_file() and not path.is_symlink()
    data = path.read_bytes()
    files.append({'file': path.name, 'bytes': len(data), 'sha256': digest(data)})
assert len(files) == 37
write_new(out/'ARCHIVE-FINAL-06.json', (json.dumps({
    'schema': 'mir2.r22.actual-paired-rollout-final-original-byte-archive.v2',
    'files': files,
    'fileCountExcludingThisInventory': len(files),
    'currentRootProof': 'ROOT-FINAL-06.json',
    'initialArchivePreservedIn': 'initial-archive-before-review-corrections.zip',
    'initialRootAndInventorySupersededAsCurrentSummary': True,
    'originalFileCopies': 26,
    'ordinarySmokeZipEntries': 11,
    'promotionZipEntries': 4,
    'privateAccountAndCredentialsCopied': False,
    'humanWheelAccepted': False,
    'fullP1P7Complete': False,
}, indent=2)+'\n').encode('utf-8'))
print(json.dumps({'currentArchiveFiles': len(files)+1, 'originalCopies': 26,
                  'actualAcknowledgedMovementReloginStories': 2,
                  'observerRelationChecks': 44, 'nativeLoginObserved': False,
                  'finalInventorySha256': digest((out/'ARCHIVE-FINAL-06.json').read_bytes())}))
