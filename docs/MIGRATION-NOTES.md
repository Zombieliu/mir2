# Migration Notes

## Crystal Reference Areas

- `Client`
  - rendering, scenes, UI flow, packet usage
- `Shared`
  - enums, packet definitions, shared data objects
- `Server`
  - gameplay behavior and authoritative rules

## Early Extraction Targets

- packet ids
- packet serialization rules
- map identifiers
- character summary data
- chat and movement messages

## Avoid In Phase 1

- full combat parity
- full inventory parity
- all NPC scripting
- on-chain realtime state
