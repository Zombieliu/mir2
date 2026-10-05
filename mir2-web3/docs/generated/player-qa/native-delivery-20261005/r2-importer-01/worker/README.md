# Frozen R17 native importer: bounded external delivery

This directory is a self-contained four-file security implementation for root
review, not a deployed service or a publication receipt. No repository,
existing proof, credentials value, dependency, Git ref, server or Cloudflare
resource was changed. No network call, workflow dispatch, upload or deployment
was performed by this writer. Previous P3 and read-only CI-plan evidence remain
frozen. Root owns CI authentication, CMS/security review, integration and live
operations.

Only these four delivery files belong in the existing bulk Worker:

| External file | Repository target |
|---|---|
| `src/index.ts` | `mir2-web3/infra/cloudflare/mir2-r2-bulk-upload/src/index.ts` |
| `src/native-r17-publication.mjs` | `mir2-web3/infra/cloudflare/mir2-r2-bulk-upload/src/native-r17-publication.mjs` |
| `src/native-r17-publication-plan.json` | `mir2-web3/infra/cloudflare/mir2-r2-bulk-upload/src/native-r17-publication-plan.json` |
| `test/native-r17-publication.test.mjs` | `mir2-web3/infra/cloudflare/mir2-r2-bulk-upload/test/native-r17-publication.test.mjs` |

`APPROVED-WORKER.patch` contains exactly those paths. `INDEX-ONLY.patch` shows
the existing-file change. Index base is exact root
`d1f7dcec0213815b6f3bdcd33eff4d9d1c2774dc`, SHA256
`353a1d7420659f4c12a3a7dc654351382ec8f7dd7a1c795237777e9372a05dfe`.
Only the native import/dispatch and reserved native-prefix refusal were added;
all other original index text/legacy Web behavior is preserved. No public
download Worker, config, workflow, publisher, updater or game code is changed.

## Exact closure and source authority

The original plan SHA is
`04bdb9d5f891bba38752e4ba4029db2d638dc7f4674d494ce0c6b5ab354f3b87`;
original root attestation SHA is
`6b36134597180ad2b423c2015c05b3c48447fbd766e858c2fb0ce6f45e2565b9`.
Those complete original bytes remain base64-bound inside the literal native
plan; the module verifies their SHA, candidate and canonical closure binding.
This consumes root's existing CMS verification, and does not independently
perform CMS or allow a changed `cmsVerified` boolean to create new authority.

36 objects, 817,657,896 bytes, closure SHA
`f38a692a4ba200177f4358c9f3d1566f97fadb32cf408c072cb1f8b8bb03d4f0`.
Runtime definition canonical SHA is
`1786ca53c2ef03e142419feefa3184070bfacd570c1b3a9495f79e3c50eb3929`.
The account, bucket, prefix, HTTPS origin, paths, sizes, SHA, MIME and cache
metadata are source literals. Modifying any field fails the hardcoded native
definition hash before fetch/storage. No environment/client field can replace
the table. Root's source proof contains a nanosecond timestamp larger than JS
safe integers: that selected provenance value is an exact decimal string in
the new definition; the original receipt is unchanged and its SHA retained.

All source fetches are fixed
`https://165.154.65.136.sslip.io/client-updates/`. Feed objects map to the exact
current `latest.json`/`latest.p7s`; 33 release objects retain their release path.
The installer maps to:

```text
releases/bootstrap-WN-CANDIDATE-20261004-invited-17/Numeron-Legend-of-Rebirth-20261004-r17-Bootstrap.exe
size 27482388
SHA aced5e53a57c0e98081f8415c5ebfb95942be7b3cbb130490db24e5f8dc15621
```

`evidence/INSTALLER-ORIGIN-SOURCE.json` selects and hashes root's ordinary-file
source proof. The separate root fresh HTTPS36 receipt is copied unchanged and
bound in final receipts. It establishes loopback served bytes plus normal TLS
and source feed recheck, not Internet throughput or R2/CDN acceptance.

## Private endpoint contract

All operations first require the exact existing
`Authorization: Bearer <MIR2_R2_UPLOAD_SECRET>`; do not rotate/recreate this
secret or provide it in argv/logs/artifacts. No value was read in this phase.

| Endpoint under `/upload/native-r17` | Operation |
|---|---|
| POST `/import` | JSON with exactly `{ "index": n }` or `{ "path": "literal36path" }`; indexes are ASCII path sort0–35 |
| GET/HEAD `/objects/n` | Only a canonical index0–35; exact stored metadata/checksum required; private responses no-store |
| GET/HEAD `/pointer` | Only `channels/invited.json`; absent404 or byte/metadata-verified exact candidate, no-store |
| POST `/promote` | The complete passed stage envelope, directly as body |

No queries, encoded aliases, other reads/writes, arbitrary URL/key/digest/body,
general pointer/plan override, listing, deletion or unconditional fallback.
Import requests are bounded to1KiB; stage envelopes to64KiB, including actual
streamed bytes, strict UTF8/JSON, duplicate-key rejection and depth/token bounds.
Normal GET/HEAD and imports never write a pointer.

CI stage must import each object and verify the full public36 byte SHA, then
generate a new retained receipt. `stageEnvelopeTemplate()` is an internal
template/test helper, **not a passed publication receipt**. Its exact fields:

