# Native nine-language implementation and acceptance

Status on 2026-09-30: the native client implements nine selectable presentation languages and has complete generated key sets for the six additions. **Key coverage and passing structural tests do not certify fluent translation or final Windows/GPU acceptance.** The previous [three-language proposal](NATIVE-MULTILINGUAL-DESIGN.md) is a historical design, not the current allowlist or implementation status.

## Language selection and persistence

The exact persisted/runtime allowlist in [native_i18n.rs](../apps/game-client/client-bevy/src/native_i18n.rs) is:

| Code | Selector label | Main bundled family |
| --- | --- | --- |
| `en` | English | Noto Sans |
| `zh-TW` | 繁體中文 | Noto Sans TC |
| `pt-BR` | Português (Brasil) | Noto Sans |
| `ru` | Русский | Noto Sans |
| `hi` | हिन्दी | Noto Sans Devanagari |
| `id` | Bahasa Indonesia | Noto Sans |
| `vi` | Tiếng Việt | Noto Sans |
| `th` | ไทย | Noto Sans Thai |
| `ar` | العربية | Noto Sans Arabic |

The shell displays a collapsed language button with nine choices opening above it. The in-game Options panel uses the same selector. Choosing a language closes the popup and increments the presentation revision; localized labels, fonts and dependent panels refresh without recreating the character or network session. Help content has nine locale slots, and long help/quest content uses the existing scrolling and wrapping surfaces. This is one client with selectable presentation, not one realm or executable per language.

[native_locale.rs](../apps/game-client/platform-windows/src/native_locale.rs) initializes the language before early startup diagnostics. It reads `%APPDATA%\mir2-web3\locale.json`, with the exact shape `{"schema":1,"locale":"ar"}`. A valid saved preference wins, followed by a valid `locale-seed.json`, then a supported Windows language hint. Unsupported hints fall back to English. OS-region hints are deliberately mapped separately from persisted codes: for example, Arabic regional hints can select `ar`, while `ar-SA` is not a valid stored code; Traditional Chinese hints select `zh-TW`, and `pt-PT` does not select Brazilian Portuguese.

Preference input is bounded to 256 bytes, rejects unknown fields/schema/codes, and refuses linked/reparse-point paths. Same-directory atomic replacement persists a selection; failure retains the in-memory language and emits a non-sensitive diagnostic. Preference changes do not edit the signed game directory, client configuration, accounts or saves.

The [installer source](../apps/game-client/platform-windows/scripts/invited-installer/Mir2-Invite.iss) also lists nine wizard languages and language-specific readmes. Its initial seed is written only when neither preference nor seed exists; an upgrade's wizard selection does not overwrite an existing choice. Source support here is separate from acceptance of a newly built installer.

## Fonts, text boundaries and Arabic

The executable bundles **nine font files across five families**: one Noto Sans TC variable face, plus regular/bold faces for Noto Sans, Noto Sans Devanagari, Noto Sans Thai and Noto Sans Arabic. They are embedded and their handles/data identities remain pinned for the native process lifetime. Language changes select an existing family; they do not repeatedly discover or reload font files. Script fallback also covers supported-script player text without translating it. The fonts are not installed into Windows.

Recorded source URLs, hashes and redistribution notices are in [SOURCE.json](../apps/game-client/platform-windows/assets/fonts/SOURCE.json), [MULTILINGUAL-SOURCES.json](../apps/game-client/platform-windows/assets/fonts/MULTILINGUAL-SOURCES.json) and the adjacent SIL Open Font License files. [native_fonts.rs](../apps/game-client/platform-windows/src/native_fonts.rs) owns embedding, fallback registration and retained identities; this does not assert that every possible player Unicode character is covered.

Translation precedes measurement and wrapping. Shell notice truncation, shop-name/search truncation and forced tracker line breaks now respect Unicode grapheme boundaries, preserving combining sequences and Indic/Thai clusters rather than slicing individual scalar values. Existing mail/friend editors also retain their grapheme-safe editing boundaries. These changes prevent split clusters; actual clipping, line height and shaping still need rendered acceptance.

Arabic uses Parley's paragraph bidi/shaping and the bundled Arabic family. Ordinary left-justified text is changed to `Justify::Start`; explicit centered/right-aligned counters keep their geometry. **The complete Crystal layout is not mirrored.** The implementation does not reverse strings, coordinates, map geometry, inventory slots, key bindings or mouse actions. Mixed Arabic, Latin player names, numeric coordinates and keyboard labels require visual review as mixed-direction text, not merely a nonempty-string check.

## Display identity and opaque data

Stable catalogue keys select translated labels; canonical item indices, quest IDs, spell tokens, map filenames, NPC option IDs and command targets continue to drive gameplay. Changing a label does not change damage, cooldowns, rewards, quest conditions, authentication policy, navigation or save contents.

