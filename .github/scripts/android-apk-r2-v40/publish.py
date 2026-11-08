"""Publish only the frozen signed v40 APK, using credentials inside Actions."""
import hashlib
import json
import os
from pathlib import Path
import re
import secrets
import sys
import time
import urllib.error
import urllib.request

SHA256 = "cbee2d6a316e37043395517c1c6ad082abad41f8e74bf399743aab476ac2417e"
SIZE = 412322437
FILENAME = "Mir2-Android-Playtest-20261107-arm64.apk"
KEY = f"mir2/v/android-playtest-v40-20261107-cbee2d6a316e/{FILENAME}"
PUBLIC_URL = f"https://assets.mir2.obelisk.build/{KEY}"
TAG = "android-playtest-v40-20261107-cbee2d6a316e-staging"
RELEASE_ID = 438412927
ROOT = Path(__file__).resolve().parent
STATE = Path(os.environ.get("RUNNER_TEMP", "/tmp")) / "mir2-apk-r2-worker-state.json"
ACCOUNT = os.environ.get("CLOUDFLARE_ACCOUNT_ID", "").strip()
TOKEN = os.environ.get("CLOUDFLARE_API_TOKEN", "")
RUN = os.environ.get("GITHUB_RUN_ID", "")
ATTEMPT = os.environ.get("GITHUB_RUN_ATTEMPT", "1")
if not re.fullmatch(r"[0-9a-f]{32}", ACCOUNT) or not TOKEN or not RUN.isdecimal() or not ATTEMPT.isdecimal():
    raise SystemExit("Missing or invalid existing Actions credentials/run binding")
NAME = f"mir2-apk-v40-{RUN}-{ATTEMPT}"
API = f"https://api.cloudflare.com/client/v4/accounts/{ACCOUNT}"


def request(url, *, method="GET", data=None, headers=None, timeout=180, opener=None):
    req = urllib.request.Request(url, data=data, headers=headers or {}, method=method)
    return (opener.open if opener else urllib.request.urlopen)(req, timeout=timeout)


def cf(path, *, method="GET", data=None, content_type="application/json", allow404=False):
    headers = {"Authorization": f"Bearer {TOKEN}", "Content-Type": content_type}
    try:
        with request(API + path, method=method, data=data, headers=headers) as response:
            result = json.load(response)
    except urllib.error.HTTPError as error:
        if allow404 and error.code == 404:
            return None
        # Do not include bodies, headers, signed URLs or credential values in errors.
        raise RuntimeError(f"Cloudflare operation {method} failed with HTTP {error.code}") from None
    if not result.get("success"):
        codes = [item.get("code") for item in result.get("errors", [])]
        raise RuntimeError(f"Cloudflare operation {method} rejected: codes={codes}")
    return result.get("result")


def cleanup():
    if not STATE.exists():
        return
    state = json.loads(STATE.read_text())
    if state.get("name") != NAME:
        raise RuntimeError("Cleanup binding does not match this exact run")
    cf(f"/workers/scripts/{NAME}", method="DELETE", allow404=True)
    STATE.unlink()
    print("Temporary transfer Worker removed", flush=True)


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, *_args, **_kwargs):
        return None


def signed_source_url():
    gh_token = os.environ.get("GH_TOKEN", "")
    if not gh_token:
        raise RuntimeError("Missing repository-scoped Actions download token")
    headers = {"Authorization": f"Bearer {gh_token}", "Accept": "application/vnd.github+json", "X-GitHub-Api-Version": "2022-11-28", "User-Agent": "mir2-apk-r2-publish"}
    for attempt in range(40):
        with request(f"https://api.github.com/repos/Zombieliu/mir2/releases/{RELEASE_ID}", headers=headers) as response:
            release = json.load(response)
        if not release.get("draft") or release.get("tag_name") != TAG:
            raise RuntimeError("Expected the temporary unpublished staging draft")
        matches = [a for a in release.get("assets", []) if a.get("name") == FILENAME]
        if len(matches) == 1 and matches[0].get("state") == "uploaded":
            break
        if attempt == 0:
            print("Awaiting completion of the pinned draft APK upload", flush=True)
        time.sleep(15)
    if len(matches) != 1 or matches[0].get("state") != "uploaded" or matches[0].get("size") != SIZE:
        raise RuntimeError("Staged APK name/size mismatch")
    digest = matches[0].get("digest")
    if digest and digest != f"sha256:{SHA256}":
        raise RuntimeError("GitHub stored APK digest mismatch")
    asset_id = matches[0]["id"]
    headers["Accept"] = "application/octet-stream"
    try:
        request(f"https://api.github.com/repos/Zombieliu/mir2/releases/assets/{asset_id}", headers=headers, opener=urllib.request.build_opener(NoRedirect()))
    except urllib.error.HTTPError as error:
        if error.code != 302:
            raise RuntimeError(f"Staged APK download failed with HTTP {error.code}") from None
        location = error.headers.get("Location", "")
    else:
        raise RuntimeError("Expected a temporary download redirect")
    from urllib.parse import urlsplit
    parsed = urlsplit(location)
    if parsed.scheme != "https" or parsed.hostname != "release-assets.githubusercontent.com" or parsed.username or parsed.password or parsed.port:
        raise RuntimeError("Unexpected staged APK download origin")
    return location


