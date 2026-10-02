"""Guarded native R2 stage/promote publisher; stdlib only.

This checks a root-supplied verification attestation. It does not verify CMS.
Stage uploads immutable objects only. Promote is a separate explicit action.
Credentials are read only from one private JSON file, never environment/argv.
"""
from __future__ import annotations

import argparse
import ctypes
import hashlib
import hmac
import json
import os
import re
import stat
import sys
import urllib.error
import urllib.parse
import urllib.request
from datetime import datetime, timezone
from pathlib import Path

ACCOUNT_ID = "85bf64d86ea9221e172d26feba9fd47e"
BUCKET = "mir2-web3-assets"
PREFIX = "mir2/native/windows-invited/"
PUBLIC_BASE = "https://assets.mir2.obelisk.build/client-updates/"
S3_ORIGIN = f"https://{ACCOUNT_ID}.r2.cloudflarestorage.com"
POINTER_PATH = "channels/invited.json"
MAX_ARTIFACT = 128 * 1024 * 1024
MAX_METADATA = 32 * 1024 * 1024
MAX_POINTER = 4096
MAX_OBJECTS = 512
POINTER_SCHEMA = "mir2.windows.r2-channel.v1"
PLAN_SCHEMA = "mir2.windows.r2-publication.v1"
VERIFICATION_SCHEMA = "mir2.windows.r2-verification.v1"
RECEIPT_SCHEMA = "mir2.windows.r2-publication-receipt.v1"
POINTER_FIELDS = {"schema", "channel", "platform", "sequence", "sourceRevision", "feedSha256", "signatureSha256"}
IMMUTABLE = "public, max-age=31536000, immutable"


class PublishError(Exception):
    def __init__(self, code, http_status=None):
        self.code, self.http_status = code, http_status
        super().__init__(code)


def require(condition, code):
    if not condition:
        raise PublishError(code)


def canonical_json(value):
    return (json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True) + "\n").encode("ascii")


def sha256(value):
    return hashlib.sha256(value).hexdigest()


def duplicate_guard(pairs):
    result = {}
    for key, value in pairs:
        require(key not in result, "duplicate_json_key")
        result[key] = value
    return result


def parse_json(value):
    try:
        return json.loads(value.decode("utf-8"), object_pairs_hook=duplicate_guard,
                          parse_constant=lambda _: (_ for _ in ()).throw(PublishError("invalid_json_number")))
    except PublishError:
        raise
    except Exception:
        raise PublishError("invalid_json") from None


def local_file_info(path, maximum):
    path = Path(path)
    require(path.is_absolute(), "absolute_local_path_required")
    for ancestor in [path, *path.parents]:
        info = ancestor.lstat()
        require(not ancestor.is_symlink() and not ancestor.is_junction()
                and not getattr(info, "st_file_attributes", 0) & 0x400, "reparse_local_input")
    info = path.lstat()
    require(stat.S_ISREG(info.st_mode) and info.st_nlink == 1, "regular_single_link_input_required")
    require(0 < info.st_size <= maximum, "local_input_size_limit")
    return path, info


def safe_local_file(path, maximum):
    path, info = local_file_info(path, maximum)
    with path.open("rb") as stream:
        data = stream.read(maximum + 1)
    after = path.lstat()
    require(len(data) == info.st_size and len(data) <= maximum
            and (info.st_dev, info.st_ino, info.st_size, info.st_mtime_ns, info.st_nlink)
            == (after.st_dev, after.st_ino, after.st_size, after.st_mtime_ns, after.st_nlink),
            "local_input_changed")
    return data


def exact_keys(value, fields, code):
    require(isinstance(value, dict) and set(value) == set(fields), code)


def valid_hash(value):
    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None


def validate_pointer(value):
    exact_keys(value, POINTER_FIELDS, "invalid_pointer_schema")
    require(value["schema"] == POINTER_SCHEMA and value["channel"] == "invited"
            and value["platform"] == "windows-x64"
            and type(value["sequence"]) is int and 1 <= value["sequence"] <= 9007199254740991
            and isinstance(value["sourceRevision"], str)
            and re.fullmatch(r"[0-9a-f]{40}", value["sourceRevision"]) is not None
            and valid_hash(value["feedSha256"]) and valid_hash(value["signatureSha256"]),
            "invalid_pointer_fields")
    require(len(canonical_json(value)) <= MAX_POINTER, "pointer_size_limit")
    return value


