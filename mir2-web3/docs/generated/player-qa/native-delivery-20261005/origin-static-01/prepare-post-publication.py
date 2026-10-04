"""Read only our fixed public additions and private non-secret proof files."""
import json
from pathlib import Path
import subprocess

out = Path(__file__).resolve().parent
source = (out / 'remote-preflight.py').read_text().split('def main():', 1)[0]
expected = json.loads((out / 'EXPECTED-02.json').read_bytes())
tail = '''
stage = directory('/srv/.mir2-origin-r17-acceleration-20261005-01')
origin = directory('/srv/mir2-client-updates')
assert os.geteuid() == 0
sources = [file_record(origin / row['path'], row) for row in EXPECTED['sourceFiles']]
additions = [file_record(origin / row['path'], row) for row in EXPECTED['objects']]
installer = {'path': 'releases/bootstrap-WN-CANDIDATE-20261004-invited-17/Numeron-Legend-of-Rebirth-20261004-r17-Bootstrap.exe', 'size': 27482388, 'sha256': 'aced5e53a57c0e98081f8415c5ebfb95942be7b3cbb130490db24e5f8dc15621'}
installer_record = file_record(origin / installer['path'], installer)
proof_names = ['GENERATION-RESULT.json', 'generated/BUILD-DELIVERY.json', 'generated/INPUT.json', 'run-preflight-01/RESULT.json', 'run-apply-01/RESULT.json', 'HTTPS-FULL-VERIFICATION01.json', 'tools/TEST-RECEIPT-1791148251243295705.json']
proof_names.extend('SIGNED-BUNDLE-%02d.json' % i for i in range(11))
proofs = []
for name in proof_names:
    record = file_record(stage / name)
    require(record['size'] < 16 * 1024**2, 'proof exceeds bound')
    data = (stage / name).read_bytes()
    require(hashlib.sha256(data).hexdigest() == record['sha256'], 'proof changed reading')
    proofs.append({'relativePath': name, 'record': record, 'content': json.loads(data)})
tcp = {}
for port in (7100, 7200):
    result = subprocess.run(['ss', '-Htn', 'state', 'established', 'sport = :' + str(port)], stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=True, timeout=10)
    tcp[str(port)] = len(result.stdout.splitlines())
result = {'schema': 'mir2.origin.acceleration-post-publication.v1', 'passed': True, 'createdUnix': int(time.time()), 'remoteFilesystemWritten': False, 'sourceFiles': sources, 'addedFiles': additions, 'installerOriginMapping': installer_record, 'privateProofs': proofs, 'health': [health(7110), health(7210)], 'services': services(), 'tcpEstablishedCounts': tcp}
print(json.dumps(result, indent=2, sort_keys=True))
'''
command = out / 'post-publication-01.command'
with command.open('x', encoding='utf-8', newline='\n') as f:
    f.write("sudo -n python3 -B - <<'MIR2_READ_OWN_PROOFS_EOF'\n")
    f.write(source + '\nEXPECTED = ' + repr(expected) + '\n' + tail)
    f.write('\nMIR2_READ_OWN_PROOFS_EOF\n')
helper = Path('C:/mir2-ui-repair-20260921/playtest-release-20260928/remote.py')
args = ['python', '-B', str(helper), '--command-file', str(command), '--timeout', '240']
with (out / 'post-publication-01.json').open('xb') as stdout, (out / 'post-publication-01.stderr.log').open('xb') as stderr:
    proc = subprocess.run(args, stdout=stdout, stderr=stderr)
print(json.dumps({'exitCode': proc.returncode, 'receipt': str(out / 'post-publication-01.json')}))
raise SystemExit(proc.returncode)
