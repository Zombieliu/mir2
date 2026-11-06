# Six-language UI expansion review

Date: 2026-09-30. Owner: playtest_protocol. This review and `ui-overrides.json` are the only files written in this assignment. Existing catalogs, canonical IDs, map filenames, gameplay, server authority, player stores and input handling were not modified.

## Scope and acceptance limit

The English source of all **969 UI-domain entries** was read for gameplay and presentation risks. This is source review, not native-speaker approval of all translated UI text. The override file now contains **969 stable keys × six locales = 5814 authored values**, covering all four UI domains. No UI-domain key depends on machine-draft fallback. Technical tokens, customary Indonesian loanwords and explicitly documented proper names remain unchanged by design. Field coverage is not a claim that every proper name has a new localized form. None of these counts prove native-speaker or complete in-game language acceptance. No Cargo, GPU, window, network, account, build, server or installer actions were run by this worker.

| Domain | Source entries reviewed | Authored overrides | Remaining baseline keys |
|---|---:|---:|---:|
| shell | 165 | 165 | 0 |
| quest | 398 | 398 | 0 |
| game | 282 | 282 | 0 |
| help | 124 | 124 | 0 |

High-risk authored coverage includes:

- All 26 newcomer quests: 22 main quests and four growth rewards, with all 52 titles and complete descriptions. Level, objective count, monster subtype, floor and NPC coordinates remain exact.
- All 70 class-practice copy entries. Passive learned skills are distinct from casts; real hits, equipped consumables, separate toggles, summoned-skeleton damage and repositioning requirements remain explicit.
- All `quest.supply.*` entries, including quantities, shortages, cost, bag constraints, vendor/navigation and class materials; quest routing, turn-in and tracker templates preserve opaque placeholder values.
- Registration/account validation, connection/startup failure instructions, destructive character deletion, shop payment and selected mail/storage/service notices. Finish is a quest turn-in button; Share means sharing the quest, not transferring rewards.
- All 42 help titles and complete bodies, all 39 shortcut descriptions and the scroll hint. These include attack modes, pickup/harvesting, skill learning, accuracy/agility, quest markers/turn-in, Hold immediate selling/repair, permanent deletion, trade consent, gems versus orbs, hero/mount requirements, guild buff directions, and the unavailable-awakening warning and item-destruction consequence.

## Terminology and opaque data

The agreed content terminology is reused for all five classes, named skills, maps, materials and task monsters. In particular, Lightning (the line beam) and ThunderBolt (the bolt spell) remain different; Half Moon and Thrusting remain separate toggles. Healing Arabic is الشفاء, Bichon Russian is Бичон, and Border Village Arabic is قرية الحدود. Proficiency in help19 is a term from the original help, absent from the current 109 canonical magic entries; it is rendered as ordinary terminology and is not assigned a fabricated spell ID.

All authored strings retain the exact named-placeholder multiset. Player/account names, chat text, paths, coordinates supplied as arguments and other captured values are opaque; the override data never recursively translates these values. There are no added Unicode formatting controls in the translations. Arabic bidi isolation must be implemented at the rendering boundary if needed, rather than mutating player text or inserting ad hoc RLM characters into catalog values.

Two source wording issues are intentionally recorded instead of changing gameplay or the existing source catalogs:

1. `quest.supply.materials` says Fireball/summoning in English. The existing Taoist supply and practice flow uses SoulFireBall and summoning with Amulets, so the six overrides explicitly name Soul Fireball. A later English source correction should use that same canonical skill.
2. `help.shortcut.18` says “Toggle pet attack pet”. The actual action is ChangePetmode; the six overrides say “toggle pet attack mode”.

A separate unresolved source inconsistency is `help.page.01.body` saying Guild mode protects allied guilds as well, while `help.shortcut.22` says only guild members. Resolve this against authoritative attack-mode rules before changing the source; this translation review does not invent a rule.


### Deliberately unchanged tokens and names

- Physical key labels `Esc` and `Enter`; `MP: {n}`; `NPC: {npc}`; the semicolon separator; `PickUpObject` / `PickUpTile` in diagnostic pickup labels are classified technical values, not English fallback.
- Indonesian uses ordinary gaming/technical loanwords where appropriate: OK, Normal, Mentor, Guild, Chat, Status, Mana, Online/Offline, Level/Lv and mouse. Full sentences and actions have been authored, not copied from the English draft.
- `quest.journey.copy.54` retains the map's canonical display name `Life Death Coffin`; `.56`/`.57` retain `Insect Cave N/W` with exact `1F`/`2F`; `.58` retains the unreviewed monster proper name `Tongs`. The current reviewed content overrides had no matching stable entries for these names, so this worker did not invent a monster identity. Match final content display aliases before removing these retained names. `B4`, `D001` and `D401` remain exact.
- Help28 retains the item identifier `BraveryGem`. NPC personal names and identifiers such as Jane and Bull remain exact. Hero-specific display terms are translated descriptively; Hindi retains the `Old General` NPC name for discoverability.
- This is full UI-key authoring, not full NPC/common/item-catalog translation; those domains are assigned separately.

## Baseline v1 spot check

The v1 local machine drafts were sampled at 88 locale/key pairs: Arabic and Hindi each 26, and Russian, Thai, Vietnamese and Indonesian each nine. These were risk-selected examples, not a statistical quality estimate. Every example below is now covered by an authored six-language override.