def feed_directory(pointer):
    return f"feeds/s{pointer['sequence']}-{pointer['feedSha256']}/"


def relative_path(value):
    require(isinstance(value, str) and re.fullmatch(r"[A-Za-z0-9._/-]+", value) is not None
            and all(part and part not in (".", "..") and not part.startswith(".") for part in value.split("/"))
            and value.split("/", 1)[0] in {"releases", "feeds", "installers"} and "/" in value,
            "unsafe_publication_path")
    return value


def object_limit(path):
    return MAX_METADATA if re.search(r"\.(json|p7s|sig|txt)$", path, re.I) else MAX_ARTIFACT


def object_closure(objects):
    return sorted([{"path": item["path"], "size": item["size"], "sha256": item["sha256"]}
                   for item in objects], key=lambda item: item["path"])


def load_plan(plan_path, forbidden_source=None):
    raw = safe_local_file(plan_path, MAX_METADATA)
    plan = parse_json(raw)
    exact_keys(plan, {"schema", "expectedCurrent", "candidate", "verificationReceipt", "objects"}, "invalid_plan_schema")
    require(plan["schema"] == PLAN_SCHEMA, "invalid_plan_schema")
    candidate = validate_pointer(plan["candidate"])
    expected = plan["expectedCurrent"]
    if expected is not None:
        validate_pointer(expected)
        require(candidate["sequence"] > expected["sequence"], "sequence_downgrade_or_reuse")
    objects = plan["objects"]
    require(isinstance(objects, list) and 2 <= len(objects) <= MAX_OBJECTS, "publication_object_count")
    paths = set()
    for item in objects:
        exact_keys(item, {"path", "source", "size", "sha256"}, "invalid_object_schema")
        path = relative_path(item["path"])
        require(path not in paths, "duplicate_publication_path")
        paths.add(path)
        require(type(item["size"]) is int and 0 < item["size"] <= object_limit(path)
                and valid_hash(item["sha256"]), "invalid_object_size_or_hash")
        require(isinstance(item["source"], str), "invalid_object_source")
        if forbidden_source is not None:
            require(Path(item["source"]).absolute() != Path(forbidden_source).absolute(), "credential_source_publication_forbidden")
        data = safe_local_file(item["source"], object_limit(path))
        require(len(data) == item["size"] and sha256(data) == item["sha256"], "local_object_hash_mismatch")
    by_path = {item["path"]: item for item in objects}
    feed = feed_directory(candidate)
    require(feed + "latest.json" in by_path and feed + "latest.p7s" in by_path, "candidate_feed_pair_missing")
    require(by_path[feed + "latest.json"]["sha256"] == candidate["feedSha256"]
            and by_path[feed + "latest.p7s"]["sha256"] == candidate["signatureSha256"], "candidate_feed_pair_hash_mismatch")
    ref = plan["verificationReceipt"]
    exact_keys(ref, {"path", "sha256"}, "invalid_verification_reference")
    require(valid_hash(ref["sha256"]) and isinstance(ref["path"], str), "invalid_verification_reference")
    verification_raw = safe_local_file(ref["path"], MAX_METADATA)
    require(sha256(verification_raw) == ref["sha256"], "verification_receipt_hash_mismatch")
    verification = parse_json(verification_raw)
    exact_keys(verification, {"schema", "passed", "candidateVerified", "cmsVerified", "sourceRevision",
                             "sequence", "feedSha256", "signatureSha256", "objectsSha256"},
               "invalid_verification_receipt")
    require(verification["schema"] == VERIFICATION_SCHEMA and verification["passed"] is True
            and verification["candidateVerified"] is True and verification["cmsVerified"] is True,
            "root_verification_required")
    for field in ["sourceRevision", "sequence", "feedSha256", "signatureSha256"]:
        require(verification[field] == candidate[field] and type(verification[field]) is type(candidate[field]),
                "verification_candidate_mismatch")
    require(verification["objectsSha256"] == sha256(canonical_json(object_closure(objects))),
            "verification_object_closure_mismatch")
    return plan, sha256(raw)


