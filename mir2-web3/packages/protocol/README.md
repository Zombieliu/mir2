# packages/protocol

This package holds the Crystal-wire MVP protocol layer for the new stack.

## Current Files

- [crystal-mvp-v1.md](/E:/mir2/mir2-web3/packages/protocol/crystal-mvp-v1.md)
  - human-readable Crystal MVP protocol extraction
- [crystal-mvp-v1.json](/E:/mir2/mir2-web3/packages/protocol/crystal-mvp-v1.json)
  - machine-readable first-pass protocol manifest
- [Cargo.toml](/E:/mir2/mir2-web3/packages/protocol/Cargo.toml)
  - Rust crate manifest
- [src/lib.rs](/E:/mir2/mir2-web3/packages/protocol/src/lib.rs)
  - crate entry and public exports
- [src/frame.rs](/E:/mir2/mir2-web3/packages/protocol/src/frame.rs)
  - Crystal frame encode/decode (`u16 length + i16 packet_id + payload`)
- [src/io.rs](/E:/mir2/mir2-web3/packages/protocol/src/io.rs)
  - little-endian reader/writer and `.NET BinaryWriter` string support
- [src/types.rs](/E:/mir2/mir2-web3/packages/protocol/src/types.rs)
  - shared enums and MVP structs
- [src/packets.rs](/E:/mir2/mir2-web3/packages/protocol/src/packets.rs)
  - MVP client/server packet enums plus codec helpers
- [tests/codec.rs](/E:/mir2/mir2-web3/packages/protocol/tests/codec.rs)
  - codec smoke tests

## Implemented Scope

The Rust crate currently covers:

- frame encode/decode
- `.NET BinaryWriter` UTF-8 string codec
- shared enums:
  - `MirGender`
  - `MirClass`
  - `MirDirection`
  - `ChatType`
- shared structs:
  - `SelectInfo`
  - `Point`
  - `MapInformation`
  - `UserLocation`
  - `ObjectMovement`
- client packet codec:
  - `ClientVersion`
  - `Disconnect`
  - `KeepAlive`
  - `NewAccount`
  - `Login`
  - `NewCharacter`
  - `StartGame`
  - `LogOut`
  - `Turn`
  - `Walk`
  - `Run`
  - `Chat` with `linked_item_count = 0`
- server packet codec:
  - `Connected`
  - `ClientVersion`
  - `Disconnect`
  - `KeepAlive`
  - `NewAccount`
  - `Login`
  - `LoginBanned`
  - `LoginSuccess`
  - `NewCharacter`
  - `NewCharacterSuccess`
  - `StartGame`
  - `StartGameBanned`
  - `StartGameDelay`
  - `MapInformation`
  - `UserLocation`
  - `ObjectTurn`
  - `ObjectWalk`
  - `ObjectRun`
  - `Chat`
  - `ObjectChat`
  - `LogOutSuccess`
  - `LogOutFailed`

## Known Gap

`UserInformation` is currently preserved as raw payload bytes rather than fully decoded.

That is intentional for the first landing. It keeps the crate usable for handshake and map-entry work while we extract the large nested bootstrap payload carefully from Crystal.

## Service Contract Direction

Post-1:1 modernization should keep two protocol layers separate:

1. External client protocols:
   - Web stays WebSocket first.
   - Crystal compatibility stays TCP trace/codec harness.
   - Future native/desktop/mobile can add TCP/KCP/QUIC after gateway/session routing is stable.

2. Internal service contracts:
   - Short term: typed Rust traits and in-process calls.
   - Medium term: gRPC + Protobuf for account, character, mail, admin command, and zone-routing boundaries.
   - Event stream: explicit event envelopes before Redpanda becomes required.

Do not turn every module into a network service before in-process boundaries are clean. Start by defining stable request/response and event shapes, then split transport when scale or ownership requires it.

## Next Step

The next concrete protocol modernization task is:

1. keep Crystal TCP packet trace compatibility green
2. define internal service boundary names and request/response shapes
3. add Protobuf contracts only after those shapes stabilize
4. keep JSON/debug surfaces for smoke tests and admin tooling
