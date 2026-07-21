# Crystal/Web Visual Parity Report

Generated: 2026-07-17T18:45:33.034Z
Input: `E:\mir2\mir2-web3\docs\generated\player-qa\visual-parity\cwp-20260718-r16-live-clean`
Samples: 1

## Summary

- Latest sample: `cwp-20260718-r16-live-clean-same-scene`
- Latest weighted score: 100%
- Estimated human visual/feel parity band: **93-100%**
- Runtime health average: 100%
- Layout average: 100%
- Entity/nameplate average: 100%
- Pixel trend average: 100%

> Pixel trend is not a final acceptance score: Crystal/Web captures can be animation-frame and coordinate-offset mismatched. Use the score for trend/regression detection, then resolve the listed gaps with human visual review.

## Top Gaps

- `P1` HUD score is polluted by dynamic character state (1/1 samples): name=VIS0716Hero; level=1; health=HP 18/18; mp=14/14; gold=0; weight=0/50; hudWeightSpace=50/46; beltItems=0; inventoryItems=0; equipment=WoodenSword, BaseDress(M); signals=level=1, hp=HP 18/18, gold=0, belt=empty, inventory=empty
- `P2` Chat panel content/state differs (1/1 samples): chat similarity=82%; changedPixels>=12=7%; meanAbsDelta=10.087; meanLumDelta=9.817

## State Diagnostics

> These checks do not replace native-state extraction. They mark Web captures that look like a fresh/starter character, so HUD/chat pixel deltas are not mistaken for pure asset/layout defects.

- `cwp-20260718-r16-live-clean-same-scene`: name=VIS0716Hero; level=1; health=HP 18/18; mp=14/14; gold=0; weight=0/50; hudWeightSpace=50/46; beltItems=0; inventoryItems=0; equipment=WoodenSword, BaseDress(M); signals=level=1, hp=HP 18/18, gold=0, belt=empty, inventory=empty

## Samples

- Changed-pixel threshold: RGB mean absolute delta >= 12/255

| Sample | Overall | Runtime | Layout | Entities | Pixels | World | Full changed | World changed | HUD Full | HUD UI | Chat | MiniMap |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `cwp-20260718-r16-live-clean-same-scene` | 100% | 100% | 100% | 100% | 100% | 91% | 7% | 6% | 86% | 88% | 82% | 87% |

## Next Pass

- Capture a fresh Crystal/Web pair at the same map/coordinate after each fix.
- Treat P0/P1 gaps as implementation candidates before expanding feature coverage.
- Once scores stabilize, run a movement recording pass and attach it to this report family.
