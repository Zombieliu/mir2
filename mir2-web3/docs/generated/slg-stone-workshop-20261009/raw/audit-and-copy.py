from pathlib import Path
from datetime import datetime, timezone
import hashlib, json, re, shutil
base=Path(__file__).resolve().parent
repo=base.parent.parent
qa=repo/'mir2-web3/docs/generated/slg-stone-workshop-20261009'
runs=['stone-session-tests-03','stone-npc-tests-02','stone-config-regression-01',
 'stone-inventory-regression-01','stone-drop-regression-01','stone-trade-regression-01',
 'stone-save-regression-01','stone-game-data-regression-01','stone-rules-tests-03',
 'stone-native-tests-01','stone-portable-tests-01','stone-native-check-01']
rows=[];passing=set();ignored=set();total=0;ignored_total=0
for run in runs:
    folder=base/run;result=json.loads((folder/'result.json').read_text())
    assert result['exitCode']==0 and not result['sourceDrift'],run
    log=(folder/'cargo.log').read_text(encoding='utf-8-sig')
    assert hashlib.sha256((folder/'cargo.log').read_bytes()).hexdigest()==result['cargoLogSha256']
    summaries=[tuple(map(int,m)) for m in re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;',log)]
    assert all(failed==0 for _,failed,_ in summaries)
    names=re.findall(r'^test (\S+) \.\.\. ok\s*$',log,re.M)
    skipped=re.findall(r'^test (\S+) \.\.\. ignored.*$',log,re.M)
    passed=sum(p for p,_,_ in summaries);skip=sum(i for _,_,i in summaries)
    assert len(names)==passed and len(skipped)==skip,(run,passed,len(names),skip,len(skipped))
    assert summaries or result['kind']=='native-check'
    total+=passed;ignored_total+=skip;passing.update(names);ignored.update(skipped)
    snapshot=json.loads((folder/'source-snapshot.json').read_text())
    for source in snapshot['files']:
        data=(repo/source['path']).read_bytes()
        assert len(data)==source['length'] and hashlib.sha256(data).hexdigest()==source['sha256'],source['path']
    rows.append(dict(run=run,passed=passed,failed=0,ignored=skip,passingNames=names,
        ignoredNames=skipped,inputs=result['declaredInputs'],logSha256=result['cargoLogSha256']))
audit=dict(schema='mir2.stone-actual-audit.v1',createdUtc=datetime.now(timezone.utc).isoformat(),
    actualPassingExecutions=total,distinctPassingNames=len(passing),ignoredExecutions=ignored_total,
    distinctIgnoredNames=len(ignored),failedFinalTests=0,finalOriginalCargoCalls=len(runs),runs=rows,
    limitations=['Headless/File fixtures and compile only; conditional PG tests are not actual PG acceptance.',
        'Earlier failed/refused calls and earlier passing snapshots are history, excluded from final totals.',
        'No renderer, screenshots, live gateway, public rollout, installer or human acceptance.'])
(base/'actual-result-audit-01.json').write_text(json.dumps(audit,indent=2),encoding='utf-8')
assert not qa.exists(),'Preserve evidence bundles'
qa.mkdir(parents=True)
selected=[]
for path in base.iterdir():
    if path.is_file() and path.suffix in {'.json','.ps1','.py','.log'}:
        selected.append(path)
    elif path.is_dir() and (path/'result.json').is_file():
        selected.extend(p for p in path.rglob('*') if p.is_file())
selected.append(base/'source06-stones-tests-reconstructed.rs')
manifest=[]
for path in sorted(set(selected)):
    relative=path.relative_to(base);target=qa/'raw'/relative
    target.parent.mkdir(parents=True,exist_ok=True)
    data=path.read_bytes();target.write_bytes(data)
    assert target.read_bytes()==data
    manifest.append(dict(path=str(target.relative_to(qa)).replace('\\','/'),length=len(data),
        sha256=hashlib.sha256(data).hexdigest(),source=str(path)))
raw=dict(schema='mir2.stone-raw-evidence.v1',rawCopies=len(manifest),allCopiesByteVerified=True,
    originalGuardNonceDirectories=sum(1 for p in base.glob('*/cargo-receipts/*') if p.is_dir()),files=manifest)
(qa/'raw-evidence.json').write_text(json.dumps(raw,indent=2),encoding='utf-8')
print(json.dumps({k:v for k,v in audit.items() if k!='runs'},indent=2))
print(json.dumps(dict(rawCopies=len(manifest),originalGuardNonceDirectories=raw['originalGuardNonceDirectories'])))
