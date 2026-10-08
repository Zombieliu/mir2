"""Bind preserved receipts before the root's bounded third native follow-up."""
import hashlib
import json
from pathlib import Path

BASE = Path('C:/mir2-build')
OLD_SOURCE = BASE / 'health-zero-authority-final-source-hashes.json'
OLD_VALIDATION = BASE / 'health-zero-authority-final-validation.json'
FOLLOWUP = BASE / 'health-zero-authority-final-validation-02.json'

def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()

assert digest(OLD_SOURCE) == '141ee65a1682908192001c935bbfb2611f066bbb8d14fea39f0c8332aa20b63f'
assert digest(OLD_VALIDATION) == 'e8216be32d61b48b23ca7155c3f0c7d9f74af73e568d4bbc465c5b94120d272b'
old = json.loads(OLD_VALIDATION.read_text(encoding='utf-8-sig'))
new = json.loads(FOLLOWUP.read_text(encoding='utf-8-sig'))
for item in old['passed_runs'] + old['known_baseline_failures'] + new['test_runs']:
    assert digest(item['log']) == item['sha256'], item['log']
for item in new['files']:
    assert digest(item['path']) == item['sha256'], item['path']
receipt = {
    'schema': 'mir2.health-zero.root-before-followup-03.v1',
    'baseline': new['baseline'], 'passed': True,
    'originalSourceReceiptSha256': digest(OLD_SOURCE),
    'originalValidationReceiptSha256': digest(OLD_VALIDATION),
    'followup02ReceiptSha256': digest(FOLLOWUP),
    'currentSourcesMatchFollowup02': True,
    'originalPassedLogsVerified': len(old['passed_runs']),
    'originalBaselineFailureLogsVerified': len(old['known_baseline_failures']),
    'followup02LogsVerified': len(new['test_runs']),
    'initialRootMismatchDiagnosticRetained': True,
    'newReviewFinding': 'An acknowledged revived/live owner marker must not override a later typed HP0/dead snapshot.',
    'publicMutation': False,
}
with (BASE / 'health-zero-authority-root-before-followup-03.json').open('x', encoding='utf-8') as out:
    json.dump(receipt, out, indent=2)
    out.write('\n')
print(json.dumps(receipt))
