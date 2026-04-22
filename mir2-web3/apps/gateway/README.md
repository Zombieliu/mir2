# apps/gateway

Rust gateway workspace for the rewrite.

## Current Status

The gateway now fronts a separate `simulation` crate for authority logic, but it
is still an early local dev gateway rather than the final production service.

Current implementation files:

- [Cargo.toml](/E:/mir2/mir2-web3/apps/gateway/Cargo.toml)
- [src/main.rs](/E:/mir2/mir2-web3/apps/gateway/src/main.rs)
- [src/session.rs](/E:/mir2/mir2-web3/apps/gateway/src/session.rs)
- [src/bin/smoke.rs](/E:/mir2/mir2-web3/apps/gateway/src/bin/smoke.rs)

## What It Does

- accepts Crystal-framed TCP packets
- exposes HTTP + WebSocket for browser manual smoke
- sends `Connected` on socket accept
- handles:
  - `ClientVersion`
  - `Login`
  - `NewCharacter`
  - `StartGame`
  - `Turn`
  - `Walk`
  - `Run`
- `Chat`
  - `KeepAlive`
  - `LogOut`
- forwards command handling into `apps/simulation`

## Bootstrap Sequence

The current local stub emits:

1. `StartGame { result = 4 }`
2. `MapInformation`
3. `UserInformation` as raw payload bytes
4. `UserLocation`
5. `Chat` welcome line

## Important Limitation

This is not yet a drop-in Crystal-compatible gameplay server.

Known gaps:

- no persistence
- no real account validation
- no AOI or entity streaming
- `UserInformation` is manually encoded for a minimal bootstrap only
- no `ObjectPlayer`/`ObjectMonster`/`ObjectNPC` feed yet
- no Sui integration yet

## Intended Near-Term Use

This gateway exists so the rewrite can validate:

- the Rust protocol crate
- the Crystal packet order we actually need
- a deterministic login/start-game smoke path
- the boundary between transport and authority

## Planned Next Step

After this step, the next gateway work is:

1. add richer browser-side smoke assertions
2. replace raw bootstrap payloads with typed decode/encode paths
3. move from single-session local state to multi-session simulation integration

## Local Run

Use non-default ports if `7000` or `7010` are already occupied on your machine.

Example:

```powershell
$env:MIR2_GATEWAY_TCP_ADDR='127.0.0.1:7100'
$env:MIR2_GATEWAY_WEB_ADDR='127.0.0.1:7110'
cargo run -p mir2-gateway --bin mir2-gateway
```

Manual browser surface:

- [http://127.0.0.1:7110/](http://127.0.0.1:7110/)

Health check:

- [http://127.0.0.1:7110/health](http://127.0.0.1:7110/health)

Scripted TCP smoke:

```powershell
$env:MIR2_GATEWAY_TCP_ADDR='127.0.0.1:7100'
cargo run -p mir2-gateway --bin smoke
```

## Current Human Test Path

In the browser page:

1. click `Connect`
2. click `Send ClientVersion`
3. click `Login`
4. click `Start Game`
5. click movement buttons and confirm `UserLocation` / `ObjectRun` style events appear
6. send chat and confirm both `Chat` and `ObjectChat` appear in the event log

The current verified local smoke sequence is:

- `Connected`
- `ClientVersion`
- `LoginSuccess`
- `StartGame`
- `MapInformation`
- `UserInformation`
- `UserLocation`
- `Chat`
- movement update
- chat echo
