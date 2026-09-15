# Native Game Shop acceptance, 2026-09-15–16

This is bounded Candidate evidence. `accepted=false`, `visualAccepted=false`,
`globalParityPercent=null`. Original Crystal and Windows have not received a
complete paired visual pass. Web typecheck is not a Web visual pass.

## Implementation and verified behavior

Game Shop now renders the Crystal Title749 frame, source product cards,
insertion-order categories, class filters, All/Top/Deals/New sections, individual
quantities, Gold/Credit payment controls and source Yes/No confirmation.
The generated catalog contains 105 real server rows. Mount/body/weapon preview
uses source frame geometry, layer ordering and direction animation.

Normal entry requires an accepted StartGame ACK. A separately authenticated
password reconnect can replay its retained character's metadata without loading
an old save. Public repeated in-game StartGame remains rejected. The internal
replay command is rejected on production player command paths; Gateway custody
checks bind account and character. Current mailbox and shop stock are projected.

Successful purchases now publish the committed ReceiveMail mailbox. Native
snapshot adaptation recognizes id/from/subject/body and excludes deleted internal
ledger rows. Packet mail retains concrete attachment metadata.

| Real native check | Result / evidence |
| --- | --- |
| Catalog after normal password reconnect | R25 105 products; restored-catalog screenshot |
| Search replacement RedTiger → BlueTiger | R25 passed; search-replacement screenshot |
| Ctrl+A visible selection / selection replacement | R34 passed; blue highlight and normal RedTiger result after replacement |
| Home / End insertion | R34 passed; XRedTiger then XRedTigerY, screenshots retained |
| Left / Backspace / Delete | R34 passed; XRedTigerZY → XRedTigerY → XRedTiger |
| Mount preview / rotation | R34 RedTiger rider and mount visible in two directions; original paired pass remains open |
| Quantity two Gold confirmation | R25 330,000 Gold shown |
| Cancel confirmation | R25 wallet remained 500,000 Gold |
| Confirm quantity two Gold | R25 wallet became 170,000 Gold |
| Confirm quantity one Credit | R25 credit became 340; Gold unchanged |
| Mail after reconnect | R29 two visible parcels, deleted ledger hidden |
| Read first parcel | R30 unread 2 → 1 |
| Collect first parcel | R30 attachment marker and Claim button disappear |
| Backpacks after collecting | R30 two AccuracyPotion icons; committed unique IDs 63/64, quantity one each |
| Menu opening does not move player | R27 before/after coordinate 335,271 |
| New purchase immediately refreshes mailbox | R30 Gold 170,000 → 5,000; mailbox total 2 → 3 without reconnect |
| Collected status persists on next login | R30 normal relogin: first parcel remains claimed, no attachment; bag stays at two AccuracyPotions |
| Complete original Crystal paired acceptance | Open |

The shop uses an isolated fork of the already-completed Warrior character.
Only that fixture initially received 500,000 Gold/1,000 Credit for UI purchases.
The natural journey store did not receive those currencies or items. See the
limited `r30-claimed-store-summary.json`; private credentials and internal ledger
bodies are excluded from evidence.

## Stability regression found during acceptance

R29 remained near 324 image assets / 235 MB until Mail interactions. It then
grew to 14,985 assets / 15,601,560,716 bytes by launch +340.220 s; the process
later disappeared. The log does not establish an OOM exit code or a user-close
event. The 512×512 RGBA-sized growth is consistent with text atlases. Static
AssetServer loads are cached and are not proven direct allocators.

Mail now retains its child tree while its model/page/selection/compose inputs
remain equal. Model updates rebuild it, close clears it, and session transitions
invalidate the cache. R30 actual telemetry stayed near 326 assets / 226,481,396
bytes through launch +290.165 s with Mail visible and interactions in progress;
later checks through +3,001.854 s remained bounded at 325 assets / 233,317,524
bytes, including additional panels. This closes the observed unbounded Mail
growth in that bounded run, not every panel or indefinite stability.

## Verification

- Gateway final serial: **778 passed, 0 failed, 8 ignored**, 1,102.13 s.
- Windows R30 full with explicit complete C asset fixture: **655/655**, 15.81 s.
  Previous R27's five talisman failures were resolver/catalog shape mismatch;
  three attack/ImmortalSkin failures used the incomplete E resource fixture.
- Native UI R34 with `native-ui`: **852/852**, 1.47 s, including retained Mail/Shop children and raw navigation regressions.
- Talisman reproduction: 0/5 before → 5/5 after; ordinary SoulFireBall 1/1;
  complete C resource Warrior/ImmortalSkin tests 3/3.
- Game Shop focused simulation: **38/38**; retained bootstrap/mail **7/7**;
  security lifecycle + replay **23/23**.
