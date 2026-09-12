# Native Android overlay retention evidence (2026-09-12)

Scope: offline `uiPreview` APK on the API 31 `Mir2_API_31_ARM64` emulator. This is synthetic UI and memory evidence, not real login, live gameplay, or physical-device acceptance.

## Result

The shared Bevy overlay renderer now retains its entity trees until an actual render input changes. The per-frame guild clock and input-only fields are excluded from the render fingerprint.

Five-second native heap / total PSS samples (KiB):

- inventory: `181980/273191 -> 172624/262586`
- gameshop: `187052/277708 -> 175868/265483`
- storage: `189000/280446 -> 179288/269675`
- guild: `185424/276613 -> 175956/266091`
- trade: `179308/270707 -> 176256/266586`
- help: `190180/281076 -> 175544/265400`
- mail-compose: `179600/270477 -> 176300/266066`

The 30-second Game Shop soak stayed flat: `178544/267567 -> 178540/267762` KiB. Screenshot: `gameshop-after-2s.png` (SHA-256 `7fa8ae79121db4f7b4442a81741f58c4b25a3e67f2e73a825789567ea90b016b`).

APK build SHA-256: `6c1351270e9fe5022db4c9450eba9f416c6ad91749e482bcf0017cbf72739b29`. The APK itself is intentionally not committed.

## Verification

- `cargo +1.95.0 test --manifest-path apps/game-client/client-bevy/Cargo.toml --features native-player-ui unchanged_ --locked --offline`: 4 passed.
- Android release `cargo-ndk` build for `arm64-v8a`, API 31: passed.
- `:app:assembleUiPreview` and streamed emulator install: passed.

