#!/usr/bin/env python3
"""Fixed two-GET read-only Cloudflare probe; no raw credential/response logging.

Only the opaque CI invocation reads the two named environment variables. Success
proves active token and readable known-bucket metadata, never upload/Worker edit.
"""
from __future__ import annotations

from enum import Enum
import http.client
import json
import os
from pathlib import Path
import re
import ssl
import stat
import sys

sys.dont_write_bytecode = True
HOST = 'api.cloudflare.com'
PORT = 443
ACCOUNT = '85bf64d86ea9221e172d26feba9fd47e'
BUCKET = 'mir2-web3-assets'
VERIFY_PATH = '/client/v4/user/tokens/verify'
BUCKET_PATH = '/client/v4/accounts/' + ACCOUNT + '/r2/buckets/' + BUCKET
ALLOWED_PATHS = (VERIFY_PATH, BUCKET_PATH)
TIMEOUT_SECONDS = 10
BODY_LIMIT = 64 * 1024
READ_LIMIT = 65 * 1024


class Code(str, Enum):
    OK = 'ok'
    ARGUMENTS_INVALID = 'arguments_invalid'
    TOKEN_MISSING = 'token_missing'
    TOKEN_INVALID = 'token_invalid'
    ACCOUNT_MISSING = 'account_missing'
    ACCOUNT_MISMATCH = 'account_mismatch'
    NETWORK_ERROR = 'network_error'
    TLS_POLICY_ERROR = 'tls_policy_error'
    REDIRECT_DENIED = 'redirect_denied'
    HTTP_ERROR = 'http_error'
    CONTENT_TYPE_REJECTED = 'content_type_rejected'
    CONTENT_ENCODING_REJECTED = 'content_encoding_rejected'
    RESPONSE_OVERSIZED = 'response_oversized'
    JSON_INVALID = 'json_invalid'
    RESPONSE_SHAPE_INVALID = 'response_shape_invalid'
    API_REJECTED = 'api_rejected'
    TOKEN_INACTIVE = 'token_inactive'
    BUCKET_SCOPE_MISMATCH = 'bucket_scope_mismatch'
    RECEIPT_ERROR = 'receipt_error'
    INTERNAL_ERROR = 'internal_error'


class SafeFailure(Exception):
    def __init__(self, code, status=None):
        self.code = code if isinstance(code, Code) else Code.INTERNAL_ERROR
        self.status = status if type(status) is int and 100 <= status <= 599 else None
        # Even a caught exception's args contain only a fixed safe enum.
        super().__init__(self.code.value)


def safe_status(value):
    if type(value) is not int or not 100 <= value <= 599:
        raise SafeFailure(Code.RESPONSE_SHAPE_INVALID)
    return value


def empty_result():
    return {
        'schema': 'mir2.cloudflare.authority-read-probe.v1',
        'result': Code.INTERNAL_ERROR.value, 'passed': False,
        'stage': 'environment', 'verify_http_status': None, 'bucket_http_status': None,
        'token_active': False, 'bucket_readable': False,
        'scope': {'host': HOST, 'port': PORT, 'method': 'GET',
                  'verify_path': VERIFY_PATH, 'bucket_path': BUCKET_PATH,
                  'account': ACCOUNT, 'bucket': BUCKET},
        'r2_write_authority': 'not_probed', 'worker_edit_authority': 'not_probed',
        'deployment_authority': 'not_probed', 'receipt_status': 'not_written',
    }


