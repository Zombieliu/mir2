import json
from pathlib import Path
import shutil
import subprocess

import verify_frozen_p3 as check

for row in check.manifest['files']:
    target = check.under(check.PROJECT, row['file'])
    if target.exists():
        before = check.under(check.OUT / 'before-source', row['file'])
        before.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(target, before)
    target.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(check.under(check.PHASE / 'frozen-source', row['file']), target)

for row in check.manifest['files']:
    data = check.under(check.PROJECT, row['file']).read_bytes()
    assert len(data) == row['bytes'] and check.digest(data) == row['sha256'], row['file']
result = subprocess.run(['git', '-c', 'gc.auto=0', '-C', str(check.PROJECT), 'diff', '--check'],
                        stdout=subprocess.PIPE, stderr=subprocess.PIPE)
(check.OUT / 'integrated-diff-check.log').write_bytes(result.stdout + result.stderr)
assert result.returncode == 0
receipt = {'schema': 'mir2.p3a.root-frozen-integration.v1', 'rootRevision': check.ROOT_HEAD,
           'sourceFilesVerified': len(check.manifest['files']), 'allExactFrozenBytes': True,
           'diffCheckPassed': True, 'sameTickLocalLifeReviewOpen': True,
           'productTestsRun': False, 'committed': False, 'published': False,
           'servicesChanged': False, 'playerSavesChanged': False}
(check.OUT / 'ROOT-INTEGRATION-01.json').write_text(json.dumps(receipt, indent=2) + '\n', encoding='utf-8')
print(json.dumps(receipt))
