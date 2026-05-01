# apps/web

Next.js host shell for the first visual client checkpoint.

Responsibilities in this slice:

- open a WebSocket session to the local gateway
- send login / start-game commands
- project packet payloads into a small world snapshot
- feed that snapshot into the Bevy WASM runtime
- render HUD, event log, and quick controls around the canvas

Gateway WebSocket configuration:

- local default: `ws://127.0.0.1:7110/ws`
- staging/hosted: set `NEXT_PUBLIC_MIR2_GATEWAY_WS_URL`
- if unset outside localhost, the client falls back to same-origin `/ws`

Local flow:

1. build the WASM runtime with `npm run runtime:build:dev`
2. start the web shell with `npm run dev`
3. open the local page and click `Quick Enter`
