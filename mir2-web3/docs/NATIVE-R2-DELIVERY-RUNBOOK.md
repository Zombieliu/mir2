# Native Windows delivery through R2

This delivery change leaves the game gateway and character stores in place.
The download origin is `https://assets.mir2.obelisk.build/client-updates/`.
The dedicated Worker route is narrower than the existing Web-asset route and
uses only `mir2/native/windows-invited/` in the existing R2 bucket. Deployment
must first verify the live account, bucket, route ownership and permissions.

## Trust and compatibility

The existing signed v1 discovery feed, v4 package manifest and pinned CMS key
remain authoritative. An independently signed `DELIVERY.json` binds the exact
Candidate, manifest and version hashes. It contains bounded gzip bundles and
optional deltas. The native readers accept no archive links, external patch
programs, URLs, scripts or arbitrary game/server commands. Every reconstructed
file must match the separately authenticated target manifest before staging.

Missing, incompatible or damaged accelerators fall back to original per-file
repair. The native client first tries the compiled CDN origin; transport/status
unavailability can use the compiled legacy origin. Accepted response integrity
failures do not trigger origin switching. Discovery fetches keep each JSON/CMS
pair on one origin and retry a mismatched pair at most three times, to tolerate
an atomic channel promotion occurring between requests. Persistent failure
rejects the update. Cancellation and failed first launch retain the existing
transaction/rollback obligations. These changes do not provide public Windows
Authenticode publisher trust.

## Artifacts and publication

1. Build an exact clean game Candidate using the existing canonical release
   pipeline. Build the new updater from clean committed source with
   `build-updater-release.ps1`, using that Candidate and the next public sequence.
2. Run `prepare-native-delivery.py --candidate-root ... --output ...` in a fresh
   external directory. Repeat `--base-exe` for supported older game executables.
   This checks the whole package closure and emits deterministic gzip artifacts.
   Bundles default to about 64 MiB expanded data (large files get their own
   bundle); each compressed object is at most 128 MiB. Expanded headers plus
   file contents are at most 512 MiB, and there are at most 64 bundles.
3. Run `sign-native-delivery.ps1` with the exact clean tooling revision and
   existing pinned code-signing identity. The independent streaming verifier
   reconstructs every bundle target and declared delta before signing. The
   Candidate itself is unchanged. Failed create-only artifacts remain available
   for diagnosis; select a new output directory for a failed build retry.
4. Build the small runtime-inclusive installer with
   `build-bootstrap-installer.ps1`. It includes the authenticated updater, four
   game seed metadata files, guides/licenses and the pinned Microsoft runtime.
   It includes no game executable or asset payload. Keep the full offline
   installer available. The online installer accepts an empty folder or an
   exact untouched retry of its own seed; existing players use their Launcher.
   It never retires an existing update journal or overwrites preferences.
5. Run `prepare-signed-native-publication.ps1` in a fresh directory. It applies
   the pinned Candidate/feed/engine/delivery CMS gates and creates the exact
   object closure, root verification receipt and publication plan. Never add
   unrelated files, credentials, logs or local proofs to this object list.
6. Deploy only `infra/cloudflare/mir2-native-downloads/wrangler.jsonc` after live
   control-plane preflight. `publish-native-r2.py stage` reads credentials from
   a private file, creates immutable objects with `If-None-Match: *`, permits
   only a full hash-verified identical resume, and hashes all public objects.
   It cannot change discovery. No credential value is printed or supplied in
   command-line arguments. Prefer bucket-scoped object credentials.
7. Verify actual public TLS, object SHA, identity encoding, cache MISS/HIT,
   GET/HEAD/ranges, unchanged existing Web paths, fresh installation, old-version
   delta, failure fallback, preferences and rollback. Compare the same payload
   on legacy/CDN paths; record transfer time separately from disk/verification
   time. No claim of throughput improvement follows from offline tests.
8. `publish-native-r2.py promote` requires the exact passed stage receipt,
   rehashes all immutable public bytes and performs one conditional R2 pointer
   write against the expected current ETag. The mutable channel aliases are
   `no-store`; immutable versioned files are cached for one year. A failed CAS
   does not overwrite another release. Inspect an unknown pointer outcome
   before retrying. Publish the paired signed legacy feed last so older installed
   Launchers discover the compatible new engine. Do not restart game services
   for a distribution-only release.

## Current local evidence (2026-10-02)

The unchanged published R10 game has 123,031 payload files / 741,339,318 bytes.
All target bytes were independently verified across 11 gzip bundles totalling
607,353,825 compressed bytes. The R9→R10 executable delta is 25,438,325 bytes;
it reconstructs the exact 110,934,016-byte target, a 77.1% payload reduction.
This measures update volume, not Internet speed. The small installer reduces
the installer download, not total first-install resource bytes. Initial
filesystem extraction/staging cost must also be measured on a real installation.

Rust reader/source/fallback tests, producer and streaming verifier tests,
installer input guards, Worker range/cache tests and publisher CAS/credential
guards pass locally. Genuine signed native integration, final installer size,
fresh install and live CDN rollout must be recorded separately after release
inputs are frozen. Existing Cloudflare OAuth refresh failed with HTTP 400;
credentials are required before upload/deployment or a real speed comparison.
The paused periodic-task and capacity goals remain paused. Private unaccepted
R11 periodic previews must not be promoted by this delivery release.

Official references: [public buckets](https://developers.cloudflare.com/r2/buckets/public-buckets/),
[R2 cache limits](https://developers.cloudflare.com/cache/interaction-cloudflare-products/r2/),
[R2 S3 conditional operations](https://developers.cloudflare.com/r2/api/s3/api/),
[Workers ranges](https://developers.cloudflare.com/r2/api/workers/workers-api-reference/).
