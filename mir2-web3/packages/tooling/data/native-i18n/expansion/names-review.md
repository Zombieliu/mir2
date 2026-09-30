# Display-name expansion review — 2026-09-30

`native-i18n-expansion-names.mjs` emits explicit Arabic and Indonesian creature/item/map display names. It expands only source suffix variants already found in the canonical catalogue; numeric variants, gender/class/size codes and IDs remain intact. It skips fields owned by the full-content review. `other-names-overrides.json` separately records Russian, Hindi, Vietnamese and Thai corrections.

Context checks distinguish Tinker fish (tench) from a craftsman, Currish as a hostile dog rather than a spelling-only unchanged draft, BoneWhoo as the skeleton leader, DY workshop/smithy location labels, and Keel Archer as a dragon-bone archer. These are presentation overrides; map keys, NPC/script identities, monster indices, item indices, assets and commands do not change.

Remaining `offlineNameDraft` values in `extra/provenance.json` are explicitly machine drafts. The generation tool preserves source codes and rejects lost numbers/parameters, but those checks cannot certify idiomatic or genre-appropriate names. Native-speaker review remains outstanding.
