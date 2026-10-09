# Native Crystal NPC goods-cell checkpoint

Date: 2026-09-03

Status: bounded implementation and live Windows evidence. This is not final
Crystal 1:1 acceptance.

## Source contract

- `Crystal/Client/MirControls/MirGoodsCell.cs:20-140` defines the `205x32`
  cell, `40x32` icon centring, label positions, Lime border/divider,
  `Prguse/550` marker and `CreateItemLabel(... hideAdded: ...)` hover path.
- `Crystal/Client/MirScenes/Dialogs/NPCDialogs.cs:1071-1082,1348` defines the
  `Prguse/1000` dialog, `(10, 34 + row*33)` placement and
  `MultipleAvailable` rule.
- `Crystal/Shared/ServerPackets.cs:3082-3104` serializes
  `NPCGoods.HideAddedStats`; `Crystal/Client/MirScenes/GameScene.cs:4199`
  installs it for shop hints.

## Implemented slice

- NPC goods use one full-cell click/hover target instead of a combined label.
- Item pixels use their exported true size and are centred within the source
  icon area.
- Name, count and price are independent source-positioned labels.
- Selection uses a Lime outline and x=40 divider.
- The source New marker art/rule is retained.
- `HideAddedStats` crosses packet and snapshot adapters, the shared shop
  model, serde and runtime ingestion. It suppresses mutable added combat/
  defence stats and `Cursed` only for NPC-shop hints; base stats and other
  bind text remain.

## Live evidence

Real Windows pointer input followed login -> Scott -> View in the ordinary
native client. Both captures share run `npc-shop-20260903-r2`, world
`BichonProvince (288,616)`, `panel=NpcShop`, logical 1024x768 and DPI 1.0.

- Baseline: `mir2-in-game-1788375662467-1.png`, SHA-256
  `B62E187B4042434875FACF9130F20A0D443CF72C4D63462A1E8C084FBDC1FF6C`.
- Selected/hover: `mir2-in-game-1788375687703-2.png`, SHA-256
  `DCECFE0FC836F6CBA4961D40E6F259622DAAA075059CFF13817F8C9B304349DA`.
- The adjacent JSON sidecars freeze the renderer state. The second image
  visibly records the Lime-selected Candle cell and the shared item hint.
- Client EXE: 87,704,576 bytes, modified `2026-09-02T18:58:10Z`, SHA-256
  `159B13E722451C6F44B036C6B3ABD141E19362EDB28ED29180F34C6849A7DD8A`.
- `process-provenance.json` records the exact client/Gateway processes and the
  intentionally untrusted working-tree provenance boundary.

## Automated gates

- `mir2-client-bevy --features native-ui`: 514/514 passed.
- `mir2-client-runtime`: 212/212 passed.
- Focused Windows Gateway NPC-goods recovery test: passed.
- Full Windows suite: 519/520 passed. The only failure is the pre-existing,
  unrelated Archer atlas fixture expectation for `/ARArmour/00/24.png`.
- Ordinary Windows Debug executable build: passed.
- `git diff --check`: passed before documentation updates.

## Remaining gates

The generated sidecars correctly remain `eligible=false` because authoritative
world state and trusted build provenance are incomplete. This run was built
from a dirty working tree and is not a signed/package-qualified Candidate.
There is no same-state Crystal capture pair. Duplicate/sub-goods topology,
other specialized item-surface layouts and populated captures, trusted light
and package provenance, 100/125/150% DPI, soak and human visual/feel comparison
remain open. Therefore `visualAccepted=false`, `accepted=false`, and
`globalParityPercent=null`.
