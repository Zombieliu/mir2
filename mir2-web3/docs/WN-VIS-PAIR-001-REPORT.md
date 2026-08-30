# WN-VIS-PAIR-001 — deterministic original/native evidence gate

Status: **pair gate plus native/original atomic capture code passed; live captures, model review and human acceptance remain open**

Date: 2026-08-24

## Outcome

The repository now has a fail-closed evidence gate for pairing a Crystal original
capture with a Windows-native capture before asking Gemini for a visual review.
This closes the previous tooling gap where arbitrary image pairs, an absent local
schema, malformed provider output, or a 24-byte pseudo-PNG could be treated as
reviewable evidence.

It does **not** prove the Windows client is visually Accepted. No desktop client
was launched, no new original/native image was captured, no external model was
called, and no human visual/feel sign-off was performed in this code-only run.

## Implemented gates

- `apps/web/scripts/verify-native-visual-pair.mjs`
  - requires `mir2-native-visual-capture-v1` sidecars from exact producers
    `crystal-original` and `windows-native`;
  - binds each sidecar to the exact PNG path and SHA-256;
  - verifies a complete non-interlaced RGB/RGBA PNG stream, chunk CRCs, IDAT
    decompression, row filters, and canonical `1024x768` dimensions;
  - requires the same run id, scene, UI state, DPI and a capture delta no greater
    than five minutes;
  - for world scenes, requires exact map, x, y and light equality;
  - binds the Gemini report to ordered reference, candidate and pair-context
    evidence plus the tracked review-schema SHA-256;
  - requires an identified Gemini/Antigravity provider, `sameScene=true`, scene
    confidence `>=0.90`, no scene blockers, no P0/P1 issues and the scene score
    threshold (`90` login/select, `92` world scenes by default).
- `tools/antigravity-visual-review/review.mjs`
  - loads, parses and hashes the tracked schema during self-test and real review;
  - validates the complete closed output contract;
  - extracts only the provider's primary response and rejects provider error
    status instead of recursively accepting an unrelated nested object;
  - records the schema SHA-256 in `review.json`.
- `apps/game-client/platform-windows/src/capture.rs`
  - replaces the asynchronous `save_to_disk` helper with a Bevy 0.19
    `ScreenshotCaptured` observer;
  - freezes the request-time scene, safe UI visibility state, DPI, map and
    `SelfPlayer` coordinates before GPU readback;
  - atomically writes and fsyncs the PNG, hashes the final bytes, and only then
    atomically writes the sidecar;
  - never serializes account/password/chat/draft text;
  - emits `mir2-native-visual-capture-draft-v1` with explicit blockers while
    light or trusted build provenance is unavailable. It cannot be mistaken for
    an acceptance-eligible v1 sidecar;
  - reads effective light only from the latest matching-map native lighting
    bridge and cross-checks the actual running EXE against packaged
    `VERSION.json` plus `PACKAGE-MANIFEST.json`; development or stale packages
    remain draft rather than accepting environment-supplied digest claims.
- `apps/gateway/src/bin/crystal_original_capture_relay.rs`
  - is a loopback-only, transparent Crystal TCP relay: client-to-server bytes
    are forwarded opaquely and are never decoded or persisted;
  - forwards server frames byte-for-byte while observing only authoritative
    `StartGame`, map, self-position and lighting packets;
  - emits an atomic, heartbeat-refreshed
    `mir2-crystal-original-state-evidence-v1` artifact with one connection id,
    monotonic packet sequence numbers, exact packet ids and frame SHA-256s;
  - fails closed on logout, disconnect, missing/invalid map light, stale world
    state or a non-loopback bind/upstream;
  - has an end-to-end loopback regression proving both directions are forwarded
    exactly while opaque client credential bytes never enter evidence JSON.