Player/account names, passwords, chat, mail bodies, guild names and captured runtime arguments remain opaque. `format_key`/`format_for` substitute arguments without looking them up as translation keys. NPC display aliases/templates have a separate lookup domain, so an NPC “Return” does not overwrite the quest meaning of “Return”. Translated NPC rows are wrapped without applying a second translation to captured names such as `Gold` or `Accept`. Slot preservation does not authorize commands contained in an authored help line.

The Windows host activates native localization. Renderer-neutral hosts retain their legacy behavior when it is inactive. Unknown display sources retain a source fallback; this is compatibility behavior, not proof that every newly introduced future string is translated. Original `en`/`zh-TW`/`pt-BR` catalogues are inputs to the expansion, not regenerated from machine output.

## Coverage and translation provenance

Eight source catalogues contain **9,385 stable keys**. Each of the six added locale overlays contains exactly those 9,385 keys, for 56,310 added values. The generated files are in [native-i18n/extra](../packages/game-data/data/native-i18n/extra); [provenance.json](../packages/game-data/data/native-i18n/extra/provenance.json) records source, draft, override and output hashes, method counts and unresolved structural findings.

The provenance snapshot read for this document reports zero `errors` and zero `identicalNeedsReview`. Its methods are:

| Added locale | Keyed developer review | Unambiguous exact-source reuse | Offline prose/content draft | Offline name draft | Total |
| --- | ---: | ---: | ---: | ---: | ---: |
| `ru` | 3,138 | 230 | 5,972 | 45 | 9,385 |
| `hi` | 3,184 | 230 | 5,804 | 167 | 9,385 |
| `id` | 3,299 | 230 | 5,106 | 750 | 9,385 |
| `vi` | 3,168 | 230 | 5,836 | 151 | 9,385 |
| `th` | 3,109 | 230 | 5,978 | 68 | 9,385 |
| `ar` | 3,348 | 230 | 5,014 | 793 | 9,385 |

Use the generated provenance when later edits change these counts. The developer-review categories are not a claim of professional native-speaker review. **Large portions of NPC prose and other content remain offline Argos drafts**, even where all keys and parameters pass validation. Early drafts demonstrably mistranslated skills, negations and whole clauses; complete fields alone are not a language-quality gate.

The authored override layers have narrower, documented scopes:

- [UI review](../packages/tooling/data/native-i18n/expansion/ui-review.md): all 969 shell/quest/game/help keys × six languages = 5,814 authored values. This includes 26 newcomer quest titles/descriptions, practice, supply and navigation instructions, 42 help pages and 39 shortcut descriptions.
- [Content review](../packages/tooling/data/native-i18n/expansion/content-review.md): 109 canonical skill names and all 102 available full canonical skill descriptions, with the same reviewed wording reused for 102 legacy tooltip keys. These are 102 skills, not 204 distinct skills. Seven skills have no source description; none was invented. The layer also covers selected item text, terminology and common UI/runtime wording.
- [Error review](../packages/tooling/data/native-i18n/expansion/review-notes.md): 148 keys × six languages correcting the identified parameter/number-sensitive error set; this is not a whole-catalogue semantic review.
- [NPC review](../packages/tooling/data/native-i18n/expansion/npc-review.md): 270 formerly identical keys, comprising 211 menu labels and 59 prose lines. Proper names, region/floor codes and unresolved legacy identifiers are explicitly classified rather than renamed to force a cosmetic difference.
- [Remaining name review](../packages/tooling/data/native-i18n/expansion/other-names-notes.md): additional bounded name corrections by key/locale, with named families and floor codes preserved. The merger also records the current names/runtime override file hashes.

Known source ambiguities remain documented. For example, `Lightning` is the beam skill and `ThunderBolt` is the strike skill; they must not be merged. Legacy `CelestalShield` text is not silently reassigned to another shield. A faithful translation of an inconsistent legacy tooltip does not certify its combat semantics.

## Offline generation and validation

Run the following maintainer commands from the `mir2-web3` project directory, replacing angle-bracket paths with separate local tooling/evidence directories. They describe regeneration; they were not run as part of this documentation edit.

The [draft tool](../packages/tooling/scripts/draft-native-i18n-expansion.py) uses Python 3.13, CTranslate2 4.6.3 and SentencePiece 0.2.1 in a separate tooling environment; the Indonesian OPUS package additionally uses Sacremoses and subword-nmt BPE. `fetch` retrieves the six English-source packages from the official Argos package index and writes `model-provenance.json` with URLs, package metadata and archive hashes. The recorded packages used here are `en_ar` 1.0, `en_hi` 1.1, and `en_id`/`en_ru`/`en_th`/`en_vi` 1.9. Model provenance records the maintainer's MIT/CC0 clarification and its source; retain that record with the exact downloaded artifacts. The retained [model record](../packages/tooling/data/native-i18n/expansion/model-provenance.json) contains the exact download hashes; its captured index SHA-256 is `a65ae909a97ddc89a01cbf0596932dc6415df9815a11d85fe8240b367926b7b0`. A later download from the mutable index is not automatically the same model set. The client does not download or run a translation model.

