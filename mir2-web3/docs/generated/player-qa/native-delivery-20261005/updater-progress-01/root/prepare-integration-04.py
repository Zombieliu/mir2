from pathlib import Path
import hashlib, json
base=Path(__file__).resolve().parent
body=(base/'integrate-03.py').read_bytes()
with (base/'MECHANICAL-REFUSAL-03.json').open('x',encoding='utf-8') as out:
    json.dump({'passed':False,'boundary':'before repository or selected-source writes',
               'assertion':'tracked root Cargo.lock raw equals LF Git blob',
               'cause':'clean text:auto root checkout uses CRLF',
               'originalHelperSha256':hashlib.sha256(body).hexdigest()},out,indent=2)
old="        assert before == git(ROOT, 'show', f'HEAD:{(REL / name).as_posix()}'), name"
new="""        blob = git(ROOT, 'show', f'HEAD:{(REL / name).as_posix()}')
        assert git(ROOT, 'diff', 'HEAD', '--', (REL / name).as_posix()) == b'', name
        assert before.replace(b'\\r\\n', b'\\n') == blob.replace(b'\\r\\n', b'\\n'), name"""
text=body.decode('utf-8'); assert text.count(old)==1
with (base/'integrate-04.py').open('x',encoding='utf-8',newline='\n') as out: out.write(text.replace(old,new))
