from pathlib import Path
import hashlib, json
base=Path(__file__).resolve().parent
body=(base/'integrate-02.py').read_bytes()
with (base/'MECHANICAL-REFUSAL-02.json').open('x',encoding='utf-8') as out:
    json.dump({'passed':False,'boundary':'before repository or selected-source writes',
               'assertion':'issued tracked updater count == 41', 'actualTrackedCount':39,
               'explanation':'Worker 41 non-target files include two ignored Python bytecode files; root counts 39 Git tracked inputs',
               'originalHelperSha256':hashlib.sha256(body).hexdigest()},out,indent=2)
old='assert len(issued_inputs) == 41'
text=body.decode('utf-8'); assert text.count(old)==1
with (base/'integrate-03.py').open('x',encoding='utf-8',newline='\n') as out:
    out.write(text.replace(old,'assert len(issued_inputs) == 39'))
