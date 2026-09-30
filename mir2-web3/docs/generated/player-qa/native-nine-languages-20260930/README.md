# Nine-language native UI and Windows launch diagnostics — 2026-09-30

The native client supports selecting en, zh-TW, pt-BR, ru, hi, id, vi, th and ar in one binary. These are implementation and automated visual results, not native-speaker acceptance or clean-PC launch acceptance. Public signing is still an external release blocker for the reported application-control failure.

## Verification

| Check | Result |
| --- | --- |
| Shared native client unit suite | 1,153 passed, 5 explicitly ignored GPU/live fixtures |
| Windows host unit suite | 784 passed, 4 explicitly ignored GPU/soak/live fixtures |
| Native catalogue tooling | 14 passed; all six extra locales have exactly 9,385 source keys, intact parameters/commands/numbers, and matching generation provenance |
| Production shell GPU | 40 captures: four screens × nine locales, two nine-choice popups, two existing Portuguese panels; zero missing glyphs; system-font discovery disabled |
| Shell switch retention | 270 selections / 1,080 screen rebuilds after warmup; pinned handles 9, registered assets 10, retained source identities 5, 40 atlas pages / 41,943,040 bytes stayed constant |
| Production game widgets GPU | 36 captures: options/shop/bag, character stats, shortcut help, long help × nine locales; no missing glyphs, text overflow or offscreen nodes |
| Production quest/NPC GPU | 63 captures: tracker top/bottom, longest detail top/bottom, opaque player-name NPC page, real menu top/bottom × nine locales; no missing glyphs or row overflow; original command targets preserved |
| Windows bundled-font GPU | Nine captures, 180 switches, no system fonts; shaped Arabic/Devanagari/Thai and accents; font/source/atlas baseline unchanged |
| Installer language recipe | All nine languages contain 281 current Inno messages and 14 custom messages; compiler-only fixture passed |
| Package guards | Complete verifier SelfTest passes on PowerShell 5.1 and 7, including exact-font, Unicode/ASCII/UTF-16 path leaks, adjacent real paths, malformed font, ADS/CMS and closure controls |
| Launch diagnosis | 63 controlled read-only fixtures; no report from the affected laptop yet |
| Publisher signing | Unsigned EXE rejection and certificate preflight checks pass; no public publisher certificate available, no actual public signing claimed |

The passing logs and JSON reports are stored beside this file. The complete local capture set has **148 PNG files** at `C:/mir2-ui-repair-20260921/native-i18n-nine-20260930`; [evidence-files.json](evidence-files.json) records each hash and byte size. A representative 26-image subset is checked into this evidence folder, including a tracker in every language. Earlier failed runs remain in the local evidence root with their original names; they are not counted as passes.

## Defects found and fixed during integration

- The in-game Options overlay rebuild discarded the open language-menu visibility. Visibility now reapplies to newly rebuilt menus in PostUpdate; the multi-rebuild interaction regression passes.
- Shop search now accepts the translated display name as well as the original name while keeping the original item index for purchases.
- A slot-only Arabic translation accidentally acted as a catch-all text template. Authored pet wording is restored, and generic template matching rejects patterns lacking literal letters. Opaque names and chat-like text remain unchanged; wrapped NPC arguments are never translated a second time.
- The small popup-arrow glyph was not in the bundled font. It now uses an ASCII chevron, which all included faces cover.
- A 55-pixel original bitmap-title rectangle wrapped the Russian quest heading. The translated detail title uses the otherwise empty title strip, with an eight-pixel gap before Close; original geometry, hitboxes and body text stay intact.
- Padding caused the Portuguese help paragraph's measured height to omit a wrapped line. The paragraph's right-side guard is outside its fixed content width, so measurement and wrapping agree.
- Quest wrapping/truncation preserves complete grapheme clusters rather than separating Indic/Thai combining marks or ZWJ sequences.
- The Windows shaping fixture previously mistook Parley's per-character ranges for complete shaping groups. It now checks actual ligature groups, exact UTF-8 coverage, nonzero glyphs, valid positions and a difference from isolated-character shaping; production font loading did not change for this test correction.
- PowerShell 5.1 decoded a Unicode negative-control path as ANSI, inserting an invalid question mark. ASCII-source character-code construction restores the intended fixture. Production scanning and pinned-font checks remain strict.

## Representative visual review

- [Arabic language menu and mixed Latin identity](shell-04/shell-ar-language-popup.png)
- [Hindi registration](shell-04/shell-hi-registration.png)
- [Thai character creation](shell-04/shell-th-create.png)
- [Russian full quest heading](quest-06/ru-detail-top.png)
- [Hindi original NPC menu](quest-06/hi-npc-menu-top.png)
- [Arabic tracker with routes, coordinates and resources](quest-06/ar-tracker-top.png)
- [Portuguese help paragraph](game-05/game-pt-BR-help-detail.png)
- [Thai help](game-05/game-th-help-detail.png)

The fixture viewport is 1024×768, rendered on the local GPU from the real original UI assets. It creates no native window and opens no network connection. It does not modify a player account, save, active installed client or server. Arabic glyph shaping/bidi and paragraph-start alignment are supported; full Crystal-window geometry mirroring is not implemented. Parley 0.9 uses its non-complex-script segmenters and can log missing Thai/CJK word-segmentation models; bounded UI rows use grapheme-safe wrapping. This evidence does not certify Thai linguistic word breaking, all DPI scales, the reported Alienware laptop, or live combat.

Large amounts of legacy NPC/content prose remain explicitly marked offline machine-translation drafts. Key completeness and intact gameplay numbers do not establish semantic accuracy. See [implementation, provenance and remaining language review](../../../NATIVE-MULTILINGUAL-NINE-LANGUAGES.md).

## Release boundary

The old r4 installer and installed clients remain unchanged. The prepared r5 recipe is a nine-language internal Candidate until its actual build receipt is recorded. It must not be described as a publicly trusted installer or a verified fix for error 4551. Candidate CMS integrity signatures and embedded Windows Authenticode publisher signatures are separate. The launch diagnostic only reads selected EXE/policy events; it never changes protection, imports trust roots or launches the game elevated. Capacity work remains paused.
