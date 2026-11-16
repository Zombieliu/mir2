# Inventory movement and HookingCat — 2026-09-21

User reported inability to move after a potion with inventory still open, and
several HookingCat names without sprites while Deer and RakingCat rendered.

## Causes and repairs

`UiState::blocks_world_click` intentionally returns true for any open panel.
The Windows pointer sender used that global gate, including the non-modal bag.
It now checks the bag's actual moved bounds in Crystal stage coordinates.
Outside clicks may emit movement; bag clicks, window/item dragging, pending
item operations, inspection and other modal guards retain capture. Other
panels and keyboard rules are unchanged; no potion cooldown or server
movement restrictions were removed.

HookingCat uses imported monster image 6. `Monster/006.Lib` contains 224
drawable frames and correct existing animation metadata, but its exported
PNG directory and atlas entries were missing. The native standalone fallback
does not handle Monster libraries. Exported all original frames and appended
the `hooking-cat` atlas page, preserving the old seven-page starter entry.
The builder now supports idempotent `--append`, and default roots/monster
closure include 006 and all seven relevant actions.

## Validation

- UI inventory moved-bounds/drag/consumed-input/modal regression: 1 passed.
- Native input suite: 72 passed, including Walk outside an open bag and no
  Walk over it; existing movement ACK/HUD/death/skill guards pass.
- Native atlas loader resolves all 224 HookingCat frames: 1 passed.
- Monster frame closure: 8 libraries passed.
- Resource worker checked all 224 atlas crops against source RGBA, all eight
  page hashes/geometry, old starter entry unchanged, and append idempotence.
- `git diff --check` passed.
- Windows release build passed. Local test package:
  `C:/numeron-legend-of-rebirth-20260921-inventory-cat`, with the existing
  asset junction and Gateway 19910 config. The active old client was not
  terminated; normal logout is required before switching for acceptance.

Existing full-default atlas build still encounters missing equipment roots
and a single-page budget limit. The bounded append repair avoids replacing
the working multi-page starter atlas; it does not claim full asset closure.

No authenticated screenshot or actual user-potion reproduction has passed
yet. These results do not constitute visual or full-game acceptance.
