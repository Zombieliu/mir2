"""Retain only the original frozen importer inventory and independent review."""
import hashlib
import json
from pathlib import Path

out = Path(__file__).resolve().parent
worker = out.parent / 'r17-native-r2-importer-worker-01'
repo = Path('C:/Users/Administrator/.codex/worktrees/crowded-combat-control/mir2-player-journey/mir2-web3')
source_inventory = worker / 'ARTIFACT-INVENTORY.json'
assert hashlib.sha256(source_inventory.read_bytes()).hexdigest() == 'fa91df989647c2b5b01062a9c9059962f5ddac97757887542d0a51c6404ecb40'
manifest = json.loads(source_inventory.read_bytes())
target = repo / 'docs/generated/player-qa/native-delivery-20261005/r2-importer-01'
target.mkdir()
inventory = []
def retain(source, relative, expected=None):
    relative = Path(relative)
    assert not relative.is_absolute() and '..' not in relative.parts
    assert source.is_file() and not source.is_symlink()
    data = source.read_bytes()
    if expected is not None:
        assert hashlib.sha256(data).hexdigest() == expected
    dest = target / relative
    dest.parent.mkdir(parents=True, exist_ok=True)
    with dest.open('xb') as f:
        f.write(data)
    inventory.append({'path':relative.as_posix(),'size':len(data),'sha256':hashlib.sha256(data).hexdigest()})
for row in manifest['artifacts']:
    retain(worker / row['path'], Path('worker') / row['path'], row['sha256'])
retain(source_inventory, 'worker/ARTIFACT-INVENTORY.json')
for name in ['INTEGRATION-01.json','ROOT-REVIEW-01.json','review-tests.py','tests-01.stdout.log','tests-01.stderr.log','store-importer-proof.py']:
    retain(out / name, Path('root') / name)
retain(worker / 'integrate-root-four-files.py', 'root/integrate-root-four-files.py')
with (target / 'BYTE-INVENTORY.json').open('x') as f:
    json.dump({'files':inventory,'originalWorkerObjects':manifest['artifactCount'],'originalWorkerFreezeUnchanged':True,
               'rootIntegrationHelperAddedAfterWorkerFreeze':True,'deployed':False},f,indent=2)
    f.write('\n')
with (target / '.gitattributes').open('x',newline='\n') as f:
    f.write('* -text -diff\n**/* -text -diff\nREADME.md text eol=lf diff\n')
with (target / 'README.md').open('x',encoding='utf-8',newline='\n') as f:
    f.write('# Root-reviewed fixed-origin R17 R2 importer\n\n')
    f.write('Root integrates only the frozen four sources after checking original index SHA and every final source byte. The old unrestricted Web uploader now refuses the reserved native namespace; other legacy source text stays unchanged. The new authenticated native route admits only36 fixed source objects, strict bounded JSON, streamed FixedLengthStream/backend SHA and create-only R2 writes. Resumes hash the full existing stored body. Promotion requires the exact passed public stage receipt, complete immutable metadata/checksum checks, unchanged unexpired source feed pair and absent-pointer CAS; actual private readback does not replace subsequent public alias verification.\n\n')
    f.write('Meaningful old uploader RED1/3, intermediate104/106 failure and final107/107 worker checks are retained. Independent root107/107 passes overlap the worker cases. These use fake platform models, not actual Cloudflare R2, CDN or CMS revalidation. Actual root origin36 HTTPS proof is source-only; both existing opaque CI verification endpoints return401, and remote deployment is pending renewed authorization. Local workerd runtime checking and future actual stage/public/promote remain separate gates. The original41-object worker inventory and original freeze are unchanged; the root integration helper was added afterward and retained here under root. No service, private credential or user save is changed.\n')
print(json.dumps({'retainedFiles':len(inventory),'originalWorkerObjects':manifest['artifactCount'],'rootTests':107,'deployment':False}))
