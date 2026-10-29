# Native Windows downloads

The dedicated Worker reads only the `mir2/native/windows-invited/` prefix in
the existing `mir2-web3-assets` bucket. Its deployed route is
`assets.mir2.obelisk.build/client-updates/*`; the existing Web and upload
routes remain intact. Workers.dev and preview URLs are disabled.

The original production code, test and Wrangler configuration were admitted byte-exact
from pushed `4c60c323aed11829abae6ee5ce6e13c675a3c1c3`. Their hashes, original
README and actual16 passing reader tests are retained in the
[publication evidence](../../../docs/generated/player-qa/native-delivery-20261006/cloudflare-publication-01/README.md).

Only GET/HEAD and canonical safe paths are accepted. Immutable `releases/`,
`feeds/` and `installers/` objects use one-year Worker Cache API storage;
aliases are always `no-store` and derive their generation from one atomic
R2 channel pointer. Private `channels/` paths and write methods are blocked.
Compressed archives remain raw bytes without HTTP Content-Encoding. Strict
ETag/size checks bound artifacts to128MiB and metadata to32MiB. Actual206
ranges, suffixes,416 and If-Range fallbacks are verified in the linked proof.

As of2026-10-06, stage37348106566 verified all36 immutable objects and
817719767 public bytes, and separate CAS promote37350173926 published
signed sequence13. Publication is performed by the registered
`web-assets-r2-release.yml` workflow's distinct `stage` and `promote` actions
using `scripts/native_r17_r2_delivery.py`; the current fixed plan is not a
general future-release publisher. Promotion must bind the exact successful
stage run and receipt SHA. No unconditional channel-pointer replacement is
permitted after a CAS failure or uncertain outcome.

Browser Integrity Check produced actual403/error1010 on desktop HTTP
requests. One configuration rule now applies `set_config`/`bic:false` only
to this host's GET/HEAD requests for `latest.json`, `latest.p7s`, and
`releases/`, `feeds/`, `installers/` paths under `/client-updates/`. The
zone-wide setting remains on and security level remains medium. This is
not a WAF/bot-management skip, and upload/private/write paths are excluded.
Exact rule input, API readback and positive/negative HTTP results are retained.
[Cloudflare configuration setting](https://developers.cloudflare.com/rules/configuration-rules/settings/#browser-integrity-check).

Do not replace an existing zone entrypoint ruleset when deploying this rule.
Reconcile the actual rule ID/ref and preserve other rules. Native CMS/SHA
verification remains the updater's responsibility; the reader does not
authenticate a release by itself. Separate alias reads may straddle a future
promotion, so the updater must preserve its immutable-generation pinning.

The linked proof includes real native Windows `ureq2.12.1` TLS, production
discovery, pinned Windows CMS, engine SHA and range checks. Whole-game
installer execution and affected-laptop timing remain separate acceptance.

## Packed generations

The packed channel uses `/client-updates/packed/` and the independent R2
prefix `mir2/native/windows-invited/packed/`. Its `channels/invited.json`
retains the strict `mir2.windows.r2-channel.v1` schema. Each `latest.json` or
`latest.p7s` request resolves an internal `feeds/s<sequence>-<feedSHA>/`
object from one atomic pointer snapshot and always returns `no-store`.
Separate discovery requests can straddle promotion; the updater still verifies
the CMS pair and signed generation binding. Neither legacy channel is a
discovery fallback for this reader.

The only public immutable packed objects are:

- `generations/<lowercase SHA256>/GENERATION.json` (up to 4 MiB).
- `generations/<lowercase SHA256>/GENERATION.p7s` (up to 32 KiB).
- `generations/<lowercase SHA256>/catalog.json` (up to 128 MiB).
- `generations/<lowercase SHA256>/mir2-platform-windows.exe` (up to 128 MiB).
- `generations/<lowercase SHA256>/mir2-client.toml` (up to 32 KiB).
- `objects/packs/<lowercase SHA256>.m2p` (up to 64 MiB).

Packed discovery objects are limited to 32 KiB each. `PACK-FEED.*` are local
updater receipts, not public download objects. Extra files, provenance,
directory listing, internal pointers and internal feed paths are refused
before R2 access. Existing root and five-class path and size limits are
unchanged. The existing streaming R2 body, GET/HEAD, ETag, Range and immutable
Cache API behavior also applies to these exact packed paths; no origin fetch
or redirect is added. The current
[R2 Workers API](https://developers.cloudflare.com/r2/api/workers/workers-api-reference/)
and [streaming guidance](https://developers.cloudflare.com/workers/runtime-apis/streams/)
describe the body and range APIs used here.

Local verification (includes legacy and five-class regression):

```text
node --test infra/cloudflare/mir2-native-downloads/test/downloads.test.mjs
```

Passing local tests does not deploy this reader or publish a packed channel.
