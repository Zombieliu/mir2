# Taoist R125 second revival proof

The bounded trace window is sequences 16840-16848 (`2026-09-15T20:29:58.040Z` through `20:29:58.546Z`) for player object `1000`.

Sequence 16840 is a world snapshot at D2031 with exact self HP 4/180 and `dead:false`. It is immediately superseded in the live observation path by the authoritative `Death` packet at sequence 16841, followed in the same millisecond by `ObjectStruck` (attacker 339300), `DamageIndicator` damage 4, `ObjectHealth percent:0`, and `ObjectDied` at sequence 16846. The runner sent `townRevive` only at sequence 16848, 16 ms after the death packet and after the death lifecycle receipts.

The reducer semantics support this order: `Death` sets the self entity to `dead:true`, exact HP 0, and exact health observation; `ObjectDied` independently marks the same object dead with HP 0. `ObjectHealth percent:0` alone is rounded and is not the death authority, but it followed the explicit `Death` receipt here. Therefore the last pre-death world snapshot is stale for the revive decision; this was an actual authoritative death, not a false revival caused by rounded zero or an unrelated object. No source, process, store, or repository state was changed.
