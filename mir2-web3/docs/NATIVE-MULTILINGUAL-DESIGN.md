# Native multilingual installer and game design

**Status: proposed design, not implemented or certified translation coverage.**
Requested selectable locales: **繁體中文 (`zh-TW`), English (`en`), Português (Brasil) (`pt-BR`)**.
Audit: 2026-09-29, checkout `96e6b4e29aca50fd18f9e7850866933318702d8c` with unrelated animation/startup work in progress.
This document changes no client behavior, package, server, account, or paused capacity work.

## Product decision

Ship one Windows installer and one executable containing all three locales. Installation chooses the initial language; the game provides a language selector. Language never chooses a different realm, account, save, gameplay data set, or executable.
The native selector contains exactly `zh-TW`, `en`, and `pt-BR`; Simplified Chinese and Spanish are not requested selectable languages. Existing `zh-CN`/`es` resources elsewhere remain until an explicit migration. Brazilian Portuguese does not imply Spanish or European Portuguese support.
Installer, native shell, in-game interface, and authored content are separate coverage claims. Translating an installer or login screen does not translate the complete game.

## Verified current baseline

Inventory counts below are not translation acceptance percentages.

| Surface | Observed state | Gap |
| --- | --- | --- |
| Shared bundle | `en` and `pt-BR`: 2,355 keys each; no `zh-TW` | Traditional Chinese and native consumers required |
| Portuguese values | 278 values identical to English | Review individually; names/codes may legitimately match, so equality is not a missing-translation count |
| Key categories | 1,071 client, 768 server, 271 content, 245 other per en/pt-BR | Existing keys do not cover the complete imported catalogue |
| Windows native | No use of shared `localized_text`/`format_localized_text`, locale selector, or persisted locale found | Native integration required |
| `player_text` | Simplified Chinese adapter: 26 newcomer quest IDs, 51 monster aliases, 93 other name entries, 60 NPC aliases, 113 text entries | No locale argument; unknown text remains unchanged |
| Imported content | 1,628 items, 555 monsters, 375 NPCs, 109 magics, 154 original quests | Audit every released field/page; 26 newcomer quests are a separate authored set, not 26 of 154 |
| Installer | ChineseSimplified wizard plus hardcoded Chinese custom messages | All installer surfaces need three-language coverage |
| Fonts | Pinned installed Arial/YaHei faces with retained fallback identities | Clean-Windows Traditional Chinese/Portuguese coverage is not established |

Sources: [shared API](../packages/game-data/src/lib.rs), [bundle](../packages/game-data/data/generated/localization_bundle.json), [generator](../packages/tooling/scripts/import-crystal-localization.mjs), [overrides](../packages/game-data/data/i18n-overrides.json), [native adapter](../apps/game-client/client-bevy/src/player_text.rs).
Catalogue counts come from the generated [item](../packages/game-data/data/generated/crystal_item_manifest.json), [monster](../packages/game-data/data/generated/crystal_monster_manifest.json), [NPC](../packages/game-data/data/generated/crystal_npc_info_manifest.json), [magic](../packages/game-data/data/generated/crystal_magic_manifest.json), and [quest](../packages/game-data/data/generated/crystal_quest_packet_manifest.json) manifests.
The audited operator recipe is [Mir2-Invite.iss](C:/mir2-playtest-releases/20260929-registration/installer-build/Mir2-Invite.iss), with sibling `verified-payload-files.iss`. This is an audit reference, not an absolute path to embed in implementation.

## Locale and catalogue contract

Introduce a typed native allowlist: `TraditionalChinese`, `English`, `BrazilianPortuguese`, serialized as `zh-TW`, `en`, `pt-BR`. Platform hints may normalize `en-US`/`en-GB` to English and `zh-Hant` to Traditional Chinese. Do not reuse the current permissive `LanguageCode` parser unchanged: it lacks `zh-TW` and aliases `pt-PT` to `pt-BR`.
Load the locale before the first shell render. A locale resource plus revision invalidates text, tooltip, measured-layout, and display-search caches on selection; do not recreate the gameplay session or mutate inventory/quest state.
Generate immutable catalogues from version-controlled translation sources, reusing reviewed en/pt-BR keys and adding native keys plus reviewed zh-TW. Do not duplicate the shared dictionary manually in Rust. A first-phase subset must declare its exact supported keys.
Use stable UI keys with typed arguments, for example password requirements `(min, max)` and retry countdown `(seconds)`. Bounds come from existing validation policy; changing language never changes policy.
Fallback is selected locale → canonical English entry → original source text. Never expose an internal key or empty label. Source fallback is a coverage gap, particularly where the source is Chinese. Record bounded missing-key diagnostics without credentials or user-authored text.
The existing formatter replaces positional arguments and strips some Crystal format suffixes; it is not a full pluralization/locale-number formatter. Validate argument types, escaped braces, rich-text tokens, and explicit English/Portuguese plural forms. Display formatting never changes numeric packet values.

