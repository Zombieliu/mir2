# Error review overrides — six-language expansion

This file records the bounded manual correction of the explicit errors from `keyed-review.json`; it is not a whole-catalogue or native-speaker acceptance claim.

- Input: `C:/mir2-build/i18n-expansion-drafts-v2/keyed-review.json`.
- Input SHA256: `2ae5107a2a778a67830e2425193adad020c3904ff485b30ae4424d37f7524771`.
- Input findings: 313 error rows, 148 unique keys, 172 unique key/locale pairs.
- Corrections: 148 keys × six locales (`ru`, `hi`, `id`, `vi`, `th`, `ar`) = 888 manually authored/reviewed values. Other-language values on these same keys were also supplied to keep their parameter/number-sensitive wording consistent.
- Source-key counts: `{"common.json": 32, "content.json": 78, "npc-menus.json": 3, "npc-prose.json": 35}`.
- Output SHA256: `5ae75fb662f195febad24d139444a53675faf5b85f7e8c449f7288865f6ebadb`.

## Boundaries and source handling

The English source (or the literal source for `ui.languageChinese`) remains authoritative. No catalogue keys, gameplay values, server data, source strings, or runtime logic were changed. Existing `ui-overrides.json` and `content-overrides.json` were checked again before freeze: there are no overlapping conflicting fields.

- Numeric placeholders, named NPC arguments, triple-brace `{{{0}}}` formatting, literal percentages, durations, counts, and displayed commands are retained. `Example@Example.Com` and `@GuardsHelp` are exact source tokens.
- Written-out numbers remain words. In particular the Archer One–Twelve labels, five pieces of meat, and two-hour limit were not converted into new digit sequences.
- NPC key `npc.prose.e76d0bc0e98b2ad5` is the original single U+200E LEFT-TO-RIGHT MARK in all six values. It is not translated or removed. The `|`, `||`, and `|  |` structural strings are also exact literals.
- The source fundraising URL, including its path/query, is copied exactly while its visible label is translated. It was not opened or contacted.
- The slash-prefixed Ghoul Cave and WhiteDragonPassage source strings retain their slash prefix. The latter's existing missing-movements note is translated; this does not assert that the runtime has a movement defect.
- Runtime-empty NPC texts retain empty value slots and do not invent gold amounts or HP/MP values. `npc.prose.ad828c27ce4dcca8` already lacks a word after “I can”; the translated blank is retained rather than inventing an amount or a new interpolation parameter. The catalogue itself is not repaired here.
- Partial NPC sentences remain partial; no missing destination is invented. The source wording “Beginner of Mid Level skill book” is rendered as beginner/intermediate skill books without adding a seller or location.
- The authored `5×5 radius` and original item package stats/durations are preserved. No attempt was made to correct gameplay semantics from prose.
- Reviewed skill names for CrippleShot, OneWithNature, TurnUndead and SwiftFeet were taken from the existing content glossary. The different literal `TwinDragonBlade` remains its source spelling; it was not silently equated with TwinDrakeBlade.
- Character/item/species proper tokens such as Oma, Mir, Keratoid, CanniTea, CannibalLeaf and CannibalSeed retain their canonical name component where no separate approved term was available. Names supplied here are display strings only.

## Verification performed

Read-only Node verification imported the existing `loadSources` and `validateValue` functions from `native-i18n-expansion.mjs`. It validated every one of the 888 values against its current source, without changing any parameter/number/command rule. Result: zero validation errors, zero unknown keys, zero conflicts with the two existing override files, and all 313 original error rows covered. A separate literal scan found no `[Xn]`, ASS subtitle-style directives, `<unk>`, or replacement-character pollution. `git diff --check` passed.

No generated overlay, game build, Cargo test, GPU run, network translation, installation, live service change, or source-catalogue edit was performed. Human language/style and in-game visual acceptance remain separate.
