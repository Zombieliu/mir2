"""Retain every actual Node attempt; toy fixtures only, no network or secrets."""
import hashlib
import json
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path

sys.stdout.reconfigure(encoding='utf-8')

HERE = Path(__file__).resolve().parent
tag = sys.argv[1]
assert tag.isascii() and tag.replace('-', '').isalnum()
command = ['node', '--experimental-strip-types', '--test', '--test-reporter=tap',
           *sys.argv[2:], 'test/native-r17-publication.test.mjs']
result = subprocess.run(command, cwd=HERE, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
for suffix, data in [('stdout.log', result.stdout), ('stderr.log', result.stderr)]:
    with (HERE / 'evidence' / f'{tag}.{suffix}').open('xb') as output:
        output.write(data)
receipt = {'command': command, 'exitCode': result.returncode,
           'finishedUtc': datetime.now(timezone.utc).isoformat(),
           'nodeRuntimeModelOnly': True, 'liveR2Acceptance': False,
           'sourceAtAttempt': []}
for relative in ['src/index.ts', 'src/native-r17-publication.mjs',
                 'src/native-r17-publication-plan.json', 'test/native-r17-publication.test.mjs']:
    path = HERE / relative
    if path.exists():
        raw = path.read_bytes()
        receipt['sourceAtAttempt'].append({'path': relative, 'size': len(raw),
                                           'sha256': hashlib.sha256(raw).hexdigest()})
with (HERE / 'evidence' / f'{tag}.json').open('x', encoding='utf-8', newline='\n') as output:
    json.dump(receipt, output, indent=2)
    output.write('\n')
print(result.stdout.decode('utf-8', errors='replace'))
print(result.stderr.decode('utf-8', errors='replace'))
print(json.dumps({'attempt': tag, 'exitCode': result.returncode}))
sys.exit(result.returncode)
