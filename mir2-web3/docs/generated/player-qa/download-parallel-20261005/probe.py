from concurrent.futures import ThreadPoolExecutor
import datetime
import hashlib
import http.client
import json
from pathlib import Path
import ssl
import time
from urllib.parse import urlsplit

OUT = Path(__file__).resolve().parent
URL = 'https://165.154.65.136.sslip.io/client-updates/releases/bootstrap-WN-CANDIDATE-20261003-invited-16/Numeron-Legend-of-Rebirth-20261003-r16-Bootstrap.exe'
PART = 1024 * 1024
parts = urlsplit(URL)
assert parts.scheme == 'https' and parts.hostname == '165.154.65.136.sslip.io'
assert not (OUT / 'RESULT-01.json').exists(), 'preserve existing diagnostic'
tls = ssl.create_default_context()

def one(index):
    start = index * PART
    end = start + PART - 1
    connection = http.client.HTTPSConnection(parts.hostname, timeout=35, context=tls)
    clock = time.monotonic()
    try:
        connection.request('GET', parts.path, headers={
            'Range': f'bytes={start}-{end}', 'Accept-Encoding': 'identity',
            'Connection': 'close', 'User-Agent': 'Mir2-authorized-bounded-download-diagnostic/1',
        })
        response = connection.getresponse()
        header_time = time.monotonic() - clock
        headers = dict(response.getheaders())
        content_range = response.getheader('Content-Range')
        if response.status != 206 or not content_range or not content_range.startswith(f'bytes {start}-{end}/'):
            raise RuntimeError(f'bounded Range not honored: status={response.status}, range={content_range!r}')
        if response.getheader('Content-Encoding') not in (None, 'identity'):
            raise RuntimeError('unexpected content transformation')
        if int(response.getheader('Content-Length', '-1')) != PART:
            raise RuntimeError('wrong bounded Content-Length')
        content = response.read(PART + 1)
        assert len(content) == PART
        elapsed = time.monotonic() - clock
        return {
            'index': index, 'range': content_range, 'status': response.status,
            'bytes': len(content), 'headersSeconds': header_time, 'elapsedSeconds': elapsed,
            'mibPerSecond': len(content) / elapsed / (1024 * 1024),
            'sha256': hashlib.sha256(content).hexdigest(),
            'etag': response.getheader('ETag'), 'lastModified': response.getheader('Last-Modified'),
            'server': response.getheader('Server'),
        }
    finally:
        connection.close()

results = []
for concurrency in (1, 2, 4):
    print(json.dumps({'startingConnections': concurrency, 'boundedPayloadBytes': PART * concurrency}), flush=True)
    clock = time.monotonic()
    attempt = {'connections': concurrency}
    try:
        with ThreadPoolExecutor(max_workers=concurrency) as pool:
            rows = list(pool.map(one, range(concurrency)))
        elapsed = time.monotonic() - clock
        assert len({row['etag'] for row in rows}) == 1
        assert len({row['lastModified'] for row in rows}) == 1
        attempt.update({'pass': True, 'parts': rows, 'elapsedSeconds': elapsed,
                        'payloadBytes': sum(row['bytes'] for row in rows),
                        'aggregateMiBPerSecond': sum(row['bytes'] for row in rows) / elapsed / (1024 * 1024)})
    except Exception as error:
        attempt.update({'pass': False, 'elapsedSeconds': time.monotonic() - clock,
                        'errorType': type(error).__name__, 'error': str(error)})
    (OUT / f'connections-{concurrency}-01.json').write_bytes((json.dumps(attempt, indent=2) + '\n').encode())
    results.append(attempt)
    print(json.dumps({key: value for key, value in attempt.items() if key != 'parts'}), flush=True)

receipt = {
    'schema': 'mir2.download.bounded-origin-parallel-probe.v1',
    'createdUtc': datetime.datetime.now(datetime.timezone.utc).isoformat(),
    'url': URL, 'host': 'current development Windows host; not the other user laptop',
    'transport': 'direct HTTP/1.1 HTTPS with verified TLS, no environment proxy',
    'requestedPayloadBytesMaximum': 7 * PART, 'maximumConnections': 4,
    'attempts': results, 'fullInstallerDownload': False,
    'updaterSpeedAcceptance': False, 'r2SpeedAcceptance': False,
    'serverMutations': False, 'servicesOrSavesChanged': False,
}
(OUT / 'RESULT-01.json').write_bytes((json.dumps(receipt, indent=2) + '\n').encode())
print(json.dumps({'completed': True, 'successfulModes': [r['connections'] for r in results if r['pass']]}), flush=True)
