sudo -n python3 -B - <<'PY'
import hashlib,os,pathlib,stat,sys
p=pathlib.Path('/srv/.mir2-classic-gateway-20261008-01/upgrade-classic-gateway-02.py')
assert os.geteuid()==0 and p.parent.resolve(strict=True)==p.parent
assert p.parent.stat().st_uid==0 and stat.S_IMODE(p.parent.stat().st_mode)==0o700
s=p.lstat();assert stat.S_ISREG(s.st_mode) and s.st_uid==0 and s.st_nlink==1 and stat.S_IMODE(s.st_mode)==0o600
fd=os.open(p,os.O_RDONLY|os.O_NOFOLLOW)
with os.fdopen(fd,'rb') as f:b=f.read(131073)
assert len(b)==30752 and hashlib.sha256(b).hexdigest()=='ce9523d7533bb0d7a8a0b8515142d86db4fbcabc79cd70b6c80217d868d79782'
sys.argv=[str(p)]+['--revision', 'a41dfe72d7e5858f11169adf88673dc44ed24200', '--expected-current', 'c6c32381a646dae1067dafdeec57691f606b1779', '--manifest-sha256', '28fb721846bff9982ef9f1cbd16b7b8114560ca002746af2a784d32f408bf13b', '--expected-active', '51', '--apply']
exec(compile(b,str(p),'exec'),{'__name__':'__main__','__file__':str(p)})
PY
