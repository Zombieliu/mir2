from pathlib import Path
import datetime, hashlib, json, os, shutil, subprocess, sys, time

BASE = Path(__file__).resolve().parent
ROOT = Path('C:/Users/Administrator/.codex/worktrees/crowded-combat-control/mir2-player-journey/mir2-web3/apps/game-client/windows-updater')
name, argv = sys.argv[1], sys.argv[2:]
assert name.replace('-', '').isalnum() and argv
assert shutil.disk_usage('C:/').free >= 50*1024**3, 'C: free-space guard'
prefix = BASE / 'commands' / name
prefix.parent.mkdir(exist_ok=True)
owned_temp = BASE / 'owned-temp'
owned_temp.mkdir(exist_ok=True)
env = dict(os.environ, TEMP=str(owned_temp), TMP=str(owned_temp))
start = time.monotonic()
started = datetime.datetime.now(datetime.timezone.utc).isoformat()
minimum_free = shutil.disk_usage('C:/').free
with prefix.with_suffix('.stdout.log').open('xb') as out, prefix.with_suffix('.stderr.log').open('xb') as err:
    child = subprocess.Popen(argv, cwd=ROOT, env=env, stdout=out, stderr=err)
    while child.poll() is None:
        minimum_free = min(minimum_free, shutil.disk_usage('C:/').free)
        if minimum_free < 50*1024**3 or time.monotonic()-start > 600:
            child.kill()
            child.wait()
            raise RuntimeError('owned command terminated by resource/time guard; raw logs retained')
        time.sleep(0.1)
    rc = child.wait()
receipt = {'argv': argv, 'cwd': str(ROOT), 'startedUtc': started, 'pid': child.pid,
           'exitCode': rc, 'elapsedSeconds': time.monotonic()-start, 'minimumCFreeBytes': minimum_free,
           'networkRequested': False, 'ownedTemp': str(owned_temp), 'processExited': True}
for stream in ['stdout', 'stderr']:
    p = prefix.with_suffix('.'+stream+'.log'); b = p.read_bytes()
    receipt[stream] = {'path': str(p), 'bytes': len(b), 'sha256': hashlib.sha256(b).hexdigest()}
with prefix.with_suffix('.command.json').open('x', encoding='utf-8', newline='\n') as f:
    json.dump(receipt, f, indent=2); f.write('\n')
print(json.dumps({'name': name, 'passed': rc == 0, 'elapsedSeconds': receipt['elapsedSeconds'], 'receipt': str(prefix.with_suffix('.command.json'))}))
sys.exit(rc)
