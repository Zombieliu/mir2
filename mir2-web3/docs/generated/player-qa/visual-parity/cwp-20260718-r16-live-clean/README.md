# Crystal/Web Same-Scene Evidence Pack

Generated: 2026-07-17T18:45:33.043Z

Target: map 0 @ 332,275

## Files

- Native Crystal screenshot: cwp-20260718-r16-live-clean-same-scene-original.png
- Native dynamic-frame selection: minimum-world-rgb-delta-across-native-effect-cycle (24 candidate(s))
- Native account state: not captured
- Web screenshot: cwp-20260718-r16-live-clean-same-scene-web.png
- Web state: cwp-20260718-r16-live-clean-same-scene-web-state.json
- Web account sync: not enabled
- Web QA state payload: not enabled
- Side-by-side: cwp-20260718-r16-live-clean-same-scene-side-by-side.png
- Region crops: native/web crop pairs generated in this folder
- Visual score: cwp-20260718-r16-live-clean-same-scene-visual-score.md
- Summary JSON: cwp-20260718-r16-live-clean-same-scene-summary.json

## Notes

The native Crystal window is captured from the currently running `Legend of Mir 2` client. When a TrapHexagon frame is fixed for Web, native capture spans at least one complete effect cycle and selects the lowest world-region RGB delta rather than assuming sample zero has the same animation phase. The Web scene is positioned through the token-gated QA control path when `MIR2_QA_CONTROL_TOKEN` is configured, then verified by waiting until the Web state reports the target map and coordinate.