- `apps/web/scripts/capture-original-visual-pair.{ps1,mjs}`
  - captures exactly one `1024x768` original-client area without focusing the
    window or injecting input, and records only process/window geometry, DPI,
    executable identity and PNG metadata as observed facts;
  - requires an explicit original process id for strict mode, avoiding ambiguous
    selection when original and native windows share the same title;
  - never derives map, coordinates, lighting or UI state from pixels/window
    inspection; world-scene claims must come from the relay while scene and the
    exact native visibility-only UI-state grammar remain explicit operator
    attestations;
  - supports strict Login and Character Select captures with `world=null`, while
    InGame/quest/combat scenes require the relay evidence path;
  - reads relay evidence both before and after the screenshot and only promotes
    to strict v1 when the connection, world state and all authoritative packet
    frame fingerprints remained unchanged across capture;
  - independently hashes the observed executable and declared asset manifest,
    checks their real byte lengths, and uses the content-addressed Crystal binary
    revision `crystal-original-artifact-<exe-sha256>` instead of accepting a
    manually entered source digest;
  - otherwise writes only `mir2-native-visual-capture-draft-v1` with explicit
    blockers. A failed strict request cannot silently become acceptance evidence.
- `apps/web/scripts/build-crystal-original-asset-manifest.mjs` and
  `prepare-original-visual-evidence.mjs`
  - enumerate only explicitly selected original-client asset directories with
    stable slash paths and canonical per-file/content-root SHA-256 bindings;
  - reject traversal, overlapping includes, symlink/reparse paths, output inside
    the source tree and every overwrite attempt;
  - let the real Crystal run include `Data`, `Map`, `Localization` and `Sound`
    while excluding mutable logs, key bindings and private INI configuration;
  - hash the actual observed `Client.exe` and manifest bytes and derive the
    source revision as `crystal-original-artifact-<exe-sha256>`; no operator can
    supply a revision or digest by hand.
- Model success is deliberately recorded as
  `READY_FOR_HUMAN_ACCEPTANCE`, never final `ACCEPTED`. The manifest keeps
  `humanAcceptanceRequired=true`, `humanAccepted=false` and `passed=false` until
  the separate human gate is completed.

## Low-load verification

```text
node --test --test-concurrency=1 tools/antigravity-visual-review/review.test.mjs
  16 passed; 0 failed

node --test --test-concurrency=1 apps/web/scripts/test-native-visual-pair.mjs
  6 passed; 0 failed

node tools/antigravity-visual-review/review.mjs --self-test
  ok=true; schemaSha256=322a4efc36b563e249c3e0079f28643f2b2af6a2a8edd81f5bdb206bd9170dd2

git diff --check -- <scoped visual-review files>
  exit 0

cargo +1.95.0 test --manifest-path apps/game-client/platform-windows/Cargo.toml \
  capture:: --jobs 1 -- --test-threads=1
  15 passed; 0 failed; 297 filtered out

cargo +1.95.0 test --manifest-path apps/game-client/platform-windows/Cargo.toml \
  capture_light_state_requires_matching_map_and_preserves_dark_override \
  --jobs 1 -- --test-threads=1
  1 passed; 0 failed; 311 filtered out

node --test --test-concurrency=1 apps/web/scripts/test-original-visual-pair.mjs
  7 passed; 0 failed

node --test --test-concurrency=1 \
  apps/web/scripts/test-build-crystal-original-asset-manifest.mjs \
  apps/web/scripts/test-prepare-original-visual-evidence.mjs
  10 passed; 0 failed

cargo +1.95.0 test --manifest-path apps/gateway/Cargo.toml \
  --bin crystal_original_capture_relay --jobs 1 -- --test-threads=1
  6 passed; 0 failed

PowerShell parser check: apps/web/scripts/capture-original-visual-pair.ps1
  passed
```

## Remaining acceptance work

1. Generate the first real Crystal asset manifest and same-run content-addressed
   evidence files from the exact original client installation used for the baseline.
2. Build a fresh packaged Candidate containing `PACKAGE-MANIFEST.json`, then
   capture Login, Select, InGame and core-panel pairs from one deterministic
   run; run Gemini scoring against those exact hashes.
3. Complete the final human 20-minute visual and input-feel acceptance. Model
   review remains a defect classifier and cannot substitute for this signature.
