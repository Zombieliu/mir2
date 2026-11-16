# Native login resolution and notebook display — 2026-10-01

The login screen offers Auto, 1024×768, 1280×960, 1440×1080,
1600×1200, 1920×1440 and 2048×1536. Choices larger than the current
monitor work area are disabled with a localized reason. Auto considers OS DPI,
taskbar, measured nonclient frame and a margin; all choices preserve 4:3.

The window host retains Bevy scale-factor override1. The window-target world
camera uses a fixed1024×768 projection; UiScale grows all controls/text with the
physical client area. Existing manual cursor paths use CrystalStageTransform
once; tooltips convert cursor/bounds into UI coordinates once. Offscreen lighting
cameras remain separate. The saved choice is bounded, atomically replaced JSON
under the same per-user roaming preference directory as language, outside the
signed game payload. Smaller-monitor/corrupt-preference fallback is Auto.

Actual Winit evidence: [seven cases](native-display-report.json).
The current monitor physically supports1024×768,1280×960,1440×1080,
1600×1200 and Auto. Every case checks native client size, fixed world projection,
UiScale, physical button hit, world/UI reference alignment and tooltip placement.
1920×1440 and2048×1536 are correctly unavailable on this monitor; they are not
claimed as physically tested here. A 2560×1440 fullscreen case has a centered
1920×1440 viewport, followed by successful restoration of the saved window size.
Captures are an owned test application,
not the user's saved character or authenticated gameplay.

Focused host display checks6/6 and shared policy/UI checks7/7 pass. Full serial
regressions pass: shared1168/1168 with8 explicit ignores and Windows790/790
with5 explicit ignores. The first parallel native run had3 existing gateway
global-state failures; all3 pass individually and the full serial run passes.
Both original failed and serial logs remain in the external build evidence.
[Nine-language GPU evidence](display-layouts.json) passes all72 captures:
four display profiles × nine locales × Login/HUD, using bundled fonts and real
r6 UI assets. Missing glyphs, layout overflow and control-hit failures are zero.
All36 HUD reference positions/sizes scale consistently (physical rounding ≤1px).
Selected Login/HUD captures accompany the report; this fixture deliberately
uses a reference rectangle and does not load a gameplay map or saved character.
Clean attested Candidate07 and actual r7 installer are built from
`fefd18370a947848d4f2f86ee3468ea4251fe011`; the strict package verifier passes.
See the [updater and installer handoff](../native-updater-20261001/README.md).
Actual affected-laptop and full-installer human acceptance remain open.
This changes presentation only, not protocol, server saves or capacity.