class FixedHttpsTransport:
    """Direct stdlib HTTPS; environment proxy settings and redirects are unused."""
    def get(self, path, token):
        if path not in ALLOWED_PATHS:
            raise SafeFailure(Code.INTERNAL_ERROR)
        connection = None
        status = None
        try:
            context = ssl.create_default_context()
            if context.verify_mode != ssl.CERT_REQUIRED or context.check_hostname is not True:
                raise SafeFailure(Code.TLS_POLICY_ERROR)
            connection = http.client.HTTPSConnection(HOST, port=PORT, timeout=TIMEOUT_SECONDS, context=context)
            connection.set_debuglevel(0)
            connection.request('GET', path, body=None, headers={
                'Authorization': 'Bearer ' + token, 'Accept': 'application/json',
                'Accept-Encoding': 'identity', 'User-Agent': 'Mir2ReadAuthorityProbe/1',
            })
            response = connection.getresponse()
            status = safe_status(response.status)
            if 300 <= status <= 399:
                raise SafeFailure(Code.REDIRECT_DENIED, status)
            if status != 200:
                raise SafeFailure(Code.HTTP_ERROR, status)
            kind = response.getheader('Content-Type', '')
            if not isinstance(kind, str) or kind.split(';', 1)[0].strip().lower() != 'application/json':
                raise SafeFailure(Code.CONTENT_TYPE_REJECTED, status)
            encoding = response.getheader('Content-Encoding', '')
            if not isinstance(encoding, str) or encoding.strip().lower() not in ('', 'identity'):
                raise SafeFailure(Code.CONTENT_ENCODING_REJECTED, status)
            body = response.read(READ_LIMIT)
            if not isinstance(body, bytes) or len(body) > BODY_LIMIT:
                raise SafeFailure(Code.RESPONSE_OVERSIZED, status)
            return status, body
        except SafeFailure:
            raise
        except BaseException:
            # Never format the exception: TLS/HTTP errors may echo request data.
            raise SafeFailure(Code.NETWORK_ERROR, status) from None
        finally:
            if connection is not None:
                try:
                    connection.close()
                except BaseException:
                    pass


def decode_json(body):
    if not isinstance(body, bytes) or len(body) > BODY_LIMIT:
        raise SafeFailure(Code.RESPONSE_OVERSIZED)
    def pairs(items):
        value = {}
        for key, item in items:
            if key in value:
                raise SafeFailure(Code.JSON_INVALID)
            value[key] = item
        return value
    def constant(_item):
        raise SafeFailure(Code.JSON_INVALID)
    try:
        value = json.loads(body.decode('utf-8'), object_pairs_hook=pairs, parse_constant=constant)
    except BaseException:
        raise SafeFailure(Code.JSON_INVALID) from None
    if not isinstance(value, dict) or not isinstance(value.get('result'), dict):
        raise SafeFailure(Code.RESPONSE_SHAPE_INVALID)
    if value.get('success') is not True:
        raise SafeFailure(Code.API_REJECTED)
    return value['result']


def probe(environ, transport=None):
    result = empty_result()
    try:
        # No fallback accounts or secrets file reads; compare account before HTTP.
        token = environ.get('CLOUDFLARE_API_TOKEN')
        account = environ.get('CLOUDFLARE_ACCOUNT_ID')
        if token is None or token == '':
            raise SafeFailure(Code.TOKEN_MISSING)
        if not isinstance(token, str) or not 1 <= len(token) <= 4096 or re.fullmatch(r'[A-Za-z0-9._~+/-]+=*', token) is None:
            raise SafeFailure(Code.TOKEN_INVALID)
        if account is None or account == '':
            raise SafeFailure(Code.ACCOUNT_MISSING)
        if not isinstance(account, str) or account != ACCOUNT:
            raise SafeFailure(Code.ACCOUNT_MISMATCH)
        transport = transport if transport is not None else FixedHttpsTransport()
        result['stage'] = 'token_verify'
        status, body = transport.get(VERIFY_PATH, token)
        result['verify_http_status'] = safe_status(status)
        if 300 <= status <= 399:
            raise SafeFailure(Code.REDIRECT_DENIED, status)
        if status != 200:
            raise SafeFailure(Code.HTTP_ERROR, status)
        verified = decode_json(body)
        token_status = verified.get('status')
        if token_status in ('disabled', 'expired'):
            raise SafeFailure(Code.TOKEN_INACTIVE)
        if token_status != 'active':
            raise SafeFailure(Code.RESPONSE_SHAPE_INVALID)
        result['token_active'] = True
        result['stage'] = 'bucket_read'
        status, body = transport.get(BUCKET_PATH, token)
        result['bucket_http_status'] = safe_status(status)
        if 300 <= status <= 399:
            raise SafeFailure(Code.REDIRECT_DENIED, status)
        if status != 200:
            raise SafeFailure(Code.HTTP_ERROR, status)
        bucket = decode_json(body)
        if bucket.get('name') != BUCKET:
            raise SafeFailure(Code.BUCKET_SCOPE_MISMATCH)
        result.update(result=Code.OK.value, passed=True, stage='complete', bucket_readable=True)
    except SafeFailure as failure:
        result['result'] = failure.code.value
        if result['stage'] == 'token_verify' and result['verify_http_status'] is None:
            result['verify_http_status'] = failure.status
        elif result['stage'] == 'bucket_read' and result['bucket_http_status'] is None:
            result['bucket_http_status'] = failure.status
    except BaseException:
        result['result'] = Code.INTERNAL_ERROR.value
    return result


