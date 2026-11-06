# M5 shared Quest presentation locale: bounded first slice

Root architecture decision and SOURCE grant. The overall cross-platform goal remains ACTIVE. This milestone fixes an explicit host/render language boundary; it is not full-game translation, a native language preference UI, or actual-client acceptance. User authorized autonomous multi-agent implementation; Root leads integration and high-risk decisions, Sol high owns the bounded write set, and an independent reviewer verifies frozen results.

## Baseline and purpose

M4 HomePage is 656200 bytes / 0bfa8285b4df17813ece39940040b38f642303844eafe104b0c75cf9f8f42d48. Its document-language effect must remain byte-identical. Existing Page selection, saved aliases, localStorage, protocol setLanguage, SSR en declaration and all unrelated behavior remain unchanged. The host currently supplies localized Quest fields while shared player_text and raw-leaf helpers can replace them with Chinese. Two root-verified read-only proposals bind the existing call chain; their findings are source evidence, not an observed UI failure.

## Locale and compatibility contract

- Initialize a shared QuestPresentationLocale Resource wrapping existing mir2_game_data::LanguageCode in Mir2QuestUiPlugin under the common native-ui/portable-quest-ui boundary. Do not put the only resource in the portable-only module. An immutable text context is passed explicitly to the selected render/build subtree; no mutable global/thread-local locale or string reverse lookup.
- Native missing presentation selection defaults to zh-CN to preserve its current authored Chinese Quest behavior. This is a client presentation default; simulation SessionResource English default is separate. A native selector, persistence, or actual default observed in a running client is not delivered.
- New Web snapshots explicitly supply the existing selected Mir2Language code: exactly en, zh-CN, es, pt-BR. These differ from BCP47 locale strings en-US/es-ES. Missing language on the new runtime means the legacy zh-CN presentation default. Present alias, wrong case, empty/unknown value, null, wrong type, or duplicate language field must reject the DTO; do not treat null as missing. Keep deny_unknown_fields.
- Parse/validate before applying snapshot state. Existing generation/revision checks remain authoritative. Invalid/stale input cannot replace the last accepted snapshot or locale; existing diagnostic error publication may remain. Accepted missing-language newer input explicitly selects the documented legacy default.
- Publish questLocaleVersion:1 and canonical accepted language in Quest status, including the initial supported shape. Capability probing accepts exact version 1 only. Older/unknown-capability runtimes receive an omitted language field and keep their current schema/readiness behavior. Malformed version-1 language status must fail readiness instead of silently claiming the requested locale is applied. Lean unsupported UI remains unsupported.
- Page change is limited to language in the Quest snapshot and its necessary immediate-refresh dependency. Do not introduce a second language state, provider/DOM observer, protocol/default change, or translation-label heuristic.

## Rendering and content boundary

Use proposal02 function-boundary.json as the explicit active call-chain guide: ten top renderers, eight leaves and five builders, plus relevant render/input/observer system params. The shared native/portable renderer observes Resource changes; compact stamps/input-current checks/observation include the locale so even a language-only switch with equal English fallback content invalidates the old tree.

For the selected Quest diary/detail/NPC Quest list/tracker paths, non-Chinese known-ID title, description line vector, objective, host guidance and reward display fields preserve the supplied UTF8/whitespace. In zh-CN, keep the existing authored Quest content helpers and legacy behavior. Preserve all authoritative model data; conversion is presentation only. Unknown/unkeyed non-Chinese content retains the host fallback. Do not claim a full NPC/content translation.

NPC name/body/option labels, player/custom names and input are opaque in every selected path and must bypass Chinese text/name helpers. Targets, option/action IDs, quest/reward indexes, counts, icons, tooltipSource, intent payloads, enabled/authorization decisions and link semantics are unchanged. Existing dialog wrapping and deliberate redundant-header suppression remain.

Factor four raw leaf families from the existing geometry implementation: quest_log_text_at, quest_log_text_button_at_sized, panel_text, action_button, with needed raw mobile/tracker adapters. Preserve old APIs as legacy Chinese delegates for out-of-scope consumers. Selected subtrees use raw output after explicit keyed chrome or field-specific conversion. Do not blanket-remap mixed lines, chosen translated chrome, or opaque NPC prose a second time. Existing line builders may remain legacy-zh delegates while active renderers use context-aware variants; non-Chinese description vectors must not be joined and rebuilt.

Approved canonical subset is exactly the following 22 existing keys; no importer/overrides/full-mirror changes or ad hoc translated dictionary are licensed:
- ui.quest
- ui.close
- ui.previous
- ui.next
- ui.questTrack
- ui.questEmpty
- ui.questAccept
- ui.questComplete
- ui.questAbandon
- ui.questShare
- ui.questNoReward
- ui.questObjective
- ui.questReward
- ui.questRewardExp
- ui.questRewardGold
- ui.questRewardCredit
- ui.questRewardSelect
- ui.questTimeLimit
- ui.questStage.available
- ui.questStage.inProgress
- ui.questStage.readyToTurnIn
- ui.questStage.completed