- Web typecheck passes. Web/original paired screenshots remain open.
- Broader simulation serial initially reported **1,808 passed, 5 failed, 20
  ignored**. All five corrected fixture tests subsequently pass individually;
  a separate valid Hero transfer/take-back/save-load UID round-trip and malformed
  custody rejection pass. Hero take-back requires valid existing custody; it is
  excluded from the malformed-ingress normalization claim. Production reservation
  and custody guards are unchanged. The full simulation suite has not been rerun
  after these fixture changes; its integration suite was not executed by the
  failed lib-first run. Focused bootstrap/mail/security integrations pass above.

Exact binaries used: Gateway R56 SHA256
`FBBE20C5F03E585AFD27E75B0072628F8D0E7AC58A2332DC60DF1678862AD677`;
native R30 SHA256
`B21BF2702F03CC3B6C45355791B303A05BC3DEAAE173B0B6E75EABC6A1FDECF0`.
Native R30 is a development acceptance EXE, not a newly signed full Candidate
installer. Its runtime uses the complete C resource package.

R34 uses native SHA256
`1CFC5729C60F8BA36258518DCD352624B77C11E7B710F2F9A20A5D0A24CD99DF`.
The R33 real input trace identified Home/End/Left as physical Numpad7/Numpad1/
Numpad4 with correct logical navigation names. R34 resolves named logical keys
while preserving numeric character input and avoiding duplicate physical/raw
dispatch. Actual R34 screenshots verify the editing results above; advanced
modifier combinations and full original paired behavior remain open. R34 runtime
redirect files are empty, so no R34 runtime trace or soak duration is claimed.

## R35 original paired follow-up (visual repair not yet accepted)

The original client was changed to a bordered 1024×768 window and the user
manually logged in as `test`. Actual original default/search/RedTiger preview
screenshots are retained here. This character differs from the isolated Windows
Warrior, so matching rider equipment or purchase balances is not claimed.

The paired screenshots exposed a real R34 presentation failure: names, prices,
counts, quantities, wallets and page text appeared at the left despite declared
text alignment. R35 uses fixed source rectangles with flex-aligned natural-width
text children. Friendly names follow source trailing-number/bracket removal and
17-character display limits; tooltip grades follow the current Crystal source
color mapping, with missing metadata left white.

The preview close action existed, but Prguse frames 361–363 were absent from the
export and runtime package. All three are now exported from the matching original
Prguse.Lib, with source hashes and exact 16×15 geometry retained in
`preview-close-source-custody.json`. The main shop close button retains its
different Prguse2 24×21 geometry. Existing PNG bytes were not rewritten.

R35 optimized build completed in 1m49s; native SHA256 is
`4B1C97EF05417D6273ABA01B0A3A09D1801B116081A9678BC0E7221B025FE567`.
Final native-ui regression: **853 passed, 0 failed**, 1.51s.
This remains a development acceptance EXE. R35 paired visual alignment/close
checks remain pending because both Mir clients were closed before the repair
could be run visually.

## Separate natural journey checkpoint

Warrior Lv30: 55/55 mandatory + 4/4 milestones. Wizard and Taoist Lv25:
45/55 + 3/4 each. Completed units **155/177 (87.57%)**; partial kills do not
increase completed units. Wizard q89 has 2/3 CursedPriest, 3/3 ShiZombie,
3/3 CursedZombie; Taoist 0/3, 3/3, 1/3 at the recorded checkpoint.
R122 starts normal held-Medium recovery after a fresh direct Shaman hit at
≤85% HP; quest/monster values and safety escape thresholds remain unchanged.
Live R122 direct Shaman hit HP72 → normal Medium UseItem → HP100 is recorded;
six Mediums were subsequently exhausted and ordinary town escape triggered.
The remaining CursedPriest objective is not complete.
R123 full controller regression passes 643/643. Its live attempt preserved the
Shaman clearance but exhausted 391 bounded search waypoints without invoking
combat; it saved/logged out with the CursedPriest objective still 2/3.
R124 search-blocker follow-up passes 645/645 controller tests and corrected
independent review. The normal R54 run cleared two live Shaman blockers and
advanced its search position, but q89 remains incomplete at this observation.
Taoist R104 has stopped with its saved checkpoint retained. This is not route success.
R125 subsequently resumed Taoist with corrected authoritative death gating:
647/647 controller tests pass, and valid saved-death revival returns a fresh
HP180/dead=false town snapshot. No quest unit or visual pass is counted for it.
See [revival evidence](../protocol-journey-20260915/percent-zero-death-r125.md).
See [controller evidence](../protocol-journey-20260915/q89-range-and-blocker-r122-r124.md).
Full three-class Lv30 native/animation/paired acceptance is open.
