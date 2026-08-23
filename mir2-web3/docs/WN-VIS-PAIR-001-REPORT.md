# WN-VIS-PAIR-001 — deterministic original/native evidence gate

Status: **code gate passed; live capture provenance and human acceptance remain open**

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
```

## Remaining acceptance work

1. Replace the Windows `save_to_disk` screenshot observer with an authoritative
   `ScreenshotCaptured` observer that atomically writes PNG then sidecar from one
   frozen runtime state.
2. Add the Crystal original capture producer with equivalent run/scene metadata.
3. Bind executable/source/asset provenance to real build artifacts rather than
   trusting manually entered digest strings.
4. Capture fresh Login, Select, InGame and core-panel pairs from one deterministic
   run; run Gemini scoring against those exact hashes.
5. Complete the final human 20-minute visual and input-feel acceptance. Model
   review remains a defect classifier and cannot substitute for this signature.
