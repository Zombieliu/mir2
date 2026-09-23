# Native map fixes and numeron branding — 2026-09-20

## Blank Diary after profile-enabled client launch

The user then reported an empty Diary with chapter 0/4. Read-only inspection
of the running Gateway's four non-secret configuration values confirmed:
`MIR2_CONTENT_PROFILE=platinum_176`, but `MIR2_QUEST_CADENCE`,
`MIR2_SPECTATOR_ENABLED`, and `MIR2_SPECTATOR_RECORDING_ENABLED` were unset.
`quest_log_snapshots` filters out saved V2 quests when server V2 cadence is
disabled. Enabling only the client profile cannot restore those filtered rows.

After the user explicitly logged out, the account store was privately backed
up and the same Gateway EXE/store restarted through the existing
`apps/web/scripts/quest-agent/launch-newcomer-v2-gateway.ps1`, with TCP19900,
WS19910, newcomer-v2 cadence and spectator recording disabled. Both ports
listen under replacement PID 20200. Saved a1 is still level 3, N1/N2 completed,
N3 in progress at 2/4. Post-login visual confirmation is pending.

The unconfigured Gateway had also been recording spectator frames to E: until
the disk filled. A closed previous recording was retained on C: to free space;
recording is now disabled for this ordinary-player local test. No character
state was edited. Logs and private backup remain local under
`C:/mir2-task-panel-recovery-20260920`; do not commit the private account backup.

## Direct-launch task follow-up

The branded EXE was launched and its title-bar name/icon were visually seen.
The copied local config still targeted WS19810, while the running Gateway
listens on WS19910; the local package config was corrected. The user stopped
Computer Use before the final login capture.

Direct launching also omitted the temporary `MIR2_QUEST_GUIDANCE` environment
used by the previous QA launcher. The Windows host now accepts a persistent
`[gameplay] quest_guidance = "newcomer-v2"` setting and installs both matching
QuestGuidance and NewcomerJourneyCatalog resources. Unknown profile values
fail config parsing; absent configuration preserves the environment fallback.
This selects presentation only and does not modify authoritative quest saves.
Session-config suite passes 11/11, including persisted V2 selection and unknown
profile/key rejection. The first test link failed because E: had no free space;
an incremental build cache was retained on C: and the repeat passed.
Release build passed. Corrected package:
`C:/numeron-legend-of-rebirth-20260920-quests/mir2-platform-windows.exe`, with
WS19910 and persisted V2 guidance in the adjacent `mir2-client.toml`.
The currently running earlier EXE is not hot-patched; the corrected package
must be opened for the persisted profile to take effect.

Read-only saved-character check: `a1`, Warrior level 3, completed 2110001 and
2110002; 2110003 is in progress at 2/4, with `kill:39 = 2` (Scarecrow).
The remaining objective is two RakingCat (44). No saved state was edited.

- Exact user-requested window/product name: `numeron-legend of rebirth`.
- Icon artwork: existing `apps/mir2-launcher-tauri/src-tauri/icons/icon.png`
  and `icon.ico`, copied into the Windows host's resources. No new logo design
  is claimed. PNG is embedded for Winit's window/taskbar icons; ICO and version
  metadata are compiled into the Windows PE resource section.
- Icon application waits for the actual primary Winit window and runs on the
  main thread once per window, independently of external game assets.
- Existing packaged EXE filename remains `mir2-platform-windows.exe` for
  launcher/script compatibility; the user-visible product name is updated.

## Map and minimap repairs included

The Bichon market cell `(290,600)` contains sand Back `WemadeMir2/Tiles#3`
and grass Middle `WemadeMir2/SmTiles#123`. Previously both received the same
floor depth. Separate ordered floor-layer bands prevent the Back covering
the same or neighboring Middle cells. Map-parser suite: **39 passed**.

Dead-player V was handled as both TownRevive and Minimap. The overlay now
reserves that V press for revival; alive V still toggles the minimap.
Focused native UI regression: **1 passed**. On the old live client, pressing
V while alive restored the minimap, confirming its collapsed state.

## Acceptance boundary

Release build with Rust 1.95.0 passed. PE resources were parsed and contain
RT_ICON (3), RT_GROUP_ICON (14), and RT_VERSION (16). Windows FileVersionInfo
reports both ProductName and FileDescription as `numeron-legend of rebirth`.
Package: `C:/numeron-legend-of-rebirth-20260920/mir2-platform-windows.exe`.
SHA-256: `C0C0BAAB957ADA58E85D5C22A7BD360E2541FC76E19DAB2301BE0D9BD121F29A`.
This local package reuses the existing asset directory via a junction and
the existing local Gateway config; it is not a standalone redistribution.

The title and title-bar icon were subsequently seen on the launched client.
The user stopped Computer Use with physical Escape before authenticated
new-package visual verification. Map/Diary and original Crystal side-by-side
acceptance remain open.
Repository-wide format check reports extensive existing unrelated differences;
new standalone Rust branding/build files are formatted without rewriting them.
