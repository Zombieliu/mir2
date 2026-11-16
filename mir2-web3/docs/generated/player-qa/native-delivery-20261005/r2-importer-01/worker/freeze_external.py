"""Freeze NEW four-file delivery/evidence; no repository/network/credentials writes."""
import difflib
import hashlib
import json
import re
import subprocess
from datetime import datetime, timezone
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = Path('C:/Users/Administrator/.codex/worktrees/crowded-combat-control/mir2-player-journey')
REV = 'd1f7dcec0213815b6f3bdcd33eff4d9d1c2774dc'
REL = 'mir2-web3/infra/cloudflare/mir2-r2-bulk-upload/'
OLD = Path('C:/mir2-playtest-releases/20261004-native-r18/r17-r2-publication-01')
ORIGIN = Path('C:/mir2-playtest-releases/20261004-native-r18/origin-acceleration-implementation-01/HTTPS-36-SOURCE-VERIFICATION-01.json')


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def canonical(value):
    return (json.dumps(value, sort_keys=True, separators=(',', ':'), ensure_ascii=True) + '\n').encode('ascii')


def write(name, raw):
    with (HERE / name).open('xb') as output:
        output.write(raw)
    return {'path': name, 'size': len(raw), 'sha256': sha(raw)}


def dump(name, value):
    return write(name, (json.dumps(value, ensure_ascii=True, indent=2) + '\n').encode('ascii'))


base = (HERE / 'evidence/base-index.ts').read_bytes()
original = subprocess.run(['git', '-C', str(ROOT), 'show', REV + ':' + REL + 'src/index.ts'],
                          stdout=subprocess.PIPE, stderr=subprocess.PIPE)
assert original.returncode == 0 and original.stdout == base
assert sha(base) == '353a1d7420659f4c12a3a7dc654351382ec8f7dd7a1c795237777e9372a05dfe'
current = (HERE / 'src/index.ts').read_bytes()
expected = base.decode('utf-8').replace('export interface Env {',
    'import { nativeR17Fetch } from "./native-r17-publication.mjs";\n\nexport interface Env {', 1)
expected = expected.replace('  async fetch(request: Request, env: Env): Promise<Response> {\n',
    '  async fetch(request: Request, env: Env): Promise<Response> {\n'
    '    if (new URL(request.url).pathname.startsWith("/upload/native-r17")) {\n'
    '      return nativeR17Fetch(request, env);\n'
    '    }\n', 1)
expected = expected.replace('      return json({ ok: false, error: "invalid_key" }, 400);\n    }\n',
    '      return json({ ok: false, error: "invalid_key" }, 400);\n    }\n'
    '    if (key.startsWith("mir2/native/windows-invited/")) {\n'
    '      return json({ ok: false, error: "reserved_native_prefix" }, 403);\n'
    '    }\n', 1)
assert expected.encode('utf-8') == current, 'non-narrow index diff'
index_diff = ''.join(difflib.unified_diff(base.decode('utf-8').splitlines(True),
    current.decode('utf-8').splitlines(True), fromfile='a/' + REL + 'src/index.ts',
    tofile='b/' + REL + 'src/index.ts'))
index_patch = 'diff --git a/' + REL + 'src/index.ts b/' + REL + 'src/index.ts\n' + index_diff
index_meta = write('INDEX-ONLY.patch', index_patch.encode('utf-8'))
patch = index_patch
files = []
for relative in ['src/index.ts', 'src/native-r17-publication.mjs',
                 'src/native-r17-publication-plan.json', 'test/native-r17-publication.test.mjs']:
    raw = (HERE / relative).read_bytes()
    files.append({'externalPath': relative, 'repositoryPath': REL + relative,
                  'size': len(raw), 'sha256': sha(raw)})
    if relative != 'src/index.ts':
        patch += 'diff --git a/' + REL + relative + ' b/' + REL + relative + '\nnew file mode 100644\n'
        patch += ''.join(difflib.unified_diff([], raw.decode('utf-8').splitlines(True),
                      fromfile='/dev/null', tofile='b/' + REL + relative))