```text
schema = mir2.windows.r2-native-stage.v1
mode = stage; passed = true
planSha256 = original plan SHA
rootVerificationSha256 = original root receipt SHA
nativePlanSha256 = reviewed canonical native definition SHA
objectsSha256 = original36 closure SHA
candidate = exact original invited/windows-x64/sequence12/source4c60 pointer
expectedCurrent = null; pointerChanged = false
publicBase = https://assets.mir2.obelisk.build/client-updates/
publicVerified = true
verifiedObjects = exact36 sorted {path,size,sha256}
```

The authenticated envelope records the trusted CI caller's public byte
verification. Worker promotion separately rechecks every R2 object's actual
metadata/stored checksum and exact unexpired source pair; it does not itself
claim it fetched the public36 objects. Mere HEAD or a template cannot close
the CI public gate. Unknown fields/changed candidate/omission/reordering/false
pass or wrong hashes are rejected before storage operations.

## Streaming and promotion guards

Import first checks an existing R2 object; a resume requires its full body SHA
through native `crypto.DigestStream` plus exact size/strong ETag/MIME/cache,
identity encoding, custom SHA and stored native checksum. It never overwrites
or repairs a mismatching immutable object. New sources use redirect0, GET200,
identity encoding, exact positive Content-Length, a120s total source timeout
and native `FixedLengthStream(expectedSize)` with backpressure. R2 receives a
32-byte SHA256 ArrayBuffer and `Headers(If-None-Match:*)`; condition null becomes
412 without any unconditional retry. Both piping and put are awaited; failure
cancels source/fixed streams. Success rereads exact HEAD metadata/checksum.
No artifact `.arrayBuffer()`, `.text()`, clone/tee, full-buffer concat or
background unawaited import is used. Buffering is limited to tiny request,
source feed and pointer data. [FixedLengthStream](https://developers.cloudflare.com/workers/runtime-apis/streams/transformstream/),
[DigestStream](https://developers.cloudflare.com/workers/runtime-apis/web-crypto/).

Promote only accepts the bound stage, verifies36 R2 HEADs, then rereads/hashes
the original JSON/CMS pair and checks `created <= now < expires`. It verifies
the current private pointer is absent, or already this exact canonical
candidate with matching byte SHA and metadata. Initial pointer put is
create-only/checksum-bound; unknown pointer or a CAS conflict stops. Exact
candidate retries only verify, never rewrite. Successful pointer put is
reread; failures retain pointerAttempted/pointerChanged/pointerOutcomeUnknown
with sanitized error codes, including storage failure after an uncertain
commit. No rollback/delete can conceal a partial attempt.
[R2 conditions/checksums](https://developers.cloudflare.com/r2/api/workers/workers-api-reference/).

Worker promotion returns `privatePointerVerified=true` on success but
`aliasesVerified=false`, `publicVerificationRequired=true`. Root CI must
independently verify the public no-store alias pair before marking overall
publication passed. A normal new promotion uses36 HEAD +2 fixed-origin GET +
pointer GET/PUT/GET =41 external storage/network operations, under a50-count
model; actual runtime accounting remains a gate. No public full36 fetch is
hidden inside that same Worker invocation.

## Actual RED/GREEN and open gates

Node22.18 offline command:

```text
node --experimental-strip-types --test --test-reporter=tap test/native-r17-publication.test.mjs
```

The self-contained test embeds exact frozen1159-byte feed and1614-byte CMS
fixture bytes, verifies their real SHA, and disables global network access.
All credentials are explicitly toy test strings. It models R2 conditions,
checksums, FixedLengthStream and DigestStream; those models do not prove
Cloudflare runtime/R2 acceptance.

| Retained attempt | Result |
|---|---|
| `red-01` unchanged base | 1/3 pass, two real legacy native-prefix writes wrongly200; original test/source and logs retained |
| `green-attempt-01` | 104/106 pass; BOM parsing and early-race source cancellation failed; full raw failures retained |
| `green-focused-02` | Both fixes and their strict-JSON neighbors pass6/6 |
| `green-full-03` | 107/107, including native ArrayBuffer checksum contract and tampered GET refusal |
| `green-self-contained-04` | Final mirrored source/import layout and embedded fixtures pass107/107 |

An initial Python stdout rendering error for U+FEFF happened **after** it saved
Node's actual failure logs/receipt; that harness diagnostic is separately
retained, then fixed to UTF8. It did not hide or change the Node failures.

Other tested boundaries: auth-before-side-effects, frozen proof changes,
caller URL/key/digest injection, duplicate/large requests, missing/changed
objects, corrupted existing bodies/metadata/checksum, redirect/compression/
length/source SHA errors, early conditional cancellation, missing stream
runtime, exact pointer retry, conflicting/concurrent CAS and unknown commits.
The111,013,376-byte exercise streamed synthetic64KiB chunks and correctly
failed the frozen EXE checksum without model commit; it is not a successful
upload of that EXE and made no real HTTP/R2 request.

Still open: root independent source/security review, actual opaque CI token
type/permissions, existing bearer match, deploy/create/route/bucket authority,
actual Workers native stream/checksum/conditional accounting, all36 public
full-byte stage, explicit promotion and real public alias verification,
issued-library raw404→origin partial repair and human laptop acceptance.
Root's separate first user-token probe401 does not prove the account-token
path invalid; this writer did not probe either. Do not bypass CMS, weaken
conditions, rebase the frozen pointer, or claim native/CDN acceptance from
these Node checks. No further writes after final freeze.