## Display identity and gameplay boundaries

| Family | Display lookup | Values that must remain canonical |
| --- | --- | --- |
| Quest | Quest ID + title/description/objective ID | Prerequisites, progress, reward and turn-in IDs |
| Item | Catalogue index + name/description field | Unique ID, quantity, price, purchase/use request |
| Skill | Spell/skill ID + display field | Spell token, binding, cast command, cooldown |
| NPC | Catalogue/script identity + page/node key | Runtime object ID, script link target such as `@Buy` |
| Monster/map | Catalogue identity/map file ID + display field | Assets, collision, spawn, routing and transfer keys |
| Result/error | Stable code/key + typed arguments | Outcome, retry interval, operation state |

Keep original model values and derive labels for rendering. Never translate player names, chat, mail bodies, guild names, account IDs, passwords, script commands, or packet identifiers. User text passed as an argument is opaque data, not a translation key or executable markup.
[NPC models](../apps/game-client/client-bevy/src/quest_model.rs) already separate `option_id`/`label`; the [native bridge](../apps/game-client/platform-windows/src/gameplay_bridge.rs) maps `links.target`/`links.text` separately. Where dialogue has no stable page/node identity, add an additive display identity or reviewed content mapping; arbitrary sentence replacement is insufficient.
[GameShop](../apps/game-client/client-bevy/src/crystal_ui/game_shop_dialog.rs) uses class/category strings for filtering and `item_name` for search. Translate labels without changing filter values; eventually search canonical and displayed names while selecting/buying by index.
The [skill page](../apps/game-client/client-bevy/src/crystal_ui/skill_page.rs) directly renders model names. [Quest UI](../apps/game-client/client-bevy/src/quest_ui.rs) and [guidance](../apps/game-client/client-bevy/src/quest_multi_guidance.rs) mix source text and Chinese adapters: translate complete instructions before wrapping.

## Per-user preference and installer

Proposed file: `%APPDATA%\mir2-web3\locale.json`, containing only `{ "version": 1, "locale": "pt-BR" }`.
Resolve the same interactive user's data directory in installer and client, not the elevated VC-runtime installer's account. Other Windows users resolve their own preference.

1. Existing valid per-user preference wins, including during upgrades.
2. On first installation only, installer selection seeds a missing preference.
3. Without a seed, a supported Windows UI-language hint chooses the initial locale.
4. Unsupported hints, including `zh-CN`, `es`, and `pt-PT`, default to English; manual selection remains available.

Use bounded input, a known schema/locale allowlist, and atomic writes. Invalid/unreadable preferences fall back safely to English. Failed persistence must not crash the client or alter account/session state.
Do not edit signed `mir2-client.toml`: its [parser](../apps/game-client/platform-windows/src/session_config.rs) rejects unknown keys. Do not append an unknown field to `options.json`: its [adapter](../apps/game-client/client-bevy/src/options_effects.rs) uses `deny_unknown_fields` and explicit version migrations. A separate locale file keeps the first change bounded.
The installer must provide exactly three wizard languages and translate all custom messages, tasks, readmes, prerequisite errors, restart advice, and uninstall labels. Keep one AppId, install directory, executable, and upgrade identity.
Never overwrite an existing preference when an upgrade uses another wizard language. Preserve it on normal upgrades and uninstall/reinstall unless removal is explicitly chosen.
Include catalogues, locale art, and licensed font files before generating the verified manifest/signature. All installer language choices copy identical verified game bytes. Locale selection writes only per-user preferences; it never patches signed payload files or weakens package/VC-runtime checks.

## Server-authored text and compatibility

[Gateway](../apps/gateway/src/web.rs) already exposes `BrowserCommand::SetLanguage`; [personal sessions](../apps/simulation/src/runtime/session.rs) store a language defaulting to English, and some [DisplayName](../apps/simulation/src/runtime/components.rs)/snapshot paths localize text. Literal content remains. The native outbound protocol currently has no matching language command.
The first foundation/shell phase can use local display lookup without server changes. For complete content support, explicitly choose local lookup or per-session localization for each family and add `zh-TW` support where required.
Two players in the **same shared Zone may select different presentation languages**. Locale must not create locale-specific simulation IDs, world state, occupancy, or routes. Shared system broadcasts should prefer stable codes plus arguments for recipient/client localization; player chat and names remain untouched.
A late `SetLanguage` alone does not translate cached panels, tooltips, names, or text baked into images. Rebuild/invalidate presentation deliberately, and verify localized display after login, map change, logout/relogin, and native resume. Do not use a process-wide or shared-Zone locale.
Preserve older Gateway compatibility with local lookup/English fallback. Keep flat/nested error-envelope support, actual result codes, and authoritative retry seconds. Language changes must not clear authentication, bypass limits, or replay registration/password operations.

