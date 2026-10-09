# Wizard q89 departure MP R138 live proof

Bounded read-only sample of `Wizard.2026-09-15T23-36-13-656Z.trace.jsonl` through sequence 3405 (`2026-09-15T23:42:41.705Z`). The active trace cutoff is 2,578,996 bytes, SHA-256 `580FCA3825D77E6D2CAE0761C823C6A4FA23A5AD25B5C21F2155A1DED6BFE865`.

The first post-StartGame owner snapshot is seq570 at `23:36:17.096Z`, map `0`, with gold `12`, player MP `152/398`, and 12 `(MP)DrugSmall` in `bag1` (unique id 22). The same public state remains at seq692 (`23:36:29.032Z`) and seq963 (`23:37:04.485Z`): gold 12, MP 152, MP-drug quantity 12. q89 is still Priest `2/3`, ShiZombie `3/3`, CursedZombie `3/3`.

This trace contains no `SellItem`, `BuyItem`, shop-buy acknowledgement, or `UseItem` event before or around these snapshots. Therefore it proves the live persisted/bootstrap inventory state of 12 MP drugs, but does not prove an ordinary sale/funding/shop purchase or causally attribute the increase to the R138 departure floor. No route or visual completion is claimed; denominator remains `155/177`.
