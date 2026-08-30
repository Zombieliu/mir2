# WN-VIS-UI-01 — Native HUD and core panel calibration

Status: **code gates and live Windows evidence pass for this slice; overall visual acceptance remains open**

Date: `2026-08-22`
Stage: `1024x768`
Scene: `BichonProvince`, authoritative player position `288,616`

## Outcome

This slice replaces the generic Menu, Options and Big Map compositions with
Crystal-source frames and coordinates, stabilizes authoritative HUD data across
partial snapshots, and removes the non-Crystal oversized combat-target panel.
The final client was built from the current source and exercised against the
already-running local gateway without restarting or modifying that gateway.

This is not a human `Accepted` result. Geometry was anchored to Crystal source
assets/specifications and checked in a live Windows run. The original client was
not captured in the same run, and unsupported controls remain visibly static
instead of pretending to work.

## Surface decisions

| Surface | Delivered | Remaining gap | Result |
|---|---|---|---|
| HUD | Authoritative HP/MP, level, wallet, map and identity survive partial snapshots; labels use compact Crystal-style white text; oversized target overlay removed | Exact GDI font metrics, belt/minimap detail and status/effect density still need comparison | pass for this slice |
| Menu | `Title/567` vertical frame; Exit, Logout, Group and Guild keep distinct real actions | Later-system icons with no shared-domain implementation are intentionally static | visual pass; functional gap remains |
| Options | `Title/411` frame, exact close art and functional music/sound volume bars | Unsupported source settings rows are static pending shared settings state | visual pass; partial functional pass |
| Big Map | `Title/820` frame, real `MMap/101`, player marker, coordinates and close action | NPC list, search, world/location navigation and teleport require authoritative data/actions | visual pass; partial functional pass |
| Combat target | Server-selected live target priority and stale-target fallback retained; unknown HP is not fabricated | Crystal-native target presentation can be added only after a source reference proves one is required | pass |

## Build identity

- Executable: `target-vis-ui01/release/mir2-platform-windows.exe`
- SHA-256: `D93AF3F94352AAD31419B33F690C7D84DAD5D4853ADE159DB761311108AC8F06`
- Size: `65,101,312` bytes
- Build time: `2026-08-22 17:14:33 +08:00`

The alternate target directory was used only because the previous release file
remained locked by Windows after the first capture. It is not part of the
candidate package and is removed after evidence collection.

## Final evidence

- `hud-final.png` — live HUD with no oversized target overlay
- `menu-final.png` — Crystal vertical menu frame
- `options-final.png` — Crystal options frame and audio controls
- `bigmap-final.png` — Crystal big-map frame with real map image and player marker
- `native-final-stderr.log` — asset completeness, gateway connection and real
  `LoginSuccess` / `StartGame` / `MapInformation` / `UserInformation` trace

## Automated gates

| Gate | Result |
|---|---|
| `cargo +1.95.0 test --manifest-path apps/game-client/client-bevy/Cargo.toml --features native-ui` | `260 passed; 0 failed` |
| `cargo +1.95.0 test --manifest-path apps/game-client/platform-windows/Cargo.toml` | `235 passed; 0 failed` |
| `npm --prefix apps/web run typecheck` | pass |
| package script self-test | pass |
| candidate verification script self-test | pass |
| release build | pass |

Google Gemini 3.7 Flash was not used for this closure. A vision model may be
added later as a repeatable screenshot classifier, but its score is advisory and
does not replace deterministic source geometry, functional tests or human
acceptance.