def check_private_file(path):
    """Reject credentials readable by other principals; never inspect their text."""
    info = Path(path).lstat()
    if os.name != "nt":
        require(info.st_uid == os.getuid() and info.st_mode & 0o077 == 0, "credential_file_not_private")
        return
    from ctypes import wintypes
    advapi, kernel = ctypes.WinDLL("advapi32", use_last_error=True), ctypes.WinDLL("kernel32", use_last_error=True)
    advapi.GetNamedSecurityInfoW.argtypes = [wintypes.LPWSTR, ctypes.c_int, wintypes.DWORD,
                                           ctypes.POINTER(ctypes.c_void_p), ctypes.c_void_p,
                                           ctypes.POINTER(ctypes.c_void_p), ctypes.c_void_p,
                                           ctypes.POINTER(ctypes.c_void_p)]
    advapi.GetNamedSecurityInfoW.restype = wintypes.DWORD
    advapi.ConvertSidToStringSidW.argtypes = [ctypes.c_void_p, ctypes.POINTER(ctypes.c_void_p)]
    advapi.ConvertSidToStringSidW.restype = wintypes.BOOL
    advapi.GetAce.argtypes = [ctypes.c_void_p, wintypes.DWORD, ctypes.POINTER(ctypes.c_void_p)]
    advapi.GetAce.restype = wintypes.BOOL
    advapi.OpenProcessToken.argtypes = [wintypes.HANDLE, wintypes.DWORD, ctypes.POINTER(wintypes.HANDLE)]
    advapi.OpenProcessToken.restype = wintypes.BOOL
    advapi.GetTokenInformation.argtypes = [wintypes.HANDLE, ctypes.c_int, ctypes.c_void_p, wintypes.DWORD, ctypes.POINTER(wintypes.DWORD)]
    advapi.GetTokenInformation.restype = wintypes.BOOL
    kernel.GetCurrentProcess.restype = wintypes.HANDLE
    kernel.LocalFree.argtypes = [ctypes.c_void_p]
    kernel.CloseHandle.argtypes = [wintypes.HANDLE]
    def sid_text(pointer):
        output = ctypes.c_void_p()
        require(bool(advapi.ConvertSidToStringSidW(pointer, ctypes.byref(output))), "credential_acl_unreadable")
        try:
            return ctypes.wstring_at(output.value)
        finally:
            kernel.LocalFree(output)
    token = wintypes.HANDLE()
    require(bool(advapi.OpenProcessToken(kernel.GetCurrentProcess(), 8, ctypes.byref(token))), "credential_acl_unreadable")
    try:
        needed = wintypes.DWORD()
        advapi.GetTokenInformation(token, 1, None, 0, ctypes.byref(needed))
        buffer = ctypes.create_string_buffer(needed.value)
        require(bool(advapi.GetTokenInformation(token, 1, buffer, needed, ctypes.byref(needed))), "credential_acl_unreadable")
        current_sid = sid_text(ctypes.cast(buffer, ctypes.POINTER(ctypes.c_void_p)).contents.value)
    finally:
        kernel.CloseHandle(token)
    allowed = {"S-1-5-18", "S-1-5-32-544", current_sid}
    owner, dacl, descriptor = ctypes.c_void_p(), ctypes.c_void_p(), ctypes.c_void_p()
    require(advapi.GetNamedSecurityInfoW(str(Path(path)), 1, 5, ctypes.byref(owner), None,
                                       ctypes.byref(dacl), None, ctypes.byref(descriptor)) == 0,
            "credential_acl_unreadable")
    try:
        require(dacl.value is not None and sid_text(owner) in allowed, "credential_file_not_private")
        # ACL header: revision/reserved/size/ACE count/reserved; 8 bytes total.
        count = ctypes.c_ushort.from_address(dacl.value + 4).value
        for index in range(count):
            ace = ctypes.c_void_p()
            require(bool(advapi.GetAce(dacl, index, ctypes.byref(ace))), "credential_acl_unreadable")
            kind = ctypes.c_ubyte.from_address(ace.value).value
            flags = ctypes.c_ubyte.from_address(ace.value + 1).value
            if flags & 8 or kind == 1:  # inherit-only or access-denied grants no read.
                continue
            require(kind == 0 and sid_text(ace.value + 8) in allowed, "credential_file_not_private")
    finally:
        kernel.LocalFree(descriptor)


