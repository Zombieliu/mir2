# NPC six-language override review

Scope: the `npc.*` keys in `identicalNeedsReview` from the v2 keyed review, excluding every key also present in `errors`. This is a bounded authored override layer; it does not claim that the rest of the machine-translated NPC corpus has passed semantic or native-speaker review.

## Coverage and ownership

- 270 of 270 eligible unique keys: 211 menu labels and 59 prose lines.
- Six explicit fields per key: `ru`, `hi`, `id`, `vi`, `th`, `ar`; 1,620 values.
- Missing eligible keys: 0. Keys outside this bounded set: 0.
- 108 entries reuse reviewed content terminology or exact content name keys, with explicitly retained level, floor, class-label, or region suffixes. The other 162 entries were authored for their NPC context.
- Existing source catalogues, source aliases, action targets, protocol identifiers and translation generators were not modified. The completed 969-key UI override layer remains unchanged.
- The following four overlapping keys are intentionally owned by the separate interpolation-error worker and are absent here: `npc.menu.0f442cdee2efeda4`, `npc.menu.b4ea8c0495ac4491`, `npc.menu.b6c33f2c1ee43f1d`, `npc.prose.e58840a3d1d0af97`.

## Source and terminology checks

The v2 review input is `C:/mir2-build/i18n-expansion-drafts-v2/keyed-review.json`, SHA-256 `2ae5107a2a778a67830e2425193adad020c3904ff485b30ae4424d37f7524771`.

Reviewed terminology is read from `native-i18n-expansion-content.mjs` and `content-overrides.json`. The reference versions used have SHA-256 `d9cc98cf969cc8e4cd224aa25f059529e93b66fa01a9ba1cfd8da9c54e917bf3` and `aa26ada93a11a2ae8baec618e43543047d7e65c59535ca029da2ec3354a360b9`, respectively. Content was copied by exact stable magic key or exact normalized name/alias, not by fuzzy matching. Numeric suffixes are copied intact.

- `ElecShock` is the legacy MirGuide spelling of canonical `ElectricShock`; `LionsRoar` refers to `LionRoar`. The MirGuide section targets and existing source catalogue confirm these mappings.
- `UltimateEnhancer` maps through `content.magic.UltimateEnhancer.name`; the name text itself has an old `Enchancer` typo. It is not confused with `PetEnhancer`.
- `VamnpireShot (Level 26)` uses the established `VampireShot` name and keeps level 26.
- `Thunderbolt 17` uses `ThunderBolt`, not `Lightning`. Their strike/beam names remain distinct.
- `TownTeleport`, `Townteleport`, and `RandomTeleports` occur in peddler supply menus; their scroll names are appropriate here. No teleport actions or command targets were altered.
- `Boot` occurs after Buy/Sell/Repair in `GM/GM-Boot`, so it means footwear. `legal` is an adjective inside the administrator's explanation of legal guild wars. `Pass` is an action in the Fox Cave rock passage. These were checked against the generated authored NPC script text.
- The L/R prose slots originate from `<$RING_L>` and `<$RING_R>`. Each translation labels the side and preserves both `L`/`R` and each opaque argument exactly once.

## Intentionally retained source identities and limitations

31 whole values remain equal to their English source, all explicitly classified:

- `npc.menu.580ce9e8774306d7`: `HwanMaJin2`, six languages; a proper map identifier, including the exact suffix 2.
- `npc.menu.d179bb77a0f6f16d`: `HwanMaJin`, six languages; a proper map identifier without an established reviewed localized name.
- `npc.prose.0839dfaa5145dad0`: `OldManInfinity.`, six languages; the NPC identity.
- `npc.prose.596ecb8e1b5e7dba`: `-Liam Thompson`, six languages; a memorial attribution/person's name.
- `npc.prose.9263fdd602313131`: `(KR)`, six languages; a region code.
- `npc.prose.b4c54a0402b89cff`: Indonesian `Level 48: CelestalShield`; `Level` is ordinary Indonesian UI usage and the remainder is the exact old skill identifier. This is not a machine fallback.

`CelestalShield` appears in the authored bookshop list at level 48 but is absent from the current 109 canonical magic identifiers. The surrounding level label is translated, and the legacy name is preserved rather than guessed to be MagicShield, SoulShield or EnergyShield. Resolving the old gameplay documentation is outside this translation-only patch.

Other source identities retained inside translated prose are `HolySword`, `LostSoul`, `MineralMine`, `VisceralWorm`, `SpittingSpider`, `ShellNipper`, and `Keratoid`. The available curated content override layer does not establish reviewed six-language names for these identities. The surrounding prose is authored in each language; this does not claim complete localization of those embedded content names. `Far`, `Prajna`, `Seokcho`, `Sabuk`, `Zuma`, `Wooma`, `Woomyon` and `Mongchon` retain their proper-name spelling where appropriate; generic geography is translated.

`IceHellTemple(S` has a missing closing parenthesis in the authored source. Its `(S` suffix is kept exactly, while the temple name is translated. N/S, KR, B1/B2 and 1F–5F codes are not converted to different directions or floor conventions.

Several authored SAY lines are sentence fragments completed by adjacent lines/menu links, including the HolySword chronicle sentence, the bones/fear quotation, and “of the village”. They remain fragments rather than inventing missing claims. The memorial date `05/05/2023`, time `4:12pm`, and `EST` are unchanged; no timezone conversion or biographical inference was made.

The legacy GM prose `Tip: Use @superman ...` is translated faithfully, with the literal command retained. This patch only changes an existing display catalogue; it does not enable a command or assert that the page is reachable to ordinary players. The source extraction includes gated GM-prefixed scripts referenced by NPC records.

## Validation and remaining acceptance

An offline validation compared the output against the exact 270-key eligible set and found zero errors for:

- exactly six nonempty string fields and no unknown keys;
- exact brace-placeholder multisets and numeric-value multisets;
- exact URLs, literal `@` commands, floor codes, HP/MP/DC/MC/SC abbreviations, date/time zone and authored direction/region suffixes;
- absence of inserted Unicode format/bidi-control characters, SentencePiece markers, or protected-token numeric sentinels.

All 1,620 values were supplied explicitly or derived only by exact reviewed-name substitution with preserved suffixes. No network translation, account login, server operation, Cargo build, GPU capture or installer operation was run for this task. Arabic shaping, bidi placement around coordinates and opaque names, Thai/Devanagari wrapping, and native-speaker language quality still require the parent's integrated visual and linguistic acceptance. No such acceptance is claimed here.

NPC override SHA-256: `d93e01e1b43a97c4bcbb428ecaccc9dee89af1ce4edf6927120d5afa23233e34`.

Frozen UI checksums remain `0b09bbc174431594226498efbacf51270d920f3868b367665fc5c3ae55e0ea86` (`ui-overrides.json`) and `6f9019da33e7bae1a34befdef0b425ae59e2c30af0c3c816860325fb7aebc433` (`ui-review.md`).
