# apps/simulation

Authoritative simulation core for the rewrite.

## Current Status

This is the first landed authority crate behind the gateway.

Current implementation:

- [Cargo.toml](/E:/mir2/mir2-web3/apps/simulation/Cargo.toml)
- [src/lib.rs](/E:/mir2/mir2-web3/apps/simulation/src/lib.rs)

## What It Owns Right Now

- world/session bootstrap state
- deterministic character list
- `StartGame` bootstrap packet sequence
- movement authority for `Walk` and `Run`
- chat echo and object chat broadcast

## Why This Exists

The gateway should not remain the long-term owner of gameplay state.

This crate is the first step toward the final split:

- `gateway`
  - transport, auth, wallet, session edges
- `simulation`
  - authoritative world state and gameplay rules

## Current Gap

This is still a lightweight authority core, not a Bevy ECS world yet.

The next simulation step is to move from this single-session deterministic state
into:

1. world instance state
2. entity collections
3. broadcastable object feeds
4. later Bevy or `bevy_ecs`
