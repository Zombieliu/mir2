# Fixed read-only Cloudflare authority probe

Status: frozen external helper plus offline fake-transport validation. This
worker did not perform an authenticated Cloudflare request, use an actual token,
register/dispatch CI, modify a secret/workflow/source repository, deploy, or
change the earlier frozen origin publication directory. Root owns integration
and any opaque credential invocation.

Only these HTTPS requests are permitted, sequentially:

1. `GET https://api.cloudflare.com:443/client/v4/user/tokens/verify`
2. `GET https://api.cloudflare.com:443/client/v4/accounts/85bf64d86ea9221e172d26feba9fd47e/r2/buckets/mir2-web3-assets`

The environment input names are `CLOUDFLARE_API_TOKEN` and
`CLOUDFLARE_ACCOUNT_ID`. Missing inputs, header-unsafe tokens or an account that
does not exactly match the fixed public scope fail before HTTP. There is no
default account, credentials file, alternate token endpoint, host override,
proxy, retry or redirect. The token is only used to construct the outgoing
Authorization header and is never included in an output.

Official shape checks, read on2026-10-05: token verification returns a result
object with an activity status; bucket metadata returns a result object with a
bucket name. The helper requires boolean success, active status and the exact
known name. [Cloudflare Verify Token](https://developers.cloudflare.com/api/resources/user/subresources/tokens/methods/verify/),
[Cloudflare Get Bucket](https://developers.cloudflare.com/api/resources/r2/subresources/buckets/methods/get/).
`DOCS-BINDINGS.json` records URLs and the narrow checks used; it contains no
credential example or copied response body.

## Transport and egress restrictions

The standalone stdlib transport uses `ssl.create_default_context`, asserts
certificate verification and hostname checking, and constructs a direct
`http.client.HTTPSConnection` to the fixed host/port. A10second socket timeout
is used for each request; this is not a total wall-clock deadline, so root's CI
job should also have a bounded timeout. HTTP debugging is explicitly zero.
Environment proxy variables are never used. The two fixed paths are the only
accepted transport inputs and each request uses GET with no request body.

Only200 is accepted. Redirects/auth/HTTP failures are closed without reading
their body or Location header. Successful responses must have a JSON content
type and no content encoding except identity. A single body read is capped at
65KiB; bodies over64KiB are refused. JSON parsing rejects malformed UTF-8,
duplicate keys and nonfinite numbers. The token identifier, API errors/messages,
response headers/body, supplied account, token and exception strings are never
serialized. Any network/request/read/close error is reported through a fixed
enum, with a bounded HTTP status if it was already received. Failed token
verification prevents the bucket request.

Outputs contain only the fixed public scope/schema, safe result/stage enums,
HTTP status/null and booleans. Both stdout and a written receipt have the same
safe JSON structure. Fields for R2 write, Worker edit and deployment authority
remain `not_probed`, including on success. Reading bucket metadata does not
prove permission to upload objects, edit Workers, promote feeds or deploy.

## Opaque invocation contract (not executed by this worker)

Root supplies the two environment values through its reviewed CI mechanism,
without putting a token on argv or echoing it. Invoke the exact frozen helper:

```sh
python3 -B cloudflare_read_authority_probe.py --receipt cloudflare-read-probe-01.json
```

The receipt parent must already exist and have real directory ancestors. The
helper reserves a fresh regular single-link file0600 with create-only flags
before reading credentials or requesting HTTP; existing files, directories,
symlinks and hardlinks are never replaced. Unsafe/symlink parents and traversal
are rejected. A failed/partial fresh output is retained, not deleted or reused.
Every invocation needs a new receipt name.

Exit0 requires both GET checks and successful receipt writing/fsync/close.
Every other outcome returns2 with a fixed safe summary; CLI argument/path errors
do not echo supplied text. Root must gate on the process exit AND the pinned
helper/result. A partial output from a write/fsync failure is not an authority
receipt even if its already-written bytes contain a success value; the safe
stdout/exit reports the write failure. A receipt alone must not authorize any
write or deployment.

## Offline verification and historical boundaries

Final result:31/31 tests, no skips, native Windows Python3.13.6. No socket or
actual Cloudflare token lookup is performed by the final tests. Explicit fake
environment dictionaries, fake response streams and patched HTTPS construction
exercise the ordinary helper paths. Mock secret markers are checked absent from
all stdout/receipt results, including errors reflecting markers in exception
strings, JSON/token IDs, HTML, headers and a wrong supplied account.

Coverage includes missing/wrong environment before HTTP, token header injection,
inactive status, auth/HTTP failures on either step, no second request after
failure, redirects, exact bucket name, JSON/type/UTF-8/duplicate/nonfinite errors,
body-size boundaries, fixed host/port/TLS/GET/no body/debugging0, proxy ignoring,
request/response/read/close failures, existing receipt preservation, real
symlink/hardlink and parent-symlink refusal, argument redaction and receipt-write
failure exit2. These are offline security regressions, not Cloudflare permission
or CI acceptance.

| Raw log | Result and meaning |
|---|---|
| `local-tests-01.log` |29/29 initial checks. Its proxy harness used `patch.dict(os.environ, clear=True)`, which internally snapshots the existing process mapping. No Cloudflare value was looked up for use, output or auth request, but this is not evidence of no environment traversal. |
| `local-tests-02-red.log` |30 tests;29 pass/1 real failure. A mocked connection close exception was ignored, allowing the second GET and a false success. Exact RED source/test snapshots and their hashes are retained. This run already used direct replacement of the virtual environment mapping. |
| `local-tests-03-green.log` |31/31 final source. Close errors now produce safe network failure; first-step close failure stops the second request, and bucket close failure cannot report readability. |

The final proxy harness uses `patch.object` to replace the mapping object with
known fake values, avoiding any iteration/copy of the real environment. Historic
logs and test receipts remain unchanged, with this qualification explicitly
recorded. The retained RED probe is historical only; never integrate or execute
it as a credential probe.

Local command, executed with explicit fake transport only:

```powershell
& C:/Python313/python.exe -B C:/mir2-playtest-releases/20261004-native-r18/native-ci-auth-probe-worker-01/test_cloudflare_read_authority_probe.py
```

Root may repeat the same-byte fake tests on its CI host:

```sh
python3 -B test_cloudflare_read_authority_probe.py
```

`COMMANDS.json`, `SOURCE-BINDINGS.json` and `FROZEN-RECEIPT.json` record raw
commands, all logs/receipts, exact sources and proof hashes. Source is frozen;
root's workflow registration, CI dispatch, actual token activity/read access,
R2 write/Worker edit authority and any deployment remain outside this worker's
executed scope.
