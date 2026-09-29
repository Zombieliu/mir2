# Native three-language UI and DPI correction — 2026-09-30

## Scope and behavior

The Windows client supports exactly Traditional Chinese (`zh-TW`), English
(`en`) and Brazilian Portuguese (`pt-BR`). The login screen and in-game Options
can change language. Per-user atomic preferences take priority over the
installer's first-run seed, then a supported OS language, then English.
Native changes do not add selectable Simplified Chinese or Spanish.

Eight embedded catalogues cover the shell, ordinary game labels, original
content, newcomer instructions and supplies, original quest text, referenced
NPC prose/links and the 42 original Help image pages. Translation occurs at
display boundaries before wrapping. Original item/map/skill IDs, NPC link
targets, quantities, stats, quest conditions and packets remain unchanged.
Player names, account input, passwords, player chat, mail bodies, guild notices
and other user-authored text remain literal. The synthetic name `Gold` is
intentionally present in screenshots to verify that boundary.

Original button word art uses translated text without changing hit rectangles
or actions. Long quest tracking content has a bounded scroll viewport above
the HUD. Translated help pages retain page-specific scroll offsets. NPC prose
has a separate lookup domain, protecting short system captions such as Return.
Ambiguous short labels such as New use contextual keys for shop/quest/options.
Template matching uses compiled prefix buckets and one-pass opaque captures.

The native EXE embeds the OFL-licensed Noto Sans TC font with pinned font
identities. The signed package includes the readable license. No font or
translation download is needed during play. The installer recipe and three
player readmes are versioned under `platform-windows/scripts`.

## Reported Alienware laptop defect

The user's freshly installed client screenshot has a full-sized world with
the fixed HUD confined to about the upper-left two thirds. The old host set a
1024×768 physical resolution and scale override 1, but also set 1024×768
**OS-logical** minimum/maximum resize constraints. Actual Winit conversion at
150% DPI makes those constraints 1536×1152 physical pixels, while the HUD stays
1024×768. This reproduces the screenshot's size ratio in code.

The host now uses one fixed pixel viewport, disables resizing/maximizing, and
removes the contradictory OS-logical lock. An event-driven guard restores that
viewport when Windows supplies another rectangle during a monitor DPI change.
It does not continuously resize the window on ordinary frames. Tests cover
100–400% DPI and repeated monitor-change events without a resize loop.
This is a code-supported diagnosis; the actual Alienware device has **not**
been retested by this agent and remains a required external confirmation.

## Executed validation

| Gate | Result |
| --- | --- |
| Shared native-UI unit/regression suite | 1,147 passed; 5 explicit GPU fixtures ignored by this command |
| Windows host unit/regression suite | 777 passed; 3 explicit environment/GPU fixtures ignored |
| Translation tooling tests | 12 passed |
| Strict catalogue validation | 8 catalogues; 9,385 entries; no missing values, placeholder errors or unclassified untranslated values |
| Candidate packer and verifier self-tests | Passed |
| Offline production shell GPU fixture | Passed, 14 PNGs |
| Offline production game widget GPU fixture | Passed, 12 PNGs |
| Offline production quest/NPC GPU fixture | Passed, 21 PNGs |

The three listed GPU fixtures were run explicitly, separately from the unit
suite, with real packaged images and fonts. They create no native window,
connection, account or character save. All 47 screenshots and measured text
bounds are preserved here. Portuguese and Traditional Chinese wrap inside
their reserved text boxes. The quest fixture selects longest authored cards
and detail text across 26 quest definitions and three classes, checks detail
pagination, reachable bottom controls, and real Jane/bookshop script sections.
Some fixture character stat values are empty by design; this validates labels
and layout, not server stat delivery.

The shell fixture performs 30 complete three-locale cycles (90 switches and
360 screen rebuilds). Font atlases stabilize at 16 pages / 16 MiB and retained
font identities remain bounded. This is a bounded regression, not a long soak.
The text engine's existing ICU segmentation-model warning is retained in local
run logs; it did not fail these glyph/bounds checks.

Generic alias collisions are reported, not hidden. Typed content keys, a
separate NPC domain and explicit context keys resolve relevant call sites;
catalogue counts alone do not prove every possible server sentence translated.
Source scope, original missing descriptions and malformed source text are
documented in `packages/tooling/data/native-i18n/README.md`.

## Evidence and reproduction

- `shared-tests.log`, `windows-tests.log`, `tooling-tests.log` contain full results.
- `catalog-validation.log`, `packer-selftest.log`, `verifier-selftest.log` record gates.
- `shell/shell-i18n-report.json`, `game/game-i18n-report.json` and
  `quest/quest-i18n-layouts.json` contain screenshot metadata and text bounds.
- `evidence-files.json` pins the 56 evidence files by size and SHA256.

Compile shared tests with Rust 1.95.0, `--locked --features native-ui`; run
ordinary tests with `--test-threads=1`. Windows tests need
`MIR2_NATIVE_ASSET_ROOT` pointing to real packaged assets. Explicit GPU tests
also need `MIR2_I18N_VISUAL_ASSET_ROOT`, and a fresh output directory:
`MIR2_I18N_VISUAL_OUTPUT` for shell/game, `MIR2_QUEST_I18N_VISUAL_OUTPUT` for quests.
Run `multilingual_shell_screens_and_font_switches_render_offscreen`,
`multilingual_game_widgets_render_offscreen`, and
`three_locale_longest_quests_and_real_npc_menus_offscreen` individually with
`--ignored --test-threads=1`.

## Acceptance boundary

These results establish source/tests/offline rendering, not clean-PC install,
actual Alienware DPI behavior, authenticated three-language gameplay, long
session stability or full Wizard/Taoist 0–30 acceptance. Existing Warrior
acceptance remains intact. No live client, account, saved character or server
was changed for these checks. The separate capacity optimization remains
paused; no global parity or multiplayer capacity claim is added.

Release build, signed Candidate and installer hashes are recorded separately
after packaging; they are not inferred from these test EXEs.
