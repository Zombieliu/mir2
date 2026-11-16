"""Verify fixed literal36 source mappings; preserve original signed input bytes."""
import hashlib
import json
from pathlib import Path
import subprocess

out = Path(__file__).resolve().parent
base = out.parent / 'r17-r2-publication-01'
plan_data = (base / 'PUBLICATION-PLAN.json').read_bytes()
assert hashlib.sha256(plan_data).hexdigest() == '04bdb9d5f891bba38752e4ba4029db2d638dc7f4674d494ce0c6b5ab354f3b87'
proof_data = (base / 'ROOT-VERIFICATION.json').read_bytes()
assert hashlib.sha256(proof_data).hexdigest() == '6b36134597180ad2b423c2015c05b3c48447fbd766e858c2fb0ce6f45e2565b9'
plan = json.loads(plan_data)
records = []
for row in plan['objects']:
    path = row['path']
    if path.startswith('releases/'):
        origin_path = path
    elif path.startswith('feeds/s12-7efdddf34fbf58ffa0bf8c4f7fcb1a9ad2bac869cb3f003cea0a0c9c53356c88/'):
        origin_path = path.rsplit('/', 1)[1]
        assert origin_path in ('latest.json', 'latest.p7s')
    else:
        assert path == 'installers/aced5e53a57c0e98081f8415c5ebfb95942be7b3cbb130490db24e5f8dc15621/Mir2Setup.exe'
        origin_path = 'releases/bootstrap-WN-CANDIDATE-20261004-invited-17/Numeron-Legend-of-Rebirth-20261004-r17-Bootstrap.exe'
    records.append({'path': origin_path, 'r2Path': path, 'size': row['size'], 'sha256': row['sha256']})
assert len(records) == 36 and sum(r['size'] for r in records) == 817657896
source = (out / 'remote-verify-serving.py').read_text().split('expected=json.loads', 1)[0]
tail = '''
values = []
for row in RECORDS:
    values.append({**read(row, row['path'] in ('latest.json', 'latest.p7s')), 'r2Path': row['r2Path']})
for row in RECORDS:
    if row['path'] in ('latest.json', 'latest.p7s'):
        read(row, True)
result = {'schema': 'mir2.native-r17-fixed-origin-full-closure.v1', 'passed': True,
          'objects': 36, 'verifiedBytes': sum(r['size'] for r in RECORDS), 'publicHostCertificateVerified': True,
          'loopbackTls': True, 'internetThroughputMeasurement': False, 'sourceFeedRecheckedAfterObjects': True,
          'originalPlanSha256': '04bdb9d5f891bba38752e4ba4029db2d638dc7f4674d494ce0c6b5ab354f3b87',
          'originalRootAttestationSha256': '6b36134597180ad2b423c2015c05b3c48447fbd766e858c2fb0ce6f45e2565b9',
          'canonicalObjectsSha256': 'f38a692a4ba200177f4358c9f3d1566f97fadb32cf408c072cb1f8b8bb03d4f0',
          'files': values, 'createdUnix': int(time.time()), 'servicesChanged': False, 'CDNPublished': False}
data = (json.dumps(result, indent=2, sort_keys=True) + '\\n').encode()
with (STAGE / 'HTTPS-36-SOURCE-VERIFICATION-01.json').open('xb') as stream:
    stream.write(data)
print(data.decode())
'''
command = out / 'verify-origin-closure-01.command'
with command.open('x', encoding='utf-8', newline='\n') as f:
    f.write("sudo -n nice -n 10 python3 -B - <<'MIR2_VERIFY_FIXED_ORIGIN_CLOSURE_EOF'\n" + source + '\nRECORDS = ' + repr(records) + '\n' + tail + '\nMIR2_VERIFY_FIXED_ORIGIN_CLOSURE_EOF\n')
args = ['python', '-B', 'C:/mir2-ui-repair-20260921/playtest-release-20260928/remote.py', '--command-file', str(command), '--timeout', '240']
with (out / 'HTTPS-36-SOURCE-VERIFICATION-01.json').open('xb') as stdout, (out / 'verify-origin-closure-01.stderr.log').open('xb') as stderr:
    result = subprocess.run(args, stdout=stdout, stderr=stderr)
print(json.dumps({'exitCode': result.returncode, 'receipt': str(out / 'HTTPS-36-SOURCE-VERIFICATION-01.json')}))
raise SystemExit(result.returncode)
