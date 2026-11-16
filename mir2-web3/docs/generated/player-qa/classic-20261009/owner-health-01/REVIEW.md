# Read-only review and root resolutions

Root is the sole code writer. Existing read-only producer and routing reviewers
audited pool producers, final validation, private transactions and teardown.
Their findings were resolved without weakening original Source assertions:

- Source spell and melee acceptance must be observed before live-channel delivery.
- Repository transactions cannot run while holding the shared Zone writer.
- Recovery applies a confirmed delta to current pools; old death/life/admission
  and old map registrations cannot authorize a newer actor's health or viewport.
- Key-based native potions/scrolls use the same bridge as ordinary packet UseItem.
- A corpse can have positive HP after LevelUp while its independent Dead remains
  true. A concurrent new death discards an old refill but retains committed stats.
- Teardown XP drains require their runtime-only exact fence generation; release,
  replacement, cleanup and cold reconstruction cannot reuse that capability.
- Mentor credits use persisted Source results before the same health bridge.
- Actor graduation reads its confirmed Source level even during fenced teardown;
  online peers use their current identity-checked experience profile. Store
  graduation validates the same captured levels, preserving epoch/rollback
  guards without writing peer level, XP or revision. Presence-generation
  changes force MentorUpdate even after durable credit-state reconciliation.
- NoLive and Closed-at-cap paths retain normal lifecycle/control packets for the
  personal Source drain while discarding expired display-only capabilities.
- Actual TownRevive commits before subsequent reward reconciliation. Every
  metadata tail preserves current Zone pools, including damage in the new life.

Final root tests cover the last two review findings. This does not certify a
universal ordered Source NPC outbox, all health recipients, a natural crowd
escape story, a native graphical login or full P1-P7 acceptance.
