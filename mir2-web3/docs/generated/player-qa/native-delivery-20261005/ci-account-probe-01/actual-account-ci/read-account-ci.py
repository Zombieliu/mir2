"""Read only safe metadata and the bounded normalized authority artifact."""
import json
import subprocess
from pathlib import Path

out = Path(__file__).resolve().parent / 'ci-run-37243580805'
out.mkdir()
cmd = ['gh', 'run', 'view', '37243580805', '--repo', 'Zombieliu/mir2', '--json', 'headSha,conclusion,jobs,url']
with (out / 'jobs.json').open('xb') as f:
    result = subprocess.run(cmd, stdout=f, stderr=subprocess.PIPE, timeout=90)
assert result.returncode == 0
jobs = json.loads((out / 'jobs.json').read_bytes())
assert jobs['headSha'] == '0250695126be552009d81a8e0b951ba85746a70d'
assert all(j['conclusion'] == 'skipped' for j in jobs['jobs'] if j['name'] != 'native-download-authority-probe')
cmd = ['gh', 'run', 'download', '37243580805', '--repo', 'Zombieliu/mir2', '--name', 'native-download-authority-37243580805', '--dir', str(out)]
with (out / 'download.log').open('xb') as f:
    result = subprocess.run(cmd, stdout=f, stderr=subprocess.STDOUT, timeout=90)
assert result.returncode == 0
receipt = json.loads((out / 'receipt.json').read_bytes())
print(json.dumps({'run': 37243580805, 'url': jobs['url'], 'conclusion': jobs['conclusion'], 'onlyNativeProbeRan': True, 'receipt': receipt}), flush=True)
