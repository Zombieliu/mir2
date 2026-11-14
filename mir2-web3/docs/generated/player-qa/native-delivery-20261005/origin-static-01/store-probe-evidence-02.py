"""Retain symlink topology as metadata, never copy fixture links into Git."""
from pathlib import Path

out = Path(__file__).resolve().parent
target = Path('C:/Users/Administrator/.codex/worktrees/crowded-combat-control/mir2-player-journey/mir2-web3/docs/generated/player-qa/native-delivery-20261005/ci-authority-probe-01')
assert target.is_dir() and not list(target.iterdir()), 'attempt01 must have made no file writes'
code = (out / 'store-probe-evidence.py').read_text()
code = code.replace('target.mkdir()', 'target.mkdir(exist_ok=True)')
code = code.replace("for row in manifest['evidenceFiles']]", "for row in manifest['evidenceFiles'] if 'sha256' in row]")
code = code.replace('The genuine close-error RED, earlier preparation boundary and raw receipts are retained.', 'The genuine close-error RED, earlier preparation boundary and raw receipts are retained. Six test symlinks remain metadata in the frozen worker receipt; their links are not copied into Git.')
exec(compile(code, str(out / 'store-probe-evidence-02.py'), 'exec'))
