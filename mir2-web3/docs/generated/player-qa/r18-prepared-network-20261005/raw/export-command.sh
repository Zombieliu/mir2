python3 - <<'PYREMOTE'
import pathlib,json,hashlib,tarfile,datetime,os,re,stat
root=pathlib.Path('/home/ubuntu/mir2-playtest-preflight-20260929/r18-dense-20261004-161010').resolve(strict=True);output=(root/'r18-execution-01').resolve(strict=True)
sha=lambda p:hashlib.file_digest(pathlib.Path(p).open('rb'),'sha256').hexdigest()
launch=json.loads((root/'EXECUTION-LAUNCH-01.json').read_text());assert launch['wrapperExitCode'] is not None and not pathlib.Path('/proc/'+str(launch['wrapperPid'])).exists(),'Single run has not fully closed'
report=json.loads((output/'wrapper-report.json').read_text());assert report['binarySha256']=='4064f6bce2337bbd1d251b37e7219163fd524828ddb83d90031232a0b1e25ca8'
assert report['sourceRevision']=='321316b791120a6fe549e4006623e63333a5543f' and report['frozenInputsSha256']=='b7ac4b4f33cc746248a6e6c0f2d9d97a5822fc4be5e953e5f1e08aad34dfc0c0'
for p in output.glob('*-stage.json'):
    data=json.loads(p.read_text());pid=data.get('ownedGatewayPid')
    assert not pid or not pathlib.Path('/proc/'+str(pid)).exists(),'An owned Gateway has not fully stopped; no public freeze/export yet'
assert sha(root/'FROZEN-INPUTS.json')==report['frozenInputsSha256']
frozen=json.loads((root/'FROZEN-INPUTS.json').read_text())
for row in frozen['files']:
    p=root/row['relativePath'];assert p.is_file() and not p.is_symlink() and p.stat().st_size==row['bytes'] and sha(p)==row['sha256']
recheck={'schema':'mir2.r18-after-single-run-frozen-input-check.v1','sourceRevision':report['sourceRevision'],'frozenManifestSha256':sha(root/'FROZEN-INPUTS.json'),'all49FrozenBytesUnchanged':True,'noPrivateStoreOrRawLogContentRead':True,'generatedUtc':datetime.datetime.now(datetime.timezone.utc).isoformat()}
(root/'FROZEN-RECHECK-FINAL-01.json').write_text(json.dumps(recheck,indent=2)+'\n')
phases=['bootstrap','purchases','travel','dense','death'];classes=['Warrior','Wizard','Taoist'];allow={'level-field-diff.json','travel-field-diff.json'}
for phase in phases:
    allow.update({phase+'-stage.json',phase+'.driver.stdout.log',phase+'-receipts/report.json'})
    for cls in classes:
        allow.add(phase+'-receipts/'+cls+'.jsonl')
        allow.add(phase+'-receipts/'+cls+'-'+phase+'-relogin.jsonl')
        if phase=='death':
            allow.add(phase+'-receipts/'+cls+'-fresh-load.jsonl');allow.add(phase+'-receipts/'+cls+'-alive-relogin.jsonl')
files=[];seen=set()
for row in report['publicReceiptFiles']:
    name=row['relativePath'];rel=pathlib.PurePosixPath(name)
    assert name in allow and name not in seen and not rel.is_absolute() and '..' not in rel.parts and not any(part.startswith('private-') for part in rel.parts)
    seen.add(name);p=output/rel;assert p.is_file() and not p.is_symlink() and p.resolve(strict=True).is_relative_to(output)
    assert p.stat().st_size==row['bytes'] and sha(p)==row['sha256']
    files.append((p,'public-receipts/'+name,sha(p)))
for p,name in [(output/'wrapper-report.json','wrapper-report.json'),(root/'PREFLIGHT-02.json','PREFLIGHT-02.json'),(root/'REMOTE-VERIFY-ONLY-01.json','REMOTE-VERIFY-ONLY-01.json'),(root/'FROZEN-RECHECK-FINAL-01.json','FROZEN-RECHECK-FINAL-01.json'),(root/'EXECUTION-LAUNCH-01.json','EXECUTION-LAUNCH-01.json'),(root/'wrapper-stdout.log','wrapper-stdout.log')]:
    assert p.is_file() and not p.is_symlink();files.append((p,name,sha(p)))
archive=root/'PUBLIC-RECEIPTS-01.tar.gz';assert not archive.exists() and not (root/'PUBLIC-EXPORT-01.json').exists();os.umask(0o077)
with tarfile.open(archive,'x:gz',compresslevel=6) as tar:
    for p,name,digest in files:tar.add(p,arcname=name,recursive=False)
proof={'schema':'mir2.r18-single-isolated-public-export.v1','generatedUtc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'sourceRevision':report['sourceRevision'],'binarySha256':report['binarySha256'],'frozenSha256':report['frozenInputsSha256'],'wrapperExitCode':launch['wrapperExitCode'],'archiveName':archive.name,'archiveSha256':sha(archive),'archiveBytes':archive.stat().st_size,'receiptCount':len(files),'uncompressedBytes':sum(p.stat().st_size for p,_,_ in files),'privateStoresKeysOrRawGatewayLogsIncluded':False,'explicitPublicAllowlistApplied':True,'allOwnedGatewaysClosedBeforeExport':True,'singleRunNoRetry':True,'files':[{'relativePath':name,'sha256':digest,'bytes':p.stat().st_size} for p,name,digest in files]}
(root/'PUBLIC-EXPORT-01.json').write_text(json.dumps(proof,indent=2)+'\n');print(json.dumps({k:v for k,v in proof.items() if k!='files'}))
PYREMOTE
