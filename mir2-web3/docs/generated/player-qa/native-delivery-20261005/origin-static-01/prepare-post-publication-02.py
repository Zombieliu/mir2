"""Correct the owned proof basename; preserve failed read-only attempt01."""
from pathlib import Path

out = Path(__file__).resolve().parent
code = (out / 'prepare-post-publication.py').read_text()
assert code.count('HTTPS-FULL-VERIFICATION01.json') == 1
code = code.replace('HTTPS-FULL-VERIFICATION01.json', 'HTTPS-FULL-VERIFICATION-01.json')
code = code.replace('post-publication-01.', 'post-publication-02.')
exec(compile(code, str(out / 'prepare-post-publication-02.py'), 'exec'))
