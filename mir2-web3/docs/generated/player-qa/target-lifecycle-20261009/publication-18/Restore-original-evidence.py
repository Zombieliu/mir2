import hashlib,json,zipfile
from pathlib import Path
root=Path(__file__).resolve().parent
record=json.loads((root/'EVIDENCE.json').read_bytes())['archive']
chunks=[]
for item in record['parts']:
    path=root/item['file']
    assert path.parent==root and path.is_file() and not path.is_symlink()
    data=path.read_bytes()
    assert len(data)==item['bytes'] and hashlib.sha256(data).hexdigest()==item['sha256']
    chunks.append(data)
whole=b''.join(chunks)
assert len(whole)==record['bytes'] and hashlib.sha256(whole).hexdigest()==record['sha256']
output=root/record['file']
assert output.parent==root and not output.exists(), 'Original already exists; nothing is overwritten.'
with output.open('xb') as stream:stream.write(whole)
with zipfile.ZipFile(output) as zipped:assert zipped.testzip() is None
print('Verified original evidence:',output.name,len(whole),record['sha256'])
