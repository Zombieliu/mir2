python3 - <<'PYREMOTE'
import hashlib,json,pathlib,subprocess,sys,datetime,os,stat
root=pathlib.Path('/home/ubuntu/mir2-playtest-preflight-20260929/r18-dense-20261004-161010').resolve(strict=True)
sha=lambda p:hashlib.file_digest(pathlib.Path(p).open('rb'),'sha256').hexdigest()
assert stat.S_IMODE(root.stat().st_mode)==0o700 and root.stat().st_uid==os.getuid()
assert sha(root/'FROZEN-INPUTS.json')=='b7ac4b4f33cc746248a6e6c0f2d9d97a5822fc4be5e953e5f1e08aad34dfc0c0'
manifest=json.loads((root/'FROZEN-INPUTS.json').read_text());assert len(manifest['files'])==49
assert manifest['sourceRevision']=='321316b791120a6fe549e4006623e63333a5543f'
for row in manifest['files']:
    rel=pathlib.PurePosixPath(row['relativePath']);assert not rel.is_absolute() and '..' not in rel.parts
    p=root/rel;assert p.is_file() and not p.is_symlink() and p.resolve(strict=True).is_relative_to(root)
    assert p.stat().st_size==row['bytes'] and sha(p)==row['sha256'],row['relativePath']
binary=root/'bin/mir2-gateway';node=pathlib.Path('/opt/node/bin/node')
assert sha(binary)=='4064f6bce2337bbd1d251b37e7219163fd524828ddb83d90031232a0b1e25ca8' and binary.stat().st_size==79856480 and os.access(binary,os.X_OK)
assert sha(node)=='92181daccf61361e7c54d6404a3e2c2307a916d076492e3c0b388e6e5f86a854' and node.stat().st_size==123395824
result=subprocess.run([sys.executable,str(root/'run_isolated_dense.py'),'--verify-only'],cwd=root,capture_output=True,text=True,check=False,timeout=30)
assert result.returncode==0,result.stderr;data=json.loads(result.stdout);assert data['frozenInputsVerified'] is True and data['fileCount']==49
proof={'schema':'mir2.r18-remote-frozen-verify-only.v1','generatedUtc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'root':str(root),'sourceRevision':manifest['sourceRevision'],'frozenSha256':sha(root/'FROZEN-INPUTS.json'),'fileCount':49,'allFrozenFilesRehashed':True,'wrapperVerifyOnlyExitCode':result.returncode,'wrapperStdout':result.stdout,'wrapperStderr':result.stderr,'binaryPath':str(binary),'binarySha256':sha(binary),'nodePath':str(node),'nodeSha256':sha(node),'gatewayStarted':False,'networkDriverRun':False,'privateStoresOrLogsRead':False,'liveServicesTouched':False}
(root/'REMOTE-VERIFY-ONLY-01.json').write_text(json.dumps(proof,indent=2)+'\n');print(json.dumps(proof))
PYREMOTE
