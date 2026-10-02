# Native Windows download route

The dedicated Worker serves
`https://assets.mir2.obelisk.build/client-updates/*` from the existing
`mir2-web3-assets` bucket using only `mir2/native/windows-invited/` keys.
Its route is more specific than the existing Web route and disjoint from
`/upload*`. This source does not deploy or modify either existing Worker.

## Read contract

Only GET and HEAD are accepted. Paths decode once and must be safe canonical
relative ASCII paths. Encoded separators, residual percent escapes, traversal,
hidden segments, queries and fragments are rejected. Direct objects must be
under `releases/`, `feeds/`, or `installers/`; `channels/` is private.

Immutable objects receive one-year caching through normalized Worker Cache API
keys. Ranges bypass the cache. HEAD is bodyless and ignores Range; If-None-Match
works on both fresh metadata and cache hits. Single closed, open and suffix
byte ranges support clamping, exact strong/date If-Range and
`416 Content-Range: bytes */<size>`. Ends use offset + length, never an absent
R2 end field.
Date and entity-tag If-Range comparison follow
[RFC 9110 section 13.1.5](https://www.rfc-editor.org/rfc/rfc9110.html#name-if-range).

Archives remain literal compressed bytes with no HTTP Content-Encoding.
Unexpected storage encoding, invalid ETags and oversized objects fail closed.
Artifacts are bounded to 128 MiB, JSON/signature metadata to 32 MiB.

`latest.json` and `latest.p7s` each use one fresh atomic R2 GET of
`channels/invited.json`. Aliases are always `no-store` and never use cache.
The pointer is at most 4096 bytes with exactly:

```text
schema: "mir2.windows.r2-channel.v1"
channel: "invited"
platform: "windows-x64"
sequence: positive JavaScript-safe integer
sourceRevision: 40 lowercase hexadecimal characters
feedSha256: 64 lowercase hexadecimal characters
signatureSha256: 64 lowercase hexadecimal characters
```

Store canonical minified JSON with an optional trailing newline. Unknown or
duplicate fields, alternate numeric encodings and malformed hashes are rejected.
Targets derive as `feeds/s<sequence>-<feedSha256>/latest.json` and `latest.p7s`.

Alias headers expose `X-Mir2-Sequence`, `X-Mir2-Feed-Sha256`,
`X-Mir2-Feed-Path`, and `X-Mir2-Signature-Path`. Each response is coherent,
but two separate alias requests can straddle promotion. Clients should pin the
immutable generation or retry a feed/signature mismatch before accepting it.
The native updater and root retain responsibility for CMS verification.

## Publisher contract

`apps/game-client/windows-updater/scripts/publish-native-r2.py` uses stdlib
SigV4 and fixed account, bucket, public domain and owned prefix. Credentials come
only from a private regular JSON file with exactly `accountId`, `accessKeyId`,
`secretAccessKey`, and optionally `sessionToken`. Values are never command
arguments, environment defaults, receipts or network error messages.
Windows ACLs allow SYSTEM/Administrators/current process user only; POSIX files
must be caller-owned with mode 0600/0400.

The plan has exactly:

```text
schema: "mir2.windows.r2-publication.v1"
expectedCurrent: previous complete pointer, or null for an absent channel
candidate: new complete pointer
verificationReceipt: {path: absolute file, sha256: lowercase hex}
objects: [{path: owned relative path, source: absolute local file,
           size: positive bytes, sha256: lowercase hex}]
```

Both candidate feed files must be present and match pointer hashes. At most
512 objects are allowed; this publishes packs, patches, metadata and installers,
not 123,000 individual game files. Inputs must be regular single-link files
without symlink/junction/reparse ancestors. The credential file cannot be an
upload source.

Root first verifies candidate/source/CMS through its trusted external verifier
and supplies this exact attestation:

```text
schema: "mir2.windows.r2-verification.v1"
passed: true
candidateVerified: true
cmsVerified: true
sourceRevision, sequence, feedSha256, signatureSha256: candidate bindings
objectsSha256: approved immutable publication closure hash
```

`objectsSha256` hashes canonical JSON of the path-sorted list of each object's
`{path,size,sha256}`, excluding `source`. Canonical JSON is Python
`json.dumps(value,sort_keys=True,separators=(",",":"),ensure_ascii=True) + "\n"`.
The plan binds exact attestation bytes by SHA-256. These booleans attest root
verification; this helper does not independently verify CMS.

## Separate actions

After root review, cloud authorization, Worker deployment and external CMS
verification, root/operator may run each explicit action with fresh receipts:

```text
python publish-native-r2.py stage --plan <absolute-plan> --credentials-file <private-json> --receipt <fresh-absolute-receipt>
python publish-native-r2.py promote --plan <absolute-plan> --credentials-file <private-json> --stage-receipt <passed-stage-receipt> --receipt <fresh-absolute-receipt>
```

Stage uses S3 `If-None-Match: *` for every immutable object. Existing owned
objects resume only after a full-byte SHA match; conflicts reject. Every
uploaded object is fully rehashed through the pinned public domain. Stage
never writes a channel pointer.

Promote requires a passed stage receipt bound to exact plan bytes. It rehashes
all local/public immutable files, then GETs and validates the actual old pointer
against `expectedCurrent`. The sole pointer write uses `If-Match: <ETag>` or
`If-None-Match: *` initially. A failed CAS never retries an unconditional write.
Same/lower candidate sequences reject. A previously committed exact candidate
can be verified idempotently by a separate explicit promote attempt.

Each attempt exclusively creates a retained public receipt before work.
Failures, partial uploads and uncertain commits stay recorded; no object,
release, cache entry or old receipt is deleted. CAS is followed by pointer and
no-store public alias verification. A post-CAS failure retains
`pointerChanged`/`pointerOutcomeUnknown` for root review.

[Official R2 S3 conditional-write support](https://developers.cloudflare.com/r2/api/s3/api/).
Cloud rollout and effective token permissions remain unverified until root has
valid authorization. Mocked tests do not prove deployed delivery.

## Local verification

```text
node --test infra/cloudflare/mir2-native-downloads/test/downloads.test.mjs
python apps/game-client/windows-updater/scripts/test-publish-native-r2.py
```

Tests use mocked R2/cache/S3/public HTTP only. They do not operate game services,
native clients, paused goals or Cloudflare resources.
