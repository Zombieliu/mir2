# VIS-03 Windows Main HUD Interaction Parity Report

Date: 2026-08-30

Scope: source-backed, non-visual Windows-native main HUD interaction work on
`codex/windows-visual-parity`. This report does not close human or same-EXE
visual acceptance.

## Source evidence

- `Crystal/Client/Forms/CMain.cs:514-568`
  - one shared hint surface;
  - 50% black background, `RGB(144,144,0)` border, yellow text;
  - cursor offset `(-text width,+20)` with screen-edge clamping.
- `Crystal/Client/MirScenes/Dialogs/MainDialogs.cs:65-190`
  - Character, Inventory, Skills, Quests, Options, Menu and Game Shop hints.
- `Crystal/Client/MirScenes/Dialogs/MainDialogs.cs:1265-1450`
  - seven chat filter buttons, Size, Chat Settings and the separate Trade
    request button.
- `Crystal/Client/MirScenes/Dialogs/MainDialogs.cs:1817-1826,2089-2108`
  - `Prguse/544` unread-mail indicator, 500 ms alternation and ten hidden-edge
    settling rule.
- `Crystal/Client/MirScenes/Dialogs/MainDialogs.cs:2022-2070`
  - minimap 2090/2091 expanded/collapsed frames and footer y=131/y=22.
- `Crystal/Client/MirScenes/Dialogs/InventoryDialog.cs:600-723`
  - horizontal/vertical belt frames, six stable slots, Rotate, Close and
    source-local control behavior.
- `Crystal/Client/KeyBindSettings.cs:227,293`
  - `Z` toggles belt visibility and `Ctrl+Z` rotates it.

## Implemented in this round

- Added one non-interactive, high-z Crystal hint overlay shared by HUD and chat
  controls. It uses the source colors, window-logical cursor coordinates and
  four-edge clamping. New or resized text stays hidden for one layout pass,
  while a non-hovered overlay remains measured but invisible so same-text
  re-entry cannot position from a zero-sized layout. Logical window-size or
  DPI scale-factor changes also force one hidden reflow pass. A target guard
  prevents the position phase from reviving a stale tooltip; the overlay also
  has bounded width/height and clips overflow.
- Added source-semantic keyless hints to the ten main HUD controls and ten
  source-visible chat controls. Key names are deliberately omitted until the
  native shortcut map itself matches Crystal.
- Added a source-distinct belt-item hint at cursor `(+28,+28)`, with 80% black
  background, `RGB(148,146,148)` border and red border for zero durability.
  Its bounded text contains only fields present in the authoritative
  `ItemModel`: name, grade, quantity, description, durability, attack, defence,
  luck and socket count. Missing requirement/bind/awake/rental/story data is
  not fabricated.
- Added the `Prguse/544` unread-mail pulse driven by unread authoritative mail
  ids: visible on receipt, 500 ms alternation, stable visible after ten hidden
  edges, and reset when unread mail clears.
- Changed minimap toggle presentation from “hide the whole frame” to Crystal's
  2090 expanded / 2091 collapsed frame behavior. Mail, Big Map, coordinates,
  light and unread indicator move with the footer. Maps without an available
  minimap profile force the small frame while preserving the user's expansion
  preference for the next supported map.
- Added working belt Rotate and Close controls, exact 1932/1933 and 1944/1945
  layouts, exact horizontal/vertical control frames, stable slot identity, and
  source keyboard behavior: `Z` toggles visibility and `Ctrl+Z` rotates without
  reopening a hidden belt. A session reset restores the new-scene horizontal,
  visible belt and clears stale new-mail blink state.
- Corrected the chat control-bar denominator to seven filters plus one
  authoritative Trade request. Trade no longer changes the current filter and
  queues exactly one existing Windows trade intent. Failed/no-target requests
  no longer leave a persistent pending key that blocks future requests, and
  the legacy serialized Trade visibility bit no longer corrupts the settings
  dialog's All-button visual.
- Corrected the typed control registry from 174 to 177 entries to record the
  separate Trade, Belt Rotate and Belt Close controls.

## Automated verification

- `cargo +1.95.0 test --manifest-path apps/game-client/client-bevy/Cargo.toml --features native-ui crystal_ui:: --quiet`
  - 189 passed, 0 failed.
- `cargo +1.95.0 test --manifest-path apps/game-client/client-bevy/Cargo.toml --features native-ui --quiet`
  - 463 passed, 0 failed.
- `cargo +1.95.0 test --manifest-path apps/game-client/platform-windows/Cargo.toml --quiet`
  - 485 passed, 0 failed.
- `cargo +1.95.0 test --manifest-path apps/game-client/ui-core/Cargo.toml --quiet`
  - 42 passed, 0 failed.
- `rustfmt +1.95.0 --edition 2021` and `git diff --check` passed for the scoped
  Rust files.

## Gates deliberately left open

- No native client was launched and no screenshot was captured in this round;
  same-EXE hover placement, real-DPI behavior and human visual feel remain
  unverified.
- Bevy currently approximates Crystal's four-sided text outline with one text
  shadow.
- Full Crystal item tooltips need additional authoritative fields and all
  eleven source sections; the belt hint is explicitly a supported-field subset.
  Inventory, equipment, storage and other overlay item cells are not wired to
  this shared hint surface yet.
- Belt pointer semantics still use the existing native left-click route.
  Crystal's left-select/drag and right-click-or-double-click use semantics are
  still open.
- Exact Crystal shortcut wiring, ButtonA/ButtonC sounds, chat physical-line
  wrapping, A/P/S mode labels, the eight disabled system-menu surfaces and
  complete menu/panel visual parity remain open.
- The older Trade panel's separate request path still uses its existing pending
  lifecycle; this round fixes only the source ChatControlBar Trade button.
- Authenticated same-EXE live WSS, real DPI, 30-minute native soak, human
  visual/audio/feel, complete semantic denominators, production installer/
  updater, legal asset closure and formal publisher signing remain open.
- `globalParityPercent` remains `null`; this report is not a strict 100% or
  full-game parity claim.
