from pathlib import Path,PurePosixPath
import hashlib,json,tarfile
root=Path(__file__).resolve().parent;sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
proof=json.loads((root/'PUBLIC-EXPORT-01.json').read_text(encoding='utf-8'));archive=root/proof['archiveName']
assert proof['privateStoresKeysOrRawGatewayLogsIncluded'] is False and proof['explicitPublicAllowlistApplied'] is True
assert archive.stat().st_size==proof['archiveBytes'] and sha(archive)==proof['archiveSha256']
assert proof['sourceRevision']=='321316b791120a6fe549e4006623e63333a5543f' and proof['binarySha256']=='4064f6bce2337bbd1d251b37e7219163fd524828ddb83d90031232a0b1e25ca8'
assert proof['frozenSha256']=='b7ac4b4f33cc746248a6e6c0f2d9d97a5822fc4be5e953e5f1e08aad34dfc0c0'
output=root/'downloaded-public';assert not output.exists(),'Public extraction is write-once'
declared={r['relativePath']:r for r in proof['files']};assert len(declared)==proof['receiptCount']
assert proof['uncompressedBytes']==sum(r['bytes'] for r in declared.values()) and proof['uncompressedBytes']<500*1024*1024
with tarfile.open(archive,'r:gz') as tar:
    members=tar.getmembers();assert len(members)==len(declared)==len({m.name for m in members})
    for member in members:
        rel=PurePosixPath(member.name);assert member.isfile() and not member.issym() and not member.islnk() and not rel.is_absolute() and '..' not in rel.parts and not any(p.startswith('private-') for p in rel.parts)
        row=declared[member.name];assert member.size==row['bytes'];data=tar.extractfile(member).read()
        assert len(data)==row['bytes'] and hashlib.sha256(data).hexdigest()==row['sha256']
        p=output.joinpath(*rel.parts);p.parent.mkdir(parents=True,exist_ok=True);p.write_bytes(data)
report=json.loads((output/'wrapper-report.json').read_text(encoding='utf-8'))
assert report['binarySha256']==proof['binarySha256'] and report['frozenInputsSha256']==proof['frozenSha256']
for row in report['publicReceiptFiles']:
    p=declared['public-receipts/'+row['relativePath']];assert (p['sha256'],p['bytes'])==(row['sha256'],row['bytes'])
receipt={'schema':'mir2.r18-downloaded-explicit-public-receipts.v1','archiveSha256':proof['archiveSha256'],'verifiedFiles':len(members),'uncompressedBytes':proof['uncompressedBytes'],'sourceRevision':proof['sourceRevision'],'binarySha256':proof['binarySha256'],'frozenSha256':proof['frozenSha256'],'privateStoresKeysOrRawGatewayLogsIncluded':False,'explicitPublicAllowlistApplied':True,'networkRunWasAcceptance':False}
(root/'DOWNLOADED-RECEIPTS-01.json').write_text(json.dumps(receipt,indent=2)+'\n',encoding='utf-8');print(json.dumps(receipt))