def verify_public():
    headers = {"Accept-Encoding": "identity", "User-Agent": "mir2-apk-verifier"}
    with request(PUBLIC_URL, method="HEAD", headers=headers) as response:
        head = dict(response.headers.items())
        if response.headers.get_content_type() != "application/vnd.android.package-archive":
            raise RuntimeError("Public APK content type mismatch")
        head_length = response.headers.get("Content-Length")
        if head_length and int(head_length) != SIZE:
            raise RuntimeError("Public APK HEAD size mismatch")
    with request(PUBLIC_URL, headers={**headers, "Range": "bytes=0-1023"}) as response:
        content_range = response.headers.get("Content-Range")
        data = response.read()
        if response.status != 206 or content_range != f"bytes 0-1023/{SIZE}" or len(data) != 1024:
            raise RuntimeError(f"Public Range verification failed: status={response.status}, content_range={content_range}, bytes={len(data)}")
    digest = hashlib.sha256()
    length = 0
    with request(PUBLIC_URL, headers=headers) as response:
        while chunk := response.read(1024 * 1024):
            digest.update(chunk)
            length += len(chunk)
    if length != SIZE or digest.hexdigest() != SHA256:
        raise RuntimeError("Public full-download SHA256/size mismatch")
    result = {"ok": True, "url": PUBLIC_URL, "bytes": length, "sha256": digest.hexdigest(), "headContentLength": head_length, "range": content_range, "versionCode": 40, "minimumAndroid": 12, "abi": "arm64-v8a", "label": "basic playtest; three-class level-30 journey not accepted"}
    (Path(os.environ["RUNNER_TEMP"]) / "mir2-apk-r2-result.json").write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(result), flush=True)
    with open(os.environ["GITHUB_STEP_SUMMARY"], "a") as summary:
        summary.write(f"Verified signed Android v40 basic playtest: [{FILENAME}]({PUBLIC_URL})\n\n{SIZE} bytes; SHA256 `{SHA256}`.\n\nFull download and HTTP Range verified. Android 12+, ARM64. Three-class level-30 journey remains unaccepted.\n")


def main():
    if sys.argv[1:] == ["--cleanup"]:
        cleanup()
        return
    bucket = (os.environ.get("MIR2_R2_ASSET_BUCKET") or os.environ.get("MIR2_R2_BUCKET") or "mir2-web3-assets").strip()
    if bucket != "mir2-web3-assets":
        raise RuntimeError("Existing bucket secret does not match the reviewed public asset bucket")
    source_url = signed_source_url()
    subdomain = cf("/workers/subdomain")["subdomain"]
    if not re.fullmatch(r"[a-z0-9-]+", subdomain):
        raise RuntimeError("Invalid existing workers.dev subdomain")
    if cf(f"/workers/scripts/{NAME}/settings", allow404=True) is not None:
        raise RuntimeError("Refusing to replace an existing transfer Worker")
    upload_secret = secrets.token_hex(32)
    print(f"::add-mask::{upload_secret}")
    metadata = {"main_module": "worker.mjs", "compatibility_date": "2026-05-18", "bindings": [
        {"type": "r2_bucket", "name": "APK", "bucket_name": bucket},
        {"type": "secret_text", "name": "UPLOAD_SECRET", "text": upload_secret},
        {"type": "plain_text", "name": "EXPIRES_AT", "text": str(int(time.time() * 1000) + 3600000)},
    ]}
    boundary = "mir2" + secrets.token_hex(16)
    body = (f'--{boundary}\r\nContent-Disposition: form-data; name="metadata"\r\nContent-Type: application/json\r\n\r\n'.encode() + json.dumps(metadata).encode() + f'\r\n--{boundary}\r\nContent-Disposition: form-data; name="worker.mjs"; filename="worker.mjs"\r\nContent-Type: application/javascript+module\r\n\r\n'.encode() + (ROOT / "worker.mjs").read_bytes() + f"\r\n--{boundary}--\r\n".encode())
    STATE.write_text(json.dumps({"name": NAME}))
    try:
        cf(f"/workers/scripts/{NAME}", method="PUT", data=body, content_type=f"multipart/form-data; boundary={boundary}")
        cf(f"/workers/scripts/{NAME}/subdomain", method="POST", data=json.dumps({"enabled": True, "previews_enabled": False}).encode())
        print("Temporary transfer ready; only the pinned v40 APK object is writable", flush=True)
        for attempt in range(6):
            try:
                with request(f"https://{NAME}.{subdomain}.workers.dev/publish", method="POST", data=json.dumps({"sourceUrl": source_url}).encode(), headers={"Authorization": f"Bearer {upload_secret}", "Content-Type": "application/json"}) as response:
                    receipt = json.load(response)
                if receipt.get("sha256") != SHA256 or receipt.get("size") != SIZE:
                    raise RuntimeError("Transfer receipt mismatch")
                break
            except (urllib.error.HTTPError, urllib.error.URLError, TimeoutError) as error:
                code = getattr(error, "code", None)
                if attempt == 5 or code in (400, 401, 403, 409):
                    raise RuntimeError(f"Transfer did not complete: HTTP {code or 'network/timeout'}") from None
                print(f"Waiting for transfer endpoint / retrying attempt {attempt + 2}", flush=True)
                time.sleep(min(2 ** attempt, 16))
        print("R2 accepted the exact APK SHA256", flush=True)
        verify_public()
    finally:
        cleanup()


if __name__ == "__main__":
    main()
