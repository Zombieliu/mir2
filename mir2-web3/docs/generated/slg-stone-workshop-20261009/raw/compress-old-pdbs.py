"""Recover disk capacity without deleting/moving or changing file contents."""
from pathlib import Path
import ctypes, hashlib, json, subprocess, time
base=Path(__file__).resolve().parent
review=json.loads((base/'independent-c-pdb-candidates-01.json').read_text(encoding='utf-8-sig'))
roots=[Path('C:/mir2-cross-platform-storage-20261002').resolve(),Path('C:/mir2-build').resolve()]
rows=[]
def sha(p):
    h=hashlib.sha256()
    with p.open('rb') as f:
        for b in iter(lambda:f.read(4*1024*1024),b''): h.update(b)
    return h.hexdigest()
def size(p):
    high=ctypes.c_uint32();fn=ctypes.windll.kernel32.GetCompressedFileSizeW
    fn.argtypes=[ctypes.c_wchar_p,ctypes.POINTER(ctypes.c_uint32)];fn.restype=ctypes.c_uint32
    low=fn(str(p),ctypes.byref(high))
    return (high.value<<32)|low
for candidate in review['largestSix'][2:]:
    p=Path(candidate['path']);resolved=p.resolve(strict=True)
    assert resolved==p and p.suffix=='.pdb' and any(p.is_relative_to(r) for r in roots)
    assert p.stat().st_size==candidate['length'] and not p.is_symlink()
    assert p.stat().st_mtime<time.mktime(time.strptime('2026-10-09 00:00:00','%Y-%m-%d %H:%M:%S'))
    before=sha(p);allocated=size(p)
    # NTFS LZNT1 compression only: no WOF /EXE reparse points, no path changes.
    run=subprocess.run(['C:/Windows/System32/compact.exe','/C','/I','/Q',str(p)],capture_output=True)
    after=sha(p);row=dict(path=str(p),logicalBytes=p.stat().st_size,beforeSha256=before,
        afterSha256=after,beforeAllocatedBytes=allocated,afterAllocatedBytes=size(p),
        exitCode=run.returncode,stdout=run.stdout.decode('utf-8',errors='replace'),stderr=run.stderr.decode('utf-8',errors='replace'))
    rows.append(row);(base/'ntfs-compression-result-01.json').write_text(json.dumps(rows,indent=2),encoding='utf-8')
    assert before==after, 'File content changed during compression'
    print(json.dumps({k:row[k] for k in ('path','exitCode','beforeAllocatedBytes','afterAllocatedBytes') }),flush=True)
