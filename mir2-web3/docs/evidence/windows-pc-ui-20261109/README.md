# Windows PC UI evidence

Scope and tests: [integration report](../../PC-UI-INTEGRATION-20261109.zh-CN.md).

- `pc-ui-native-login-login-1794172945740-1.png`: actual Windows development executable, Simplified Chinese login screen, empty account/password, no wallet request or authentication. Captured at 2026-11-08T21:22:25.782Z, then closed normally. See `native-login-capture.json` and `native-login-smoke-receipt.json`.
- `wallet-modal-zh-CN-offscreen-fixture.png`: actual native UI renderer with fixed dummy QR payload, no window or wallet authentication.
- `hud-simulation-fixture.png`: actual native HUD renderer, synthetic HP/MP, empty world, no gameplay or authentication.

The native capture is not eligible for signed-release acceptance because trusted build provenance is unavailable. No published package, production service, saved account, or historical candidate was changed. Images must not be described as a completed visual match to the latest design rendering.