def load_credentials(path):
    local_file_info(path, 16384)
    check_private_file(path)
    value = parse_json(safe_local_file(path, 16384))
    check_private_file(path)
    require(isinstance(value, dict) and set(value) in [
        {"accountId", "accessKeyId", "secretAccessKey"},
        {"accountId", "accessKeyId", "secretAccessKey", "sessionToken"}], "invalid_credential_schema")
    require(value["accountId"] == ACCOUNT_ID and isinstance(value["accessKeyId"], str)
            and re.fullmatch(r"[0-9a-fA-F]{32}", value["accessKeyId"]) is not None
            and isinstance(value["secretAccessKey"], str)
            and re.fullmatch(r"[0-9a-fA-F]{64}", value["secretAccessKey"]) is not None, "invalid_credentials")
    if "sessionToken" in value:
        require(isinstance(value["sessionToken"], str) and 0 < len(value["sessionToken"]) <= 12000
                and all(32 < ord(c) < 127 for c in value["sessionToken"]), "invalid_session_token")
    return value


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, *_args, **_kwargs):
        return None


def send_http(method, url, headers, body, maximum):
    """Only fixed-origin callers; raw network exceptions/response bodies never log."""
    try:
        request = urllib.request.Request(url, data=body, headers=headers, method=method)
        with urllib.request.build_opener(NoRedirect()).open(request, timeout=30) as response:
            require(response.status in (200, 201, 204), "unexpected_http_status")
            data = response.read(maximum + 1)
            require(len(data) <= maximum, "remote_object_size_limit")
            return data, {key.lower(): value for key, value in response.headers.items()}
    except urllib.error.HTTPError as error:
        raise PublishError("http_request_failed", error.code) from None
    except PublishError:
        raise
    except Exception:
        raise PublishError("network_request_failed") from None


def sign_v4(credentials, method, path, body, extra_headers=None, now=None):
    require(path == POINTER_PATH or relative_path(path), "unsafe_s3_path")
    uri = "/" + BUCKET + "/" + urllib.parse.quote(PREFIX + path, safe="/-_.~")
    timestamp = (now or datetime.now(timezone.utc)).strftime("%Y%m%dT%H%M%SZ")
    date = timestamp[:8]
    digest = sha256(body or b"")
    headers = {"host": urllib.parse.urlsplit(S3_ORIGIN).netloc, "x-amz-date": timestamp, "x-amz-content-sha256": digest}
    if credentials.get("sessionToken"):
        headers["x-amz-security-token"] = credentials["sessionToken"]
    for key, value in (extra_headers or {}).items():
        require("\r" not in value and "\n" not in value, "unsafe_request_header")
        headers[key.lower()] = value
    names = ";".join(sorted(headers))
    canonical_headers = "".join(f"{key}:{' '.join(headers[key].strip().split())}\n" for key in sorted(headers))
    canonical = "\n".join([method, uri, "", canonical_headers, names, digest])
    scope = date + "/auto/s3/aws4_request"
    to_sign = "AWS4-HMAC-SHA256\n" + timestamp + "\n" + scope + "\n" + sha256(canonical.encode("utf-8"))
    key = ("AWS4" + credentials["secretAccessKey"]).encode("utf-8")
    for part in (date, "auto", "s3", "aws4_request"):
        key = hmac.new(key, part.encode("ascii"), hashlib.sha256).digest()
    signature = hmac.new(key, to_sign.encode("utf-8"), hashlib.sha256).hexdigest()
    headers["authorization"] = f"AWS4-HMAC-SHA256 Credential={credentials['accessKeyId']}/{scope}, SignedHeaders={names}, Signature={signature}"
    return S3_ORIGIN + uri, headers


class S3Client:
    def __init__(self, credentials, transport=send_http):
        self.credentials, self.transport = credentials, transport

    def get(self, path, maximum):
        url, headers = sign_v4(self.credentials, "GET", path, None)
        return self.transport("GET", url, headers, None, maximum)

    def put(self, path, data, condition, cache_control):
        require(set(condition) in [{"if-none-match"}, {"if-match"}], "conditional_put_required")
        if "if-none-match" in condition:
            require(condition["if-none-match"] == "*", "invalid_put_condition")
        else:
            require(valid_etag(condition["if-match"]), "invalid_put_condition")
        headers = {"content-type": mime_type(path), "cache-control": cache_control,
                   "x-amz-meta-sha256": sha256(data), **condition}
        url, headers = sign_v4(self.credentials, "PUT", path, data, headers)
        return self.transport("PUT", url, headers, data, MAX_METADATA)[1]


