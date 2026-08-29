# Crystal Server Parity

> Windows NPC quest-marker server/client note (2026-08-29; follow-on over
> revision `630dd957e0f5dcbee6e03e366efe6f82c20b8484`): server-owned personal quest
> state now selects Crystal's exact `QuestIcon` discriminant and exposes it in
> the NPC world snapshot. Active finish-NPC quests win in insertion order;
> only then do level/class/prerequisite-valid start-NPC quests participate.
> Shared Zone state never owns the per-character icon, and Gateway reapplies it
> from the requesting personal snapshot after world composition. The q1
> Jane/Jude transition, native partial-packet retention, focused two-session
> Gateway isolation, Windows 483/483 and Candidate self-test pass. The client
> now uses exact NPC standing-frame-zero geometry with Crystal's marker formula
> and renders it independently of `NameView`; same-EXE body-anchor/live-transition
> acceptance still remains, and the supplied `Prguse`
> export has no drawable daily-blue `991..994` frames. Broader quest/server,
> persistence, distributed ownership, device and signing denominators remain
> open; no server/global percentage is reported.

> Windows safe-zone server-client note (2026-08-29): revision
> `aae9c2c7e06dbceb6f6539c7b29eba63ece293c4` preserves existing safe-zone
> authority and packet shapes, but removes a configuration mismatch: the
> imported Crystal server has `SafeZoneBorder=True`, its generated map data
> contains persistent `TrapHexagon` boundary objects, and Simulation now emits
> them by default instead of hard-coding the option off. Explicit disable is
> still supported. Windows consumes the existing `ObjectSpell` lifecycle and
> exact `Magic 1390..1399` animation; `inSafeZone` reaches the shared read
> model. Full Simulation 1482/1482 and focused client tests pass. Same-EXE and
> human visual acceptance remain open, with no server/global percentage claim.

> Windows pointer/NPC/motion server-client note (2026-08-29): revision
> `4d035489a966d827ef5aa49567d4b53bf344d2a7` adds no server packet or
> authority. Cursor states and the right-click NewMove marker are exact
> client-owned presentation; one native self-motion window follows existing
> authoritative movement. The NPC compatibility bridge approaches through
> existing Zone movement and sends one existing interaction at adjacency.
> Crystal's direct `CallNPC [@Main]` within `Globals.DataRange` remains open
> server-semantic work and is not declared equivalent. Runtime 199/199,
> Windows 463/463 and Candidate verification pass; no server/global percentage
> is reported.

> FireBounce server-client checkpoint (2026-08-28): revision `90f861a9d`
> preserves Crystal's authoritative timing boundary across shared Zone and
> Windows presentation. ObjectMagic owns the local first leg, whose hit is due
> after `500ms + 50ms/tile`; a successful hit then selects at most one live,
> hostile, path-clear target within radius three, emits an ObjectProjectile
> for that hop and delays the next hit by `50ms/tile`. Pending hit, target
> location and remaining bounces survive checkpoint replay. Native presentation
> deduplicates only the legacy first-leg supplement and keeps later hop packets.
> Shared Zone 204/204 and Windows 436/436 pass. The current deterministic Zone
> selector remains an explicit difference from Crystal runtime randomness, and
> this checkpoint does not close the complete spell, combat, persistence,
> live-WSS, device, human or signing denominators. No server or global
> percentage is reported.

> VIS-02 Hallucination server-client note (2026-08-28): revision
> `60eae9561c5b18bc79456105e455d6964c14fafe` adds no packet shape or
> server authority. Existing spell id, target identity and movement feed the
> client-owned Crystal action/projectile clock; a present target gets impact
> and `M76-0.wav` unless its rendered action is terminal `Dead` at completion.
> Missing targets fail closed. Windows 421/421, exporters, Candidate gates and
> P0=0/P1=0/P2=0 review pass. Live WSS, the complete spell denominator and all
> exact-head device/human/signing gates remain open; no server or global
> percentage is reported.

> VIS-01/VIS-02/VIS-04 server-client note (2026-08-28): revisions
> `2a83c0062dd60916730c46c752e044f668b243db`,
> `473a56137c7af458d5c982c90f3d4a658a9243fd` and
> `fd3b5d552bbb9292ce49d95709477da3f6966d38` are presentation-only. They
> consume existing player guild/name colour, typed FrostCrunch target/spell
> events, and Scarecrow movement packets; no packet shape, damage/freeze,
> movement, simulation or shared-Zone authority changes. Exact client frames,
> audio identity, lifecycle and transcripts pass the combined Windows 416/416
> and focused gates. Live WSS and the complete semantic denominators remain
> open; no server or global percentage is reported.

> VIS-02 FireBall-family server-client note (2026-08-28): revision
> `8d8c5f12f6faa4617ce87017f82738458f164bd9` adds no server packet or
> authority. Existing target identity, spell and projectile timing feed a
> client-owned Crystal completion callback: only the terminal rendered
> `Dead` action suppresses FireBall, GreatFireBall and SoulFireBall impact
> bitmap/audio; `Die` remains visible and audible. Windows 410/410 and review
> pass. Other missiles, Web parity, live WSS and the complete denominator
> remain open; no server or global percentage is reported.

> VIS-04 Scarecrow Revive server-client note (2026-08-28): revision
> `04121747c70d1c5487947f027d07b5209ca84f6c` adds no packet or
> server authority. Existing `ObjectRevived` remains authoritative for life
> state and its existing `effect` flag; the client no longer lets a stale 0%
> derived marker reverse that decision. `Monster/005` signed frames and return
> to Standing are source-locked, with Windows 407/407 and Web gates passing.
> Zone policy, live WSS and the complete monster denominator remain open; no
> server or global percentage is reported.

> VIS-01 corpse-name server-client note (2026-08-28): revision `cda55ef5a`
> adds no packet or server authority. Existing authoritative `dead` state now
> drives the exact client-owned Crystal name placement: keep the name, shift it
> 27px, and keep dead self health hidden. Windows 406/406 and independent
> review pass. Guild labels, complete player presentation, live WSS and the
> full denominator remain open; no server or global percentage is reported.

> Shared-Zone wall-clock respawn server checkpoint (2026-08-28): revision
> `7f991ec34fbde6ac07a5799b35d352f2785c1aa9` makes `ZoneRuntime`
> the sole authority for monster death-to-new-incarnation scheduling. It
> preserves the Crystal delay roll, harvestable corpse gate, recovery due time,
> late-join suppression and one-revive/multi-observer semantics. A new trusted
> `CancelPendingMovement` command is server-internal only; Gateway invokes it
> before NPC/dialog and quest operations so an accepted Walk intent cannot
> cross the interaction boundary. `shared_zone` 203/203, focused regressions,
> dual-crate check and the ordinary Gateway Q1-to-Q4 turn-in/reload path pass.
> This does not close all Zone persistence, cross-Gateway ownership, content or
> AI semantics, and it adds no client-exposed debug authority. No server or
> global percentage is reported.

> VIS-03 seven-button HUD server-client note (2026-08-28): revision
> `4f7efffca093cb59d0e4f468dbd08ea2c61d314f` is entirely local
> presentation. It adds no packet, server state or gameplay authority; exact
> `Prguse` button states and ButtonA/ButtonC cues are resolved from existing
> client actions. Inventory expansion remains outside this commit. No exact-
> head live WSS or whole-game percentage is reported.

> VIS-04 Scarecrow Struck-audio server-client note (2026-08-28): revision
> `354bb9f9648758c9f38d5ce149a273ae07cd2a7e` adds no server packet or
> authority. Crystal's client-owned `Monster/005` flinch-first plus optional
> attacker-weapon clang now resolves exact `005-2.wav` then `60..65.wav` on
> native and Web from existing typed events and authoritative actor context.
> Windows 406/406, Bevy 419/419, runtime 191/191, Web/audio/export/typecheck
> and Candidate script gates pass; review is P0=0/P1=0/P2=0. Other monster
> actions/families, live WSS and the complete denominator remain open. No
> server or global percentage is reported.

> VIS-04 Scarecrow Attack1-audio server-client note (2026-08-28): revision
> `e1dd6d6379d23efeafe57aa01c170452f1261b83` adds no server packet or
> authority. Crystal's client-owned `Monster/005 BaseSound+1` projection now
> resolves exact `005-1.wav` on native and Web from existing typed
> `ObjectAttack` events plus authoritative actor context. Windows 403/403,
> Bevy 419/419, runtime 191/191, Web/audio/export/typecheck and Candidate
> script gates pass; review is P0=0/P1=0. Flinch/weapon-struck ordering, other
> monsters, live WSS and the complete denominator remain open. No server or
> global percentage is reported.

> VIS-04 Scarecrow death-audio server-client note (2026-08-28): revision
> `cf4f5b5197c492324be23beb73611c0e0162c403` adds no server packet or
> authority. Crystal's client-owned `Monster/005 BaseSound+3` projection now
> resolves exact `005-3.wav` on native and Web from existing typed death
> events; identity replay and local lifecycle cleanup are bounded. Windows
> 401/401, Bevy 419/419, runtime 191/191, Web/audio/export/typecheck and
> Candidate script gates pass; review is P0=0/P1=0. The later checkpoint above
> closes Attack1; flinch/struck, other monsters, live WSS and the complete
> denominator remain open. No server
> or global percentage is reported.

> VIS-03 Inventory locked-second-tab server-client note (2026-08-28): revision
> `83f081149375fb402b9c7e6711fdb4e6bed68a0e` changes no authoritative
> inventory, purchase, protocol, simulation or Zone semantics. The optional
> Gateway snapshot field is normalized at the client boundary to Crystal's
> exact `46,54,58,...,86` domain; absence/invalidity locks page two and the
> local click emits only ButtonA. Focused 5/5, Bevy 419/419, Windows 399/399,
> runtime 191/191 and package/verifier self-tests pass; final review is
> P0=0/P1=0. Production `inventoryCapacity` emission and the Crystal
> `ExtraSlots8`/`@ADDINVENTORY` expansion chain remain open. No exact-head EXE,
> package, live WSS or screenshot was produced. Same-EXE/DPI/soak/human/
> signing/denominator gates remain open; global parity remains unreported.

> VIS-02 Healing server-client note (2026-08-28): revision
> `24d9b73a30fc18edf0649283d14495c6f4900aff` adds no server packet or
> authority. The caster `Magic/200..209`, target-owned `Magic/370..379`, exact
> `M61-0.wav`/`M61-1.wav` and their lifecycle are client projections over
> existing typed `ObjectMagic`/`ObjectEffect` fields. Focused 4/4, Windows
> 398/398, Bevy 416/416, Web gates and Candidate self-tests pass; review is
> P0=0/P1=0 with one non-blocking Web retransmit-deduplication P2. Existing
> Healing gameplay authority was not live-revalidated, and no exact-head EXE,
> package, live WSS or screenshot was produced. All same-EXE/DPI/soak/human/
> signing/denominator gates remain open; global parity remains unreported.

> VIS-03 HelpDialog movable server-client note (2026-08-28): revision
> `4545465a2e31a6646f247c55906764952d44cd58` changes no packet, simulation,
> Zone or Gateway authority. Grab-offset movement, shared-stage clamping,
> release/focus/Hide/reset cleanup and `Sort=true` z-order are local renderer
> state and emit no intent. Focused Help 14/14, Bevy 416/416 and Windows 394/394
> pass; review is P0=0/P1=0 for this leaf. Dynamic binding/localization and all
> same-EXE/DPI/soak/human/signing/denominator gates remain open. No server or
> global percentage is claimed.

> VIS-03 HelpDialog server-client note (2026-08-28): revision
> `e22f2aa4c683447b0e57805a580fd29e0a84c37c` changes no server packet,
> simulation, Zone or Gateway authority. Help visibility/page state, Menu/H/P
> input and ButtonA are strictly local; no UI or gameplay intent is emitted.
> The source-bound default-English/default-binding leaf passes Help 9/9,
> Bevy 411/411, Windows 394/394, ui-core registry 13/13 and Candidate script
> self-tests. Dynamic rebind/localization remains one reviewed client P1; at
> that revision dragging, exact typography and every same-EXE/DPI/soak/human/signing/
> denominator gate remain open. No server or global percentage is claimed.

> VIS-01 living hover-name server-client note (2026-08-28): revision
> `066f6f3b576cbdc03106c8a221ccdaf13f7dfa83` changes no server combat,
> session or Zone authority. It consumes the existing authoritative entity
> identity/name/color/HP projection and closes only a bounded local living
> name/hover/health presentation leaf. Windows 394/394 and final independent
> P0=0/P1=0 review pass. Corpse/DisplayBodyName, exact multiline formatting,
> same-EXE/live-WSS/GPU/DPI/soak/human/signing and the incomplete denominator
> remain open. No server or global percentage is claimed.

> VIS-02 LeftGuard range-projectile server-client note (2026-08-28): revision
> `d2dfff14308256c07c3b3169798afee0a051b97b` adds no server packet or
> authority. Exact `Monster/100` selection, the 400 ms frame-4 delay,
> Direction16 `Magic` missile, target following and source-owned lifecycle are
> native-client projection concerns over existing `ObjectRangeAttack` fields.
> LeftGuard 5/5, guard-range 10/10, Windows 392/392, the 74-spell exporter/
> validator and final P0=0/P1=0 review pass. No asset, audio, EXE, live WSS or
> screenshot was created. Monster ActionFeed and all final same-EXE/DPI/soak/
> human/signing/denominator gates remain open; global parity remains
> unreported.

> VIS-02 RightGuard range-hit server-client note (2026-08-28): revision
> `7d08b53f8d78161655254bb83ebd519ecbd62fed` adds no server packet or
> authority. Exact `Monster/099` selection, the 400 ms frame-4 delay,
> target-bound `Magic2/10..14` rendering and the 400/401 ms source-to-target
> ownership boundary are native-client projection concerns. Focused 6/6,
> Windows 387/387, the 74-spell exporter/validator and final P0=0/P1=0 review
> pass. No asset, EXE, live WSS/audio or screenshot was created. Missing
> `995.wav`, the full monster ActionFeed and all final same-EXE/DPI/soak/
> human/signing/denominator gates remain open; global parity remains
> unreported.

> VIS-03 CharacterDialog close server-client note (2026-08-28): revision
> `225ae951d95894458b7f1cbd30d78ee100fe4362` adds no server packet or
> authority. Exact close geometry/frames, pointer-only ButtonA and local
> panel/page reset are client concerns. Four-page edge, held/re-press,
> non-InGame and empty-intent tests pass with Bevy 402/402, Windows 381/381 and
> final P0=0/P1=0 review. No EXE, live WSS/audio or screenshot was created.
> Remaining Character/UI and every final real-window gate stay open; global
> parity remains unreported.

> VIS-03 CharacterDialog tabs server-client note (2026-08-28): revision
> `ac4ae1686ff60c01437100554c7a5d4cd6c78a65` adds no server packet or
> authority. Exact tab geometry/active frames, pointer-only ButtonA and local
> Character/Status/State/Skill selection are client concerns. Four-page edge,
> held/re-press and empty-intent tests pass with Bevy 402/402, Windows 381/381
> and final P0=0/P1=0 review. No EXE, live WSS/audio or screenshot was created.
> Page contents, remaining UI and every final real-window gate stay open;
> global parity remains unreported.

> VIS-01 hover-target server-client note (2026-08-28): revision
> `1deb930483f3eca5f26f11020f091454fc96b183` adds no server packet or
> authority. The native client consumes the existing authoritative actor
> projection for Crystal's body-alpha/same-tile hover scan and local 30%
> redraw. `HighlightTarget` is persisted locally and neither changes the
> selected combat object nor emits Gateway traffic. Windows 381/381, Bevy
> 402/402, ui-core 42/42, runtime 191/191 and focused 5/5 pass with two final
> P0=0/P1=0 reviews. Authenticated live delivery, same-EXE GPU pixels, real
> DPI/mouse feel, wider DrawBehind/special composites, Web symmetry, soak,
> human/signing and denominator gates remain open. No server or global
> percentage is emitted.

> VIS-03 Character HUD server-client note (2026-08-28): revision
> `849f1f0b5120867d1358e0e7db9ba675e9866f9c` adds no server packet or
> authority. Exact Character button normal/hover/pressed pixels, pointer-only
> ButtonA and CharacterPage-aware open/return/close behavior are local client
> concerns. Default C/F10 shares the transition without audio or outbound
> intent. Bevy 401/401, Windows 376/376, focused 4/4, script self-tests and
> final P0=0/P1=0 review pass. No EXE, live WSS/audio or screenshot was
> created. Remaining Character/HUD and all final real-window gates stay open;
> global parity remains unreported.

> VIS-03 Inventory ButtonA server-client note (2026-08-28): revision
> `5b70511316b084ac677b5978f7f03e440241ca4c` adds no server packet or gameplay
> authority. As in Crystal `MirControl.OnMouseClick`, the native client alone
> emits exact local `ButtonA=10103 -> 103.wav` once for the enabled Inventory
> HUD pointer edge before its callback; the keyboard toggle remains silent.
> UI/gameplay audio lifecycles are independent and the Candidate scripts bind
> the exact sound bytes. Windows 376/376, Bevy 397/397, focused 4/4, script
> self-tests and P0=0/P1=0 final review pass. No EXE, live audio/WSS or visual
> evidence was created. The rest of HUD interaction and every final real-
> window gate remain open; global parity stays unreported.

> VIS-01 selected-target server-client note (2026-08-28): revision
> `a58ab0aaa2202731a5c55e7a684261d6c15c2f8d` adds no server authority. The
> Windows client now consumes the existing selected object identity and
> authoritative actor projection as Crystal's post-world 30% full-composite
> redraw. Exact-atlas fail-closed behavior, dead-monster eligibility, selection
> lifecycle and the world/target/foreground-effect ordering are automated;
> Persistent ObjectSpell remains in the world pass. Windows 376/376, Bevy
> 393/393, shared runtime 191/191 and focused tests pass with no reviewed
> P0/P1. This does not prove authenticated live selection delivery, transparent
> pixel mouse targeting, hover behavior, general DrawBehind semantics or any
> same-EXE/GPU/DPI/soak/human/signing gate. No server or global percentage is
> emitted.

> VIS-02 GreatFireBall server-client note (2026-08-28): revision
> `9457e5618449d22350baedd01e3775f5b1fe59c6` changes no server combat
> authority. The Windows projection now follows Crystal's client-owned
> `ObjectMagic` cast, 600 ms local launch, sixteen directional six-frame
> missile ranges, target-bound impact and exact M34-0/M34-1/M34-2 audio; the
> compatibility `ObjectProjectile` cannot duplicate it. Source metadata, 90
> new direction frames, package requirements and byte/hash verification close
> the clean-checkout asset path. Windows 372/372, Bevy 393/393, focused 5/5,
> Gateway projection, Web full logic/type, offline resources and script
> self-tests pass with no remaining reviewed P0/P1. This does not prove live
> WSS timing, server damage/revalidation or a retained dead target's impact
> suppression. Same-EXE/GPU/DPI/soak/human/signing and denominator gates stay
> open; no server or global percentage is emitted.

> Web Crystal ActionFeed server-client note (2026-08-28): revision
> `7bc42cfd77e196297b165436716484732db18d83` adds no server authority. It makes
> the Web client preserve the server's consecutive Struck order as one current
> action and one queued Struck tail, applies queued pose/audio only at action
> start, and drops only a further tail duplicate. Death, revive and MapChanged
> clear the queue; packet-first snapshots preserve it otherwise. Full Web
> logic/type, state/store/event and offline resource/audio gates pass with no
> reviewed P0/P1. Regenerating the 331-entry present-sound manifest also closes
> the prior CI omission of existing `M79-1.wav` locally. Authenticated live
> delivery/audio ordering, final-head CI, same-EXE/GPU/DPI/soak/human/signing
> and the incomplete denominator remain open; no server or global percentage
> is emitted.

> Native player combat-audio server-client note (2026-08-28): revision
> `144226df3c7a81ae7e7b15866ae4091d610fffb8` adds no server authority. It
> consumes the existing authoritative Struck/Death/Revive/MountUpdate stream
> for exact player body/armour, mount, flinch, delayed death and revive cues;
> lethal hit order, delayed-cue cancellation, owner revive-alias deduplication
> and mounted-attacker numeric weapon identity are regression-covered. Native
> allowlist/package/verifier closure now binds 15 combat WAVs plus M79. Windows
> 367/367, rustfmt and script self-tests pass, and independent review found no
> P0/P1 inside this bounded claim. Web ActionFeed, Crystal-random tiger choice,
> authenticated live delivery/audio and all same-EXE/GPU/DPI/soak/human/
> signing/denominator gates remain open; no server or global percentage is
> emitted.

> Player combat-state server-client note (2026-08-28): revision
> `9eaa62283ec453bfa42f8bc3cbddb4c8811abf09` adds no server authority. It
> makes the clients preserve the Zone's existing health-before-death packet
> order, authoritative death pose and one death incarnation, and separates
> revive action/effect handling from later authoritative HP. Native/Web mounted
> Die/Dead/Revive now use ordinary player frames without a standing mount;
> PlayerRevive Magic2/M79 assets and Candidate required-file gates are bounded.
> Windows 360/360 and Web/package checks pass with no reviewed P0/P1 inside the
> claim. Crystal Struck ActionFeed queuing, Native generic struck/death audio,
> authenticated live delivery and all real-window/final gates remain open, so
> no server or global percentage is emitted.

> VIS-02 FlamingSword server-client note (2026-08-28): revision
> `160e8d3ccc0eb17f8e49b6505c5a58666a35029f` changes no simulation or shared-
> Zone combat authority. It closes only Gateway preservation and native/Web
> consumption of typed `ObjectAttack(spell=8)` as the source Attack1 overlay,
> exact M8-1 plus frame-1 swing, with ordinary attacks and lifecycle cleanup
> fail-closed. Windows 357/357, runtime 191/191, Bevy 393/393, focused 5/5,
> Gateway 1/1, Web/resource and package/verifier gates pass; review found no
> P0/P1. The fixture is projection-only and does not prove the live silent-
> toggle/next-valid-melee/single-consumption path. The broader backend matrix
> and same-EXE/live-WSS/GPU/DPI/soak/human/signing gates remain open. No server
> or global percentage is claimed.

> VIS-02 FireWall server-client note (2026-08-28): revision
> `f6f78f3eddb813897cf4ce4c6056183130ab7f35` changes no server combat
> authority. It closes only the bounded native projection of the source cast,
> exact M39-0/M39-1, and independently persistent `ObjectSpell` ground cells.
> Windows 351/351, Bevy native-ui 393/393, FireWall 5/5, Gateway projection,
> Web resource/type gates and package/verifier self-tests pass. The typed
> fixture represents the all-valid five-cell case and does not prove live
> timing; `cast=false` is a separate synthetic compatibility case. Shared Zone
> already schedules the cross after 500 ms with 2,000 ms damage cadence, but
> the full collision/duplicate, caster-lifecycle, expiry, oldest-group and AOI
> identity matrix remains unaccepted. FlamingSword and all same-EXE/live-WSS/
> GPU/DPI/soak/human/signing gates remain open. No server or global percentage
> is claimed.

> Windows VIS-00 baseline / VIS-01 and VIS-02 in-progress / VIS-03 bounded server-client note
> (2026-08-27): no new server
> completion percentage is claimed. The authoritative `ObjectPlayerInfo`
> identity, guild, normal/Transform body and equipment routes now survive the
> Gateway projection into the native renderer, and harvest/corpse packets
> drive Harvest then persistent Skeleton rather than being dropped. Focused
> Gateway 1/1 and runtime 191/191 tests pass. The first VIS-01 code increment
> additionally preserves real `ObjectMonster(image=10)` CannibalPlant through
> source-timed Hide/Show while leaving other Crystal Hide completion policies
> fail-closed. The next increment adds Scarecrow source `DrawEffects` frames
> `224..233` as a packed-atlas additive layer, sharing map guard-band depth and
> obeying the local Effect option. Commit `ef619b551` adds a 17-event exact
> typed Gateway/native fixture for six Bichon actors. Incremental monster
> packets now carry their sprite contract, preserve snapshot disposition or
> fail closed to neutral, and retain death location/direction/kind. Fifteen
> exact render checkpoints bind production frame-set hashes, Candidate atlas
> entries and a real `0.map` front-tile render state. Follow-up `434bb06e6`
> preserves later raw-snapshot relationship changes over retained packet data
> and closes all seven atlas pages by byte count, SHA-256, PNG decode and
> dimensions across runtime/test/package/verify. Focused latest-head tests pass.
> The first VIS-02 checkpoint preserves typed Lightning `cast` authority into a
> post-Spell-action, caster-attached native effect and one exact audio cue; no
> client projectile or impact is invented. Shared Zone six-tile Lightning
> scheduling remains the gameplay authority and was not changed. Fresh-source
> Windows tests pass 333/333 after the gate began generating its required
> keyed/additive map pack. This closes only bounded
> projection/action-loss and render-state defects. VIS-03 revision
> `448db4f72` is intentionally client-boundary-only: BigMap Teleport is disabled
> on non-current active maps, uses Crystal `Title/823` when disabled, and keeps
> legacy normal-frame fallback for controls without explicit disabled art.
> Full Bevy native-ui 393/393, Windows 333/333 and package/verifier self-tests
> pass; independent review found no P0/P1. It does not prove live WSS
> ordering or GPU pixels. Monster special rendering, effect/wing overlays,
> complete assets, the other first-slice skill effects, environment visuals and all real-window
> acceptance gates remain open.

> VIS-02 FireBall server-client note (2026-08-27): revision
> `d85d7368119053e6b2609316c4f5c76faaa298cb` changes no server combat
> authority. It makes the Windows projection follow Crystal's local
> `ObjectMagic` post-action missile while deduplicating the simulation's
> compatibility `ObjectProjectile`. The cast, 16-direction finite missile,
> target-bound impact, M31-0/1/2 audio and asset closure are automated;
> Gateway fixture 1/1, effects 59/59, Windows 340/340, Bevy native-ui 393/393,
> Web typecheck and offline resource/audio verification pass, and final
> independent review found no P0/P1. This is not whole-spell/server parity:
> an explicit target-dead input is still required to suppress impact for a
> corpse that remains in AOI, and FlamingSword, SoulFireBall, FireWall plus
> same-EXE/live-WSS/GPU/DPI/soak/human/signing gates remain open. No completion
> percentage is claimed.

> VIS-02 SoulFireBall server-client note (2026-08-28): revision
> `19991af6ddb289dc2fb22569849599caabf9195e` changes no server combat
> authority. It closes the bounded Windows presentation path for Crystal's
> audio-only start, 600 ms local missile, Direction16 finite target tracking,
> bound impact and M64-0/1/2 assets, and ignores the Rust compatibility
> `ObjectProjectile`. Native 346/346, Bevy native-ui 393/393, focused effects,
> Gateway event projection, Web resource/type gates and package/verifier
> self-tests pass. The projection fixture is not an authenticated transcript;
> production no-amulet `cast=false` emission remains absent. Shared-Zone
> damage delay, PvP, target/range/flight revalidation and item-commit atomicity
> remain open, as do target-dead impact suppression and all same-EXE/live-WSS/
> GPU/DPI/soak/human/signing gates. No server or global percentage is claimed.

> Windows verifiable vertical-slice server closeout (2026-08-26; packaged
> runtime source `b5c0ecb60`): Simulation `vertical_slice` passes 8/8,
> including the original Bichon quest 1-to-9 route, and `shared_zone` passes
> 195/195. `ordinary_candidate_loop` remains 2/2; Gateway fresh-account
> persistence 1/1; ordered Zone restore plus `zone_rpc` 21/21; clean-source
> assets 312/312; Web typecheck; and all 15 combat milestone cases also pass.
> The newcomer proof follows authoritative quest/drop/reward identities and
> ground-drop claim tickets without QA gold, damage multipliers, or direct HP
> mutation.
>
> The runtime now uses one avalanche-mixed deterministic accuracy roll across
> personal-session and native Zone physical attacks. This removes the prior
> modulo resonance that could make a Scarecrow permanently hit or missed for a
> fixed attacker/target pair. The agility-eight unit regression proves both
> outcomes across successive ticks; focused native Zone high-evasion and
> deterministic replay checks also pass. This closes that accuracy defect, not
> full Crystal server parity.
>
> Runtime source revision `b5c0ecb604946a858bf5d060a2cca306032c0e62` is
> bound to Windows Release EXE SHA-256
> `9E51CBF3E81D50A182F08CE11D02D9829268881A2124BAFC1D963829CC634E8C`
> (66,665,472 bytes) and build-attestation SHA-256
> `74F7D06336D486C6430263519282AED02C3B0429C6711FE0829DA7BE08311370`.
> Nonvisual Candidate `WN-CANDIDATE-03-20260826` passed clean-source
> verification (`sourceRepoCheck=checked`) and copied-artifact verification
> (`sourceRepoCheck=unavailable`). All six copied root anchors match. Its
> 10,254-payload manifest SHA-256 is
> `58F88AD84D1F7F9C9CC1CC44E59932D2A39136FD62FCA1F56CDAB0CF6C861884`
> with aggregate
> `6788698E6ED19209D5463B10FF15E5D7972D714C62C6D0093808571C97ABF83A`;
> the complete package has 10,258 files. Candidate-03 supersedes Candidate-02
> for current runtime evidence.
>
> Open acceptance gates remain Windows pure-UI execution, same-EXE live
> WebSocket continuity, real 125%/150% OS-DPI validation, a real 30-minute
> native-client soak, and human visual/feel review. The statement's detached
> CMS signer is an internal self-signed certificate, not a formal release
> certificate, and the EXE is Authenticode `NotSigned`; formal release signing
> remains open. Therefore this entry must not be read as global or strict
> Windows Candidate 100%.

> 2026-08-26 map-event E1 parity checkpoint: six current `_MAPCOORD` bindings
> retain exact script source provenance, typed `LEVEL`/`CHECKPKPOINT`
> thresholds, exact Hint failure text, `ENTERMAP`, and a one-to-one current
> `NeedMove` destination. Generator validation fails closed for duplicate,
> missing, ambiguous, and unsupported input. Generator 7/7, focused runtime
> 3/3, personal/shared integration 3/3, and the Gateway allowed-turn transfer
> regression 1/1 pass. The 18 general event scripts are still marked `open`;
> this checkpoint does not claim their execution, the complete six-gate
> Gateway matrix, live Crystal packet traces, door/gate/wall behavior, exact
> delayed ordering, or RNG parity.

> 2026-08-26 map-environment data parity checkpoint: binary import follows
> `MapInfo.Load/Save` order for `Music`, `Fire`/`FireDamage`,
> `Lightning`/`LightningDamage`, and `FireWallLimit`/`FireWallCount`. The real
> database currently yields 464 records (463 named maps and one empty source
> placeholder), 12 hazard maps, no enabled fire-wall
> limits, and no nonzero map-music values. Crystal runtime profiles construct
> hazard configuration from those records and expose the corresponding
> `MapInformation` bits. Exact LightningCave/MoltenRockCave fixtures and
> personal/shared hazard regressions pass; Package/Web manifests are
> byte-identical, their sync gate passes, and Web typecheck is green. The
> checkpoint does not claim exact
> `System.Random` trace equivalence, general delayed map actions, or
> door/gate/wall event semantics; those remain open P0 map-parity work.

> 2026-08-26 Slice D durable post-commit recovery is accepted for the bounded
> item-identity settlement slice. Lookup/transact use one advisory-lock domain;
> unknown database outcomes remain retryable; ground projection plus save is
> atomic; detached claim authority survives world-only checkpoints. Teardown and
> `Drop` consume finalized outcomes only. Ordered recovery runs after StartGame's
> Zone join and proves exactly one credit across a fresh factory/login boundary.
>
> Gates: Simulation 1472/0, shared_zone 195/0, Gateway 642/0/1 ignored out of
> 643, social_economy 3/0, Web typecheck 0, exact-file Rustfmt 0, Gate18 checks 0,
> diff check 0. Independent audit: GO, P0=0, P1=0, P2=1. The remaining P2 asks
> the PostgreSQL service itself to expose missing context as explicit Deferred;
> current routing already excludes that service call during teardown. This local
> closure must not be read as full Crystal server/game Accepted parity.

> 2026-08-25 GroundDrop identity Slice C complete: authoritative claim tickets
> now bind Zone key, object id, monotonic generation, claim id, canonical payload
> digest, session/owner identity, and the complete drop payload. Player and
> IntelligentCreature pickup both settle through the same account-inventory
> boundary; success commits the exact ticket and failure cancels/restores it.
> Legacy object-id-only follow-ups fail closed, reconnect/checkpoint restore
> validates pending tickets against presence plus the restored Zone claim, v1
> checkpoints rebuild unsigned authority fields, and exhausted u64 allocators
> reject instead of reusing ids. Independent P0/P1 review is closed. Locked
> serial evidence passes Simulation lib 1465/1465, shared_zone 193/193,
> Gateway lib 609 passed with 1 ignored, Web typecheck, exact-file Rustfmt, and
> diff checks. Slice D durable post-commit crash recovery remains the final
> item-identity settlement blocker; this entry does not pre-claim overall 100%.
> GroundDrop identity Slice A/B checkpoint (2026-08-25): exact recursive
> Crystal `UserItem` identity is lossless through drop creation, internal
> snapshots, checkpoints, Zone RPC, state roots, and current local/shared/quest
> pickup. The exact staged planner is shared by preflight and commit, preserves
> assigned UIDs or retires fully absorbed source UIDs, emits every changed
> stack, canonicalizes only unambiguous legacy items, fails closed on identity
> conflicts, and binds idempotency to the payload. External clients receive the
> redacted projection only. Locked serial gates pass Simulation 1461/1461,
> Gateway 606 passed with 1 ignored, Web typecheck, exact-file Rustfmt, and diff
> check. Slice C claim generation/identity and Slice D durable crash recovery
> remain open and prevent a full server-parity claim.

> Clean-checkout map-source correction (2026-08-25): active Bichon map `0`
> now uses the same world collision loader as CrystalWorld hosting, including
> the tracked gzipped map-pack fallback when no original client is installed.
> The former full-client-only branch degraded to `Starter Field` collision on
> Linux CI and erased the real fishing-cell surface. The exact active-map test
> passes 1/1 and the fishing suite passes 16/16. This closes that server loader
> regression without expanding the wider map or client-visual parity claim.

> Zone recovery checkpoint (2026-08-25): World Director factory restores now
> validate and stage every Zone before one atomic live-map replacement. The
> replica marker and autonomous-tick state is updated under a single fixed
> replicas -> Zones lock order for mark/promote/resume/resource creation and
> restore. Focused atomic gates pass 2/2 and replica gates pass 4/4, with Gateway
> compile green. This closes Zone-factory partial restore and standby tick races;
> durable checkpoint file publication remains a separate open security boundary.

> AI 41/42 Yin/Yang Devil Node checkpoint (2026-08-25): the runtime now preserves
> Crystal immobility and support-cast timing, requires a friendly target within seven
> tiles, and maps AI-specific `BlessedArmour/MaxAC` or
> `UltimateEnhancer/MaxDC` stats at `target level / 7 + 4`. The existing player Buff
> authority is used only when the node is genuinely friendly to the player. Monster
> target Buff state is not yet authoritative, so no synthetic `AddBuff` is emitted for
> it. Focused locked tests pass 2/2; complete monster Buff storage/expiry/snapshot
> semantics remain an open parity item.

> Map-coordinate event runtime checkpoint (2026-08-25): the six active Crystal
> bindings now run from authoritative personal-session and shared-Zone movement.
> The evaluator consumes live level and PK-point state, fails closed on unsupported
> conditions/actions, emits the imported denial hint, and exposes an `ENTERMAP`
> transfer only after its gate succeeds. The Gateway already completes authorized
> post-Zone transfers from that snapshot, so denied cells cannot bypass the gate.
> End-to-end tests pass 2/2 across all six bindings and exact level 49/50 and PK
> 199/200 boundaries. This is functional parity for the imported subset only;
> arbitrary event scheduling and full script-command semantics remain open.
>
> Unmatched Crystal spell correction (2026-08-25): the Rust manifest dispatcher
> now matches `HumanObject`'s default switch behavior instead of converting an
> unknown spell into a generic target hit. MP deduction remains before dispatch;
> the caster receives/broadcasts `cast=false`, with no CastTime-equivalent
> cooldown, magic progression, projectile, or damage action. Focused tests also
> lock FireBall's explicit successful path. This change is intentionally
> fail-closed: unsupported spell semantics remain in the parity backlog and are
> not represented by fabricated gameplay.
>
> AI 50 GreatFoxSpirit recall correction (2026-08-25): recall now evaluates the
> local authoritative equivalent of Crystal `FindAllTargets(30)` in deterministic
> ring/y/x order, skips dead, hidden, same-side, near, stale, or unsupported
> entities, checks MagicResist per candidate, and returns on the first successful
> teleport. `RemotePlayer` mirror transforms are never mutated; `SelfPlayer` and
> opposing monster objects preserve their real IDs across teleport packets.
> Monster MagicResist is still a documented zero fallback because the personal
> monster ECS has no imported resistance stat. Locked focused tests pass 2/2.
>
> AI 48 GuardianRock parity correction (2026-08-24): the queued pull movement
> now passes through Crystal's `Random(MagicResistWeight) < MagicResist`
> all-or-nothing resistance rule. Resistance does not cancel the delayed
> `ObjectRangeAttack` animation and does not introduce damage. A deterministic
> two-phase behavior regression locks the ordinary four-tile pull and the
> resisted zero-movement branch. This is a bounded AI correction, not closure
> of the remaining monster behavior matrix.
>
> AI 27 Khazard parity correction (2026-08-24): the runtime pull branch now
> checks the authoritative player's Crystal-numeric `MagicResist` against
> `Settings.MagicResistWeight` before moving the target, matching
> `Khazard.PullAttack`. The `ObjectRangeAttack` surface remains visible on a
> resist and the pull remains non-damaging. A deterministic two-phase behavior
> regression proves movement without resistance and zero movement with a
> successful resistance roll. This does not close the remaining AI matrix.
>
> NPC per-player/global visibility correction (2026-08-24): Rust now mirrors
> Crystal `NPCObject.CheckVisible` for `FlagNeeded`, level, and class, plus the
> minute schedule in `NPCObject.Process` for `DayofWeek` and `TimeVisible`.
> The existing persisted character NPC flags are read during StartGame and live
> AOI reconciliation. A flag transition produces the matching `ObjectNpc` or
> `ObjectRemove`; hidden NPCs cannot be interacted with and are not exposed in
> authoritative world snapshots. Server-local time follows Crystal's local
> clock and exact `start <= current < finish` rule, with a deterministic UTC
> fallback only where a target has no local-calendar API. Focused schedule and
> live-AOI regressions pass together with the locked serial Simulation compile.
> Map-coordinate events, complete NPC include semantics, and full server parity
> are still open.
>
> WN-CANDIDATE R12 server parity note (2026-08-23): ordinary `AcceptQuest` and
> `FinishQuest` can no longer mutate task state from anywhere on the map. The
> native path requires the exact current server-owned dialog link, correct
> active NPC, real NPC ECS identity, authoritative one-tile proximity, valid
> lifecycle stage, and (for the starter task) the configured proof item. The
> Web quest log enables Accept/Complete only under the exact matching link in
> the current server-owned dialog, which the server revalidates before
> mutation; the former no-dialog sentinel path
> is rejected. Opening the guide no longer changes task state; explicit actions emit the matching
> `ChangeQuest`/`CompleteQuest` surfaces and close the dialog only on success.
> Gateway world snapshots now echo a monotonic native typed-quest request id in the
> exact ACK/NACK for normal execution and capacity rejection. Native handling
> consumes every same-frame ACK once, rejects malformed envelopes, ignores a
> delayed old id for a replacement submission, filters stale connection
> generations, and gives retained unsent retries a new id after resume. Existing
> Crystal/Web `@quest:accept` / `@quest:finish` dialog targets remain accepted.
> Exceptional server paths close the socket rather than claiming a result.
> Native pickup transport
> reports a saturated bounded lane, leaves over-limit reliable commands queued,
> and retries retained UI intent, while Gateway
> tests prove `pickUp(objectId)` and `pickUpTile` map to distinct authoritative
> actions. The ordinary functional loop selects the explicit server-owned
> dialog targets and passes movement, combat, quest drop,
> ground-gold and object-id item pickup, exact reward, Bichon identity, save,
> and relog assertions (current Cargo result 2/2). Windows GUI, Gemini/human visual acceptance, deployed
> WebSocket behavior, and a fresh full Simulation suite remain unclaimed.
>
> Web artifact sync (2026-08-22): the current source has a fresh complete Web
> production build. Dual WASM runtime budget, map-atlas budget, TypeScript and
> 13/13 static pages pass under runtime `bevy-1813be587ef98bc1` and BUILD_ID
> `OXQE2c59Nd1B4bxoWcPQf`. This is client artifact evidence only and does not
> close the remaining server-environment gates: live PostgreSQL, deployed
> remote Zone and crash recovery. A strict local pre-seeded 64-client/30-minute
> Gateway soak passed on 2026-08-22, but does not prove a deployed environment.
>
> Aggregate shared-Zone authority correction (2026-08-21): authoritative
> monster disposition is now an explicit game-data/session/Zone/checkpoint
> field. Gateway projection refreshes `ObjectMonster` from the live native Zone
> snapshot and otherwise fails neutral; it does not manufacture hostility from
> an untrusted client or stale projection. Player targets use Zone PVP commands,
> while monster targets retain materialized combat transactions. Fresh
> mail/GameShop item trees receive recursively fresh IDs and storage split keeps
> its Crystal grid-scoped identity. Full evidence is Simulation 1,283/1,283,
> shared Zone 189/189, Gateway 529/0 with one environment-gated ignored test,
> plus a green default Gateway check. A focused authenticated Axum `/ws`
> GameShop buy/receipt/mail/claim black-box and its exactly-once reload neighbor
> now pass. Live PostgreSQL, deployed remote-Zone and crash-recovery E2E remain
> outside this proof.

> Native GameShop Sol-audit correction (2026-08-21): both generic transport and
> session paths now force `NativeGameShopPurchaseV2` through the V2-capable,
> one-endpoint executor. Ordinary `GameShopBuy` remains old-host compatible but
> is also one-attempt after endpoint selection; raw common-call economic Execute
> cannot bypass the rule, and non-economic commands retain normal fallback.
> Native pre-execution pending state clears only after its receipt is sent.
> Complete operation-4,097 tests cover Gold/Credit, finite global/individual
> stock, visible mail, packets, durable store and oldest-key replay, while player
> mail commands cannot expose, collect or delete the hidden ledger. Focused gates
> pass Simulation 8/8, typed RPC 12/12, native handler 7/7 and generic-session
> bypass 1/1. No Gateway full was run during concurrent `routing.rs` ownership.
> Local scoped status is P0=0/P1=0, not fresh independent acceptance; the hard
> 4,096 availability limit/hidden-mail carrier and missing real authenticated
> WS, live PostgreSQL, deployed remote-Zone and crash-recovery E2E remain P2.
> `typedGameShopOutcomeV1` is retained only as a legacy optional-outcome marker;
> Native V2 authority is `nativeGameShopPurchaseV2`.

> Native GameShop at-most-once parity note (2026-08-21): the opted-in JSON path
> now generates a trusted 256-bit operation key and uses a versioned
> `NativeGameShopPurchaseV2` Zone command. The authoritative transaction stores
> debit, stock, purchase mail and exact typed outcome together; duplicate keys
> replay the outcome with no normal mutation packets. Its hidden character
> ledger survives Gateway-session rollover and uses a deterministic union merge
> so stale A/B refresh/save cannot forget either key; conflicting keys and the
> hard 4,096-entry limit fail closed without eviction. Typed mutation Execute
> never crosses endpoints after send, rolling old hosts execute V2 zero times,
> and post-execution `CommitFailed`/response loss closes unknown with zero
> receipt. Ordinary non-opted-in old-host behavior is unchanged. Focused gates
> pass Simulation 6/6, Gateway handler 6/6, typed RPC 4/4 and old-host 2/2; a
> full Simulation snapshot passes 1,267/1,267. At that intermediate snapshot,
> Gateway full 513 was blocked by shared combat/routing failures followed by
> Windows `STATUS_STACK_BUFFER_OVERRUN`; the top aggregate correction supersedes
> this with the repaired 529/0/1 result. The 4,096 cap/hidden-mail carrier, real
> authenticated WebSocket path, live PostgreSQL and deployed remote Zone remain
> explicitly P2/unclaimed.

> Native reconnect Phase 1 P1 rework parity note (2026-08-21): no Crystal binary
> packet ID, Simulation authentication path or ordinary Web protocol changed.
> The native JSON capability path now read-only validates binding/identity,
> reserves the exact retained lease without consuming its credential, prepares
> route refresh and Zone registration, and only then atomically commits family
> consumption plus lease transfer. RAII rollback preserves token/session/permits
> after injected route or Zone failure; replay and concurrent commit remain
> single-winner. Credentials fail construction unless exact 43-character
> unpadded base64url decoding to 32 bytes. Production defaults cap
> WS/active/reconnect counts at 2,048/512/512, frames/messages at 64 KiB and the
> 256-entry socket input queue at an enforced 16 MiB byte budget. Native resume
> is 14/14, registry 6/6, and full Gateway lib 490 passed / 0 failed / 1 existing
> database test ignored. Windows-client/live reconnect and cross-process state
> remain open; source nonce is metadata, not asserted device binding.

> Latest mail durability closure (2026-08-21): GameShop delivery, cross/self
> `SendMail`, and ordinary `ClientPacket::CollectParcel` are
> durable-before-success. Mail has a persisted 128-bit delivery identity;
> equal-content sends do not collapse, repeat refresh is idempotent, incoming
> ID collisions re-key only the not-yet-visible external mail, and local
> reversible lock state wins. Claim preflight reuses the authoritative exact
> item core in a staged CharacterSave and commits before GainedGold/GainedItem/
> ParcelCollected(1); failure returns ParcelCollected(-1) with World/store/File
> unchanged. Anonymous active saves no longer write `demo`. Legacy identity
> ignores mutable status and claim-cleared payload, safely merging ambiguous
> same-ID/same-header history to prevent double collection. Simulation lib
> 1,234/1,234, legacy focused 3/3, mail 28/28, social-economy 3/3, security lifecycle 18/18 and
> simulation check pass. Live PostgreSQL and distributed cross-backend 2PC are
> still not claimed.

> Native Windows social client boundary (2026-08-21): the Windows adapter now
> consumes the existing ordinary Group/Guild/Trade packet surface using typed,
> bounded commands and read models, with fail-closed pending handling. No
> server protocol changes were made in this slice; TradeGold/TradeConfirm
> sender correlation remains a known protocol limitation. See
> `docs/generated/player-qa/native-social/WN-SOCIAL-01-REPORT.md`.

> Latest SendMail server-transaction parity note (2026-08-21): remote-character
> delivery no longer persists the recipient before debiting the sender. The
> server stages the active sender save, exact unique-ID attachment removal,
> gold/postage debit and recipient mailbox append in one account-store unit;
> persistence must succeed before the shared store, live World or `MailSent(1)`
> changes. File uses atomic replacement and PostgreSQL source mode uses one
> transaction with optimistic version checks across the touched accounts.
> Self-mail is also all-or-nothing, and stale online recipient saves merge the
> durable mailbox before persisting. Simulation lib 1,220/1,220, mail 21/21,
> social-economy 3/3 and security lifecycle 18/18 pass. The PostgreSQL rollback
> regression auto-skipped because no DB service was reachable, so live-DB
> execution remains an environment gate. File+PostgreSQL mirror mode performs
> synchronous mirror-first write plus compensation on File failure, but is not
> distributed 2PC and can temporarily diverge if the process dies between
> backend commits.

> Correction closed (2026-08-21): the unauthenticated mail/save fallbacks and
> ordinary-player CollectParcel durability gap are repaired and covered by the
> latest gates above. Strict transfer parsing and real-TCP-peer safety are
> tracked by their separate named closures; fresh independent review remains
> required for Candidate promotion.

> Latest GameShop/mail security parity note: 2026-08-21 adds the dedicated
> ordinary-player GameShop packet and makes product/class/payment/price/
> balance/mail/attachment checks authoritative before mutation. Purchases use
> exact StackSize-bounded ItemState attachments and the Crystal mailbox hint;
> a corrupt exact attachment causes a repeatable all-or-nothing claim failure.
> Player-command safety is fail-closed even when deployment environment
> variables are absent; only explicit loopback dev/test can opt out, never
> production/staging. Independent full gates pass Simulation 1,206/1,206 and
> Gateway 461 passed / 1 ignored. Persistent finite stock and a correlated
> GameShop purchase-result packet remain open and are not claimed here.

> Latest NPC teleport atomicity note: 2026-08-21 additionally sources the
> advertised and charged fee from the same runtime-discovered Crystal
> `Setup.ini`. Missing/invalid cost disables World Map rather than accepting a
> client value. Gateway checkpoint failure is tested before dispatch and rolls
> the single-writer Zone back without observer half-packets; rollback restores
> AOI/occupancy, and success clears queued pre-teleport movement. Full gates are
> Simulation 1,194/1,194 and Gateway 456 passed / 1 ignored. This remains a
> guarded implementation only: real `WorldMap.ini` is disabled and has no
> eligible teleport NPC.

> Latest shared-Zone NPC teleport parity note: 2026-08-21 adds the guarded
> successful path anticipated below. The client sends only `objectId`; Gateway
> reads authoritative gold and the Zone derives destination/cost from runtime-
> loaded `WorldMap.ini` plus imported map/NPC data. A retained same-map NPC,
> eligibility, walkable free destination and sufficient gold are mandatory.
> Success updates single-writer Zone occupancy/AOI, commits the personal
> checkpoint, emits the Crystal-facing gold/map/location packets, and saves the
> authoritative transform for relogin. Failure emits nothing and preserves
> currency/transform. This does not enable unavailable content: the actual
> Crystal file remains `Enabled=False` and imported `CanTeleportTo` count is
> zero, so live production requests still reject safely.

> Latest Big Map packet parity note: 2026-08-21 replaces the incorrect
> `RequestMapInfo -> MapInformation` behavior with Crystal
> `WorldMapSetup/NewMapInfo`, adds authoritative bounded `SearchMapResult`, and
> exposes exact Gateway command mappings. Setup/map de-duplication is scoped to
> the active in-world connection and resets on logout. Current source data has
> `WorldMap Enabled=False` and no `CanTeleportTo` NPCs; consequently
> `TeleportToNpc` intentionally emits no fabricated success and changes no
> transform or currency. A later enabled-world implementation must route
> teleport through the shared Zone and revalidate destination, cost, occupancy
> and save state under one frontier-led writer.

> Latest native-client parity evidence: 2026-08-19 revalidates, without changing
> server code, that the production Gateway/Simulation path is consumable by a
> non-Web frontend. A fresh native account completed q1/q2 through real NPC,
> Walk, Attack, death/revive, quest and reconnect packets. The q2 GingerTea path
> is explicitly the Crystal `Q` contract: on the rewarded Scarecrow kill the
> eligible player receives `GainedItem(item_index=1112)` directly in the quest
> container; ordinary ground objects continue to use ObjectItem/PickUp. Turn-in
> and a second login confirmed exact reward and persistence behavior. This note
> adds end-to-end evidence only; it neither expands the accepted server matrix
> nor converts the still-open frontend visual/feel gate into Accepted.

> Latest level-50 skill parity note: 2026-08-18 verifies all 63 active
> Warrior/Wizard/Taoist books with required level <=50 across imported source
> data, protocol mapping, personal runtime, shared-Zone authority, Gateway
> route, profile availability, book acquisition and original visual routing.
> The implementation includes Crystal-specific reagent consumption, stat
> scaling, melee geometry and delayed hits, movement/control, AoE/ground
> lifetimes, buffs/debuffs, poison, healing/reincarnation and summons. Direct
> FireBang/IceStorm retains its existing immediate target contract; their
> object-id-zero form uses authoritative ground resolution. Zone-native Buffs
> notify their owner on expiry, while Buffs mirrored from the personal runtime
> remain observer-only at Zone expiry to avoid duplicate owner packets. The
> audit deliberately excludes the commented-out FastMove initializer and
> classifies SlashingBurst/IceThrust at level 53 as adjacent, not <=50. These
> are deterministic server/client implementation gates; real learned-book
> browser casts and human timing/feel remain separate acceptance evidence.

> Latest StartGame transform parity note: 2026-08-15 distinguishes a real map
> boundary from the bounded collision window used by starter rendering. Loaded
> characters keep valid Crystal full-map coordinates even when those coordinates
> are outside the starter window, while an impossible coordinate for the saved
> map still recovers to the configured bind map. The regression is locked by
> both preservation and legacy-recovery tests; full Simulation and Gateway
> library suites pass. No client teleport, QA command, or production packet
> surface was added.

> Latest production-login observability note: 2026-08-12 changes only the
> non-Crystal operator surface. OnConnect now has dedicated inflight,
> request/error, and latency metrics; World Director exposes durable checkpoint
> write health, last-success time, file size, and embedded Zone-factory size;
> and Gate 12 alerts on stalls, failures, staleness, journal growth, and bounded
> size thresholds. Successful/rejected OnConnect, checkpoint write/restore, and
> forced write-failure regressions pass, together with Gateway 438/438
> non-ignored unit tests, Gate 11 workload 2/2, Home Tunnel 4/4, Zone RPC 29/29,
> Rust fmt/diff checks, and Prometheus 3.5 `promtool` validation of all 17 rules.
> Crystal TCP/WebSocket packets, login authorization, Zone authority, gameplay,
> and persistence semantics are unchanged; this does not substitute for an
> authenticated production-player or long-soak acceptance run.

> Latest map-environment parity note: 2026-08-01 preserves Crystal's separation
> between server `TimeOfDay` and per-map environment metadata. `Light=Normal`
> follows global Dawn/Day/Evening/Night, nonzero map Light overrides it, and the
> original `MapDarkLight`/`WeatherParticles` bytes now survive generated data,
> Simulation packets, and Gateway browser adaptation. Developer-only Day is an
> explicit local environment default; production Release behavior remains the
> UTC formula unless an operator intentionally configures an override.

> Latest Zone replica parity note: 2026-07-29 makes v5 base restore preserve
> the installed authoritative shared-Zone image exactly. A standby Session now
> rebuilds only its local movement binding and sequence cursor after restore;
> it does not resynchronize reconstructed static entities into the installed
> Zone checkpoint. This removes the observed Royal_Archer light drift while
> preserving the Crystal Session snapshot, movement ingress, and post-base
> mutation order. The byte-stability regression and the complete Gateway
> integration matrix pass, including Zone RPC 28/28.

> Latest deployment-observability note: 2026-07-25 extends only the
> non-Crystal HTTP health surface. When `MIR2_DEPLOY_REVISION` is configured,
> Gateway `/health` reports the running revision so the protected shared
> acceptance server can prove that its live process matches Git HEAD. The field
> is omitted in ordinary local/Crystal-compatible operation, and TCP,
> WebSocket, packet, session, and gameplay behavior are unchanged. Focused
> configured/unset/blank revision tests pass.

> Latest collision-cache parity note: 2026-07-25 changes ownership, not game
> rules. Parsed full-map and world-map Crystal collision records are immutable
> and now shared through `Arc` from their process caches, eliminating repeated
> deep copies during StartGame respawn projection and map queries. Owned
> Zone/ECS resources still receive their own value at the existing three
> installation boundaries. Full-world Bichon bounds, visible-object bootstrap,
> spread density, and representative Crystal spawn placement remain green in
> four focused regressions; the live browser entered BichonProvince without a
> collision correction or console error.

> Latest remote-Zone failover parity note: 2026-07-23 completes Gate 11.1-11.4.
> Zone RPC v5/checkpoint v4 now restores the private durable session projection
> and complete shared map image, including player vitals, autonomous monster/AI
> timers, pending combat/effects, public drops/claims, doors, hazards, map
> layers, trades/rentals, and NPC state. Real Crystal combat/drop/cross-map
> handoff survives takeover, and four sessions across two maps survive two
> consecutive fenced host failures. Release evidence is emitted as one
> fail-closed JSON manifest; multi-AZ RTO remains a deployment measurement.

> Latest timed system-chat server parity note: 2026-07-23 models Crystal's
> process-wide `Online Players` and `LineMessage` cadence in Gateway rather than
> synthesizing fixed browser text. TCP and WebSocket players share one online
> count, register only after StartGame reaches an active Zone, and unregister on
> every world/socket exit path. The scheduler emits Hint at five-minute and
> LineMessage at ten-minute production intervals, reads the original line file
> when present, and uses deterministic tests for ordering and lifecycle. QA-only
> interval/fixed-line/packet-limit overrides fail closed without a configured
> control token. Five focused tests and the full Gateway 307/307 regression pass.
> This is presentation/system-message
> parity only; it does not bypass the existing authenticated Session and shared
> Zone command boundaries.

> Latest monster defence-type parity note: 2026-07-22 follows the original
> ShamanZombie and WaterDragon server classes instead of inferring every defence
> channel from range. AI 26 uses `MACAgility` even beside the player; AI 181 uses
> AC for its adjacent DC strike and MAC plus MagicResist for its ranged MC
> strike. Rust now has a deterministic authoritative Min/Max MAC roll alongside
> AC, and a regression gives the player more than 10,000 physical AC to prove a
> ranged Hydra hit is not accidentally reduced by that stat while Green poison
> still applies. Core verification is green at 1,126 Simulation unit tests,
> shared Zone 154/154, all package integrations and vertical slice 8/8. This is
> not a claim that every Crystal monster defence type is imported: the remaining
> AI-specific table is explicit parity debt and must be source-verified.

> Latest original beginner-route server parity note: 2026-07-18 closes the
> Crystal q1-q9 fresh-Warrior path through level 6. The imported quest chain
> now preserves the original NPC hand-offs, task quantities, prerequisites,
> hand-in XP/gold, fixed rewards, q3/q6 mandatory choices, Scarecrow
> `GingerTea` Q drops, Deer corpse-harvest `DeerMeat` Q drops, and equipment
> template stats. Player melee, skills, and player-owned poison feed one
> authoritative defeat path that grants Crystal monster EXP and quest credit;
> environmental/NPC deaths do neither. A release vertical slice finishes all
> nine quests with exact rewards and reaches level 6 naturally, while focused
> regressions cover ownership, drops, harvest, combat stats, Gateway reward
> projection, and Web parsing. This supersedes the older q1-q4/q5-unlocked
> note below; remaining work is client-side dialog/route feel acceptance and
> later quest bands, not q1-q9 server progression.

> Latest NPC name-colour server parity note: 2026-07-18 aligns every ordinary
> NPC spawn projection with Crystal `Server.MirObjects.NPCObject`, which sets
> `NameColour = Color.Lime` and copies it into `S.ObjectNPC`. The Rust initial
> manifest spawn and snapshot were already Lime, but the later visible-object
> bundle was White. One shared `0xFF00FF00` constant now covers all three, and a
> focused transfer regression rejects any mixed-color packet sequence. Crystal
> client behavior remains source-accurate: the first underscore-delimited name
> line uses the packet colour and subsequent lines use White. Focused transfer
> 3/3, bootstrap 1/1, shared Zone 153/153, fmt, Release build, and live r04 pass.

> Latest moving-object AOI parity note: 2026-07-13 keeps retained monsters and
> summons in the spatial cell matching their latest authoritative packet
> position. Previously the object record moved while `object_grid` stayed at
> the spawn cell, allowing false visibility removal or a missed reappearance.
> The Zone regression moves a monster out of view, moves the observer beside
> its new position, and verifies ObjectMonster re-entry there. Full shared Zone
> coverage passes 153/153. This is server visibility parity only; frontend
> attack rendering is tracked separately.

> Latest entity-light server parity note: 2026-07-12 closes loss of the Crystal
> `light` byte between authoritative entity snapshots and AOI object packets.
> Shared player/monster entities retain incoming light, generated monster spawn
> packets resolve the Crystal template light, and the normal spawn projection
> emits `entity.light` instead of zero. This does not move rendering into the
> server and does not claim frontend light-compositor parity. Simulation/
> Gateway check, shared Zone 152/152, and focused snapshot plus routing tests
> pass; r12/r16 browser evidence receives the resulting object-light data.

> Latest mounted movement server parity note: 2026-07-12 closes the previously
> open three-cell mounted/Swift Feet path. Session-side mount, sneak, and buff
> transitions now update shared Zone state before movement intents; PauseBuff is
> retained, so paused Swift Feet cannot incorrectly extend Run. Zone validates
> all cells of a three-cell path, keeps Crystal's 600ms server MoveDelay, and
> sends correction only on failed authority checks.
>
> Real Release proof
> `docs/generated/player-qa/movement-jitter/movement-mounted-walk8-run3-webgpu-20260712-r6.json`
> sends one keyboard Walk and one keyboard Run after equipping and using a real
> RedTiger. Owner ACKs land at one and three cells in 18/22ms, with zero
> corrections/degradations and final delta `(4,0)`. Gateway integration and the
> full shared Zone suite (152/152) pass. Mounted and Swift Feet movement are no
> longer listed as open server gaps; broader command actorization still is.

> Latest Zone cadence/live-outbound server parity note: 2026-07-12 closes the
> per-socket world-clock and observer-mailbox gaps without weakening Zone
> authority. Walk/Run/Turn remain intents under Zone collision, occupancy,
> degradation, AOI, correction, and persisted transforms. The same per-Zone
> owner now executes one monotonic 300ms global cadence for pending movement,
> combat/projectiles, summons, monster AI, doors, hazards, buffs, and expiry;
> personal Session ticks no longer multiply shared time. Realtime owner/AOI
> movement, player appearance, and removal packets use a bounded token-fenced
> socket channel, with reliable mailbox fallback on pressure/closure.
>
> Strict Release evidence
> `docs/generated/player-qa/two-client-zone/two-client-zone-zone-owned-cadence-tick5000-release-20260712.json`
> passes with personal Tick forced to 5000ms and no observer pulse: B receives
> movement in 12ms, both clients expose 16 entities, Bevy records one remote
> event and 29 packed-offset matches, and decode errors, queue drops, console
> errors, and 404s remain zero. Focused cadence, blocked-runtime, fencing,
> fallback, and delayed combat regressions plus shared Zone 148/148 pass.
> Remaining server architecture work is routing all non-movement shared commands
> and durable side effects through fully fenced actor ownership; mounted and
> true three-cell sprint semantics also remain open.

> Latest dynamic TimeOfDay server parity note: 2026-07-09 supersedes the
> earlier fixed-Day bootstrap. Crystal `Envir.Now` is seeded from
> `DateTime.UtcNow`, and `AdjustLights()` maps `Now.Hour * 2 % 24` to
> Dawn/Day/Evening/Night before broadcasting `S.TimeOfDay`. Simulation
> StartGame and `WorldSnapshot.lightSetting` now use the same UTC-hour formula,
> while Web applies `snapshot.lightSetting` and exposes it through
> `window.__mir2Stage5.state` for QA evidence. Verification passed Rust fmt,
> focused Simulation/Gateway tests, Gateway check/build in an isolated target
> dir, Web TypeScript, and scoped diff checks. Live evidence
> `docs/generated/player-qa/visual-parity/light-setting-snapshot-20260709/`
> records direct WS `TimeOfDay.lights=4`, `worldSnapshot.lightSetting=4`, and
> browser state `lightSetting=4` with 0 critical console errors and 0
> non-favicon 404s. Production clients still cannot send raw QA/debug
> commands; remaining work is frontend scene-light rendering, not server light
> propagation.

> Latest movement ACK server parity note: 2026-07-12 unifies the Web early-ACK
> path and movement-controller reconciliation on the same
> `classifyMovementAckOutcome` classifier. A requested Run's one-tile first-cell
> `UserLocation` ACK is now `confirmed`, not `correction`, matching the Zone
> semantic that an originally stationary Run degrades to a one-tile Walk.
> Simulation `shared_zone` passed 148/148. Release raw `packetSequence` evidence
> `docs/generated/player-qa/movement-jitter/movement-protocol-expired-run-degrades-release-202607120745.json`
> records Walk followed by an expired Run with ACKs at `16ms/99ms`,
> `degradedRunCount=1`, `correctionCount=0`, and final delta `(2,0)`. Release
> normal UI Walk -> Run evidence
> `docs/generated/player-qa/movement-jitter/movement-normal-walk-run-chain-release-202607120750.json`
> records ACKs at `22ms/28ms`, command-to-pose latency `17ms/1ms`,
> `degradedRunCount=0`, `correctionCount=0`, and final delta `(3,0)`. Remaining
> architecture risk is private `SimulationSession` heavy `Tick` work serialized
> with movement ingress on the same WebSocket task; Release mode reduces but
> does not eliminate that risk. The next step remains a Gateway-owned
> single-writer Zone ingress/loop, and this architecture is not yet complete.

> Previous QA evidence/TimeOfDay server parity note: 2026-07-09 keeps production
> command safety intact while fixing the local evidence lane. Token-gated
> `qa.applyNativeState` now synchronizes the shared Zone authoritative
> transform after applying native character state, and the browser capture gate
> requires the expected `mapFileName` plus `position.x/y` before accepting the
> state. Focused Gateway regression
> `shared_in_process_registry_qa_apply_native_state_syncs_zone_transform`
> passed, and 0056 evidence proves Web `player` and `authoritativePlayer` both
> at `334,263`. That earlier StartGame pass stopped forcing Night
> `TimeOfDay` for the Bichon parity lane by emitting Day (`lights=2`),
> matching Crystal's `Prguse/2093` MiniMap light icon in 0057; the later
> dynamic TimeOfDay pass above now supersedes fixed Day.

> Latest HUD weight server parity note: 2026-07-09 keeps the production command
> boundary unchanged while correcting the Web-facing snapshot used by the HUD.
> `WorldSnapshot.maxWeight` now uses Crystal player `BagWeight` stats instead
> of a fixed `100`, and the token-gated native-state regression asserts the
> level-6 Warrior case as `current_weight=1`, `max_weight=62`. Live evidence
> `docs/generated/player-qa/visual-parity/crystal-web-pack-20260709-0027-hud-weight-diagnostics/`
> then shows the same native character at `currentWeight=14`, `maxWeight=62`,
> HUD `48 / 38`, and gold `3457` with 0 network 404s and 0 critical console
> errors. Production clients still cannot send raw QA/debug commands.

> Latest native-state evidence-control/max-MP/EXP server parity note: 2026-07-09 keeps the
> production command boundary intact while allowing fair local Crystal/Web
> comparison. Native Crystal account state can now be converted into a Web save
> plus a token-gated QA payload, and `qa.applyNativeState` applies that payload
> to the active session without allowing ordinary clients to send raw
> `Stage5Command` or debug movement. Gateway now emits fresh snapshots for
> bootstrap/state packet responses so Web automation observes the applied state,
> and `WorldSnapshot` carries `player_max_mp` through to Web `playerMaxMp`.
> Crystal `ExpList.ini` is used by the Web account sync so native EXP `435/900`
> also reaches the Web HUD as `48.33%`.
> Evidence
> `docs/generated/player-qa/visual-parity/crystal-web-pack-20260709-0025-exp-debug/`
> is runtime-clean with native HP/MP/EXP/gold/inventory/belt/equipment aligned
> and 0 network 404s / 0 critical console errors. Remaining parity risk for
> this lane is now visual/frontend: HUD asset/layout, chat content/state,
> minimap crop/color, and world scene frame/object mismatch.

> Latest QA-control server parity note: 2026-07-08 introduces a safe local
> automation path without weakening production command safety. `qaControl`
> requires `MIR2_GATEWAY_QA_CONTROL_TOKEN`, and ordinary clients still cannot
> send debug transfer or Stage5 commands. Focused gateway tests passed, and
> live Rust `7111` evidence
> `docs/generated/player-qa/combat-survival-default-selfcamera-rust7111-qacontrol2-20260708/report.md`
> passed real-client incoming damage plus `townRevive`. Remaining parity risk:
> QA-control side effects need explicit ACK/settle evidence, normal
> attack-kill/XP/drop are still red, and the damage-floater/Monster metadata
> frontend gaps remain visible.

> Latest hostile-retaliation server parity note: 2026-07-08 real Web-client
> Rust `7111` evidence now proves the incoming-damage chain. The updated
> combat-survival attack-trace run records target map/object id, attack frames,
> approach, delayed server combat packets, and retry attempts. Report
> `docs/generated/player-qa/combat-survival-default-selfcamera-rust7111-survivalattacktrace5-20260708/report.md`
> reached melee with natural `ForestYeti` object `258949`, observed target
> `ObjectAttack`, `ObjectStruck`, and `DamageIndicator` packets, and reduced
> player HP `18 -> 3`. Remaining server parity risk is now control/evidence
> stability rather than missing retaliation itself: `transferMap` sends but does
> not authoritatively move the player, `event.spawn RakingCat0` returns no
> visible hostile, death/revive is unstable beside live hostile AI, and
> attack-kill/XP/drop still need a normal same-scene acceptance rerun.

> Latest combat-survival server parity note: 2026-07-08 targeted Rust `7111`
> evidence keeps shared pickup and death/revive green after harness route
> hardening. Verification passed `node --check` for `qa-combat-survival.mjs`,
> focused Gateway tests for hostile passive-template AI override and Stage5
> `event.spawn` Zone synchronization, and live reports
> `docs/generated/player-qa/combat-survival-default-selfcamera-rust7111-pickupwait5s-20260708/report.md`
> plus
> `docs/generated/player-qa/combat-survival-default-selfcamera-rust7111-survivaltick-20260708/report.md`.
> Remaining server parity risk is now narrower but still open: RakingCat0 /
> ForestYeti retaliation has not produced accepted player-damage evidence from
> a stable real-client adjacent attack sequence, and full kill/XP/drop evidence
> still needs a normal-window rerun.

> Latest shared Zone/session item parity sync: 2026-07-08 fixes the current
> Rust `7111` pickup/death lifecycle split exposed by browser QA. Shared
> Gateway pickup now syncs current personal-session drops into the Zone before
> claim, and session fallback for pickup/drop/gold-drop is forced to the latest
> Zone authoritative transform so item actions no longer depend on stale local
> session coordinates. GM chat commands such as `@DIE` route to the personal
> session while normal chat continues through shared Zone broadcast. Verification
> passed focused Gateway regressions for Zone-authoritative packet-drop pickup
> and GM chat command routing, plus live evidence
> `docs/generated/player-qa/combat-survival-default-selfcamera-rust7111-authpickupseed7-20260708/report.md`:
> deterministic pickup produced `GainedItem x1` and death/revive restored
> `playerHp 0 -> 18` at town. Remaining server parity risk: monster retaliation
> and unseeded kill/XP/drop progression still need green same-scene evidence.

> Latest Rust combat server parity sync: 2026-07-07 moves real-client melee
> from "packet observed but no outcome" to "damage outcome verified." The
> Gateway test
> `shared_in_process_runtime_level_one_field_melee_resolves_damage_on_tick`
> now proves a level-1 field attack emits `ObjectAttack` immediately and
> resolves `ObjectStruck` / `DamageIndicator` on tick. Live Web evidence
> `docs/generated/player-qa/combat-survival-default-selfcamera-rust7111-floaterfix30s-20260707/report.md`
> confirms the same route through Rust `7111`: server damage indicators arrive
> and target HP falls. Remaining server parity risk: fresh-character kill pace
> does not yet produce `ObjectDied`/XP/drop evidence in the accepted window,
> and normal-client death/revive state still needs lifecycle repair.

> Latest held/chorded movement server parity sync: 2026-07-07 closes the
> forced WebGL2 long-run rollback caused by starter-demo transfer leakage in
> full Crystal world runtime. `with_crystal_world_runtime()` no longer keeps
> the hand-authored `starter-east-field-gate`; generated Crystal movement
> records are the authoritative travel source. Before fix, held Shift+Right
> hit `0:339,270`, batched transfer/reset packets, delayed ACKs by
> `7481/4066ms`, and produced a rollback warning in
> `docs/generated/player-qa/movement-jitter/web-motion-heldrun-bichon-right-webgl2-movelog-20260707.json`.
> After fix,
> `docs/generated/player-qa/movement-jitter/web-motion-heldrun-bichon-right-worldtransferfix-20260707.json`
> passed with `ok=true`, 8/8 ACKs at
> `359/152/200/247/91/57/92/146ms`, no `MapInformation` reset, no logical
> rollback, no ACK warnings, and Bevy WebGL2 packed/no DOM fallback. The
> chorded cardinal keyboard rerun
> `docs/generated/player-qa/movement-jitter/web-motion-keyseq-bichon-cardinal-worldtransferfix-rerun-20260707.json`
> also passed with all eight expected movement ACKs. Remaining parity risk:
> native Crystal frame cadence and animation timing still need side-by-side
> scoring now that this server rollback class is closed.

> Latest Bichon movement ACK parity sync: 2026-07-07 closes the crowded-town
> click-route repro under clean interaction diagnostics. Shared Zone movement
> now ACKs a Walk/Run that arrives after the player is ready immediately in the
> command response, and Gateway keeps the WebSocket task free for one Crystal
> run-grace window plus one Crystal tick (1.5s) after movement ACKs. This keeps
> heavy world ticks from delaying the next chained Crystal input while retaining
> the 75ms runtime tick wake for movement packets. Evidence
> `docs/generated/player-qa/movement-jitter/web-motion-clickroute-bichon-leftclean-postgrace1500-20260707.json`
> passed with `ok=true`, 4/4 ACKs at `490/164/33/5ms`, no entity-hit or
> non-movement pollution, and Bevy WebGL2 packed/no DOM fallback. Remaining
> parity risk: longer manual held/chorded movement and animation cadence still
> need human-feel acceptance.

> Latest Gateway movement ACK/input-priority parity sync: 2026-07-06 brings the
> local shared-Zone movement cadence closer to Crystal by keeping the WebSocket
> task available for chained movement immediately after a `UserLocation` ACK.
> A heavy background `WorldCommand::Tick` could previously win that race and
> delay follow-up Walk/Run input by about 2.5s, producing one-tile run
> degradation and visible stop/go. Shared in-process Zone runtime now consumes
> ready `TickPlayerMovement` before heavy ticks and yields heavy world ticks
> while pending movement or the 1.2s post-ACK Crystal input window is active;
> Gateway still wakes movement input at 75ms. Verification passed focused
> Gateway/simulation regressions, Gateway build, raw packetRun latency probing,
> and full Web click capture
> `docs/generated/player-qa/startgame-debug-20260706-213036/current-web-jitter-r2-gateway-postackgrace1200-click.json`
> with `ok=true`, no logical rollback, and a settled Bevy WebGL2 scene.
> Remaining parity risk: broaden long held/chorded movement sampling in
> crowded AOI before treating the local feel as fully human-accepted.

> Latest player/monster state parity sync: 2026-05-27 makes lethal player
> damage authoritative instead of clamping to 1 HP. Player damage now updates
> ECS vitals and `PlayerRuntimeResource` together, emits health/death packets,
> and exposes self death through world snapshots. Dead players can no longer
> move, attack, cast, harvest, or use normal consumables; resurrection-scroll
> revival restores vitals and movement. MP spend and generic skill healing now
> keep runtime vitals aligned with entity vitals. Status-effect parity is now
> gameplay-visible: paralysis/frozen/dazed/stun stop movement and actions,
> blindness blocks attacks/magic, slow lengthens move/attack cadence, green
> poison and bleeding tick HP, and red poison increases incoming damage.
> Monster death coverage now locks ObjectHealth 0/ObjectDied, non-blocking dead
> bodies, no repeat kill, respawn reset, and summoned-totem despawn behavior.
> Focused tests and TypeScript/gateway checks passed; the wider simulation
> suite still has unrelated Skill preflight/effect and account-store failures
> to close before full-suite green.

> Latest NPC input and skill preflight parity sync: 2026-05-27 brings two
> Crystal gameplay guardrails into the shared runtime baseline. NPC input
> confirmation packets now route to active input labels with NPC id validation,
> NPC script diagnostics are part of the debug world snapshot, and the command
> coverage report explicitly separates implemented, simplified, and missing
> command buckets. Skill casting now advertises cast kind/offensive metadata
> and runs Crystal-style preflight before spending MP or committing cooldown
> timing: missing/dead/out-of-range/non-hostile targets, LOS, safe-zone,
> required items, map restrictions, and passive skills fail without resource
> loss. Verification passed focused NPC packet/service-distance regressions,
> dynamic visibility TODO coverage, and focused skill preflight regressions.
> Remaining parity risk: this is still preflight/state safety; full Zone-native
> skill damage/Buff/lifetime coverage and durable NPC/economy commits remain
> broader work.

> Latest production movement input-buffer parity sync: 2026-05-26 closes the
> server-side piece of the live `walk -> run -> reverse` rollback/drift repro.
> The shared Zone now treats movement packets as timestamped intents, preserves
> the active pending action plus newest follow-up, consumes ready movement
> before accepting replacement input, buffers near-ready follow-up actions for
> Crystal cadence, and keeps Run authoritative when the packet arrived during
> run grace even if a later tick consumes it. Gateway now grants active
> movement packets a 900ms runtime-tick input grace so background personal
> session ticks do not delay ACK processing. UCloud release
> `20260526T1918CST-move-input-buffer` is active and passed public health plus
> WSS smoke. Headed production evidence paired with Web deployment
> `dpl_HttHWiP21hufr1d3mm6fMsHNwcmW` shows ordered
> `walk Right -> run Right -> walk Left` ACKs at `251/51/50ms`, and the 180ms
> stress pass at `73/54/55ms`, both settling at `332,270 Left` with no
> rollback or residual movement queues. Remaining server parity risk: long
> held/chorded movement under crowded AOI still needs human-feel acceptance
> after this deterministic repro is closed.

> Latest ZoneOwner handoff parity sync: 2026-05-26 adds tested hosted-owner
> runtime takeover. `HostedZoneOwnerCommandClient` can now export its owned
> runtime once for handoff, rejects old host reads after export, and allows a
> replacement hosted owner to continue the same active session and world
> snapshot under the new lease/fencing token. Verification passed the focused
> handoff regression plus the full `zone_owner` suite. Remaining parity risk:
> this is still an in-process handle move, not durable cross-process Zone state
> migration.

> Latest ZoneOwner RPC transport parity sync: 2026-05-26 adds a replaceable
> transport seam for owner-hosted Zone execution. `RpcZoneOwnerCommandClient`
> delegates commands and owner views through `ZoneOwnerRpcTransport`, while
> the hosted owner implements that transport as the current loopback runtime
> host. Gateway can therefore execute through a transport-owned runtime and
> read snapshots/identity/save/mail through the same path without mutating the
> caller's local runtime. Verification passed focused RPC transport isolation
> and stale-owner fencing regressions. Remaining parity risk: this is still
> loopback/in-process, not durable cross-process ZoneOwner RPC or state
> handoff.

> Latest SkillItemConsume request-id parity sync: 2026-05-26 hardens
> item-consuming shared-Zone spells against duplicate Account/Inventory
> delivery. `SkillItemConsume` now carries the Gateway-generated cast
> `request_id`, and the default committed-receipt key includes account,
> character, spell, and request id, so retrying one service command has a
> stable identity without collapsing distinct later casts of the same spell.
> Verification passed the new key regression, the in-process Account/Inventory
> service group, and the PoisonCloud/SummonSkeleton item-boundary route
> regressions. Remaining parity risk: durable external Account/Inventory
> ownership and ZoneOwner RPC/fencing still need to replace the in-process
> receipt store.

> Latest ZoneOwner hosted-runtime boundary parity sync: 2026-05-26 turns the
> ZoneOwner command-client seam into a hosted owner-runtime boundary. Gateway
> code can now dispatch a fenced `ZoneOwnerCommandRequest` to a
> `HostedZoneOwnerCommandClient` whose runtime is owned by the host, while the
> Gateway caller's local runtime remains untouched. Gateway snapshot, active
> identity, save, and external-mail refresh calls now go through the same owner
> client view interface, and the hosted owner validates current fencing through
> the shared lease authority before rejecting stale pre-handoff requests at the
> owner boundary. Verification passed the two hosted-owner regressions.
> The newer RPC transport seam above moves this hosted owner behind a
> replaceable transport. Remaining parity risk: this is still an in-process
> loopback host, not durable cross-process RPC/handoff; state takeover still
> needs a real network/process transport.

> Latest Account/Inventory idempotency parity sync: 2026-05-26 reduces the
> duplicate economy-commit risk on the shared Zone reward path. Shared
> Account/Inventory commits for Zone monster awards and ground-drop pickups now
> have deterministic committed-receipt keys, and the default service returns a
> cached receipt on retry instead of granting experience or gold twice. This
> protects the current in-process boundary and defines the exact-once behavior
> the durable actor/RPC service must preserve. Verification passed the new
> idempotent reward regression, the in-process service group, and the existing
> shared Account/Inventory boundary regression. The newer SkillItemConsume
> request-id sync above applies the missing cast-command identity. Remaining
> parity risk: the service still needs durable external ownership/fencing.

> Latest NPC world-service atomic outcome parity sync: 2026-05-26 improves the
> shared NPC/quest side-effect boundary toward Crystal's single shared world
> behavior. NPC script saved values, shared random seed, and entity mutation
> packets now cross the Gateway -> world-service boundary as one committed
> `ApplyScriptOutcome`, and Gateway only exposes the outcome to the shared Zone
> after a committed receipt echoes the expected entity side-effect payload. A
> rejected service no longer leaves one player with shared quest/script state
> advanced while entity mutations are missing. Verification passed the new
> atomic NPC outcome regression, existing NPC world-service boundary coverage,
> and shared saved/random sync regressions. Remaining parity risk: the service
> still needs a durable out-of-process implementation and broader quest/economy
> side effects.

> Latest Zone-native CharmedSnake parity sync: 2026-05-26 brings the
> `CharmedSnake` successful-hit paralysis effect into shared Zone authority.
> The native minion now emits the Crystal `ObjectPoisoned` paralysis state only
> after its delayed melee hit damages the target, and the effect uses the
> Crystal `10 - PetLevel` chance / `4 + PetLevel` duration shape while feeding
> the same native monster control timer used by other Zone-owned control
> spells. Verification passed the focused paralysis regression,
> SnakeTotem/Archer adjacent groups, `zone_native_player_` (30/30), and the
> Gateway self-Buff mirror regression. Remaining parity risk is broader
> monster AI/status coverage, not this summon effect.

> Latest Zone self-Buff parity sync: 2026-05-26 closes the first post-cast
> skill-state drift gap between shared Zone authority and the personal runtime.
> When Zone sends a self-target `AddBuff` or `RemoveBuff`, Gateway now mirrors
> that authoritative result into the owner's `SimulationSession` Buff state in
> addition to forwarding the packet to the client. Accepted MagicShield-style
> Buffs therefore appear in `world_snapshot.active_buffs` and are removed on
> Zone removal instead of existing only as transient client packets. Verification
> passed the new Gateway self-Buff mirror regression, shared-Zone Magic routing,
> focused `zone_native_player_` (30/30), and fmt check. Remaining parity risk:
> broader Buff families still need true Zone-owned lifetime/service persistence.

> Latest Zone-native SnakeTotem parity sync: 2026-05-26 closes the current
> Archer summon-family parity slice in shared Zone. `SnakeTotem` now enforces
> the Crystal `PetLevel + 1` active minion cap, refreshes expired
> `CharmedSnake` minions, self-destructs on missing/far Archer master, and
> kills owned minions when the Totem dies. `CharmedSnake` lifetime expiry and
> missing/far Totem checks now emit `ObjectDied` and run the Crystal 3x3 death
> explosion through Zone-native monster damage with owner attribution and no
> player damage from the summon path. Verification passed the two SnakeTotem
> regressions, Archer summon group, VampireSpider regressions, focused
> `zone_native_player_` (30/30), Gateway summon item-boundary regression, and
> fmt check. Remaining parity risk shifts to durable skill-state,
> process-external NPC/economy/account services, broader monster AI, and
> ZoneOwner handoff.

> Latest Zone-native VampireSpider parity sync: 2026-05-26 closes the
> `SummonVampire` Crystal pet-specific gap in shared Zone. Native
> `VampireSpider` hits now apply the Crystal `MasterVampire` side effect by
> broadcasting Bleeding `ObjectEffect` effect 18 on damaged targets and healing
> the owning Archer through authoritative Zone health state plus
> `PlayerHealed`. `VampireSpider` lifetime expiry and missing/far-master checks
> now produce `ObjectDied` self-destructs, and the 3x3 explosion damages nearby
> hostile Zone monsters through the native hit resolver while avoiding player
> damage. Verification passed the two VampireSpider regressions, the Archer
> summon group, focused `zone_native_player_` (30/30), Gateway summon
> item-boundary regression, and fmt check. Remaining parity risk: SnakeTotem
> swarm cap/expiry hardening, durable skill-state, and process-external
> services.

> Latest Zone-native Archer summon parity sync: 2026-05-26 brings the first
> Archer summon profiles under shared-Zone authority. `SummonVampire`,
> `SummonToad`, `SummonSnakes`, and `Stonetrap` are now Zone-routed summon
> spells with Crystal target-point/projectile-delay validation, retained
> friendly `ObjectMonster` packets, visible `extra`, master binding,
> summon-cap checks, lifetime expiry, and Gateway recognition without the
> Taoist amulet item-consumption path. Verification covers `VampireSpider`
> target-point spawn and retained-pet recall, `SpittingToad` stationary ranged
> `ObjectRangeAttack`, retained static `SnakeTotem` spawn plus owned
> `CharmedSnake` minion attack, and expiring static `StoneTrap` decoy aggro
> that pulls hostile native monsters without player damage. Evidence passed
> `zone_native_archer_` (4/4), the StoneTrap decoy regression, focused
> `zone_native_player_` (30/30), adjacent HolyDeva/PetEnhancer/summon tests,
> the Gateway summon item-boundary regression, and fmt check. Remaining parity
> risk: full SnakeTotem swarm cap/expiry hardening, VampireSpider
> self-destruct/vampire-heal details, and durable skill-state.

> Latest Zone-native summon/PetEnhancer parity sync: 2026-05-25 moves the
> first summon ownership, recast-recall, Taoist summon-family melee, and ranged
> HolyDeva combat surfaces into shared-Zone authority.
> `SummonSkeleton` / `SummonShinsu` / `SummonHolyDeva` are targetless Zone magic candidates
> behind the Account/Inventory skill-item command boundary; verified
> `SummonSkeleton` casts publish the Crystal magic packet surface, defer
> spawning for 500ms, and then create a retained friendly `BoneFamiliar`
> `ObjectMonster` with `master_object_id` set to the Zone player and
> `extra=true`. Late AOI joins see the owned summon, native monster tick logic
> does not treat it as a hostile player target, and recasting while it is active
> recalls the retained summon to the Zone player's authoritative position
> without a duplicate spawn or second item transaction. The retained
> `BoneFamiliar` now attacks hostile native monsters through Zone-owned
> `ObjectAttack` / delayed monster-damage packets while never targeting players,
> and summon damage keeps kill/drop ownership on the master. The retained
> `Shinsu` now shares the one-amulet Zone item boundary, delayed retained
> spawn, master binding, and hostile-monster melee path. The retained
> `HolyDeva` now verifies its 1.5s delayed spawn, six-tile `ObjectRangeAttack`,
> and 500ms delayed DC damage against hostile monsters without player damage.
> `PetEnhancer` now applies a retained visible Buff type 22 with DC/AC stats to
> owned Zone summons and increases later summon damage from that Buff state.
> Verification passed the spawn, recall, melee summon-combat, HolyDeva
> ranged-combat, Shinsu summon, and PetEnhancer Simulation regressions, focused
> `zone_native_player_` suite (30/30), Gateway summon item-boundary regression,
> existing item precheck coverage, and fmt check. Remaining parity risk:
> HolyDeva kiting polish, archer summon families, and durable skill-state.

> Latest Zone-native area-healing parity sync: 2026-05-25 moves MassHealing
> and HealingCircle into the shared-Zone combat model. Native area healing now
> validates near self-target casts, selects wounded Zone players in the recovery
> radius, applies delayed Zone-owned recovery for each target, broadcasts
> authoritative `ObjectHealth`, returns `PlayerHealed` to Gateway, and emits
> HealingCircle's delayed `ObjectSpell` circle from Zone state. Verification
> passed the two area-healing regressions with nearby-player recovery
> assertions, the focused `zone_native_player_` suite (28/28), existing Gateway
> Magic route coverage, and fmt check. Remaining parity risk: party/group
> filtering and summons remain open.

> Latest Zone-native Healing parity sync: 2026-05-25 moves the starter
> self-Healing spell into shared-Zone authority. Native Healing validates the
> self target, missing HP, MP/cooldown, and action window; publishes the
> Crystal owner/observer magic and healing effect; applies the delayed HP
> restore in Zone; and returns `PlayerHealed` so Gateway mirrors the
> authoritative result into the personal runtime. Verification passed the new
> Healing regression, the focused `zone_native_player_` suite (26/26),
> existing Gateway Magic route coverage, fmt check, and scoped diff check.
> Remaining parity risk: broader friendly/area healing and summons still need
> Zone-owned implementations.

> Latest Zone-native MagicShield parity sync: 2026-05-25 moves a real
> self-target Buff spell into the shared-Zone combat model. Native
> MagicShield validates and spends at the Zone boundary, emits the Crystal
> owner/observer packet surface (`Magic`, `ObjectMagic`, `AddBuff`,
> shield-up `ObjectEffect`), persists the visible Buff on the authoritative
> `ZonePlayer` for AOI joins, and applies its
> damage-reduction-percent stat when Zone-native monsters hit the player.
> Verification passed the new MagicShield regression, the focused
> `zone_native_player_` suite (25/25), Gateway Magic route coverage, and fmt
> check. Remaining parity risk: broader self/friendly Buff and healing
> families are not all Zone-owned yet.

> Latest production movement/input parity sync: 2026-05-25 closes the live
> rollback/input-stall slice with deployed evidence. Gateway release
> `20260525T0334CST-starter-transfer-cleanup` is active on UCloud and passed
> loopback/public health plus WSS smoke
> `docs/generated/load/remote-starter-transfer-cleanup-wss-smoke-20260525.json`.
> Production headed WebGPU packet-walk crossed the old fake same-map transfer
> cells with ACKs `339..343`, no `MapInformation`, and no `339 -> 330`
> rollback. Player Web deployment `dpl_7iG3bPgA7HTxkvEzN4LxP2rmFmFC` verified
> that movement input remains unlocked after initial playable scene readiness
> even while viewport scene assets continue background-loading. Headed Chrome
> evidence
> `docs/generated/player-qa/movement-jitter/prod-scene-input-unlocked2-webgpu-headed-keyboard-a-nosample-hold-20260525.json`
> passed with held-Walk cadence, ACKs `343,342,341,340,339`,
> `sceneInteractionReady=true`, WebGPU selected, packed prebuilt atlas active,
> no critical console errors, and no non-favicon 404s. Remaining server parity
> risk moves back to the larger shared MMO authority gaps: durable process
> boundaries, full skills/Buffs, NPC/economy commits, monster AI, and
> 30-active gameplay acceptance.

> Latest Crystal runtime starter-transfer parity sync: 2026-05-25 removes the
> non-Crystal starter demo gate from production Crystal runtime. The default
> starter scenario still exposes `starter-east-field-gate`, but
> `with_crystal_map_runtime()` clears it so production walking onto
> `339..341,268..271` is not converted into the fake same-map
> `330,270/Down` transfer. Verification passed the config test, the Gateway
> Crystal-runtime movement regression, and adjacent real Crystal movement
> transfer coverage. Deployed headed production verification is recorded in the
> movement/input parity sync above.

> Latest PoisonCloud live item-route parity sync: 2026-05-25 enables
> end-to-end shared routing for the item-consuming PoisonCloud cast. Gateway
> now recognizes targetless PoisonCloud, prechecks Zone acceptance before
> consuming required items, commits the amulet + green-poison cost through the
> Account/Inventory service, and dispatches the Zone ground spell only after
> that commit succeeds. Verification passed the focused Gateway precheck
> regression, the account/inventory boundary regression, and focused
> PoisonCloud/targetless Zone regressions. Remaining parity risk: the default
> command service is still in-process, not a durable production actor.

> Latest Zone-native ExplosiveTrap parity sync: 2026-05-25 moves Trap-family
> detonation into the shared-Zone combat model. Native ExplosiveTrap now emits
> the delayed caster-facing trap row, damages hostile Zone monsters that stand
> on those trap cells, and removes the ground action after first contact.
> Gateway also treats ExplosiveTrap as a non-item targetless Zone ground spell.
> Verification passed the focused ExplosiveTrap regression and broader
> `zone_native_player` group. Remaining parity risk: broader control skills,
> summons, and durable skill state are still incomplete.

> Latest Zone-native TrapHexagon parity sync: 2026-05-25 moves the next
> Trap-family root/control effect into the shared-Zone combat model. Native
> TrapHexagon now applies area root control to eligible hostile Zone monsters,
> emits the delayed eight-object `ObjectSpell` ring from Zone-owned state, and
> blocks rooted monster movement during the control window. Verification passed
> the focused TrapHexagon regression. Remaining parity risk: broader control
> skills, summons, and durable skill state are still incomplete.

> Latest Skill item-consumption boundary sync: 2026-05-25 adds an
> identity-bearing Account/Inventory command for item-consuming Zone skills.
> `SkillItemConsume` now covers PoisonCloud's amulet + green-poison cost
> through the default in-process service and returns `DeleteItem` receipt
> packets on commit; the 2026-05-26 sync adds the missing request id to that
> envelope. Verification passed the focused skill-consumption command
> regression and the existing account/inventory boundary regression. Remaining
> parity risk: the default account/inventory service is still in-process.

> Latest targetless ground-magic parity sync: 2026-05-25 removes the first
> object-target-only limitation from shared-Zone magic. `ZoneRuntime` now
> accepts ground-target `PlayerCastMagic` with `target_id=0` for
> FireWall/Blizzard/MeteorStrike/PoisonCloud, emits owner `Magic` plus observer
> `ObjectMagic`, and schedules the delayed ground-spell objects from the target
> point. Gateway shared routing now recognizes learned non-item targetless
> ground Magic and item-consuming PoisonCloud, dispatching them to Zone without
> fabricating a monster target after the required item-cost precheck/commit.
> Verification passed the focused targetless FireWall regression, the broader
> `zone_native_player` group, and locked Gateway+Simulation check.

> Latest Zone-native Trap parity sync: 2026-05-25 moves the first Trap-family
> root/control effect into the shared-Zone combat model. Zone native monsters
> now retain Crystal level data, and native Trap enforces the lower-level gate,
> roots the hostile monster, and emits the delayed Trap `ObjectSpell` with
> direction/param semantics. Verification passed the focused Trap regression
> and the broader `zone_native_player` group. Remaining parity risk:
> broader control skills, summons, and durable skill state are still
> incomplete.

> Latest Zone-native PoisonCloud parity sync: 2026-05-25 moves PoisonCloud's
> monster-side ground effect into the shared-Zone combat model. Native
> PoisonCloud now emits the delayed visible cloud object, ticks 3x3
> occupied-cell damage, and projects green `ObjectPoisoned` state from
> `ZoneRuntime`. Verification passed the focused PoisonCloud regression and
> the broader `zone_native_player` group. Remaining parity risk: the
> Account/Inventory command service still needs a durable external backend.

> Latest Zone-native chain/splash parity sync: 2026-05-25 brings MeteorShower
> and FireBounce secondary effects into the shared-Zone combat model.
> MeteorShower now publishes Zone-selected secondary targets and applies
> half-damage hits authoritatively; FireBounce now emits chained projectile hops
> and delayed damage between Zone monsters. Verification passed the focused
> MeteorShower/FireBounce regressions plus the broader `zone_native_player`
> group. Remaining parity risk: item commits, remaining Trap-style ground
> actions, summons, durable skill state, and many bespoke profession effects
> are still incomplete.

> Latest Zone-native ground-spell parity sync: 2026-05-25 brings persistent
> ground magic into the shared-Zone combat model. Native FireWall now follows
> the Crystal packet shape by delaying the five-cell `ObjectSpell` cross and
> applying recurring damage only from occupied fire cells in `ZoneRuntime`;
> Blizzard/MeteorStrike now do the same for their delayed 5x5 ground-spell
> cells and later damage ticks. Verification passed the focused FireWall and
> Blizzard-family regressions plus the broader `zone_native_player` group.
> Remaining parity risk: remaining Trap-style ground actions and bespoke
> profession effects are still incomplete.

> Latest Zone-native area magic parity sync: 2026-05-25 moves the first
> target-centered multi-monster magic branch into shared Zone. FireBang/IceStorm
> style native casts now carry secondary target ids in `Magic`/`ObjectMagic`
> and damage the secondary Zone monsters authoritatively. Verification passed
> the new area magic regression plus the focused `zone_native_player` group.
> Remaining parity risk: persistent ground spells, chain/splash variants,
> skill-specific scaling, and many special spell effects are still incomplete.

> Latest Zone-native special arrow Buff parity sync: 2026-05-25 moves the
> first Crystal Archer special-arrow Buff state into shared Zone. PoisonShot
> can now add the visible PoisonShot marker Buff to the authoritative Zone
> player, late observers receive the Buff with the player appearance, and
> CrippleShot consumes that Zone-held Buff to spread green poison over nearby
> native monsters. VampireShot healing is now Zone-owned too: the Zone updates
> player HP, broadcasts `ObjectHealth`, and tells Gateway to apply the heal to
> the personal runtime via `PlayerHealed`. Verification passed the new
> PoisonShot Buff / CrippleShot spread / VampireShot heal regressions, Gateway
> pending-heal coverage, CrippleShot vampire follow-up coverage, and the
> focused `zone_native_player` group. Remaining parity risk: complete Buff
> semantics, summons, and area magic still need native authority.

> Latest Gateway Magic route parity sync: 2026-05-25 verifies the practical
> Web/Gateway `Magic` route is no longer personal-session-only for seeded
> spells. The shared in-process runtime now has focused coverage showing a
> client magic command enters shared Zone authority, returns owner
> `Magic`/`ObjectMagic`, and broadcasts `ObjectMagic` to another observer.
> Remaining parity risk: spell-specific Crystal behavior is still incomplete;
> more native effects, Buffs, projectile spells, and skill-state persistence
> must move under Zone/world-service authority.

> Latest Zone-native poison tick parity sync: 2026-05-25 brings one
> damage-over-time skill path into shared Zone authority. `PoisonShot` no
> longer only launches a native magic packet against Zone monsters: it now
> records green poison on the Zone monster, broadcasts `ObjectPoisoned`, ticks
> Crystal-paced poison damage every 2 seconds, emits health/damage updates, and
> can finish the monster through the same Zone-owned death, drop, and award
> path. Verification passed the new PoisonShot poison-tick/kill-award
> regression plus the focused `zone_native_player` suite and locked Simulation
> check. Remaining parity risk: this is the first green-poison skill slice;
> broader poison formulas, CrippleShot/PoisonCloud, monster-applied poison
> damage, and Boss/status variants remain open.

> Latest ZoneOwner heartbeat parity sync: 2026-05-25 moves distributed-owner
> liveness from a manual renewal hook to scheduled Gateway-session behavior.
> Web sessions now run a ZoneOwner heartbeat on the runtime tick before deferred
> world ticks, using a deterministic time-aware lease renewal surface. Focused
> Gateway coverage proves before-expiry renewal, missed-heartbeat stale
> rejection, handoff rejection, and owner-boundary fencing. Remaining parity
> risk: this is still an in-process owner/client adapter; complete production
> MMO parity needs RPC transport, process-external owner loops, migration,
> fencing, and takeover recovery.

> Latest Zone-native player action-window parity sync: 2026-05-25 brings
> shared-Zone player combat timing closer to Crystal packet semantics. Zone now
> owns attack and spell action windows for native player combat: melee/range
> cannot relaunch before the attack window, magic cannot cast another spell
> before the spell window, and rejected commands only correct the owner instead
> of broadcasting combat packets or queuing damage. Verification passed the new
> range/magic action-window regressions, the focused `zone_native_player`
> suite, and Gateway shared-runtime coverage for early RangeAttack rejection.
> Remaining parity risk: this is timing authority only; full skill/Buff,
> poison damage, Boss AI, durable economy/NPC commits, and process-external
> ZoneOwner execution remain open.

> Latest NPC world-service command-envelope parity sync: 2026-05-25 replaces
> direct shared-NPC bridge mutation for saved values, script random seed, and
> diff-derived NPC entity side effects with `SharedNpcWorldService` command
> envelopes. The envelope includes active account/character identity and the
> `SyncSavedValues`, `SyncRandomSeed`, or `ApplyEntitySideEffects` payload, and
> Gateway applies shared Zone NPC/map state only after a committed receipt.
> Verification passed
> `shared_in_process_runtime_uses_npc_world_service_boundary`. Remaining parity
> risk: this is still bridge-state synchronization for entity packets; complete
> Crystal parity needs NPC map/event/service, quest, and economy mutations as
> authoritative Zone/world-service commands.

> Latest Account/Inventory command-envelope parity sync: 2026-05-25 turns
> the replaceable reward service into an explicit command boundary.
> Gateway now submits `SharedAccountInventoryCommandEnvelope` values carrying
> the active account/character identity plus `MonsterKillAward` or
> `GroundDropPickup` command payloads, instead of calling separate ad-hoc
> methods for each reward shape. Focused coverage proves both identity-bearing
> commands reach the injected service, that failed pickup commits still
> cancel/restore the Zone claim, and that the default service rejects identity
> mismatches before mutating runtime state. Remaining parity risk: this is still an
> in-process command envelope; a durable Account/Inventory actor or
> transactional store must own the commits next.

> Latest ZoneOwner command-client parity sync: 2026-05-25 moves Gateway
> command dispatch one step closer to a real ZoneOwner process boundary.
> `GatewaySession` now validates the current `ZoneOwnerLease` and then
> submits the full `ZoneOwnerCommandRequest` through a replaceable
> `ZoneOwnerCommandClient`; the default client preserves the in-process
> runtime path, while tests can inject a recording client to prove valid
> production commands cross that boundary and stale leases are rejected before
> the client is called. Gateway sessions also expose a lease-renewal hook
> through `ZoneOwnerLeaseAuthority`, so current owners can renew while old
> owners fail renewal after handoff. The in-process owner client can also
> carry the same authority and reject stale fenced requests at the owner
> boundary, even if a future Gateway caller skipped local validation.
> The in-memory authority now supports optional TTL renewal semantics, with
> focused coverage for before-expiry renewal and expired-renewal rejection that
> advances the next fencing token. Verification passed the focused Gateway
> command-client, renewal, owner-boundary stale-request, and TTL authority
> regressions. Remaining parity risk: this is still an in-process client
> adapter; the final target remains RPC transport, scheduled owner heartbeat,
> takeover, and process-external fencing enforcement.

> Latest Zone-native monster status parity sync: 2026-05-25 moves the first
> Crystal special monster hit effects into shared Zone authority. Native
> monster hits now evaluate Cave Maggot / Incarnated ZT paralysis and Toxic
> Ghoul-style green poison inside Zone, mutate the authoritative Zone player
> poison state, broadcast `ObjectPoisoned`, block movement while Zone-owned
> paralysis is active, and broadcast `ObjectPoisoned(poison=0)` when the
> status expires. Verification passed the new paralysis movement/expiry and
> green-poison non-blocking regressions plus the focused `zone_native_monster`
> shared-Zone suite and Simulation fmt check. Remaining parity risk: this is
> the first explicit player-status AI slice; full Crystal monster parity still
> needs the larger Boss/area/status matrix, poison damage ticks, summons, and
> distributed ZoneOwner execution.

> Latest Account/Inventory service-boundary parity sync: 2026-05-25 reduces
> the direct personal-session dependency in shared reward commits. Gateway
> now submits Zone kill awards and shared ground-drop claims through
> `SharedAccountInventoryService`; the default service keeps current behavior
> but the boundary can be replaced by an Account/Inventory actor. Verification
> passed `shared_in_process_runtime_uses_account_inventory_service_boundary`,
> including service-produced reward packets and failed pickup commit
> claim-cancel behavior. Remaining parity risk: the service implementation is
> still in-process/session-backed until a durable actor or transaction service
> owns gold/items/experience/quest reward commits.

> Latest NPC entity side-effect parity sync: 2026-05-25 reduces the NPC
> script mutation gap for shared monster state. NPC commands now get a
> pre/post monster snapshot diff in Gateway: new monsters become
> Crystal-backed `ObjectMonster` packets, cleared monsters produce
> `ObjectHealth(0)` plus `ObjectDied`, and removed monsters produce
> `ObjectRemove`. Health/death/remove packets are now valid shared-entity
> observer anchors, so `MONGEN` / `MONCLEAR` are visible through the shared
> Zone bridge instead of remaining local-only ECS mutations. Remaining parity
> risk: this is still bridge-based; full parity needs NPC map/event/service
> and quest/economy side effects submitted as authoritative Zone/world-service
> commands with rollback semantics.

> Latest NPC random shared-state parity sync: 2026-05-25 reduces another
> shared-NPC divergence from the personal-session bridge. Crystal NPC
> `RANDOM` seed progression now flows through Gateway shared Zone state:
> sessions apply the current shared seed before NPC commands and publish the
> post-command seed afterward, so two players do not fork the same NPC random
> script into independent local sequences. Verification passed
> `shared_in_process_registry_syncs_npc_random_seed_between_sessions` and
> `cargo +1.89.0 fmt --check -p mir2-gateway -p mir2-simulation`. Remaining
> parity risk: script randomness is shared, but NPC world mutations such as
> `MONGEN` / `MONCLEAR`, event flags, quest/economy commits, and service
> rollback still need native Zone/world-service authority.

> Latest Zone-owner command fencing parity sync: 2026-05-25 closes the first
> stale-owner command hole in the Gateway boundary. `GatewaySession` now wraps
> direct and production player commands in `ZoneOwnerCommandRequest`, requires
> a matching `ZoneOwnerLease` before executing them, and the production Web
> action dispatcher routes normal player commands through that validation
> point. `ZoneOwnerLeaseAuthority` now owns the current token for each zone,
> so an in-memory handoff that increments the fencing token also invalidates
> pre-handoff sessions. Focused coverage proves the current lease executes, a
> stale fencing token is rejected before runtime mutation/gameplay-event
> publication, a wrong owner id is rejected before production execution, and a
> superseded owner is rejected after handoff. Remaining parity risk: this is
> still an in-process guard/authority, not the final distributed ZoneOwner
> RPC/handoff implementation.

> Latest Zone-owner fencing metadata parity sync: 2026-05-25 starts replacing
> implicit single-process ownership assumptions with explicit route-owner
> facts. `ZoneOwnerLease` now travels with routed runtimes and Gateway
> sessions, and online session-cache records/routes persist `zoneOwnerId` plus
> `fencingToken` alongside `zoneId`. Verification passed focused registry,
> cache, admin-record, fmt/diff, and locked Simulation/Gateway checks.
> Remaining parity risk: this is metadata groundwork only; complete
> production MMO parity still needs a real ZoneOwner process/RPC boundary,
> command-side fencing validation, lease renewal, handoff, and stale-owner
> rejection under multi-Gateway operation.

> Latest NPC saved-value shared-state parity sync: 2026-05-25 closes the
> first concrete NPC/world-state divergence in the shared-Zone bridge. Crystal
> NPC `SAVEVALUE` / `LOADVALUE` state is now exposed as `SharedNpcSavedValue`
> and synchronized through Gateway shared Zone state, so two sessions sharing
> the same NPC script save slot see the same value instead of isolated
> personal-session copies. Verification passed the new cross-session saved
> value regression, existing sparse shared-NPC interact and shared quest-NPC
> CallNpc tests, Account/Inventory receipt coverage, fmt/diff checks, and
> locked Simulation/Gateway check. Remaining parity risk: this handles one
> saved-value primitive; full NPC/quest parity still needs quest state,
> service/economy mutations, flags, map/event side effects, and rollback to be
> submitted through a real Zone/world-service authority path.

> Latest Account/Inventory transaction-boundary parity sync: 2026-05-25
> reduces the remaining shared reward/economy bridge surface. Shared
> ground-drop pickup and Zone-native monster kill awards now flow through a
> single `SharedAccountInventoryTransactionReceipt` contract instead of two
> unrelated personal-session reward calls. The receipt marks the operation
> kind and commit result, carries only packets generated after character-state
> mutation, and remains the source of truth for committing or canceling shared
> Zone ground-drop claims. Verification passed the new Gateway receipt
> regression, existing Zone kill-award commit and ground-drop rollback tests,
> Simulation ground-drop receipt coverage, fmt/diff checks, and locked
> Simulation/Gateway check. Remaining parity risk: this is the service
> boundary, not yet an external Account/Inventory actor; NPC/quest side
> effects, gold/items/experience persistence, and rollback still need to move
> behind a real transactional world-service implementation.

> Latest 30-active movement/chat parity sync: 2026-05-25 promotes the current
> single UCloud shared-Zone server from the old 15-active safety cap to
> accepted 30-active movement/chat operation. This closes the specific
> "walking command delay under 30 active" gap by moving route-lease refresh
> out of the WebSocket hot loop, caching same-map transfer tiles instead of
> reading a full session snapshot on every move, coalescing observer movement
> packets, combining movement intent plus player tick under one Zone lock, and
> lazily generating retained AOI visibility packets. Production release
> `20260525T1348CST-route-refresh-background-task` passed public
> movement-only 30-active evidence with `ready=30/30`, `errors=0`,
> `keepAlive.p95=63ms`, plus move/chat evidence with chat every 30 actions
> (`keepAlive.p95=222ms`) and chat every 10 actions (`keepAlive.p95=68ms`).
> Remaining parity risk: movement/chat feel is now accepted for this
> single-Gateway shared Zone, but complete Crystal server parity still needs
> transactional reward/inventory commits, NPC/quest shared side effects,
> special monster AI, and distributed Zone owner fencing/handoff.

> Latest shared ground-drop commit receipt parity sync: 2026-05-25 makes the
> shared pickup transaction boundary explicit. Gateway now consumes a
> `SharedGroundDropPickupCommit` receipt from the character/economy commit
> path and uses `committed` to decide Zone `CommitGroundDropClaim` vs
> `CancelGroundDropClaim`; it no longer infers success from visible packet
> shapes. Verification passed the new Simulation receipt regression, Gateway
> rollback and normal remote pickup regressions, local/remote locked
> Simulation/Gateway checks, and production release
> `20260525T0843CST-grounddrop-commit-receipt` with public health, WSS smoke,
> and the current 30-client safe-cap baseline. Remaining parity risk: explicit
> receipt semantics are still implemented on top of the personal-session
> economy bridge; full parity needs the same contract backed by a transactional
> account/inventory service.

> Latest shared kill-award commit parity sync: 2026-05-25 removes a false
> reward notification edge from Zone-native monster death. Zone now reports
> `MonsterKillAward` to the Gateway but does not pre-send `GainExperience`;
> the character commit path mutates experience and emits the visible
> `GainExperience` packet only after the write succeeds and caps are applied.
> Verification passed native Zone kill/drop coverage, the new Gateway
> kill-award commit regression, shared routing/fallback drop regressions,
> local/remote locked Simulation/Gateway checks, and production release
> `20260525T0827CST-zone-award-commit` with public health, WSS smoke, and the
> current 30-client safe-cap baseline. Remaining parity risk: this is a
> cleaner commit boundary, not the final Account/Inventory actor; complete
> parity still needs transactional reward/quest/drop side-effect commits.

> Latest shared fallback drop-template parity sync: 2026-05-25 closes a
> concrete shared-combat fallback hole. If a monster target is available only
> from the shared map snapshot, the Gateway now builds its fallback
> `ZoneMonsterSpawn` with Crystal/starter drop templates instead of an empty
> drop list, so Zone-native kill resolution can still spawn owner-window
> ground drops. Verification passed the new fallback drop-template regression,
> neutral AI fallback coverage, native Zone kill/drop coverage, Gateway shared
> routing and rollback regressions, local/remote locked Simulation/Gateway
> checks, and production release `20260525T0804CST-zone-fallback-drops` with
> public health, WSS smoke, and current 30-client safe-cap baseline. Remaining
> parity risk: this is a fallback-spawn correctness fix; full Crystal parity
> still needs comprehensive Zone-owned drop generation and transactional
> account/inventory reward commits.

> Latest shared drop/economy rollback parity sync: 2026-05-25 locks down the
> current bridge behavior when a shared Zone ground-drop claim cannot be
> committed by the personal economy path. The new Gateway regression forces an
> over-cap shared gold pickup after the Zone has claimed the drop; the Gateway
> cancels the claim, restores the Zone/shared-map drop, suppresses
> `ObjectRemove`, and sends the owner a fresh `ObjectGold` spawn instead of
> granting impossible gold. Verification passed the rollback regression,
> adjacent normal claim and remote pickup regressions, locked Gateway check,
> and Gateway fmt check. Remaining parity risk: this proves the current
> two-phase claim/commit/cancel guard, but it is still a bridge; final parity
> requires Zone-owned drop generation plus a transactional account/inventory
> commit service.

> Latest Zone-native ranged monster AI parity sync: 2026-05-25 closes the
> first shared-Zone gap where ranged/magic native monsters behaved like simple
> melee chasers. Zone now retains each native monster's Crystal `ai`; when a
> ranged/magic AI branch has a visible non-adjacent player in range, it emits
> `ObjectRangeAttack` anchored at the monster's current tile and schedules the
> delayed player hit inside Zone instead of moving one tile toward the player.
> Verification passed the new ranged-monster regression, adjacent native melee
> and Buff authority regressions, Gateway shared routing coverage, local/remote
> locked Simulation/Gateway checks, and production release
> `20260525T0734CST-zone-monster-ranged` with public health, WSS smoke, and
> the current 30-client safe-cap baseline. Remaining parity risk: this covers
> the first generic ranged/magic native AI path; full Crystal parity still
> needs the many special Boss/range/area/status branches, summon lifecycle,
> NPC/quest side effects, economy commit, distributed Zone ownership, and
> accepted 30-active gameplay feel.

> Latest Zone-owned defensive Buff parity sync: 2026-05-25 extends Zone Buff
> authority from outgoing attack stats to incoming native monster damage. Zone
> now uses the target player's retained `MAX_AC` Buff stat when committing
> delayed native monster hits, suppressing HP mutation and hit packets when the
> Zone-held defence fully mitigates the current seed hit, and restoring normal
> damage after Buff expiry. Verification passed the new defensive Buff
> regression, adjacent attack Buff/native monster hit tests, Gateway shared
> routing coverage, local/remote locked Simulation/Gateway checks, and
> production release `20260525T0720CST-zone-buff-defence` with WSS smoke plus
> the current 30-client safe-cap baseline. Remaining parity risk: the current
> native monster damage model is still the MVP seed, and full Crystal Buff
> parity still needs rate/status effects, summon/pet Buffs, richer monster AI,
> and distributed Zone ownership.

> Latest Zone-owned Buff stat parity sync: 2026-05-25 closes the first gap
> where shared combat still trusted personal-session Buff math. The personal
> session now supplies unbuffed Zone-native damage profiles for melee, range,
> and object Magic; Zone applies its retained player Buff stats during native
> monster hit commit and removes the effect when the Zone-held Buff expires.
> Verification passed the new
> `zone_native_player_buff_stats_authoritatively_modify_damage_until_expiry`
> regression, existing Zone object-Magic coverage, Gateway shared routing
> coverage, local/remote locked Simulation/Gateway checks, and production WSS
> smoke after deploying release `20260525T0709CST-zone-buff-stats`. The
> current 30-client safe-cap baseline remains green at `15 active / 15
> rejected` with no client errors. Remaining parity risk: this is a first
> attack-stat Buff slice, not complete Buff/stat authority across defense,
> rates, pet/summon buffs, status logic, and distributed Zone ownership.

> Latest Zone-native Magic control parity sync: 2026-05-25 turns the first
> targeted control spells from personal-session side effects into shared Zone
> monster-authoritative state. Zone now records native monster control expiry,
> blocks controlled native monsters from walking or attacking, publishes
> Entrapment/CatTongue Crystal control packets, clears retained poison on
> expiry, and keeps ElectricShock/Entrapment as zero-damage control profiles
> instead of synthetic damage hits. Verification passed the new
> `zone_native_player_magic_control_stops_monster_ai_until_expiry` regression,
> the existing Zone magic damage/MP cooldown tests, native monster tick tests,
> Gateway shared routing coverage, and locked Simulation/Gateway check locally
> and on UCloud. Release `20260525T0651CST-zone-magic-control` is deployed;
> public health, 1-client WSS smoke
> `docs/generated/load/remote-zone-magic-control-wss-smoke-20260525.json`, and
> the current 30-client safe-cap baseline
> `docs/generated/load/remote-zone-magic-control-30active-timeout60-20260525.json`
> passed. Remaining parity risk: control is now Zone-native for this first
> object-Magic slice, but full Crystal skill authority still needs stat Buffs,
> summon lifecycle, AoE/ground spells, richer monster AI, NPC/quest side
> effects, economy transactions, and distributed Zone owner handoff.

> Latest Zone-native ranged combat server parity sync: 2026-05-25 moves
> targeted ranged/player magic monster hits one step further out of the
> personal-session bridge. Zone now owns live-monster target validation,
> submitted target-tile validation, range checks, launch packet fanout, and
> tick-time HP/death/drop/experience commit for `PlayerRangeAttackObject` and
> `PlayerCastMagic`; the follow-up same-day slice also makes Zone own object
> magic MP spend, per-Spell cooldown rejection, and `ObjectMana` AOI fanout.
> Shared Gateway routing now sends browser/client
> `RangeAttack` packets through this Zone path and has the object-target Magic
> route wired for learned Crystal magic profiles. Verification passed the new
> Simulation Zone ranged/magic authority tests, MP/cooldown rejection,
> invalid-target rejection, the new Gateway shared `RangeAttack` routing
> regression, the existing shared delayed melee regression, and locked Gateway
> check. This slice is deployed as UCloud Gateway release
> `20260525T0630CST-zone-magic-mp-cooldown` over
> `20260525T0615CST-zone-range-magic`; public health and WSS smoke
> `docs/generated/load/remote-zone-magic-mp-cooldown-wss-smoke-20260525.json`
> passed with `ready=1/1`, `capacityRejected=0`, `errors=0`, `messages=623`,
> and `ok=true`. The matching headed Chrome production pass
> `docs/generated/player-qa/movement-jitter/live-webgpu-keyboard-after-magic-mp-20260525.json`
> kept WebGPU selected and movement assertions green after the Gateway deploy.
> Remaining parity risk: this is not yet full Crystal skill authority;
> Buff/stat/control/summon/AoE semantics and broader monster AI/ranged/boss
> behavior are still future Zone work.

> Latest blocked transfer-source server parity sync: 2026-05-25 covers Crystal
> direct movement rows whose source tile is also static-blocked by map
> collision. A live Chrome test moved `Scout` to `BichonProvince 322:247`, the
> Library entrance source for `0104`, and proved the current production Gateway
> did not transfer. Server source now treats valid direct movement source cells
> as player-step targets for transfer evaluation while keeping non-transfer
> blocked cells, retained NPC/monster/player occupancy, and non-player
> placement strict. The shared Zone collision loader now uses the full original
> Crystal collision data for map `0`, matching the personal runtime's original
> Bichon path instead of the starter fragment. Verification passed the new
> personal-runtime and Gateway Library transfer regressions, the existing
> direct walk-on transfer regressions, adjacent Crystal movement import tests,
> Simulation/Gateway fmt check, and locked Simulation/Gateway check. Live parity
> is pending a Gateway release; until then the production Chrome tab remains on
> the old server behavior.

> Latest movement command latency parity sync: 2026-05-24 brought the live
> single-user walk cadence back inside the Crystal 600ms action window. The
> production command path no longer builds a full world snapshot for Walk/Run,
> Turn, KeepAlive, or Tick outcomes, and Gateway briefly defers runtime ticks
> after bootstrap/player input so immediate movement ACKs are not queued behind
> unrelated per-socket work. UCloud Gateway release
> `20260524Tmovelowlatency` is live and public health/WSS smoke passed. Player
> Web deployment `dpl_BommXyKsMcAX3Lmw4TYcg82a7Rsw` now connects normal custom
> domain sessions to `wss://165.154.65.136.sslip.io/ws` instead of the
> higher-jitter Cloudflare Worker `/ws` route. Production evidence
> `docs/generated/player-qa/movement-jitter/prod-normal-directws-keyboard-d-20260524T1513.json`
> records six `walk Right` commands, six `UserLocation` ACKs, frame latencies
> `555/522/516/523/517/517ms`, clean settle, zero logical rollback, zero
> scene blackouts, zero critical console errors, and zero non-favicon 404s.
> The browser diagnostic path now prints movement send/ack logs under
> `?movementLog=1` for future live parity checks.

> Latest movement rollback parity sync: 2026-05-24 corrected the remaining
> first-run rollback semantic in source and deployed the matching Web
> mitigation. Shared Zone now treats a Run received while the player is not yet
> in Crystal run grace as an effective one-tile Walk instead of issuing an
> origin correction, while still validating static collision, retained
> NPC/monster/player occupancy, and the intermediate/destination tiles for true
> Run. The matching Web client no longer mutates authoritative self coordinates
> for local prediction and waits for server ACK when the loaded map region
> cannot prove the next tile is walkable. Verification passed Web typecheck,
> scoped diff check, local movement smoke
> `docs/generated/player-qa/movement-jitter/local-left-walk-wait-map-20260523T233000.json`,
> and production Web smoke
> `docs/generated/player-qa/movement-jitter/prod-left-walk-web-rollback-fix-20260524T0034.json`
> on deployment `dpl_3BwwKyjXY9UFZS3jSZk3vCsybCrW`, with no visual jump,
> logical rollback, scene blackout, critical console error, or non-favicon 404.
> Server-side live parity is now also deployed: UCloud Gateway release
> `20260524T0310Z-rollbackfix` replaced `20260523T071900Z-actionqueue`, local
> `run_from_standstill_degrades_to_walk` passed on the host before packaging,
> public origin health and `mir2-status` are green, WSS smoke
> `docs/generated/load/remote-rollbackfix-wss-smoke-20260524.json` passed, and
> post-Gateway production movement smoke
> `docs/generated/player-qa/movement-jitter/prod-left-walk-gateway-rollbackfix-20260524T0320.json`
> passed with no rollback or blackout.

> Latest shared-Zone movement parity sync: 2026-05-22 closed the production
> same-map movement rollback caused by a stale personal-session snapshot
> overwriting Zone-authoritative position/direction after successful Walk/Run.
> Shared-zone upsert now preserves existing authoritative same-map transforms,
> keeping `UserLocation` acknowledgements monotonic with the accepted Zone
> movement. Focused Gateway regressions passed and the UCloud Gateway is live
> on release `20260522T174413Z-zone-transform` with public health and WSS smoke
> green. This is the server-side half of the final production movement closeout;
> the matching Web evidence is recorded in `docs/FRONTEND-1TO1-GAPS.md`.

> Latest movement-transfer server parity sync: 2026-05-22 fixed the gap where
> Crystal movement rows were exposed as transfer options but not guaranteed as
> authoritative server walk-on transfers. After a successful Walk/Run, both the
> personal runtime and the shared in-process Zone Gateway now detect when the
> authoritative player tile matches a direct Crystal movement source and apply
> the matching transfer, including Zone leave/join for the destination map.
> Normal production clients still cannot use debug `crystal:<map>:<x>:<y>`
> teleports. Verification passed the focused personal-runtime and shared-Zone
> direct-walk transfer regressions, adjacent Crystal movement import coverage,
> the existing same-map shared transfer sync regression, Rust fmt check, and
> locked Simulation/Gateway check. The graph evidence in
> `docs/generated/map/latest-crystal-map-reachability.json` shows direct
> Bichon-start reachability is 268/463 maps and 185/284 positive-respawn maps;
> maps outside that graph remain a separate NPC/script/event/item/special-route
> parity task rather than a screenshot resource task. Remote deployment is live
> as UCloud Gateway release `20260522T064157Z-walktransfer`, with local/public
> health and 1-client WSS smoke passing after restart.

> Latest original-map spawn parity sync: 2026-05-21 moved production Gateway
> startup onto the Crystal map runtime so original map `0` no longer mixes in
> the starter tutorial's `Training Dummy` / `Field Wasp` entities. Crystal
> runtime starts normalize map metadata from the respawn manifest, rebuild the
> current map from original NPC rows and `MapRespawn` rows, and leave the
> starter fixture data only for isolated demo/vertical-slice coverage. A
> follow-up same-day regression covers saved non-default Crystal starts such as
> `WoomyonWoods(S)`: broad Crystal respawn rectangles now materialize a
> representative original respawn at the source point when the player enters
> that data range, so live map screenshots can show the map's own `Oma` /
> `ForestYeti`-style roster instead of an empty local scene. The
> all-map gameplay audit now finds the local full Crystal client root by
> default on this Mac and passed in strict mode with 463 maps, 6341 respawns,
> 6293 walkable-candidate respawns, 48 Crystal-inert no-candidate respawns, and
> zero movement, respawn, NPC, or static failures.

> Latest shared NPC/monster visual parity sync: 2026-05-21 fixed a retained
> shared-object sprite loss where Gateway could ingest `ObjectNpc` /
> `ObjectMonster` packets into the shared map layer without sprite metadata and
> later rebroadcast them with `image=0`. This matched the live symptom of NPC
> labels, quest markers, and minimap dots remaining visible while the body
> sprite disappeared. Shared entity conversion now retains `NPC/<image>` and
> `Monster/<image>` sprite snapshots, merge logic keeps existing sprite metadata
> across later packets, and shared spawn serialization uses the retained image.
> Focused Gateway regressions passed for NPC and monster shared spawn retention.
> Player Web now mitigates the current live snapshot shape with a Crystal actor
> manifest fallback, but server-side parity still depends on deploying the
> updated Gateway binary.

> Latest Gateway scheduling server sync: 2026-05-19 deployed release `20260519T141920Z-fastka` to keep the single UCloud Gateway observable during a 30-client entry/hold soak. Synchronous per-socket simulation action, tick, snapshot, and save work is now isolated with Tokio blocking handoff; the Tokio worker count is configurable; Redis route lease refresh is throttled to a per-socket interval and maintained during idle ticks; runtime tick cadence is configurable; and Web KeepAlive returns a direct ACK without forcing a full session/Zone tick. Remote evidence passed `ready=30/30 capacityRejected=0 errors=0 ok=true`, and `/health` passed 30/30 samples during load with Redis records and route leases both reaching 30. Current server posture: release `fastka` is live, but capacity is intentionally returned to `30/15/15` because scripted keepalive latency during the immediate post-StartGame action burst is still too high for accepting 30 active as the normal gameplay target.

> Latest Gateway health-soak server sync: 2026-05-19 deployed release `20260519T124942Z-healthfast` to reduce live `/health` pressure while testing the current single-Gateway 30-player ceiling. The Redis session-cache status path now scans once, uses one `MGET` for records, avoids duplicate lease scans, and runs from a blocking worker. Remote WSS evidence passed 30/30 clients over a 20-minute soak and again over a 5-minute post-release soak, but health probes still timed out during load and keepalive p95 stayed high. Current server posture: keep the UCloud Gateway on the safe `30/15/15` cap; 30 active is reachable but not yet a stable accepted operating target.

> Latest Gateway Postgres-pool server sync: 2026-05-19 hardened the live account persistence path for the current single-Gateway production profile. Postgres account-store access now uses a bounded in-process connection pool, runs schema migration once per pool, serializes same-process source writes, and writes only the touched account for hot account/character saves. Release `20260519T105412Z-nogit` is deployed on the UCloud 4H8G host with pool size 8; remote WSS load evidence passed 30/30 clients without capacity rejection or client errors, and post-run health was Redis healthy after reverting to the safer `30/15/15` cap. Remaining server-parity risk: this stabilizes the current single-process Gateway persistence pressure, but full production scale still needs a longer health-responsive soak plus future distributed owner/shared-Zone architecture.

> Latest new-account server sync: 2026-05-19 fixed the account bootstrap path that made fresh Sui Passkey/Wallet logins appear with the development `Scout` Warrior already present. The server now keeps that template only on `demo`, rejects missing password accounts, and creates first-time Sui accounts with zero characters so the client select screen's `NEW` flow is authoritative for class/gender/name selection. Verification passed focused Simulation account lifecycle regressions and locked Simulation/Gateway check.

> Latest original Bichon intro quest-chain sync: 2026-05-18 extended the automated live gameplay acceptance from the custom starter loop into Crystal's original early Bichon chain. `apps/simulation/tests/vertical_slice.rs` now starts on original map `0` near Assistant Jane/CraftLady/Merchant John, accepts and completes q1 carry semantics, advances q2 through original Scarecrow `GingerTea` Q drops, completes q3 through Merchant John, advances q4 through original passive Deer melee kill plus corpse harvest `DeerMeat` Q drops, turns in q4, and verifies q5 is unlocked. This deliberately keeps passive Deer behavior separate from hostile spell targeting: FireBall remains hostile-target only, while Deer uses close melee plus Harvest like Crystal. Verification passed Simulation vertical slice 6/6, Simulation shared Zone 77/77, security lifecycle 9/9, and `cargo +1.89.0 check --locked -p mir2-simulation -p mir2-gateway`. Remaining parity risk: this proves the first original quest band through q5 availability, while the full 1-45 chain still needs live-client visual/dialog route acceptance across later NPCs.

> Latest Zone-native monster combat/drop seed: 2026-05-18 moved explicit shared monster melee from a personal-runtime mirror to the shared Zone path, added the first native monster AI tick surface, and wired native monster-to-player damage back into the target personal session. `ZoneCommand::SpawnMonster` retains live map monsters as Zone-native objects, Gateway routes explicit `WorldCommand::Attack` against those shared monsters through `PlayerAttackObject`, and Zone now preserves Crystal timing by broadcasting `ObjectAttack` at launch while deferring strike, damage, health, death, Zone-created drops, experience, and `MonsterKillAward` until the next Zone tick. Zone-native monsters also pursue nearby players with `ObjectWalk`, emit delayed adjacent melee hit visuals, update Zone player HP, broadcast player `ObjectHealth`, and send `PlayerDamaged` so Gateway mutates the target `SimulationSession` HP instead of leaving damage as a visual-only packet. Personal sessions still apply quest/experience inventory-side effects, but the authoritative HP/death/drop producer for this melee path is the Zone. Verification passed Simulation `shared_zone` 77/77, Gateway `shared_in_process` 40/40, security lifecycle 9/9, focused delayed-hit/native-attack/AI-tick/HP-writeback regressions, and `cargo +1.89.0 check --locked -p mir2-simulation -p mir2-gateway`. Remaining parity risk: RangeAttack/Magic and complete Crystal drop-table behavior are not fully native yet.

> Latest Postgres+Redis server sync: 2026-05-18 hardened the server runtime boundary for prod-like operation. Gateway startup now refuses production/staging or explicit required-cache modes without Redis session/routing cache, and required Redis is pinged before accepting player traffic. This complements the existing Postgres account-store source-of-truth requirement so prod-like runs do not accidentally use the single JSON account file or process-local online route map. Local development remains lightweight with JSON/in-memory fallback. Remaining server-parity risk: deeper persisted character domains and distributed owner handoff are still outside this slice.

> Latest ranking-system server sync: 2026-05-18 closed the basic Crystal ranking packet/runtime surface. `GetRanking` now produces typed `Rankings` packets with current-character rank, total count, player-id listings, and listing details for Overall and the five Crystal classes. The implementation uses account-store character saves plus the active in-session character snapshot, keeps unauthenticated requests silent, and treats `OnlineOnly` as the current active-session online subset until a broader shared roster exists. Gateway routes the Web command to the real packet instead of a placeholder. Verification passed Simulation ranking regression, Gateway command/event regressions, Rust fmt/check, and Web typecheck. Remaining parity risk: original multi-player online-only semantics, pagination breadth, and ranking-entry NPC/statue presentation need more live data and UI acceptance once production persistence/shared-online state exists.

> Latest shared Zone drop-claim server sync: 2026-05-18 made shared ground-drop pickup arbitration Zone-native. The server now synchronizes shared drop snapshots into the Zone runtime, lets Zone reserve the exact or nearest eligible drop under range/ownership/group rules, commits successful pickups so observers and late joiners cannot see or pick the same object again, and cancels failed claims so personal award failures restore the shared drop cleanly. Gateway manual pickup and IntelligentCreature pickup no longer directly remove shared drops from its map cache first; they ask Zone for the claim, then apply the personal inventory/gold mutation and commit or cancel. The Zone run continuation grace is also 5s to keep valid Crystal-style run chains intact under slow Gateway scheduling. Verification passed Simulation `shared_zone` 74/74 and Gateway `shared_in_process` 38/38. Remaining parity risk: drop creation, monster combat, and item/gold ownership mutation are still not fully Zone-owned, so this closes pickup arbitration but not the whole shared combat/drop source of truth.

> Latest vertical-slice server sync: 2026-05-18 locked the current playable core into one integration acceptance path. `apps/simulation/tests/vertical_slice.rs` now proves five-class creation enters game with class/gender/vitals and Crystal-empty personal inventory/belt/storage/equipment/skill state while still surfacing available starter quests; Warrior/Wizard/Taoist/Assassin/Archer each has at least one basic skill loop plus combat/health-effect evidence; the Bichon starter Village Guide -> Field Wasp -> quest item -> turn-in reward loop closes; and shared Zone presence, Walk/Run/Turn, chat, and owner-window drop pickup are stable together. Verification passed Simulation fmt, vertical slice 4/4, shared Zone 74/74, and security lifecycle 9/9. Remaining parity risk: this is a playable vertical slice, not full Crystal completion; deeper work still includes exact live-client feel, full class skill trees, and native Zone-owned monster AI/combat/drop authority beyond packet-retained/shared ownership surfaces.

> Latest Redis route-admission server sync: 2026-05-18 turned the Web Gateway route lease into a server-side online uniqueness gate. Authenticated `StartGame` now reserves the account/character route lease before the runtime enters the world, so a competing socket or Redis-backed Gateway is rejected while the first owner is fresh. The pending lease is released if StartGame does not claim the matching active identity, and normal session-cache refresh/owned cleanup keeps successful sessions renewing and stale sockets unable to erase a newer owner. Redis `/health` now counts lease keys directly. Verification passed Gateway fmt, new StartGame route-admission tests 2/2, in-memory/Redis route-lease regressions, production Web path safety 3/3, full session-cache focused suite 14/14, and health boundary coverage. Remaining parity risk: this closes duplicate online entry at the route layer, while full distributed reconnect/kick still needs an owner handoff or shared Zone service.

> Latest Gateway hot-path server sync: 2026-05-18 reduced the Web Gateway save hot path and separated account-action pressure from online gameplay capacity. Login, new-character, and StartGame now use optional in-flight capacity guards (`MIR2_GATEWAY_MAX_LOGIN_IN_FLIGHT`, `MIR2_GATEWAY_MAX_NEW_CHARACTER_IN_FLIGHT`, `MIR2_GATEWAY_MAX_START_GAME_IN_FLIGHT`) that return capacity rejection before the server piles work onto those paths, and `/health` reports their configured/current values. Active-character persistence now batches dirty saves with `MIR2_GATEWAY_SAVE_DEBOUNCE_MS`, `MIR2_GATEWAY_SAVE_CHECKPOINT_SECONDS`, and `MIR2_GATEWAY_SAVE_QUEUE_LIMIT`, while socket close force-saves the active character so an otherwise movement-only session cannot drop the final Zone-authoritative position/direction. Verification passed Gateway fmt, focused capacity/action-inflight tests 3/3, save-queue tests 2/2, production Web path safety 3/3, health reporting, reconnect store tests 2/2, Web script syntax/typecheck, and live hot-path smoke `docs/generated/load/gateway-hotpath-codex-smoke.json` with `ready=4/4`, `capacityRejected=0`, `errors=0`, `ok=true`. Remaining parity risk: this protects the current JSON-store Gateway process from avoidable per-action write pressure, but production-grade persistence still needs a database or single-owner account actor before high-concurrency acceptance.

> Latest Gateway capacity server sync: 2026-05-18 added first-class capacity policy to the Web Gateway. `MIR2_GATEWAY_MAX_WS_CONNECTIONS` rejects excess WebSocket upgrades with `503`, `MIR2_GATEWAY_MAX_ACTIVE_SESSIONS` rejects excess authenticated `StartGame` attempts before creating another active player, and `MIR2_GATEWAY_MAX_RECONNECT_LEASES` bounds retained reconnect grace sessions. Capacity permits are released on socket close, failed StartGame, reconnect expiry, or reconnect restore, and `/health` exposes the configured and current counters. Verification passed Gateway fmt, capacity unit tests 2/2, reconnect store capacity-transfer tests 2/2, production Web path safety 3/3, health reporting, Web typecheck/script syntax, plus live active-session and WebSocket-capacity smokes under `docs/generated/load/` with `ready=2/4`, `capacityRejected=2`, `errors=0`, `ok=true`. Remaining parity risk is operational sizing: the code now enforces limits, but production cap values still need real hardware/network soak and cross-process coordination if more than one Gateway owns players.

> Latest reconnect grace server sync: 2026-05-18 added a bounded Gateway-side reconnect grace path for unexpected WebSocket loss. Active sessions are now retained in memory for `MIR2_GATEWAY_RECONNECT_GRACE_SECONDS` after close, their route lease is refreshed for the same window, and the next authenticated `StartGame` for the same account/character restores the prior `GatewaySession` before replaying bootstrap packets. This preserves shared Zone presence through short network drops while keeping unauthenticated StartGame and debug/admin command rejection intact. Verification passed Gateway fmt, reconnect store 2/2, reconnect key helper, production Web path safety 3/3, route-lease stale-owner regression, Web typecheck/script syntax, and the live reconnect smoke artifact `docs/generated/player-qa/reconnect/reconnect-resume-codex-reconnect-grace-smoke-final.json` with `ok=true`. Remaining parity risk: the grace store is local to the running Gateway process; production multi-process resume still needs a durable owner/shared Zone handoff.

> Latest original quest-chain server sync: 2026-05-18 promoted the original Crystal 1-45 normal quest chain from packet/data visibility into a tested server gameplay loop. The quest manifest imports Crystal `NewQuestInfo` plus source quest-text task blocks, while Simulation now handles availability projection, level/class/prerequisite-chain gates, accept/no-task-ready semantics, carry item grants, kill/item/flag task advancement, item-task drops into quest inventory, `ChangeQuest` task refreshes, NPC finish rejection until ready, reward option selection, reward grant, quest-item cleanup, `CompleteQuest`, sharing, and NPC loaded-object quest links. Verification passed locked game-data/simulation check, focused original Crystal quest coverage 5/5, seed-state visibility tests 3/3, packet-driving finish/share coverage, Field Wasp local quest regression, Crystal quest-drop regressions 3/3, generator syntax check, and Rust fmt checks. Remaining parity risk is live client acceptance of exact dialog text/branching and route guidance, not the backend quest-state loop.

> Latest continuous-run grace server sync: 2026-05-15 refreshed the shared Zone run-chain deadline after every accepted movement, preventing long held Run sequences from losing their continuation grace between ticks and degrading valid follow-up Run intents into walk/correction behavior. Verification passed focused Simulation `continuous_run_extends_run_grace_after_successful_run` and `cargo +1.89.0 fmt --check -p mir2-simulation`. The matching Player Web long-run rollback/route-spam acceptance is recorded in `docs/FRONTEND-1TO1-GAPS.md`.

> Latest shared object-action Zone AOI sync: 2026-05-14 moved shared monster/generated-object action fanout out of the Gateway same-map queue and into the simulation shared Zone. Gateway now seeds shared Monster/NPC object surfaces into Zone as needed, then routes shared-object action/result packets through `BroadcastSharedObjectPackets`; Zone keeps monster/generated actor ids intact, rebases only local-self target/result ids to the authoritative Zone player id, applies retained health/death/buff stale guards, and delivers by retained object AOI/visible sets instead of every player on the same map. Verification passed focused shared-object Zone regressions 3/3, Simulation `shared_zone` 69/69, Gateway `shared_in_process` 35/35, and Simulation/Gateway fmt/check. Remaining parity risk: drop claim/award ownership and the underlying monster AI/combat/drop generation are still not fully native Zone-owned.

> Latest retained object authority hardening: 2026-05-14 tightened the shared Zone retained-object lifecycle beyond health/mana. Retained non-player Buffs now store and replay full `AddBuff` payloads, including values/stats and paused state, so late joiners and object-AOI entrants no longer see only an expiry shell. Dead or harvested retained objects now suppress stale later movement, mana, and positive-health packets, and retained object health is monotonic downward until an explicit `ObjectRevived` clears the stale guard. Retained NPCs and live monsters/heroes/players now participate in Zone movement occupancy, while dead/removed/drop/deco objects do not block movement and `ObjectRemove` clears the blocker. Verification passed Simulation `shared_zone` 66/66, Gateway `shared_in_process` 35/35, and Simulation/Gateway fmt/check. Remaining parity risk is still the deeper migration of monster AI, combat damage, drop award, and NPC side-effect generation from personal runtime output into native shared Zone authority.

> Latest retained object-vitals Zone sync: 2026-05-14 made retained object health and mana part of the shared Zone visibility surface. `ObjectHealth` packets for retained non-player objects are now stored alongside the retained spawn, updated to the latest percent/expire payload, emitted after `ObjectMonster` when a player joins or enters the object's AOI, and cleared on `ObjectRevived` so old zero-health does not survive a revive. `ObjectMana` is likewise retained for hero/generated object late-join visibility and cleared on death/revive. Verification passed focused retained-object health/mana regressions 3/3, full Simulation shared_zone 60/60, Gateway shared_in_process 35/35, and Simulation/Gateway fmt/check. This closes a late-join/AOI gap where wounded shared monsters or MP-bearing retained heroes could appear without their current Crystal vitals packets; damage/mana ownership itself is still bridged from personal runtime output.

> Latest retained harvest-corpse Zone sync: 2026-05-14 moved the harvested-corpse guard from Gateway-only read-model state into the simulation shared Zone retained-object lifecycle. Non-player `ObjectHarvested` packets now mark the retained object as harvested/dead at the packet movement anchor, duplicate `ObjectHarvested` packets are suppressed, stale live spawns after harvest are canonicalized back to the harvested dead surface for late joiners, and actor-local player harvest packets are explicitly not treated as corpse lifecycle state. Verification passed focused harvested retained-object regressions 3/3, full Simulation shared_zone 57/57, Gateway shared_in_process 35/35, and Simulation/Gateway fmt/check. Actual harvest reward transfer still comes from the personal runtime bridge, so native Zone-owned harvest/drop generation remains open.

> Latest Crystal action-queue movement sync: 2026-05-23 replaced the Zone
> latest-intent movement shortcut with a bounded per-player
> `ZoneMovementAction` queue for Walk/Run/Turn, matching Crystal's ordered
> `_retryList`/`ActionTime` semantics without adding an external broker.
> Walk/Run actions now consume one ready action per Crystal cadence, Turn uses
> the 350ms turn delay, Walk/Run use the 600ms action window, and the later
> movement-rollback correction above changed raw Run from standstill to an
> effective one-tile Walk rather than an origin correction. Failed Walk
> preserves the old direction, failed Run after `CanRun` keeps the Crystal
> direction update before correction, successful actions emit owner
> `UserLocation`, observer `ObjectWalk`/`ObjectRun`/`ObjectTurn`, and
> `SaveTransform`. Verification passed Simulation `shared_zone` 78/78,
> focused Gateway Walk+Run and Turn routing regressions, Simulation/Gateway
> fmt-check, Web typecheck, Web production build, remote Gateway release
> `20260523T071900Z-actionqueue`, Player Web action-queue verification deployment
> `dpl_HmHQ4CXfy7d895kHFMfiNLHWespN`, custom-domain production `/health`, and production
> walk/run captures
> `docs/generated/player-qa/movement-jitter/prod-action-queue-keyboard-walk-fix2-20260523T1331.json`
> plus
> `docs/generated/player-qa/movement-jitter/prod-action-queue-keyboard-run-fix2-20260523T1332.json`,
> both `ok=true` with no visual jumps, logical rollback, scene blackouts,
> critical console errors, or non-favicon 404s.

> Latest delayed combat status-result/server sync: 2026-05-14 expanded delayed player-owned combat result forwarding to include status and buff effects. When a later Tick emits a local-player-owned `ObjectStruck` bundle, Gateway now preserves matching `ObjectPoisoned`, `AddBuff`, `RemoveBuff`, and `PauseBuff` packets for the struck target or acting player, while unrelated attacker bundles stay suppressed. Verification passed the focused delayed-player-action filter regression. This closes another visible combat-effect gap for shared observers; native Zone combat/drop resolution remains the larger parity target.

> Latest retained Zone object/server sync: 2026-05-14 made the simulation shared Zone retain non-player object surfaces instead of treating them as fire-and-forget broadcasts. Rebased `ObjectMonster`, `ObjectHero`, `ObjectNpc`, `ObjectItem`, `ObjectGold`, and `ObjectDeco` packets now become retained Zone objects; later movement/death/revive/zero-health/hidden/effect/poison/buff/name/NPC-image packets update that retained surface; retained visible object Buffs expire from Zone tick with observer `RemoveBuff`; remove and pet-pickup packets tombstone retained objects; retained object spawn/update/remove delivery now uses the object's AOI/visible set instead of the owner actor's AOI; Join/movement AOI diffing sends spawn or `ObjectRemove` as players enter/leave range; owner Leave removes owner-generated retained objects; retained item/gold drops expire from Zone tick with `ObjectRemove`; stale live spawn/drop packets can no longer reinsert removed objects or revive dead retained objects; and `ObjectRevived` markers suppress stale dead spawns until a live retained spawn arrives. Verification passed focused retained-object regressions 16/16, Simulation shared_zone 55/55, Gateway shared_in_process 35/35, and Simulation/Gateway fmt/check. This closes a server-parity gap where late entrants or current observers could miss existing shared monsters/summons/NPCs/drops, see stale object Buffs, miss object-centric spawn/remove/pet-pickup while the actor is out of view, or have zero-health/dead/revived/removed object state undone by late personal-runtime snapshots; deeper combat/drop source-of-truth work remains.

> Latest shared entity-action observer/server sync: 2026-05-14 expanded shared non-player packet visibility for monster/generated-object actions. `ObjectAttack`, `ObjectRangeAttack`, `ObjectMagic`, `ObjectProjectile`, and attacker-anchored `ObjectStruck` emitted by actors already present in the shared map now reach same-map observers through Gateway pending queues, separate from the player-origin Zone rebasing path, and target references to the acting player's local self id are rewritten to the Zone player id. Same-batch current-player `ObjectHealth`, `DamageIndicator`, `ObjectDied`, `ObjectPoisoned`, `AddBuff`, `RemoveBuff`, and `PauseBuff` results are only forwarded when anchored by a shared-actor strike. Verification passed focused shared entity movement/action regressions 2/2 and Gateway shared-in-process 35/35. This closes another visible multiplayer packet gap; authoritative health/death/drop resolution still needs the deeper Zone-native combat migration.

> Latest shared entity-movement observer/server sync: 2026-05-14 closed a shared-world visibility gap for non-player object movement. When the acting runtime emits `ObjectTurn`, `ObjectWalk`, or `ObjectRun` for a shared monster or generated object, Gateway now updates the shared map cache and queues that movement packet to same-map observers, while avoiding a personal snapshot read unless such a movement packet is present. Verification passed focused entity-movement broadcast and Run timing regressions, Gateway shared-in-process 34/34, Gateway shared-zone-state 36/36, and Simulation/Gateway fmt/check. This improves multiplayer monster/generated-object movement visibility; the source of monster AI movement remains personal runtime until the deeper Zone-native AI migration.

> Latest shared drop despawn-expiry/server sync: 2026-05-14 aligned shared-map drop lifetime cleanup with Crystal-style ground-drop despawn semantics. Shared drops now get a local lifetime deadline as soon as they enter the Gateway shared map cache, and Tick/KeepAlive removes expired drops from shared state, tombstones their ids, clears stale owner/despawn metadata, and sends `ObjectRemove` to the current player plus same-map observers. Verification passed focused shared expiry coverage 4/4, Gateway shared-zone-state 36/36, Gateway shared-in-process 33/33, and Simulation/Gateway fmt/check. This closes the visible stale-drop class after owner windows expire; full Zone-native drop generation/award authority remains deeper work.

> Latest shared drop ownership-expiry/server sync: 2026-05-14 aligned shared ground-drop owner windows with Crystal's expiring ownership semantics. Gateway now records a local deadline from `ownership_remaining_ticks` when shared drops enter the map cache or are committed from death drops, and both manual shared pickup and IntelligentCreature auto pickup expire stale ownership before enforcing owner/group pickup checks. Verification passed focused manual/auto expiry regressions, Gateway shared-zone-state 35/35, Gateway shared-in-process 32/32, and Simulation/Gateway fmt/check. This prevents stale shared drops from staying permanently owner-locked; full shared drop despawn/expiry and native drop generation remain deeper work.

> Latest shared object-movement cache/server sync: 2026-05-14 aligned the shared Gateway map cache with ordinary Crystal object movement packets. `ObjectTurn`, `ObjectWalk`, and `ObjectRun` now mutate shared entity coordinates/direction using the existing dead-object guard, matching the transform handling already present for push/backstep/dash-style packets. Verification passed a focused Gateway shared-zone-state movement regression. This improves shared snapshot freshness for moved monsters/generated objects; full Zone-native monster AI movement and combat/drop effects remain a deeper migration.

> Latest shared owned-generated lifecycle/server sync: 2026-05-14 tightened shared lifecycle handling for player-owned generated objects. Summoned `ObjectMonster` packets now retain the owner by resolving `master_object_id` to the online Zone player, including the local self-id form produced by the personal runtime before observer rebasing, stale ownerless snapshot merges keep that owner, and player leave or map change removes that player's owned shared generated rows from the old shared map cache while queuing `ObjectRemove` to same-map observers. Verification passed Gateway shared-zone-state 33/33, Gateway shared-in-process 32/32, plus focused shared runtime/state regressions for hero disconnect, local-master summon disconnect, owner-preserving snapshot merge, and owner map-change cleanup. This closes a visible multiplayer residue class after logout/transfer; full Zone-native ownership of generated object behavior and combat/drop effects remains a deeper migration.

> Latest shared intelligent-creature pickup/server sync: 2026-05-14 aligned the shared multiplayer manual and auto pet-pickup paths with Crystal's intelligent-creature pickup semantics. Gateway now recovers when the visible drop is shared-map state rather than a local ECS ground-drop entity: it looks up the drop by target location, checks the active creature's fullness, pickup mode, gold/item filters, item pickup grade, ownership, and auto range using the same Simulation-side rules as local pickup, awards the item/gold through the personal state layer, removes the shared drop, and fans out `IntelligentCreaturePickup` to nearby sessions. Filter-blocked shared item pickup now restores the drop instead of tombstoning it. Verification passed focused Gateway intelligent-creature coverage 6/6, Simulation shared_zone 38/38, Gateway shared-zone-state 29/29, Gateway shared-in-process 30/30, and fmt/check. This closes a real shared-world usability gap for intelligent creatures, while native Zone-owned drop award/inventory authority remains a later deeper migration.

> Latest shared spawn/skill-target server sync: 2026-05-14 closed another multiplayer visibility class around generated actors and skill target references. Zone now forwards `ObjectHero`, `ObjectMonster`, `ObjectNpc`, NPC update/image packets, and `IntelligentCreaturePickup` to AOI observers, while rewriting summoned-monster masters and self-target skill/projectile/struck references from personal local ids to shared Zone ids. Gateway shared state now removes picked drops on `IntelligentCreaturePickup`, materializes ObjectHero/ObjectMonster/ObjectNpc into the shared map read model, applies existing dead markers to late monster spawn packets instead of reviving stale objects, and keeps pushed/backstep-style object transforms in the shared entity cache. Verification passed the focused Simulation and Gateway shared-zone regressions for these surfaces. This improves hero/summon/NPC/pet-pickup/moved-monster multiplayer consistency; full server parity still needs native shared ownership of the underlying combat/drop/NPC side effects.

> Latest shared dead-marker server sync: 2026-05-13 hardened monster lifecycle ordering in the shared Gateway layer. Death packets now create a shared dead marker even before the entity row exists, stale later snapshots are forced back to dead state at the packet's death location/direction, action checks reject that object immediately, and death-drop commit can anchor from `ObjectDied` location without a prior live snapshot. Out-of-order revive and harvest packets are now locked by regressions as well. Verification passed Gateway shared-zone-state 23/23. This closes another stale snapshot/revive/harvest class while full Zone-native damage/drop generation remains open.

> Latest shared delayed-damage server sync: 2026-05-13 closed a visible gap for player-owned delayed combat packets emitted on later ticks. Gateway now extracts only delayed damage bundles whose `ObjectStruck.attacker_id` is the local player, forwards those matching target health/death/remove/drop packets through Zone AOI, and avoids rebasing unrelated monster AI Tick packets as player actions. Shared `ObjectHealth(percent=0)` also kills the shared entity even without max HP, and a shared-runtime pair regression verifies rebased delayed `ObjectStruck/ObjectHealth` reaches the observer after `Attack -> Tick -> observer drain`. Verification passed the focused delayed-damage filter regression, focused no-max-HP death regression, focused delayed combat regression, Gateway shared-zone-state 19/19, and Gateway shared-in-process 26/26. This improves delayed combat visibility while full Zone-native combat/drop authority remains open.

> Latest shared transform-cache server sync: 2026-05-13 aligned Gateway read-model freshness with Zone-authoritative movement. `SaveTransform` outbounds now update shared player presence immediately before pending packet/transform drain, and `world_snapshot()` reads the local `SelfPlayer` transform from that Zone presence so session-cache readers no longer observe the pre-Zone tile for one extra command. Verification passed the focused transform-cache regression, Gateway shared-zone-state 18/18, and Gateway shared-in-process 25/25. This closes a concrete movement/cache lag while full Zone-native monster/combat/drop generation remains open.

> Latest shared viewport/transform server sync: 2026-05-13 stopped personal viewport snapshots from acting as authoritative full-map deletes. Shared Gateway state now keeps monsters and drops when they disappear from one client's scene snapshot, relying on explicit `ObjectRemove`, shared pickup, or duplicate death-drop guards for deletion; committed death-drop anchors also survive corpse-row removal. Zone now rebases `TransformUpdate` and retains transform type for late `ObjectPlayer` visibility. Verification passed Simulation shared Zone 35/35, Gateway shared-zone-state 17/17, and Gateway shared-in-process 25/25. This closes a concrete shared-world state-loss bug while full Zone-native monster/drop generation remains open.

> Latest shared revive-state/server sync: 2026-05-13 added shared lifecycle handling for `ObjectRevived`. Gateway shared state now clears dead/harvested/death-drop/remove tombstones on revive, restores HP from max HP where known, and rejects stale dead snapshot reapplication after the revive packet. Verification passed the focused revive/remove-tombstone regressions and Gateway shared-zone-state 15/15. This keeps revive semantics coherent with the shared death, corpse, and drop guards while fuller Zone-native monster lifecycle remains open.

> Latest shared harvest-corpse/server sync: 2026-05-13 closed the cross-session duplicate harvest-corpse path. `ObjectHarvested` now updates Gateway shared map state, the harvested flag survives stale session snapshot resync, and future `Harvest` packets aimed at that corpse are stopped before another personal session can harvest it again. Verification passed the focused reharvest regression and Gateway shared-zone-state 13/13. Full Zone-native corpse/drop generation remains open, but duplicate shared corpse harvesting is now guarded.

> Latest shared death-drop/server sync: 2026-05-13 closed the first duplicate-drop failure mode after shared monster death. Gateway now treats `ObjectDied` and zero-percent `ObjectHealth` as death-drop anchors, commits the acting runtime's matching nearby drop snapshots into the shared map once, and suppresses later stale duplicate drops from other personal session snapshots. Verification passed focused death-drop commit/spawn regressions 3/3 and Gateway shared-zone-state 12/12. Full Zone-native monster damage/drop generation remains open, but stale duplicate shared drops after death are now guarded.

> Latest shared late-join status/server sync: 2026-05-13 retained player visual status fields in Zone so late AOI entry does not reset them to defaults. `ObjectPlayer` now reflects retained name colour, display name, guild name, light, weapon, weapon effect, armour, poison, wing effect, mount/riding, fishing, and level effects after the corresponding live packets have been observed. Verification passed focused late-join status retention and full Simulation shared Zone 35/35. This closes a late-join appearance consistency gap while monster damage/death/drop authority remains open.

> Latest shared late-status/server sync: 2026-05-13 added Zone AOI rebasing for broader Crystal player status and late-system packet surfaces. Observer fanout now rewrites the local self id to the shared Zone player id for `PlayerUpdate`, `DamageIndicator`, `ObjectColourChanged`, `ObjectGuildNameChanged`, `ObjectLeveled`, `ObjectName`, `MagicDelay`, `PauseBuff`, `MountUpdate`, `FishingUpdate`, `ObjectTeleportOut`, `ObjectTeleportIn`, and `ObjectDeco`. Verification passed focused late-status coverage and full Simulation shared Zone 34/34. Late-join retention for every visual field is still a follow-up; visible observer delivery is now covered.

> Latest shared teleport/poison server sync: 2026-05-13 made Zone consume player-origin `UserLocation` action packets before observer fanout. This covers Teleport/Blink-style skill outcomes where the personal runtime emits a location change plus effect: Zone now updates authoritative position/direction, occupancy, and `SaveTransform` before sending observers the rebased `ObjectEffect`. `ObjectPoisoned` also now rebases to the shared player id. Verification passed focused transform/poison regressions and full Simulation shared Zone 33/33. Monster damage/death/drop authority remains the next larger server-parity gap.

> Latest shared skill-transform/server sync: 2026-05-13 promoted movement-skill transform outcomes into shared Zone authority. Zone now consumes successful BackStep/Dash/DashAttack/AttackMove/Pushed-style owner transform packets, updates the authoritative player position/direction and occupancy, emits `SaveTransform`, and blocks invalid occupied/static destinations with a `UserLocation` correction instead of broadcasting impossible movement. Verification passed focused transform success/reject regressions and full Simulation shared Zone 32/32. This reduces the class of multiplayer rollback/drift bugs for movement skills; combat damage/death/drop authority still needs the next migration.

> Latest shared skill-movement/server sync: 2026-05-13 added Zone AOI rebasing for Crystal movement-skill and special-skill state packets. `ObjectBackStep`, `ObjectDash`, `ObjectDashFail`, `ObjectDashAttack`, `ObjectSitDown`, `SetConcentration`, `SetElemental`, `SetBindingShot`, `RemoveDelayedExplosion`, `ObjectSneaking`, and `ObjectLevelEffects` now reach observers with the shared Zone player object id instead of the acting personal session's local self id. Verification passed focused movement/special-skill observer tests and full Simulation shared Zone 30/30. This closes another visible multiplayer skill-surface mismatch; full shared skill outcome authority remains open.

> Latest shared harvest/server sync: 2026-05-13 fixed harvest observer semantics in the shared multiplayer layer. Zone `BroadcastPackets` now handles `ObjectHarvest` and `ObjectHarvested` like other player-origin action packets, rebasing the personal-session actor id to the online Zone player id and sending observers the Zone-authoritative movement anchor. Verification passed the focused harvest observer regression and full Simulation shared Zone 28/28. This closes the visible multiplayer harvesting packet gap; full Zone-owned harvest/combat/drop authority remains open.

> Latest shared NPC/task/server sync: 2026-05-13 extended the shared multiplayer layer beyond visible NPC fallback into task, group packet, and stale-health behavior. Sparse Gateway sessions can `CallNpc @Main` against a shared Village Guide snapshot and start the Crystal quest state locally, `ShareQuest` now reaches online group members through the shared pending packet path, shared drop owner windows respect online owner group members, shared monster health no longer increases from stale personal-session `ObjectHealth` packets, and the acting personal runtime receives shared monster snapshots before target-based and direction-only combat/harvest resolution. Verification passed focused Gateway tests plus Gateway shared state 9/9, shared registry 25/25, focused Simulation shared-monster snapshot application, and focused Gateway current-map shared-monster application. Full Zone-native monster damage/death/drop authority is still open.

> Latest shared drop ownership/server sync: 2026-05-13 carried Crystal drop owner-window semantics into the shared multiplayer layer. Active `DropOwnership` is now exported on `GroundDropSnapshot`, Gateway translates personal owner ids to Zone player ids while syncing shared drops, non-owners are blocked during the ownership window without deleting the shared drop, and `ObjectItem` / `ObjectGold` spawn packets can fan out through Zone AOI. Verification passed focused Simulation shared Zone drop fanout and Gateway shared-zone-state 7/7. This closes a concrete multiplayer consistency bug for monster drops, while full Zone-owned monster/drop resolution remains open.

> Latest shared player appearance/server sync: 2026-05-13 added Zone-retained hidden/dead/effect appearance state for player actors. Rebased self `ObjectHidden`, `ObjectHide`, `ObjectShow`, `ObjectDied`, `ObjectRevived`, and `ObjectEffect` packets now mutate `ZonePlayer`, so late `ObjectPlayer` visibility uses the current Crystal appearance flags instead of defaulting to visible/alive/no-effect. Verification passed Simulation shared Zone 25/25. This improves late-AOI state continuity for player visuals while full combat/death authority still needs deeper Zone migration.

> Latest shared Buff expiry/server sync: 2026-05-13 made retained Zone player Buff visuals expire instead of lingering forever. Broadcasted self `AddBuff` packets now record a Zone expiry deadline from Crystal's relative `expire_time`, `tick` emits observer `RemoveBuff` when the deadline is reached, and future `ObjectPlayer` visibility no longer exposes stale Buff state. Verification passed Simulation shared Zone 24/24. This closes the first Zone-side Buff lifecycle gap for visible player appearance, while stat effects remain owned by the personal runtime for now.

> Latest shared Buff state/server sync: 2026-05-13 added Zone-retained visible player Buff state for multiplayer appearance. Self-targeted `AddBuff` / `RemoveBuff` packets now update the authoritative `ZonePlayer` state after object-id rebasing; `ObjectPlayer` packets expose the active visible buff type list and newly visible observers receive rebased `AddBuff` details so late AOI entry no longer misses existing player Buff visuals. Verification passed Simulation shared Zone 23/23. This improves shared-player state continuity, while Zone-native Buff duration, expiry, and skill effect ownership remain open.

> Latest shared skill/Buff server sync: 2026-05-13 broadened the shared Zone visual fanout for player-origin skill state. Observer broadcasts now rebase local self ids to the authoritative Zone player object id for visible mana, Buff, effect, spell, push, revive, hide/show, and toggle surfaces, including `ObjectMana`, `AddBuff`, `RemoveBuff`, `ObjectEffect`, `ObjectSpell`, and `ObjectPushed`. Verification passed Simulation shared Zone 22/22. This improves multiplayer visual consistency for skills/Buffs, while full Zone-owned skill resolution remains open.

> Latest shared NPC/server sync: 2026-05-13 connected shared-view NPC interaction to the Crystal NPC path. The typed Crystal `CallNpc` client packet now runs the same NPC script/dialog machinery as high-level `Interact`, and Gateway shared sessions can click NPCs sourced from the shared map layer even when the sparse personal ECS did not spawn that NPC locally. The fallback creates a local NPC entity from the shared snapshot, then delegates to the existing script/service path so service range checks and dialog state stay consistent. Verification passed Simulation shared Zone 21/21 and Gateway shared registry 20/20. This closes the visible-NPC-but-no-response multiplayer bug; full Zone-native quest/NPC side-effect authority remains open.

> Latest shared-authority/server sync: 2026-05-13 moved successful combat/skill visual packet delivery into the shared Zone path and started reconciling shared monster state from those packets. The personal `SimulationSession` still validates and resolves the action locally, but Gateway now forwards successful Attack/RangeAttack/Harvest/Magic packet surfaces to Zone, and Zone rebases actor ids from the local self object to the shared online player object before AOI fanout. Covered observer packets are `ObjectAttack`, `ObjectRangeAttack`, `ObjectMagic`, `ObjectProjectile`, `ObjectStruck`, `ObjectHealth`, `ObjectDied`, and `ObjectRemove`. The shared map layer now applies health/death/remove packets, keeps stale personal sessions from reviving or healing a shared monster snapshot, tombstones removed entities, and blocks stale attacks against shared dead/removed targets. Verification passed Simulation shared Zone 20/20, Gateway shared entity state 5/5, Gateway shared registry 20/20, and Simulation/Gateway fmt/check. This is a real shared-world improvement for observers, but not the final combat authority migration: native Zone-owned monster damage/drop ownership remains the next server-parity gap.

> Latest chat/server sync: 2026-05-13 aligned the implemented player chat path with Crystal's channel and linked-item semantics. Protocol now decodes/encodes Crystal `C.Chat` linked `ChatItem` payloads and the complete observed `ChatType` range through value 16. Session preparation handles persisted chat bans, 2s anti-spam tick banning, `@ADDSTORAGE`, linked item lookup, and Crystal internal link text generation. Shared Zone routing now owns live chat delivery for normal AOI `ObjectChat`, whisper, group, guild, mentor, relationship, local shout, map shout, server shout, and GM announcement, with `$pos`, level-8 shout gate, 10s shout cooldown, one-shot map/server shout permission consumption, `NewChatItem`, and server-wide `ToAll` Gateway fanout for Shout3. Web chat display types now include Mentor and Relationship. Verification passed Protocol `chat_` 2/2, Simulation `shared_zone` 18/18, Simulation `chat_` 46 + shared filtered coverage, Gateway `chat_` 3/3, locked Protocol/Simulation/Gateway check, Rust fmt check, and Web `npm exec tsc -- --noEmit`.

> Latest architecture/server sync: 2026-05-13 wired the new shared Zone state machine into the Gateway shared-runtime path, closed the normal WebSocket production command boundary for this MVP, and verified it with live two-client smoke. StartGame now produces a Zone join snapshot through the personal session, Gateway presence maps are registered as Zone sessions, Walk/Run/Turn/Chat are handled by `ZoneManager` instead of mutating only the personal `SimulationSession`, movement Tick/KeepAlive consumes the latest Zone intent, and LeaveZone runs before LogOut/Disconnect save. Zone outbounds are fanned out to active or pending Gateway sessions, including observer `ObjectWalk`, `ObjectRun`, `ObjectTurn`, `ObjectChat`, and `ObjectRemove`, while owner `SaveTransform` updates the personal runtime and advances snapshot freshness for persistence/cache readers. The WebSocket command path can now enforce production safety, rejecting unauthenticated StartGame and normal-client debug `MoveTo`, `Stage5Command`, and `crystal:<map>:<x>:<y>` transfers while allowing only HMAC-verified passkey login through the gateway edge. Verification passed Gateway lib 121/121, Gateway shared in-process registry 20/20, production WebSocket safety 3/3, Simulation shared Zone 12/12, security lifecycle 9/9, Simulation/Gateway fmt/check, Gateway health, `docs/generated/load/two-client-zone-smoke-133316.json` with 2/2 ready clients and 0 errors, ad hoc browser evidence with mutual visibility, and the repeatable `npm run smoke:two-client-zone` evidence `docs/generated/player-qa/two-client-zone/two-client-zone-script-135930.json` with mutual player visibility plus movement and chat broadcast delivery. Remaining server-parity risk in this architecture track is migrating more gameplay authority into the shared Zone and human Crystal feel acceptance, not missing shared-world movement broadcasts, production command gating, repeatable smoke coverage, or visible two-client presence.

> Latest architecture/server sync: 2026-05-12 added the first real shared Zone state machine instead of extending personal-session `RemotePlayer` projection. `ZoneRuntime` is synchronous and single-writer, stores online players by `SessionId`, owns occupancy/AOI/movement intent state, emits owner `UserLocation`, observer `ObjectPlayer` / `ObjectWalk` / `ObjectRun` / `ObjectTurn` / `ObjectRemove` / `ObjectChat`, and emits `SaveTransform` for session persistence write-back. The security lifecycle helper rejects unsafe normal-player commands before runtime execution. Verification passed focused shared Zone tests 12/12, security lifecycle tests 9/9, Simulation fmt, and locked Simulation check. This is the simulation-layer Zone foundation; Gateway production routing to consume it remains the next server-parity integration step.

> Latest backend worker sync: 2026-05-11 added a source-backed Hero book/stat requirement close. Crystal `HeroObject.UseItem` first calls `CanUseItem(item)` for Hero inventory use, while `HumanObject.CanUseItem` rejects unmet gender/class, level/max-level, `Max*`/`Min*` stat requirements, and already-known book spells before the Hero book path creates `UserMagic` and sends `NewMagic` with the Hero flag. Rust now mirrors that requirement gate for Hero book learning by reading current non-broken equippable HeroInventory stat totals before mutating `heroLearnedMagics`, preventing stat-required books from learning when the Hero does not meet the Crystal gate. Verification passed the focused stat-book regression, focused `hero_inventory` 16/16 plus book/key integration 1/1, Hero AI 28/28, Simulation fmt check, and locked Simulation check.

> Latest backend worker sync: 2026-05-11 added a source-backed Friend/blacklist Stage 5 state-flow close. Crystal `PlayerObject.AddFriend(name, blocked)` represents both friend and blacklist rows in `Info.Friends` and treats any existing row as `PlayerAlreadyAdded`, so a normal friend is not silently converted into a blocked row and a blocked row is not silently converted into a normal friend. Rust high-level social commands now match that single-entry rule for modeled Stage 5 state, preserve self-target rejection, and save/reload the resulting list. Verification passed focused social/economy integration, adjacent social/mail regressions, Simulation fmt check, and locked Simulation check.

> Latest coordinator sync: 2026-05-11 reconciled the Hero progression, player movement-skill progression, and Mail exact-claim pass. Hero learned magic now follows the bounded Crystal `UserMagic` progression loop after successful keyed Hero AI spell use, including persisted level/experience, `MagicLeveled`, `MagicDelay` on level-up, and subsequent learned-level reuse. Player movement skills now avoid generic practice gain and follow Crystal success gates: `BackStep` levels only after actual back-step movement, `ShoulderDash` only after a successful dash, and `FlashDash` only after a real hit target is found. Stage 5 Mail claim now preflights exact serialized parcel attachments as an all-or-nothing batch, leaving mail/gold/items untouched on no-space failure and consuming `gold/items/item_states_json` after a successful exact claim. Verification passed focused Hero progression 2/2, Hero AI 28/28, `magic_packet_crystal_` 73/73, Mail 9 unit + 2 integration tests, `cargo +1.89.0 fmt --check -p mir2-simulation`, `cargo +1.89.0 check --locked -p mir2-simulation`, and targeted diff checks. Remaining server risk is broader Hero book/stat requirement exactness, Crystal skill-gain modifier tuning, and deeper late social/economy packet-perfect surfaces.

> Latest sync: 2026-05-11 closed a bounded Hero learned-magic progression server gap. Crystal `UserMagic` persists `Level`, `Key`, and `Experience`, `HeroObject.UseItem` learns books through `NewMagic(hero=true)`, `HeroObject.CanUseMagic` requires learned magic plus `Key > 0`, Hero AI classes select spells through `CanUseMagic`, and `HumanObject.LevelMagic` advances practice experience by 1..3, uses `Need1/Need2/Need3` thresholds, sends `MagicDelay` on level-up, and always sends `MagicLeveled` after a successful practice update. Rust now advances Stage 5 Hero learned-magic level/experience from successful Hero AI learned-spell use, emits the same progression packet surface, saves the new state, and lets later Hero AI selections use the progressed learned level. Verification passed focused Hero progression 2/2, Hero AI integration 28/28, focused `hero_inventory` 15 lib tests plus book/key integration 1/1, `cargo +1.89.0 fmt --check -p mir2-simulation`, and `cargo +1.89.0 check --locked -p mir2-simulation`. Remaining server risk is broader Hero book/stat requirements and exact skill-gain modifier tuning.

> Latest coordinator verification: 2026-05-11 reran the current server-parity gate after merging the bounded Hero/Guild work with the frontend movement-feel rollback fix. Server-side evidence is green: locked GameData/Protocol/Simulation/Gateway check, Simulation/Gateway fmt, full locked `mir2-simulation` 856/856 plus Hero AI 26/26, focused Hero AI 26/26, focused `guild_` 16/16, and Gateway shared in-process registry 15/15. After the later bounded Hero progression sync above, remaining server-parity risk is broader Hero book/stat requirement exactness, exact skill-gain modifier tuning, and any future Crystal source evidence for a Guild alliance client/dialog surface.

> Latest sync: 2026-05-11 closed the bounded Hero learned-magic packet/save gap. Crystal source shows `HeroObject.UseItem` learns `ItemType.Book` into `Info.Magics` and sends `NewMagic` with the Hero flag, while `MirConnection.MagicKey` selects the Hero actor whenever `key > 16` or `oldKey > 16`; `HeroObject.CanUseMagic` then requires a learned magic and `Key > 0`. Rust now mirrors that loop for the modeled Stage 5 Hero state: HeroInventory books create `heroLearnedMagics` at key 0, Hero MagicKey updates/persists the key, and Hero AI consumes it only after key assignment. Verification passed focused Hero loop 1/1, Hero AI 26/26, focused `hero_inventory`, Simulation fmt, and locked Simulation check. Remaining server risk is Hero magic level/experience progression and broader book requirement/stat parity.

> Latest sync: 2026-05-11 closed a bounded Guild alliance save-surface gap. Crystal `GuildObject` declares runtime `AllyGuilds` / `AllyCount` and Crystal `GuildRankOptions` includes `CanAlterAlliance`, but the audited `GuildDialog`, packet enums, `PlayerObject.RequestGuildInfo`, and `GuildInfo.Save/Load` show no typed alliance packet or persisted alliance data. Runtime Stage 5 alliance state remains visible in-session through the existing RequestGuildInfo guild-chat readback, but saved Stage 5 JSON no longer rehydrates alliance list/count/broadcasts after restart. Verification passed `cargo +1.89.0 test --locked -p mir2-simulation guild_ -- --test-threads=1` with 16/16, plus `cargo +1.89.0 fmt --check -p mir2-simulation` and `cargo +1.89.0 check --locked -p mir2-simulation`.

> Latest sync: 2026-05-10 deepened Crystal server parity for Hero learned-magic selection and Guild alliance readback. Optional Stage 5 `heroLearnedMagics` now drives Hero AI spell eligibility like Crystal learned magic state: learned spell presence, `key > 0`, and learned level all gate the Wizard Hero priority chain before MP/cooldown/cast behavior. Guild alliance readback now preserves Crystal `RequestGuildInfo` type 0/1 semantics and appends Stage 5 guild-chat visibility for `AllyCount`, `AllyGuilds`, and recent alliance broadcasts, avoiding an invented packet where this Crystal source tree has none. Verification passed focused `guild_` 15/15, Hero AI 25/25, full locked `mir2-simulation` 855/855 plus Hero AI 25/25, Gateway shared registry 15/15, fmt/check, Web typecheck, NPC marker evidence, and diff checks. Remaining server parity risk is full Hero magic learning/key packet progression, any future source-backed Guild alliance dialog surface, broader Hero class exactness, and final client visual/feel acceptance.

> Latest sync: 2026-05-10 deepened Crystal server parity for Guild alliance state and Wizard Hero late single-target spell selection. Guild alliance now tracks `AllyGuilds` / `AllyCount`-style state, gates changes with `CanAlterAlliance`, handles known-guild canonicalization, duplicate no-op, self/missing/permission/active-war rejection, and alliance broadcast logging through Stage 5 command surfaces because this Crystal tree does not expose a typed alliance-return packet. Wizard Hero now follows the rest of `WizardHero.cs::ProcessAttack` after area spells: low-level undead `TurnUndead`, `FlameDisruptor`, `Vampirism`, `FrostCrunch`, then the old ThunderBolt/GreatFireBall/FireBall fallback, with Hero MP/cooldown and packet/state evidence. Verification passed focused `guild_` 14/14, Hero AI 23/23, full locked `mir2-simulation` 854/854 plus Hero AI 23/23, Gateway shared registry 15/15, fmt/check, Web typecheck, movement blocked-target evidence, and diff checks. Remaining server parity risk is exact Hero learned-magic state/progression, deeper alliance protocol if future source evidence appears, broader Hero class exactness, and final client visual/feel acceptance.

> Latest sync: 2026-05-10 deepened Crystal server parity for Guild war lifecycle and Wizard Hero attack selection. Guild war state now models duration expiry, start/end colour packets, `WarEndedWithGuild` guild messages, Newbie/self/missing/duplicate/funds rejection order, and at-war state removal. Wizard Hero now uses Crystal source-backed `ProcessAttack` priority for Repulsion, caster-centered and target-centered area spells, then single-target fallback, with Hero MP/cooldown and packet evidence. Verification passed focused `guild_` 12/12, Hero AI 20/20, full locked `mir2-simulation` 852/852 plus Hero AI 20/20, Gateway shared registry 15/15, fmt/check, Web typecheck, movement settle capture, and diff checks. Remaining server parity risk is full alliance semantics, late Wizard Hero branches, exact learned-magic state, and human visual/feel acceptance.

> Latest sync: 2026-05-10 advanced the remaining server-behavior edge cases for Guild and Hero. Stage 5 Guild now models known guilds, active wars, war broadcasts, richer territory fields, Crystal request-war prompt packets, `GuildWarReturn` rejection/order/rollback, war cost deduction via guild storage gold, and territory page/purchase packet surfaces. Wizard Hero AI now covers Crystal `ProcessFriend` support spells, including `MagicShield` and `MagicBooster` with level gates, MP spend, `ObjectMana`, self-target `ObjectMagic`, `AddBuff`, active buff gates, cooldown, and insufficient-mana no-op behavior. Verification passed `guild_` 10/10, Hero AI 17/17, full locked `mir2-simulation` 850/850 plus Hero AI 17/17, Gateway `shared_in_process_registry` 15/15, Simulation/Gateway fmt, locked four-package check, Web typecheck, movement script syntax, live route-spam obstacle capture, and diff checks. Remaining server parity risk is full alliance/war lifecycle breadth, exact Hero learned-magic/skill priority breadth, and human visual/feel acceptance.

> Latest sync: 2026-05-10 completed a verified Guild/Hero server-parity slice on top of the latest client-feel pass. Server-side Guild now models Crystal rank option bits for the covered Stage 5 operations, gates notice edits and storage actions by permission, gates guild-gold withdrawal by leader rank, enforces safe-zone usage for item/gold storage, rejects `DontStore` and rental `DontStore` items, and stores exact guild item state/user id through list and retrieve. Server-side Hero AI now covers Wizard Hero ranged spell casting for `FireBall` / `GreatFireBall` / `ThunderBolt` with level gate, Hero MP spend, `ObjectMana`, Hero `ObjectMagic`, cooldown, and delayed target damage. Evidence passed focused Simulation `guild_` 5/5, `trade_` 12/12, Hero AI 13/13, full locked `mir2-simulation` 845/845 plus Hero AI 13/13, Simulation/Gateway fmt, locked GameData/Protocol/Simulation/Gateway check, Web typecheck, movement harness syntax, and targeted diff check.

> Latest sync: 2026-05-10 consolidated the current server-parity worker round after coordinator review. Trade escrow now has true two-account commit/rollback/full-bag/disconnect behavior, Taoist Hero owner healing has Crystal packet/state regressions, and the client action-feel bridge remains typechecked against the live Web harness. Evidence passed focused Simulation `trade_` 12/12, Gateway shared registry 15/15, Hero AI 11/11, full locked `mir2-simulation` 843/843 plus Hero AI 11/11, Simulation/Gateway fmt, locked GameData/Protocol/Simulation/Gateway check, Web typecheck, movement harness syntax check, and targeted diff check.

> Latest sync: 2026-05-10 completed the Worker TradeEscrow server-parity slice for true two-account Trade escrow. Server-side Trade now rejects bound, soulbound, rental-bound, rental-owned, rental-expiring, rental-locked, and invalid item offers before lock; paired Gateway settlement checks both online recipients' free bag capacity before delivery; full-bag failures explicitly roll back both locked offers; partner cancel and disconnect restore escrowed gold/items; and successful paired confirmation keeps the existing Crystal trade packet surface while actually moving gold and item state between sessions. Verification passed locked focused Simulation Trade tests 12/12, locked Gateway shared-registry tests 15/15, Simulation/Gateway fmt, and locked Simulation/Gateway check.

> Latest sync: 2026-05-10 completed a bounded Taoist Hero owner-healing server slice. Hero AI now gives Taoist support a class-priority pass before ordinary hostile attack targeting, gates `Healing` through Crystal manifest level requirements, spends Hero MP with `ObjectMana`, emits Hero `ObjectMagic(Healing)`, restores the owner through `ObjectHealth`, and applies a cooldown gate to prevent immediate recast. Verification passed Hero AI integration 11/11, locked `mir2-simulation` check, and the later coordinator Simulation/Gateway fmt plus full Simulation regression pass.

> Latest sync: 2026-05-10 closed the current server-parity coordinator slice with a Crystal packet action-timing gate. `Walk`, `Run`, `Attack`, `RangeAttack`, and `Magic` now share modeled Crystal action readiness so repeated packets before the expected movement/attack/spell delay are corrected with `UserLocation` and do not produce duplicate action packets. The same close reconciles the bounded Archer Hero AI and Mail-Parcel worker slices: Archer Hero `Concentration` / `StraightShot` level gates, Hero MP and `ObjectMana`, `SetConcentration`, ranged `StraightShot` damage, serialized mail attachments, opened/locked mail state, remote account-store delivery, and exact parcel item claims are all covered by regressions. Verification passed focused action-timing tests, `magic_packet_crystal_` 73/73, `packet_` 280/280, Hero AI 9/9, Mail 9/9, full locked `mir2-simulation` 841/841 plus Hero AI 9/9, package fmt, locked four-package check, and targeted diff checks.

> Latest sync: 2026-05-10 added bounded Crystal mail parcel fidelity for player sends. The server runtime now consumes `SendMail` attachment unique IDs, stores exact serialized inventory item state on mail, validates recipient/item/cost before any sender mutation, keeps blacklist rejection behavior intact, removes sender gold/items on accepted sends, persists remote recipient parcels through account-store Stage 5 mail, exposes item previews plus opened/locked state in `ReceiveMail`, and claims the exact parcel item through `GainedItem` / `ParcelCollected`. Verification passed focused mail regressions 9/9, locked `mir2-simulation` check, and coordinator-reconciled package fmt.

> Latest sync: 2026-05-10 added a bounded Archer Hero class AI slice after the Warrior Hero skill surface. Server-side Hero AI now has private Archer skill/buff state, gates Crystal `Concentration` and `StraightShot` from the generated magic manifest, spends Hero MP and emits `ObjectMana`, emits `SetConcentration` once during the active buff window, and uses `StraightShot` on ranged `ObjectRangeAttack` with Crystal magic-level damage scaling. Verification passed Hero AI integration 9/9, locked `mir2-simulation` check, and coordinator-reconciled package fmt.

> Latest sync: 2026-05-10 completed Worker Agility's Crystal server import/application path for monster Agility. Monster DB generation now preserves `Stat.Agility` as `agility`, projects it to respawns as `monster_agility`, and the server runtime attaches `MonsterCombatStats.agility` to Crystal spawn-table entities, current-map imported visible monsters, revived respawns, and dynamic Crystal template summons/spawns. Focused evidence proves a nonzero-agility Crystal template spawned through the runtime path can miss without passive accuracy and hit with modeled `Fencing` / `SpiritSword` accuracy. Verification passed focused Simulation and game-data tests, JS syntax check, Rust fmt, and locked GameData/Simulation check. The local Mac workspace lacks `Server.MirDB`, so generated data refresh remains a Windows source-data follow-up.

> Latest sync: 2026-05-10 deepened Hero server combat with bounded Crystal Warrior skill semantics. Hero AI now gates modeled `Slaying` and `FlamingSword` by Crystal magic level requirements, emits those spells on Hero melee `ObjectAttack`, adds the Slaying passive DC bonus, and uses FlamingSword burst scaling for scheduled monster damage while preserving the prior carried-equipment stat projection. Verification passed Hero AI integration 7/7, `cargo +1.89.0 fmt --check -p mir2-simulation`, and `cargo +1.89.0 check --locked -p mir2-simulation`. Remaining server-parity risks are real Hero magic inventory/learning and MP/cooldown state, wider Hero class skill families, guild/storage/notice/member semantics, full multi-account settlement, and final client acceptance.

> Latest sync: 2026-05-10 closed the focused Crystal server gaps left by the previous multi-agent continuation. The Rust server path now applies Crystal `Fencing`, `Slaying`, and `SpiritSword` accuracy bonuses, equipment `Accuracy`, modeled monster `Agility`, miss `DamageIndicator` packets, passive melee skill progression, and accuracy-derived `MPEater` count/recovery. Hero carried equipment now affects Hero AI attack damage and is projected into `HeroInformation` stat/equipment payloads. Fishing now reads the actual slot-backed bait/hook/float/finder/reel items for cast/retry/reel/autocast behavior, including durability and broken-reel cancellation. Market packets now reject underbids and settle seller proceeds from the accepted bid minus 5% commission while preserving the Crystal `SoldItemEarningsCommission` surface. Verification passed focused passive accuracy 1/1, `magic_packet_crystal_` 73/73, Fishing 11/11, Market 1/1, Auction 6/6, Hero AI 5/5, full locked `mir2-simulation` 836/836 plus Hero AI 5/5, fmt, locked GameData/Protocol/Simulation/Gateway check, and targeted diff checks. Remaining server-parity risks are broad imported monster Agility coverage, deeper Hero equipment/skills, guild/storage/notice/member semantics, full multi-account market/mail settlement, and final client visual/dialog acceptance.

> Latest sync: 2026-05-10 continued Crystal server parity with focused player skill, Hero AI, Fishing, and social-economy slices. All generated Crystal magic-manifest spells are now explicitly classified or routed by the runtime, avoiding accidental generic damage behavior for passive/toggle skills. The modeled server path now emits Crystal-shaped melee/range packets for `Thrusting`, `FlamingSword`, `Slaying`, `Focus`, and incoming-hit `CounterAttack`, plus bounded passive effect/mana/poison/element updates for `FatalSword`, `MPEater`, `Hemorrhage`, and `Meditation`. Hero server ticks now cover bounded Attack/Follow/CounterAttack targeting, melee/ranged attack packet surfaces, scheduled monster damage, and chase fallback. Fishing server behavior now uses Crystal drop/event resolution, miss/no-space/gold paths, stat-based cast chance, reel durability proxy, and `GiantKeratoid` event spawn rather than only fixed loot. Mail sending now respects the sender's blocked-friend list with Crystal blacklist rejection. Verification passed focused `magic_packet_crystal_` regressions 72/72, Hero AI 3/3, Fishing 7/7, blacklist mail 1/1, full locked `mir2-simulation` 831/831 plus integration Hero AI 3/3, fmt/check, and targeted diff checks. Remaining server-parity risks are precise Crystal hit-rate/stat-passive math, Hero equipment/stat projection, true fishing slot-item fidelity, deeper market/guild settlement semantics, and final client visual/effect acceptance.

> Latest sync: 2026-05-10 advanced server parity for Hero map gates, shared ItemRental, rental lifecycle returns, and player/archer/Taoist/Wizard/stealth/control skills. Crystal `NoHero` map data is now modeled in game-data/config, Hero entities are unsummoned on no-hero transfers, and no-hero `NewHero` / `ChangeHero` attempts keep spawn state unsummoned with Crystal system messages. Hero inventory transfer/take-back/use now moves, persists, and consumes Hero-bag items, while Hero auto-pot consumes matching Hero inventory HP/MP potions and normalizes invalid item indexes like Crystal. Shared in-process ItemRental now goes beyond name resolution: Gateway queues the borrower invite, pairs borrower fee locks with lender item locks, commits lender gold plus rented-record state, delivers the rental-bound item with owner/expiry metadata to the borrower, and rolls back both sides when a partner cancels before confirmation. Simulation now also returns expired/death-triggered rental items through Stage 5 mail with exact `ItemState` payloads, cleared rental binding flags, `rental_locked=true`, and extended expiry, while deleting the borrower copy before normal drop paths. Player `SpellToggle` now rejects unlearned spells, persists modeled toggle state, handles `FlamingSword` MP consumption/state latch, applies Crystal `CounterAttack` buff type 18 with the expected AC/MAC stat payload, and cycles `MentalState` buff type 19 values while applying archer shot damage penalties. Repulsion-family skill surfaces now model `Repulsion` / `EnergyRepulsor` / `FireBurst` adjacent lower-level monster pushes through per-tile `ObjectPushed`, Crystal can-push gates, and ThunderElement's repulsion-only damage bypass. `StormEscape` now models its ThunderStorm-style nearby damage, successful target relocation, `ObjectEffect(StormEscape)`, and TemporalFlux buff type 1 with TeleportManaPenaltyPercent. Archer skill surfaces now model `Concentration` (`AddBuff` type 15 plus `SetConcentration` enable/disable), `ElementalShot` / `ElementalBarrier` Crystal element-orb gather/spend through `SetElemental` plus orb-boosted damage or buff type 25, `StraightShot` / `DoubleShot` one/two delayed ranged hits, `BackStep` real opposite-facing relocation with `UserBackStep` / `ObjectBackStep` and blocked distance-0 reporting, `BindingShot` (`SetBindingShot` plus short root window), `VampireShot` delayed damage/heal with visible buff type 16, `PoisonShot` delayed target damage plus Green poison with visible buff type 17, `CrippleShot` active special-arrow buff consumption with delayed `RemoveBuff` plus follow-up heal/area-poison effects, `NapalmShot` target-centered area damage, `DelayedExplosion` delayed marker/effect/removal plus target-area explosion, and `Trap` lower-level monster root plus Trap `ObjectSpell`. Wizard skill surfaces now model HellFire forward/level-3 side lanes, FireBang/IceStorm target 3x3 damage, Blizzard/MeteorStrike 5x5 ground spell spawn plus persistent damage, FireBounce chain projectiles/damage, MeteorShower primary plus secondary target damage, ThunderBolt undead bonus damage, ElectricShock lower-level shock root, FlameDisruptor non-undead bonus damage, and IceThrust three-column delayed damage plus Frozen poison packet state. Taoist skill surfaces now model `MassHealing` delayed 3x3 friendly healing, `HealingCircle` delayed `ObjectSpell` plus Crystal's 25-point heal tick, `Curse` amulet consumption with delayed hostile-area buff type 12 stat-rate penalties, `Purification` delayed Curse `RemoveBuff`, `Revelation` delayed target health reveal packets, `Poisoning` Green/Red poison item consumption with delayed `ObjectPoisoned` and monster poison-state projection, `PoisonCloud` amulet/GreenPoison consumption with 3x3 ground cloud ticks, `Plague` amulet/optional-poison consumption with 3x3 debuff/damage, and `TrapHexagon` amulet consumption with 3x3 hostile root plus eight delayed ring `ObjectSpell` packets. LightBody/MoonLight/DarkBody/Hiding/MassHiding now model buff types 8/13/14/2, Agility payloads, visible or hidden stealth buffs, and `ObjectHidden` hide/reveal lifecycle; FrostCrunch queues delayed magic damage plus a target freeze buff/root window, Vampirism queues delayed damage plus player healing, TurnUndead only damages undead targets with level-gated instant-kill behavior, EnergyShield applies Crystal buff type 20 with HP-gain/shield-percent stats, ImmortalSkin applies buff type 23 with defence/stat-tradeoff payloads, PetEnhancer buffs friendly/summoned monsters, LionRoar paralyses nearby lower-level monsters with `LRParalysis`, and BattleCry forces nearby hostile monsters to reacquire the caster. Verification passed focused Simulation Hero/NoHero/auto-pot tests, focused Simulation Hero inventory/auto-pot 25/25, ItemRental expiry/death/mail tests, focused skill toggle tests 6/6, casting 13/13, magic-packet Crystal skill tests 54/54 after adding Hiding/FrostCrunch/Vampirism/TurnUndead, EnergyShield/ImmortalSkin/PetEnhancer/LionRoar/BattleCry, MentalState/NapalmShot/DelayedExplosion/Trap/ExplosiveTrap/PoisonSword/PoisonCloud/Plague, and HellFire/FireBang/IceStorm/Blizzard/MeteorStrike/FireBounce/MeteorShower/ThunderBolt/ElectricShock/FlameDisruptor/IceThrust on top of FireWall/Lightning/ThunderStorm, Gateway shared registry tests 13/13, Rust fmt, and locked four-package GameData/Protocol/Simulation/Gateway check. Remaining server parity risks are full skill-family bespoke semantics, Hero combat/equipment AI, and final client visual/dialog/feel acceptance.
> Follow-up skill slices: `ShoulderDash` now uses Crystal dash/fail packets and target push, `FlashDash` uses dash-attack/fallback attack packets with delayed hit and Stun poison, and `SlashingBurst` uses `UserAttackMove` with delayed front-tile damage. `FireWall` now queues Crystal delayed cross-shaped `ObjectSpell` cells and persistent ground damage ticks, `Lightning` scans six tiles forward, `ThunderStorm` hits the current-location 5x5 square with non-undead damage reduction, and HellFire/FireBang/IceStorm/Blizzard/MeteorStrike/FireBounce/MeteorShower/ThunderBolt/ElectricShock/FlameDisruptor/IceThrust now have focused Wizard regressions. `Hiding`, `MassHiding`, `FrostCrunch`, `Vampirism`, `TurnUndead`, `EnergyShield`, `ImmortalSkin`, `PetEnhancer`, `LionRoar`, `BattleCry`, `MentalState`, `NapalmShot`, `DelayedExplosion`, `Trap`, `ExplosiveTrap`, `PoisonSword`, `PoisonCloud`, and `Plague` now have focused packet/state regressions.

> Latest sync: 2026-05-08 advanced the skill-system plus late-game deep-semantic parity track. `ObjectHero` now carries Crystal's owner-name payload through Protocol encode/decode, Gateway JSON, Web state, and world snapshots. Stage 5 Hero create/change/recruit now materializes a visible Hero entity with `ObjectHero` and `ObjectHealth`, blocks tiles like other actors, follows the player by emitting Hero `ObjectWalk`/`ObjectRun`, reports Crystal `UpdateHeroSpawnState` values (`Summoned=2`), routes default Hero `SpellToggle` packets to the spawned Hero, and keeps the frontend owner label stable across snapshots. Hero auto-pot state now appears in `HeroInformation` and `SetAutoPotValue` / `SetAutoPotItem` echo and persist Crystal settings. The modeled magic path now emits `ObjectProjectile` for Crystal projectile spells FireBall, GreatFireBall, ThunderBolt, and SoulFireBall, and MagicBooster emits a Crystal `AddBuff` payload with buff type 21, MinMC/MaxMC, and ManaPenaltyPercent. Verification passed focused Protocol Hero codec coverage, focused Simulation Hero 18/18, SpellToggle 2/2, MagicBooster 1/1, projectile skill coverage, locked Protocol/Simulation/Gateway fmt/check, and Web `npx tsc --noEmit`.

> Latest sync: 2026-05-08 closed the current smoke-blocking item identity/metadata edge cases on the server path. Runtime now normalizes loaded inventory/storage/belt unique IDs, rekeys dirty duplicate IDs when storage items are taken back, allocates collision-free IDs for crafted and QA-created items, and restores known potion HP/MP metadata during load and `qa.giveItem` seeding. This prevents reused demo saves from producing false Crystal item-action failures for `UseItem`, `SplitItem`, `DropItem`, `StoreItem`, `TakeBackItem`, and ground pickup flows. Verification passed focused Simulation `stage5_qa_give_item_seeds_usable_healing_metadata` 1/1, focused `unique_id` 13/13, locked Simulation/Gateway check, Rust fmt, Web typecheck/script syntax, and a full live Gateway/Web Stage 5 UI smoke with 114 screenshots and zero critical console errors.

> Latest sync: 2026-05-08 closed the current late-dialog command bridge gap for Hero, ItemRental, Creature, Mount, Fishing, and social System Menu actions. Player Web now sends the existing typed Gateway browser commands for Hero create/behaviour/change, ItemRental request/fee/period/cancel/list, IntelligentCreature update, Mount equipment use, and Fishing cast/autocast, and the fast Stage 5 smoke asserts those commands enter the browser command history rather than only opening static panels. Simulation snapshots now derive `stage5Systems.itemRental` from the live rental resource so request/fee/deposit/lock/confirm/cancel state is visible to the client. Verification passed Simulation item-rental regressions 3/3, Gateway command mapping 7/7, locked Simulation/Gateway check, Web typecheck/script syntax, and live local Gateway/Web smoke with 22 screenshots and 44 System Menu social states. Remaining server-parity rental risk is exact multi-account borrower/expiry/death semantics and final Crystal dialog acceptance, not packet command reachability.

> Latest sync: 2026-05-07 stabilized the server/runtime surfaces needed by the full Stage 5 browser smoke. `event.spawn` now uses a Crystal-resolvable default monster and places spawned monsters on nearby valid current-map tiles, preventing Crystal map smoke runs from logging zero spawned monsters; deterministic `qa.openNpcDialog` opens the real InnKeeper_Brittney script dialog; and the Web smoke harness can disable automatic tick traffic while preserving explicit command/tick verification. Verification passed focused Simulation and Gateway regressions, shared in-process registry 11/11, Rust fmt/check for Simulation/Gateway, Web typecheck/script syntax, full locked Gateway 107/107 plus packet-trace bin 17/17, full locked Simulation 731/731, and the live isolated-Gateway Stage 5 UI smoke with 102 screenshots and 0 critical console errors.

> Latest sync: 2026-05-07 added a focused service-backed equipped-repair parity slice used by the frontend smoke. Runtime `RepairItem` / `SRepairItem` now resolve equipped Crystal slot unique IDs before inventory unique IDs, so equipped weapon slot `0` is repairable even when bag slot `0` is occupied; normal repair applies the Crystal max-durability loss path, special repair preserves max durability, both require the active in-range repair service context, and `qa.damageEquipment` provides deterministic equipped-durability damage for UI verification. Verification passed focused Simulation repair/damage regressions, Rust fmt/check for Simulation/Gateway, Web typecheck/script syntax, `git diff --check`, and the live isolated-Gateway Stage 5 UI smoke with 101 screenshots and 0 critical console errors. This closes the previous frontend-observed gap where Character repair UI could be exercised without proving equipped durability/gold mutation.

> Latest sync: 2026-05-07 tightened the post-typing observability layer. Gateway/Web fallback packet events now serialize newly typed server packets as structured JSON fields instead of Debug-only summaries, and packet traces display typed server enum names for IDs that previously appeared as `Raw` through the legacy static-name fallback. Focused coverage now proves `NewMapInfo`, `Rankings`, unit typed packets, raw-display naming, and trace naming behavior. Game-data parity tests also now lock the generated NPC command and monster AI summaries at zero unimplemented command occurrences and zero remaining runtime priorities, so those old audit rows cannot silently reopen. Verification passed fmt/diff/check plus full locked GameData/Gateway/Protocol/Simulation tests: GameData 27/27, Gateway lib 105/105 plus packet-trace bin 17/17, Protocol lib 33/33 plus codec 33/33, and Simulation 722/722.

> Latest sync: 2026-05-07 completed full typed Crystal server-packet payload coverage. All 279 Crystal server IDs `0..278` now decode through explicit typed `ServerPacket` variants rather than falling through to Raw; the final pass added map/world-map/search/user-slot refresh, player update/inspect/status/damage/death/poison/map-change, guild status/member/notice/storage/war, auto-pot, NPC image/input/pearl goods, quest inventory, reincarnation, dash/attack-move/concentration/elemental, awakening material, transform, game-shop stock, ranking, notice, and guild territory payloads. Local scan evidence: `explicit=279 remaining=0`. Verification passed protocol round-trip coverage for the new payloads, `cargo +1.89.0 fmt --check -p mir2-protocol -p mir2-gateway`, locked `cargo +1.89.0 check --locked -p mir2-protocol -p mir2-gateway -p mir2-simulation`, and full locked tests covering Gateway lib 104/104 plus packet-trace bin 17/17, Protocol lib 32/32 plus codec 33/33, and Simulation 722/722. This supersedes prior wording that treated complex guild/listing/ranking/map payloads as still-Raw server gaps; remaining parity work is exact behavior/state/client acceptance behind the typed packets.

> Latest sync: 2026-05-07 completed the next P1/P2 packet-runtime closure. Protocol/Gateway now type, name, serialize, and regression-test Crystal Group utility, Quest, and Refine server packets; Simulation now gives the matching client packets stateful modeled behavior for group membership/toggle, quest accept/finish/abandon/share, market consign/buy/get-back/sell-now, refine deposit/retrieve/cancel/start/check, `OpenDoor`, and map/monster/NPC info requests backed by generated Crystal manifests. Verification passed focused regressions, Rust fmt/check, Web typecheck, fast Stage 5 UI smoke, and the full locked Protocol/Gateway/Simulation regression: Gateway lib 103/103 plus packet-trace bin 17/17, Protocol lib 29/29 plus codec 32/32, and Simulation 722/722. This supersedes the older gap wording that treated these packet families as empty/no-op surfaces; remaining server parity depth is exact market page/list payloads, refine timers/probabilities/material economics, auction bid/commission settlement, exact Quest Diary client acceptance, and broader human visual/feel acceptance.

> Latest sync: 2026-05-07 completed the P1/P2 exact-gate pass on top of the multi-agent late-system closure. Server observability now preserves Raw payload replay details in Web events and packet traces; IntelligentCreature now uses Crystal default rules and pickup filters/modes with correct blackstone-vs-fullness behavior; Fishing now has rod/bait/hook/reel/cell/durability gates plus reel/autocast regression coverage; Mount now enforces map `NoMount`, `NeedBridle`, saddle, and reins state, with map generator/game-data support for those Crystal flags. Verification passed focused Protocol, Simulation, and Gateway regressions plus Rust fmt/check/diff gates, Web typecheck, script syntax checks, live Stage 5 UI smoke with 83 screenshots and 0 critical console errors, and full locked package tests: GameData 27/27, Gateway lib 100/100 plus packet-trace bin 17/17, Protocol lib 26/26 plus codec 32/32, and Simulation 716/716. This supersedes the prior wording that treated creature filters, fishing rod/bait/durability gates, mount map gating, and Raw replay fields as open P1/P2 server gaps; remaining server parity risk is deeper exact behavior tuning and final client visual/dialog acceptance.

> Latest sync: 2026-05-07 completed the current multi-agent late-system backend closure. The modeled server path now includes shared two-account Trade item/gold delivery plus cancel/disconnect rollback, IntelligentCreature automatic pickup with fullness and blackstone ticking, Fishing loot/reel/autocast progression, equipped Mount ride toggling, Hero create/change/behaviour state, and Gateway Web/packet-trace exposure for the unique-id/equipment/trade paths. Verification passed through `cargo +1.89.0 fmt --check -p mir2-protocol -p mir2-simulation -p mir2-gateway`, `cargo +1.89.0 check --locked -p mir2-protocol -p mir2-simulation -p mir2-gateway`, and full `CARGO_CACHE_AUTO_CLEAN_FREQUENCY=never cargo +1.89.0 test --locked -p mir2-protocol -p mir2-simulation -p mir2-gateway -- --test-threads=1` covering Gateway lib 99/99 plus packet-trace bin 16/16, Protocol lib 25/25 plus codec 32/32, and Simulation 711/711. This supersedes the previous server-parity gap wording for Trade partner delivery/rollback and IntelligentCreature fullness/blackstone/automatic pickup; remaining server-side parity risk is exact Crystal semantic tuning and client dialog acceptance, not those missing flow closures.

> Latest sync: 2026-05-06 completed the current stateful IntelligentCreature backend slice. Stage 5 now stores intelligent-creature payloads, `UpdateIntelligentCreature` handles create/update/summon/unsummon/release, new creatures emit `NewIntelligentCreature`, list refreshes report actual creature rows and summoned type, and active creatures can pick up targeted ground drops through `IntelligentCreaturePickup` while preserving normal gain packets. Verification passed through focused creature update/pickup coverage, locked fmt/check for Protocol/Simulation/Gateway, and full package validation with Gateway lib 96/96 plus packet-trace bin 16/16, Protocol lib 25/25 plus codec 32/32, and Simulation 708/708. This closes the modeled list/update/pickup slice; fullness, feeding, blackstone timers, exact automatic pickup filter/range behavior, visible pet movement, and final client dialog acceptance remain separate parity work.

> Latest sync: 2026-05-06 completed the current stateful Trade backend slice. Shared in-process Gateway sessions now use an adjacent remote player for `TradeRequest`, and Simulation now models the Crystal trade packet flow with Stage 5 state for partner, offered gold, offered trade slots, accept/lock/complete flags, deposit/retrieve item packets, trade item echo payloads, confirm-time gold deduction and offered-item removal, and cancellation cleanup. Verification passed through focused Simulation trade packet tests, adjacent Stage 5 trade command regressions, Gateway adjacent-player trade request coverage, locked fmt/check for Protocol/Simulation/Gateway, and full package validation with Gateway lib 96/96 plus packet-trace bin 16/16, Protocol lib 25/25 plus codec 32/32, and Simulation 708/708. This closes the modeled one-sided Trade packet loop; true partner inventory/gold delivery, disconnect rollback after both-side offers, and final client dialog acceptance remain separate parity work.

> Latest sync: 2026-05-06 completed the current stateful Mail/Friend backend slice. Mail client packets now operate against Stage 5 mailbox state: sends validate cost and unsupported attachment ids, deduct gold, persist a mail row, return `MailSent`, and refresh `ReceiveMail`; reads, parcel collection, deletion, lock refresh, and mail cost now return Crystal-shaped packets sourced from mailbox state, including `GainedGold` and `ParcelCollected` on parcel claim. Friend packets now operate against persisted Stage 5 social state: add/remove/refresh/memo updates return `FriendUpdate` with Crystal `ClientFriend` rows. Verification passed through focused and adjacent mail/social tests, locked fmt/check for Protocol/Simulation/Gateway, and full package validation with Gateway lib 95/95 plus packet-trace bin 16/16, Protocol lib 25/25 plus codec 32/32, and Simulation 707/707. This closes the bounded empty Mail/Friend surface for the modeled backend state; exact live item attachments, persistent lock/reply behavior, online fanout, and final client dialog acceptance remain separate parity work.

> Latest sync: 2026-05-06 completed full Crystal packet-ID coverage for the Rust protocol boundary. Client IDs `0..152` are known and all 153 client packets have typed `ClientPacket` variants; server IDs `0..278` are known and decode either to typed `ServerPacket` variants or to Raw-preserved payloads for complex server packets whose semantics are not yet fully modeled. The Crystal item-combine ID correction is locked by tests: client `CombineItem=110`, `AwakeningNeedMaterials=111`, server `CombineItem=214`, and `ItemUpgraded=215`. The server typed packet set also now includes visual and late-system packets around projectiles, range attack, pushes, map effects, observe, paused buffs, hidden state, dash/fail dash, delayed explosion cleanup, deco/sneak/level effects, binding shot, output messages, awakening NPC dialogs/results/locked items, and inventory resize. Gateway Web serializes these packets as browser events and packet trace names them directly. Verification passed: focused protocol coverage and round-trip tests, locked `fmt` and `check` for Protocol/Simulation/Gateway, and full package validation with Gateway lib 95/95 plus packet-trace bin 16/16, Protocol lib 25/25 plus codec 32/32, and Simulation 707/707. This closes missing packet IDs and silent decode loss as server-parity risks; remaining server work is semantic depth for still-Raw payload families and multi-actor gameplay completion, not packet-table coverage.

> Latest sync: 2026-05-06 completed a late-system Crystal packet-surface expansion across Trade, Fishing/Mount, Mail/Friend, and IntelligentCreature. Protocol now has typed Crystal IDs/codecs/trace names for the audited client and server packet families, including `ClientMail`, `ClientFriend`, `ClientIntelligentCreature`, intelligent-creature rules/filter payloads, fishing/mount update payloads, and trade item option lists. Gateway Web can drive the new client commands and expose the new server packets as browser events, while `packet_trace` reports useful details for the packet families. Simulation keeps the current backend honest by returning Crystal-shaped bounded surfaces where full persistent systems are not yet implemented: no-partner trade no-ops/failure acks, mail locked-item echo and cost/failure responses, empty friend/mail/intelligent-creature update lists, and fishing update toggles. Verification passed with focused regressions and full package validation: Gateway lib 91/91 plus packet-trace bin 16/16, Protocol lib 15/15 plus codec 32/32, Simulation 705/705, locked `check`, and `fmt --check`. This reduces Raw packet exposure and locks client/server packet semantics, but does not claim full gameplay closure for multi-player trade, real mail/friend persistence, pet lifecycle/pickup, fishing rewards/durability, mount progression, or final client-dialog acceptance.

> Latest sync: 2026-05-06 completed the current Crystal item-rental packet/runtime parity slice. Protocol now has Crystal IDs and codecs for `GetRentedItems`, `ItemRentalRequest`, `ItemRentalFee`, `ItemRentalPeriod`, `DepositRentalItem`, `RetrieveRentalItem`, `CancelItemRental`, `ItemRentalLockFee`, `ItemRentalLockItem`, `ConfirmItemRental`, `UpdateRentalItem`, `ItemRentalLock`, `ItemRentalPartnerLock`, and `CanConfirmItemRental`, backed by `ItemRentalInformation` and persisted rental metadata on `UserItem` payloads. Simulation now accepts the Crystal rental packets, validates deposit/period/fee state, enforces the Crystal rental binding restrictions, emits the request/fee/period/deposit/retrieve/cancel/lock/confirm response packets, records confirmed lender-side rental entries, and restores those records after save/reload. Gateway Web can drive and inspect those packet surfaces, and the shared in-process registry uses nearby remote players for `ItemRentalRequest` partner resolution. Verification passed through focused rental regressions, full Protocol lib 7/7 plus codec 32/32, full Gateway lib 83/83 plus packet-trace bin 16/16, full Simulation 701/701, and locked fmt/check. This closes the packet/runtime rental slice; real cross-account borrower delivery, rental expiry return/mail behavior, death-return handling, and final client dialog acceptance remain separate parity tasks.

> Latest sync: 2026-05-06 completed the current Crystal magic/buff packet parity slice. Protocol now has Crystal IDs and codecs for `MagicKey`, `Magic`, `SpellToggle`, `NewMagic`, `RemoveMagic`, `MagicLeveled`, `Magic`, `MagicDelay`, `MagicCast`, `ObjectMagic`, server `SpellToggle`, `ObjectMana`, `AddBuff`, and `RemoveBuff`, backed by the expanded Crystal `Spell` enum plus `ClientMagic`, `ClientBuff`, and `ObjectManaInfo` payload shapes. Simulation now accepts real `ClientPacket::Magic` from Crystal clients, applies modeled skill effects when available, supports manifest-backed casts when a spell exists in Crystal data, emits `UserLocation` rejection fallback plus successful `ObjectMana`/`Magic`/`ObjectMagic`, tracks `MagicKey` hotkeys, acknowledges `SpellToggle`, teaches books with `NewMagic`, sends `AddBuff`/`RemoveBuff` for Crystal-mapped buff states, updates mana for casts and potions, and progresses `UserMagic` through `MagicLeveled` / `MagicDelay`. Gateway Web and packet trace expose the same packet commands/events for QA. Verification passed through focused Protocol/Simulation/Gateway regressions, packet-trace flow-name coverage, Player Web `npx tsc --noEmit`, locked fmt/check for those packages, `git diff --check`, and full Protocol/Simulation/Gateway tests: Gateway lib 82/82 plus packet-trace bin 16/16, Protocol lib 5/5 plus codec 32/32, and Simulation 698/698. Exact per-spell combat/damage/AI edge cases remain in the broader skill and monster-behavior parity queue.

> Latest sync: 2026-05-06 completed Crystal server console parity for the Rust/Admin operating surface. The WinForms `SMain` control classes mapped into Admin API/Gateway/Admin Web coverage: account create/update/delete/unban/storage-password clear, character rename/stat/currency/location/vital/PK edits, chat ban apply/clear, safe-zone return, kill player, kill pets, NPC flag set/clear, GM direct message, broadcast, market listing cancel/expire/delete, guild member/message moderation, NameLists create/add/remove/delete, generated-content read models, content override bundle publish, server status/reload/control, and session-cache reads. High-risk mutations stay behind approval IDs and all commands enter the audit timeline. Runtime persistence now carries Crystal PK/chat-ban fields and auction expired state, with focused gameplay regressions for chat-ban enforcement and expired-auction rejection. Verification passed through full Simulation tests, full Admin API tests, focused Gateway admin endpoint test, locked Rust check, Admin Web typecheck/build, live HTTP mutation/readback smoke, SSR page probes, and Playwright snapshots. This is operational/admin parity, not a change to the R300 gameplay packet acceptance decision.

> Latest sync: R315 corrected real new-character inventory/equipment/storage/quest/skill/gold defaults to Crystal-empty behavior. `NewCharacter` saves now start with no Web demo bag, belt, storage, equipment, quest, or skill state and `gold=0`; empty save arrays load as explicit empty; exact old level-1 Web seed saves migrate to empty; and `demo/Scout` remains seeded for Stage 5 automation. Focused tests cover new Crystal character state, legacy level-1 seed migration, and default demo preservation. R315 browser evidence for `QA0429A / QA0429Hero` records zero counts for inventory, belt, storage, equipment, quests, and skills with `gold=0`. Verification passed: focused `mir2-simulation start_game_` 16/16, `cargo +1.89.0 build --locked -p mir2-gateway`, web `tsc --noEmit`, R315 capture, `cargo +1.89.0 fmt --check`, and capture-script `node --check`. Server-only accepted packet status remains R300 stable-diff; exact frontend panel bitmap parity remains open.

> Latest sync: R314 corrected the modeled starter/default character vitals to match Crystal `Shared/BaseStats.cs` calculations. Runtime default resources, new/default character saves, and legacy saves matching the former hardcoded `120/120/45` values now use class/level Crystal HP/MP formulas. Focused tests cover the current level-7 Warrior default (`60/60` HP, `35` MP) and a migrated level-1 Warrior (`18/18` HP, `14` MP); browser evidence for `QA0429A / QA0429Hero` records the Crystal HP-only HUD label `HP 18/18`. Verification passed: focused `mir2-simulation start_game_` 15/15, `cargo +1.89.0 build --locked -p mir2-gateway`, web `tsc --noEmit`, R314 capture, `cargo +1.89.0 fmt --check`, and `git diff --check`. Server-only accepted packet status remains R300 stable-diff; full frontend visual 1:1 remains open.

> Latest sync: R310 added `questIds` to server world entity snapshots for frontend Crystal marker scoping. This lets the Web client avoid drawing quest icons on unrelated NPCs while preserving the current backend packet acceptance state. Verification passed: `cargo fmt --check` and focused `mir2-simulation crystal_current_map_transfer_spawns_visible` 2/2. Server-only accepted packet status remains R300 stable-diff; full frontend visual 1:1 remains open.

> Latest sync: R305 completed for current-map visible Crystal respawn runtime population. Visible Crystal respawns now populate the ECS/worldSnapshot for saved-character start and Crystal transfers, instead of appearing only as bootstrap `ObjectMonster` packets. Evidence at `docs/generated/player-qa/r305-bichon-visible-world-snapshot.json` records `QA0429A / QA0429Hero` at `BichonProvince` map `0`, `284,607` with `entityCount=17`, `npcCount=8`, `monsterCount=8`, including `Deer` and `Royal_Guard`; browser state evidence records 8 monster sprite elements. Verification passed: focused R305 regression, visible-respawn density regression, `cargo +1.89.0 fmt --check`, `cargo +1.89.0 build --locked -p mir2-gateway`, live WS probe, browser capture, gateway health, and web HTTP 200. Server-only accepted packet status remains R300 stable-diff; full frontend visual 1:1 remains open.

> Latest sync: R304 completed for current-map Crystal NPC runtime population. Saved-character `StartGame` and Crystal transfer paths now repopulate the current Crystal map with NPC-info manifest entries before visible-object packets/snapshots are emitted. Live WS evidence at `docs/generated/player-qa/r304-bichon-npc-world-snapshot.json` confirms `QA0429A / QA0429Hero` at `BichonProvince` map `0`, `284,607` has `npcCount=8`, including `Assistant_Jane` and `Merchant_Ruben`. Verification passed: focused R304 simulation regression, adjacent `transfer_map`, `start_game_emits_visible_object_packets`, `world_snapshot_marks_safe_zone_after_start_game`, `cargo +1.89.0 fmt --check`, `cargo +1.89.0 build --locked -p mir2-gateway`, and a live WS probe. Server-only accepted packet status remains R300 stable-diff; full frontend visual 1:1 remains open.

> Latest sync: R301 refreshed the final automated Candidate acceptance pack after the R300 backend/server packet acceptance decision. Server-only status remains **100% Accepted under explicit stable-diff packet acceptance** for the tracked backend/server slice; strict exact remains diagnostic. R301 verification passed without Docker: packet-trace bin 15/15, `mir2-game-data` 27/27, `mir2-gateway` 55/55 plus packet-trace bin 15/15, `mir2-admin-api` 22/22, `mir2-simulation` 674/674, web `tsc --noEmit`, web build, map API smoke 18/18, minimap smoke 0 failures with known 450/451 warning, WS load 64/64 ready with 0 errors, and Stage 5 UI smoke 88 screenshots with 0 critical console errors. Evidence summary: `docs/generated/player-qa/r301-summary.json`.

> Latest sync: R300 completed the backend/server packet acceptance decision. The current tracked backend/server packet matrix is **100% Accepted under the explicit stable-diff policy**: R298 provides 9/9 local OK, 9/9 Crystal OK, `crystalMissingCount=0`, `stableDiffCleanCount=9`, and `acceptedStableLiveComparisonCount=9`; R299 proves strict exact dirtiness is Crystal dynamic state (object IDs, login timestamps, role lifecycle index, AOI order/payload, dynamic NPC state); and R300 wires `packet_trace` to enforce accepted parity with `MIR2_PACKET_TRACE_ACCEPT_STABLE_DIFF=1`. Strict exact remains a diagnostic comparator until a deterministic Crystal fixture controls those volatile fields. Verification: `cargo +1.89.0 test --locked -p mir2-gateway --bin packet_trace -- --test-threads=1` passed 15/15, `cargo +1.89.0 fmt --check` passed, and `docs/PACKET-PARITY-ACCEPTANCE.md` plus `docs/generated/packet-traces/r300-stable-acceptance.json` record the acceptance decision.

> Previous sync: R298 completed on Windows for live Crystal stable packet evidence refresh. `packet_trace --matrix` ran against local gateway `127.0.0.1:7310` and Crystal `127.0.0.1:7000` using the full client resource root `E:\mir2\Crystal\Build\Client\Debug` and fixture character `Cdx0428030348` index `8`. Artifact `docs/generated/packet-traces/r298-live-matrix/latest-matrix.json` recorded 9/9 local OK, 9/9 Crystal OK, `crystalMissingCount=0`, `stableDiffCleanCount=9`, and `acceptedStableLiveComparisonCount=9` after treating `TimeOfDay` as stable-comparator volatile. The strict exact diff remains dirty (`diffDirtyCount=9`, `acceptedLiveComparisonCount=0`), which R300 now treats as diagnostic rather than blocking for this accepted stable-diff packet gate. Validation passed without Docker: `mir2-simulation` 674/674, `mir2-gateway` 55/55 plus packet-trace bin 14/14, `mir2-admin-api` 22/22, `cargo +1.89.0 fmt --check`, `git diff --check`, and web `tsc --noEmit`.

> Latest sync: R297 completed on Windows for refreshed automation around the live-parity baseline. The backend-adjacent changes were limited to concurrent JSON account-store save hardening, gateway `MapInformation` minimap/big-map index exposure, and WS load alignment with Crystal-style empty accounts. Validation passed without Docker: `mir2-simulation` 674/674, `mir2-gateway` 55/55 plus packet-trace bin 14/14, `mir2-admin-api` 22/22, `cargo +1.89.0 fmt --check`, `git diff --check`, web `tsc --noEmit`, web build, WS load 64/64 ready with 0 errors, map API smoke 18/18, minimap smoke 0 failures with known 450/451 warning, and Stage 5 UI smoke 88 screenshots with 0 critical console errors. R300 later closed the backend/server packet gate through explicit stable-diff acceptance; human visual/feel acceptance remains outside this server-only document.

> Previous sync: R292 completed on Windows for live Crystal stable packet evidence. `packet_trace --matrix` ran against local gateway `127.0.0.1:7310` and Crystal `127.0.0.1:7000` using the full client resource root `E:\mir2\Crystal\Build\Client\Debug` and fixture character `Cdx0428030348` index `8`. Artifact `docs/generated/packet-traces/r292-live-matrix/latest-matrix.json` recorded 9/9 local OK, 9/9 Crystal OK, `crystalMissingCount=0`, `stableDiffCleanCount=9`, and `acceptedStableLiveComparisonCount=9`. The strict exact diff remained dirty (`diffDirtyCount=9`, `acceptedLiveComparisonCount=0`), which R300 later classifies as diagnostic under stable-diff packet acceptance. Validation passed: `mir2-simulation` 674/674, `mir2-gateway` 55/55 plus packet-trace bin 14/14, `mir2-admin-api` 22/22, `cargo +1.89.0 fmt --check`, `git diff --check`, and web `tsc --noEmit`.

> Latest sync: R248 completed on Windows for the Crystal server-data import gate. `generate-crystal-respawn-manifest.mjs` regenerated Crystal respawn/monster/item/NPC-info manifests from local `E:\mir2\Crystal\Build\Server\Debug\Server.MirDB` plus `E:\mir2\Crystal\Build\Server\Debug\Envir\Routes`, and map records now include real `NoThrowItem`, `NoDropPlayer`, and `NoDropMonster` flags. Validation passed: `mir2-game-data` 22/22, focused `no_drop_monster_map_rule` 2/2, full `mir2-simulation` 670/670, and `mir2-gateway` 55/55 plus packet-trace bin tests 7/7. R300 later closed the remaining backend packet gate through explicit stable-diff acceptance.

> Previous sync: R225 completed on the integration/global Candidate track; backend/server gameplay code was unchanged from R183 and remained green. `mir2-gateway` passed 54/54 including packet trace bin tests 7/7 after adding matrix summary coverage, and require-local matrix evidence wrote 9/9 TCP-traceable artifacts with `localOk=true` under `docs/generated/packet-traces/r225-matrix`. At R225 time backend/server tracked-slice parity estimate was 99.70%; R300 later closed the packet gate under explicit stable-diff acceptance.

> Previous sync: R224 completed on the integration/global Candidate track; backend/server gameplay code was unchanged from R183 and remained green. The `mir2-gateway` packet trace bin target was restored, `mir2-gateway` passed 53/53 including packet trace bin tests 6/6, and require-local matrix evidence wrote 9/9 TCP-traceable artifacts with `localOk=true`. Later Windows rounds closed the server-data import gate and accepted the stable-diff live packet gate.

> Latest sync: R219-R222 completed on the frontend/global track; backend/server code is unchanged from R183. R183 left runtime interaction quest hints in the UI namespace `custom.interaction.questHint`, synchronized generated localization, and left no `sim.*` references in `apps/simulation/src/runtime.rs`. Latest backend validation remains `mir2-game-data` 22/22, focused simulation 1/1, `cargo +1.89.0 fmt --check`, and full `mir2-simulation` 664/664. Active backend/global round is R223; backend/server parity estimate is 99.70%, whole-project 1:1 estimate is 90.0%.


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


This document tracks the Rust backend migration against `E:\mir2\Crystal\Server`.

Scope note: this is backend/server parity only. Whole-project 1:1 progress, including frontend UI, assets, integration, and playability, is tracked in `docs/CRYSTAL-1TO1-ROADMAP.md`.

Progress tracker: `docs/BACKEND-1TO1-PROGRESS.md`

Current backend/server tracked-slice parity status: **100% Accepted under explicit stable-diff packet acceptance**.

Backend rounds:
- Completed rounds: **R82**, **R83**, **R84**, **R85**, **R86**, **R87**, **R88**, **R89**, **R90**, **R91**, **R92**, **R93**, **R94**, **R95**, **R96**, **R97**, **R98**, **R99**, **R100**, **R101**, **R102**, **R103**, **R104**, **R105**, **R106**, **R107**, **R108**, **R109**, **R110**, **R111**, **R112**, **R113**, **R114**, **R115**, **R116**, **R117**, **R118**, **R119**, **R120**, **R121**, **R122**, **R123**, **R124**, **R125**, **R126**, **R127**, **R128**, **R129**, **R130**, **R131**, **R132**, **R133**, **R134**, **R135**, **R136**, **R137**, **R138**, **R139**, **R140**, **R141**, **R142**, **R143**, **R144**, **R145**, **R146**, **R147**, **R148**, **R149**, **R150**, **R151**, **R152**, **R153**, **R154**, **R155**, **R156**, **R157**, **R158**, **R159**, **R160**, **R161**, **R162**, **R163**, **R164**, **R165**, **R166**, **R167**, **R168**, **R169**, **R170**, **R171**, **R172**, **R173**, **R174**, **R175**, **R176**, **R177**, **R178**, **R179**, **R180**, **R181**, **R182**, **R183**
- Active round: **R305**
- Full-suite regression status: **674/674** passing after R301; R305 focused/adjacent `mir2-simulation` tests and `mir2-gateway` build passing; latest `mir2-gateway` **55/55** plus packet-trace bin **15/15** passing after R301; `mir2-admin-api` **22/22** passing after R301; `mir2-game-data` **27/27** passing after R301
- Historical Web/Stage5 automated-evidence status: **100.0% Candidate**. This
  does not include formal WN-CANDIDATE signing/attestation, native 30-minute
  client soak, real OS-DPI, independent-model review, or human acceptance.
- Whole-project real accepted 1:1 estimate: **roughly 90.0%**

## Current Migration Target

The Rust backend is considered "done" only when these areas are functionally aligned with Crystal:

1. world runtime and map rules
2. player movement, combat, death, revive
3. monster AI, respawn, drops
4. inventory, equipment, item use, repair
5. skills, buffs, cooldowns, summons
6. NPC scripting and quests
7. map transfer, AOI, safe zones, world events
8. persistence and reconnect-safe state
9. protocol behaviour parity

## Crystal Source Areas

- `MirEnvir`
  - map loading, world ticking, respawn timers, route logic
- `MirObjects`
  - player, monster, NPC, item, spell, hero object behaviour
- `MirDatabase`
  - map, monster, NPC, quest, respawn, buff, character persistence models
- `MirNetwork`
  - connection/session flow
- `Helpers`
  - shared behaviour such as chat flow

## Rust Status

### Done

- `R301` refreshed the final automated Candidate acceptance pack and reverified the current backend/server packages around the R300 packet acceptance decision. Evidence is summarized in `docs/generated/player-qa/r301-summary.json`; server-side verification passed `mir2-game-data` 27/27, `mir2-gateway` 55/55 plus packet-trace bin 15/15, `mir2-admin-api` 22/22, and `mir2-simulation` 674/674.
- `R300` closed the current tracked backend/server packet parity gate by explicitly accepting the stable live comparator as the packet acceptance criterion. The harness now distinguishes strict exact diagnostics from accepted packet parity, supports `MIR2_PACKET_TRACE_ACCEPT_STABLE_DIFF=1`, records `acceptanceMode`, `acceptedPacketParityCount`, and `packetParityAccepted`, and preserves strict exact diff as a diagnostic until deterministic Crystal volatile-state fixtures exist.
- `R248` closed the Windows real-data import gate for the current Crystal map/drop-rule slice: generated respawn/monster/item/NPC-info manifests were refreshed from local `Server.MirDB` plus matching route files, map records now include real `NoThrowItem`, `NoDropPlayer`, and `NoDropMonster` flags, and existing manifest-backed `NoThrowItem` / `NoDropMonster` runtime behavior was reverified.
- `R183` moved the remaining runtime interaction quest hint out of the `sim.*` namespace into `custom.interaction.questHint`, synchronized importer/generated localization bundles, and left no `sim.*` references in `apps/simulation/src/runtime.rs`.
- `R182` removed the runtime-only no-script/no-page NPC idle dialog fallback, matching Crystal `NPCScript.Call` no-response behavior when no page is found.
- `R181` routed quest-required drop feedback through Crystal `server.YouFound` and removed runtime-only quest drop/progress chats while preserving `GainedItem` and quest state updates.
- `R180` localized `StartGame` welcome chat through Crystal `server.Welcome` with localized `server.GameName` and `ChatType::Hint`, replacing runtime-only `sim.welcomeCharacter` System text.
- `R179` removed runtime-only normal chat self echo: chat before `StartGame` is silent, and in-game normal chat emits only Crystal-shaped `ObjectChat` with `Name: message`.
- `R178` removed runtime-only cast-skill failure chats from high-level unknown-skill, cooldown, unwired-definition, missing-player, no-MP, unwired summon-spell, and missing summon-template branches while preserving successful buff/summon behavior.
- `R177` removed runtime-only `MoveItem` unsupported-grid/missing-source fallback chat while preserving failed-ack-only unsupported grids and Crystal `server.ItemMoveErrorReport` for Inventory/Storage.
- `R176` removed runtime-only stale active-dialog missing-NPC/no-script chats while preserving ordinary no-script NPC idle fallback.
- `R175` removed runtime-only NPC dialog helper no-active/invalid-target/no-pending-input chats while preserving successful dialog link/input/service flows.
- `R174` removed runtime-only direct NPC interaction invalid target/direction/range chats while preserving successful NPC dialog/script/service flows.
- `R173` removed runtime-only direct attack invalid target/state/range chats while preserving normal attack, turn, hidden reveal, Zuma wake, and delayed hit surfaces.
- `R172` removed runtime-only `sim.talkingToNpc` from successful high-level NPC interaction while preserving NPC `ObjectChat`/dialog packet surfaces and Crystal NPC script/service flows.
- `R171` removed runtime-only direct pickup invalid target/distance chats (`sim.itemNoLongerOnGround`, `sim.targetNotGroundDrop`, `sim.moveCloserToPickItem`) while preserving Crystal owner/full-bag pickup messages and current-cell packet pickup behavior.
- `R170` removed runtime-only `sim.defeatedMonsterEntityMissing` from missing defeated-monster entity handling while preserving normal death/drop packet surfaces.
- `R169` removed runtime-only monster death-drop success chats (`sim.monsterDroppedGoldOnGround` / `sim.monsterDroppedItem`) while preserving ground drop creation, quest-drop routing, owner windows, and pickup packet surfaces.
- `R168` removed runtime-only `sim.targetDefeated` from summoned VampireSpider death explosion while preserving explosion damage and summon despawn behavior.
- `R167` removed runtime-only ordinary combat damage narration from player/monster hit resolution while preserving packet health/struck/death surfaces and Trainer DPS reporting.
- `R166` removed runtime-only generic `sim.castSkill` success chat from buff/heal and summon `cast_skill` success paths while preserving state mutation and spawns.
- `R165` removed runtime-only pre-start cast-skill helper chat from high-level `cast_skill`, leaving silent no-packet rejection before `StartGame`.
- `R164` removed runtime-only pre-start interaction helper chats from high-level `interact` and dialog target follow-up, leaving silent no-packet rejection before `StartGame`.
- `R163` removed runtime-only pre-start harvest helper chats from high-level `harvest` and packet `Harvest`, leaving silent no-packet rejection before `StartGame`.
- `R162` removed runtime-only pre-start attack helper chats from high-level `attack` and packet `Attack` and `RangeAttack`, leaving silent no-packet rejection before `StartGame`.
- `R161` removed runtime-only pre-start movement/turning helper chats from high-level `move_to` and packet `Walk`, `Run`, and `Turn`, leaving silent no-packet rejection before `StartGame`.
- `R82` completed item-use parity now enforces manifest-backed `RequiredGender` and `RequiredClass` restrictions, applies `RequiredType == Level` level gating from item requirements, blocks repeat skill-book learn retries, and confirms successful skill-book learn mutation with scroll consumption.
- `R83` completed item-use parity for remaining manifest-backed shapes now covers `AncientBanga[Green]` and `AncientBanga[Purple]` via scroll shape 8/9, including `free_map_shout` / `free_server_shout` set behavior, Crystal hint-chat emission, and localized credit-token hint behavior (`server.CreditsAddedToAccount`).
- `R84` completed item-use parity for manifest-backed scroll shapes `26/27` on `GtInvite` and `GTTeleport`: `UseItem` now succeeds without active-effect branching, consumes one scroll, emits success ack only, and does not emit chat, `UserLocation`, or teleport side effects.
- `R85` completed item-use `CanUseItem` parity expansion beyond `R82` level checks by enforcing modeled stat and level gates (`MaxAC`, `MaxMAC`, `MaxDC`, `MaxMC`, `MaxSC`, `MinAC`, `MinMAC`, `MinDC`, `MinMC`, `MinSC`, `MaxLevel`) from `Crystal/Server/MirObjects/HumanObject.cs::CanUseItem` and existing equipment/buff totals; focused regressions for low/high equipment requirements now pass.
- `R86` completed current manifest-backed `UseItem` scroll-shape `0/2` parity for `DungeonEscape` / `TeleportHome` and `RandomTeleport`: same-map occupied destinations now search current map tiles before mutating; success consumes one scroll, emits `UseItem` success ack plus location/map refresh; failures emit failed ack only and preserve inventory and state.
- `R87` completed current manifest-backed `ItemType.Food` branch for `RawMeat` and `LeanMeat`: requires equipped mount, fails/does not mutate when no mount is equipped or mount dura is full, successful feeds consume one meat and emit `server.MountFed` and `ItemRepaired`, `RawMeat` shape `0` applies Crystal-style max-dura loss before feeding, while `LeanMeat` shape `1` does not.
- `R88` completed Crystal `UseItem` shape `0` normal potion support as a modeled/timed recovery subset: `UseItem` now enqueues manifest-backed `pending_pot_health_amount` / `pending_pot_mana_amount` into `SimulationResources`, consumes the potion without immediate HP/MP mutation or hint-chat text, and `advance_world` now emits `ObjectHealth` / `ObjectMana` in small increments as the queue drains.
- `R89` completed manifest-backed equipment item type to runtime `EquipmentSlot` mapping for current item gain and `UseItem` fallback, reducing hand-coded slot setup for Crystal equipment items while leaving broader equipment stat parity as a separate surface.
- `R90` completed Crystal map-rule rejection for manifest-backed scroll shapes `0/2`: configured `NoEscape` and `NoRandom` maps now block dungeon escape and random teleport use with localized system messages before item consumption.
- `R91` completed Crystal repair-bind rejection for manifest-backed repair oils: `DontRepair` blocks `RepairOil` / `WarGodOil`, and `NoSRepair` blocks `WarGodOil`, preserving item and weapon durability on failure.
- `R92` completed the bounded Crystal `ResurrectionScroll` revive-vitals surface: successful dead-player revive now restores modeled MP along with full HP before consuming the scroll.
- `R93` completed explicit target-slot compatibility for manifest-backed ring/bracelet equipment, allowing `EquipItem` to place those item types into the right-side slots while preserving default `UseItem` slot selection.
- `R95` locked explicit `ItemType.Amulet` compatibility coverage for targeting the right bracelet slot.
- `R96` completed explicit `EquipItem` requirement gating for dynamic manifest-backed equipment: Crystal gender/class/required-type failures now silently fail before mutation like `CanEquipItem`, while `UseItem` keeps localized requirement messages.
- `R97` locked the same explicit `EquipItem` dynamic manifest-backed requirement rejection surface for storage-sourced equipment, including ack-only failure, storage preservation, and no target-slot mutation.
- `R98` locked dynamic manifest-backed credit-token use for `CreditToken3`, including success ack, `GainedCredit`, localized hint chat, credit-state update, and item consumption.
- `R99` locked the positive explicit `EquipItem` path for dynamic manifest-backed equipment when requirements are met, using `SpiritRing` at level 15 into the right ring slot.
- `R100` removed runtime-only `sim.equippedItem*` chat from successful modeled use-equip, leaving the success surface as ack/refresh/equipment-state only.
- `R101` removed literal runtime-only non-inventory equipment `UseItem` failure chat, leaving belt-sourced equipment-like use as failed-ack/no-chat/no-mutation.
- `R102` removed runtime-only `sim.itemNoActiveUse` from the unusable inventory item fallback, leaving unknown/unusable items as failed-ack/no-chat/no-mutation.
- `R103` removed runtime-only `sim.itemNotFoundInBag` from missing-item and invalid-source `UseItem` failures, leaving missing inventory ids as failed-ack/no-chat/no-mutation.
- `R104` made unmodeled `UseItem(grid=HeroInventory)` return a failed `UseItem` ack instead of empty packets while preserving the no-fallback/no-mutation guard.
- `R114` added Crystal `NoDrug` map-rule rejection for static starter and dynamic manifest-backed potion `UseItem`, preserving items and avoiding HP/MP recovery when potions are blocked on the current map.
- `R115` removed runtime-only normal item/gold pickup success chat while preserving Crystal `ShowGroupPickup` group notices.
- `R116` localized owner-blocked pickup rejection through Crystal `server.CannotPickupNotOwner`.
- `R117` localized harvest no-drop/full-bag messages through Crystal `server.NothingWasFound` and `server.YouCannotCarryAnymore`.
- `R123` localized Stage 5 credit-shop purchase chat through Crystal `server.BoughtItemForCredit`.
- `R124` localized Stage 5 item-seal reseal-delay rejection through Crystal `server.ItemCannotBeResealedFor`.
- `R125` localized Stage 5 item socket/seal success chats through Crystal `server.ItemSocketsIncreased` and `server.ItemSealedFor`.
- `R126` localized expanded-storage expiry notice through Crystal `server.ExpandedStorageExpired`.
- `R127` removed runtime-only harvest success chat, leaving `GainedItem` plus `ObjectHarvested`.
- `R128` localized Stage 5 gold-shop purchase chat through Crystal `server.BoughtItemForGold`.
- `R129` localized Stage 5 socket/seal invalid-source rejection chats through Crystal `server.InvalidCombination`.
- `R130` removed runtime-only ordinary map-transfer success chat, leaving movement packets only.
- `R131` localized Stage 5 socket/seal missing-source rejection chats through Crystal `server.NotFound`.
- `R132` localized Stage 5 socket/seal missing-equipped-item rejection chats through Crystal `server.NotFound`.
- `R133` localized Stage 5 socket metadata-missing rejection chat through Crystal `server.NotFound`.
- `R134` localized Stage 5 mail/trade/auction missing-entity rejection chats through Crystal `server.NotFound`.
- `R135` localized Stage 5 credit-shop insufficient-credit rejection through Crystal `server.YouDontHaveEnoughCurrency`.
- `R136` localized Stage 5 craft no-ore rejection through Crystal `server.CraftingAttemptFailed`.
- `R137` localized Stage 5 guild creation success through Crystal `server.SuccessfullyCreatedGuild`.
- `R138` localized Stage 5 event-spawn missing-template rejection through Crystal `server.NotFound`.
- `R139` localized Stage 5 hero-behaviour missing-hero rejection through Crystal `server.NotFound`.
- `R150` localized map-transfer bounds rejection through Crystal `server.CannotPositionMoveOnMap`.
- `R151` localized missing-template `RequestItemInfo` failure through Crystal `server.NotFound`.
- `R152` localized map-transfer not-in-world rejection through Crystal `server.NotFound`.
- `R153` removed runtime-only high-level `drop_item(key)` missing-item chat, leaving no-packet/no-chat/no-mutation behavior.
- `R154` removed runtime-only high-level `use_item(key)` / `drop_item(key)` before-start chats, leaving no-packet/no-chat behavior.
- `R155` localized `ShowGroupPickup` item notices through Crystal `server.FriendlyPickedUpItem`.
- `R156` removed runtime-only expanded-storage helper success chat, leaving the modeled `ResizeStorage` packet surface.
- `R157` localized benediction-oil weapon luck outcome chats through Crystal weapon luck keys.
- `R158` localized trainer average damage reporting through Crystal `server.AverageDamageOnTrainer` and added `{index:format}` localization placeholder support.
- `R149` removed runtime-only Stage 5 `event.spawn` and `hero.behaviour` helper success chats.
- `R148` removed runtime-only debug Crystal transfer success chat, leaving map/location packets only.
- `R147` removed generic runtime-only Stage 5 helper success chats across group/social/mail/trade/auction/conquest/hero/profession helpers.
- `R146` localized Stage 5 event-spawn missing-player/position rejections through Crystal `server.NotFound`.
- `R145` localized unknown map-transfer rejection through Crystal `server.NotFound`.
- `R144` localized Stage 5 unknown-command rejection through Crystal `server.InvalidPacketReceived`.
- `R143` localized Stage 5 inactive-trade rejections through Crystal `server.NotFound`.
- `R142` localized Stage 5 `auction.buy` / `auction.cancel` missing-id rejections through Crystal `server.InvalidPacketReceived`.
- `R141` localized Stage 5 `mail.claim` / `mail.delete` missing-id rejections through Crystal `server.InvalidPacketReceived`.
- `R140` localized Stage 5 `trade.offerGold` missing-amount rejection through Crystal `server.InvalidPacketReceived`.
- `R122` localized Stage 5 successful trade completion through Crystal `server.TradeSuccessful`.
- `R121` localized Stage 5 trade/shop/auction low-gold rejection keys through Crystal `server.LowGold`.
- `R120` localized direct ground-drop pickup full-bag rejection through Crystal `server.YouCannotCarryAnymore`.
- `R119` localized Stage 5 mail/shop/auction/craft full-bag rejection keys through Crystal `server.YouCannotCarryAnymore`.
- `R118` localized Stage 5 item socket/seal rejection keys through Crystal `server.ItemMaxSockets` and `server.ItemAlreadySealed`.
- `R113` aligned static starter HP/MP potion use with Crystal normal-potion timed recovery: successful use consumes and acks immediately, but HP/MP restoration is queued and emitted on follow-up ticks rather than mutating immediately.
- `R112` removed runtime-only static `repair-powder` success/failure chat, leaving starter equipment repair use as repair mutation plus `ItemRepaired` packets without generic `sim.noEquipmentNeedsRepair` / `sim.repairedEquippedItems` chat.
- `R111` removed runtime-only static `town-teleport` success chat, leaving successful teleports as movement/location packets without generic success chat.
- `R110` removed hardcoded runtime-only static `benediction-oil` no-weapon failure chat, leaving invalid luck attempts failed/no-chat/no-consume.
- `R109` removed runtime-only success chat from `SplitItem`, leaving inventory/storage split success as `SplitItem1` plus `SplitItem` only.
- `R108` aligned static `repair-oil` / `war-god-oil` with Crystal's localized weapon-repair Hint success chat and chat-free no-repair failure.
- `R107` removed runtime-only `custom.itemDropped` from successful `DropItem`, leaving normal/split-stack drops as success-ack plus ground-object visibility without generic success chat.
- `R106` removed runtime-only `sim.usedItem` from static HP/MP consumable `UseItem` success, leaving inventory/belt starter potions as heal/consume/success-ack without chat.
- `R105` removed runtime-only `sim.itemNotFoundInBag` from missing-source `DropItem`, leaving absent inventory ids as failed-ack/no-chat/no-mutation.

- Rust gateway/session bridge is in place.
- ECS headless runtime is in place.
- player, monster, NPC, and ground-drop entities live in ECS.
- deterministic starter-world bootstrap is working.
- movement, melee attack, NPC interaction, quest progression, drop and pickup loops are working.
- inventory, equipment, and quest snapshots are exposed through `worldSnapshot`.
- `worldSnapshot` now applies player-centered AOI filtering for entities, drops, terrain, and decor.
- monster respawn is now driven by explicit spawn rules / spawn slots instead of per-monster ad hoc timers.
- equipment replacement now returns the previous item to bag instead of silently overwriting it.
- skill casting now enforces mana cost before applying cooldown/effects.
- starter travel-item flow now supports town teleport back to spawn.
- starter monster spawn, drop, skill, quest, NPC dialog, and buff metadata now live in `mir2-game-data` instead of being embedded only in runtime branches.
- Crystal spell, buff, drop-table, and NPC-script manifests can now be generated directly from Crystal source/runtime data into `mir2-game-data`.
- imported Crystal `MonsterInfo.DropPath` values now resolve to `Envir/Drops` tables, and current runtime death/harvest rewards prefer those tables before starter fallback. Current coverage includes grouped-section chance rolls, generated nested Crystal `GROUP` tree preservation plus runtime `GROUP*` random-one-item / `GROUP^` first-success semantics, Crystal item metadata-backed Hen/Deer harvest rewards with `GainedItem`, imported item durability, Crystal `CreateDropItem` current-durability rolls plus generated `RandomItemStats.ini`-backed full current Jev random-stat family payloads (`UserItemStat`, curse, socket slots), imported ground `ObjectItem` grade/name-colour metadata, `NeedIdentify`-backed `UserItem.Identified` payloads, Deer meat quality durability, imported gold death drops with Crystal `N / 2 .. N + N / 2` amount ranges and `MaxDropGold=2000` ground chunking, Crystal quest-drop `Q` gating into active matching quest inventory with no ground fallback, Crystal `CanGainGold` cap preservation for ground gold pickup, Crystal-style player `DropGold` zero/insufficient-gold edge behavior, Crystal-style player `DropItem` stack-count splitting plus `DontDrop` / `DestroyOnDrop` bind semantics, Crystal-style death-drop owner pickup windows with source-confirmed immediate visibility, current-cell player pickup with Crystal scan/skip behavior for owner-blocked/full-bag/gold-cap candidates, `ItemTimeOut` ground-drop expiry, `ShowGroupPickup` group pickup notices, Crystal slot/stack-only pickup and harvest gain checks with overweight allowed after gain, HarvestMonster pending `_drops` generation/next-call transfer/full-bag retry semantics, harvest owner/EXPOwner corpse scan rejection with grouped-owner bypass and `NoNearbyOwnedCarcasses`, and preserved starter-only Field Wasp/Training Dummy behavior.
- Crystal NPC-script manifests now retain raw script text and line arrays, so the Rust backend has enough source material to build an actual Crystal NPC command interpreter instead of only relying on manifest counts.
- Crystal NPC-script manifests now also retain section bodies by label, and runtime NPC records can bind a Crystal `script_key` so imported `@Main` `#SAY` text, links, and simple `#ACT GOTO` flow can render through the backend.
- Crystal NPC command coverage can now be generated into `crystal_npc_command_summary.json` plus `docs/generated/crystal-npc-command-summary.md`; current coverage is 81/81 command names and 7,044/7,044 command occurrences. Runtime diagnostics still record any future unknown action/condition command with script key, section, and line number.
- Crystal monster manifests can now be generated directly from `Server.MirDB`, and dynamic summon templates like `BugBat` now resolve from imported Crystal monster data instead of runtime literals.
- Crystal monster AI family coverage can now be generated from `MonsterObject.GetMonster`, the monster manifest, and all respawns into `crystal_monster_ai_summary.json` plus `docs/generated/crystal-monster-ai-summary.md`, giving Stage 4 a concrete prioritization list for spawned generic AI families.
- Default `MonsterObject` / AI 0 is now explicit coverage instead of an implicit generic bucket: imported template stats, hostile movement/chase, adjacent `ObjectAttack`, delayed `Struck` damage, respawn/drop plumbing, and packet visibility are covered for normal Crystal monsters; subclass-specific behavior remains tracked by each AI row.
- `CaveMaggot` / AI 7 now uses a Crystal-specific attack baseline: delayed melee damage is sourced from imported monster DC data, hit resolution can apply the Crystal 1/20 five-second paralysis poison as an active movement-stopping status, and its corpse uses the shared HarvestMonster two-pass harvest flow.
- `SandWorm` / AI 35 now uses a Crystal-specific SpittingSpider-style baseline: two-tile line attack shape, `ObjectAttack`, 300 ms delayed hit timing, imported DC damage, forward-line multi-target fanout, and shared HarvestMonster corpse-harvest state.
- `HolyDeva` / AI 38 now uses a Crystal-specific ranged baseline: six-tile `ObjectRangeAttack`, summoned `extra` presentation, 500 ms delayed hit timing, imported DC damage, and Crystal fear-window kiting movement.
- `SandSnail` / AI 115 now uses a Crystal-specific adjacent attack split: primary type-0 DC `ObjectAttack`, type-1 DC halfmoon arc fanout, and type-2 MC one-tile area fanout with Green poison on player hit, all using Crystal 300 ms delayed timing.
- `CannibalTentacles` / AI 130 now uses a Crystal-specific attack baseline: view-range non-adjacent `ObjectRangeAttack` with distance-scaled imported MC damage, plus adjacent type-1 halfmoon with fixed 500 damage, green poison on player hit, and arc fanout to adjacent opposing monsters.
- `Jar1` / AI 119 now uses a Crystal-specific static baseline: no route/chase/patrol movement, one-tile DC melee, and delayed regular-monster slave spawn on death from the valid non-boss same-level-band pool; exact global RNG ordering remains a deterministic runtime approximation.
- `Jar2` / AI 120 now uses a Crystal-specific static ranged baseline: six-tile `ObjectRangeAttack`, no route/chase/patrol movement, 500 ms ranged hit timing, zero-MC data gating, adjacent 1/3 DC `ObjectAttack`, adjacent 2/3 `ObjectRangeAttack`, and the Frozen poison hook for successful ranged hits; current generated Jar2 rows have `MC=0`, so ranged damage and poison remain data-gated.
- `TurtleGrass` / AI 173 now uses a Crystal-specific Zuma-family baseline: stone/wake state, `ObjectShow` wake packets, two-tile Crystal attack shape, `ObjectAttack`, imported DC-based damage, and the type-1 single-push branch with three-tile displacement plus delayed DC damage.
- `ManTree` / AI 174 now uses a Crystal-specific Zuma-family baseline for the spawned `FineSoul` row: stone/wake state, adjacent `ObjectAttack`, Crystal 600 ms hit timing, type-0/type-1 halfmoon/type-2 boulder packet branches, and a Stun hook for successful boulder hits; current generated FineSoul rows have `DC=0`/`MC=0`, so delayed damage, halfmoon fanout, and Stun remain data-gated.
- `ToxicGhoul` / AI 28 now uses a Crystal-specific attack baseline: delayed melee damage is sourced from imported monster DC data, hit resolution can apply the Crystal 1/8 five-second green-poison status, and its corpse uses the shared HarvestMonster two-pass harvest flow; death explosion remains data-gated because current imported AI28 rows have `Effect=0`.
- `ThunderElement` / AI 49 now uses a Crystal-specific attack baseline: two-tile reach, 1-in-3 near-target repositioning before attacks, delayed due-time `ObjectAttack`, DC-based damage against the player and nearby opposing monsters, normal-damage immunity matching Crystal's repulsion-only damage gate, and player-skill `Repulsion` / `EnergyRepulsor` / `FireBurst` push-damage when a lower-level adjacent ThunderElement is pushed.
- `DarkBeast` / AI 112 now uses Crystal-specific primary and secondary attacks: delayed primary melee damage is sourced from imported monster DC data for the spawned CatWidow family, while the secondary type-1/bleeding hook is wired and correctly data-gated by current CatWidow `MC=0` / `Effect=0`.
- `FlamingWooma` / AI 10 now uses a Crystal-specific attack baseline: adjacent `ObjectAttack`, 300 ms delayed hit timing, and imported DC-based damage for the magic-melee branch.
- `RedThunderZuma` / AI 16 now uses a Crystal-specific ranged Zuma baseline: stoned wake state, wake propagation, nine-tile reach, non-adjacent `ObjectRangeAttack`, fixed 500 ms ranged hit timing, and imported DC damage with zero-DC no-damage gating.
- `FrostTiger` / AI 34 now uses a Crystal-specific passive ranged baseline: it does not auto-acquire players, can still fight after target lock, uses six-tile reach, non-adjacent `ObjectRangeAttack`, distance-scaled ranged hit timing, imported DC damage, effect-driven ranged bleeding/slow poison rolls, and `ObjectSitDown` sitting/standing presentation.
- `IceGuard` / AI 102 now uses Crystal-specific near/ranged branches: eight-tile reach, adjacent `ObjectAttack`, non-adjacent `ObjectRangeAttack`, fixed 500 ms ranged hit timing, imported MC ranged damage, imported DC melee gating, fire `ObjectRangeAttack` type 1, and ice-branch slow/frozen poison rolls.
- `FrozenMiner` / AI 187 now uses Crystal-specific attack branches: `ObjectAttack` type 0 with 600 ms delayed imported DC damage, plus type-1 `ObjectAttack` with 1000 ms delayed imported 80% DC damage for the player target and adjacent opposing-monster `FindAllTargets(1)`-style fanout.
- `FrozenAxeman` / AI 188 now uses Crystal-specific two-tile and adjacent branches: line/diagonal `ObjectAttack` type 1 with 500 ms delayed imported DC*2 damage, plus adjacent type-2 pull/push with a Crystal-shaped 2/3 deterministic trigger, 10s cooldown, 2-4 tile player push, and 500 ms imported DC hit.
- `FrozenMagician` / AI 189 now uses Crystal-specific nine-tile ranged branches: type-0 `ObjectRangeAttack` with distance-scaled 600 ms base delay and imported MC damage, plus type-1 boosted `ObjectRangeAttack` with 750 ms base delay and imported MC*3/2 damage.
- `SnowWolf` / AI 179 now uses Crystal-specific attack branches: type-0 `ObjectAttack` with 350 ms delayed imported DC damage, type-1 `ObjectAttack` with raw-MC gating plus defence mitigation, current zero-MC data staying damage-free, nonzero-MC Slow/Frozen poison rolls, and `FindAllTargets(2)`-style fanout.
- `FrozenWarewolf` through `SnowWolfKing` / AI 180 now uses Crystal-specific attack, spawn, and death baselines: `ObjectAttack` type 0/1/2/3 packet variants, 500 ms delayed imported DC damage, the below-70% one-time SnowWolf slave spawn, and delayed one-tile death explosion damage; weaker-target teleport and pet transfer remain pending.
- `SnowYeti` / AI 190 now uses Crystal-specific ranged and adjacent branches: nine-tile non-adjacent `ObjectRangeAttack` with distance-scaled delayed imported DC damage and the frozen poison roll, plus adjacent type-0/type-1 `ObjectAttack` packets with 500/1500 ms imported DC double hits.
- `DarkWraith` / AI 192 now uses Crystal-specific line and adjacent area branches: row/column/diagonal four-tile `ObjectAttack` type 2 with imported DC*3 damage, four-tile line fanout, and a 3-7s line cooldown approximation, plus adjacent type-1 area fanout with 600 ms imported DC hits against the player and nearby opposing targets.
- `CrystalSpider` / AI 37 now uses a Crystal-specific line branch baseline: three-tile row/column/diagonal reach, non-adjacent `ObjectAttack` type 1, distance-scaled delayed DC damage, forward-line multi-target fanout, and 1/8 green poison.
- `TucsonMage` / AI 126 now uses Crystal's WideLine attack baseline: three-tile square reach, type-1 `ObjectAttack`, zero-MC no-damage gating for current imported data, adjacent 1-in-3 WideLine branch selection, and forward plus three-lane two-step fanout when MC is available.
- `TucsonWarrior` / AI 127 now uses Crystal-specific attack selection: two-tile row/column/diagonal reach, non-adjacent type-1 `ObjectAttack` with imported MC damage, adjacent 4/5 type-0 halfmoon DC fanout, adjacent 1/5 type-1 MC smash, and one-tile target-area multi-target coverage.
- `IncarnatedZT` / AI 22 now uses a Crystal-specific active Zuma melee baseline: it starts outside the stoned Zuma wake state, emits adjacent `ObjectAttack`, uses 300 ms delayed hit timing, resolves imported DC damage, and wires Crystal's 1/12 five-second paralysis poison chance.
- `ZumaTaurus` / AI 17 now uses the shared Crystal Zuma wake baseline plus adjacent melee and HP-stage slave spawning: stoned `extra` presentation, wake/show propagation, `ObjectAttack`, 300 ms delayed hit timing, imported DC damage, seven-stage HP tracking, and Crystal's configured Zuma minion waves with the original 8-per-wave / 40-slave cap.
- `BoneLord` / AI 30 now uses a Crystal-specific ranged and HP-stage baseline: seven-tile reach, non-adjacent `ObjectRangeAttack`, distance-scaled delayed hit timing, imported DC damage, and type-1 HP-stage slave waves using Crystal's BoneSpearman/BoneBlademan/BoneArcher/BoneCaptain set with the original 8-per-wave / 40-slave cap.
- `MinotaurKing` / AI 33 now uses a RightGuard-derived ranged baseline: six-tile reach, non-adjacent `ObjectRangeAttack`, 500 ms delayed ranged hit timing, imported DC damage, and Crystal's three-tile range-damage fanout around the target.
- `DarkDevil` / AI 20 now uses a Crystal-specific ranged burst baseline: three-tile reach, non-adjacent `ObjectRangeAttack`, 500 ms delayed hit timing, imported DC*3 damage, Crystal's 2-4 second area cooldown, and one-tile fanout around the point two tiles in front of the monster.
- `OmaKing` / AI 43 now uses a Crystal-specific ranged/close split: seven-tile type-1 `ObjectAttack` magic with 500 ms imported MC damage, plus close type-0 push/paralysis handling and two-tile line fanout using imported DC damage.
- `GreatFoxSpirit` / AI 50 now uses a static ranged baseline: no route/chase/patrol movement, seven-tile reach, non-adjacent `ObjectRangeAttack`, 300 ms delayed hit timing, imported DC damage, Crystal `FindAllTargets` fanout, `ObjectEffect GreatFoxSpirit` broadcasts on ranged targets, slow/paralysis poison rolls on successful player hit, HP-stage `extra_byte` update broadcasts, nearby GuardianRock activation/deactivation, and 10-second-cooldown far-target recall movement to a nearby tile with `ObjectTeleportOut` / `ObjectTeleportIn` effect 11; multi-target/MagicResist recall details remain pending.
- `ManectricKing` / AI 88, through spawned `Master_DragonYang`, now uses a three-tile row/column/diagonal magic line baseline: type-0 `ObjectAttack`, 500 ms delayed hit timing, imported MC damage, the close type-1 DC push line branch, and the low-HP type-0 `ObjectRangeAttack` mass attack with seven-tile target coverage and distance-delayed imported MC damage.
- `SeedingsGeneral` / AI 121 now uses Crystal-specific ranged and close mixed branches: non-adjacent `ObjectRangeAttack`, 300 ms delayed imported MC damage, type-0 Echo Shout slow poison, type-1 Stomp frozen poison with adjacent opposing-target fanout, close type-0 DC Blood Attack, and close type-1 MC Green Splash.
- `RestlessJar` / AI 122 now uses a static ranged packet baseline and adjacent Crystal branches: no route/chase/patrol movement, six-tile reach, non-adjacent `ObjectRangeAttack` with Crystal `ProjectileAttack` distance*50+500ms timing and current zero-MC no-damage gating, adjacent spin fanout, tornado/blindness, and low-HP stomp push/fanout.
- `HellKeeper` / AI 79 now uses static view-range attack branches: no route/chase/patrol movement, locked initial attack facing, type-0 `ObjectAttack` with 300 ms delayed imported DC damage, type-1 `ObjectAttack` with raw-MC gating plus defence mitigation and Dazed for nonzero-MC data, and `FindAllTargets`-style fanout to view-range opposing targets; current zero-MC HellKeeper data still emits type 1 without damage/dazed.
- `GeneralMeowMeow` / AI 123 now uses a twelve-tile ranged magic baseline plus close slam branch: `ObjectRangeAttack` beyond two tiles, 500 ms delayed hit timing, imported MC damage, two-tile target-area fanout, close two-tile `ObjectAttack`/DC switching, the 1-in-9 type-1 triple-DC slam, HP shield windows with `GeneralMeowMeowShield` presentation and 100-AC damage absorption, shield-phase `ObjectSpell` `GeneralMeowMeowThunder` mass thunder with delayed MC damage to the player and opposing monsters near the target, and the 60s periodic cat-minion slave spawn with Crystal's 3-per-wave / 6-slave cap.
- `TucsonGeneral` / AI 131 now uses a rage plus ranged and close-stomp baseline: opening type-0 `ObjectRangeAttack` with no direct damage, 20-second rage cooldown, 8-second attack pause, 15 delayed `TucsonGeneralRock` `ObjectSpell` objects with deterministic target-biased scatter and raw-DC impact damage, normal type-1 ranged `ObjectRangeAttack` using imported SC damage, the 1-in-4 type-2 ranged branch using imported SC * 2 with 500 ms delay, and close type-1 MC stomp with three-tile area damage plus paralysis poison; exact Crystal global RNG ordering remains approximated.
- `TrapRock` / AI 47 now uses a hidden trap baseline: hidden/non-visible initial state, static route/chase/patrol blocking, delayed reveal near a target, deterministic `SpawnCorner` ordering for the adjacent target teleport, parent and child `ObjectShow`, parent `ObjectRangeAttack` with no direct damage, three cardinal child rocks using `ObjectAttack`, reveal paralysis, child-hit parent `FirstAttack` clearing, target-move death after reveal, first-hit parent collapse, and Crystal's 1-in-8 parent repeated-attack paralysis roll.
- `Armadillo` / AI 124 and `ArmadilloElder` / AI 125 now share the DigOut reveal baseline: hidden/non-visible initial state, near-player `ObjectShow`, delayed `DigOutArmadillo` `ObjectSpell` presentation, delayed post-reveal attack, Armadillo primary DC `ObjectAttack`, Armadillo type-1 three-hit combo, Armadillo retreat `ObjectBackStep` with delayed radius damage, Armadillo run-away after failed retreat damage, Elder primary DC*2 `ObjectAttack`, Elder type-1 two-tile push with no direct damage, Elder retreat `ObjectBackStep`, and Elder run-away movement.
- `Chieftain_Priest` through `ManectricClaw` / AI 86 now uses a Crystal-specific thrust baseline: three-tile reach, random pre-thrust step-toward-target branch, non-adjacent `ObjectRangeAttack`, 500 ms delayed hit timing, near-DC / far-MC imported damage, player slow/frozen poison rolls, and full three-column cone fanout to opposing monsters.
- `KingScorpion` / AI 19 now uses Crystal-specific two-tile and adjacent branches: row/column/diagonal reach, non-adjacent `ObjectRangeAttack` with delayed imported MC damage, two-tile line fanout, adjacent `ObjectAttack` with imported DC damage, and adjacent random/second-tile `ObjectRangeAttack` override.
- `MirStatue` through `DragonStatue` / AI 54 now uses a static delayed ranged baseline: no route/chase/patrol movement, view-range reach, due-time `ObjectRangeAttack`, 500 ms delayed hit timing, imported DC damage, `FindAllTargets(2)`-style fanout around the player target, lethal-damage sleep instead of death, sleeping damage immunity, and 15-minute full-HP wake.
- `GuardianRock` / AI 48 now uses a static range-pull baseline: no route/chase/patrol movement, normal-damage immunity, 500 ms delayed `ObjectRangeAttack`, no direct player damage, and Crystal pull distance toward the rock capped at four tiles; magic-resist handling remains pending.
- `RedMoonEvil` / AI 13 now uses a static view-range attack baseline: no route/chase/patrol movement, `ObjectAttack`, 300 ms delayed hit timing, imported DC damage, Crystal's multi-target fanout across opposing targets in view range, and `ObjectEffect RedMoonEvil` broadcasts for each target.
- `EvilCentipede` / AI 14 now uses a hidden/static attack baseline: hidden initial state, three-tile `ObjectShow` reveal, seven-tile hide/HP-restore reset, no route/chase/patrol movement, visible `ObjectAttack`, 500 ms delayed hit timing, imported DC damage, seven-tile multi-target fanout, and Crystal green/paralysis poison rolls on successful player hit.
- `Yimoogi` / AI 36 now uses a Crystal-specific ranged and lifecycle baseline: seven-tile reach, non-adjacent `ObjectRangeAttack` beyond the close two-tile shape, 500 ms delayed hit timing, imported DC damage, the four-tile type-1 red-poison branch, four-second sister child spawning, final low-HP teleport with two `WhiteSerpent` spawns at the old location, and paired drop suppression while the sister is alive.
- `Lamia` through `Kirin` / AI 186 now uses Crystal-specific two-tile and IceThrust branches: row/diagonal reach, `ObjectAttack` type 0 imported DC baseline, type-1 500 ms imported DC branch, and nonzero-MC type-2 IceThrust cone with slow poison plus opposing-target fanout; current Lamia MC=0 data naturally gates the IceThrust branch.
- `Khazard` / AI 27 now uses a Crystal-specific ranged pull branch: four-tile row/column/diagonal reach, non-adjacent `ObjectRangeAttack`, no direct player damage, immediate player pull movement toward Khazard, and 5s `PullTime` cooldown; exact magic-resist checks remain pending.
- `HedgeKekTal` / AI 51 now uses a Crystal-specific near-vs-range attack baseline: eight-tile range, `ObjectRangeAttack` when non-adjacent, distance-scaled delayed ranged damage, adjacent `ObjectAttack`, and imported DC-based damage.
- `Trainer` / AI 56 now uses a Crystal-specific static target-dummy baseline: trainers are neutral/passive, do not route/chase/patrol or attack, ordinary damage does not reduce HP or kill them, and trainer damage/DPS plus idle average reports use Crystal `ChatType.Trainer`.
- `WoomaTaurus` / AI 11 now uses a Crystal-specific elite baseline: FlamingWooma 300 ms delayed melee with imported DC damage, seven-stage HP threshold tracking, mad speed phase, surrounded teleport movement, and `ObjectTeleportOut` / `ObjectTeleportIn` effect packets.
- `HarvestMonster` / AI 9 now uses Crystal corpse-harvest semantics: death skips immediate ground drops, `Harvest` emits `ObjectHarvest`, the corpse requires the two skin-count harvest passes, configured rewards transfer from the corpse on the follow-up harvest, and `ObjectHarvested` marks the corpse harvested/skeleton-visible.
- `Hen` / `Pig` / `Bull` via AI 1 now use the Crystal HarvestMonster passive baseline: they are neutral/passive, do not target or normally attack players, and their corpses require the default two skin-count harvest passes before `ObjectHarvested`.
- `Deer` / AI 2 now uses a Crystal-specific passive harvest baseline: Deer/Deer1/Sheep do not normally attack players, their corpses require Crystal's five skin-count harvest passes before `ObjectHarvested`, and the Crystal run-away subset now flees away from nearby players; exact `Quality` randomization remains pending with item/drop quality parity.
- `TucsonEgg` / AI 128 now uses a Crystal-specific egg baseline: it is immobile, does not route/chase/patrol or perform normal attacks, each successful hit removes exactly 1 HP, and death has the delayed poison/damage plus Effect=1 GeneralTucson/TucsonGeneral spawn hook.
- `Tree` / AI 3 now uses a Crystal-specific static-object baseline: tree-style objects are neutral/passive, do not route/chase/patrol or attack, runtime Crystal spawns face up, and successful hits remove exactly 1 HP.
- HellFire AI coverage now includes `HellKnight` packet `extra`, `HellBomb` immobility / damage immunity / timeout explosion with delayed radius damage and HellBomb1/HellBomb2/HellBomb3 Frozen/Dazed/Bleeding poison variants, and `HellLord` immobility / stage-gated immunity / knight and bomb spawning / stage update packets.
- High-count line/range AI coverage now includes `ShamanZombie` six-tile line/diagonal `ObjectRangeAttack` and `BlackFoxman` two-tile type-1 line `ObjectAttack` behavior.
- `DigOutZombie` AI now starts hidden/non-targetable/non-blocking and reveals with `ObjectShow` when the player comes within the Crystal three-tile trigger range.
- `RevivingZombie` AI now revives after a delay with reduced HP and emits `ObjectRevived` / `ObjectHealth`, with a deterministic two-revival baseline.
- `RedFoxman` and `WhiteFoxman` AI now use six-tile `ObjectRangeAttack` pressure with imported DC damage instead of closing to one-tile generic melee; RedFoxman also covers Crystal's type-0/type-1 ranged packet split, fear-window kiting, and adjacent teleport with `ObjectTeleportOut` / `ObjectTeleportIn` effect type 2, while WhiteFoxman covers the type-1 delayed status-only slow branch and fear-window kiting movement.
- `WaterDragon` and `BlackTortoise` AI now switch to `ObjectRangeAttack` when not adjacent, instead of using the one-tile generic melee baseline. WaterDragon ranged hits now use imported MC damage plus the 1-in-7 five-tick green poison roll; BlackTortoise has the same green-poison hook with current `SmallDrake` zero-MC data correctly gating damage and poison, plus the close 1-in-5 type-1 halfmoon fanout.
- Cat-family AI now covers `BlackHammerCat` type-1 line `ObjectAttack`, `StrayCat` type-2 line `ObjectAttack` plus close type-1 push variant with current zero-MC follow-up damage gating, and `CatShaman` six-tile `ObjectRangeAttack` baselines, including CatShaman's type-1 red-poison packet/hook with current zero-MC damage/poison gating.
- `YinDevilNode` / `YangDevilNode` AI no longer behaves like a normal player-attacking monster; it is immobile and holds a support-node baseline pending friendly buff parity.
- starter map movement and occupancy now use Crystal type100 wall/door data inside the exported starter region instead of scene-view clamping only.
- Crystal respawn metadata and route points can now be generated from `Server.MirDB` plus `Envir/Routes` into `mir2-game-data`.
- spawn table generation now supports a Crystal starter-region import source and uses walkable-cell selection across the full `origin +/- spread` square instead of a fixed pattern.
- runtime map transfers and safe zones now consume generated Crystal map movement/safe-zone metadata beyond the starter config bridge.
- runtime `MapInformation` now propagates generated Crystal `mini_map`, `big_map`, and `light` values when moving between imported maps.
- current-map respawn placement now uses target-map collision data with cached runtime map parsing, so representative imported maps such as `HF1`, `D1801`, and `HKR` no longer inherit starter-map collision assumptions.
- imported Crystal starter-region respawns now use Crystal-style minute delay/random-delay scheduling semantics.
- imported Crystal route monsters now follow filtered patrol paths in the starter-region runtime, including route-point wait delays.
- imported Crystal monster metadata now carries AI, view range, move speed, attack speed, and guard/archer neutrality into the Rust runtime.
- imported guard / town-archer style AI no longer auto-aggro the player in the starter-region runtime, and snapshot disposition now distinguishes hostile vs neutral monsters.
- guard-style Crystal AI now acquires eligible hostile monster targets, ignores hen/deer/tree-style AI, and emits packet-visible attack actions when engaging.
- Rust protocol and gateway now expose `ObjectAttack` and `ObjectRangeAttack`, so monster attack events are no longer limited to movement plus health deltas.
- visible `ObjectMonster` packets now preserve imported Crystal AI values instead of flattening all imported monsters to `ai = 0`.
- player melee now emits packet-visible attack actions, and monsters hit by the player now lock onto the player and continue fighting through the world tick instead of using the old fake immediate retaliation path.
- Rust protocol/runtime/gateway now expose Crystal-style `Struck` and `ObjectStruck`, AOI packet finalization now spawns newly visible objects before same-tick tracked combat packets instead of dropping or mis-ordering them, and current `TownArcher` / `ArcherGuard` style AI now uses `ObjectRangeAttack` against the player instead of being flattened into melee packets.
- current `Guard` melee now uses Crystal-style target-back `ObjectAttack` plus follow-up `ObjectTurn`, and `ArcherGuard` no longer incorrectly follows patrol routes in the Rust runtime.
- monster-origin damage is no longer applied in the same tick as the attack packet: the Rust runtime now has a minimal delayed combat queue, current melee hits resolve on a future tick, ranged hits use distance-scaled delay, and ordinary monster attacks no longer emit an extra pre-attack `ObjectTurn` packet that Crystal does not send.
- player melee hit resolution is now delayed into the follow-up world tick as well, so `ObjectStruck`, HP loss, kill resolution, and hit chat no longer land at attack-launch time.
- current `SpittingSpider` / AI 4 now attacks from Crystal-style two-tile line and diagonal patterns, current `AxeSkeleton` / AI 8 uses ranged packets at six tiles with Crystal fear-window close/kite movement, and current `CannibalPlant` / AI 5 now hides and reveals through `ObjectHide` / `ObjectShow` with client visibility and blocking tied to the hidden state.
- current `ZumaMonster` / AI 15 now carries a stone/wake state through monster `extra`, starts untargetable while stoned, wakes when the player comes near, and propagates wake-up `ObjectShow` packets to nearby stoned Zuma units; current `BoneSpearman` / AI 29 now attacks using the same two-tile line reach family instead of flattening to adjacent melee only.
- current `RightGuard` / AI 31 and `LeftGuard` / AI 32 now switch between adjacent melee packets and ranged packets based on distance instead of staying flattened to one attack family, and monster respawn now restores special AI presentation state like hidden plant / stoned Zuma defaults instead of reviving with stale runtime state.
- `BugBagMaggot` summon flow now uses Crystal attack range / spawn-cap timing and resolves the spawned `BugBat` body from imported Crystal monster metadata, including preserving `master_object_id` in visible summon packets.
- current `RootSpider` / AI 39 now follows Crystal summon timing and back-offset spawn semantics for `BombSpider`, and current `BombSpider` / AI 40 now self-destructs on adjacency / timeout and applies follow-up explosion damage through the delayed combat path.
- summon runtime metadata is now generalized beyond Zuma stone state, so Crystal-style summon `extra` presentation, owner binding, timeout / out-of-range self-destruct, and delayed corpse cleanup are shared runtime concepts instead of one-off AI branches.
- current `SnakeTotem` / AI 62 now spawns Crystal-style `CharmedSnake` bodies with summon `extra` packets and owner tracking, and current `CharmedSnake` summon bodies now self-destruct when the totem owner disappears, moves out of range, or times out.
- player summon spell entries now exist in `mir2-game-data` and the Rust runtime for `SummonShinsu`, `SummonVampire`, `SummonToad`, `SummonSnakes`, and `Stonetrap`, so summon skills are no longer limited to buff/heal placeholders.
- player-cast `SummonShinsu` now spawns a friendly owned summon with Crystal-style visible `extra`, recall behavior, cap handling, and shared summon lifetime metadata instead of being missing from the backend.
- player-cast `Stonetrap` now spawns a friendly owned `StoneTrap` body with Crystal-style summon `extra`, owner binding, timeout/range cleanup, and trap-style immobility instead of being absent from the backend.
- summon spawning is now generic across monster and player owners, so pending summon resolution no longer assumes the summoner is always a monster entity.
- friendly summon combat no longer reuses guard insta-kill damage when targeting hostile monsters; runtime summon-vs-monster attacks now resolve through imported Crystal monster damage ranges.
- player-cast `SummonSnakes` now spawns a friendly `SnakeTotem` with Crystal-style skill-level minion cap, friendly `CharmedSnake` children, and totem-owned minion cleanup instead of hostile placeholder behavior.
- player-cast `SummonToad` now uses friendly ranged summon-vs-monster combat instead of only spawning a body, and player-cast `SummonVampire` now resolves Crystal-style delayed death explosion damage against nearby hostile targets instead of only dying visually.
- hostile monsters can now retarget nearby friendly summons and trap bodies, so current friendly summon and `StoneTrap` presence affects hostile combat selection instead of only existing as passive owned monsters.
- current `StoneTrap` now acts as a stronger Crystal-style hostile aggro sink, with trap-priority hostile target selection and `Struck`-style incoming damage immunity instead of normal monster HP loss.
- current `Shinsu` now drives mode/show/hide from real hostile targets instead of owner proximity alone, refreshes its active window while targets exist, hides after timeout when targets are gone, and applies current two-tile line pressure through the delayed combat path.
- current `CharmedSnake` death explosion now participates in the same delayed summon-vs-monster damage chain as `VampireSpider`, so friendly summon death effects can hit nearby hostile monsters.
- equipped weapon durability now drops from player attacks, equipped non-weapon durability drops when delayed monster-origin hits resolve against the player, and durability-zero gear no longer contributes attack/defence in snapshots or combat totals.
- `repair-powder` now uses the item-use pipeline to restore equipped durability, consumes only when at least one equipped item needed repair, and returns localized system feedback.
- current durability loss and repair flows now emit Crystal-shaped `DuraChanged` / ID 76 and `ItemRepaired` / ID 114 packets through protocol, runtime, and gateway JSON conversion; mapped Crystal `RepairOil` and `WarGodOil` scroll use now repairs the equipped weapon and emits `ItemRepaired`.
- current item packet grid values now match Crystal for the active Inventory/Equipment/Trade/Storage/QuestInventory/Refine/HeroEquipment/HeroInventory paths, and runtime equipment-slot references now use Crystal Belt=10, Boots=11, Mount=13.
- current item move/equip/remove/split/merge/drop/use/store/take-back flows now emit Crystal-shaped action ack packets (`MoveItem`, `EquipItem`, `MergeItem`, `RemoveItem`, `RemoveSlotItem`, `TakeBackItem`, `StoreItem`, `SplitItem1`, `UseItem`, and `DropItem`) through protocol, runtime, and gateway JSON conversion. Current packet `UseItem`, packet `EquipItem`, and `MergeItem` now also resolve the exact referenced current item by unique id instead of duplicate-key fallback or slot aliases, so bag-page duplicates no longer mutate the wrong stack or equipment candidate. Current `UseItem` now also matches the bounded Crystal dead-state, scroll map-rule, and failure-ack surface: ordinary items fail while dead, alive `ResurrectionScroll` emits `CannotResurrection`, dead `ResurrectionScroll` revives only on allowed maps, `TownTeleport` respects `NoTownTeleport`, successful modeled use-equip no longer emits runtime-only `sim.equippedItem*` chat, non-inventory equipment use failures no longer emit literal runtime-only chat, unusable inventory item fallback no longer emits `sim.itemNoActiveUse`, missing-item failures no longer emit `sim.itemNotFoundInBag`, unmodeled `UseItem(grid=HeroInventory)` returns a failed ack instead of empty packets, and missing-source `DropItem` fails without runtime-only chat. Current `MoveItem` now also keeps unsupported `Belt` / `QuestInventory` / `HeroInventory` / `HeroEquipment` / `Equipment` / `Fishing` / `Trade` / `Refine` requests ack-only, uses Crystal's `ItemMoveErrorReport` surface for current Inventory/Storage missing-source failures, keeps storage-lock and invalid-slot failures ack-only, requires the active `@Storage` service context for `MoveItem(grid=Storage)`, resolves slot-based current inventory selection through Crystal single-array indices across local `Bag1` / `Bag2`, scopes current bag moves away from quest items that share the same local slot number, and no longer emits runtime-only success chat on successful current Inventory/Storage moves. Current `MergeItem` now keeps unsupported `QuestInventory` requests ack-only and keeps the remaining unsupported `Storage <-> Belt` cross-grid requests ack-only without runtime-only chat, while preserving the modeled `Inventory <-> Storage` and `Inventory <-> Belt` stack-merge surfaces. Current `DropItem` runtime also follows Crystal inventory slot/count semantics, including partial-stack ground drops, invalid count failure acks, manifest-backed and rental-backed `DontDrop` rejection, `DestroyOnDrop` deletion without spawning a ground object, and the bounded current hero-inventory guard where unavailable hero inventory ack-fails without mutating matching player bag items. Current `StoreItem` / `TakeBackItem` now require active `@Storage` service context from the real `NPCStorage` link flow, preserve Crystal password-lock/capacity/occupied-target no-swap semantics, reject base and rental `DontStore` for store only, keep ack-only failure behavior, and now use the same single-array current inventory indexing across local `Bag1` / `Bag2`.
- current equipment/item metadata now preserves Crystal `NeedIdentify` and `SoulBoundId` through runtime/item payload round-trips, successful equip/use-equip identifies the item before the visible refresh and successful modeled use-equip is chat-free and non-inventory equipment-use failure is ack-only, unusable inventory item fallback is chat-free, missing-item failures are chat-free, and unmodeled hero-inventory use returns a failed ack, storage/current equipment keep the metadata intact, and items soul-bound to another character are rejected on equip.
- dynamic manifest-backed current-data `UseItem` now routes Crystal `SunPotion`, duration buffs, `TownTeleport`, `BenedictionOil`, `RepairOil`, and `WarGodOil` through template stats and scroll shapes, including Crystal same-key buff duration stacking and the current bounded `WarGodOil` name fallback because the generated manifest still reports `shape = 0`.
- current `SplitItem` now follows Crystal single-array inventory placement across local `Bag1` / `Bag2`, prefers eligible belt slots first for inventory splits, supports only `Inventory` / `Storage`, requires active `@Storage` service context for storage splits, and keeps unsupported/invalid/full/locked failures on the failed ack with no extra chat.
- current gold pickup/drop flows now emit Crystal-shaped `GainedGold` / ID 67 and `LoseGold` / ID 68 packets through protocol, runtime, and gateway JSON conversion.
- current sell/repair request flows now emit Crystal-shaped `SellItem` / ID 111 and `RepairItem` / ID 113 entry packets through protocol, runtime, and gateway JSON conversion; successful sell also emits `GainedGold`; NPC repair now keeps Crystal's entry `RepairItem` ack and follows with `LoseGold` plus `ItemRepaired` only after service, item, repairability, type, and gold checks pass.
- current `SellItem` runtime now honors Crystal stack-count semantics for partial stack sales, requires an active Crystal `@SELL` / `@BUYSELL` page before mutating inventory/gold, rejects `DontSell` and script `[Types]` mismatches with Crystal ack/message behavior, rejects partial-stack sales that would exceed Crystal's `uint.MaxValue` gold cap, preserves full-stack sale success with clamped zero-gold gain at cap, and uses Crystal `UserItem.Price() / 2` style sale value for mapped items.
- `mir2-protocol` and gateway JSON now support Crystal `GainedCredit` / ID 69 and `LoseCredit` / ID 70 credit delta packets; current mapped Crystal `CreditToken` scroll use mutates account credit state, emits `GainedCredit`, updates `UserInformation.credit`, and survives save/reload, while the current credit-shop purchase path emits `LoseCredit` after balance checks and mails item attachments like Crystal game-shop purchases instead of failing on a full bag. Imported Crystal game-shop product catalogs remain pending.
- `mir2-protocol` and gateway JSON now support Crystal `CombineItem` / client ID 111 plus `CombineItem` / server ID 215 and `ItemUpgraded` / server ID 216 alongside `ItemSlotSizeChanged` / ID 115 and `ItemSealChanged` / ID 116 item-state packets; current inventory-grid `CombineItem` dispatch now reuses the existing shape-7 socket-growth, shape-8 seal, and bounded shape-3/4 gem/orb upgrade semantics, emits the Crystal ack payload plus the adjacent slot/seal/upgrade packets, preserves saved `SealedInfo` plus persisted `gem_count` and rental `BindingFlags` through runtime/equipment/inventory round-trips, applies equipment-backed player `GemRatePercent` to current shape-3/4 upgrade success chance, blocks rental `DontUpgrade` ack-only for the current socket/upgrade branches, matches Crystal's shared top-level target item-type gate before branch-specific handling, resolves current source/target inventory items by unique id instead of slot aliases, and now regression-locks the bounded `HeroInventory` grid guard so unavailable hero inventory ack-fails without mutating matching player bag items. Broader hero-inventory handling and other combine branches remain open.
- current bag-item packet lookups now follow Crystal inventory unique-id semantics for `SplitItem`, `DeleteItem`, `DropItem`, `SellItem`, `RepairItem`, and `MergeItem`, while packet `UseItem` and packet `EquipItem` now also resolve the exact current bag item by unique id even when duplicate-key items exist on different bag pages; runtime fallback ids no longer alias `Bag1` / `Bag2` same-slot items in the current inventory model.
- `mir2-protocol` and gateway JSON now support the Crystal NPC service packet block for current shop/repair/storage/refine/craft surfaces (`TeleportIn`, `NPCGoods`, `NPCSell`, `NPCRepair`, `NPCSRepair`, `NPCRefine`, `NPCCheckRefine`, `NPCCollectRefine`, `NPCReplaceWedRing`, `NPCStorage`, and `CraftItem`), and current imported NPC service labels now emit open-service packets for buy, buy/sell, sell, repair, special repair, craft, refine, refine-check, wedding-ring replacement, and storage pages. Buy/buy-sell/craft `NPCGoods` now includes imported `[Trade]` / `[Recipe]` goods as Crystal `UserItem` payloads, current `NPCGoods`/repair service packets use imported `NPCInfo.Rate / 100F`, buy panels use Crystal `GoodsHideAddedStats` flags, current sell-service actions populate per-NPC buy-back goods for `@BuyBack` with Crystal sell flag/type/price/cap rejection semantics, buy-back survives save/reload, expired buy-back entries move into persisted used goods, and Crystal `BuyItem` / ID 51 can purchase current static trade, buy-back, and used goods with `LoseGold`/`GainedItem`; `BuyItem` also silently rejects invalid panel/count, missing active service, non-buy service pages, missing goods/metadata, insufficient gold, and full bags before any mutation. NPC `RepairItem` / `SRepairItem` now require the matching active repair page, use current backpack unique-id lookup, calculate normal/special repair cost from Crystal item price and service rate, apply normal max-dura loss but not special max loss, and match Crystal message/silent rejection branches for non-repairable, type-disallowed, and insufficient-gold cases. Refine result state remains pending.
- current inventory delete requests now support Crystal client `DeleteItem` / ID 149 and server `DeleteItem` / ID 79 packet behavior, including partial stack reduction, full stack removal, gateway JSON conversion, and Crystal's server-side quirk where the packet `HeroInventory` flag is ignored and deletion still searches only current player inventory by unique id.
- `mir2-protocol` now has reusable Crystal `UserItem.Save`-order serialization, and current split-stack flows emit Crystal `SplitItem` / ID 44 payload packets after the existing `SplitItem1` success ack.
- current inventory pickup updates now emit Crystal `GainedItem` / ID 66 payload packets using the shared `UserItem` serializer, while gold pickup remains on `GainedGold`.
- Crystal `Server.MirDB` item rows now generate into `crystal_item_manifest.json` with core `ItemInfo.Save` fields, and current mapped starter `UserItem.item_index` values resolve through that manifest instead of using only local icon ids.
- `mir2-protocol` and gateway JSON now support Crystal `RefreshItem` / ID 148 payload packets on the shared `UserItem` serializer, and current `BenedictionOil` weapon Luck success use emits `RefreshItem` for the equipped weapon; Crystal's random curse/no-effect weapon-oil outcomes and other equipment mutation refreshes remain pending.
- current item metadata requests now support Crystal client `RequestItemInfo` / ID 39 and server `NewItemInfo` / ID 32 packet behavior, with runtime responses backed by the imported Crystal item manifest.
- current item gain and stack merge flows now apply imported Crystal `StackSize` for mapped items, including capped potion stacks, partial merge leftovers, and single-count non-stackable items.
- current pickup, shop, auction, NPC item grant, and quest reward paths now run StackSize-aware bag-capacity checks before mutating state, so full bags do not consume ground drops, gold, listings, quest state, or reward inputs when the item cannot fit.
- simulation config now owns a shared account/character store, so login, new-character, start-game, log-out, and gateway command boundaries can preserve character state across sessions instead of always rebuilding from the starter defaults.
- current shared character saves preserve position, direction, HP/MP, gold, inventory, belt, equipment, quests, and skill cooldowns for reconnect-style flows.
- starter NPC interactions now resolve through data-driven NPC script lookup plus NPC quest-id binding, with idle fallback for unscripted NPCs, instead of branching directly on the guide NPC object id.
- NPC dialog snapshots now expose parsed Crystal-style dialog links, allowing client/UI work to consume backend-resolved NPC choices instead of reparsing script text.
- optional JSON-backed account storage is now available through `SimulationConfig::with_account_store_path` and the gateway `MIR2_ACCOUNT_STORE_PATH` setting, so account/character saves can survive a fresh config/process reload.
- JSON-backed account storage now carries `schemaVersion`, migrates legacy stores to the current schema, preserves corrupt source files while falling back safely, and writes through same-directory temporary files plus atomic replacement instead of direct overwrite.
- current map file/title is now runtime state and part of character saves, so reconnect flows can restore the selected map metadata instead of assuming the starter map every time.
- starter map-transfer and safe-zone records now exist in simulation config, runtime can execute a transfer rule by key, refresh `MapInformation` / `UserLocation`, and expose `mapFileName` plus `inSafeZone` in world snapshots.
- current Crystal NPC interpreter now persists NPC flag state in character saves, with backward-compatible load behavior for older save JSON that predates the new flag field.
- current Crystal NPC condition handling now covers `CHECK`, `CHECKCALC`, `CHECKCLASS`, `CHECKGENDER` / `GENDER`, `CHECKITEM`, `CHECKGOLD`, `CHECKQUEST`, `RANDOM`, `CHECKMON`, `CHECKEXACTMON`, and the pet checks `PETCOUNT`, `PETLEVEL`, and `CHECKPET`.
- current Crystal NPC condition handling now covers `CHECK`, `CHECKCALC`, `CHECKCLASS`, `CHECKGENDER` / `GENDER`, `CHECKITEM`, `CHECKGOLD`, `CHECKQUEST`, `RANDOM`, `CHECKMON`, `CHECKEXACTMON`, `CHECKMAP`, `CHECKMAPLIGHT`, `CHECKRANGE`, `CHECKHUM`, `CHECKCONQUEST`, `HASBAGSPACE`, `DAYOFWEEK`, `HOUR`, `MIN`, `ISADMIN`, `GROUPLEADER`, `GROUPCOUNT`, `GROUPCHECKNEARBY`, and the pet checks `PETCOUNT`, `PETLEVEL`, and `CHECKPET`.
- current Crystal NPC action handling now covers `MOVE` with coordinates, `TAKEGOLD`, `TAKEITEM`, `GIVEITEM`, `GIVEGOLD`, `GIVEEXP`, `GIVESKILL`, `GIVEPET`, `REMOVEPET`, `SET`, `MOV`, `CALC`, `LOCALMESSAGE`, `LOADVALUE`, `SAVEVALUE`, `GROUPGOTO`, `GROUPTELEPORT`, `BREAK`, `CLOSE`, `CLEARPETS`, `MONCLEAR`, and the event-script parameter/spawn path `PARAM1/2/3` + `MONGEN`.
- Crystal NPC parameter-page flow now supports argumentized labels like `@Guess(1)` resolving into section labels such as `[@Guess()]`, plus `%ARG(n)`, embedded `%A1`, `<$OUTPUT(A1)>`, and input-driven `%INPUTSTR` substitution for imported script flow.
- gateway/session/web now support the current backend NPC input loop end-to-end, so Crystal-style `@@label` input waits can collect browser text and resume script execution against `%INPUTSTR`.
- player experience now lives in runtime/save state instead of being hardcoded-only, and current `UserInformation` packets expose the persisted runtime `experience` / `max_experience` values.
- current NPC `SAVEVALUE` / `LOADVALUE` uses runtime-backed key-value persistence stored with character saves, giving imported Crystal scripts a reconnect-safe baseline instead of being missing entirely.
- current admin/event condition handling now has runtime-backed Rust behaviour for `ISADMIN`, `CHECKMAPLIGHT`, `GROUPCHECKNEARBY`, and `CHECKCONQUEST`, so imported Crystal scripts no longer fall through those checks as unimplemented.
- current group script actions use configured runtime-visible party members, so `GROUPGOTO` and `GROUPTELEPORT` now execute as real backend behaviour instead of being absent.
- `CHECKQUEST` now supports Crystal-style `ACTIVE` / `COMPLETE` keywords instead of only the earlier numeric-stage fallback.
- current operational regression coverage includes a two-session shared account-store smoke, file-backed restart restore, save/reload-under-load coverage, bounded entity-count monitoring during a 1,200-tick simulation soak, and account-store backup/restore.
- WebSocket and TCP gateway sessions now save active characters on socket-close paths and wrap connect/action/snapshot/save boundaries in panic catches with stderr logging, so a session failure is surfaced as a client-visible Web error or a clean TCP session error instead of silently losing the final save path.
- `mir2-protocol` now exposes stable client/server packet trace entries with packet ids and variant names, and runtime regressions cover bootstrap, combat delayed-hit, and map-transfer packet ordering.
- Stage 5 broad-system runtime state now exists for group/guild/social/mail, trade/shop/auction, conquest/events, hero, mining, and crafting. The state is persisted with character saves, exposed through `WorldSnapshot`, reachable through gateway/browser `stage5Command`, and covered by runtime/UI smoke tests.
- The gateway now has a real TCP packet trace harness (`apps/gateway/src/bin/packet_trace.rs`) that captures local traces, can diff a live Crystal endpoint through `MIR2_CRYSTAL_TCP_ADDR`, and writes JSON evidence under `docs/generated/packet-traces`.
- The gateway now has real WebSocket and TCP load harnesses with process RSS sampling. Latest evidence under `docs/generated/load` shows 64/64 WebSocket clients ready with 0 errors and 64/64 TCP clients ready with 0 failures / 0 decode errors.
- current Crystal `AddItem` gains merge stackables across belt and inventory, prioritize player potion/scroll/script effect 1 and amulet belt ranges before bag fallback, and `UseItem` consumes the referenced belt slot for belt packets.
- current ground item drops follow Crystal `ItemObject.Drop(distance)` placement for player item drops, player gold drops, and monster ground drops: ring search, blocked-cell rejection, transfer-source rejection, `DropStackSize=5` item-object cap, and least-populated fallback cell selection are covered.
- current Crystal `Q` drop entries are no longer discarded before chance rolls; death and harvest paths attempt active matching quest-inventory gain and suppress normal ground/bag fallback when the quest item is not needed or the quest inventory is full.
- current generated drop manifests preserve nested Crystal `GROUP` entries, and runtime resolution recursively applies Crystal group semantics: child gold accumulates, `GROUP*` keeps one successful child item after child rolls, `GROUP^` stops after the first successful child, and nested groups compose through the same evaluator.
- current drop-created Crystal items roll MaxDura and the full current Jev random-stat family baseline using generated `RandomItemStats.ini` profile data plus deterministic equivalents of Crystal `UpgradeItem` / `RandomomRange`, carrying generic `UserItemStat` entries, curse flag, and socket slots through ground drops, pickup/harvest `GainedItem`, equipment/inventory state, and save/reload.
- current added-stat and socketed ground item drops expose Crystal `ItemObject` Cyan item-name colour through `ObjectItem` packets and world snapshots.
- current NPC buy-back entries are per player, survive save/reload, expire after Crystal `GoodsBuyBackTime=60`, move into NPC used goods, and used goods persist and can be purchased through Buy/BuyUsed flows.
- current socket-slot growth now checks imported item socket capacity before mutating equipment and only emits `ItemSlotSizeChanged` on successful capacity-backed growth.
- current seal flow now rejects already-sealed equipment without overwriting its expiry and only emits `ItemSealChanged` for a successful first active seal.
- current BenedictionOil can now add Luck, curse the weapon with negative Luck, or consume with no effect using Crystal-shaped branch rules.
- current seal flow can now validate and consume an optional source item for Stage 5 sealing, matching Crystal's source-item gate while keeping the legacy seal command path available for older harnesses.
- current socket-slot growth can now validate and consume an optional source item for Stage 5 socket growth, matching Crystal's shape-7 source and target-type unique-flag gate while keeping the legacy socket command path available for older harnesses.
- current seal flow now persists Crystal reseal-delay metadata: `NextSealDate` is exposed in `UserItem.SealedInfo`, survives save/reload, defaults safely for older saves, and blocks reseal after expiry until `Settings.ItemSealDelay=60` minutes has elapsed.
- current inventory-grid `CombineItem` now also covers Crystal repair-hammer and sewing source shapes `1/2/5/6`: wrong target families and `DontRepair` fail ack-only, full-durability targets emit `ItemNoRepairNeeded`, and successful repair-combine emits `ItemRepaired` after durability mutation and source consumption.
- current rental `BindingFlags` now survive runtime item/equipment state and surface in `UserItem.RentalInformation`; storage rejects rental `DontStore`, current socket/upgrade `CombineItem` rejects rental `DontUpgrade` ack-only like Crystal, and current `DropItem` now also rejects rental `DontDrop` ack-only.
- current inventory-grid `CombineItem` shape-3/4 upgrade success chance now applies equipment-backed player `GemRatePercent` from non-broken equipped item stats, matching Crystal's `Stats[Stat.GemRatePercent]` success-rate hook.
- current inventory-grid `CombineItem` no longer misroutes current-data `DurabilityGem` / `DurabilityOrb` stat `48` control metadata into a fake added stat, so current-data durability upgrades now follow Crystal's `MaxDura` branch and focused regressions lock the current-data durability, attack-speed, magic-resist, and durability-cap surfaces.
- current inventory-grid `CombineItem` current-data coverage now closes the remaining present-data shape-3/4 families and the shape-0 source failed-ack surface for the current manifest slice.
- current dead-player `ResurrectionScroll` now also respects map `CurrentMap.Info.NoReincarnation`, emitting `CannotUseOnMap`, preserving the item, and suppressing revive packets on blocked maps.
- current storage-family item actions now also require the recorded Crystal storage NPC object to still exist and remain within `CRYSTAL_DATA_RANGE`, so stale/out-of-range storage service context ack-fails across `StoreItem`, `TakeBackItem`, `MoveItem(grid=Storage)`, `SplitItem(grid=Storage)`, and any `MergeItem` touching `Storage`.
- current storage password set/unlock/remove now require the active in-range Crystal storage service context, successful password removal clears `LastSetTime` back to `0`, accepted passwords follow Crystal's alphanumeric `5..=15` format, and reopening `@Storage` resets the session unlock state before deciding whether storage contents can be sent, matching `ResetStorageUnlock()`.
- current `BuyItem`, `SellItem`, and `RepairItem`/`SRepairItem` now also require the recorded Crystal NPC object to still exist and remain within `CRYSTAL_DATA_RANGE`, so stale/out-of-range current NPC service context no longer mutates the implemented buy/sell/repair item surfaces.
- `mir2-protocol` and gateway JSON now also support Crystal `UserStorage` / ID 130, and successful current `@Storage` open now emits `UserStorage` before `NPCStorage` when storage is available while successful `UnlockStorage` emits `StorageUnlockResult` followed by `UserStorage`.
- repeated unchanged current `@Storage` opens now suppress duplicate `UserStorage` after the first send, matching Crystal `Connection.StorageSent` resend behavior while preserving the locked reopen/unlock resend path.
- current `@Storage` open now also sends `UserStorage` at the full backing storage length even when expanded access is inactive, matching `Account.Storage.Length` while higher-slot storage actions still follow `Account.IsValidStorageIndex`.
- expired expanded storage now downgrades to inactive during current `StartGame` state refresh, and the first world tick emits Crystal-style expiry chat plus `ResizeStorage` while persisting `Account.HasExpandedStorage = false` and preserving the 160-slot backing array.
- current `EquipItem(grid=Storage)` now resolves the exact storage item through the active `@Storage` service, and current `RemoveItem(grid=Inventory|Storage)` now follows Crystal's exact destination-slot semantics with ack-only packet shape instead of accepting `grid=Equipment` or falling back into another bag slot.
- current `RemoveSlotItem` now keeps Crystal's bounded source-grid envelope for the modeled runtime, so invalid `grid=Equipment` requests and unmodeled `Mount` / `Fishing` / `Socket` slot-item requests ack-fail without falling through into whole-equipment removal, including socket requests that only match the parent equipment id.
- current `UseItem(grid=HeroInventory)` no longer falls back into matching player bag items while hero inventory is unmodeled, current `SplitItem(grid=HeroInventory)` now failed-acks without mutating matching player bag stacks, current `EquipItem(grid=HeroInventory)`, `RemoveItem(grid=HeroInventory)`, and `RemoveSlotItem(grid=HeroEquipment|HeroInventory)` now likewise failed-ack without mutating matching player inventory/equipment, current `MergeItem` hero-grid requests now failed-ack without extra chat or player-bag mutation, current `MoveItem` unsupported-grid parity now covers `HeroInventory`, `HeroEquipment`, `Equipment`, `Fishing`, `Trade`, and `Refine` ack-only failures without extra chat or player/equipment mutation, current `MoveItem` storage-lock and invalid-slot failures now also stay ack-only, current `MoveItem(grid=Storage)` now requires the active Crystal storage service, successful current `MoveItem` Inventory/Storage paths no longer emit runtime-only success chat, current `MergeItem` unsupported-grid parity now also covers `Equipment`, `Fishing`, `Trade`, and `Refine` ack-only failures without extra chat, current Inventory/Storage `MergeItem` failure and success paths no longer emit runtime-only chat, and current `MergeItem` now supports Crystal-style `Inventory <-> Storage` plus modeled `Inventory <-> Belt` stack merges with the correct current storage-service / ack-only failure guards.
- current Crystal current-map visible respawns now keep low-density AOI representatives but distribute them over nearby walkable spawn/data-range cells instead of collapsing every monster at the respawn origin. The live gateway release `20260521T0830Z-spreadrep` was verified on production with representative map screenshots for BichonProvince, WoomyonWoods(S), NaturalCave, DeadMineEntrance, InsectCave_2F, and ZumaMaze under `docs/generated/player-qa/live-map-monsters/`; all captured states reported `network404=0`, Monster meta `503=0`, and Monster PNG failed count `0`.

### In Progress

- packet-level AOI enter/leave notifications beyond the current spawn/action/remove ordering fix
- map event script bindings, map weather/lightning/fire flags, and exact transfer/event packet parity beyond the imported movement/safe-zone baseline
- broader consumable/equipment use pipeline, including special-case item types, exact item-roll edge semantics, remaining hero-inventory item packet routing gaps beyond the current `DropItem` / `CombineItem` / `UseItem` / `SplitItem` / `EquipItem` / `RemoveItem` / `RemoveSlotItem` / `MergeItem` / `MoveItem` guards, the unmodeled successful current `RemoveSlotItem(Mount|Fishing|Socket)` surfaces plus the unmodeled current `MergeItem` `Inventory <-> Equipment` amulet-only and `Inventory <-> Fishing` bait-only surfaces that still depend on missing local equipment/fishing slot state, and remaining unsupported-grid `CombineItem` family gaps. `R90` is complete; `R91` now targets the next remaining backend parity gap outside this bounded slice.
- broader monster target acquisition / attack-type parity beyond current guard + default-MonsterObject + spider/plant/CaveMaggot/ToxicGhoul/ThunderElement/DarkBeast/WoomaTaurus/dig-out-zombie/reviving-zombie/axe-skeleton/zuma/RedThunderZuma/ZumaTaurus/IncarnatedZT/BoneLord/MinotaurKing/DarkDevil/OmaKing/GreatFoxSpirit/ManectricKing/SeedingsGeneral/RestlessJar/HellKeeper/GeneralMeowMeow/TucsonGeneral/TrapRock/Armadillo/ArmadilloElder/ManectricClaw/KingScorpion/Khazard/MirStatue/GuardianRock/RedMoonEvil/EvilCentipede/Yimoogi/Lamia/FrostTiger/IceGuard/FrozenMiner/FrozenAxeman/FrozenMagician/SnowWolf/FrozenWarewolf/SnowYeti/DarkWraith/CrystalSpider/TucsonMage/TucsonWarrior/bone-spearman/right-guard/left-guard/HellFire/ShamanZombie/BlackFoxman/RedFoxman/WhiteFoxman/WaterDragon/BlackTortoise/cat-family/DevilNode/Hen/Pig/Bull/Deer + player-lock + struck baseline; the generated AI summary currently shows 0 spawned AI families still using generic baseline behavior
- broader NPC semantic parity beyond command-name coverage; the generated NPC command summary no longer has unknown command names, but every imported script path still needs representative behavior comparison before claiming packet-perfect Crystal parity
- deeper summon-family parity beyond the current Shinsu/StoneTrap/SnakeTotem/VampireSpider/SpittingToad baseline, especially edge-case spell timing and remaining summon bodies
- combat timing / delayed-damage parity beyond the current monster delayed-hit baseline
- skill and buff runtime structures
- For the current migration scope, the Rust backend now functionally aligns with the Crystal-backed gameplay slice tracked in this document; further work is expansion beyond the present imported parity target, not remaining blocker parity gaps.
- Crystal parity tooling and module checklist
- live side-by-side Crystal packet capture and behavior diffing for the representative trace flows
- longer operational soak, production telemetry, and alert/rollback readiness beyond the current 64-client WS/TCP load smokes
- packet-perfect Crystal social/guild/mail/trade/auction/conquest/hero/mining/crafting behavior beyond the current functional baseline

### Not Started

- map event script binding import and exact map weather/lightning/fire behavior
- full monster AI behavior parity for all spawned AI families
- full repair NPC/store economy parity beyond the current active-page, storage-page, cost, and rejection baseline
- spell tables, cast rules, projectile objects
- full semantic NPC script parity beyond the current 81/81 command-name baseline
- durable persistent world/map state beyond account/character JSON saves
- imported full Crystal shop tables, auction packet shapes, guild war schedules, hero equipment/inventory AI, and refining probability tables

## Working Rule

When behaviour differs, Crystal is the source of truth. Rust structure does not need to mirror Crystal classes, but gameplay results and packet-visible state eventually must.

## 2026-07-23 Snapshot/Light Parity Note

Crystal monster `AI` now survives the full simulation snapshot -> shared zone
merge -> `ObjectMonster` observer-seed path instead of falling back to zero.
This fixes packet-visible minimap disposition for AI 6 guards without changing
authoritative monster movement or combat. A server-only fixed light setting
(`MIR2_SIMULATION_FIXED_LIGHT_SETTING=1..4`) was added for deterministic QA;
normal servers continue to use the Crystal UTC time-of-day formula when the
override is absent or invalid. Focused AI/light regressions and locked
Simulation/Gateway checks pass.

## 2026-08-12 Zone Restart and Login Availability Note

Production recovery now distinguishes durable world state from process-local
session state. World Director checkpoints no longer persist or restore players,
Zone session mappings, outbound registrations, pending packets, or in-flight
trade/rental state. Live and restored per-player pending queues are bounded to
the 1,024-message RPC capacity, and the journal compactor is part of the clean
source build with per-Zone fencing.

The exact incident checkpoint was verified offline: all 16 Zones restored, 20
orphan sessions plus 910,015 undrainable packet frames were removed, and the
embedded factory checkpoint contracted from 49,987,670 to 6,378,866 bytes.
This is backend availability/persistence hardening; it does not change the
Crystal gameplay-parity percentage or replace post-deploy WSS, human, device,
and soak acceptance.
## 2026-08-12 Monster speed and death-state parity

Shared monsters now retain Crystal `MoveSpeed` and `AttackSpeed` through the
session/shared-map spawn boundary and schedule Zone AI with those real
millisecond values. Gateway suppresses duplicate personal-session motion for
shared monster ids, leaving the Zone as the only packet-visible movement
authority. Regression coverage locks Scarecrow's 1500 ms movement interval and
proves a killed monster emits its authoritative death/drop sequence, remains a
dead corpse, and seeds as dead to a late joiner. All 157 shared-zone integration
tests and simulation/gateway checks pass. Production rollout remains separate
from this local parity result.

## 2026-08-26 Packet melee defence parity

The personal-session melee packet path now captures Crystal's declared defence
type when it queues delayed weapon-skill damage: `FlamingSword` uses AC without
Agility dodge and `Thrusting` uses Agility. The scope is restored immediately
after scheduling so other pending combat is unchanged. The milestone harness
also re-arms consumed one-shot melee skills before each bounded attempt.

Verification passed the 15-case Platinum 1.76 combat milestone twice with
identical case/assertion payloads and all seven assertions true. The previously
stable failure now records Warrior level 45 / D504 / ZumaGuardian at 23 damage,
7 MP spent, and 788 HP remaining. Focused FlamingSword/Slaying and Thrusting
packet tests also pass. This is backend Candidate evidence only; same-EXE UI,
live WebSocket, real-DPI, 30-minute native soak, human visual/feel, and official
release-signing gates remain open.

## 2026-08-26 Five-class creation and Wizard q10-q12 slice

The full Crystal runtime now creates all five classes with the imported
class/gender-filtered `Envir.StartItems` loadout. Assassin and Archer creation
are covered through normal `NewCharacter -> StartGame`, including their
`HoaSword` and `WoodenBow` starts. The Platinum 1.76 content profile remains a
separate three-class contract and was revalidated unchanged.

The original Wizard newcomer branch is now automated from q10 through q12. It
crosses from Assistant Jane to MasterMage_Don on `0115`, earns kill credit from
ten real Oma and ten real RakingCat deaths, retains the original `OldLoafer` and
`FireBall` book rewards, completes with MirGuide_Peter, and preserves quest,
inventory, experience, gold, position, direction, and class state across
logout and a new session.

Clean revision `004549e9f15ca6fa4b7fad119cb305fcad7d3230` passed the new Windows
functional gate's seven fixed controls in 692,198 ms. The aggregate includes
native host 312/312, `vertical_slice` 10/10, ordinary loop 2/2, security 18/18,
shared Zone 195/195, Gateway reload 1/1, and Web typecheck. The evidence summary
SHA-256 is
`0590F2CEA720E69FA8755C34A0D22580A3F631647351BBF3C6F4DC136631753B`.

This is a bounded automated functional slice. Its scoped controls are 100%
green, but global Crystal parity remains unscored (`globalParityPercent=null`)
until the semantic inventory and denominator are complete. It does not replace
same-EXE human UI/live-WebSocket, real DPI, 30-minute native soak, human
visual/gameplay-feel, or official signing acceptance.

## 2026-08-26 Taoist q13-q15 source journey

The next original newcomer branch is now automated for a level-four Taoist.
Quest 13 is absent for the same-level Wizard control, becomes available for the
Taoist, and follows Assistant Jane to loaded object 11, HighPriest Jude, on map
`0`. Quest 14 credits ten real Oma and ten real RakingCat deaths through player
combat authority. Quest 15 returns to loaded object 26, MirGuide Peter.

The journey proves the original q13/q14/q15 reward sequence: 48/180/48 EXP and
60/45/60 gold. `OldLoafer` and `Healing` are retained in inventory, and
`Healing` remains a skill book rather than becoming an invented automatic
learn. Completed quests, rewards, transform, class, level, experience, gold,
and known-skill state survive logout and a new `SimulationSession`.

The Windows functional gate now begins with deterministic map-atlas generation
so a clean hosted checkout exposes the complete native asset root. It also
captures Cargo stderr correctly under Windows PowerShell 5.1 while still using
the native exit code as the fail-closed result. Revision
`23ac6012adfd4132896f01642b96ab210320065b` passed 8/8 fixed controls in
640,891 ms, including native 312/312 and the expanded functional slice 11/11.
The summary SHA-256 is
`23593A5E4CC564DA9D38729ED4FEE36C6EC93C54D7EEB9629377BDCFACE8EE80`.

This closes another bounded source-backed functional branch, not whole-game
parity. The global percentage remains undefined until the semantic inventory
and denominator are complete, and same-EXE UI/live WSS, real DPI, 30-minute
native soak, human visual/gameplay-feel, and formal release signing remain
external gates.

## 2026-08-26 Gateway ordinary-packet quest boundary

The Bichon starter loop now crosses the Gateway and shared-Zone boundary using
only ordinary Crystal client packets. A fresh account and Warrior walk to
Village Guide, open the server-owned dialog, accept quest 1001, kill a real
Field Wasp through authoritative combat, pick up its visible tile gold, finish
the quest, log out, and reload through a fresh Gateway session. No high-level
runtime action or QA/admin command substitutes for these packet paths.

The test also proves the security boundary: remote accept and finish packets
without a nearby active dialog do not mutate quest state. The valid path grants
and consumes the Wasp Stinger proof, awards exactly 300 gold, two
`RareCopperOre`, and `CopperRing`, then preserves completed quest state,
inventory, equipment, gold, position, and direction across reload.

Clean revision `f676a2a81f9fae949d6640df747dedf493d913e9` passed 8/8 Windows
functional controls from `2026-08-26T07:32:31.5299048Z` through
`2026-08-26T07:43:42.0626754Z`. Gateway persistence is 2/2 in 33.917 s; native
is 312/312, the functional journey 11/11, ordinary 2/2, security 18/18, shared
Zone 195/195, and Web typecheck passes. The summary SHA-256 is
`DE942CEB2D105AD039C3758FF00C7160880BC213BAA4FFC99A6AA826B118C0B6`.

This expands a bounded automated vertical slice. It does not define global
Crystal parity: the semantic inventory and denominator remain incomplete, and
same-EXE UI/live WSS, real DPI, 30-minute native soak, human visual/gameplay
feel, and formal publisher signing remain open gates.

## 2026-08-26 Assassin q16-q18 journey

The original level-four Assassin branch now runs as one continuous source-backed
journey. A non-Assassin control cannot see q16. The Assassin accepts from
Assistant Jane, hands q16 to HighAssassin Cloud at loaded object 13 on map `0`,
completes Cloud's q17 test through ten real Oma and ten real RakingCat
player-owned deaths, then hands q18 to MirGuide Peter at loaded object 26.

The three hand-ins preserve the original 48/180/48 EXP and 60/45/60 gold
sequence. `OldLoafer` and `FatalSword` remain inventory rewards;
`FatalSword` is a skill book and is not silently converted into a learned
skill. Completed quests, inventory, equipment, class, experience, gold,
position, direction, and empty known-skill state survive logout and reload.

Clean revision `82441f7b1257486d6f2b51206f5cffa4ef20f9b8` passed all 8/8
Windows functional controls from `2026-08-26T08:04:06.0201767Z` through
`2026-08-26T08:17:03.3518161Z`. The functional suite is 12/12 in 670.046 s;
native is 312/312, ordinary 2/2, security 18/18, shared Zone 195/195, Gateway
2/2, and Web typecheck passes. Summary SHA-256 is
`D1A9ACE4920A834541B5798BBE53F38DEE1D37DD261226DD91394C82AA8BC105`.

This is another bounded automated branch, not whole-game parity. Global parity
remains undefined until the semantic inventory and denominator are complete;
same-EXE UI/live WSS, real DPI, 30-minute native soak, human visual/gameplay
feel, and formal publisher signing remain open.

## 2026-08-26 Archer q19-q21 journey

The original level-four Archer branch now runs as one continuous source-backed
journey. A non-Archer control cannot see q19. The Archer accepts from Assistant
Jane, hands q19 to Captain Jerald at loaded object 14 on map `0`, completes
Jerald's q20 test through ten real Oma and ten real RakingCat player-owned
deaths, then hands q21 to MirGuide Peter at loaded object 26.

The three hand-ins preserve the original 48/180/48 EXP and 60/45/60 gold
sequence. `OldLoafer` and `Focus` remain inventory rewards; `Focus` is a skill
book and is not silently converted into a learned skill. Completed quests,
inventory, equipment, class, experience, gold, position, direction, and empty
known-skill state survive logout and reload.

Clean revision `d01910a1694d45e85dc54eafab6e61c43a063f5f` passed all 8/8
Windows functional controls from `2026-08-26T08:36:39.4707209Z` through
`2026-08-26T08:51:19.8844889Z`. The functional suite is 13/13 in 773.360 s;
native is 312/312, ordinary 2/2, security 18/18, shared Zone 195/195, Gateway
2/2, and Web typecheck passes. Summary SHA-256 is
`BE47F67645A9DF165635C173CF2F04BB85895B5DC6666F8F8DE3E963BC721197`.

This completes automated original instructor journeys for Wizard, Taoist,
Assassin, and Archer, but remains bounded source evidence. Global parity is
undefined until the semantic inventory and denominator are complete; same-EXE
UI/live WSS, real DPI, 30-minute native soak, human visual/gameplay feel, and
formal publisher signing remain open.

## 2026-08-27 Gateway original q1-q4 lifecycle parity

The original Bichon q1-q4 chain now crosses the Gateway/shared-Zone boundary as
one ordinary-packet journey. It covers the initial five-leaf transfer, real
Scarecrow kills and probabilistic `GingerTea`, the original three-choice q3
weapon dialog and selected `SharpDagger`, then neutral Deer melee, multi-pass
Crystal corpse harvesting, five probabilistic `DeerMeat`, exact hand-in rewards,
logout, and a new-session reload. Navigation uses the bundled `0.map` collision
atlas; no runtime movement, direct quest mutation, synthesized item, or QA/admin
command substitutes for the player path.

The acceptance rerun caught a stale harvest tombstone from a preceding monster
incarnation. Gateway map reconciliation now enforces that a live Zone-native
monster is not harvested, and a fresh live-to-dead transition clears only the
older incarnation marker. A repeated death packet after current-corpse harvest
does not reopen it. Focused unit regressions and the strict q1-q4 journey both
pass, so this is a lifecycle repair rather than a test retry/skip.

Clean revision `e1290bea3de1bdcd1663ee0f823c849c937eff3d` passed all 8/8
Windows controls from `2026-08-26T21:16:58.2471471Z` through
`2026-08-26T21:45:46.6861359Z`: native 312/312, functional 13/13, ordinary 2/2,
security 18/18, shared Zone 196/196, Gateway 5/5, Web typecheck, and deterministic
map-atlas preparation. Summary SHA-256 is
`8C942979F9D59178C33BC72D5BAAD0F3986348B76F331EAF6B0C0DF003714849`.
The Gateway-only 10 ms QA movement wait is recorded in the evidence and does not
change combat, drop, harvest, reward, or persistence semantics.

This is 100% only for the declared eight automated controls and is follow-on
source evidence, not a newly packaged Candidate EXE. `globalParityPercent`
remains `null`; same-EXE UI/live WSS, real 125%/150% DPI, a real 30-minute native
soak, human original-client visual/gameplay-feel acceptance, the complete
semantic denominator, and formal publisher signing remain open.

## 2026-08-27 Web runtime artifact consistency

The current runtime source generated `bevy-5046abca14947f40` twice on the same
Windows evidence host, replacing the stale tracked manifest for
`bevy-1813be587ef98bc1`. Both local builds produced manifest SHA-256
`4EC8644042F6926D7D724A7E7E500BA7DAFA1476B49780DF7EFAC7AEEC4806C1`, and the
full production build passed both WASM budgets, 9,650 entity frames, 40,763
original assets, the 57-page map atlas, TypeScript, and 13/13 static pages. This
is same-host repeatability, not a cross-machine byte-reproducibility claim.

Developer Handoff first verifies the immutable prebuilt package and, when it is
absent, compiles with runtime-pinned Rust `1.95.0` and wasm-bindgen `0.2.118`.
The prebuilt path retains its exact four-file repository content lock. A source
fallback instead verifies that the active manifest matches its generated files,
both WASM modules validate, both backend budgets pass, and the complete Web
build succeeds before restoring the tracked manifest. This is a clean-checkout
artifact gate only: no production R2/deployment was changed, no server parity
percentage is added, and all live/human/release gates remain open.

The first exact-head source-fallback attempt failed before compilation because
the wasm-bindgen install pinned Cargo but not its rustc child, allowing the
Windows runner's default stable toolchain to update concurrently. The install
now pins `RUSTUP_TOOLCHAIN` plus `RUSTC` in-process. The developer image and both
developer wrappers additionally carry a missing-prebuilt source fallback, with
fault-injected Bash/PowerShell contracts proving the branch is taken. This is
CI/developer-environment hardening only; the real exact-head Linux Compose smoke
remains pending and does not change any parity numerator or acceptance field.

Linux clean-checkout source compilation produced valid host-local
`bevy-a314c804ae9919d3`, distinct from tracked `bevy-5046abca14947f40`.
Exact-head run `33023784689` then passed source compilation on all four matrix
hosts but showed Windows CI emits `bevy-efe7c0554bdf9a45`; its JS wrappers were
unchanged and only the WASM hashes differed. Inspection confirms release WASM
contains absolute Cargo-registry source paths, so a cross-account byte comparison
is not a valid source-equivalence gate. The matrix now validates the generated
bundle on every host and restores only its host-local manifest before proving
the checkout is otherwise clean. The separately fetched immutable prebuilt
package remains SHA-locked to the tracked manifest.

Exact-head Handoff run `33023777224` independently rebuilt the same CI-host
`bevy-efe7c0554bdf9a45` and passed the complete Player Web build before failing
only the superseded tracked-manifest comparison. The repeated host-local id
supports the corrected integrity/budget gate and is not a server-parity or
deployed-runtime claim.

## 2026-08-27 Shared-Zone single-authority correction

Hosted Candidate q2 produced 121 aggregate `ObjectDied` packets while recording
zero confirmed player-owned kills. The diagnosis was not insufficient player
damage: shared Gateway ticks drained the authoritative Zone and then ran the
full personal-session monster/hazard tick, creating an independent second world
writer. Revision `fb7cd29e8a0afdd09cd7f3f3592ed5fa1c6c5dff` advances only
personal compatibility state in this path. Zone remains the public authority
for monster movement, combat, death, drops, experience, and hazards; private
movement retry and intelligent-pet ECS drop pickup are also excluded.

The existing personal spawn table continues to provide the Crystal respawn
schedule and an explicit revive boundary that the Zone reconciles. Moving that
timer to a Zone-native wall-clock/checkpointed scheduler is still required for
strict whole-game architecture parity and is not counted as closed here.

The q4 western-Deer search changed from a 36-tile grid to 16-tile spacing so no
safe static slot falls outside the final snapshot AOI plus walk-stop radius.
Final-source results are Gateway unit 653 passed, 0 failed, 1 ignored; focused
single-authority 1/1, geometry 1/1, and ordinary-packet q1-q4 1/1 in 455.10 s.
Q2 completed after three confirmed player-owned kills; q4 collected five meat
after seven confirmed player-owned kills. The Windows evidence self-test is
8/8. Only `MIR2_QA_NATURAL_MOVEMENT_DELAY_MS=10` was active; no combat,
damage, drop, harvest, or reward acceleration was used.

The older attested EXE from
`4c7e60baa5d85e63858a6fd1717af01c5f893f3d` does not contain this source
correction, so it cannot be promoted as the new exact-head Candidate. The
declared automated controls remain 100%, but `globalParityPercent=null`,
`accepted=false`, and `visualAccepted=false`. Same-EXE UI/live WSS, real
125%/150% DPI, native 30-minute soak, human original-client visual/gameplay
feel, the complete semantic denominator, and formal publisher signing remain
open.

## 2026-08-27 Exact-head native artifact binding

`WN-CANDIDATE-04-20260827` now binds the correction to clean revision
`4074445ccac7c73adcf34c2e6fc775210d6c8a50`. Its 66,665,472-byte EXE has
SHA-256 `60A3C78D401385E6294FB129FABA50BA9E0EE0253F1C1A572FF0B9F2B70C6CB9`;
the build-attestation SHA-256 is
`25643FE5883152FBB7BE7EC6AE68340B5810FF9712A43DB6F579885047429765`.
The package holds 10,258 files / 325,281,417 bytes. Manifest SHA-256 is
`043F565024955ED4570D898FB7CE6C20CBEBE02D0993895E80A1E43CBB8ED2E9`,
and its aggregate SHA-256 is
`3DCEADF75D9EE64607B5322525886C0EF0946F170A40CAB8C105E6F17AC1A325`.

The formal package command exercised `npm.cmd`; built-in and independent
verification both passed with exact-source checking, PE validation, detached
CMS validation, `nonvisual=true`, and `launchRequested=false`. No client was
started. The CMS certificate is internal/self-signed and does not satisfy the
formal Authenticode publisher gate. This artifact closes only the exact-head
nonvisual binding; it does not increase or define whole-game server parity.
