# Windows notice and nameplate text-layout regression report

Date: 2026-08-30

Status: bounded Windows-native regression fixed and automatically captured;
exact Candidate and human visual acceptance remain open.

## User-observed regression

Two independent layout mistakes made text appear to drift outside its authored
Crystal position:

- The login notice rendered a 420 px-wide Bevy row inside a 316 px Crystal
  panel without parent clipping. A long sentence therefore painted through the
  panel border and over the world.
- Underscore-separated NPC and monster names were joined into one Bevy
  multiline text node. Bevy then selected the font-engine line height and one
  colour for the whole node, while Crystal creates one `MirLabel` per row,
  offsets each row by exactly 12 px, and renders later NPC rows white.

This was not diagnosed as a global DPI or Arial substitution issue. The
incorrect width, overflow and multiline ownership were explicit in the native
layout code.

## Source-aligned correction

- The notice panel and body rows now clip overflow before Crystal's scrollbar
  gutter. Notice content is pre-wrapped by Unicode display width while
  preserving explicit blank rows and splitting oversized tokens.
- The project-owned default Candidate notice is authored with stable line
  breaks so the same content remains inside the body denominator.
- NPC and monster names now use independent no-wrap text nodes. Their first
  row retains the existing Crystal base offset and multi-row centering term;
  subsequent rows advance by exactly 12 px. Later NPC rows use white, matching
  `NPCObject.CreateNPCLabel`.
- The native capture harness has a `notice-open` state target. It waits for the
  authoritative in-game notice instead of taking a generic early in-game
  screenshot.

Crystal source references:

- `Crystal/Client/MirObjects/NPCObject.cs`, `DrawName` and
  `CreateNPCLabel`: split rows, `s * 12`, and later-row colour.
- `Crystal/Client/MirObjects/MonsterObject.cs`, `DrawName`: split rows and
  `s * 12`.
- `Crystal/Client/MirObjects/MapObject.cs`, `DrawName`: single-row anchor.

## Automated evidence

| Gate | Result |
|---|---|
| Windows native host suite, Rust 1.95 | PASS, 485/485 |
| Notice layout/state focused suite with `native-ui` | PASS, 9/9 |
| Simulation login-notice contract | PASS, 3/3 |
| Native entity-overlay focused suite | PASS, 11/11 |
| Rust formatting and scoped diff checks | PASS |

An isolated local Gateway and freshly built native debug client produced the
following state-gated capture without mouse or keyboard control:

| Evidence | Value |
|---|---|
| Scene | `notice-open` |
| Logical size | 1024 x 768 |
| Captured DPI scale | 1.0 |
| Captured UTC | `2026-08-29T15:52:39.771Z` |
| PNG SHA-256 | `090a1eafd14a08116ef51a4dcbd95d054cc2041458a573caf5e0431aafd596b5` |
| Local PNG | `C:\Users\Administrator\AppData\Local\Temp\mir2-notice-qa-a8f1c2\captures3\notice-and-nameplate-regression-notice-open-1788018759771-1.png` |
| Local sidecar | `C:\Users\Administrator\AppData\Local\Temp\mir2-notice-qa-a8f1c2\captures3\notice-and-nameplate-regression-notice-open-1788018759771-1.json` |

The screenshot confirms that every default notice row stays inside the panel
and that visible split NPC name rows no longer rely on Bevy multiline spacing.
The sidecar correctly marks this debug capture `eligible: false` because it is
not bound to an attested Candidate run ID, source revision, executable hash or
asset-manifest hash.

## Explicitly open gates

- exact packaged same-EXE screenshot bound to Candidate provenance;
- authenticated live WSS path;
- real Windows DPI matrix rather than only captured scale 1.0;
- 30-minute native soak;
- human Crystal side-by-side visual, audio and feel acceptance;
- complete frontend and full-game semantic denominators;
- production installer/updater, legal asset closure and formal publisher
  signing.

This bounded result does not set or imply a global parity percentage and does
not close the Draft PR.
