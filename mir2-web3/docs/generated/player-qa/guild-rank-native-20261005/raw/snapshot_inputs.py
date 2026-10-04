from pathlib import Path
import hashlib
import json
import sys

OUT = Path(__file__).resolve().parent
run_name = sys.argv[1]
receipt = json.loads((OUT / (run_name + '-input.json')).read_text(encoding='utf-8'))
root = Path(receipt['root'])
target = OUT / (run_name + '-source/mir2-web3')
assert not target.exists()
for relative, expected in receipt['source_before_sha256'].items():
    data = (root / relative).read_bytes()
    assert hashlib.sha256(data).hexdigest() == expected, relative
    output = target / relative
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_bytes(data)
print(json.dumps({'run': run_name, 'exact_input_copies': len(receipt['source_before_sha256'])}))
