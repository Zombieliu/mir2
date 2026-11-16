# Shared primary-hit magic practice — 2026-09-15

FireBall and GreatFireBall omitted Crystal's positive-hit `LevelMagic` bridge in
the shared world. The bounded owner bridge now handles those two spells and
SoulFireBall. It uses actual primary-target HP loss, existing 1–3 XP/multiplier,
already-learned skills and character-level requirements. Spell-aware replay fences
preserve legacy SoulFireBall checkpoint behavior. No historic XP, skill grants or
damage changes are included.

Coordinator source/log hashes match [verification-summary.json](verification-summary.json).
Simulation **9/9**, Gateway **13/13** and multiplier **1/1** pass. Tests cover normal
hit-driven upgrades, logout/save/reload, subsequent public casts, owner/life/map
changes, zero damage and replay. The successful release build and isolated R54
integration are recorded in [r54-integration.json](r54-integration.json).

Natural R53 Taoist SoulFireBall progression reached 193 XP and persisted at revision
8290 after normal logout. Natural R54 FireBall/GreatFireBall evidence is pending.
Projectile timing is unchanged; the earlier distance/delay difference is not closed
by this practice fix. This is not complete skill, journey or native visual acceptance.
`visualAccepted=false`, `globalParityPercent=null`.
