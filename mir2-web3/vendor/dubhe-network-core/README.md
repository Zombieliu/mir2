# Dubhe Network Core

`dubhe-network-core` is the game-independent boundary extracted from the first
Mir2 integration. It defines the signed contracts shared by a chain control
plane, official relays, community Home Nodes, telemetry services, and reward
settlement.

Included:

- Ed25519 node identity and replay protection
- Sui-finalized node registration and capacity certificates
- signed enrollment challenges and node bundles
- CGNAT-safe reverse-tunnel registration and stream authorization
- signed sandbox manifests and runtime attestations
- signed agent releases, resource policies, and update state
- privacy-preserving node telemetry and regional aggregation
- multi-game verified-work receipts and reward settlement
- production-beta plans, journals, and acceptance evidence

Excluded by design:

- Mir2 packets and Crystal assets
- map simulation and gameplay rules
- player accounts, inventories, databases, and product UI
- transport-specific process supervision

The wire schemas keep their existing `obelisk.*.v1` names so deployed Home
Nodes remain compatible. Mir2 is one adapter identified by `game_id = "mir2"`;
other games may use the same contracts without depending on Mir2 code.
