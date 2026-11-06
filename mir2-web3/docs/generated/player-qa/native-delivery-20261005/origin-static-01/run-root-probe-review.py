"""Offline root review, retaining fresh receipts and exact source bytes."""
import hashlib
import json
from pathlib import Path
import subprocess
import yaml

repo = Path('C:/Users/Administrator/.codex/worktrees/crowded-combat-control/mir2-player-journey')
out = Path('C:/mir2-playtest-releases/20261004-native-r18/native-ci-auth-probe-root-01')
out.mkdir()
script_dir = repo / 'mir2-web3/scripts'
pins = {
    'cloudflare_read_authority_probe.py': '1ce602ca8e1e1dc69c6745dec0ebce68237b040aa40ea11830b6aec9cdcf02dd',
    'test_cloudflare_read_authority_probe.py': 'c9f830c69b2f552e526d0f26c251027d7a443e2504423e90289a11807fc758d5',
}
for name, digest in pins.items():
    data = (script_dir / name).read_bytes()
    assert hashlib.sha256(data).hexdigest() == digest
    with (out / name).open('xb') as f:
        f.write(data)
workflow_path = '.github/workflows/web-assets-r2-release.yml'
baseline = subprocess.check_output(['git', '-c', 'gc.auto=0', '-C', str(repo), 'show', 'd1f7dcec0213815b6f3bdcd33eff4d9d1c2774dc:' + workflow_path])
actual = (repo / workflow_path).read_bytes()
before = yaml.load(baseline, Loader=yaml.BaseLoader)
after = yaml.load(actual, Loader=yaml.BaseLoader)
with (out / 'workflow-before.yml').open('xb') as f:
    f.write(baseline)
with (out / 'workflow-after.yml').open('xb') as f:
    f.write(actual)
native = after['jobs'].pop('native-download-authority-probe')
native_input = after['on']['workflow_dispatch']['inputs'].pop('native_delivery_operation')
expected_guard = "${{ github.event_name == 'workflow_dispatch' && (inputs.native_delivery_operation == '' || inputs.native_delivery_operation == 'off') }}"
assert after['jobs']['build-web-assets']['if'] == expected_guard
after['jobs']['build-web-assets']['if'] = before['jobs']['build-web-assets']['if']
assert before == after, 'legacy workflow structure changed beyond explicit native exclusion'
assert native_input['default'] == 'off' and native_input['options'] == ['off', 'probe']
assert native['if'] == "${{ github.event_name == 'workflow_dispatch' && inputs.native_delivery_operation == 'probe' }}"
assert native['steps'][0]['with']['persist-credentials'] == 'false'
assert set(native['steps'][1]['env']) == {'CLOUDFLARE_API_TOKEN', 'CLOUDFLARE_ACCOUNT_ID'}
assert 'cloudflare_read_authority_probe.py --receipt' in native['steps'][1]['run']
assert native['steps'][2]['with']['path'] == '${{ runner.temp }}/mir2-native-authority/receipt.json'
args = ['python', '-B', str(out / 'test_cloudflare_read_authority_probe.py')]
with (out / 'root-tests-01.log').open('xb') as log:
    proc = subprocess.run(args, cwd=out, stdout=log, stderr=subprocess.STDOUT)
receipts = list(out.glob('TEST-RECEIPT-*.json'))
assert proc.returncode == 0 and len(receipts) == 1
result = json.loads(receipts[0].read_bytes())
assert result['tests'] == 31 and result['passed'] and not result['failures'] and not result['errors'] and not result['skipped']
review = {'schema': 'mir2.native-ci-probe-root-review.v1', 'passed': True,
          'sourcePins': pins, 'rootTestReceipt': {'name': receipts[0].name, 'sha256': hashlib.sha256(receipts[0].read_bytes()).hexdigest()},
          'workflowBeforeSha256': hashlib.sha256(baseline).hexdigest(), 'workflowAfterSha256': hashlib.sha256(actual).hexdigest(),
          'legacyWorkflowStructureUnchangedExceptNativeExclusion': True,
          'selectedNativeProbeHasOnlyReadOnlyCloudflareCredentials': True,
          'realCredentialValueRead': False, 'actualGitHubDispatch': False,
          'actualAuthRequests': False, 'tests': 31, 'writeOrDeploymentAuthorityProven': False}
with (out / 'ROOT-REVIEW.json').open('xb') as f:
    f.write((json.dumps(review, indent=2) + '\n').encode())
print(json.dumps(review, separators=(',', ':')))
