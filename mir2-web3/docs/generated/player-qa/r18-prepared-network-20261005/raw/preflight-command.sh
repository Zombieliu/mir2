python3 - <<'PYREMOTE'
import hashlib,json,pathlib,socket,subprocess,sys,datetime,os,stat
parent=pathlib.Path('/home/ubuntu/mir2-playtest-preflight-20260929').resolve(strict=True)
staging=pathlib.Path('/home/ubuntu/mir2-playtest-preflight-20260929/r18-dense-20261004-161010')
assert staging.parent.resolve(strict=True)==parent and not staging.exists()
directory=parent/'capacity-verified-321316b791120a6fe549e4006623e63333a5543f'
assert directory.is_dir() and not directory.is_symlink()
actualEntries=[{'name':p.name,'file':p.is_file(),'symlink':p.is_symlink(),'bytes':p.stat().st_size if p.is_file() else None} for p in sorted(directory.iterdir())]
binary=directory/'mir2-gateway'
assert binary.is_file() and not binary.is_symlink() and os.access(binary,os.X_OK)
def sha(p):
    with pathlib.Path(p).open('rb') as h:return hashlib.file_digest(h,'sha256').hexdigest()
assert sha(binary)=='4064f6bce2337bbd1d251b37e7219163fd524828ddb83d90031232a0b1e25ca8' and binary.stat().st_size==79856480
with binary.open('rb') as h:assert h.read(4)==b'\x7fELF'
node=pathlib.Path('/opt/node/bin/node').resolve(strict=True)
assert sha(node)=='92181daccf61361e7c54d6404a3e2c2307a916d076492e3c0b388e6e5f86a854' and node.stat().st_size==123395824
version=subprocess.run([str(node),'--version'],capture_output=True,text=True,check=True,timeout=10).stdout.strip();assert version=='v22.21.1'
sockets=[]
for _ in range(2):
    s=socket.socket();s.bind(('127.0.0.1',0));sockets.append(s)
ports=[s.getsockname()[1] for s in sockets]
for s in sockets:s.close()
os.umask(0o077);staging.mkdir(mode=0o700)
assert stat.S_IMODE(staging.stat().st_mode)==0o700 and staging.stat().st_uid==os.getuid()
for name in ['evidence', 'maps', 'tests', 'vendor']:
    p=staging/pathlib.PurePosixPath(name);assert p.is_relative_to(staging);p.mkdir(mode=0o700,parents=True,exist_ok=True)
proof={'schema':'mir2.r18-exact-remote-preflight.v1','generatedUtc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'staging':str(staging),'sourceRevision':'321316b791120a6fe549e4006623e63333a5543f','actualBinaryDirectory':str(directory),'actualBinaryDirectoryEntries':actualEntries,'binaryPath':str(binary.resolve()),'binarySha256':sha(binary),'binaryBytes':binary.stat().st_size,'nodePath':str(node),'nodeVersion':version,'nodeSha256':sha(node),'nodeBytes':node.stat().st_size,'pythonVersion':sys.version.split()[0],'freshLoopbackBindProbes':ports,'portsStillReserved':False,'stagingOwnerUid':staging.stat().st_uid,'stagingMode':'0700','gatewayStarted':False,'liveServicesTouched':False,'privateStoreOrLogsRead':False}
(staging/'PREFLIGHT-01.json').write_text(json.dumps(proof,indent=2)+'\n',encoding='utf-8');print(json.dumps(proof))
PYREMOTE
