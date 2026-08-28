# Agent Task Queue

> VIS-01/VIS-02/VIS-04 bounded visual checkpoints (2026-08-28): revisions
> `2a83c0062dd60916730c46c752e044f668b243db`,
> `473a56137c7af458d5c982c90f3d4a658a9243fd` and
> `fd3b5d552bbb9292ce49d95709477da3f6966d38` close three independent
> automated leaves. Player/self-player now render Crystal's name-above-guild
> two-line label with shared colour/outline and corpse delta; FrostCrunch now
> owns exact `Magic2/400..413,570..577`, `M41-1/2`, delayed completion, terminal
> `Dead` suppression and Candidate asset identity; Scarecrow Right Walking is
> transcript-locked as `44..49 -> Standing/8` at 100ms without invented audio.
> Focused gates, both Candidate self-tests and the combined Windows 416/416
> suite pass; independent reviews have P0=0 and no semantic P1. Player,
> monster, spell and UI denominators plus same-EXE/live-WSS, DPI, soak, human
> and signing gates remain open, so `globalParityPercent` stays null. Reports:
> `VIS-01-PLAYER-GUILD-NAME-REPORT.md`, `VIS-02-FROST-CRUNCH-REPORT.md` and
> `VIS-04-SCARECROW-WALKING-REPORT.md` under
> `docs/generated/player-qa/windows-visual-parity/`.

> VIS-02 FireBall-family Dead-target bounded checkpoint (2026-08-28): revision
> `8d8c5f12f6faa4617ce87017f82738458f164bd9` binds Windows native
> FireBall, GreatFireBall and SoulFireBall completion to Crystal's exact
> `Target.CurrentAction == Dead` condition. The shared presentation clock,
> not the early `dead` boolean, gates both impact bitmap and impact sound;
> `Die` and Revive-before-arrival are retained. Full Windows 410/410 and
> independent P0=0/P1=0 review pass. Other missile spells, the Web equivalent,
> the skill/effect denominator and all exact-head device/human/signing gates
> remain open; `globalParityPercent` stays null. Report:
> `docs/generated/player-qa/windows-visual-parity/VIS-02-FIREBALL-DEAD-TARGET-REPORT.md`.

> VIS-04 Scarecrow Revive bounded checkpoint (2026-08-28): revision
> `04121747c70d1c5487947f027d07b5209ca84f6c` closes the exact
> `Monster/005` remote Revive packet/render leaf. It fixes a native stale-0%
> merge that could turn `ObjectRevived` dead again without fabricating HP,
> then locks Crystal's signed Right sequence `164..155 -> Standing`, 100ms
> cadence, no lingering Die additive layer, Web action token and frame
> denominator. Windows passes 407/407; Web gates and staged-diff review pass.
> Zone respawn policy, eight-direction captures, other monsters and final
> device/human/signing gates remain open; no global percentage is claimed.
> Report:
> `docs/generated/player-qa/windows-visual-parity/VIS-04-SCARECROW-REVIVE-REPORT.md`.

> VIS-01 corpse/body-name bounded checkpoint (2026-08-28): revision
> `cda55ef5a` aligns the Windows native overlay with Crystal's
> `Dead ? 35 : 8` name placement. Dead player/base-monster names remain
> visible without a synthetic `Dead` line, move exactly 27px below the living
> position, and retain the existing NameView/hover gates; the independent dead
> self-health path remains suppressed. Focused regression and the full Windows
> suite pass (406/406), and exact-worktree review found no P0/P1 issue. This is
> one player-character presentation leaf only: guild labels, complete player
> libraries/actions and all exact-head device/human/signing gates remain open;
> `globalParityPercent` stays null. Report:
> `docs/generated/player-qa/windows-visual-parity/VIS-01-CORPSE-NAMEPLATE-REPORT.md`.

> Shared-Zone wall-clock monster respawn checkpoint (2026-08-28): revision
> `7f991ec34fbde6ac07a5799b35d352f2785c1aa9` moves ordinary and
> harvest-gated monster resurrection out of each personal
> `SimulationSession` and into the single-writer `ZoneRuntime`. Crystal's
> `D10/R30` delay distribution, Deer harvest gate, checkpoint/recovery due
> time, late-join behavior and two-observer one-incarnation rule are covered.
> A trusted NPC/dialog boundary now cancels queued movement before
> `CallNpc`/quest adjacency checks, closing the Q4 return-to-Merchant race.
> `shared_zone` passes 203/203; the ordinary client-packet Q1-to-Q4 Gateway
> chain passes in 748.77 s including five real DeerMeat, turn-in, logout and
> reload. This is a bounded shared-world authority checkpoint, not full Zone,
> quest, economy, AI or whole-game parity; no global percentage is claimed.

> VIS-03 main-HUD seven-button matrix checkpoint (2026-08-28): revision
> `4f7efffca093cb59d0e4f468dbd08ea2c61d314f` binds Character,
> Inventory, Skill, Quest and Option to exact `Prguse/1900..1914` geometry
> and ButtonA (`103.wav`), plus Menu `Prguse/1960..1962` and GameShop
> `Prguse/826..828` to ButtonC (`105.wav`). Real press edges, panel toggles,
> exact asset paths and Candidate allowlists/identity are automated. This
> commit deliberately excludes the still-uncommitted inventory expansion
> draft. No exact-head EXE, screenshot, physical audio or human acceptance was
> produced; the wider 410-leaf UI denominator and final device gates remain
> open, so `globalParityPercent` stays null.

> Visual/interation execution goal sync (2026-08-28): the Windows native
> client is playable enough to expose the real gap, but it is not visually
> complete. A live native Bichon window already shows terrain, actors, labels,
> minimap, orb HUD, chat strip, quick bar and the right-side HUD cluster.
> The current source-audited denominator is still open: player libraries
> `7/477`, monster libraries `8/546`, non-None spells `first bounded leaves
> only`, and fixed/template UI leaves `partial out of 410`. The active visual
> execution goal is now explicitly split into four waves: `HUD/button UI`,
> `player-character`, `skill/effect`, and `monster-family` expansion. The
> first bounded write target after this sync is the `VIS-03` main-HUD button
> matrix expansion so the next user-visible work lands on exact button/panel
> fidelity rather than another hidden backend-only slice. This queue entry does
> not authorize whole-game percentages or close same-EXE/live-WSS, real-DPI,
> soak, human, legal-asset or signing gates. Detailed note:
> `docs/generated/player-qa/windows-visual-parity/VISUAL-GOAL-20260828.md`.

> VIS-04 Scarecrow Struck-audio bounded automated checkpoint (2026-08-28):
> revision `354bb9f9648758c9f38d5ce149a273ae07cd2a7e` binds Crystal's
> `MonsterObject.Struck` order to exact `005-2.wav` flinch followed by the
> optional attacker-weapon clang `60..65.wav`. Native and Web cover the full
> audited weapon-image grouping, Assassin override, unknown-attacker
> fail-closed, lethal flinch/clang/death order, feed dedupe and actor/scene
> stale-event gates. Focused 3/3, Windows 406/406, Bevy 419/419, runtime
> 191/191, Web 49 groups, audio/export/typecheck and both Candidate self-tests
> pass; final review is P0=0/P1=0/P2=0. Scarecrow Attack1/Struck/Death now
> have bounded automated source/script closure, but movement/swing/revive,
> other monsters and the complete denominator remain open. No EXE, package,
> live WSS, screenshot, physical-audio or human evidence was produced; no
> global percentage is claimed.

> VIS-04 Scarecrow Attack1-audio bounded automated checkpoint (2026-08-28):
> revision `e1dd6d6379d23efeafe57aa01c170452f1261b83` binds Crystal
> `Scarecrow=5`, `BaseSound=50`, immediate `Attack1 PlayAttackSound=51` and
> unlisted-ID filename synthesis to exact `005-1.wav`, never `51.wav`.
> Native actor-context routing, exact kind/body checks, per-action replay and
> Remove/Hide/map/logout cancellation, Web ID 51 resolution, direct export and
> Candidate exact identity are automated. Focused 2/2 plus bridge 1/1,
> Windows 403/403, Bevy native-ui 419/419, runtime 191/191, Web 47 groups,
> audio/export/typecheck and both Candidate self-tests pass; final review is
> P0=0/P1=0. Flinch `005-2`, weapon struck clang/order, other monsters and the
> complete monster-audio denominator remain open. No EXE, package, live WSS,
> screenshot, physical-audio or human evidence was produced; no global
> percentage is claimed.

> VIS-04 Scarecrow death-audio bounded automated checkpoint (2026-08-28):
> revision `cf4f5b5197c492324be23beb73611c0e0162c403` binds Crystal
> `Scarecrow=5`, `BaseSound=50`, `PlayDieSound=53` and the unlisted-ID
> filename synthesis to exact `005-3.wav`, never the unrelated `53.wav`.
> Native exact-body routing, one-shot identity, same-batch Remove/Hide and
> map/logout reset cancellation, Web immediate Monster versus 100 ms Player
> timing, direct sound export and Candidate exact identity are automated.
> Focused 2/2, Windows 401/401, Bevy native-ui 419/419, runtime 191/191, Web
> 46 groups, audio/export/typecheck and both Candidate script self-tests pass;
> final review is P0=0/P1=0. That revision left Attack `005-1` open; the
> later checkpoint above closes it. Flinch `005-2`, struck/walk/swing/Dead/
> Revive, other monsters and the complete monster-audio denominator remain
> open. No EXE, package, live WSS, screenshot, physical-audio or human
> evidence was produced; no global percentage is claimed.

> VIS-03 Inventory locked-second-tab bounded automated checkpoint
> (2026-08-28): revision `83f081149375fb402b9c7e6711fdb4e6bed68a0e`
> binds Crystal's unexpanded array length 46 to the exact `Title/169` locked
> tab, one local ButtonA click cue and no page/Gateway transition. Only the
> real Crystal capacity domain `46,54,58,...,86` is accepted; missing or
> illegal values fail closed, and an authoritative downgrade returns to page
> one while clearing pending local item UI state. `Title/168,169,196,197,198,
> 737,738,739` are required by Candidate packaging/verification. Focused 5/5,
> Bevy native-ui 419/419, Windows 399/399, runtime 191/191 and both Candidate
> script self-tests pass; final review is P0=0/P1=0. Production capacity
> emission, `ExtraSlots8`/`@ADDINVENTORY`, `Prguse2/307` lock bars and
> `Title/483..485` AddButton remain P2/follow-on work. No EXE, package, live
> WSS, screenshot or device evidence was produced; same-EXE/DPI/soak/human/
> signing and denominator gates remain open, so no global percentage is
> claimed.

> VIS-02 Healing additional bounded automated checkpoint (2026-08-28):
> revision `24d9b73a30fc18edf0649283d14495c6f4900aff` reproduces the
> Crystal caster `Magic/200..209` ten-frame 60 ms sequence plus exact
> `M61-0.wav`, and the immediate target-owned `Magic/370..379` ten-frame
> 80 ms sequence plus exact `M61-1.wav`. Native replay, moving/removing target,
> reset and missing-asset boundaries are covered; Web typed projection, string
> and numeric spell audio, source export and Candidate fail-closed rules are
> also automated. Focused Healing 4/4, Windows 398/398, Bevy 416/416, Web
> event/export/runtime/type checks and both Candidate script self-tests pass.
> Final review is P0=0/P1=0; Web retransmit deduplication remains a non-blocking
> P2. This changes no Healing gameplay authority and produced no exact-head
> package, EXE, live WSS or device capture. Same-EXE/DPI/soak/human/signing and
> denominator gates remain open; no global percentage is claimed.

> VIS-03 HelpDialog movable/Sort follow-on checkpoint (2026-08-28): revision
> `4545465a2e31a6646f247c55906764952d44cd58` implements Crystal's grab-offset
> movement through the shared logical-stage transform, four-boundary clamp
> including the source right/bottom `-1`, release/focus/Hide/reset cleanup and
> fail-closed headless handling. Show and valid header drag raise Help above
> peer dialogs while preserving Death/Menu modal layers. Focused Help 14/14,
> Bevy native-ui 416/416 and Windows 394/394 pass; independent review reports
> P0=0/P1=0 for this leaf. Dynamic keybinding/localization, exact typography,
> same-EXE/real-DPI/soak/human/signing and denominator gates remain open. No
> EXE, package, screenshot or global percentage is claimed.

> VIS-03 HelpDialog bounded default-English automated checkpoint
> (2026-08-28): revision `e22f2aa4c683447b0e57805a580fd29e0a84c37c`
> adds Crystal's independent 536x509 Help window, exact frame/title and
> previous/next/close triples, three shortcut pages plus `Help/0..41`, all
> 45 titles and wraparound navigation. Menu Help and default H are local and
> silent; the internal controls emit one ButtonA; Escape/session reset and
> core-panel coexistence are covered. P now follows the displayed Crystal
> default and opens Group rather than Storage. The typed control inventory is
> 174 entries and Candidate scripts fail closed for missing Help assets.
> Focused Help 9/9, Bevy native-ui 411/411, Windows 394/394, ui-core registry
> 13/13 and both script self-tests pass. Review has P0=0; one retained P1 is
> explicit: native Help uses default English/default bindings instead of
> Crystal's live rebind/localization model. At that revision movable dragging,
> exact font/bold
> raster, same-EXE/DPI/soak/human/signing and the incomplete denominator also
> remain open. No EXE, package or screenshot was produced; do not emit a
> global percentage.

> VIS-01 living hover-name additional bounded automated checkpoint
> (2026-08-28): revision `066f6f3b576cbdc03106c8a221ccdaf13f7dfa83`
> decouples Crystal living `NameView`, non-self MouseObject/self MouseOver
> names, `HighlightTarget` redraws and the living self health bar. Self,
> remote player, NPC and monster matrices; simultaneous self/non-self hover;
> selected-only suppression; alpha/same-tile/reverse hit testing; dead/empty
> fail-closed behavior; health stability and local reset are automated.
> Windows 394/394 and final independent P0=0/P1=0 review pass. No server,
> Gateway, asset, EXE/package, live WSS or device capture changed. Corpse/
> DisplayBodyName, guild/line-color/special-offset formatting, same-EXE/DPI/
> soak/human/signing and the incomplete denominator remain open. VIS-01 stays
> in progress; do not emit a global percentage.

> VIS-02 LeftGuard range-projectile additional bounded automated checkpoint
> (2026-08-28): revision `d2dfff14308256c07c3b3169798afee0a051b97b`
> routes typed `ObjectRangeAttack` through exact `Monster/100` client logic.
> After the frame-4 400 ms delay it launches the source-owned, target-tracking
> `Magic/10 + Direction16*10 .. +5` missile: six 30 ms additive frames,
> opacity 1, light 6 and a Crystal 50 ms/tile flight clock. Packet location,
> locked launch direction, moving-target retiming, Hide/Remove ordering,
> adapter tombstones, replay, map/generation/session boundaries and fail-closed
> assets are automated. LeftGuard 5/5, guard-range 10/10, Windows 392/392,
> the 74-spell exporter/validator, diff checks and final P0=0/P1=0 review pass.
> This adds no server/Gateway authority, new asset, audio, EXE/package, live
> WSS or device capture. Monster ActionFeed, same-EXE/DPI/soak/human/signing
> gates and the incomplete semantic denominator remain open. VIS-02 stays in
> progress; do not emit a global percentage.

> VIS-02 RightGuard range-hit additional bounded automated checkpoint
> (2026-08-28): revision `7d08b53f8d78161655254bb83ebd519ecbd62fed`
> routes typed `ObjectRangeAttack` through the exact `Monster/099`
> frame-4 client branch. At 400 ms it starts target-bound `Magic2/10..14`,
> five 60 ms additive frames with opacity 1 and light 6. Source and target
> presence are required through the exact 400 ms boundary; from 401 ms the
> effect is target-owned. Replay, movement, remove/hide, expiry and
> generation/session reset behavior are automated. Focused 6/6, Windows
> 387/387, the 74-spell exporter/validator, Rust 1.95 fmt/diff and final
> P0=0/P1=0 review pass. This adds no server/Gateway authority, new asset,
> EXE/package, live WSS or device capture. Missing `995.wav`, the complete
> monster ActionFeed, same-EXE/DPI/soak/human/signing gates and the incomplete
> semantic denominator remain open. VIS-02 stays in progress; do not emit a
> global percentage.

> VIS-03 CharacterDialog close additional bounded automated checkpoint
> (2026-08-28): revision `225ae951d95894458b7f1cbd30d78ee100fe4362`
> source-binds the common Character close control to exact
> `Prguse2/360/361/362`, `(241,3,24,21)`, local Hide semantics and
> `ButtonA=10103 -> 103.wav`. A dedicated CloseCharacter action prevents
> unaudited generic close controls from inheriting the cue. All four pages,
> held/re-press, non-InGame blocking, page/panel reset and both empty intent
> queues are covered. Focused 1/1, Bevy 402/402, Windows 381/381, Rust 1.95
> fmt/diff and independent P0=0/P1=0 review pass. No asset, EXE, package, live
> audio/WSS or screenshot was produced. Page contents, other controls,
> same-EXE/DPI/soak/human/signing and the incomplete denominator remain open.
> Do not emit a global percentage.

> VIS-03 CharacterDialog tabs additional bounded automated checkpoint
> (2026-08-28): revision `ac4ae1686ff60c01437100554c7a5d4cd6c78a65`
> source-binds the four 64x20 Character/Status/State/Skill controls at
> `(8,70)/(70,70)/(132,70)/(194,70)` to active `Title/500..503` frames and
> exact local `ButtonA=10103 -> 103.wav`. Every Changed-to-Pressed pointer edge
> queues one cue before its local page transition; held state does not repeat,
> release/re-press emits once, all four pages are covered and neither UI intent
> queue receives Gateway work. UI audio synchronization now follows all local
> input producers in the same update. Focused 1/1, Bevy native-ui 402/402,
> Windows 381/381, Rust 1.95 fmt and diff checks pass. Independent review's
> same-update audio P1 was remediated; final P0=0/P1=0. No asset, EXE, package,
> live audio/WSS or screenshot was produced. Page contents, remaining panel/UI,
> same-EXE/DPI/soak/human/signing and incomplete semantic-denominator gates
> remain open. Do not emit a global percentage.

> VIS-01 hover-target additional bounded automated checkpoint (2026-08-28):
> revision `1deb930483f3eca5f26f11020f091454fc96b183` retains the verified
> entity-atlas RGBA pages for Crystal body-only transparent-pixel MouseOver,
> including the same-tile shortcut and Y/X descending five-by-five/reverse-
> object scan. Self/dead are excluded, NPC/player/monster are eligible, and
> the exact-atlas full composite redraw is atomic. Separate world, hover,
> selected and foreground-effect depth bands preserve source order; identical
> hover/selection emits only selected. Persisted `HighlightTarget` schema v3
> defaults true for old v1/v2 configs and gates both redraws without adding an
> OptionDialog row, Gateway command or combat-target mutation. Windows
> 381/381, Bevy native-ui 402/402, ui-core 42/42, runtime 191/191 and focused
> 5/5 pass; two final reviews report P0=0/P1=0. No EXE, package, live WSS,
> GPU screenshot or DPI/human-feel evidence was produced. General
> DrawBehind/special composites, Web symmetry and every same-EXE/DPI/soak/
> human/signing/denominator gate remain open. Do not emit a global percentage.

> VIS-03 Character HUD additional bounded automated checkpoint (2026-08-28):
> revision `849f1f0b5120867d1358e0e7db9ba675e9866f9c` source-binds the
> 20x20 `(905,692)` main Character control to exact `Prguse/1900/1901/1902`
> normal/hover/pressed assets. Enabled pointer edges queue exact ButtonA once
> before the callback; held or disabled states do not repeat/act. The callback
> now follows Crystal: closed opens CharacterPage, Stats1/Stats2/Spells stay
> open and return to CharacterPage, and an already visible CharacterPage
> closes. Default C and F10 share that state machine without click audio or
> network intent. Bevy native-ui 401/401, Windows 376/376, focused Character
> 4/4, package/verifier self-tests, rustfmt and diff checks pass. Independent
> review's one keyboard P1 was remediated; final P0=0/P1=0. No new asset, EXE,
> package, live audio or screenshot was produced. Character panel content,
> remaining controls, real DPI/hit feel and every same-EXE/live-WSS/soak/
> human/signing/denominator gate remain open. Do not emit a global percentage.

> VIS-03 Inventory ButtonA additional bounded automated checkpoint
> (2026-08-28): revision `5b70511316b084ac677b5978f7f03e440241ca4c`
> binds the enabled InGame Inventory HUD pointer-press edge to Crystal
> `SoundList.ButtonA = 10103`, whose exact source mapping is `103.wav`.
> The sound is queued once immediately before the inventory click callback;
> a held press cannot repeat it, a later press can, and the direct F9/I
> keyboard toggle remains silent. Typed UI and packet-authoritative gameplay
> queues plus their spawned-player lifecycles are separate, including a
> same-frame regression. Missing source, disabled sound and zero volume fail
> closed without fallback. Candidate package/verify now allowlist, require,
> copy and identity-bind the existing 26,546-byte WAV at SHA-256
> `7A55D27DEA18F70EB4FF4F324B682EFAB4996406EFAE3E94467D3C39CCCC674A`.
> Windows 376/376, Bevy native-ui 397/397, focused ButtonA/audio 4/4,
> package/verifier self-tests, rustfmt and diff checks pass; independent final
> review found no P0/P1 after lifecycle remediation. No EXE, package, live
> audio or screenshot was produced. Other controls, hover/pressed feel,
> same-EXE/live-WSS/audio-device/DPI/soak/human/signing and the incomplete
> denominator remain open. Do not emit a global percentage.

> VIS-01 selected-target additional bounded automated checkpoint (2026-08-28):
> revision `a58ab0aaa2202731a5c55e7a684261d6c15c2f8d` implements
> Crystal's post-world `DrawBlend` redraw for explicitly selected remote
> players and monsters. The complete resolved mount/weapon/body/hair or
> monster body composite is cloned from exact atlas geometry at opacity 0.3;
> any missing rendered-layer atlas identity suppresses the whole highlight.
> Numeric/string object IDs, live selection replacement/removal, dead monsters,
> Hidden opacity independence, real front-map occlusion and Scarecrow's
> non-duplicated `DrawEffects` layer are regression-covered. Separate world,
> target and post-world effect bands preserve Crystal ordering for
> ObjectEffect/MapEffect and actor effects while Persistent ObjectSpell remains
> in-world. Windows 376/376, Bevy native-ui 393/393, shared runtime 191/191,
> focused selected 3/3 and foreground-depth 1/1 pass; independent review found
> no P0/P1. No EXE, package or screenshot was produced. Hover `MouseObject`,
> HighlightTarget option wiring, complete `Effect.DrawBehind` extraction,
> special actor/effect composites, Web symmetry and every same-EXE/live-WSS/
> GPU/DPI/soak/human/signing gate remain open. Do not emit a global percentage.

> VIS-02 GreatFireBall additional bounded automated checkpoint (2026-08-28):
> revision `9457e5618449d22350baedd01e3775f5b1fe59c6` binds typed
> `ObjectMagic(spell=34)` to Crystal's immediate `Magic/400..409` cast,
> 600 ms action-completion launch, all sixteen six-frame projectile ranges
> `410 + direction*10 .. +5`, target-bound `Magic/570..579` impact and exact
> M34-0/M34-1/M34-2 phase audio. The Rust compatibility
> `ObjectProjectile` is ignored so the client-owned missile cannot double
> draw. Target removal and map/session lifecycle fail closed. Ninety formerly
> missing direction PNGs, their source metadata and the three exact WAVs are
> now tracked; package/verifier require all 116 frames and verify sound
> byte/hash identities. Windows 372/372, Bevy native-ui 393/393, focused
> GreatFireBall 5/5, Gateway projection 1/1, Web type/full logic, 74-spell
> exporter, offline assets and both script self-tests pass; independent review
> found no remaining P0/P1 after the clean-checkout asset closure. This is an
> additional checkpoint beyond the original first-five list, not VIS-02 or
> visual acceptance. A retained-but-dead target, authenticated live WSS,
> same-EXE/GPU/DPI/soak/human/signing and the incomplete denominator remain
> open. Do not emit a global percentage.

> VIS-02 FlamingSword bounded automated checkpoint (2026-08-28): revision
> `160e8d3ccc0eb17f8e49b6505c5a58666a35029f` preserves Crystal's silent
> toggle and starts presentation only from `ObjectAttack(spell=8)` Attack1.
> Native and Web use the attacker-bound eight-direction, six-frame
> `Magic/3480 + direction*10` overlay at 100 ms/frame, additive opacity 0.7,
> no light/shadow, exact M8-1 at time zero and the ordinary swing at 100 ms.
> Lifecycle cleanup, 48-frame/audio package closure and ordinary-attack
> isolation fail closed. Windows 357/357, runtime 191/191, Bevy native-ui
> 393/393, focused effects 5/5, Gateway projection 1/1, Web type/full logic,
> 74-spell exporter, offline assets and package/verifier self-tests pass;
> independent final review found no P0/P1. The first five automated
> presentation checkpoints are now bounded, but VIS-02, the combat-state
> chain, backend live semantics, same-EXE/live-WSS/GPU/DPI/soak/human/signing
> and the incomplete denominator remain open. Do not emit a global percentage.

> VIS-02 FireWall bounded automated checkpoint (2026-08-28): revision
> `f6f78f3eddb813897cf4ce4c6056183130ab7f35` binds typed
> `ObjectMagic(FireWall)` to the 600 ms `Magic/1620..1629` caster action and
> exact M39-0/M39-1 phase audio, while five independent `ObjectSpell` objects
> use repeating `Magic/1630..1635`, light 3 and authoritative `ObjectRemove`
> lifecycle. Windows 351/351, Bevy native-ui 393/393, focused effects 5/5,
> Gateway projection 1/1, Web typecheck/export/offline assets and package/
> verifier self-tests pass. Final review found no P0 and its two P1 evidence-
> boundary findings were corrected: `cast=false` is a labeled synthetic
> compatibility case outside the canonical timeline, and source-asset tests
> do not claim a packaged Candidate. This is the fourth bounded first-five
> spell checkpoint; FlamingSword remains open, as do backend negative/
> lifecycle coverage, same-EXE/live-WSS/GPU/DPI/soak/human/signing and the
> incomplete denominator. Do not emit a global percentage.

> Active Windows visual-parity goal (2026-08-27): `VIS-00` is the first
> bounded implementation checkpoint on `codex/windows-visual-parity`. It adds
> the Phase-A denominator ledger and repairs Arial routing plus 8pt
> chat/nameplates, remote normal/Transform body routing, Harvest/CWeapon-01/
> Skeleton, ordinary NameView alive-only labels and Hidden/corpse opacity.
> The first `VIS-01` code checkpoint now preserves the real `image=10`
> CannibalPlant through Crystal Show/Hide frame timing, including early Show,
> non-Cannibal/unknown packet behavior and object-ID reuse regressions. The next
> bounded checkpoint renders Scarecrow `Monster/005` Die-phase frames
> `224..233` through the packed-atlas additive material, above the real map
> producer's full six-cell guard band, and removes/restores the layer when the
> Effect option changes without another packet. Commit `ef619b551` adds a
> source-bound 17-event typed Bichon transcript for male/female Warriors, Hen,
> Deer, Scarecrow and CannibalPlant, with 15 exact render checkpoints, exact
> layer sets, authoritative monster sprite/disposition/death projection,
> Candidate `CWeapon/01` atlas closure and production frame-set/real-`0.map`
> render-state binding. Review remediation `434bb06e6` then prevents retained
> packet overlays from masking hostile/neutral/friendly snapshot changes and
> makes all seven schema-v2 atlas pages fail closed on missing bytes, hash,
> decode or dimensions in runtime, VIS-01, packaging and copied-Candidate
> verification. VIS-02 has now closed its first bounded automated spell
> checkpoint: Lightning waits for the 600 ms Spell-action completion, follows
> the caster through `Magic/970 + direction*20` six-frame playback, emits the
> exact allowlisted `M40-0.wav` once, and fabricates neither projectile nor
> impact. Cast-false, reconnect/map/logout/departure and Effect-option
> regressions fail closed. Fresh source map and keyed packs now drive the full
> Windows suite at 333/333, and the vertical-slice gate explicitly generates
> both packs. VIS-01 and VIS-02 still require real Gateway/WSS sequence, GPU
> raster pixels and same-EXE captures; at that checkpoint the remaining
> VIS-02 spells were FlamingSword, FireBall, SoulFireBall and FireWall. The first bounded VIS-03
> UI-state checkpoint is now implemented at `448db4f72`: the 1024x768
> Inventory button remains source-bound to `Prguse/1903..1905`, BigMap
> Teleport uses Crystal's explicit disabled `Title/823` frame, and cached
> non-current-map search results cannot enable Teleport. Full Bevy native-ui
> passes 393/393, full Windows source-root tests pass 333/333, package and
> verifier self-tests pass, and independent read-only review found no P0/P1.
> This is render-state/semantic automation, not a same-EXE or visual
> acceptance result. Do not
> report full-game 90% or visual 100%: clean source binding, the semantic
> inventory, full legal assets, additive weapon/wing layers, same-EXE live WSS,
> 100/125/150% DPI, 30-minute native soak, human visual/audio/feel and
> publisher-signing gates remain open.

> VIS-02 FireBall bounded automated checkpoint (2026-08-27): revision
> `d85d7368119053e6b2609316c4f5c76faaa298cb` now derives FireBall's local
> missile from typed `ObjectMagic` after the 600 ms Spell action and consumes
> the Rust simulation's adjacent compatibility `ObjectProjectile` without a
> duplicate. Cast `Magic/0..9`, all 16 directional six-frame missile ranges,
> target-bound `Magic/170..179` impact, finite `MaxDistance * 50 ms` flight,
> launch-time direction locking and exact M31-0/1/2 audio identities are
> closed across runtime, exporter, package and verifier. Review remediation
> separates projectile frame cycling from lifecycle repeat, so unbound
> point-target missiles expire. Independent final review found no P0/P1;
> effects pass 59/59, Windows 340/340, Bevy native-ui 393/393, Gateway fixture
> 1/1, Web typecheck and the complete offline resource/audio gate pass. This
> does not close VIS-02: FlamingSword, SoulFireBall, FireWall, target-dead
> impact suppression and the Struck/Die/Dead/Revive chain remain open, as do
> same-EXE/live-WSS/GPU/DPI/soak/human/signing gates. Do not emit a global
> percentage.

> VIS-02 SoulFireBall bounded automated checkpoint (2026-08-28): revision
> `19991af6ddb289dc2fb22569849599caabf9195e` now follows Crystal's
> `ObjectMagic`-owned client presentation: immediate M64-0 with no cast bitmap,
> a local missile after the 600 ms Spell action, all 16 three-frame directional
> ranges at ten-frame stride, launch-time target binding/direction lock, finite
> target-following flight, target-bound impact and exact M64-0/1/2 identities.
> The native adapter ignores the Rust compatibility `ObjectProjectile` in every
> replay order. Windows 346/346, Bevy native-ui 393/393, focused SoulFireBall
> 6/6, FireBall regressions 11/11, Gateway projection 1/1, Web typecheck,
> exporter/resource gates and package/verifier self-tests pass. The Gateway
> fixture is serializer-only: the production no-amulet `cast=false` route is
> not wired. Target-dead suppression, backend timing/revalidation/PvP gaps,
> FlamingSword, FireWall, same-EXE/live-WSS/GPU/DPI/soak/human/signing and the
> incomplete denominator remain open. Do not emit a global percentage.

> MAP-E0/E1 bounded closure (2026-08-26): the real `Server.MirDB` map records
> now drive hazard/music packet metadata for all 464 source records, and all six
> current `_MAPCOORD` entries are typed and one-to-one bound to `NeedMove`.
> Package/Web respawn manifests are byte-identical; Web typecheck, generator
> 7/7, focused map tests 6/6, personal/shared coordinate tests 3/3, and the
> Gateway allowed-turn transfer regression 1/1 pass. General event scripts,
> doors/gates/walls, the complete six-gate Gateway matrix, exact delayed packet
> ordering, RNG traces, and persistent map state remain open; no whole-map or
> full-game parity claim is made. Reports:
> `docs/generated/player-qa/map-environment/MAP-HAZARD-DATA-SLICE-E0-REPORT.md`
> and `MAP-EVENT-BINDING-SLICE-E1-REPORT.md`.

> WN-UI-FUNC-01 R7.2 BuySell UI closure (2026-08-23): the targeted frontier
> re-review found one remaining P1 after the R7.1 model fix: the Buy renderer
> still gated on the legacy single `service_mode`, so a valid
> `NPCGoods -> NPCSell` sequence retained both capabilities but displayed only
> Sell. The native state now has an explicit local Buy/Sell presentation tab,
> both entries are reachable only when their authoritative capability is
> present, ShopConfirm follows the selected tab, and close/new-service resets
> to the safe Buy-first view. ECS input and Windows double-packet tests pass.
> This closes the final P1 from the independent review. The later
> `WN-STORAGE-REQID-01` slice closed the request-ID Storage protocol P2 in
> commit `c4652baf1`; real-window/visual/human gates remain open.

> WN-UI-FUNC-01 R7.1 independent-review remediation (2026-08-23): a
> `gpt-5.6-sol high` read-only review of `e32caf2cc` found no P0 and six P1;
> all six are now fixed. Login/register receipts are operation-scoped;
> NPCGoods followed by NPCSell retains combined Buy+Sell capabilities;
> Guild gold failures return exact non-mutating type 3/4 receipts while type 2
> applies authoritative guild expenditure; rank-name/permission receipts use
> `changeType + rankIndex`; and Android Guild storage is reachable through the
> shared `UiAction -> GatewayCommand -> wire` path. The adjacent chat P2 was
> also closed: Announcement/LevelUp/Hint no longer inherit Shout/System
> visibility. Updated gates pass ui-core 37/37, native-ui 374/374, Windows
> 276/276, runtime 180/180, Android 48/48, plus the focused Simulation Guild
> receipt test. The protocol-level Storage P2 identified by this review was
> subsequently closed by `WN-STORAGE-REQID-01`: Candidate Web, Windows and the
> shared Android adapter now issue `StoreItemV2` / `TakeBackItemV2`, and ACK/NACK
> correlation requires the exact request ID, operation and coordinates. Legacy
> packets remain compatibility-only. See
> `docs/generated/player-qa/native-storage-request-id/WN-STORAGE-REQID-01-REPORT.md`.

> WN-UI-FUNC-01 R7 non-visual functional closure (2026-08-23): shared UI
> Core, Windows native, runtime and Android adapters now cover the remaining
> bounded functional gaps without using desktop visual automation. Login and
> registration requests use operation-specific in-flight state and redacted
> diagnostics; storage acknowledgements release one exact transfer only; NPC
> shop Buy/Sell/Repair/SpecialRepair modes are authoritative and mutually
> exclusive; all 13 Crystal chat channel families and aliases are filtered
> independently. Group name invite and Guild recruit, member rank editing,
> eight canonical permission bits, notice editing, authoritative 112-slot
> Guild storage, paging and gold deposit/withdraw now share typed intents and
> bounded packet adapters on Windows and Android. Registry coverage is 173
> controls. Gates pass: ui-core 36/36, client-bevy default 119/119,
> client-bevy native-ui 366/366, Windows 275/275, runtime 180/180, Android
> 45/45 and Web typecheck. This closes the current non-visual code gate only;
> real-window mouse/keyboard, DPI, device, visual parity and human acceptance
> remain explicitly open in `docs/WN-UI-FUNCTIONAL-PARITY-CHECKLIST.md`.

> Fresh Web production-build closure (2026-08-22): the current source snapshot
> passes the complete `apps/web` production pipeline, including both release
> WASM backends, 9,650 entity frames, the 40,808-entry original-asset manifest,
> 58 map-atlas pages, TypeScript and 13/13 static pages. Isolating native-only
> Bevy UI/Text and the native ingest queue from the browser runtime reduced
> WebGPU to 27,119,641 raw / 5,902,117 gzip and WebGL2 to 28,489,677 raw /
> 6,342,038 gzip; the strict runtime budget and policy gates pass. BUILD_ID is
> `OXQE2c59Nd1B4bxoWcPQf`. This closes the local Web production-build P1 only;
> signed package/v2 attestation, deployed environment and human
> visual/interaction acceptance remain open. The separate strict local
> pre-seeded 64-client/30-minute Gateway soak passed on 2026-08-22; it closes
> WN-CANDIDATE Closing 4b only, not the Windows native-client soak.
>
> Windows package-chain audit (2026-08-22): both formal packaging and verifier
> self-tests pass, including ADS and reparse-point fail-closed fixtures. Formal
> staging remains externally blocked because the current user certificate store
> has no private-key certificate with Code Signing EKU. The historical `dist`
> package also predates v4 and lacks build attestation, package manifest,
> canonical release statement and detached CMS signature, so it remains an
> unsigned internal-playtest artifact and is not called Candidate.
>
> WN-WEB-PARITY-01 final non-visual integration sync (2026-08-21): explicit
> game-data monster disposition now flows through personal-session snapshots,
> shared-Zone authority and checkpoint restore without trusting stale Gateway
> projections. Player-versus-player attacks route to the player command family;
> monster attacks retain materialized authoritative transactions. Fresh
> external mail/GameShop item trees receive fresh server IDs recursively while
> Crystal storage split keeps its grid-scoped ID contract. Final gates pass
> Simulation 1,283/1,283, shared Zone 189/189, Gateway 529/0 with one
> environment-gated ignored test, ui-core 30/30, runtime 166/166, native-ui
> 254/254, Windows 237/237, Android 36/36, Gateway check, Web typecheck and
> diff-check. A focused real Axum `/ws` client now passes authenticated native
> GameShop buy -> currency/mail/receipt -> CollectParcel plus durable exact-claim
> checks, and the adjacent exactly-once reload test passes. Keep Candidate/package
> work open: no live PostgreSQL, deployed remote Zone, signed v2 package,
> 30-minute soak, Android device, or human visual/feel acceptance is claimed.
> See `docs/WN-WEB-PARITY-01-12H-EXECUTION.md`.

> Secure native reconnect server Phase 1 P1 rework closed (2026-08-21): the
> opt-in `nativeResumeV1` path now uses a real reserve/prepare/commit protocol.
> Binding and authoritative identity state are read-only validated first; the
> exact reconnect lease and capacity permits are then exclusively reserved
> without consuming the credential family. Route refresh and Zone live-outbound
> registration must both prepare successfully before one mutex transaction
> consumes the credential family and commits the lease. RAII restores the exact
> lease/token on route or Zone failure and prevents early-return permit loss;
> replay and concurrent commit still have one winner. Credentials are rejected
> during deserialization unless they are exactly 43-character unpadded
> base64url decoding to 32 bytes. Production defaults now bound WebSockets at
> 2,048, active sessions at 512 and reconnect leases at 512; both WebSocket
> upgrades cap frames/messages at 64 KiB, and the 256-entry socket queue has an
> enforced 16 MiB byte semaphore. Non-opted Web behavior, explicit leave
> revocation and MapChanged rotation remain unchanged. Gates: native resume
> 14/14, registry 6/6, Gateway lib 490 passed / 0 failed / 1 environment-gated
> PostgreSQL test ignored, and Gateway check passes. Keep Windows Phase 2 open:
> the source nonce is provenance metadata, not device binding; registry/leases
> are process-local, so no cross-instance or live-client acceptance is claimed.

> Mail durability follow-up closed (2026-08-21): ordinary GameShop mailbox
> creation, cross/self `SendMail`, and `ClientPacket::CollectParcel` now commit
> durable account state before changing the live World or emitting success.
> Every new delivery carries an opaque persisted 128-bit `deliveryNonce`;
> legacy entries receive a deterministic compatibility identity derived only
> from mail ID and immutable headers, never mutable status or claim-cleared
> payload. Ambiguous legacy rows with the same ID/header safely collapse to
> prevent double claim. Refresh keeps
> already-visible local mail IDs stable, rekeys only incoming collisions,
> preserves distinct identical-content deliveries, is idempotent for the same
> delivery, and keeps the active reversible lock state. Claim failure,
> malformed exact items, full bags, injected persistence failure, duplicate or
> concurrent claim all leave World/store/File unchanged. Active-character save
> without authenticated account identity now logs and exits instead of writing
> `demo`. Gates: Simulation lib 1,234/1,234, legacy focused 3/3, mail 28/28,
> `social_economy_integration` 3/3, `security_lifecycle` 18/18 and simulation
> check. Live PostgreSQL remains an environment gate; mirror mode remains
> compensating dual-write rather than distributed 2PC.

> Latest WN-WEB-PARITY-01 native social client sync (2026-08-21): Windows
> Group/Guild/Trade now has bounded typed read models, ordinary Crystal wire
> commands, modal UI intent routing, session reset handling, and fail-closed
> pending reconciliation. Scene reset preserves account-scoped social state;
> empty GuildStatus identity/rank clears stale guild state. Personal skill
> deltas require nonzero objectId equality with the current player. Automated
> gates: ui-core 25/25, client-bevy native-ui 243/243, platform-windows
> 195/195, social focused 9/9, and diff-check. Evidence:
> `docs/generated/player-qa/native-social/WN-SOCIAL-01-REPORT.md`. Live
> authenticated social flow and sender-correlated TradeGold/TradeConfirm ACK
> remain unclaimed.

> Latest SendMail persistence transaction sync: 2026-08-21 closes the
> recipient-first cross-character persistence P1. Player `SendMail` now stages
> the active sender checkpoint, exact unique-ID attachment removal, currency
> debit, and recipient mailbox append in an isolated `AccountStore` snapshot;
> File uses the existing temp-file + atomic-replace writer, while PostgreSQL
> source mode persists the touched accounts under one transaction and existing
> version checks. The shared store, live World and success ACK are updated only
> after persistence succeeds; self-mail uses the same all-or-nothing path.
> Fault injection covers fail-before-persist and fail-persist with byte-identical
> File state, plus successful cross-account/self-mail reload and one-call/
> one-mail conservation. A stale online recipient save also merges the durable
> mailbox instead of overwriting an external delivery. Gates: Simulation lib
> 1,220/1,220, mail 21/21,
> `social_economy_integration` 3/3, `security_lifecycle` 18/18, simulation check,
> rustfmt check for implementation files and scoped diff-check. The PostgreSQL
> rollback test is compiled and auto-runs against `MIR2_TEST_POSTGRES_URL`, but
> this workstation had no Docker/PostgreSQL service, so a live-DB execution is
> not claimed. File+PostgreSQL mirror mode is synchronously compensated on File
> failure, not a distributed 2PC; a process crash between the two backend
> commits can temporarily leave the non-authoritative mirror ahead.

> Correction closure for the WN-WEB-PARITY-01 security note below: the
> simulation mail findings are now repaired. Send/claim/save require real
> account identity, ordinary `CollectParcel` is durable-before-success, and
> malformed or failed claims are zero-change. The strict transfer parser,
> TCP-peer-only unsafe-loopback decision, `qaControl` boundary and Mail result
> correction were closed in their named slices. A fresh independent read-only
> review is still required before changing the overall Candidate label.

> Latest WN-WEB-PARITY-01 authority/security sync: 2026-08-21 adds the
> dedicated `gameShopBuy` player packet, server-side catalog/class/payment/
> price/balance/mail-capacity/attachment validation, exact stack-preserving
> Gameshop Mail delivery, and all-or-nothing claim of persisted attachment
> JSON. Player-command safety is now fail-closed by default; the only unsafe
> opt-out requires an explicit dev/test environment and a loopback peer, and
> production/staging cannot disable the gate. Normal Web React sources no
> longer wire generic `stage5Command`; incomplete social/market/conquest
> actions are disabled until a typed flow has real target/input/authority.
> Independent gates: Simulation 1,206/1,206, Gateway 461 passed / 1 ignored,
> ordinary candidate loop 1/1, Web typecheck, 68 Stage5 adapter groups and the
> player-UI source guard. Windows-native parity is not closed: active P1s are
> GameShop client closure, reconnect/session restoration, Group/Guild/Trade,
> SendMail, AbandonQuest and learned-skill casting. Finite GameShop stock and a
> request-correlated purchase result also remain open. Continue non-visual work;
> do not reinterpret older Web/package `100% Candidate` notes as native parity.

> Latest WN-WEB-PARITY-01 backend hardening: 2026-08-21 removes the remaining
> World Map fee split. Runtime discovery now reads `[Game]
> TeleportToNPCCost` from authoritative `Setup.ini`; the same loaded value is
> advertised in `WorldMapSetup` and enforced by the shared Zone. Missing,
> malformed, negative, or excessive values disable the feature. Gateway has a
> forced checkpoint-failure regression proving no client/observer packets and
> no gold/transform mutation before rollback; Zone regressions prove rollback
> restores AOI/occupancy and a committed teleport clears stale movement intent.
> Gates: Simulation 1,194/1,194 plus focused stale-intent, Gateway 456 passed / 1
> ignored, Big Map 7/7 + 11 internal, and Web typecheck. Real Crystal data
> remains disabled with zero eligible destinations. Continue the non-visual
> Windows queue with session isolation and inventory command parity; do not
> start visual/computer-control work.

> Latest R3 non-visual checkpoint: 2026-08-21 completes the bounded native
> audio adapter, runtime-authoritative `WorldMap.ini` loader, and shared-Zone
> `TeleportToNpc` transaction path. Windows Options now controls real Bevy WAV
> sinks and the package includes validated legal fallbacks. Teleport accepts
> only an object id, derives cost/map/destination server-side, validates the
> retained Zone NPC plus collision/occupancy, updates AOI/occupancy, commits
> gold through the personal checkpoint, and persists the Zone transform.
> Rejections are silent and mutation-free. The checked-in authoritative source
> remains `Enabled=False`, with zero imported `CanTeleportTo` NPCs, so live
> teleport intentionally remains disabled; enabled behavior is covered only by
> an explicit server-side fixture. Continue to defer visual/computer-control
> work per user direction. See `R3-AUDIO-WORLDMAP-ZONE-REPORT.md`.

> Latest non-visual checkpoint: 2026-08-21 completes Options runtime window/
> persistence consumers, shared-authority Chat Settings with local persistence,
> and BM-BE-01 map setup/search contracts. Android shared-state/vector
> validation now passes 10/10 host tests. The next non-visual work is, only
> when authoritative WorldMap data is enabled with eligible destinations, a
> separate frontier-led shared-Zone
> teleport round. Do not implement client-side teleport or mutate personal
> session coordinates. This R2 note is superseded by the R3 audio/Zone entry
> above. Per user direction, defer native Big Map rendering,
> lighting, screenshot calibration and computer-control QA.

> Latest native UI-core checkpoint: 2026-08-21 closes the bounded account
> shell, delete-confirmation, local Safe Key, shared reducer and strict
> Mail/Shop/Storage ingestion slices. Automated gates are green (ui-core 12,
> native-ui 167, runtime 146, Windows 164, Android host 7, Web typecheck).
> WN-UI-FUNC-01 remains the active UI queue: first wire Options effect
> consumers; then implement Crystal's server-data-driven Big Map; then Chat
> Settings and live server/mouse evidence. Android SDK/emulator/device proof
> and final visual/human acceptance remain separate release gates.

> Latest Windows-native alternate-class/combat checkpoint: 2026-08-19 adds
> Web-equivalent Archer alternate body/hair/weapon routing and Assassin
> alternate body/hair/directional dual-weapon routing. Numeric combat feedback
> is now sourced from Crystal's separate `DamageIndicator` packet while
> `ObjectStruck` remains the pose hint; hit/miss/crit/heal floaters are bounded,
> deduplicated and animated. Native F-key target ordering now prevents a stale
> server selection from overriding the nearest live hostile. Live Deer evidence
> is `10/25 -> 9/25 -> 8/25` with a visible red `1` in capture SHA-256
> `0E24F11B963382F02C82F0DAEEE745F51794A01460175E92523AABE7DCBD49AA`.
> Gates: Windows 104/104, runtime 133/133, Simulation 1183/1183, Release
> SHA-256 `B6A7078173865DF3415B089DE4119EAA438886EF518AFD9DEC69054B445773D9`,
> and Web typecheck. Keep WN-VIS-006 open. Next single-writer slice must add
> exporter/pipeline support for absent shadow/effect-mask metadata, then spell/
> projectile effects and lighting. Exact text/overlap, same-scene Gemini review,
> DPI/offline/reconnect matrix and human acceptance follow.

> Latest Windows-native entity-composition checkpoint: 2026-08-19 closes the
> bounded per-library frame-set and basic actor-overlay slice. Windows loads 697
> Crystal catalogs with exact frame ranges/cadence/fallbacks, respawns an actor
> when its body library or mount state changes, and composes body/hair/front-or-
> rear weapon/mount layers with source atlas geometry. Native-only labels,
> dead-state lines and the self HP bar now follow the authoritative world. Live
> evidence moved `288,615 -> 287,613`; capture SHA-256 is
> `343B4B05A9E67EF7B687F0DFA9B5D8D2F34E222A7AD95BC03AD6D1E4569E8DC4`.
> Gates: Windows 98/98, shared runtime 133/133, Release SHA-256
> `34B5053B58C2808FF5B7DD7ACE4FEE1721817BDE5977E7DE363EDA76F9B6A738`,
> and Web typecheck. Keep one native renderer writer on WN-VIS-006. Next order:
> alternate class libraries, shadow/effect masks and combat effects; then
> lighting, exact text/overlap policy, same-scene Gemini review, DPI/package
> matrix and human acceptance. Do not mark the whole entity or Goal gate closed.

> Latest Windows-native entity-animation checkpoint: 2026-08-19 moves atlas
> frame production from Gateway-message cadence to a persistent native main-
> thread Crystal clock. Monotonic packet hints cover walk/run/attack/range/
> magic/struck/die/revive, repeated snapshots preserve phase, and unsupported
> monster run/range/spell normalize to current audited actions. Real F12 frames
> 254 ms apart changed 5,087 world-only pixels; live TownRevive and movement
> reached `18/18 @ 288,616` then `288,615`. Gates: Windows 90/90, shared runtime
> 133/133, Release and Web typecheck. The single writer remains on WN-VIS-006:
> add per-library frameSet manifest data, then stable class/equipment composite
> layers and native labels/health bars. Do not mark the whole entity gate closed.

> Latest Windows-native map/entity/vitals checkpoint: 2026-08-19 closes three
> concrete renderer/data-adapter failures. The Windows entity producer consumes
> every schema-v2 atlas page plus source offsets; packet-first movement rebuilds
> the local real-map camera/HUD center; and the shared renderer invalidates a
> cached map layout when a later viewport exposes new rects, removing black map
> holes. Packet-authoritative self health/death now overrides stale personal
> snapshots, proven by visible `0/18` death, real `V` TownRevive to `288,616`,
> and live `18 -> 17 -> 16` damage. Evidence:
> `docs/generated/player-qa/native-windows-map-layout-round1/` and
> `docs/generated/player-qa/native-windows-vitals-round1/`. Gates: Windows
> 83/83, shared runtime 133/133, Release and Web typecheck. Full offline asset
> staging now passes with 8,325 files / 269.91 MiB and all required sentinels.
> This is not yet a release gate: make native-keyed/late ChrSel inputs
> reproducible from a clean checkout and launch the EXE outside the repository
> with `MIR2_NATIVE_ASSET_ROOT` unset.
> The single-writer queue remains WN-VIS-006: finish actor action/composite
> layers and labels, obtain deterministic exact `0 @ 257,594` full-window
> evidence, add lighting/effects, then run DPI, packaged-EXE launch and final
> >=92/human gates.

> Latest Windows-native HUD checkpoint: 2026-08-19 completes the first Crystal
> in-game HUD Candidate. Native Bevy now composes the source MainDialog, HP/MP
> orb clipping, horizontal belt, chat controls/four-line panel, main buttons and
> Bichon minimap on the fixed 1024x768 stage while preserving authoritative
> Gateway read models. Stable evidence is
> `docs/generated/player-qa/native-windows-hud-round2/windows-native-crystal-hud-round2-in-game-1787106442919-2.png`;
> HUD-only Antigravity High review is Accepted 88/100, `sameScene=true`, with no
> P0/P1, so the planned first-Candidate >=85 gate is closed. Tests pass at native
> UI 90/90 and Windows 80/80; Release and Web typecheck pass. The next writer is
> map/entity/effect and same-coordinate scene convergence. EXP/weight binding,
> bitmap text, final HUD >=92, DPI/package gates and human acceptance stay open.

> Latest Windows-native visual-parity checkpoint: 2026-08-19 completes
> WN-VIS-005 exact Crystal character select. The native shell now renders the
> source `Prguse/65` background, four empty/occupied class slots, selected-state
> frames, source-offset 16-frame class/gender preview animation, last-access row,
> and the five Crystal bottom buttons while preserving the authoritative Gateway
> roster and StartGame flow. The same-scene empty-roster review is Accepted
> 100/100 with `sameScene=true` and zero visible issues; an occupied-roster capture
> verifies the animated Warrior preview and selected metadata, and a real native
> pointer click reached authoritative `BichonProvince`. Final gates pass at
> Windows 80/80, shared Bevy 22/22, native-ui 77/77, Windows Release, Web
> typecheck, local shared-asset HTTP probes, and packaged ChrSel 237/237. Login
> remains Accepted 100/100. The next visual writer is the Crystal in-game
> HUD/chat/shortcut/minimap slice, followed by missing map/entity coverage,
> lighting/effects, 100/125/150% DPI and final human acceptance. The 2,494
> unavailable map references and map animation evidence remain explicitly open.

> Latest Windows-native vertical-slice round: 2026-08-19 completes the bounded
> main-branch work needed for a human-visible, independently rendered Windows
> client. `mir2-platform-windows.exe` now owns login, character, Bichon HUD,
> NPC/quest, target/combat, inventory and capture presentation through Bevy;
> Gateway/Simulation remain authoritative. The fresh-account q1-to-q2 smoke is
> green in one process, including real movement, HP-reducing attacks, direct
> Crystal Q-item delivery, TownRevive, reward turn-in and relog persistence.
> Six 1024x768 frames and the manual E2E matrix are indexed by
> `docs/NATIVE-WINDOWS-PLAYER-QA.md`. Final automated gates pass: Windows 73/73,
> shared Bevy 22/22, native-ui 56/56, WASM WebGL2/WebGPU, Windows Release, Web
> typecheck/runtime policy, Gateway/Web health and map API 18/18. The next
> native queue is visual parity rather than another protocol demo: correct the
> remaining entity atlas frame/anchor/library mismatches, replace the compact
> native HUD with exact Crystal HUD/chat/shortcut/minimap composition, and run
> 125%/150% DPI plus final human visual/feel acceptance. Do not mark those gaps
> closed merely because the playable vertical slice passes.

> Latest Bevy/mobile delivery sync: 2026-08-01. The adaptive touch/gamepad
> tutorial branch was rebased onto current main and its device-profile,
> gamepad-input, tutorial-flow, and TypeScript gates pass. The independent WASM
> runtime now uses Bevy 0.19 with Rust 1.95 and a size-oriented `wasm-release`
> profile; WebGPU is 27,605,807 bytes and WebGL2 is 28,999,520 bytes, reducing
> the previous tracked artifacts by 24.66% and 25.27%. Forced WebGPU, forced
> WebGL2, fallback, movement/presentation, and raw WebGL2 browser assertions all
> pass with no critical console errors. The Brazil support policy is now 2 GiB
> unsupported, 3 GiB experimental, 4 GiB engineering-only, 6 GiB provisional
> public minimum, and 8 GiB recommended. The code-side P0 is now closed: login,
> character-select, and game/HUD prewarm are screen-staged; low tier skips
> optional audio/scene-frame scatter; WebGL2 map textures use decoded-byte LRU
> budgets with explicit release and retain the previous frame during cold page
> loads; Bevy takeover releases duplicate WebGL2 residency; and all 40 map pages
> are 4096-safe with build/dev manifest gates. Full frontend logic, TypeScript,
> focused prewarm/LRU/atlas tests, and a live forced-low login are green. The
> remaining delivery P0 is publishing the regenerated pages through a new
> immutable R2 release (the live 20260730 prefix still has two 8192 pages),
> followed by physical 4 GiB Android certification; see
> `docs/LOW-END-ANDROID-SUPPORT.md`.
> The old World Director branch must not be merged wholesale; its true 12-commit
> tail is classified in `docs/WORLD-DIRECTOR-BRANCH-INTEGRATION.md`.

> Latest Gate 15 real-player continuity closure: 2026-07-24. Real WebSocket and
> Crystal TCP `StartGame` now acquire Commonware-finalized account/character
> session leases, both player Gateways observe quorum placement and dynamically
> refresh the Zone fencing generation, and repeated multi-session Zone
> checkpoints replay from an isolated account baseline. The Docker acceptance
> runs two players through separate Gateways, stops Dubhe A, finalizes Dubhe B
> at generation 2, proves both sockets remain connected and execute 113/52
> post-failover `UserLocation` responses, then recovers A with reverse
> replication. All validators and projectors finish at height 16 and one state
> root. Evidence and runbook:
> `docs/GATE15-REAL-PLAYER-FAILOVER.md` and
> `docs/generated/gate15/gate15-acceptance.json`. Production follow-up is
> continuous long-session lease renewal/revocation, a dynamic-fence movement
> fast path, multi-host soak, and production network/security packaging.

> Latest Gate 14 distributed-control closure: 2026-07-24. A real four-validator
> Commonware `v2026.2.0` Simplex network now finalizes event-driven control
> commands with 3-of-4 quorum and persistent certificates. Dual Gateways derive
> placement and fenced session leases only from quorum state; account,
> character, inventory, gold, placement, and lease authority replay
> deterministically into independent Postgres/Redis projections. The Docker
> acceptance stops a validator, Gateway, Redis, Postgres, and the active Dubhe
> Zone Host, then recovers all services at height 14 with identical validator
> and database roots. Evidence and the whole-project architecture are in
> `docs/GATE14-NO-SINGLE-POINT-POC.md` and
> `docs/generated/gate14/gate14-acceptance.json`. Production follow-up is
> authenticated ingress/mempool, non-root packaging, dynamic asymmetric
> committee identity, multi-region deployment, and long-running live handoff
> soak rather than more POC authority work.

> Latest final deterministic presentation closure: 2026-07-23. The maintained
> Web client now has a Rust-owned per-object animation state machine with
> persistent incarnation, FIFO action, death/revive, seeded idle, and Crystal
> NPC harvest phases; TypeScript consumes one pose result for every entity
> renderer instead of restarting animation from React snapshots. Chat now keeps
> Crystal's 17 channel types, colours, 614px wrapping, four-line history and
> scroll/filter semantics without browser timestamps. TCP and WebSocket clients
> share the production 5-minute `Online Players` and 10-minute `LineMessage`
> scheduler; all accelerated/fixed capture controls fail closed unless the
> Gateway has a non-empty QA control token. Eight fixed acceptance strings use
> reproducible Windows GDI raster assets, while arbitrary runtime text retains
> an accessible CSS fallback rather than pretending to be exact GDI.
>
> Same-scene r40 at Bichon `0 @ 328,275`, light 1, runtime
> `bevy-e9d354eada933661` is automated **100% Candidate**: runtime/layout/entity/
> pixel gates are 100%, world similarity is 89%, HUD UI 91%, chat 84%, MiniMap
> 87%, with zero critical console errors and zero non-favicon 404s. The current
> four-left-step WebGPU capture has 4/4 commands and ACKs, exact final delta
> `(-4,0)`, 320/320 local-pose comparisons, and no jump, rollback, route spam,
> or interaction pollution; the native/Web temporal report aligns all four
> actions and emits four bounded frame pairs. Forced WebGPU and WebGL2 runtime
> gates pass, all 1,440 full-pack libraries / 4,446 unique pages verify, and the
> final Gateway library regression passes 307/307.
> There is no remaining automated P0/P1 implementation item in this final
> deterministic scene. The sole queue item is a human **Accepted** visual/feel
> decision; roaming actor composition and sampled animation/effect phase mean
> raw full-window pixels are intentionally not claimed as bit-identical.

> Latest reproducible developer-handoff sync: 2026-07-22. The root clone path,
> maintained Crystal fork/branch, Node 22/Rust 1.89 bootstrap, Gateway/Web
> startup, verification suite, and private full-asset installation are now
> documented and scripted. The private bundle is content-addressed by
> `f71b89aa38504c6c127b937043d4af6ecd26d9dd1a2b9ed3b91100e6a1f0052e`
> and contains the exact 1,440-shard/4,446-page closure. Deterministic USTAR,
> per-part/archive/page hashing, safe-entry inspection, transactional install,
> exact remote-release closure, streaming R2 upload, and all-object remote
> probes form the handoff gate. Remaining delivery work after private Release
> and clean-clone proof is the credentialed R2 upload plus Brazil physical
> device acceptance; visual parity work below remains independently open.

> Latest fresh-native effect/HUD closure: 2026-07-18. A live Crystal/Web pair
> now uses the corrected Lime NPC packet state at Bichon `0.map @ 332,275`, Day
> setting 2. The pre-fix r05 baseline measured 15.0% full-window and 14.8%
> world changed pixels with Belt MAE 38.05. TrapHexagon frames were present in
> the DOM but trapped below the GPU canvas because a translated `z-index:auto`
> sprite parent formed a stacking context. Crystal effects now render in a
> pass-through layer and receive camera translation per effect; the incorrect
> CSS half-opacity Belt overlay is disabled. Final live r16 improves to 7.1%
> full-window / 6.0% world changed pixels, world MAE 4.499, and Belt MAE 10.765
> with 0 critical console errors and 0 404s. A locked-effect A/B gate proves 28
> visible nodes change 57,282 pixels inside their union; forced WebGL2 r09 also
> passes. The wrapper now waits for native files and reads long raw paths as
> Buffers; r15 completed with a 271-character raw path. Next: normalize chat
> content and remaining HUD/minimap typography, recapture without the Codex
> Computer Use status bubble in the native top-left, then run final strict
> movement/map and human visual/feel acceptance.

> Latest deterministic same-scene/CDP closure: 2026-07-18. The Edge 150
> `Runtime.enable` blocker is closed end-to-end through Next 16.2.1's compiled
> `ws`: Web-only r03 produced a complete pack at `0.map @ 332,275`, and r04
> removes the extension-only `Unchecked runtime.lastError` false positive while
> retaining real console failures. r04 reports 100% automated weighted
> Candidate trend, runtime/layout/entity/pixel gates 100%, 0 critical console
> errors, and 0 404s; raw comparison remains 10% full-window and 9% world
> changed pixels, so this is not human Accepted. Rust now also uses one Crystal
> Lime NPC-name constant across initial spawn, snapshot, and visible-object
> bundle paths; focused transfer 3/3, visible-object bootstrap 1/1, shared Zone
> 153/153, Rust fmt, Web CDP/report tests, and TypeScript pass. Next: capture a
> fresh native/Web pair after the NPC packet fix, then reduce the remaining
> HUD/chat and visible scene-effect deltas before the final human gate.

> Latest full-pack/low-tier delivery closure: 2026-07-14. The offline compiler
> now converts and verifies all 1,440 Crystal libraries as 1,440 lazy shards and
> 4,446 unique immutable PNG pages, with every one of 2,143,132 frame slots
> classified. Entity rendering prefers the full pack with legacy rollback;
> Bevy and WebGL2 residency are bounded by adaptive byte/entry LRU budgets.
> Forced-low WebGL2 Bichon stayed at 58,379,430 decoded bytes, passed 28/28
> movement/render assertions, and completed 403/403 reduced prewarm requests
> without 404s or critical console errors. Evidence is in
> `docs/generated/player-qa/full-asset-pack-low-tier/`. Next delivery tasks are
> CDN/release publication and physical Brazil 2/4 GiB Android throttled-4G soak;
> optional KTX2/UASTC is gated on WebGL2 pixel/alpha proof. The active visual
> queue remains deterministic same-scene Crystal/Web full-window delta
> reduction across camera, population, HUD, lighting, and effects.

> Latest map-render pipeline closure: 2026-07-13. Bichon black blocks were
> opaque matte texels entering the raw atlas path, not absent resources. Normal
> object-library frames now bypass the atlas for black-key decode, Mir3
> `Dungeonsc` is included, Crystal middle/front per-cell blend metadata survives
> live parsing, packaged fallback generation, and scene-cache misses/hits, and
> Bevy additive RGB no longer writes black alpha into its transparent canvas.
> A dedicated floor depth band also restores Crystal's floor-before-object and
> entity order. Final WebGPU/WebGL2 captures at `0.map @ 320,43` have zero pure
> black crop pixels, zero DOM map fallbacks, and zero console errors; mean
> backend RGB delta is about 0.10/255. Runtime 101/101, full frontend logic,
> TypeScript, focused routing, and release dual-WASM build pass. Evidence is in
> `docs/generated/player-qa/map-rendering/`. Next map queue: revision/key-aware
> GPU-ready ownership ACK, bounded additive material/image residency, then
> Crystal lighting/effects and human visual acceptance. Do not reopen the fixed
> black rectangles as missing map resources.

> Latest monster target-flow closure: 2026-07-13. A live monster click now
> locks selection, follows the monster through the existing authoritative
> movement/ACK pipeline, recalculates its adjacent approach tile as the target
> moves, attacks only after movement settles within one tile, and continues at
> the Crystal-local attack cadence. Manual movement, target death/removal,
> selection change, map change, and disconnect clear the chase. Browser proof
> moved Scout from `310,51` to `320,43`, entered attack at one tile after 2.3s,
> retained target selection, and stopped attacking for a 3.5s observation after
> a ground click. Evidence:
> `docs/generated/player-qa/combat/web-monster-lock-chase-20260713.{json,png}`.
> Full frontend logic and TypeScript pass. Next interaction queue remains
> alpha-aware/narrow entity hit testing; map-rendering differences remain a
> separate visual pipeline task.

> Latest self-melee closure: 2026-07-13. The missing animation was not an atlas
> failure: shared Zone emits observer-facing player id `50001`, while the owner
> renders personal self id `1000`, so its `ObjectAttack` matched no local entity.
> Crystal does not use that echo for self animation; it queues `Attack1` locally
> before sending `C.Attack`. Web now does the same for a live adjacent target and
> keeps short world-action commits out of React transition priority. Browser A/B
> changed from a 900ms `.attacking` timeout with accepted damage to a visible
> swing in 123ms followed by clean 600ms detach. Evidence is under
> `docs/generated/player-qa/combat/`; full frontend logic and TypeScript pass.
> Next combat-adjacent queue item: constrain alpha-transparent entity hit bounds
> so a large CherryTree cannot intercept clicks intended for a nearby Deer.
> The moving-monster AOI visual gate remains separate from this accepted local
> attack action.

> Latest movement-presentation ownership closure: 2026-07-12 supersedes the
> earlier r6 semantic baseline with exact Crystal composite-phase evidence.
> The remaining defects were duplicate TypeScript plus Bevy interpolation,
> separately committed map/entity centers, immediate fallback on one rejected
> pose, a hard-coded Bevy Run target, and phase 0 being shortened by the next
> global 100ms pulse. Bevy now owns local interpolation, map and entity
> producers commit as one scene transaction, the last coherent pose is held for
> a 250ms watchdog, movement shadow consumes the explicit command target, and
> local phases start at `command start + 100ms` without catch-up.
>
> Canonical WebGPU evidence is
> `docs/generated/player-qa/movement-jitter/movement-mounted-scene-transaction-full-phases-webgpu-20260712-r12.json`
> (33/33); WebGL2 evidence is
> `docs/generated/player-qa/movement-jitter/movement-mounted-scene-transaction-full-phases-webgl2-20260712-r16.json`
> (33/33). Both cover Walk phases `0..7` at exact map offsets
> `-6,-12,-18,-24,-30,-36,-42,-48px` and mounted Run phases `0..5` at
> `-24,-48,-72,-96,-120,-144px`, with a pinned self sprite, zero split or
> synthetic centers, zero shadow/pose/console failures, and matching
> `bevy-bd9004a17f2873ea` output. Runtime tests are 99/99, shared Zone is
> 152/152, frontend logic, TypeScript, focused Gateway routing, and the dual
> backend smoke pass. Preserve this movement core; the next queue is lighting
> composition, scene effects/demo-population cleanup, and human feel acceptance.
> Unattended capture is also hardened: Chrome now chooses an ephemeral CDP port
> and the harness reads that port from its own profile; an explicitly occupied
> port fails before launch. A no-`--debugPort` WebGPU rerun,
> `movement-mounted-autocdp-cleanup-webgpu-20260712-r19.json`, passes 33/33 and
> leaves zero new Chrome profiles.
> Do not report full Debug Gateway regression as green on this host. Two
> single-thread attempts aborted non-deterministically: first at test 43 with
> `0xc0000409`, then after passing that point near test 72 with
> `0xc0000374`. Both named tests pass alone; the first three-test group also
> passes. The repository has no matching local unsafe path, while Windows has
> seven WHEA records in the last seven days, including APIC 32 TLB/internal
> parity corrected machine checks. Keep Release e2e and focused movement gates
> as valid evidence, but retain host BIOS/CPU/RAM stabilization as a soak gate.

> Latest mounted-movement closure: 2026-07-12 closes the previously open
> Crystal eight-phase mount walk and true three-tile mounted Run slice. Crystal
> source truth is now one shared profile: foot Walk/Run use six 100ms phases,
> mounted Walk uses eight 100ms phases, mounted Run uses six phases and advances
> three cells, and an active unpaused Swift Feet buff also advances three cells
> unless the player is sneaking. Gateway now forwards owner-state packets such
> as `MountUpdate`, `ObjectSneaking`, and buff state from the personal Session
> into the shared Zone before later movement intents execute. A paused Swift
> Feet buff no longer remains active in Zone movement state.
>
> Strict Release evidence is
> `docs/generated/player-qa/movement-jitter/movement-mounted-walk8-run3-webgpu-20260712-r6.json`.
> It uses two real keyboard taps through Web prediction, WebSocket, Zone, and
> Bevy: Walk ACK is 18ms for one cell, Run ACK is 22ms for three cells, final
> delta is `(4,0)`, Bevy observes phase counts 8 and 6, command-to-pose coverage
> is 2/2 with a 26ms maximum sink latency, and all 27 strict assertions pass.
> Pose atomicity, rollback, direction lag, stale prediction, queue warnings,
> console errors, and non-favicon 404s are zero. Runtime
> `bevy-78d40eb80133609c` passes 96/96; shared Zone passes 152/152; complete
> frontend logic and dual-backend smoke pass. The user-facing Release Gateway is
> PID `42688` on `7011/7111` with logs
> `docs/generated/player-qa/runtime-logs/gateway-mount-state-sync-7111.{out,err}.log`.
>
> Next queue: preserve this movement baseline while closing the remaining
> scene-lighting/effects/actor rendering differences and final human feel
> acceptance. Do not reopen a PC-only Bevy rewrite as a movement fix: the
> closed defects were split semantic/state ownership, not browser GPU limits.

> Latest Zone-owned cadence and live-outbound architecture sync: 2026-07-12
> closes the two follow-up gaps previously listed here. Each shared Zone owner
> now runs one monotonic 300ms cadence, coalesces late ticks instead of replaying
> bursts, and advances pending movement, combat/projectiles, summons, monster
> AI, doors, hazards, buffs, and drop expiry exactly once per Zone. Personal
> `WorldCommand::Tick` no longer drives global Zone Tick, per-player movement
> Tick, or shared-drop expiry, so adding sockets cannot multiply world time.
> Realtime `UserLocation`, player appearance/removal, Turn, Walk, and Run packets
> use a bounded token-fenced socket channel; a blocked observer Session is no
> longer required to drain its mailbox. Queue full/closed still falls back to
> the reliable mailbox, and non-realtime gameplay side effects remain there.
>
> Strict latest Release evidence is
> `docs/generated/player-qa/two-client-zone/two-client-zone-zone-owned-cadence-tick5000-release-20260712.json`.
> With personal Session Tick intentionally slowed to 5000ms and observer pulses
> disabled, observer movement arrived in 12ms against a 250ms budget; both
> clients retained 16 entities, Bevy recorded one remote-motion event and 29
> packed-offset matches, and decode errors, queue drops, console errors, and
> non-favicon 404s were all zero. Focused cadence/live-outbound/blocked-runtime/
> fencing/fallback and delayed-combat tests pass; Simulation `shared_zone` is
> 148/148, full frontend logic and TypeScript pass, and Release was rebuilt.
> Final normal-port keyboard evidence
> `docs/generated/player-qa/movement-jitter/movement-zone-owned-cadence-final-release-keyboard-20260712.json`
> traverses the real input and Bevy presentation path: Walk/Run ACKs are 23/6ms,
> both commands reach the pose sink within 12ms, final delta is exactly `(3,0)`,
> and every strict movement/render/network assertion passes.
> Final user-facing Gateway is PID `44152` on `7011/7111` with logs
> `gateway-release-zone-cadence-final-20260712.{out,err}.log`.
>
> Do not claim total actor ownership yet: movement and global cadence are Zone
> owned, but several non-movement commands and personal side-effect commits
> still enter shared state from Session paths. Mounted eight-phase and true
> three-cell sprint parity, scene lighting/effects, and final human acceptance
> remain open. Windows Debug `0xc0000005` is a pre-existing host stability gate
> correlated with WHEA corrected machine checks and old BIOS/microcode; latest
> Release is live-green, but this host is not a production soak acceptance
> machine until WHEA is clean.
>
> Latest movement degradation/protocol sync: 2026-07-12 closes the first
> correction/degraded-run evidence slice. The early page ACK path and the
> movement controller now share `classifyMovementAckOutcome`, so a Run's
> one-cell first-step `UserLocation` is confirmed instead of being cleared as a
> correction. Shared Zone now matches Crystal by degrading an unprimed
> standstill Run to a one-cell Walk. `shared_zone` is green at 148/148 and the
> complete Web frontend-logic suite is green. Release raw packet evidence
> `docs/generated/player-qa/movement-jitter/movement-protocol-expired-run-degrades-release-202607120745.json`
> records ACKs at 16/99ms, one degradation, zero corrections, and delta `(2,0)`;
> normal UI evidence
> `docs/generated/player-qa/movement-jitter/movement-normal-walk-run-chain-release-202607120750.json`
> records Walk/Run ACKs at 22/28ms, pose latency 17/1ms, zero degradation or
> correction, and delta `(3,0)`. Debug's earlier 2375ms tail exposed an
> architecture risk rather than a Zone movement-cost problem: private
> `SimulationSession::tick()` still serializes with socket input. The active
> queue is now a bounded Gateway-owned Zone movement ingress with deterministic
> blocked-private-tick coverage; mounted eight-frame and true three-cell sprint
> motion remain the next movement-model slices.
>
> Latest default shared-clock/additive sync: 2026-07-12 promotes the guarded
> Bevy local self/camera pose and synchronous pose commit to the normal URL.
> Crystal's shared 100ms scene pulse now owns all six walk phases without
> waiting for a movement ACK; shared Zone remains the sole gameplay authority,
> and `?bevyLocalMotion=0&bevyPoseCommit=0` is the tested rollback. The default
> continuous route reached 3/3 commands with a 10ms maximum command-to-pose
> delay, the committed-keyboard route reached 4/4 with a 15ms maximum and
> returned exactly to `328,275`, and the aligned native/Web four-action spans
> both measured 2701ms. Exact rollback evidence completed 2/2 commands and ACKs
> with both ownership flags inactive. The final 25 Crystal-additive map sprites
> now use a custom Bevy `SrcAlpha + One` material; WebGPU and WebGL2 map smokes
> report zero DOM world sprites, zero image failures, and zero map 404s. Runtime
> `bevy-630a77b3535f95bd` passes 94/94 Rust tests and the dual-backend report
> `docs/generated/player-qa/bevy-runtime-backends/bevy-runtime-backends-default-shared-clock-202607120620.json`.
> Movement evidence:
> `docs/generated/player-qa/movement-jitter/movement-default-shared-clock-continuous-202607120610.json`,
> `docs/generated/player-qa/movement-jitter/movement-default-shared-clock-keyboard-committed-ref-202607120617.json`,
> `docs/generated/player-qa/movement-jitter/temporal-crystal-native-vs-web-default-shared-clock-horizontal-20260712-001.md`,
> and
> `docs/generated/player-qa/movement-jitter/movement-explicit-legacy-rollback-202607120623.json`.
> Next queue: capture real correction and degraded-run transitions, align the
> mounted eight-frame and three-tile sprint cases, then close native/Web scene
> population, ambient effect, light, and combat-VFX differences. Full-window
> visual-delta ratios are not actor-isolated and must not be scored as movement
> completeness when the two server worlds or capture geometries differ.
>
> Latest release-pose/map sync: 2026-07-10 removes the delayed TypeScript-window
> dependency from clean local-command presentation and gives map plus entities one
> atomic render center. The synchronous pose sink is rollback-gated by
> `?bevyPoseCommit=1` / `mir2-bevy-pose-commit`; local motion remains gated by
> `?bevyLocalMotion=1` / `mir2-bevy-local-motion`. Corrections, degraded runs, and
> target/path mismatch remain TypeScript-owned, while shared Zone remains the only
> movement authority. `npm run dev` now uses release WASM; use
> `npm run dev:debug-runtime` only for debug-runtime investigation. Map producer
> deduplication plus retained Rust tile generations reduce a four-step route from
> 53 sampled map revisions (`687 -> 999`) to five (`13 -> 21`) and avoid repeated
> sprite rebinding/full-state clones. Strict release WebGPU evidence
> `docs/generated/player-qa/bevy-movement-shadow/bevy-movement-shadow-webgpu-20260710220403-44ba1f45-report.json`
> passes 4/4 accepted sink takeovers at `14/18/32/16ms`, exact final `328,275`,
> and every geometry/provenance/cleanup/console/network assertion. Default-off
> compatibility report
> `docs/generated/player-qa/bevy-movement-shadow/bevy-movement-shadow-webgpu-20260710221024-ce1066ce-report.json`
> is green. Runtime `bevy-9ce93936c0841d7e` passes 86/86 Rust tests and the fully
> green WebGPU/WebGL2 report
> `docs/generated/player-qa/bevy-runtime-backends/bevy-runtime-backends-20260710221430.json`.
> Next queue: (1) capture exact native clean/correction/degraded-run temporal
> routes against this release candidate, (2) move map tiles to stable world-space
> cells with edge/chunk deltas and one-time atlas metadata, (3) promote the two
> rollback flags only after native temporal and human-feel acceptance. Additive
> materials and the Gateway multi-session crash remain separate lanes.
>
> Latest guarded local-motion sync: 2026-07-10 completes a default-off visible
> Bevy self/camera presentation slice without moving gameplay authority out of
> shared Zone. Normalized self commands and ACKs enter a bounded Rust
> `PreUpdate` resource; the unified pose buffer now tags matched self/camera
> output as `localCommand`. A three-part object + target + from/to path handshake
> is mandatory: corrections clear the local segment, degraded/rebased path or
> target mismatch remains `selfWindow`/TypeScript-owned, and a completed matched
> segment settles at zero rather than reconnecting a late window. Enable only
> with `?bevyLocalMotion=1` or `mir2-bevy-local-motion`; default remains false.
> Runtime `bevy-e50cfdd1e6c8d229` passes Rust 83/83, pose parser 6/6, movement
> bridge 9/9, TypeScript, and validated WebGPU/WebGL2 release packages. Final
> backend evidence
> `docs/generated/player-qa/bevy-runtime-backends/bevy-runtime-backends-20260710173210.json`
> proves path-mismatch fallback and matched takeover in default/forced WebGPU
> and forced WebGL2. Real WebGPU baseline
> `docs/generated/player-qa/bevy-movement-shadow/bevy-movement-shadow-webgpu-20260710173245-17db8e6b-report.json`
> is green with 76/76 exact geometry samples; forced takeover
> `docs/generated/player-qa/bevy-movement-shadow/bevy-movement-shadow-webgpu-20260710173356-7b3abddd-report.json`
> is also green with 76/76, final self/camera `localCommand`, 4/4 commands, 4/4 ACKs,
> 0 jumps, 0 drops/errors/404s. Map regression
> `docs/generated/player-qa/bevy-map-standalone/bevy-map-standalone-webgpu-20260710173500-ca321fe7-report.json`
> remains green. Next implementation/evidence order: (1) capture the same native
> Crystal route and Web default-off/forced-on frame sequences, (2) score the
> command-start phase difference (currently up to 32px / 326ms vs the TS window)
> and correction/degraded-run transitions, (3) promote local takeover to default
> only if native temporal evidence improves, (4) independently add Bevy additive
> materials for the final 25 DOM world sprites. The native Gateway two-client
> crash remains a separate blocker to real packet-to-render multiplayer proof.
>
> Latest unified-pose sync: 2026-07-10 gives packed Bevy sprites, the self
> camera, and DOM overlays one versioned per-frame presentation pose. `isSelf`
> is explicit on the packed wire; frame start computes one camera pose, self is
> its exact inverse, remote entities record the actual packet/fallback offset,
> and the bounded 256-entry buffer is exposed to the DOM rAF driver. Invalid,
> stale (>250ms), missing, or disabled data falls back to TypeScript, while
> `?bevyPresentationPose=0` disables only the bridge and leaves Bevy transforms
> unchanged. A first real-route failure caught two 20/22px jumps caused by
> independently sampled self/camera windows; the implementation was corrected
> at the source and the strengthened rerun is green. Runtime
> `bevy-8a40d0bdcf0dc14a` passes Rust 72/72, pose parser 5/5, movement bridge
> 9/9, TypeScript, release build, and package self-check. Dual-backend evidence
> `docs/generated/player-qa/bevy-runtime-backends/bevy-runtime-backends-unified-pose-20260710.json`
> passes every default/forced WebGPU, forced WebGL2, raw WebGL2, package, API,
> pose, and console assertion. Real route report
> `docs/generated/player-qa/bevy-movement-shadow/bevy-movement-shadow-webgpu-20260710163125-1a4aff1b-report.json`
> records 0 jumps, 4/4 commands, 4/4 ACKs, 1219 Bevy pose samples, 4 startup
> fallbacks, 38908 entity hits, and 0 overflows/errors/404s. Map report
> `docs/generated/player-qa/bevy-map-standalone/bevy-map-standalone-webgpu-20260710162936-ca18422e-report.json`
> is also green. Next implementation order: (1) keep the current self path as
> fallback and shadow-compare Bevy local prediction/reconciliation, (2) enable
> its presentation ownership behind a rollback flag only after exact ACK and
> correction parity, (3) separately add Bevy additive materials for the final
> 25 DOM world sprites. Shared Zone retains all gameplay authority.

> Latest Bevy movement-architecture sync: 2026-07-10 completes the first visible
> packet-driven presentation slice. Copies of normalized remote motion/remove
> events now enter a bounded Bevy `PreUpdate` resource and directly drive packed
> remote-sprite offsets at Crystal's 600ms stepped cadence. A target-coordinate
> handshake prevents a packet segment from attaching to a stale packed snapshot;
> target mismatch, feature disable, or inactive motion falls back to the existing
> TypeScript presentation path. Connected segments preserve their fractional
> displayed pose, while stale events, discontinuities, removals, and capacity
> limits are deterministic. The feature is default-on with packed Bevy entities
> and has `?bevyRemoteMotion=0` as the rollback switch. Runtime
> `bevy-63449641a633efc2` passes 67/67 Rust tests (13 focused remote-presentation)
> and 9/9 TypeScript bridge tests. Real Chrome/WASM evidence
> `docs/generated/player-qa/bevy-runtime-backends/bevy-runtime-backends-remote-motion-probe-20260710.json`
> passes default/forced WebGPU and forced WebGL2, proving mismatch fallback,
> matched-target offset takeover, disable cleanup, zero decode/event drops, and
> zero critical console errors. Map and movement-shadow regressions remain green:
> `docs/generated/player-qa/bevy-map-standalone/bevy-map-standalone-webgpu-20260710162936-ca18422e-report.json`
> and
> `docs/generated/player-qa/bevy-movement-shadow/bevy-movement-shadow-webgpu-20260710154640-c847d5b3-report.json`.
> Unified sprite/camera/DOM pose is complete in the sync above; the next guarded
> slice is local self prediction/reconciliation. Shared Zone keeps collision,
> occupancy, cooldown, AOI, correction, and persisted transforms.
>
> Open evidence blocker, tracked separately from renderer work: a real
> two-client shared-Zone run repeatedly terminates the native Gateway during
> multi-session/reconnect lifecycle with Windows exception `0xc0000005` or
> `0xc0000374`, including isolated Rust 1.89 and 1.93 builds. Therefore no real
> packet-to-render end-to-end claim is accepted yet. Diagnostic record:
> `docs/generated/player-qa/two-client-zone/two-client-zone-native-crash-20260710.json`.

> Latest Bevy map-pipeline sync: 2026-07-10 closes the non-additive packed-atlas
> miss path and adds an explicit renderer-ownership handshake. Runtime lifecycle
> readiness no longer aliases diagnostic status events, so
> `map-render-synced` cannot disable map rendering or stop world snapshots.
> Normal-blend atlas misses decode through a bounded cache, upload as standalone
> Bevy images, and remain DOM-owned until Rust confirms a complete map sync;
> packed atlas coverage similarly keeps the WebGL2 fallback until all required
> page keys are confirmed. Failed atlas Promise entries are evicted for retry,
> ordered upload/evict operations preserve bridge ordering, and the previous
> complete Bevy frame remains visible while new standalone images decode.
> Isolated evidence
> `docs/generated/player-qa/bevy-map-standalone/bevy-map-standalone-webgpu-20260710162936-ca18422e-report.json`
> is `ok=true` at `0 @ 324,41`: 421 atlas tiles, 109 standalone draws, 108/108
> decoded standalone sources, 7 pages / 115 images, 0 failed images, 0 map 404s,
> 0 critical console errors, and only 25 additive DOM sprites. Dual-backend
> evidence
> `docs/generated/player-qa/bevy-runtime-backends/bevy-runtime-backends-unified-pose-20260710.json`
> is fully green; current runtime is `bevy-8a40d0bdcf0dc14a`. The shadow task
> named here is now complete in the sync above.

> Latest native/Web movement comparison sync: 2026-07-10 reran the movement
> temporal evidence after the Crystal stepped-cadence change. The initial
> native capture blocker was a stale/black Direct3D window, so Crystal was
> relaunched from `E:\mir2\Crystal\Build\Client\Debug\Client.exe` and logged in
> as `cdx0708235326`; the valid native capture is
> `docs/generated/player-qa/movement-jitter/original-crystal-valid-step-route-20260710.json`
> with 90 JPEG frames, 4 real Computer Use clicks, and average sample delta
> `50.12ms`. Web was captured against the live `7111` Gateway using a fresh QA
> account and headed window-frame capture:
> `docs/generated/player-qa/movement-jitter/web-crystal-window-fresh-step-route-20260710.json`
> with 86 JPEG frames, average sample delta `50.11ms`, 3/3 walk ACKs, average
> ACK `233ms`, max ACK `457ms`, 0 failed assertions, 0 interaction pollution,
> 0 critical console errors, and 0 non-favicon 404s. Final report
> `docs/generated/player-qa/movement-jitter/temporal-crystal-native-vs-web-window-20260710.md`
> is `ok=true` and records aggregate visual delta/sec Crystal `68.0367` vs Web
> `37.9166` (Web ratio `0.5573`). Next tasks: treat movement transport as green
> for this route, then tune residual render/scene-motion energy, object/layer
> motion, and exact-route capture setup; production Web correctly rejects debug
> crystal transfer, so use natural accounts or safe QA-only setup for future
> route alignment.

> Latest movement/render cadence sync: 2026-07-09 source-audited Crystal
> `MapControl.DrawFloor/DrawObjects`, `PlayerObject.Process()`, and
> `GameScene.CanMove` before touching the Web map pipeline. The 1024x768 Web
> origins (`tileLeft=470`, `entityLeft=480`) are intentional and match
> Crystal's split floor/object vs entity anchors, so this round preserves the
> map render anchor instead of chasing the visible 10px difference. The actual
> mismatch was temporal: Web/Bevy entity and camera motion used linear
> interpolation, while Crystal movement advances in 6 frames on the 100ms
> `CanMove` cadence with even-pixel offsets. Web
> `original-client-scene-motion.ts` and Bevy runtime `motion.rs` now use the
> Crystal stepped cadence for entity offsets, camera offsets, and fractional
> chained moves; `build-bevy-runtime.mjs` now reads `Cargo.lock` correctly on
> CRLF files. Dev runtime packages were rebuilt as
> `bevy-e48cd43dadfddb17`. Evidence passed `node
> scripts/test-scene-motion.mjs`, `cargo fmt --check; cargo test --lib motion
> -- --nocapture` (24/24), and Bevy backend smoke
> `docs/generated/player-qa/bevy-runtime-backends/crystal-step-motion-runtime-20260709.json`
> (`ok=true`, package fetches healthy, default WebGPU selected, forced WebGL2
> rendered, 0 critical console errors). Next tasks: rerun same-route native/Web
> movement video capture against this runtime, then tune residual object/light
> scene deltas.

> Latest main-scene light render sync: 2026-07-09 lands the first frontend
> render pass after dynamic `lightSetting` propagation. Web now mounts
> `.viewport-crystal-light-overlay` for Dawn/Evening/Night, leaves Day/Normal
> unchanged, and positions the layer after sprites but before nameplates so the
> world/actors darken while labels and UI remain readable like Crystal's
> `DrawLights()` order. Evidence
> `docs/generated/player-qa/visual-parity/scene-light-render-20260709/`
> records a clean Night scene with `overlayClass=viewport-crystal-light-overlay
> night`, `data-light-setting=4`, `z-index=6`, `pointer-events=none`,
> `tutorialOpen=false`, and browser console errors `0`. The same round exports
> `OriginalMapCell.light` through the scene API and renders map-cell light nodes
> inside the overlay; `map-light-export-probe-20260709.json` confirms map `0`
> samples with 127 / 127 / 25 / 26 light cells. A fresh map-light DOM screenshot
> was not captured because the real Crystal UTC light window rotated back to
> Day, correctly suppressing non-Day overlay rendering. Next tasks: recapture
> map-cell lights during Night/Evening/Dawn or add a safe QA-only light
> override, tune intensity against native same-time captures, then add
> object/equipment/effect light sources and rerun same-coordinate visual packs.

> Latest dynamic TimeOfDay/lightSetting sync: 2026-07-09 closes the server ->
> snapshot -> browser state light propagation lane. Crystal source confirms
> `Envir.Now = DateTime.UtcNow + Time` and `AdjustLights()` maps
> `Now.Hour * 2 % 24` to Dawn/Day/Evening/Night before sending
> `S.TimeOfDay`; Simulation StartGame and `WorldSnapshot.lightSetting` now use
> the same UTC-hour formula instead of a fixed Day/Night value. Web applies
> `snapshot.lightSetting` and exposes it through `window.__mir2Stage5.state`.
> Evidence
> `docs/generated/player-qa/visual-parity/light-setting-snapshot-20260709/`
> records direct WS `TimeOfDay.lights=4`, `worldSnapshot.lightSetting=4`, and
> browser state `lightSetting=4` with 0 critical console errors and 0
> non-favicon 404s. Verification passed Rust fmt, focused Simulation/Gateway
> tests, Gateway check/build in isolated target, Web TypeScript, and scoped
> diff checks. Next tasks: implement Crystal-like main-scene light rendering
> for Night/Evening/Dawn, then rerun same-coordinate visual packs before
> judging HUD-belt transparent-slot pixels.

> Latest Crystal/Web same-coordinate HUD/chat sync: 2026-07-09 re-ran the
> native/Web pack at Crystal account `cdx0708235326` on Bichon `0 @ 335,266`.
> Evidence `docs/generated/player-qa/visual-parity/crystal-web-pack-20260709-0060-minimap-source-panel-viewrect-native335266-clean/`
> is the clean rebuilt-gateway coordinate proof: runtime/layout/entities `100%`,
> 0 network 404s, 0 critical console errors, MiniMap `86%`, HUD UI `86%`, and
> Web state matched native vitals/items/gold/belt at `335,266`. Follow-up
> packs 0061/0062 proved `crystalVisibleChatLines` must replace, not append,
> bootstrap logs and that `Mode`/`Pet`/`Now in Net` need Crystal channel colors;
> however native `LineMessage.txt`/history keeps rotating, so chat pixels remain
> diagnostic noise unless the native visible slots are controlled. Packs
> 0063-0065 fixed Web Belt consumable quantity `1` visibility and Crystal-style
> black shortcut labels/yellow belt counts. `hudUi` meanAbsDelta improved
> (`~17.3` -> `15.8` in 0065), but `hud-belt` remains `78%` because transparent
> belt slots expose remaining world camera/light/background mismatch, not a
> standalone Belt data bug. Verification: `npm.cmd exec tsc -- --noEmit` passed.
> Next tasks: stop chasing dynamic chat/belt score directly; prioritize
> camera/viewport parity, world light rendering, AOI/object-set alignment, then
> movement/video feel evidence.

> Latest fair-coordinate/MiniMap light sync: 2026-07-09 closes two evidence
> blockers after the 0055 HUD pass. First, `capture-crystal-parity.mjs` now
> requires `qa.applyNativeState` to match `mapFileName` and `position.x/y`, and
> shared Gateway routing marks `qa.applyNativeState` as a Zone transform-sync
> command so `world_snapshot()` cannot be pulled back to stale Zone presence.
> Evidence
> `docs/generated/player-qa/visual-parity/crystal-web-pack-20260709-0056-main-hud-fair-visible-coord/`
> records Web `player` and `authoritativePlayer` both at `334,263`, transfer
> mode `alreadyAtTarget`, runtime/layout/entities `100%`, 0 network 404s, 0
> critical console errors, and a fair-coordinate overall `99.5%` / pixel
> `98.6%`. Second, the earlier Simulation StartGame pass stopped fixing the
> current client to Night (`TimeOfDay { lights: 4 }`) by emitting Day
> (`lights=2`) so Web used Crystal's then-current `Prguse/2093` MiniMap light
> icon; the dynamic TimeOfDay/lightSetting sync above now supersedes fixed Day.
> Evidence
> `docs/generated/player-qa/visual-parity/crystal-web-pack-20260709-0057-minimap-light-day-bootstrap/`
> records `miniMapLight.originalSrc=/original-ui/Prguse/2093.png`, MiniMap
> `0.784/32.788` -> `0.786/32.545`, and runtime/layout/entities `100%`. Next
> tasks: reduce true MiniMap raster/color/marker gap, reduce world
> scene/camera/object-frame mismatch, model chat `History` / `StartIndex`, then
> attach movement/video evidence.

> Latest Main HUD content-y sync: 2026-07-09 closes the stable 2px vertical
> drift across the main HUD shell. 0050/0054 crop analysis showed `hud-left`,
> `hud-right-controls`, `hud-right-status`, and `hud-bottom-center` all
> aligned best with Web shifted down 2px, while independent `hud-belt` and
> `chat` did not. Web now keeps `.main-hud-shell` at `0,616` but shifts the
> inner `.main-hud` content by `top: 2px`; `capture-crystal-parity.mjs` also
> has opt-in `--targetTolerance` for QA transfer's one-tile capture wobble,
> defaulting to strict `0`. Evidence
> `docs/generated/player-qa/visual-parity/crystal-web-pack-20260709-0055-main-hud-content-y-offset/`
> records rightControls `0.720/49.436` to `0.986/0.303`, rightStatus
> `0.734/42.642` to `0.824/14.189`, bottomCenter `0.800` to `0.886`, and
> hudUi `0.782/34.113` to `0.856/15.453`, with runtime/layout/entities
> `100%`, 0 network 404s, and 0 critical console errors. 0055 is a HUD proof,
> not a new overall baseline, because dynamic world/minimap/chat lowered
> overall to `95.9%`. Next tasks: reduce true MiniMap raster/color sampling
> gap, reduce world scene/object-frame mismatch, model chat `History` /
> `StartIndex`, then attach movement/video evidence.

> Latest Belt/HUD overlay draw-order sync: 2026-07-09 fixes a source-backed
> Belt brightness mismatch. Crystal `InventoryDialog.BeltDialog` draws
> `Index + 1` (`1933` / `1945`) from `BeltPanel_BeforeDraw`, and
> `MirControl.Draw()` runs `BeforeDrawControl()` before the control's main
> `DrawControl()`, so the half-opacity overlay sits behind the main Belt frame
> (`1932` / `1944`). Web had the DOM order reversed, causing the overlay to
> darken the panel. Evidence
> `docs/generated/player-qa/visual-parity/crystal-web-pack-20260709-0054-belt-overlay-draw-order/`
> records auto-generated crop pairs and moves `hudBelt` from the 0050 baseline
> `0.765` / meanAbsDelta `48.963` to `0.791` / `38.920`; `hudUi` improves
> `0.778` / `35.215` to `0.782` / `34.113`. 0054 is a Belt proof, not a new
> overall baseline, because chat rotated again (`chat=75.9%`, overall
> `97.9%`). Next tasks: reduce remaining HUD rightControls/rightStatus
> asset/color/antialias drift, reduce true MiniMap raster/color sampling gap,
> reduce world scene/object-frame mismatch, then attach movement/video
> evidence.

> Latest same-scene evidence tooling sync: 2026-07-09 extends
> `capture-crystal-web-pack.mjs` so every Crystal/Web pack automatically writes
> native/Web crop pairs for the same report regions: `world`, `hud-full`,
> `hud-left`, `hud-belt`, `hud-right-controls`, `hud-right-status`,
> `hud-bottom-center`, `minimap`, and `chat`. Evidence
> `docs/generated/player-qa/visual-parity/crystal-web-pack-20260709-0053-auto-region-crops/`
> confirms 9 crop pairs plus `summary.cropSet` entries. 0053 is a tooling
> validation, not a new fair baseline, because native chat rotated again
> (`chat=67%`, overall `96.9%`); `hudRightStatus` stayed at the 0050 baseline
> (`0.734`, meanAbsDelta `42.642`). The 0051/0052 GDI-outline HUD text probes
> did not beat 0050 and were not retained. Next tasks stay: reduce remaining
> HUD rightControls/rightStatus asset/color/antialias drift, investigate belt
> background brightness/overlay deltas, reduce true MiniMap raster/color
> sampling gap, reduce world scene/object-frame mismatch, then attach
> movement/video evidence.

> Latest clean same-scene chat-slot baseline: 2026-07-09 adds a
> `crystalVisibleChatLines` JSON override to the Web startup-chat capture path
> and uses it in
> `docs/generated/player-qa/visual-parity/crystal-web-pack-20260709-0050-chat-visible-slots-current/`.
> This lets the harness reproduce the native client's current four visible
> ChatDialog slots instead of letting `LineMessage.txt` rotation or native
> history/scroll state pollute the comparison. 0050 records visible chat
> `Online Players: 1 / Welcome to Crystal Mir 2 released by Suprcode. / Online
> Players: 1 / Online Players: 1`, 0 network 404s, 0 critical console errors,
> runtime/layout/entities `100%`, overall `98.5%`, pixel trend `96%`, chat
> `83%`, HUD full/UI `78%`, world `83%`, MiniMap `80%`, and the 0046
> weight-bar diagnostics (`weightRatio=0.2258`, `fillWidth=16`,
> `hudRightStatus=0.734`). Use 0050 as the current fair automated visual
> baseline. Next tasks, in order: reduce remaining HUD rightControls/rightStatus
> asset/color/antialias drift, investigate belt background brightness/overlay
> deltas, reduce true MiniMap raster/color sampling gap, reduce world
> scene/object-frame mismatch, then attach movement/video evidence.

> Latest chat LineMessage capture-control diagnostic: 2026-07-09 adds explicit
> `--gatewayWs` and `--crystalLineMessage` options to
> `capture-crystal-web-pack.mjs`, so the pack harness can append deterministic
> WebSocket routing and Crystal startup LineMessage text without hand-building
> the full URL. Evidence
> `docs/generated/player-qa/visual-parity/crystal-web-pack-20260709-0047-chat-line-message-sync/`
> records Web visible chat lines matching the requested native text, preserves
> the 0046 weight-bar diagnostics (`weightRatio=0.2258`, `fillWidth=16`), and
> stays runtime-clean with 0 network 404s and 0 critical console errors. The
> chat metric remains low (`65%`) because native Crystal currently leaves an
> empty/filtered line slot before the LineMessage while Web renders seeded
> startup lines contiguously. Next chat task: model Crystal `ChatDialog`
> `History` / `StartIndex` line-slot behavior; do not treat LineMessage content
> seeding alone as closed.

> Latest HUD weight-bar visual parity sync: 2026-07-09 closes the
> source-backed rightStatus fill-width mismatch. Crystal `MainDialogs.cs`
> keeps `WeightBar.DrawImage=false` and draws only
> `(WeightBar.Size.Width - 2) * CurrentBagWeight / Stats[BagWeight]` pixels in
> `WeightBar_BeforeDraw`, with `Prguse/76` for <=50%, `UI_32bit/473` for
> <=75%, and `UI_32bit/472` above 75%. Web was rendering `Prguse/76.png` as a
> full 76px bar; it now clips the source sprite to the Crystal fill width and
> exports the missing `UI_32bit/472.png` and `473.png` frames. Evidence
> `docs/generated/player-qa/visual-parity/crystal-web-pack-20260709-0046-weightbar-source-fill/`
> records the native-state case `currentWeight=14`, `maxWeight=62`,
> `weightRatio=0.2258`, `fillWidth=16`, 0 network 404s, 0 critical console
> errors, runtime/layout/entities `100%`, and `hudRightStatus` moving from
> 0045's similarity `0.727` / meanAbsDelta `45.137` to `0.734` / `42.642`.
> Overall stays `97%` because chat is still dynamically mismatched (`71%`);
> keep 0042 as the cleaner fair overall score. Next tasks, in order: reduce
> remaining rightControls/rightStatus asset/color/antialias drift, investigate
> belt background brightness/overlay deltas, stabilize chat-line capture,
> reduce MiniMap raster/color sampling gap, reduce world scene/object-frame
> mismatch, then attach movement/video evidence.

> Latest HUD right-button coordinate visual parity sync: 2026-07-09 closes a
> small source-backed rightControls coordinate drift. Crystal `MainDialogs.cs`
> positions the 1024px HUD buttons at `Size.Width - 105/55/119/96/73/50/27`
> (`919`, `969`, `905`, `928`, `951`, `974`, `997`), while Web was rendering
> each one 1px too far left. Web CSS now uses the Crystal source coordinates.
> Evidence
> `docs/generated/player-qa/visual-parity/crystal-web-pack-20260709-0045-hud-right-button-source-coords/`
> records 0 network 404s, 0 critical console errors, runtime/layout/entities
> `100%`, and `hudRightControls` moving from 0042's similarity `0.715`
> / meanAbsDelta `51.576` to `0.720` / `49.436`. Overall stays `97%` because
> this capture's chat crop is dynamically mismatched (`67%`), so keep 0042 as
> the fair overall score and use 0045 as right-button proof. Next tasks, in
> order: reduce rightControls/rightStatus HUD asset/color drift, investigate
> belt background brightness/overlay deltas, stabilize chat-line capture,
> reduce MiniMap raster/color sampling gap, reduce world scene/object-frame
> mismatch, then attach movement/video evidence.

> Latest Belt/HUD diagnostic visual parity sync: 2026-07-09 closes the
> source-backed Belt shortcut-label layering mismatch and improves HUD evidence
> granularity. Crystal `BeltDialog` creates `Key[i]` as direct children at
> `(8 + i*35, 2)` while item cells sit at `(i*35 + 12, 3)`, so labels remain
> visible over occupied potion slots. Web now renders belt labels as direct
> belt children with those parent coordinates and a higher z-index instead of
> nesting them inside slots and hiding `1`/`2` behind potion icons. The capture
> harness records `labelRect`, and `report-crystal-visual-parity.mjs` now emits
> HUD subregions plus a `hudUi` aggregate. Evidence
> `docs/generated/player-qa/visual-parity/crystal-web-pack-20260709-0044-belt-key-label-diagnostics/`
> records label rects `1 @ 238,620 26x14` and `2 @ 273,620 26x14`, 0 network
> 404s, 0 critical console errors, runtime/layout/entities `100%`, `hudUi=78%`,
> and subregions `left=79%`, `belt=77%`, `rightControls=72%`,
> `rightStatus=73%`, `bottomCenter=80%`. Overall is `97%` because this sample's
> chat crop is dynamically mismatched (`67%`); keep 0042 as the latest fair
> overall score while using 0044 for Belt proof. Next tasks, in order: reduce
> rightControls/rightStatus HUD asset/color drift, investigate belt background
> brightness/overlay deltas, stabilize chat-line capture, reduce MiniMap
> raster/color sampling gap, reduce world scene/object-frame mismatch, then
> attach movement/video evidence to this native-state pack flow.

> Latest MiniMap label/light/radar visual parity sync: 2026-07-09 closes the
> current source-backed MiniMap label, light-icon, and radar-dot mismatches.
> Crystal `LocationLabel` uses `Functions.PointToString`, so Web displays
> coordinates as `335, 262`; the coordinate label now keeps Crystal's `56x18`
> vertically centered box, and MiniMap labels use Arial like Crystal `MirLabel`.
> Missing `Prguse` light frames `2092`, `2094`, and `2095` were exported, and
> Web maps Crystal `TimeOfDay` light states to `2093/2095/2094/2092`. The radar
> overlay now uses Crystal-style 2x2 `RadarTexture` rects at `(x - 0.5,
> y - 0.5)`, skips dead entities, and keeps Crystal's player/NPC/other/owned
> object color rules where Web state exposes ownership. Evidence
> `docs/generated/player-qa/visual-parity/crystal-web-pack-20260709-0042-minimap-radar-dot-label-welcome/`
> records `miniMapLightOriginal=/original-ui/Prguse/2092.png`, 0 network 404s,
> 0 critical console errors, overall `98%`, estimated human band `91-100%`,
> pixel trend `96%`, HUD `78%`, world `83%`, minimap `80%`, chat `83%`, and
> MiniMap meanAbsDelta `29.535` versus 0039's `29.718`; crop pairs plus diff
> heatmaps are attached. Next tasks, in order: tune remaining bottom-panel HUD
> asset/layout drift and text placement, reduce true MiniMap raster crop/color
> sampling gap, reduce world scene/object-frame mismatch, then attach
> movement/video evidence to this native-state pack flow.

> Latest main-HUD text visual parity sync: 2026-07-09 closes another small but
> concrete HUD mismatch from the fair native-state comparison. Crystal
> `MainDialogs.cs` formats the bottom-right `GoldLabel` as
> `GameScene.Gold.ToString("###,###,##0")`, and Crystal `Settings.FontName`
> defaults to `Arial`, so Web now renders grouped gold (`3,457`) instead of raw
> `3457` and pins the main HUD to Arial instead of inheriting the page serif.
> Evidence
> `docs/generated/player-qa/visual-parity/crystal-web-pack-20260709-0036-hud-font-arial-cleanline/`
> records Web DOM HUD gold text `3,457`, weight `48`, space `38`, HP `51/51`,
> EXP `48.33%`, 0 network 404s, 0 critical console errors, overall `98%`,
> estimated human band `91-100%`, pixel trend `95%`, HUD `77%`, world `83%`,
> minimap `79%`, and chat `82%`; crop pairs are included in the pack. The
> earlier gold-only probe
> `docs/generated/player-qa/visual-parity/crystal-web-pack-20260709-0034-hud-gold-format/`
> scored HUD `78%`, so treat the Arial step as source-backed visual cleanup
> rather than an automated score improvement. Next tasks, in order: tune
> remaining bottom-panel HUD asset/layout drift and text placement, align
> minimap crop/color, reduce world scene/object-frame mismatch, then attach
> movement/video evidence to this native-state pack flow.

> Latest ChatDialog/HUD orb visual parity sync: 2026-07-09 closes the visible
> startup chat content/state gap from the fair native-state comparison and
> fixes the obvious HP-only orb crop error. Web no longer pollutes the opening
> ChatDialog with `Game shop stock`, `TimeOfDay`, or mode/pet state-sync
> messages; `LineMessage` now renders as Crystal's blue AutoSize chat label;
> the startup visible window can follow Crystal's rotating `Envir/LineMessage.txt`
> via `?crystalLineMessage=...`; the empty chat input is hidden like Crystal
> `ChatTextBox.Visible=false`; and low-level Warrior full HP uses the complete
> 101px red orb instead of the previous two-resource half-width crop. Evidence
> `docs/generated/player-qa/visual-parity/crystal-web-pack-20260709-0033-chat-and-hp-orb-clean/`
> records matching visible chat lines `Online Players: 1 / Welcome to Crystal
> Mir 2 released by Suprcode. / Online Players: 1 / Online Players: 1`, 0
> network 404s, 0 critical console errors, overall `98%`, estimated human band
> `91-100%`, pixel trend `96%`, HUD `78%`, world `83%`, minimap `79%`, and chat
> passing at `83%`. Next tasks, in order: tune remaining bottom-panel HUD
> asset/layout drift and text placement, align minimap crop/color, reduce world
> scene/object-frame mismatch, then attach movement/video evidence to this
> native-state pack flow.

> Latest bottom-right HUD parity sync: 2026-07-09 closes the clearest P1 HUD
> semantic mismatch from the fair native-state comparison. Crystal
> `MainDialogs.cs` shows `WeightLabel = Stats[BagWeight] - CurrentBagWeight`
> and `SpaceLabel = User.Inventory.Count(null)`, so Web no longer renders the
> old `freeBagSlots/maxBagSlots` plus current weight in that area.
> `WorldSnapshot.maxWeight` now comes from Crystal player stats; the main HUD
> displays remaining bag weight and the Crystal 46-slot free-space view; and
> the previously hidden gold row is visible again. Evidence
> `docs/generated/player-qa/visual-parity/crystal-web-pack-20260709-0027-hud-weight-diagnostics/`
> records `currentWeight=14`, `maxWeight=62`, HUD `48 / 38`, gold `3457`,
> expected HUD `48 / 38`, 0 network 404s, 0 critical console errors, and a
> matching right-HUD crop against native Crystal. Overall remains `94%`
> because the remaining top gaps are chat panel content/state, bottom-panel
> asset/layout drift, minimap crop/color, and world scene/frame mismatch.

> Latest native-state/max-MP/EXP visual parity sync: 2026-07-09 moves the Crystal/Web
> same-scene lane past the state-pollution blocker. `capture-crystal-web-pack.mjs`
> can now run the native Crystal account extractor, upsert the matching Web
> account-store character, emit `qa-character-state.json`, and pass that payload
> into `capture-crystal-parity.mjs`. The live Web session then applies the
> snapshot through token-gated `qaControl -> stage5Command -> qa.applyNativeState`
> and waits for HP/MP/gold/inventory/belt/equipment alignment before transfer
> and scoring. Missing potion icons were exported as
> `public/original-ui/Items/398.png` and `394.png`, and `WorldSnapshot` now
> carries `playerMaxMp` so post-transfer Web state stays at `MP 32/32`.
> `upsert-web-account-from-crystal-state.mjs` now reads Crystal `ExpList.ini`,
> so the same level-6 character syncs as EXP `435/900` and the Web HUD shows
> `48.33%` instead of `100.00%`.
> Evidence
> `docs/generated/player-qa/visual-parity/crystal-web-pack-20260709-0025-exp-debug/`
> is runtime-clean: overall `94%`, estimated human band `87-100%`,
> runtime/layout/entities `100%`, pixel trend `85%`, 0 network 404s, and 0
> critical console errors. Superseded next HUD subtask: the bottom-right status
> numbers are now covered by the 0027 pass above. Next tasks, in order: tune
> true HUD asset/layout drift (chat panel state and panel overlap), align
> minimap crop/color, reduce world scene/object-frame mismatch, then add
> movement/video evidence from this same native-state pack flow.

> Latest HUD-state diagnostic sync: 2026-07-09 turns the remaining P1 HUD gap
> into a concrete state-alignment task instead of a vague screenshot complaint.
> `capture-crystal-parity.mjs` now records HUD DOM text plus inventory, belt,
> storage, and equipment item summaries, `report-crystal-visual-parity.mjs`
> detects fresh/starter Web state and compares against optional native account
> state, and `extract-crystal-account-state.mjs` can read local Crystal
> `Server.MirADB` with the generated item manifest. Evidence
> `docs/generated/player-qa/visual-parity/crystal-web-pack-20260709-0019-hud-state-diagnostics/`
> parsed native account `cdx0708235326` / character `Cdx0708235326` fully
> (`finalOffset=byteLength=106534`) and explains the HUD score drop: Web is
> level 1, HP `18/18`, MP `14/?`, gold 0, empty belt/inventory, and starter
> equipment, while native Crystal is level 6, HP 51, MP 32, gold 3457, belt
> `(HP)DrugSmall` + `(MP)DrugSmall`, and geared equipment. Next tasks, in
> order: use this native-state extractor to seed or safely align the Web
> capture character state, rerun same-scene evidence, then tune true HUD
> asset/layout, minimap crop/color, and reproducible chat contents.

> Latest Crystal/Web visual blend sync: 2026-07-09 moves the same-scene
> visual pass forward on additive map effects and evidence hygiene. Bevy map
> rendering now keeps Crystal glow/blend sprites on the DOM fallback instead
> of drawing them as normal-alpha atlas quads, and the DOM path records
> per-sprite `filter` in the capture state. Tall blue/white columns and compact
> torch glows use separate screen-blend curves so torches avoid the previous
> yellow/dark orb while columns remain bright. The capture harness now supports
> `--createAccount` / `--characterName`, which allowed Web to use the native
> visible character name. Evidence
> `docs/generated/player-qa/visual-parity/crystal-web-pack-20260709-0017-same-account-native335/`
> passed at the native client coordinate `0 @ 335,262` with overall `97%`,
> human band `90-100%`, pixel trend `92%`, 400 Bevy map tiles, 12 DOM blend
> sprites, 0 network 404s, and 0 critical console errors. Next tasks, in order:
> close the P1 HUD state/asset delta, align minimap crop/color, make chat
> contents/state reproducible, then align Web player HP/MP/equipment/belt state
> with the native character so HUD scoring stops mixing real asset gaps with
> dynamic account-state gaps.

> Latest QA-control landing: 2026-07-08 adds a token-gated WebSocket
> `qaControl` wrapper for local automation only. Normal production player
> commands still reject `MoveTo`, raw `Stage5Command`, and debug
> `crystal:<map>:<x>:<y>` transfers, while `qaControl` requires
> `MIR2_GATEWAY_QA_CONTROL_TOKEN` on the gateway and `MIR2_QA_CONTROL_TOKEN`
> in the harness. Verification passed `node --check
> apps/web/scripts/qa-combat-survival.mjs`, `cargo test -p mir2-gateway
> qa_control -- --nocapture`, and `cargo build -p mir2-gateway --bin
> mir2-gateway`. Live evidence
> `docs/generated/player-qa/combat-survival-default-selfcamera-rust7111-qacontrol2-20260708/report.md`
> ran against Rust `7111` with production command safety enabled: incoming
> damage passed (`18 -> 0`) and death/revive passed (`0 -> 18`, town revive).
> Remaining tasks, in order: add explicit QA-control ACK/settle evidence because
> transfer/spawn packets can arrive late, stabilize seeded pickup movement,
> restore DOM damage-floater rendering, then tune normal fresh-character kill
> pace for `ObjectDied`/XP/drop.

> Latest Rust `7111` attack-trace result: 2026-07-08 hostile incoming damage is
> now green from a real Web client. `qa-combat-survival.mjs` records target
> map/object id, sent attack frames, approach trace, delayed server combat
> packets, and `StartGame` retry attempts. Evidence
> `docs/generated/player-qa/combat-survival-default-selfcamera-rust7111-survivalattacktrace5-20260708/report.md`
> reached melee with natural `ForestYeti` object `258949`, sent 24 attack
> frames, observed target `ObjectAttack` / `ObjectStruck` /
> `DamageIndicator`, and dropped player HP `18 -> 3`. Next tasks, in order:
> repair or replace the QA/admin control lane so `transferMap`, controlled
> hostile spawn, and death/revive isolation are deterministic; rerun
> death/revive away from live hostile AI; rerun normal attack-kill, XP, and
> loot evidence; then address recurring original asset/metadata 404s.

> Latest Rust `7111` combat-survival rerun: 2026-07-08 pickup/death are green in
> targeted evidence after QA route hardening. `qa-combat-survival.mjs` now waits
> for drop-click route progress before injecting fallback movement, supports QA
> pickup/combat seed windows, excludes passive animals from retaliation probes,
> and sends explicit ticks during survival. Evidence
> `docs/generated/player-qa/combat-survival-default-selfcamera-rust7111-pickupwait5s-20260708/report.md`
> passes Blue Potion pickup (`GainedItem x1`, carried `0 -> 1`) and
> `@DIE`/`townRevive`; `survivaltick` keeps pickup/death green but does not yet
> prove incoming monster damage. Focused Gateway tests pass for hostile passive
> AI override and Stage5 `event.spawn` Zone synchronization. Next tasks, in
> order: stabilize real-client hostile encounter approach so an adjacent
> attack/ObjectAttack/player-damage packet chain is captured, rerun a normal
> combat window for kill/XP/drop, then address the recurring first StartGame
> attempt race.

> Latest Rust-gateway combat/effect settled probe: 2026-07-07 supersedes the
> earlier anchor-only combat red sample for the current frontend feel lane.
> `apps/web/scripts/qa-combat-survival.mjs` now falls back from missing DOM tile
> hitboxes to normal one-step `walk` packets (never debug `MoveTo`), rotates
> melee-adjacent approach tiles, waits for late CDP WebSocket combat frames
> before declaring a target unresponsive, and records state-backed target HP
> drops. `apps/web/app/page.tsx` now sends targeted combat-confirm `tick`
> packets after attack/range/cast commands and spawns `DamageIndicator`
> floaters directly from the packet path with Rust/Zone positive-damage
> semantics. Evidence
> `docs/generated/player-qa/combat-survival-default-selfcamera-rust7111-floaterfix30s-20260707/report.md`
> connected to Rust `ws://127.0.0.1:7111/ws` (`gatewayIsRust=true`), completed
> 11/11 beats, and improved combat from "no damage observed" to: attack landed,
> target HP fell (`minPercent=95`), 4 server `DamageIndicator` packets arrived,
> and DOM `.scene-damage-floater` reached peak 1 (`damageIndicators=pass`).
> Remaining red/gap tasks: attack-kill still fails because the level-1 melee
> loop did not kill the target inside 30s, loot/XP remain unproven without a
> kill, `@DIE` still does not force a dead/revive state for this normal client,
> and missing original UI assets/metadata still produce 404s (`Sound/103.wav`,
> Monster metadata, and run-dependent sound ids). Next task: fix kill cadence /
> death lifecycle / loot+XP evidence, then rerun the same Rust `7111`
> combat-survival path until attack-kill, death/revive, loot, and XP are green
> or explicitly accepted.

> Latest Rust-gateway combat/effect anchor probe: 2026-07-07 supersedes the
> earlier `7110`/safe-zone-heavy combat sample. The default self-camera combat
> harness now writes partial reports after every beat, uses atomic report-file
> writes, treats Crystal field safe-zone circles as unsafe-for-combat anchors,
> and transfers directly to the Woomyon combat anchor `1:315,100` instead of
> judging the safe spawn `1:315,82`. Evidence
> `docs/generated/player-qa/combat-survival-default-selfcamera-rust7111-anchor-20260707/report.md`
> connected to Rust `ws://127.0.0.1:7111/ws` (`gatewayIsRust=true`) and ran
> outside the safe zone (`map=1`, player around `307,102`). It completed 11
> beats with 10 ok, but remains red: 10 melee attacks were sent against
> `ForestYeti` object `258949`, the server echoed attack movement/position
> traffic, yet no `ObjectStruck` / `DamageIndicator` / target `ObjectHealth`
> drop / `ObjectDied` was observed. Survival also stayed red (`RakingCat0`
> retaliation left HP at 18), and `@DIE` did not enter a dead/revive state.
> Next task: debug Rust gateway/Zone combat routing and player death lifecycle
> from real client `attack` / `chat @DIE` frames, then rerun this anchor probe
> until attack-kill, damage-floater, incoming damage, death/revive, loot, and XP
> have green or intentionally accepted evidence. Secondary frontend asset gaps
> from the same run: missing `original-ui/Sound/103.wav` and missing
> `api/original-ui-meta?library=Monster%2F007` metadata for `RakingCat0`.

> Latest combat/effect-heavy probe: 2026-07-07 starts the next evidence lane
> after held/chorded movement. Short default self-camera combat QA
> `docs/generated/player-qa/combat-survival-default-selfcamera-20260707/report.md`
> produced 11 stage screenshots and completed 11/11 harness beats against
> `http://127.0.0.1:3002/?bevyBackend=webgl2&bevyEntities=1&bevyAtlas=1`, but
> it is red evidence, not acceptance: the client connected through `7110`
> (Rust `7111` was not running), forced field transfer was unreliable, melee
> engagement failed (`attackKill=fail`), `.scene-damage-floater` stayed skipped
> because no damage landed, and `@DIE` did not enter a dead/revive state. One
> useful signal did pass: the survival beat observed player HP falling
> `18 -> 9`, so incoming damage / HP-surface evidence is present. Next task:
> bring up Rust gateway `7111` or harden the combat harness against `7110`,
> then get a green attack-kill/damage-floater/death-revive evidence set. The
> magic/effect harness remains blocked before report generation: attempts under
> `docs/generated/player-qa/magic-skills-default-selfcamera-*` currently stall
> around login/register and need harness timeout/flow repair before they can be
> used as effect-layer evidence.
> Harness note: `qa-combat-survival.mjs` and `qa-magic-skills.mjs` now put a
> 15s timeout around each Chrome DevTools Protocol command, so future browser
> stalls should fail into the existing report-writing path instead of being
> killed by an outer command timeout with no summary.

> Latest default self-camera held/chorded sync: 2026-07-07 extends the current
> Bichon movement baseline beyond click routes. The default-URL chorded/cardinal
> keyboard capture
> `docs/generated/player-qa/movement-jitter/web-motion-keyseq-bichon-cardinal-default-selfcamera-windowfps-content-jpeg-20260707-2000.json`
> is `ok=true` with 148 JPEG frames, 8 movement commands, final `329,270`, no
> failed assertions, no logical rollback, no interaction pollution, and Bevy
> WebGL2 packed rendering. The first default self-camera held Shift+Right
> window capture
> `docs/generated/player-qa/movement-jitter/web-motion-heldrun-bichon-right-default-selfcamera-windowfps-content-jpeg-20260707-2000.json`
> intentionally remains red evidence: it moved from `330,270` to `345,270` but
> exposed one non-render logical rollback when a queued direction intent was not
> counted as active movement evidence (`predicted 332,270 -> server 331,270`).
> `apps/web/app/page.tsx` now treats an unconsumed, fresh direction
> `queuedMoveIntent` as self-movement transport evidence, preventing prediction
> cleanup between consecutive run ACKs. Verified rerun
> `docs/generated/player-qa/movement-jitter/web-motion-heldrun-bichon-right-default-selfcamera-windowfps-content-queuedintentfix-jpeg-20260707-2000.json`
> is `ok=true` with 122 JPEG frames at ~50ms cadence, 8 movement commands,
> average ACK `198.5ms`, max ACK `439ms`, final `345,270`, 0 logical rollback
> warnings, 0 failed assertions, 0 capture errors, 0 interaction pollution, and
> no console/network failures. Temporal report
> `docs/generated/player-qa/movement-jitter/temporal-keyhold-native-sendinputscan-vs-web-default-selfcamera-heldrun-queuedintentfix-20260707.md`
> is `ok=true`, but the native side is still a short 12-frame SendInput scan
> sample, so keep the next task focused on equal-duration native held/video
> evidence plus combat/effect-heavy scenes.

> Latest default self-camera temporal sync: 2026-07-07 promotes the Bevy
> self-camera + per-entity interpolation path from opt-in to default when the
> Bevy entity/map renderer is actually active, with `?bevySelfCamera=0` /
> `?bevyEntityInterp=0` escape hatches. The residual DOM self overlay now
> cancels the parent camera transform so the nameplate/health overlay stays
> pinned to the Crystal stage center instead of producing visual jumps. Native
> Crystal evidence
> `docs/generated/player-qa/movement-jitter/original-motion-computeruse-route-bichon-4click-highfps-20260707-2000.json`
> remains `ok=true`, 104 JPEG frames over 5167ms, average sample delta
> `50.17ms`, and 4 real clicks at `51/950/1860/2763ms`. Matching default-URL
> Web content-only evidence
> `docs/generated/player-qa/movement-jitter/web-motion-clicksequence-bichon-samedir-4click-windowfps-content-default-selfcamera-jpeg-20260707-2000.json`
> is `ok=true`, 105 JPEG frames at ~50ms cadence, 4/4 Walk ACKs with average
> ACK `139.25ms` and max `369ms`, no visual jumps, no interaction pollution,
> no failed assertions, and no console/network failures. The final report
> `docs/generated/player-qa/movement-jitter/temporal-native-highfps-route-vs-web-windowfps-content-default-selfcamera-clicksequence-bichon-20260707.md`
> is `ok=true`: normalized visual delta/sec is Crystal `63.7831` vs Web `62`
> (Web ratio `0.972`), and changed-pixel/sec is Crystal `1.718936` vs Web
> `1.7788` (Web ratio `1.0348`). Next task: broaden this default self-camera
> evidence to held/chorded and busier combat/effect scenes, then tune HUD/chat
> and effect-layer temporal polish.

> Latest native/Web 4-click temporal sync: 2026-07-07 extends the real-input
> Computer Use path from one click to a sustained four-click route. Native
> Crystal evidence
> `docs/generated/player-qa/movement-jitter/original-motion-computeruse-route-bichon-4click-20260707-2000.json`
> is `ok=true`, with 23 captured native frames and 4 real window clicks through
> Computer Use. Web now supports explicit `--interaction clickSequence` routes
> in `capture-web-movement-jitter.mjs`; the first same-area route
> `docs/generated/player-qa/movement-jitter/web-motion-clicksequence-bichon-4click-left-jpeg-20260707-2000.json`
> intentionally remains red evidence because the fourth click hit
> `Teleport_Gilbert` and emitted a non-movement `interact`. The accepted clean
> Web route
> `docs/generated/player-qa/movement-jitter/web-motion-clicksequence-bichon-leftclean-4click-jpeg-20260707-2000.json`
> is `ok=true`, 29 JPEG frames, 4/4 walk ACKs, average ACK `204.25ms`, max ACK
> `590ms`, 0 frame capture errors, 0 critical console errors, and 0 interaction
> pollution. Temporal report
> `docs/generated/player-qa/movement-jitter/temporal-native-computeruse-route-vs-web-clicksequence-bichon-leftclean-20260707.md`
> is `ok=true` and records aggregate visual delta Crystal `11.42` vs Web
> `10.11` (ratio `0.8853`). Next task: capture the native side at higher
> cadence/video-derived frames and replay the same clean route so the remaining
> smoothness gap is judged on equivalent input geometry and frame timing.

> Latest native Computer Use movement sync: 2026-07-07 closes the native
> synthetic-input capture blocker for this round. The new
> `apps/web/scripts/capture-original-computer-use.mjs` uses Computer Use
> window capture/input against `Legend of Mir 2`, saves frame images in the
> same JSON shape as native movement evidence, and successfully drives real
> Crystal movement. Evidence
> `docs/generated/player-qa/movement-jitter/original-motion-computeruse-click-620-520-20260707-2000.json`
> captured 9 native frames while Crystal moved from the `287,611` area toward
> `288,612`. Matching Web same-scene/same-input evidence
> `docs/generated/player-qa/movement-jitter/web-motion-clicktarget-bichon-287-611-plus1-left-jpeg-1800ms-20260707-2000.json`
> is `ok=true`, 10 JPEG frames, one `walk DownRight`, final `288,612`, 0
> failed assertions, 0 capture errors, and 0 interaction pollution. Temporal
> report
> `docs/generated/player-qa/movement-jitter/temporal-native-computeruse-click-vs-web-clicktarget-bichon-1800ms-20260707.md`
> is `ok=true`: native mean visual delta `7.09` / changed-pixel ratio
> `0.16855` versus Web `4.51` / `0.108783` over an aligned ~1.8s click window.
> Next task: repeat this Computer Use path on longer run/route samples and
> tighten capture cadence/video extraction before making human-feel parity
> claims.

> Latest frame-cadence automation sync: 2026-07-07 converts the current
> "Crystal feels smoother than Web" concern into repeatable temporal evidence.
> `report-movement-temporal-parity.mjs` now analyzes consecutive frame images
> with downscaled pixel-diff metrics, and `capture-web-movement-jitter.mjs`
> supports scheduled frame sampling, blank WebGL canvas detection/fallback, and
> optional JPEG screenshots. The reliable Web evidence
> `docs/generated/player-qa/movement-jitter/web-motion-keyhold-right-jpeg-cadence-20260707-2000.json`
> is `ok=true`, captures 23 real JPEG frames at about 98ms average cadence,
> sends `Walk, Run, Run` to `335,270`, has 0 failed assertions, 0 capture
> errors, and 0 interaction pollution. The generated report
> `docs/generated/player-qa/movement-jitter/temporal-keyhold-native-static-vs-webjpeg-cadence-20260707.md`
> shows Web aggregate visual delta `7.09` versus the current native Crystal
> synthetic-input sample `0.37`. Do not treat that native sample as accepted
> Crystal movement cadence yet: Win32 keyboard/click automation captured
> frames, but it did not reliably move the Crystal client. Follow-up SendInput
> scan-code keyboard, right-click target, and left-click target samples also
> stayed near static deltas (`0.43`, `0.33`, `0.46` respectively), so the next
> task is native Crystal real-input or video-frame capture automation rather
> than more synthetic Win32 input tweaks.

> Latest held/chorded Bichon movement sync: 2026-07-07 closes the next
> long-keyboard movement repro after the crowded click-route pass. A forced
> WebGL2 held Shift+Right capture first exposed a non-render rollback at
> `0:339,270`: full Crystal world runtime was still carrying the hand-authored
> `starter-east-field-gate` demo transfer, so the fifth run step batched
> transfer packets and snapped the player back toward `0:330,270`. Full
> Crystal world runtime now clears starter demo `map_transfers` and relies on
> generated Crystal movement records. Evidence before fix:
> `docs/generated/player-qa/movement-jitter/web-motion-heldrun-bichon-right-webgl2-movelog-20260707.json`
> with ACK warnings `7481/4066ms` and rollback `339 -> 337`; evidence after
> fix:
> `docs/generated/player-qa/movement-jitter/web-motion-heldrun-bichon-right-worldtransferfix-20260707.json`
> is `ok=true`, ACKs `359/152/200/247/91/57/92/146ms`, final `345,270`,
> no rollback, no ACK warnings, and Bevy WebGL2 packed/no DOM fallback. The
> chorded cardinal rerun
> `docs/generated/player-qa/movement-jitter/web-motion-keyseq-bichon-cardinal-worldtransferfix-rerun-20260707.json`
> is also `ok=true` with 8/8 expected movement ACKs, no stale prediction, no
> command queue warnings, and no interaction pollution. Next task: compare
> native Crystal held-key animation/frame cadence against these now-clean Web
> server movement traces.

> Latest crowded Bichon movement ACK sync: 2026-07-07 closes the clean
> Bichon click-route repro after separating entity-hit pollution from a Gateway
> post-ACK timing race. The movement harness now supports route patterns,
> entity-hit avoidance, interaction-pollution assertions, and final Bevy
> renderer readiness waits; self sprites/nameplates do not intercept ground
> movement clicks. Shared Zone ACKs movement that arrives after ready without
> waiting for a later world tick, and the Gateway post-movement input window is
> now 1.5s (Crystal run grace plus one tick). Evidence
> `docs/generated/player-qa/movement-jitter/web-motion-clickroute-bichon-leftclean-postgrace1500-20260707.json`
> is `ok=true`, 4/4 ACKs at `490/164/33/5ms`, no interaction pollution, and
> Bevy WebGL2 packed/no DOM fallback; temporal summary is
> `docs/generated/player-qa/movement-jitter/temporal-clickroute-postgrace1500-20260707.md`.

> Latest Gateway movement ACK/input-priority sync: 2026-07-06 closes the local
> Web click repro where chained `Walk -> Walk -> Run -> Walk/Left` could pause
> because a heavy shared-Zone world tick ran on the WebSocket task just after a
> `UserLocation` ACK. Shared in-process Zone now drains `TickPlayerMovement`
> before heavy ticks, yields heavy world ticks while player movement is pending,
> and keeps a 1.2s post-ACK Crystal input window so follow-up Walk/Run packets
> are read before background work; Gateway movement input still wakes at 75ms.
> Verification passed Rust fmt/check, focused Gateway/simulation tests, Gateway
> build, raw packetRun probing, and full Web click capture
> `docs/generated/player-qa/startgame-debug-20260706-213036/current-web-jitter-r2-gateway-postackgrace1200-click.json`
> with `ok=true`, Run ACK about 205ms, no rollback, no residual pending plan,
> and Bevy WebGL2 packed rendering. Next task: leave PR #123 unmerged for now;
> port only its solid uncovered-map Bevy slice later on a clean branch/worktree.

> Latest entity-atlas resource hardening: 2026-06-02 adds
> `/bevy-entity-atlases/` to the remote asset release roots, asset manifest
> static prefixes, service-worker static/remote-cache handling, release doctor,
> and production original-asset smoke. The Bichon spawn critical pack now
> prewarms both the prebuilt entity-atlas manifest and PNG, while scene
> readiness uses entity `preloadPaths` for DOM fallback walk/run/equipment
> frames without extra scatter-fetching when the GPU atlas is already ready.
> Verification passed script syntax checks, Web `tsc --noEmit`,
> `preflight:asset-release`, `test:resource-loading`, `git diff --check`, and
> production web-origin asset smoke. Current production CDN still returns 404
> for the entity-atlas files until the next remote asset release is rebuilt and
> uploaded to R2.

> Latest Crystal map/minimap/resource parity closeout: 2026-05-27 prevents
> scene-blueprint reloads from clearing existing mini/big map indices when a
> partial blueprint has `null`, resolves Bichon map `0` mini/big map index
> `101` from Crystal minimap transform metadata instead of depending on the
> respawn manifest, normalizes minimap map names by basename/lowercase/.map
> stripping, and makes object drawMode map frames honor exported Crystal
> offset metadata for every object frame. Legacy Bichon torch offset fallback
> remains only for old starter JSON without offsets. Scene asset readiness keys
> now use a stable visible-asset URL hash instead of raw player x/y, avoiding
> per-step preload churn when the visible asset set is unchanged. Verification
> passed `MIR2_CANDIDATE_SCOPE=local bash infra/check-candidate-gate.sh`,
> including Web typecheck, movement-controller, minimap-transform,
> resource-loading, focused Rust gateway/simulation/admin gates, and
> `git diff --check`.

> Latest Crystal resource loading hardening: 2026-05-27 aligns Player Web's
> Crystal map/library path with Crystal MLibrary behavior. `.Lib` parsing is
> now index-only, frame RGBA decode happens lazily per requested frame behind a
> decoded-frame LRU byte budget, server map/library caches are capped, and
> production request-time original-map writes/synthetic fallbacks are disabled
> unless explicitly opted in. Scene blueprint cache keys are quantized by
> map/chunk/size bucket/schema version with disk TTL/size trim, scene readiness
> now preloads visible asset URLs and reports interaction-ready separately from
> visual-ready, and cache metrics expose scene cache keys, sprite/cell counts,
> DOM image counts, sprite library counts, Bevy atlas bytes, and alpha-keyed
> blob counts. Verification passed `MIR2_CANDIDATE_SCOPE=local bash
> infra/check-candidate-gate.sh`, including Web typecheck,
> `test:movement-controller`, `test:resource-loading`, focused Rust gateway /
> simulation / admin gates, and `git diff --check`. Production browser deploy
> acceptance is still the next step.

> Latest minimap raster transform sync: 2026-05-27 adds per-map Crystal MMap
> world-to-image transforms for Player Web instead of assuming `asset/mapWidth`
> linear scaling. Bichon map `0` now uses MMap `101` as a 1052x700 isometric
> projection, MiniMap and BigMap both render entity/NPC/debug points through
> the shared transform, `?mapDebug=1` reports map/asset/player image coords,
> and BigMap no longer falls back from `bigMapIndex` to `miniMapIndex`.
> Verification passed Web typecheck and `pnpm --dir apps/web run
> test:minimap-transform`. Production/browser visual acceptance is the next step
> before marking the map-panel parity slice deployed.

> Latest Crystal movement authority deployment: 2026-05-27 puts the server-
> authority Web movement convergence on production. UCloud Gateway release
> `20260527T0020CST-crystal-movement-authority` is installed at
> `/opt/mir2/gateway/current`; Web deployment
> `dpl_5rwcVtQcNBnZy5XiXvaS4axpPJSD` is live. Verification passed public
> Gateway/Web health, WSS smoke
> `docs/generated/load/remote-crystal-movement-authority-wss-smoke-20260527.json`,
> and headed Chrome production WebGL2 movement evidence
> `docs/generated/player-qa/movement-jitter/prod-crystal-movement-authority-walk-run-reverse-webgl2-skiptransfer3-20260527.json`.
> The production capture sent only direction packets:
> `walk Right -> run Right -> run Right -> walk Left -> run Left -> run Right`,
> with no `moveTo`, send intervals `724/718/722/742/736ms`, ACK latencies
> `449/57/131/55/38/38ms`, raw WebGL2 atlas `renderedLayers=17`, final player
> `343,270 Right`, no pending plan/prediction, no visual jumps, no logical
> rollback, no stale prediction warnings, no command-queue warnings, no
> critical console errors, and no non-favicon 404s. The movement QA harness now
> treats `sceneAssetReadiness.ready` as a valid ready signal when the legacy
> `state.sceneInteractionReady` field is absent.

> Latest movement input-buffer sync: 2026-05-26 closes the reproduced
> `walk -> run -> reverse` rollback/drift path on production. Frontend
> keyboard input now keeps a one-action reverse backlog, preserves Shift/run
> edges, upgrades same-direction queued Walk to Run, and the movement QA
> harness now asserts that declared keyboard sequences really emit the matching
> WebSocket `walk/run` frames. Shared Zone movement now timestamps Walk/Run/Turn
> intents, consumes a ready pending action before replacing it, keeps a bounded
> current+follow-up queue, accepts Run if the intent arrived inside the Crystal
> run grace even when the Zone tick consumes late, and buffers near-ready
> follow-up input for Crystal cadence instead of dropping it. Gateway release
> `20260526T1918CST-move-input-buffer` is installed on UCloud; Web deployment
> `dpl_HttHWiP21hufr1d3mm6fMsHNwcmW` is live. Verification passed Web
> typecheck, movement harness syntax, Rust fmt, focused shared-Zone run tests
> (7/7), Gateway runtime-tick and Zone-movement regressions, public health,
> WSS smoke `docs/generated/load/remote-move-input-buffer-wss-smoke-20260526.json`,
> and headed/production WebGL2 movement evidence
> `docs/generated/player-qa/movement-jitter/prod-move-input-buffer-walk-run-turn-webgl2-20260526b.json`
> plus the faster 180ms turn stress
> `docs/generated/player-qa/movement-jitter/prod-move-input-buffer-walk-run-turn-fast-webgl2-20260526a.json`.
> Both production captures sent ordered `walk Right -> run Right -> walk Left`,
> settled at `332,270 Left`, had no rollback/pending queues, rendered raw
> WebGL2 atlas layers, and had zero critical console errors/non-favicon 404s.

> Latest production movement/asset sync: 2026-05-26 closes the live WebGL2
> movement-delay repro after separating two issues. Current-scene original-map
> asset 404s for `Objects/2652..2661` and
> `Objects23/1418/1420/1423/1425/1429` are now uploaded to the active R2 prefix,
> and immutable asset failures are negative-cached so a true missing source file
> does not spin `mir2ImgRetry` forever. The remaining movement delay was the
> Gateway runtime tick scheduler: movement input had an inherited 1200ms defer,
> so the second queued Walk could sit until the next delayed runtime tick.
> Gateway release `20260526T1435CST-move-tick-grace0` is installed on UCloud,
> focused tick tests passed locally and remotely, WSS smoke passed, and headed
> Chrome production WebGL2 evidence shows two Walk ACKs at `398ms` and `609ms`
> with no critical console errors and no non-favicon 404s. Remaining frontend
> asset follow-up: investigate the isolated `Objects/289.png` mapping/source
> gap; it is now contained by negative caching rather than a retry storm.

> Latest ZoneOwner runtime handoff/takeover sync: 2026-05-26 adds the first
> tested owner-host state transfer path. `HostedZoneOwnerCommandClient` can now
> export its owned `ZoneRuntimeHandle` exactly once via
> `take_runtime_for_handoff`, old owner reads/commands fail with an
> already-handed-off error, and a next hosted owner can resume that runtime
> under a new lease/fencing token. Focused coverage proves the active Scout
> session survives the handoff, the next owner continues ticking the same
> runtime, and stale pre-handoff leases are rejected at the new owner boundary.
> Remaining distributed work: serialize/persist Zone state and replace this
> in-process runtime move with real process/network owner takeover.

> Latest ZoneOwner RPC transport seam sync: 2026-05-26 moves the hosted owner
> boundary behind an explicit RPC-facing abstraction. Added
> `ZoneOwnerRpcTransport` plus `RpcZoneOwnerCommandClient`; Gateway can now
> dispatch commands, snapshots, identity reads, saves, and mail refresh through
> a transport object that owns the Zone runtime instead of relying on the
> Gateway caller's local runtime. The current hosted owner implements that
> transport as a loopback owner host, preserving owner-side fencing checks and
> giving the future network transport the same command/view surface. Focused
> regressions prove the RPC client mutates the transport owner runtime rather
> than the Gateway runtime, and that stale pre-handoff leases are rejected at
> the transport owner boundary. Remaining distributed work: replace the
> loopback transport with a real process/network transport and implement
> durable Zone state handoff/takeover.

> Latest SkillItemConsume request-id sync: 2026-05-26 closes the immediate
> cast-id gap in the shared Account/Inventory command boundary. Zone-routed
> item-consuming casts now attach a monotonic per-session `request_id` to
> `SharedAccountInventoryCommand::SkillItemConsume`, and the default
> Account/Inventory service includes account, character, spell, and request id
> in the committed-receipt key. This makes a retried delivery of the same
> Zone skill-item command return the original receipt instead of being
> classified as an unrelated cast, while later legitimate casts receive fresh
> request ids. Focused coverage proves the key differentiates request ids and
> spells, and the PoisonCloud/SummonSkeleton Gateway route regressions prove
> the cast path emits request id `1` before dispatching accepted Zone magic.
> Remaining economy work: move the receipt store to a durable external
> Account/Inventory actor and bind command retries to ZoneOwner RPC/fencing.

> Latest ZoneOwner hosted-runtime boundary sync: 2026-05-26 moves the
> command-client seam closer to a real Gateway -> ZoneOwner process split.
> Added `HostedZoneOwnerCommandClient`, which owns a `ZoneRuntimeHandle`
> internally and executes fenced `ZoneOwnerCommandRequest`s inside that owner
> host instead of mutating the Gateway caller's local runtime. The hosted
> client also validates the shared `ZoneOwnerLeaseAuthority` at the owner
> boundary, so a request holding a pre-handoff lease is rejected even if it
> reaches the owner host. `GatewaySession` now also asks the command client for
> `world_snapshot`, `active_identity`, save, and external-mail refresh, so the
> owner host is the read/write surface rather than only the command mutation
> target. Evidence passed focused hosted-owner regressions for owner-runtime
> execution, owner-backed Gateway reads, and stale-request rejection after
> handoff. The newer RPC transport seam above moves this from direct hosted
> client usage to a replaceable loopback transport. Remaining distributed work:
> replace the loopback transport with process/network RPC and implement durable
> handoff/takeover of hosted Zone state.

> Latest Account/Inventory idempotency sync: 2026-05-26 advances the durable
> economy command boundary for shared Zone rewards. The default
> `InProcessAccountInventoryService` now keeps committed receipt keys for
> shared `MonsterKillAward` and `GroundDropPickup` commands, keyed by
> account/character plus the Zone reward object, so retrying the same Zone
> award or pickup returns the original receipt without mutating experience or
> gold a second time. `SkillItemConsume` is now covered by the newer
> request-id sync above, which keeps repeated delivery distinct from legitimate
> later recasts. Evidence passed the new idempotent reward
> regression, the full `in_process_account_inventory_service_` group, and the
> shared Account/Inventory service-boundary regression. Remaining economy work:
> move this semantics to a durable external actor and connect rollback/fencing
> to ZoneOwner RPC.

> Latest NPC world-service atomic outcome sync: 2026-05-26 advances the
> process-external NPC/quest side-effect track. Shared NPC script execution now
> publishes saved values, the shared NPC random seed, and entity side-effect
> packets through one `SharedNpcWorldCommand::ApplyScriptOutcome` envelope
> instead of three separate commits. Gateway only merges the saved/random state
> and forwards entity mutation packets after the world service returns a
> committed receipt with the expected side-effect payload, so a rejected NPC
> service leaves the shared Zone state unchanged instead of half-applying quest
> flags or entity removals. Evidence passed the new atomic NPC outcome
> regression plus the existing NPC world-service boundary and shared
> saved/random sync regressions. Remaining NPC work: move the service behind a
> durable process/RPC boundary and broaden this from script outcome envelopes to
> full quest/economy/account side-effect authority.

> Latest Zone-native CharmedSnake status sync: 2026-05-26 closes the remaining
> first-order `CharmedSnake` hit side effect from Crystal. Native
> `CharmedSnake` delayed melee damage now attempts the Crystal post-hit
> paralysis poison path after a successful target hit, using deterministic
> Zone authority instead of personal-session random state, and publishes
> `ObjectPoisoned` with the paralysis bit on the damaged Zone monster. The
> poison uses the Crystal `10 - PetLevel` chance and `4 + PetLevel` duration
> shape and feeds the existing native monster control timer. Evidence passed
> the new CharmedSnake paralysis regression, `zone_native_snake_totem_` (2/2),
> `zone_native_archer_` (4/4), focused `zone_native_player_` (30/30), and the
> self-Buff Gateway regression. Remaining monster AI work is now broader AI
> family coverage rather than this summon status effect.

> Latest Zone self-Buff state sync: 2026-05-26 starts closing the durable
> skill-state boundary behind the verified Zone-native spell work. Gateway now
> mirrors Zone-owned self `AddBuff` / `RemoveBuff` packets back into the
> personal `SimulationSession` `BuffResource` while still treating Zone as the
> authority and forwarding the original packets to the client. This covers
> pending/off-thread Zone packets as well as immediate Zone command results, so
> `world_snapshot.active_buffs` no longer lags behind accepted shared-Zone Buff
> state such as MagicShield. Evidence passed the new Gateway pending
> self-Buff mirror regression, the existing shared-Zone Magic route regression,
> focused `zone_native_player_` (30/30), and fmt check. Remaining durable
> skill-state work: broaden this from visible Buff packet mirroring to full
> Zone-owned skill/Buff lifetime services and process-external persistence.

> Latest Zone-native SnakeTotem swarm sync: 2026-05-26 closes the remaining
> Archer summon-family swarm/expiry hardening. `SnakeTotem` now follows the
> Crystal `PetLevel + 1` active `CharmedSnake` cap, refreshes the swarm after a
> minion lifetime expiry, and self-destructs with `ObjectDied` when its master
> is missing or more than 15 tiles away. `SnakeTotem` death now kills owned
> `CharmedSnake` minions; `CharmedSnake` lifetime expiry or missing/far Totem
> now emits `ObjectDied` and applies its Crystal-style 3x3 death explosion
> through the Zone native monster-hit resolver while keeping player damage out
> of the summon path. Evidence passed
> `zone_native_snake_totem_caps_minions_and_respawns_after_minion_expiry`,
> `zone_native_snake_totem_self_destruct_kills_owned_minions`,
> `zone_native_archer_` (4/4), `zone_native_vampire_spider_` (2/2), focused
> `zone_native_player_` (30/30), the Gateway summon item-boundary regression,
> and fmt check. Remaining 100% Candidate work shifts to durable skill-state,
> process-external NPC/economy/account services, full monster AI coverage, and
> ZoneOwner handoff.

> Latest Zone-native VampireSpider summon sync: 2026-05-26 closes the
> remaining Crystal-specific `SummonVampire` pet behavior in shared Zone.
> `VampireSpider` melee hits now run the Crystal `MasterVampire` side effect
> from Zone authority: successful damage broadcasts `ObjectEffect` Bleeding
> (effect 18) on the target and heals the owning Archer through `PlayerHealed`
> / authoritative `ObjectHealth`. Expiring `VampireSpider` pets, or pets whose
> master is missing or more than 15 tiles away, now self-destruct with
> `ObjectDied`; the 3x3 explosion damages nearby hostile Zone monsters through
> the same native monster-hit path, preserving owner heal/effect behavior and
> avoiding player damage. Evidence passed
> `zone_native_vampire_spider_hit_bleeds_target_and_heals_owner`,
> `zone_native_vampire_spider_explodes_on_expiry_and_vampires_nearby_target`,
> `zone_native_archer_` (4/4), focused `zone_native_player_` (30/30), the
> Gateway summon item-boundary regression, and fmt check. Remaining
> Archer/summon work: full SnakeTotem swarm cap/expiry hardening, durable
> skill-state persistence, and process-external service boundaries.

> Latest Zone-native Archer summon profile sync: 2026-05-26 moves the first
> Archer summon family slice into shared-Zone authority instead of falling back
> to personal-session summon materialization. `SummonVampire`,
> `SummonToad`, `SummonSnakes`, and `Stonetrap` now have native Zone summon
> profiles with Crystal target-point/projectile-delay validation, retained
> friendly `ObjectMonster` packets, master binding, visible `extra`, active
> summon caps, lifetime expiry, and Gateway Zone-route recognition without the
> Taoist amulet item-consumption boundary. `SummonVampire` can spawn an owned
> `VampireSpider` beside a hostile target point and recast to recall the
> retained pet to a new target point; `SummonToad` spawns `SpittingToad` and
> uses a stationary twelve-tile `ObjectRangeAttack`; `SummonSnakes` now creates
> the retained static `SnakeTotem`, emits the totem attack, spawns an owned
> `CharmedSnake` minion with the totem as visible master, and lets that minion
> attack hostile Zone monsters for the owning player; `Stonetrap` creates a
> static owned `StoneTrap`, draws hostile native monster aggro as a decoy
> target, avoids player damage while the hostile monster attacks the trap, and
> removes it on expiry. Evidence passed
> `zone_native_archer_` (4/4),
> `zone_native_stonetrap_draws_hostile_monster_aggro_without_player_damage`,
> the focused `zone_native_player_` suite (30/30),
> `zone_native_holy_deva_uses_ranged_summon_attack_against_hostile_monster`,
> `zone_native_pet_enhancer_buffs_owned_summon_and_increases_damage`,
> focused `zone_native_summon`, the Gateway summon item-boundary regression,
> and fmt check. Remaining Archer/summon work: full SnakeTotem swarm cap/expiry
> hardening, VampireSpider self-destruct / vampire-heal details, and durable
> skill-state persistence.

> Latest Zone-native summon/PetEnhancer sync: 2026-05-25 adds ranged
> summon-vs-monster combat and real pet Buff stats on top of summon ownership
> and recall.
> `SummonSkeleton` /
> `SummonShinsu` / `SummonHolyDeva` remain targetless Zone magic behind the
> Account/Inventory `SkillItemConsume` boundary; the verified `SummonSkeleton`
> path now both
> schedules the initial 500ms `BoneFamiliar` spawn and, on recast while the
> owned summon is active, recalls that retained summon to the Zone player's
> authoritative position with an `ObjectWalk` update instead of queuing another
> spawn. Gateway asks Zone whether the recast is a recall before committing
> skill items, so recall does not emit a second `DeleteItem` / item-consumption
> transaction. The owned `BoneFamiliar` now searches hostile native monsters,
> emits a summon `ObjectAttack`, applies delayed Zone-owned damage through the
> existing monster-hit path, and does not target/damage players. Kills from
> summon damage keep drop ownership and awards on the master object/session.
> `SummonShinsu` now uses the same one-amulet Zone item boundary, 500ms delayed
> retained `Shinsu` spawn, master binding, and hostile-monster melee path.
> `SummonHolyDeva` now waits its 1.5s Crystal summon delay, spawns retained
> `HolyDeva`, and uses AI-38-style six-tile `ObjectRangeAttack` against hostile
> native monsters with 500ms delayed DC damage while still avoiding player
> targets. `PetEnhancer` now validates an owned Zone summon target, emits the
> visible Crystal buff type 22 with DC/AC stats, retains that Buff on the
> summon object, expires it through Zone, and applies the DC stat to subsequent
> summon damage.
> Evidence passed
> `zone_native_player_summon_skeleton_spawns_owned_friendly_summon_after_delay`,
> `zone_native_player_summon_skeleton_recalls_existing_owned_summon_without_respawn`,
> `zone_native_summon_attacks_hostile_monster_for_owner_without_hitting_players`,
> `zone_native_holy_deva_uses_ranged_summon_attack_against_hostile_monster`,
> `zone_native_summon_shinsu_spawns_owned_pet_and_attacks_hostile_monster`,
> `zone_native_pet_enhancer_buffs_owned_summon_and_increases_damage`,
> the focused `zone_native_player_` group (30/30),
> `shared_in_process_runtime_routes_summon_magic_through_zone_item_boundary`,
> the existing item precheck regression, and fmt check. Remaining summon work:
> HolyDeva kiting polish, archer summon families, and durable skill-state
> persistence.

> Latest Zone-native area healing sync: 2026-05-25 extended the native
> self/friendly recovery path beyond starter Healing. `MassHealing` now
> validates a near self-target point, finds wounded Zone players within the
> native recovery radius, and schedules delayed Zone-owned heals for each
> target; `HealingCircle` validates the same near target, schedules the same
> multi-player delayed recovery, and emits the delayed Crystal `ObjectSpell`
> circle from Zone state. Gateway self-target magic preparation recognizes both
> spells. Evidence passed
> `zone_native_player_mass_healing_schedules_area_zone_heal`,
> `zone_native_player_healing_circle_spawns_spell_and_heals_in_zone`, the full
> focused `zone_native_player_` shared-Zone group (28/28), existing Gateway
> Magic route coverage, and fmt check. Remaining friendly-skill work: party /
> group membership filtering, summons, and durable skill-state persistence.

> Latest Zone-native Healing self-route sync: 2026-05-25 moved the starter
> self-Healing path into shared Zone authority. `Healing` can now target the
> Zone player (`target_id` self or zero), validate action window, MP/cooldown,
> self position, and missing HP, spend inside Zone, emit owner `Magic`,
> observer `ObjectMagic`, and the Crystal healing `ObjectEffect`, then apply a
> delayed Zone-owned heal through `ObjectHealth` plus `PlayerHealed` so Gateway
> synchronizes personal-session HP from the Zone result. Gateway preparation
> recognizes self-target Healing alongside MagicShield. Evidence passed
> `zone_native_player_healing_self_schedules_zone_heal`, the full focused
> `zone_native_player_` group (26/26), existing Gateway Magic route coverage,
> fmt check, and scoped diff check. Remaining friendly-skill work: other
> healing/friendly target surfaces, group/area healing, summons, and durable
> skill-state persistence.

> Latest Zone-native MagicShield Buff sync: 2026-05-25 moved the first
> self-target defensive Wizard Buff into shared Zone authority. `MagicShield`
> with `target_id=0` is now a native Zone self-magic path: Zone validates the
> player's action window, MP, cooldown, target position, and existing Buff,
> spends MP/cooldown, emits owner `Magic`, observer `ObjectMagic`, visible
> `AddBuff`, and the Crystal shield-up `ObjectEffect`, stores the Buff on
> `ZonePlayer` for late AOI joins, and applies
> `CRYSTAL_STAT_DAMAGE_REDUCTION_PERCENT` during Zone-native monster hits.
> Gateway preparation now recognizes self-target Zone magic so learned
> MagicShield can route through the same command surface. Evidence passed
> `zone_native_player_magic_shield_adds_zone_buff_and_mitigates_hits`, the
> full focused `zone_native_player_` shared-Zone group (25/25), existing
> Gateway Magic route coverage, and Rust fmt check. Remaining Buff work:
> broader self/friendly Buff families, durable skill-state persistence, and
> process-external service boundaries.

> Latest production movement/input closeout: 2026-05-25 deployed the
> starter-transfer cleanup Gateway release and the Player Web scene-input
> unlock fix to production. Gateway release
> `20260525T0334CST-starter-transfer-cleanup` is active on the UCloud host;
> local, public `sslip.io`, and `mir2.obelisk.build` health checks passed, and
> WSS smoke `docs/generated/load/remote-starter-transfer-cleanup-wss-smoke-20260525.json`
> passed with 1/1 ready and 0 errors. Headed production WebGPU packet-walk
> evidence crossed `0:339,270` with ACKs `339..343`, no `MapInformation`, no
> `339 -> 330` rollback, no console errors, packed prebuilt Bevy atlas, and
> WebGPU selected. Player Web deployment `dpl_7iG3bPgA7HTxkvEzN4LxP2rmFmFC`
> then removed the movement-input stall by keeping scene interaction unlocked
> after the first playable scene while later viewport asset preloads run in the
> background. Headed Chrome evidence
> `docs/generated/player-qa/movement-jitter/prod-scene-input-unlocked2-webgpu-headed-keyboard-a-nosample-hold-20260525.json`
> passed with held `A` sending five Walk packets at Crystal cadence, ACKs
> `343,342,341,340,339`, `sceneInteractionReady=true` while 699 assets were
> still background-loading, WebGPU plus packed atlas active, no critical console
> errors, and no non-favicon 404s. Remaining 100% Candidate work shifts back to
> Zone-native skill/Buff/NPC/economy/ZoneOwner completeness and long 30-active
> gameplay acceptance.

> Latest Crystal runtime starter-transfer cleanup: 2026-05-25 removed the
> early demo `starter-east-field-gate` transfer from production/Crystal map
> runtime config. The default starter scenario still keeps that explicit
> same-map gate, but `with_crystal_map_runtime()` now uses generated Crystal
> movement records only, so walking right from `0:338,270` no longer triggers
> the fake `339..341,268..271 -> 330,270` teleport that production headed
> movement capture flagged as a logical rollback. Evidence passed
> `crystal_map_runtime_drops_starter_demo_transfer`,
> `shared_in_process_crystal_runtime_does_not_apply_starter_demo_gate_transfer`,
> the adjacent real Crystal movement-transfer regression, fmt check, and scoped
> diff check. Deployed verification is recorded in the production
> movement/input closeout above.

> Latest Bevy atlas direct-image sync: 2026-05-25 removed the remaining
> prebuilt entity-atlas PNG readback from the hot frontend path. Prebuilt
> `/bevy-entity-atlases/*.png` records now flow to the Bevy runtime as
> `imageUrl`, and the runtime binds the AssetServer-loaded image directly to
> `TextureAtlasLayout`; the old RGBA pixel upload path remains only for live or
> explicit pixel atlases. Evidence passed Web typecheck, WebGPU/WebGL2 wasm
> checks, release runtime rebuild `bevy-b9389323fd0dbead`, production `next
> build`, and headed Chrome local WebGPU play against the live Gateway. Chrome
> observed `pkg-webgpu` plus `starter-bichon-base.png`, and movement
> diagnostics
> `docs/generated/player-qa/movement-diagnostics/manual-mplj7xmo-rpw2ln.jsonl`
> recorded 4 movement commands, 4 `UserLocation` ACKs, 367-443ms ACK latency,
> and 0 anomalies. Remaining renderer work: deploy this web bundle and rerun
> production headed Chrome/WebGPU acceptance on `https://mir2.obelisk.build`.

> Latest PoisonCloud live item-route sync: 2026-05-25 enabled the
> item-consuming targetless PoisonCloud Gateway path behind a Zone precheck.
> Gateway now recognizes PoisonCloud as a targetless ground spell, asks the
> shared Zone whether the cast would be accepted before submitting
> `SkillItemConsume`, then dispatches only after the Account/Inventory service
> commits the amulet + green-poison cost. Evidence passed
> `shared_in_process_runtime_prechecks_item_skill_before_consuming_items`, the
> account/inventory boundary regression, and focused PoisonCloud/targetless
> Zone regressions. Remaining work: back the service with a durable external
> account/inventory actor and add production RPC fencing around the same
> precheck/commit/dispatch sequence.

> Latest Zone-native ExplosiveTrap sync: 2026-05-25 completed the next
> Trap-family ground action in shared Zone authority. Native ExplosiveTrap now
> uses the caster direction to spawn the front-row trap objects, ticks contact
> damage from Zone-owned ground-spell state, and removes the trap after the
> first detonation. Gateway targetless ground-magic routing now recognizes
> ExplosiveTrap alongside non-item ground spells. Evidence passed
> `zone_native_player_explosive_trap_spawns_front_row_and_detonates_once` and
> the focused `zone_native_player` group. Remaining profession-control work:
> broader bespoke controls, summons, and durable skill-state persistence.

> Latest Zone-native TrapHexagon sync: 2026-05-25 extended Trap-family
> control authority beyond single-target Trap. Native TrapHexagon now roots
> hostile Zone monsters in the target area, queues the delayed Crystal-style
> eight-object ring `ObjectSpell`, and prevents rooted monsters from walking
> during the control window. Evidence passed
> `zone_native_player_trap_hexagon_roots_area_and_spawns_ring_objects`.
> Remaining profession-control work: broader bespoke controls, summons, and
> durable skill-state persistence.

> Latest Skill item-consumption boundary sync: 2026-05-25 added a
> `SkillItemConsume` command to the shared Account/Inventory envelope for
> item-consuming skills. The default in-process service now handles
> PoisonCloud's amulet + green-poison consumption as a transaction receipt,
> giving Gateway a service boundary for item costs instead of calling personal
> inventory helpers inline. Evidence passed
> `in_process_account_inventory_service_handles_skill_item_consumption_command`
> and the existing account/inventory boundary regression. Follow-up: replace
> the default in-process service with a durable account/inventory actor.

> Latest targetless ground-magic route sync: 2026-05-25 widened the shared
> Zone command surface beyond object-target magic. `PlayerCastMagic` with
> `target_id=0` can now execute ground-target FireWall/Blizzard/MeteorStrike/
> PoisonCloud casts directly in `ZoneRuntime`, emitting owner `Magic`,
> observer `ObjectMagic`, and delayed ground-spell objects without requiring a
> monster object target. Gateway's shared attack preparation now recognizes
> learned targetless ground Magic packets with `target_id=0`, including the
> item-consuming PoisonCloud path after Zone precheck, and dispatches them to
> Zone without fabricating a monster spawn. Evidence passed
> `zone_native_player_firewall_accepts_targetless_ground_cast`, the focused
> `zone_native_player` group, and locked Gateway+Simulation check.

> Latest Zone-native Trap sync: 2026-05-25 added the first Trap-style
> ground/control action to shared Zone authority. `ZoneNativeMonster` now
> retains Crystal monster level from spawns, allowing Zone to enforce Trap's
> lower-level target gate. Native Trap now roots eligible hostile Zone monsters
> for the control window and queues the delayed Trap `ObjectSpell` with the
> Crystal direction/param surface. Evidence passed
> `zone_native_player_trap_spawns_object_and_roots_lower_level_monster` and the
> focused `zone_native_player` group. Next shared MMO authority tasks: extend
> this root/object-spell pattern to broader profession bespoke controls,
> summons, and durable skill-state persistence.

> Latest Zone-native PoisonCloud sync: 2026-05-25 extended the shared-Zone
> ground-spell scheduler to poison cloud monster effects. Native PoisonCloud
> now queues the delayed visible cloud `ObjectSpell`, ticks 3x3 occupied-cell
> damage in `ZoneRuntime`, and applies/broadcasts green `ObjectPoisoned` state
> on affected Zone monsters. Evidence passed
> `zone_native_player_poison_cloud_spawns_ground_spell_and_poisons_monsters`
> and the focused `zone_native_player` group. Remaining work: replace the
> in-process Account/Inventory command boundary with a durable actor-backed
> transaction service.

> Latest Zone-native chain/splash sync: 2026-05-25 moved two more Wizard
> secondary-damage branches into shared Zone authority. Native MeteorShower
> now selects up to three nearby hostile secondary Zone monsters, publishes
> their ids in `Magic`/`ObjectMagic`, and applies half-damage secondary hits;
> native FireBounce now schedules chained `ObjectProjectile` hops and delayed
> damage between Zone monsters. Evidence passed
> `zone_native_player_meteor_shower_damages_primary_and_secondary_monsters`,
> `zone_native_player_fire_bounce_chains_projectiles_and_damage`, and the
> focused `zone_native_player` group. Next shared MMO authority tasks: extend
> the same approach to remaining Trap-family actions, profession bespoke
> skills, summons, and durable skill-state persistence.

> Latest Zone-native ground-spell sync: 2026-05-25 moved the first persistent
> ground spells into shared Zone authority. Native FireWall now queues the
> Crystal-style delayed five-cell `ObjectSpell` cross, while
> Blizzard/MeteorStrike queue delayed 5x5 `ObjectSpell` cells with center-cell
> marker semantics; both tick damage from Zone-owned ground-spell state instead
> of applying immediate personal-session hits. Evidence passed
> `zone_native_player_firewall_spawns_ground_spell_and_ticks_damage`,
> `zone_native_player_blizzard_family_spawns_ground_spell_and_ticks_damage`,
> and the focused `zone_native_player` group. Next shared MMO authority tasks:
> extend the scheduler to remaining Trap-style ground actions and remaining
> profession-specific persistent effects.

> Latest Zone-native area magic sync: 2026-05-25 added the first shared-Zone
> multi-target spell damage slice. Native `PlayerCastMagic` now computes
> secondary target ids for 3x3 target spells such as `FireBang`/`IceStorm`,
> includes them in `Magic` and `ObjectMagic`, and schedules authoritative Zone
> damage against those secondary native monsters instead of damaging only the
> primary target. Evidence passed
> `zone_native_player_area_magic_damages_secondary_monsters_authoritatively`
> and the focused `zone_native_player` group. Next shared MMO authority tasks:
> expand this target collector to Blizzard/MeteorStrike ground spells,
> MeteorShower/FireBounce chains, and skill-specific secondary damage formulas.

> Latest Zone-native special arrow Buff sync: 2026-05-25 moved the first
> Archer special-arrow Buff side effect into shared Zone authority. Native
> `PoisonShot` can now add the visible Crystal arrow marker Buff to the Zone
> player state, late AOI joins see that Buff on `ObjectPlayer` plus `AddBuff`,
> and native `CrippleShot` consumes the Zone-held PoisonShot Buff before
> spreading green poison to nearby Zone monsters. Native `VampireShot` now also
> schedules Zone-owned player healing, emits authoritative player health, and
> returns a `PlayerHealed` outbound that Gateway applies back to the personal
> runtime. Evidence passed the new PoisonShot Buff, CrippleShot spread,
> VampireShot heal, CrippleShot vampire follow-up, Gateway pending-heal, and
> focused `zone_native_player` regressions. Next shared MMO authority tasks:
> continue moving broader spell/Buff stats, summons, and AoE/ground skills fully
> into Zone.

> Latest Gateway Magic route sync: 2026-05-25 added shared-runtime coverage
> proving client `Magic` packets can route through shared Zone authority and
> broadcast `ObjectMagic` to another observer instead of staying inside the
> personal `SimulationSession`. The focused Gateway route group now covers both
> RangeAttack and Magic practical paths. Remaining task: widen this from seeded
> skill launch coverage into full Zone-owned skill effects, Buff state, and
> projectile/damage variants for all Crystal spells.

> Latest Zone-native poison tick sync: 2026-05-25 moved the first
> player-applied green-poison damage loop into shared Zone authority.
> `PoisonShot` cast through `ZoneCommand::PlayerCastMagic` now records
> monster poison state in `ZoneNativeMonster`, publishes `ObjectPoisoned`,
> ticks Crystal-style green damage every 2 seconds, updates monster health,
> and can kill through the same Zone-native drop plus `MonsterKillAward`
> path instead of falling back to personal-session poison ticking. Evidence
> passed
> `zone_native_player_poison_shot_ticks_green_damage_and_awards_kill`, the
> focused `zone_native_player` group, Simulation fmt check, and locked
> Simulation check. Next shared MMO authority tasks: widen poison/status
> effects beyond PoisonShot, add more Boss/area AI branches, and replace the
> remaining in-process Account/Inventory, NPC world-service, and ZoneOwner
> command adapters with durable process boundaries.

> Latest ZoneOwner heartbeat sync: 2026-05-25 wired the owner TTL renewal
> groundwork into Gateway session scheduling. `ZoneOwnerLeaseAuthority` now has
> a time-aware renewal method for deterministic owner-heartbeat tests, and Web
> sessions configure a ZoneOwner heartbeat that runs on the runtime tick before
> deferred world ticks. Current owners renew before TTL expiry, missed
> heartbeats fail with a stale-owner fencing error, and the existing
> command-client/owner-boundary regressions still pass. Next shared MMO
> authority tasks: replace the in-process command client with real Gateway ->
> ZoneOwner RPC transport and process handoff, then continue native NPC,
> economy, skill/Buff, and monster AI authority.

> Latest Zone-native player action-window sync: 2026-05-25 moved the remaining
> melee/range/magic packet action timing into shared Zone player authority.
> `ZonePlayer` now owns attack and spell ready timestamps; native melee/range
> launches respect the Crystal attack window, while magic launches respect a
> cross-spell action window in addition to per-spell cooldown and MP checks.
> Early commands return owner correction packets and do not broadcast attack,
> magic, mana, projectile, or delayed-hit packets. Verification passed focused
> range and magic action-window regressions, the `zone_native_player`
> shared-Zone group, and Gateway shared-runtime coverage proving a second early
> RangeAttack cannot rebroadcast through the practical route. Next shared MMO authority tasks: continue widening
> Zone-native skill/Buff effects, poison damage ticks, Boss AI branches, and
> real Account/Inventory/NPC/ZoneOwner process boundaries.

> Latest NPC world-service command-envelope sync: 2026-05-25 moved the shared
> NPC saved-value/random-seed/entity-side-effect bridge behind an
> identity-bearing command boundary. `SharedInProcessZoneSessionRuntime` now submits
> `SharedNpcWorldCommandEnvelope` values carrying active account/character
> identity plus `SyncSavedValues`, `SyncRandomSeed`, or
> `ApplyEntitySideEffects { map_file_name, packets }` payloads through
> `SharedNpcWorldService` before mutating shared Zone NPC/map state. The default
> in-process service preserves current behavior, while focused coverage proves
> the command envelopes, identity, committed saved values, committed random seed,
> and committed NPC entity side-effect packet path. Next shared MMO authority
> tasks: replace diff-derived NPC entity packets with first-class NPC map/event
> commands, NPC service trades, quest rewards, and rollback-sensitive economy
> side effects.

> Latest Account/Inventory command-envelope sync: 2026-05-25 completed the
> next transaction-service boundary slice. Gateway reward commits now enter
> `SharedAccountInventoryService` as `SharedAccountInventoryCommandEnvelope`
> values carrying active account/character identity plus monster kill award or
> ground-drop pickup commands, rather than as separate hard-coded service
> methods. Focused coverage verifies both identity-bearing command shapes,
> service-generated reward packets, failed-pickup Zone rollback, and default
> service rejection when the envelope identity does not match the active
> runtime character. Next
> shared MMO authority tasks: back this command interface with a real
> actor/transaction store, then route NPC service trades, quest rewards, and
> broader economy mutations through the same command surface.

> Latest ZoneOwner command-client sync: 2026-05-25 completed the next
> distributed-Zone boundary slice. `GatewaySession` no longer directly
> chooses direct-vs-production runtime execution after lease validation; it
> submits the `ZoneOwnerCommandRequest` through a replaceable
> `ZoneOwnerCommandClient`. The in-process client is the default, but focused
> tests prove a valid production command crosses the client boundary and a
> stale owner lease is rejected before the client sees it. A renewal hook now
> lets the current owner lease renew through the authority and rejects old
> owners after handoff. The in-process owner client can also validate against
> the authority, so stale fenced requests are rejected at the owner boundary
> even if local Gateway validation is bypassed. The in-memory authority also has
> an optional TTL mode: current owners can renew before expiry, expired renewals
> fail, and the next owner read advances the fencing token for takeover. Next
> shared MMO authority tasks: replace the in-process client with a real Gateway
> -> ZoneOwner RPC transport, wire heartbeat scheduling to TTL renewal, and keep
> NPC/economy/monster authority moving behind Zone/world-service boundaries.

> Latest Zone-native monster status sync: 2026-05-25 completed the first
> special-monster AI status slice in shared Zone. Native monster hits now own
> Cave Maggot / Incarnated ZT paralysis and Toxic Ghoul-style green poison:
> Zone commits the player poison bitfield, sends `ObjectPoisoned`, blocks
> movement only while Zone-owned paralysis is active, and clears the status on
> expiry. Verification passed the new paralysis movement/expiry regression,
> the green-poison non-blocking regression, focused `zone_native_monster`
> shared-Zone coverage, and Simulation fmt check. Next shared MMO authority
> tasks: expand Zone-native monster AI to poison damage ticks and Boss/area
> status branches, then continue the real Account/Inventory actor, NPC
> world-service commands, and distributed ZoneOwner RPC/handoff.

> Latest Account/Inventory service-boundary sync: 2026-05-25 moved the
> Gateway reward commit path behind a replaceable
> `SharedAccountInventoryService`. `SharedInProcessZoneSessionRuntime` now
> submits Zone monster kill awards and shared ground-drop claims through that
> service instead of directly calling the personal `InProcessWorldRuntime`;
> the default service preserves the current session-backed behavior, while
> tests can inject an actor-style service that commits, rejects, or returns
> packets without mutating the personal session. Verification passed
> `shared_in_process_runtime_uses_account_inventory_service_boundary`, proving
> kill-award packets can come from the service and failed pickup commits still
> cancel/restore Zone claims. Next shared MMO authority tasks: replace the
> default in-process implementation with a real Account/Inventory actor or
> transactional service, then move NPC service trades and quest/economy
> commits onto the same boundary.

> Latest NPC entity side-effect sync: 2026-05-25 added an explicit shared
> entity diff around NPC command execution. Gateway now snapshots local
> monster entities before/after NPC commands, emits Crystal-backed
> `ObjectMonster` packets for newly generated monsters, and emits
> `ObjectHealth(0)` / `ObjectDied` / `ObjectRemove` packets when NPC script
> side effects clear or remove monsters. Shared entity observer routing now
> treats health/death/remove packets as shared-object updates, so `MONGEN`
> and `MONCLEAR` are no longer only silent personal-session ECS mutations.
> Verification passed
> `shared_npc_entity_side_effects_emit_spawn_packets_for_new_monsters`,
> `shared_npc_entity_side_effects_emit_death_packets_for_monclear`, and the
> adjacent NPC random/saved-value regressions. Next shared MMO authority
> tasks: convert these diff-derived packets into native Zone/world-service
> commands for map/event flags and NPC services, then continue the
> Account/Inventory actor and special monster AI slices.

> Latest NPC random shared-state sync: 2026-05-25 extended the NPC
> world-service bridge beyond `SAVEVALUE` slots. Crystal NPC `RANDOM` now
> uses a seed that can be read from and applied to `SimulationSession`, and
> Gateway shared Zone state applies that seed before NPC commands and
> publishes it afterward. This keeps shared NPC script random branches on one
> Zone-owned sequence instead of letting each personal session roll its own
> divergent branch. Verification passed
> `shared_in_process_registry_syncs_npc_random_seed_between_sessions` plus
> `cargo +1.89.0 fmt --check -p mir2-gateway -p mir2-simulation`. Next shared
> MMO authority tasks: move NPC `MONGEN` / `MONCLEAR` and event/map mutations
> from personal-session ECS side effects into shared Zone/world-service
> submissions, then continue Account/Inventory actor and special monster AI
> slices.

> Latest Zone-owner command fencing sync: 2026-05-25 completed the next
> distributed-Zone groundwork slice after owner metadata. `GatewaySession`
> now wraps player commands in `ZoneOwnerCommandRequest` envelopes carrying
> execution mode plus `ZoneOwnerLease`, validates the lease before dispatching
> to `ZoneRuntime`, and the production Web session action path now routes
> normal player commands through that fencing point using the session's
> current owner lease. A shared `ZoneOwnerLeaseAuthority` now owns the current
> owner token, and the in-memory authority can hand off a zone by incrementing
> the fencing token; old sessions are rejected even when they submit their own
> saved lease after that handoff. Focused regressions prove the current owner
> lease still executes, stale fencing tokens are rejected before runtime
> mutation/event publication, wrong owner ids are rejected before production
> command execution, and superseded owner leases are rejected after authority
> handoff. Next shared MMO authority tasks: replace the in-process authority
> with a real Gateway -> ZoneOwner RPC transport, add TTL renewal and
> takeover recovery, then continue NPC/quest world-service,
> Account/Inventory actor, and special monster AI slices.

> Latest Zone-owner fencing metadata sync: 2026-05-25 completed the next
> distributed-Zone architecture slice. Routed Gateway sessions now carry an
> explicit `ZoneOwnerLease` with `zoneOwnerId` and `fencingToken`, and the
> session cache/route records persist that owner metadata for online character
> routing. Verification passed registry route owner assertions, session-cache
> owner metadata regressions, admin session record coverage, fmt/diff checks,
> and locked Simulation/Gateway check. Next shared MMO authority tasks:
> replace the in-process owner with a Gateway -> ZoneOwner RPC boundary,
> validate fencing tokens on commands, add owner handoff/renewal semantics,
> and continue broader NPC/quest plus Account/Inventory world-service commits.

> Latest NPC saved-value shared-state sync: 2026-05-25 completed the first
> Zone/world-service slice for NPC script side effects. `SharedNpcSavedValue`
> now represents Crystal NPC `SAVEVALUE` / `LOADVALUE` state, and Gateway
> shared Zone state synchronizes those values across sessions before and after
> NPC commands. Verification passed the new cross-session saved-value
> regression, existing sparse shared-NPC interact and shared guide CallNpc
> quest regressions, the Account/Inventory receipt regression, fmt check,
> scoped diff check, and locked Simulation/Gateway check. Next shared MMO
> authority tasks: route broader NPC/quest side effects through world-service
> commits, replace personal-session reward storage behind Account/Inventory
> receipts, then continue special monster AI and cross-Gateway Zone-owner
> fencing/handoff.

> Latest Account/Inventory transaction-boundary sync: 2026-05-25 completed
> the next non-visual shared MMO architecture slice. Shared ground-drop pickup
> and Zone-native monster kill awards now both return a unified
> `SharedAccountInventoryTransactionReceipt` with explicit `kind`,
> `committed`, and visible packets. Gateway uses these receipts for reward
> commit decisions, preserving Zone claim rollback semantics while removing
> another direct ad-hoc reward call path. Verification passed the new Gateway
> receipt regression, existing kill-award and gold-claim rollback regressions,
> the Simulation commit receipt regression, fmt check, scoped diff check, and
> locked Simulation/Gateway check. Next shared MMO authority tasks: replace
> the in-process personal-session storage behind the receipt with an
> Account/Inventory actor or transaction service, then move NPC/quest shared
> side effects and special monster AI onto the same world-service boundary.

> Latest 30-active movement/chat acceptance sync: 2026-05-25 completed the
> production-feel task that had been holding the Gateway at a conservative
> 15-active policy. The current UCloud Gateway release
> `20260525T1348CST-route-refresh-background-task` is live at
> `60 ws / 30 active / 30 reconnect leases` and passed public 30-active
> movement-only plus move/chat pressure with no capacity rejects or client
> errors. The fix moved owned route-lease refresh into a background per-socket
> task, removed full personal-session snapshot reads from every movement by
> caching same-map transfer tiles, coalesced observer movement packets, folded
> movement intent plus player tick into one Zone lock, and made retained AOI
> visibility packet generation lazy. Verification artifacts:
> `docs/generated/load/public-route-refresh-background-task-30active-movementonly1m-settle30s-20260525.json`,
> `docs/generated/load/public-route-refresh-background-task-30active-movechat1m-chat30-settle30s-20260525.json`,
> and
> `docs/generated/load/public-route-refresh-background-task-30active-movechat1m-chat10-settle30s-20260525.json`.
> Next shared MMO authority tasks: Account/Inventory transaction service,
> NPC/quest Zone/world-service state, special monster AI, and cross-Gateway
> Zone owner fencing/handoff.

> Latest shared ground-drop commit receipt sync: 2026-05-25 completed the
> next transaction-boundary cleanup for shared ground-drop pickup. The
> character/economy commit path now returns
> `SharedGroundDropPickupCommit { committed, packets }`, and Gateway uses the
> receipt to drive Zone claim Commit/Cancel instead of inferring success from
> `GainedGold` / `GainedItem` packets. Verification passed the Simulation
> receipt regression, Gateway rollback and normal remote-pickup regressions,
> local and UCloud locked Simulation/Gateway checks, and production release
> `20260525T0843CST-grounddrop-commit-receipt` with WSS smoke plus the
> current 30-client safe-cap baseline. Next shared MMO authority tasks:
> replace the personal-session bridge behind this receipt with a real
> Account/Inventory transaction service, then continue NPC/quest world state,
> special monster AI, Zone owner fencing/handoff, and accepted 30-active
> gameplay feel.

> Latest shared kill-award commit sync: 2026-05-25 completed a reward-commit
> boundary cleanup for Zone-native monster death. Zone still owns kill/drop
> resolution and emits `MonsterKillAward`, but `GainExperience` now comes from
> the Gateway/personal character commit after the experience write is applied.
> Verification passed native Zone kill/drop, the Gateway commit regression,
> shared routing/fallback drop coverage, local and UCloud locked
> Simulation/Gateway checks, and production release
> `20260525T0827CST-zone-award-commit` with WSS smoke plus the current
> 30-client safe-cap baseline. Next shared MMO authority tasks: make the same
> commit model real for gold/items/quest side effects through an
> Account/Inventory transaction service, then continue NPC/quest world state,
> special monster AI, Zone owner fencing/handoff, and accepted 30-active
> gameplay feel.

> Latest shared fallback drop-template sync: 2026-05-25 completed the next
> drop/economy authority slice. Gateway fallback materialization from shared
> monster snapshots now fills `ZoneMonsterSpawn.drops` from Crystal/starter
> drop templates instead of leaving it empty, so sparse shared combat can still
> reach Zone-native death/drop resolution. Verification passed the new fallback
> drop-template regression, neutral AI fallback coverage, Simulation native
> kill/drop authority, shared `RangeAttack` routing, rollback claim coverage,
> local and UCloud locked Simulation/Gateway checks, and production release
> `20260525T0804CST-zone-fallback-drops` with WSS smoke plus the current
> 30-client safe-cap baseline. Next shared MMO authority tasks: Zone-owned
> drop generation across the full monster lifecycle, transactional
> Account/Inventory reward commit, NPC/quest side effects, special monster AI,
> cross-Gateway Zone owner fencing/handoff, and accepted 30-active gameplay
> feel.

> Latest shared drop/economy rollback sync: 2026-05-25 completed a focused
> guardrail for the current shared drop/economy bridge. The new Gateway
> regression forces a shared Zone gold claim to fail during personal economy
> commit via the gold cap, and verifies cancel/restore behavior all the way
> back into Zone/shared-map state plus owner `ObjectGold` respawn. Verification
> passed the rollback regression, adjacent normal shared-drop claim, remote
> shared-gold pickup, intelligent creature remote shared-gold pickup, locked
> Gateway check, and Gateway fmt check. Next shared MMO authority tasks:
> replace the bridge with Zone-owned drop generation plus transactional
> Account/Inventory commit, then continue NPC/quest side effects, special
> monster AI, cross-Gateway Zone owner fencing/handoff, and accepted 30-active
> gameplay feel.

> Latest Zone-native ranged monster AI sync: 2026-05-25 completed the first
> non-melee native monster AI authority slice. Zone-native monsters now retain
> Crystal `ai`; ranged/magic-style AI such as `ai=19` attacks visible
> non-adjacent players with `ObjectRangeAttack` and delayed Zone-owned player
> damage instead of walking until adjacent. Verification passed
> `zone_native_ranged_monster_attacks_without_chasing_when_target_not_adjacent`,
> adjacent native melee and Buff authority regressions, Gateway shared routing
> coverage, local and UCloud locked Simulation/Gateway checks, and production
> release `20260525T0734CST-zone-monster-ranged` with WSS smoke plus the
> 30-client safe-cap baseline. Next shared MMO authority tasks: special
> ranged/magic/Boss AI branches, rate/status Buff effects, AoE/ground spell
> resolution, summon lifecycle, NPC/quest side effects, transactional
> drop/economy commit, cross-Gateway Zone owner fencing/handoff, and accepted
> 30-active gameplay feel.

> Latest Zone-owned defensive Buff sync: 2026-05-25 completed the first
> incoming-damage Buff stat authority slice. Zone-native monster delayed hits
> now subtract the target player's Zone-held `MAX_AC` Buff stat before
> committing Zone HP damage or emitting hit packets; after Zone Buff expiry,
> the same native monster hit commits normal damage again. Verification passed
> `zone_native_player_defence_buff_mitigates_monster_damage_until_expiry`,
> adjacent attack Buff/native monster hit regressions, Gateway shared routing
> coverage, local and UCloud locked Simulation/Gateway checks, and production
> release `20260525T0720CST-zone-buff-defence` with WSS smoke plus the
> 30-client safe-cap baseline. Next shared MMO authority tasks: rate/status
> Buff effects, AoE/ground spell resolution, summon lifecycle, broader monster
> ranged/magic and Boss AI, NPC/quest side effects, transactional drop/economy
> commit, cross-Gateway Zone owner fencing/handoff, and accepted 30-active
> gameplay feel.

> Latest Zone-owned Buff stat sync: 2026-05-25 completed the first stat Buff
> authority slice after Magic control. Zone-native melee/range/object-Magic
> damage profiles now enter Zone without personal-session Buff attack stats;
> `ZoneRuntime` applies retained player `AddBuff.stats` during native monster
> damage commit and removes the effect after Zone Buff expiry. Verification
> passed `zone_native_player_buff_stats_authoritatively_modify_damage_until_expiry`,
> existing Zone magic tests, Gateway shared routing coverage, local and UCloud
> locked Simulation/Gateway checks, and production release
> `20260525T0709CST-zone-buff-stats` with WSS smoke plus the 30-client
> safe-cap baseline. Next shared MMO authority tasks: defensive/rate/status
> Buff stats, AoE/ground spell resolution, summon lifecycle, broader monster
> ranged/magic and Boss AI, NPC/quest side effects, transactional drop/economy
> commit, cross-Gateway Zone owner fencing/handoff, and accepted 30-active
> gameplay feel.

> Latest Zone-native Magic control sync: 2026-05-25 completed the next
> shared-combat authority slice after object Magic MP/cooldown. Zone now owns
> control expiry for targeted ElectricShock, Entrapment, and CatTongue on
> native monsters; controlled monsters do not walk or attack until the
> Zone-owned expiry, Entrapment/CatTongue fan out Crystal control packets, and
> poison clears when control expires. ElectricShock/Entrapment no longer report
> fake damage in the Zone magic profile. Verification passed the new
> `zone_native_player_magic_control_stops_monster_ai_until_expiry` regression,
> existing Zone magic/monster tick tests, Gateway shared routing coverage, and
> locked Simulation/Gateway check locally and on UCloud. Gateway release
> `20260525T0651CST-zone-magic-control` is live; public health, WSS smoke
> `docs/generated/load/remote-zone-magic-control-wss-smoke-20260525.json`, and
> 30-client safe-cap baseline
> `docs/generated/load/remote-zone-magic-control-30active-timeout60-20260525.json`
> passed. Next shared MMO authority tasks: Zone-owned stat Buff application,
> AoE/ground spell resolution, summon lifecycle, broader monster ranged/magic
> and Boss AI, NPC/quest side effects, transactional drop/economy commit,
> cross-Gateway Zone owner fencing/handoff, and accepted 30-active gameplay
> feel.

> Latest Zone-native magic authority sync: 2026-05-25 completed the next
> shared-combat slice after Zone-native ranged/object Magic launch. Zone now
> owns object-magic MP spend, per-Spell cooldown rejection, and `ObjectMana`
> AOI fanout for `PlayerCastMagic`; the personal session only supplies the
> learned Crystal magic profile and mirrors accepted MP/cooldown spend for
> UI/save. Verification passed the focused Zone-native player attack suite
> including `zone_native_player_magic_spends_mana_and_enforces_cooldown`, the
> Gateway shared `RangeAttack` routing regression, and locked
> Simulation/Gateway check locally and on the UCloud host. Gateway release
> `20260525T0630CST-zone-magic-mp-cooldown` is live over
> `20260525T0615CST-zone-range-magic`; public health, WSS smoke
> `docs/generated/load/remote-zone-magic-mp-cooldown-wss-smoke-20260525.json`,
> and headed Chrome WebGPU movement
> `docs/generated/player-qa/movement-jitter/live-webgpu-keyboard-after-magic-mp-20260525.json`
> all passed. Same-release 30-simultaneous baseline
> `docs/generated/load/remote-zone-magic-mp-cooldown-30active-baseline-20260525.json`
> held the current production cap at `ready=15/30`, `capacityRejected=15`,
> `errors=0`, `ok=true`, and keepalive p95 `22076ms`; this is evidence that
> 30-active gameplay feel is still not accepted. Remaining shared MMO
> authority work: Buff/stat/control/summon/AoE skill effects, broader
> ranged/magic/Boss monster AI, Zone-native NPC/quest side effects,
> transactional drop/economy commit, multi-process Zone owner fencing/handoff,
> and accepted 30-active gameplay feel.

> Latest Bevy WebGL2 entity-atlas renderer sync: 2026-05-25 completed the
> headed Chrome, atlas-cache, animation-frame hardening, and production rollout
> slice. Player Web now keeps visible entity body/hair/weapon sprite layers on
> the Bevy canvas, leaves map/HUD/nameplates/hit boxes in React, keeps DOM
> entity sprites visible only while a cold atlas is warming, and then hides the
> DOM fallback once the packed atlas is active. Atlas sources now include
> standing/walking/running frames plus all eight player movement directions, so
> a keyboard move no longer invalidates the atlas when the player changes from
> default facing to the movement direction. Production deployment
> `dpl_4PXPyp3VuAT7vHRQr4ueKBTikbtU` is live behind
> `https://mir2.obelisk.build`. Verification passed Web typecheck, scoped diff
> checks, Vercel build/deploy, public `/health`, production capture
> `docs/generated/player-qa/movement-jitter/prod-bevy-atlas-dir-20260525T043729.json`
> with `ok=true`, `atlasMode="packed"`,
> `atlasCurrentKey="entity-atlas-1iogxdg"`, `atlasPendingKey=null`,
> `atlasCachedCurrent=true`, `domEntityFallback=false`, 584 atlas sources, two
> keyboard Walk sends, two `UserLocation` ACKs, no critical console errors, and
> no non-favicon 404s, plus headed Chrome screenshot
> `docs/generated/player-qa/movement-jitter/headed-chrome-prod-bevy-atlas-final-20260525T0439.png`.
> Remaining renderer work is performance, not correctness: the first cold
> production atlas build is still large (`lastBuildMs=54672`), so the next
> optimization should reduce cold atlas build cost with prebuilt/offline or
> tighter warmed packs.

> Latest live Chrome blocked-transfer diagnosis: 2026-05-25 reproduced the
> user-reported "walk to map transfer" path in the current production Chrome
> tab by moving `Scout` on `BichonProvince` to the Library entrance at
> `322:247`; the live server stayed on map `0` instead of transferring to
> `0104 Library`. Root cause: direct Crystal movement source cells such as
> `0:322:247` can also be static/closed-door blocked cells, and the shared Zone
> movement validator rejected the step before the manifest-backed transfer
> detector could run. A second source mismatch made map `0` Zone collision use
> the starter fragment instead of the full original Bichon collision map. Source
> now lets player movement step onto valid direct movement source cells while
> preserving strict static collision for ordinary blocked cells and non-player
> occupancy, and Zone collision prefers the full original map data for Crystal
> map `0`. Verification passed focused personal and shared-Zone Library
> regressions, existing walk-on transfer regressions, adjacent
> `crystal_manifest_movements`, Simulation/Gateway fmt check, and locked
> Simulation/Gateway check. Production Gateway release is still pending, so the
> already-open Chrome tab remains evidence of the old live binary until the
> server is rebuilt/restarted with this patch.

> Latest production asset CORS closeout: 2026-05-25 fixed the live browser
> error where `https://mir2.obelisk.build` was blocked from fetching
> `https://assets.mir2.obelisk.build/mir2/v/37596e16d64fde7c/original-map/WemadeMir2/Objects/2136.png`
> because the cached asset-domain response lacked
> `Access-Control-Allow-Origin`. The R2 asset-cache Worker already emitted
> CORS on new R2 responses, but cache-hit responses could replay older cached
> headers without CORS. `mir2-r2-asset-cache` now reapplies CORS and exposed
> headers on every Cache API HIT before returning to the browser. Deployed
> Worker version `ea9ec199-d3e4-4627-a57a-c677ddd426be`. Verification:
> Wrangler dry-run/deploy succeeded, and
> `docs/generated/remote-assets/cors-asset-worker-20260525.json` shows GET,
> cache-busted GET, HEAD, and OPTIONS from origin `https://mir2.obelisk.build`
> all returning `access-control-allow-origin: *`; the normal GET remains
> `x-mir2-edge-cache=HIT`, proving old edge hits are now header-safe.

> Latest production movement command-latency closeout: 2026-05-24 fixed the
> remaining "walk command feels delayed and does not print in frontend" report.
> Root cause was not missing input dispatch: Player Web only recorded movement
> commands in debug arrays, while the production Gateway movement outcome path
> still built a full `world_snapshot()` after every Walk/Run/Turn. The web
> client now prints `[mir2-move:send]`, `[mir2-move:ack]`, and correction events
> when `?movementLog=1` or `localStorage["mir2-movement-log"]="1"` is enabled,
> and the Gateway/Simulation path skips outcome snapshots for low-latency
> movement and tick commands. The user-reported React #418 console path was
> also mitigated by making the document `notranslate`/hydration-warning safe
> and removing a random client-only overlay initializer. Remote Gateway release
> `20260524Tmovelowlatency` is live, and Player Web production deployment
> `dpl_BommXyKsMcAX3Lmw4TYcg82a7Rsw` is aliased behind
> `https://mir2.obelisk.build` with
> `NEXT_PUBLIC_MIR2_GATEWAY_WS_URL=wss://165.154.65.136.sslip.io/ws`, avoiding
> the high-jitter Cloudflare Worker `/ws` proxy. Verification passed focused
> Simulation/Gateway regressions, Gateway locked check, Web typecheck, script
> syntax/diff checks, public health, WSS
> smoke, and production movement capture
> `docs/generated/player-qa/movement-jitter/prod-normal-directws-keyboard-d-20260524T1513.json`
> (`ok=true`, normal entry WebSocket
> `wss://165.154.65.136.sslip.io/ws`, six walk sends, six `UserLocation` ACKs,
> frame latencies `555/522/516/523/517/517ms`, 12 movement console events, no
> visual jumps, no logical rollback, no scene blackouts, no critical console
> errors, and no non-favicon 404s). Screenshot:
> `docs/generated/player-qa/movement-jitter/prod-normal-directws-keyboard-d-20260524T1513.png`.

> Latest production movement visual closeout: 2026-05-24 deployed Player Web
> `dpl_8wQigG43KBLpaZY5oPPWHwNhz3QK` to close the remaining "movement still
> feels broken" reports after the guard/ACK fixes. The root causes split into
> two frontend paths: rapid discrete keyboard taps could expire while waiting
> for server `UserLocation` ACKs, and newly exposed Crystal map tiles/Objects
> showed black loading/transparent-key backgrounds during movement. Player Web
> now carries a bounded same-direction discrete input debt across ACKs, keeps
> held-key repeat behavior unchanged, gives the original-map scene a textured
> floor fallback while tile images load, and alpha-keys black-background map
> Object images before showing them. Verification passed Web typecheck, scoped
> diff check, Vercel prebuilt build/prune/deploy, custom-domain `/health`, and
> production headed Chrome evidence
> `docs/generated/player-qa/movement-jitter/prod-underlay-headed-keyboard-d-20260524T112744.json`
> (`ok=true`, six `walk Right` sends, six `UserLocation` ACKs from `331` through
> `336`, no `ObjectWalk`/`ObjectRun` guard spam, no visual jumps, no logical
> rollback, no scene blackouts, no console errors, and no non-favicon 404s).
> Screenshot:
> `docs/generated/player-qa/movement-jitter/prod-underlay-headed-keyboard-d-20260524T112744.png`.

> Latest production Web movement rollback correction: 2026-05-24 fixed two
> remaining causes of visible walk rollback in the current code path. Player Web
> no longer writes local predicted self movement into authoritative world state,
> and it waits for server ACK when the loaded original-map region cannot prove
> the next tile is valid. Shared Zone source now degrades standstill Run to an
> effective Walk instead of hard-correcting to origin. Verification passed Web
> typecheck, scoped diff check, focused shared-Zone standstill-run regression,
> local movement smoke
> `docs/generated/player-qa/movement-jitter/local-left-walk-wait-map-20260523T233000.json`,
> and production Web smoke
> `docs/generated/player-qa/movement-jitter/prod-left-walk-web-rollback-fix-20260524T0034.json`
> on deployment `dpl_3BwwKyjXY9UFZS3jSZk3vCsybCrW`, with `ok=true`, no visual
> jumps, no logical rollback, no scene blackouts, no critical console errors,
> and no non-favicon 404s. Remote Gateway release
> `20260524T0310Z-rollbackfix` is now live over
> `20260523T071900Z-actionqueue`; `mir2-status`, public origin health, WSS smoke
> `docs/generated/load/remote-rollbackfix-wss-smoke-20260524.json`, and
> post-Gateway production Web smoke
> `docs/generated/player-qa/movement-jitter/prod-left-walk-gateway-rollbackfix-20260524T0320.json`
> passed. Next task: run a longer production Chrome movement soak around open
> paths, NPC/monster clusters, and region edges to separate legitimate
> occupied-tile corrections from any remaining feel issues.

> Latest production scene-blackout follow-up: 2026-05-23 fixed the
> user-reported movement flicker where the main scene went black while HUD,
> minimap, and chat stayed visible. Root cause was the `scene-assets-pending`
> CSS state setting the primary scene layers to `opacity: 0` while movement
> triggered scene asset readiness to reload. The fix keeps the current scene
> visible during pending checks and only disables grid pointer input.
> Production deployment `dpl_5J4k5qF8mAbnjoj79gGYw2ypZTNv` is live through
> `https://mir2.obelisk.build`. Verification passed Web typecheck, movement
> harness syntax, scoped diff check, production `/health`, direct resource
> probes, and production keyboard movement evidence
> `docs/generated/player-qa/movement-jitter/prod-scene-blackout-normal-walk-20260523134030.json`
> with `ok=true`, `noSceneLayerBlackouts.count=0`, no visual jumps, no logical
> rollback, no route spam, no critical console errors, and no non-favicon
> 404s. Screenshot:
> `docs/generated/player-qa/movement-jitter/prod-scene-blackout-normal-walk-20260523134030.png`.

> Latest production movement/resource closeout: 2026-05-22 deployed remote
> Gateway release `20260522T174413Z-zone-transform` and Player Web deployment
> `dpl_BHimAGw5LRUVHUTFaWSUZsGhf2AH` to close the user-reported non-smooth
> movement, missing walk/idle sprites, and red resource errors. Root cause on
> movement was stale personal-session transform re-entering shared-zone state;
> root cause on invisible player/NPC-like sprites was scene sprite library/frame
> readiness allowing transient CArmour/CHair metadata failures to become
> permanent missing layers. Verification: Gateway focused transform-preserve
> regressions passed, remote public `/health` and WSS smoke passed, Web
> typecheck/script syntax passed, production `/health` passed, direct
> production resource probes for `CArmour/00`, `CHair/00`, `NPC/83/1.png`, and
> `Monster/010/3.png` returned 200, and final production movement evidence
> `docs/generated/player-qa/movement-jitter/prod-zone-transform-sprite-retry-2m-20260522.json`
> reports `ok=true` with no visual jumps, route spam, logical rollback,
> direction lag, critical console errors, or non-favicon 404s. Screenshot:
> `docs/generated/player-qa/movement-jitter/prod-zone-transform-sprite-retry-2m-20260522.png`.
> Remaining engineering hygiene: make the Vercel prebuilt flow avoid copying
> large `public/original-ui` and `public/original-map` before pruning.

> Latest production movement/resource follow-up: 2026-05-22 deployed and then
> promoted `dpl_8NeUFDsKu2NKMTFuAf1yF9YEoxXV` back to current after rejecting a
> worse ACK-preserve experiment. The landed production-safe fixes keep
> WebSocket keepalive active for normal clients, relax near-monster prediction
> stalls to exact occupied path tiles, hold turn visuals for one Crystal action
> frame, suppress repeated held blocked-direction attempts, and deploy updated
> Cloudflare domain/R2 Workers for asset proxy response cleanup. Verification:
> Web `pnpm --dir apps/web exec tsc --noEmit --pretty false`, Vercel prebuilt
> build/prune/deploy, Cloudflare Worker deploys, `/health` 200, and direct
> resource probes including `/original-ui/NPC/83/1.png` and
> `/original-ui/Prguse/65.png` returned 200. Production movement evidence
> `docs/generated/player-qa/movement-jitter/prod-movement-fix-15m-20260522.json`
> ran 15 minutes with no reconnect samples, clean settle, no movement residuals,
> no non-favicon 404s, and 196/196 scene assets loaded. Remaining active gap:
> Chrome still reports `net::ERR_QUIC_PROTOCOL_ERROR` because Cloudflare injects
> `alt-svc: h3`; current Wrangler OAuth can deploy Workers but gets 403 on
> zone `http3` settings. Disable Cloudflare HTTP/3 for `obelisk.build` with
> zone-settings access, then rerun the movement/resource capture.

> Latest production frontend movement/layout sync: 2026-05-22 deployed
> `dpl_Gr9WgZX275rpfDfk9f4SdzAshogb` for the user-reported Chrome resource,
> DevTools-width, hydration, and movement residual issues. The web client now
> has `/health`, viewport-driven 1024x768 stage scaling, deterministic initial
> motion state with a live fallback motion clock, and ACK-confirmed pruning for
> local self movement action feeds. Custom-domain `/health` returned 200 and
> `Monster/000/51.png` returned 200. Production evidence passed at
> `docs/generated/player-qa/movement-jitter/prod-final-narrow-stage-scale-20260522.json`
> (`ok=true`, 150x647 stage inside viewport, no console errors, no non-favicon
> 404s) and
> `docs/generated/player-qa/movement-jitter/prod-final-movement-ack-prune-skip-transfer-20260522.json`
> (`ok=true`, strict movement checks green, no visual jumps, no route spam, no
> logical rollback, no residual `directionStepPending` or
> `outstandingSelfMovementActions`). The blocked
> `prod-final-movement-ack-prune-20260522` transfer run confirms normal
> production clients still cannot use debug `crystal:<map>:<x>:<y>` teleports.

> Latest production walkable map-transfer sync: 2026-05-22 separated the
> all-map screenshot evidence from real player traversal evidence, then closed
> the server-side direct movement trigger gap. The production map-monster
> screenshot artifacts are still present: the main run has 807 PNGs, with
> 38 failure retakes, 1 GA1 Objects10 retake, 44 network-clean retakes, and the
> focused `hyunwol1` local-fulfill retake. New reachability evidence is
> `docs/generated/map/latest-crystal-map-reachability.json`: 463 maps,
> 1999 Crystal movement rows, 1906 direct rows, 93 ignored/special rows,
> 268 maps reachable by direct movement graph from Bichon map `0`, and
> 185/284 positive-respawn maps reachable by that direct graph. The remaining
> positive-respawn maps are not proven walk-direct from Bichon; they require
> NPC/script/event/item/special entry evidence or are isolated content maps.
> Runtime now applies Crystal map transfers when a player actually Walks/Runs
> onto a direct movement tile in both personal `SimulationSession` and the
> production shared in-process Zone Gateway path, without exposing debug
> `crystal:<map>:<x>:<y>` teleports to normal clients. Verification passed
> focused Simulation and Gateway direct-walk transfer regressions, adjacent
> `crystal_manifest_movements`, existing same-map shared-zone transfer sync,
> Rust fmt check, and locked Simulation/Gateway check. The UCloud Gateway was
> then rebuilt on the host and restarted to release
> `20260522T064157Z-walktransfer` (archive sha256
> `6682a9481370bde4f1f1c4def010047fb52aca3540f8605737e2cf03a84cb7c5`,
> binary sha256
> `4fc1dba3711b93cc60128e0c3fdbf14bab543a4e6ee58ac0008a53606373e75f`),
> replacing `20260521T0830Z-spreadrep`; remote `/health`, `mir2-status`, and
> 1-client WSS smoke
> `docs/generated/load/remote-walktransfer-wss-smoke-20260522.json` passed.

> Latest production map-monster screenshot sync: 2026-05-21 landed the
> production-safe cross-map QA channel for original map plus respawn evidence.
> `/api/qa/map-monster-scenes` enumerates the Crystal respawn manifest into 807
> representative scenes across 463 source maps / 284 maps with positive
> respawns / 6340 positive respawn rows, and `/qa/map-monsters` renders each
> scene using the production `crystal-map-loader` output without debug player
> teleports. The QA renderer now uses the loader-clamped `sceneView.center` for
> map sprites and clamps offscreen respawn markers, fixing blank screenshots for
> out-of-bounds source respawn centers. The capture script supports exact
> `--sceneIndexes` retakes so production failures can be repaired without
> rerunning all 807 scenes. The full production evidence is
> `docs/generated/player-qa/production-map-monsters/production-full-map-monsters-qa807-resource-strict-20260521/summary.aggregate.json`:
> `ok=true`, 807/807 captured, failure count 0, zero map-sprite scenes 0,
> broken images 0, network 404s 0, network failures 0, and console errors 0.
> This aggregate is the production resource-health gate; heavy-map visual
> retakes can use the capture script's QA-only
> `--fulfillOriginalMapFromPublic` mode to avoid browser connection queue
> pending while still opening the production QA URL. A focused `hyunwol1`
> retake verified that mode with `imagesComplete=true` and `pendingImageCount=0`.
> During retake, GA1 exposed a real missing `Objects10` slice; 27 frames
> `5172..5234` used by the production GA1 court were exported from
> `WemadeMir2/Objects10.Lib`, uploaded to R2 prefix
> `mir2/v/37596e16d64fde7c` as
> `docs/generated/remote-assets/prod-ga1-objects10-patch-20260521/remote-asset-release.json`,
> and direct CDN probes plus the focused GA1 retake passed. Vercel production
> deployment `dpl_9L3LsRnN8mfJmDirFCpjnrBdeNJR` is READY at
> `https://mir2-web3-7ov6lp1xs-obelisk-labs.vercel.app`, aliased through
> `https://mir2.obelisk.build`.

> Latest original-map runtime-data production sync: 2026-05-21 fixed the live
> report where representative maps appeared as fallback/blank terrain even
> though monsters and labels were visible. Root cause: Vercel did not have the
> full Crystal client `Map/` and `Data/Map/*.Lib` source tree, so
> `/api/scene/crystal` served the packaged starter fragment or empty regions.
> Player Web now generates and traces compressed production runtime map data
> (`lib/generated/crystal-map-pack/**/*.map.gz`) plus map library frame
> metadata (`lib/generated/crystal-map-library-meta/**/*.json.gz`) and only
> writes PNG files when full RGBA frame data is available locally. The scene
> blueprint cache schema was bumped to avoid stale fallback regions. Runtime
> generation covered 1624 maps and 138 libraries / 1,327,368 frame metadata
> entries; the focused R2 upload added 1867 objects including newly exported
> original-map PNGs and the full release manifest was restored. Deployment
> `dpl_CLp4KrpvspZaPHExjdjtazkRdFUs` is READY at
> `https://mir2-web3-5kzhyxrns-obelisk-labs.vercel.app`, aliased to
> `https://mir2-web3-web.vercel.app`, and visible through
> `https://mir2.obelisk.build`. Verification passed Web typecheck, generator
> syntax check, Vercel production build/deploy, sample PNG 200 probes, and
> production `/api/scene/crystal` probes for Bichon, WoomyonWoods,
> NaturalCave, DeadMineEntrance, InsectCave_2F, and ZumaMaze with non-zero
> sprite/cell counts. Playable Bichon evidence:
> `docs/generated/player-qa/live-map-monsters/prod-map0-bichon-runtime-wait20-20260521Tnow.png`
> / `-state.json`, with `mapObjectSpriteCount=120` and `network404Count=0`.
> Next active slice: add a production-safe QA relocation/admin snapshot path
> for cross-map screenshots, since production correctly rejects debug
> `crystal:<map>:<x>:<y>` transfer keys on the player WebSocket path.

> Latest original-ui metadata/exporter split sync: 2026-05-21 completed the
> follow-up resource-management cut from the CDN-first Vercel deployment.
> `/api/original-ui-meta` no longer imports
> `lib/original-ui-export-server.ts`; it now uses the lightweight
> `lib/original-ui-meta-server.ts` reader to fetch already deployed static
> `meta.json` from the player domain or configured R2/CDN asset base, and it
> returns `library_not_deployed` instead of attempting request-time Crystal
> export. This keeps local export/repair work in scripts and prevents the
> production route bundle from tracing the full `public/original-ui` tree.
> Production build verification reduced Turbopack broad-pattern warnings from
> two to one: the `original-ui-export-server.ts` warning is gone; the remaining
> warning is the separate `lib/crystal-map-loader.ts` / `public/original-map`
> path. Deployment `dpl_Fq8FkQb2JxjEmMAHwNXJCU4v7Xdi` is READY at
> `https://mir2-web3-ezaeeogvv-obelisk-labs.vercel.app`, aliased to
> `https://mir2-web3-web.vercel.app`, and visible through
> `https://mir2.obelisk.build`. The post-build prune report
> `docs/generated/remote-assets/vercel-output-prune-meta-reader-split-20260521.json`
> reduced `.vercel/output` from 427,399,093 bytes / 20,516 files to
> 43,657,235 bytes / 278 files. Production probes returned 200 for
> `/api/original-ui-meta?library=Items`, `/api/original-ui-meta?library=NPC/94`,
> R2-backed `/original-ui`, `/original-map`, `/generated/original-map-blend`,
> retained debug samples, and same-origin Bevy wasm; `Map/foo` correctly
> returned `unsupported_library`. Verification passed Web typecheck, production
> cache-maintenance smoke
> `docs/generated/player-qa/cache-metrics/cache-metrics-meta-reader-split-prod-20260521.json`
> with `ok=true`, 387/387 prewarm ok, warm transfer 0 bytes, reset cleanup
> returning to 0 caches, no critical console errors, and no non-favicon 404s,
> plus playable production smoke
> `docs/generated/player-qa/cache-metrics/cache-metrics-meta-reader-split-playable-prod-20260521.json`
> with `ok=true`, cold/warm first playable 13745.3ms / 14118.8ms, 387/387
> prewarm ok, and no non-favicon 404s. Next resource-management cut: split
> production scene metadata from `crystal-map-loader.ts` so `/api/scene/crystal`
> stops tracing `public/original-map`.

> Latest CDN-first Vercel output sync: 2026-05-21 landed the first
> production-safe resource-management cutover from "ship Crystal media inside
> every Vercel deployment" to "ship the player shell/runtime and serve large
> Crystal media from R2/CDN." `apps/web/scripts/prune-vercel-output-assets.mjs`
> now removes only generated `.vercel/output/static/original-ui`,
> `.vercel/output/static/original-map`, and
> `.vercel/output/static/generated/original-map-blend` after `vercel build`;
> `apps/web/package.json` exposes `vercel:build:prod` and
> `vercel:deploy:prod` for this prebuilt flow. The final prune report
> `docs/generated/remote-assets/vercel-output-prune-resource-cdn-first-20260521.json`
> reduced `.vercel/output` from 420,957,251 bytes / 18,650 files to
> 43,478,680 bytes / 278 files, removing 377,478,571 bytes / 18,372 files,
> while retaining `static/debug` because the playable page still requests
> `/debug/map-samples/smtile-72.png` and `smtile-80.png`. Production
> deployment `dpl_ieQqdaZMnnZYNe4wxksuoqsj7Sgg` is READY at
> `https://mir2-web3-js3ofmmod-obelisk-labs.vercel.app`, aliased to
> `https://mir2-web3-web.vercel.app`, and visible through
> `https://mir2.obelisk.build`; the deploy uploaded 15.7MB instead of the
> unpruned static output. Production probes returned 200 for R2-backed
> `/original-ui`, `/original-map`, `/generated/original-map-blend`, the
> retained debug samples, and same-origin `/bevy-runtime`. Verification passed
> Web typecheck, script syntax check, production cache-maintenance smoke
> `docs/generated/player-qa/cache-metrics/cache-metrics-resource-cdn-first-final-prod-20260521.json`
> with `ok=true`, 387/387 prewarm ok, warm transfer 900 bytes, reset cleanup
> returning to 0 caches, no critical console errors, and no non-favicon 404s,
> plus playable production smoke
> `docs/generated/player-qa/cache-metrics/cache-metrics-resource-cdn-first-playable-final-prod-20260521.json`
> with `ok=true`, cold/warm first playable 14212.5ms / 14163.9ms, 387/387
> prewarm ok, warm transfer 600 bytes, and no non-favicon 404s. Next resource
> slice: split production metadata readers away from local exporter modules so
> Turbopack no longer warns that `lib/original-ui-export-server.ts` traces the
> full `public/original-ui` tree during build analysis.

> Latest resource cache-tier production sync: 2026-05-21 moved Player Web from
> a single bulk static asset cache toward staged MMORPG-style resource
> management. `/api/asset-manifest` now exposes critical/background/runtime
> static cache budgets, cache packs declare `cacheTier`, and the asset Service
> Worker writes login/select/HUD URLs into `mir2-asset-cache-static-critical-*`
> while Bichon scene prewarm hints dynamic frame URLs into
> `mir2-asset-cache-static-background-*`. Deployment
> `dpl_9qZP7jXVU1Q6BzUWZVyQKKkMgiaf` is READY at
> `https://mir2-web3-aefb2e729-obelisk-labs.vercel.app`, aliased to
> `https://mir2-web3-web.vercel.app`, and visible through
> `https://mir2.obelisk.build`; production `/api/asset-manifest` reports
> version `5d1ec8e93c1caa62`, `staticCriticalMaxEntries=3000`,
> `staticBackgroundMaxEntries=6000`, `staticRuntimeMaxEntries=16000`,
> login/select/HUD as critical, and Bichon spawn as background. Verification
> passed service-worker/script syntax checks, Web typecheck, local production
> build, local cache-maintenance smoke
> `docs/generated/player-qa/cache-metrics/cache-metrics-resource-tier-local-20260521.json`,
> Vercel production build/deploy, and production cache-maintenance smoke
> `docs/generated/player-qa/cache-metrics/cache-metrics-resource-tier-prod-20260521.json`
> with `ok=true`, 387/387 prewarm ok, warm CacheStorage 3 caches / 383 entries,
> no critical console errors, no non-favicon 404s, and reset cleanup returning
> to 0 Mir2 caches. Next asset-management slice: remove the remaining
> `original-ui` static copy from Vercel output once the R2-only metadata/static
> fallback path is accepted for all player-domain resource classes.

> Latest original-map spawn cleanup sync: 2026-05-21 routed production Gateway
> startup through the Crystal map runtime so original Bichon no longer displays
> the starter `Training Dummy` / `Field Wasp` fixture monsters. Crystal starts
> now normalize map metadata from the respawn manifest and rebuild original
> current-map NPC/monster surfaces from Crystal data. Saved non-default starts
> also materialize representative monsters from broad original respawn regions
> when the player is inside that data range, so production QA can capture
> forest/cave/temple maps with their own roster visible. The all-map gameplay audit
> auto-detects the local full client root and strict mode passed: 463 maps,
> 6341 respawns, respawn failures 0, NPC failures 0, movement failures 0, static
> failures 0. Next active slice: deploy/restart the live Gateway and browser
> smoke Bichon plus representative respawn-heavy maps.

> Latest scene backdrop fallback sync: 2026-05-21 closed the live-map black
> edge/gap report without changing minimap/big-map coordinate projection. Player
> Web now keeps the synthetic terrain tile backdrop active under partial
> original Crystal floor sprites, retries scene/UI image failures through
> cache-busted same-origin URLs plus the configured remote asset base, applies
> the same retry candidates during visible scene preload, and requests a wider
> Crystal scene blueprint margin so chunk edges refresh before play-bound edges.
> Verification passed Web typecheck and local Web `127.0.0.1:13017` against live
> `wss://mir2.obelisk.build/ws`; evidence
> `docs/generated/player-qa/movement-jitter/map-scene-fallback-ui-retry-final-20260521.json`
> has `ok=true`, scene assets `127/127`, no visual jumps/rollback/route spam,
> no non-favicon 404s, and no critical console errors. Screenshot:
> `docs/generated/player-qa/movement-jitter/map-scene-fallback-ui-retry-final-20260521.png`.

> Latest production WebSocket fallback sync: 2026-05-20 fixed the live Player Web bundle path that could still connect hosted users to `ws://127.0.0.1:7110/ws` when `NEXT_PUBLIC_MIR2_GATEWAY_WS_URL` was missing from the production build. The client now resolves Gateway URLs in this order: explicit `?gatewayWs=`, configured public env var, local default for `localhost`/`127.0.0.1`, and hosted same-origin `/ws` otherwise. The asset service worker stale-while-revalidate path for `/api/scene/crystal` now returns a controlled retryable `503` JSON response on no-cache network failure instead of surfacing an unhandled second `fetch()`. Deployment `dpl_9U4QFRQHubk8vzaKXYN7FQMWhRhp` is READY at `mir2-web3-1ywu3e52h-obelisk-labs.vercel.app`, aliased to `mir2-web3-web.vercel.app`, and visible through `https://mir2.obelisk.build`; no-env production smoke `docs/generated/player-qa/cache-metrics/cache-metrics-prod-ws-fallback-20260520.json` recorded `gatewayConnectStart` as `wss://mir2.obelisk.build/ws` on cold and warm runs, with first playable 11463.3ms / 5893.5ms. This smoke also exposed a separate R2/static resource follow-up: `/original-ui` 404s for Title/Prguse/Monster/character equipment frames, so it is not a full green cache acceptance artifact.

> Latest prewarm-latency movement-feel sync: 2026-05-19 landed and production-deployed the follow-up optimization for the scene-ready movement path. Player Web now splits cache packs into critical versus background phases, rotates the manifest hash when `ASSET_CACHE_PACKS` changes, delays the Bichon background prewarm until after the first playable frame plus a 20s idle window, reduces that background frame cap to 180, and only mounts original map object sprites whose pixel bounds intersect the visible viewport margin. This keeps movement authority unchanged while reducing focused Bichon first-scene work from the previous 217/218 visible assets to 112/112 locally and 124/124 on the current production test camera. Deployment `dpl_4YwqgqQdhA1HQQwPhFrA1KoTCpXP` is READY at `mir2-web3-7r34j61kg-obelisk-labs.vercel.app`, aliased to `mir2-web3-web.vercel.app`; `https://mir2.obelisk.build/api/asset-manifest` reports version `ecb5ff44ad1ad66b`, `asset-cache-packs` SHA256 `ccb99631adab3fda78d4db3029e6199cb79f0c29256662cbb33691aee016d8f0`, and `bichon-spawn` as background. Verification passed Web typecheck, movement/cache script syntax checks, Vercel production build/deploy, and direct production reruns on `https://mir2.obelisk.build`: playable cache `docs/generated/player-qa/cache-metrics/cache-metrics-prod-viewport-pruned-delay20-cache-existing-20260519-221410.json` has `ok=true`, cold first playable 11673.5ms, warm first playable 13549.9ms, 387/387 prewarm ok, warm CacheStorage 437 entries / 54.5MB, no critical console errors, and no non-favicon 404s; movement `docs/generated/player-qa/movement-jitter/prod-viewport-pruned-existing-settle9-20260519-221630.json` has `ok=true`, 124/124 scene assets loaded, `packetRuntimeModes={"packetRefresh":58}`, no visual jumps/rollback/route spam/stale prediction/queue warnings, no critical console errors, and no non-favicon 404s. Next slice: run a separate fresh-account NEW/start-game lifecycle smoke on production, because one first-attempt cache smoke with account creation timed out before game stage while existing-character movement/cache validation passed.

> Latest new-account character-list sync: 2026-05-19 fixed the fresh-account `Scout` Warrior leak. `demo` remains the only automatically seeded smoke account; missing password accounts fail login, and first-time Passkey/Wallet accounts start with an empty select list before the normal `NEW` character creation flow. Verification passed focused Simulation account lifecycle tests and locked Simulation/Gateway check. Next live slice: deploy/restart the production Gateway/Web pair, then smoke `https://mir2.obelisk.build` with a fresh Sui login through NEW/class selection.

> Latest original Bichon intro quest-chain sync: 2026-05-18 closed the next starter-experience validation gap by driving original Crystal q1-q4 on map `0` through real NPC dialog commands and Q drops. The vertical slice now proves Assistant Jane -> CraftLady -> Assistant -> Merchant John, Scarecrow `GingerTea`, passive Deer melee kill plus Harvest `DeerMeat`, q4 turn-in, and q5 availability. Verification passed focused original Bichon intro test, full Simulation `vertical_slice` 6/6, Simulation `shared_zone` 77/77, Simulation `security_lifecycle` 9/9, and Simulation/Gateway locked check. Next active slice: extend live-client acceptance across q5+ early combat tasks and representative mid/late 1-45 NPC routes while preserving the current automated starter gate.

> Latest Zone-native monster combat/drop sync: 2026-05-18 landed the first normal shared-monster melee route, native monster AI tick, and native monster-to-player HP write-back on the Zone producer. Gateway now seeds live personal-session map monsters into Zone and sends explicit shared monster `WorldCommand::Attack` through `PlayerAttackObject`; Zone sends `ObjectAttack` immediately and resolves the Crystal hit frame on `Tick` with strike/damage/health/death, owner-window drops, experience, and `MonsterKillAward`. Native Zone monsters also walk toward nearby players, launch adjacent delayed melee-hit visuals, update Zone-held player HP, emit player `ObjectHealth`, and use `PlayerDamaged` so Gateway writes the same damage into the target `SimulationSession` `PlayerVitals`. Verification passed Simulation `shared_zone` 77/77, Gateway `shared_in_process` 40/40, Simulation `security_lifecycle` 9/9, focused delayed-hit/native-attack/AI-tick/HP-writeback regressions, and Simulation/Gateway locked check. Next active backend slice: native RangeAttack/Magic skill semantics and full Crystal drop-table exactness.

> Latest Postgres+Redis cutover sync: 2026-05-18 landed the prod-like runtime policy for account/character persistence plus online route/session cache. Gateway now requires Redis when production/staging account-store policy is active or when `MIR2_GATEWAY_REQUIRE_REDIS_CACHE=1` is set, validates required Redis on startup, and keeps local JSON/in-memory fallback scoped to development. Staging and systemd env examples now default to Postgres source-of-truth plus Redis route leases. Verification passed focused Gateway session-cache environment 6/6, session-cache/Redis/lease suite 20/20, Gateway health boundary, Simulation account-store environment, Gateway/Simulation locked check, Gateway fmt, and scoped diff checks. Next infrastructure slice: add longer prod-like soak evidence and then push remaining inventory/mail/economy character domains out of monolithic account JSON shapes.

> Latest ranking-system sync: 2026-05-18 landed the Crystal-style player ranking path end to end. Simulation now handles `ClientPacket::GetRanking` from saved account-store characters plus the active in-world character, supports overall and class filters (`Warrior`, `Wizard`, `Taoist`, `Assassin`, `Archer`), sorts by Crystal-like level/experience order, returns `ServerPacket::Rankings` with listing details, total count, and the current character's rank, and keeps `OnlineOnly` scoped to the currently online session instead of inventing global online state. Gateway maps Web `getRanking` commands to the typed Crystal packet, and Player Web now has a real System Menu `Ranking` panel with Overall/class/Online tabs, refresh commands, selected-row detail, and My Rank display. Verification passed Simulation ranking test, Gateway command/event tests, Web typecheck, Simulation/Gateway fmt/check, and live Browser smoke with evidence under `docs/generated/player-qa/ranking-system/`. Next ranking slice: broader multi-account/player ranking acceptance once a production shared online roster and persistence backend replace the current JSON-store/dev-session limits.

> Latest shared Zone drop-claim sync: 2026-05-18 completed the previously queued "move shared drop claim/reserve/commit/cancel into Zone" slice. Gateway now seeds Zone with shared ground drops, routes manual and IntelligentCreature pickup through Zone claims, mirrors Zone removals back into the Gateway map layer, commits successful pickups, and cancels/restores failed personal award/filter/full-bag claims without leaving stale object-remove packets. Zone owns nearest eligible auto-pick selection by range, allowed ids, ownership, and group rules. Verification passed Simulation `shared_zone` 74/74 and Gateway `shared_in_process` 38/38. Next active backend slice: move drop generation, monster AI/combat, and item/gold award mutation out of personal-session mirroring and into shared Zone or an actor-owned world service.

> Latest Web Packet Runtime movement sync: 2026-05-18 replaced the remaining live-game state race where normal `worldSnapshot` refreshes could overwrite packet-applied player/entity positions. Player Web now classifies snapshots into bootstrap/reconnect/map-change/scene-bootstrap versus packet-refresh modes; packet-refresh keeps Crystal typed packets (`UserLocation`, `ObjectWalk/ObjectRun`, object spawn/remove, and ground-drop packets) as the live state authority and only merges durable snapshot metadata. Removed packet objects are tombstoned so a stale snapshot cannot reinsert them. Movement diagnostics now report packet-runtime modes, and the original-ui meta API now triggers on-demand export for missing full-client source libraries such as `NPC/94`. Verification passed Web `pnpm --dir apps/web exec tsc --noEmit --pretty false`, `node --check apps/web/scripts/capture-web-movement-jitter.mjs`, `curl .../api/original-ui-meta?library=NPC%2F94`, high-frequency key-sequence capture `docs/generated/player-qa/movement-jitter/r-web-packet-runtime-keyseq-20260518b.json`, and hold-run plus target-spam capture `docs/generated/player-qa/movement-jitter/r-web-packet-runtime-holdspam-20260518d.json`, both with `ok=true`, all samples in `packetRefresh`, no visual jumps, no logical rollback, responsive queue, clean settle, no console errors, and no non-favicon 404s. Local Web is running on `127.0.0.1:13014` with Gateway `127.0.0.1:7210` for human retest.

> Latest original 1-45 quest-chain sync: 2026-05-18 implemented the original Crystal normal quest backend loop for automated acceptance. The active task queue moves from "build 1-45 quest semantics" to live acceptance: use the Web client to verify representative NPC dialog flow, visible quest markers/Quest Diary text, route guidance, and reward presentation across early, mid, and late normal quest bands. Backend evidence is locked check for game-data/simulation, focused original Crystal quest tests, seed-state quest visibility, packet finish/share coverage, Field Wasp regression, Crystal quest-drop regressions, generator syntax check, and Rust fmt checks.

> Latest strict Crystal CurrentLocation movement sync: 2026-05-16 tightened the user-reported movement rollback fix to Crystal's hard client semantic rather than another prediction layer. Player Web now advances the local self `WorldEntity` to the Walk/Run action target at action start, like Crystal `SetAction()` advancing `CurrentLocation` before rendering `OffSetMove` from the source tile. Self `UserLocation` packets and `worldSnapshot` self entries can confirm or stale-echo active local actions without pulling the player back; only true corrections overwrite the local transform. Verification passed Web `npx tsc --noEmit`, `node --check apps/web/scripts/capture-web-movement-jitter.mjs`, rapid Shift+`D/A` capture `docs/generated/player-qa/movement-jitter/r-strict-actionfeed-current-location-230738.json`, 10s Shift+`D` capture `docs/generated/player-qa/movement-jitter/r-strict-actionfeed-current-location-long-230835.json`, and held-run plus repeated target-click stress `docs/generated/player-qa/movement-jitter/r-strict-actionfeed-current-location-clickspam-231926.json`, all with `ok=true`, no visual jumps, no logical rollback, responsive queue, clean settle, `pendingPlanAtEnd=null`, no console errors, and no non-favicon 404s. Local Web remains running on `127.0.0.1:13010` with Gateway `127.0.0.1:7210` for human retest.

> Latest all-map resource/gameplay sync: 2026-05-16 closed the current map acceptance automation gap. Web `audit:crystal-map-coverage` now classifies empty/out-of-range Crystal map frame references as source-client no-draw behavior instead of fallback risk, records 463/463 maps present/parseable, missing minimap indices `[]`, missing sampled map libraries 0, and `visualFallbackRisk.mapCount=0`. Added `audit:crystal-map-gameplay`, which checks the full manifest/runtime surface for movements, respawns, NPC scripts, safe zones, safe-zone spell flags, doors, cell lights, fishing cells, drop rules, light/feature flags, and static map semantics. It records 1999 movement rows with 1906 direct transfers, 93 Crystal-ignored/deferred/special rows, movement failures 0, 6341 respawns with 6293 walkable-candidate rows and 48 Crystal-inert no-candidate warnings, respawn failures 0, 375 NPC rows with scripts found, 7 empty placeholder warnings, unimplemented NPC commands 0, and static failures 0. Simulation now finds the local full Crystal client root, fixes type-1 map cell stride parsing, filters invalid/special Crystal movement rows from runtime direct transfers, and leaves no-candidate respawns inert instead of spawning on invalid origins. Verification passed both map audits, Web `npx tsc --noEmit`, Simulation fmt check, focused `crystal_manifest_movements` 2/2, and focused `spread_slots` 2/2. Next active map slice is human visual walk-through rather than an automated missing-source blocker.

> Latest local CurrentLocation movement sync: 2026-05-15 closed the remaining user-reported high-frequency movement residual/rollback path. Player Web now commits visually completed self Walk/Run actions into the local self `WorldEntity` / `sceneView.center`, matching Crystal's local `CurrentLocation` semantics, and stale `worldSnapshot` self positions no longer overwrite a plausible forward local action while Zone/UserLocation confirmations catch up. Verification passed Web `npx tsc --noEmit`, direct `npx next build`, `node --check apps/web/scripts/capture-web-movement-jitter.mjs`, rapid Shift+`D/A` capture `docs/generated/player-qa/movement-jitter/r-highfreq-keyseq-da-after-local-current-location-16ms.json`, 10s Shift+`D` capture `docs/generated/player-qa/movement-jitter/r-long-shiftd-after-local-current-location-16ms.json`, and right/left reversal capture `docs/generated/player-qa/movement-jitter/r-right-left-after-local-current-location-16ms.json`, all with `ok=true`, no visual jumps, no logical rollback, responsive queue, clean settle, `pendingPlanAtEnd=null`, no console errors, and no non-favicon 404s. Local Web remains running on `127.0.0.1:13010` with Gateway `127.0.0.1:7210`.

> Latest high-frequency movement sync: 2026-05-15 closed the user-reported high-frequency WASD/Arrow run jitter path. Player Web now keeps a separate outstanding self-movement action ledger for service confirmations, so opposite-direction inputs update the latest intent but cannot be sent from stale speculative tiles while older Walk/Run confirmations are still in flight. The movement harness now covers strict `keyboardSequence` input, pre-input warmup, WebSocket movement frame tails, and direction-step source-aware route-spam classification. Verification passed Web `npx tsc --noEmit`, direct `npx next build`, script `node --check apps/web/scripts/capture-web-movement-jitter.mjs`, rapid Shift+`D/A` capture `docs/generated/player-qa/movement-jitter/r-highfreq-keyseq-da-after-outstanding-gate-170756.json`, right-run then left-run reversal `docs/generated/player-qa/movement-jitter/r-highfreq-right-then-left-after-outstanding-gate-170859.json`, and 12s Shift+`D` long-run regression `docs/generated/player-qa/movement-jitter/r-long-shiftd-after-outstanding-gate-170946.json`, all with `ok=true`, no visual jumps, no logical rollback, no route-spam warnings, responsive queue, clean settle, no console errors, and no non-favicon 404s. Local Web remains running on `127.0.0.1:13010` with Gateway `127.0.0.1:7210`.

> Latest long-run movement rollback sync: 2026-05-15 closed the user-reported long continuous Run+Walk rollback/retry loop on Player Web. The frontend now distinguishes Crystal local action lookahead from render lead, preserves non-stale render positions while Zone `UserLocation` catches up, and treats repeated same-tile confirmations during held direction input as a blocked action: it records no-progress self acks, marks the authoritative source tile/direction as blocked for both walk and run, suppresses the held direction for the route-block memory window, and clears local action/render anchors on true correction. Verification passed Web `npx tsc --noEmit`, direct `npx next build`, CDP 12s Shift+`D` capture `docs/generated/player-qa/movement-jitter/r-long-shiftd-fresh-after-first-block.json` with `ok=true`, `noVisualJumps`, `noLogicalTileRollback`, `noRouteSpamWarnings`, responsive queue, clean settle, no browser console errors, and no non-favicon 404s; focused Simulation `continuous_run_extends_run_grace_after_successful_run`; and `cargo +1.89.0 fmt --check -p mir2-simulation`. Local Web is running on `127.0.0.1:13010` for human retest.

> Latest Crystal ActionFeed movement sync: 2026-05-15 aligned Player Web self movement with Crystal's local action semantics instead of treating every self `UserLocation` as an immediate rollback authority. The client now tracks a local self-action feed for Walk/Run source, target, direction, mode, sent time, and visual window; self `UserLocation` packets confirm or stale-echo those actions before correction, and rendering/debug player state falls back to the latest local action target while the Zone/server catches up. The Web runtime boot path also supports DOM-only `skipRuntime=1` for movement harnesses and avoids duplicate Bevy boot on same-page HMR. Verification passed Web `npx tsc --noEmit`, direct `npx next build`, movement harness syntax check, and CDP mini smoke `docs/generated/player-qa/movement-jitter/r-crystal-actionfeed-mini-smoke3.json` with Shift+`D` `{330,270}->{345,270}`, `rollbackCount=0`, `staleSampleCount=0`, final `feed=[]`, final `queue=[]`. Next active frontend slice: rerun broader click/hold target-spam and manual in-browser feel acceptance on the user's live page.

> Latest frontend chat-control sync: 2026-05-15 closed the player-facing Crystal chat control row under the belt. The Web `ChatControlBar` now uses Crystal outgoing prefixes for All/Shout/Whisper/Lover/Mentor/Group/Guild, moves channel visibility filtering into Settings, adds transparency toggling, sends real `tradeRequest` from Trade, and keeps Size/Report behavior covered. The sprite buttons now have 24x13 hit boxes and are topmost over the HUD, fixing the previously visible-but-not-clickable row. Verification passed Web `node --check apps/web/scripts/smoke-stage5-ui.mjs`, Web `npx tsc --noEmit`, direct `npx next build`, and the dedicated live smoke `MIR2_STAGE5_ACCOUNT_MODE=demo MIR2_STAGE5_SMOKE_CHAT_ONLY=1 ... node apps/web/scripts/smoke-stage5-ui.mjs`, which wrote `docs/stage5-screenshots/stage5-chat-controls-smoke-manifest.json` with 13 screenshots, every chat-control hit test `topMatches=true`, verified chat prefixes/send/trade/settings/size/report, and `criticalConsoleErrors=[]`. Next active frontend slice: broader full Stage 5 smoke refresh once the dirty demo-save inventory split fixture is reset or bypassed.

> Latest shared object-action Zone AOI sync: 2026-05-14 migrated shared monster/generated-object observer fanout from Gateway same-map pending queues into Zone retained-object AOI. Gateway now seeds shared Monster/NPC objects into Zone and sends shared-object action/result packets through Zone; Zone preserves monster actor ids, rebases only local self target/result ids to the Zone player id, updates retained object health/death state, and skips far same-map observers outside object visibility. Verification passed focused Simulation shared-object tests 3/3, Simulation `shared_zone` 69/69, Gateway `shared_in_process` 35/35, and Simulation/Gateway fmt/check. Next active slice: move shared drop claim/reserve/commit/cancel plus owner/despawn expiry from Gateway maps into Zone while keeping inventory/gold award in personal Session.

> Latest retained object authority sync: 2026-05-14 completed the next shared Zone hardening slice. Retained object Buffs now keep full Crystal `AddBuff` payloads for late join/AOI replay, dead or harvested retained objects suppress stale movement/mana/positive-health packets, retained object HP is monotonic downward until revive, and retained NPC/live-monster occupancy now blocks impossible player movement while dead/removed/drop/deco objects stay non-blocking. Verification passed Simulation `shared_zone` 66/66, Gateway `shared_in_process` 35/35, and Simulation/Gateway fmt/check. Next active slice: replace more personal-runtime combat/drop/NPC result mirroring with native shared Zone authority, especially shared monster action/result fanout and drop claim/award ownership.

> Latest retained object-vitals sync: 2026-05-14 added retained health/mana packets to the shared Zone object visibility surface. `ObjectHealth` for retained non-player objects is now remembered as latest state, sent with the retained object spawn for late joiners and object-AOI entry, preserves zero-health death behavior, and is cleared by revive; `ObjectMana` for MP-bearing retained heroes/generated objects now follows the same late-join visibility path and clears on death/revive. Verification passed focused retained-object health/mana regressions 3/3, Simulation shared_zone 60/60, Gateway shared_in_process 35/35, and Simulation/Gateway fmt/check. Next active slice: continue moving damage/death/drop calculation itself from personal-runtime mirroring into shared Zone authority.

> Latest retained harvest-corpse sync: 2026-05-14 promoted shared corpse harvest completion into simulation Zone retained-object state. `ObjectHarvested` for non-player retained objects now records a harvested/dead marker, keeps late joiners on the harvested corpse anchor instead of stale live snapshots, suppresses repeated harvest-complete packets, and avoids treating rebased player harvest animation packets as dead object state. Verification passed focused harvested retained-object regressions 3/3, Simulation shared_zone 57/57, Gateway shared_in_process 35/35, and Simulation/Gateway fmt/check. Next active slice: continue moving the actual harvest reward/drop source of truth from personal-runtime output mirroring into shared Zone authority.

> Production Crystal action-queue movement closeout: 2026-05-23 supersedes the
> older latest-intent/run-grace approximation. Zone movement now keeps a bounded
> ordered per-player Walk/Run/Turn queue, consumes ready actions on Crystal
> `ActionTime`, applies Turn 350ms and Walk/Run 600ms timing, and the later
> local movement-rollback correction changes raw Run from standstill to an
> effective one-tile Walk rather than an origin correction. Web self
> movement treats `UserLocation` as confirmation/correction instead of a new
> animation source, renders two-tile Run as one 600ms action, caps local
> ActionFeed lead to two tiles, and no longer lets predicted-ahead state swallow
> real server corrections. Verification passed Simulation shared_zone 78/78,
> focused Gateway Walk+Run/Turn routing, Simulation/Gateway fmt-check, Web
> typecheck, Web production build, local movement captures
> `crystal-action-queue-local-shiftd-20260523` /
> `crystal-action-queue-local-da2-20260523`, remote Gateway release
> `20260523T071900Z-actionqueue`, Player Web action-queue verification deployment
> `dpl_HmHQ4CXfy7d895kHFMfiNLHWespN`, custom-domain production `/health`, and production
> walk/run captures
> `docs/generated/player-qa/movement-jitter/prod-action-queue-keyboard-walk-fix2-20260523T1331.json`
> plus
> `docs/generated/player-qa/movement-jitter/prod-action-queue-keyboard-run-fix2-20260523T1332.json`,
> both `ok=true` with zero visual jumps, logical rollback, scene blackouts,
> critical console errors, and non-favicon 404s. Next active slice: continue
> shared-native combat/drop/NPC authority work plus final human Crystal feel
> acceptance on broader collision edge cases.

> Latest delayed combat status-result sync: 2026-05-14 extended Gateway delayed player-action filtering beyond strike/health/death/drop bundles. Tick-delayed packets owned by the local player now keep matching `ObjectPoisoned`, `AddBuff`, `RemoveBuff`, and `PauseBuff` results for struck targets or the acting player, while still dropping unrelated monster-AI tick results from other attackers. Verification passed the focused delayed-player-action filter regression. Next active slice: keep replacing personal-runtime combat result mirroring with shared Zone-owned combat/drop state.

> Latest retained Zone object sync: 2026-05-14 moved the simulation `ZoneRuntime` beyond pure packet relay for non-player world objects. `BroadcastPackets` now retains rebased `ObjectMonster`, `ObjectHero`, `ObjectNpc`, `ObjectItem`, `ObjectGold`, and `ObjectDeco` surfaces in Zone state, updates retained object position/death/zero-health/hidden/effect/poison/buff/name/NPC-image state from later packets, expires retained visible object Buffs on Zone tick, tombstones retained objects on `ObjectRemove` / `IntelligentCreaturePickup`, diffs retained-object AOI when players join or move, dispatches retained spawn/update/remove packets by object visibility instead of actor visibility, removes owner-generated retained objects when their owner leaves, expires retained item/gold drops on Zone tick using the Crystal ground-drop lifetime, suppresses/canonicalizes stale retained spawn packets so old personal-runtime snapshots cannot reinsert removed drops or revive dead objects, and keeps `ObjectRevived` markers authoritative over stale dead spawns. Verification passed focused retained-object regressions 16/16, Simulation shared_zone 55/55, Gateway shared_in_process 35/35, and Simulation/Gateway fmt/check. Next active slice: connect more combat/drop/NPC outcomes to this retained Zone world state instead of relying only on Gateway read-model mirroring.

> Latest shared entity-action observer sync: 2026-05-14 extended the same Gateway shared-entity observer bridge from movement packets into non-player action packets. Shared monsters/generated objects that emit `ObjectAttack`, `ObjectRangeAttack`, `ObjectMagic`, `ObjectProjectile`, or attacker-anchored `ObjectStruck` now have those packets queued to same-map observers when the actor/source exists in the shared map, without sending them through the player-action Zone rebasing path or duplicating player-origin combat packets. Target references to the current player's local self object id are rewritten to the authoritative Zone player object id before observer delivery, including same-batch `ObjectHealth` / `DamageIndicator` / `ObjectDied` / `ObjectPoisoned` / `AddBuff` / `RemoveBuff` / `PauseBuff` result packets when the shared actor struck the current player. Verification passed focused shared entity movement/action regressions 2/2 and Gateway shared_in_process 35/35. Next active slice: keep moving monster combat results and drop generation from personal-runtime outputs toward native shared Zone authority.

> Latest shared entity-movement observer sync: 2026-05-14 added Gateway observer delivery for shared monster/generated-object `ObjectTurn` / `ObjectWalk` / `ObjectRun` packets produced outside the normal player movement command path. The bridge now cheaply filters for real shared entity movement packets, reads the current map from shared Zone presence instead of taking an expensive personal `world_snapshot`, updates the shared map cache, and queues the movement packets to same-map observers without delaying player Run grace windows. Verification passed the focused entity-movement broadcast regression, focused Run timing regression, Gateway shared_in_process 34/34, Gateway shared_zone_state 36/36, and Simulation/Gateway fmt/check. Next active slice: keep moving monster AI movement/combat/drop generation toward native shared Zone authority.

> Latest shared drop despawn-expiry sync: 2026-05-14 completed Gateway shared-map despawn deadlines for ground drops, not just owner-window expiry. Shared drops now receive a Crystal-tick-derived lifetime when merged from personal snapshots, restored, or committed from death drops; Tick/KeepAlive expires due drops in the shared map, tombstones them, clears ownership/despawn indexes, returns `ObjectRemove` to the current session, and queues `ObjectRemove` to same-map observers. Verification passed focused shared drop expiry regressions 4/4, Gateway shared_zone_state 36/36, Gateway shared_in_process 33/33, and Simulation/Gateway fmt/check. Next active slice: continue replacing personal-runtime combat/drop generation with native shared Zone authority.

> Latest shared drop ownership-expiry sync: 2026-05-14 added a Gateway-side deadline for shared ground-drop owner windows. When shared drops are merged from personal snapshots or committed death drops, Gateway records an owner-window expiry derived from Crystal runtime ticks; manual pickup and IntelligentCreature auto pickup refresh expired ownership before enforcing owner/group rules, so stale shared drops do not remain owner-locked forever if no newer personal snapshot arrives. Verification passed focused manual/auto expiry regressions, Gateway shared_zone_state 35/35, Gateway shared_in_process 32/32, and Simulation/Gateway fmt/check. Next active slice: continue replacing personal-runtime combat/drop generation with native shared Zone authority, including full drop expiry/despawn ownership.

> Latest shared object-movement cache sync: 2026-05-14 tightened the Gateway shared map read model for ordinary Crystal object movement packets. `ObjectTurn`, `ObjectWalk`, and `ObjectRun` now update shared entity position/direction with the same dead-entity guard already used for push/backstep/dash transforms, so monster/summon/NPC-style objects moved by personal-runtime packet surfaces do not leave stale coordinates in the shared map cache before the next snapshot merge. Verification passed a focused shared-zone-state movement regression. Next active slice: keep replacing personal-runtime combat/drop/NPC side-effect generation with native shared Zone authority.

> Latest shared owned-generated lifecycle sync: 2026-05-14 closed a generated-entity cleanup gap in the shared Gateway map layer and runtime flow. `ObjectMonster.master_object_id` is now mapped back to the owning Zone player name when the master is an online shared player, including the real personal-runtime case where the emitted summon packet still uses the local self object id before Zone rebasing, and later snapshot merges preserve that owner instead of overwriting it with an ownerless stale entity row. When a player leaves or changes map, Gateway removes shared entities owned by that player from the old map read model, tombstones them, clears their dead/revive/harvest/drop anchors, and queues `ObjectRemove` packets to other players on the same map. Verification passed Gateway shared_zone_state 33/33, Gateway shared_in_process 32/32, a focused shared runtime disconnect regression for owner-generated `ObjectHero`, a local-master summon cleanup regression, an owner-preserving snapshot merge regression, and an owner map-change cleanup regression. Next active slice: keep replacing personal-runtime combat/drop/NPC side-effect generation with native shared Zone authority.

> Latest shared intelligent-creature pickup sync: 2026-05-14 closed a shared-drop fallback gap for manual and auto pet pickup. Gateway now treats `ClientPacket::IntelligentCreaturePickup` as a command-result fallback instead of waiting for the whole response buffer to be empty, so pending observer packets such as a prior `ObjectGold` cannot mask the actual pickup action. Tick-driven auto pickup also searches the shared map by Crystal range/ownership/filter/grade rules when the personal ECS has no local drop, and blocked filters restore the shared drop instead of tombstoning it. Both success paths award shared gold/items through the personal state layer, remove the shared drop, and broadcast `IntelligentCreaturePickup` to observers. Verification passed focused Gateway intelligent-creature coverage 6/6, Simulation shared_zone 38/38, Gateway shared_zone_state 29/29, Gateway shared_in_process 30/30, and Simulation/Gateway fmt/check. Next active slice: keep migrating combat/drop/NPC side effects from personal-runtime bridges into native shared Zone authority.

> Latest shared spawn/skill-target sync: 2026-05-14 broadened the Zone observer bridge from player action visuals into generated object and target-reference surfaces. Zone now forwards `ObjectHero`, `ObjectMonster`, `ObjectNpc`, NPC update/image packets, and `IntelligentCreaturePickup` through AOI, rebases summoned-monster `master_object_id`, and rewrites owner-local target references inside `ObjectRangeAttack`, `ObjectMagic.secondary_target_ids`, `ObjectProjectile.destination_id`, and self-target `ObjectStruck`. Gateway shared map state now treats `IntelligentCreaturePickup` as a drop-removal fact, materializes `ObjectMonster`/`ObjectHero`/`ObjectNpc` packets into the shared read model, preserves dead markers when late monster spawn packets arrive after death, and applies live object transform packets such as `ObjectPushed`/`ObjectBackStep` to shared entities without moving dead objects. Verification passed focused Simulation shared-zone regressions for pet pickup, spawned monster, hero/NPC spawn, magic/projectile target rebasing, and action-packet rebasing, plus focused Gateway shared-zone-state regressions for pet pickup removal, monster spawn, late dead-marker spawn, hero/NPC spawn, and shared object transform updates. Next active slice: continue migrating actual combat/drop/NPC side effects from personal-runtime bridge packets into native shared Zone authority.

> Latest shared dead-marker sync: 2026-05-13 added explicit out-of-order lifecycle markers in the Gateway map layer. `ObjectDied` / zero-health packets now preserve death state even if the entity snapshot is missing or arrives later, later stale snapshots are materialized as dead at the authoritative death location/direction, and death-drop commit can use an `ObjectDied` location without a prior live entity row. Out-of-order `ObjectRevived` and `ObjectHarvested` are also covered so later stale dead/corpse snapshots cannot undo revive or re-enable harvesting. Verification passed Gateway `shared_zone_state_` 23/23. Next active slice: keep pushing delayed combat/drop authority from personal runtime bridges into native shared Zone state.

> Latest shared delayed-damage sync: 2026-05-13 moved another combat-result edge through the shared Gateway/Zone bridge. Gateway Tick results now filter player-owned delayed damage bundles (`ObjectStruck` with the local player as attacker plus the matching target health/death/remove/drop surfaces) before Zone observer fanout, instead of dropping all delayed personal-runtime combat packets or forwarding unrelated monster AI Tick packets as player actions. Shared `ObjectHealth(percent=0)` also now marks an entity dead even when the snapshot lacks max HP, and a stable shared-runtime pair regression proves `Attack -> Tick -> observer drain` delivers rebased delayed `ObjectStruck/ObjectHealth` to the other session. Verification passed the focused delayed-damage filter regression, focused no-max-HP death regression, focused delayed two-runtime combat regression, Gateway `shared_zone_state_` 19/19, and Gateway `shared_in_process` 26/26. Next active slice: continue replacing personal-runtime combat/drop generation with native shared authority.

> Latest shared transform-cache sync: 2026-05-13 tightened the Gateway bridge between Zone `SaveTransform` outbounds and the shared read model. `SaveTransform` now immediately updates the shared player presence position/direction before any pending owner drain, and `world_snapshot()` overlays the local `SelfPlayer` from that authoritative Zone presence so cache/snapshot readers do not lag a command behind the Zone state. Verification passed the focused transform-cache regression, Gateway `shared_zone_state_` 18/18, and Gateway `shared_in_process` 25/25. Next active slice: continue replacing personal-runtime reconciliation with native shared authority for monster/combat/drop/NPC state.

> Latest shared viewport/transform sync: 2026-05-13 corrected two shared-world consistency edges. Gateway map sync is now additive for scene snapshots, so one session losing sight of a monster/drop no longer globally tombstones it; explicit `ObjectRemove`, shared pickup, and committed duplicate-drop guards remain the removal authorities. Death-drop anchors are now stored independently as monster/location records, so duplicate stale drops remain blocked even after the corpse entity is removed. Zone also now rebases and retains `TransformUpdate`, exposing the retained transform type in future `ObjectPlayer` packets. Verification passed Simulation `shared_zone` 35/35, Gateway `shared_zone_state_` 17/17, and Gateway `shared_in_process` 25/25. Next active slice: continue moving transform/lifecycle/cache correctness and gameplay authority deeper into shared state.

> Latest shared revive-state sync: 2026-05-13 added shared `ObjectRevived` handling to the Gateway map layer. Revive packets now clear shared dead, harvested-corpse, committed-death-drop, and remove-tombstone markers for that object, restore HP from max HP when available, and keep stale dead snapshots from re-killing the revived entity. Verification passed the focused Gateway revive/remove-tombstone regressions and Gateway `shared_zone_state_` 15/15. Next active slice: continue lifting remaining monster/corpse/drop lifecycle authority into shared state.

> Latest shared harvest-corpse sync: 2026-05-13 added a shared tombstone for harvested corpses in the Gateway shared map layer. `ObjectHarvested` now marks the shared monster corpse as harvested, stale snapshot syncs preserve that state, and later `Harvest` commands aimed at an already-harvested shared corpse are blocked before another personal session can re-run the harvest path. Verification passed the focused Gateway reharvest regression and Gateway `shared_zone_state_` 13/13. Next active slice: keep moving monster corpse/drop generation from personal-session execution into shared authority.

> Latest shared death-drop commit sync: 2026-05-13 tightened the Gateway shared map/drop authority after shared monster death. `ObjectDied` and `ObjectHealth(percent=0)` now anchor a one-time committed death-drop set per shared monster, copy newly produced personal-session drops into the shared map layer immediately, and tombstone later stale duplicate drops from other personal snapshots before they can reappear. Verification passed focused Gateway death-drop commit/spawn regressions 3/3 and Gateway `shared_zone_state_` 12/12. Next active slice: continue moving the remaining monster damage/drop generation itself out of personal-session reconciliation and into shared authority.

> Latest shared zero-health death sync: 2026-05-13 tightened shared monster death authority in the Gateway shared map layer. `ObjectHealth(percent=0)` now marks the shared entity dead, keeps HP at 0 across stale snapshot syncs, and blocks future shared actions even if an `ObjectDied` packet has not arrived yet. Verification passed focused Gateway zero-health regression and Gateway `shared_zone_state_` 10/10. Next active slice: continue moving the remaining damage/death/drop source-of-truth out of personal-session reconciliation.

> Latest shared late-join status retention sync: 2026-05-13 retained more Crystal player visual state inside `ZonePlayer` instead of only forwarding live packets. Zone now stores and exposes name colour, renamed display name, guild name, light, weapon, weapon effect, armour, poison, wing effect, mount type/riding state, fishing state, and level effects in future `ObjectPlayer` packets for late joiners/new AOI observers. Verification passed focused Simulation late-join visual-status retention and full Simulation `shared_zone` 35/35. Next active slice: continue the larger monster damage/death/drop source-of-truth migration.

> Latest shared late-status packet sync: 2026-05-13 broadened Zone observer rebasing for Crystal player status and late-system packets. Player-origin `PlayerUpdate`, `DamageIndicator`, `ObjectColourChanged`, `ObjectGuildNameChanged`, `ObjectLeveled`, `ObjectName`, `MagicDelay`, `PauseBuff`, `MountUpdate`, `FishingUpdate`, `ObjectTeleportOut`, `ObjectTeleportIn`, and `ObjectDeco` now carry the authoritative Zone player object id to observers instead of the personal-session local self id. Verification passed focused Simulation late-status observer coverage and full Simulation `shared_zone` 34/34. Next active slice: move retained late-status fields and monster damage/death/drop source-of-truth deeper into Zone/shared authority.

> Latest shared teleport/poison sync: 2026-05-13 closed another skill-authority gap in the Zone bridge. Player-origin `UserLocation` packets emitted by successful action packets, such as Teleport/Blink-style skill outcomes, now update the authoritative Zone transform before observer effects are rebased and fanned out; `ObjectPoisoned` is also rebased to the shared Zone player object id for AOI observers. Verification passed focused Simulation regressions for `UserLocation` action transform and poison rebasing, plus full Simulation `shared_zone` 33/33. Next active slice: continue migrating monster damage/death/drop resolution into Zone/shared authority.

> Latest shared skill-transform authority sync: 2026-05-13 moved Crystal movement-skill outcomes one step beyond visual fanout. Zone now extracts owner action transforms from successful movement-skill packets (`UserBackStep`, `UserDash`, `UserDashAttack`, `UserAttackMove`, `Pushed`, `ObjectBackStep`, `ObjectDash`, `ObjectDashAttack`, `ObjectPushed`, and correction/fail variants), applies the resulting position/direction to the authoritative `ZonePlayer`, updates occupancy, clears stale movement intent, emits `SaveTransform`, and rejects occupied/static target tiles with a `UserLocation` correction instead of broadcasting invalid movement. Verification passed focused Simulation success/reject transform regressions and full Simulation `shared_zone` 32/32. Next active slice: continue pulling damage/death/drop and monster state from personal-session resolution into Zone-owned authority.

> Latest shared skill-movement packet sync: 2026-05-13 widened Zone observer rebasing for Crystal movement-skill and special-skill state packets. Player-origin `ObjectBackStep`, `ObjectDash`, `ObjectDashFail`, `ObjectDashAttack`, `ObjectSitDown`, `SetConcentration`, `SetElemental`, `SetBindingShot`, `RemoveDelayedExplosion`, `ObjectSneaking`, and `ObjectLevelEffects` now rebase the personal-session self object id to the authoritative Zone player id before AOI delivery. Verification passed focused Simulation regressions for movement-skill packets and special skill state packets, plus full Simulation `shared_zone` 30/30. Next active slice: keep turning bridged skill/combat visuals into Zone-owned transform/damage/drop authority.

> Latest shared harvest packet sync: 2026-05-13 closed a Zone observer fanout hole for harvesting actions. `ClientPacket::Harvest` already enters the shared action broadcast path, and `ZoneRuntime` now rebases both `ObjectHarvest` and `ObjectHarvested` from the personal-session self object id to the authoritative Zone player object id while using the Zone position/direction for AOI observers. Verification passed the focused Simulation harvest observer regression and full Simulation `shared_zone` 28/28. Next active slice: keep moving monster damage/death/drop and harvest results from personal-session execution plus bridge fanout toward native shared Zone authority.

> Latest shared NPC/task/social-drop sync: 2026-05-13 hardened the shared multiplayer fallback around task-facing NPCs, group semantics, and stale monster damage reconciliation. Gateway now has a regression proving a sparse personal session can `CallNpc @Main` on the shared Village Guide snapshot and start the matching quest locally, `ShareQuest` packets are relayed through the shared in-process pending queue to online group members, shared drop owner windows now allow the owner's online group members instead of only the owner object id, shared monster `ObjectHealth` application is monotonic so stale personal-session damage packets cannot raise shared HP, and Gateway now applies shared monster snapshots back into the acting personal runtime before local Attack/RangeAttack/Magic/Harvest-style resolution including direction-only attack scans. Verification passed focused Gateway regressions, Gateway `shared_zone_state_` 9/9, Gateway `shared_in_process` 25/25, focused Simulation shared-monster snapshot application, and focused Gateway current-map shared-monster application. Next active slice: continue toward shared-native monster damage/death/drop resolution instead of personal-session resolution plus reconciliation.

> Latest shared drop ownership sync: 2026-05-13 preserved Crystal monster-drop ownership in the shared map/drop layer. `GroundDropSnapshot` now carries owner object id plus remaining ownership ticks, Gateway rebases the personal self object id to the authoritative Zone player object id while syncing drops, shared pickup refuses non-owners during the owner window without tombstoning the drop, and Zone observer fanout now includes `ObjectItem` / `ObjectGold` spawn packets. Verification passed focused Simulation drop-spawn observer fanout and Gateway shared-zone-state 7/7. Next active slice: continue from ownership-preserving shared drops toward shared-native monster damage/drop authority and NPC/task side effects.

> Latest shared player appearance sync: 2026-05-13 retained late-join appearance state for Zone players. Player-origin `ObjectHidden`, `ObjectHide`, `ObjectShow`, `ObjectDied`, `ObjectRevived`, and `ObjectEffect` packets now update `ZonePlayer` state after local-to-Zone object-id rebasing, and future `ObjectPlayer` packets expose the current hidden/dead/effect fields instead of always using the default live/visible/no-effect values. Verification passed Simulation `shared_zone` 25/25. Next active slice: continue native shared authority migration for monster damage/drop ownership and NPC/task side effects.

> Latest shared Buff expiry sync: 2026-05-13 added Zone-owned visible Buff expiry for the retained player Buff state. `BroadcastPackets` now carries `now_ms`, so `ZoneRuntime` converts Crystal relative `ClientBuff.expire_time` values into Zone-local expiry times, removes expired visible Buffs on `tick`, broadcasts `RemoveBuff` to current AOI observers, and prevents late joiners from receiving stale `ObjectPlayer` buff flags or `AddBuff` details. Verification passed Simulation `shared_zone` 24/24. Next active slice: continue replacing personal-session reconciliation with shared authority for monster damage, drop ownership, and NPC/task side effects.

> Latest shared Buff state sync: 2026-05-13 moved the Zone observer bridge one step closer to persistent shared authority. `ZonePlayer` now retains active visible self-buffs from rebased `AddBuff` / `RemoveBuff` packets, `ObjectPlayer` visibility packets include the active buff type list, and newly visible / late-joining players receive the matching rebased `AddBuff` details immediately after `ObjectPlayer`. Verification passed Simulation `shared_zone` 23/23. Next active slice: continue migrating from observer fanout state toward native Zone-owned skill/Buff duration, monster damage, drops, and NPC/task side effects.

> Latest shared skill/Buff visual sync: 2026-05-13 extended Zone observer packet rebasing beyond attack/projectile basics. `BroadcastPackets` now also rebases and fans out visible skill/Buff surfaces such as `ObjectMana`, `AddBuff`, `RemoveBuff`, `ObjectEffect`, `ObjectSpell`, `ObjectPushed`, `ObjectRevived`, hide/show, `SpellToggle`, and `MapEffect` where applicable, preserving the authoritative shared Zone player object id for observers. Verification passed Simulation `shared_zone` 22/22. Next active slice: keep moving from visual fanout/reconciliation toward shared-native gameplay authority for monster damage, drops, and NPC/task side effects.

> Latest shared NPC/task sync: 2026-05-13 closed the first shared-view NPC interaction gap. Crystal `ClientPacket::CallNpc` now routes into the existing NPC script/dialog path instead of no-oping, and Gateway shared sessions can fall back from a sparse personal ECS to a shared map NPC snapshot, materializing that NPC locally before running the same script/service interaction. This fixes the multiplayer case where a client could see an NPC from the shared map layer but clicking it returned nothing because the personal `SimulationSession` did not own that NPC entity. Verification passed Simulation `shared_zone` 21/21 and Gateway `shared_in_process_registry` 20/20. Next active slice: continue migrating NPC/task side effects and monster damage/drop ownership from personal-session reconciliation into native shared authority.

> Latest shared-authority sync: 2026-05-13 advanced the non-chat Zone migration with a bounded combat/skill observer bridge plus shared map entity state. `ZoneCommand::BroadcastPackets` now lets Gateway feed successful personal-session action packets into the shared `ZoneRuntime`; Zone rewrites player-origin `ObjectAttack`, `ObjectRangeAttack`, `ObjectMagic`, `ObjectProjectile`, and `ObjectStruck` packets from the local self object id to the shared Zone object id, then broadcasts those plus shared `ObjectHealth` / `ObjectDied` / `ObjectRemove` surfaces to AOI observers only. Gateway's shared map layer now applies `ObjectHealth`, `ObjectDied`, and `ObjectRemove` to the shared entity snapshot, preserves lower HP/dead state when another personal Session later syncs a stale monster snapshot, tombstones removed monsters, and rejects attacks against shared dead/removed targets before local execution. Verification passed Simulation `shared_zone` 20/20, Gateway shared entity state 5/5, Gateway `shared_in_process_registry` 20/20, and Rust fmt/check for Simulation/Gateway. Next active slice: continue lifting monster damage/drop ownership into a fuller Zone/shared combat authority instead of relying on personal Session resolution plus shared-state reconciliation.

> Latest chat parity sync: 2026-05-13 completed the next shared-Zone/player-chat slice. The active implementation now treats Crystal chat packets as structured payloads with linked `ChatItem`s, maps `ChatType` values 0-16, and routes production Gateway chat through Session preparation plus Zone delivery instead of a normal-only `ObjectChat` shortcut. Covered semantics: persisted chat ban, Crystal spam ban cadence, `@ADDSTORAGE`, case-insensitive one-link replacement, `NewChatItem`, normal AOI chat, whisper, group, guild, mentor, relationship, GM announcement, local shout, map shout, server shout, `$pos`, level-8 shout gate, 10s shout cooldown, one-shot map/server shout consumption, and frontend Mentor/Relationship log channels. Verification passed Protocol `chat_` 2/2, Simulation `shared_zone` 18/18, Simulation `chat_`, Gateway `chat_` 3/3, locked Protocol/Simulation/Gateway check, Rust fmt check, and Web `npm exec tsc -- --noEmit`. Next active slice: continue deeper non-chat shared authority and final human Crystal feel/visual acceptance.

> Latest architecture/backend sync: 2026-05-13 completed the Gateway integration, normal WebSocket/player command-boundary, and live two-client acceptance-smoke slice for the production-safe shared Zone MVP. `SharedInProcessZoneRuntimeFactory` now owns a real `ZoneManager`, maps active Gateway presences to Zone `SessionId`s, drains/queues cross-session `ZoneOutbound` packets, writes `SaveTransform` back into each personal `InProcessWorldRuntime`, joins Zone after StartGame/map bootstrap, routes Walk/Run/Turn/Chat through Zone, ticks pending movement intents on KeepAlive/Tick, and leaves Zone before LogOut/Disconnect saves. The WebSocket path can now enforce production player-command safety through production-like envs or `MIR2_GATEWAY_ENFORCE_PLAYER_COMMAND_SAFETY`, rejecting unauthenticated StartGame and blocking normal-client `MoveTo`, `Stage5Command`, and debug `crystal:<map>:<x>:<y>` transfers while allowing passkey login only after HMAC token verification. Gateway regressions now cover shared-zone ObjectWalk/ObjectRun/ObjectTurn/ObjectChat/ObjectRemove observer delivery, production WebSocket safety, and authoritative session-cache freshness while preserving shared drops, gold pickup, Trade escrow, and ItemRental flows. Live evidence now includes `docs/generated/load/two-client-zone-smoke-133316.json` with 2/2 WebSocket clients ready and the repeatable `npm run smoke:two-client-zone` browser harness at `docs/generated/player-qa/two-client-zone/two-client-zone-script-135930.json` with `ok=true`, both browser pages in game, A seeing B, B seeing A, B receiving A's movement broadcast, A receiving B's chat broadcast, no console errors, and no non-favicon 404s. Verification passed Gateway lib 121/121, shared registry 20/20, production WebSocket safety 3/3, Simulation shared Zone 12/12, Simulation security lifecycle 9/9, Simulation/Gateway fmt, locked Simulation/Gateway check, Gateway health, two-client WebSocket smoke, browser two-client smoke, `node --check apps/web/scripts/smoke-two-client-zone.mjs`, and the repeatable two-client Web smoke script. Next active slice: keep moving from Candidate toward broader human Crystal visual/feel acceptance.

> Latest architecture/backend sync: 2026-05-12 started the production-safe shared Zone MVP from the simulation side. Root `AGENTS.md` now records the hard boundary that personal `SimulationSession` handles login/bootstrap/personal state while shared `ZoneRuntime` owns online world state. Added the new `apps/simulation/src/runtime/zone/` module family plus integration tests under `apps/simulation/tests/`. The new tests cover two-player join visibility, ObjectWalk/ObjectRun/ObjectTurn/ObjectRemove observer packets, occupancy collision, Run intermediate collision, latest-intent consumption, stale-intent suppression, `SaveTransform`, session transform write-back, unsafe command rejection, and unique player object ids. Verification passed shared Zone 12/12, security lifecycle 9/9, Simulation fmt, and locked Simulation check. Next active slice: integrate Gateway production routing with this Zone layer without breaking the existing dev/QA command surface.

> Latest backend worker sync: 2026-05-11 completed a bounded Friend/blacklist Stage 5 state-flow slice. Crystal `PlayerObject.AddFriend(name, blocked)` stores friends and blocked players as one `FriendInfo` list and rejects an already-added target instead of moving it between normal friend and blacklist views. Stage 5 high-level `social.friend` / `social.block` now preserve that single-entry rule for modeled state, reject self-add/self-block with Crystal localization, and persist the resulting list across reload. Verification passed focused `social_economy_integration` 3/3, adjacent `stage5_social_group_guild_mail_persist_across_reload`, adjacent `mail_friend_packets_preserve_crystal_ack_surface`, `cargo +1.89.0 fmt --check -p mir2-simulation`, and `cargo +1.89.0 check --locked -p mir2-simulation` with the existing `CRYSTAL_STAT_SKILL_GAIN_MULTIPLIER` dead-code warning.

> Latest coordinator verification sync: 2026-05-11 completed the next 5.5 xhigh backend parity pass after the frontend input/NPC marker close. Server-side Hero learned-magic progression now advances Stage 5 `heroLearnedMagics` on successful keyed Hero AI spell use, emits Crystal-shaped `MagicLeveled` plus `MagicDelay` on level-up, persists the learned level/experience, and feeds progressed level back into Hero AI gate/damage/cooldown selection. Player skill progression was tightened for Crystal movement skills: `BackStep` now emits `MagicCast` and only levels on successful movement, `ShoulderDash` only levels after actual dash movement, and `FlashDash` only levels after finding a real hit target, avoiding failed-action practice gain. Mail claim deep semantics now preflight exact `item_states_json` parcel attachments as an all-or-nothing batch; full-bag failure leaves gold/items/mail unchanged, while success consumes the exact parcel payload and persists claimed state. Verification passed `magic_packet_crystal_` 73/73, Hero AI integration 28/28, focused Hero progression 2/2, Mail 9 unit + 2 integration, `cargo +1.89.0 fmt --check -p mir2-simulation`, `cargo +1.89.0 check --locked -p mir2-simulation`, and targeted `git diff --check`. Remaining next slices: broader Hero book/stat requirement exactness, Crystal skill-gain multiplier/mentor tuning, and wider late social/economy packet-perfect semantics.

> Latest coordinator verification sync: 2026-05-11 completed the Crystal input/NPC marker follow-up after the 5.5 xhigh worker pass. Player Web now recovers stale in-flight movement actions before the 1200ms responsiveness threshold by re-anchoring target plans to the authoritative server tile, clearing pending state, and retrying after the short Crystal correction delay; visible player position no longer uses unconfirmed pending target/direction tiles, while immediate facing remains responsive. Evidence: `docs/generated/player-qa/movement-jitter/r-click-target-crystal-input-final-090309.json`, `r-route-spam-obstacle-crystal-input-final-090355.json`, `r-blocked-target-crystal-input-final-090443.json`, and `r-input-queue-held-run-spam-click-crystal-input-final-090527.json` all record `ok=true`, `jumps=[]`, `logicalRollbackWarnings=[]`, `directionLagWarnings=[]`, `stalePredictionWarnings=[]`, `commandQueueWarnings=[]`, `pendingPlanAtEnd=null`, no console errors, and no non-favicon 404s. The held-run stress path now passes `movementCommandQueueResponsive` and `holdThenSpamClickTargetQueueStrict`. NPC clicking was re-smoked through `r-npc-click-marker-crystal-085224-summary.json`, and an isolated temporary account-store marker fixture `r-npc-click-marker-crystal-anchor-final-090830.json` verified Crystal source-math marker placement with `crystalLeftDeltaPx=0`, `crystalTopDeltaPx=0`, `dialogTitle=MirGuide_Peter`, and `interactCount=1`. Verification passed Web `pnpm --dir apps/web exec tsc --noEmit`, movement/NPC script syntax checks, live local Gateway/Web captures, isolated marker capture, and screenshot inspection. Remaining next slices: broader human feel acceptance on real manual play, deeper skill-system semantics, and late-system packet-perfect parity.

> Latest coordinator verification sync: 2026-05-11 reconciled the Hero learned-magic, Guild alliance runtime-persistence, and frontend movement-feel rollback work into one green acceptance pass. Player Web now keeps the local predicted anchor through the server-lag window for held-run plus repeated target-click transitions, treats same-tile confirmed prediction as settled, and avoids converting a still-in-flight sent source into a hard route correction. Evidence: `docs/generated/player-qa/movement-jitter/r-direction-lag-logical-rollback-0511-fix-bust-063119.json` records `ok=true`, `settle.status="settled"`, `pendingPlanAtEnd=null`, final player `{338,270}`, `predictedPlayer=null`, `jumps=[]`, `logicalRollbackWarnings=[]`, and `directionLagWarnings=[]`; `r-route-spam-obstacle-regression-063209.json` and `r-blocked-target-regression-063209.json` remain `ok=true` with explicit target-blocked non-failure status. Verification passed `cargo +1.89.0 fmt --check -p mir2-simulation -p mir2-gateway`, locked GameData/Protocol/Simulation/Gateway check, full locked `mir2-simulation` 856/856 plus Hero AI 26/26, focused Hero AI 26/26, focused `guild_` 16/16, Gateway `shared_in_process_registry` 15/15, Web `pnpm --dir apps/web exec tsc --noEmit`, movement/NPC script syntax checks, live movement captures, screenshot inspection, and targeted `git diff --check`. One live NPC click rerun against the long-running local Gateway did not find MirGuide at the historical `0:329,259` smoke coordinate and is treated as an environment/fixture follow-up, not a regression in this movement slice.

> Latest Hero learned-magic closure worker sync: 2026-05-11 completed the next bounded Hero learned book/key/save loop. Hero `UseItem` from `HeroInventory` now follows Crystal `HeroObject.UseItem` book semantics for the modeled path: a valid Crystal book creates a Stage 5 `heroLearnedMagics` row at level 0/key 0, emits `NewMagic(hero=true)`, and consumes the Hero-bag book. Hero `MagicKey` now mirrors Crystal `MirConnection.MagicKey` actor selection by routing `key > 16` or `oldKey > 16` to the Hero learned-magic list, clearing same-key collisions and persisting the assigned Shift-F key through the existing save path. The focused regression proves newly learned key-0 FireBall is not used by Hero AI until `MagicKey(key=17)` is saved, after which the Wizard Hero casts it through the existing AI path. Verification passed focused Hero loop 1/1, Hero AI 26/26, focused `hero_inventory` 15 lib tests plus the new integration filter, `cargo +1.89.0 fmt --check -p mir2-simulation`, and `cargo +1.89.0 check --locked -p mir2-simulation`. Remaining next slices: deeper Hero learned-magic level/experience progression and broader Hero book/stat requirement parity.

> Latest Guild alliance deep-surface sync: 2026-05-11 confirmed the current Crystal tree has no typed alliance packet/dialog surface beyond `GuildRankOptions.CanAlterAlliance`, runtime-only `GuildObject.AllyGuilds` / `AllyCount`, and `RequestGuildInfo` type 0/1 notice/member refreshes. Crystal `GuildInfo.Save/Load` does not persist alliance state, so Stage 5 alliance runtime fields now intentionally do not rehydrate from saved `stage5_systems_json` while keeping guild name, known guilds, permissions, and visible in-session RequestGuildInfo readback intact. Verification passed focused `guild_` 16/16, the new save/reload alliance regression, `cargo +1.89.0 fmt --check -p mir2-simulation`, and `cargo +1.89.0 check --locked -p mir2-simulation`. Remaining risk is deeper client UI/dialog presentation only if future Crystal source evidence adds a real alliance surface.

> Latest 5.5 xhigh multi-worker sync: 2026-05-10 completed the Hero learned-magic, Guild alliance visible-info, and NPC marker/click verification slice. Hero AI now reads saved Stage 5 `heroLearnedMagics` when present and applies Crystal `HeroObject.CanUseMagic`-style gates: only learned spells with `key > 0` are considered, learned spell level caps the Crystal manifest level gate, and an empty learned list preserves the old default behavior for existing fixtures. Guild `RequestGuildInfo` type 0/1 keeps the Crystal notice/member packets and appends Stage 5 guild-chat visibility for `AllyCount`, `AllyGuilds`, and recent alliance broadcasts because this Crystal tree has no typed alliance-info packet surface. NPC click automation now verifies both out-of-range approach-and-interact and adjacent direct-interact paths while measuring quest-marker anchor alignment over the NPC body. Evidence: `docs/generated/player-qa/npc-click/r-npc-click-marker-quest-0511a-summary.json` records `ok=true`, `dialogTitle=MirGuide_Peter`, out-of-range `moveCount=2` / `interactCount=1`, adjacent `moveCount=0` / `interactCount=1`, marker `horizontalDeltaPx=0`, no console errors, and no non-favicon 404s. Verification passed focused `guild_` 15/15, Hero AI 25/25, full locked `mir2-simulation` 855/855 plus Hero AI 25/25, Gateway `shared_in_process_registry` 15/15, Simulation/Gateway fmt, locked GameData/Protocol/Simulation/Gateway check, Web typecheck, NPC/movement script syntax, NPC marker evidence parse, and targeted diff checks. Remaining next slices: Hero book-learning/key progression feeding `heroLearnedMagics`, deeper Guild alliance UI/dialog parity if source evidence appears, broader Hero class exactness, and human Crystal visual/dialog/feel acceptance.

> Latest 5.5 xhigh multi-worker sync: 2026-05-10 completed the Guild alliance, Wizard Hero late-priority, and blocked-target client-feel slice. Guild Stage 5 now exposes Crystal-shaped `AllyGuilds` / `AllyCount` state through `guild.ally` / `guild.unally`, gates alliance mutation with the Crystal `CanAlterAlliance` permission bit, preserves known-guild canonicalization, rejects missing/self/active-war targets without mutation, and records alliance broadcasts without extending protocol where this Crystal tree has no typed alliance packet. Wizard Hero `ProcessAttack` now continues after `IceStorm` / `FireBang` through Crystal's late single-target order: low-level undead `TurnUndead`, then `FlameDisruptor`, `Vampirism`, `FrostCrunch`, then classic ThunderBolt/GreatFireBall/FireBall fallback, with Hero MP, `ObjectMana`, `ObjectMagic`, cooldown, delayed damage, Vampirism heal, and FrostCrunch freeze evidence. Player Web now has a `blockedTarget` movement harness and short-lived blocked-step memory so unreachable/blocked repeated clicks settle cleanly instead of endlessly resending stale vectors. Evidence: `docs/generated/player-qa/movement-jitter/r-blocked-target-nonfailure-0511-fixed6.json` records `ok=true`, `settle.status="settled"`, `movementPlan=null`, `predictedPlayer=null`, empty direction queue, `jumps=[]`, `routeSpamWarnings=[]`, `consoleErrors=[]`, and `nonFaviconNetwork404s=[]`. Verification passed focused `guild_` 14/14, Hero AI 23/23, full locked `mir2-simulation` 854/854 plus Hero AI 23/23, Gateway `shared_in_process_registry` 15/15, Simulation/Gateway fmt, locked four-package check, Web typecheck, movement script syntax, blocked-target evidence parse, and targeted diff checks. Remaining next slices: exact Hero learned-magic inventory/progression, any deeper Crystal alliance packet surface if discovered, broader class Hero AI tuning, and human Crystal visual/dialog/feel acceptance.

> Latest 5.5 xhigh multi-worker sync: 2026-05-10 completed the follow-up Guild war lifecycle, Wizard Hero attack-priority, and route-spam settle slice. Guild wars now carry Crystal-style duration state, at-war colour packet/log surfaces, Newbie-guild rejection without pre-registration, and timed war expiry that removes active war state and emits end-war guild chat plus normal colour restoration. Wizard Hero `ProcessAttack` now follows Crystal priority through `Repulsion`, `FlameField` / `ThunderStorm`, `IceStorm` / `FireBang`, then ThunderBolt/GreatFireBall/FireBall, with manifest level gates, Hero MP spend, `ObjectMana`, `ObjectMagic`, `ObjectPushed` for Repulsion, cooldown, and scheduled area damage. Player Web route-spam obstacle handling now rechecks early corrections after the reroute delay, updates the world ref synchronously on movement packets, and the movement harness records `settle` / `pendingPlanAtEnd` so clean runs prove final `movementPlan=null`, `predictedPlayer=null`, and empty direction queue. Verification passed focused `guild_` 12/12, Hero AI 20/20, full locked `mir2-simulation` 852/852 plus Hero AI 20/20, Gateway `shared_in_process_registry` 15/15, Simulation/Gateway fmt, locked four-package check, Web typecheck, movement script syntax, live `r-route-spam-obstacle-settle-followup5` capture with `settle.status="settled"`, `jumps=[]`, `routeSpamWarnings=[]`, `consoleErrors=[]`, and targeted diff checks. Remaining next slices: full Guild alliance semantics, later Wizard Hero branches (`TurnUndead`, `FlameDisruptor`, `Vampirism`, `FrostCrunch`), exact Hero learned-magic inventory/progression, and human Crystal visual/dialog/feel acceptance.

> Latest 5.5 xhigh multi-worker sync: 2026-05-10 completed the Guild war/territory, Wizard Hero support, and route-spam/obstacle input-feel slice. Guild now has Stage 5 known-guild and active-war state, `guild.requestWar` returns Crystal `GuildRequestWar` prompts with leader/no-guild checks, `GuildWarReturn` covers leader-only silence, missing/self/Newbie/already-war/funds rejection, war-cost bank deduction, `GuildStorageGoldChange` type 2, and duplicate/insufficient rollback, and guild territory page/purchase packets now expose Crystal-shaped listings and bank-gold purchase rollback. Wizard Heroes now run a Crystal-like `ProcessFriend` support priority for `MagicShield` before `MagicBooster`, with level gates, Hero MP spend, `ObjectMana`, self-target `ObjectMagic`, `AddBuff`, active-buff gating, cooldown, and missing-mana no-recast coverage. Player Web now remembers recently blocked route steps, reroutes instead of repeatedly sending the same stale vector, reanchors target movement from confirmed server positions, consumes dash/push movement packets through the same reconciliation path, and the movement harness covers `routeSpamObstacle` with richer packet/runtime diagnostics. Verification passed focused Simulation `guild_` 10/10, Hero AI integration 17/17, full locked `mir2-simulation` 850/850 plus Hero AI 17/17, Gateway `shared_in_process_registry` 15/15, Simulation/Gateway fmt, locked four-package check, Web typecheck, movement script syntax, live route-spam obstacle capture `r-route-spam-obstacle-final4` with `movementPlan=null`, `predictedPlayer=null`, `jumps=[]`, `routeSpamWarnings=[]`, and coordinator live captures with no jumps/warnings/errors. Remaining next slices: fuller Guild alliance/war broadcast lifecycle, Wizard Hero wider attack spell priority and learned-magic state, route-spam human feel acceptance, and final Crystal visual/dialog acceptance.

> Latest 5.5 xhigh multi-worker sync: 2026-05-10 completed the next bounded Guild/Hero/Input-feel slice after the Trade/Taoist/client-action pass. Guild now maps Crystal `GuildRankOptions` bits into Stage 5 permissions, gates notice edits, storage item store/retrieve/move, and guild-gold withdrawal by rank/permission/safe-zone rules, rejects Crystal `DontStore` and rental `DontStore` items, persists exact stored `ItemState` plus storing user id, and returns Crystal-shaped guild-storage list/change packets. Hero AI now gives Wizard Heroes a Crystal level-gated ranged spell chain for `FireBall` / `GreatFireBall` / `ThunderBolt`, spending Hero MP through `ObjectMana`, emitting `ObjectMagic`, enforcing cooldown, and scheduling monster damage. Player Web now preserves the predicted draw anchor through the Crystal 600ms direction visual window on same-tile server confirmation, and the movement harness uses direction-aware rollback detection. Verification passed focused Simulation `guild_` 5/5, `trade_` 12/12, Hero AI integration 13/13, full locked `mir2-simulation` 845/845 plus Hero AI 13/13, Simulation/Gateway fmt, locked four-package check for GameData/Protocol/Simulation/Gateway, Web typecheck, movement script syntax, and targeted diff checks. Remaining next slices: deeper guild war/alliance/territory semantics, Wizard Hero friend/self buffs and wider spell priorities, broader route-spam/obstacle input feel, and final human visual/dialog/feel acceptance.

> Latest coordinator verification sync: 2026-05-10 reconciled the TradeEscrow, HeroClassBreadth, and ClientActionFeel worker outputs under a single acceptance pass. Evidence now includes focused Simulation `trade_` 12/12, Gateway `shared_in_process_registry_` 15/15, Hero AI integration 11/11, full locked `mir2-simulation` 843/843 plus Hero AI 11/11, `cargo +1.89.0 fmt --check -p mir2-simulation -p mir2-gateway`, locked four-package check for GameData/Protocol/Simulation/Gateway, Web `pnpm --dir apps/web exec tsc --noEmit`, `node --check apps/web/scripts/capture-web-movement-jitter.mjs`, and targeted `git diff --check`. Next active slices should continue Guild rank/permission/storage depth, remaining Hero class AI breadth, and Crystal frame/input feel acceptance.

> Latest Worker TradeEscrow sync: 2026-05-10 completed the true two-account Trade escrow delivery/rollback backend slice. Trade deposit/confirm now rejects Crystal `DontTrade`, soulbound, rental-bound, rental-owned, rental-expiring, and rental-locked items before escrow lock; shared gateway settlement preflights both recipients' bag capacity before final delivery; full-bag settlement failures roll both locked offers back; partner cancel and disconnect preserve locked gold/items; and successful two-sided confirms still deliver real gold/items through the existing `TradeConfirm` / `GainedGold` / `GainedItem` packet surface. Verification passed focused locked simulation `trade_` tests 12/12, focused locked gateway `shared_in_process_registry_` tests 15/15 including Trade commit/cancel/disconnect/full-bag rollback, `cargo +1.89.0 fmt --check -p mir2-simulation -p mir2-gateway`, and `cargo +1.89.0 check --locked -p mir2-simulation -p mir2-gateway`. Remaining next slices are Crystal frame/input action queue feel, guild rank/permission/storage depth, Hero Wizard/Taoist breadth, and final human client feel acceptance.

> Latest Worker HeroClassBreadth sync: 2026-05-10 completed a bounded Taoist Hero owner-healing AI slice without touching `packets.rs`, `runtime/tests.rs`, gateway, or web files. Hero AI now evaluates Taoist support before monster attack selection, level-gates Crystal `Healing`, spends Hero MP through `ObjectMana`, emits Hero `ObjectMagic(Healing)`, restores the owner through `ObjectHealth`, and records a private heal cooldown so it does not recast on the next tick. Focused regressions prove a level-7 Taoist Hero heals the low-HP owner before melee attacking a nearby Scarecrow, while a level-6 Taoist Hero stays below the Crystal Healing gate. Verification passed Hero AI integration 11/11, `cargo +1.89.0 check --locked -p mir2-simulation`, and the later coordinator package fmt/check/full Simulation pass.

> Latest coordinator sync: 2026-05-10 closed the current 5.5 xhigh worker round and added a bounded Crystal packet action-timing gate. Simulation now tracks packet-side action readiness for `Walk`, `Run`, `Attack`, `RangeAttack`, and `Magic`; repeated packets before the Crystal-style movement/attack/spell delay now return `UserLocation` and do not enqueue duplicate action packets, while pre-start packet behavior still preserves the old silent/empty rejection surface. This sync also reconciles Worker Hero-Class-AI and Worker Mail-Parcel outputs: Archer Hero AI now covers `Concentration` / `StraightShot` level gates, Hero MP spending, `ObjectMana`, `SetConcentration`, and ranged `StraightShot` damage; Mail parcels now preserve exact attached `ItemState`, opened/locked flags, remote account-store delivery, and exact claim packets. Verification passed focused action-timing regressions, `magic_packet_crystal_` 73/73, `packet_` 280/280, Hero AI integration 9/9, Mail regressions 9/9, full locked `mir2-simulation` 841/841 plus Hero AI 9/9, `cargo +1.89.0 fmt --check -p mir2-simulation`, locked four-package check for GameData/Protocol/Simulation/Gateway, and targeted diff checks. Remaining next slices: Crystal frame/input action queue alignment beyond packet gates, true two-account Trade escrow delivery/rollback, guild rank/permission/storage depth, Hero Wizard/Taoist breadth, and final human client feel acceptance.

> Latest Worker Mail-Parcel sync: 2026-05-10 completed a bounded Crystal mail parcel fidelity slice. Client `SendMail` now accepts protocol attachment unique IDs, preserves attached `ItemState` payloads in Stage 5 mail, removes sender gold/items only after recipient/item/cost validation, delivers remote character mail through account-store Stage 5 state, exposes parcel previews plus opened/locked mail flags in `ReceiveMail`, and lets recipients claim exact serialized item state with `GainedItem` / `ParcelCollected` evidence. Rejection coverage proves insufficient-gold sends preserve sender gold, item, and mailbox state, while sender-side blacklist rejection remains first-priority. Verification passed focused `cargo +1.89.0 test --locked -p mir2-simulation mail_ -- --test-threads=1` (9/9), `cargo +1.89.0 check --locked -p mir2-simulation`, and later coordinator reconciliation passed package `cargo +1.89.0 fmt --check -p mir2-simulation`.

> Latest Worker Hero-Class-AI sync: 2026-05-10 added a bounded non-Warrior Hero class AI slice for Archer. Hero AI now keeps private Archer skill state in `hero_ai.rs`, level-gates Crystal `Concentration` and `StraightShot`, spends Hero MP through the existing Hero `PlayerVitals` / `ObjectMana` surface, emits `SetConcentration` once while the modeled buff window is active, and tags gated ranged Hero attacks with `StraightShot` plus Crystal magic-level damage scaling. Verification passed Hero AI integration 9/9, locked `cargo +1.89.0 check --locked -p mir2-simulation`, and later coordinator reconciliation passed package `cargo +1.89.0 fmt --check -p mir2-simulation`.

> Latest Worker Agility sync: 2026-05-10 completed the Crystal monster Agility code path. `generate-crystal-respawn-manifest.mjs` now preserves monster stat 11 as `agility` and projects it into respawn rows as `monster_agility`; game-data accepts both fields with backward-compatible defaults; runtime Crystal spawn/import paths now attach `MonsterCombatStats { agility }` for spawn-table monsters, current-map visible imports, respawns, and dynamic Crystal template spawns. Verification passed focused production-spawn regression `magic_packet_crystal_imported_agility_drives_melee_hit_roll`, game-data manifest load tests, JS syntax check, Rust fmt, and locked GameData/Simulation check. This Mac workspace has no `Crystal/Build/Server/Debug/Server.MirDB`, so checked-in generated manifests were not refreshed from source here; the next Windows data refresh should materialize live nonzero Agility values.

> Latest Hero deep follow-up: 2026-05-10 completed a bounded Warrior Hero skill slice beyond carried-item projection. Hero AI now level-gates modeled Warrior `Slaying` and `FlamingSword` from Crystal magic level requirements, tags Hero melee `ObjectAttack` packets with the matching spell/level, applies Slaying's Crystal passive DC bonus, and applies FlamingSword burst scaling to scheduled Hero monster damage while preserving Archer ranged and carried-equipment behavior. Verification passed Hero AI integration 7/7, `cargo +1.89.0 fmt --check -p mir2-simulation`, and `cargo +1.89.0 check --locked -p mir2-simulation`. Remaining Hero depth is full Hero magic inventory/learning, mana/cooldown persistence, wider class-specific Hero skills, and human client visual/feel acceptance.

> Latest 5.5 xhigh closure: 2026-05-10 completed and verified the follow-up slice that was still open after the previous skill/Hero/Fishing pass. Local reconciliation now mirrors Crystal's passive accuracy math for `Fencing`, `Slaying`, and `SpiritSword`, applies equipment `Accuracy`, resolves player-vs-monster hit rolls against modeled monster `Agility`, emits Crystal miss `DamageIndicator` packets, advances `Fencing` / `SpiritSword` progression on melee hits, and updates `MPEater` count/MP recovery from the Crystal accuracy formula. Parallel workers closed Hero equipment/stat projection into Hero AI damage plus `HeroInformation`, Fishing slot-item fidelity for bait/hook/float/finder/reel durability and autocast gates, and Market underbid rejection plus 5% `SoldItemEarningsCommission` net settlement. Verification passed focused passive accuracy 1/1, focused `magic_packet_crystal_` 73/73, Fishing 11/11, Market 1/1, Auction 6/6, Hero AI integration 5/5, full locked `mir2-simulation` 836/836 plus Hero AI 5/5, `cargo +1.89.0 fmt --check -p mir2-simulation`, locked `cargo +1.89.0 check --locked -p mir2-game-data -p mir2-protocol -p mir2-simulation -p mir2-gateway`, and targeted `git diff --check`. Remaining queue after this closure: populate imported monster Agility broadly from Crystal data, deepen guild permissions/storage/notice/member semantics, expand Hero equipment/skill semantics beyond carried-item projection, harden full multi-account market/mail settlement, and final human Crystal visual/feel acceptance.

> Latest 5.5 xhigh continuation: 2026-05-10 completed the next multi-agent reconciliation slice across skills, Hero, Fishing, and social economy. Runtime now classifies every generated Crystal manifest spell explicitly (`unmatched manifest spells: 0`), and the player attack path has focused coverage for `Thrusting`, `FlamingSword`, `Slaying`, `Focus`, incoming-hit `CounterAttack`, `FatalSword`, `MPEater`, `Hemorrhage`, and `Meditation` packet/state surfaces. Hero AI now has bounded Attack/Follow/CounterAttack targeting plus melee/ranged attack packet evidence; Fishing now uses Crystal drop/event resolution with reel miss/no-space/gold/event spawn paths; Mail now blocks sends to sender-side blacklisted friends with the Crystal system message and no gold/mail mutation. Verification passed focused `magic_packet_crystal_` tests 72/72, Hero AI 3/3, Fishing 7/7, blacklist mail 1/1, full locked `mir2-simulation` 831/831 plus integration Hero AI 3/3, `cargo +1.89.0 fmt --check -p mir2-simulation`, targeted `git diff --check`, and locked `cargo +1.89.0 check --locked -p mir2-simulation`. Remaining queue after this slice: exact stat/hit-rate passive math (`Fencing`, `SpiritSword`), Hero equipment/stat projection, embedded Fishing slot-item fidelity, market/guild settlement depth, and final human Crystal visual/feel acceptance.

> Latest 8-hour parity sync: 2026-05-10 in progress. This round closed additional Hero/late-system/skill deep semantics on top of the 2026-05-08 slice: Crystal `NoHero` map flags are now imported into game-data/config and enforced when transferring into no-hero maps, `NewHero`/`ChangeHero` on no-hero maps keep the Hero unsummoned and emit Crystal system feedback, Hero inventory `TransferHeroItem` / `TakeBackHeroItem` / `UseItem` now moves, persists, and consumes Hero-bag potion payloads, and Hero auto-pot item settings now normalize invalid item indexes and consume matching Hero inventory potions like Crystal. Shared in-process ItemRental now has a real two-player flow plus lifecycle return coverage: adjacent request queues the borrower-side `Renting=true` invite, borrower fee lock and lender item lock are paired, confirmation transfers rental gold to the lender, delivers a rental-metadata item to the borrower, records lender `GetRentedItems`, partner cancel rolls both sides back, expired borrower items are deleted and mailed back to the owner with exact `ItemState` metadata, and dead-player ticks return unexpired rental items before normal drop paths. Skill parity now covers the player `SpellToggle` gates (`FlamingSword`, `CounterAttack`, `MentalState`), Crystal repulsion-family pushes (`Repulsion`, `EnergyRepulsor`, `FireBurst`) with `ObjectPushed` and ThunderElement repulsion damage, `StormEscape` teleport/effect/TemporalFlux plus nearby damage, and Crystal archer deep semantics for `Concentration`, `ElementalShot`, `ElementalBarrier`, `StraightShot`, `DoubleShot`, `BackStep`, `BindingShot`, `VampireShot`, `PoisonShot`, `CrippleShot`, `NapalmShot`, `DelayedExplosion`, and `Trap`: Concentration emits `AddBuff` type 15 and `SetConcentration` on/off, MentalState cycles Crystal buff type 19 values and applies archer shot damage penalties, ElementalShot/ElementalBarrier gather and spend Crystal element-orb state with `SetElemental` packets before applying orb-boosted damage or buff type 25, StraightShot/DoubleShot queue one/two delayed ranged hits, ExplosiveTrap spawns front-row trap objects and detonates on contact, PoisonSword consumes poison and marks the frontal arc, BackStep relocates opposite facing and emits `UserBackStep` / `ObjectBackStep` including blocked distance-0 reporting, BindingShot roots the center 3x3 monster group and queues `SetBindingShot`, VampireShot queues delayed damage/heal with visible buff type 16, PoisonShot queues delayed target damage plus Green poison with visible buff type 17, CrippleShot consumes the active special-arrow buff and queues `RemoveBuff` before the follow-up, NapalmShot now hits the target-centered Crystal area instead of the caster square, DelayedExplosion marks/removes the delayed marker and explodes in the target area, and Trap roots lower-level monsters while spawning a Trap `ObjectSpell`. Wizard line/area coverage now includes HellFire forward/level-3 side lanes, FireBang/IceStorm target 3x3, Blizzard/MeteorStrike 5x5 ground spell spawn plus persistent damage, FireBounce chain projectiles/damage, MeteorShower primary plus secondary target damage, ThunderBolt undead bonus damage, ElectricShock lower-level shock root, FlameDisruptor non-undead bonus damage, and IceThrust three-column delayed damage plus Frozen poison packet state. The Taoist slice now models MassHealing delayed 3x3 friendly healing, HealingCircle delayed `ObjectSpell` plus the Crystal 25-point heal tick, Curse amulet consumption with delayed hostile-area buff type 12 stat-rate penalties, Purification delayed `RemoveBuff` for Curse debuffs, Revelation delayed target `ObjectHealth(expire)` reveal packets, Poisoning equipped Green/Red poison consumption with delayed `ObjectPoisoned` and Green poison monster ticks, PoisonCloud amulet/GreenPoison consumption with 3x3 ground cloud ticks, Plague amulet/optional-poison consumption with 3x3 debuff/damage, and TrapHexagon amulet consumption with 3x3 hostile root plus eight delayed ring `ObjectSpell` packets. LightBody/MoonLight/DarkBody/Hiding/MassHiding now model Crystal buff types 8/13/14/2, Agility payloads, visible or hidden stealth buffs, and `ObjectHidden` hide/reveal lifecycle; FrostCrunch queues delayed magic damage plus a target freeze buff/root window, Vampirism queues delayed damage plus player healing, TurnUndead only damages undead targets with level-gated instant-kill behavior, EnergyShield applies Crystal buff type 20 with HP-gain/shield-percent stats, ImmortalSkin applies buff type 23 with defence/stat-tradeoff payloads, PetEnhancer buffs friendly/summoned monsters, LionRoar paralyses nearby lower-level monsters with `LRParalysis`, and BattleCry forces nearby hostile monsters to reacquire the caster. Verification passed focused Simulation ItemRental expiry/death/mail tests, Stage5 mail attachment regressions, focused Simulation Hero 25/25, focused Simulation casting 13/13, SpellToggle 6/6, magic-packet Crystal skill tests 54/54 after adding Hiding/FrostCrunch/Vampirism/TurnUndead, EnergyShield/ImmortalSkin/PetEnhancer/LionRoar/BattleCry, MentalState/NapalmShot/DelayedExplosion/Trap/ExplosiveTrap/PoisonSword/PoisonCloud/Plague, and HellFire/FireBang/IceStorm/Blizzard/MeteorStrike/FireBounce/MeteorShower/ThunderBolt/ElectricShock/FlameDisruptor/IceThrust on top of FireWall/Lightning/ThunderStorm, focused Gateway shared registry 13/13 including ItemRental commit/rollback, Rust fmt, and locked four-package `cargo +1.89.0 check --locked -p mir2-game-data -p mir2-protocol -p mir2-simulation -p mir2-gateway`. Remaining queue items are broader per-profession skill semantics beyond the covered archer/Taoist/Wizard/stealth/control packet slices, exact Hero combat/equipment AI, and human Crystal visual/feel acceptance.
> Active follow-up: `ShoulderDash`, `FlashDash`, and `SlashingBurst` motion skills plus ground/line magic (`FireWall`, `Lightning`, `ThunderStorm`, `HellFire`, `FireBang`, `IceStorm`, `Blizzard`, `MeteorStrike`, `FireBounce`, `MeteorShower`, `FlameDisruptor`, `IceThrust`) are now covered by Crystal packet/state surfaces and focused regressions. The latest bespoke skill follow-up also covers `Hiding`, `MassHiding`, `FrostCrunch`, `Vampirism`, `TurnUndead`, `EnergyShield`, `ImmortalSkin`, `PetEnhancer`, `LionRoar`, `BattleCry`, `MentalState`, `NapalmShot`, `DelayedExplosion`, `Trap`, `ExplosiveTrap`, `PoisonSword`, `PoisonCloud`, and `Plague`; next backend skill targets should move to the remaining profession-specific bespoke skills.

> Latest skill/Hero deep-semantic sync: 2026-05-08 completed. The current worker round closed the next concrete items under "skill system" and "late gameplay deep semantics": Protocol/Gateway/Web now preserve Crystal Hero owner names, Simulation turns Hero create/change/recruit into a visible follow-capable `ObjectHero` entity with health and snapshot state, Hero spawn-state now reports Crystal `Summoned=2`, Hero default `SpellToggle` packets target the spawned Hero, and Hero auto-pot settings now round-trip through `HeroInformation`, `SetAutoPotValue`, and `SetAutoPotItem`. Targeted projectile spells now emit `ObjectProjectile` packets for the modeled FireBall/GreatFireBall/ThunderBolt/SoulFireBall path, and MagicBooster now applies Crystal buff type 21 with MC and mana-penalty stats. Verification passed focused Protocol Hero codec coverage, Simulation Hero 18/18, SpellToggle 2/2, MagicBooster 1/1, focused projectile skill coverage, locked Protocol/Simulation/Gateway fmt/check, and Web typecheck. Next remaining queue items are exact Hero combat/equipment AI, fuller per-spell profession fidelity, and final human visual/feel acceptance rather than missing Hero display/projectile/auto-pot packet plumbing.

> Latest Stage 5 full-smoke hardening sync: 2026-05-08 completed. The live Player Web smoke now defaults to a fresh throwaway account so human `demo/Scout` acceptance state is not polluted; dirty/reused demo-save coverage remains available only with explicit `MIR2_STAGE5_ACCOUNT_MODE=demo`. The script self-seeds missing red/blue potions through real Gateway commands, restores stored items through the real InnKeeper_Brittney storage service, verifies inventory split/use/drop/take-back by exact `uniqueId`, verifies ground pickup by exact `objectId`, includes belt plus all bag containers when checking picked-up consumables, and uses object-id fallback when the ground marker is outside the current clickable viewport. Backend support now normalizes dirty item unique IDs/known potion metadata and covers `qa.giveItem` red-potion usability with a focused regression. Verification passed Web `node --check scripts/smoke-stage5-ui.mjs`, Web `npx tsc --noEmit`, focused Simulation `stage5_qa_give_item_seeds_usable_healing_metadata` 1/1, focused Simulation `unique_id` 13/13, locked `cargo +1.89.0 check --locked -p mir2-simulation -p mir2-gateway`, and a full live local Gateway/Web Stage 5 UI smoke capturing 114 screenshots with `criticalConsoleErrorCount=0`, `compactMatrixCount=3`, `systemMenuSocial=44`, `storageTakeBackFlow=4`, `inventorySplitFlow=3`, `groundPickupFlow=3`, and `groundGoldPickupFlow=3`.

> Latest late-dialog frontend command sync: 2026-05-08 completed. Player Web System Menu now exposes actionable Hero and Item Rental late-system panels in addition to the existing Creature/Mount/Fishing and social panels. Creature summon/dismiss/release, Mount ride use, Fishing cast/autocast, Hero create/behaviour/change, ItemRental request/fee/period/cancel/list, Mentor, Marriage/Relationship, Trade, Market, Group, Guild, and Friend actions now dispatch real Gateway browser commands or Stage 5 commands instead of inert UI buttons. Simulation snapshots also expose live `stage5Systems.itemRental` state derived from `ItemRentalResource`, including active partner, fee, period, deposited item, lock state, and rented-record rows, so the Item Rental panel can observe runtime state. Verification passed Web `node --check scripts/smoke-stage5-ui.mjs`, Web `npx tsc --noEmit`, a live local Gateway/Web fast Stage 5 smoke with 22 screenshots (`systemMenuFeature=10`, `systemMenuSocial=44`, `systemMenuQaTransfer=3`), focused Simulation `item_rental_` 3/3, locked `cargo +1.89.0 check --locked -p mir2-simulation -p mir2-gateway`, and focused Gateway browser-command mapping 7/7. Remaining late-dialog work is deeper pixel/interaction acceptance and production-grade multi-account rental expiry/borrower return semantics, not missing Player Web buttons for these packet families.

> Latest frontend 2/4/5/6 closure sync: 2026-05-07 completed. The player client now applies live Crystal combat/magic/effect packets (`Magic`, `MagicCast`, `MagicDelay`, `MagicLeveled`, `ObjectMagic`, `ObjectProjectile`, `MapEffect`, `AddBuff`, `RemoveBuff`, `PauseBuff`) to Web visual state and HUD skill/buff state instead of depending only on snapshots; attack/struck/death visual windows use Crystal-like timing. Late-system UI now has a real `trade` chat filter, dynamic System Menu social panels for ranking/friend/group/guild/trade/market/marriage/mentor/relationship, and supported social/trade/market actions dispatch through Stage 5 commands. NPC/quest smoke now drives InnKeeper_Brittney through the real Crystal dialog path without `qa.openStorage` fallback, strips raw script markup from visible dialog text, exposes quest/dialog state for assertions, and verifies Quest Diary detail rows. Responsive smoke now covers compact viewports 900x640, 768x640, and 820x540, with overflow-safe text coverage for mail/storage/system/social/quest surfaces and repo-stable screenshot output. Verification passed Web `npx tsc --noEmit`, smoke script syntax, and a full live isolated-Gateway Stage 5 UI smoke capturing 113 screenshots with `criticalConsoleErrorCount=0`, `compactMatrixCount=3`, `systemMenuSocial=36`, `npcDialogFlow=11`, and `combatFlow=2`. Remaining frontend status is Candidate: human Crystal visual/feel acceptance and full per-skill bitmap/effect fidelity remain open.

> Latest typed-observability sync: 2026-05-07 completed. Gateway/Web packet events now expose newly typed Crystal server packets as structured JSON payloads instead of a Debug-only summary, and packet trace display names use typed enum names for server IDs that previously surfaced as `Raw` through the legacy static-name fallback. The protocol trace model now stores packet names as owned strings so `NewMapInfo`, rankings, guild/map/status, and other newly typed payload families remain readable in generated traces. Game-data regressions also now lock the current Crystal NPC script command surface at `81/81` command names and `7,044/7,044` occurrences implemented, and the generated monster AI summary at `remaining_runtime_priorities=[]`. Verification passed: focused Protocol trace, Gateway Web event, and GameData Crystal-summary regressions; `cargo +1.89.0 fmt --check -p mir2-protocol -p mir2-game-data -p mir2-gateway -p mir2-simulation`; `git diff --check`; locked check for Protocol/GameData/Gateway/Simulation; and full locked tests covering GameData 27/27, Gateway lib 105/105 plus packet-trace bin 17/17, Protocol lib 33/33 plus codec 33/33, and Simulation 722/722.

> Latest full server-packet typed sync: 2026-05-07 completed. Crystal server packet payload coverage is now explicit for all `ServerPacketId` values `0..278`: the remaining 58 Raw decode branches were replaced with typed Rust variants and round-trip tests for map metadata/world map setup/search results/user slot refresh, chat linked item stats, player update/inspect/status/damage/death/poison/map-change surfaces, guild status/member/notice/storage/war packets, auto-pot, NPC image/input/pearl goods, quest inventory, reincarnation, dash/attack-move/concentration/elemental packets, awakening materials, transform, game-shop stock, rankings, notices, and guild territory pages. The local protocol scan reports `explicit=279 remaining=0`; `ServerPacket::Raw` remains an encode escape hatch, but no known Crystal server packet now silently decodes as Raw. Verification passed: `cargo +1.89.0 fmt --check -p mir2-protocol -p mir2-gateway`, locked `cargo +1.89.0 check --locked -p mir2-protocol -p mir2-gateway -p mir2-simulation`, focused Protocol tests 32/32 plus codec 33/33, and full locked Protocol/Gateway/Simulation regression covering Gateway lib 104/104 plus packet-trace bin 17/17, Protocol lib 32/32 plus codec 33/33, and Simulation 722/722. Remaining work has moved from server packet typing to exact gameplay semantics, client dialog/visual acceptance, and production-grade late-system edges.

> Latest P1/P2 packet-runtime sync: 2026-05-07 completed. The next Crystal parity slice is now landed and verified: typed Group utility, Quest, and Refine server packets are exposed through Protocol, packet trace names, and Gateway Web browser events; Simulation now drives Crystal-shaped stateful behavior for group invite/member/toggle packets, quest accept/finish/abandon/share, Stage 5 market consign/buy/get-back/sell-now paths, refine deposit/retrieve/cancel/start/check, `OpenDoor`, and `RequestMapInfo` / `RequestMonsterInfo` / `RequestNpcInfo` from the generated Crystal manifests. Frontend System Menu social panels also replaced visible Web/QA placeholder language with player-facing group/guild/mentor/ranking surfaces. Verification passed: focused Protocol/Gateway/Simulation regressions for the new packet/runtime paths, `cargo +1.89.0 fmt --check -p mir2-protocol -p mir2-simulation -p mir2-gateway`, locked `cargo +1.89.0 check --locked -p mir2-protocol -p mir2-simulation -p mir2-gateway`, Web `npx tsc --noEmit`, live fast Stage 5 UI smoke with 17 screenshots, and full locked three-package regression covering Gateway lib 103/103 plus packet-trace bin 17/17, Protocol lib 29/29 plus codec 32/32, and Simulation 722/722. Remaining depth is exact NPC market page/list payload fidelity, full Crystal refine probability/timer/ore economics, market bids/commission/mail settlement, exact Quest Diary client-dialog acceptance, and human visual/feel acceptance.

> Latest P1/P2 exact-gate sync: 2026-05-07 completed. Supervisor reconciled the multi-agent handoff and closed the next concrete backend gaps instead of leaving them as broad TODOs: Gateway/Web raw server events and `packet_trace` now expose copyable `packetName` / `packetId` / `payloadLength` / `payloadHex` fields for Raw and raw-payload server packets; IntelligentCreature now imports Crystal default rule profiles, applies mouse/semi-auto/manual pickup mode gates, item category and grade filters, and keeps blackstone production progressing independently of pickup fullness; Fishing now requires an equipped Crystal fishing rod, bait, hook flag, reel flag for autocast, valid fishing cell attribute, rod durability damage, reel loot, and autocast bait/durability consumption; Mount now honors map `NoMount`, `NeedBridle`, saddle, and reins gates, with the respawn-manifest generator and game-data model preserving Crystal `NoMount` / `NeedBridle` flags on the next data refresh. Frontend System Menu also no longer exposes placeholder text for creature/mount/fishing panels; it renders Crystal-style static shells and the original scene sprite loader now avoids 404 requests for Crystal libraries that were not exported into `public/original-ui`. Verification passed: `cargo +1.89.0 fmt --check -p mir2-protocol -p mir2-game-data -p mir2-simulation -p mir2-gateway`, `git diff --check`, locked `cargo +1.89.0 check --locked -p mir2-protocol -p mir2-game-data -p mir2-simulation -p mir2-gateway`, Web `npx tsc --noEmit`, Node syntax checks for the respawn manifest generator plus Stage 5 smoke script, live Stage 5 UI smoke against local Gateway/Web with 83 screenshots and 0 critical console errors, focused regressions for Protocol payload hex, Simulation fishing/mount/intelligent-creature, Gateway raw Web/packet-trace payloads, and full `CARGO_CACHE_AUTO_CLEAN_FREQUENCY=never cargo +1.89.0 test --locked -p mir2-protocol -p mir2-game-data -p mir2-simulation -p mir2-gateway -- --test-threads=1` covering GameData 27/27, Gateway lib 100/100 plus packet-trace bin 17/17, Protocol lib 26/26 plus codec 32/32, and Simulation 716/716. Remaining P1/P2 work after this sync is no longer these missing gates; it is full human visual acceptance, exact fishing rod-slot stat tuning beyond the modeled hook/reel flags, deeper hero combat/equipment AI, and remaining cross-account late-system production semantics.

> Latest multi-agent gameplay closure sync: 2026-05-07 completed. Supervisor split the requested late-system closure across Simulation, Gateway/Web, verification, and docs workers, then reverified locally. The current modeled backend now covers shared two-account Trade item/gold commit plus partner cancel/disconnect rollback, IntelligentCreature tick-based automatic pickup/fullness decay/blackstone progress, Fishing tick/reel/autocast loot, equipped Mount use toggling, Hero create/change/behaviour state surfaces, and Gateway BrowserCommand/packet-trace detail for the new paths. Verification passed: `cargo +1.89.0 fmt --check -p mir2-protocol -p mir2-simulation -p mir2-gateway`, `cargo +1.89.0 check --locked -p mir2-protocol -p mir2-simulation -p mir2-gateway`, focused Gateway `use_item_with_unique_id_maps_to_packet`, and full `CARGO_CACHE_AUTO_CLEAN_FREQUENCY=never cargo +1.89.0 test --locked -p mir2-protocol -p mir2-simulation -p mir2-gateway -- --test-threads=1` covering Gateway lib 99/99 plus packet-trace bin 16/16, Protocol lib 25/25 plus codec 32/32, and Simulation 711/711. This supersedes the 2026-05-06 Trade/IntelligentCreature remaining-depth notes for delivery/rollback, fullness, blackstone, and automatic pickup; remaining work is exact Crystal UI/dialog human acceptance plus deeper per-system tuning such as exact creature item-category filters/visual movement, full hero equipment/combat AI, fishing rod/bait/durability rates, and mount source/visual ride physics.

> Latest IntelligentCreature stateful protocol sync: 2026-05-06 completed. IntelligentCreature is no longer an always-empty update surface for the modeled backend path: `UpdateIntelligentCreature` now creates or updates persisted Stage 5 creature rows, supports summon/unsummon/release state, emits `NewIntelligentCreature` for first registration, and returns `UpdateIntelligentCreatureList` with `creatureSummoned` / `summonedCreatureType`; `RequestIntelligentCreatureUpdates` reads that state; `IntelligentCreaturePickup` can now use an active creature to collect a targeted ground drop and emits `IntelligentCreaturePickup` plus the normal `GainedGold` / `GainedItem` payload. Verification passed: focused `intelligent_creature_packets_update_state_and_pick_up_ground_gold`, `cargo +1.89.0 fmt --check -p mir2-protocol -p mir2-simulation -p mir2-gateway`, `cargo +1.89.0 check --locked -p mir2-protocol -p mir2-simulation -p mir2-gateway`, and full `CARGO_CACHE_AUTO_CLEAN_FREQUENCY=never cargo +1.89.0 test --locked -p mir2-protocol -p mir2-simulation -p mir2-gateway -- --test-threads=1` covering Gateway lib 96/96 plus packet-trace bin 16/16, Protocol lib 25/25 plus codec 32/32, and Simulation 708/708. Remaining IntelligentCreature depth is Crystal fullness decay/food, blackstone production timers, automatic/semi-automatic pickup scanning, item-category filter fidelity, pet visuals/AI movement, and final client dialog acceptance.

> Latest Trade stateful protocol sync: 2026-05-06 completed. Trade is no longer only a no-partner no-op surface for the modeled backend path: adjacent shared Gateway sessions can now resolve the remote player name for `TradeRequest`, Simulation starts a Stage 5 trade session, `TradeReply` emits `TradeAccept`, `TradeGold` records and echoes the offered amount, `DepositTradeItem` / `RetrieveTradeItem` maintain trade slots and emit `TradeItem`, `TradeConfirm` locks/completes the offer while deducting gold and removing offered inventory items, and `TradeCancel` clears active trade state with Crystal-shaped `TradeCancel`. Verification passed: focused Simulation `trade_packets` 2/2, adjacent Stage 5 trade command tests 3/3, focused Gateway shared trade request test 1/1, `cargo +1.89.0 fmt --check -p mir2-protocol -p mir2-simulation -p mir2-gateway`, `cargo +1.89.0 check --locked -p mir2-protocol -p mir2-simulation -p mir2-gateway`, and full `CARGO_CACHE_AUTO_CLEAN_FREQUENCY=never cargo +1.89.0 test --locked -p mir2-protocol -p mir2-simulation -p mir2-gateway -- --test-threads=1` covering Gateway lib 96/96 plus packet-trace bin 16/16, Protocol lib 25/25 plus codec 32/32, and Simulation 708/708. Remaining Trade depth is true two-account item/gold exchange delivery to the partner session, rollback on partner disconnect after both sides offer, and final client dialog acceptance.

> Latest Mail/Friend stateful protocol sync: 2026-05-06 completed. The late-system Mail/Friend slice is no longer only an empty/bounded ack surface: `SendMail`, `ReadMail`, `CollectParcel`, `DeleteMail`, `LockMail`, and `MailCost` now route through Stage 5 mailbox state and emit Crystal packet surfaces (`LoseGold`, `MailSent`, `ReceiveMail`, `GainedGold`, `ParcelCollected`) with persisted mail rows, delivery cost, gold parcel collection, deletion filtering, and failure acks for unsupported attachments or invalid/insufficient-gold sends. Friend packets now also use Stage 5 social state: `AddFriend`, `RemoveFriend`, `RefreshFriends`, and `AddMemo` mutate/read persisted friend/block/memo lists and return `FriendUpdate` with `ClientFriend` rows instead of always-empty results. Verification passed: focused `mail_friend_packets_preserve_crystal_ack_surface`, adjacent `stage5_social_group_guild_mail_persist_across_reload` and `stage5_credit_shop_mails_purchase_and_claim_transfers_attachment`, `cargo +1.89.0 fmt --check -p mir2-protocol -p mir2-simulation -p mir2-gateway`, `cargo +1.89.0 check --locked -p mir2-protocol -p mir2-simulation -p mir2-gateway`, and full `CARGO_CACHE_AUTO_CLEAN_FREQUENCY=never cargo +1.89.0 test --locked -p mir2-protocol -p mir2-simulation -p mir2-gateway -- --test-threads=1` covering Gateway lib 95/95 plus packet-trace bin 16/16, Protocol lib 25/25 plus codec 32/32, and Simulation 707/707. Remaining Mail/Friend depth is exact Crystal attachment transfer from live bag item ids, persistent lock/reply state, multi-character online notification behavior, and final client-dialog acceptance.

> Latest full-protocol coverage sync: 2026-05-06 completed. Crystal packet coverage is now locked at the table level: all 153 Crystal client packet IDs `0..152` are known and represented by typed `ClientPacket` variants, and all 279 Crystal server packet IDs `0..278` are known with typed coverage where implemented plus Raw-safe fallback for known-but-not-yet-typed payloads. This also fixes two packet-ID parity hazards: client `CombineItem` is Crystal ID `110` (with `AwakeningNeedMaterials=111`), and server `CombineItem` is Crystal ID `214` with `ItemUpgraded=215`. The typed server surface was expanded again for projectile/range/push/dash/observe/buff-pause/hidden/map-effect visuals and late magic/awakening/inventory packets: `ObjectProjectile`, `RangeAttack`, `Pushed`, `ObjectPushed`, `MapEffect`, `AllowObserve`, `PauseBuff`, `ObjectHidden`, `UserDash`, `ObjectDash`, `UserDashFail`, `ObjectDashFail`, `RemoveDelayedExplosion`, `ObjectDeco`, `ObjectSneaking`, `ObjectLevelEffects`, `SetBindingShot`, `SendOutputMessage`, `NPCAwakening`, `NPCDisassemble`, `NPCDowngrade`, `NPCReset`, `AwakeningLockedItem`, `Awakening`, and `ResizeInventory`. Gateway Web event serialization and packet trace names cover the new variants. Verification passed: focused protocol regressions for full ID coverage/Raw fallback and the new server visual/late packets, `cargo +1.89.0 fmt --check -p mir2-protocol -p mir2-simulation -p mir2-gateway`, `cargo +1.89.0 check --locked -p mir2-protocol -p mir2-simulation -p mir2-gateway`, and full `CARGO_CACHE_AUTO_CLEAN_FREQUENCY=never cargo +1.89.0 test --locked -p mir2-protocol -p mir2-simulation -p mir2-gateway -- --test-threads=1` covering Gateway lib 95/95 plus packet-trace bin 16/16, Protocol lib 25/25 plus codec 32/32, and Simulation 707/707. Remaining protocol depth is payload semantics for still-Raw server packets such as complex guild/status/listing/ranking/hero-info payloads, not missing packet IDs.

> Latest gameplay magic/buff parity sync: 2026-05-06 completed. Crystal client/server magic and buff coverage now includes `MagicKey`/`Magic`/`SpellToggle` client packets plus `NewMagic`/`RemoveMagic`/`MagicLeveled`/`Magic`/`MagicDelay`/`MagicCast`/`ObjectMagic`/`SpellToggle`/`ObjectMana`/`AddBuff`/`RemoveBuff` server packets with Crystal IDs and round-trip codec coverage. Simulation routes real `ClientPacket::Magic` through Crystal spell lookup, returns `UserLocation` on invalid/no-cast like Crystal, emits MP/magic/buff packets on successful casts, persists magic hotkeys/level/experience/delay in skill snapshots, acknowledges `SpellToggle`, teaches Crystal books through `NewMagic`, drains potion MP through `ObjectMana`, removes expired buffs through `RemoveBuff`, and can execute manifest-backed Crystal spell effects for target damage, teleport, MagicShield, and Fury-style buffs before full per-spell fidelity is complete. Gateway Web admin/session commands and packet trace can now send and inspect these real Crystal magic/buff surfaces. Verification passed: `cargo +1.89.0 fmt -p mir2-protocol -p mir2-simulation -p mir2-gateway`, `cargo +1.89.0 check --locked -p mir2-protocol -p mir2-simulation -p mir2-gateway`, Player Web `npx tsc --noEmit`, focused Protocol/Simulation/Gateway magic/buff regressions, packet-trace flow-name coverage, `git diff --check`, and full `cargo +1.89.0 test --locked -p mir2-protocol -p mir2-simulation -p mir2-gateway -- --test-threads=1` covering Gateway lib 82/82 plus packet-trace bin 16/16, Protocol lib 5/5 plus codec 32/32, and Simulation 698/698.

> Latest admin-console parity sync: 2026-05-06 completed. Crystal `SMain` / account / player / market / guild / NameLists / database-editor operations now have Admin coverage instead of remaining WinForms-only: Admin API exposes audited `/admin/commands/console` commands for account create/update/delete/unban/storage-password clear, character rename/stat/currency/location/vital/PK edits, chat ban apply/clear, safe-zone return, kill player, kill pets, NPC flag set/clear, direct GM message, world broadcast, market listing cancel/expire/delete, guild member/message moderation, NameLists create/add/remove/delete, content override bundle publish, and server control; Gateway exposes `/admin/sessions` plus `/admin/control`; Admin Web adds Console, Accounts, Market, Guilds, NameLists, Content, and player-detail editor/flag/chat-ban surfaces. Simulation persistence now carries Crystal PK/chat-ban fields, chat packets honor active bans, and Stage 5 auction listings carry Crystal-style `expired` state. Verification passed: `cargo +1.89.0 fmt --check -p mir2-simulation -p mir2-admin-api -p mir2-gateway`, `cargo +1.89.0 check --locked -p mir2-simulation -p mir2-admin-api -p mir2-gateway`, full `mir2-simulation` 692/692, full `mir2-admin-api` 33/33 lib tests plus 6/6 outbox bin tests, focused Gateway admin endpoint test, Admin Web `npm run typecheck`, Admin Web `npm run build`, live HTTP smoke against temp Gateway/Admin API/Admin Web on `17110/17420/13020` covering PK/chat-ban/market-expire/market-delete/NameLists-create-delete/content/server-control mutations and readback, SSR page probes, and Playwright page snapshots for Market, NameLists, and player detail.

> Latest product-evolution sync: 2026-05-06 expanded the production architecture observability and gate slice. The prior boundary work split `apps/simulation/src/runtime.rs` into `apps/simulation/src/runtime/`, exposed `WorldRuntime` / `WorldCommand` / `InProcessWorldRuntime`, opened gateway sessions through `ZoneRegistry`, and added shared in-process zone state, route leases, gameplay command outcomes, Redpanda/Pandaproxy publishing, ClickHouse `gameplay_events`, Admin API `/admin/gameplay-events`, and `AccountStoreRepository` adapters. This continuation adds Admin API `/admin/gameplay-events/summary` for command-volume, lag, and readiness alerts with `windowSeconds`, `limit`, `zoneId`, `commandKind`, `maxLagSeconds`, and `minEvents` filters; Admin Web dashboard now surfaces that summary as command-stream readiness with command volume, lag, latest event time, alert messages, and top commands; `infra/check-architecture-gates.sh` now repeats the runtime/routing/session-cache/event/schema/repository/Admin Web/Compose/diff gates; `infra/check-candidate-gate.sh` now provides local/full/live 100% Candidate command bundles; `.github/workflows/mir2-candidate-gate.yml` wires the local Candidate gate into CI; and Gateway has a schema compatibility regression that locks `GatewayGameplayEvent` JSON fields to the ClickHouse Kafka/materialized-view columns. Architecture completion is tracked separately from Crystal parity and is now **93%** in `docs/ARCHITECTURE-IMPLEMENTATION-STATUS.md`. Verification passed this continuation: focused `mir2-admin-api` ClickHouse gameplay event tests 4/4, `mir2-admin-api` gameplay-event summary/readiness tests 4/4, full `mir2-admin-api` tests 37/37 total across lib/bin targets, `cargo +1.89.0 fmt --check -p mir2-admin-api`, `cargo +1.89.0 check --locked -p mir2-admin-api`, Admin Web `npm run typecheck`, Admin Web `npm run build`, Playwright dashboard smoke screenshots at `output/playwright/admin-dashboard-gameplay-events.png` and `output/playwright/admin-dashboard-gameplay-readiness-degraded.png`, the full `bash infra/check-architecture-gates.sh` gate including `mir2-gateway` shared registry 7/7, session-cache/Redis/lease 14/14, gameplay-event/schema 4/4, Gateway `/health` boundary 1/1, `mir2-admin-api` gameplay-event/readiness 6/6, `mir2-simulation` repository 1/1, Docker Compose config, Admin Web typecheck, and `git diff --check`, plus `MIR2_CANDIDATE_SCOPE=local bash infra/check-candidate-gate.sh` covering architecture gate, `mir2-game-data` 27/27, `packet_trace` bin 16/16, Player Web typecheck, and diff check. The earlier 2026-05-05 architecture slice verification remains green: `mir2-gateway` lib 77/77, `mir2-gateway` packet-trace bin 16/16, `mir2-simulation` config slice 16/16, full `mir2-simulation` lib 689/689, `cargo +1.89.0 check --locked -p mir2-gateway -p mir2-admin-api -p mir2-simulation`, and Docker Compose config. Remaining architecture work is promoting combat mutation, AI ticks, remote drop pickup inventory gain, NPC services, AOI deltas, cross-zone route-transfer RPC handoff, normalized gameplay repositories beyond account store, external notification/incident delivery for alerts, reconnect soak, and expanding CI to full/live scheduled evidence refreshes.

> Latest runtime/frontend comparison sync: 2026-05-01-R327 completed. The user-requested Gameshop Buy and map-click arrival paths now have end-to-end evidence. Web Gameshop cells pass their Crystal `gameShopIndex` through the Buy button, expose account credit in page state, and send `gameShop.buyCredit` / `gameShop.buyGold`; the runtime resolves those commands against the generated Crystal game-shop manifest, deducts credit/gold, and delivers credit purchases through Stage 5 mail. QA browser evidence uses `QA0429A / QA0429Hero`: `docs/generated/player-qa/r327-gameshop-buy-click-final-clean-state.json` records `gameShop.visible=true`, `firstCellName=AccuracyPotion`, command `gameShop.buyCredit` with args `20,1`, expected zero-credit rejection, `network404Count=0`, and `consoleErrorCount=0`. Map click-to-arrive now waits for pending self movement packet confirmation before sending the next target step, reconciles `ObjectRun` / `ObjectWalk` for the player immediately, and removes the 180ms movement-time tick flood that delayed queued `moveTo` behind monster updates. Evidence: `docs/generated/player-qa/movement-jitter/r327-map-click-target-arrival-fixed3.json` records right-click target `338,270`, final player `338,270`, `movementPlan=null`, four run `moveTo` commands, and `jumps=[]`; gateway move log confirms `MoveTo` through `338,270`. Verification passed: web `tsc --noEmit`, capture-script syntax checks, focused `mir2-simulation` game-shop credit delivery test, `cargo +1.89.0 check --locked -p mir2-gateway`, and targeted CDP captures. `NPC/25` was exported from Crystal client data to remove the prior resource 404.

> Latest runtime/frontend comparison sync: 2026-04-30-R319 completed. The latest user-reported label/cursor/BigMap/Mail mismatches now have a source-aligned frontend pass. Web entity nameplates no longer append selected HP/action helper text into the object name label, and NPC/monster underscore names render as Crystal stacked labels centered on the object (`Teleport` / `Gilbert`, `BorderVillage` / `Board`). BigMap NPC rows now come from the Crystal NPC-info manifest for the whole map, use exported `MapLinkIcon` frames, and format names like `(Teleport)Gilbert`; Mail empty state no longer displays Web `No mail`; and the stage/NPC/monster/text cursors use Crystal `.CUR` files. Evidence: `docs/generated/player-qa/r319-label-bigmap-mail-cursor/r319-label-bigmap-mail-cursor-final.png` and `docs/generated/player-qa/r319-label-bigmap-mail-cursor/r319-label-bigmap-mail-cursor-final-state.json`, recording `mailPanel.emptyVisible=false`, `bigMap.npcRowCount=18`, `bigMap.npcRows[0].text=(Teleport)Gilbert`, `bigMap.npcRows[0].icon=/original-ui/MapLinkIcon/120.png`, Crystal cursor URLs for stage/NPC/monster hits, `nonFaviconNetwork404s=[]`, and `consoleErrors=[]`. Verification passed: UI asset export, web `tsc --noEmit`, capture script `node --check`, and focused CDP capture with `--openMail true --openBigMap true`. Remaining comparison queue: exact BigMap movement/selected-NPC icon interactions, service-backed Gameshop buy/preview behavior, exact Quest Diary and Storage dialog bitmap/layout parity, character paperdoll base sprite hair/body details, dynamic animal placement, lighting/effect feel, and human visual acceptance.

> Latest runtime/frontend comparison sync: 2026-04-30-R318 completed. The user-reported BigMap and Mail UI mismatch is now covered by a Crystal source-aligned frontend pass. The minimap BigMap button opens a real `BigMapDialog` instead of expanding the small minimap, using exported `Title/820`, original close/scroll/search/world/my-location/teleport sprites, the `MapInformation.bigMapIndex` raster, coordinate label, NPC rows, and radar dots. The Mail button opens the Crystal `MailListDialog` frame (`Title/670`) at `562,5,312,444`, with `Title/7`, original close/help/page/action buttons, 10-row layout, row icons/flags, and no visible Web overlay header. Evidence: `docs/generated/player-qa/r318-mail-bigmap/r318-mail-bigmap-final.png` and `docs/generated/player-qa/r318-mail-bigmap/r318-mail-bigmap-final-state.json`, recording `mailPanel.bounds=562,5,312,444`, `mailPanel.hasFrame=true`, `mailPanel.visibleOverlayHead=false`, `mailPanel.oldOverlayRowCount=0`, `bigMap.bounds=132,134,760,500`, `bigMap.viewport=146,186,568,380`, `bigMap.hasFrame=true`, `bigMap.hasRaster=true`, `bigMap.title=BichonProvince`, `bigMap.coordinate=[ 287, 618 ]`, `nonFaviconNetwork404s=[]`, and `consoleErrors=[]`. Verification passed: web `tsc --noEmit`, capture/smoke script `node --check`, focused CDP capture with `--openMail true --openBigMap true`, UI asset export, and `git diff --check`. Remaining comparison queue: service-backed Gameshop buy/preview behavior, exact Quest Diary and Storage dialog bitmap/layout parity, character paperdoll base sprite hair/body details, dynamic animal placement, lighting/effect feel, and human visual acceptance.

> Latest runtime/frontend comparison sync: 2026-04-30-R317 completed. Continued the user-reported Gameshop 1:1 work beyond the R316 shell fix: the Web Gameshop no longer uses placeholder product cells. It now renders the generated Crystal `crystal_game_shop_packet_manifest` product list through an app-local generated data module, joins each product to Crystal item icon/type metadata, exports the required original assets (`Title/750`, `Title/778-783`, and 58 Gameshop `Items` icon indices), and lays out item cells at Crystal `MirGameShopCell` coordinates. The dialog shows real category filters, class tabs, search, `1 / 14` pagination for 105 products, original quantity/page controls, stock/count/credit/gold labels, gold/credit payment checkbox state, and buy/preview button sprites. Evidence: `docs/generated/player-qa/r317-gameshop-products/r317-gameshop-products.png` and `docs/generated/player-qa/r317-gameshop-products/r317-gameshop-products-state.json`, recording `gameShop.bounds=164,70,696,476`, `cellCount=8`, `firstCellName=AccuracyPotion`, `pageLabel=1 / 14`, `categoryCount=10`, `loadedIconCount=8`, `buyButtonCount=8`, `previewButtonCount=1`, `oldPlaceholderCellCount=0`, `inventoryVisible=false`, `nonFaviconNetwork404s=[]`, and `consoleErrors=[]`. Verification passed: web `tsc --noEmit`, capture-script `node --check`, focused CDP capture with `--openGameShop true`, UI asset export, and `git diff --check`. Remaining comparison queue: service-backed Gameshop buy/preview behavior, exact Quest Diary and Storage dialog bitmap/layout parity, character paperdoll base sprite hair/body details, dynamic animal placement, lighting/effect feel, and human visual acceptance.

> Latest runtime/frontend comparison sync: 2026-04-30-R316 completed. The user-reported Gameshop/Menu mismatch was traced to Web HUD/UI wiring: Gameshop was still calling `onOpenInventoryTab("quest")`, and Menu rendered a large Web QA/debug transfer panel instead of Crystal `MenuDialog`. Crystal source confirms `GameShopButton.Click` toggles `GameShopDialog` and `MenuButton.Click` toggles `MenuDialog` (`Title` index 567 with 13 icon buttons). Web now toggles a Crystal-framed `GameShopDialog` shell from the Gameshop HUD button without opening Inventory, renders the Menu as the exported 36x282 `Title/567` vertical icon strip with original sprite triples at Crystal offsets, and keeps QA transfer controls offscreen for automation only. Missing UI assets were exported from Crystal for Gameshop/Menu frames, tabs, buttons, scroll controls, payment checkboxes, and menu icons. Evidence: `docs/generated/player-qa/r316-gameshop-menu/r316-gameshop-open.png`, `docs/generated/player-qa/r316-gameshop-menu/r316-menu-open.png`, and `docs/generated/player-qa/r316-gameshop-menu/r316-gameshop-menu-state.json`, recording `shopVisible=true`, `inventoryVisible=false`, `shopBounds=164,70,696,476`, `menuBounds=988,349,36,282`, `iconCount=13`, `oldOverlayHeadVisible=false`, `nonFaviconNetwork404s=[]`, and `consoleErrors=[]`. Verification passed: web `tsc --noEmit`, capture-script `node --check`, focused CDP click capture, and `git diff --check`. Remaining comparison queue: Gameshop real product data/buy interaction, exact Quest Diary and Storage dialog bitmap/layout parity, character paperdoll base sprite hair/body details, dynamic animal placement, lighting/effect feel, and human visual acceptance.

> Latest runtime/frontend comparison sync: 2026-04-30-R315 completed. The user-reported character/inventory/spells/quest/storage mismatch was traced to Web demo seed state, not only panel CSS. Crystal source confirms new `CharacterInfo` equipment, inventory, quest inventory, magic list, and account storage start empty, with account gold defaulting to 0 unless `StartItems` are configured. Runtime now creates real `NewCharacter` saves with Crystal-empty bag/belt/storage/equipment/quest/skill state and gold 0, treats empty save arrays as explicit empty instead of silently refilling Web seed items, migrates old level-1 exact Web seed saves to empty Crystal state, and preserves the default `demo/Scout` Stage 5 seed state for existing automation. Frontend character spells no longer backfill empty magic rows with Web hints/buffs, and the web-only Character repair/special-repair buttons were removed from the character page. R315 evidence for `QA0429A / QA0429Hero` at Bichon `0:287,618` records `playerHp=18`, `playerMaxHp=18`, `playerMp=14`, `gold=0`, `inventoryItemCount=0`, `beltItemCount=0`, `storageItemCount=0`, `equipmentItemCount=0`, `questCount=0`, `skillCount=0`, `hudHealthOnlyLabel="HP 18/18"`, `nonFaviconNetwork404s=[]`, and `consoleErrors=[]`. Evidence: `docs/generated/player-qa/r315-empty-new-character-panels/r315-empty-new-character-panels.png` and `docs/generated/player-qa/r315-empty-new-character-panels/r315-empty-new-character-panels-state.json`. Verification passed: focused `mir2-simulation start_game_` 16/16, `cargo +1.89.0 build --locked -p mir2-gateway`, web `tsc --noEmit`, R315 capture, `cargo +1.89.0 fmt --check`, and `node --check` for the capture script. Remaining comparison queue: exact Quest Diary and Storage dialog bitmap/layout parity, character paperdoll base sprite hair/body details, dynamic animal placement, lighting/effect feel, and human visual acceptance.

> Latest runtime/frontend comparison sync: 2026-04-30-R314 completed. The user-reported HUD/text/hotbar mismatch now has a source-aligned pass: Web uses Crystal low-level Warrior HP-only `MainDialog` behavior with `Prguse` frame 6 and shows `HP 18/18` for the level-1 `QA0429Hero`; chat uses the Crystal 4-row/13px/Arial-style feed with white/blue/red row backgrounds; the belt uses `Prguse` 1932 plus the 0.5-opacity 1933 overlay. Backend default and legacy hardcoded `120/120/45` save vitals now derive from Crystal `BaseStats` formulas, so R314 evidence for `QA0429A / QA0429Hero` at Bichon `0:287,618` records `playerHp=18`, `playerMaxHp=18`, `playerMp=14`, `hudHealthOnlyLabel="HP 18/18"`, exact stage/HUD/minimap/chat bounds, `visibleChatLines` count 4, `nonFaviconNetwork404s=[]`, and `consoleErrors=[]`. Evidence: `docs/generated/player-qa/r314-crystal-vitals-hud/r314-bichon-287-618-vitals-hud.png` and `docs/generated/player-qa/r314-crystal-vitals-hud/r314-bichon-287-618-vitals-hud-state.json`. Verification passed: focused `mir2-simulation start_game_` 15/15, `cargo +1.89.0 build --locked -p mir2-gateway`, web `tsc --noEmit`, R314 capture, `cargo +1.89.0 fmt --check`, and `git diff --check`. Remaining comparison queue: exact dynamic animal density/placement, lighting/effect feel, and human visual acceptance.

> Latest runtime/frontend comparison sync: 2026-04-30-R312 completed. Reconciled the Bichon same-scene projection work against Crystal source instead of keeping the R311 playfield-centered camera experiment: Web restores Crystal `MapControl.OffSetY = Settings.ScreenHeight / 2 / CellHeight - 1`, keeps floor/object map layers on the source `drawX = ... * 48 - OffSetX` path, and places entity sprites/nameplates/health bars from Crystal `DrawLocation` / `DisplayRectangle` anchors. Evidence at `docs/generated/player-qa/r312-entity-crystal-anchor/r312-bichon-287-618-entity-anchor-state.json` records `QA0429A / QA0429Hero` at `BichonProvince` map `0`, `287,618`, self nameplate `top=275`, exact stage/HUD/minimap/chat bounds, `questMarkerCount=0`, `nonFaviconNetwork404s=[]`, and `consoleErrors=[]`; screenshot: `docs/generated/player-qa/r312-entity-crystal-anchor/r312-bichon-287-618-entity-anchor.png`. R311's Crystal bitmap HP/MP orb fill remains in place. Remaining comparison queue: exact dynamic animal density/placement, lighting/effect feel, chat/HUD text feel, and human visual acceptance.

> Latest runtime/frontend comparison sync: 2026-04-30-R311 completed. The Web aligned Bichon camera now centers the map view on Crystal's playable area above the 152px HUD rather than the full 768px client frame, moving `QA0429Hero` from the R310 web nameplate `top=389` to `top=325` at `BichonProvince` map `0`, `287,618`. The main HUD HP/MP orb fill now uses exported Crystal `Prguse` frame 4 bitmap slices instead of CSS gradients, with `Prguse` frames 4/6 added to the UI export manifest. Evidence: `docs/generated/player-qa/r311-playfield-camera/r311-bichon-287-618-playfield-camera.png`, `docs/generated/player-qa/r311-playfield-camera/r311-bichon-287-618-hud-orb.png`, and `docs/generated/player-qa/r311-playfield-camera/r311-bichon-287-618-hud-orb-state.json` with exact stage/HUD bounds, `questMarkerCount=0`, `nonFaviconNetwork404s=[]`, and `consoleErrors=[]`. Remaining comparison queue: exact dynamic animal density/placement, lighting/effect feel, chat/HUD text feel, and human visual acceptance.

> Latest runtime/frontend comparison sync: 2026-04-29-R310 completed/monitoring. R310 fixed the Web login-success transition leaking over the game scene by clearing the login overlay once `screen=game`, scoped NPC quest icons to server-provided `questIds`, and added repeatable visual-watch tooling: `apps/web/scripts/capture-crystal-parity.mjs` for Web same-scene captures plus `apps/web/scripts/r310-visual-watch.ps1` for original/Web long-run sampling. Evidence: `docs/generated/player-qa/r310-visual-watch/r310-final-web-scene-state.json` records `QA0429A / QA0429Hero` at Bichon `0:287,618` with `transitionOverlayVisible=false`, `questMarkerCount=0`, exact `1024x768` stage/HUD bounds, `nonFaviconNetwork404s=[]`, and `consoleErrors=[]`; screenshot `docs/generated/player-qa/r310-visual-watch/r310-final-web-scene.png`. One-sample watch evidence wrote `watch-20260429-042013-original.png`, `watch-20260429-042013-web.png`, and `r310-visual-watch-log.jsonl` with no errors. Remaining comparison queue: exact dynamic animal density/placement, light/effect feel, and human visual acceptance.

> Latest runtime/frontend comparison sync: 2026-04-29-R309 completed. The aligned Bichon minimap/HUD boundary no longer overflows the exact 1024x768 Crystal-size stage: `.mini-map-panel` moved from `right=-2px` to `right=0`, and R309 desktop evidence records `left=896`, `right=1024`, `width=128` with `desktopOverflows=[]`. Compact `820x640` evidence also records `compactOverflows=[]`; both captures have `nonFaviconNetwork404s=[]` and `consoleErrors=[]`. Evidence: `docs/generated/player-qa/r309-minimap-bounds-web-page-state.json`, `docs/generated/player-qa/r309-minimap-bounds-web-page.png`, and `docs/generated/player-qa/r309-minimap-bounds-compact-web-page.png`. Remaining comparison queue: exact dynamic animal density/placement, and human visual acceptance.

> Latest runtime/frontend comparison sync: 2026-04-29-R308 completed. The Bichon browser comparison no longer applies the web-only 0.9 stage downscale at original client comparison sizes: desktop evidence records `.client-stage-frame` at exact `0,0,1024,768` with scale 1, black page/frame background, and no box shadow; compact evidence keeps the stage inside `820x640` at `798.72x599.04`. R308 also exports the missing Bichon visible-object sprite libraries from Crystal client data (`NPC/00`, `NPC/01`, `NPC/03`, `NPC/11`, `NPC/15`, `Monster/003`, `Monster/004`, `Monster/005`), removing the non-favicon sprite 404s from the aligned view. Evidence: `docs/generated/player-qa/r308-stage-scale-web-page-state.json`, `docs/generated/player-qa/r308-stage-scale-web-page.png`, and `docs/generated/player-qa/r308-stage-scale-compact-web-page.png` record `QA0429A / QA0429Hero` at `BichonProvince` map `0`, `287,618`, `hasGuard=true`, `hasArcherGuard=true`, `questTrackerVisible=false`, `nonFaviconNetwork404s=[]`, and `consoleErrors=[]`. Remaining comparison queue: exact dynamic animal density/placement, and human visual acceptance.

> Latest runtime/frontend comparison sync: 2026-04-29-R307 completed. The second aligned Bichon browser comparison point now has explicit ordinary Guard/ArcherGuard evidence. Added a focused `mir2-simulation` regression for `crystal:0:287:618` requiring `Guard` at `291,620` and `ArcherGuard` at `295,624` in both `ObjectMonster` packets and `worldSnapshot`. Browser evidence at `docs/generated/player-qa/r307-bichon-guard-archer-web-page-state.json` records `QA0429A / QA0429Hero` at `BichonProvince` map `0`, `287,618` with `hasGuard=true`, `hasArcherGuard=true`, `monsterCount=7`, `npcCount=5`, `hasUnderscoreNameplate=false`, and `questTrackerVisible=false`; screenshot evidence is `docs/generated/player-qa/r307-bichon-guard-archer-web-page.png`. Verification passed: focused simulation regression and CDP browser capture with zero console errors. Remaining comparison queue: exact dynamic animal density/placement, HUD scale/letterboxing differences, and human visual acceptance.

> Latest runtime/frontend comparison sync: 2026-04-29-R306 completed. The aligned Bichon browser view now removes the default web-only quest tracker overlay from the playfield and displays NPC/monster nameplates with Crystal-style spaces while keeping raw runtime names unchanged. Evidence: `docs/generated/player-qa/r306-bichon-display-web-page-state.json` records `QA0429A / QA0429Hero` at `BichonProvince` map `0`, `284,607` with `entityCount=17`, `npcCount=8`, `monsterCount=8`, `npcSpriteElementCount=8`, `monsterSpriteElementCount=8`, `hasUnderscoreNameplate=false`, and `questTrackerVisible=false`; screenshot evidence is `docs/generated/player-qa/r306-bichon-display-web-page.png`. Verification passed: web `tsc --noEmit`, CDP login/start/transfer/browser capture, and zero browser console errors. Remaining comparison queue: exact object density/placement, ordinary guard/archer placement, HUD scale/letterboxing differences, and human visual acceptance.

> Latest runtime/frontend comparison sync: 2026-04-29-R305 completed. The aligned Bichon web view now includes first-pass visible Crystal respawns in ECS/worldSnapshot, fixing the issue where `ObjectMonster` packets were emitted but later snapshots had only player/NPC entities. Evidence: `docs/generated/player-qa/r305-bichon-visible-world-snapshot.json` records `entityCount=17`, `npcCount=8`, `monsterCount=8`, including `Deer`, `Scarecrow`, `Hen`, and two `Royal_Guard` entries around `0:284,607`; browser evidence at `docs/generated/player-qa/r305-bichon-visible-web-page.png` and `docs/generated/player-qa/r305-bichon-visible-web-page-state.json` records 8 NPC sprite elements and 8 monster sprite elements. Verification passed: focused R305 regression, visible-respawn density regression, `fmt --check`, `mir2-gateway` build, live WS probe, browser state/screenshot capture, gateway health, and web HTTP 200. Remaining comparison queue: exact object density/placement, ordinary guard/archer placement, NPC display-name normalization, quest tracker/HUD/letterboxing differences, and human visual acceptance.

> Latest runtime/frontend comparison sync: 2026-04-29-R304 completed. The user's same-scene Bichon screenshots showed a real gap: the web runtime snapshot had only the player after entering a saved Crystal map, while the original client had nearby NPCs. R304 updates `apps/simulation/src/runtime.rs` so saved-character start and Crystal transfer paths repopulate the current map with Crystal NPC-info manifest entries. Live WS evidence is archived at `docs/generated/player-qa/r304-bichon-npc-world-snapshot.json` for `QA0429A / QA0429Hero` at `BichonProvince` map `0`, `284,607`: `entityCount=9`, `npcCount=8`, including `Assistant_Jane` and `Merchant_Ruben`. Browser evidence is archived at `docs/generated/player-qa/r304-bichon-npc-web-page.png` and `docs/generated/player-qa/r304-bichon-npc-web-page-state.json`, with `npcSpriteElementCount=8`. Verification passed: focused/adjacent simulation tests, `cargo +1.89.0 fmt --check`, `cargo +1.89.0 build --locked -p mir2-gateway`, gateway restart on `127.0.0.1:7110`, live WS probe, and browser state/screenshot capture. Remaining comparison queue: align deer/guard/monster density, normalize NPC display names, reduce quest tracker/HUD/letterboxing differences, and get human visual acceptance.

> Historical map-resource audit sync: 2026-04-29-R303 completed. Added `npm.cmd run audit:crystal-map-coverage --prefix apps\web` and archived evidence at `docs/generated/map/r303-crystal-map-coverage.json` plus `latest-crystal-map-coverage.json`. Static coverage checked all 463 Crystal manifest maps against local Crystal client map files and sampled map sprite source references: 463/463 map files present, 0 unsupported map types, 0 parse errors, 463/463 sampled viewports with source frames, and 0 missing map libraries. The 2026-05-16 all-map audit above supersedes R303's then-open source-frame/minimap warnings by classifying Crystal no-draw frames separately and adding gameplay semantic checks.

> Latest original-client comparison sync: 2026-04-28-R302 completed. Windows launched original Crystal server/client locally, generated a retained Crystal QA character through `MIR2_PACKET_TRACE_KEEP_LIFECYCLE_CHARACTER=1`, and archived original select/game screenshots plus web Stage 5 comparison evidence under `docs/generated/player-qa/r302-original-client/summary.json`. Diagnostic fresh current-live matrix evidence is also archived there; it confirms Crystal 9/9 reachable but not accepted in the fresh state (`stableDiffCleanCount=2/9`, `packetParityAccepted=false`) because local and Crystal fixtures were not deterministic/state-aligned. R302 is evidence for original-client launchability and visual-reference capture, not a replacement for R300 packet acceptance and not whole-project 100% Accepted.

> Latest frontend/player QA sync: 2026-04-28-R301 completed. The final automated Candidate acceptance pack was refreshed after R300 stable-diff packet acceptance. Evidence is archived at `docs/generated/player-qa/r301-summary.json`, with map API smoke 18/18 and 0 failures, minimap smoke 0 failures with a historical preview-index warning later closed by the 2026-05-16 map audit, WS load 64/64 ready with 0 errors and keepalive p95 637 ms, and Stage 5 UI smoke 88 screenshots with 0 critical console errors plus 32 compact text nodes checked without overflow. Verification passed without Docker: packet-trace bin 15/15, web `tsc --noEmit`, web `npm.cmd run build`, `mir2-game-data` 27/27, `mir2-gateway` 55/55 plus packet-trace bin 15/15, `mir2-admin-api` 22/22, `mir2-simulation` 674/674, and temporary gateway/web services were stopped with ports 7000/7110/3002 closed. Automation remains **100% Candidate**; backend/server tracked slice remains **100% Accepted under stable-diff packet acceptance**; whole-project accepted Crystal 1:1 remains **roughly 90%** until human visual/feel QA closes.

> Latest backend parity sync: 2026-04-28-R300 completed. Stable live packet comparison is now the accepted packet parity gate for the current tracked backend/server slice. R298 live Crystal matrix evidence remains the source artifact (`docs/generated/packet-traces/r298-live-matrix/latest-matrix.json`: 9/9 local OK, 9/9 Crystal OK, `crystalMissingCount=0`, `stableDiffCleanCount=9`, `acceptedStableLiveComparisonCount=9`), and R299 payload-hex probing records why strict exact remains dirty. R300 adds explicit stable acceptance mode to `packet_trace` (`MIR2_PACKET_TRACE_ACCEPT_STABLE_DIFF=1`), acceptance fields in matrix summaries, `docs/PACKET-PARITY-ACCEPTANCE.md`, and `docs/generated/packet-traces/r300-stable-acceptance.json`. Backend/server tracked slice is now **100% Accepted for the tracked backend/server slice under stable-diff packet acceptance**; whole-project accepted Crystal 1:1 remains **roughly 90%** until human visual/feel QA closes.

> Latest frontend/player QA sync: 2026-04-28-R297 completed. Windows refreshed automated Candidate evidence with `CRYSTAL_CLIENT_ROOT=E:\mir2\Crystal\Build\Client\Debug`: web build/typecheck, map API smoke 18/18, minimap smoke 0 failures with a historical preview-index warning later closed by the 2026-05-16 map audit, WS load 64/64 ready with 0 errors, Stage 5 UI smoke 88 screenshots with 0 critical console errors, `mir2-simulation` 674/674, `mir2-gateway` 55/55 plus packet-trace bin 14/14, `mir2-admin-api` 22/22, `fmt --check`, and `git diff --check` passed. R300 closes the backend/server packet gate under stable-diff acceptance; whole-project accepted Crystal 1:1 still needs human visual/feel acceptance.

> Previous backend parity sync: 2026-04-28-R298 completed. Windows live Crystal stable packet matrix evidence is recorded under `docs/generated/packet-traces/r298-live-matrix/latest-matrix.json`: 9/9 local OK, 9/9 Crystal OK, `crystalMissingCount=0`, `stableDiffCleanCount=9`, and `acceptedStableLiveComparisonCount=9`. Strict exact diff is still dirty (`diffDirtyCount=9`, `acceptedLiveComparisonCount=0`) and remains a diagnostic after R300 stable-diff packet acceptance. Verification passed without Docker: `mir2-simulation` 674/674, `mir2-gateway` 55/55 plus packet-trace bin 14/14, `mir2-admin-api` 22/22, `cargo +1.89.0 fmt --check`, `git diff --check`, and web `tsc --noEmit`.

> Latest backend parity sync: 2026-04-28-R248 completed. Windows closed the previously blocked `Server.MirDB` / `Envir\Routes` data-import gate for the current backend slice: `node packages\tooling\scripts\generate-crystal-respawn-manifest.mjs` read `E:\mir2\Crystal\Build\Server\Debug\Server.MirDB` and `E:\mir2\Crystal\Build\Server\Debug\Envir\Routes`, refreshed the Crystal respawn/monster/item/NPC-info manifests, and real map rows now carry `no_throw_item`, `no_drop_player`, and `no_drop_monster`. Verification passed: `mir2-game-data` 22/22, focused `mir2-simulation no_drop_monster_map_rule` 2/2, full `mir2-simulation` 670/670, and `mir2-gateway` 55/55 plus packet-trace bin tests 7/7. R300 later closed the remaining backend packet acceptance gate under explicit stable-diff acceptance.

> Latest product-evolution sync: 2026-04-28-R247 completed. Fixed the Admin Web mail-submit dead path and added explicit command status loading: GM Tools system mail now submits through a server action with pending state, Admin API exposes `GET /admin/commands/:command_id/status`, and the post-submit page shows command status, result, trace, operator, delivery mode, and mail ids. Browser smoke verified `Queue System Mail` -> `succeeded` / `gateway_live / 1` / mail id, and Player Web Mail shows `Compensation Package` with `5000 Gold · Unclaimed`.

> Latest product-evolution sync: 2026-04-28-R246 completed. Fixed the online-player visibility gap for admin-delivered gold/mail: Gateway sessions now merge externally delivered Stage 5 mail from the shared account store before snapshots and saves, so keepalive/tick cannot overwrite a just-delivered admin mail and the player UI sees the new mail while still online. Browser smoke verified `GM Currency Grant` with `888 Gold · Unclaimed` in the Player Web Mail panel after an Admin API grant.

> Latest product-evolution sync: 2026-04-28-R245 completed. Local backend testing is now browser-ready: Docker Postgres/Redis/NATS/Redpanda/ClickHouse are healthy; Gateway runs in explicit Postgres source mode with Redis routing cache; Admin API runs with Postgres command/audit/approval/outbox storage, ClickHouse event reads, gateway mail/kick URLs, and local bearer auth; Admin Web runs on `http://127.0.0.1:3020`, and Player Web runs on `http://127.0.0.1:3010`. Admin API also gained optional `ADMIN_OPERATOR_POLICY_PATH` bearer-to-operator policy loading, requester self-approval is blocked by default, and Admin Web GM Tools now exposes grant item, grant gold, kick player, and ban account forms.

> Latest product-evolution sync: 2026-04-27-R244 completed. Phase 1-7 production-control-plane route is now landed: approvals are persistent and emit approval events; Admin outbox has JetStream mode plus retry/dead-letter lifecycle events; GM routes cover grant item, grant gold, kick player, and ban account; Postgres source mode has explicit stale `save_version` conflict coverage; Redis session cache has a character-name routing index; Admin API/Web expose a merged timeline read model; Admin Web forwards optional operator bearer tokens. Verification is being refreshed against the full requested baseline.

> Latest product-evolution sync: 2026-04-27-R238 completed. Admin command events now cover terminal control-plane outcomes, not only success: Postgres-backed command completion emits `admin.command.succeeded`, `admin.command.failed`, or `admin.command.denied` envelopes. ClickHouse now subscribes to all three Redpanda topics through the v2 admin event consumer group, and Admin Web Audit can filter denied event status. Smoke verified denied events from the real Admin API permission path and failed events through Redpanda -> ClickHouse -> `/admin/events`.

> Latest product-evolution sync: 2026-04-27-R237 completed. Admin outbox delivery state is now split per publisher with `nats_status`, `redpanda_status`, `last_error`, and `dispatched_at_ms`. `dispatch-admin-outbox` records NATS and Redpanda/Pandaproxy delivery independently, retries/dead-letters without marking rows dispatched when any configured publisher fails, and only marks dispatched when all configured publishers succeed. Admin API `/admin/events` now supports `limit`, `commandId`, `eventType`, and `status` filters and returns a degraded response instead of failing hard when ClickHouse is unavailable. Admin Web Audit exposes those filters and a separate event-stream health badge.

> Latest product-evolution sync: 2026-04-27-R236 completed. Admin outbox events now use a stable envelope (`eventId`, `eventType`, `schemaVersion`, `commandId`, `operatorId`, `status`, `occurredAtMs`, `payload`, `payloadJson`). `dispatch-admin-outbox` can publish the same event to Redpanda through Pandaproxy via `ADMIN_OUTBOX_REDPANDA_URL` while preserving NATS dispatch, and marks rows dispatched only after configured publishers succeed. Admin API now exposes `GET /admin/events` from ClickHouse, and Admin Web Audit shows the projected event stream. End-to-end smoke passed: Admin API command -> Postgres `admin_outbox` -> dispatcher -> Redpanda -> ClickHouse `admin_events` / `admin_command_events` -> Admin API `/admin/events`.

> Latest product-evolution sync: 2026-04-27-R235 completed. Local event analytics infrastructure now includes Redpanda and ClickHouse in the default dev Compose stack. Redpanda exposes internal/external Kafka listeners, ClickHouse initializes a Kafka-engine table plus materialized view for `admin.command.succeeded`, and infra/docs include a Redpanda-to-ClickHouse smoke path. NATS remains the existing lightweight admin outbox notification dispatcher; Redpanda/ClickHouse are non-authoritative analytics infrastructure.

> Latest product-evolution sync: 2026-04-27-R234 completed. Admin production boundary hardening advanced: Admin API now supports optional `ADMIN_OPERATOR_TOKEN` Bearer validation, high-risk command `approvalId` validation, `GrantItem` / gold `GrantCurrency` executors through audited system-mail delivery, and admin outbox retry/dead-letter state for failed dispatch attempts. Verification passed: `cargo +1.89.0 test --locked -p mir2-admin-api -- --test-threads=1` (11/11).

> Latest product-evolution sync: 2026-04-27-R233 completed. Postgres account-store source-of-truth mode now tracks loaded `store_version` / `save_version` metadata and rejects stale source writers before overwriting newer DB state. Successful source saves refresh in-memory version metadata. Docker Postgres integration coverage now verifies stale writer rejection and reload-then-save version refresh. Verification passed: `cargo +1.89.0 test --locked -p mir2-simulation postgres_source_mode -- --test-threads=1` (2/2).

> Latest product-evolution sync: 2026-04-27-R232 expanded. Gateway session caching now has an optional Redis adapter behind `MIR2_GATEWAY_REDIS_CACHE_URL`, configurable `MIR2_GATEWAY_SESSION_CACHE_TTL_SECONDS`, Redis SETEX/GET/DEL support, TTL expiry coverage, and cache hit/miss equivalence against authoritative world snapshots. Default gateway startup still uses the in-memory cache when Redis env is unset. Verification passed: `cargo +1.89.0 test --locked -p mir2-gateway session_cache -- --test-threads=1` (5/5).

> Latest product-evolution sync: 2026-04-27-R232 completed. Added the first gateway session/cache boundary without making Redis authoritative: `apps/simulation` now exposes active account/character identity, `apps/gateway` has a `GatewaySessionCache` contract plus in-memory implementation for online session records, and the web gateway refreshes the cache after authoritative saves and removes the record on disconnect. Focused verification passed: `cargo +1.89.0 test --locked -p mir2-gateway session_cache -- --test-threads=1` (4/4) and `cargo +1.89.0 fmt --check`. The Redis endpoint remains the external target; a real Redis adapter/invalidation integration is the next cache slice.

> Latest product-evolution sync: 2026-04-27-R229 completed. First Postgres/NATS persistence slice landed and was live-verified against Docker: `infra/postgres/migrations/0001_core.sql`, Postgres command/audit adapters behind `ADMIN_DATABASE_URL`, an admin outbox repository boundary, `dispatch-admin-outbox` for publishing pending rows to NATS, and `cargo +1.89.0 run --locked -p mir2-admin-api --bin import-account-store -- .mir2-data/accounts.json` for JSON account-store import. Docker smoke confirmed imported demo/Scout state, Postgres command/audit/outbox writes, NATS `admin.command.succeeded` publish, and outbox `dispatched` status.

> Latest product-evolution sync: 2026-04-27-R230 completed. Gameplay account-store saves now have an optional Postgres mirror through `MIR2_ACCOUNT_STORE_DATABASE_URL`; JSON remains the runtime source of truth. Gateway and Admin API fallback mail both pass the DB URL into `SimulationConfig`, and Docker smoke proved fallback mail mirrored Stage 5 mail into `character_saves.stage5_systems_json`. Verification passed: simulation config 11/11, admin-api 8/8, gateway 55/55, fmt, diff check, and healthy Docker core services.

> Latest product-evolution sync: 2026-04-27-R231 completed. Explicit Postgres account-store source-of-truth mode landed behind `MIR2_ACCOUNT_STORE_BACKEND=postgres`. It loads from Postgres, saves transactionally with account row locks, increments `store_version` / `save_version`, and was Docker-smoked through Admin API fallback mail. Verification passed: simulation config 11/11, admin-api 8/8, gateway 55/55, fmt, compose config/healthy services, and diff check.

> Latest truth-audit sync: 2026-04-27. `docs/PARITY-TRUTH-AUDIT.md` now defines the authoritative wording for Accepted vs Candidate vs Fallback vs Blocked. Use **100% Candidate**, backend/server tracked-slice **99.70% Candidate**, and whole-project accepted Crystal 1:1 **roughly 90%** until live Crystal trace, source-data, and human visual/feel gates close.

> Latest product-evolution sync: 2026-04-27-R228 completed. Admin `SendSystemMail` now reaches live game-visible state: `apps/admin-api` tries `ADMIN_GATEWAY_MAIL_URL` via a reqwest-free plain TCP HTTP POST helper and falls back to the persistent account store; `apps/gateway` exposes `POST /admin/system-mail` to deliver into the running gateway `SimulationConfig.account_store`; `apps/simulation` persists Stage 5 mail into `CharacterSaveRecord.stage5_systems_json`; and the player web Mail panel can display, claim, and delete those messages. Runtime smoke proved Admin Web `:3020` -> Admin API `:7420` -> gateway `:7110` delivered `deliveryMode: "gateway_live"` to `Scout`, then a gateway WS `stage5Command mail.claim` marked it claimed, raised gold from 1280 to 6280, and delivered one `red-potion`.

> Latest product-evolution sync: 2026-04-27 admin-web i18n slice completed. `apps/admin-web` now has `admin_locale` cookie driven server-rendered English / Simplified Chinese dictionaries, a top-bar language switcher, localized navigation/page heads/tables/statuses/forms/empty states, and verified Chinese render smoke on `/` and `/gm-tools`.

> Latest product-evolution sync: 2026-04-27 Admin operations foundation advanced. `apps/admin-api` now has persistent-storage-ready command/audit repository traits, in-memory repositories, Axum HTTP routes, and a `SendSystemMail` domain outbox executor. `apps/admin-web` now has a production-shaped desktop operations UI across Dashboard, Players, Player Detail, Economy, Activities, Servers, Risk, GM Tools, and Audit, with the GM mail form wired through Next to the Rust Admin API. Verification: `cargo +1.89.0 test --locked -p mir2-admin-api -- --test-threads=1`, `cargo +1.89.0 fmt --check`, admin-web `tsc --noEmit`, admin-web `next build`, direct Rust API curl write, Next route proxy curl write, and Playwright screenshots `docs/admin-web-dashboard-smoke.png` / `docs/admin-web-gm-tools-smoke.png`.

> Previous sync: R225 completed. Mac-local Candidate regression was green: web `tsc --noEmit`, direct `next build`, Stage 5 UI smoke (88 screenshots, summary counts in manifest), map API smoke 18/18, minimap asset smoke 0 failures with a historical preview-index warning later closed by the 2026-05-16 map audit, WS load 64/64, `mir2-game-data` 22/22, `mir2-gateway` 54/54 including packet trace bin tests 7/7, `mir2-simulation` 664/664, require-local `packet_trace --matrix` wrote 9 local artifacts with 17 intended skips under `docs/generated/packet-traces/r225-matrix`, `cargo +1.89.0 fmt --check`, and `git diff --check`. R225 also added the Windows continuation checklist and cleaned the stale gateway README. At R225 time backend/server tracked slice was 99.70%; R300 later closed the packet gate under explicit stable-diff acceptance.

> Latest sync: R224 completed. The `mir2-gateway` `packet_trace` bin target is restored, `--list-flows` works, `mir2-gateway` now passes 53/53 including packet trace bin tests 6/6, and local require-mode `packet_trace --matrix` wrote 9/9 TCP-traceable matrix artifacts with `localOk=true` under `docs/generated/packet-traces/r224-matrix`. Truthful status split: automated evidence is **100% Candidate**, backend/server tracked slice remains **99.70%**, and real full-project accepted 1:1 remains **roughly 90.0%**. Active follow-up round is R225 for final human acceptance / external blockers; remaining non-routine gates are final human Crystal visual/feel acceptance, missing local `Crystal/Build/Server/Debug/Server.MirDB`, and missing live `MIR2_CRYSTAL_TCP_ADDR`.

> Latest sync: R219-R222 completed. Frontend/global evidence advanced across login/select lifecycle, archived map API/minimap asset smoke JSON, refreshed WS load, compact multi-panel bounds, compact system-menu overflow fix, and NPC dialog link-capable rendering. Stage 5 UI smoke now captures 85 screenshots and records `loginFlow`, `selectFlow`, expanded `compactPanelLayout`, and existing broad gameplay/system flows. Validation: web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke (85 screenshots), map API smoke 18/18, minimap asset smoke 0 failures with a historical preview-index warning later closed by the 2026-05-16 map audit, WS load 64/64, `cargo +1.89.0 fmt --check`, and `git diff --check`. Active backend/global round is R223; backend/server parity estimate is 99.70%, whole-project 1:1 estimate is 90.0%.


> Latest sync: R172 completed. Successful high-level NPC interaction no longer emits runtime-only `sim.talkingToNpc`; NPC `ObjectChat`/dialog packet surfaces and Crystal NPC script/service flows are preserved. Validation: focused `npc_interaction` 2/2, `crystal_npc_dialog` 1/1, `crystal_npc_service` 1/1, broad `crystal_npc` 52/52, `cargo +1.89.0 fmt --check`, and full `mir2-simulation` 648/648. Active backend round is R173; backend/server parity estimate is 99.70%.


> Latest sync: R171 completed. Direct high-level ground-drop pickup invalid target/distance handling no longer emits runtime-only `sim.itemNoLongerOnGround`, `sim.targetNotGroundDrop`, or `sim.moveCloserToPickItem`; Crystal owner/full-bag pickup messages and current-cell packet pickup behavior are preserved. Validation: focused direct-pickup tests 3/3, `pickup` 18/18, adjacent `drop` 42/42, `cargo +1.89.0 fmt --check`, and full `mir2-simulation` 648/648. Active backend round is R172; backend/server parity estimate is 99.70%.


> Latest sync: R170 completed. Missing defeated-monster entity handling no longer emits runtime-only `sim.defeatedMonsterEntityMissing`; normal death/drop packet surfaces are preserved. Validation: focused missing-entity silent test 1/1, visible death packet test 1/1, adjacent `drop` 41/41, `cargo +1.89.0 fmt --check`, and full `mir2-simulation` 645/645. Active backend round is R171; backend/server parity estimate is 99.70%.


> Latest sync: R169 completed. Monster death drop success paths no longer emit runtime-only `sim.monsterDroppedGoldOnGround` / `sim.monsterDroppedItem` chats; ground gold/item drops, quest-drop routing, and pickup packet surfaces are preserved. Validation: focused item-drop no-chat 1/1, gold-drop no-chat/pickup 1/1, adjacent `drop` 41/41, `pickup` 15/15, `attack` 76/76, `cargo +1.89.0 fmt --check`, and full `mir2-simulation` 644/644. Active backend round is R170; backend/server parity estimate is 99.70%.


> Latest sync: R168 completed. VampireSpider summoned death explosion no longer emits runtime-only `sim.targetDefeated` defeat chat; explosion damage, summon despawn timing, and packet health surfaces are preserved. Validation: focused vampire-spider no-chat explosion test 1/1, adjacent `spider` 6/6, `attack` 76/76, `cargo +1.89.0 fmt --check`, and full `mir2-simulation` 643/643. Active backend round is R169; backend/server parity estimate is 99.70%.


> Latest sync: R167 completed. Ordinary combat hit resolution no longer emits local runtime damage narration (`sim.youHitTargetForDamage`, `sim.targetDefeated`, `sim.monsterPressuresYouForDamage`); packet health/struck/death surfaces and Trainer DPS reporting are preserved. Validation: focused player-hit no-chat test 1/1, adjacent `attack` 76/76, `cargo +1.89.0 fmt --check`, and full `mir2-simulation` 643/643. Active backend round is R168; backend/server parity estimate is 99.70%.


> Latest sync: R166 completed. Successful cast-skill paths no longer emit local `sim.castSkill` helper chat; buff/heal and summon success now preserve state mutation/spawn behavior without generic success narration. Validation: focused `casting` suite 6/6, `cargo +1.89.0 fmt --check`, and full `mir2-simulation` 643/643. Active backend round is R167; backend/server parity estimate is 99.70%.


> Latest sync: R165 completed. Cast-skill high-level entrypoint (`cast_skill`) now silently rejects before `StartGame` instead of emitting local `sim.joinWorldBeforeCastingSkills` helper chat. Validation: focused pre-start cast-skill test 1/1, adjacent `casting` 6/6, `cargo +1.89.0 fmt --check`, and full `mir2-simulation` 643/643. Active backend round is R166; backend/server parity estimate is 99.70%.


> Latest sync: R164 completed. Interaction high-level/dialog entrypoints (`interact`, `select_npc_dialog_target`) now silently reject before `StartGame` instead of emitting local `sim.joinWorldBeforeInteracting` helper chat. Validation: focused pre-start interaction test 1/1, adjacent `npc_interaction` 2/2, `crystal_npc_dialog` 1/1, `crystal_npc_service` 1/1, `cargo +1.89.0 fmt --check`, and full `mir2-simulation` 642/642. Active backend round is R165; backend/server parity estimate is 99.70%.


> Latest sync: R163 completed. Harvest high-level and packet entrypoints (`harvest`, `Harvest`) now silently reject before `StartGame` instead of emitting local `sim.joinWorldBeforeHarvesting` helper chat. Validation: focused pre-start harvest test 1/1, adjacent `harvest` 9/9, `cargo +1.89.0 fmt --check`, and full `mir2-simulation` 641/641. Active backend round is R164; backend/server parity estimate is 99.70%.


> Latest sync: R162 completed. Attack high-level and packet entrypoints (`attack`, `Attack`, `RangeAttack`) now silently reject before `StartGame` instead of emitting local `sim.joinWorldBeforeAttacking` helper chat. Validation: focused pre-start attack test 1/1, adjacent `attack` 76/76, combat trace focused test 1/1, `cargo +1.89.0 fmt --check`, and full `mir2-simulation` 640/640. Active backend round is R163; backend/server parity estimate is 99.70%.


> Latest sync: R161 completed. Movement high-level and packet entrypoints (`move_to`, `Walk`, `Run`, `Turn`) now silently reject before `StartGame` instead of emitting local `sim.joinWorldBeforeMoving` / `sim.joinWorldBeforeTurning` helper chat. Validation: focused pre-start movement test 1/1, adjacent `walk` 6/6, `run_` 3/3, `transfer_map` 2/2, `cargo +1.89.0 fmt --check`, and full `mir2-simulation` 639/639. Active backend round is R162; backend/server parity estimate is 99.70%.


> Latest sync: R160 completed. Pickup high-level and packet entrypoints now silently reject before `StartGame` instead of emitting local `sim.joinWorldBeforePickingUpItems` helper chat. Validation: focused pre-start pickup test 1/1, pickup suite 15/15, `cargo +1.89.0 fmt --check`, and full `mir2-simulation` 638/638. Active backend round is R161; backend/server parity estimate is 99.70%.


> Latest sync: R159 completed. Trainer immediate damage reporting now routes through Crystal `server.PetInflictedDamageDps` with localized `server.You` actor; modeled `Physical Agility` damage type and DPS value are preserved. Validation: focused trainer test 1/1, `cargo +1.89.0 fmt --check`, and full `mir2-simulation` 638/638. Active backend round is R160; backend/server parity estimate is 99.70%.


Last updated: 2026-05-10

Purpose: queue autonomous tasks for reaching **100% Candidate**. The Coordinator should keep this file current as rounds complete.

Restart handoff: if the Codex session is reopened after shutdown or context loss, read `docs/AGENT-RESUME-HANDOFF.md` before continuing the active round. The user wants the previous subagent workflow to continue without routine confirmations.

Product evolution handoff: after the 1:1 Candidate baseline, future product work should also read `docs/POST-1TO1-EVOLUTION-PLAN.md`, `docs/TECH-MODERNIZATION-RFC.md`, `docs/ARCHITECTURE-ADOPTION-PLAN.md`, `docs/PLATFORM-CLIENT-STRATEGY.md`, and `docs/ADMIN-OPERATIONS-ARCHITECTURE.md`. Database, cache, login UI, admin backend, global zone, client distribution, and NPC script parser changes are expected product-evolution areas, not automatic Crystal parity regressions.

Truth audit handoff: read `docs/PARITY-TRUTH-AUDIT.md` before changing progress percentages or handoff wording. Fallbacks such as synthetic map terrain, Admin mock read models, in-memory command/audit stores, JSON account-store persistence, and modeled Stage 5 systems are useful Candidate evidence, but are not final accepted Crystal 1:1.

Status values:

- `[ ]` queued
- `[~]` active
- `[x]` complete and verified
- `[!]` blocked

## Completed Round: 2026-05-07-P1P2-Packet-Runtime

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Close Group/Quest/Market/Refine/OpenDoor/request-info packet-runtime gaps | Coordinator | `packages/protocol/src/packets.rs`, `packages/protocol/src/trace.rs`, `apps/gateway/src/web.rs`, `apps/simulation/src/config.rs`, `apps/simulation/src/lib.rs`, `apps/simulation/src/runtime/packets.rs`, `apps/simulation/src/runtime/tests.rs` | Focused regressions for Group utility, Quest, Market, Refine, OpenDoor, and manifest-backed map/monster/NPC info requests passed; full locked three-package regression passed with Gateway 103/103 + packet-trace 17/17, Protocol 29/29 + codec 32/32, and Simulation 722/722. |
| [x] | Replace visible System Menu social placeholder wording | Coordinator | `apps/web/app/original-client-shell.tsx`, `apps/web/app/globals.css`, `apps/web/scripts/smoke-stage5-ui.mjs`, frontend docs | Web `npx tsc --noEmit` passed and fast Stage 5 UI smoke against local Gateway/Web captured 17 screenshots, including 24 social-menu checks and no critical console errors. |

## Active Round: 2026-05-01-R327

Restart note: R248 closed the Windows server-data import gate with local `Crystal/Build/Server/Debug/Server.MirDB` plus matching `Build/Server/Debug/Envir/Routes`. R298/R300 remain the accepted stable-diff packet parity decision for the tracked backend/server slice. R301 refreshed the final automated Candidate acceptance pack. R302-R319 progressively closed original/Web visual comparison gaps through source-backed map, entity, HUD, Gameshop, BigMap, Mail, label, and cursor passes. R321-R326 added original/Web movement diagnostics and Crystal-like held-mouse queued movement behavior. R327 verifies service-backed Gameshop Buy command routing and right-click map-click arrival without jitter or packet starvation. Real full-project accepted 1:1 remains roughly 90.0% until human Crystal visual/feel acceptance or explicit accepted differences close.

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Wire Gameshop Buy and fix map-click target arrival | Coordinator | `apps/web/app/page.tsx`, `apps/web/app/original-client-shell.tsx`, `apps/web/scripts/capture-crystal-parity.mjs`, `apps/web/scripts/capture-web-movement-jitter.mjs`, `apps/web/scripts/crystal-ui-export-manifest.json`, `apps/web/public/original-ui/NPC/25`, `apps/simulation/src/runtime.rs`, frontend/backend docs, `docs/generated/player-qa/r327-gameshop-buy-click-final-clean-state.json`, `docs/generated/player-qa/movement-jitter/r327-map-click-target-arrival-fixed3.json` | Gameshop Buy now sends manifest-backed `gameShop.buyCredit` / `gameShop.buyGold`; QA browser capture records expected zero-credit rejection with no 404/console errors, while the focused simulation test covers positive credit mail delivery. Map-click target movement reaches `338,270` with four run `moveTo` steps, `movementPlan=null`, and `jumps=[]`; gateway move log confirms movement through `338,270`. Verified by web `tsc --noEmit`, script syntax checks, focused simulation test, `mir2-gateway` check, and CDP captures. |
| [x] | Align Bichon entity projection/nameplates to Crystal source anchors | Coordinator | `apps/web/app/original-client-shell.tsx`, `apps/web/app/globals.css`, frontend parity docs, `docs/generated/player-qa/r312-entity-crystal-anchor/` | Web entity sprites/nameplates/health bars now use Crystal `DrawLocation` / `DisplayRectangle` placement while map floor/object sprites retain Crystal map-layer math. Browser capture for `QA0429A / QA0429Hero` at Bichon `0:287,618` records exact stage/HUD bounds, self nameplate `top=275`, `questMarkerCount=0`, `nonFaviconNetwork404s=[]`, and `consoleErrors=[]`. Verified by web `tsc --noEmit`, R312 browser capture, screenshot review, and `git diff --check`. |
| [x] | Fix login-transition leakage and over-broad NPC quest markers; add original/Web visual-watch tooling | Coordinator | `apps/web/app/original-client-shell.tsx`, `apps/web/app/page.tsx`, `apps/web/scripts/capture-crystal-parity.mjs`, `apps/web/scripts/r310-visual-watch.ps1`, `apps/simulation/src/config.rs`, `apps/simulation/src/runtime.rs`, frontend/backend parity docs, `docs/generated/player-qa/r310-visual-watch/` | Web capture for `QA0429A / QA0429Hero` at Bichon `0:287,618` records `screen=game`, `transitionOverlayVisible=false`, `questMarkerCount=0`, exact stage/HUD/minimap bounds, zero non-favicon 404s, and zero console errors. One-sample watch run captured original and Web screenshots with no errors. Verified by web `tsc --noEmit`, `cargo fmt --check`, focused `mir2-simulation crystal_current_map_transfer_spawns_visible` 2/2, and R310 browser capture. |
| [x] | Close aligned Bichon minimap/HUD 2px boundary overflow | Coordinator | `apps/web/app/globals.css`, frontend parity docs, `docs/generated/player-qa/r309-minimap-bounds-web-page.png`, `docs/generated/player-qa/r309-minimap-bounds-compact-web-page.png`, `docs/generated/player-qa/r309-minimap-bounds-web-page-state.json` | Browser capture for `QA0429A / QA0429Hero` at Bichon `0:287,618` records desktop minimap `left=896`, `right=1024`, `desktopOverflows=[]`, compact minimap inside `820x640`, `compactOverflows=[]`, `nonFaviconNetwork404s=[]`, and `consoleErrors=[]`. |
| [x] | Remove Bichon comparison stage downscale and visible-object sprite 404s | Coordinator | `apps/web/app/globals.css`, `apps/web/scripts/crystal-ui-export-manifest.json`, `apps/web/public/original-ui/manifest.generated.json`, `apps/web/public/original-ui/NPC/00`, `apps/web/public/original-ui/NPC/01`, `apps/web/public/original-ui/NPC/03`, `apps/web/public/original-ui/NPC/11`, `apps/web/public/original-ui/NPC/15`, `apps/web/public/original-ui/Monster/003`, `apps/web/public/original-ui/Monster/004`, `apps/web/public/original-ui/Monster/005`, frontend parity docs, `docs/generated/player-qa/r308-stage-scale-web-page.png`, `docs/generated/player-qa/r308-stage-scale-compact-web-page.png`, `docs/generated/player-qa/r308-stage-scale-web-page-state.json` | Browser capture for `QA0429A / QA0429Hero` at Bichon `0:287,618` records exact 1024x768 desktop stage bounds with scale 1, compact 820x640 bounds inside viewport, `hasGuard=true`, `hasArcherGuard=true`, `nonFaviconNetwork404s=[]`, and `consoleErrors=[]`. Verified by web `tsc --noEmit`, JSON parse check, focused `mir2-simulation` R307 regression, `cargo fmt --check`, targeted `git diff --check`, gateway health, and web HTTP 200. |
| [x] | Lock Bichon ordinary Guard/ArcherGuard visibility evidence | Coordinator | `apps/simulation/src/runtime.rs`, `docs/FRONTEND-1TO1-GAPS.md`, `docs/AGENT-TASK-QUEUE.md`, `docs/CRYSTAL-1TO1-ROADMAP.md`, `docs/generated/player-qa/r307-bichon-guard-archer-web-page.png`, `docs/generated/player-qa/r307-bichon-guard-archer-web-page-state.json` | Focused regression and browser capture prove `Guard` and `ArcherGuard` are visible at the second Bichon comparison point `0:287,618`, while R306 display cleanup remains intact. Verified by focused `mir2-simulation` regression and CDP browser capture with zero console errors. |
| [x] | Clean up aligned Bichon display-only nameplates and quest overlay | Coordinator | `apps/web/app/original-client-shell.tsx`, `apps/web/app/globals.css`, `docs/FRONTEND-1TO1-GAPS.md`, `docs/AGENT-TASK-QUEUE.md`, `docs/CRYSTAL-1TO1-ROADMAP.md`, `docs/generated/player-qa/r306-bichon-display-web-page.png`, `docs/generated/player-qa/r306-bichon-display-web-page-state.json` | Browser view keeps R305 population counts while visible nameplates no longer contain underscores and the default web quest tracker is absent. Verified by web `tsc --noEmit` and CDP browser capture for `QA0429A / QA0429Hero` at `0:284,607` with zero console errors. |
| [x] | Populate current Crystal map visible respawns for aligned Bichon comparison | Coordinator | `apps/simulation/src/runtime.rs`, frontend/backend parity docs, `docs/generated/player-qa/r305-bichon-visible-world-snapshot.json`, `docs/generated/player-qa/r305-bichon-visible-web-page.png`, `docs/generated/player-qa/r305-bichon-visible-web-page-state.json` | Current-map visible respawns now populate ECS/worldSnapshot, not only `ObjectMonster` packets. WS and browser evidence show 8 NPCs plus 8 monsters at `0:284,607`, including Deer and Royal_Guard. Verified by focused R305 regression, visible-respawn density regression, `fmt --check`, `mir2-gateway` build, live WS probe, browser capture, gateway health, and web HTTP 200. |
| [x] | Populate current Crystal map NPCs for aligned Bichon comparison | Coordinator | `apps/simulation/src/runtime.rs`, frontend/backend parity docs, `docs/generated/player-qa/r304-bichon-npc-world-snapshot.json`, `docs/generated/player-qa/r304-bichon-npc-web-page.png`, `docs/generated/player-qa/r304-bichon-npc-web-page-state.json` | Saved-character `StartGame` and Crystal transfer paths now rebuild current-map world population from the Crystal NPC-info manifest. Live WS probe for `QA0429A / QA0429Hero` at `0:284,607` reports `npcCount=8` with `Assistant_Jane` and `Merchant_Ruben` visible; browser CDP state records 8 NPC sprite elements and expected visible nameplates. Verified by `fmt --check`, focused R304 NPC regression, adjacent `transfer_map`, `start_game_emits_visible_object_packets`, `world_snapshot_marks_safe_zone_after_start_game`, `mir2-gateway` build, live WS probe, and browser screenshot/state capture. |
| [x] | Capture original Crystal client/server live visual reference | Coordinator | `apps/gateway/src/bin/packet_trace.rs`, `docs/generated/player-qa/r302-original-client/`, parity/frontend docs | Original `Server.exe` listened on `127.0.0.1:7000`; visible `Client.exe` reached select and game with retained `R302HeroB` character. R302 archived Crystal screenshots, web Stage 5 screenshots, and `summary.json`. Packet-trace bin 16/16 and Stage 5 UI smoke passed. Fresh matrix is diagnostic only: `stableDiffCleanCount=2/9`, `packetParityAccepted=false`. |
| [x] | Close live packet comparison through accepted stable-diff policy | Coordinator | `apps/gateway/src/bin/packet_trace.rs`, `docs/PACKET-PARITY-ACCEPTANCE.md`, `docs/generated/packet-traces/r300-stable-acceptance.json`, parity docs | R298 provides the accepted source matrix (`stableDiffCleanCount=9/9`, `crystalMissingCount=0`). R299 single-flow payload-hex probe confirmed the current movement command surface is already aligned for `Turn`/`Walk`/`Run` plus `UserLocation`, while exact diff dirtiness is driven by dynamic Crystal object ids, login timestamps, character lifecycle indices, AOI object packet ordering/payloads, and dynamic `DefaultNPC` / `NPCUpdate` payloads. R300 adds `MIR2_PACKET_TRACE_ACCEPT_STABLE_DIFF=1`, `acceptanceMode`, `acceptedPacketParityCount`, and `packetParityAccepted`; strict exact remains diagnostic. Verification: `cargo +1.89.0 test --locked -p mir2-gateway --bin packet_trace -- --test-threads=1` (15/15), `cargo +1.89.0 fmt --check`. |
| [x] | Refresh final automated Candidate acceptance pack after R300 | Coordinator | generated R301 evidence plus parity docs | R301 passed packet-trace bin 15/15, web `tsc --noEmit`, web `npm.cmd run build`, map API smoke 18/18 with 0 failures, minimap smoke 0 failures with a historical preview-index warning later closed by the 2026-05-16 map audit, WS load 64/64 ready with 0 errors and keepalive p95 637 ms, Stage 5 UI smoke 88 screenshots with 0 critical console errors and 32 compact text nodes checked without overflow, `mir2-game-data` 27/27, `mir2-gateway` 55/55 plus packet-trace bin 15/15, `mir2-admin-api` 22/22, and `mir2-simulation` 674/674. Evidence summary: `docs/generated/player-qa/r301-summary.json`. |
| [~] | Track remaining whole-project human acceptance | Coordinator | frontend QA docs unless user accepts/fails differences | R304/R305 removed the largest aligned Bichon snapshot data gaps by restoring current-map NPCs and first-pass visible respawns. R306 removed the default quest tracker overlay and display-name underscore gap. R307 added ordinary Guard/ArcherGuard evidence at `0:287,618`. R308 removed browser-only original-size stage downscaling/frame decoration and visible-object sprite 404s for that comparison view. R309 closed the measured minimap 2px overflow. R310 removed the game-entry login overlay leak and over-broad quest markers while starting long-run visual-watch evidence. Human player QA is still open: exact dynamic animal density/placement, light/effect feel, and visual/feel acceptance remain. Historical Web/Stage5 automation status was 100.0% Candidate; it is not WN-CANDIDATE formal packaging/native-soak acceptance. Backend/server tracked slice is 100% Accepted under stable-diff packet acceptance, and real full-project accepted 1:1 remains roughly 90.0%. |
| [x] | Add explicit parity truth audit | Coordinator | `docs/PARITY-TRUTH-AUDIT.md`, handoff docs | Truth audit now separates Accepted, Candidate, Fallback, Blocked, and Product evolution. It explicitly calls out synthetic map fallback, missing Crystal resources, live trace blocker, Admin mock/read-model gaps, local persistence, and human acceptance boundaries. |
| [~] | Plan post-1:1 product evolution boundaries | Coordinator | docs/product specs first | `docs/POST-1TO1-EVOLUTION-PLAN.md` defines the first boundary for database/cache, login UI, NPC script parser, and product gameplay changes while preserving the current Candidate baseline as a regression reference. |
| [~] | Finalize technical modernization RFC | Coordinator | docs only until approved | `docs/TECH-MODERNIZATION-RFC.md` captures the current first-principles direction: Rust simulation authority, Postgres authoritative persistence, Redis non-authoritative cache/session/routing, global services plus zone/channel runtime, Bevy + NextJS frontend split, audited admin backend, and developer-oriented NPC DSL compiled to Rust IR. |
| [x] | Add architecture adoption plan and local dev infra skeleton | Coordinator | `docs/ARCHITECTURE-ADOPTION-PLAN.md`, `infra/docker-compose.dev.yml`, `infra/README.md`, `README.md` | Added immediate/defer architecture matrix and local Compose stack. Core services are Postgres, Redis, and NATS; Redpanda, ClickHouse, Meilisearch, Loki, and Grafana are optional profiles and not required for normal gameplay/parity runs. |
| [~] | Validate platform/client distribution strategy | Coordinator | docs and prototypes only until approved | `docs/PLATFORM-CLIENT-STRATEGY.md` records Web as first-class, Tauri shell for near-term Windows/macOS, mobile after validation, Bevy native desktop as a performance escape hatch, and consoles as a deferred separate platform project. |
| [~] | Finalize admin operations architecture | Coordinator | docs first, then admin command/audit model | `docs/ADMIN-OPERATIONS-ARCHITECTURE.md` defines Admin Web, Admin API/control plane, RBAC, audit records, typed admin commands, command execution, online/offline target handling, content publishing, and MVP scope. |
| [~] | Build admin command/audit foundation | Coordinator | `apps/admin-api` | `apps/admin-api` now has typed permissions, operators, targets, admin commands, command envelopes, audit records, idempotency guard, executor trait, and in-memory control-plane tests. First verification: `cargo +1.89.0 test --locked -p mir2-admin-api -- --test-threads=1` (5/5). |
| [~] | Build admin HTTP and web console foundation | Coordinator | `apps/admin-api`, `apps/admin-web`, docs | `apps/admin-api` now exposes Axum routes and repository traits; `SendSystemMail` is wired to a domain outbox executor. `apps/admin-web` implements the first desktop operations UI and forwards GM mail commands to Rust through `/api/admin/system-mail`. Live game-state mail delivery, Postgres repositories, and real operator auth remain next-step work. |

## Product Evolution Round: 2026-04-27-R229

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Land first Postgres persistence slice and admin outbox boundary | Coordinator | `apps/admin-api`, `infra/postgres/migrations/0001_core.sql`, docs | Added Postgres command/audit adapters selected by `ADMIN_DATABASE_URL`, an `AdminOutboxRepository` with in-memory and Postgres implementations, the first core Postgres schema for admin and account/character tables, `import-account-store` for JSON-to-Postgres migration, and `dispatch-admin-outbox` for NATS publish. Verified Rust tests 8/8, fmt, compose config, diff check, live Docker Postgres import, live Admin API Postgres command/audit/outbox write, and live NATS publish/dispatched state. |
| [x] | Mirror gameplay JSON account-store saves into Postgres | Coordinator | `apps/simulation`, `apps/gateway`, `apps/admin-api`, docs | Added `MIR2_ACCOUNT_STORE_DATABASE_URL` mirror path. Docker smoke verified fallback GM mail wrote Stage 5 mail into Postgres `character_saves.stage5_systems_json`; JSON remains source of truth until a dedicated Postgres gameplay repository replaces it. Verified simulation config 11/11, admin-api 8/8, gateway 55/55, fmt, diff check, and healthy Docker core services. |
| [x] | Add explicit Postgres account-store source-of-truth mode | Coordinator | `apps/simulation`, `apps/gateway`, `apps/admin-api`, `infra/postgres/migrations/0001_core.sql`, docs | Added `MIR2_ACCOUNT_STORE_BACKEND=postgres`, Postgres load from `accounts.raw_json`, source-mode transaction/row-lock save, and `store_version` / `save_version` increments. Docker smoke verified source-mode fallback mail and version increments. Verified simulation config 11/11, admin-api 8/8, gateway 55/55, fmt, compose config/healthy services, and diff check. |

## Product Evolution Round: 2026-04-27-R232

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Add first gateway session-cache boundary and Redis adapter | Coordinator | `apps/simulation`, `apps/gateway`, docs | Added `ActiveSessionIdentity`, `GatewaySessionCache`, `InMemoryGatewaySessionCache`, cache record refresh/remove helpers, web-gateway write-through refresh after authoritative saves, and optional Redis cache selected by `MIR2_GATEWAY_REDIS_CACHE_URL` with TTL support. Verified focused gateway cache tests 5/5, including Redis roundtrip/remove/expire. |

## Product Evolution Round: 2026-04-27-R233

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Harden Postgres account-store source mode against stale writers | Coordinator | `apps/simulation`, docs | Source-mode account stores now retain loaded account/save versions, reject stale writers on `store_version` / `save_version` mismatch, and refresh local version metadata after successful source saves. Docker Postgres integration tests cover stale writer rejection and reload-save success. |

## Product Evolution Round: 2026-04-27-R234

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Add Admin API production-boundary hardening | Coordinator | `apps/admin-api`, docs | Added optional bearer operator token validation, high-risk command approval-id validation, item/gold grant executors routed through audited system-mail delivery, and outbox retry/dead-letter status transitions. Verified admin-api 11/11. |

## Product Evolution Round: 2026-04-27-R235

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Add Redpanda and ClickHouse local event analytics stack | Coordinator | `infra/docker-compose.dev.yml`, `infra/clickhouse/initdb/001_admin_events.sql`, docs | Redpanda and ClickHouse are now part of the local Compose event/analytics baseline. ClickHouse consumes Redpanda topic `admin.command.succeeded` into `mir2_events.admin_command_events`. NATS remains the existing command/notification dispatch path; Redpanda/ClickHouse are not gameplay authority. |

## Product Evolution Round: 2026-04-27-R236

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Wire real Admin outbox events to Redpanda and ClickHouse | Coordinator | `apps/admin-api`, `apps/admin-web`, `infra`, docs | Added admin event envelopes, Redpanda Pandaproxy publishing in `dispatch-admin-outbox`, ClickHouse `admin_events` projection, Admin API `/admin/events`, and Admin Web Audit event stream. NATS remains the notification dispatcher; Redpanda/ClickHouse remain analytics/read-side infrastructure. |

## Product Evolution Round: 2026-04-27-R237

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Harden admin outbox multi-publisher delivery semantics | Coordinator | `apps/admin-api`, `infra/postgres/migrations/0001_core.sql`, `apps/admin-web`, docs | Added per-publisher outbox delivery columns, independent NATS/Redpanda delivery attempts, retry/dead-letter behavior for partial publisher failure, ClickHouse event filters/degraded reads, and Admin Web Audit filters. Verified partial-failure DB state, successful NATS+Redpanda+ClickHouse smoke, API filter/degraded smoke, Rust tests, web/admin-web type checks, fmt, and diff check. |

## Product Evolution Round: 2026-04-27-R238

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Expand Admin command analytics beyond success events | Coordinator | `apps/admin-api`, `apps/admin-web`, `infra/clickhouse`, docs | Terminal Postgres-backed commands now enqueue `admin.command.succeeded`, `admin.command.failed`, or `admin.command.denied` envelopes. ClickHouse Kafka source subscribes to all three topics with a v2 group, and Admin Web Audit exposes denied status filtering. Verified denied event through real API permission rejection and failed event through Redpanda/ClickHouse readback. |

## Product Evolution Round: 2026-04-27-R239-R244

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Add persistent Admin approval workflow | Coordinator | `apps/admin-api`, `apps/admin-web`, `infra/postgres/migrations/0001_core.sql`, `infra/clickhouse`, docs | Added `admin_approvals`, approval API routes, Admin Web Approvals page, approval gates for high-risk commands, and approval requested/approved/rejected outbox events projected through Redpanda/ClickHouse. |
| [x] | Harden outbox production lifecycle and JetStream mode | Coordinator | `apps/admin-api/src/bin/dispatch-admin-outbox.rs`, `infra/clickhouse`, docs | Dispatcher now supports `ADMIN_OUTBOX_NATS_MODE=jetstream`, creates the configured stream, publishes with JetStream ack, and emits non-recursive `admin.outbox.retry` / `admin.outbox.dead_letter` Redpanda lifecycle events. |
| [x] | Add broader GM executors | Coordinator | `apps/admin-api`, `apps/gateway`, `apps/simulation`, docs | Added Admin API routes for item grant, gold grant, kick player, and ban account. Kick calls gateway character routing removal; ban persists on account records and simulation rejects banned login/start-game. |
| [x] | Harden Postgres source-mode conflicts | Coordinator | `apps/simulation`, `infra/postgres/migrations/0001_core.sql`, docs | Added account ban columns and a focused Docker Postgres test for stale `save_version` conflict after account version refresh. Existing reload-save and stale account writer coverage remains. |
| [x] | Extend Redis session/routing cache | Coordinator | `apps/gateway/src/cache.rs`, docs | Redis cache now writes a character-name routing index with the same TTL as the authoritative session cache record. In-memory and Redis remove-by-character tests prove kick routing equivalence. |
| [x] | Add Admin timeline read model and auth wiring | Coordinator | `apps/admin-api`, `apps/admin-web`, docs | Added `/admin/timeline` merging command/audit/approval/ClickHouse event records, Admin Web Timeline page, and Admin Web bearer-token forwarding when `ADMIN_OPERATOR_TOKEN` is set. |

## Product Evolution Round: 2026-04-28-R245

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Make the local admin backend browser-testable | Coordinator | `apps/admin-api`, `apps/admin-web`, docs | Added `ADMIN_OPERATOR_POLICY_PATH` operator policy loading, default self-approval blocking with local `ADMIN_APPROVAL_ALLOW_SELF=true` override, and Admin Web GM forms for grant item, grant gold, kick player, and ban account. Started Docker infra, Gateway, Admin API, and Admin Web; smoke-verified API/Gateway health and `/gm-tools`. |

## Product Evolution Round: 2026-04-27-R227

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Land Admin API repository/HTTP foundation and Admin Web UI | Coordinator | `apps/admin-api`, `apps/admin-web`, `docs/ADMIN-OPERATIONS-ARCHITECTURE.md`, docs/screenshots | Added `AdminCommandRepository` and `AuditRepository` traits, in-memory command/audit stores, Axum HTTP routes, `SendSystemMail` domain executor/outbox, standalone Next admin console pages, Next proxy route for GM mail, docs, and smoke screenshots. Verified by Rust locked tests/fmt, admin-web typecheck/build, direct Rust API curl write, Next proxy curl write, and Playwright screenshots. |

## Product Evolution Round: 2026-04-27-R228

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Connect audited GM system mail to live game-visible Stage 5 mail | Coordinator | `apps/admin-api`, `apps/gateway`, `apps/simulation`, `apps/web`, `apps/admin-web`, `docs/ADMIN-OPERATIONS-ARCHITECTURE.md` | Added live gateway delivery for `SendSystemMail`, persistent account-store fallback, a gateway admin mail endpoint, in-game Mail panel claim/delete actions, and a gateway endpoint unit test. Verified by focused simulation/admin-api/gateway tests, web/admin-web typecheck/build, Admin Web curl through Rust API, outbox `deliveryMode: "gateway_live"`, account-store inspection, gateway WS snapshot mail visibility, and WS `mail.claim` state mutation. |

## Completed Round: 2026-04-26-R225

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Refreshed Mac-local Candidate regression and Windows handoff | Coordinator | `apps/gateway/src/bin/packet_trace.rs`, `apps/web/scripts/smoke-stage5-ui.mjs`, `apps/gateway/README.md`, `docs/WINDOWS-CONTINUATION.md`, `docs/generated/packet-traces/r225-matrix/*`, `docs/stage5-screenshots/*`, docs | Added manifest summary counts to Stage 5 UI smoke and packet trace matrix summary counts to `latest-matrix.json`; fixed the summary field to use `compactTextLayout.checked`; refreshed Stage 5/map/minimap/WS evidence; wrote R225 packet trace matrix artifacts; cleaned stale gateway README status; and added the Windows continuation checklist. Verified by web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke, map/minimap smokes, WS load, Rust package tests, `fmt --check`, and `diff --check`. |

## Completed Round: 2026-04-26-R224

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Restored local packet trace matrix harness | Coordinator | `apps/gateway/src/bin/packet_trace.rs`, `docs/generated/packet-traces/r224-matrix/*`, docs | Reintroduced `packet_trace` with `--list-flows`, single-flow capture, matrix artifact writing, local/Crystal endpoint capture, diff summaries, fixture metadata, and require-mode enforcement. Local gateway on `127.0.0.1:7310` passed `MIR2_PACKET_TRACE_REQUIRE_LOCAL=1 cargo +1.89.0 run --locked -p mir2-gateway --bin packet_trace -- --matrix` with 9 artifacts and 17 intentionally skipped non-TCP matrix entries. `cargo +1.89.0 test --locked -p mir2-gateway -- --test-threads=1` passed 53/53. Live Crystal diff remains blocked until `MIR2_CRYSTAL_TCP_ADDR` is provided. |

## Completed Round: 2026-04-26-R223

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Completed the 100% Candidate automated evidence gate | Coordinator | `apps/web/scripts/smoke-stage5-ui.mjs`, `docs/stage5-screenshots/*`, `docs/generated/*`, docs | R223 added advanced Stage 5 systems smoke evidence for trade item/cancel, shop gold purchase, auction buy/cancel, conquest end, hero behaviour, mining/craft, and mail delete state; added compact Mail/Report panel bounds screenshots; refreshed map/minimap/WS evidence; and reran full web/Rust validation. The then-missing `packet_trace` bin target was closed in R224. |

## Completed Round: 2026-04-26-R222

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Completed the 90% frontend/global evidence batch | Coordinator | `apps/web/app/original-client-shell.tsx`, `apps/web/app/page.tsx`, `apps/web/app/globals.css`, `apps/web/scripts/smoke-stage5-ui.mjs`, `apps/web/scripts/smoke-crystal-map-api.mjs`, `apps/web/scripts/smoke-crystal-minimap-assets.mjs`, `docs/stage5-screenshots/*`, `docs/generated/*`, docs | R219-R222 added login/select lifecycle smoke evidence, character delete/recreate evidence, archived map API/minimap smoke JSON, refreshed WS load, compact multi-panel bounds, compact system-menu overflow fix, and NPC dialog link-capable rendering. Stage 5 UI smoke now captures 85 screenshots. Validation: web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke, map/minimap smokes, WS load, `cargo +1.89.0 fmt --check`, and `git diff --check`. |

## Completed Round: 2026-04-26-R218

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Added compact inventory panel layout evidence and completed the 80% target batch | Coordinator | `apps/web/scripts/smoke-stage5-ui.mjs`, `docs/stage5-screenshots/*`, docs | R210-R218 added Mail/Report/NPC/system menu panel state, broad systems state, guild/group chat filters, Character repair/special-repair UI, ground item/gold pickup, combat target state, system menu transfer-list routing, Battle Focus casting, and compact inventory bounds evidence. Stage 5 UI smoke now captures 71 screenshots and writes the extended manifest. Validation: web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 71 screenshots, visual screenshot inspection, `cargo +1.89.0 fmt --check`, and `git diff --check`. |

## Completed Round: 2026-04-26-R209

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Added storage password submit/no-service smoke evidence | Coordinator | `apps/web/scripts/smoke-stage5-ui.mjs`, `docs/stage5-screenshots/*`, docs | Stage 5 UI smoke now fills Set Storage Password, verifies mismatched confirmation keeps submit disabled and shows the mismatch warning, submits matching `Safe123` without an active storage service, verifies `hasStoragePassword` remains false with no-service chat feedback, captures `stage5-storage-password-mismatch.png` and `stage5-storage-password-submit-no-service.png`, and records the extended `storagePasswordFlow`. Validation: web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 60 screenshots, visual screenshot inspection, `cargo +1.89.0 fmt --check`, and `git diff --check`. |

## Completed Round: 2026-04-26-R208

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Enabled and smoke-verified storage password panel entry | Coordinator | `apps/web/app/original-client-shell.tsx`, `apps/web/app/page.tsx`, `apps/web/scripts/smoke-stage5-ui.mjs`, `docs/stage5-screenshots/*`, docs | Protect is now reachable when no storage password is set. Stage 5 UI smoke opens Set Storage Password, verifies title/prompt/input count/disabled submit/debug storage password state, closes the panel without submitting credentials, captures `stage5-storage-password-panel.png`, and records `storagePasswordFlow`. Validation: web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 58 screenshots, visual screenshot inspection, `cargo +1.89.0 fmt --check`, and `git diff --check`. |

## Completed Round: 2026-04-26-R207

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Added storage Take Back no-service smoke evidence | Coordinator | `apps/web/scripts/smoke-stage5-ui.mjs`, `docs/stage5-screenshots/*`, docs | Stage 5 UI smoke now opens Take Back for stored Red Potion, selects an inventory slot without an active storage service, verifies bag1 Red Potion remains quantity 3 and storage Red Potion remains quantity 10, captures `stage5-storage-takeback-red-potion-selected.png`, `stage5-storage-takeback-red-potion-result.png`, and `stage5-storage-takeback-red-potion-feedback.png`, and records `storageTakeBackFlow`. Validation: web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 57 screenshots, visual screenshot inspection, `cargo +1.89.0 fmt --check`, and `git diff --check`. |

## Completed Round: 2026-04-26-R206

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Added storage Store Item no-service smoke evidence | Coordinator | `apps/web/app/page.tsx`, `apps/web/scripts/smoke-stage5-ui.mjs`, `docs/stage5-screenshots/*`, docs | Stage 5 UI smoke now opens Store Item for Dagger, selects a warehouse slot without an active storage service, verifies Dagger remains in bag1 slot 4 and existing storage items are preserved, exposes `storageItems` in Stage 5 debug state, captures `stage5-storage-store-dagger-selected.png`, `stage5-storage-store-dagger-result.png`, and `stage5-storage-store-dagger-feedback.png`, and records `storageStoreFlow`. Validation: web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 54 screenshots, visual screenshot inspection, `cargo +1.89.0 fmt --check`, and `git diff --check`. |

## Completed Round: 2026-04-26-R205

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Added inventory Sell Item no-service smoke evidence | Coordinator | `apps/web/scripts/smoke-stage5-ui.mjs`, `docs/stage5-screenshots/*`, docs | Stage 5 UI smoke now opens Sell Item for Dagger, confirms without an active sell service, verifies Dagger remains in bag1 slot 4 and gold stays at 1180, captures `stage5-inventory-sell-dagger-panel.png` and `stage5-inventory-sell-dagger-no-service.png`, and records `inventorySellFlow`. Validation: web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 51 screenshots, visual screenshot inspection, `cargo +1.89.0 fmt --check`, and `git diff --check`. |

## Completed Round: 2026-04-26-R204

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Added belt mouse-use smoke evidence | Coordinator | `apps/web/scripts/smoke-stage5-ui.mjs`, `docs/stage5-screenshots/*`, docs | Stage 5 UI smoke now clicks Red Potion directly in the belt, verifies belt quantity drops from 5 to 4, keeps the existing hotkey path verifying 4 to 3, captures `stage5-belt-mouse-use-red-potion.png`, and records `beltMouseUseFlow`. Validation: web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 49 screenshots, visual screenshot inspection, `cargo +1.89.0 fmt --check`, and `git diff --check`. |

## Completed Round: 2026-04-26-R203

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Fixed and verified Character equipment remove | Coordinator | `apps/web/app/page.tsx`, `apps/web/scripts/smoke-stage5-ui.mjs`, `docs/stage5-screenshots/*`, docs | Character RemoveItem now targets the `inventory` grid and chooses the first free bag1 slot instead of hardcoding occupied slot 0 / invalid `equipment` grid. Stage 5 UI smoke verifies Dagger leaves the weapon slot and returns to bag1 slot 4, captures `stage5-character-remove-dagger.png`, and records `characterRemoveFlow`. Validation: web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 48 screenshots, visual screenshot inspection, `cargo +1.89.0 fmt --check`, and `git diff --check`. |

## Completed Round: 2026-04-26-R202

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Added inventory item-drop smoke evidence | Coordinator | `apps/web/scripts/smoke-stage5-ui.mjs`, `docs/stage5-screenshots/*`, docs | Stage 5 UI smoke now opens Delete Item for Blue Potion, confirms the drop, verifies quantity drops from 3 to 2 and a `Blue Potion` ground label appears, captures `stage5-inventory-drop-blue-potion-panel.png` and `stage5-inventory-drop-blue-potion.png`, and records `inventoryDropFlow`. Validation: web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 47 screenshots, visual screenshot inspection, `cargo +1.89.0 fmt --check`, and `git diff --check`. |

## Completed Round: 2026-04-26-R201

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Added inventory Split Item smoke evidence | Coordinator | `apps/web/scripts/smoke-stage5-ui.mjs`, `docs/stage5-screenshots/*`, docs | Stage 5 UI smoke now opens Split Item for Red Potion, confirms count 1, verifies inventory quantity drops from 4 to 3 while belt quantity rises from 5 to 6 and total Red Potion quantity stays 9, captures `stage5-inventory-split-red-potion-panel.png` and `stage5-inventory-split-red-potion.png`, and records `inventorySplitFlow`. Validation: web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 45 screenshots, visual screenshot inspection, `cargo +1.89.0 fmt --check`, and `git diff --check`. |

## Completed Round: 2026-04-26-R200

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Added inventory item-move smoke evidence | Coordinator | `apps/web/scripts/smoke-stage5-ui.mjs`, `docs/stage5-screenshots/*`, docs | Stage 5 UI smoke now context-clicks Wooden Sword in bag1, moves it from slot 4 to slot 10, captures `stage5-inventory-move-wooden-sword.png`, and records `inventoryMoveFlow`. Validation: web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 43 screenshots, visual screenshot inspection, `cargo +1.89.0 fmt --check`, and `git diff --check`. |

## Completed Round: 2026-04-26-R199

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Added inventory Drop Gold smoke evidence | Coordinator | `apps/web/app/page.tsx`, `apps/web/app/original-client-shell.tsx`, `apps/web/scripts/smoke-stage5-ui.mjs`, `docs/stage5-screenshots/*`, docs | Stage 5 debug state now exposes `gold`; UI smoke opens Drop Gold, confirms 100 gold, verifies gold drops from 1280 to 1180 and a `100 Gold x100` ground label appears, captures `stage5-inventory-drop-gold-panel.png` and `stage5-inventory-drop-gold.png`, and records `inventoryGoldFlow`. Missing `ui.confirm` fallback text is fixed. Validation: web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 42 screenshots, visual screenshot inspection, `cargo +1.89.0 fmt --check`, and `git diff --check`. |

## Completed Round: 2026-04-26-R198

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Added HUD Skill/Option button smoke evidence | Coordinator | `apps/web/scripts/smoke-stage5-ui.mjs`, `docs/stage5-screenshots/*`, docs | Stage 5 UI smoke now clicks HUD Skill to open Character Spells and HUD Option to open Stats II, captures `stage5-hud-skill-spells.png` and `stage5-hud-option-stats2.png`, and records `hudButtonFlow`. Validation: web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 40 screenshots, visual screenshot inspection, `cargo +1.89.0 fmt --check`, and `git diff --check`. |

## Completed Round: 2026-04-26-R197

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Added inventory equipment smoke evidence | Coordinator | `apps/web/app/page.tsx`, `apps/web/scripts/smoke-stage5-ui.mjs`, `docs/stage5-screenshots/*`, docs | Stage 5 debug state now exposes `equipmentItems`; UI smoke clicks Dagger in bag1, verifies Dagger moves into the weapon equipment slot, captures `stage5-inventory-equip-dagger.png`, and records `inventoryEquipFlow`. Validation: web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 38 screenshots, visual screenshot inspection, `cargo +1.89.0 fmt --check`, and `git diff --check`. |

## Completed Round: 2026-04-26-R196

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Added inventory item-use smoke evidence | Coordinator | `apps/web/scripts/smoke-stage5-ui.mjs`, `docs/stage5-screenshots/*`, docs | Stage 5 UI smoke now clicks Red Potion in bag1, verifies the quantity drops from 5 to 4, captures `stage5-inventory-use-red-potion.png`, and records `inventoryUseFlow`. Validation: web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 37 screenshots, visual screenshot inspection, `cargo +1.89.0 fmt --check`, and `git diff --check`. |

## Completed Round: 2026-04-26-R195

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Added expanded storage rent smoke evidence | Coordinator | `apps/web/app/page.tsx`, `apps/web/scripts/smoke-stage5-ui.mjs`, `docs/stage5-screenshots/*`, docs | Stage 5 debug state now exposes `hasExpandedStorage`; UI smoke clicks Rent from locked storage page 2, verifies page 2 becomes unlocked with expanded storage active and 160-slot capacity text, captures `stage5-storage-page2-rented.png`, and records the rented state in `storageFlow`. Validation: web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 36 screenshots, visual screenshot inspection, `cargo +1.89.0 fmt --check`, and `git diff --check`. |

## Completed Round: 2026-04-26-R194

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Added system menu action smoke evidence | Coordinator | `apps/web/scripts/smoke-stage5-ui.mjs`, `docs/stage5-screenshots/*`, docs | Stage 5 UI smoke now records `systemMenuFlow` for menu open and Character, Inventory, and Quest menu actions; captures `stage5-system-menu.png`, `stage5-system-menu-character.png`, `stage5-system-menu-inventory.png`, and `stage5-system-menu-quest.png`; and verifies transfer/action labels plus resulting panels. Validation: web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 35 screenshots, visual screenshot inspection, `cargo +1.89.0 fmt --check`, and `git diff --check`. |

## Completed Round: 2026-04-26-R193

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Added chat control smoke evidence | Coordinator | `apps/web/scripts/smoke-stage5-ui.mjs`, `docs/stage5-screenshots/*`, docs | Stage 5 UI smoke now records `chatFlow` for All, Shout filter, All restored, Settings open, collapsed, expanded restored, and Report open; captures `stage5-chat-shout-filter.png`, `stage5-chat-settings.png`, `stage5-chat-collapsed.png`, and `stage5-chat-report.png`; and verifies DOM state transitions. Validation: web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 31 screenshots, visual screenshot inspection, `cargo +1.89.0 fmt --check`, and `git diff --check`. |

## Completed Round: 2026-04-26-R192

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Added storage page navigation smoke evidence | Coordinator | `apps/web/scripts/smoke-stage5-ui.mjs`, `docs/stage5-screenshots/*`, docs | Stage 5 UI smoke now records storage page 1, locked page 2, and restored page 1 states in `storageFlow`; captures `stage5-storage-page2-locked.png` and `stage5-storage-page1-restored.png`; and verifies locked expanded-storage text plus restored item counts. Validation: web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 27 screenshots, visual screenshot inspection, `cargo +1.89.0 fmt --check`, and `git diff --check`. |

## Completed Round: 2026-04-26-R191

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Added character tab smoke evidence | Coordinator | `apps/web/app/page.tsx`, `apps/web/scripts/smoke-stage5-ui.mjs`, `docs/stage5-screenshots/*`, docs | Stage 5 debug state now exposes `activeCharacterTab` and `knownSkills`; UI smoke switches char -> stats1 -> stats2 -> spells -> char, captures `stage5-character-stats1.png`, `stage5-character-stats2.png`, `stage5-character-spells.png`, and `stage5-character-char-restored.png`, and records `characterFlow` with equipment/stat/spell counts. Validation: web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 25 screenshots, visual screenshot inspection, `cargo +1.89.0 fmt --check`, and `git diff --check`. |

## Completed Round: 2026-04-26-R190

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Added inventory tab smoke evidence | Coordinator | `apps/web/app/page.tsx`, `apps/web/scripts/smoke-stage5-ui.mjs`, `docs/stage5-screenshots/*`, docs | Stage 5 debug state now exposes `inventoryItems` and `activeInventoryTab`; UI smoke switches bag1 -> bag2 -> quest -> bag1, captures `stage5-inventory-bag2.png`, `stage5-inventory-quest.png`, and `stage5-inventory-bag1-restored.png`, and records `inventoryFlow` with item counts and quest entry count. Validation: web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 21 screenshots, visual screenshot inspection, `cargo +1.89.0 fmt --check`, and `git diff --check`. |

## Completed Round: 2026-04-26-R189

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Added belt hotkey-use smoke evidence | Coordinator | `apps/web/app/page.tsx`, `apps/web/scripts/smoke-stage5-ui.mjs`, `docs/stage5-screenshots/*`, docs | Stage 5 debug state now exposes `beltItems`; UI smoke presses hotkey `1`, waits for slot-1 Red Potion quantity to fall from 5 to 4, captures `stage5-belt-hotkey-use.png`, and records `beltUseFlow`. Validation: web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 18 screenshots, visual screenshot inspection, `cargo +1.89.0 fmt --check`, and `git diff --check`. |

## Completed Round: 2026-04-26-R188

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Added belt interaction smoke evidence | Coordinator | `apps/web/app/globals.css`, `apps/web/lib/original-ui.ts`, `apps/web/scripts/smoke-stage5-ui.mjs`, `docs/stage5-screenshots/*`, docs | Stage 5 UI smoke now records horizontal, vertical, rotate-back, and closed belt states in `beltFlow`; captures `stage5-belt-vertical.png`, `stage5-belt-horizontal.png`, and `stage5-belt-closed.png`; fixes doubled belt slot-label offsets; moves the vertical belt clear of Quest; and asserts labels stay inside the belt with no Quest overlap. Validation: web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 17 screenshots, visual screenshot inspection, `cargo +1.89.0 fmt --check`, and `git diff --check`. |

## Completed Round: 2026-04-26-R187

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Added minimap interaction smoke evidence | Coordinator | `apps/web/scripts/smoke-stage5-ui.mjs`, `docs/stage5-screenshots/*`, docs | Stage 5 UI smoke now clicks minimap collapse, BigMap re-expand, and Mail open paths; captures `stage5-minimap-collapsed.png`, `stage5-minimap-expanded.png`, and `stage5-minimap-mail.png`; and writes `minimapFlow` state to the manifest. Validation: web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 14 screenshots, visual screenshot inspection, `cargo +1.89.0 fmt --check`, and `git diff --check`. |

## Completed Round: 2026-04-26-R186

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Added compact visible-text overflow checks | Coordinator | `apps/web/app/original-client-shell.tsx`, `apps/web/app/globals.css`, `apps/web/scripts/smoke-stage5-ui.mjs`, `docs/stage5-screenshots/*`, docs | Stage 5 UI smoke now checks visible core quest/HUD/minimap/belt/chat/entity text at compact viewport and writes `compactTextLayout`; the check caught minimap title overflow, fixed by splitting map title and Safe Zone into stable two-line text. Validation: web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 11 screenshots and 33 compact text nodes checked, visual screenshot inspection, `cargo +1.89.0 fmt --check`, and `git diff --check`. |

## Completed Round: 2026-04-26-R185

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Added desktop/compact Stage 5 screenshot evidence | Coordinator | `apps/web/scripts/smoke-stage5-ui.mjs`, `docs/stage5-screenshots/*`, docs | Stage 5 UI smoke now records desktop 1024x768 and compact 820x640 viewports, captures `stage5-compact-game.png`, writes compact layout bounds into the manifest, and fails on core stage/HUD/chat/minimap overflow. Validation: `node --check`, gateway/web health, Stage 5 UI smoke with 11 screenshots, compact screenshot visual inspection, `cargo +1.89.0 fmt --check`, and `git diff --check`. |

## Completed Round: 2026-04-26-R184

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Advanced frontend/global smoke parity | Coordinator | `apps/web/app/original-client-shell.tsx`, `apps/web/app/page.tsx`, `apps/web/lib/crystal-map-loader.ts`, `apps/web/scripts/smoke-stage5-ui.mjs`, `docs/stage5-screenshots/*`, `docs/generated/load/latest-ws.json`, docs | Chat panel now defaults/follows latest filtered lines with a live scroll knob; no-WebGL headless browsers stay on DOM UI instead of Bevy panic; Crystal map API uses packaged starter-region fallback when local Crystal map files are missing; Stage 5 UI smoke detects macOS Chrome. Validation: web `tsc --noEmit`, direct `next build`, minimap smoke, map API smoke, Stage 5 UI smoke (10 screenshots), gateway health 7110, WS load 64/64, `cargo +1.89.0 fmt --check`, `git diff --check`. |

## Completed Round: 2026-04-26-R183

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Moved quest interaction hint out of runtime `sim` namespace | Coordinator | `apps/simulation/src/runtime.rs`, `packages/tooling/scripts/import-crystal-localization.mjs`, `packages/game-data/data/generated/localization_bundle.json`, `apps/web/lib/generated/localization_bundle.json`, docs | UI/localization namespace cleanup: `build_interaction_hints` now uses `custom.interaction.questHint`, generated bundles and importer are in sync, and runtime has no `sim.*` references; `mir2-game-data` (22/22); focused snapshot test (1/1); `cargo +1.89.0 fmt --check`; full `mir2-simulation` (664/664). |

## Completed Round: 2026-04-26-R182

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Removed no-script NPC idle fallback dialog | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal packet-surface parity: no-script/no-page NPC interaction now silently returns existing packets like Crystal `NPCScript.Call` with no matching page, instead of opening runtime-only idle dialog text; focused no-script NPC (1/1); adjacent `npc_interaction` (2/2); broad `crystal_npc` (52/52); `cargo +1.89.0 fmt --check`; full `mir2-simulation` (664/664). |

## Completed Round: 2026-04-26-R181

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Localized quest-required drop feedback | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal localization/packet-surface parity: quest-required drop feedback now uses Crystal `server.YouFound` and no longer emits runtime-only `sim.youSecuredQuestItem`, `sim.questReturnForReward`, or `sim.questProgressWasps` progress chats; `GainedItem` and quest state updates remain intact; focused quest-required drop (1/1); adjacent `quest_required_drop` (3/3); `cargo +1.89.0 fmt --check`; full `mir2-simulation` (664/664). |

## Completed Round: 2026-04-26-R180

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Localized start-game welcome chat | Coordinator | `apps/simulation/src/runtime.rs`, `apps/gateway/src/session.rs`, docs | Crystal localization/packet-surface parity: `StartGame` welcome chat now uses `server.Welcome` with localized `server.GameName` and `ChatType::Hint` instead of runtime-only `sim.welcomeCharacter` System text; focused simulation/gateway `start_game_emits_bootstrap_sequence` (1/1 each); `cargo +1.89.0 fmt --check`; full `mir2-simulation` (664/664); full `mir2-gateway` (47/47). |

## Completed Round: 2026-04-26-R179

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Removed normal chat runtime echo | Coordinator | `apps/simulation/src/runtime.rs`, `apps/gateway/src/session.rs`, docs | Crystal packet-surface parity: normal `ClientPacket::Chat` before `StartGame` now returns no packets, and in-game normal chat emits only `ObjectChat` with `Name: message` instead of a runtime-only `sim.echoChat` self `Chat` echo; `@ADDSTORAGE` remains as the modeled helper command; simulation `chat_` (43/43); gateway `chat_` (2/2); `cargo +1.89.0 fmt --check`; full `mir2-simulation` (664/664); full `mir2-gateway` (47/47). |

## Completed Round: 2026-04-26-R178

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Removed cast-skill failure runtime chats | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal packet-surface parity: high-level `cast_skill` unknown-skill, cooldown, unwired-definition, missing-player, no-MP, unwired summon-spell, and missing summon-template failures no longer emit runtime-only `sim.skillNotKnown`, `sim.skillCooldown`, `sim.skillLogicNotWired`, `sim.playerNotInWorld`, or `sim.notEnoughMp`; successful buff/summon behavior remains intact; `casting` (9/9); `cargo +1.89.0 fmt --check`; full `mir2-simulation` (663/663). |

## Completed Round: 2026-04-26-R177

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Removed MoveItem unsupported fallback runtime chat | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal packet-surface parity: unreachable/unsupported `MoveItem` missing-source fallback no longer emits `sim.itemNotFoundInBag`; unsupported grids remain failed-ack only, while Inventory/Storage missing-source keeps Crystal `server.ItemMoveErrorReport`; `move_item` (26/26); `cargo +1.89.0 fmt --check`; full `mir2-simulation` (660/660). |

## Completed Round: 2026-04-26-R176

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Removed stale active-dialog missing-NPC/no-script runtime chats | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal packet-surface parity: active NPC dialog target follow-up with a missing NPC entity or an NPC lacking script metadata now dismisses silently without `sim.targetNotGroundDrop` or `sim.npcNoMilestoneScript`; ordinary no-script NPC idle fallback remains intact; focused stale-dialog tests (2/2), adjacent `npc_interaction` (2/2), broad `crystal_npc` (52/52); `cargo +1.89.0 fmt --check`; full `mir2-simulation` (660/660). |

## Completed Round: 2026-04-26-R175

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Removed NPC dialog helper no-active/invalid-target/no-input runtime chats | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal packet-surface parity: high-level dialog target/input helper no-active-dialog, invalid-target, and no-pending-input failures no longer emit `sim.npcNoMilestoneScript` or `sim.itemNoActiveUse`; successful dialog link/input/service flows remain intact; focused dialog-helper tests (3/3), adjacent `npc_interaction` (2/2), broad `crystal_npc` (52/52); `cargo +1.89.0 fmt --check`; full `mir2-simulation` (658/658). |

## Completed Round: 2026-04-26-R174

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Removed direct NPC interaction invalid target/direction/range runtime chats | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal packet-surface parity: high-level `interact(object_id)` missing-target, same-tile/no-direction, and out-of-range failures no longer emit `sim.targetNoScriptedInteraction`, `sim.noValidInteractionDirection`, or `sim.moveCloserToTalkToNpc`; successful NPC dialog/script/service flows remain intact; focused direct-interact tests (3/3), adjacent `npc_interaction` (2/2), broad `crystal_npc` (52/52); `cargo +1.89.0 fmt --check`; full `mir2-simulation` (655/655). |

## Completed Round: 2026-04-26-R173

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Removed direct attack invalid target/state/range runtime chats | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal packet-surface parity: high-level `attack(object_id)` missing-target, non-monster, dead/hidden/stoned, no-direction, and out-of-range failures no longer emit runtime-only `sim.*` chats while preserving turn packets, normal attacks, hidden reveal, Zuma wake, and delayed hit behavior; focused direct-attack tests (4/4), hidden/Zuma focused tests (2/2), adjacent `attack` (80/80); `cargo +1.89.0 fmt --check`; full `mir2-simulation` (652/652). |

## Completed Round: 2026-04-26-R172

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Removed successful NPC interaction runtime chat | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal packet-surface parity: high-level NPC interaction no longer emits `sim.talkingToNpc`; NPC `ObjectChat`/dialog surfaces and Crystal script/service flows remain intact; focused `npc_interaction` (2/2), `crystal_npc_dialog` (1/1), `crystal_npc_service` (1/1), broad `crystal_npc` (52/52); `cargo +1.89.0 fmt --check`; full `mir2-simulation` (648/648). |

## Completed Round: 2026-04-26-R171

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Removed direct pickup invalid target/distance runtime chats | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal packet-surface parity: high-level `pick_up(object_id)` missing-object, non-ground-target, and out-of-cell failures now return silently instead of emitting `sim.itemNoLongerOnGround`, `sim.targetNotGroundDrop`, or `sim.moveCloserToPickItem`; Crystal owner/full-bag pickup messages and current-cell packet pickup behavior remain intact; focused direct-pickup tests (3/3); adjacent `pickup` (18/18), `drop` (42/42); `cargo +1.89.0 fmt --check`; full `mir2-simulation` (648/648). |

## Completed Round: 2026-04-26-R170

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Removed runtime-only missing defeated-entity chat | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal packet-surface parity: missing defeated-monster entity handling now silently returns without `sim.defeatedMonsterEntityMissing`, while normal death/drop packet surfaces remain intact; focused missing-entity silent test (1/1), visible death packet test (1/1); adjacent `drop` (41/41); `cargo +1.89.0 fmt --check`; full `mir2-simulation` (645/645). |

## Completed Round: 2026-04-26-R169

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Removed runtime-only monster death-drop success chats | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal packet-surface parity: monster death drop success paths no longer emit `sim.monsterDroppedGoldOnGround` / `sim.monsterDroppedItem` while preserving ground gold/item drops, quest-drop routing, and pickup packets; focused item-drop no-chat (1/1), focused gold-drop no-chat/pickup (1/1); adjacent `drop` (41/41), `pickup` (15/15), `attack` (76/76); `cargo +1.89.0 fmt --check`; full `mir2-simulation` (644/644). |

## Completed Round: 2026-04-26-R168

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Removed runtime-only summoned VampireSpider defeat chat | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal packet-surface parity: summoned VampireSpider death explosion no longer emits `sim.targetDefeated` while preserving explosion damage and summon despawn behavior; focused vampire-spider no-chat explosion test (1/1); adjacent `spider` (6/6), `attack` (76/76); `cargo +1.89.0 fmt --check`; full `mir2-simulation` (643/643). |

## Completed Round: 2026-04-26-R167

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Removed runtime-only ordinary combat damage narration | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal packet-surface parity: ordinary player/monster hit resolution no longer emits `sim.youHitTargetForDamage`, `sim.targetDefeated`, or `sim.monsterPressuresYouForDamage`; focused player-hit no-chat test (1/1); adjacent `attack` (76/76); `cargo +1.89.0 fmt --check`; full `mir2-simulation` (643/643). |

## Completed Round: 2026-04-26-R166

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Removed runtime-only cast-skill success chat | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal packet-surface parity: successful buff/heal and summon `cast_skill` paths no longer emit generic `sim.castSkill` chat while preserving state mutation/spawns; focused `casting` (6/6); `cargo +1.89.0 fmt --check`; full `mir2-simulation` (643/643). |

## Completed Round: 2026-04-26-R165

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Removed runtime-only cast-skill helper chat before `StartGame` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal packet-surface parity: high-level `cast_skill` now silently rejects before `StartGame`; focused pre-start cast-skill test (1/1); adjacent `casting` (6/6); `cargo +1.89.0 fmt --check`; full `mir2-simulation` (643/643). |

## Completed Round: 2026-04-26-R164

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Removed runtime-only interaction helper chats before `StartGame` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal packet-surface parity: high-level `interact` plus dialog target follow-up now silently reject before `StartGame`; focused pre-start interaction test (1/1); adjacent `npc_interaction` (2/2), `crystal_npc_dialog` (1/1), `crystal_npc_service` (1/1); `cargo +1.89.0 fmt --check`; full `mir2-simulation` (642/642). |

## Completed Round: 2026-04-26-R163

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Removed runtime-only harvest helper chats before `StartGame` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal packet-surface parity: high-level `harvest` plus packet `Harvest` now silently reject before `StartGame`; focused pre-start harvest test (1/1); adjacent `harvest` (9/9); `cargo +1.89.0 fmt --check`; full `mir2-simulation` (641/641). |

## Completed Round: 2026-04-26-R162

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Removed runtime-only attack helper chats before `StartGame` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal packet-surface parity: high-level `attack` plus packet `Attack` and `RangeAttack` now silently reject before `StartGame`; focused pre-start attack test (1/1); adjacent `attack` (76/76); combat trace focused test (1/1); `cargo +1.89.0 fmt --check`; full `mir2-simulation` (640/640). |

## Completed Round: 2026-04-26-R161

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Removed runtime-only movement/turning helper chats before `StartGame` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal packet-surface parity: high-level `move_to` plus packet `Walk`, `Run`, and `Turn` now silently reject before `StartGame`; focused pre-start movement test (1/1); adjacent `walk` (6/6), `run_` (3/3), `transfer_map` (2/2); `cargo +1.89.0 fmt --check`; full `mir2-simulation` (639/639). |

## Completed Round: 2026-04-26-R158

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Localized trainer average damage reporting and Crystal format placeholders | Coordinator | `packages/game-data/src/lib.rs`, `apps/simulation/src/runtime.rs`, docs | Crystal localization parity: `{index:format}` placeholders now substitute in localization templates and trainer idle average damage uses `server.AverageDamageOnTrainer`; `mir2-game-data` (22/22); focused trainer test (1/1); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (638/638). |

## Completed Round: 2026-04-26-R157

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Localized benediction-oil weapon luck outcome chats | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal localization parity: benediction-oil no-effect/luck/curse outcomes now use `server.WeaponNoEffect`, `server.WeaponLuck`, and `server.WeaponCurse`; focused `benediction_oil` (4/4); adjacent `use_item` (42/42); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (638/638). |

## Completed Round: 2026-04-26-R156

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Removed runtime-only expanded-storage helper success chat | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal packet-surface parity: `@ADDSTORAGE` now emits modeled `ResizeStorage` without hardcoded `"Expanded storage activated."` chat; focused `addstorage` (2/2); adjacent `storage` (43/43); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (638/638). |

## Completed Round: 2026-04-26-R155

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Localized group pickup notice through Crystal `server.FriendlyPickedUpItem` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal localization parity: `ShowGroupPickup` item notices now use the generated localization bundle instead of hardcoded English formatting; focused group pickup test (1/1); adjacent `pickup` (14/14); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (638/638). |

## Completed Round: 2026-04-26-R154

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Removed runtime-only high-level use/drop before-start chats | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal packet-surface parity: high-level `use_item(key)` and `drop_item(key)` before `StartGame` now emit no packets/chat while preserving post-start behavior; adjacent `drop_item` (10/10); focused consumable helper (1/1); adjacent `use_item` (42/42); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (638/638). |

## Completed Round: 2026-04-26-R153

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Removed runtime-only high-level drop helper missing-item chat | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal packet-surface parity: missing high-level `drop_item(key)` requests now emit no packets/chat and preserve state, aligned with packet `DropItem` missing-source behavior; focused drop helper test (1/1); adjacent `drop_item` (10/10); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (638/638). |

## Completed Round: 2026-04-26-R152

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Localized map-transfer not-in-world rejection through Crystal `server.NotFound` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: generated localization bundle contains `server.NotFound`; focused transfer-bound test (1/1); adjacent `transfer_map` (2/2); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (638/638). |

## Completed Round: 2026-04-26-R151

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Localized missing-template `RequestItemInfo` failure through Crystal `server.NotFound` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: generated localization bundle contains `server.NotFound`; focused request-item-info test (1/1); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (638/638). |

## Completed Round: 2026-04-26-R150

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Localized map-transfer bounds rejection through Crystal `server.CannotPositionMoveOnMap` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: generated localization bundle contains `server.CannotPositionMoveOnMap`; focused transfer-bounds test (1/1); adjacent `transfer_map` (2/2); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (638/638). |

## Completed Round: 2026-04-26-R149

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Removed remaining runtime-only Stage 5 event/hero helper success chats | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal packet-surface parity: `event.spawn` and `hero.behaviour` successes now mutate state without simulator-only narration; focused conquest/event/hero test (1/1); broader `stage5_` (26/26); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (638/638). |

## Completed Round: 2026-04-26-R148

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Removed runtime-only debug Crystal transfer success chat | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal packet-surface parity: debug `crystal:<map>:<x>:<y>` transfers now emit map/location packets without simulator-only `"Transferred to Crystal map ..."` chat; focused debug transfer test (1/1); adjacent `transfer_map` (2/2); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (638/638). |

## Completed Round: 2026-04-26-R147

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Removed generic runtime-only Stage 5 helper success chats while preserving helper state mutations | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal packet-surface parity: group/social/mail/trade/auction/conquest/hero/profession helper successes no longer emit simulator-only narration; focused `stage5_` (26/26); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (638/638). |

## Completed Round: 2026-04-26-R146

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Localized Stage 5 event-spawn missing-player/position rejections through Crystal `server.NotFound` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: generated localization bundle contains `server.NotFound`; focused `stage5_conquest_event_hero_mining_and_crafting_flow` (1/1); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (638/638). |

## Completed Round: 2026-04-26-R145

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Localized unknown map-transfer rejection through Crystal `server.NotFound` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: generated localization bundle contains `server.NotFound`; focused `transfer_map_requires_player_on_transfer_bounds` (1/1); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (638/638). |

## Completed Round: 2026-04-26-R144

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Localized Stage 5 unknown-command rejection through Crystal `server.InvalidPacketReceived` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: generated localization bundle contains `server.InvalidPacketReceived`; focused `stage5_trade_shop_and_auction_cancel_error_paths_preserve_gold` (1/1); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (638/638). |

## Completed Round: 2026-04-26-R143

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Localized Stage 5 inactive-trade rejections through Crystal `server.NotFound` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: generated localization bundle contains `server.NotFound`; focused `stage5_trade_shop_and_auction_cancel_error_paths_preserve_gold` (1/1); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (638/638). |

## Completed Round: 2026-04-26-R142

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Localized Stage 5 `auction.buy` / `auction.cancel` missing-id rejections through Crystal `server.InvalidPacketReceived` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: generated localization bundle contains `server.InvalidPacketReceived`; focused `stage5_trade_shop_and_auction_cancel_error_paths_preserve_gold` (1/1); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (638/638). |

## Completed Round: 2026-04-26-R141

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Localized Stage 5 `mail.claim` / `mail.delete` missing-id rejections through Crystal `server.InvalidPacketReceived` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: generated localization bundle contains `server.InvalidPacketReceived`; focused `stage5_social_group_guild_mail_persist_across_reload` (1/1); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (638/638). |

## Completed Round: 2026-04-26-R140

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Localized Stage 5 `trade.offerGold` missing-amount rejection through Crystal `server.InvalidPacketReceived` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: generated localization bundle contains `server.InvalidPacketReceived`; focused `stage5_trade_shop_and_auction_cancel_error_paths_preserve_gold` (1/1); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (638/638). |

## Completed Round: 2026-04-26-R139

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Localized Stage 5 hero-behaviour missing-hero rejection through Crystal `server.NotFound` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: generated localization bundle contains `server.NotFound`; focused `stage5_conquest_event_hero_mining_and_crafting_flow` (1/1); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (638/638). |

## Completed Round: 2026-04-26-R138

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Localized Stage 5 event-spawn missing-template rejection through Crystal `server.NotFound` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: generated localization bundle contains `server.NotFound`; focused `stage5_conquest_event_hero_mining_and_crafting_flow` (1/1); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (638/638). |

## Completed Round: 2026-04-26-R137

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Localized Stage 5 guild creation success chat through Crystal `server.SuccessfullyCreatedGuild` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: generated localization bundle contains `server.SuccessfullyCreatedGuild`; focused `stage5_social_group_guild_mail_persist_across_reload` (1/1); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (638/638). |

## Completed Round: 2026-04-26-R136

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Localized Stage 5 craft no-ore rejection through Crystal `server.CraftingAttemptFailed` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: generated localization bundle contains `server.CraftingAttemptFailed`; focused `stage5_conquest_event_hero_mining_and_crafting_flow` (1/1); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (638/638). |

## Completed Round: 2026-04-26-R135

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Localized Stage 5 credit-shop insufficient-credit rejection through Crystal `server.YouDontHaveEnoughCurrency` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: generated localization bundle contains `server.YouDontHaveEnoughCurrency`; focused `stage5_trade_shop_and_auction_cancel_error_paths_preserve_gold` (1/1); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (638/638). |

## Completed Round: 2026-04-26-R134

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Localized Stage 5 mail/trade/auction missing-entity rejection chats through Crystal `server.NotFound` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: generated localization bundle contains `server.NotFound`; focused `stage5_` (26/26); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (638/638). |

## Completed Round: 2026-04-26-R133

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Localized Stage 5 socket metadata-missing rejection chat through Crystal `server.NotFound` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: generated localization bundle contains `server.NotFound`; focused `stage5_item_` (16/16); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (636/636). |

## Completed Round: 2026-04-26-R132

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Localized Stage 5 socket/seal missing-equipped-item rejection chats through Crystal `server.NotFound` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: generated localization bundle contains `server.NotFound`; focused `stage5_item_` (15/15); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (635/635). |

## Completed Round: 2026-04-26-R131

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Localized Stage 5 socket/seal missing-source rejection chats through Crystal `server.NotFound` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: generated localization bundle contains `server.NotFound`; focused `stage5_item_` (13/13); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (633/633). |

## Completed Round: 2026-04-26-R130

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Removed runtime-only ordinary map-transfer success chat | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal packet surface: ordinary map transfers now emit `MapInformation` and `UserLocation` without generic `"Transferred to ..."` chat; focused `transfer_map` (2/2); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (633/633). |

## Completed Round: 2026-04-26-R129

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Localized Stage 5 socket/seal invalid-source rejection chats through Crystal `server.InvalidCombination` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: generated localization bundle contains `server.InvalidCombination`; focused `stage5_item_` (13/13); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (633/633). |

## Completed Round: 2026-04-26-R128

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Localized Stage 5 gold-shop purchase chat through Crystal `server.BoughtItemForGold` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: generated localization bundle contains `server.BoughtItemForGold`; focused `stage5_trade_shop_and_auction_are_transactional` (1/1); broader `stage5_` (22/22); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (633/633). |

## Completed Round: 2026-04-25-R127

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Removed runtime-only harvest success chat from transferred harvest-drop success | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal surface: successful harvest transfer now emits `GainedItem` plus `ObjectHarvested` without generic `"Harvested ..."` chat; focused/broader `harvest` (8/8); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (633/633). |

## Completed Round: 2026-04-25-R126

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Localized expanded-storage expiry notice through Crystal `server.ExpandedStorageExpired` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: generated localization bundle contains `server.ExpandedStorageExpired`; focused `expired_expanded_storage_tick_emits_resize_notice_once_and_persists_flag` (1/1); broader `storage` (43/43); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (633/633). |

## Completed Round: 2026-04-25-R125

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Localized Stage 5 item socket/seal success chats through Crystal `server.ItemSocketsIncreased` and `server.ItemSealedFor` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: generated localization bundle contains both keys; focused `stage5_item_` (13/13); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (633/633). |

## Completed Round: 2026-04-25-R124

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Localized Stage 5 item-seal reseal-delay rejection through Crystal `server.ItemCannotBeResealedFor` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: generated localization bundle contains `server.ItemCannotBeResealedFor`; focused `stage5_item_seal_rejects_before_next_seal_date_after_expiry` (1/1); broader `stage5_item_` (13/13); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (633/633). |

## Completed Round: 2026-04-25-R123

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Localized Stage 5 credit-shop purchase chat through Crystal `server.BoughtItemForCredit` while preserving mailbox delivery | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: generated localization bundle contains `server.BoughtItemForCredit`; focused `stage5_credit_shop_mails_purchase_and_claim_transfers_attachment` (1/1); broader `stage5_` (22/22); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (633/633). |

## Completed Round: 2026-04-25-R122

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Localized Stage 5 successful trade completion through Crystal `server.TradeSuccessful` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: generated localization bundle contains `server.TradeSuccessful`; focused `stage5_trade_shop_and_auction_are_transactional` (1/1); broader `stage5_` (22/22); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (633/633). |

## Completed Round: 2026-04-25-R121

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Localized Stage 5 trade/shop/auction low-gold rejection messages through Crystal `server.LowGold` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: generated localization bundle contains `server.LowGold`; focused `stage5_trade_shop_and_auction_cancel_error_paths_preserve_gold` (1/1); broader `stage5_` (22/22); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (633/633). |

## Completed Round: 2026-04-25-R120

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Localized direct ground-drop pickup full-bag rejection through Crystal `server.YouCannotCarryAnymore` while preserving current-cell skip semantics | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: generated localization bundle contains `server.YouCannotCarryAnymore`; focused `pickup` (14/14); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (633/633). |

## Completed Round: 2026-04-25-R119

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Localized Stage 5 mail, shop, auction, and craft full-bag rejection messages through Crystal `server.YouCannotCarryAnymore` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: generated localization bundle contains `server.YouCannotCarryAnymore`; focused `stage5_shop_and_auction_full_bag_preserve_gold_and_items` (1/1); broader `stage5_` (22/22); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (633/633). |

## Completed Round: 2026-04-25-R118

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Localized Stage 5 item socket max-capacity and already-sealed rejection messages through Crystal `server.ItemMaxSockets` and `server.ItemAlreadySealed` keys | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: generated localization bundle contains both server text keys; focused `stage5_item_` (13/13); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (633/633). |

## Completed Round: 2026-04-25-R117

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Localized harvest no-drop and full-bag messages through Crystal `server.NothingWasFound` and `server.YouCannotCarryAnymore` while preserving pending-drop retry and `ObjectHarvested` timing | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: generated localization bundle contains both server text keys; focused `harvest` (8/8); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (633/633). |

## Completed Round: 2026-04-25-R116

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Localized owner-blocked pickup rejection through Crystal `server.CannotPickupNotOwner` while preserving owner window, group-owner bypass, and scan-skip behavior | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: `PlayerObject.PickUp` emits `ServerTextKeys.CannotPickupNotOwner` only when no later pickable current-cell candidate exists; focused `pickup` (14/14); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (633/633). |

## Completed Round: 2026-04-25-R115

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Removed runtime-only normal pickup success chat so item and gold pickup success follows Crystal packet/chat surface while preserving `ShowGroupPickup` group notices | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: `PlayerObject.PickUp` gains items/gold and returns without normal success chat; focused `pickup` (14/14); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (633/633). |

## Completed Round: 2026-04-25-R114

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Added Crystal `NoDrug` map-rule rejection for static starter and dynamic manifest-backed potion `UseItem` so blocked maps emit `server.YouCannotUsePotionsHere`, fail ack, preserve items, and avoid HP/MP queueing | Coordinator | `apps/simulation/src/config.rs`, `apps/simulation/src/runtime.rs`, docs | Crystal evidence: `HumanObject.CanUseItem` rejects `ItemType.Potion` on `CurrentMap.Info.NoDrug` with `ServerTextKeys.YouCannotUsePotionsHere`; focused `no_drug` (2/2); adjacent `use_item_packet_` (42/42); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (633/633). |

## Completed Round: 2026-04-25-R113

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Aligned static starter HP/MP potion use with Crystal normal-potion timed recovery so successful use consumes and acks immediately but restores HP/MP on follow-up ticks via `ObjectHealth` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: `PlayerObject.UseItem` `ItemType.Potion` shape `0` queues `PotHealthAmount` / `PotManaAmount`, while shape `1` is the immediate `SunPotion` branch; focused `crystal_use_item_packet_consumes_` (2/2); adjacent `use_item_packet_` (40/40); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (631/631). |

## Completed Round: 2026-04-25-R112

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Removed runtime-only static `repair-powder` success/failure chat so starter equipment repair use preserves repair mutation and `ItemRepaired` packets without extra generic chat | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: no Crystal `UseItem` branch emits the starter `sim.noEquipmentNeedsRepair` / `sim.repairedEquippedItems` messages; focused `repair_powder` (2/2); adjacent `use_item_packet_` (40/40); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (631/631). |

## Completed Round: 2026-04-25-R111

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Removed runtime-only static `town-teleport` success chat so successful teleport use emits movement/location packets without generic success chat | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: existing dynamic Crystal town-teleport path and source-audited `NoTownTeleport` gating have no success-side chat; focused `town_teleport` (3/3); adjacent `use_item_packet_` (40/40); `cargo +1.89.0 fmt --check`; `git -C mir2-web3 diff --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (631/631). |

## Completed Round: 2026-04-25-R110

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Removed hardcoded static `benediction-oil` no-weapon failure chat so invalid weapon-luck attempts fail without runtime-only chat or item consumption | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source cross-check: `PlayerObject.UseItem` case 3 enqueues failed `UseItem` when `TryLuckWeapon()` returns false; `HumanObject.TryLuckWeapon` only chats after a valid outcome; focused `benediction_oil` (4/4); adjacent `use_item_packet_` (40/40); `cargo +1.89.0 fmt --check`; `git -C mir2-web3 diff --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (631/631). |

## Completed Round: 2026-04-25-R109

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Removed runtime-only `SplitItem` success chat so inventory/storage splits emit Crystal-shaped `SplitItem1` plus `SplitItem` packets without extra chat | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source cross-check: `PlayerObject.SplitItem` success enqueues `S.SplitItem1` and `S.SplitItem` only; focused `split_item_packet` (7/7); focused `storage_split_item_stack_creates_new_storage_slot`; adjacent `storage` (43/43); `cargo +1.89.0 fmt --check`; `git -C mir2-web3 diff --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (630/630). |

## Completed Round: 2026-04-25-R108

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Aligned static `repair-oil` / `war-god-oil` with Crystal's localized weapon-repair hint surface and removed the runtime-only failure chat/no-repair message | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source cross-check: `PlayerObject.UseItem` scroll shape `4`/`5` silently failed-acks when no weapon repair is possible and emits `WeaponPartiallyRepaired` / `WeaponCompletelyRepaired` hint plus `ItemRepaired` on success; focused `cargo +1.89.0 test --locked -p mir2-simulation repair_oil -- --test-threads=1 --nocapture` (3/3); focused `repair_and_war_god_oil_emit_item_repaired_for_weapon`; adjacent `use_item_packet_` (40/40); `cargo +1.89.0 fmt --check`; `git -C mir2-web3 diff --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (630/630). |

## Completed Round: 2026-04-25-R107

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Removed runtime-only `custom.itemDropped` from successful `DropItem` so normal and split-stack inventory drops return success ack plus ground-object visibility without generic success chat | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source cross-check: `PlayerObject.DropItem` only chats for `NoThrowItem` and success ends with `p.Success = true; Enqueue(p);` without success chat; adjacent `cargo +1.89.0 test --locked -p mir2-simulation drop_item_packet -- --test-threads=1 --nocapture` (10/10); `cargo +1.89.0 fmt --check`; `git -C mir2-web3 diff --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (629/629). |

## Completed Round: 2026-04-25-R106

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Removed runtime-only `sim.usedItem` from the static HP/MP consumable `UseItem` success path so inventory/belt starter potions heal, consume, and ack success without chat | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source cross-check: `PlayerObject.UseItem` potion shape `0`/`1` queues restore or changes HP/MP without normal success chat; focused `cargo +1.89.0 test --locked -p mir2-simulation crystal_use_item_packet_consumes_inventory_slot -- --test-threads=1 --nocapture`; focused `cargo +1.89.0 test --locked -p mir2-simulation crystal_use_item_packet_consumes_belt_slot -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_ -- --test-threads=1 --nocapture` (40/40); `cargo +1.89.0 fmt --check`; `git -C mir2-web3 diff --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (629/629). |

## Completed Round: 2026-04-25-R105

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Removed runtime-only `sim.itemNotFoundInBag` from missing-source `DropItem` so absent inventory ids now return only the failed `DropItem` ack | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source cross-check: `PlayerObject.DropItem` enqueues the failed `S.DropItem` for missing item/count failures without chat; focused `cargo +1.89.0 test --locked -p mir2-simulation drop_item_packet_missing_inventory_item_rejects_without_runtime_chat -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation drop_item_packet -- --test-threads=1 --nocapture` (10/10); `cargo +1.89.0 fmt --check`; `git -C mir2-web3 diff --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (629/629). |

## Completed Round: 2026-04-25-R104

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Changed unmodeled `UseItem(grid=HeroInventory)` from an empty response to a Crystal-shaped failed `UseItem` ack while preserving the existing no-fallback/no-mutation behavior | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source cross-check: `MirConnection.UseItem` routes `HeroInventory` to `HeroObject.UseItem`, which starts with `S.UseItem { Grid = HeroInventory, Success = false }`; focused `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_hero_inventory_grid_does_not_mutate_matching_player_item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_ -- --test-threads=1 --nocapture` (40/40); `cargo +1.89.0 fmt --check`; `git -C mir2-web3 diff --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (628/628). |

## Completed Round: 2026-04-25-R103

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Removed runtime-only `sim.itemNotFoundInBag` from missing-item and invalid-source `UseItem` failures so missing inventory ids now failed-ack without chat or mutation | Coordinator | `apps/simulation/src/runtime.rs`, docs | Focused `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_missing_inventory_item_rejects_without_runtime_chat -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_ -- --test-threads=1 --nocapture` (40/40); `cargo +1.89.0 fmt --check`; `git -C mir2-web3 diff --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (628/628). |

## Completed Round: 2026-04-25-R102

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Removed runtime-only `sim.itemNoActiveUse` from the final unusable inventory `UseItem` fallback so unknown/unusable items now failed-ack without chat or mutation | Coordinator | `apps/simulation/src/runtime.rs`, docs | Focused `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_unusable_inventory_item_rejects_without_runtime_chat -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_ -- --test-threads=1 --nocapture` (39/39); `cargo +1.89.0 fmt --check`; `git -C mir2-web3 diff --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (627/627). |

## Completed Round: 2026-04-25-R101

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Removed the literal runtime-only non-inventory equipment `UseItem` failure chat so belt-sourced equipment attempts now failed-ack without chat or mutation | Coordinator | `apps/simulation/src/runtime.rs`, docs | Focused `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_belt_equipment_rejects_without_runtime_chat -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_ -- --test-threads=1 --nocapture` (38/38); `cargo +1.89.0 fmt --check`; `git -C mir2-web3 diff --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (626/626). |

## Completed Round: 2026-04-25-R100

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Removed runtime-only `sim.equippedItem*` chat from the successful `UseItem` equipment path so the modeled success surface stays ack/refresh/equipment-state only, matching Crystal's explicit equip packet surface | Coordinator | `apps/simulation/src/runtime.rs`, docs | Focused `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_equipping_need_identify_item_emits_refresh_item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_ -- --test-threads=1 --nocapture` (37/37); adjacent `cargo +1.89.0 test --locked -p mir2-simulation equip_item_packet -- --test-threads=1 --nocapture` (13/13); `cargo +1.89.0 fmt --check`; `git -C mir2-web3 diff --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (625/625). |

## Completed Round: 2026-04-25-R99

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Locked the positive explicit `EquipItem` path for dynamic manifest-backed equipment when Crystal requirements are met, using `SpiritRing` at required level 15 into the right ring slot | Coordinator | `apps/simulation/src/runtime.rs`, docs | Focused `cargo +1.89.0 test --locked -p mir2-simulation equip_item_packet_manifest_equipment_allows_when_requirements_are_met -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation equip_item_packet -- --test-threads=1 --nocapture` (13/13); `cargo +1.89.0 fmt --check`; `git -C mir2-web3 diff --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (625/625). |

## Completed Round: 2026-04-25-R98

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Locked dynamic manifest-backed `CreditToken3` `UseItem` coverage for credit gain, localized `server.CreditsAddedToAccount` hint, success ack, and item consumption | Coordinator | `apps/simulation/src/runtime.rs`, docs | Focused `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_dynamic_crystal_credit_token_emits_localized_hint_chat -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_ -- --test-threads=1 --nocapture` (37/37); `cargo +1.89.0 fmt --check`; `git -C mir2-web3 diff --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (624/624). |

## Completed Round: 2026-04-25-R97

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Locked `EquipItem(grid=Storage)` coverage for dynamic manifest-backed equipment requirement rejection so storage-sourced items fail ack-only, preserve storage state, and do not equip when Crystal requirements are unmet | Coordinator | `apps/simulation/src/runtime.rs`, docs | Focused `cargo +1.89.0 test --locked -p mir2-simulation equip_item_packet_storage_manifest_equipment_rejects_unmet_requirements_silently -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation equip_item_packet -- --test-threads=1 --nocapture` (12/12); `cargo +1.89.0 fmt --check`; `git -C mir2-web3 diff --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (623/623). |

## Completed Round: 2026-04-25-R96

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Added Crystal `CanEquipItem` requirement gating for explicit `EquipItem` on dynamic manifest-backed equipment: gender/class/required-type failures now silently fail before mutation like Crystal, while legacy fixture aliases keep existing test behavior | Coordinator | `apps/simulation/src/runtime.rs`, docs | Focused `cargo +1.89.0 test --locked -p mir2-simulation equip_item_packet_manifest_equipment_rejects_unmet_requirements_silently -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation equip_item_packet -- --test-threads=1 --nocapture` (11/11); adjacent `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_crystal_equipment_ -- --test-threads=1 --nocapture` (2/2); `cargo +1.89.0 fmt --check`; `git -C mir2-web3 diff --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (622/622). |

## Completed Round: 2026-04-25-R95

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Added explicit regression coverage for Crystal `CanEquip` compatibility where manifest-backed `ItemType.Amulet` can target the right bracelet slot | Coordinator | `apps/simulation/src/runtime.rs`, docs | Focused `cargo +1.89.0 test --locked -p mir2-simulation equip_item_packet_manifest_amulet_can_target_right_bracelet_slot -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation equip_item_packet -- --test-threads=1 --nocapture` (10/10). |

## Completed Round: 2026-04-25-R94

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Wider validation pass after R89-R93 item/equipment parity changes | Coordinator | docs | `cargo +1.89.0 test --locked -p mir2-simulation item -- --test-threads=1 --nocapture` (218/218); `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture` (42/42); `cargo +1.89.0 fmt --check`; `git -C mir2-web3 diff --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (620/620). |

## Completed Round: 2026-04-25-R93

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Fixed explicit `EquipItem` target-slot compatibility for manifest-backed ring/bracelet equipment: imported item type compatibility now allows rings in either ring slot and bracelets in either bracelet slot while preserving `UseItem` default slot behavior | Coordinator | `apps/simulation/src/runtime.rs`, docs | Focused `cargo +1.89.0 test --locked -p mir2-simulation equip_item_packet_manifest_ring_and_bracelet_can_target_right_slots -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation equip_item_packet -- --test-threads=1 --nocapture` (9/9). |

## Completed Round: 2026-04-25-R92

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Matched Crystal `ResurrectionScroll` revive vitals by restoring modeled MP to the current runtime cap when a dead player revives, alongside existing full HP revive and consume behavior | Coordinator | `apps/simulation/src/runtime.rs`, docs | Focused `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_dead_player_resurrection_scroll_revives_and_consumes_item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_ -- --test-threads=1 --nocapture` (36/36). |

## Completed Round: 2026-04-25-R91

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Added Crystal repair-bind rejection to manifest-backed `RepairOil` / `WarGodOil`: equipped weapon `DontRepair` blocks repair oils and `NoSRepair` also blocks full/special `WarGodOil`, preserving item and weapon durability on failure | Coordinator | `apps/simulation/src/runtime.rs`, docs | Focused `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_dynamic_crystal_repair_oils_respect_weapon_repair_binds -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_ -- --test-threads=1 --nocapture` (36/36). |

## Completed Round: 2026-04-25-R90

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Added Crystal `CanUseItem` map-rule rejection for manifest-backed scroll shape `0/2`: `NoEscape` blocks `DungeonEscape` / `TeleportHome` with `server.CanNotDungeon`, and `NoRandom` blocks `RandomTeleport` with `server.CanNotRandom`, preserving item and position on failure | Coordinator | `apps/simulation/src/config.rs`, `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `HumanObject.CanUseItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_dynamic_crystal_dungeon_escape_rejects_on_no_escape_map -- --test-threads=1 --nocapture`; focused `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_dynamic_crystal_random_teleport_rejects_on_no_random_map -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_ -- --test-threads=1 --nocapture` (35/35). |

## Completed Round: 2026-04-25-R89

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Mapped manifest-backed Crystal equipment item types to runtime `EquipmentSlot` for item gain, test helpers, and `UseItem` fallback, removing test-only manual slot setup for current manifest equipment use | Coordinator | `apps/simulation/src/runtime.rs`, docs | Focused `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_crystal_equipment_ -- --test-threads=1 --nocapture` (2/2); adjacent `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_ -- --test-threads=1 --nocapture` (33/33). |

## Completed Round: 2026-04-25-R88

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Implemented manifest-backed `UseItem` pending timed-recovery behavior for normal potion `shape 0`, using modeled `pending_pot_health_amount` / `pending_pot_mana_amount` fields and world-tick drain emissions without immediate HP/MP mutation or hint chat | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.UseItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_dynamic_crystal_normal_potion_queues_timed_restore -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_ -- --test-threads=1 --nocapture` (33/33). |

## Completed Round: 2026-04-25-R87

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Expand manifest-backed `UseItem` `ItemType.Food` mount-feed branch for `RawMeat`/`LeanMeat`, including equipped-mount requirement, full-dura guard, success consume/emit behavior, and Crystal-style `ItemRepaired` / `server.MountFed` hints | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.UseItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_dynamic_crystal_food_requires_equipped_mount -- --test-threads=1 --nocapture`; focused `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_dynamic_crystal_food_feeds_equipped_mount -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_ -- --test-threads=1 --nocapture` (32/32) |

## Completed Round: 2026-04-25-R86

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Expand manifest-backed current `UseItem` for `DungeonEscape`/`TeleportHome` and `RandomTeleport` scroll-shape `0/2` with same-map occupiable destination search and bounded success/failure behavior | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.UseItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_dynamic_crystal_dungeon_escape_teleports_same_map -- --test-threads=1 --nocapture` (9/9); focused `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_dynamic_crystal_random_teleport_teleports_same_map -- --test-threads=1 --nocapture` (30/30); adjacent `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_ -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation use_item_ -- --test-threads=1 --nocapture` |

## Completed Round: 2026-04-25-R85

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Expanded `UseItem` `CanUseItem` parity beyond the R82 level-only requirement by adding modeled stat gates for `MaxAC` / `MaxMAC` / `MaxDC` / `MaxMC` / `MaxSC`, `MinAC` / `MinMAC` / `MinDC` / `MinMC` / `MinSC`, and `MaxLevel` from existing modeled equipment/buff totals | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `Crystal/Server/MirObjects/HumanObject.cs::CanUseItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_crystal_equipment_rejects_low_max_dc_requirement -- --test-threads=1 --nocapture`; focused `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_crystal_equipment_allows_modeled_max_mc_requirement -- --test-threads=1 --nocapture`; focused `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_crystal_ -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_ -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation use_item_ -- --test-threads=1 --nocapture` |

## Completed Round: 2026-04-25-R84

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Corrected manifest-backed `UseItem` shape-26/27 branch for `GtInvite` and `GTTeleport` so `CanUseItem` pass now consumes once with `UseItem` success ack only, no chat, and no `UserLocation`/teleport side effect while leaving `GTTeleport` guild-territory behavior to NPC script paths | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.UseItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_dynamic_crystal_gt_invite_consumes_without_active_effect -- --test-threads=1 --nocapture`; focused `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_dynamic_crystal_gt_teleport_consumes_without_teleporting -- --test-threads=1 --nocapture`; focused `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_dynamic_crystal -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_ -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation use_item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation equip_item_packet -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 fmt --check`; `git -C mir2-web3 diff --check` |

## Completed Round: 2026-04-25-R83

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Remaining manifest-backed item-use small surface completed for `AncientBanga[Green]` / `AncientBanga[Purple]`, map/server shout flags, Crystal hint chat, and credit-token usage hint localization | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.UseItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_ -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation use_item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation equip_item_packet -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 fmt --check`; `git -C mir2-web3 diff --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-25-R82

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal `CanUseItem` parity for current subset (`Gender`, `Class`, `RequiredType==Level`, repeated skill-book learn block, and successful skill-book learn consume) | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.CanUseItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_ -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation use_item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation equip_item_packet -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 fmt --check`; `git -C mir2-web3 diff --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-25-R81

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Dynamic manifest-backed current-data `UseItem` now routes Crystal `SunPotion`, duration buffs, `TownTeleport`, `BenedictionOil`, `RepairOil`, and `WarGodOil` through template stats and scroll shapes, including Crystal-style same-key buff duration stacking and the current `WarGodOil` shape-0 name fallback | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.UseItem` plus `MapObject.AddBuff`; focused `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_dynamic_crystal -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation use_item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation equip_item_packet -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 fmt --check`; `git -C mir2-web3 diff --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-25-R80

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Current equipment/item metadata now preserves Crystal `NeedIdentify` and `SoulBoundId` through runtime/item payload round-trips, auto-identifies items on equip/use-equip, and rejects equipping items soul-bound to another character | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.EquipItem` / `PlayerObject.UseItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation equip_item_packet -- --test-threads=1 --nocapture`; focused `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_equipping_need_identify_item_emits_refresh_item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 fmt --check`; `git -C mir2-web3 diff --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-25-R79

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Current `MysteryWater` plus cursed current-equipment semantics now match Crystal's bounded runtime surface: first use unlocks and consumes, repeat use hint-chats without consuming, cursed current `RemoveItem` and replacement `EquipItem` require the unlock, successful cursed removal/replacement clears it again, and storage-grid replacement rejects replaced equipment that cannot be stored | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.UseItem`, `PlayerObject.EquipItem`, and `PlayerObject.RemoveItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet -- --test-threads=1 --nocapture`; focused `cargo +1.89.0 test --locked -p mir2-simulation remove_item -- --test-threads=1 --nocapture`; focused `cargo +1.89.0 test --locked -p mir2-simulation equip_item_packet -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 fmt --check`; `git -C mir2-web3 diff --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-25-R78

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Current `RemoveSlotItem` now follows Crystal's bounded source-grid envelope for the modeled runtime: invalid `grid=Equipment` requests and unmodeled `Mount` / `Fishing` / `Socket` slot-item requests ack-fail without falling through into whole-equipment removal, including socket requests that only match the parent equipment id | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.RemoveSlotItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation remove_slot_item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 fmt --check`; `git -C mir2-web3 diff --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-25-R77

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Current `EquipItem(grid=Storage)` now resolves the exact storage item through the active `@Storage` service, and current `RemoveItem(grid=Inventory|Storage)` now follows Crystal's exact destination-slot semantics with ack-only packet shape instead of accepting `grid=Equipment` or falling back into another bag slot | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.EquipItem` / `PlayerObject.RemoveItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation equip_item_packet_ -- --test-threads=1 --nocapture`; focused `cargo +1.89.0 test --locked -p mir2-simulation remove_item_packet_ -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 fmt --check`; `git -C mir2-web3 diff --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-25-R76

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Expired expanded storage now downgrades to inactive on current `StartGame`, then emits Crystal-style expiry chat plus `ResizeStorage` on the first world tick and persists the account flag back to `false` while preserving the 160-slot backing array | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject` expanded-storage expiry / `BuildUserInformation`; focused `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 fmt --check`; `git -C mir2-web3 diff --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-25-R75

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Current `@Storage` open now sends Crystal `UserStorage` with the full backing storage length even when expanded storage is no longer active, while higher-slot storage actions remain gated by current accessible capacity | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.SendStorage` / `AccountInfo.IsValidStorageIndex`; focused `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 fmt --check`; `git -C mir2-web3 diff --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-25-R74

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Repeated unchanged current `@Storage` opens now suppress duplicate `UserStorage` after the first send, matching Crystal `Connection.StorageSent` resend behavior while preserving the locked reopen/unlock resend path | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `NPCScript.StorageKey` / `PlayerObject.SendStorage` / `MirConnection.UnlockStorage`; focused `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 fmt --check`; `git -C mir2-web3 diff --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-25-R73

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Successful current `@Storage` open now emits Crystal `UserStorage` before `NPCStorage` when storage is available, and successful `UnlockStorage` now emits `StorageUnlockResult` followed by `UserStorage`, through protocol/gateway/runtime with focused regressions | Coordinator | `packages/protocol/src/ids.rs`, `packages/protocol/src/packets.rs`, `packages/protocol/src/trace.rs`, `packages/protocol/tests/codec.rs`, `apps/gateway/src/web.rs`, `apps/web/app/page.tsx`, `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `NPCScript.StorageKey` / `PlayerObject.SendStorage` / `MirConnection.UnlockStorage`; focused `cargo +1.89.0 test --locked -p mir2-protocol --test codec`; focused `cargo +1.89.0 test --locked -p mir2-gateway`; focused `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture`; focused `cargo +1.89.0 test --locked -p mir2-simulation crystal_npc_service_links_emit_packets_and_close_dialog -- --test-threads=1 --nocapture`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-25-R72

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Reopening Crystal `@Storage` now resets the session unlock state before deciding whether storage contents can be sent, matching `ResetStorageUnlock()` and blocking stale unlocked sessions | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `NPCScript.StorageKey`; focused `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`; `git -C mir2-web3 diff --check` |

## Completed Round: 2026-04-25-R71

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Current storage password set/unlock/remove now enforce Crystal's `^[A-Za-z0-9]{5,15}$` password format semantics instead of accepting runtime-only values | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for storage password validation; focused `cargo +1.89.0 test --locked -p mir2-simulation storage_password -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-25-R70

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Current storage password actions now require the active in-range Crystal storage service context, and successful password removal clears `LastSetTime` back to `0` like Crystal | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for current storage password handlers; focused `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-25-R69

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Current inventory-grid `CombineItem` current-data coverage now closes the remaining present-data shape-3/4 families and the shape-0 ack-only source surface for the current manifest slice | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.CombineItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation combine_item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-25-R68

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Current inventory-grid `CombineItem` now routes current-data `DurabilityGem` / `DurabilityOrb` through Crystal's `MaxDura` branch instead of misusing stat `48` as the applied upgrade stat, and focused regressions now lock the current-data durability, attack-speed, magic-resist, and durability-cap surfaces | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.CombineItem` / `GetGemType` / `GetCurrentStatCount`; focused `cargo +1.89.0 test --locked -p mir2-simulation combine_item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-25-R67

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Current `BuyItem`, `SellItem`, and `RepairItem`/`SRepairItem` now require the recorded Crystal NPC object to still exist and remain within `CRYSTAL_DATA_RANGE`, so stale/out-of-range service context no longer mutates the implemented current NPC buy/sell/repair item surfaces | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for current NPC item-service handlers; focused `cargo +1.89.0 test --locked -p mir2-simulation buy_item -- --test-threads=1 --nocapture`; focused `cargo +1.89.0 test --locked -p mir2-simulation sell_item -- --test-threads=1 --nocapture`; focused `cargo +1.89.0 test --locked -p mir2-simulation repair_item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-24-R66

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Current storage-family item actions now require the recorded Crystal storage NPC object to still exist and remain within `CRYSTAL_DATA_RANGE`, so stale/out-of-range storage service context now ack-fails across `StoreItem`, `TakeBackItem`, `MoveItem(grid=Storage)`, `SplitItem(grid=Storage)`, and any `MergeItem` touching `Storage` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.StoreItem` / `TakeBackItem` / `MoveItem` / `SplitItem` / `MergeItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation crystal_npc_storage_service_context_rejects_storage_actions_when_player_leaves_data_range -- --test-threads=1 --nocapture`; focused `cargo +1.89.0 test --locked -p mir2-simulation storage_service_context_requires_live_npc_object -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-24-R65

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Current `SplitItem` now matches Crystal's supported-grid and failed-ack surface: only `Inventory` / `Storage` are live, `Storage` requires active Crystal storage service, and unsupported/invalid/full/locked failures stay ack-only | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.SplitItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation split_item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-24-R64

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Current `SplitItem(grid=Inventory)` now follows Crystal single-array placement across local `Bag1` / `Bag2`, including belt-first placement for belt-eligible items instead of source-container page scoping | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.SplitItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation split_item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-24-R63

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Slot-based current `MoveItem` / `StoreItem` / `TakeBackItem` inventory paths now resolve Crystal single-array indices across local `Bag1` / `Bag2`, including `Bag2` swaps and storage transfers on slots `40+` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.MoveItem` / `PlayerObject.StoreItem` / `PlayerObject.TakeBackItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation crystal_inventory_index_for_bag2 -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation move_item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-24-R62

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Remaining unsupported `MergeItem` `Storage <-> Belt` cross-grid requests now follow Crystal's ack-only surface without runtime-only chat | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.MergeItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation merge_item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-24-R61

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Current `MergeItem` now rejects `QuestInventory` requests ack-only without extra chat or quest-item mutation | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.MergeItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation merge_item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-24-R60

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Current `MoveItem` now rejects `Belt` / `QuestInventory` requests ack-only, enforces current inventory slot bounds, and keeps bag moves from mutating quest items | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.MoveItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation move_item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-24-R59

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Current missing-source `MoveItem` Inventory/Storage failures now use Crystal's `ItemMoveErrorReport` chat surface before the failed ack instead of `sim.itemNotFoundInBag` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.MoveItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation move_item -- --test-threads=1 --nocapture` plus new missing-source move regressions; adjacent `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-24-R58

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Current successful `MoveItem` current `Inventory` and `Storage` paths now follow Crystal's ack-only surface, removing the runtime-only `Item slot updated.` chat | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.MoveItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation move_item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-24-R57

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Current `MoveItem(grid=Storage)` now requires active Crystal `@Storage` / `NPCStorage` service context, with ack-only inactive-service failures | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.MoveItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation move_item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-24-R56

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Current `MoveItem` storage-lock and invalid-slot failures now follow Crystal's ack-only surface without extra chat | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.MoveItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation move_item -- --test-threads=1 --nocapture` plus new storage-lock/invalid-slot move regressions; adjacent `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-24-R55

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Current `MoveItem` unsupported-grid parity now also covers `HeroEquipment`, `Equipment`, and `Fishing` ack-only failures without extra chat or player/equipment mutation | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.MoveItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation move_item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-24-R54

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Current `MergeItem` now supports the next bounded modeled cross-grid surface via `Inventory <-> Belt` stack merges for Crystal belt-eligible items, with ack-only non-beltable failures | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.MergeItem` plus local belt-model audit; focused `cargo +1.89.0 test --locked -p mir2-simulation merge_item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-24-R53

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Current `MergeItem` now supports Crystal-style `Inventory <-> Storage` stack merges through the active storage-service gate, with ack-only inactive/locked failures | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.MergeItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation merge_item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-24-R52

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Current `MergeItem` same-grid failure/success message shape now follows Crystal's ack-only surface for current Inventory/Storage paths | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.MergeItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation merge_item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-24-R51

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Current `MergeItem` unsupported-grid parity now also covers `Trade` and `Refine` ack-only failures without extra chat or player-bag mutation | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.MergeItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation merge_item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-24-R50

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Current `MergeItem` unsupported-grid parity now also covers `HeroInventory`, `HeroEquipment`, `Equipment`, and `Fishing` ack-only failures without extra chat or player-bag mutation | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.MergeItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation merge_item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-24-R49

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Current `MoveItem` unsupported-grid parity now covers `HeroInventory`, `Trade`, and `Refine` ack-only failures without extra chat or player-bag mutation | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.MoveItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation move_item -- --test-threads=1 --nocapture`; `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-24-R48

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal `MoveItem(grid=HeroInventory)` failed-ack without extra chat or player-bag mutation while hero inventory is unmodeled | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.MoveItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation move_item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation hero_inventory -- --test-threads=1 --nocapture`; `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-24-R47

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal `MergeItem` hero-grid requests failed-ack without extra chat or player-bag mutation while hero inventory/equipment are unmodeled | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.MergeItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation merge_item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation hero_inventory -- --test-threads=1 --nocapture`; `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-24-R46

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal `EquipItem(grid=HeroInventory)`, `RemoveItem(grid=HeroInventory)`, and `RemoveSlotItem(grid=HeroEquipment|HeroInventory)` failed-ack without mutating matching player inventory/equipment while hero grids are unmodeled | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.EquipItem`, `PlayerObject.RemoveItem`, and `PlayerObject.RemoveSlotItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation equip_item_packet_hero_inventory_grid_does_not_mutate_matching_player_item -- --test-threads=1 --nocapture`; focused `cargo +1.89.0 test --locked -p mir2-simulation remove_item_packet_hero_inventory_grid_does_not_mutate_matching_player_equipment -- --test-threads=1 --nocapture`; focused `cargo +1.89.0 test --locked -p mir2-simulation remove_slot_item_packet_hero_equipment_grid_does_not_mutate_matching_player_equipment -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation hero_inventory -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation equip_item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation remove_item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation remove_slot_item -- --test-threads=1 --nocapture`; `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-28-R301

R301 refreshed the final automated Candidate acceptance pack after the R300 stable-diff packet acceptance decision. It intentionally does not mark whole-project 100% Accepted because human Crystal visual/feel acceptance remains open.

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Refresh full automated acceptance pack and archive R301 evidence | Coordinator | generated player-QA/map/minimap/load evidence, parity docs | Evidence summary: `docs/generated/player-qa/r301-summary.json`. Verification passed without Docker: `cargo +1.89.0 test --locked -p mir2-gateway --bin packet_trace -- --test-threads=1` (15/15), `apps\web .\node_modules\.bin\tsc --noEmit`, `apps\web npm.cmd run build`, `npm.cmd run smoke:crystal-map-api` (18/18, 0 failures, archived at `docs/generated/map/r301-crystal-map-api.json`), `npm.cmd run smoke:crystal-minimap-assets` (0 failures, a historical preview-index warning later closed by the 2026-05-16 map audit, archived at `docs/generated/assets/r301-minimap-assets.json`), `npm.cmd run smoke:stage5-ui` (88 screenshots, 0 critical console errors, archived manifest under `docs/generated/player-qa/r301/`), `npm.cmd run load:gateway-ws` (64/64 ready, 0 errors, keepalive p95 637 ms, archived at `docs/generated/load/r301-ws.json`), `mir2-game-data` 27/27, `mir2-gateway` 55/55 plus packet-trace bin 15/15, `mir2-admin-api` 22/22, and `mir2-simulation` 674/674. Temporary gateway/web services were stopped and ports 7000/7110/3002 verified closed. |

## Completed Round: 2026-04-28-R298

R298 refreshed the live Crystal stable packet matrix on Windows after the R297 frontend/player evidence pass. It intentionally does not mark strict exact packet parity accepted because exact diffs remain dirty.

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Refresh live Crystal stable matrix and keep strict exact acceptance gate open | Coordinator | `apps/gateway/src/bin/packet_trace.rs`, parity docs, trace artifacts | `cargo +1.89.0 run --locked -p mir2-gateway --bin packet_trace -- --matrix` with `MIR2_CRYSTAL_TCP_ADDR=127.0.0.1:7000`, local gateway `127.0.0.1:7310`, and `CRYSTAL_CLIENT_ROOT=E:\mir2\Crystal\Build\Client\Debug` wrote `docs/generated/packet-traces/r298-live-matrix/latest-matrix.json` (`stableDiffCleanCount=9`, `acceptedStableLiveComparisonCount=9`, `diffDirtyCount=9`, `acceptedLiveComparisonCount=0`). The stable comparator now treats Crystal `TimeOfDay` payloads as volatile. Verification passed: `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (674/674), `cargo +1.89.0 test --locked -p mir2-gateway -- --test-threads=1` (55/55 plus packet_trace 14/14), `cargo +1.89.0 test --locked -p mir2-admin-api -- --test-threads=1` (22/22), `cargo +1.89.0 fmt --check`, `git diff --check`, and `apps\web .\node_modules\.bin\tsc --noEmit`. |

## Completed Round: 2026-04-28-R297

R297 refreshed Windows frontend/player QA automation and fixed real issues encountered by that evidence path. It intentionally does not mark Accepted 100% because human visual/feel acceptance and strict exact live packet diff acceptance remain open.

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Refresh Windows player QA with full client resources and fix load/UI evidence blockers | Coordinator | `apps/simulation/src/config.rs`, `apps/gateway/src/web.rs`, `apps/web/app/page.tsx`, `apps/web/scripts/load-gateway-ws.mjs`, `apps/web/scripts/smoke-stage5-ui.mjs`, `apps/web/scripts/crystal-ui-export-manifest.json`, `apps/web/public/original-ui/*`, parity docs, generated QA artifacts | Account-store atomic JSON writes now serialize/retry under concurrent Windows load; WS load creates a character for Crystal-aligned empty accounts; gateway `MapInformation` sends minimap/big-map indices; Stage 5 smoke reports network URLs for critical errors; missing original scene `NPC/*` and `Monster/*` libs were exported. Verification passed: web `npm.cmd run build`, map API smoke 18/18, minimap smoke 0 failures with a historical preview-index warning later closed by the 2026-05-16 map audit, `npm.cmd run load:gateway-ws` 64/64 ready with 0 errors, `npm.cmd run smoke:stage5-ui` 88 screenshots with 0 critical console errors, `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (674/674), `cargo +1.89.0 test --locked -p mir2-gateway -- --test-threads=1` (55/55 plus packet_trace 14/14), `cargo +1.89.0 test --locked -p mir2-admin-api -- --test-threads=1` (22/22), `cargo +1.89.0 fmt --check`, `git diff --check`, and `apps\web .\node_modules\.bin\tsc --noEmit`. |

## Completed Round: 2026-04-28-R292

R292 completed the first clean live Crystal stable packet-matrix run on Windows. It intentionally does not mark strict exact packet parity accepted because exact diffs remain dirty.

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Capture stable live Crystal matrix and align matrix harness/runtime packet surfaces without inflating parity percentages | Coordinator | `apps/simulation/src/runtime.rs`, `apps/gateway/src/bin/packet_trace.rs`, `apps/gateway/src/session.rs`, parity docs, trace artifacts | `cargo +1.89.0 run --locked -p mir2-gateway --bin packet_trace -- --matrix` with `MIR2_CRYSTAL_TCP_ADDR=127.0.0.1:7000` wrote `docs/generated/packet-traces/r292-live-matrix/latest-matrix.json` (`stableDiffCleanCount=9`, `diffDirtyCount=9`); `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (674/674); `cargo +1.89.0 test --locked -p mir2-gateway -- --test-threads=1` (55/55 plus packet_trace 14/14); `cargo +1.89.0 test --locked -p mir2-admin-api -- --test-threads=1` (22/22); `cargo +1.89.0 fmt --check`; `git diff --check`; `apps\web .\node_modules\.bin\tsc --noEmit`. |

## Completed Round: 2026-04-28-R248

R248 completed the previously blocked R39 data-import follow-up on Windows. The runtime/game-data/tooling scaffolding was already in place; this round supplied the missing real Crystal DB and route inputs, regenerated the manifests, and reverified the backend packages.

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Promote Crystal map `NoThrowItem` / `NoDropPlayer` / `NoDropMonster` flags into generated respawn/map data and switch runtime off config-only overrides | Coordinator | generated Crystal manifests, docs | Crystal `MapInfo` save-layout audit was already in place; Windows regeneration used `E:\mir2\Crystal\Build\Server\Debug\Server.MirDB` and `E:\mir2\Crystal\Build\Server\Debug\Envir\Routes`. Verification passed: `node packages\tooling\scripts\generate-crystal-respawn-manifest.mjs`; `cargo +1.89.0 test --locked -p mir2-game-data -- --test-threads=1` (22/22); `cargo +1.89.0 test --locked -p mir2-simulation no_drop_monster_map_rule -- --test-threads=1 --nocapture` (2/2); full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (670/670); `cargo +1.89.0 test --locked -p mir2-gateway -- --test-threads=1` (55/55 plus packet-trace bin 7/7). |

## Completed Round: 2026-04-24-R45

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal `SplitItem(grid=HeroInventory)` no longer falls back into player inventory when hero inventory is unmodeled | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.SplitItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation split_item_packet_hero_inventory_grid_does_not_mutate_matching_player_stack -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation split_item -- --test-threads=1 --nocapture`; `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-24-R44

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal `UseItem(grid=HeroInventory)` no longer falls back into player inventory when hero inventory is unmodeled | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `MirConnection.UseItem`, `PlayerObject.HeroUseItem`, and `HeroObject.UseItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_hero_inventory_grid_does_not_mutate_matching_player_item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet -- --test-threads=1 --nocapture`; `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-24-R43

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal `ResurrectionScroll` map `NoReincarnation` rejection for dead current players | Coordinator | `apps/simulation/src/runtime.rs`, `apps/simulation/src/config.rs`, docs | Crystal source audit for `PlayerObject.UseItem` shape-6 and `HumanObject.CanUseItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_dead_player_resurrection_scroll_rejects_on_no_reincarnation_map -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation dead_player -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation town_teleport -- --test-threads=1 --nocapture`; `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-24-R42

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal `TownTeleport` map `NoTownTeleport` rejection for current `UseItem` | Coordinator | `apps/simulation/src/runtime.rs`, `apps/simulation/src/config.rs`, docs | Crystal source audit for `HumanObject.CanUseItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation town_teleport -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet -- --test-threads=1 --nocapture`; `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-24-R41

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal dead-state `UseItem` parity for ordinary items plus alive/dead `ResurrectionScroll` behavior | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.UseItem` shape-6 and `HumanObject.CanUseItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation dead_player -- --test-threads=1 --nocapture`; `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-24-R40

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal dead-state current item mutation family for `BuyItem` / `DeleteItem` / `SellItem` / `RepairItem` / `DropItem` / `CombineItem` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for current dead-player item/service branches; focused `cargo +1.89.0 test --locked -p mir2-simulation dead_player -- --test-threads=1 --nocapture`; adjacent current item/service packet tests; `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-24-R38

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal monster-drop map `NoDropMonster` suppression for normal kills, field-wasp quest drop, and harvest loot | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `MonsterObject.Drop` / `DropItem` and harvest paths; focused `cargo +1.89.0 test --locked -p mir2-simulation no_drop_monster_map_rule -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation harvest -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation drop -- --test-threads=1 --nocapture`; `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-24-R37

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal `DropItem` map `NoThrowItem` rejection and `CanNotDrop` message parity | Coordinator | `apps/simulation/src/runtime.rs`, map metadata/config if needed, docs | Crystal source audit for `PlayerObject.DropItem` map-flag branch; focused `cargo +1.89.0 test --locked -p mir2-simulation drop_item_packet -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation item -- --test-threads=1 --nocapture`; `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-23-R36

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal `DropItem` rejects rental `BindingFlags.DontDrop` ack-only | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.DropItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation drop_item_packet -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation item -- --test-threads=1 --nocapture`; `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-23-R35

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal bounded hero-inventory packet guard audit for current `DropItem` / `CombineItem` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for hero-inventory packet routing; focused `cargo +1.89.0 test --locked -p mir2-simulation hero_inventory -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation item -- --test-threads=1 --nocapture`; `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-23-R34

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal `DeleteItem` ignores packet `HeroInventory` and still deletes matching player inventory by unique id | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `MirConnection.DeleteItem` / `PlayerObject.DeleteItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation delete_item_packet -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation item -- --test-threads=1 --nocapture`; `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-23-R33

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal current item packet unique-id cleanup for `UseItem`, `EquipItem`, and `MergeItem` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for current item packet unique-id usage; focused `cargo +1.89.0 test --locked -p mir2-simulation unique_id -- --test-threads=1 --nocapture`; `cargo +1.89.0 test --locked -p mir2-simulation item -- --test-threads=1 --nocapture`; `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-23-R32

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal current inventory unique-id cleanup for `CombineItem` and current bag item packet lookups | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `CombineItem`, `SplitItem`, `DeleteItem`, `DropItem`, `SellItem`, `RepairItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation unique_id -- --test-threads=1 --nocapture`; `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-23-R31

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal player `GemRatePercent` for current inventory-grid `CombineItem` upgrade chance | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit, focused `GemRatePercent` upgrade regression, `cargo +1.89.0 fmt --check`, `cargo +1.89.0 test -p mir2-simulation combine_item_packet_upgrade_branch_applies_player_gem_rate_percent_bonus -- --test-threads=1 --nocapture`, `cargo +1.89.0 test -p mir2-simulation combine_item_packet -- --test-threads=1 --nocapture`, `cargo +1.89.0 test -p mir2-simulation storage -- --test-threads=1 --nocapture`, `cargo +1.89.0 test -p mir2-simulation item -- --test-threads=1 --nocapture`, full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-23-R30

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal rental binding flags for current storage and combine item paths | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit, focused rental `DontStore` / `DontUpgrade` regressions, `cargo +1.89.0 fmt --check`, `cargo +1.89.0 test -p mir2-simulation combine_item_packet -- --test-threads=1 --nocapture`, `cargo +1.89.0 test -p mir2-simulation storage -- --test-threads=1 --nocapture`, `cargo +1.89.0 test -p mir2-simulation item -- --test-threads=1 --nocapture`, full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-23-R29

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal inventory-grid `CombineItem` repair-hammer and sewing parity | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit, focused repair packet regressions, `cargo +1.89.0 fmt --check`, `cargo +1.89.0 test -p mir2-simulation combine_item_packet -- --test-threads=1 --nocapture`, `cargo +1.89.0 test -p mir2-simulation storage -- --test-threads=1 --nocapture`, `cargo +1.89.0 test -p mir2-simulation item -- --test-threads=1 --nocapture`, full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-23-R28

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal `CombineItem` target item-type gating across packet branches | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit, focused socket/seal packet rejection regressions, `cargo +1.89.0 fmt --check`, `cargo +1.89.0 test -p mir2-simulation combine_item_packet -- --test-threads=1 --nocapture`, `cargo +1.89.0 test -p mir2-simulation storage -- --test-threads=1 --nocapture`, `cargo +1.89.0 test -p mir2-simulation item -- --test-threads=1 --nocapture`, full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-23-R27

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal inventory-grid `CombineItem` shape-3/4 gem/orb upgrade parity | Coordinator | `packages/protocol/src/ids.rs`, `packages/protocol/src/packets.rs`, `packages/protocol/src/trace.rs`, `packages/protocol/tests/codec.rs`, `apps/gateway/src/web.rs`, `apps/simulation/src/runtime.rs`, docs | Crystal source audit, protocol/gateway/runtime `ItemUpgraded` coverage, persisted `gem_count` flow-through, `cargo +1.89.0 fmt --check`, `cargo +1.89.0 test -p mir2-protocol item_slot_seal_and_upgrade_server_packets_use_crystal_ids -- --nocapture`, `cargo +1.89.0 test -p mir2-gateway item_slot_and_seal_server_events_expose_crystal_payload_fields -- --nocapture`, `cargo +1.89.0 test -p mir2-simulation combine_item_packet -- --test-threads=1 --nocapture`, `cargo +1.89.0 test -p mir2-simulation storage -- --test-threads=1 --nocapture`, `cargo +1.89.0 test -p mir2-simulation item -- --test-threads=1 --nocapture`, full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-23-R26

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal `CombineItem` packet parity for current socket/seal branches | Coordinator | `packages/protocol/src/ids.rs`, `packages/protocol/src/packets.rs`, `packages/protocol/src/trace.rs`, `packages/protocol/tests/codec.rs`, `apps/gateway/src/web.rs`, `apps/simulation/src/runtime.rs`, docs | Crystal source audit, protocol/gateway/runtime `CombineItem` coverage, `cargo +1.89.0 fmt --check`, `cargo +1.89.0 test -p mir2-protocol item_and_combat_client_packets_use_crystal_payloads -- --nocapture`, `cargo +1.89.0 test -p mir2-protocol item_action_ack_server_packets_use_crystal_ids -- --nocapture`, `cargo +1.89.0 test -p mir2-gateway combine_item_server_event_exposes_crystal_payload_fields -- --nocapture`, `cargo +1.89.0 test -p mir2-simulation combine_item_packet -- --test-threads=1 --nocapture`, `cargo +1.89.0 test -p mir2-simulation storage -- --test-threads=1 --nocapture`, `cargo +1.89.0 test -p mir2-simulation item -- --test-threads=1 --nocapture`, full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-23-R25

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal storage item flag/rejection semantics | Coordinator + Explorers | `apps/simulation/src/runtime.rs`, `apps/simulation/Cargo.toml`, docs | Crystal source audit, `NPCStorage` service-context activation, end-to-end `@Storage` store/take-back regression, `cargo +1.89.0 fmt --check`, `cargo +1.89.0 test -p mir2-simulation storage -- --test-threads=1 --nocapture`, `cargo +1.89.0 test -p mir2-simulation item -- --test-threads=1 --nocapture`, full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-23-R24

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal `SellItem` item flag/type rejection semantics | Coordinator + Explorers | `apps/simulation/src/runtime.rs`, docs | Crystal source audit, focused sell rejection tests, `cargo test -p mir2-simulation sell`, `cargo test -p mir2-simulation item`, full `mir2-simulation` regression |

## Completed Round: 2026-04-22-R23

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal repair service rejection/cost semantics | Coordinator + Explorers | `apps/simulation/src/runtime.rs`, docs | Crystal source audit, focused repair rejection tests, `cargo test -p mir2-simulation item`, full `mir2-simulation` regression |

## Completed Round: 2026-04-22-R22

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal NPC BuyItem rejection edge semantics | Coordinator + Explorers | `apps/simulation/src/runtime.rs`, docs | Crystal source audit, focused buy rejection tests, `cargo test -p mir2-simulation item`, full `mir2-simulation` regression |

## Completed Round: 2026-04-22-R21

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal sell/game-shop/mail rejection semantics | Coordinator + Explorers | `apps/simulation/src/runtime.rs`, `apps/simulation/src/config.rs`, docs | Crystal source audit, focused sell/credit-shop/mail tests, `cargo test -p mir2-simulation item`, full `mir2-simulation` regression |

## Completed Round: 2026-04-22-R20

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal harvest owner/EXPOwner scan rejection semantics | Coordinator + Explorers | `apps/simulation/src/runtime.rs`, docs | Crystal source audit, focused owner-rejected/group-member corpse tests, `cargo test -p mir2-simulation harvest`, `cargo test -p mir2-simulation drop`, full `mir2-simulation` regression |

## Completed Round: 2026-04-22-R19

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal HarvestMonster transfer timing and leftover inventory semantics | Coordinator + Explorers | `apps/simulation/src/runtime.rs`, docs | Crystal source audit, focused Hen/Deer/pass-count/pending-drop tests, `cargo test -p mir2-simulation harvest`, `cargo test -p mir2-simulation drop`, full `mir2-simulation` regression |

## Completed Round: 2026-04-22-R18

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal drop visibility and pickup rejection edges | Coordinator + Explorers | `apps/simulation/src/runtime.rs`, docs | Crystal source audit, focused owner/full-bag/overweight pickup tests, `cargo test -p mir2-simulation pickup`, `cargo test -p mir2-simulation drop`, `cargo test -p mir2-simulation harvest`, full `mir2-simulation` regression |

## Completed Round: 2026-04-22-R17

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal `GROUP` drop semantics | Coordinator + Explorers | `packages/tooling`, `packages/game-data`, `apps/simulation/src/runtime.rs`, docs | Crystal source audit, generated drop parser tests, focused group-drop tests, `cargo test -p mir2-game-data`, `cargo test -p mir2-simulation drop`, `cargo test -p mir2-simulation item`, full `mir2-simulation` regression |

## Completed Round: 2026-04-22-R16

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Data-driven `RandomItemStats.ini` manifest import | Coordinator + Worker | `packages/tooling`, `packages/game-data`, `apps/simulation/src/runtime.rs`, docs | generated manifest tests, focused random-stat tests, `cargo test -p mir2-game-data`, `cargo test -p mir2-simulation drop`, `cargo test -p mir2-simulation item`, full `mir2-simulation` regression |

## Completed Round: 2026-04-22-R15

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Full random-stat family source mapping and runtime payload baseline | Coordinator + Explorers | `apps/simulation/src/runtime.rs`, docs | Crystal source audit, `cargo fmt --check`, focused random-stat/persistence tests, `cargo test -p mir2-simulation drop`, `cargo test -p mir2-simulation item`, `cargo test -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-22-R14

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Seal reseal-delay metadata baseline | Coordinator | `apps/simulation/src/runtime.rs`, `apps/simulation/src/config.rs`, docs | `cargo fmt --check`, focused seal tests, `cargo test -p mir2-simulation item`, legacy save test |

## Completed Round: 2026-04-22-R13

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Socket source gem validation baseline | Coordinator + Explorer | `apps/simulation/src/runtime.rs`, docs | `cargo fmt --check`, focused socket tests, `cargo test -p mir2-simulation item` |

## Completed Round: 2026-04-22-R12

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Seal source item validation baseline | Coordinator | `apps/simulation/src/runtime.rs`, docs | `cargo fmt --check`, focused seal tests, `cargo test -p mir2-simulation item` |

## Completed Round: 2026-04-22-R11

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Frontend scene target keyboard action chain | Coordinator | `apps/web/app/original-client-shell.tsx`, docs | `npm.cmd run build --prefix apps\web` |

## Completed Round: 2026-04-22-R10

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Implement BenedictionOil curse/no-effect branches | Coordinator | `apps/simulation/src/runtime.rs` | `cargo fmt --check`, focused BenedictionOil tests, `cargo test -p mir2-simulation item` |

## Completed Round: 2026-04-22-R9

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Implement seal already-sealed validation first stage | Coordinator | `apps/simulation/src/runtime.rs` | `cargo fmt --check`, focused seal tests, `cargo test -p mir2-simulation item` |

## Completed Round: 2026-04-22-R8

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Implement socket slot-capacity validation first stage | Coordinator | `apps/simulation/src/runtime.rs` | `cargo fmt --check`, focused socket tests, `cargo test -p mir2-simulation item` |

## Completed Round: 2026-04-22-R7

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Select next backend/frontend parity bite from explorer findings | Coordinator + Explorers | docs | R7 selected NPC buy-back / used-goods parity |
| [x] | Implement NPC buy-back persistence, expiry, and used-goods baseline | Coordinator | `apps/simulation/src/runtime.rs`, `apps/simulation/src/config.rs` | `cargo fmt --check`, focused buy-back tests, `cargo test -p mir2-simulation sell`, `cargo test -p mir2-simulation npc` |

## Completed Round: 2026-04-22-R6

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Added-stat ground item display investigation | Coordinator | none | Crystal `ItemObject` / Rust packet/render map |
| [x] | Implement added-stat cyan ground item display baseline | Coordinator | `apps/simulation/src/config.rs`, `apps/simulation/src/runtime.rs`, `apps/web/app/page.tsx`, `apps/web/app/original-client-shell.tsx` | `cargo fmt --check`, focused colour tests, `cargo test -p mir2-simulation drop`, `npm.cmd run build --prefix apps\web` |

## Completed Round: 2026-04-22-R5

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal random-stat source investigation | Crystal Explorer | none | source paths and behavior notes |
| [x] | Rust item-stat/import implementation investigation | Rust Explorer | none | bounded implementation map |
| [x] | Implement current random-stat roll baseline | Coordinator | `apps/simulation/src/runtime.rs` | `cargo fmt --check`, focused random/drop/harvest tests |

## Completed Round: 2026-04-22-R4

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Implement frontend login/select/game shell first patch | Frontend Worker | `apps/web/app/original-client-shell.tsx` | `npm.cmd run build --prefix E:\mir2\mir2-web3\apps\web` |
| [x] | Review and integrate frontend shell patch | Coordinator | docs and frontend queue | build verified locally |

## Completed Round: 2026-04-22-R3

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal quest-drop `Q` gating source investigation | Crystal Explorer | none | source paths and behavior notes |
| [x] | Rust quest/drop implementation investigation | Rust Explorer | none | function/test map |
| [x] | Frontend shell first-patch investigation | Frontend Explorer | none | bounded write-set recommendation |
| [x] | Implement backend Crystal quest-drop gating | Coordinator | `apps/simulation/src/runtime.rs` | `cargo fmt --check`, focused drop/quest/harvest tests |

## Completed Round: 2026-04-22-R2

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal `DropStackSize` / ground-drop position source investigation | Crystal Explorer | none | source paths and behavior notes |
| [x] | Rust ground-drop placement implementation investigation | Rust Explorer | none | function/test map |
| [x] | Implement backend Crystal `DropStackSize` and drop-position search | Coordinator | `apps/simulation/src/runtime.rs` | `cargo fmt --check`, focused and broad drop tests |

## Completed Round: 2026-04-22-R1

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal `AddItem` belt-priority source investigation | Crystal Explorer | none | source paths and behavior notes |
| [x] | Rust inventory/belt implementation investigation | Rust Explorer | none | function/test map |
| [x] | Frontend 1:1 acceptance matrix investigation | Frontend Explorer | none | QA matrix proposal |
| [x] | Implement backend Crystal `AddItem` belt-priority | Coordinator | `apps/simulation/src/runtime.rs` | `cargo fmt --check`, focused item gain/use/pickup tests |
| [x] | Create orchestration docs and Candidate workflow | Coordinator | `docs/AGENT-ORCHESTRATION.md`, `docs/AGENT-TASK-QUEUE.md`, `docs/AGENT-RUN-LOG.md`, `docs/PLAYER-QA-SCRIPT.md` | docs created |

## Backend Queue

| Status | Task | Notes |
| --- | --- | --- |
| [x] | Crystal `AddItem` belt-priority placement | Potion/Scroll/Script effect 1 -> belt 0..3, Amulet -> belt 4..5, fallback to bag, belt `UseItem` consumes belt slot. |
| [x] | Crystal ground-drop position search and `DropStackSize` | Current player item drops, player gold drops, and monster ground drops use Crystal `ItemObject.Drop(distance)` placement semantics. |
| [x] | Production Crystal map monster screenshot verification | Gateway release `20260521T0830Z-spreadrep` keeps low-density current-map visible respawn representatives but spreads them over nearby walkable cells. Live `mir2.obelisk.build` screenshots/states for BichonProvince, WoomyonWoods(S), NaturalCave, DeadMineEntrance, InsectCave_2F, and ZumaMaze are under `docs/generated/player-qa/live-map-monsters/`, with `network404=0`, Monster meta `503=0`, and Monster PNG failed count `0`. |
| [x] | Crystal quest-drop `Q` gating | `Q` entries now roll normally, route to active matching quest inventory, suppress ground fallback, and preserve full quest-inventory failures. |
| [x] | Random item stat generation | Current runtime rolls the full Jev profile family baseline for imported Crystal drop items from generated `RandomItemStats.ini` manifest data, including `MaxDura`, all supported `UserItemStat` families, curse flag, and socket slots; metadata survives pickup, harvest, equipment/inventory state, and save/reload. |
| [x] | Crystal `GROUP` drop semantics | Drop manifest entries can now preserve nested `GROUP`, `GROUP*`, and `GROUP^` trees, and runtime recursively applies Crystal group behavior: successful child gold accumulates, `GROUP*` keeps one successful item, `GROUP^` short-circuits after the first successful child, and nested group rules compose. |
| [x] | Crystal drop visibility and pickup rejection edges | Crystal source shows owned item/gold drops are broadcast immediately; owner windows restrict pickup only. Current `PickUp` scans the current cell, skips owner-blocked/full-bag/gold-cap candidates when later pickable drops exist, and treats bag weight as post-gain state instead of a pickup/harvest rejection gate. |
| [x] | Crystal HarvestMonster pending transfer semantics | Harvest monsters now generate and persist pending `_drops` after the configured skin count, transfer them on the next harvest call, preserve leftover drops when the bag cannot accept every item, and avoid re-rolling pending harvest rewards. |
| [x] | Crystal harvest owner/EXPOwner rejection | Harvest target scanning now skips corpses owned by another player unless the owner is in the configured group set, emits Crystal `NoNearbyOwnedCarcasses` only when no eligible corpse is found, and attaches current-player harvest ownership when a harvest monster is defeated. |
| [x] | Crystal NPC `BuyItem` rejection edges | `BuyItem` now silently rejects invalid panel/count, missing active NPC service, non-buy service pages such as `@Repair`, missing goods/metadata, insufficient gold, and full-bag purchases without mutating gold or inventory. |
| [x] | Crystal NPC `RepairItem` / `SRepairItem` rejection and cost edges | NPC repair now uses current backpack item unique ids, requires the matching active `@Repair` / `@SRepair` service page, applies Crystal repair/special-repair cost and normal max-dura loss semantics, emits `LoseGold` / `ItemRepaired` on success, and preserves Crystal message/silent rejection edges for non-repairable items, type mismatch, and insufficient gold. |
| [x] | Crystal NPC `SellItem` remaining rejection edges | `SellItem` now follows Crystal ack-only failures for zero count, missing service/item/count, `DontSell`, and partial-stack gold overflow; emits `CannotSellItemHere` only for script type mismatch; uses `UserItem.Price() / 2` style sale value; and preserves full-stack gold-cap clamping. |
| [x] | Crystal storage item flag/rejection edges | R25 now aligns `StoreItem` / `TakeBackItem` active `@Storage` / `NPCStorage` service context, `DontStore`/rental flags, password lock, accessible capacity, occupied-target no-swap behavior, and ack-only failure semantics. |
| [x] | Added-stat cyan ground item display | Current added-stat ground drops now surface Crystal Cyan through `ObjectItem.name_colour_argb`, world snapshots, and the web ground-drop label. |
| [x] | NPC buy-back expiry / used-goods persistence | Buy-back entries now persist across save/reload, carry Crystal 60-minute expiry, expire into NPC used goods, and used goods can be bought back through Buy/BuyUsed flows. |
| [~] | Full gem/socket validation | Socket slot-capacity validation, source gem validation, the real inventory-grid `CombineItem` packet path, shape-1/2/5/6 repair-hammer/sewing parity, bounded shape-3/4 gem/orb upgrade parity with `ItemUpgraded` / persisted `gem_count`, shared Crystal target-type gating, rental `DontUpgrade` rejection for current socket/upgrade combine branches, equipment-backed player `GemRatePercent` success bonus, current bag-item unique-id lookup cleanup, current item packet `UseItem` / `EquipItem` / `MergeItem` unique-id cleanup, Crystal `DeleteItem` hero-flag ignore semantics, and bounded current `DropItem` / `CombineItem` hero-inventory no-player-mutation guards are in. Broader hero-inventory handling and other gem-family branches remain. |
| [~] | Full seal-source validation | Already-sealed rejection, source item validation, reseal-delay metadata, save/reload, the real inventory-grid `CombineItem` packet path, and shared Crystal target-type gating are in. Hero-inventory handling and remaining shared combine-branch gaps remain. |
| [~] | Map event script bindings | Six `_MAPCOORD` gates and real DB light/dark-light/weather/music/fire/lightning metadata are wired fail-closed. General Event commands, exact RNG traces, and door/wall/gate bindings remain open. |
| [ ] | Broader combat/skill parity | Spell tables, projectile objects, buff edge cases, live packet comparison. |

## Frontend Queue

| Status | Task | Notes |
| --- | --- | --- |
| [x] | Build frontend 1:1 acceptance matrix | Evidence Gate, panel matrix, and `docs/FRONTEND-1TO1-GAPS.md` are in place. |
| [~] | Login/select/game shell Crystal visual pass | First bounded patch landed: tile pointer double-dispatch guard and Enter-key login submit. Pixel/human comparison remains open. |
| [~] | Inventory/equipment/belt interaction parity | Belt slots 1-6, rotate, close, basic occupied/empty visual states, and hotkey `1` item use are smoke-verified; item drag/split/merge/drop/tooltips and inventory/equipment panel interactions remain. |
| [ ] | NPC dialog/shop/storage UI parity | Link flow, input pages, shop goods, repair/storage panels. |
| [~] | Combat HUD and target feedback parity | Selected-target keyboard approach/primary actions and localized action-distance feedback are in; HP/MP, attack feedback, object packets, and damage/struck display remain. |
| [~] | Map/minimap interaction parity | R303 all-map source audit confirms 463/463 manifest map files are present and parser-supported, and the 2026-05-16 all-map audit closes automated source/fallback risk with Crystal no-draw frame classification plus movement/respawn/NPC/static semantic checks. Remaining risk is full-map visual comparison/human acceptance. |
| [~] | Screenshot baseline pack | Desktop 1024x768 and compact 820x640 Stage 5 route screenshots are captured with manifest bounds; broader mobile/route coverage and Crystal comparison remain open. |

## Assets/Data Queue

| Status | Task | Notes |
| --- | --- | --- |
| [ ] | Event binding manifest | Map event scripts and referenced script validation. |
| [~] | Full visual asset coverage audit | R303 covers all manifest maps at source-file/parser/sampled-frame level, and the 2026-05-16 all-map audit closes the automated source/fallback risk with Crystal no-draw frame classification plus gameplay semantic checks. Remaining work: representative screenshot/human comparison for sprites, effects, sounds, icons, density, and map feel. |
| [ ] | Economy table import audit | Credit products, shop tables, refine/gem/seal probabilities. |
| [~] | Full map metadata audit | R248 covers generated map metadata for transfers/safe zones/minimap/bigmap/light/drop rules, and R303 verifies source map files/parser coverage for all 463 manifest maps. Weather, fire, door/wall/gate/object state and full visual comparison remain open. |

## QA/Integration Queue

| Status | Task | Notes |
| --- | --- | --- |
| [~] | Packet trace live Crystal fixture setup | R298 has a working Windows fixture: Crystal `127.0.0.1:7000`, local gateway `127.0.0.1:7310`, `CRYSTAL_CLIENT_ROOT=E:\mir2\Crystal\Build\Client\Debug`, account `cdx0428030348`, character `Cdx0428030348`, index `8`. Stable matrix is clean; strict exact diff remains dirty. |
| [ ] | Representative local-vs-Crystal trace matrix | Login, start, move, combat, pickup, NPC, item, map transfer. |
| [~] | Stage screenshot comparison harness | Stage 5 smoke archives route screenshots plus named desktop/compact viewport metadata; R303 adds all-map source-resource coverage evidence; true baseline diffing against Crystal/reference images remains open. |
| [x] | 100% Candidate gate command bundle | `infra/check-candidate-gate.sh` now provides `local`, `full`, and `live` scopes, and `.github/workflows/mir2-candidate-gate.yml` runs the local scope in CI. `MIR2_CANDIDATE_SCOPE=local bash infra/check-candidate-gate.sh` passed on 2026-05-06, covering the architecture gate, `mir2-game-data` 27/27, `packet_trace` bin 16/16, Admin Web typecheck, Player Web typecheck, and `git diff --check`. `full` and `live` are the explicit command bundle for build/static smoke and running Gateway/Web evidence refreshes. |
| [ ] | Final human QA route | Keep under 40 hours by batching checks and evidence. |

## 2026-07-23 Visual-Parity Queue Sync

| Status | Task | Notes |
| --- | --- | --- |
| [x] | Deterministic same-account pair | r32 Night and r33 Dawn are overlay-free, same state/map/coordinate, fixed-light, zero-error pairs. |
| [x] | Camera/HUD/light/minimap normalization slice | Dawn full/world changed pixels fell from `36.4%/40.2%` to `24.2%/26.1%`; Night remains `12.5%/12.6%`. |
| [x] | Visual parity regression gates | Connected-handshake, stale-socket, secret-redaction, cursor-parking, HUD metric, minimap, Rust AI/light, and dual-backend tests are green. Final headed WebGPU and WebGL2 runs each pass 28/28 strict movement/map assertions. |
| [x] | Captured Bichon source-frame closure | Commit the 555 deterministic missing map PNGs; generated map-atlas output stays ignored. |
| [x] | GDI text and deterministic dynamic-state pass | r40 adds exact-key Windows GDI assets, Crystal four-line chat state, shared TCP/Web system-chat scheduling, and persistent seeded per-object animation phases; WebGPU/WebGL2 and temporal gates are green. |
| [ ] | Final human visual/feel Accepted decision | Automated status is 100% Candidate. Compare the r40 native/Web pair and live windowed clients; do not reinterpret independent roaming actors, random particles, or compositor sampling as a deterministic implementation defect. |
