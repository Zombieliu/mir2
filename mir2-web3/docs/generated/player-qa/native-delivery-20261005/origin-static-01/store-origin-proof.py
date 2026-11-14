"""Archive byte-exact non-secret publication proof, retaining fake-link metadata."""
import hashlib
import io
import json
from pathlib import Path
import tarfile

out = Path(__file__).resolve().parent
worker = out.parent / 'origin-acceleration-publisher-worker-01'
frozen = worker / 'FROZEN-RECEIPT.json'
assert hashlib.sha256(frozen.read_bytes()).hexdigest() == 'adf5156678cf98831c6562d867e12f1431e8bdc2bf888fe872eba1a4593f8ba8'
manifest = json.loads(frozen.read_bytes())
archive_path = out / 'PUBLISHER-EVIDENCE-01.tar.gz'
topology = [row for row in manifest['evidenceFiles'] if 'sha256' not in row]
entries = [row for row in manifest['evidenceFiles'] if 'sha256' in row]
entries.append({'path': 'FROZEN-RECEIPT.json', 'sha256': hashlib.sha256(frozen.read_bytes()).hexdigest()})
with archive_path.open('xb') as raw, tarfile.open(fileobj=raw, mode='w:gz', compresslevel=6) as archive:
    for row in entries:
        path = Path(row['path'])
        assert not path.is_absolute() and '..' not in path.parts
        source = worker / path
        assert not source.is_symlink()
        data = source.read_bytes()
        assert hashlib.sha256(data).hexdigest() == row['sha256']
        info = tarfile.TarInfo(path.as_posix())
        info.size, info.mode, info.mtime = len(data), 0o600, 0
        archive.addfile(info, io.BytesIO(data))
    data = (json.dumps({'topologyMetadataOnly': topology}, indent=2) + '\n').encode()
    info = tarfile.TarInfo('FIXTURE-LINK-TOPOLOGY.json')
    info.size, info.mode, info.mtime = len(data), 0o600, 0
    archive.addfile(info, io.BytesIO(data))
with tarfile.open(archive_path, 'r:gz') as archive:
    actual = {member.name: hashlib.sha256(archive.extractfile(member).read()).hexdigest() for member in archive.getmembers()}
for row in entries:
    assert actual[row['path']] == row['sha256']
root = Path('C:/Users/Administrator/.codex/worktrees/crowded-combat-control/mir2-player-journey/mir2-web3')
target = root / 'docs/generated/player-qa/native-delivery-20261005/origin-static-01'
target.mkdir()
inventory = []
selected = [p for p in out.iterdir() if p.is_file() and not p.name.startswith('github-secret-names')]
for source in sorted(selected):
    data = source.read_bytes()
    with (target / source.name).open('xb') as f:
        f.write(data)
    inventory.append({'path': source.name, 'size': len(data), 'sha256': hashlib.sha256(data).hexdigest()})
with (target / 'BYTE-INVENTORY.json').open('xb') as f:
    f.write((json.dumps({'schema': 'mir2.native-origin-append-durable-proof.v1', 'files': inventory, 'archivedProofFiles': len(entries), 'fixtureLinksMetadataOnly': len(topology)}, indent=2) + '\n').encode())
with (target / '.gitattributes').open('x', newline='\n') as f:
    f.write('* -text -diff\n**/* -text -diff\nREADME.md text eol=lf diff\n')
with (target / 'README.md').open('x', encoding='utf-8', newline='\n') as f:
    f.write('# Actual signed R17 origin accelerator publication\n\n')
    f.write('Root reviewed the frozen narrow append tool, ran all 31 negatives on Linux, decoded the twelve actual artifacts to their 125,965 signed target hashes and R16/R17 EXE identity, then appended fourteen create-only immutable objects. The original DELIVERY JSON is last, after all twelve artifacts and its original detached CMS. Existing signed feed sequence12, Candidate, engine, Bootstrap, Gateway, Caddy and real saves are unchanged. No service stop/reload or private signing key is involved.\n\n')
    f.write('`publisher-apply-01.json` is the actual apply receipt; `post-publication-02.json` retains the actual private generation/preflight/apply proofs and service snapshots. Failed preflight01, SFTP shared-mode refusal and proof-basename read failure remain. The latter made no public changes. `PUBLISHER-EVIDENCE-01.tar.gz` stores all byte-bound regular worker proof files, including raw failed Windows fixtures and final sources; fake symlink topology stays inert metadata, with no archive link extraction. The archive was read back and each retained hash checked.\n\n')
    f.write('`https-full-verification-01.json` proves all fourteen actual HTTPS-served object hashes over verified TLS loopback, 652,887,693 bytes. `PUBLIC-VERIFICATION-01.json` independently checks public feed/signature, fourteen HEADs, full DELIVERY pair and two1MiB archive ranges; the ranges still run around0.11MiB/s. This is bounded network sampling, not complete Internet/laptop throughput. `HTTPS-36-SOURCE-VERIFICATION-01.json` verifies the entire frozen future R2 closure, including its exact existing immutable R17 installer source mapping, 817,657,896 bytes. That is source-only TLS evidence, not R2 publication or fresh CMS validation.\n\n')
    f.write('The exact r16 shipped engine already supports the signed descriptor. An already-running per-file attempt needs normal cancel/reopen to rediscover it; verified cache is reusable. Actual Windows library install/cancel/rollback proof is separate and not a GUI or shipped-EXE speed benchmark. CDN authority, upload/promote and affected-laptop speed remain open. `BYTE-INVENTORY.json` binds these raw receipts.\n')
print(json.dumps({'retainedFiles': len(inventory), 'archivedRegularFiles': len(entries), 'archiveBytes': archive_path.stat().st_size, 'linksAsMetadata': len(topology), 'target': str(target)}))
