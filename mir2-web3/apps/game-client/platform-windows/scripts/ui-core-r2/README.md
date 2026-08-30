# WN-UI-CORE-02 live UI evidence harness

`ui-core-r2-live-e2e.ps1` is a reproducible Windows-native UI harness for the
six requested flows:

- Change Password
- Delete Character confirmation
- Safe Key
- Mail
- Shop
- Storage

It uses Win32 `SendInput` mouse/keyboard events against the native client window
and captures the resulting client surface as PNG. It does not open a WebSocket,
send `BrowserCommand` JSON, or fabricate a server reply.

## Run

From the repository root:

```powershell
powershell -ExecutionPolicy Bypass -File .\apps\game-client\platform-windows\scripts\ui-core-r2\ui-core-r2-live-e2e.ps1 -Stage Preflight
powershell -ExecutionPolicy Bypass -File .\apps\game-client\platform-windows\scripts\ui-core-r2\ui-core-r2-live-e2e.ps1 -Stage LoginShell
powershell -ExecutionPolicy Bypass -File .\apps\game-client\platform-windows\scripts\ui-core-r2\ui-core-r2-live-e2e.ps1 -Stage CharacterSelect
powershell -ExecutionPolicy Bypass -File .\apps\game-client\platform-windows\scripts\ui-core-r2\ui-core-r2-live-e2e.ps1 -Stage InGame
```

Use `-LaunchClient` only when the client is not already running. The default
EXE is `apps/game-client/platform-windows/target/release/mir2-platform-windows.exe`.
Evidence is written below `docs/generated/player-qa/native-ui-controls/r2-live/<UTC-run-id>/`.

## Safety and limitations

- The script never logs password, token, passkey, secret, or authorization values.
- Change Password deliberately stops after opening the form; the final submit is
  a human hand-off.
- Delete Character captures the confirmation modal but does not click the
  destructive action by default. `-ConfirmDelete` only records that an explicit
  destructive gate was requested; a human still performs the final click.
- No unrelated gateway/browser/client process is killed. `-StopClient` is kept
  as a visible no-op safety flag so a copied command cannot accidentally kill a
  process.
- The HUD coordinates are the registered 1024×768 logical-stage coordinates.
  The harness scales them to the actual client area and fails preflight when the
  stage is materially smaller.
- Mail/Shop/Storage capture proves mouse entry into the panel. It does not make
  server mutations or claim/delete/buy/sell/deposit/withdraw items.
