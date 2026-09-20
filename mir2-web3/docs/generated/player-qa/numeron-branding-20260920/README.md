# Native map fixes and numeron branding — 2026-09-20

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

The user stopped Computer Use with physical Escape before new-package visual
verification. Neither the repaired map nor the new branding is visually
accepted yet. No original Crystal side-by-side acceptance is claimed.
Repository-wide format check reports extensive existing unrelated differences;
new standalone Rust branding/build files are formatted without rewriting them.
