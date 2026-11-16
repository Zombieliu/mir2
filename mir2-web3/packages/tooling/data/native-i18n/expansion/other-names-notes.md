# Remaining content-name review — Russian, Hindi, Vietnamese, Thai

Bounded input: the `content.*` rows in `identicalNeedsReview` from `C:/mir2-build/i18n-expansion-drafts-v2/keyed-review-2.json`, for `ru`, `hi`, `vi`, and `th` only. No existing curated field was overwritten.

- Input SHA256: `92bf3807c3c5ed5c370c9c6efa70e5cabb3c1d596f26e7211037e5222b713a3e`.
- Scope: 145 catalogue keys, 213 key/locale pairs, 106 distinct source strings.
- Locale counts: `{"hi": 87, "ru": 44, "th": 13, "vi": 69}`.
- Output SHA256: `f631d5a3827f2d8c190205e0b962be2f6a78f34a22c639db75071cce2901efbc`.
- Output is partial by design: keys contain only the assigned missing locale fields. Arabic and Indonesian are handled separately by the parent task.

## Terminology and limits

Gender/size/category descriptors are translated, while authored numbers and suffixes are retained. Repeated named families are consistent across numeric variants; no new effect, map, floor, skill, or grade was inferred.

All `GT 1F`, `GT 2F`, and `GT 3F` map entries retain the complete original `1F`/`2F`/`3F` code. Localized floor words are added for readability, for example `GT · этаж 1F`. The abbreviation GT is not expanded. All thirty map keys and three assigned locales receive the corresponding exact floor code.

Opaque identifiers/components such as Oma, Mir, Gonryun Eunhyung, Gonryunpasackle, Gonryunyongdrama, WT, ZT, DY, Cl, Nd, and Shi remain identifiable. Descriptive English components are translated without inventing expansions for these names. `Manectric Blest` and `Hwan Ma Jin` use Hindi transliteration instead of guessing a creature class or map meaning.

Three Vietnamese values intentionally retain the exact accepted proper spelling: `Rudolph` (`content.item.1088.name`), `Hydra` (`content.monster.432.name`), and `Lamia` (`content.monster.438.name`). This explicit keyed review does not invent head counts, anatomy, or skill mechanics to force a cosmetic text difference.

Some English source labels are ambiguous in isolation. Existing accepted Traditional Chinese and Brazilian Portuguese catalogue fields were inspected as corroborating context, without changing them:

- `Gobby`: existing `蝦虎魚` / `Góbio` supports Vietnamese `Cá bống`.
- `Keratoid 0`: existing `角蠅 0` / `Mosca cornuda 0` supports `Ruồi sừng 0`.
- `Yob`: existing `野人` / `Bárbaro` supports `Dã nhân`; the numeric variant retains `0`.
- `Hyeoncheon Maseok`: existing `玄天魔石` / `Pedra mágica de Hyeoncheon` supports Vietnamese `Ma thạch Hyeoncheon`; Hindi conservatively transliterates the source name.
- `Excavenger Stuart`: existing `挖掘者 Stuart` / `Escavador Stuart` supports `Thợ khai quật Stuart`.
- `Bossy (F)`: existing costume context supports `Trang phục uy quyền (Nữ)` for both its name and duplicate description.
- The literal source `wont` in `content.item.593.description` and `content.item.709.description` already has the accepted missing-description wording `未提供有效說明。` / `Descrição não fornecida.`. The Vietnamese value `Chưa có mô tả hợp lệ.` follows that established fallback. No item behavior was invented and the source text remains unchanged.

## Verification

A read-only Node check used the unchanged existing `loadSources`/`validateValue` functions. It checked every field in both this file and the error-review file, verified membership in the exact assigned report set, separately required complete GT floor codes, and compared all five present curated override files for conflicts. Result: zero validation errors, zero conflicting fields, zero uncovered assigned pairs. The only exact-source values in this file are the three documented proper names above. JSON parsing and `git diff --check` pass.

No generated overlay, build, Cargo/GPU run, live service action, network translation, or catalogue source edit was performed. These are source-backed developer translations; native-speaker and visual acceptance remain separate.
