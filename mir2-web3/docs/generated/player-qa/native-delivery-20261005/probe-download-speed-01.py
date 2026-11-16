import hashlib
import http.client
import json
import ssl
import time
from pathlib import Path
from urllib.parse import quote

proof = Path(__file__).parent
old = Path('C:/Users/Administrator/.codex/worktrees/r17/mir2-player-journey/mir2-web3/dist/r17')
manifest = json.loads((old / 'PACKAGE-MANIFEST.json').read_bytes())
samples = [e for e in manifest['files'] if e['path'].startswith('mir2-assets/original-ui/') and e['path'].endswith('.png') and 512 <= e['size'] <= 32768][:8]
host = '165.154.65.136.sslip.io'
conn = http.client.HTTPSConnection(host, timeout=12, context=ssl.create_default_context())
results = []

def request(connection, target_host, path, limit, range_request=False):
    started = time.monotonic()
    try:
        headers = {'User-Agent': 'Mir2-bounded-download-diagnostic/1'}
        if range_request:
            headers['Range'] = f'bytes=0-{limit-1}'
        connection.request('GET', path, headers=headers)
        response = connection.getresponse()
        header_ms = (time.monotonic() - started) * 1000
        body = response.read(limit)
        elapsed = time.monotonic() - started
        report = {'host': target_host, 'path': path, 'status': response.status,
            'bytesRead': len(body), 'headersMs': round(header_ms, 2), 'elapsedSeconds': round(elapsed, 3),
            'boundedMiBPerSecond': round(len(body) / 1024**2 / max(elapsed, 0.001), 4),
            'sha256ReadBytes': hashlib.sha256(body).hexdigest(),
            'headers': {key: response.getheader(key) for key in ('Content-Length', 'Content-Range', 'Server', 'Cache-Control', 'CF-Cache-Status')}}
        # Never drain a potentially ignored range and never export downloaded executables.
        if range_request and response.status != 206:
            connection.close()
        return report, body
    except Exception as error:
        connection.close()
        return {'host': target_host, 'path': path, 'errorType': type(error).__name__, 'elapsedSeconds': round(time.monotonic()-started, 3)}, None

feed_probe, feed_bytes = request(conn, host, '/client-updates/latest.json', 32769)
results.append(feed_probe)
assert feed_probe.get('status') == 200 and feed_bytes and len(feed_bytes) <= 32768
feed = json.loads(feed_bytes)
game_prefix = '/client-updates/' + feed['game']['directory'] + '/'
assert feed['sequence'] == 12 and feed['game']['identity'] == 'WN-CANDIDATE-20261004-invited-17'
for entry in samples:
    row, body = request(conn, host, game_prefix + quote(entry['path'], safe='/'), entry['size'] + 1)
    if row.get('status') == 200:
        row['exactManifestBytes'] = len(body) == entry['size'] and hashlib.sha256(body).hexdigest() == entry['sha256'].lower()
    results.append(row)
row, _ = request(conn, host, game_prefix + 'DELIVERY.json', 32769)
results.append(row)
for path in (
    '/client-updates/releases/bootstrap-WN-CANDIDATE-20261003-invited-16/Numeron-Legend-of-Rebirth-20261003-r16-Bootstrap.exe',
    game_prefix + 'mir2-platform-windows.exe',
):
    row, _ = request(conn, host, path, 2 * 1024 * 1024, True)
    results.append(row)
conn.close()
cdn_host = 'assets.mir2.obelisk.build'
cdn = http.client.HTTPSConnection(cdn_host, timeout=12, context=ssl.create_default_context())
row, _ = request(cdn, cdn_host, '/client-updates/latest.json', 32769)
results.append(row)
cdn.close()
report = {'schema': 'mir2.bounded-public-download-probe.v1', 'createdUnix': int(time.time()),
    'sourceOfMeasurements': 'This Windows development host only; not the other laptop or all regions',
    'fullInstallerDownloaded': False, 'gameLaunched': False, 'playerStoresTouched': False,
    'feedSequence': feed['sequence'], 'gameCandidate': feed['game']['identity'],
    'signedLocalPayloadCount': manifest['fileCount'], 'signedLocalPayloadBytes': manifest['totalBytes'],
    'results': results}
with (proof / 'download-speed-probe-01.json').open('x', encoding='utf-8') as handle:
    json.dump(report, handle, indent=2)
print(json.dumps(report, indent=2))