patch_meta = write('APPROVED-WORKER.patch', patch.encode('utf-8'))
definition = json.loads((HERE / 'src/native-r17-publication-plan.json').read_text(encoding='utf-8'))
native_hash = sha(canonical(definition))
module = (HERE / 'src/native-r17-publication.mjs').read_text(encoding='utf-8')
assert native_hash == re.search(r"NATIVE_PLAN_SHA256 = '([0-9a-f]{64})'", module).group(1)
plan_raw = (OLD / 'PUBLICATION-PLAN.json').read_bytes()
root_raw = (OLD / 'ROOT-VERIFICATION.json').read_bytes()
assert plan_raw == (HERE / 'evidence/original-PUBLICATION-PLAN.json').read_bytes()
assert root_raw == (HERE / 'evidence/original-ROOT-VERIFICATION.json').read_bytes()
assert sha(plan_raw) == definition['sourcePlanSha256'] and sha(root_raw) == definition['rootVerificationSha256']
source_root = json.loads(root_raw)
closure = [{key: obj[key] for key in ('path', 'size', 'sha256')} for obj in definition['objects']]
assert len(closure) == 36 and sha(canonical(closure)) == source_root['objectsSha256']
origin_raw = ORIGIN.read_bytes()
origin = json.loads(origin_raw)
assert origin['passed'] is True and origin['objects'] == 36 and origin['verifiedBytes'] == 817657896
assert origin['originalPlanSha256'] == sha(plan_raw)
assert origin['originalRootAttestationSha256'] == sha(root_raw)
assert origin['canonicalObjectsSha256'] == source_root['objectsSha256']
assert origin['sourceFeedRecheckedAfterObjects'] is True and origin['loopbackTls'] is True
origin_meta = write('evidence/root-HTTPS-36-SOURCE-VERIFICATION-01.json', origin_raw)
attempts = []
for tag in ['red-01', 'green-attempt-01', 'green-focused-02', 'green-full-03', 'green-self-contained-04']:
    receipt = json.loads((HERE / 'evidence' / f'{tag}.json').read_text(encoding='utf-8'))
    stdout = (HERE / 'evidence' / f'{tag}.stdout.log').read_text(encoding='utf-8')
    counts = {name: int(re.search(r'^# ' + name + r' (\d+)$', stdout, re.M).group(1))
              for name in ['tests', 'pass', 'fail', 'cancelled', 'skipped']}
    attempts.append({'tag': tag, 'exitCode': receipt['exitCode'], **counts,
                     'receiptSha256': sha((HERE / 'evidence' / f'{tag}.json').read_bytes()),
                     'stdoutSha256': sha((HERE / 'evidence' / f'{tag}.stdout.log').read_bytes()),
                     'stderrSha256': sha((HERE / 'evidence' / f'{tag}.stderr.log').read_bytes())})
assert attempts[0]['pass'] == 1 and attempts[0]['fail'] == 2
assert attempts[1]['pass'] == 104 and attempts[1]['fail'] == 2
assert attempts[-1]['exitCode'] == 0 and attempts[-1]['pass'] == 107 and attempts[-1]['fail'] == 0
last = json.loads((HERE / 'evidence/green-self-contained-04.json').read_text(encoding='utf-8'))
assert {entry['path']: entry['sha256'] for entry in last['sourceAtAttempt']} == {
    entry['externalPath']: entry['sha256'] for entry in files}
receipt = dump('FROZEN-DELIVERY.json', {
    'schema': 'mir2.native-r17-importer-external-delivery.v1',
    'frozenUtc': datetime.now(timezone.utc).isoformat(),
    'baseRevision': REV, 'baseIndexSha256': sha(base),
    'approvedSourceFiles': files, 'indexPatch': index_meta, 'completePatch': patch_meta,
    'nativeDefinitionCanonicalSha256': native_hash,
    'originalPlanSha256': sha(plan_raw), 'originalRootAttestationSha256': sha(root_raw),
    'objectCount': 36, 'totalObjectBytes': sum(obj['size'] for obj in closure),
    'objectsSha256': sha(canonical(closure)),
    'rootFreshOriginProofCopy': origin_meta,
    'rootProofBoundary': 'loopback TLS full bytes plus source feed recheck, no Internet throughput/CDN/R2 proof',
    'actualNodeAttempts': attempts, 'latestOfflinePassCount': 107,
    'nativeR2Acceptance': False, 'cmsReverified': False, 'credentialValuesRead': False,
    'cloudflareAuthValidated': False, 'networkCallsByWriter': False,
    'repoModified': False, 'workflowModified': False, 'dependenciesModified': False,
    'deployment': False, 'gitMutation': False, 'priorProofModified': False,
    'scope': 'only NEW external four-source implementation and evidence',
    'remaining': ['root independent security/source integration', 'opaque CI account/token/bearer permission probe',
                  'actual Workers streamed checksum/conditional R2 import', 'all36 public full SHA stage',
                  'explicit promote and public alias pair verification',
                  'issued-library partial raw404 origin repair and laptop acceptance'],
})
inventory = []
for path in sorted(HERE.rglob('*')):
    if path.is_file() and path.name != 'ARTIFACT-INVENTORY.json':
        raw = path.read_bytes()
        inventory.append({'path': str(path.relative_to(HERE)).replace('\\', '/'),
                          'size': len(raw), 'sha256': sha(raw)})
inventory_meta = dump('ARTIFACT-INVENTORY.json', {
    'schema': 'mir2.native-r17-importer-evidence-inventory.v1',
    'selfExcluded': 'ARTIFACT-INVENTORY.json', 'artifactCount': len(inventory), 'artifacts': inventory,
})
print(json.dumps({'frozen': True, 'files': files, 'completePatch': patch_meta,
                  'indexPatch': index_meta, 'receipt': receipt,
                  'inventory': inventory_meta, 'latestOfflinePassCount': 107}, indent=2))
