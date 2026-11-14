# Quest target guidance acceptance — R16

Date: 2026-09-15 (Asia/Shanghai)

## Accepted behavior

- An incomplete in-progress objective identifies matching monster names without substring false positives. Crystal-style plural, possessive, and camel-case labels are normalized.
- Every visible living quest monster keeps a fixed gold diamond above its name. Its name remains visible and gold when ordinary name display is disabled.
- The minimap renders visible quest monsters as 5 x 5 gold markers, including in a dark scene.
- The newcomer journey tracker shows the nearest visible target for both the next main quest and an optional quest as `target name · distance`.
- Clicking that tracker action arms the ordinary moving-target pursuit. The client follows the live server position until melee range and then requests the normal authoritative attack.

## Native acceptance evidence

- Build: `C:\mir2-natural-journey-20260911\hookingcat-resource-package-r2\mir2-platform-windows-quest-targets-r16-final.exe`
- SHA-256: `DF36BCEC4BEA677DC999E240312F203FD3DA01C0BCC63D550F8F3CA3A89EC149`
- Screenshot: `C:\mir2-natural-journey-20260911\captures-r16\quest-targets-r16-accepted-in-game-1789401584424-1.png`
- Scenario: `j1`, BichonProvince, dark scene, active optional `Hunt for the Butcher` objective.
- Visual result: the left tracker showed `Deer · 9 tiles`; the Deer had a gold world marker and a gold minimap marker.
- Interaction result: one tracker click moved the player from `(250,519)` to `(254,527)`, updated the live distance to `1 tiles`, and entered melee attack range.

## Automated verification

- `mir2-client-bevy`: quest UI tests — 66 passed.
- `mir2-client-bevy`: quest-target matching tests — 3 passed.
- `mir2-client-bevy`: minimap tests — 5 passed.
- `mir2-platform-windows`: stable quest marker test — passed.
- `mir2-platform-windows`: quest action arms pursuit — passed.
- `mir2-platform-windows`: moving-target chase/attack regression — passed.
- Windows release build — passed.

The complete Windows test binary ran 645 tests: 637 passed and 8 existing `effects::tests` failed in the concurrent working tree. Those failures cover ground talisman, immortal skin, and warrior effect frame assertions; none touches quest matching, overlays, minimap, tracker input, or pursuit state.

## Remaining boundary

The client can mark only monsters present in its authoritative visible-entity set. A cross-map spawn-area marker needs server-provided quest destination or spawn metadata and is tracked separately from this visible-target guidance.
