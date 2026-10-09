from pathlib import Path
import re,json,hashlib
root=Path(__file__).resolve().parents[2]
project=root/'mir2-web3'
before=Path(__file__).parent/'before-carriers'
before.mkdir(exist_ok=True)
rows=[]
for name in ['items.rs','equipment.rs','inventory.rs','stage5.rs','tests.rs']:
    p=project/'apps/simulation/src/runtime'/name
    old=p.read_bytes()
    dest=before/name
    if dest.exists(): raise RuntimeError('baseline exists '+name)
    dest.write_bytes(old)
    t=old.decode('utf-8')
    if name=='items.rs':
        marker='pub(super) struct ItemState {'
        assert t.count(marker)==1
        nl='\r\n' if '\r\n' in t else '\n'
        t=t.replace(marker,marker+nl+'    /// Server registry reference only; no hidden outcome or random seed.'+nl+'    #[serde(default, skip_serializing_if="Option::is_none")]'+nl+'    pub(super) stone_serial: Option<u64>,')
    t,n=re.subn(r'(?<![A-Za-z_])ItemState \{(\r?\n)([ \t]+)(key[:,])',lambda m:'ItemState {'+m[1]+m[2]+'stone_serial: None,'+m[1]+m[2]+m[3],t)
    assert n>0,(name,n)
    p.write_bytes(t.encode('utf-8'))
    rows.append({'path':str(p.relative_to(root)).replace('\\','/'),'constructors':n,'beforeSha256':hashlib.sha256(old).hexdigest(),'afterSha256':hashlib.sha256(p.read_bytes()).hexdigest()})
(Path(__file__).parent/'carrier-edit-receipt.json').write_text(json.dumps({'files':rows},indent=2),encoding='utf-8')
print(json.dumps(rows))
