"""Correct a mistyped inventory pin; first refusal occurred before any writes."""
from pathlib import Path
import hashlib
import json

out = Path(__file__).resolve().parent
source = out / 'store-importer-proof.py'
old = 'fa91df989647c2b5b01062a9c9059962f5ddac97757887542d0a51c6404ecb40'
new = 'fa91df5a463fc02896a1a855b991227fce5fda11d4c43b08cdf36a3e1bd8cb40'
inventory = out.parent / 'r17-native-r2-importer-worker-01/ARTIFACT-INVENTORY.json'
assert hashlib.sha256(inventory.read_bytes()).hexdigest() == new
assert not Path('C:/Users/Administrator/.codex/worktrees/crowded-combat-control/mir2-player-journey/mir2-web3/docs/generated/player-qa/native-delivery-20261005/r2-importer-01').exists()
receipt = {'schema':'mir2.importer-proof-pin-correction.v1','firstAttemptExitCode':1,
           'reason':'mistyped_inventory_expected_hash','firstAttemptWrites':False,
           'actualInventorySha256':new,'originalSourcesAndProofUnchanged':True,
           'firstScriptSha256':hashlib.sha256(source.read_bytes()).hexdigest()}
with (out / 'STORE-REFUSAL-01.json').open('x') as f:
    json.dump(receipt,f,indent=2)
    f.write('\n')
code = source.read_text()
assert code.count(old) == 1
code = code.replace(old,new)
exec(compile(code, str(source), 'exec'))
target = Path('C:/Users/Administrator/.codex/worktrees/crowded-combat-control/mir2-player-journey/mir2-web3/docs/generated/player-qa/native-delivery-20261005/r2-importer-01/root')
for name in ['store-importer-proof-02.py','STORE-REFUSAL-01.json']:
    with (target / name).open('xb') as f:
        f.write((out / name).read_bytes())
