python3 -u - <<'PYREMOTE'
import hashlib,json,pathlib,subprocess,sys,datetime,os,stat
root=pathlib.Path('/home/ubuntu/mir2-playtest-preflight-20260929/r18-dense-20261004-161010').resolve(strict=True);sha=lambda p:hashlib.file_digest(pathlib.Path(p).open('rb'),'sha256').hexdigest()
assert sha(root/'FROZEN-INPUTS.json')=='b7ac4b4f33cc746248a6e6c0f2d9d97a5822fc4be5e953e5f1e08aad34dfc0c0'
assert (root/'REMOTE-VERIFY-ONLY-01.json').is_file()
verified=json.loads((root/'REMOTE-VERIFY-ONLY-01.json').read_text());assert verified['wrapperVerifyOnlyExitCode']==0 and verified['fileCount']==49
manifest=json.loads((root/'FROZEN-INPUTS.json').read_text())
for row in manifest['files']:
    p=(root/row['relativePath']).resolve(strict=True);assert p.is_relative_to(root) and p.stat().st_size==row['bytes'] and sha(p)==row['sha256']
binary=root/'bin/mir2-gateway';node=pathlib.Path('/opt/node/bin/node')
assert binary.is_file() and not binary.is_symlink() and binary.stat().st_uid==os.getuid() and binary.stat().st_nlink==1 and os.access(binary,os.X_OK)
assert sha(binary)=='4064f6bce2337bbd1d251b37e7219163fd524828ddb83d90031232a0b1e25ca8' and binary.stat().st_size==79856480
assert sha(node)=='92181daccf61361e7c54d6404a3e2c2307a916d076492e3c0b388e6e5f86a854'
os.umask(0o077)
command=[sys.executable,'-u',str(root/'run_isolated_dense.py'),'--binary',str(binary),'--node',str(node),'--output',str(root/'r18-execution-01'),'--travel-mode','prepared','--formation-seconds','30','--trials','1','--include-death','--death-seconds','180']
assert not (root/'r18-execution-01').exists() and not (root/'EXECUTION-LAUNCH-01.json').exists()
with (root/'wrapper-stdout.log').open('xb') as log:
    child=subprocess.Popen(command,cwd=root,stdin=subprocess.DEVNULL,stdout=log,stderr=subprocess.STDOUT)
    receipt={'schema':'mir2.r18-authorized-single-isolated-execution.v1','startedUtc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'wrapperPid':child.pid,'command':command,'sourceRevision':manifest['sourceRevision'],'frozenSha256':sha(root/'FROZEN-INPUTS.json'),'binarySha256':sha(binary),'nodeSha256':sha(node),'verifiedInputCount':49,'liveServicesTouched':False,'wrapperExitCode':None,'singleAttemptNoRetry':True}
    (root/'EXECUTION-LAUNCH-01.json').write_text(json.dumps(receipt,indent=2)+'\n')
    print(json.dumps({'started':True,'wrapperPid':child.pid,'root':str(root),'verifiedInputCount':49}),flush=True)
    code=child.wait()
receipt['wrapperExitCode']=code;receipt['completedUtc']=datetime.datetime.now(datetime.timezone.utc).isoformat()
(root/'EXECUTION-LAUNCH-01.json').write_text(json.dumps(receipt,indent=2)+'\n')
print(json.dumps({'completed':True,'wrapperExitCode':code,'stdoutSha256':sha(root/'wrapper-stdout.log')}),flush=True)
sys.exit(code)
PYREMOTE