| Locale | Concrete v1 issue |
|---|---|
| ar | `quest.supply.materials` confused the spell/material sentence with urine/toxins; the Taoist description rendered buffs as handcuffs. Help19 rendered Agility as infertility. Help01 lost the first monster-attack sentence. Several quest descriptions remained English or lost whole sentences. |
| hi | Help02 confused left-click and corpse/harvesting words; help01 lost the first monster-attack sentence. |
| ru | Help01 lost the first monster-attack sentence; help19 rendered Fencing as physical fencing; quest2110014 lost the consumable-restocking sentence. |
| th | SentencePiece `▁` symbols leaked. Help02 omitted the Alt/left-button harvesting procedure. Help40 omitted the final-failure-destroys-item sentence. The Taoist term was mistranslated. |
| vi | SentencePiece symbols and numeric protection sentinels leaked. Help40 omitted the final item-destruction consequence; several named-skill and attribute terms were mistranslated. |
| id | Quest2110018 rendered Wizards as lizards. Quest2110005 omitted both the city-safe-area-only requirement and its supply advice. |

Parent is regenerating v2 with sentence segmentation, protected tokens and reviewed terminology. **Do not publish v1 or treat a nonempty translated value / zero token errors as semantic acceptance.** These v1 findings do not establish v2 quality; v2 needs a separate spot check. The complete authored UI layer supersedes the v1/v2 UI drafts; the observations below still matter to the untranslated content/NPC domains. Exact v1 draft identities at review time:

| Locale | Draft token problems | SHA-256 |
|---|---:|---|
| ar | 366 | `fcd9d652aadd30470113fb4e333d6bfbd6c9f75e65bf365d5ec610b061729a1d` |
| hi | 88 | `ddfa1bbad36666be40eff0a0368cd0b7d04e7c891b4794f505af8048a9833121` |
| ru | 11 | `511b759ba6e3ffb26556fbd0d86d83a2cb1a5fe6ac55742bc5245fca0358fc9a` |
| th | 106 | `e7753c26a2954ecbc93bc6968e38de244b9844075d69c65ca29ecaa94d2eee93` |
| vi | 2668 | `34a04c8255f47b3f507366353cba7b9b85b8c1f6a981b04c1b89f1a0a9b51880` |
| id | 120 | `10912f212c0603a8141472b2e49ba6f2a5aeed60a35e12e16379544799b33242` |

The nine-key common sample was: quest.practice.copy.27; help.page.01.body; help.page.02.body; help.page.19.body; help.page.40.body; shell.taoist_description; quest.2110005.description; quest.2110014.description; quest.2110018.description. The broader ar/hi sample additionally covered registration safe-key wording, class descriptions, mine/Wooma instructions, awakening stat tables, keyboard shortcuts, guild confirmation and movement options.

## Read-only UI audit and integration checks

Initial three-locale assumptions were reported to the parent. Current source now uses Locale::COUNT for the nine-locale enum/catalog rows and help-scroll slots; the parent also replaced the crowded language row with a collapsed picker, added shared script font fallback and grapheme-aware quest wrapping. These are parent-owned fixes, not changes in this worker's patch.

Remaining concrete truncation checks reported to the parent (line numbers at review, before any parent fix):

- `apps/game-client/client-bevy/src/native_shell_ui.rs:2054–2055`: startup/notice summary truncates with `chars().take(MAX_NOTICE_CHARS - 1)`. Use graphemes or measured text so Devanagari/Thai combining sequences are not split.
- `apps/game-client/client-bevy/src/crystal_ui/game_shop_dialog.rs:288`: `text.chars().take(23)` can split a combining sequence.
- `apps/game-client/client-bevy/src/crystal_ui/game_shop_dialog.rs:959`: a localized friendly item name ends in `chars().take(24)`, with the same issue.

Fonts for Latin/Cyrillic, Devanagari, Thai and Arabic are already bundled in `platform-windows/src/native_fonts.rs` with explicit script fallbacks and nine-language test samples. Font presence alone is not proof of complete glyph shaping or layout. Current help uses start alignment and word-or-character wrapping; lack of hand-written bidi code is not evidence that the shaping engine is broken.

The final shared/native visual check should include:

- Arabic right-to-left paragraphs, shaped joining letters, mixed Latin player name `Gold`, literal player text `{count}`, coordinates `(147,33)`, HP `162/162`, `F1` / `Q` / `Ctrl + H`, and a map filename such as `0.map.gz`. Do not reverse entire strings or translate the opaque fragments.
- Hindi and Thai long notices and item names at the actual truncation boundary, ensuring no broken combining cluster and no clipped glyphs.
- Long Russian/Indonesian/Vietnamese quest tracker rows, complete button hitboxes and reachable scroll content; every locale must retain its own help scroll state.
- No automatic translation of player/chat/mail body text, account identifiers, protocol IDs or server commands.

No final render/RTL/native-speaker acceptance is claimed by this review.

## Validation record

Local data validation checked every authored key against the four source catalogs; exact six-locale fields; nonempty strings; exact placeholder multisets; exact numeric-token multisets outside placeholders; and absence of Unicode formatting controls. All passed. Keyboard tokens were checked with Unicode word boundaries, so Vietnamese words beginning in M/V are not mistaken for keys. The English verb “Enter” and the item name “Dungeon Escape” are not keyboard commands. Actual shortcut tokens in the authored help/practice/navigation strings remain intact. Canonical identifiers and `1F` floors remain unchanged. JSON is deterministic in source-key order.

Source catalog SHA-256:

- `shell.json`: `d1a5255c3f265a3b36490d40c26eb43a9e603698f47c8ef29619cb099904cc31`
- `quest.json`: `e524756ea4067a835a49002f5e4e98e279020933d28dfc536792f72826c25821`
- `game.json`: `28de1c1dc52ca46ac764d71e95db3f2f56c38c9860b021056c0ece71f9463945`
- `help.json`: `5ce6fa52b935f528424c3963dc16c5e430387d3be2dac6d70d0701cf834b3b9e`

Authored override SHA-256: `0b09bbc174431594226498efbacf51270d920f3868b367665fc5c3ae55e0ea86`.
