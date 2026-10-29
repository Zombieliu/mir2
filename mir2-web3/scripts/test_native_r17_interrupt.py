"""Actual SIGINT/owned child checks; fake transport, no network or credentials."""
from __future__ import annotations

import importlib.util
import io
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import sys
import tempfile
import time
import unittest

sys.dont_write_bytecode = True
BASE = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location('native_interrupt_subject', BASE / 'scripts/native_r17_r2_delivery.py')
P = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = P
SPEC.loader.exec_module(P)
FAKE_SECRET = 'PUBLIC_FAKE_INTERRUPT_FIXTURE_ONLY'


class EmptyResponse:
    status = 404

    def getheaders(self):
        return list({'cache-control': 'no-store', 'x-content-type-options': 'nosniff',
            'x-mir2-native-plan-sha256': P.NATIVE_SHA, 'x-mir2-source-plan-sha256': P.SOURCE_PLAN_SHA,
            'x-mir2-root-verification-sha256': P.ROOT_SHA, 'x-mir2-objects-sha256': P.OBJECTS_SHA}.items())

    def read(self, _maximum):
        return b''

    def close(self):
        pass


class InterruptTransport:
    def __init__(self, external_interrupt=False):
        self.calls = []
        self.external_interrupt = external_interrupt

    def open(self, method, path, *, token=None, body=None):
        assert token == FAKE_SECRET
        self.calls.append((method, path))
        if path == P.PRIVATE_PREFIX + '/pointer':
            assert method == 'GET' and body is None
            return EmptyResponse()
        if path == P.PRIVATE_PREFIX + '/objects/0':
            assert method == 'HEAD' and body is None
            return EmptyResponse()
        assert method == 'POST' and path == P.PRIVATE_PREFIX + '/import'
        assert body == P.canonical({'index': 0})
        if self.external_interrupt:
            print('OWNED_POST_READY', flush=True)
            time.sleep(30)
            raise AssertionError('timeout must interrupt before sleep ends')
        signal.raise_signal(signal.SIGINT)
        raise AssertionError('actual SIGINT must interrupt before returning')


def check_failed_receipt(path):
    value = json.loads(path.read_bytes())
    assert value['passed'] is False and value['receiptStatus'] == 'written'
    # The existing request context maps interrupted transport to this safe code.
    assert value['result'] == 'transport_failed'
    assert value['objectWriteAttempted'] is True and value['objectOutcomeUnknown'] is True
    assert value['pointerAttempted'] is False and value['pointerChanged'] is False
    assert value['pointerOutcomeUnknown'] is False and value['verifiedObjectCount'] == 0
    assert len(value['calls']) == 3 and value['calls'][-1]['complete'] is False
    return value


def child(path):
    result = P.main(['--mode', 'stage', '--receipt', path],
        environ={'MIR2_R2_UPLOAD_SECRET': FAKE_SECRET}, transport=InterruptTransport(True))
    check_failed_receipt(Path(path))
    return result


class InterruptTests(unittest.TestCase):
    def directory(self):
        parent = BASE / 'evidence/interrupt-fixtures'
        parent.mkdir(parents=True, exist_ok=True)
        return Path(tempfile.mkdtemp(prefix='owned-', dir=parent))

    def test_actual_sigint_keeps_unknown_import_without_retry_or_pointer(self):
        path = self.directory() / 'receipt.json'
        transport, output = InterruptTransport(), io.StringIO()
        result = P.main(['--mode', 'stage', '--receipt', str(path)],
            environ={'MIR2_R2_UPLOAD_SECRET': FAKE_SECRET}, transport=transport, output=output)
        self.assertEqual(result, 2)
        check_failed_receipt(path)
        self.assertEqual(len(transport.calls), 3)
        self.assertNotIn(FAKE_SECRET, path.read_text(encoding='utf-8') + output.getvalue())

    @unittest.skipUnless(os.name == 'posix' and shutil.which('timeout'), 'actual GNU timeout needs the Linux CI runner')
    def test_actual_gnu_timeout_interrupts_only_owned_child_and_writes_receipt(self):
        path = self.directory() / 'receipt.json'
        argv = [shutil.which('timeout'), '--signal=INT', '--kill-after=2s', '3s',
            sys.executable, '-B', str(Path(__file__).resolve()), '--timeout-child', str(path)]
        result = subprocess.run(argv, cwd=BASE, capture_output=True, timeout=15, check=False)
        self.assertEqual(result.returncode, 124, result.stderr.decode(errors='replace'))
        self.assertIn(b'OWNED_POST_READY', result.stdout)
        check_failed_receipt(path)
        self.assertNotIn(FAKE_SECRET.encode(), result.stdout + result.stderr + path.read_bytes())


if __name__ == '__main__':
    if len(sys.argv) == 3 and sys.argv[1] == '--timeout-child':
        raise SystemExit(child(sys.argv[2]))
    unittest.main(verbosity=2)
