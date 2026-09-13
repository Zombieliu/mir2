# Native Android/Web entity release alignment — 2026-09-12

## Result

This Android-only slice started from
`codex/android-player-journey@e1a19ddd59cd8aaf634ad22afbfaec000acbf165`.
Both Android package scripts now accept
`MIR2_ANDROID_ENTITY_RELEASE_MANIFEST`. When supplied, package success is
withheld until the scripts extract the completed APK's embedded lock and
`verify-entity-release-alignment.mjs` proves that it and the selected Web
manifest have the same pack ID, exact
manifest bytes/SHA-256 and aggregate atlas/page/rect/page-byte counts.

The manifest input may be a local file or a public HTTPS URL. Remote inputs
reject credentials, query data, fragments, redirects, non-200 responses,
oversize bodies and a 15-second timeout. The verifier downloads no atlas pages,
does not mutate a deployment and does not approve a release by itself.

## Verification

- Node verifier tests pass 6/6, covering an exact match, same-count content
  drift, aggregate drift, pack-ID drift, non-HTTPS input and query-data input
  (`node-tests.txt`).
- Node syntax, Bash syntax, Rust formatting and Android 153/153 pass. PowerShell
  parsing was not run because `pwsh` is not installed on this Mac
  (`local-gates.txt`).
- The tracked Android pack lock aligns with the exact tracked Web manifest:
  SHA-256
  `2ae6fb0dfe626bc90b41caac027a1fa219aaa11471ac27e42cc9980c286f48bd`,
  3,347,237 manifest bytes, one atlas, seven pages, 10,482 rects and
  13,249,890 declared page bytes (`tracked-local-alignment.json`,
  `tracked-manifest-summary.json`, `tracked-manifest-sha256.txt`).
- The API31 `uiPreview` build passes with the exact ignored 35-atlas local
  manifest as its alignment input. The unchanged 375,663,860-byte APK has
  SHA-256 `f420968ea3ec633649bc0dd940594eb7fedeb12f5463af7c7bd2f1f8da20e10b`
  and the verifier input byte-for-byte matches its embedded lock
  (`build-android-output.txt`, `apk-package.txt`, `local-gates.txt`).
- The previous head's GitHub `Android Capacitor and native Bevy` lane completed
  successfully (`previous-head-ci.json`).

## Public release observation

Repeated read-only HTTPS reads agreed that the current public
`https://mir2.obelisk.build/bevy-entity-atlases/manifest.json` has SHA-256
`254b0e2cd422dd6e6b1ce37ecce2a25cbca060b7fc69698cc51583f7816ef1fa`,
one atlas, seven pages, 9,650 rects and 13,164,823 declared page bytes. It is not
the tracked manifest above, and the new verifier rejects it
(`public-manifest-sha256.txt`, `public-manifest-summary.json`,
`tracked-public-mismatch.txt`). One separate HEAD request encountered a
transient LibreSSL `SSL_ERROR_SYSCALL`; the successful complete GETs and the
two differing content hashes are the acceptance evidence, not that HEAD.

## Acceptance boundary

No production state was changed. The mismatch means the current public Web
manifest and this checkout's tracked Android input are explicitly not accepted
as one immutable release. The asset-release owner must identify or publish the
approved manifest and version before rerunning this gate. The local proof pack,
APK and caches remain ignored and uncommitted. This adds release-integrity and
package evidence only; it does not add real login, live map transition,
physical-device, emulator render-ready, soak or human acceptance.
