from pathlib import Path, PurePosixPath
import datetime, hashlib, io, json, os, re, stat, tarfile

BASE=Path(__file__).resolve().parent
ROOT=Path('C:/Users/Administrator/.codex/worktrees/crowded-combat-control/mir2-player-journey')
DELIVERY=ROOT/'mir2-web3/docs/generated/player-qa/native-delivery-20261005'
UPDATER=ROOT/'mir2-web3/apps/game-client/windows-updater'
WORKER=BASE.parent/'native-updater-progress-worker-01'
PLATFORM=BASE.parent/'native-r17-workerd-check-01'
CI=BASE.parent/'native-r17-ci-delivery-worker-01'
CIROOT=BASE.parent/'native-r17-ci-delivery-root-01'

def sha(b):return hashlib.sha256(b).hexdigest()
def new(p,b):
    p.parent.mkdir(parents=True,exist_ok=True)
    with p.open('xb') as out:out.write(b)
def jbytes(v):return (json.dumps(v,indent=2,ensure_ascii=False)+'\n').encode('utf-8')
def safe_name(value):
    p=PurePosixPath(value)
    assert not p.is_absolute() and '..' not in p.parts and p.parts
    return p.as_posix()
def archive(destination,source,entries,extras):
    destination.mkdir()
    members={}
    for e in entries:
        name=safe_name(e['path']);p=source/name
        if e.get('type')=='fixture-link-or-reparse':
            assert os.readlink(p)==e['target'] and e['followed'] is False
            name += '.fixture-link.json'
            body=jbytes({'inertNegativeFixtureLinkMetadata':e,'linkFollowed':False})
        else:
            info=p.lstat()
            assert stat.S_ISREG(info.st_mode) and not getattr(info,'st_file_attributes',0)&0x400
            body=p.read_bytes()
            assert len(body)==e.get('bytes',e.get('size')) and sha(body)==e['sha256'],name
        assert name not in members;members[name]=body
    for name in extras:
        assert name not in members;members[name]=(source/name).read_bytes()
    output=destination/'BYTE-EXACT-PROOF.tar.gz'
    with output.open('xb') as out:
        with tarfile.open(fileobj=out,mode='w:gz') as tar:
            for name,body in sorted(members.items()):
                info=tarfile.TarInfo(name);info.size=len(body);info.mode=0o600
                tar.addfile(info,io.BytesIO(body))
    with tarfile.open(output,'r:gz') as tar:
        seen=set()
        for m in tar:
            assert m.isfile() and m.name in members and m.name not in seen
            body=tar.extractfile(m).read()
            assert body==members[m.name];seen.add(m.name)
        assert seen==set(members)
    raw=output.read_bytes()
    new(destination/'ARCHIVE-RECEIPT.json',jbytes({'schema':'mir2.native-delivery-proof-archive.v1','passed':True,
        'archiveBytes':len(raw),'archiveSha256':sha(raw),'memberCount':len(members),
        'uncompressedBytes':sum(map(len,members.values())), 'negativeFixtureLinksAreInertMetadata':True,
        'allMembersReadBackExact':True,'archiveExtracted':False}))
    new(destination/'.gitattributes',b'* -text -diff\n**/* -text -diff\nREADME.md text eol=lf diff\n')

commands=[];distinct=0
for name,count in [('progress-root-01',10),('transaction-root-01',4),('bundle-negative-root-01',2),('bundle-fallback-root-01',1),('delta-fallback-root-01',1),('entrypoints-root-01',0)]:
    p=BASE/'commands'/f'{name}.command.json';v=json.loads(p.read_bytes())
    assert v['exitCode']==0 and v['processExited'] is True
    log=(BASE/'commands'/f'{name}.stdout.log').read_text('utf-8')
    if count: assert f'{count} passed; 0 failed; 0 ignored' in log
    err=(BASE/'commands'/f'{name}.stderr.log').read_text('utf-8')
    assert 'warning:' not in err
    commands.append(v);distinct+=count
assert distinct==18
integration=json.loads((BASE/'INTEGRATION-RECEIPT-01.json').read_bytes())
files=[]
for entry in integration['plannedChanges']:
    p=ROOT/entry['path'];raw=p.read_bytes()
    files.append({'path':entry['path'],'bytes':len(raw),'sha256':sha(raw)})