## Baked UI text, fonts, and layout

[Login rendering](../apps/game-client/client-bevy/src/crystal_ui/login.rs) uses art for labels/controls; [spec.rs](../apps/game-client/client-bevy/src/crystal_ui/spec.rs) references Title frames 31/32 and buttons 323 onward. A dictionary cannot translate those pixels.
Inventory text-bearing assets across login, character, quest, inventory, and shop windows. Use neutral art plus localized text or verified per-locale art variants; preserve hit areas, hover/pressed/disabled states, focus, timing, and original mappings. Do not overlay translated text on a still-visible English label.
[Font setup](../apps/game-client/platform-windows/src/native_fonts.rs) pins Arial/YaHei and retains fallback identities; [typography](../apps/game-client/client-bevy/src/crystal_ui/typography.rs) defaults to Arial while some quest controls request YaHei.
Provide verified Traditional Chinese and Portuguese diacritic coverage on clean Windows, using redistribution-approved bundled fonts or tested system fallback. Do not redistribute Windows fonts without confirmed rights. Player chat may contain scripts outside the selected UI language.
Retain font/source/atlas identities; repeated locale changes must not reproduce the prior font-memory growth. Translate before wrapping and measure actual rendered text; character-count truncation is insufficient for Portuguese/CJK. Test long labels, tooltips, instructions, pending/error notices, and scrolling at 100%, 125%, and 150% DPI.

## Planned phases and acceptance

No phase below is claimed implemented by this document.

| Phase | Bounded implementation scope | Required evidence / allowed claim |
| --- | --- | --- |
| 1. Foundation + installer | Locale/catalogue/preference modules, generation checks, wizard/custom messages/readmes | Three-locale selection, fallback, safe preferences, upgrades and identical signed payload; claim installer/foundation only |
| 2. Native shell | Login/register/password/connect/reconnect/character/safe-key/delete UI and associated art | Real states/errors in all locales, glyph/layout captures, unchanged validation/outbound commands; claim shell only |
| 3. In-game interface | HUD, menus, options, chat controls, maps, inventory/equipment, shop, mail, skills, quest controls | Declared UI call-site inventory and interaction/layout checks; claim only listed interfaces |
| 4. Game content | Shipped quest/NPC/item/skill/monster/map display fields and needed additive identities | Field/page coverage, terminology review, unchanged route/turn-in/shop/cast/resume behavior; claim only audited content/profile |
| 5. Release | Verified combined payload and single installer | Clean installs/upgrades, native screenshots, fallback report, font-memory and package checks; claim tested release scope |

Keep one worker per shared/high-conflict file. Use separate focused catalogue/preference tests. Any Gateway/simulation change needs its own bounded review; localization does not authorize combat, economy, authentication, persistence, or Zone-authority changes.

Required acceptance checks:

- **Coverage:** check keys, nonempty values, actual native consumers, explicit fallbacks, and reviewed exceptions. Existing unused keys do not count as native coverage.
- **Content:** record locale/profile/catalogue revision/entity ID/field or page/status/fallback reason/reviewer. Distinguish excluded content from untranslated released content.
- **Parameters:** verify placeholder names/types, numeric bounds, plural forms, markup/link syntax, and rendered 0/1/many/boundary examples; matching key counts alone is insufficient.
- **Identity:** compare outbound commands and retained IDs across locales for navigation, NPC options, quest turn-in, purchases/use, and skill casting; player text remains byte equivalent.
- **Failure behavior:** exercise invalid fields, pending/duplicate submissions, actual registration results, flat/nested errors, rate limits, reconnect failures, and editing recovery without automatic retries or weakened checks.
- **Persistence/package:** restart and upgrade with another installer language; prior selection survives and signed payload hashes remain valid. Regenerate/sign/verify releases containing new language assets.
- **Native rendering:** capture shell and representative game panels in all locales/DPI settings; check Traditional Chinese, accents, long text, cached-panel rebuild and resume, and bounded font/atlas retention after repeated changes and warmup.

A successful build, translated installer menu, dictionary count, or shell demonstration cannot certify complete game-content localization.