def mime_type(path):
    if path.endswith(".json"):
        return "application/json"
    if path.endswith(".p7s"):
        return "application/pkcs7-signature"
    if path.endswith(".gz"):
        return "application/gzip"
    return "application/octet-stream"


def valid_etag(value):
    return isinstance(value, str) and re.fullmatch(r'"[^"\r\n]+"', value) is not None


def validate_remote_bytes(data, headers, item):
    require(not headers.get("content-encoding") or headers["content-encoding"] == "identity", "remote_storage_encoding")
    require(len(data) == item["size"] and sha256(data) == item["sha256"], "remote_object_hash_mismatch")


def verify_public(item, transport=send_http, alias=None):
    path = alias if alias is not None else relative_path(item["path"])
    require(alias is None or alias in ("latest.json", "latest.p7s"), "unsafe_public_alias")
    data, headers = transport("GET", PUBLIC_BASE + path, {"Accept-Encoding": "identity"}, None, item["size"])
    validate_remote_bytes(data, headers, item)
    policy = headers.get("cache-control", "")
    require("no-store" in policy.lower() if alias else ("max-age=31536000" in policy and "immutable" in policy),
            "unexpected_public_cache_policy")
    return {"path": item["path"], "size": len(data), "sha256": sha256(data)}


def read_pointer(client):
    try:
        data, headers = client.get(POINTER_PATH, MAX_POINTER)
    except PublishError as error:
        if error.http_status == 404:
            return None, None
        raise
    require(not headers.get("content-encoding") or headers["content-encoding"] == "identity", "pointer_storage_encoding")
    require(valid_etag(headers.get("etag")), "pointer_etag_required")
    value = validate_pointer(parse_json(data))
    require(data == canonical_json(value), "pointer_not_canonical")
    return value, headers["etag"]


def expected_pointer(client, expected):
    current, etag = read_pointer(client)
    require(current == expected, "current_pointer_mismatch")
    return current, etag


def validate_stage_receipt(path, plan, plan_hash):
    value = parse_json(safe_local_file(path, MAX_METADATA))
    require(isinstance(value, dict) and value.get("schema") == RECEIPT_SCHEMA and value.get("mode") == "stage"
            and value.get("passed") is True and value.get("planSha256") == plan_hash
            and value.get("candidate") == plan["candidate"] and value.get("expectedCurrent") == plan["expectedCurrent"]
            and value.get("pointerChanged") is False and value.get("verifiedObjects") == object_closure(plan["objects"]),
            "passed_bound_stage_receipt_required")


def publish(plan, plan_hash, client, mode, receipt, public_transport=send_http, stage_receipt=None, journal=lambda: None):
    candidate = plan["candidate"]
    if mode == "stage":
        expected_pointer(client, plan["expectedCurrent"])
        for item in sorted(plan["objects"], key=lambda x: x["path"]):
            data = safe_local_file(item["source"], object_limit(item["path"]))
            require(len(data) == item["size"] and sha256(data) == item["sha256"], "local_object_hash_mismatch")
            try:
                client.put(item["path"], data, {"if-none-match": "*"}, IMMUTABLE)
                receipt["uploadedObjects"].append(item["path"])
            except PublishError as error:
                if error.http_status != 412:
                    raise
                remote, headers = client.get(item["path"], item["size"])
                try:
                    validate_remote_bytes(remote, headers, item)
                except PublishError:
                    raise PublishError("immutable_object_conflict") from None
                receipt["resumedObjects"].append(item["path"])
            receipt["verifiedObjects"].append(verify_public(item, public_transport))
            journal()
        receipt["verifiedObjects"] = sorted(receipt["verifiedObjects"], key=lambda x: x["path"])
        expected_pointer(client, plan["expectedCurrent"])
        return
    require(mode == "promote" and stage_receipt is not None, "separate_promote_stage_receipt_required")
    validate_stage_receipt(stage_receipt, plan, plan_hash)
    for item in sorted(plan["objects"], key=lambda x: x["path"]):
        # Rehash local staged inputs and public immutable bytes before any pointer write.
        data = safe_local_file(item["source"], object_limit(item["path"]))
        require(len(data) == item["size"] and sha256(data) == item["sha256"], "local_object_hash_mismatch")
        receipt["verifiedObjects"].append(verify_public(item, public_transport))
        journal()
    current, etag = read_pointer(client)
    if current == candidate:
        receipt["alreadyPromoted"] = True
    else:
        require(current == plan["expectedCurrent"], "current_pointer_mismatch")
        condition = {"if-match": etag} if current else {"if-none-match": "*"}
        receipt["pointerAttempted"] = True
        journal()
        try:
            client.put(POINTER_PATH, canonical_json(candidate), condition, "no-store")
        except PublishError as error:
            if error.http_status == 412:
                raise PublishError("pointer_compare_and_swap_failed", 412) from None
            receipt["pointerOutcomeUnknown"] = True
            raise
        receipt["pointerChanged"] = True
        journal()
        promoted, _ = read_pointer(client)
        require(promoted == candidate, "promoted_pointer_verification_failed")
    by_path = {item["path"]: item for item in plan["objects"]}
    for alias in ("latest.json", "latest.p7s"):
        verify_public(by_path[feed_directory(candidate) + alias], public_transport, alias)
    receipt["aliasesVerified"] = True


