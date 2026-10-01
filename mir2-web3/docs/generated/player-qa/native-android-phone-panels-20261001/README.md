# Android phone panel workspace — 2026-10-01

Bounded G4 presentation leaf after published source `4ec9b2df8`.
The frozen Windows baseline remains `3f5e61533235921369bc13a7760b4a56b0e467e5`.
The Windows-completeness goal remains **Active**, not complete.

## Baseline and source change

Actual v10 preview APK/source `452398d4c` showed BAG covering status/chat/left
joystick, and a small CHAR/SPELLS window. The unchanged original screenshots
are retained as `inventory-before.png` and `character-before.png`; their exact
APK identity is in the preceding item-geometry QA checkpoint.

- A common Android logical-pixel workspace is used by both panel focus and
  phone HUD adapters, rather than fitting the panel to the entire viewport.
- On supported wide landscape phones, the shared BAG/CHAR tree stays in the
  center; status, one visible chat history row and all six original belt use
  targets occupy a left lane; both thumb-control footprints stay clear.
- The original belt targets/counts reflow to 3x2 at48dp each, keeping shared
  actions, item identity and the user's orientation/visibility preferences.
  Closing the panel restores the normal phone HUD. History is not discarded.
- All four shared character pages use the same real focused panel. The only
  shared source change exposes `OverlayEquipment` to the Android host; there
  is no Windows layout, rule, model, authentication or packet modification.
- Authoring coordinates, image bytes and model state are not rewritten.
  Shared ordinary Button picking uses the moved real Bevy nodes; inventory
  drag continues through its existing inverse panel transform. Actual APK
  tab/cell/drag checks remain a separate gate below.
- Sidebar eligibility is restricted to ordinary core inventory/character
  views. Storage/trade and their companion windows do not share this lane.
  Keyboard/chat-focus and insufficient short/notched areas keep the previous
  layout, explicitly **not** a complete compact-phone or IME acceptance.
- Diagnostic version11 is `0.1.8-phone-panels`. Build/runtime evidence follows
  a clean source commit; setting this version alone proves no APK.

## Source checks and retained failures

- Failure-first focused run: both new character/bag tests FAIL on the old
  focus implementation; `before.log` records tiny CHAR and status occlusion.
- Final Android249/249 and ui-preview260/260, no ignores/failures.
- Shared native-player UI1203 pass /8 existing ignored, fresh serial run.
- Both actual arm64/API31 normal and ui-preview-feature target checks PASS.
- Five new tests cover all four character pages at densities1/2.75/3.5,
  unchanged authored geometry/close reset, BAG workspace bounds, safe insets,
  all six belt targets/counts/preferences and restored HUD/pointer bounds.
- `first-compile.log` retains a test's private-field misuse. `query-arity.log`
  retains the initial Bevy tuple-limit error, repaired with nested disjoint
  filters/data. `normal-float-assertion.log` and `preview-float-assertion.log`
  retain a too-strict new test (33.999996 versus34.0); only its numeric tolerance
  was corrected. These are not silently relabelled as final passes.
- No Java behavior changed; prior Java33+33 is historical, not newly rerun in
  this source leaf. Package compilation is a separate check.

## Open acceptance denominator

This source leaf does **not** close G4: compact landscape reflow, all window
controls' minimum touch sizes, character/body/skill tabs and scrolling, login
forms, IME, all shared service dialogs, languages, audio/performance, multi-finger
runtime evidence and whole UI remain OPEN. Source-only coordinate tests are not
device measurements. Current diagnostic asset packs remain partial/offline.

Actual v11 APK hashes, unchanged resource identities, installation, before/after
captures and click/drag receipts will be appended after building the exact clean
commit. No online login/WSS, server-authoritative player loop, real item custody,
physical-device or human acceptance follows from the offline specimens.
