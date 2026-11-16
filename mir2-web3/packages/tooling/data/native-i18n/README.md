# Native translation source and verification

This catalogue targets **English (`en`), Traditional Chinese (`zh-TW`) and
Brazilian Portuguese (`pt-BR`)**. Other languages retained in the shared web
bundle do not add selectable native languages. These files contain display
copy, not new gameplay rules or translations of player names and chat.

## Reproduce

Run from `mir2-web3` using Node.js 22 (verified with 22.18.0) and Python 3.
The only conversion dependency is the official
[OpenCC](https://github.com/BYVoid/OpenCC) package, pinned to **1.4.2** (`s2twp`,
Apache-2.0). Install it into a development virtual environment, not the game.

```text
python -m pip install opencc==1.4.2
python packages/tooling/scripts/generate-native-i18n-traditional.py
node packages/tooling/scripts/generate-native-i18n-content.mjs
node packages/tooling/scripts/extract-native-i18n-npc.mjs
node packages/tooling/scripts/generate-native-i18n-menus.mjs
node packages/tooling/scripts/npc-prose-translations.mjs
node packages/tooling/scripts/npc-prose-translations.mjs --check
node --test packages/tooling/scripts/native-i18n.test.mjs
node packages/tooling/scripts/verify-native-i18n.mjs
```

Run the sequence in order: reviewed terminology and Portuguese corrections are
applied after the initial Traditional conversion. Runtime needs only the
committed JSON catalogues. `shell.json`, `quest.json`, `game.json` and `help.json` have their
own native UI owners and are verified with the generated catalogues.

`source-baseline.json` pins the seven gameplay manifests used by this review.
Generation and verification fail on a changed source hash. Update that baseline
only alongside review of the affected source copy; generation never rewrites it.
The generator preserves existing English, Simplified Chinese and Spanish shared
translations, adds Traditional Chinese, and applies explicit Portuguese fixes.
The shared bundle and web mirror must remain byte-identical.

## Authored content coverage

| Source | Records | Translated nonempty fields |
| --- | ---: | --- |
| Items | 1,628 | 1,628 names; 233 tooltips (202 unique source texts) |
| Monsters | 555 | 555 names |
| NPC identities | 375 | 375 names |
| Skills | 109 | 109 names; 102 available descriptions |
| Maps | 464 | 463 names; one source title is empty |
| Original quests | 154 | 154 titles, 149 descriptions, 141 task blocks, 4 completion blocks |

The original quest payloads have no nonempty return-text blocks. Seven skills
have no source description in the imported shared bundle: `BattleCry`,
`BindingShot`, `CatTongue`, `FireBounce`, `MentalState`, `MeteorShower`, `Portal`.
Their names are translated; no invented effect, damage or level explanation is
added. The generator resolves existing legacy description keys for
`DoubleSlash`, `FatalSword` and `Haste` instead of counting them as missing.

`common.json` includes all 2,355 shared English source keys plus native common
labels. Stable keys preserve item/NPC/monster/map IDs and skill enum identities.
For example, `ThunderBolt` (雷電術 / Raio) and `Lightning` (疾光電影 /
Relâmpago) remain separate skills. Map filenames, packet IDs, command targets and
numeric gameplay requirements are unchanged.

## NPC text extraction

The 375 NPC records reference 364 unique scripts, including 24 gated `GM/`
scripts. Their INSERT closure contains the same 364 scripts; 270 unreferenced
scripts are outside this corpus. No script is executed by these tools.

From 7,832 `SAY`/`ELSESAY` occurrences, extraction finds 1,962 unique source
lines, **1,461 body texts and 607 menu labels**. All have three-language copy.
Eighteen additional body variants model existing runtime removal of unsupported
angle-bracket tokens. The final prose catalogue therefore has 1,479 entries.
These counts are visible strings, not conditional commands, script nodes, or
proof that all pages are reachable in ordinary play.

Each source record retains script/label/line provenance and menu targets as
audit metadata. Captures become named opaque placeholders. Both plain display
aliases and original colour-markup aliases are retained, so runtime translation
does not strip colour-like text from a player's interpolated name. Unsupported
named tokens are omitted only in separately identified variants matching the
current server behavior; this does not implement missing script features.

Known malformed source text is explicitly classified rather than counted as
English prose: `[@before1>`, a directionality-only mark, and `SET [526] 1`
authored inside a SAY block. The latter remains displayed text, never a command.
The original monster placeholder `00` and invalid item tooltip `wont` are also
identified source defects. Proper names and same-form Portuguese words have
explicit reasons; arbitrary untranslated sentences fail verification.

## Verification boundaries

The strict verifier requires all eight native catalogues, all three nonempty
values, unique stable keys, identical named/numeric placeholder multiplicities,
unchanged displayed command tokens, every shared key, and current NPC sources
including runtime variants, plus all 42 help pages and 39 shortcut explanations
(124 help keys including the scroll hint). It reports exact file hashes and alias owners in
`packages/game-data/data/native-i18n/validation-report.json`.

Identical aliases with identical translations are valid. Conflicting aliases
retain all keys and values for review: canonical content uses stable IDs; NPC
menu/prose lookup uses its own domain; ambiguous short labels such as `Return`,
`All` and `New` require explicit UI keys. A nonempty dictionary is not proof
that a UI call site uses the correct context. Numeric .NET-style placeholders
retain their values; the native template matcher must recognize their restricted
format suffix without interpreting colour markup as a parameter.

This verification does **not** accept font glyph coverage, Portuguese wrapping,
texture-baked English labels, installed locale persistence, reconnect behavior,
or every native call site. Native integration tests and actual rendered
screenshots establish those separately. No game-wide translation or player
acceptance is inferred from key counts alone.
