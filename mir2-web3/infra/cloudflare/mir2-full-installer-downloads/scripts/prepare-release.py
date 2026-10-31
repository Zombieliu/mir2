"""Pin an existing, verified installer as immutable 64-MiB delivery objects."""
from pathlib import Path
import argparse
import datetime
import email.utils
import hashlib
import json

EXPECTED_SETUP = '3b926d5eb5dd8ee1fc46b6d39fba3eda3eb538ba257405733abe1defae840172'
EXPECTED_BYTES = 1026815762
EXPECTED_DESCRIPTOR = '1c83b417423710aad71e1b50316b452e45a99b4c67ea11c31054f086333760ef'
EXPECTED_SIGNATURE = '43265b21ccde2f460dbb9e4db79606913c6915663bad0bf10ed8ba89037aab3a'
CHUNK_BYTES = 64 * 1024 * 1024

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--installer', type=Path, required=True)
    parser.add_argument('--metadata', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    assert args.installer.is_file() and args.installer.stat().st_size == EXPECTED_BYTES
    assert hashlib.sha256((args.metadata / 'release.json').read_bytes()).hexdigest() == EXPECTED_DESCRIPTOR
    assert hashlib.sha256((args.metadata / 'release.p7s').read_bytes()).hexdigest() == EXPECTED_SIGNATURE
    descriptor = json.loads((args.metadata / 'release.json').read_text(encoding='utf-8'))
    assert descriptor['installer']['sha256'].lower() == EXPECTED_SETUP
    prefix = 'mir2/native/windows-full-installers/' + EXPECTED_SETUP + '/'
    parts = []
    hasher = hashlib.sha256()
    offset = 0
    with args.installer.open('rb') as stream:
        while data := stream.read(CHUNK_BYTES):
            hasher.update(data)
            sha = hashlib.sha256(data).hexdigest()
            parts.append({'offset': offset, 'bytes': len(data), 'sha256': sha, 'key': prefix + 'parts/' + sha + '.bin'})
            offset += len(data)
    assert offset == EXPECTED_BYTES and hasher.hexdigest() == EXPECTED_SETUP
    files = []
    for name, content_type in [('release.json', 'application/json'), ('release.p7s', 'application/pkcs7-signature'), ('SHA256SUMS.txt', 'text/plain; charset=utf-8'), ('README.zh-CN.txt', 'text/plain; charset=utf-8')]:
        data = (args.metadata / name).read_bytes()
        assert 0 < len(data) <= 32768
        files.append({'name': name, 'bytes': len(data), 'sha256': hashlib.sha256(data).hexdigest(), 'key': prefix + 'files/' + name, 'contentType': content_type})
    created = datetime.datetime.fromisoformat(descriptor['createdAtUtc']).astimezone(datetime.timezone.utc)
    release = {'schema': 1, 'sha256': EXPECTED_SETUP, 'bytes': EXPECTED_BYTES,
               'publicBase': 'https://assets.mir2.obelisk.build/client-updates/installers/' + EXPECTED_SETUP + '/',
               'lastModified': email.utils.format_datetime(created.replace(microsecond=0), usegmt=True),
               'allowedHosts': ['assets.mir2.obelisk.build', 'mir2-full-installer-downloads.zombiesliu.workers.dev'],
               'parts': parts, 'files': files}
    args.output.mkdir(parents=True, exist_ok=True)
    text = json.dumps(release, ensure_ascii=False, indent=2) + '\n'
    with (args.output / 'release.json').open('x', encoding='utf-8', newline='\n') as stream:
        stream.write(text)
    with (args.output / 'release.mjs').open('x', encoding='utf-8', newline='\n') as stream:
        stream.write('// Generated from the already-verified release; no installer bytes are changed.\nexport default ' + text.rstrip() + ';\n')
    print(json.dumps({'passed': True, 'installerSha256': EXPECTED_SETUP, 'bytes': offset, 'parts': len(parts), 'companions': len(files), 'manifestSha256': hashlib.sha256(text.encode()).hexdigest()}))

if __name__ == '__main__':
    main()
