from pathlib import Path
import subprocess, sys
base=Path(__file__).resolve().parent
common=['C:/Users/Administrator/.cargo/bin/cargo.exe','+1.95.0']
flags=['--offline','--locked','--jobs','2','--target-dir','C:/mir2-build/native-updater-progress-root-20261005']
cases=[('transaction-root-01',['test',*flags,'--lib','transaction::tests','--','--nocapture']),
       ('bundle-negative-root-01',['test',*flags,'--lib','bundles_reject','--','--nocapture']),
       ('bundle-fallback-root-01',['test',*flags,'--lib','corrupt_bundle_falls_back_counts_discarded_bytes_and_reuses_verified_partial_entries','--','--nocapture']),
       ('delta-fallback-root-01',['test',*flags,'--lib','delta_base_mismatch_and_bad_patch_fall_back_without_touching_live_base','--','--nocapture']),
       ('entrypoints-root-01',['check',*flags,'--all-targets'])]
for name,args in cases:
    rc=subprocess.call([sys.executable,str(base/'run.py'),name,*common,*args])
    if rc: sys.exit(rc)