def run(plan_path, credentials_path, receipt_path, mode, stage_receipt_path=None, client_factory=S3Client,
        public_transport=send_http):
    receipt_path = Path(receipt_path)
    require(receipt_path.is_absolute(), "absolute_receipt_path_required")
    require(not receipt_path.exists(), "fresh_receipt_path_required")
    # Exclusive creation retains the initial failure record even if later work fails.
    receipt = {"schema": RECEIPT_SCHEMA, "mode": mode, "passed": False, "startedUtc": datetime.now(timezone.utc).isoformat(),
               "planSha256": None, "candidate": None, "expectedCurrent": None, "uploadedObjects": [],
               "resumedObjects": [], "verifiedObjects": [], "pointerAttempted": False, "pointerChanged": False}
    with receipt_path.open("x+", encoding="utf-8", newline="\n") as output:
        def journal():
            output.seek(0)
            output.write(json.dumps(receipt, indent=2) + "\n")
            output.truncate()
            output.flush()
            os.fsync(output.fileno())
        journal()
        try:
            plan, plan_hash = load_plan(plan_path, credentials_path)
            receipt.update(planSha256=plan_hash, candidate=plan["candidate"], expectedCurrent=plan["expectedCurrent"])
            require(mode in ("stage", "promote") and (mode != "promote" or stage_receipt_path is not None),
                    "separate_promote_stage_receipt_required")
            # Credentials never come from the plan, environment, CLI values, or defaults.
            credentials = load_credentials(credentials_path)
            publish(plan, plan_hash, client_factory(credentials), mode, receipt, public_transport, stage_receipt_path, journal)
            receipt["passed"] = True
        except PublishError as error:
            receipt["failure"] = {"code": error.code, **({"httpStatus": error.http_status} if error.http_status is not None else {})}
        except (OSError, ValueError, TypeError, KeyError):
            receipt["failure"] = {"code": "local_input_error"}
        except Exception:
            receipt["failure"] = {"code": "publication_failed"}
        receipt["finishedUtc"] = datetime.now(timezone.utc).isoformat()
        journal()
    return receipt


class SafeArgumentParser(argparse.ArgumentParser):
    def error(self, _message):
        # Do not echo mistaken credential values supplied as unknown arguments.
        print(json.dumps({"passed": False, "failure": {"code": "invalid_cli_arguments"}}))
        raise SystemExit(2)


def main():
    parser = SafeArgumentParser(description=__doc__)
    parser.add_argument("mode", choices=["stage", "promote"])
    parser.add_argument("--plan", required=True)
    parser.add_argument("--credentials-file", required=True, help="Private JSON filename only; never pass credential values.")
    parser.add_argument("--receipt", required=True, help="Fresh absolute retained public receipt path.")
    parser.add_argument("--stage-receipt", help="Passed stage receipt required for a separate promote action.")
    args = parser.parse_args()
    try:
        result = run(args.plan, args.credentials_file, args.receipt, args.mode, args.stage_receipt)
        print(json.dumps({"passed": result["passed"], "mode": args.mode, "receipt": args.receipt,
                          "failure": result.get("failure")}))
        return 0 if result["passed"] else 1
    except PublishError as error:
        print(json.dumps({"passed": False, "failure": {"code": error.code}}))
        return 1
    except Exception:
        print(json.dumps({"passed": False, "failure": {"code": "receipt_or_local_input_error"}}))
        return 1


if __name__ == "__main__":
    sys.exit(main())
