"""Verify repaired public URLs using complete response bytes and the signed package pins."""
from concurrent.futures import ThreadPoolExecutor
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import urllib.request

BASE = Path(__file__).parent
environment = json.loads((BASE / 'environment-exploration.json').read_text(encoding='utf-8-sig'))
applied = json.loads((BASE / 'distribution-apply.stdout.json').read_text(encoding='utf-8-sig'))
assert applied['apply'] and applied['protectedBytesUnchanged'] and applied['serviceProcessesUnchanged']
assert len(applied['assets']) == 4 and all(x['action'] == 'appended_verified' for x in applied['assets'])
assert not (BASE / 'distribution-apply.stderr.txt').read_text(encoding='utf-8-sig').strip()
prefix = 'https://165.154.65.136.sslip.io/client-updates/'
release = prefix + 'releases/game-WN-CANDIDATE-20261009-invited-23/'
jobs = [(x['publicUrl'], x['length'], x['sha256'])
        for x in environment['updateTarget']['missingLocalResources']]
jobs += [(release + 'VERSION.json', 1221, 'd8c2e7d380c4b37e90695c825e89e1c75ecf6738ce04e67780b4439a002957ee'),
         (release + 'BUILD-ATTESTATION.json', 2416, '62254955d4376366dfdbe2976c93bcaf4a688cca8e6b283ac1ed61b5ab85ff90'),
         (release + 'README-START.txt', 49328, '5bc26223994015306280ee17952cafe2d30d61b1649f8a0ff7f1de3cc40b8bbd'),
         (prefix + 'latest.json', 1159, '2d87c59edb2aff2b55237dad1fcc418eac6ba03b6514720982ac16c1215f20d2'),
         (prefix + 'latest.p7s', None, '1f94346017807b0d6a2c24a3f4cdd3226461cc9618ed6dc0daa43460e7c0f8f4')]

def verify(job):
    url, size, expected = job
    with urllib.request.urlopen(url, timeout=20) as response:
        limit = size if size is not None else 1000000
        payload = response.read(limit + 1)
        actual = hashlib.sha256(payload).hexdigest()
        result = {'url': url, 'utc': datetime.now(timezone.utc).isoformat(),
                  'httpStatus': response.status, 'receivedBytes': len(payload),
                  'expectedBytes': size, 'sha256': actual, 'expectedSha256': expected,
                  'cacheControl': response.headers.get('Cache-Control'),
                  'server': response.headers.get('Server'),
                  'cfCacheStatus': response.headers.get('CF-Cache-Status')}
    result['passed'] = response.status == 200 and (size is None or len(payload) == size) and actual == expected
    return result

with ThreadPoolExecutor(max_workers=4) as pool:
    results = list(pool.map(verify, jobs))
report = {'schema': 'mir2.r23-missing-assets.public-verification.v1',
          'utc': datetime.now(timezone.utc).isoformat(), 'results': results,
          'allPassed': all(x['passed'] for x in results),
          'protectedBytesUnchanged': applied['protectedBytesUnchanged'],
          'serviceProcessesUnchanged': applied['serviceProcessesUnchanged'],
          'clientUpdateRetried': False, 'nativeGameplayAccepted': False}
with (BASE / 'distribution-public-verification.json').open('x', encoding='utf-8') as f:
    json.dump(report, f, indent=2)
print(json.dumps(report, indent=2))
if not report['allPassed']:
    raise SystemExit(1)