def canonical(result):
    return (json.dumps(result, sort_keys=True, separators=(',', ':'), ensure_ascii=True) + '\n').encode('ascii')


def reserve_receipt(value):
    """Reserve a fresh regular file before HTTP; never replace or remove a file."""
    if not isinstance(value, str) or not value or '\x00' in value:
        raise SafeFailure(Code.RECEIPT_ERROR)
    path = Path(value)
    if '..' in path.parts:
        raise SafeFailure(Code.RECEIPT_ERROR)
    path = Path(os.path.abspath(path))
    for parent in [*reversed(path.parent.parents), path.parent]:
        info = parent.lstat()
        if not stat.S_ISDIR(info.st_mode) or stat.S_ISLNK(info.st_mode) or getattr(info, 'st_file_attributes', 0) & 0x400:
            raise SafeFailure(Code.RECEIPT_ERROR)
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, 'O_NOFOLLOW', 0) | getattr(os, 'O_CLOEXEC', 0), 0o600)
    try:
        opened = os.fstat(fd)
        if not stat.S_ISREG(opened.st_mode) or opened.st_nlink != 1:
            raise SafeFailure(Code.RECEIPT_ERROR)
        current = path.lstat()
        if current.st_dev != opened.st_dev or current.st_ino != opened.st_ino or not stat.S_ISREG(current.st_mode):
            raise SafeFailure(Code.RECEIPT_ERROR)
        return os.fdopen(fd, 'wb')
    except BaseException:
        os.close(fd)
        raise SafeFailure(Code.RECEIPT_ERROR) from None


def main(argv=None, *, environ=None, transport=None, output=None):
    result = empty_result()
    stream = None
    args = sys.argv[1:] if argv is None else argv
    try:
        if len(args) != 2 or args[0] != '--receipt' or not isinstance(args[1], str):
            result.update(result=Code.ARGUMENTS_INVALID.value, stage='arguments')
        else:
            try:
                stream = reserve_receipt(args[1])
            except BaseException:
                raise SafeFailure(Code.RECEIPT_ERROR) from None
            result = probe(os.environ if environ is None else environ, transport)
            result['receipt_status'] = 'written'
            try:
                stream.write(canonical(result))
                stream.flush()
                os.fsync(stream.fileno())
                stream.close()
                stream = None
            except BaseException:
                raise SafeFailure(Code.RECEIPT_ERROR) from None
    except BaseException:
        # No exception class, arguments, receipt path or response values are emitted.
        result.update(result=Code.RECEIPT_ERROR.value, passed=False,
                      stage='receipt', receipt_status='not_written')
    finally:
        if stream is not None:
            try:
                stream.close()
            except BaseException:
                pass
    try:
        (sys.stdout if output is None else output).write(canonical(result).decode('ascii'))
    except BaseException:
        return 2
    return 0 if result['passed'] is True and result['receipt_status'] == 'written' else 2


if __name__ == '__main__':
    raise SystemExit(main())
