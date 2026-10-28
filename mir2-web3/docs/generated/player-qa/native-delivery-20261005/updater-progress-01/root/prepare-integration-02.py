from pathlib import Path
import hashlib, json

base = Path(__file__).resolve().parent
original = (base / 'integrate.py').read_bytes()
with (base / 'MECHANICAL-REFUSAL-01.json').open('x', encoding='utf-8') as out:
    json.dump({'passed': False, 'boundary': 'before repository or selected-source writes',
               'assertion': 'tracked working bytes equal Git blob',
               'path': 'scripts/archive-update-release.py',
               'cause': 'text:auto checkout CRLF compared against LF Git blob',
               'originalHelperSha256': hashlib.sha256(original).hexdigest()}, out, indent=2)
text = original.decode('utf-8')
old = "    assert body == git(ISSUED, 'show', f'{REV}:{name}'), name\n    issued_inputs.append({'path': name, 'bytes': len(body), 'sha256': sha(body)})"
new = """    blob = git(ISSUED, 'show', f'{REV}:{name}')
    relative = Path(name).relative_to(REL).as_posix()
    original_entry = baseline_entries[relative]
    assert len(body) == original_entry['bytes'] and sha(body) == original_entry['sha256'], name
    assert body.replace(b'\\r\\n', b'\\n') == blob.replace(b'\\r\\n', b'\\n'), name
    issued_inputs.append({'path': name, 'bytes': len(body), 'sha256': sha(body),
                          'gitBlobSha256': sha(blob), 'checkoutCRLFDifference': body != blob})"""
assert text.count(old) == 1
text = text.replace(old, new)
old2 = "issued_inputs = []\ntracked ="
new2 = "issued_inputs = []\nassert git(ISSUED, 'status', '--short', '--', REL.as_posix()) == b''\nbaseline_entries = {x['relative']: x for x in json.loads((WORKER/'BASELINE-01.json').read_bytes())['files']}\ntracked ="
assert text.count(old2) == 1
text = text.replace(old2, new2)
with (base / 'integrate-02.py').open('x', encoding='utf-8', newline='\n') as out: out.write(text)