The generator reads the committed canonical game-data localization mirror, checks equality with the Web mirror, and deterministically emits only this exact allowlist into one narrow Rust table. Sorted keys/codes, correct Rust string escaping, no timestamps/absolute paths. Lookup uses selected -> English -> explicit stable fallback; source presence or pt-BR values equal to English is not translation completeness. Do not call game-data localized_text/localization_bundle or embed the whole 736994-byte bundle into WASM for this subset.

Use keys only where semantics match: generic ui.quest for selected Quest headings, Track plus a check marker for tracked state, Previous/Next for paging, and exact action/status/reward keys. Do not relabel PrepareQuestFinish as Complete. Unkeyed body tabs/back/current-primary/confirmation/journey/practice/supply/tooltip text and all unrelated HUD/bag/target/pickup/content/preferences/PWA locales remain explicitly OPEN. Keep old out-of-scope copy and behavior.

## Revision, state and input

Locale-only changes increment the existing snapshot revision, trigger redraw, and require acknowledgement of that locale/revision before shared input is allowed. New current status must agree with the live requested locale and minimum applied revision, including the callback-before-next-poll case. Preserve open_revision, selection, diary/detail/reward page, scroll, selected reward, pending operations, dialog history and session identity; language is not an open/close or session reset. Malformed/stale status, stalled frames, unsupported/hidden layout and existing owner handoff rules remain gated. Actual ECS/layout fixtures can prove state/marker behavior; they do not prove GPU/native/browser rendering.

## Bounded single-writer product scope

- packages/tooling/scripts/build-quest-presentation-text.mjs
- packages/game-data/data/generated/quest_presentation_text.rs
- apps/game-client/client-bevy/src/quest_presentation_text.rs
- apps/game-client/client-bevy/src/lib.rs
- apps/game-client/client-bevy/src/quest_ui.rs
- apps/game-client/runtime/src/quest_ui_host.rs
- apps/web/lib/bevy-quest-ui.ts
- apps/web/lib/use-bevy-quest-ui.ts
- apps/web/app/page.tsx
- apps/game-client/client-bevy/tests/quest_presentation_locale.rs
- apps/web/scripts/test-bevy-quest-locale.mjs
- packages/tooling/scripts/test-quest-presentation-text.mjs

Six existing source paths and six declared-absent new paths. Root archives 386 existing current source entries and six new absence entries (392 total), plus protected references. player_text.rs, portable_quest_ui.rs, canonical importer/overrides/mirrors, Cargo manifests/locks, native config/preferences/gateway/session/zone/auth/account/wallet/channel/save, M1/M2/M3/M4 behavior are protected. Worker writes only its fresh implementation01 QA subtree and the 12 product paths. Root owns five global progress docs, leases, architecture contract and build/artifact promotion. Request a concrete scope amendment if a compiler/contract need requires another file; do not change it silently.

## Verification and promotion

Worker runs focused deterministic generator/check fixtures; Rust portable/native controlled ECS/tree/locale/stamp regressions and strict DTO tests; Web actual-source DTO/capability/hook-seam fixtures; adjacent existing quest localization and M4 document-language fixture; bound direct TypeScript --noEmit. Archive before/final product bytes, actual used inputs and raw command stdout/stderr/close receipts. Failures remain preserved. Controlled fixtures are not actual React/user/browser/native/device/persistence evidence.

Cargo commands use pinned +1.95.0, --locked --offline, the bounded client/runtime manifests/features and the existing single-writer Windows target cache. Pure controlled Bevy test worlds are allowed; no window, renderer backend, network/login, native host executable launch, browser/headless-browser/GUI service, private data, full-pack/index/payload access or destructive cleanup is authorized. No dependency update, formatting rewrite or generated runtime publication by worker.

After source freeze and independent review, Root separately authorizes native DEBUG build and all three canonical release WASM variants, then production Next webpack/final NFT, clean standalone/copy/dependency/HTTP gates as warranted. Root records actual closes and byte budgets; do not inherit M3/M4 build or HTTP acceptance for changed Rust/client bytes. Existing 31MiB per-WASM and 360MiB (377487360-byte) standalone caps remain unchanged. M3 clean baseline376371942 has1115418-byte headroom; a 5348-byte compact 33-key JSON feasibility estimate is not a Rust/WASM/package delta. Overflow prevents build/package promotion and must be resolved in source or packaging, never by raising caps.

Actual normal-authenticated Windows and shared GL2/GPU/compatibility Web language switching/render/persistence/font/overflow, normal Logout/save/relogin, and physical Android/iOS remain OPEN. Desktop work remains stopped after human Escape until explicit consent arrives; no alternative GUI/headless path bypasses that stop. Source/build Candidate can proceed independently. No commit/push/deploy, stored-state rollback, private read or whole-goal completion follows from this grant.