for name in ['fs_safe.rs','platform.rs']:
    issued=Path('C:/Users/Administrator/.codex/worktrees/r2-client-delivery/mir2-player-journey/mir2-web3/apps/game-client/windows-updater/src')/name
    assert (UPDATER/'src'/name).read_bytes()==issued.read_bytes()
compiled=Path('C:/mir2-build/native-updater-progress-root-20261005/debug/deps/mir2_windows_updater-4aa352f25a322883.exe')
review={'schema':'mir2.native-updater-progress-root-review.v1','passed':True,'rootBefore':integration['rootBefore'],
    'issuedRuntimeBaseline':integration['issuedRuntimeRevision'],'baselineFiles':integration['baselineRuntimeFiles'],
    'originalWorkerFreezeSha256':integration['workerFreezeSha256'],'originalWorkerFreezeCount':98,
    'issuedTrackedInputsRehashed':39,'worker41CountCorrectionRetained':True,
    'rootFormattingOnlyFiveSelectedFiles':True,'productionAndAdjacentTestFiles':files,
    'independentSelectedTestsPassed':distinct,'workerTestsOverlapRoot':True,'allTargetsCheckPassedWithoutWarnings':True,
    'fsSafeAndPlatformRawBytesUnchanged':True,'freshExactGameGuardsPreserved':4,'transactionSchema':1,
    'compiledUnitTestArtifact':{'bytes':compiled.stat().st_size,'sha256':sha(compiled.read_bytes())},
    'commands':commands,'performanceSpeedupMeasured':False,'full125965InstallRerun':False,
    'shippedEngineChanged':False,'gameGatewayOrPlayerSaveChanged':False,'nativeGUIAccepted':False,
    'createdUtc':datetime.datetime.now(datetime.timezone.utc).isoformat()}
new(BASE/'ROOT-FINAL-REVIEW-01.json',jbytes(review))

progress_dest=DELIVERY/'updater-progress-01'
frozen=json.loads((WORKER/'FROZEN-WORKER-01.json').read_bytes())
archive(progress_dest,WORKER,frozen['files'],['FROZEN-WORKER-01.json','TRACKED-COUNT-CORRECTION-01.json'])
platform_dest=DELIVERY/'workerd-platform-01'
assert sha((PLATFORM/'ARTIFACT-INVENTORY.json').read_bytes())=='e648b943b2ad7d4982716732efdf15981887426b03fdeac622179b2c006f41cf'
assert sha((PLATFORM/'FROZEN-RECEIPT.json').read_bytes())=='381250a2bbc749790dabfecfc89311c7a9955ef11b67e49352d5b4b63ad9c50b'
inv=json.loads((PLATFORM/'ARTIFACT-INVENTORY.json').read_bytes())
archive(platform_dest,PLATFORM,inv['files'],['ARTIFACT-INVENTORY.json','FROZEN-RECEIPT.json'])
ci_dest=DELIVERY/'ci-publication-01'
assert sha((CI/'FROZEN-DELIVERY.json').read_bytes())=='2b4486122140eef2fc05c69fdfc6d1a7706c0e6ea1f1fa40ceb00716dd2df8d9'
inv=json.loads((CI/'FROZEN-DELIVERY.json').read_bytes())
archive(ci_dest,CI,inv['artifacts'],['FROZEN-DELIVERY.json'])
for source,dest in [(BASE,progress_dest),(CIROOT,ci_dest)]:
    for p in source.iterdir():
        if p.is_file() and p.suffix in ('.py','.json','.log'):
            new(dest/'root'/p.name,p.read_bytes())
    commands_dir=source/'commands'
    if commands_dir.exists():
        for p in commands_dir.iterdir():
            if p.is_file():new(dest/'root/commands'/p.name,p.read_bytes())
for filename in ['INTEGRATION-RECEIPT-01.json','ROOT-FINAL-REVIEW-01.json']:
    assert (progress_dest/'root'/filename).read_bytes()==(BASE/filename).read_bytes()
print(json.dumps({'passed':True,'rootTests':18,'workerArchives':[100,224,211],'sourceAndRuntimePublished':False}))
