"""Prepare only this NEW external implementation from immutable read-only inputs."""
import base64
import hashlib
import json
import subprocess
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = Path('C:/Users/Administrator/.codex/worktrees/crowded-combat-control/mir2-player-journey')
REV = 'd1f7dcec0213815b6f3bdcd33eff4d9d1c2774dc'
OLD = Path('C:/mir2-playtest-releases/20261004-native-r18/r17-r2-publication-01')
POST = Path('C:/mir2-playtest-releases/20261004-native-r18/origin-acceleration-implementation-01/post-publication-02.json')


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def canonical(value):
    return (json.dumps(value, sort_keys=True, separators=(',', ':'), ensure_ascii=True) + '\n').encode('ascii')


def write(name, raw):
    path = HERE / name
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open('xb') as output:
        output.write(raw)
    return {'path': name, 'size': len(raw), 'sha256': sha(raw)}


def dump(name, value):
    return write(name, (json.dumps(value, ensure_ascii=True, indent=2) + '\n').encode('ascii'))


source_path = 'mir2-web3/infra/cloudflare/mir2-r2-bulk-upload/src/index.ts'
result = subprocess.run(['git', '-C', str(ROOT), 'show', REV + ':' + source_path],
                        stdout=subprocess.PIPE, stderr=subprocess.PIPE)
assert result.returncode == 0
assert sha(result.stdout) == '353a1d7420659f4c12a3a7dc654351382ec8f7dd7a1c795237777e9372a05dfe'
base = write('src/index.ts', result.stdout)
write('evidence/base-index.ts', result.stdout)
plan_raw = (OLD / 'PUBLICATION-PLAN.json').read_bytes()
root_raw = (OLD / 'ROOT-VERIFICATION.json').read_bytes()
assert sha(plan_raw) == '04bdb9d5f891bba38752e4ba4029db2d638dc7f4674d494ce0c6b5ab354f3b87'
assert sha(root_raw) == '6b36134597180ad2b423c2015c05b3c48447fbd766e858c2fb0ce6f45e2565b9'
write('evidence/original-PUBLICATION-PLAN.json', plan_raw)
write('evidence/original-ROOT-VERIFICATION.json', root_raw)
original = json.loads(plan_raw)
root_receipt = json.loads(root_raw)
post_raw = POST.read_bytes()
post = json.loads(post_raw)
installer = post['installerOriginMapping']
assert post['passed'] is True
assert installer['nlink'] == 1 and installer['uid'] == 0
assert installer['size'] == 27482388
assert installer['sha256'] == 'aced5e53a57c0e98081f8415c5ebfb95942be7b3cbb130490db24e5f8dc15621'
assert installer['path'] == '/srv/mir2-client-updates/releases/bootstrap-WN-CANDIDATE-20261004-invited-17/Numeron-Legend-of-Rebirth-20261004-r17-Bootstrap.exe'
installer_proof = {'originalEvidencePath': str(POST).replace('\\', '/'),
                   'originalEvidenceSha256': sha(post_raw), 'selectedMapping': installer,
                   'noLiveCheckThisWorker': True}
dump('evidence/INSTALLER-ORIGIN-SOURCE.json', installer_proof)
runtime_installer_proof = {**installer_proof,
    'selectedMapping': {key: str(value) if isinstance(value, int) and abs(value) > 2**53 - 1 else value
                        for key, value in installer.items()}}
closure = [{key: obj[key] for key in ('path', 'size', 'sha256')}
           for obj in sorted(original['objects'], key=lambda obj: obj['path'])]
assert len(closure) == 36
assert sha(canonical(closure)) == root_receipt['objectsSha256']
origin_base = 'https://165.154.65.136.sslip.io/client-updates/'
objects = []
for index, obj in enumerate(closure):
    path = obj['path']
    if path.startswith('feeds/'):
        origin_relative = path.rsplit('/', 1)[1]
    elif path.startswith('installers/'):
        origin_relative = installer['path'].removeprefix('/srv/mir2-client-updates/')
    else:
        assert path.startswith('releases/')
        origin_relative = path
    content_type = ('application/json' if path.endswith('.json') else
                    'application/pkcs7-signature' if path.endswith('.p7s') else
                    'text/plain; charset=utf-8' if path.endswith(('.txt', '.md')) else
                    'application/octet-stream')
    objects.append({**obj, 'index': index,
                    'r2Key': 'mir2/native/windows-invited/' + path,
                    'originRelativePath': origin_relative, 'contentType': content_type,
                    'cacheControl': 'public, max-age=31536000, immutable'})
feed_object = next(obj for obj in original['objects'] if obj['path'].startswith('feeds/') and obj['path'].endswith('/latest.json'))
sig_object = next(obj for obj in original['objects'] if obj['path'].startswith('feeds/') and obj['path'].endswith('/latest.p7s'))
feed_raw = Path(feed_object['source']).read_bytes()
sig_raw = Path(sig_object['source']).read_bytes()
assert len(feed_raw) == feed_object['size'] and sha(feed_raw) == feed_object['sha256']
assert len(sig_raw) == sig_object['size'] and sha(sig_raw) == sig_object['sha256']
write('evidence/fixture-latest.json', feed_raw)
write('evidence/fixture-latest.p7s', sig_raw)
definition = {
    'schema': 'mir2.native.r17.fixed-origin-plan.v1',
    'accountId': '85bf64d86ea9221e172d26feba9fd47e', 'bucket': 'mir2-web3-assets',
    'r2Prefix': 'mir2/native/windows-invited/',
    'originBase': origin_base,
    'publicBase': 'https://assets.mir2.obelisk.build/client-updates/',
    'sourceRevision': '4c60c323aed11829abae6ee5ce6e13c675a3c1c3',
    'sourcePlanSha256': sha(plan_raw), 'sourcePlanBase64': base64.b64encode(plan_raw).decode('ascii'),
    'rootVerificationSha256': sha(root_raw), 'rootVerificationBase64': base64.b64encode(root_raw).decode('ascii'),
    'objectsSha256': sha(canonical(closure)),
    'candidate': original['candidate'], 'expectedCurrent': original['expectedCurrent'],
    'installerOriginEvidence': runtime_installer_proof,
    'objects': objects,
}
definition_meta = dump('src/native-r17-publication-plan.json', definition)
dump('evidence/PREPARE-EXTERNAL.json', {
    'schema': 'mir2.native-r17-external-prepare.v1',
    'baseRevision': REV, 'baseRepositoryPath': source_path, 'base': base,
    'originalPlanSha256': sha(plan_raw), 'originalRootReceiptSha256': sha(root_raw),
    'definition': definition_meta, 'definitionCanonicalSha256': sha(canonical(definition)),
    'objectCount': 36, 'objectsSha256': sha(canonical(closure)),
    'totalObjectBytes': sum(obj['size'] for obj in objects),
    'fixtureShaVerified': ['latest.json', 'latest.p7s'],
    'credentialValuesRead': False, 'sourceModified': False,
    'networkCalls': False, 'payloadAllShaRechecked': False,
})
print(json.dumps({'base': base, 'definition': definition_meta,
                  'definitionCanonicalSha256': sha(canonical(definition)),
                  'objectsSha256': sha(canonical(closure)), 'objectCount': 36}, indent=2))
