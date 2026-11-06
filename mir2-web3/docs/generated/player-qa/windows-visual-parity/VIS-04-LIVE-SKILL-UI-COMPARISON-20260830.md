# VIS-04 Windows/Crystal live skill UI comparison — 2026-08-30

Status: **NOT ACCEPTED**

This report records a bounded, same-fixture desktop comparison. It does not
claim that every class skill, the whole frontend, or the whole game has reached
Crystal 1:1 parity. `globalParityPercent` remains `null`.

## Fixture and evidence

- Branch base under evaluation: `d10611a5bcd12abe8f8ba606a807bbfb00d2a524`,
  plus the uncommitted Windows fixes named in this report.
- Original Crystal: account `QA0429A`, character `QA0429Hero`, level 26 Wizard,
  FireBall level 3 on `F1`, Lightning level 3 on `F2`, Bichon near `289,617`.
- Windows native: isolated Gateway `ws://127.0.0.1:7112/ws`, account `demo`,
  character `Scout`, level 26 Wizard, Bichon near `288,618`.
- Original UI capture:
  `artifacts/visual-acceptance/skill-live-postfix6/crystal-skill-panel-same-fixture.png`.
- Windows UI capture:
  `artifacts/visual-acceptance/skill-live-postfix6/windows-fireball-cooldown-ui.png`.
- Final Windows FireBall trace:
  `artifacts/visual-acceptance/skill-live-postfix6/native.stderr.log`.
- The acceptance account store and recordings are local QA artifacts and are
  intentionally not release evidence or source-controlled fixtures.

## Deterministic comparison

| Check | Original Crystal | Windows native | Result |
|---|---|---|---|
| Skill-panel shortcut | `F11` opens/closes the SPELLS panel | `F11` now opens/closes the native skill panel | **PASS after fix** |
| Primary skill hotkeys | Unmodified `F1`-`F8` | Previously required Ctrl/backquote; now unmodified `F1`-`F8` | **PASS after fix** |
| Server-learned skills | FireBall/Lightning appear and bind as `F1`/`F2` | Authoritative `knownSkills` now reaches the Bevy skill resource and resolver | **PASS after fix** |
| FireBall resource cost | MP `424 -> 415` | MP `424 -> 415` | **PASS** |
| FireBall owner packet path | One cast | Compact owner `Magic` is rehydrated from authoritative source state | **PASS after fix** |
| FireBall compatibility echo | One visible/audio cast chain | Owner `Magic` plus Zone-remapped `ObjectMagic` previously duplicated the effect; final run consumes the nonbroadcast echo | **PASS after fix** |
| FireBall effect topology | Cast startup, bright projectile, impact, cleanup | Final trace contains exactly one sequence (`seq=53`) with cast, projectile and target phases | **FUNCTIONAL PASS** |
| FireBall exact visual/audio parity | Crystal timing, brightness, scale and sound are the reference | Not yet frame/audio-measured to a signed tolerance | **OPEN** |
| Lightning functional lane | Learned and castable on `F2` | Live cast observed with MP `93 -> 34` and visible local effect | **PARTIAL PASS** |
| Skill cooldown after reconnect | Cooldown expires on the server clock | Persisted session-relative timestamps can reopen as permanent `cooldown` | **FAIL** |
| Skill panel placement | Upper right | Upper left | **FAIL** |
| Skill panel identity/header | Character name, avatar/weapon area | Blank identity/header area | **FAIL** |
| Skill icons and rows | Crystal icons, full names and native row geometry | Text-only rows, truncation and different spacing | **FAIL** |
| Skill pages | Learned skills start on the first populated page | Placeholder starter skills force learned skills to page `2/2` | **FAIL** |
| Assignment popup | Opaque Crystal frame and buttons | Transparent/incomplete popup presentation | **FAIL** |
| Complete skill denominator | All Crystal class skills and interactions | Only FireBall and Lightning were exercised in this run | **OPEN** |

## Final FireBall trace proof

The final live run received both compatibility packets but emitted only one
native effect sequence:

- line 16277: compact owner `Magic` for FireBall;
- line 16279: one native `ObjectMagic` effect event, `seq=53`;
- line 16284: Zone `ObjectMagic { selfBroadcast: false }` with remapped
  `objectId=50000`, consumed as the same cast;
- lines 16343-16678: only `seq=53`, progressing through `cast`, `projectile`
  and `target` frames.

There is no second FireBall effect sequence for that cast. This closes the
native owner/observer echo duplication defect, not the full visual-parity gate.

## Automated regression

All focused tests passed (`1 passed`, `0 failed` for each):

- `owner_magic_rehydrates_authoritative_source_for_native_cast_effects`;
- `owner_magic_fails_closed_without_authoritative_source_context`;
- `fireball_object_magic_owns_cast_delayed_projectile_impact_and_three_sounds`;
- `overlay_keyboard_toggles_skills_with_f11` with `native-ui` enabled;
- `crystal_primary_skill_bar_uses_unmodified_function_keys`;
- `f1_selects_a_server_learned_skill_with_target_and_direction`;
- `authoritative_world_skills_are_forwarded_to_the_native_skill_resource`.

## Acceptance consequence

The FireBall Windows lane is now functionally connected end to end and no
longer duplicates its owner effect. That is a bounded pass. Overall skill/UI
acceptance remains **NOT ACCEPTED** until at least the cooldown persistence
defect, Crystal panel placement/header/icons/assignment UI, complete class-skill
denominator, same-EXE authenticated live WSS, real-DPI checks, 30-minute native
soak, human visual/audio/feel comparison, legal asset closure, production
installer/updater, and formal publisher signing are closed.