```text
python packages/tooling/scripts/draft-native-i18n-expansion.py fetch --models <models-directory>
python packages/tooling/scripts/draft-native-i18n-expansion.py translate --models <models-directory> --output <draft-directory> --device cpu
python packages/tooling/scripts/draft-native-i18n-expansion.py translate --models <models-directory> --output <draft-directory> --device cpu --retry-errors
node packages/tooling/scripts/native-i18n-expansion-content.mjs --check --self-test
node packages/tooling/scripts/native-i18n-expansion.mjs --drafts <draft-directory> --report <review-json>
```

The current draft pass splits English into sentence units while retaining paragraph/newline separators, locks reviewed content names, protects adjacent interpolation slots as opaque units, and restores protected numbers, shortcuts, stat abbreviations and displayed commands. Failed or duplicated restoration is recorded, not guessed. Thai tokenizer separators are normalized before restoring protected values. Reusing a draft directory reuses its unit cache: retain the model/glossary/source provenance, and use a fresh cache when those inputs or the translation algorithm change.

If the review contains untranslated content-name drafts, the bounded name pass can supply candidates; it does not replace human review:

```text
python packages/tooling/scripts/draft-native-i18n-expansion.py names --models <models-directory> --output <draft-directory> --review <review-json>
node packages/tooling/scripts/native-i18n-expansion.mjs --drafts <draft-directory> --report <review-json>
```

Review unresolved strings in the keyed override files under `packages/tooling/data/native-i18n/expansion`. The merger rejects unknown keys, unknown locales and conflicting reviewed values. Reuse by exact English source is allowed only when reviewed contexts agree; different meanings stay separate. It writes overlays only when both structural errors and unclassified identical values are empty.

```text
node packages/tooling/scripts/native-i18n-expansion.mjs --drafts <draft-directory> --write --report <final-review-json>
node packages/tooling/scripts/native-i18n-expansion.mjs
node packages/tooling/scripts/verify-native-i18n.mjs
node --test packages/tooling/scripts/native-i18n.test.mjs
```

The no-argument expansion command verifies exact key closure, current source/override/output hashes and each value. Source and reviewed JSON fingerprints normalize CRLF to LF, matching Git text attributes across platforms; binary/model and generated overlay hashes stay byte-exact. The original-catalogue verifier also writes its validation report. Checks include placeholder identity/multiplicity, displayed `@` commands, literal numeric values, replacement characters, unrestored markers and lost surrounding words. Numeric glyph normalization checks value equality without changing gameplay numbers. Specialized content checks additionally protect floors, stat/material tokens, source numbers and documented source hashes. These are structural/source-fidelity gates; they do not prove grammar, negation, naturalness or every gameplay clause in unreviewed prose.

## Confirmed tests and outstanding acceptance

Only the confirmed integration results are recorded here:

| Check | Confirmed result | Limit |
| --- | --- | --- |
| Node native-i18n tooling suite | 14 passed | Catalogue/key/substitution checks; not a visual or linguistic certification |
| Shared client suite | 1,153 passed, 0 failed, 5 ignored | Ignored fixtures are not passes; final native/GPU results remain separate |
| Nine-language GPU review | 148 captures across shell/game/quest/bundled-font fixtures passed | Offline 1024×768 only; see [visual evidence](generated/player-qa/native-nine-languages-20260930/README.md) |
| Windows native suite | 784 passed, 0 failed, 4 ignored | Actual final installer build receipt and clean-laptop launch remain separate |

The confirmed shared log is the local evidence file `C:/mir2-build/shared-i18n9-20260930-06-test.log`. It supersedes the preceding failed run; this document does not erase that earlier evidence or count ignored GPU work as completed. No build or tests were launched by this documentation-only task.

Final visual work must cover all nine choices, persisted restart behavior, long quest/help/shop text, Arabic mixed-direction names and coordinates, Devanagari/Thai clusters, and repeated locale switches without renewed font-memory growth. Linguistic review must sample ordinary gameplay and long NPC/content prose, particularly the remaining machine-draft portion. GPU output and successful process creation alone do not establish live gameplay or native-speaker acceptance.

The reported Windows application-control launch refusal, including **CreateProcess error 4551** on another machine, is a separate public-distribution/signing blocker. Nine-language support and package-integrity checks do not establish a trusted publisher signature. The installer preserves the actual failure code and useful diagnostics; this language work does not resolve publisher trust, bypass application control or change operating-system security policy. Final signed-package availability and clean-machine launch acceptance must be reported independently.
