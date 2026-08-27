# Backend 1:1 Progress

> VIS-03 Character HUD client-boundary checkpoint (2026-08-28): revision
> `849f1f0b5120867d1358e0e7db9ba675e9866f9c` changes no simulation, Zone,
> Gateway or gameplay authority. The native client locally binds the exact
> Character control assets/geometry, one enabled pointer ButtonA edge and the
> source CharacterPage-aware callback. C/F10 reuse the state transition but
> emit neither sound nor network intent. Bevy 401/401, Windows 376/376,
> focused 4/4, script self-tests and final P0=0/P1=0 review pass. No live WSS,
> EXE, package or visual/audio evidence was produced. The panel contents and
> all same-EXE/DPI/soak/human/signing/denominator gates remain open. No
> backend, visual or global percentage is claimed.

> VIS-03 Inventory ButtonA client-boundary checkpoint (2026-08-28): revision
> `5b70511316b084ac677b5978f7f03e440241ca4c` changes no simulation, shared-
> Zone, Gateway or gameplay authority. The native client locally reproduces
> Crystal's enabled Inventory mouse-click `ButtonA=10103 -> 103.wav` edge
> before the panel toggle; keyboard F9/I is intentionally silent. Typed UI
> audio is isolated from packet-authoritative gameplay audio, and package/
> verify identity-bind the exact WAV. Windows 376/376, Bevy 397/397, focused
> 4/4, both script self-tests and independent P0=0/P1=0 review pass. No live
> WSS, EXE, package or audible-device evidence was produced. Other controls
> and all same-EXE/DPI/soak/human/signing/denominator gates remain open. No
> backend, visual or global percentage is claimed.

> VIS-01 selected-target client-boundary checkpoint (2026-08-28): revision
> `a58ab0aaa2202731a5c55e7a684261d6c15c2f8d` changes no simulation, combat or
> shared-Zone authority. It consumes the existing selected object ID and typed
> player/monster projection to reproduce Crystal's full actor redraw at 0.3
> opacity, with atomic atlas closure and lifecycle cleanup. It also separates
> client render depth so default-foreground ObjectEffect/MapEffect and actor
> effects draw after selection, while persistent ObjectSpell stays in the
> world pass. Windows 376/376, Bevy 393/393, shared runtime 191/191 and focused
> depth/selection tests pass; independent review found no P0/P1. No live WSS,
> server ownership, package, EXE or visual evidence was recertified. Hover,
> HighlightTarget setting, general DrawBehind classification and every final
> real-window gate remain open. No backend, visual or global percentage is
> claimed.

> VIS-02 GreatFireBall client-boundary checkpoint (2026-08-28): revision
> `9457e5618449d22350baedd01e3775f5b1fe59c6` adds no gameplay or shared-Zone
> authority. It consumes existing typed `ObjectMagic(GreatFireBall)` as
> Crystal's client-owned immediate cast plus delayed local projectile and
> target-bound impact, while ignoring the Rust compatibility
> `ObjectProjectile`. Sixteen direction ranges, M34-0/M34-1/M34-2 identities,
> target-removal/lifecycle cleanup and clean-checkout package closure are
> automated. Windows 372/372, Bevy native-ui 393/393, focused effects 5/5,
> Gateway projection 1/1, Web type/full logic, exporter/offline assets and
> package/verifier self-tests pass; final review has no remaining P0/P1. The
> fixture is projection-only and its `cast=false` event is compatibility-only.
> Server cast/damage/revalidation semantics were not recertified here;
> retained-dead impact suppression, authenticated delivery and every same-EXE/
> GPU/DPI/soak/human/signing/denominator gate remain open. No backend or global
> percentage is claimed.

> Web Crystal ActionFeed client-boundary follow-up (2026-08-28): revision
> `7bc42cfd77e196297b165436716484732db18d83` changes no simulation or shared-
> Zone authority. It consumes consecutive authoritative Struck packets as the
> source client does: one current action, one queued tail, tail duplicate drop,
> deferred queued location/direction and deferred second audio. Death, revive
> and MapChanged clear the client queue; snapshot refresh retains it otherwise.
> Full Web logic/type checks, focused state/store/event coverage and the offline
> resource/audio gate pass, and independent review found no P0/P1. The same
> revision regenerates the deterministic sound manifest for existing
> `M79-1.wav`, fixing the prior exact-head CI resource-gate omission locally.
> Authenticated live packet/audio ordering, final-head CI and every same-EXE/
> GPU/DPI/soak/human/signing/denominator gate remain open. No backend, visual or
> global percentage is claimed.

> Native player combat-audio client-boundary follow-up (2026-08-28): revision
> `144226df3c7a81ae7e7b15866ae4091d610fffb8` changes no simulation or shared-
> Zone combat authority. It consumes existing Struck/Death/Revive/MountUpdate
> state for exact body/armour, mount, flinch, delayed death and revive audio;
> preserves lethal hit ordering, cancels delayed cues on lifecycle boundaries,
> deduplicates the owner's two revive aliases and uses authoritative numeric
> weapon identity for mounted attackers. The Native allowlist and Candidate
> scripts now identity-bind 15 combat WAVs plus M79. Windows 367/367, rustfmt,
> package/verifier self-tests and independent no-P0/P1 review pass. Web
> ActionFeed queuing, Crystal-random tiger selection, authenticated live audio
> and all same-EXE/GPU/DPI/soak/human/signing/denominator gates remain open. No
> backend, visual or global percentage is claimed.

> Player combat-state client-boundary checkpoint (2026-08-28): revision
> `9eaa62283ec453bfa42f8bc3cbddb4c8811abf09` changes no simulation or shared-
> Zone combat authority. It repairs consumption of the existing authoritative
> `ObjectHealth(0) -> ObjectDied` order, keeps numeric health separate from
> death/revive state, preserves one death incarnation across snapshots and
> consumes self/remote revive with the source effect gate. Windows 360/360,
> Web full logic/type and package/verifier self-tests pass; independent final
> review found no P0/P1 within the bounded claim. The Web one-slot Struck model
> still is not Crystal ActionFeed queuing, and Native generic hit/flinch/death
> audio is absent. No backend, visual or global percentage is claimed.

> VIS-02 FlamingSword client-boundary checkpoint (2026-08-28): revision
> `160e8d3ccc0eb17f8e49b6505c5a58666a35029f` adds no gameplay authority. It
> preserves `ObjectAttack.spell=8` through Gateway and closes bounded native/
> Web Attack1 overlay and audio consumption, including ordinary-attack
> isolation and lifecycle cleanup. Windows 357/357, runtime 191/191, Bevy
> native-ui 393/393, focused effects 5/5, Gateway projection 1/1, Web/full
> resource gates and package/verifier self-tests pass; final review found no
> P0/P1. Existing personal/shared toggle and next-melee consumption code was
> not changed or live-revalidated here. Authenticated single-consumption/order
> evidence, the wider combat/backend matrix and all same-EXE/GPU/DPI/soak/
> human/signing gates remain open. No server or global percentage is claimed.

> VIS-02 FireWall client-boundary checkpoint (2026-08-28): revision
> `f6f78f3eddb813897cf4ce4c6056183130ab7f35` adds no gameplay authority. It
> closes bounded Windows presentation automation for the 600 ms cast,
> M39-0/M39-1, and five persistent center/cardinal `ObjectSpell` projections
> using repeating `Magic/1630..1635`. Native 351/351, Bevy native-ui 393/393,
> focused effects, Gateway projection, Web resource/type gates and package/
> verifier self-tests pass. The fixture is not an authenticated transcript;
> its `cast=false` branch is labeled synthetic and outside the canonical
> production timeline. Existing Zone 500 ms geometry, 2,000 ms damage tick and
> duration support were not changed. Collision/duplicate omissions, caster
> cleanup, expiry, oldest-group replacement and observer identity still need a
> complete exact current-head backend matrix. No server or global percentage
> is claimed.

> Windows visual projection VIS-00 baseline / VIS-01 and VIS-02 in-progress / VIS-03 bounded checkpoint
> (2026-08-27): shared authority
> still owns actor identity and state, while the Gateway/native projection now
> preserves remote player class, gender, guild, hair, armour, weapon,
> mount/fishing mode and normal/Transform body selection instead of reducing
> every remote player to an unstyled body. `ObjectHarvest` and
> `ObjectHarvested` now reach native
> Harvest/Skeleton actions; Skeleton is persistent until Revive. The focused
> Gateway appearance regression passes 1/1. The first VIS-01 increment now
> treats real `ObjectMonster(image=10)` as CannibalPlant, plays its source
> Show/Hide lifecycle, keeps unknown/non-Cannibal Hide on the old removal path
> and adds Scarecrow `Monster/005` Die-phase `224..233` as a packed-atlas
> additive post-world layer. Its depth shares the actual map producer's
> six-cell guard-band/front contract, and the layer follows the Effect option
> without a new Gateway packet. Commit `ef619b551` closes the bounded typed
> projection transcript: incremental monster packets carry authoritative
> sprite data, preserve snapshot disposition or fail closed to neutral, and
> retain death location/direction/kind. Seventeen typed events drive the six
> VIS-01 actors through 15 exact render checkpoints and production frame-set,
> atlas and real-`0.map` render-state bindings. Review follow-up `434bb06e6`
> observes raw snapshot disposition before overlay merge so relationship
> changes remain authoritative, and integrity-checks all seven atlas pages in
> runtime, production test, source package and copied-Candidate verification.
> The complete client runtime library passes 191/191; focused latest-head
> Gateway/native gates pass. VIS-02 now adds one bounded Lightning projection:
> typed `ObjectMagic` cast authority reaches the native 600 ms delayed,
> caster-attached six-frame effect and exact one-shot audio without fabricated
> projectile/impact. The existing shared Zone six-tile gameplay scheduling is
> unchanged. Fresh-source Windows assets now pass 333/333 after the functional
> gate was corrected to generate its required keyed/additive map pack. This is a
> presentation-boundary repair, not a claim
> that shared-Zone gameplay, whole-game semantics or Windows visuals are
> complete; the other four first-slice spells remain open. VIS-03 revision
> `448db4f72` adds no server authority: it preserves the source 1024x768
> Inventory three-state assets, gives BigMap Teleport its exact `Title/823`
> disabled art, and rejects teleport intents when the active search map is not
> the authoritative current map. This bounded client-state checkpoint passes
> Bevy native-ui 393/393 and Windows 333/333 plus package/verifier self-tests
> and an independent no-P0/P1 review; it is not live-WSS or raster evidence.
> Real Gateway/WSS order, GPU raster proof, additive weapon/wing
> layers, full assets, same-EXE live evidence, DPI, soak and human gates stay
> open under the visual
> contract.

> VIS-02 FireBall client-boundary checkpoint (2026-08-27): no new gameplay
> authority or server-completion percentage is claimed. Typed Gateway
> `ObjectMagic` now drives the source-timed native cast, delayed local missile
> and bound impact; the existing simulation compatibility `ObjectProjectile`
> is consumed only to avoid a duplicate visual. Sixteen direction ranges,
> finite target-tracking flight, M31-0/1/2 identities and full package/verify
> closure are automated at revision
> `d85d7368119053e6b2609316c4f5c76faaa298cb`. Gateway typed fixture 1/1,
> effects 59/59, Windows 340/340, Bevy native-ui 393/393, Web typecheck,
> offline resource/audio verification and independent no-P0/P1 review pass.
> Shared-Zone damage remains the authority. Target-dead impact suppression
> still needs an explicit authoritative dead bit at the effect boundary;
> FlamingSword, SoulFireBall, FireWall and all final visual/real-window gates
> remain open, so no global percentage is emitted.

> VIS-02 SoulFireBall client-boundary checkpoint (2026-08-28): revision
> `19991af6ddb289dc2fb22569849599caabf9195e` adds no gameplay authority. The
> Windows adapter implements Crystal's audio-only cast start, 600 ms local
> missile launch, 16-direction finite target tracking, target-bound impact and
> exact M64-0/1/2 asset closure while ignoring the Rust compatibility
> `ObjectProjectile`. Native 346/346, Bevy native-ui 393/393, focused effect
> and Gateway projection gates, Web gates and package/verifier self-tests pass.
> The Gateway fixture proves only `ServerPacket -> event` projection; the
> current production no-amulet branch emits no `cast=false` packet. Shared-Zone
> monster timing, PvP, authoritative target/range/flight validation and
> preflight/item atomicity remain backend gaps. Target-dead impact suppression
> and every real-window/final acceptance gate also remain open; no completion
> percentage is claimed.

> Windows verifiable vertical-slice evidence closeout (2026-08-26; packaged
> runtime source `b5c0ecb60`): this round records only evidence that passed.
> Simulation `vertical_slice` passes 8/8 in 283.03 seconds, including the
> focused original Bichon quest 1-to-9 route in 216.69 seconds; `shared_zone`
> passes 195/195. The newcomer flow uses the current authoritative quest item
> and CopperRing reward identities, safe-zone natural regeneration, and exact
> `GroundDropClaimedWithTicket` authority without QA gold, damage multipliers,
> or direct HP mutation. `ordinary_candidate_loop` remains 2/2, Gateway
> fresh-account persistence 1/1, ordered Zone restore plus `zone_rpc` 21/21,
> clean-source assets 312/312, Web typecheck, and the 15-case combat milestone
> certificate also pass.
>
> A deterministic accuracy defect was corrected in both personal-session and
> native Zone physical paths. The old tick coefficient was divisible by
> Scarecrow's agility modulus, allowing a fixed attacker/target pair to remain
> permanently hit or missed; the new avalanche-mixed roll varies across ticks
> while remaining deterministic across identical runs. Its exact agility-eight
> unit regression and focused native Zone high-evasion/replay checks pass.
>
> Runtime source revision `b5c0ecb604946a858bf5d060a2cca306032c0e62`
> produced an attested Windows Release EXE with SHA-256
> `9E51CBF3E81D50A182F08CE11D02D9829268881A2124BAFC1D963829CC634E8C`
> at 66,665,472 bytes. Build-attestation SHA-256 is
> `74F7D06336D486C6430263519282AED02C3B0429C6711FE0829DA7BE08311370`.
> Candidate `WN-CANDIDATE-03-20260826` contains 10,258 files. Its manifest
> SHA-256 is
> `58F88AD84D1F7F9C9CC1CC44E59932D2A39136FD62FCA1F56CDAB0CF6C861884`
> and payload aggregate is
> `6788698E6ED19209D5463B10FF15E5D7972D714C62C6D0093808571C97ABF83A`.
> Clean-source verification passed nonvisually with
> `sourceRepoCheck=checked`; the copied artifact passed an independent
> nonlaunch verification with `sourceRepoCheck=unavailable`, and all six root
> anchors matched. Candidate-03 supersedes Candidate-02 for current runtime
> evidence.
>
> This closeout does not claim whole-game or strict Candidate 100% parity.
> Windows pure-UI account-to-quest proof, a same-EXE authenticated live
> WebSocket run, real 125%/150% DPI, a real 30-minute native-client soak, and
> human visual/feel acceptance remain open. The statement is signed only by
> the internal self-signed detached-CMS certificate
> `B179E9D6222332C9DB5E960BAECF9990252CFBC7`; the EXE is Authenticode
> `NotSigned`, so a formal release certificate/signing path also remains open.
> Existing user changes outside the scoped evidence files are not reclassified
> by this entry.

> Crystal map-event binding E1 checkpoint (2026-08-26): all six current
> `_MAPCOORD` entries are generated as typed conditions/actions with exact
> script provenance and exactly one linked `Server.MirDB` `NeedMove` row.
> Personal-session and shared-Zone movement now consume only those typed
> bindings and fail closed on invalid or duplicate data. Generator 7/7,
> manifest/runtime 3/3, personal/shared integration 3/3, and the Gateway
> allowed-turn transfer regression 1/1 pass. The 18 general event files remain
> explicitly `open`; delayed event actions, doors/gates/walls, the complete
> six-gate Gateway matrix, live cross-map packet traces, and RNG remain P0 map
> work.

> Crystal map-environment import checkpoint (2026-08-26): the current
> `Server.MirDB` generator now preserves map music, fire/lightning enablement
> and damage caps, plus fire-wall limit metadata. The regenerated manifest has
> 464 maps and 12 hazard-enabled maps; `SimulationConfig` no longer relies on
> hand-seeded hazard rows for Crystal map/world profiles. The source count is
> 464 records: 463 named maps and one empty Crystal DB placeholder. The selected
> map's
> `MapInformation` now carries the exact hazard flag bits and music alongside
> the already imported light/dark-light/weather values. Game-data 1/1,
> map-hazard integration 3/3, and existing personal/shared hazard behavior 5/5
> focused tests pass; both generated consumers are byte-identical, their sync
> gate passes, and Web typecheck is green. This is a bounded
> data/packet closure, not complete map-event parity: general scripts,
> door/gate/wall actions, and exact RNG trace comparison remain open.

> 2026-08-26 Slice D durable settlement is closed for its bounded item-identity
> scope. PostgreSQL idempotency lookup/transact share an advisory transaction lock;
> uncertainty remains `OutcomeUnknown`; exact ground projection and save are atomic;
> detached claims survive world-only checkpoint restore with complete authority.
> Teardown/`Drop` now applies finalized packets only, while unresolved settlement is
> retried only with an ordered economy context. StartGame performs recovery after
> Zone join, and the fresh-process/new-login regression proves exactly one credit.
>
> Evidence: Simulation 1472/0, shared_zone 195/0, Gateway 642/0/1 ignored out of
> 643, social_economy 3/0, Web typecheck 0, exact-file Rustfmt 0, Gate18 bins 0,
> and diff check 0. Independent final audit is GO with P0=0/P1=0/P2=1; the P2 is
> a non-blocking service-layer `ContextUnavailable/Deferred` defense-in-depth item.
> This is not whole-game parity: an independent full-project audit currently puts
> backend Crystal semantic coverage at about 49% (45-54% range).

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
> GroundDrop identity Slice A/B complete (2026-08-25): complete recursive
> Crystal `UserItem` identity now remains lossless through drop creation,
> internal snapshots, checkpoints, Zone RPC, state roots, and current
> local/shared/quest pickup. Preflight and commit use one staged atomic planner;
> assigned UIDs are preserved or retired after full absorption, every changed
> stack emits `GainedItem`, unique legacy names canonicalize, ambiguous legacy
> identity fails closed, and idempotency is bound to the exact payload. External
> Web/spectator views stay redacted. Locked serial evidence is Simulation lib
> 1461/1461, Gateway lib 606 passed with 1 ignored, Web typecheck, exact-file
> Rustfmt, and diff check. Claim generations/ids, stable canonical digests, pet
> pickup routing, and durable crash recovery remain Slice C/D P1 work, so full
> backend parity is not yet declared complete.

> Clean-host Bichon collision correction (2026-08-25): active non-starter map
> `0` now uses `runtime_world_map_collision_data`, preserving the installed
> Crystal-client preference while adding the repository gzipped map-pack
> fallback. A clean Linux worker previously fell through to the starter slice,
> causing one full-map assertion and seven real-fishing-fixture tests to fail.
> The focused map gate passes 1/1 and all fishing tests pass 16/16. This is a
> loader-source correction only; it does not claim complete map/content parity.

> Zone factory recovery safety checkpoint (2026-08-25): a complete World
> Director Zone image is staged off-line and replaces the live Zone map only
> after every Zone validates and restores. Replica markers, resource creation,
> promotion/resume, atomic restore, and autonomous-tick initialization now share
> the fixed replicas -> Zones lock order, closing partial factory mutation and
> standby double-tick races. Atomic restore tests pass 2/2, replica tests pass
> 4/4, Rustfmt/diff gates pass, and locked Gateway compile passes. This does not
> claim the still-separate durable filesystem publication path.

> Crystal AI 41/42 support-node checkpoint (2026-08-25): YinDevilNode and
> YangDevilNode are now immobile non-chasing support casters. They require a
> same-disposition friend within seven tiles, broadcast the delayed Crystal
> `ObjectAttack`, and map AI 41 to `BlessedArmour/MaxAC` and AI 42 to
> `UltimateEnhancer/MaxDC` with the exact `target level / 7 + 4` value and five-second
> player Buff lifetime. Because target-owned monster Buff state is still absent, monster
> targets fail closed without a fake client Buff. Locked integration tests pass 2/2;
> monster-target Buff aggregation/expiry/snapshot remains explicitly open.

> Crystal map-coordinate runtime checkpoint (2026-08-25): all six active
> `_MAPCOORD` bindings are now evaluated after authoritative movement in both
> personal Simulation and shared Zone execution. `LEVEL` and `CHECKPKPOINT`
> consume the real player level/PK state (including the former NPC-script PK
> hardcode), denied gates emit the imported `Hint`, and authorized `ENTERMAP`
> rows feed the existing transfer authority. The Gateway's post-Zone movement
> handoff can therefore transfer only from an authorized snapshot row. Focused
> integration gates pass 2/2 over every active binding and the level 49/50 and
> PK 199/200 boundaries; locked Simulation compile also passes. This closes
> runtime execution for the imported six-binding subset, not arbitrary event
> scheduling or a general Crystal script interpreter.
>
> Crystal unmatched-spell safety correction (2026-08-25): imported spells
> without an explicit Rust implementation no longer fall through to a generic
> 500 ms offensive damage action. The default branch now follows
> `HumanObject` dispatch: MP is spent before the switch, `Magic` and
> `ObjectMagic` publish `cast=false`, and no cooldown, skill progress,
> projectile, or delayed damage is created. Explicit FireBall remains on its
> normal cast/projectile/damage path. Ten focused locked serial skill tests pass.
> This is a fail-closed correction; each still-unsupported spell remains an open
> implementation item rather than being claimed as functional.
>
> Crystal AI 50 recall correction (2026-08-25): GreatFoxSpirit now performs a
> bounded local `FindAllTargets(30)` candidate pass, filters unsupported/stale,
> dead, hidden, near, and same-disposition entities, applies MagicResist for each
> candidate, and stops after the first successful teleport exactly as Crystal's
> loop does. `RemotePlayer` mirrors are excluded from local mutation; `SelfPlayer`
> and opposing monster entities use their real object IDs in teleport packets.
> Player resistance is authoritative; monster resistance remains zero until an
> imported monster stat component exists. Locked focused tests pass 2/2.
>
> Crystal AI 48 correction (2026-08-24): GuardianRock's delayed pull now uses
> the same `MagicResist / MagicResistWeight` all-or-nothing gate as
> `GuardianRock.PullAttack`. A successful resistance roll suppresses only the
> player movement; the normal 500 ms wind-up and `ObjectRangeAttack` remain
> packet-visible, and the attack remains non-damaging. The existing real-map
> pull-lane regression now covers both unresisted and deterministic resisted
> paths in one locked serial test. Other spawned-AI details remain open.
>
> Crystal AI 27 correction (2026-08-24): Khazard's four-tile pull now applies
> the same `MagicResist / MagicResistWeight` all-or-nothing resistance gate as
> `Khazard.PullAttack`. A resisted pull still broadcasts the range-attack
> animation but does not move or damage the player. The existing real-map lane
> test now proves both an unresisted pull and a deterministic resisted pull in
> one run; the focused locked serial test passes. Remaining spawned-AI details
> are still open and are not covered by this bounded closure.
>
> Crystal NPC visibility closure (2026-08-24): generated `FlagNeeded`,
> `DayofWeek`, and `TimeVisible` metadata now controls authoritative NPC
> visibility instead of permanently hiding every gated NPC. Character flags
> use the existing per-character persisted NPC flag state; schedule evaluation
> uses server-local `.NET DayOfWeek` names and Crystal's inclusive-start,
> exclusive-finish minute boundary. Crystal NPC entities remain materialized so
> later flag changes can reconcile live AOI: the client receives `ObjectNpc`
> when a flag becomes true and `ObjectRemove` when it becomes false. Hidden
> NPCs are also removed from `worldSnapshot` and rejected by the interaction
> range guard. Locked serial compile and focused pure-schedule/live-AOI tests
> pass. This closes this bounded semantic gap only; full NPC script/event and
> whole-project parity remain open.
>
> WN-CANDIDATE R12 ordinary-player closure (2026-08-23): quest accept and
> finish are now explicit dialog actions rather than side effects of opening
> the Village Guide. The native path requires the matching current dialog link,
> correct active NPC, authoritative one-tile distance, valid stage, and starter
> proof item before rewards. Web quest-log Accept/Complete are enabled only by
> an exact matching link from the current server-owned NPC dialog, which the
> server revalidates before mutation; the former
> no-dialog sentinel path is rejected. The native bounded command lane reports
> saturation, preserves a
> ninth reliable command for the next drain, and the UI bridge retains/retries
> old pickup and quest intents instead of silently losing them. Native typed quest
> requests now carry a monotonic request id. Normal execution and capacity
> rejection echo an exact ACK/NACK in the causative world snapshot; all
> same-frame ACKs are consumed once, delayed old ids cannot release replacement
> submissions, malformed ACKs fail native transport validation, and WebSocket
> generation rollover isolates old snapshots and rebinds retained unsent
> retries. Existing Crystal/Web `@quest:*` dialog links remain accepted. The
> ordinary candidate loop proves movement, guide interaction, Field Wasp
> combat, quest-container drop, exact ground-gold and object-id item pickup,
> exact rewards, Bichon map identity, and save/relogin restoration; the loop
> currently passes 2/2 through Cargo. Focused quest, native backpressure, and Gateway object/tile
> pickup mappings also pass. This is functional protocol/simulation evidence,
> not a claim of Windows GUI, Gemini visual, deployed WebSocket, or human
> acceptance; the full 1,285-test Simulation run was intentionally stopped to
> protect a host with recent hardware/driver bugchecks.
>
> Web build/runtime follow-up (2026-08-22): the current integrated source now
> has a fresh green production build and strict dual-backend runtime budget.
> Runtime `bevy-1813be587ef98bc1` measures WebGPU 27,119,641 raw / 5,902,117
> gzip and WebGL2 28,489,677 raw / 6,342,038 gzip; Next TypeScript and 13/13
> static pages pass under BUILD_ID `OXQE2c59Nd1B4bxoWcPQf`. This does not expand
> backend proof: live PostgreSQL, deployed remote Zone and crash recovery remain
> open. A strict local pre-seeded 64-client/30-minute Gateway soak passed on
> 2026-08-22, but it is not deployed-environment evidence.
>
> WN-WEB-PARITY-01 aggregate non-visual closeout (2026-08-21): trusted
> `MonsterDisposition` is explicit in game data and preserved through session,
> shared Zone, Gateway rehydration and checkpoint state. Legacy projected
> `ObjectMonster` data can no longer silently turn a live hostile Zone monster
> neutral. PVP uses the Zone player-target command path; native monster combat
> remains transaction-materialized. Fresh mail/GameShop embedded items are
> recursively re-IDed, while storage split retains Crystal grid identity.
> Current full gates pass Simulation 1,283/1,283, shared Zone 189/189 and
> Gateway 529/0 with one environment-gated ignored test; default Gateway check
> also passes. A focused authenticated Axum `/ws` black-box now proves native
> GameShop buy, exact receipt/mail ordering and durable parcel claim; its
> adjacent exactly-once reload test also passes. This closes the local WS gate,
> not live PostgreSQL, deployed remote Zone, crash recovery or soak evidence.

> Native GameShop Sol-audit follow-up (2026-08-21): the generic RPC and session
> entry points now enforce the same invariant as the dedicated Native handler.
> `NativeGameShopPurchaseV2` requires `nativeGameShopPurchaseV2` and one Execute
> endpoint; ordinary non-opted-in `GameShopBuy` remains compatible with old hosts
> but is also single-attempt after endpoint selection. Raw common-call economic
> Execute is rejected before I/O, while unrelated commands retain fallback.
> Pre-execution receipt state clears only after successful send. Full transaction
> tests prove operation 4,097 changes neither Gold/Credit, global/individual
> stock, visible mail, packets nor durable store and does not evict the oldest
> replay key; hidden ledger mail is not player-readable/collectible/deletable.
> Stable-snapshot focused gates pass Simulation 8/8, typed RPC 12/12, native
> Gateway handler 7/7 and generic-session bypass 1/1. Gateway full was deliberately
> not run while another worker owned `routing.rs`. Local scoped status is P0=0,
> P1=0; fresh independent acceptance and real WS/PostgreSQL/remote-Zone/crash E2E
> remain open, as does the 4,096-entry P2 availability/carrier limitation.
> `typedGameShopOutcomeV1` remains an advertised legacy optional-outcome marker;
> only `nativeGameShopPurchaseV2` authorizes the Native V2 operation.

> Native GameShop at-most-once closeout (2026-08-21): opted-in purchases now
> carry a Gateway-generated 256-bit key through the versioned V2 Zone RPC into
> the authoritative character transaction. Currency, stock, purchase mail and
> the exact typed outcome ledger commit together; an exact duplicate returns
> the original outcome with zero ordinary mutation packets. The hidden durable
> ledger no longer drops old Gateway sessions and has a special fail-closed
> union merge, covering session A -> B -> delayed A and stale A/B save/reload
> races. Same-key conflicts and entry 4,097 fail closed without evicting replay
> history. Typed Execute is sent once to one V2-capable endpoint; response loss
> cannot fall back, old hosts receive zero typed Execute, and post-execution
> ambiguity emits zero receipt plus `CloseUnknown`. Focused evidence passes
> Simulation 6/6, Gateway handler 6/6, V2/no-fallback 4/4 and old-host 2/2;
> full Simulation snapshot passes 1,267/1,267. At that intermediate snapshot,
> Gateway full 513 was not green because shared combat/routing failures preceded
> Windows `STATUS_STACK_BUFFER_OVERRUN`; the top aggregate closeout supersedes
> this with the repaired 529/0/1 result. Remaining P2:
> 4,096 is an availability cap over a pragmatic hidden-mail carrier, and no
> authenticated real WebSocket, live PostgreSQL or deployed remote-Zone E2E is
> claimed.

> Secure reconnect backend Phase 1 P1 rework (2026-08-21): resume now performs
> read-only credential binding, revocation and identity-row validation before
> exclusively reserving—but not consuming—the retained session lease. Route
> refresh and Zone live-outbound registration are fallible preparations; only
> after both succeed does one mutex commit consume the hash-only credential
> family and transfer the lease. The reservation owns an RAII rollback path, so
> injected route/Zone failures preserve the exact token, session and capacity
> permits for a second successful attempt; concurrent commit and replay remain
> single-winner/fail-closed. `ResumeCredential` validates during deserialization
> as exact 43-character unpadded base64url/32 decoded bytes. Production defaults
> bound WS/active/reconnect counts to 2,048/512/512, WebSocket frames/messages to
> 64 KiB, and the 256-entry input queue to an enforced 16 MiB byte budget.
> Gates pass native resume 14/14, registry 6/6, Gateway lib 490/0 with one
> existing live-DB test ignored, and Gateway check. The resume registry remains
> process-local; source nonce is provenance metadata rather than device proof,
> and no deployed cross-instance or live Windows reconnect is claimed.

> Latest mail transaction closure (2026-08-21): all new mail deliveries now
> carry a persisted 128-bit opaque identity, so equal-content sends remain
> distinct and repeated refresh of one delivery remains idempotent. If an
> incoming durable mail collides with an ID already visible to the active
> client, the local ID is retained and only the incoming mail is
> deterministically re-keyed; reversible `locked=false` is not overwritten by
> stale storage. GameShop mail creation and ordinary `CollectParcel` use the
> account-store transaction and expose World changes/success packets only after
> persistence. Exact gold/items, bad JSON, capacity failure, injected failure,
> reload, repeat and concurrent claim are covered. Anonymous active save no
> longer falls back to `demo`. Legacy identity excludes claim-cleared payload
> and mutable status; same-ID/same-header ambiguity merges safely to prevent
> duplicate collection. Simulation lib 1,234/1,234, legacy focused 3/3, mail 28/28,
> social-economy 3/3, security lifecycle 18/18 and check pass. Live PostgreSQL
> execution remains unavailable on this workstation; the documented mirror
> crash window is unchanged.

> Client-facing native social P1 sync (2026-08-21): ordinary Group/Guild/Trade
> commands are wired through bounded Windows read models and typed intents;
> no Stage5/admin fallback is used. The client gates personal skill deltas on
> an explicit nonzero current-player objectId. Automated evidence is recorded
> in `docs/generated/player-qa/native-social/WN-SOCIAL-01-REPORT.md`; this is
> not a live backend protocol sign-off.

> Latest SendMail durability sync (2026-08-21): the cross-character
> recipient-first crash window is closed. A send now creates one staged
> multi-account snapshot containing the sender checkpoint with exact
> unique-ID item/gold debit and the recipient's new mail, persists that unit,
> then commits the shared store and live World before emitting `MailSent(1)`.
> Validation, serialization, injected persistence failure, mailbox capacity,
> bound/rental eligibility and missing/duplicate attachment failures return
> `MailSent(-1)` with no sender/recipient/live mutation. File commits through
> temp-file + atomic replace; PostgreSQL source mode scopes both accounts into
> one existing version-checked DB transaction. Self-mail follows the same
> path. A stale online recipient checkpoint cannot overwrite newly committed
> external mail. Evidence: Simulation lib 1,220/1,220, mail 21/21,
> `social_economy_integration` 3/3, `security_lifecycle` 18/18 and simulation
> check. The live PostgreSQL rollback test was skipped because no DB service was
> reachable; mirror dual-write compensation is not distributed 2PC and retains
> a documented crash-only temporary-divergence boundary.

> Security correction status (2026-08-21): the previously reported simulation
> mail findings are closed by the transaction/identity work above. The other
> parser, peer-authority, qaControl and result-code findings retain their own
> named evidence; final Candidate promotion still requires independent review.

> Latest GameShop/mail/player-boundary hardening: 2026-08-21 introduces the
> dedicated `ClientPacket::GameShopBuy` route and revalidates the authoritative
> product, class, enabled payment mode, unit/total price, balance, mail capacity
> and the Crystal maximum of five attachment stacks before mutation. Gold and
> Credit purchases both deliver exact `ItemState` stacks through Gameshop Mail;
> malformed exact attachment JSON now rejects the whole claim without changing
> gold, inventory, mail payload or claimed state. Invalid quantities are silent
> and success emits the Crystal `PurchasesSentMailbox` hint. Gateway normal
> player-command safety now defaults closed, with only an explicit loopback
> dev/test opt-out. Independently rerun gates pass Simulation 1,206/1,206,
> Gateway 461 passed / 1 ignored and the ordinary persistence loop 1/1. Finite
> stock remains fail-closed rather than persisted, and purchase request/result
> correlation is still an explicit protocol gap.

> Latest World Map transaction hardening: 2026-08-21 makes Crystal `Setup.ini`
> the single runtime source for `TeleportToNPCCost`, including the value sent in
> `WorldMapSetup` and the value enforced by the shared Zone. Invalid or missing
> configuration fails closed. A forced Gateway checkpoint-write failure now
> proves that gold, private/Zone transform, AOI, occupancy, and all outbound
> packets remain atomic; a separate regression proves pre-teleport movement
> intent cannot overwrite a committed destination on a later tick. Simulation
> passed 1,194/1,194, Gateway passed 456 with 1 ignored, and the ordinary quest
> 1-to-9 vertical slice passed. Authoritative `WorldMap.ini` is still disabled
> with zero eligible NPCs, so this does not enable unavailable content.

> Latest shared-Zone World Map sync: 2026-08-21 implements the successful
> `TeleportToNpc` path without giving the client destination or currency
> authority. `WorldMap.ini` is discovered at runtime (or via an explicit
> server-root override), parsed into `WorldMapSetup`, and combined with imported
> map/NPC metadata. The Zone validates enabled policy, same-map retained NPC,
> exact server cost, walkability and occupancy, then refreshes occupancy/AOI and
> emits a save transform; Gateway atomically commits the personal gold
> checkpoint and rolls the Zone transform back if that commit fails. Shared-Zone
> integration tests cover success, insufficient gold, unknown/ineligible NPC,
> occupied destination, cross-map rejection, disabled policy and relogin
> persistence. The actual Crystal source is SHA-256
> `182E958C1314F5C0CA22E51511400383E9F0774377E3E877CB0642DB03865765`,
> says `Enabled=False`, and the imported NPC manifest has zero eligible
> destinations, so production behavior remains a truthful silent rejection.

> Latest Big Map backend sync: 2026-08-21 completes BM-BE-01 without claiming
> teleport success. `RequestMapInfo` now returns Crystal's connection-cached
> `WorldMapSetup` followed by map-cached `NewMapInfo`; logout resets that cache.
> `SearchMap` uses the imported map/NPC catalog with normalized 3-64 character
> queries, map-before-NPC stable ordering and bounded map payloads. Gateway maps
> `requestMapInfo`, `searchMap` and `teleportToNpc` to existing protocol packets
> with strict input validation. The imported authoritative WorldMap is disabled
> and has zero teleportable NPCs, so teleport is a regression-tested silent
> no-op with unchanged map, position and gold. Simulation 1,186/1,186 plus Big
> Map 7/7 and Gateway 453 passed/1 ignored are green. Successful teleport is a
> future shared-Zone single-writer task, not part of this slice.

> Latest Windows-native consumer verification: 2026-08-19 does not change
> backend authority or production packet semantics. A new native Gateway client
> exercised the existing account, character, StartGame, shared-Zone movement,
> combat, quest, TownRevive, save and reconnect paths end to end with a fresh
> account. Q2 completed after 20 authoritative Scarecrow hits; the shared Zone
> emitted Crystal's intended direct-Q `GainedItem(item_index=1112)` rather than
> a normal ground object, then Jane turn-in granted 30 EXP, 200 Gold,
> GoldenPendant and CopperRing. Logout/login restored position, experience,
> gold, inventory and q1/q2 completion. Gateway `/health` remained HTTP/WS/TCP
> ready and the representative map API passed 18/18. This is additional client-
> consumer evidence for the existing backend, not a claim that remaining server
> parity categories or human acceptance have changed.

> Latest three-class skill backend sync: 2026-08-18 closes the strict active
> Warrior/Wizard/Taoist <=50 set at 63/63 automated gates. Shared Zone now owns
> the relevant melee shapes/timing, spell damage and MAC mitigation, movement
> and control effects, buffs/debuffs, poison item shape, healing/revival,
> summons and support targeting; Gateway routes those actions through the Zone
> and propagates item parameters without restoring a personal-session-as-world
> shortcut. Regression cleanup preserves immediate monster-target FireBang and
> IceStorm while allowing their ground-target form, distinguishes mirrored
> personal Buff expiry from Zone-native owner notification, and keeps
> Entrapment pull plus HealingCircle friendly-area semantics authoritative.
> The reproducible audit also requires exact profile gates and reachable world
> book sources. Browser casting with a new ordinary account is still a human
> acceptance prerequisite because that account legitimately knows no skills;
> no production admin permission was weakened to manufacture evidence.

> Latest staged Quest Agent backend sync: 2026-08-15 corrects StartGame legacy
> transform recovery to use the loaded map's authoritative full collision
> bounds. A valid field position outside the starter preload window is retained;
> a town coordinate persisted under an incompatible field map is still
> recovered to the configured bind map. Focused preservation/recovery tests and
> both previously failing FireBall vertical-slice tests pass, followed by the
> full Simulation package and Gateway library 451/451 non-ignored tests. The
> Gateway paid-sailor fixture now seeds isolated Platinum state instead of using
> a profile-rejected QA mutation; production transport behavior is unchanged.

> Latest production-login prevention sync: 2026-08-12 closes the monitoring
> blind spots around the remote Zone OnConnect bootstrap and World Director's
> durable checkpoint. The Zone Host now publishes inflight, request/error, and
> latency telemetry specifically for OnConnect. World Director now records
> durable write attempts, successes, failures, bytes, duration, current file
> size, embedded Zone-factory size, and last-success time across fresh and
> restored runtimes. Gate 12 contains 12 new alarms covering login stalls and
> errors, mean latency, journal backlog/compactor stalls, failed or stale
> checkpoint writes, and two-stage checkpoint-size thresholds. Regression
> coverage includes successful and rejected OnConnect, successful persistence,
> restart restore, and a forced filesystem write failure. Verification passed
> Rust fmt/diff checks, Gateway 438/438 non-ignored unit tests, Gate 11 workload
> 2/2, Home Tunnel 4/4, Zone RPC 29/29, and Prometheus 3.5 `promtool` validation
> of all 17 rules. No production debug command, authentication fallback, or
> personal-Session-as-world shortcut was introduced.

> Latest map-environment backend sync: 2026-08-01 extends the existing Crystal
> map import with `MapDarkLight` and `WeatherParticles` without changing Zone
> authority. Simulation emits those values through its existing typed
> `MapInformation` path, and Gateway now keeps light/weather fields in explicit
> browser-facing `MapInformation` and `MapChanged` events. The generator was
> corrected to the current v117 `MapInfo` boolean layout and regenerated 463
> maps; `DogYoHyun` carries the source combination `weather_particles=3`.
> Focused GameData, Simulation and Gateway regressions pass.

> Latest v5 replica-image stability sync: 2026-07-29 keeps an installed base
> snapshot byte-identical when a restored Session is rebound on the standby.
> Replica rebinding now restores only the Session-local movement ingress,
> cached map transfers, and `lastSeenMoveSeq`; it no longer runs a full
> `sync_zone_snapshot()` that can merge reconstructed static entity fields back
> into the already validated authoritative Zone image. The regression covers
> the previous Royal_Archer light drift (`0` to `5`) and proves the re-exported
> gzip payload is identical. Gateway verification passes 390/390 library tests,
> 17/17 packet-trace tests, Gate 11 2/2, Home Tunnel 4/4, and Zone RPC 28/28.

> Latest reproducible-deployment health sync: 2026-07-25 adds an optional
> top-level `revision` to Gateway `/health`, sourced once from
> `MIR2_DEPLOY_REVISION` at process startup. Local responses omit the field when
> unset, preserving the prior health contract. The shared acceptance deploy now
> compares this runtime value and Player Web `/version` against the checked-out
> Git HEAD after force-recreating production containers. Focused Gateway
> revision tests pass 2/2, Gateway check/fmt pass, and Player Web revision tests
> pass 3/3 with typecheck green.

> Latest StartGame collision-cache sync: 2026-07-25 keeps full/world Crystal
> collision data behind one immutable `Arc` per normalized map instead of
> deep-cloning the complete walkable/blocked cell sets for every respawn and
> placement query. Callers that only inspect collision now share the cached
> object; the three boundaries that install owned Zone/ECS collision state
> still take an explicit clone, so movement, transfer, occupancy, and save
> semantics do not change. `cargo +1.89.0 fmt --check` plus the focused
> full-world Zone collision, StartGame visible-object, spread-density, and
> representative-spawn regressions pass 4/4. A live isolated Gateway/Web flow
> also completed New Account, Login, New Character, and Start Game into
> BichonProvince with the full pack and no browser warnings/errors.

> Latest remote-Zone workload sync: 2026-07-23 completes Gate 11.1-11.4.
> Checkpoint v4 restores the durable session projection plus complete shared
> Zone state: player vitals, monsters/AI timers, pending combat/effects, drops
> and claims, doors, hazards, map layers, trades/rentals, and NPC state. One
> real Crystal combat/drop/map-handoff workload survives active loss, while a
> separate four-session/two-map harness survives two consecutive host failures;
> both old generations are fenced at tokens 2 and 3. The full operations binary
> writes one fail-closed, versioned JSON evidence manifest atomically.

> Latest Crystal system-chat parity sync: 2026-07-23 adds one shared Gateway
> scheduler for native TCP and WebSocket sessions. Presence begins only after a
> successful StartGame/Zone join and is removed by LogOut, Disconnect, socket
> close, or RAII drop. Production emits `ChatType::Hint` online-count packets
> every five minutes and `ChatType::LineMessage` announcements every ten
> minutes, loading the original `LineMessage.txt` when available with a bounded
> built-in fallback. Accelerated intervals, a fixed line, and packet limits are
> ignored unless the Gateway process has a non-empty QA control token, so normal
> clients cannot activate capture behavior. Focused scheduler, shared TCP/WS,
> lifecycle, and fail-closed configuration tests pass 5/5, and the complete
> Gateway library regression passes 307/307; Web packet rendering consumes the
> resulting real chat types. This closes the final deterministic
> chat-state dependency for visual Candidate without changing Zone movement,
> combat, save, or account authority.

> Latest source-verified monster-defence sync: 2026-07-22 corrects the player
> mitigation channel for Crystal AI 26 ShamanZombie and AI 181 WaterDragon /
> Hydra. ShamanZombie `LineAttack` always uses `MACAgility`; WaterDragon uses
> physical AC for its adjacent branch and MAC for its ranged MC branch. The
> runtime now rolls authoritative Min/Max MAC with an independent deterministic
> salt and applies MagicResist on those magic channels instead of subtracting
> physical AC. A high-AC Hydra regression proves the ranged hit still damages
> and applies Green poison through MAC. Verification passes all 1,126 Simulation
> unit tests, 154 shared-Zone tests, all integration suites including 8/8
> vertical slices, and the focused strengthened Hydra test. A complete imported
> per-AI `DefenceType` table remains open; unverified AIs still retain the older
> distance fallback and must be migrated from Crystal source rather than guessed.

> Latest original Bichon q1-q9 backend sync: 2026-07-18 completes an exact
> fresh-Warrior route from Assistant Jane through MirGuide Peter and naturally
> reaches level 6. The runtime now matches Crystal's task counts, prerequisites,
> q1-q9 XP/gold, fixed items, q3/q6 mandatory class reward selection, Q-drop
> semantics, monster EXP, and real equipment template stats. Quest progression
> is attached to the player-owned death boundary, including direct skills and
> player poison, so ambient/NPC damage cannot grant kill credit or EXP. The
> release integration test proves all nine quests complete, level 6 with the
> next 900 EXP band active, exact reward retention, and exact gold after its
> declared potion purchases. Focused ownership, reward-parser, equipment,
> combat, harvest, Gateway reward-contract, and Web packet/UI suites pass;
> `cargo check --workspace` and the Next production build also pass. This
> supersedes the older q1-q4/q5-available backend checkpoint below.

> Latest NPC name-colour packet consistency sync: 2026-07-18 removes a
> duplicate production-path conflict for Crystal NPCs. Initial manifest
> `ObjectNpc` and `WorldEntitySnapshot` already emitted Crystal Lime
> (`0xFF00FF00`), while `visible_object_bundle_for_entity` later emitted White
> and could overwrite a native client's final label. All three paths now share
> one `CRYSTAL_NPC_NAME_COLOUR_ARGB` constant. The transfer regression asserts
> every Assistant_Jane ObjectNpc plus the snapshot agree on Lime. Verification:
> focused transfer 3/3, visible-object bootstrap 1/1, shared Zone 153/153,
> Release Gateway build, Rust fmt, and live r04 Web login/capture against the
> rebuilt 7111 Gateway. Crystal's client keeps later underscore-delimited name
> lines White; this change concerns only the packet-provided primary line.

> Latest retained-object AOI sync: 2026-07-13 fixes the spatial index for
> authoritative monsters/summons that move after being inserted. Packet state
> was updating `ZoneObject.position`, but `object_grid` remained at the old
> cell, which could produce incorrect ObjectRemove/ObjectMonster visibility
> transitions. Every retained-object packet now relocates the grid entry before
> the next visibility diff. The dedicated leave-and-reenter regression passes,
> and the full shared Zone suite is 153/153.

> Latest actor-light transport sync: 2026-07-12 preserves Crystal entity light
> values through the shared snapshot and AOI spawn path. `WorldEntitySnapshot`
> now carries `light`; player/monster packet projection retains the packet
> value, seeded NPC projection uses the Crystal-compatible light value, and
> shared/Zone monster spawn packets no longer overwrite it with zero. This is a
> data-contract fix only; browser light blending remains frontend work.
> `cargo +1.89.0 check -p mir2-simulation -p mir2-gateway`, shared Zone 152/152,
> and focused Gateway snapshot/movement/routing tests pass. Live r12/r16 scene
> evidence observes Dawn mode plus 15/14 object-light nodes without movement or
> pose regressions.
> The complete Debug Gateway process remains a host-stability exception, not a
> passed gate: two single-thread runs aborted at different tests with
> `0xc0000409` and `0xc0000374`, while each reported test passes alone. Seven
> recent WHEA records include corrected TLB/internal-parity machine checks.
> Focused Gateway and live Release evidence are green, but full Debug 300/300
> must be rerun after BIOS/CPU/RAM stabilization.

> Latest mounted/Swift Feet backend sync: 2026-07-12 closes the movement-state
> split between personal Session and shared Zone. Non-combat owner-state packets
> that affect shared behavior, including `MountUpdate`, `ObjectSneaking`, and
> Add/Remove/Pause Buff state, are now sent through `ZoneCommand::BroadcastPackets`
> before subsequent movement. Zone stores PauseBuff changes, validates every
> intermediate tile, and resolves Run distance as 3 for a mounted player or an
> active unpaused Swift Feet player who is not sneaking, otherwise 2. Server
> movement delay remains Crystal's 600ms; the mounted client's eight visual
> phases remain an 800ms presentation gate rather than a false server cooldown.
>
> Focused Gateway real-item equip/use/run coverage passes, shared Zone is
> 152/152, and live Release evidence
> `docs/generated/player-qa/movement-jitter/movement-mounted-walk8-run3-webgpu-20260712-r6.json`
> records one-cell Walk plus one three-cell Run with 18/22ms ACKs, no correction
> or degradation, and exact `(4,0)` delta. This closes mounted movement semantics;
> full non-movement command actorization and durable side-effect ownership remain.

> Latest Zone-owned cadence/live-outbound backend sync: 2026-07-12 extends the
> bounded movement owner into the single shared-world clock. Each Zone owner now
> advances one monotonic 300ms global cadence and coalesces late work rather than
> replaying catch-up bursts. Personal `WorldCommand::Tick` no longer invokes
> global Zone Tick, per-player movement Tick, or shared-drop expiry, eliminating
> player-count-dependent world speed. Realtime owner/AOI `UserLocation`, player
> appearance/removal, Turn, Walk, and Run packets use a capacity-256 token-fenced
> socket path independent of private Session draining; full/closed channels
> retain mailbox fallback. The existing capacity-64 ingress, serial RW gate,
> owner fencing, ordered events, save-transform sync, and 5s reply bound remain.
>
> Latest strict Release evidence is
> `docs/generated/player-qa/two-client-zone/two-client-zone-zone-owned-cadence-tick5000-release-20260712.json`.
> Personal Tick is intentionally 5000ms and observer pulses are off, yet movement
> reaches B in 12ms, both clients retain 16 entities, Bevy records one remote
> motion event and 29 offset matches, and all drop/error/404 counters are zero.
> Focused unique-cadence, queued movement without Session Tick, blocked runtime,
> fencing/fallback, and delayed combat tests pass; Simulation `shared_zone` is
> 148/148, complete Web frontend logic and TypeScript pass, and Release builds.
> Remaining backend architecture work is full non-movement command actorization
> and fenced durable side-effect/save ownership. Windows Debug crashes remain a
> separate host WHEA/BIOS stability gate, not evidence of a new safe-Rust actor
> memory fault.

> Latest dynamic TimeOfDay snapshot parity sync: 2026-07-09 supersedes the
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
> non-favicon 404s. The light lane is now dynamic; the next frontend task is
> rendering Crystal night/evening/dawn ambience in the main scene.

> Latest movement ACK semantics sync: 2026-07-12 unifies the Web early-ACK path
> and movement-controller reconciliation on the same
> `classifyMovementAckOutcome` classifier. A requested Run's one-tile first-cell
> `UserLocation` ACK is therefore `confirmed`, not `correction`, matching Zone
> authority where an originally stationary Run degrades to a one-tile Walk.
> Simulation `shared_zone` passed 148/148. Release raw `packetSequence` evidence
> `docs/generated/player-qa/movement-jitter/movement-protocol-expired-run-degrades-release-202607120745.json`
> records Walk followed by an expired Run with ACKs at `16ms/99ms`,
> `degradedRunCount=1`, `correctionCount=0`, and final delta `(2,0)`. Release
> normal UI Walk -> Run evidence
> `docs/generated/player-qa/movement-jitter/movement-normal-walk-run-chain-release-202607120750.json`
> records ACKs at `22ms/28ms`, command-to-pose latency `17ms/1ms`,
> `degradedRunCount=0`, `correctionCount=0`, and final delta `(3,0)`. Remaining
> architecture risk is unchanged: private `SimulationSession` heavy `Tick` work
> and movement ingress still serialize on the same WebSocket task. Release mode
> reduces that risk but does not eliminate it. The next architecture step is a
> Gateway-owned single-writer Zone ingress/loop; it is not implemented yet.

> Previous QA evidence/TimeOfDay parity sync: 2026-07-09 closes a shared-Zone
> evidence bug and a visible MiniMap bootstrap mismatch. `qa.applyNativeState`
> now forces the shared Zone transform sync path after applying native
> character state, so Web `world_snapshot()` no longer reverts to stale Zone
> presence coordinates; the capture harness also verifies `mapFileName` and
> `position.x/y` before accepting native-state alignment. Focused Gateway
> regression
> `shared_in_process_registry_qa_apply_native_state_syncs_zone_transform`
> passed, and 0056 live evidence records Web `player` and `authoritativePlayer`
> both at `334,263`. That earlier Simulation StartGame pass emitted the
> then-current Crystal-like Day/Normal `TimeOfDay` (`lights=2`) instead of
> fixed Night (`lights=4`), matching the native Bichon MiniMap `Prguse/2093`
> light icon; the dynamic TimeOfDay sync above now supersedes fixed Day.
> Focused simulation `start_game_emits_bootstrap_sequence` passed. Live 0057
> evidence
> `docs/generated/player-qa/visual-parity/crystal-web-pack-20260709-0057-minimap-light-day-bootstrap/`
> is runtime-clean with 0 network 404s and 0 critical console errors.

> Latest HUD weight snapshot parity sync: 2026-07-09 replaces the Web-facing
> fixed bag-weight cap with Crystal player stat data. `WorldSnapshot.maxWeight`
> now comes from `player_stats(world).bag_weight()` instead of the old constant
> `100`, letting the frontend reproduce Crystal's main-HUD remaining-weight
> readout. The focused QA-control regression now asserts a level-6 Warrior
> native-state apply yields `current_weight=1` and `max_weight=62`, and live
> evidence
> `docs/generated/player-qa/visual-parity/crystal-web-pack-20260709-0027-hud-weight-diagnostics/`
> shows the full native character state with `currentWeight=14`,
> `maxWeight=62`, HUD `48 / 38`, and gold `3457` through the real Web gateway.
> Remaining work in this lane is frontend-facing asset/chat/minimap/world visual
> parity; full Crystal inventory-capacity modeling is still a separate broader
> slice.

> Latest native-state QA-control/max-MP/EXP sync: 2026-07-09 adds a bounded backend bridge
> for fair Crystal/Web same-scene evidence. `qa.applyNativeState` runs only
> behind the existing local token-gated `qaControl` wrapper and reuses typed
> item/equipment state decoding before updating the active character save, ECS
> `SelfPlayer` transform/facing, and runtime vitals. Gateway snapshot forcing
> now treats bootstrap/state packets (`StartGame`, `MapInformation`,
> `UserInformation`, `UserLocation`, `ObjectHealth`) as snapshot-worthy so the
> browser sees the freshly applied native state instead of stale pre-apply
> state. `WorldSnapshot` now includes `player_max_mp`, matching the existing
> Web `playerMaxMp` field and keeping max MP visible after later snapshots.
> The Web account sync now derives max EXP from Crystal `ExpList.ini`, and the
> QA-control regression asserts EXP `435/900` survives apply/start/snapshot.
> Verification passed focused Gateway QA-control and snapshot tests plus
> the live pack
> `docs/generated/player-qa/visual-parity/crystal-web-pack-20260709-0025-exp-debug/`,
> where Web aligned to native level `6`, `HP 51/51`, `MP 32/32`, EXP `435/900`,
> gold `3457`, 6 inventory items, 2 belt items, and 8 equipment items with 0
> critical console errors. Remaining work in this lane is frontend-facing:
> bottom-right HUD status semantics, chat state, minimap crop/color, and
> world-frame mismatch.

> Latest QA-control backend safety sync: 2026-07-08 adds a local-only,
> token-gated `qaControl` WebSocket wrapper for automation while preserving the
> production player-path rejection of debug `MoveTo`, raw `Stage5Command`, and
> debug crystal transfer. Focused tests passed with production command safety
> enabled (`cargo test -p mir2-gateway qa_control -- --nocapture`). Live Rust
> `7111` evidence
> `docs/generated/player-qa/combat-survival-default-selfcamera-rust7111-qacontrol2-20260708/report.md`
> passed hostile incoming damage and death/revive through the browser. Remaining
> backend/control work: add an explicit QA-control acknowledgement or stricter
> settle probe because transfer/spawn side effects can arrive late, then use the
> stable control lane to rerun normal kill/XP/drop.

> Latest Rust `7111` hostile-retaliation evidence: 2026-07-08 moves incoming
> monster damage from open blocker to real-client verified evidence. The
> attack-trace harness now captures target map/object id, sent attack frames,
> melee approach, delayed combat packets, and `StartGame` retry attempts.
> Evidence
> `docs/generated/player-qa/combat-survival-default-selfcamera-rust7111-survivalattacktrace5-20260708/report.md`
> reached melee with natural `ForestYeti` object `258949`, sent 24 attack
> frames, observed 7 target `ObjectAttack` frames plus `ObjectStruck` /
> `DamageIndicator`, and dropped player HP `18 -> 3`. The remaining backend
> parity gap has shifted: QA/admin controls are not reliable enough for
> isolated reruns (`transferMap` returns sent but does not move the player,
> `event.spawn RakingCat0` produces `0x`, and `@DIE`/`townRevive` can fail
> beside a live hostile), and normal kill/XP/drop evidence still needs a green
> route after that control lane is repaired or replaced.

> Latest Rust `7111` combat-survival follow-up: 2026-07-08 keeps pickup and
> death/revive green while narrowing the remaining hostile-retaliation gap.
> Gateway regressions now cover explicit hostile passive-template AI override
> (`zone_monster_spawn_from_shared_entity_preserves_explicit_hostile_passive_override`)
> and Stage5 `event.spawn` synchronization into the shared Zone
> (`shared_in_process_registry_syncs_stage5_event_spawn_to_zone`). The live
> targeted report
> `docs/generated/player-qa/combat-survival-default-selfcamera-rust7111-pickupwait5s-20260708/report.md`
> passed QA-seeded pickup (`GainedItem x1`) and `@DIE`/`townRevive`. The
> follow-up `survivaltick` run proves the harness no longer mistakes passive
> Deer for a retaliation target, but incoming monster damage is still not
> accepted because the live trace did not produce a clean adjacent
> attack/object-attack/player-damage sequence before timeout. Next backend task:
> stabilize hostile encounter routing/AI tick evidence, then rerun a normal
> kill/XP/drop plus incoming-damage window.

> Latest shared Zone pickup/death lifecycle sync: 2026-07-08 closes the
> Web-observed split between personal session item state and shared Zone
> movement authority for the current pickup lane. Gateway shared runtime now
> merges current personal-session ground drops into Zone `SyncGroundDrops`
> before `ClaimGroundDrop`, and position-sensitive personal commands
> (`PickUp`, `DropItem`, `DropGold`) force the inner session transform to the
> current Zone authoritative position before falling back to session execution.
> A new Gateway regression keeps shared-Zone `@DIE` GM chat commands routed to
> the personal session while normal chat remains Zone-broadcasted, and the
> existing pickup regression still verifies packet-spawned drops use Zone
> authoritative position. Verification passed Web script syntax, Web
> TypeScript, `shared_in_process_runtime_pickup_uses_zone_authoritative_position_for_packet_drop`,
> `shared_in_process_registry_routes_gm_chat_commands_to_personal_session`,
> and live Rust `7111` evidence
> `docs/generated/player-qa/combat-survival-default-selfcamera-rust7111-authpickupseed7-20260708/report.md`
> with pickup and death/revive green. Remaining backend parity work: monster
> retaliation did not reduce player HP in the same run, and unseeded kill/XP
> evidence still needs a green route.

> Latest Rust-gateway combat tick parity sync: 2026-07-07 verifies that real
> Web attack commands can now reach the shared Zone damage path when followed
> by gameplay ticks. Gateway regression
> `shared_in_process_runtime_level_one_field_melee_resolves_damage_on_tick`
> covers a level-1 field melee attack and asserts `ObjectAttack` on attack plus
> `ObjectStruck` and `DamageIndicator` on the next tick. End-to-end Web
> evidence
> `docs/generated/player-qa/combat-survival-default-selfcamera-rust7111-floaterfix30s-20260707/report.md`
> connected to Rust `7111`, landed melee damage, dropped target HP, and passed
> the client damage-floater gate. Remaining backend parity work: kill cadence
> is still too weak/slow for the 30s acceptance window (`ObjectDied`, XP, and
> loot are unproven), and normal-client death/revive lifecycle is still red
> (`@DIE` does not enter a dead state).

> Latest Crystal world transfer parity sync: 2026-07-07 removes the last
> starter-demo transfer from full Crystal world runtime. Local held Shift+Right
> evidence showed a server-side rollback at Bichon `0:339,270`: the fifth run
> ACK was delayed `7481ms`, included transfer/reset traffic, and the player
> snapped back toward `0:330,270`. Root cause was
> `SimulationConfig::with_crystal_world_runtime()` preserving the hand-authored
> `starter-east-field-gate` transfer while the Gateway uses full Crystal world
> runtime. The config now clears starter `map_transfers` for full Crystal world
> mode, matching `with_crystal_map_runtime()` and leaving generated Crystal
> movement records as the only world-travel source. Regression coverage:
> `crystal_map_runtime_drops_starter_demo_transfer` now asserts both map and
> world runtime cleanup, and Gateway
> `shared_in_process_crystal_world_runtime_does_not_apply_starter_demo_gate_transfer`
> asserts walking right from `338,270` stays normal movement with no
> `MapInformation`. Post-fix Web evidence
> `docs/generated/player-qa/movement-jitter/web-motion-heldrun-bichon-right-worldtransferfix-20260707.json`
> is `ok=true` with 8/8 ACKs under 359ms, no logical rollback, no ACK warnings,
> and final `345,270`; the cardinal keyboard rerun
> `docs/generated/player-qa/movement-jitter/web-motion-keyseq-bichon-cardinal-worldtransferfix-rerun-20260707.json`
> also passed strict movement checks.

> Latest crowded-AOI movement ACK sync: 2026-07-07 follows up the 2026-07-06
> Gateway input-priority fix with the Bichon crowded click-route repro. Shared
> Zone now consumes movement that arrives after `movement_ready_at_ms` in the
> command response instead of waiting for a later world tick, while preserving
> same-instant input replacement semantics. Gateway post-movement input grace is
> widened from Crystal run grace alone to run grace plus one Crystal tick
> (1.5s), preventing heavy world ticks from winning the race just before the
> browser's next ACK-driven Walk/Run. Verification passed focused shared-Zone
> movement tests, focused Gateway runtime/post-movement tests, Gateway build,
> and live Web Bichon evidence
> `docs/generated/player-qa/movement-jitter/web-motion-clickroute-bichon-leftclean-postgrace1500-20260707.json`
> with `ok=true`, clean settle, 4/4 ACKs at `490/164/33/5ms`, no interaction
> pollution, and Bevy WebGL2 packed/no DOM fallback.

> Latest Gateway movement ACK/input-priority parity sync: 2026-07-06 closes the
> local Web `Walk -> Walk -> Run -> Walk/Left` stutter repro where a heavy
> shared in-process Zone world tick on the same WebSocket task could start just
> after a player `UserLocation` ACK and delay the next movement input by about
> 2.5s. Shared Zone runtime now tracks pending player movement plus a 1.2s
> post-ACK movement-input window; world ticks drain `TickPlayerMovement` first
> and yield during that window so chained Crystal inputs are read before heavy
> personal/world ticks. Gateway movement packets still wake the runtime tick at
> 75ms, and personal simulation ticks avoid full `advance_world` while Crystal
> movement retry is pending. Verification passed Rust fmt/check, focused
> Gateway and simulation regressions, Gateway build, packetRun probing with the
> first four Run ACKs at <=312ms, and full Web click Bevy evidence
> `docs/generated/player-qa/startgame-debug-20260706-213036/current-web-jitter-r2-gateway-postackgrace1200-click.json`
> with `ok=true`, Run ACK about 205ms, no logical rollback, clean settle, and
> Bevy WebGL2 packed rendering with no DOM fallback. Remaining risk: PR #123's
> uncovered-map Bevy work is intentionally deferred to a clean branch; longer
> crowded-AOI human-feel movement sampling remains useful.

> Latest player/monster state parity sync: 2026-05-27 removes the remaining
> player-damage clamp that kept lethal hits at 1 HP. Pending monster/player
> combat damage can now drive `PlayerVitals.hp` to 0, synchronizes
> `PlayerRuntimeResource.player_vitals`, emits `ObjectHealth` percent 0 plus
> `ObjectDied`, and world snapshots mark the self entity dead with `hp=0`.
> Dead players are rejected from Walk/Run/Attack/RangeAttack/Magic/Harvest and
> ordinary item use, while Resurrection Scroll revival restores authoritative
> vitals and movement ability. Skill MP spend and generic skill healing now
> synchronize entity vitals and runtime vitals. Crystal status buffs now have
> gameplay effects beyond icons: paralysis/frozen/dazed/stun block movement,
> attack, and magic; blindness blocks attack/magic targeting; slow extends
> movement/attack cadence; green poison and bleeding tick player HP; red poison
> increases incoming player damage. Monster death regressions now assert
> `ObjectHealth` 0, `ObjectDied`, released occupancy, no repeat death, respawn
> reset, and summoned-totem despawn coverage. Focused verification passed the
> new player death/revive/status/runtime-vitals tests, potion and resurrection
> regressions, monster death/respawn tests, frontend TypeScript, and gateway
> check. The current full `cargo test -p mir2-simulation` run still fails on
> unrelated broader skill-preflight/effect and account-store persistence tests;
> one representative failing skill test (`FireWall`) also fails in isolation on
> this branch, so that remains a separate Skill parity cleanup queue item.

> Latest NPC input and skill preflight parity sync: 2026-05-27 adds the first
> gameplay-preflight slice for Crystal NPC labels and active skill casting.
> `ClientPacket::NpcConfirmInput` now executes active input labels through the
> NPC dialog runtime while rejecting stale/wrong NPC ids, NPC script
> diagnostics are exposed in world snapshots for debug/admin inspection, and
> the generated NPC command report now calls out implemented/simplified/missing
> buckets explicitly. Skill snapshots now expose Crystal-style cast metadata
> (`passive`, `toggle`, `self`, `target`, `ground`, `direction`, plus
> offensive/spell names), passive skills are rejected as active casts, and
> target/range/LOS/safe-zone/item/map preflight runs before MP, cooldown, or
> spell action timing is committed. Verification covered input-label NPC tests,
> reserved BUY/SELL/REPAIR/STORAGE service routing, walk-away service
> rejection, dynamic visibility TODO coverage, skill preflight regressions,
> and cast metadata snapshots. Remaining backend risk: many full skill effects
> still need Zone-native authority and durable item/economy/NPC services.

> Latest shared-Zone movement input-buffer sync: 2026-05-26 closes the backend
> half of the production `walk -> run -> reverse` drift repro. `ZoneCommand`
> Walk/Run/Turn now carries the Gateway receive timestamp; `ZoneRuntime` keeps
> only the current pending movement plus the newest follow-up, consumes any
> ready pending movement before accepting replacement input, buffers near-ready
> follow-up movement for the legal Crystal cadence, and evaluates Run grace
> against packet arrival time so an unrelated late Zone tick cannot degrade a
> timely Run into a rollback-producing Walk. Gateway runtime ticks now yield a
> 900ms grace window around active Walk/Run/Turn packets so background personal
> session ticks do not block movement ACKs. Verification passed Rust fmt,
> shared-Zone run coverage (7/7), Gateway runtime-tick coverage, Gateway
> Zone-movement regression, UCloud release
> `20260526T1918CST-move-input-buffer`, public health, WSS smoke
> `docs/generated/load/remote-move-input-buffer-wss-smoke-20260526.json`, and
> production WebGL2 captures for normal and 180ms `walk/run/reverse`.
> Remaining backend movement risk: broaden production human-feel sampling to
> longer held/chorded key sessions and crowded AOI, while the larger Shared MMO
> service/ZoneOwner authority gaps remain open.

> Latest ZoneOwner runtime handoff/takeover sync: 2026-05-26 closes the next
> thin slice after the RPC transport seam. The hosted owner now stores its
> runtime as a one-shot handoff slot, exposes `take_runtime_for_handoff`, and
> lets a replacement hosted owner resume the same `ZoneRuntimeHandle` under a
> fresh lease. Evidence passed the focused handoff regression and the full
> `zone_owner` test group: active identity and map snapshot survive the move,
> old owner access is rejected after export, and stale pre-handoff leases are
> rejected by the new owner host. Remaining backend risk: state transfer is
> still in-process and memory-resident; durable serialization plus real
> cross-process owner takeover remain open.

> Latest ZoneOwner RPC transport seam sync: 2026-05-26 adds the first
> transport-facing owner command path after the hosted-runtime boundary.
> `RpcZoneOwnerCommandClient` now consumes a `ZoneOwnerRpcTransport` for
> command execution, snapshots, active identity, save, and external-mail
> refresh. `HostedZoneOwnerCommandClient` implements that transport as the
> current loopback owner host, so Gateway can be tested against an owner-owned
> runtime without direct local mutation while keeping owner-side fencing
> validation. Evidence passed focused regressions for RPC transport mutation
> isolation and stale-lease rejection at the transport owner boundary.
> Remaining backend risk: the transport is still loopback/in-process; durable
> Zone state transfer and real process/network RPC remain open.

> Latest SkillItemConsume request-id sync: 2026-05-26 removes the known
> non-idempotent cast-command hole from the shared Account/Inventory boundary.
> Gateway now assigns a monotonic per-session request id when an accepted
> Zone-native spell needs item consumption, and `SkillItemConsume` receipt keys
> include account, character, spell, and request id. A retried service delivery
> of the same cast can therefore be recognized as the same command, while a
> later recast is not collapsed with the earlier one. Evidence passed the new
> key regression, the focused in-process Account/Inventory service group, and
> the PoisonCloud plus SummonSkeleton Zone route regressions. Remaining backend
> risk: the receipt store is still in-process; production parity still needs a
> durable Account/Inventory actor plus ZoneOwner RPC/fencing rollback handling.

> Latest ZoneOwner hosted-runtime boundary sync: 2026-05-26 advances
> distributed Zone ownership beyond the replaceable command-client shell.
> `HostedZoneOwnerCommandClient` now owns a `ZoneRuntimeHandle` behind the
> owner boundary, executes `ZoneOwnerCommandRequest`s against that hosted
> runtime, and preserves owner-side fencing validation through
> `ZoneOwnerLeaseAuthority`. Focused coverage proves a Gateway session can send
> commands to the hosted owner runtime without mutating its own local runtime,
> read `world_snapshot` and `active_identity` back through that owner client,
> and reject a pre-handoff lease at the hosted owner boundary after the
> authority advances to `zone-owner:next`. Save and external-mail refresh calls
> are now part of the same command-client view interface. Remaining backend
> risk: this is still in-process hosting; the newer RPC transport seam above
> gives it a replaceable transport surface, but durable state transfer and
> takeover orchestration still need real process/network RPC.

> Latest Account/Inventory idempotency sync: 2026-05-26 hardens the shared
> reward/economy service boundary before it moves out of process. The default
> in-process Account/Inventory service now records committed receipt keys for
> Zone `MonsterKillAward` and `GroundDropPickup` commands, so a duplicate
> delivery/retry returns the cached receipt and does not apply experience or
> gold twice. The focused regression proves repeated kill-award and gold-pickup
> commits leave player state at the first committed value. The newer
> SkillItemConsume request-id sync above extends the same command identity
> shape to item-consuming casts. Evidence passed the new idempotency test, the
> focused in-process service group, and the shared Account/Inventory boundary
> regression. Remaining backend risk: durable external actor persistence and
> ZoneOwner fencing/RPC.

> Latest NPC world-service atomic outcome sync: 2026-05-26 closes a
> half-commit risk in the shared NPC/quest bridge. Shared NPC script results
> now leave the personal runtime as one `ApplyScriptOutcome` command containing
> saved values, random seed, and any entity mutation packets. The
> `SharedNpcWorldService` must commit that combined outcome before Gateway
> merges shared saved/random state or forwards entity side effects, and the new
> regression proves a rejected service leaves shared Zone state empty. Evidence
> passed the atomic outcome regression, the existing NPC world-service boundary
> regression, and shared NPC saved/random sync regressions. Remaining backend
> risk: this is still an in-process/default service; full production parity
> still needs a durable NPC/world actor, broader quest/economy commits, and
> ZoneOwner RPC/handoff.

> Latest Zone-native CharmedSnake progress sync: 2026-05-26 completes the
> Crystal post-hit status side effect for the Archer `SummonSnakes` minion.
> Shared Zone now applies `CharmedSnake` paralysis after delayed melee damage
> successfully lands on a native monster, preserving the Crystal chance/duration
> shape from `PoisonTarget(target, 10 - PetLevel, 4 + PetLevel,
> PoisonType.Paralysis, 1000)` through deterministic Zone rolls and the
> existing native monster control timer. Evidence passed the focused
> CharmedSnake paralysis regression, SnakeTotem/Archer adjacent groups,
> focused `zone_native_player_` (30/30), and the Gateway self-Buff mirror
> regression. Remaining backend risk moves to broader monster AI families,
> full skill/Buff services, process-external NPC/economy/account services, and
> ZoneOwner handoff.

> Latest Zone self-Buff progress sync: 2026-05-26 adds the first durable
> skill-state bridge after the Zone-native MagicShield / arrow-Buff work.
> Zone-owned self Buff packet results are now applied back into the owning
> personal runtime's `BuffResource` on both immediate and pending Gateway
> delivery paths. This keeps `world_snapshot.active_buffs`, local cooldown/HP
> state, and client packets aligned after Zone accepts a shared spell instead
> of leaving the personal session with an empty Buff snapshot. Evidence passed
> the new Gateway self-Buff mirror regression, the existing shared-Zone Magic
> route regression, focused `zone_native_player_` (30/30), and fmt check.
> Remaining backend risk: this is still a bridge; full skill/Buff lifetime
> authority, process-external skill state, NPC/economy/account services, broad
> monster AI coverage, and ZoneOwner handoff remain open.

> Latest Zone-native SnakeTotem progress sync: 2026-05-26 finishes the current
> Archer summon-family backend parity slice. Shared Zone now enforces
> `SnakeTotem`'s Crystal `PetLevel + 1` active `CharmedSnake` cap, refreshes
> minions after lifetime expiry, self-destructs the Totem when the Archer
> master is missing or more than 15 tiles away, and kills owned minions when
> the Totem dies. `CharmedSnake` now dies on lifetime expiry or missing/far
> Totem and runs its 3x3 death explosion through Zone-native monster damage
> while preserving owner attribution and avoiding player damage. Evidence
> passed the two SnakeTotem regressions, `zone_native_archer_` (4/4),
> `zone_native_vampire_spider_` (2/2), focused `zone_native_player_` (30/30),
> the Gateway summon item-boundary regression, and fmt check. Remaining backend
> risk is no longer this summon family; it is durable skill-state,
> process-external NPC/economy/account services, broader monster AI coverage,
> and ZoneOwner/handoff.

> Latest Zone-native VampireSpider progress sync: 2026-05-26 finishes the
> Crystal `SummonVampire` pet side effects that were still missing from the
> Archer summon slice. Shared Zone now applies `MasterVampire` from
> `VampireSpider` hits: successful native monster damage emits target
> Bleeding `ObjectEffect` effect 18 and heals the owning Archer via
> authoritative `ObjectHealth` plus `PlayerHealed`. Zone expiry also now
> self-destructs `VampireSpider` instead of silently removing it, including the
> Crystal master-distance check and 3x3 explosion damage against nearby
> hostile Zone monsters through the same owner/drop-safe hit resolver. Evidence
> passed the two focused VampireSpider regressions, `zone_native_archer_`
> (4/4), focused `zone_native_player_` (30/30), the Gateway summon
> item-boundary regression, and fmt check. Remaining backend risk: SnakeTotem
> swarm cap/expiry hardening, durable skill-state, and process-external
> services.

> Latest Zone-native Archer summon sync: 2026-05-26 moves the first Archer
> summon family behaviors onto the shared-Zone spell surface. Zone native
> summon profiles now cover `SummonVampire`, `SummonToad`, `SummonSnakes`, and
> `Stonetrap` with target-point/projectile-delay validation, retained friendly
> monster objects, `extra` visibility, master object binding, summon-cap checks,
> expiry, and no Account/Inventory item consumption for these Archer spells.
> The verified `SummonVampire` branch spawns `VampireSpider` beside a hostile
> target and recasts into a retained-pet recall; `SummonToad` spawns a
> stationary `SpittingToad` that uses Zone-owned `ObjectRangeAttack`;
> `SummonSnakes` creates the retained static `SnakeTotem`, emits totem attack,
> spawns an owned `CharmedSnake` minion with the totem as visible master, and
> lets the minion attack hostile Zone monsters; and `Stonetrap` creates/removes
> an expiring owned `StoneTrap` that hostile native monsters prefer as a decoy
> target without damaging the player. Evidence passed the four
> `zone_native_archer_` regressions, the StoneTrap decoy regression, focused
> `zone_native_player_` 30/30, HolyDeva/PetEnhancer/summon regressions, the
> Gateway summon item-boundary regression, and fmt check. Remaining backend
> risk: full SnakeTotem swarm cap/expiry hardening, VampireSpider self-destruct
> / owner heal details, durable skill-state, and process-external services.

> Latest Zone-native summon/PetEnhancer sync: 2026-05-25 adds ranged
> shared-Zone summon-vs-monster combat and real pet Buff stats on top of
> Crystal-style recast recall.
> Gateway
> recognizes
> `SummonSkeleton` / `SummonShinsu` / `SummonHolyDeva` as targetless summon
> magic, prechecks Zone acceptance, commits the first amulet cost through the
> Account/Inventory
> `SkillItemConsume` boundary, and dispatches to Zone. Zone schedules the
> initial delayed native summon; the verified `SummonSkeleton` branch creates a
> Crystal-template `BoneFamiliar` with `master_object_id` bound to the Zone
> player, `extra=true`, late-AOI retention, and non-hostile player disposition.
> When the same owner recasts while that summon is active, Zone recalls the
> existing retained summon to the authoritative player position and Gateway
> skips the second item transaction. Owned `BoneFamiliar` summons now acquire
> hostile native monsters, emit summon `ObjectAttack`, resolve delayed
> Zone-owned monster damage, and keep awards/drops owned by the master
> session/object instead of the pet object. Owned `Shinsu` now uses the same
> one-amulet item boundary, delayed retained spawn, master binding, and
> hostile-monster melee path. Owned `HolyDeva` now covers its
> delayed retained spawn plus six-tile `ObjectRangeAttack` and 500ms delayed DC
> damage against hostile monsters without damaging players. `PetEnhancer` now
> validates owned Zone summons, emits and retains Buff type 22 with DC/AC stats,
> expires it through Zone, and uses its DC stat for later summon damage.
> Evidence passed the spawn, recall, melee summon-combat, HolyDeva
> ranged-combat, Shinsu summon, and PetEnhancer Simulation regressions, the
> focused `zone_native_player_` suite (30/30), the Gateway summon item-boundary
> regression, existing item precheck coverage, and fmt check. Remaining backend
> risk: HolyDeva kiting polish, archer summon families, durable skill-state,
> and process-external services.

> Latest Zone-native area healing sync: 2026-05-25 moves MassHealing and
> HealingCircle onto the same shared-Zone recovery surface as starter Healing.
> Zone validates near self-target points, selects wounded Zone players inside
> the recovery radius, schedules delayed HP restoration for each target,
> broadcasts `ObjectHealth`, returns `PlayerHealed`, and for HealingCircle emits
> the delayed `ObjectSpell` circle from Zone-owned state. Evidence passed the
> two area-healing regressions with nearby-player recovery assertions, the
> focused `zone_native_player_` suite (28/28), existing Gateway Magic route
> coverage, and fmt check. Remaining backend risk: party/group filtering,
> summons, broader friendly spells, durable skill-state, and process-external
> services.

> Latest Zone-native Healing sync: 2026-05-25 moves starter self-Healing out
> of the personal-session-only skill path. Zone now accepts Healing casts
> against the authoritative player, validates missing HP plus MP/cooldown and
> action windows, emits owner/observer magic and the healing effect, schedules
> delayed HP restoration, broadcasts `ObjectHealth`, and returns
> `PlayerHealed` for Gateway to apply back to the personal runtime. Evidence
> passed the new Healing regression, the focused `zone_native_player_` suite
> (26/26), existing Gateway Magic route coverage, fmt check, and scoped diff
> check. Remaining backend risk: area/friendly healing, summons, Buff families,
> durable skill-state, and process-external ZoneOwner/NPC/economy services.

> Latest Zone-native MagicShield sync: 2026-05-25 adds native shared-Zone
> authority for the first self-target defensive Buff spell. MagicShield now
> resolves through `PlayerCastMagic` with `target_id=0`, spending MP/cooldown
> in Zone, publishing owner/observer magic packets, adding the visible
> MagicShield Buff plus shield-up effect, retaining that Buff for late AOI
> joins, and applying damage-reduction-percent stats to Zone-native monster
> hits. Evidence passed the new focused MagicShield regression, the focused
> `zone_native_player_` group (25/25), existing Gateway Magic route coverage,
> and fmt check. Remaining backend risk: this is one self Buff; full Buff
> families, healing/friendly spells, summons, durable skill-state ownership,
> and process-external ZoneOwner/NPC/economy services still need completion.

> Latest production movement/input sync: 2026-05-25 verifies the live
> movement rollback and input-delay fixes end to end. Gateway release
> `20260525T0334CST-starter-transfer-cleanup` is active on the UCloud host,
> with loopback/public health checks and WSS smoke
> `docs/generated/load/remote-starter-transfer-cleanup-wss-smoke-20260525.json`
> green. Headed production WebGPU packet-walk evidence crossed the old
> starter-demo gate cells from `0:338,270` through `0:343,270`, receiving
> authoritative ACKs `339..343` with no map-change packet and no rollback to
> `330,270`. Player Web deployment `dpl_7iG3bPgA7HTxkvEzN4LxP2rmFmFC` also
> verified that later scene-asset background preloads no longer block movement
> input after first playable readiness:
> `docs/generated/player-qa/movement-jitter/prod-scene-input-unlocked2-webgpu-headed-keyboard-a-nosample-hold-20260525.json`
> passed with held-Walk cadence, WebGPU plus packed prebuilt atlas, clean
> console/network assertions, and ACKs `343,342,341,340,339`. Remaining
> backend/MMO risk is no longer this movement bug; it is the unfinished
> durable ZoneOwner RPC/handoff, durable Account/Inventory and NPC services,
> full Zone-owned skill/Buff state, full monster AI, and 30-active long-run
> gameplay acceptance.

> Latest Crystal runtime starter-transfer sync: 2026-05-25 fixes a production
> movement rollback false-positive that came from an early demo map transfer.
> Crystal runtime config now drops `starter-east-field-gate`, so shared-Zone
> movement from `0:338,270` to `0:339,270` is no longer followed by a same-map
> teleport back to `0:330,270`; generated Crystal movement transfers still
> execute through the existing Zone path. Evidence passed the config regression,
> the Gateway Crystal-runtime movement regression, and the adjacent real
> Crystal movement-transfer regression. Deployed headed production evidence is
> recorded in the production movement/input sync above.

> Latest PoisonCloud live item-route sync: 2026-05-25 enables the
> item-consuming targetless PoisonCloud route through Gateway. The shared route
> now recognizes PoisonCloud as targetless ground magic, prechecks Zone
> acceptance before item consumption, commits `SkillItemConsume` through the
> Account/Inventory service, and only then dispatches the Zone cast. Evidence
> passed
> `shared_in_process_runtime_prechecks_item_skill_before_consuming_items`,
> `shared_in_process_runtime_uses_account_inventory_service_boundary`, and the
> focused PoisonCloud/targetless Zone regressions. Remaining backend risk:
> the Account/Inventory service is still in-process by default and needs a
> durable actor/transaction backend for production MMO parity.

> Latest Zone-native ExplosiveTrap sync: 2026-05-25 adds shared-Zone authority
> for Trap-family detonation behavior. Native ExplosiveTrap now spawns the
> caster-facing trap row, emits delayed `ObjectSpell` trap objects, applies
> contact damage from Zone-owned ground-spell state, and removes itself after
> the first detonation. Gateway's targetless ground-magic route now recognizes
> ExplosiveTrap as a non-item Zone spell. Evidence passed
> `zone_native_player_explosive_trap_spawns_front_row_and_detonates_once` and
> the focused `zone_native_player` group. Remaining backend risk: broader
> control skills, summons, and durable skill-state persistence still need
> Zone-owned implementations.

> Latest Zone-native TrapHexagon sync: 2026-05-25 adds shared-Zone authority
> for the next Trap-family control spell. Native TrapHexagon now roots hostile
> Zone monsters in the target area, queues the delayed eight-point ring of
> `ObjectSpell` packets, and keeps rooted monsters from walking until the
> control window expires. Evidence passed
> `zone_native_player_trap_hexagon_roots_area_and_spawns_ring_objects`.
> Remaining backend risk: broader control skills, summons, and durable
> skill-state persistence still need Zone-owned implementations.

> Latest Skill item-consumption boundary sync: 2026-05-25 adds the first
> Account/Inventory command for item-consuming Zone skills. Shared Gateway
> services now accept identity-bearing `SkillItemConsume` envelopes, and the
> default in-process service can transact PoisonCloud's amulet plus green-poison
> item costs into `DeleteItem` receipt packets. The 2026-05-26 sync adds the
> missing request id to that envelope. Evidence passed
> `in_process_account_inventory_service_handles_skill_item_consumption_command`
> and `shared_in_process_runtime_uses_account_inventory_service_boundary`.
> Remaining backend risk: the default service is still in-process and needs a
> durable actor/transaction backend.

> Latest targetless ground-magic route sync: 2026-05-25 removes the strict
> object-target requirement from the first shared-Zone ground magic path.
> `ZoneRuntime` now accepts `PlayerCastMagic` with `target_id=0` for
> ground-target spells, validates range/action windows, emits owner/observer
> magic packets, and schedules delayed ground-spell objects from the target
> point. Gateway shared attack preparation now treats learned
> FireWall/Blizzard/MeteorStrike/PoisonCloud packets with `target_id=0` as
> Zone commands instead of falling back to personal-session object targeting;
> PoisonCloud item cost commits through Account/Inventory after Zone precheck.
> Evidence passed `zone_native_player_firewall_accepts_targetless_ground_cast`,
> the focused `zone_native_player` group, and locked Gateway+Simulation check.

> Latest Zone-native Trap sync: 2026-05-25 adds shared-Zone authority for the
> first Trap-family root/control effect. `ZoneNativeMonster` now retains the
> Crystal monster level from spawns, letting Zone enforce Trap's lower-level
> target gate. Native Trap roots eligible hostile Zone monsters for the control
> window and queues the delayed Trap `ObjectSpell` with direction and param
> semantics. Evidence passed
> `zone_native_player_trap_spawns_object_and_roots_lower_level_monster` and the
> focused `zone_native_player` group. Remaining backend risk: broader control
> skills, summons, and durable skill-state persistence still need Zone-owned
> implementations.

> Latest Zone-native PoisonCloud sync: 2026-05-25 adds shared-Zone authority
> for PoisonCloud's monster-side ground effects. Native PoisonCloud now
> schedules the delayed visible cloud object, damages hostile monsters standing
> in the 3x3 cloud, and applies/broadcasts green poison state from
> `ZoneRuntime`. Evidence passed
> `zone_native_player_poison_cloud_spawns_ground_spell_and_poisons_monsters`
> and the focused `zone_native_player` group. Remaining backend risk: the
> Account/Inventory command boundary still needs a durable external service.

> Latest Zone-native chain/splash sync: 2026-05-25 adds native shared-Zone
> authority for MeteorShower and FireBounce secondary effects. MeteorShower
> now collects nearby hostile secondary targets inside `ZoneRuntime`, publishes
> those ids in owner/observer magic packets, and applies half-damage secondary
> hits. FireBounce now schedules follow-up `ObjectProjectile` hops and delayed
> damage between Zone monsters instead of leaving bounce behavior in the
> personal runtime. Evidence passed
> `zone_native_player_meteor_shower_damages_primary_and_secondary_monsters`,
> `zone_native_player_fire_bounce_chains_projectiles_and_damage`, and the
> focused `zone_native_player` group. Remaining backend risk:
> remaining Trap-family actions, summons, durable skill state, and
> profession-specific bespoke effects still need Zone-owned implementations.

> Latest Zone-native ground-spell sync: 2026-05-25 adds the first persistent
> ground-spell implementations under shared Zone authority. Native FireWall
> casts now schedule delayed `ObjectSpell` fire cells, avoid immediate direct
> monster damage, and tick recurring same-cell damage from `ZoneRuntime`
> ground-spell state. Native Blizzard/MeteorStrike use the same scheduler for
> delayed 5x5 `ObjectSpell` cells and later recurring occupied-cell damage.
> Evidence passed `zone_native_player_firewall_spawns_ground_spell_and_ticks_damage`,
> `zone_native_player_blizzard_family_spawns_ground_spell_and_ticks_damage`,
> and the focused `zone_native_player` group. Remaining backend risk:
> remaining Trap-style ground actions and broader profession-specific spell
> effects still need the same Zone-owned treatment.

> Latest Zone-native area magic sync: 2026-05-25 adds the first native
> multi-target spell damage path. Shared Zone now derives secondary target ids
> for target-centered 3x3 spells such as FireBang/IceStorm, publishes those ids
> in owner `Magic` and observer `ObjectMagic`, and schedules native damage for
> each affected Zone monster. Evidence passed
> `zone_native_player_area_magic_damages_secondary_monsters_authoritatively`
> and the focused `zone_native_player` group. Remaining backend risk: ground
> spell persistence, chain spells, splash formula differences, and broader
> per-spell special behavior still need Zone-owned implementations.

> Latest Zone-native special arrow Buff sync: 2026-05-25 adds the first
> shared-Zone-owned Archer special-arrow Buff side effect. Native PoisonShot
> now records the visible arrow marker Buff on `ZonePlayer`, includes it in
> late `ObjectPlayer` visibility, and native CrippleShot consumes that
> Zone-held PoisonShot Buff before applying green poison to nearby native Zone
> monsters. Native VampireShot now schedules Zone-owned player healing,
> broadcasts authoritative player health, and returns `PlayerHealed` so Gateway
> updates the personal runtime HP rather than leaving save/combat state stale.
> Evidence passed
> `zone_native_player_poison_shot_applies_visible_arrow_buff`,
> `zone_native_player_cripple_shot_consumes_poison_buff_and_spreads_green_poison`,
> `zone_native_player_vampire_shot_heals_owner_through_zone_authority`, the
> CrippleShot vampire follow-up regression, Gateway pending-heal regressions,
> and the focused `zone_native_player` shared-Zone group. Remaining backend
> risk: full Buff stat families, summons, AoE/ground spells, and durable
> skill-state ownership still need native authority.

> Latest Gateway Magic route sync: 2026-05-25 adds practical shared-runtime
> coverage for `ClientPacket::Magic` routing into shared Zone authority.
> `shared_in_process_runtime_routes_magic_through_shared_zone` now proves a
> seeded magic launch returns owner `Magic`/`ObjectMagic` packets and reaches a
> second observer through the shared Zone drain path; the focused Gateway route
> group also keeps the existing RangeAttack route covered. Remaining backend
> risk: this is route coverage for a seeded spell; complete parity still needs
> Zone-owned spell-specific effects, Buffs, projectile variants, and durable
> skill-state authority.

> Latest Zone-native poison tick sync: 2026-05-25 adds the first shared-Zone
> damage-over-time loop for player-applied monster poison. Native `PoisonShot`
> now commits green poison state on `ZoneNativeMonster`, broadcasts the poison
> state, ticks damage every 2 seconds inside Zone, publishes health/damage
> packets, and can kill through the existing Zone drop and kill-award commit
> surface. Evidence passed
> `zone_native_player_poison_shot_ticks_green_damage_and_awards_kill`, the
> focused `zone_native_player` shared-Zone group, Simulation fmt check, and
> locked Simulation check. Remaining backend risk: many poison variants,
> CrippleShot spread, PoisonCloud, Boss/area status AI, and durable economy/NPC
> process boundaries still need native authority.

> Latest ZoneOwner heartbeat sync: 2026-05-25 adds the first scheduled owner
> liveness path on top of the existing command-client and TTL authority work.
> Gateway sessions can now configure a ZoneOwner heartbeat interval, renew
> leases with a time-aware authority call, and the Web runtime tick performs
> that heartbeat before any deferred world tick. Evidence passed focused
> `zone_owner` gateway coverage including current-owner renewal, handoff
> rejection, owner-boundary stale rejection, TTL renewal/expiry, and the new
> heartbeat-before-expiry / missed-heartbeat regressions. Remaining backend
> risk: the command client is still in-process; real RPC transport,
> process-external owner loops, handoff, and takeover are still required.

> Latest Zone-native player action-window sync: 2026-05-25 adds authoritative
> shared-Zone action timing for player combat commands. `ZonePlayer` now keeps
> attack and spell readiness timestamps; native melee/range attacks set and
> honor the Crystal attack window, and native magic sets and honors a
> cross-spell cast window before committing MP, per-spell cooldown, control,
> projectile, or delayed damage effects. Evidence passed
> `zone_native_player_range_attack_respects_attack_action_window`,
> `zone_native_player_magic_respects_spell_action_window_across_spells`, and
> the focused `zone_native_player` shared-Zone group, plus
> `shared_in_process_runtime_rejects_early_range_attack_at_zone_boundary` for
> the Gateway shared-runtime route. Remaining backend risk:
> more skill/Buff effects, poison damage ticks, Boss AI, and process-external
> ZoneOwner/NPC/economy actors still need to replace in-process adapters.

> Latest NPC world-service command-envelope sync: 2026-05-25 adds the first
> identity-bearing NPC world command surface in Gateway. NPC saved values and
> Crystal random seed updates plus diff-derived NPC entity side-effect packets
> now submit `SharedNpcWorldCommandEnvelope` payloads through
> `SharedNpcWorldService` before shared Zone NPC/map state is mutated. Evidence passed
> `shared_in_process_runtime_uses_npc_world_service_boundary`, which verifies
> active account/character identity, saved-value commit, random-seed commit, and
> entity-side-effect commit. Remaining backend risk: the default service still
> commits in-process bridge state; full NPC authority still needs native
> world-service commands for `MONGEN`, `MONCLEAR`, event flags, NPC services,
> quest rewards, and economy rollback.

> Latest Account/Inventory command-envelope sync: 2026-05-25 replaces the
> reward service's operation-specific Gateway calls with one command envelope.
> `SharedAccountInventoryCommandEnvelope` now carries the active
> account/character identity plus `MonsterKillAward` or `GroundDropPickup`
> command payloads; `SharedInProcessZoneSessionRuntime` submits those
> envelopes through the service and the default in-process service dispatches
> them to the existing character commit functions. Evidence passed
> `shared_in_process_runtime_uses_account_inventory_service_boundary`, which
> now verifies the identity-bearing command envelopes as well as
> service-produced packets and rejected pickup rollback. Evidence also passed
> `in_process_account_inventory_service_rejects_identity_mismatch`, proving the
> default adapter refuses envelopes for a different account/character instead
> of mutating the active runtime. Remaining backend risk: the command envelope
> still executes against session-backed state until a durable actor/store
> implementation replaces it.

> Latest ZoneOwner command-client sync: 2026-05-25 inserts the first
> replaceable command client between `GatewaySession` and the in-process Zone
> runtime. Gateway still owns lease validation first, but valid
> `ZoneOwnerCommandRequest` envelopes now execute through
> `ZoneOwnerCommandClient`; the default `InProcessZoneOwnerCommandClient`
> preserves current behavior and test injection proves both valid dispatch
> and stale-lease rejection before client invocation. `GatewaySession` can now
> renew its current owner lease through the authority, while stale owners fail
> renewal after handoff. The in-process command client can carry the same
> authority and enforce fencing at the owner boundary too. The in-memory
> authority now has optional TTL semantics, so renewals before expiry extend the
> lease while expired renewals fail and advance the next fencing token. Evidence passed
> `gateway_session_dispatches_valid_zone_owner_request_through_command_client`
> and `gateway_session_rejects_stale_zone_owner_before_command_client`, plus
> the current-owner, post-handoff renewal, and owner-boundary stale-request
> regressions, plus the TTL renewal/expired-renewal authority regressions.
> Remaining backend risk: the command client is not yet a network/RPC client to
> an external ZoneOwner process with scheduled heartbeat renewal.

> Latest Zone-native monster status sync: 2026-05-25 adds the first shared
> authority slice for special monster-to-player status effects. Zone-native
> delayed monster hits now evaluate AI 7/22 paralysis and AI 28/37 green
> poison using the Crystal deterministic chance shape, update Zone player
> poison state, emit `ObjectPoisoned`, expire Zone-owned poison state on
> tick, and reject movement while paralysis is active. Evidence passed
> `zone_native_monster_paralysis_blocks_movement_until_status_expires`,
> `zone_native_monster_green_poison_does_not_block_movement`, focused
> `zone_native_monster` shared-Zone coverage, and Simulation fmt check.
> Remaining backend risk: broader status variants, poison damage ticks, Boss
> AI branches, and summon/controller mechanics still need Zone-native
> implementations.

> Latest Account/Inventory service-boundary sync: 2026-05-25 inserts the
> first replaceable actor/service boundary behind Zone reward commits.
> Gateway's shared Zone runtime now owns a `SharedAccountInventoryService`
> handle; monster kill awards and shared ground-drop pickup claims submit
> through that handle, with `InProcessAccountInventoryService` preserving the
> existing personal-session implementation. Evidence passed
> `shared_in_process_runtime_uses_account_inventory_service_boundary`, which
> injects a recording service, verifies kill-award packets can be produced
> without mutating personal-session experience, and verifies rejected pickup
> commits still cancel/restore the Zone claim. Remaining backend risk: the
> default service is still session-backed; production MMO parity needs an
> external Account/Inventory actor or transactional service behind the same
> trait.

> Latest NPC entity side-effect sync: 2026-05-25 closes the silent
> `MONGEN` / `MONCLEAR` bridge gap enough for shared-Zone observers to see
> deterministic entity packets. Gateway now captures pre/post NPC command
> monster snapshots, emits Crystal-data-backed `ObjectMonster` packets for
> generated monsters, and emits `ObjectHealth(0)` / `ObjectDied` /
> `ObjectRemove` packets for cleared or removed monsters. Shared entity
> observer routing also treats health, death, and remove as shared-object
> update anchors. Evidence passed
> `shared_npc_entity_side_effects_emit_spawn_packets_for_new_monsters`,
> `shared_npc_entity_side_effects_emit_death_packets_for_monclear`, plus the
> adjacent NPC random/saved-value regressions. Remaining backend risk: these
> are still diff-derived bridge packets, not first-class Zone-owned NPC
> commands; map/event flags, NPC services, and rollback-sensitive
> quest/economy commits still need native world-service submission.

> Latest NPC random shared-state sync: 2026-05-25 moves Crystal NPC
> `RANDOM` branch state out of isolated personal-session execution.
> `SimulationSession` now exposes `shared_npc_random_seed` and
> `apply_shared_npc_random_seed`, while Gateway shared Zone state stores the
> current seed, applies it before NPC commands, and publishes the new seed
> after those commands. Evidence passed
> `shared_in_process_registry_syncs_npc_random_seed_between_sessions` and
> `cargo +1.89.0 fmt --check -p mir2-gateway -p mir2-simulation`. Remaining
> backend risk: this synchronizes script branch randomness only; full
> NPC/quest authority still needs Zone/world-service commits for `MONGEN`,
> `MONCLEAR`, event flags, service trades, and rollback-sensitive
> quest/economy side effects.

> Latest Zone-owner command fencing sync: 2026-05-25 adds the first
> command-side stale-owner rejection point. `GatewaySession` now sends player
> commands through `ZoneOwnerCommandRequest` envelopes and validates a
> supplied `ZoneOwnerLease` before `execute_with_outcome` or
> `execute_production_player_command` can reach the runtime; the production
> Web session-action path passes through that fenced execution wrapper for
> normal player commands. `ZoneRegistry` now supplies a shared
> `ZoneOwnerLeaseAuthority`, and the in-memory authority can advance a zone's
> owner/fencing token on handoff so a session holding the old lease is
> rejected by the authority, not just by local request/session comparison.
> Evidence passed
> `gateway_session_accepts_current_zone_owner_fenced_command`,
> `gateway_session_rejects_stale_zone_owner_fencing_token`, and
> `gateway_session_rejects_wrong_zone_owner_before_production_command`,
> `gateway_session_rejects_superseded_zone_owner_after_handoff`.
> Remaining backend risk: the current owner is still in-process and the Web
> path supplies the session's own current lease; complete distributed MMO
> ownership still needs Gateway -> ZoneOwner RPC, TTL renewal, takeover
> recovery, fencing-token propagation across processes, and stale command
> rejection at the real owner boundary.

> Latest Zone-owner fencing metadata sync: 2026-05-25 adds the first explicit
> owner/fencing facts to the Gateway routing boundary. `ZoneRuntimeFactory`
> now exposes a `ZoneOwnerLease`, `RoutedZoneRuntime` carries
> `zoneOwnerId` / `fencingToken` alongside `zoneId`, `GatewaySession`
> preserves that owner lease, and session-cache records/routes persist the
> same metadata for online character routing. The current in-process owner is
> stable (`in-process:<zoneId>`, token `1`), which does not implement
> cross-process handoff by itself, but it gives the later ZoneOwner RPC and
> stale-owner rejection path concrete fields instead of only a map/zone string.
> Evidence passed
> `in_process_registry_routes_new_sessions_to_primary_zone`,
> `zone_registry_can_route_sessions_through_policy`,
> `session_cache_hit_matches_authoritative_world_snapshot_after_refresh`,
> `session_cache_routes_online_character_to_zone`,
> `session_cache_route_request_can_drive_zone_registry`,
> `admin_sessions_and_control_endpoints_are_queryable`,
> `cargo +1.89.0 fmt --check -p mir2-gateway -p mir2-simulation`, scoped
> `git diff --check`, and locked `cargo +1.89.0 check -p mir2-gateway -p
> mir2-simulation`. Remaining backend risk: Gateway still hosts the in-process
> shared Zone; the final target is Gateway -> ZoneOwner RPC with lease renewal,
> fencing-token validation, handoff, and stale command rejection.

> Latest NPC saved-value shared-state sync: 2026-05-25 moves the first
> Crystal NPC script side effect out of per-session-only state. Crystal
> `SAVEVALUE` / `LOADVALUE` data is now exposed as `SharedNpcSavedValue`;
> Gateway's shared Zone state keeps a case-insensitive world-service copy,
> applies saved values into a session before NPC commands, and publishes
> updated values back after NPC commands. This means sparse/shared sessions no
> longer have to see separate values for the same NPC script save slot.
> Evidence passed
> `shared_in_process_registry_syncs_npc_saved_values_between_sessions`,
> `shared_in_process_registry_surfaces_shared_npcs_for_sparse_sessions`,
> `shared_in_process_registry_callnpc_shared_guide_starts_sparse_session_quest`,
> `shared_in_process_runtime_uses_account_inventory_receipts_for_zone_rewards`,
> `cargo +1.89.0 fmt --check -p mir2-simulation -p mir2-gateway`, scoped
> `git diff --check`, and locked `cargo +1.89.0 check -p mir2-simulation -p
> mir2-gateway`. Remaining backend risk: this covers one persistent NPC script
> value family; quest acceptance/progress, NPC service trades, event flags,
> and map/world mutations still need real Zone/world-service submission.

> Latest Account/Inventory transaction-boundary sync: 2026-05-25 moves the
> current shared reward bridge one step closer to a real transaction service.
> `SimulationSession` now exposes
> `SharedAccountInventoryTransactionReceipt { kind, committed, packets }` for
> shared ground-drop pickup and Zone-native monster kill awards, and Gateway
> consumes that receipt for Zone reward commits instead of calling separate
> ad-hoc personal-session reward methods. `GroundDropPickup` receipts still
> drive Zone `CommitGroundDropClaim` / `CancelGroundDropClaim`; `MonsterKillAward`
> receipts carry the committed experience/quest packets after character state
> mutation. Evidence passed
> `shared_in_process_runtime_uses_account_inventory_receipts_for_zone_rewards`,
> `shared_in_process_runtime_emits_gain_experience_after_kill_award_commit`,
> `shared_in_process_runtime_rolls_back_shared_gold_claim_when_gold_cap_blocks_commit`,
> `shared_ground_drop_pickup_commit_reports_gold_commit_and_cap_reject`,
> `cargo +1.89.0 fmt --check -p mir2-simulation -p mir2-gateway`, scoped
> `git diff --check`, and locked `cargo +1.89.0 check -p mir2-simulation -p
> mir2-gateway`. Remaining backend risk: the receipt boundary is now unified,
> but storage is still the in-process personal character state; the next
> target is an Account/Inventory actor or transactional service that can commit
> gold/items/experience/quest side effects outside the Zone hot path.

> Latest 30-active movement/chat backend sync: 2026-05-25 accepted the single
> UCloud Gateway at `60 ws / 30 active / 30 reconnect leases` for sustained
> shared-Zone movement and chat. The 30-active delay was isolated to backend
> hot-path work, not WebGPU/DOM rendering: route-lease refresh was coupled to
> the busy WebSocket select loop, and each movement checked transfer tiles by
> pulling a full personal-session world snapshot. Gateway now refreshes owned
> route leases from a per-socket background task, combines movement intent and
> player movement tick under one Zone lock, coalesces pending observer
> movement packets, caches same-map transfer tiles from the shared snapshot,
> and lazily builds retained AOI visibility packets only when visibility
> changes. Evidence passed local and UCloud locked Simulation/Gateway checks;
> production release `20260525T1348CST-route-refresh-background-task`
> (`archive sha256
> 76bd65385ce14ce7926ce072613cda9d7e4e4e5fdc478fbe149cfe237ad27b96`,
> `binary sha256
> d5aebfa9c82a440dcc63ca13d67d27f34c36e3b20e6996421d8f22567b3d608b`)
> is live. Public 30-active movement-only evidence
> `docs/generated/load/public-route-refresh-background-task-30active-movementonly1m-settle30s-20260525.json`
> passed `ready=30/30`, `capacityRejected=0`, `errors=0`, `ok=true`,
> `keepAlive.p95=63ms`; public 30-active move/chat evidence passed with
> chat every 30 actions
> `docs/generated/load/public-route-refresh-background-task-30active-movechat1m-chat30-settle30s-20260525.json`
> (`keepAlive.p95=222ms`) and chat every 10 actions
> `docs/generated/load/public-route-refresh-background-task-30active-movechat1m-chat10-settle30s-20260525.json`
> (`keepAlive.p95=68ms`). Remaining backend risk: this promotes movement/chat
> feel for the current single-Gateway shared Zone, but full Shared MMO parity
> still needs the Account/Inventory transaction service, NPC/quest world
> state, special monster AI, and cross-Gateway Zone owner fencing/handoff.

> Latest shared ground-drop commit receipt sync: 2026-05-25 replaced the
> Gateway's packet-shape inference for shared ground-drop claims with an
> explicit transaction receipt. `SimulationSession` now returns
> `SharedGroundDropPickupCommit { committed, packets }` for shared pickup
> commits, and Gateway drives `CommitGroundDropClaim` / `CancelGroundDropClaim`
> from `committed` instead of guessing from `GainedGold` / `GainedItem`
> packets. Evidence passed
> `shared_ground_drop_pickup_commit_reports_gold_commit_and_cap_reject`,
> `shared_in_process_runtime_rolls_back_shared_gold_claim_when_gold_cap_blocks_commit`,
> normal remote shared-gold pickup, local locked Simulation/Gateway check,
> remote UCloud focused tests, and remote locked Simulation/Gateway check.
> UCloud Gateway release `20260525T0843CST-grounddrop-commit-receipt` is live
> over `20260525T0827CST-zone-award-commit` (`archive sha256
> c9652900c7a98e261872a32c71c21ea18b51e3a4eb30e4e3227d82bf174733be`,
> `binary sha256
> 324837e4d622a0fbfbc248def8f6a9630820dec4e0e4a2452575a8d9e959a944`).
> Public health, 1-client WSS smoke
> `docs/generated/load/remote-grounddrop-commit-receipt-wss-smoke-20260525.json`
> (`ready=1/1`, `capacityRejected=0`, `errors=0`, `messages=625`,
> `ok=true`), and 30-client safe-cap evidence
> `docs/generated/load/remote-grounddrop-commit-receipt-30active-timeout60-20260525.json`
> (`ready=15/30`, `capacityRejected=15`, `errors=0`, `messages=9629`,
> `ok=true`) passed. Remaining backend risk: this hardens the current
> personal-session bridge into an explicit commit receipt, but the final target
> is still a real Account/Inventory transaction service covering all
> gold/items/quest side effects and rollback.

> Latest shared kill-award commit sync: 2026-05-25 moved Zone-native kill
> reward notification closer to a transactional commit model. `ZoneRuntime`
> now emits `MonsterKillAward` for the owner but no longer pre-sends
> `GainExperience`; the Gateway/personal commit side applies the award to the
> authoritative character state and emits `GainExperience` only for the
> actually written experience amount. Evidence passed
> `zone_native_monster_combat_kill_and_drop_are_authoritative`,
> `shared_in_process_runtime_emits_gain_experience_after_kill_award_commit`,
> shared `RangeAttack` routing, fallback drop-template coverage, local locked
> Simulation/Gateway check, remote UCloud focused tests, and remote locked
> Simulation/Gateway check. UCloud Gateway release
> `20260525T0827CST-zone-award-commit` is live over
> `20260525T0804CST-zone-fallback-drops` (`archive sha256
> 0f45247318dc656abc8e7d4bb02adc4744f644d298be68599096f31b21b8e58e`,
> `binary sha256
> ca8983284a60f22f1823bdf2c0d8a4eb6c360a19ee5bd24789f080a72ba03461`).
> Public health, 1-client WSS smoke
> `docs/generated/load/remote-zone-award-commit-wss-smoke-20260525.json`
> (`ready=1/1`, `capacityRejected=0`, `errors=0`, `messages=625`,
> `ok=true`), and 30-client safe-cap evidence
> `docs/generated/load/remote-zone-award-commit-30active-timeout60-20260525.json`
> (`ready=15/30`, `capacityRejected=15`, `errors=0`, `messages=9230`,
> `ok=true`) passed. Remaining backend risk: experience now follows the
> current commit boundary, but the long-term target is still a real
> Account/Inventory transaction service for all rewards, drops, gold, quest
> side effects, and rollback.

> Latest shared fallback drop-template sync: 2026-05-25 moved another
> drop/economy authority edge out of personal-session-only behavior. When the
> Gateway must materialize a Zone-native monster from a shared entity snapshot
> instead of from the local `SimulationSession`, the fallback
> `ZoneMonsterSpawn` now restores Crystal/starter drop templates at the
> current shared tick instead of carrying `drops=[]`. This means a sparse
> shared-session attack path can still let Zone-native monster death spawn
> owner-window ground drops. Evidence passed
> `zone_monster_spawn_from_shared_entity_restores_crystal_drop_templates`,
> neutral Royal Guard/Archer AI fallback coverage, the Simulation native
> kill/drop authority regression, shared `RangeAttack` routing, rollback claim
> coverage, local locked Simulation/Gateway check, remote UCloud focused tests,
> and remote locked Simulation/Gateway check. UCloud Gateway release
> `20260525T0804CST-zone-fallback-drops` is live over
> `20260525T0734CST-zone-monster-ranged` (`archive sha256
> 998843b7c94f7f9ee2dc227b02fa3d6d905c731f5e4f2a8d28b2d87d931c73c2`,
> `binary sha256
> 057fc064eaf640bfc491f46f173590b1df3f280525ec090b91c04baea2a59ace`).
> Public health, 1-client WSS smoke
> `docs/generated/load/remote-zone-fallback-drops-wss-smoke-20260525.json`
> (`ready=1/1`, `capacityRejected=0`, `errors=0`, `messages=625`, `ok=true`),
> and 30-client safe-cap evidence
> `docs/generated/load/remote-zone-fallback-drops-30active-timeout60-20260525.json`
> (`ready=15/30`, `capacityRejected=15`, `errors=0`, `messages=9629`,
> `ok=true`) passed. Remaining backend risk: this closes the fallback spawn
> drop-template hole, but full drop/economy parity still needs Zone-owned drop
> generation for every monster lifecycle plus transactional Account/Inventory
> reward commit.

> Latest shared drop/economy rollback sync: 2026-05-25 added Gateway
> regression evidence for the existing shared Zone drop claim transaction
> boundary. A shared Zone gold drop is now claimed through
> `GroundDropClaimed`, the personal economy commit is forced to fail via the
> gold cap, and the test proves the Gateway does not emit `GainedGold` or
> `ObjectRemove`; instead it cancels the Zone claim, restores the shared map
> drop, and respawns `ObjectGold` for the owner. Evidence passed
> `shared_in_process_runtime_rolls_back_shared_gold_claim_when_gold_cap_blocks_commit`,
> adjacent normal shared-drop claim, remote shared-gold pickup, intelligent
> creature remote shared-gold pickup, locked Gateway check, and Gateway fmt
> check. No production release was needed because this slice adds regression
> coverage for already-present commit/cancel behavior. Remaining backend risk:
> this validates the current bridge rollback boundary, but full Crystal/MMO
> parity still needs Zone-owned drop generation and a real Account/Inventory
> transaction service instead of personal-session economy mutation.

> Latest Zone-native ranged monster AI sync: 2026-05-25 moved the first
> non-melee native monster attack branch into shared Zone authority.
> `ZoneNativeMonster` now retains its Crystal `ai`, and ranged/magic-style AI
> such as `ai=19` attacks visible non-adjacent players with
> `ObjectRangeAttack` plus delayed Zone-owned player damage instead of walking
> forward until adjacent. The delayed hit reuses the Zone player-damage commit
> path, so Zone-held defensive Buff mitigation still applies before HP mutation
> or `PlayerDamaged` outbounds. Evidence passed
> `zone_native_ranged_monster_attacks_without_chasing_when_target_not_adjacent`,
> adjacent native-monster melee tick, attack/defence Buff regressions, Gateway
> shared `RangeAttack` routing coverage, local locked Simulation/Gateway
> check, remote UCloud focused test, and remote locked Simulation/Gateway
> check. UCloud Gateway release
> `20260525T0734CST-zone-monster-ranged` is live over
> `20260525T0720CST-zone-buff-defence` (`archive sha256
> 6b18bec9d9a9b2eb1578bc16a99a9efef237633ad47a4502214ca8a11bfabdee`,
> `binary sha256
> c12b1a33255a8d9c87946d5cf9a3257d3097013aa3c7aaf91b6a832a0821cf53`).
> Public health and WSS smoke
> `docs/generated/load/remote-zone-monster-ranged-wss-smoke-20260525.json`
> passed with `ready=1/1`, `capacityRejected=0`, `errors=0`, `messages=414`,
> and `ok=true`. The current 30-client safe-cap baseline
> `docs/generated/load/remote-zone-monster-ranged-30active-timeout60-20260525.json`
> also passed with `ready=15/30`, `capacityRejected=15`, `errors=0`,
> `ok=true`, and keepalive p95 `15881ms`. Remaining backend risk: this is the
> first generic ranged/magic monster AI slice, not complete Boss/ranged spell
> parity; wider special AI, AoE/ground spells, summons, NPC/quest side effects,
> economy transactions, Zone owner handoff, and accepted 30-active feel remain
> open.

> Latest Zone-owned defensive Buff stat sync: 2026-05-25 moved the first
> defensive Buff stat into shared Zone player-damage authority. Zone-native
> monster delayed hits now subtract the target player's Zone-held `MAX_AC`
> Buff stat before mutating Zone HP or emitting `ObjectStruck` /
> `DamageIndicator` / `ObjectHealth` / `PlayerDamaged`; once the Zone-held
> Buff expires, the same native monster hit again commits normal damage.
> Evidence passed
> `zone_native_player_defence_buff_mitigates_monster_damage_until_expiry`,
> adjacent attack-stat Buff and native-monster delayed-hit regressions, the
> Gateway shared `RangeAttack` routing regression, local locked
> Simulation/Gateway check, remote UCloud focused test, and remote locked
> Simulation/Gateway check. UCloud Gateway release
> `20260525T0720CST-zone-buff-defence` is live over
> `20260525T0709CST-zone-buff-stats`; public health and WSS smoke
> `docs/generated/load/remote-zone-buff-defence-wss-smoke-20260525.json`
> passed with `ready=1/1`, `capacityRejected=0`, `errors=0`, `messages=623`,
> and `ok=true`. The current 30-client safe-cap baseline
> `docs/generated/load/remote-zone-buff-defence-30active-timeout60-20260525.json`
> also passed with `ready=15/30`, `capacityRejected=15`, `errors=0`,
> `ok=true`, and keepalive p95 `16458ms`. Remaining backend risk: Buff
> attack/defence stats now affect Zone-native hit commits, but rate stats,
> status-specific Buff behavior, summons/pets, AoE/ground spells, richer monster
> AI, NPC/quest side effects, economy transactions, Zone owner handoff, and
> accepted 30-active feel remain open.

> Latest Zone-owned Buff stat authority sync: 2026-05-25 moved the first
> player Buff stat effect into shared Zone damage authority. The personal
> session now gives Zone-native melee/range/object-Magic paths an unbuffed
> equipment/passive base damage profile, while `ZoneRuntime` applies its own
> tracked player `AddBuff.stats` before committing native monster damage.
> Buff expiry removes the Zone-side stat effect, so the same attack returns to
> base damage after the Zone-owned expiry. Evidence passed
> `zone_native_player_buff_stats_authoritatively_modify_damage_until_expiry`,
> the existing Zone object-Magic tests, the Gateway shared `RangeAttack`
> routing regression, local locked Simulation/Gateway check, remote UCloud
> focused test, and remote locked Simulation/Gateway check. UCloud Gateway
> release `20260525T0709CST-zone-buff-stats` is live over
> `20260525T0651CST-zone-magic-control`; public health and WSS smoke
> `docs/generated/load/remote-zone-buff-stats-wss-smoke-20260525.json` passed
> with `ready=1/1`, `capacityRejected=0`, `errors=0`, `messages=623`, and
> `ok=true`. The current 30-client safe-cap baseline
> `docs/generated/load/remote-zone-buff-stats-30active-timeout60-20260525.json`
> also passed with `ready=15/30`, `capacityRejected=15`, `errors=0`,
> `ok=true`, and keepalive p95 `16546ms`. Remaining backend risk: this covers
> the first DC-style Buff stat in Zone damage commit; full Buff authority still
> needs defensive stats, rate stats, status-specific behavior, summon/pet Buffs,
> AoE/ground spells, and broader monster/NPC/economy authority.

> Latest Zone-native Magic control sync: 2026-05-25 moved the first real object
> Magic control effect into shared Zone authority. `ZoneNativeMonster` now
> carries Zone-owned control state; `PlayerCastMagic` applies ElectricShock,
> Entrapment, and CatTongue control in the Zone, blocks native monster movement
> and attacks until the control expires, emits Crystal control packets for
> Entrapment/CatTongue (`ObjectEffect`/`ObjectPoisoned`) and clears poison on
> expiry. ElectricShock and Entrapment now use a zero-damage Zone magic profile
> so control spells no longer get bridged into fake one-point hits. Evidence
> passed locally and on UCloud with
> `zone_native_player_magic_control_stops_monster_ai_until_expiry`, the
> existing Zone magic damage/MP cooldown tests, the native monster tick tests,
> the Gateway shared `RangeAttack` routing regression, and locked
> Simulation/Gateway check. UCloud Gateway release
> `20260525T0651CST-zone-magic-control` is live over
> `20260525T0630CST-zone-magic-mp-cooldown`; public health and WSS smoke
> `docs/generated/load/remote-zone-magic-control-wss-smoke-20260525.json`
> passed with `ready=1/1`, `capacityRejected=0`, `errors=0`, `messages=623`,
> and `ok=true`. A 30-client production-capacity baseline with the longer
> login-ready gate passed at the current safe cap in
> `docs/generated/load/remote-zone-magic-control-30active-timeout60-20260525.json`
> (`ready=15/30`, `capacityRejected=15`, `errors=0`, `ok=true`, keepalive p95
> `16470ms`). Remaining backend risk: this is the first Zone-owned control
> slice, not full skill authority; Buff stats, summons, AoE/ground spells,
> broader monster ranged/magic/Boss AI, NPC/quest side effects, transactional
> economy commit, and true Zone owner handoff remain open.

> Latest Zone-native RangeAttack/Magic authority sync: 2026-05-25 added the
> next shared-combat slice after native melee. `ZoneRuntime` now accepts
> `PlayerRangeAttackObject` and `PlayerCastMagic` commands, validates that the
> requested target object is a live Zone-native monster at the submitted target
> tile, checks action range, emits Crystal launch packets
> (`RangeAttack`/`ObjectRangeAttack` and `Magic`/`ObjectMagic`/`ObjectProjectile`),
> owns object-magic MP spend/cooldown rejection with `ObjectMana` AOI fanout,
> and defers the actual `ObjectStruck` / `DamageIndicator` / `ObjectHealth` /
> death/drop/experience side effects until the Zone tick commits the pending
> hit. Gateway shared sessions now route `ClientPacket::RangeAttack` to this
> Zone path and can route object-target `ClientPacket::Magic` when the
> personal session exposes a learned Crystal magic profile; the personal
> session contributes the current skill/stat damage/cost profile and mirrors a
> Zone-accepted MP/cooldown spend for UI/save, but no longer mutates target
> monster HP/death/drop authority. Evidence passed
> `zone_native_player_range_attack_damages_monster_authoritatively`,
> `zone_native_player_magic_damages_monster_and_projects_authoritatively`,
> `zone_native_player_magic_spends_mana_and_enforces_cooldown`,
> `zone_native_player_range_attack_rejects_invalid_target`,
> `shared_in_process_runtime_routes_range_attack_through_shared_zone`, the
> existing delayed melee Gateway regression, and
> `cargo +1.89.0 check --locked -p mir2-gateway`. The UCloud Gateway host was
> rebuilt and rolled forward first to `20260525T0615CST-zone-range-magic`, then
> to `20260525T0630CST-zone-magic-mp-cooldown`; public health and WSS smoke
> `docs/generated/load/remote-zone-magic-mp-cooldown-wss-smoke-20260525.json`
> passed with `ready=1/1`, `capacityRejected=0`, `errors=0`, `messages=623`,
> and `ok=true`. Remaining backend risk: full skill authority is still not
> complete; Buff/stat effects, control effects, summons, AoE/ground spells,
> and complete Crystal drop/economy commit still need to move into Zone/world
> services.

> Latest blocked-source movement-transfer backend sync: 2026-05-25 fixed the
> server-side cause of the live Chrome Library-door transfer miss. The current
> production tab proved `Scout` could walk from `BichonProvince 322:248` to the
> direct Crystal movement source at `322:247`, but the live server corrected or
> stalled on map `0` instead of emitting `0104 Library`. Source now carries
> direct movement source cells in personal and Zone collision checks, allows
> only player movement to step onto those manifest-backed transfer cells, and
> keeps ordinary static collision, door collision, retained object occupancy,
> and spawn/join occupancy strict. The shared Zone loader now prefers full
> original Crystal map collision data for map `0` so Bichon transfers are
> validated against the same map family as the personal runtime. Evidence:
> focused Simulation
> `walk_onto_blocked_crystal_manifest_movement_source_transfers_map`, focused
> Gateway
> `shared_in_process_registry_walk_onto_library_movement_transfers_map`,
> existing walk-on transfer regressions, adjacent Simulation
> `crystal_manifest_movements`, Simulation/Gateway fmt check, and locked
> Simulation/Gateway check passed. Remaining backend risk: production still
> needs the Gateway binary rebuilt/restarted before the live Chrome tab can pass
> this exact route.

> Latest movement hot-path latency sync: 2026-05-24 removed the full-snapshot
> work from production movement command outcomes. `execute_with_outcome()` now
> skips `world_snapshot()` for KeepAlive, Turn, Walk, Run, and Tick outcomes,
> returning `snapshot_tick=0` for those low-latency paths instead of rebuilding
> the entire world snapshot after every movement ACK. Gateway runtime ticks are
> also deferred briefly after StartGame and player movement so the socket loop
> does not compete with immediate input handling. Evidence passed focused
> regressions
> `world_runtime_skips_snapshot_for_low_latency_movement_outcome`,
> `gateway_session_movement_event_skips_snapshot_tick`, and
> `runtime_tick_defers_after_bootstrap_and_player_movement`, plus
> `cargo +1.89.0 check --locked -p mir2-gateway`. Remote UCloud release
> `20260524Tmovelowlatency` was installed at `/opt/mir2/gateway/current`
> (`archive sha256 ac24c905823b029c9de5fa1030a4d88e81de811bc8d25ad209f273289bdc474a`,
> `binary sha256 bf114caeebeed6064e9af99f3fe69259df1ad713907fab014e08f81ffa201918`)
> and verified with public health plus WSS smoke. The normal production browser
> capture after the matching Web deploy,
> `docs/generated/player-qa/movement-jitter/prod-normal-directws-keyboard-d-20260524T1513.json`,
> connects to `wss://165.154.65.136.sslip.io/ws` and records six walk ACK
> frame latencies at `555/522/516/523/517/517ms`, with no rollback, critical
> console errors, or non-favicon 404s. Remaining backend risk: broader
> multi-player soaks still need separate pressure evidence, but the single-user
> movement hot path is no longer gated by full snapshot generation.

> Latest movement rollback correction sync: 2026-05-24 aligned the shared-Zone
> first-run behavior with the current Crystal action semantics and removed a
> remaining frontend/server prediction mismatch. Zone no longer hard-corrects a
> raw Run received from standstill; it consumes that action as an effective
> one-tile Walk, emits owner `UserLocation`, observer `ObjectWalk`, refreshes
> run grace, and records `SaveTransform`. Player Web no longer writes predicted
> self movement into authoritative `world.entities`, and it now waits for server
> acknowledgement when the map region is absent, the next tile is outside the
> loaded region, or the loaded map marks the tile blocked. Evidence: Web
> `pnpm --dir apps/web exec tsc --noEmit --pretty false` passed, scoped
> `git diff --check` passed, local movement smoke
> `docs/generated/player-qa/movement-jitter/local-left-walk-wait-map-20260523T233000.json`
> passed with zero logical rollback and zero scene blackouts, and focused
> shared-Zone standstill-run regressions passed. Player Web production
> deployment `dpl_3BwwKyjXY9UFZS3jSZk3vCsybCrW` is live through
> `https://mir2.obelisk.build`; production smoke
> `docs/generated/player-qa/movement-jitter/prod-left-walk-web-rollback-fix-20260524T0034.json`
> passed with `ok=true`, zero visual jumps, zero logical rollback, zero scene
> blackouts, no critical console errors, and no non-favicon 404s. Caveat: the
> final production sample still had scene asset readiness in `loading`, so this
> is movement rollback evidence rather than a full resource-readiness gate. The
> remote Gateway was then built on the UCloud host and installed as release
> `20260524T0310Z-rollbackfix` over `20260523T071900Z-actionqueue`
> (`archive sha256 ba7a0a5aeb1b98155e400a3626c78827c7c9fe6f0a5438eef60c0d53b3f0b693`,
> `binary sha256 64b87485315bfe4d846e974ab295e88c4611e05ac5de012183c7b1dc084004d5`).
> Post-release verification passed public origin health, `mir2-status`, WSS
> smoke `docs/generated/load/remote-rollbackfix-wss-smoke-20260524.json`
> (`ready=1/1`, `capacityRejected=0`, `errors=0`, `ok=true`), and production Web
> movement smoke
> `docs/generated/player-qa/movement-jitter/prod-left-walk-gateway-rollbackfix-20260524T0320.json`
> with zero visual jumps, zero logical rollback, zero scene blackouts, no
> critical console errors, and no non-favicon 404s.

> Latest shared-zone transform backend sync: 2026-05-22 fixed a production
> rollback source where `SharedInProcessZoneSessionRuntime` snapshot sync could
> upsert the personal `SimulationSession` self transform over the already
> accepted shared-Zone transform on the same map. Same-map shared upserts now
> preserve the authoritative Zone position/direction and only use incoming
> personal-session coordinates for new presence or map changes. Evidence:
> focused Gateway regressions
> `shared_zone_upsert_preserves_same_map_authoritative_transform` and
> `shared_in_process_registry_routes_walk_through_shared_zone` passed with
> `cargo +1.89.0 test`. Remote deployment: UCloud Gateway release
> `20260522T174413Z-zone-transform` was installed over
> `20260522T064157Z-walktransfer`; public `/health` and 1-client WSS smoke
> `docs/generated/load/remote-zone-transform-wss-smoke-20260522.json` passed.

> Latest Crystal movement-transfer backend sync: 2026-05-22 promoted imported
> Crystal movement rows from UI-assisted transfer options into server-side
> walk-on behavior. `SimulationSession` now checks the current player tile
> after successful Walk/Run movement and applies the matching Crystal
> `MapTransferRecord`; the shared in-process Zone Gateway path does the same
> after Zone accepts the movement and writes back the authoritative transform,
> then rejoins the correct map Zone. The production player command boundary
> still rejects debug `crystal:<map>:<x>:<y>` teleports; only manifest-backed
> direct movement keys are used. Evidence: focused Simulation
> `walk_onto_crystal_manifest_movement_transfers_map`, focused Gateway
> `shared_in_process_registry_walk_onto_crystal_movement_transfers_map`,
> adjacent Simulation `crystal_manifest_movements` 2/2, Gateway
> `shared_in_process_registry_same_map_transfer_syncs_zone_movement_origin`,
> Rust fmt check, and locked `cargo check -p mir2-simulation -p mir2-gateway`
> passed. The reachability audit at
> `docs/generated/map/latest-crystal-map-reachability.json` records 268/463 maps
> directly reachable from Bichon map `0` by direct Crystal movement rows and
> 185/284 positive-respawn maps in that direct graph; remaining maps need
> separate NPC/script/event/item/special route proof. Remote deployment:
> UCloud Gateway release `20260522T064157Z-walktransfer` was built on the host,
> installed over `20260521T0830Z-spreadrep`, and verified with local/public
> `/health`, `mir2-status`, and 1-client WSS smoke
> `docs/generated/load/remote-walktransfer-wss-smoke-20260522.json`
> (`ready=1/1`, `capacityRejected=0`, `errors=0`, `ok=true`).

> Latest original-map monster/NPC bootstrap backend sync: 2026-05-21 removed the
> starter fixture monsters from the production Crystal map bootstrap path.
> Gateway startup now uses `SimulationConfig::with_crystal_map_runtime()`, saved
> character map metadata is normalized from the Crystal respawn manifest, and
> `StartGame` rebuilds Crystal-map sessions from current-map original NPC and
> `MapRespawn` data instead of keeping the starter scene's `Training Dummy` /
> `Field Wasp` world. The starter/demo data remains available for its isolated
> vertical-slice tests, but original Bichon map `0` no longer emits those
> non-original monsters in the Crystal runtime path. The map gameplay audit now
> auto-detects the local full Crystal client root on this Mac and revalidated
> all maps: 463 maps, 6341 respawns, 6293 with walkable candidates, 48
> Crystal-inert no-candidate respawns, respawn failures 0, NPC failures 0,
> movement failures 0, static failures 0. Evidence: focused Simulation
> regressions for the Crystal bootstrap, saved non-default map roster bootstrap,
> and Royal Guard AI/route behavior
> passed, Simulation/Gateway `cargo +1.89.0 check` passed, Rust fmt check
> passed, and strict `audit:crystal-map-gameplay` passed with zero failures.

> Latest shared NPC/monster sprite backend sync: 2026-05-21 fixed a shared-Zone
> retained-object visual regression where packet-derived `ObjectNpc` and
> `ObjectMonster` entities were stored without sprite metadata, then later
> serialized back to Web clients as `image=0`. This could leave the NPC name,
> quest marker, and minimap dot visible while the Crystal body sprite vanished
> after a shared packet refresh. Gateway now converts packet images into retained
> `NPC/<image>` and `Monster/<image>` sprite snapshots, preserves an existing
> sprite across shared entity merges, and emits the retained image in shared
> spawn packets. Evidence: focused Gateway regressions
> `shared_zone_state_records_object_hero_and_npc_spawn_packets` and
> `shared_zone_state_records_object_monster_spawn_packet` passed. Remaining
> backend risk: Player Web now has a manifest fallback for the current live
> snapshot shape, but live production still needs the matching Gateway release
> rolled forward before the server itself stops emitting placeholder images.

> Latest Gateway scheduling/keepalive backend sync: 2026-05-19 deployed release `20260519T141920Z-fastka` after tracing the 30-client health failures to synchronous Gateway simulation work starving Tokio scheduling. Web session action/tick execution now uses blocking isolation, Gateway worker threads are configurable, route/session cache refresh is throttled per socket, idle ticks keep Redis route leases alive without per-packet writes, runtime tick cadence is env-tunable, and Web KeepAlive has a lightweight ACK fast path instead of driving a full Zone/runtime tick. Remote evidence `docs/generated/load/remote-fastka-30-soak5m-20260519.json` passed `ready=30/30 capacityRejected=0 errors=0 ok=true`, while `docs/generated/load/remote-fastka-30-soak5m-health-20260519.health.jsonl` had `30/30` successful 5s health probes with Redis record/lease counts reaching 30. Final live state keeps release `20260519T141920Z-fastka` but returns capacity to the safer `30/15/15` profile, with `MIR2_GATEWAY_TOKIO_WORKER_THREADS=8`, `MIR2_GATEWAY_ROUTE_REFRESH_INTERVAL_MS=5000`, and runtime tick restored to `300ms`. Remaining backend risk: this clears observability under 30-client entry pressure, but the current synthetic keepalive p95 remains high during the scripted post-StartGame burst, so 30 active is still not accepted as the normal gameplay-feel target.

> Latest Gateway health-soak backend sync: 2026-05-19 deployed release `20260519T124942Z-healthfast` after the first 30-client long soak proved connection stability but not health responsiveness. Redis session-cache health now scans keys once, fetches session records with one `MGET`, avoids a second route-lease scan, and runs the session-cache status work from the health handler on Tokio's blocking pool. Remote evidence: `docs/generated/load/remote-pgpool-30-soak20m-health-20260519.json` passed `ready=30/30 capacityRejected=0 errors=0 ok=true` over 20 minutes, but health sampling had 14/25 ten-second timeouts and keepalive p95 `651784 ms`; after `healthfast`, `docs/generated/load/remote-healthfast-30-soak5m-20260519.json` passed `ready=30/30 capacityRejected=0 errors=0 ok=true`, but health sampling still had 14/20 five-second timeouts and keepalive p95 `285956 ms`. Final live state is release `20260519T124942Z-healthfast` at safe capacity `30/15/15`. Remaining backend risk: 30 active is reachable, but not accepted as the normal internal-test cap until login/NewCharacter/StartGame pressure is moved off the Web runtime or tightly in-flight limited.

> Latest Gateway Postgres-pool backend sync: 2026-05-19 added an in-process Postgres account-store connection pool, one-time migration per pool, same-process source-write serialization, and account-scoped hot saves so login/character/save paths no longer open a fresh database connection and rewrite every account row under load. The UCloud 4H8G Gateway was deployed with release `20260519T105412Z-nogit`, pool size 8, 2s pool wait, and 3s connect timeout. Verification passed focused Postgres source-mode regressions, pool-config and shared-persist-lock regressions, Gateway save-queue tests, locked Simulation/Gateway check, and WSS remote load evidence `docs/generated/load/remote-pgpool-30-wss-pool30-timeout60-20260519.json` (`ready=30/30`, `capacityRejected=0`, `errors=0`, `ok=true`). Final live health after rollback was Redis healthy with capacity `30/15/15` and zero active sessions. Remaining backend risk: this is a short remote pressure gate, not a long 30-minute soak; keep 15 active players as the safe internal cap until longer health-responsive soak evidence exists.

> Latest new-account character-list backend sync: 2026-05-19 removed the leaked development default character from real new-account login paths. The `demo` account still keeps the `Scout` Warrior template for local smoke testing, but missing password accounts now fail login instead of auto-creating that template, and first-time Sui Passkey/Wallet accounts are created with an empty Crystal character list so the player must use the normal `NEW` character creation flow. Character create/save/delete refresh paths now avoid re-seeding the default character for non-demo missing accounts. Evidence: focused Simulation account lifecycle regressions passed for empty `NewAccount`, missing-account login, Passkey first login, duplicate account, and delete-last-character, plus `cargo +1.89.0 check --locked -p mir2-simulation -p mir2-gateway` passed.

> Latest original Bichon intro quest-chain backend sync: 2026-05-18 added an automated original-map Bichon q1-q4 live flow on top of the broader quest manifest work. The Simulation vertical slice now boots a player near Crystal `0` Assistant Jane/CraftLady/Merchant John, opens the original NPC dialogs, accepts/finishes q1, farms Scarecrow `GingerTea` Q drops for q2, finishes q2/q3, farms passive Deer through close melee plus Harvest for `DeerMeat` Q drops, finishes q4, and asserts q5 becomes available. Evidence: `cargo +1.89.0 fmt -p mir2-simulation`, focused `original_bichon_level_1_to_10_intro_quest_chain_uses_npc_scripts_and_q_drops`, full `cargo +1.89.0 test --locked -p mir2-simulation --test vertical_slice -- --test-threads=1 --nocapture` passed 6/6, `shared_zone` passed 77/77, `security_lifecycle` passed 9/9, and `cargo +1.89.0 check --locked -p mir2-simulation -p mir2-gateway` passed. Remaining backend risk: q5+ and later 1-45 quest bands are data/runtime-covered but still need representative live-client walkthroughs for dialog wording, route hints, and branch feel.

> Latest Zone-native monster combat/drop backend sync: 2026-05-18 moved explicit monster melee attacks onto the shared-Zone producer path, added the first Zone-native monster AI tick, and closed native monster-to-player HP write-back into the personal session. Gateway now seeds live personal-session map monsters into Zone during snapshot sync and routes explicit `WorldCommand::Attack` against shared monsters through `ZoneCommand::PlayerAttackObject`; Zone emits only the `ObjectAttack` launch immediately, then resolves the pending hit on `ZoneCommand::Tick` with `ObjectStruck` / `DamageIndicator` / `ObjectHealth` / `ObjectDied`, owner-window Zone drops, `GainExperience`, and `MonsterKillAward` side effects. Native Zone monsters choose nearby players, walk toward them with `ObjectWalk`, launch adjacent delayed melee hits, update Zone-held player HP, emit player `ObjectHealth`, and send `PlayerDamaged` so Gateway applies the same damage to the target `SimulationSession` `PlayerVitals`. Evidence: `cargo +1.89.0 test --locked -p mir2-simulation --test shared_zone -- --test-threads=1` passed 77/77, `cargo +1.89.0 test --locked -p mir2-gateway shared_in_process -- --test-threads=1` passed 40/40, `cargo +1.89.0 test --locked -p mir2-simulation --test security_lifecycle -- --nocapture` passed 9/9, focused native attack/delayed-hit/AI-tick/HP-writeback regressions passed, and `cargo +1.89.0 check --locked -p mir2-simulation -p mir2-gateway` passed. Remaining backend risk: native RangeAttack/Magic plus full Crystal drop-table exactness are still next before every combat path is native.

> Latest Postgres+Redis cutover backend sync: 2026-05-18 made the production-like Gateway fail closed on both authoritative account persistence and online routing cache. Production/staging envs, `MIR2_ACCOUNT_STORE_REQUIRE_POSTGRES=1`, or `MIR2_GATEWAY_REQUIRE_REDIS_CACHE=1` now require `MIR2_GATEWAY_REDIS_CACHE_URL`; when Redis is required the Gateway pings it during startup instead of silently falling back to in-memory route/session state. Local development can still use JSON plus in-memory cache, while staging/systemd examples now run with Postgres account source-of-truth and Redis route/session leases by default. Evidence: `cargo +1.89.0 test --locked -p mir2-gateway session_cache_environment -- --test-threads=1` passed 6/6, `cargo +1.89.0 test --locked -p mir2-gateway session_cache -- --test-threads=1` passed 20/20, `cargo +1.89.0 test --locked -p mir2-gateway health_reports_cache_and_gameplay_event_boundaries -- --test-threads=1` passed, `cargo +1.89.0 test --locked -p mir2-simulation account_store_environment -- --test-threads=1` passed, `cargo +1.89.0 check --locked -p mir2-gateway -p mir2-simulation` passed, `cargo +1.89.0 fmt --check -p mir2-gateway` passed, and scoped `git diff --check` passed. Remaining backend risk: inventory/mail/economy normalization and cross-Gateway owner handoff are still future production-hardening work; this slice removes the single-JSON/in-memory hot path from prod-like Gateway startup policy.

> Latest ranking-system backend sync: 2026-05-18 implemented the Crystal `GetRanking` / `Rankings` loop for the current server model. The Simulation packet path now rejects unauthenticated ranking requests, loads all account-store characters, overlays the active logged-in character so live level/experience is reflected, filters Overall or class-specific rank types 0-5, sorts by level descending then experience descending, computes the current character's `myRank`, and returns typed ranking listing details plus player-id listings/count. `OnlineOnly` is implemented conservatively for the current single-process online view: it returns the active character only unless a later shared roster provides more online identities. Gateway now routes Web `getRanking` commands into `ClientPacket::GetRanking` and preserves the typed `Rankings` event payload for Web. Evidence passed `cargo +1.89.0 fmt --check -p mir2-simulation -p mir2-gateway`, `cargo +1.89.0 check --locked -p mir2-simulation -p mir2-gateway`, `cargo +1.89.0 test --locked -p mir2-simulation --test ranking`, focused Gateway command/event tests, and Web `npx tsc --noEmit --pretty false`. Remaining backend risk: this is correct for persisted/dev account data and current online session scope; production-scale rankings still need real persistence/query indexing and a multi-session online roster.

> Latest shared Zone drop-claim backend sync: 2026-05-18 moved shared ground-drop claim/reserve/commit/cancel authority into the simulation Zone path. Gateway now seeds Zone ground drops from shared map snapshots, routes manual pickup and IntelligentCreature targeted/auto pickup through `ZoneCommand::ClaimGroundDrop` / `ClaimNearestGroundDrop`, removes successful claims from the Gateway map read model from Zone outbounds, and cancels/restores claims when personal inventory/gold award rules reject the pickup so filtered or full-bag drops do not get tombstoned. Zone gained nearest-drop claim selection by range/allowed ids/ownership, and the run continuation grace window is now 5s so valid second Run intents survive slow full-suite Gateway scheduling instead of downgrading into another Walk. Evidence: `cargo +1.89.0 test --locked -p mir2-simulation --test shared_zone -- --test-threads=1` passed 74/74 and `cargo +1.89.0 test --locked -p mir2-gateway shared_in_process -- --test-threads=1` passed 38/38. Remaining backend risk: the actual monster/drop generation and inventory mutation are still bridged from personal sessions; this slice makes shared drop pickup arbitration Zone-owned, not the full combat/drop economy source of truth.

> Latest vertical-slice backend sync: 2026-05-18 turned the four current main-experience targets into an automated Simulation acceptance suite. New `vertical_slice` coverage locks five-class creation/start-game state, per-class basic skill/combat loops, the Bichon starter Village Guide/Field Wasp quest-drop/reward chain, and shared multiplayer presence/movement/chat/drop-ownership stability. Evidence: `cargo +1.89.0 fmt -p mir2-simulation`, `cargo +1.89.0 test --locked -p mir2-simulation --test vertical_slice -- --nocapture` passed 4/4, `cargo +1.89.0 test --locked -p mir2-simulation --test shared_zone -- --test-threads=1` passed 74/74, and `cargo +1.89.0 test --locked -p mir2-simulation --test security_lifecycle -- --test-threads=1` passed 9/9. Remaining backend risk: these tests prove the current playable core is coherent, but they do not replace full Crystal packet/tree parity for every skill, NPC branch, map script, or a fully Zone-native monster AI/combat/drop source of truth.

> Latest Redis route-admission backend sync: 2026-05-18 promoted the Gateway session-cache route lease from passive online metadata into a StartGame admission guard. For authenticated Web `StartGame`, the Gateway now acquires the account/character route lease before executing the world entry path; a second socket, process, or Redis-backed Gateway that cannot obtain that lease is rejected before creating another online player. Failed StartGame/capacity/error paths release the pending lease, successful StartGame claims it through the normal session-cache refresh, and reconnect restore continues to renew the existing owner lease. Redis health now counts lease keys directly, so `/health.sessionCache.routeLeaseCount` reflects pending and active route locks instead of only records that already finished cache write-through. Evidence: `cargo +1.89.0 fmt --check -p mir2-gateway`, new StartGame route-admission regressions 2/2, session-cache route-lease regressions 2/2, Redis route-lease regression, production Web path safety tests 3/3, full session-cache focused suite 14/14, and health boundary regression. Remaining backend risk: Redis now prevents duplicate entry for the same account/character, but true cross-Gateway handoff still needs an owner RPC/shared Zone path for reconnect and Admin kick to close the owning socket rather than only mutate route metadata.

> Latest Gateway hot-path backend sync: 2026-05-18 moved Web Gateway character persistence off the per-action hot path and added separate pressure guards for account lifecycle work. Login, new-character, and StartGame now have independent optional in-flight caps via `MIR2_GATEWAY_MAX_LOGIN_IN_FLIGHT`, `MIR2_GATEWAY_MAX_NEW_CHARACTER_IN_FLIGHT`, and `MIR2_GATEWAY_MAX_START_GAME_IN_FLIGHT`, with `/health` reporting max/current counters beside the existing WebSocket/session/reconnect capacity state. Active character saves now use a dirty save queue with `MIR2_GATEWAY_SAVE_DEBOUNCE_MS` (default 1500 ms), `MIR2_GATEWAY_SAVE_CHECKPOINT_SECONDS` (default 15s), and `MIR2_GATEWAY_SAVE_QUEUE_LIMIT` (default 64) instead of saving after every non-low-latency Web action. Gameplay movement/chat stays low-latency, durable actions mark the session dirty, periodic checkpoints and queue pressure flush saves, and socket close still forces a final active-character save so movement-only sessions do not lose their last authoritative transform. Evidence: `cargo +1.89.0 fmt --check -p mir2-gateway`, focused Gateway capacity/action-inflight tests 3/3, save-queue tests 2/2, production Web path safety tests 3/3, health capacity test, reconnect store tests 2/2, `node --check apps/web/scripts/load-gateway-ws.mjs`, Web `npx tsc --noEmit --pretty false`, and live hot-path smoke `docs/generated/load/gateway-hotpath-codex-smoke.json` (`ready=4/4`, `capacityRejected=0`, `errors=0`, `ok=true`) with the temp Gateway stopped after verification. Remaining backend risk: the account store is still a JSON-backed development store, so true production scale still needs a database or actor-owned persistence layer plus deployment-specific soak.

> Latest Gateway capacity backend sync: 2026-05-18 added hard, observable web Gateway capacity guards for current process load. `MIR2_GATEWAY_MAX_WS_CONNECTIONS` caps concurrent WebSocket upgrades, `MIR2_GATEWAY_MAX_ACTIVE_SESSIONS` caps players that successfully enter game, and `MIR2_GATEWAY_MAX_RECONNECT_LEASES` caps retained reconnect-grace sessions; unset or non-positive values remain unlimited for dev. `/health` now reports max/current counts, active session permits transfer into reconnect leases and are released on restore/expiry, and reconnect lease pressure drops presence instead of exceeding the configured cap. The Web load harness now runs production-safe real login/new-character/StartGame/KeepAlive/Walk/Run/Chat flows by default, records expected capacity rejections, and exposes `npm run smoke:gateway-capacity`. Evidence: `cargo +1.89.0 fmt --check -p mir2-gateway`, Gateway capacity tests 2/2, reconnect store capacity-transfer tests 2/2, production Web path safety tests 3/3, health capacity test, `node --check apps/web/scripts/load-gateway-ws.mjs`, Web `npx tsc --noEmit --pretty false`, active-session cap live smoke `docs/generated/load/gateway-capacity-codex-active-smoke.json` (`ready=2/4`, `capacityRejected=2`, `errors=0`, `ok=true`), and WebSocket handshake cap live smoke `docs/generated/load/gateway-capacity-codex-ws-smoke.json` (`ready=2/4`, `capacityRejected=2`, `errors=0`, `ok=true`). Remaining backend risk: final production numbers still need deployment-specific CPU/RSS/network soak, and multi-process capacity requires an external owner/shared Zone coordinator.

> Latest reconnect grace backend sync: 2026-05-18 added a short Gateway reconnect grace lease for active WebSocket game sessions. When an in-game socket closes unexpectedly, the web Gateway now saves/refreshes the active character, refreshes the route lease with `MIR2_GATEWAY_RECONNECT_GRACE_SECONDS` (default 15s, clamped 1-120s), and retains the `GatewaySession` in memory by account/character instead of immediately dropping shared Zone presence. A new authenticated connection records the login account and, on `StartGame`, restores the retained session for the same character before replaying the bootstrap, keeping the player in flow while preserving the production command safety boundary. Evidence: `cargo +1.89.0 fmt --check -p mir2-gateway`, focused reconnect session store tests 2/2, reconnect key helper test, production Web path safety tests 3/3, existing route-lease stale-owner regression, Web `npx tsc --noEmit --pretty false`, `node --check apps/web/scripts/smoke-reconnect-resume.mjs`, and live `npm run smoke:reconnect-resume` against Web `127.0.0.1:13011` plus Gateway `127.0.0.1:7211` passed with `ok=true`. Remaining backend risk: this is single-process in-memory grace for the dev/current Gateway; cross-process reconnect handoff would need a durable session owner or shared Zone process.

> Latest original quest-chain backend sync: 2026-05-18 closed the automated backend loop for the original Crystal normal quest chain through level 45. The generated quest manifest now carries Crystal quest-text task semantics for carry items, kill tasks, item tasks, and flag tasks, and the runtime uses those definitions for Quest Diary availability, level/class/prerequisite-chain gates, accept state, carry-item grants, no-task immediate ready state, kill/item/flag progress, `ChangeQuest` task updates, NPC finish gating, quest-item cleanup, gold/exp/credit/fixed/select rewards, `CompleteQuest`, share packet handling, and loaded-object NPC quest links. Evidence: `cargo +1.89.0 check --locked -p mir2-game-data -p mir2-simulation`, `cargo +1.89.0 test --locked -p mir2-simulation original_crystal -- --test-threads=1`, focused `seed_state`, quest packet, Field Wasp, and Crystal quest-drop regressions, plus `node --check packages/tooling/scripts/generate-crystal-respawn-manifest.mjs` and Rust fmt checks. Remaining backend risk: this is automated state/data/progress/reward coverage; human acceptance still needs to walk representative NPC dialog wording, route guidance, and source-script branch feel in the live client.

> Latest continuous-run grace backend follow-up: 2026-05-15 tightened the shared Zone run-chain state for long held movement. Successful Zone movement now refreshes the run grace deadline after each accepted movement, so a long run sequence does not silently lose its run chain between ticks and force later valid Run intents back into walk-only correction behavior. Evidence: focused Simulation regression `continuous_run_extends_run_grace_after_successful_run` passed and `cargo +1.89.0 fmt --check -p mir2-simulation` passed. Frontend long-run acceptance is tracked separately in `docs/FRONTEND-1TO1-GAPS.md`.

> Latest shared object-action backend sync: 2026-05-14 replaced the Gateway same-map shared-entity action fanout with a Zone AOI path. `ZoneCommand::SyncSharedObjects` seeds shared Monster/NPC retained objects, and `ZoneCommand::BroadcastSharedObjectPackets` now handles monster/generated-object `ObjectAttack`, `ObjectRangeAttack`, `ObjectMagic`, `ObjectProjectile`, `ObjectStruck`, and matching result packets. Actor ids remain the shared object ids; only the current player's local self target/result ids are rebased to the Zone player id. Delivery now uses Zone retained-object visibility, so far same-map players no longer receive object actions they cannot see. Evidence: focused Simulation shared-object regressions passed 3/3, Simulation `shared_zone` passed 69/69, Gateway `shared_in_process` passed 35/35, and Simulation/Gateway fmt/check passed. Remaining backend risk: shared drop claim/award ownership and native monster combat/drop generation remain open.

> Latest retained object authority backend sync: 2026-05-14 hardened shared Zone retained objects after the vitals slice. Retained Buff state now stores full `AddBuff` payloads for non-player objects, including stat/value data and paused state, and replays those packets for late joiners and object-AOI entry. Dead/harvested retained objects now reject stale movement, mana, and positive-health updates, and retained health cannot increase from a stale personal-runtime packet until `ObjectRevived` explicitly resets that lifecycle. Retained NPCs plus live monster/hero/player objects now block Zone movement occupancy; dead, removed, drop, and decoration objects do not, and `ObjectRemove` releases the tile. Evidence: Simulation `shared_zone` passed 66/66, Gateway `shared_in_process` passed 35/35, and Simulation/Gateway fmt/check passed. Remaining backend risk: this makes more observable object facts shared-native, but combat/damage/drop/NPC side-effect production still needs migration out of personal runtime mirroring.

> Latest retained object-vitals backend sync: 2026-05-14 retained the latest `ObjectHealth` / `ObjectMana` packets for shared Zone objects. When a retained monster/generated object takes damage, Zone now keeps the latest health percent/expire payload, includes it for late joiners and players entering object AOI, preserves zero-health death semantics, and clears stale health on revive. MP-bearing retained heroes/generated objects likewise carry the latest mana percent to late joiners, with mana cleared on death/revive. Evidence: focused retained-object health/mana regressions passed 3/3, Simulation `shared_zone` passed 60/60, Gateway `shared_in_process` passed 35/35, and Simulation/Gateway fmt/check passed. Remaining backend risk: this retains observable vitals facts, but damage/mana calculation and ownership are still produced by personal runtime and need a later Zone-native combat migration.

> Latest retained harvest-corpse backend sync: 2026-05-14 added Zone-side harvested-corpse lifecycle state for retained shared objects. The shared `ZoneRuntime` now records non-player `ObjectHarvested` as a harvested/dead object fact, preserves the harvest anchor and direction for late joiners, suppresses duplicate harvest-complete packets, prevents stale live retained spawns from clearing the harvested/dead state, and ignores actor-local player `ObjectHarvested` movement when deciding corpse lifecycle. Evidence: focused harvested retained-object regressions passed 3/3, Simulation `shared_zone` passed 57/57, Gateway `shared_in_process` passed 35/35, and Simulation/Gateway fmt/check passed. Remaining backend risk: the harvest reward/drop transfer is still produced by personal runtime and mirrored through Zone; full Zone-native harvest/drop ownership remains deeper work.

> Latest Crystal action-queue backend sync: 2026-05-23 replaced the shared Zone
> `latest_intent` movement model with bounded ordered `ZoneMovementAction`
> queues and verified the pipeline in production. Walk/Run/Turn commands are
> queued per player, stale sequence numbers are ignored, overflow is corrected
> to owner `UserLocation`, ready actions are consumed in order on `tick`, Turn
> advances the 350ms Crystal delay, Walk/Run advance a single 600ms Crystal
> action window, and the later movement-rollback correction above changed raw
> Run from standstill to degrade into an effective one-tile Walk instead of
> hard-correcting the player at origin. Successful Walk/Run update occupancy and emit
> owner `UserLocation`, observer `ObjectWalk`/`ObjectRun`, and `SaveTransform`;
> blocked Walk preserves old direction while blocked Run after `CanRun` retains
> Crystal's direction update before correction. Evidence: Simulation
> `shared_zone` passed 78/78, focused Gateway Walk+Run chain and Turn routing
> regressions passed, Simulation/Gateway fmt-check passed, Web typecheck and
> production build passed, remote Gateway release
> `20260523T071900Z-actionqueue` is live, Player Web action-queue verification deployment
> `dpl_HmHQ4CXfy7d895kHFMfiNLHWespN`, custom-domain production `/health`, and production walk/run captures
> `docs/generated/player-qa/movement-jitter/prod-action-queue-keyboard-walk-fix2-20260523T1331.json`
> plus
> `docs/generated/player-qa/movement-jitter/prod-action-queue-keyboard-run-fix2-20260523T1332.json`
> are both `ok=true` with zero rollback, scene blackouts, critical console
> errors, and non-favicon 404s. Remaining backend work is broader native
> combat/drop/NPC authority, not proving the action-queue movement hot path.

> Latest delayed combat status-result backend sync: 2026-05-14 tightened the Gateway delayed player-action bundle filter. Delayed Tick packets anchored by a local-player `ObjectStruck` now also retain matching `ObjectPoisoned`, `AddBuff`, `RemoveBuff`, and `PauseBuff` result packets for the struck object or acting player, while unrelated tick results from other attackers remain filtered out. Evidence: focused delayed-player-action filter regression passed. Remaining backend risk: this still mirrors personal-runtime combat output; broader combat/drop ownership should move deeper into shared Zone state.

> Latest retained Zone object backend sync: 2026-05-14 promoted non-player spawn/drop/NPC packet surfaces into retained simulation Zone state. The shared `ZoneRuntime` now stores rebased monster, hero, NPC, item, gold, and decoration objects emitted through `BroadcastPackets`, applies later movement, death/revive, zero-health death, hidden/effect, poison, buff add/remove, name/colour, and NPC image updates to the retained spawn surface, expires retained visible object Buffs on Zone tick with observer `RemoveBuff`, tombstones retained objects on `ObjectRemove` / `IntelligentCreaturePickup`, performs retained-object AOI diffing on Join and movement, dispatches retained object spawn/update/remove packets by object visibility instead of actor visibility, removes owner-generated retained objects on owner Leave, expires retained item/gold drops on Zone tick using the Crystal ground-drop lifetime, canonicalizes or suppresses stale retained spawn/drop packets so old personal-runtime snapshots cannot resurrect dead/removed shared objects, and keeps `ObjectRevived` authoritative over stale dead retained spawns until a live spawn clears the marker. Evidence: focused retained-object regressions passed 16/16, Simulation `shared_zone` passed 55/55, Gateway `shared_in_process` passed 35/35, and Simulation/Gateway fmt/check passed. Remaining backend risk: combat resolution and drop awards are still only partly Zone-native; this slice gives Zone a real retained object read model for late entrants, movement AOI, object-centric AOI delivery including pickup removals, owner-generated cleanup, retained drop despawn, non-player Buff expiry, zero-health late-join death state, revived-state ordering, and out-of-order retained object lifecycle protection.

> Latest shared entity-action observer backend sync: 2026-05-14 broadened same-map observer delivery for shared non-player actors beyond movement. Gateway now recognizes shared monster/generated-object action origins in `ObjectAttack`, `ObjectRangeAttack`, `ObjectMagic`, `ObjectProjectile`, and attacker-anchored `ObjectStruck`, verifies the actor/source is present in shared map state, rewrites any current-player local self target reference to the authoritative Zone player object id, and queues those packets directly to same-map observers instead of passing them through the player-origin rebasing path. Same-batch `ObjectHealth`, `DamageIndicator`, `ObjectDied`, `ObjectPoisoned`, `AddBuff`, `RemoveBuff`, and `PauseBuff` for the current player are also rebased only when a shared actor struck that local self target, avoiding duplicate player-origin combat results. Evidence: focused shared entity movement/action regressions passed 2/2 and Gateway `shared_in_process` passed 35/35. Remaining backend risk: broader health/death/drop outcomes still need deeper shared-native authority rather than personal-runtime output mirroring.

> Latest shared entity-movement observer backend sync: 2026-05-14 added same-map observer delivery for shared monster/generated-object `ObjectTurn`, `ObjectWalk`, and `ObjectRun` packets that originate from personal-runtime Tick/object movement rather than player Zone movement. Gateway now filters those packets before any expensive snapshot read, verifies the object exists in the shared map, and queues the packet to other players on the same map while preserving the tight Run grace timing for player movement. Evidence: focused shared entity movement broadcast regression passed, focused Run timing regression passed, Gateway `shared_in_process` passed 34/34, Gateway `shared_zone_state_` passed 36/36, and Simulation/Gateway fmt/check passed. Remaining backend risk: the movement is still produced by personal runtime/monster AI; native Zone ownership of monster AI movement and combat/drop generation remains deeper work.

> Latest shared drop despawn-expiry backend sync: 2026-05-14 completed shared Gateway lifetime expiry for ground drops. Drops entering the shared map cache from personal snapshots, death-drop commits, or restore paths now get a Crystal-tick-derived despawn deadline. Tick/KeepAlive expires due shared drops, removes them from the shared map, records the remove tombstone, clears owner/despawn deadlines, returns `ObjectRemove` to the current session, and queues `ObjectRemove` to same-map observers so stale drops cannot remain visible or pickable forever. Evidence: focused shared drop expiry regressions passed 4/4, Gateway `shared_zone_state_` passed 36/36, Gateway `shared_in_process` passed 33/33, and Simulation/Gateway fmt/check passed. Remaining backend risk: actual monster/drop generation still needs deeper native Zone ownership.

> Latest shared drop ownership-expiry backend sync: 2026-05-14 added Gateway-side expiry for shared ground-drop owner windows. Shared drops merged from personal snapshots and one-time death-drop commits now get a local owner-window deadline from their Crystal remaining ticks; manual shared pickup and IntelligentCreature auto pickup clear expired ownership before applying owner/group checks. Evidence: focused manual/auto ownership-expiry regressions passed, Gateway `shared_zone_state_` passed 35/35, Gateway `shared_in_process` passed 32/32, and Simulation/Gateway fmt/check passed. Remaining backend risk: owner-window expiry is now shared-safe, while full shared drop despawn/expiry and native Zone-owned drop generation remain open.

> Latest shared object-movement cache backend sync: 2026-05-14 tightened ordinary object movement handling in the Gateway shared map read model. `ObjectTurn`, `ObjectWalk`, and `ObjectRun` now update shared entity position/direction through the same guarded transform path as push/backstep/dash packets, preventing moved monsters or generated objects from staying at stale coordinates in shared snapshots before the next full personal-session merge. Evidence: focused Gateway shared-zone-state movement regression passed. Remaining backend risk: this keeps shared cache coordinates fresher; native Zone ownership of monster AI movement/combat/drop generation remains open.

> Latest shared owned-generated lifecycle backend sync: 2026-05-14 tightened cleanup for player-owned generated shared entities. Gateway now records an owner name for summoned `ObjectMonster` rows by resolving `master_object_id` against online Zone players, the shared-state apply path rebases the local personal-session self id to the authoritative Zone object id before storing summon ownership, and stale ownerless snapshot merges preserve the existing owner. On player leave or map change, shared map state removes owned generated entities from the old map, marks them removed, clears stale lifecycle/drop anchors, and queues `ObjectRemove` to other players in the same map so observers do not keep seeing a disconnected or transferred player's hero/summon residue. Evidence: Gateway `shared_zone_state_` passed 33/33, Gateway `shared_in_process` passed 32/32, and focused shared runtime/state regressions proved observer `ObjectRemove` for owner-generated hero disconnect, local-master summon disconnect, owner-preserving snapshot merge, and owner map-change cleanup. Remaining backend risk: this cleans generated-entity lifecycle; native Zone ownership of combat/drop/NPC side-effect generation is still open.

> Latest shared intelligent-creature pickup backend sync: 2026-05-14 moved shared pet pickup one step past packet mirroring. When an intelligent creature tries to pick up a drop that exists in the shared Gateway map but not in the acting personal ECS, Gateway now falls back to shared target-location lookup, preserves Crystal fullness/mode/filter/grade gating through a Simulation-exported helper, applies the award through the personal inventory/gold state, removes the shared drop, and sends `IntelligentCreaturePickup` to AOI observers. Tick-driven auto pickup now does the same shared-map scan using Crystal range and ownership rules, and blocked filters restore the shared drop instead of deleting it. The manual fallback is keyed off empty command result packets so unrelated pending Zone packets no longer suppress the pickup. Evidence: focused Gateway intelligent-creature coverage passed 6/6, Simulation shared_zone passed 38/38, Gateway `shared_zone_state_` passed 29/29, Gateway `shared_in_process` passed 30/30, and Simulation/Gateway fmt/check passed. Remaining backend risk: pickup award still mutates the personal state layer; broader combat/drop/NPC side-effect generation still needs native shared Zone ownership.

> Latest shared spawn/skill-target backend sync: 2026-05-14 extended shared-Zone packet authority to generated objects and self-target references. Zone AOI fanout now covers `ObjectHero`, `ObjectMonster`, `ObjectNpc`, NPC update/image surfaces, and `IntelligentCreaturePickup`, including rebasing summoned-monster `master_object_id` to the authoritative Zone player id. Skill packets now also rebase owner-local target/destination references in range attacks, magic target/secondary target ids, projectiles, and self-target `ObjectStruck`. Gateway shared map state removes drops on `IntelligentCreaturePickup`, records spawned heroes/monsters/NPCs into the shared read model, keeps prior dead markers authoritative over late `ObjectMonster` packets, and applies live object transform packets to shared entities while ignoring transforms for dead objects. Evidence: focused Simulation shared-zone regressions for pet pickup, spawned monster, hero/NPC spawn, magic/projectile target rebasing, and action-packet rebasing passed; focused Gateway shared-zone-state regressions for pet pickup removal, monster spawn, dead-marker-late-monster, hero/NPC spawn, and object transform updates passed. Remaining backend risk: these are still bridged packet surfaces; full Zone-native combat/drop/NPC side-effect generation remains open.

> Latest shared dead-marker/backend sync: 2026-05-13 made shared monster lifecycle facts independent of snapshot arrival order. Gateway map state now stores a dead marker for `ObjectDied` and zero-health events, blocks actions against that object even before a shared entity row exists, applies the marker to later stale live snapshots using the death location/direction where available, and can commit death drops from an `ObjectDied` location without a prior entity snapshot. Out-of-order revive and harvest markers are covered too, so later stale dead/corpse snapshots cannot re-kill a revived object or reopen a harvested corpse. Evidence: Gateway `shared_zone_state_` passed 23/23. Remaining backend risk: this preserves lifecycle facts across shared snapshots; actual damage/drop generation is still not fully Zone-native.

> Latest shared delayed-damage/backend sync: 2026-05-13 added a bounded Gateway bridge for delayed player combat results produced on later world ticks. Tick packets are now filtered so only player-owned delayed damage bundles, anchored by `ObjectStruck.attacker_id == local_self_object_id`, are forwarded to Zone observer fanout with their matching health/death/remove/drop surfaces; unrelated monster AI Tick packets are not rebased as if they were player actions. Shared zero-percent `ObjectHealth` also marks entities dead even when max HP is absent. A stable shared-runtime pair regression now covers the full `Attack -> Tick -> observer drain` route for rebased delayed `ObjectStruck/ObjectHealth`. Evidence: focused delayed-damage filter regression passed, focused no-max-HP death regression passed, focused delayed two-runtime combat regression passed, Gateway `shared_zone_state_` passed 19/19, and Gateway `shared_in_process` passed 26/26. Remaining backend risk: full combat/drop generation is still not Zone-native.

> Latest shared transform-cache/backend sync: 2026-05-13 closed a Gateway-side authority lag after Zone movement. `ZoneOutbound::SaveTransform` now writes the shared `ZonePlayerPresence` position/direction immediately, including outbounds queued for non-current sessions, and shared `world_snapshot()` overlays the local `SelfPlayer` from Zone presence before returning cached/read-model state. Evidence: focused transform-cache regression passed, Gateway `shared_zone_state_` passed 18/18, and Gateway `shared_in_process` passed 25/25. Remaining backend risk: this fixes transform/cache freshness; full monster/combat/drop generation still needs deeper Zone-native authority.

> Latest shared viewport/transform backend sync: 2026-05-13 fixed shared-world state loss caused by treating a personal scene snapshot as a full map snapshot. Gateway `sync_map_layer` now merges visible entities/drops without deleting objects that merely left one session's viewport; explicit `ObjectRemove`, shared pickup, and duplicate death-drop guards drive removal. Death-drop duplicate suppression now uses retained death anchors instead of live entity rows, so stale duplicate drops stay blocked after corpse removal. Zone also now carries Crystal `TransformUpdate` through observer rebasing and late-join `ObjectPlayer.transform_type`. Evidence: Simulation `shared_zone` passed 35/35, Gateway `shared_zone_state_` passed 17/17, and Gateway `shared_in_process` passed 25/25. Remaining backend risk: these are shared-state correctness guards; full monster/drop generation is still not Zone-native.

> Latest shared revive-state/backend sync: 2026-05-13 added Gateway shared map handling for `ObjectRevived`. Revive now clears the shared dead flag, harvested-corpse marker, committed death-drop guard, and remove tombstone for that object, restores HP from max HP when available, and prevents stale dead personal snapshots from immediately re-deading the revived entity. Evidence: focused Gateway revive/remove-tombstone regressions passed, and Gateway `shared_zone_state_` passed 15/15. Remaining backend risk: broader monster lifecycle and respawn ownership are still not fully Zone-native, but revive no longer conflicts with the shared death/harvest/drop tombstones.

> Latest shared harvest-corpse/backend sync: 2026-05-13 added shared-state protection for Crystal harvest corpses. Gateway now records `ObjectHarvested` in the shared map layer, keeps that harvested-corpse tombstone across stale personal snapshot syncs, and rejects later shared `Harvest` attempts against the already-harvested corpse before a second personal runtime can award duplicate harvest results. Evidence: focused Gateway reharvest regression passed, and Gateway `shared_zone_state_` passed 13/13. Remaining backend risk: harvest drop preparation still runs in the acting personal runtime; this slice prevents cross-session duplicate corpse harvesting while the larger Zone/shared-native harvest/drop authority migration remains.

> Latest shared death-drop/backend sync: 2026-05-13 moved a concrete monster death/drop consistency edge into the shared Gateway map layer. When shared monster death is observed through either `ObjectDied` or zero-percent `ObjectHealth`, Gateway now commits nearby newly produced drops from the acting personal runtime once, records the dead monster id, and rejects later duplicate drops from stale personal-session snapshots. Evidence: focused Gateway death-drop commit/spawn regressions passed 3/3, and Gateway `shared_zone_state_` passed 12/12. Remaining backend risk: drop generation still originates in the personal runtime; this slice prevents duplicate/stale shared drops while the next step is deeper Zone/shared-native monster damage/drop authority.

> Latest shared late-join status/backend sync: 2026-05-13 retained common Crystal player visual status fields in Zone for later visibility. `ZonePlayer` now carries name colour, display name, guild name, light, weapon, weapon effect, armour, poison, wing effect, mount type/riding state, fishing state, and level effects, and `ObjectPlayer` packets for late joiners/new AOI observers expose those retained values instead of resetting to defaults. Evidence: focused late-join visual-status retention passed, and full Simulation `shared_zone` passed 35/35. Remaining backend risk: this covers retained player visual state; monster damage/death/drop ownership still needs the next Zone/shared authority migration.

> Latest shared late-status/backend sync: 2026-05-13 expanded shared Zone observer rebasing for player status, appearance, and late-system packets. `PlayerUpdate`, `DamageIndicator`, `ObjectColourChanged`, `ObjectGuildNameChanged`, `ObjectLeveled`, `ObjectName`, `MagicDelay`, `PauseBuff`, `MountUpdate`, `FishingUpdate`, `ObjectTeleportOut`, `ObjectTeleportIn`, and `ObjectDeco` now fan out through Zone AOI using the authoritative Zone player object id. Evidence: focused late-status observer regression passed, and full Simulation `shared_zone` passed 34/34. Remaining backend risk: these packets are correctly visible to observers, but several fields are not yet retained into late-join `ObjectPlayer` state, and monster damage/death/drop authority is still the next deeper migration.

> Latest shared teleport/poison backend sync: 2026-05-13 extended Zone action authority to `UserLocation` outputs from successful personal-runtime skill actions. Teleport/Blink-style packets now move the authoritative `ZonePlayer` before observer `ObjectEffect` fanout, so shared position, occupancy, and `SaveTransform` follow the skill outcome instead of remaining on the old tile. `ObjectPoisoned` also now rebases to the shared Zone player object id for observers. Evidence: focused `UserLocation` transform and poison observer regressions passed, and full Simulation `shared_zone` passed 33/33. Remaining backend risk: player transform and visible poison state are covered, but monster damage/death/drop source-of-truth still needs Zone-native migration.

> Latest shared skill-transform/backend sync: 2026-05-13 made shared Zone authoritative for movement-skill transform outcomes sourced from successful personal-runtime action packets. Zone now recognizes owner transform packets from BackStep/Dash/DashAttack/AttackMove/Pushed-style surfaces, applies the final position/direction to the shared `ZonePlayer`, updates occupancy, clears stale movement intents, emits `SaveTransform`, and rejects occupied/static destinations with an owner `UserLocation` correction without broadcasting the invalid movement to observers. Evidence: focused success/reject transform regressions passed, and full Simulation `shared_zone` passed 32/32. Remaining backend risk: the transform is now Zone-owned, but damage/death/drop resolution for the same combat actions still needs deeper Zone-native authority.

> Latest shared skill-movement/backend sync: 2026-05-13 expanded shared Zone observer coverage for Crystal movement-skill and special-skill packet surfaces. `ZoneRuntime` now rebases player-origin `ObjectBackStep`, `ObjectDash`, `ObjectDashFail`, `ObjectDashAttack`, `ObjectSitDown`, `SetConcentration`, `SetElemental`, `SetBindingShot`, `RemoveDelayedExplosion`, `ObjectSneaking`, and `ObjectLevelEffects` from the personal self object id to the authoritative Zone player id before AOI fanout. Evidence: focused movement-skill and special-skill observer regressions passed, and full Simulation `shared_zone` passed 30/30. Remaining backend risk: these packets are now multiplayer-visible with correct actor identity, but the deeper transform/damage/effect authority for skill outcomes is still not fully Zone-native.

> Latest shared harvest/backend sync: 2026-05-13 closed the next shared Zone packet-surface gap for harvest actions. The Gateway action path already forwards successful personal-session `Harvest` results to `ZoneRuntime`; Zone observer rebasing now includes `ObjectHarvest` and `ObjectHarvested`, replacing the local self object id with the authoritative Zone player object id and overriding the movement origin with the current Zone position/direction. Evidence: focused `player_harvest_packets_use_zone_object_id_for_observers` passed, and full Simulation `shared_zone` passed 28/28. Remaining backend risk: harvest resolution still originates in the personal session; the result is now visible and correctly identified for other players, but the deeper target is Zone-native monster/harvest/drop authority.

> Latest shared NPC/task/backend sync: 2026-05-13 tightened the shared Gateway semantics for task-facing NPCs, group-owned interactions, and stale monster health reconciliation. A sparse personal session can now directly `CallNpc @Main` on a Village Guide entity sourced only from the shared map snapshot and still get the quest side effect (`InProgress`) through the existing Crystal quest path. Gateway also relays `ShareQuest` packets to online group members through the shared in-process pending packet queue, shared drop owner windows now allow online group members of the owner instead of only exact owner-object-id pickup, shared `ObjectHealth` application keeps the lower HP value so stale personal-session damage packets cannot heal a shared monster, and combat/harvest packet execution now first applies shared monster snapshots into the acting personal runtime, including current-map batch application for direction-only attacks. Evidence: focused Gateway regressions passed, Gateway `shared_zone_state_` passed 9/9, Gateway `shared_in_process` passed 25/25, focused Simulation shared-monster snapshot application passed, and focused Gateway current-map shared-monster application passed. Remaining backend risk: these are still bridges around personal quest/combat execution; full monster damage/death/drop authority remains the next shared-Zone migration target.

> Latest shared drop ownership/backend sync: 2026-05-13 preserved modeled Crystal drop ownership across the shared Gateway map/drop layer. `GroundDropSnapshot` now includes owner object id and remaining ownership ticks, `collect_ground_drops` exports active `DropOwnership`, Gateway rebases owned drops from the personal self object id to the authoritative Zone player object id during shared sync, and shared pickup blocks non-owners during the owner window without removing the drop. Zone observer fanout also forwards `ObjectItem` / `ObjectGold` spawn packets to AOI observers. Evidence: focused Simulation shared Zone drop-spawn fanout passed, and Gateway `shared_zone_state_` passed 7/7. Remaining backend risk: actual monster death/drop resolution still originates in personal sessions, though its ownership metadata is now preserved in the shared layer.

> Latest shared player appearance/backend sync: 2026-05-13 retained additional player appearance state inside Zone. Rebased player-origin `ObjectHidden`, `ObjectHide`, `ObjectShow`, `ObjectDied`, `ObjectRevived`, and `ObjectEffect` packets now update the authoritative `ZonePlayer` hidden/dead/effect fields, and later `ObjectPlayer` visibility packets reflect that state for observers entering AOI after the original packet fanout. Evidence: `cargo +1.89.0 test --locked -p mir2-simulation --test shared_zone -- --test-threads=1` passed 25/25. Remaining backend risk: these are still appearance/lifecycle mirrors; the damage/death cause and broader combat effects are not yet fully Zone-owned.

> Latest shared Buff expiry/backend sync: 2026-05-13 added Zone-local expiry for active visible player Buffs. `ZoneCommand::BroadcastPackets` now includes the Zone timestamp used to translate Crystal relative `ClientBuff.expire_time` into an expiry deadline; `ZoneRuntime::tick` removes expired retained Buffs, sends observer `RemoveBuff`, and ensures later AOI entry no longer receives stale buff flags or details. Evidence: `cargo +1.89.0 test --locked -p mir2-simulation --test shared_zone -- --test-threads=1` passed 24/24. Remaining backend risk: actual Buff stat effects and skill resolution still live in personal-session systems until the next shared-authority migration.

> Latest shared Buff state/backend sync: 2026-05-13 added persistent active-player Buff state inside the shared Zone. When the personal skill runtime emits self-targeted `AddBuff` / `RemoveBuff`, `ZoneRuntime` records the rebased `ClientBuff` against the authoritative Zone player object id; future `ObjectPlayer` packets include the visible buff type list and send the corresponding rebased `AddBuff` payloads to late joiners or newly visible observers. Evidence: `cargo +1.89.0 test --locked -p mir2-simulation --test shared_zone -- --test-threads=1` passed 23/23. Remaining backend risk: Buff timing/expiry and full skill resolution are still not Zone-native authority.

> Latest shared skill/Buff backend sync: 2026-05-13 expanded shared Zone observer fanout for visible skill/Buff packet surfaces. `ZoneCommand::BroadcastPackets` now rebases the local self object id to the shared Zone player object id for `ObjectMana`, `AddBuff`, `RemoveBuff`, `ObjectEffect`, `ObjectSpell`, `ObjectPushed`, `ObjectRevived`, hide/show, `SpellToggle`, and `MapEffect` in addition to the previous attack/projectile/health/death set. Evidence: `cargo +1.89.0 test --locked -p mir2-simulation --test shared_zone -- --test-threads=1` passed 22/22. Remaining backend risk: these are still visual observer packets sourced from successful personal-session execution, not full Zone-owned skill/Buff authority.

> Latest shared NPC/backend sync: 2026-05-13 fixed the first NPC/task entry-point gap exposed by the shared Zone map layer. `ClientPacket::CallNpc` now opens the existing Crystal NPC script/dialog flow, and Gateway shared sessions can recover when a sparse personal `SimulationSession` sees an NPC supplied by the shared map snapshot but lacks the local ECS NPC entity: the runtime materializes the shared NPC snapshot locally and then runs the same `Interact` / `CallNpc` script path. Evidence: `cargo +1.89.0 test --locked -p mir2-simulation --test shared_zone -- --test-threads=1` passed 21/21 and `cargo +1.89.0 test --locked -p mir2-gateway shared_in_process_registry -- --test-threads=1` passed 20/20. Remaining backend risk: NPC/task script side effects are still personal-session execution with shared-view fallback, not full Zone-native quest/NPC authority.

> Latest shared-authority sync: 2026-05-13 added the first combat/skill packet fanout bridge and shared entity-state reconciliation on top of the shared Zone MVP. Gateway now captures successful personal-session Attack/RangeAttack/Harvest/Magic packet results and forwards them to `ZoneRuntime` as `BroadcastPackets`; Zone rewrites local self actor ids to the authoritative shared Zone player object id for `ObjectAttack`, `ObjectRangeAttack`, `ObjectMagic`, `ObjectProjectile`, and `ObjectStruck`, and broadcasts those plus `ObjectHealth`, `ObjectDied`, and `ObjectRemove` to AOI observers. The shared map layer now applies health/death/remove packets to shared monster snapshots, preserves lower HP/dead state across stale personal-session syncs, tombstones removed entities, and blocks follow-up attacks against shared dead/removed targets. Evidence: `cargo +1.89.0 test --locked -p mir2-simulation --test shared_zone -- --test-threads=1` passed 20/20, `cargo +1.89.0 test --locked -p mir2-gateway shared_zone_state_ -- --test-threads=1` passed 5/5, `cargo +1.89.0 test --locked -p mir2-gateway shared_in_process_registry -- --test-threads=1` passed 20/20, and Simulation/Gateway fmt/check passed. Remaining shared-authority risk: full monster damage/drop ownership is still reconciled from personal-session resolution instead of being natively owned by Zone.

> Latest chat/backend sync: 2026-05-13 completed the Crystal chat-channel parity slice on top of the shared Zone MVP. Protocol `Chat` now carries Crystal `LinkedItems`, supports `ChatItem` encode/decode, and covers `ChatType` values 0-16 including Trainer, System2, Relationship, Mentor, Shout2, Shout3, and LineMessage. Session chat preparation now preserves persisted chat-ban messaging, Crystal 2s/5-strike spam ban behavior, `@ADDSTORAGE`, case-insensitive first-match item link rewriting to `<Title/UniqueID>`, and `NewChatItem` payload dispatch for resolved inventory/storage/equipment/Hero items. Shared Zone chat now routes Crystal prefixes for normal AOI chat, whisper, group, guild, mentor, relationship, GM announcement, local shout, map shout, and server shout, including `$pos`, shout level gate, 10s shout cooldown, one-shot shout-scroll consumption back into the personal session, and `ToAll` fanout for server shout through Gateway pending queues. Web log channels now render Mentor/Relationship channels and filter them through the original chat tabs. Evidence: Protocol `chat_` 2/2, Simulation `shared_zone` 18/18, Simulation `chat_` 46 lib + 2 shared-zone filtered tests, Gateway `chat_` 3/3, locked Protocol/Simulation/Gateway check, Rust fmt check, and Web `npm exec tsc -- --noEmit` passed.

> Latest architecture/backend sync: 2026-05-13 completed the Gateway side, production WebSocket command boundary, and live two-client smoke of the shared multiplayer Zone MVP. The shared in-process Gateway runtime now uses the simulation `ZoneManager` as the authoritative online world for StartGame join, Walk, Run, Turn, Chat, Tick/KeepAlive movement consumption, and LogOut/Disconnect leave. Zone outbounds are either returned to the active session or queued for other online Gateway sessions, and `SaveTransform` is applied back to the personal `InProcessWorldRuntime` with a runtime tick refresh so saves and session-cache snapshots follow authoritative Zone movement. The normal WebSocket path can enforce production player-command safety through production-like envs or `MIR2_GATEWAY_ENFORCE_PLAYER_COMMAND_SAFETY`, rejecting unauthenticated StartGame and blocking normal-client debug `MoveTo`, `Stage5Command`, and `crystal:<map>:<x>:<y>` transfer commands while preserving HMAC-verified passkey login. Existing shared drop/gold pickup, Trade escrow, and ItemRental behavior remains green. Evidence: Gateway lib passed 121/121, Gateway `shared_in_process_registry` passed 20/20 including ObjectWalk/ObjectRun/ObjectTurn/ObjectChat/ObjectRemove route regressions, production WebSocket safety passed 3/3, Simulation shared Zone passed 12/12, Simulation security lifecycle passed 9/9, locked Simulation/Gateway fmt/check passed, Gateway health was ready on `127.0.0.1:7210`, WebSocket two-client smoke passed with 2/2 ready clients and 0 errors, the ad hoc browser two-client Zone smoke passed, and the committed `npm run smoke:two-client-zone` harness passed at `docs/generated/player-qa/two-client-zone/two-client-zone-script-135930.json` with mutual player visibility, movement broadcast, chat broadcast, no console errors, and no non-favicon 404s. Remaining backend integration risk in this track is broader human acceptance and deeper non-movement gameplay migration into shared authority, not the core Gateway-to-Zone, WebSocket safety, repeatable smoke harness, or two-client visible-presence path.

> Latest architecture/backend sync: 2026-05-12 started the shared multiplayer Zone MVP foundation. The simulation crate exposed a synchronous single-writer `ZoneRuntime` / `ZoneManager` with `ZoneKey`, `SessionId`, `PlayerId`, `ZoneJoin`, `ZoneCommand`, `MoveIntent`, and `ZoneOutbound`. The MVP covered Join/Leave, rectangular AOI, unique player object ids, occupancy collision, map static collision wrapper, latest-intent-only Walk/Run, intermediate-tile Run validation, Turn, Chat packet surfaces, `SaveTransform`, and `SimulationSession` authority write-back through `active_zone_join_snapshot` / `force_authoritative_player_transform`; the early standstill-Run-as-Walk behavior is now the current 2026-05-23 rollback-correction semantic again, implemented on top of the later ordered action queue rather than the original latest-intent shortcut. Production-player command validation now rejects unauthenticated StartGame/NewCharacter/DeleteCharacter plus raw `PasskeyLogin`, debug `MoveTo`, `Stage5Command`, and `crystal:<map>:<x>:<y>` transfer keys. Evidence: `cargo +1.89.0 test --locked -p mir2-simulation --test shared_zone -- --test-threads=1` passed 12/12, `cargo +1.89.0 test --locked -p mir2-simulation --test security_lifecycle -- --test-threads=1` passed 9/9, `cargo +1.89.0 fmt --check -p mir2-simulation` passed, and `cargo +1.89.0 check --locked -p mir2-simulation` passed. Remaining work: wire Gateway production routing so StartGame joins Zone and Walk/Run/Turn/Chat are served by the shared Zone rather than the existing personal-session movement path.

> Latest backend worker sync: 2026-05-11 completed a bounded Hero book/stat requirement exactness slice. Crystal `HeroObject.UseItem` validates the Hero inventory item with `CanUseItem` before handling `ItemType.Book`, and `HumanObject.CanUseItem` applies gender, class, level/max-level, full stat `RequiredType` gates, and duplicate-book rejection before `Info.Magics.Add(new UserMagic(...))` sends `NewMagic(hero=true)`. The Rust Hero book-learning helper now applies the same requirement family using current non-broken equippable HeroInventory stat totals, so a Hero book with an unmet stat requirement is rejected without adding learned magic. Evidence: focused stat-book regression 1/1, focused `hero_inventory` 16/16 plus book/key integration 1/1, Hero AI integration 28/28, Simulation fmt check, and locked Simulation check passed.

> Latest backend worker sync: 2026-05-11 completed a bounded late social/economy Friend/blacklist state slice. Crystal `PlayerObject.AddFriend` uses one `FriendInfo` list for friends and blocked entries; duplicate detection checks that shared list, so adding an already-friended player to blacklist, or an already-blocked player to friends, rejects without mutation. Stage 5 high-level social commands now preserve that rule, reject self-targets through Crystal `server.CannotAddYourself`, and keep the modeled state persistent across reload. Evidence: focused `social_economy_integration` 3/3, adjacent `stage5_social_group_guild_mail_persist_across_reload`, adjacent `mail_friend_packets_preserve_crystal_ack_surface`, Simulation fmt check, and locked Simulation check passed.

> Latest coordinator sync: 2026-05-11 completed the multi-agent backend pass covering Hero progression, Crystal movement-skill practice gates, and Mail exact parcel claim atomicity. Hero learned magic now updates persisted Stage 5 level/experience from successful keyed Hero AI casts and emits the Crystal `MagicLeveled` / level-up `MagicDelay` surfaces. Player `BackStep`, `ShoulderDash`, and `FlashDash` now bypass generic progression and call practice progression only on Crystal success conditions: moved, dashed, or hit target respectively. Stage 5 `mail.claim` now preflights serialized attachment slots as a batch and consumes exact parcel payload only on successful claim. Evidence: focused Hero progression 2/2, Hero AI 28/28, `magic_packet_crystal_` 73/73, Mail 9 unit + 2 integration tests, `cargo +1.89.0 fmt --check -p mir2-simulation`, `cargo +1.89.0 check --locked -p mir2-simulation`, and targeted diff checks passed. Remaining backend depth is wider Hero book/stat requirement exactness, Crystal skill-gain multiplier/mentor tuning, and broader late social/economy packet-perfect semantics.

> Latest sync: 2026-05-11 completed a bounded Hero magic level/experience progression slice. Crystal source evidence is `UserMagic.Level/Key/Experience` plus `UserMagic.GetInfo(hero)` in `MagicInfo.cs`, `HeroObject.UseItem` / `HeroObject.CanUseMagic` in `HeroObject.cs`, Hero class `CanUseMagic` spell selection, and `HumanObject.LevelMagic` adding 1..3 practice experience, carrying `Need1/Need2/Need3` threshold overflow, sending `MagicLeveled`, and sending `MagicDelay` on level-up. Runtime `heroLearnedMagics` now advances experience when Hero AI successfully uses a keyed learned spell, emits the Crystal-shaped progression packets, persists the updated level/experience through the existing Stage 5 save path, and subsequent Hero AI casts use the progressed learned level for gates, damage, and cooldown choice. Evidence: focused Hero progression 2/2, full Hero AI integration 28/28, focused `hero_inventory` 15 lib tests plus the Hero book/key integration filter 1/1, `cargo +1.89.0 fmt --check -p mir2-simulation`, and `cargo +1.89.0 check --locked -p mir2-simulation` passed. Remaining Hero risk is broader book/stat requirement exactness plus Crystal mentor/stat skill-gain multiplier tuning beyond the bounded deterministic 1..3 practice model.

> Latest coordinator verification: 2026-05-11 reverified the current backend parity head after the Hero learned-magic and Guild alliance runtime-persistence changes. Evidence now includes locked GameData/Protocol/Simulation/Gateway check, `cargo +1.89.0 fmt --check -p mir2-simulation -p mir2-gateway`, full locked `mir2-simulation` 856/856 plus Hero AI 26/26, focused Hero AI 26/26, focused `guild_` 16/16, and Gateway `shared_in_process_registry` 15/15. No new backend failures were found during the frontend movement-feel rollback fix; after the later bounded Hero progression sync above, remaining backend depth is wider Hero book/stat requirements, exact skill-gain tuning, and any future source-backed Guild alliance dialog surface.

> Latest sync: 2026-05-11 completed the Hero learned-magic book/key/save closure. The backend now creates Hero learned magic state through the existing HeroInventory `UseItem` packet when a Hero uses a valid Crystal book, returns `NewMagic(hero=true)`, stores the learned spell at level 0/key 0, and consumes the book. `MagicKey` with `key > 16` or `oldKey > 16` now updates the Hero learned-magic key instead of being discarded, preserving same-key clearing and saving the result in `stage5_systems_json`. Evidence: focused Hero loop 1/1, Hero AI 26/26, focused `hero_inventory` 15 lib tests plus integration filter, `cargo +1.89.0 fmt --check -p mir2-simulation`, and `cargo +1.89.0 check --locked -p mir2-simulation` passed.

> Latest sync: 2026-05-11 completed a Guild alliance deep-surface follow-up. Crystal source confirms `GuildObject.AllyGuilds` / `AllyCount` and `GuildRankOptions.CanAlterAlliance` exist, but `GuildDialog` only requests notice/member data through `RequestGuildInfo` type 0/1, the client/server packet enums have no alliance packet, and `GuildInfo.Save/Load` does not persist alliance fields. Stage 5 now matches that persistence boundary by treating alliance list/count/broadcasts as runtime-only on reload while preserving current in-session readback. Evidence: focused `guild_` 16/16, the new alliance save/reload regression, `cargo +1.89.0 fmt --check -p mir2-simulation`, and `cargo +1.89.0 check --locked -p mir2-simulation` passed.

> Latest sync: 2026-05-10 completed the Hero learned-magic and Guild alliance visible-info follow-up. Stage 5 now persists optional `heroLearnedMagics` rows and Hero AI uses them like Crystal `HeroObject.GetMagic` / `CanUseMagic`: learned inventory is authoritative when present, `key > 0` is required, and learned level caps the manifest-derived Crystal level gate, while empty learned state remains backwards-compatible. Guild alliance state is now visible through `RequestGuildInfo` type 0/1 without inventing a non-Crystal packet: notice/member packets are preserved and guild chat lines expose ally count, ally names, and recent alliance broadcasts. Evidence: focused `guild_` 15/15, Hero AI 25/25, full locked `mir2-simulation` 855/855 plus Hero AI 25/25, Gateway `shared_in_process_registry` 15/15, Simulation/Gateway fmt, locked GameData/Protocol/Simulation/Gateway check, Web typecheck, NPC/movement script syntax, NPC marker evidence parse, and targeted diff checks passed.

> Latest sync: 2026-05-10 completed the Guild alliance and Wizard Hero late-priority follow-up. Guild Stage 5 now carries Crystal-style alliance state (`allied_guilds`, `ally_count`, and alliance broadcasts), accepts `guild.ally` / `guild.unally` command surfaces, maps `CanAlterAlliance` through the existing guild-rank option bit, and rejects missing, self, permission, and active-war targets without mutation. Wizard Hero AI now implements the late `ProcessAttack` single-target chain after the prior area-priority slice: `TurnUndead`, `FlameDisruptor`, `Vampirism`, `FrostCrunch`, then classic single-target fallbacks, with manifest gates, Hero MP spend, `ObjectMana`, `ObjectMagic`, cooldowns, delayed damage, Vampirism Hero `ObjectHealth`, and FrostCrunch freeze/root `AddBuff` evidence. Evidence: `guild_` 14/14, Hero AI 23/23, full locked `mir2-simulation` 854/854 plus Hero AI 23/23, Gateway `shared_in_process_registry` 15/15, Simulation/Gateway fmt, locked GameData/Protocol/Simulation/Gateway check, Web typecheck, movement script syntax, blocked-target evidence parse, and targeted diff checks passed.

> Latest sync: 2026-05-10 completed the Guild war lifecycle and Wizard Hero attack-priority follow-up. Guild war state now includes Crystal-style duration ticks, active-war expiry, `WarEndedWithGuild` guild chat surfaces, colour-change packets for war start/end, Newbie-guild rejection before registry lookup, and focused rollback/state tests. Wizard Hero AI now implements the next `ProcessAttack` priority slice from Crystal `WizardHero.cs`: adjacent lower-level `Repulsion`, close self-surrounded `FlameField` / `ThunderStorm`, crowded target `IceStorm` / `FireBang`, and the existing ThunderBolt/GreatFireBall/FireBall fallback, with Crystal manifest gates, MP spend, `ObjectMana`, `ObjectMagic`, `ObjectPushed`, cooldowns, and scheduled area damage. Evidence: `guild_` 12/12, Hero AI 20/20, full locked `mir2-simulation` 852/852 plus Hero AI 20/20, Gateway `shared_in_process_registry` 15/15, Simulation/Gateway fmt, locked GameData/Protocol/Simulation/Gateway check, Web typecheck, movement script syntax, live route-spam settle capture, and diff checks passed.

> Latest sync: 2026-05-10 completed the Guild war/territory and Wizard Hero support parity slice and reverified it with the current route-spam frontend bridge. Guild state now tracks known guilds, active wars, war broadcasts, and richer territory listing fields. `GuildRequestWar` / `GuildWarReturn` now cover Crystal prompt behavior, leader/no-guild silence, missing/self/Newbie/already-war/funds failures, war-bank cost deduction, `GuildStorageGoldChange` type 2, and rollback for duplicate/insufficient states. Guild territory page/purchase packets now expose `ClientGtMap` listings and purchase through guild-bank gold with Crystal-shaped failure/success surfaces. Wizard Hero AI now performs `ProcessFriend` support before hostile spell attacks, casting `MagicShield` before `MagicBooster` with Crystal level gates, Hero MP spend, `ObjectMana`, self-target `ObjectMagic`, `AddBuff`, cooldown, active-buff gating, and low-mana no-recast coverage. Evidence: focused `guild_` 10/10, Hero AI 17/17, full locked `mir2-simulation` 850/850 plus Hero AI 17/17, Gateway `shared_in_process_registry` 15/15, Simulation/Gateway fmt, locked GameData/Protocol/Simulation/Gateway check, Web typecheck, movement script syntax, live route-spam obstacle evidence, and targeted diff checks passed.

> Latest sync: 2026-05-10 completed the next Guild/Hero worker slice and reverified it with the current frontend-feel bridge. Guild storage and rank handling now follow more of Crystal's real server rules: `GuildRankOptions` permission bits are derived from the Stage 5 guild rank/permission model, notice edits require `CanChangeNotice`, guild storage item store/retrieve/move require the matching storage permissions and safe-zone access, guild-gold withdrawal requires leader rank, `DontStore` and rental `DontStore` items are rejected, and stored guild items preserve exact serialized `ItemState` plus storing user id through list/retrieve. Wizard Hero AI now casts level-gated `FireBall` / `GreatFireBall` / `ThunderBolt` at range, spends Hero MP, emits `ObjectMana` and Hero `ObjectMagic`, respects cooldown, and applies delayed monster damage. Evidence: focused `guild_` 5/5, `trade_` 12/12, Hero AI 13/13, full locked `mir2-simulation` 845/845 plus Hero AI 13/13, Simulation/Gateway fmt, locked GameData/Protocol/Simulation/Gateway check, Web typecheck, movement harness syntax check, and targeted diff checks passed.

> Latest sync: 2026-05-10 reconciled the current multi-worker backend/frontend-feel round. TradeEscrow, Taoist Hero owner healing, and the client action-feel bridge were reverified together with focused Simulation `trade_` 12/12, Gateway `shared_in_process_registry_` 15/15, Hero AI integration 11/11, full locked `mir2-simulation` 843/843 plus Hero AI 11/11, Simulation/Gateway fmt, locked four-package GameData/Protocol/Simulation/Gateway check, Web typecheck, movement harness syntax check, and targeted diff check.

> Latest sync: 2026-05-10 completed Worker TradeEscrow's backend Trade escrow slice. Simulation now prevents Crystal `DontTrade`, soulbound, rental-bound, rental-owned, rental-expiring, rental-locked, and invalid offered items from entering or confirming Trade escrow; shared in-process Gateway tracks free bag capacity for online presences and preflights both sides before applying paired delivery; full-bag settlement failures roll both confirmed offers back instead of partially delivering; partner cancel/disconnect restores locked gold/items; and successful two-account confirms still deliver real gold/items with the existing `TradeConfirm`, `GainedGold`, and `GainedItem` packets. Evidence: `cargo +1.89.0 test --locked -p mir2-simulation trade_ -- --test-threads=1` passed 12/12, `cargo +1.89.0 test --locked -p mir2-gateway shared_in_process_registry_ -- --test-threads=1` passed 15/15, `cargo +1.89.0 fmt --check -p mir2-simulation -p mir2-gateway` passed, and `cargo +1.89.0 check --locked -p mir2-simulation -p mir2-gateway` passed.

> Latest sync: 2026-05-10 added Worker HeroClassBreadth's bounded Taoist Hero Healing semantics. Server-side Hero AI now runs a Taoist support priority before hostile attack selection, uses Crystal manifest level gates for `Healing`, consumes Hero MP and emits `ObjectMana`, publishes Hero `ObjectMagic(Healing)`, heals the owner with `ObjectHealth`, and stores a private heal cooldown. Evidence: `cargo +1.89.0 test --locked -p mir2-simulation --test hero_ai -- --test-threads=1` passed 11/11, including owner-heal priority and below-level-gate regressions; `cargo +1.89.0 check --locked -p mir2-simulation` passed, and the later coordinator Simulation/Gateway fmt plus full Simulation regression pass also passed.

> Latest sync: 2026-05-10 closed the current coordinator/worker backend slice and added Crystal packet action-timing gates. The packet runtime now tracks per-player Crystal action readiness for `Walk`, `Run`, `Attack`, `RangeAttack`, and `Magic`; too-fast repeated packets are corrected with `UserLocation` and do not emit duplicate action surfaces, matching the Crystal server's action-delay discipline at the packet boundary. This sync also reconciles the bounded Archer Hero AI and Mail-Parcel worker outputs: Archer Heroes use Crystal level gates for `Concentration` / `StraightShot`, spend Hero MP, emit `ObjectMana`, gate `SetConcentration`, and tag ranged `StraightShot` attacks; player mail parcels preserve exact serialized item attachments, opened/locked flags, remote account-store delivery, and exact `GainedItem` / `ParcelCollected` claim behavior. Evidence: action-timing regressions passed, `magic_packet_crystal_` 73/73, `packet_` 280/280, Hero AI integration 9/9, Mail regressions 9/9, full locked `mir2-simulation` 841/841 plus Hero AI 9/9, package `cargo +1.89.0 fmt --check -p mir2-simulation`, locked four-package check for GameData/Protocol/Simulation/Gateway, and targeted diff checks all passed.

> Latest sync: 2026-05-10 completed Worker Mail-Parcel's bounded mail parcel fidelity slice. Player `SendMail` now resolves and serializes attached inventory `ItemState`s from protocol unique IDs, validates target/item/cost before mutation, preserves sender-side blacklist priority, removes sender gold/items on success, stores remote recipient mail in account-store Stage 5 systems, exposes `ClientMail.items` plus opened/locked flags, and claim returns exact serialized items with `GainedItem` / `ParcelCollected`. Evidence: focused `mail_` simulation tests 9/9, `cargo +1.89.0 check --locked -p mir2-simulation`, and coordinator-reconciled package `cargo +1.89.0 fmt --check -p mir2-simulation` passed.

> Latest sync: 2026-05-10 added Worker Hero-Class-AI's bounded Archer Hero semantics. `apps/simulation/src/runtime/hero_ai.rs` now carries private Hero AI skill state for Archer `Concentration` / `StraightShot`, derives Crystal magic levels from the generated magic manifest, spends Hero MP through Hero `PlayerVitals` with `ObjectMana`, emits `SetConcentration` only while the modeled buff window is not already active, and tags ranged `ObjectRangeAttack` packets with `StraightShot` plus Crystal damage scaling. Evidence: Hero AI integration 9/9, locked `cargo +1.89.0 check --locked -p mir2-simulation`, and coordinator-reconciled package `cargo +1.89.0 fmt --check -p mir2-simulation` passed.

> Latest sync: 2026-05-10 completed Worker Agility's backend data/runtime slice. The Crystal respawn generator now imports monster `Agility` from stat 11, game-data models it on monster and respawn templates with default-compatible deserialization, and Simulation attaches `MonsterCombatStats.agility` from imported Crystal templates across spawn-table rebuilds, visible current-map imports, respawn revival, and dynamic Crystal monster spawns. The focused production-spawn regression proves a nonzero-agility Crystal monster receives `MonsterCombatStats` and participates in the modeled passive accuracy miss/hit roll. Evidence: `magic_packet_crystal_imported_agility_drives_melee_hit_roll` 1/1, `crystal_monster_manifest_loads` 1/1, `crystal_respawn_manifest_loads` 1/1, `node --check packages/tooling/scripts/generate-crystal-respawn-manifest.mjs`, `cargo +1.89.0 fmt --check -p mir2-game-data -p mir2-simulation`, and `cargo +1.89.0 check --locked -p mir2-game-data -p mir2-simulation` all passed. Current checked-in manifests were not regenerated because this Mac workspace lacks `Crystal/Build/Server/Debug/Server.MirDB`.

> Latest sync: 2026-05-10 added the next bounded Hero skill/combat semantics after carried-equipment projection. `apps/simulation/src/runtime/hero_ai.rs` now derives modeled Warrior Hero `Slaying` and `FlamingSword` availability from Crystal magic level gates, carries the spell/level on Hero melee `ObjectAttack`, adds Slaying's passive DC bonus, and applies FlamingSword burst scaling to the scheduled monster hit while preserving existing Hero equipment DC and Archer range behavior. Regression evidence: Hero AI integration 7/7, `cargo +1.89.0 fmt --check -p mir2-simulation`, and `cargo +1.89.0 check --locked -p mir2-simulation` pass. Remaining backend Hero depth is real Hero magic inventory/learning state, MP/cooldown lifecycle, and broader class-specific Hero skills.

> Latest sync: 2026-05-10 closed the previously named passive-hit, Hero-equipment, Fishing-slot, and Market-settlement backend gaps with GPT-5.5 xhigh workers plus local reconciliation. Player melee now follows Crystal passive accuracy totals for `Fencing` (`level * 3`), `Slaying` (`level`), and `SpiritSword` (`0/3/5/8`) together with equipment `Accuracy`; modeled monsters can carry `Agility`, player attacks roll Crystal-style miss/hit outcomes, misses emit `DamageIndicator { damage: 0, damage_type: 1 }`, successful melee hits advance `Fencing` / `SpiritSword`, and `MPEater` now uses Crystal's accuracy-derived count and MP recovery formula. Hero AI now projects carried equippable weapon stats into Hero attack damage and projects carried Hero equipment/stat bonuses into `HeroInformation` / `ChangeHero`. Fishing now prefers actual fishing slot items for bait, hook, float, finder, and reel behavior, including slot durability damage and broken-reel autocast cancellation. Market buy/get-back now rejects underbids, records accepted bid gross value, and pays seller net proceeds after the modeled 5% commission with `SoldItemEarningsCommission`. Regression evidence: focused passive accuracy 1/1, `magic_packet_crystal_` 73/73, Fishing 11/11, Market 1/1, Auction 6/6, Hero AI integration 5/5, full locked `mir2-simulation` 836/836 plus Hero AI 5/5, `mir2-simulation` fmt, locked GameData/Protocol/Simulation/Gateway check, and targeted diff checks pass. Remaining backend depth is broad Crystal Agility import for all relevant monsters, fuller Hero equipment/skill semantics, deeper guild and market multi-account/mail settlement, and final client visual/effect acceptance.

> Latest sync: 2026-05-10 continued the multi-agent skill-system and late-game pass with GPT-5.5 xhigh workers and local reconciliation. The generated Crystal magic manifest now has explicit runtime handling for every non-`None` spell (`unmatched manifest spells: 0` in the local scan). Player melee now supports Crystal-shaped `Thrusting` second-tile attacks, `FlamingSword` / `Slaying` spell-tagged attacks, incoming-hit `CounterAttack` `ObjectMagic` / delayed counter damage, and bounded passive proc surfaces for `FatalSword`, `MPEater`, `Hemorrhage`, and `Meditation`; player ranged attacks now surface `Focus` through `RangeAttack` / `ObjectRangeAttack` plus delayed damage while preserving the existing `ObjectAttack` bridge. The parallel late-game slices added bounded Hero Attack/Follow/CounterAttack AI with melee/ranged packet surfaces, Crystal drop/event-based Fishing reel behavior including miss/no-space/gold/event spawn paths, and sender-side mail blacklist rejection for blocked friends. Regression evidence: focused `magic_packet_crystal_` tests pass 72/72, Hero AI 3/3, Fishing 7/7, blacklist mail 1/1, full locked `mir2-simulation` 831/831 plus integration Hero AI 3/3, `cargo +1.89.0 fmt --check -p mir2-simulation`, `git diff --check` for the touched backend files, and locked `cargo +1.89.0 check --locked -p mir2-simulation` pass. Remaining backend depth is exact stat/hit chance math for passive accuracy skills, Hero equipment/stat math, embedded fishing slot item fidelity, market/guild deep semantics, and further Crystal effect tuning.

> Latest sync: 2026-05-10 advanced the uninterrupted Hero, ItemRental, and skill deep-semantic track. Runtime now imports and respects Crystal `NoHero` map flags, unsummons Heroes with Crystal feedback on no-hero map transfer, and prevents `NewHero` / `ChangeHero` from spawning a Hero on no-hero maps while preserving Hero records. Hero inventory transfer/take-back/use now moves, persists, and consumes Hero-bag items, and Hero auto-pot item packets now normalize invalid item indexes while consuming matching Hero HP/MP potions like Crystal. Gateway shared in-process ItemRental now runs a real two-account transaction path, while Simulation covers lifecycle return semantics: the adjacent request creates a borrower-side `Renting=true` invite, borrower fee lock and lender item lock are paired in zone state, confirmation transfers gold to the lender, delivers a rental-bound item to the borrower with owner/expiry metadata, records lender rented items, partner cancel queues rollback, expired rental items are removed from borrower inventory/equipment and mailed back to the owner with binding flags cleared / rental lock set / expiry extended, and dead-player ticks return rental items before normal drop handling. Skill parity now has player-side `SpellToggle` gates plus Crystal archer, Taoist, Wizard, and stealth/control packet semantics: `FlamingSword` consumes MP and latches, `CounterAttack` applies Crystal buff type 18 with MinAC/MaxAC/MinMAC/MaxMAC, `MentalState` cycles Crystal buff type 19 values and applies archer shot damage penalties, `Repulsion` / `EnergyRepulsor` / `FireBurst` push adjacent lower-level monsters with per-tile `ObjectPushed` and apply ThunderElement's repulsion-only damage path, `StormEscape` applies the ThunderStorm-style nearby damage tick, relocates the player, emits `ObjectEffect(StormEscape)`, and applies TemporalFlux teleport mana penalty, `Concentration` emits buff type 15 plus `SetConcentration` enabled/disabled state, `ElementalShot` / `ElementalBarrier` now maintain Crystal element-orb state through `SetElemental` gather/spend packets and apply orb-boosted damage or buff type 25, `StraightShot` / `DoubleShot` queue one/two delayed ranged damage hits, `ExplosiveTrap` spawns front-row trap objects and detonates on contact, `PoisonSword` consumes poison and marks the frontal arc, `BackStep` moves opposite facing with `UserBackStep` / `ObjectBackStep` and blocked distance-0 reporting, `BindingShot` queues `SetBindingShot` and roots nearby monsters, `VampireShot` queues delayed damage/heal and visible buff type 16, `PoisonShot` queues delayed damage plus Green poison ticks and visible buff type 17, `CrippleShot` consumes the active Vampire/Poison Shot buff, queues `RemoveBuff`, and triggers the buff follow-up effect, `NapalmShot` hits the target-centered Crystal area, `DelayedExplosion` marks/removes the delayed marker and explodes in the target area, `Trap` roots lower-level monsters while spawning a Trap `ObjectSpell`, `HellFire` hits forward and level-3 side lanes, `FireBang` / `IceStorm` hit target 3x3, `Blizzard` / `MeteorStrike` spawn 5x5 ground spells with persistent damage, `FireBounce` chains projectiles/damage, `MeteorShower` damages primary and secondary targets, `ThunderBolt` applies undead bonus damage, `ElectricShock` roots lower-level monsters without generic damage, `FlameDisruptor` applies non-undead bonus damage, `IceThrust` hits the Crystal three-column path and emits Frozen poison state, `MassHealing` queues delayed area healing, `HealingCircle` queues a delayed `ObjectSpell` plus Crystal's 25-point heal tick, `Curse` consumes an amulet before delayed hostile-area buff type 12 stat-rate penalties, `Purification` queues delayed `RemoveBuff` for player Curse debuffs, `Revelation` queues target `ObjectHealth` reveal packets, `PoisonCloud` consumes amulet/GreenPoison and ticks a 3x3 ground cloud, `Plague` consumes amulet plus optional poison and applies 3x3 damage/debuff branches, `LightBody` applies buff type 8 Agility stats, `MoonLight` / `DarkBody` / `Hiding` / `MassHiding` apply Crystal stealth buff and `ObjectHidden` hide/reveal lifecycle, `FrostCrunch` queues delayed target damage plus a freeze buff/root window, `Vampirism` queues delayed damage plus player healing, `TurnUndead` only damages undead targets with level-gated instant-kill behavior, `EnergyShield` applies Crystal buff type 20 with HP-gain/shield-percent stats, `ImmortalSkin` applies buff type 23 with defence/stat-tradeoff payloads, `PetEnhancer` buffs friendly/summoned monsters, `LionRoar` paralyses nearby lower-level monsters with `LRParalysis`, `BattleCry` forces nearby hostile monsters to reacquire the caster, `Poisoning` consumes equipped Green/Red poison, queues delayed `ObjectPoisoned`, projects monster poison state into `ObjectMonster.poison`, and ticks Green poison monster damage while Red poison remains a visible marker, and `TrapHexagon` consumes an amulet, roots the hostile 3x3 group, and queues the delayed eight-point `ObjectSpell` ring. Verification passed focused Hero/NoHero/auto-pot regressions, Hero inventory/auto-pot regressions 25/25, ItemRental expiry/death/mail regressions, focused SpellToggle/FlamingSword/CounterAttack/MentalState 6/6, casting 13/13, magic-packet Crystal skill tests 54/54 after adding Hiding/FrostCrunch/Vampirism/TurnUndead, EnergyShield/ImmortalSkin/PetEnhancer/LionRoar/BattleCry, MentalState/NapalmShot/DelayedExplosion/Trap/ExplosiveTrap/PoisonSword/PoisonCloud/Plague, and HellFire/FireBang/IceStorm/Blizzard/MeteorStrike/FireBounce/MeteorShower/ThunderBolt/ElectricShock/FlameDisruptor/IceThrust on top of FireWall/Lightning/ThunderStorm, shared Gateway registry 13/13, Rust fmt, and locked four-package check for GameData/Protocol/Simulation/Gateway. Remaining backend depth is broader profession-by-profession skill fidelity, exact Hero combat/equipment AI, and human client feel acceptance.
> Follow-up skill slices: `ShoulderDash` now drives Crystal per-step `UserDash` / `ObjectDash`, low-level target push, fail packets, and `MagicCast`; `FlashDash` drives `UserDashAttack` / `ObjectDashAttack`, fallback `ObjectAttack`, delayed hit, and Stun poison; `SlashingBurst` drives `UserAttackMove` plus delayed front-tile damage. Ground/line magic now covers `FireWall` delayed five-cell `ObjectSpell` spawn plus persistent same-cell damage ticks, `Lightning` six-tile forward scans, `ThunderStorm` current-location 5x5 damage with Crystal non-undead 1/10 scaling, `HellFire`, `FireBang`, `IceStorm`, `Blizzard`, `MeteorStrike`, `FireBounce`, `MeteorShower`, `ThunderBolt`, `ElectricShock`, `FlameDisruptor`, and `IceThrust`. The latest bespoke follow-up covers `Hiding`, `MassHiding`, `FrostCrunch`, `Vampirism`, `TurnUndead`, `EnergyShield`, `ImmortalSkin`, `PetEnhancer`, `LionRoar`, `BattleCry`, `MentalState`, `NapalmShot`, `DelayedExplosion`, `Trap`, `ExplosiveTrap`, `PoisonSword`, `PoisonCloud`, and `Plague` with focused packet/state regressions.

> Latest sync: 2026-05-08 advanced the requested skill-system and late-game deep-semantic slice. Crystal `ObjectHero` now round-trips the owner name payload, Gateway/Web exposes it as `ownerName`, Simulation spawns Stage 5 Hero state as a real visible `ObjectHero`/`ObjectHealth` world entity, and the Hero follows the player through server `ObjectWalk`/`ObjectRun` packets while snapshots preserve the owner label. Hero lifecycle now uses Crystal spawn-state values (`Summoned=2` for newly spawned/changed Heroes), default Hero `SpellToggle` packets route to the spawned Hero like Crystal, `HeroInformation` includes auto-pot state, and `SetAutoPotValue` / `SetAutoPotItem` mutate and echo Hero auto-pot settings. Targeted projectile spells now emit Crystal `ObjectProjectile` for FireBall, GreatFireBall, ThunderBolt, and SoulFireBall, and MagicBooster now applies Crystal buff type 21 with MinMC/MaxMC and ManaPenaltyPercent stats. Verification passed focused Protocol Hero round-trip, focused Simulation Hero 18/18, SpellToggle 2/2, MagicBooster 1/1, FireBall projectile/damage regression, locked three-package fmt/check for Protocol/Simulation/Gateway, and Web typecheck. Remaining depth is exact Hero combat/equipment AI plus fuller per-spell tuning and human Crystal feel acceptance, not missing Hero visibility/basic projectile/auto-pot packet plumbing.

> Latest sync: 2026-05-08 stabilized the full Player Web Stage 5 smoke against a repeatedly reused demo account and locked the backend item metadata fixes behind regression coverage. Runtime now repairs loaded known-item metadata for potions, `qa.giveItem` seeds red/blue potions with usable HP/MP metadata, inventory unique IDs are normalized/rekeyed for dirty duplicate inventory/storage states, and crafted/QA-created items allocate collision-free unique IDs. The smoke path now verifies split/use/drop/pickup/storage take-back by exact `uniqueId` / `objectId` rather than brittle same-name or fixed-slot assumptions. Verification passed focused `stage5_qa_give_item_seeds_usable_healing_metadata` 1/1, focused `unique_id` 13/13, locked `cargo +1.89.0 check --locked -p mir2-simulation -p mir2-gateway`, Web `node --check` / `npx tsc --noEmit`, and a live local Gateway/Web full Stage 5 UI smoke with 114 screenshots and `criticalConsoleErrorCount=0`.

> Latest sync: 2026-05-08 connected the late-dialog frontend surface to real Gateway commands and exposed ItemRental runtime state in snapshots. Player Web can now drive Hero create/behaviour/change and ItemRental request/fee/period/cancel/list commands from the System Menu, while Creature/Mount/Fishing buttons send their typed protocol-backed browser commands instead of no-op clicks. Simulation now projects `ItemRentalResource` into `stage5Systems.itemRental`, including active partner, fee, days, deposited item, gold/item lock state, and persisted rented records, with regression coverage on request/cancel/confirm flows. Verification passed focused Simulation `item_rental_` 3/3, locked `cargo +1.89.0 check --locked -p mir2-simulation -p mir2-gateway`, focused Gateway browser-command mapping 7/7, Web `node --check` / `npx tsc --noEmit`, and a live local Gateway/Web fast Stage 5 smoke with 22 screenshots and `systemMenuSocial=44`. Remaining backend rental depth is cross-account borrower delivery/expiry return and death-return behavior; the frontend/backend command bridge is no longer the blocker.

> Latest sync: 2026-05-07 stabilized the frontend-facing runtime smoke path and locked the backend fixes behind full two-package regression. Stage 5 `event.spawn` now defaults to a Crystal-resolvable `BugBat`, searches the current Crystal map collision data for nearby spawnable tiles, and is covered at `crystal:0:330:270`; `qa.openNpcDialog` opens the real InnKeeper_Brittney Crystal script dialog for smoke setup; and the browser smoke uses `autoTick=0` plus WebSocket diagnostics so business commands are no longer starved behind automatic keep-alive/tick snapshots. Verification passed focused drop/unique-id/QA damage/NPC/event/gateway tests, shared in-process registry 11/11, `cargo +1.89.0 fmt --check -p mir2-simulation -p mir2-gateway`, locked `cargo +1.89.0 check --locked -p mir2-simulation -p mir2-gateway`, Web `npx tsc --noEmit`, smoke script syntax, full locked `mir2-gateway` 107/107 plus packet-trace bin 17/17, full locked `mir2-simulation` 731/731, and the live isolated-Gateway Stage 5 UI smoke with 102 screenshots and 0 critical console errors.

> Latest sync: 2026-05-07 added service-backed equipped repair coverage for the frontend/runtime parity path. `RepairItem` and `SRepairItem` now resolve equipped Crystal slot unique IDs before inventory fallback, preventing weapon slot id `0` from colliding with bag slot `0`; normal repair restores current durability while applying Crystal-style max-durability loss, and special repair restores current durability without reducing max durability. Stage 5 QA setup gained `qa.damageEquipment` so UI smoke can damage equipped items deterministically before using real Blacksmith `@Repair` / `@SRepair` services. Verification passed focused `equipped_slot_id` repair regressions 2/2, focused `stage5_qa_damage_equipment` 1/1, `cargo +1.89.0 fmt --check -p mir2-simulation -p mir2-gateway`, locked `cargo +1.89.0 check --locked -p mir2-simulation -p mir2-gateway`, Web typecheck/script syntax, `git diff --check`, and a live isolated-Gateway Stage 5 UI smoke with 101 screenshots and 0 critical console errors.

> Latest sync: 2026-05-07 tightened typed packet observability after the full server-packet pass. Newly typed Crystal server packets now reach Gateway/Web as structured JSON payloads rather than only Debug summaries, and trace display names use typed enum names for IDs that still route through the legacy static-name fallback. Protocol trace entries now store packet names as owned strings, `ServerPacket` / packet ID types are serializable for observability, and focused regressions lock `NewMapInfo`, `Rankings`, and unit-packet event payloads. Game-data tests now also assert the current NPC command summary has `0` unimplemented commands/occurrences and the monster AI summary has no remaining runtime priorities. Verification passed fmt/diff/check gates plus full locked GameData/Gateway/Protocol/Simulation tests: GameData 27/27, Gateway lib 105/105 plus packet-trace bin 17/17, Protocol lib 33/33 plus codec 33/33, and Simulation 722/722. The remaining risk is semantic/client acceptance, not blind observability for typed packets.

> Latest sync: 2026-05-07 completed the full Crystal server-packet typed payload pass. The Rust protocol decoder now has explicit typed branches for all 279 Crystal server packet IDs `0..278`; the previous remaining Raw-only payloads are now modeled for map/world-map/search/user-slot refresh, player update/inspect/status and map-change surfaces, guild status/member/notice/storage/war packets, auto-pot, NPC image/input/pearl goods, quest inventory, reincarnation, dash/attack-move/concentration/elemental, awakening material, transform, game-shop stock, rankings, notice, and guild territory payloads. A local packet scan reports `explicit=279 remaining=0`, so known Crystal server packets no longer silently decode as Raw. `ServerPacket::Raw` remains available for explicit manual frame construction. Verification passed: `cargo +1.89.0 fmt --check -p mir2-protocol -p mir2-gateway`, locked `cargo +1.89.0 check --locked -p mir2-protocol -p mir2-gateway -p mir2-simulation`, focused Protocol tests 32/32 plus codec 33/33, and full locked three-package tests with Gateway lib 104/104 plus packet-trace bin 17/17, Protocol lib 32/32 plus codec 33/33, and Simulation 722/722. Remaining backend risk is now semantic exactness and multi-actor gameplay/client acceptance, not missing server packet typing.

> Latest sync: 2026-05-07 landed the next P1/P2 packet/runtime parity slice after the late-system closure. Protocol now has typed Crystal server packets and round-trip coverage for Group utility, Quest, and Refine responses, with packet-trace names and Gateway Web event serialization. Simulation now mutates real modeled state for group invite/member/toggle packets, quest accept/finish/abandon/share packets, Stage 5 market consign/buy/get-back/sell-now packets, refine deposit/retrieve/cancel/start/check packets, `OpenDoor`, and `RequestMapInfo` / `RequestMonsterInfo` / `RequestNpcInfo` using the imported Crystal map/monster/NPC manifests. Verification passed: focused regressions for each new path, `cargo +1.89.0 fmt --check -p mir2-protocol -p mir2-simulation -p mir2-gateway`, locked `cargo +1.89.0 check --locked -p mir2-protocol -p mir2-simulation -p mir2-gateway`, Web `npx tsc --noEmit`, fast live Stage 5 UI smoke, and full locked Protocol/Gateway/Simulation regression with Gateway lib 103/103 plus packet-trace bin 17/17, Protocol lib 29/29 plus codec 32/32, and Simulation 722/722. Remaining backend risk in this slice is deeper exactness: NPC market page/list payloads, Crystal refine timers/probability/ore economics, market bid/commission/mail settlement, and final client-dialog acceptance.

> Latest sync: 2026-05-07 tightened the P1/P2 backend gates after the multi-agent gameplay closure. Raw/known-raw server payload observability now includes packet name/id, payload length, and lowercase payload hex in Gateway Web events and `packet_trace`; IntelligentCreature now applies Crystal default rule profiles, mouse/semi/manual pickup gates, item category and pickup-grade filters, fullness pickup blocking, and blackstone progress independent of pickup fullness; Fishing now requires an equipped Crystal fishing rod, bait, hook flag, reel flag for autocast, valid fishing attribute, and rod durability damage before emitting `FishingUpdate`; Mount toggling now enforces map `NoMount`, `NeedBridle`, saddle, and reins state, and the Crystal respawn manifest/game-data structs preserve those map flags for future data refreshes. Verification passed focused protocol/simulation/gateway regressions for these surfaces, Rust fmt/check/diff gates, Web `npx tsc --noEmit`, script syntax checks, live Stage 5 UI smoke with 83 screenshots and 0 critical console errors after the Web sprite loader stopped requesting unexported Crystal scene libraries, and full locked tests across GameData 27/27, Gateway lib 100/100 plus packet-trace bin 17/17, Protocol lib 26/26 plus codec 32/32, and Simulation 716/716. Remaining backend risk is deeper exact tuning such as fishing slot stat math, hero AI/equipment/combat behavior, and production edge semantics, not the previously missing P1/P2 gates.

> Latest sync: 2026-05-07 closed the requested multi-agent gameplay depth for the modeled backend path. Shared Gateway sessions now commit two-account Trade item/gold delivery and roll back pending offers on partner cancel/disconnect; IntelligentCreature ticks now perform automatic pickup while advancing fullness decay and blackstone progress; Fishing ticks can find/reel loot and autocast; equipped Mount use toggles riding through the Crystal `UseItem` path; Hero create/change/behaviour packets now update Stage 5 hero state and user information; Gateway Web command mapping and packet traces expose the new unique-id/equipment/trade details. Verification passed: locked three-package fmt/check plus full `CARGO_CACHE_AUTO_CLEAN_FREQUENCY=never cargo +1.89.0 test --locked -p mir2-protocol -p mir2-simulation -p mir2-gateway -- --test-threads=1` with `mir2-gateway` lib 99/99 plus packet-trace bin 16/16, `mir2-protocol` lib 25/25 plus codec 32/32, and `mir2-simulation` 711/711. Remaining backend depth is exact client-dialog acceptance and deeper fidelity tuning, not the previously open Trade delivery/rollback or IntelligentCreature fullness/blackstone/automatic-pickup closure.

> Latest sync: 2026-05-06 made IntelligentCreature stateful for the modeled backend path. Stage 5 state now stores `ClientIntelligentCreature` rows; `UpdateIntelligentCreature` creates/updates creatures, supports summon, unsummon, and release flags, emits `NewIntelligentCreature` for first registration, and returns `UpdateIntelligentCreatureList` with summoned status; `RequestIntelligentCreatureUpdates` reads the persisted list; `IntelligentCreaturePickup` can collect a targeted ground drop through an active creature and emits `IntelligentCreaturePickup` plus `GainedGold` or `GainedItem`. Verification passed: focused `intelligent_creature_packets_update_state_and_pick_up_ground_gold`, locked fmt/check for Protocol/Simulation/Gateway, and full `CARGO_CACHE_AUTO_CLEAN_FREQUENCY=never cargo +1.89.0 test --locked -p mir2-protocol -p mir2-simulation -p mir2-gateway -- --test-threads=1` with `mir2-gateway` lib 96/96 plus packet-trace bin 16/16, `mir2-protocol` lib 25/25 plus codec 32/32, and `mir2-simulation` 708/708. Remaining backend depth is fullness/feeding decay, blackstone production, automatic pickup scanning with exact filters/ranges, visible pet movement, and final client-dialog acceptance.

> Latest sync: 2026-05-06 made the Trade packet family stateful for the modeled backend path. `TradeRequest` can now be initiated through an adjacent shared Gateway player name, `TradeReply` returns `TradeAccept`, `TradeGold` records and echoes offered gold, `DepositTradeItem` and `RetrieveTradeItem` maintain Stage 5 trade slots and emit `TradeItem`, `TradeConfirm` locks/completes the offer while deducting gold and removing offered inventory items, and `TradeCancel` clears active trade state. Focused verification passed for Simulation trade packet behavior 2/2, existing Stage 5 trade command behavior 3/3, and Gateway adjacent-player trade request 1/1. Full verification also passed: locked fmt/check for Protocol/Simulation/Gateway and full `CARGO_CACHE_AUTO_CLEAN_FREQUENCY=never cargo +1.89.0 test --locked -p mir2-protocol -p mir2-simulation -p mir2-gateway -- --test-threads=1` with `mir2-gateway` lib 96/96 plus packet-trace bin 16/16, `mir2-protocol` lib 25/25 plus codec 32/32, and `mir2-simulation` 708/708. Remaining Trade depth is partner-side two-account exchange delivery, disconnect rollback after cross-offers, and final client-dialog acceptance.

> Latest sync: 2026-05-06 made the Mail/Friend packet family stateful on top of Stage 5 persistence. `ClientPacket::SendMail` now validates recipient/cost/gold, rejects unsupported live item attachments with Crystal-shaped failure, deducts gold through `LoseGold`, creates a persisted `Stage5MailMessage`, and returns `MailSent` plus `ReceiveMail`; `ReadMail`, `CollectParcel`, `DeleteMail`, `LockMail`, and `MailCost` now return mailbox-derived packets, including parcel gold gain and deletion filtering. Friend packets now mutate/read persisted social state as well: `AddFriend`, `RemoveFriend`, `RefreshFriends`, and `AddMemo` produce `FriendUpdate` rows with `ClientFriend` names, block flags, online flags, and memo text instead of static empty lists. Verification passed: focused `mail_friend_packets_preserve_crystal_ack_surface`, adjacent Stage 5 social/mail persistence tests, locked fmt/check for Protocol/Simulation/Gateway, and full `CARGO_CACHE_AUTO_CLEAN_FREQUENCY=never cargo +1.89.0 test --locked -p mir2-protocol -p mir2-simulation -p mir2-gateway -- --test-threads=1` with `mir2-gateway` lib 95/95 plus packet-trace bin 16/16, `mir2-protocol` lib 25/25 plus codec 32/32, and `mir2-simulation` 707/707. Remaining backend depth in this family is exact Crystal live-item attachment transfer, mail lock/reply persistence, online friend/session fanout, and final UI acceptance.

> Latest sync: 2026-05-06 closed the full Crystal packet-ID coverage gap for the backend protocol layer. Rust now knows every Crystal client packet ID `0..152` and every Crystal server packet ID `0..278`; all 153 client packets have typed `ClientPacket` variants, while server decoding has typed variants for the implemented surfaces and preserves known-but-complex server payloads through `ServerPacket::Raw` instead of failing or dropping bytes. This sync also fixes and tests the Crystal ID ordering around item combining: client `CombineItem=110`, `AwakeningNeedMaterials=111`, server `CombineItem=214`, and `ItemUpgraded=215`. Additional typed server packets now cover Crystal projectile/range/push/dash/observe/buff-pause/hidden/map-effect visuals and late magic/awakening/inventory surfaces: `ObjectProjectile`, `RangeAttack`, `Pushed`, `ObjectPushed`, `MapEffect`, `AllowObserve`, `PauseBuff`, `ObjectHidden`, `UserDash`, `ObjectDash`, `UserDashFail`, `ObjectDashFail`, `RemoveDelayedExplosion`, `ObjectDeco`, `ObjectSneaking`, `ObjectLevelEffects`, `SetBindingShot`, `SendOutputMessage`, `NPCAwakening`, `NPCDisassemble`, `NPCDowngrade`, `NPCReset`, `AwakeningLockedItem`, `Awakening`, and `ResizeInventory`. Gateway Web event JSON and packet trace naming expose these packets. Verification passed: focused protocol tests for full ID ranges, Raw server fallback, and the new server packet round trips; `cargo +1.89.0 fmt --check -p mir2-protocol -p mir2-simulation -p mir2-gateway`; `cargo +1.89.0 check --locked -p mir2-protocol -p mir2-simulation -p mir2-gateway`; and full `CARGO_CACHE_AUTO_CLEAN_FREQUENCY=never cargo +1.89.0 test --locked -p mir2-protocol -p mir2-simulation -p mir2-gateway -- --test-threads=1` with `mir2-gateway` lib 95/95 plus packet-trace bin 16/16, `mir2-protocol` lib 25/25 plus codec 32/32, and `mir2-simulation` 707/707. Remaining backend work is no longer missing packet IDs; it is deeper stateful semantics for still-Raw or bounded surfaces such as complex guild/status/listing/ranking/hero-information payloads, cross-account transaction closure, and full per-spell gameplay fidelity.

> Latest sync: 2026-05-06 added Crystal late-system packet-surface parity for Trade, Fishing/Mount, Mail/Friend, and IntelligentCreature. Protocol now models the audited Crystal packet IDs and payloads for trade request/reply/gold/confirm/cancel/deposit/retrieve, fishing cast/autocast and mount/fishing updates, mail send/read/collect/delete/lock/cost plus receive/sent/parcel/cost responses, friend add/remove/refresh/memo plus `FriendUpdate`, and intelligent creature update/request/pickup plus list/new/rename/pickup responses. Gateway Web can send these client packet commands, serialize their server packet events, and packet-trace detail/trace names include the new families. Simulation now returns Crystal-safe surfaces for the bounded backend state that exists today: no-partner trade no-ops and failed deposit/retrieve acks, fishing update toggles, empty/no-mail friend and mailbox update shapes, mail cost/locked-item echoes, empty intelligent-creature list updates, and existing item-rental lender-side flow. Verification passed: focused Protocol/Simulation/Gateway regressions for the new packet families, `cargo +1.89.0 fmt --check -p mir2-protocol -p mir2-simulation -p mir2-gateway`, locked `check` for those packages, and full `cargo +1.89.0 test --locked -p mir2-protocol -p mir2-simulation -p mir2-gateway -- --test-threads=1` with `mir2-gateway` lib 91/91 plus packet-trace bin 16/16, `mir2-protocol` lib 15/15 plus codec 32/32, and `mir2-simulation` 705/705. The remaining backend depth is not packet typing but stateful gameplay completion: two-player trade exchange/commit, real mail/friend persistence and delivery, intelligent-creature ownership/pickup lifecycle, full fishing loot/durability behavior, and mount ride/equip progression.

> Latest sync: 2026-05-06 added Crystal item-rental packet and runtime parity coverage. Protocol now models the full Crystal item-rental client packet family (`GetRentedItems`, `ItemRentalRequest`, `ItemRentalFee`, `ItemRentalPeriod`, `DepositRentalItem`, `RetrieveRentalItem`, `CancelItemRental`, `ItemRentalLockFee`, `ItemRentalLockItem`, `ConfirmItemRental`) and server packet family (`GetRentedItems`, `ItemRentalRequest`, `ItemRentalFee`, `ItemRentalPeriod`, `DepositRentalItem`, `RetrieveRentalItem`, `UpdateRentalItem`, `CancelItemRental`, `ItemRentalLock`, `ItemRentalPartnerLock`, `CanConfirmItemRental`, `ConfirmItemRental`) with Crystal packet IDs and codec regressions. Simulation implements the lender-side Crystal flow for rental request, fee/period selection, item deposit/retrieve, cancel/refund, lock state, confirmation, rental binding flags, `GetRentedItems`, and save/reload of rental records. Gateway Web can drive every packet command and serialize every rental server event, and shared in-process routing resolves adjacent remote players for `ItemRentalRequest`. Verification passed: focused ItemRental Protocol/Simulation/Gateway regressions, full `mir2-protocol` lib 7/7 plus codec 32/32, full `mir2-gateway` lib 83/83 plus packet-trace bin 16/16, full `mir2-simulation` 701/701, and locked `fmt`/`check` across Protocol/Simulation/Gateway. Remaining backend rental depth is true cross-account borrower delivery, expiry return/mail handling, and death-return semantics.

> Latest sync: 2026-05-06 added Crystal magic/buff packet and runtime parity coverage. Protocol now models Crystal `MagicKey`, `Magic`, and `SpellToggle` client packets plus the server magic/buff packet family (`NewMagic`, `RemoveMagic`, `MagicLeveled`, `Magic`, `MagicDelay`, `MagicCast`, `ObjectMagic`, `SpellToggle`, `ObjectMana`, `AddBuff`, `RemoveBuff`) with Crystal packet IDs and codec regressions. Simulation maps real `ClientPacket::Magic` into the skill runtime by Crystal `Spell`, returns `UserLocation` on unknown/rejected casts, emits `ObjectMana`, `Magic`, `ObjectMagic`, `AddBuff`, `RemoveBuff`, `ObjectHealth`, `MagicLeveled`, and `MagicDelay` on the modeled paths, persists magic hotkeys/level/experience/delay into skill snapshots, acknowledges `SpellToggle`, teaches Crystal books through `NewMagic`, and executes a broader manifest-backed subset for target damage, Teleport, MagicShield, Fury/buff, potion MP, and book-learning behavior. Gateway Web and packet trace can send/inspect these packet surfaces for QA/admin sessions. Verification passed: focused Protocol/Simulation/Gateway magic/buff regressions, packet-trace flow-name coverage, Player Web `npx tsc --noEmit`, `cargo +1.89.0 fmt -p mir2-protocol -p mir2-simulation -p mir2-gateway`, locked `check` across the same packages, `git diff --check`, and full `cargo +1.89.0 test --locked -p mir2-protocol -p mir2-simulation -p mir2-gateway -- --test-threads=1` covering Gateway lib 82/82 plus packet-trace bin 16/16, Protocol lib 5/5 plus codec 32/32, and Simulation 698/698.

> Latest sync: 2026-05-06 added Admin-console backend parity for Crystal server operations. Admin API now routes Crystal console-style operations through audited command envelopes with RBAC/approval enforcement and real account-store/content/NameLists/Gateway execution for account writes, character writes/rename/NPC flags/PK/chat-ban, player messaging, broadcasts, market cancel/expire/delete, guild moderation, NameLists create/add/remove/delete, content bundle publishing, and server control. Simulation saves now persist Crystal `PKPoints`, chat-ban state, and Stage 5 auction `expired`, with chat packets rejecting active chat-banned players and expired auctions no longer buyable. Gateway exposes admin session/control endpoints for the Admin API. Admin Web consumes these read/write models through Console, Accounts, Market, Guilds, NameLists, Content, and player-detail editor/chat-ban pages. Verification passed: `cargo +1.89.0 fmt --check -p mir2-simulation -p mir2-admin-api -p mir2-gateway`, `cargo +1.89.0 check --locked -p mir2-simulation -p mir2-admin-api -p mir2-gateway`, full `mir2-simulation` 692/692, full `mir2-admin-api` tests 33/33 plus outbox bin 6/6, focused Gateway admin endpoint test, Admin Web typecheck/build, live HTTP smoke covering mutations plus readback, and Playwright snapshots for the new Admin surfaces.

> Latest sync: R327 added manifest-backed Gameshop purchase command handling for the Web Crystal Gameshop. Runtime `stage5Command` now accepts `gameShop.buyCredit` and `gameShop.buyGold`, resolves `gameShopIndex` through the generated Crystal game-shop manifest, validates price/currency/capacity, deducts credit or gold, delivers credit purchases through Stage 5 mail, and adds gold purchases directly to inventory. Focused coverage `crystal_game_shop_credit_buy_uses_manifest_and_mail_delivery` passed, and Web browser evidence records the expected zero-credit rejection for `QA0429A / QA0429Hero`. This is a Stage 5/frontend integration capability and does not change the R300 backend packet-acceptance gate.

> Latest sync: R315 aligned real new-character save state with Crystal's empty defaults. Crystal source shows `CharacterInfo` inventory/equipment/quest inventory/magics arrays start empty, `AccountInfo.Storage` starts empty, and account gold defaults to 0 unless server `StartItems` are configured. Runtime now creates `NewCharacter` saves with empty bag/belt/storage/equipment/quest/skill state and gold 0; `apply_character_save` treats empty arrays as explicit empty; old level-1 saves that exactly match the former Web demo seed state migrate to empty; and the default `demo/Scout` automation character still receives its Stage 5 demo seed data. Focused coverage proves new Crystal characters and migrated level-1 legacy seed saves have no Web starter items/skills/quests/storage/equipment and gold 0 while `demo/Scout` keeps seed state. Verification passed: focused `mir2-simulation start_game_` 16/16, `cargo +1.89.0 build --locked -p mir2-gateway`, web `tsc --noEmit`, R315 browser capture, `cargo +1.89.0 fmt --check`, and capture-script `node --check`. R300 remains the backend packet-acceptance gate.

> Latest sync: R314 corrected default player vitals to Crystal `BaseStats` formulas instead of the old hardcoded `120/120/45` starter values. New `CharacterSaveRecord` defaults, runtime default resources, generated default saves, and legacy saves that exactly match the old hardcoded triple now derive HP/MP by class and level. Focused coverage proves the built-in level-7 Warrior starts at `60/60` HP and `35` MP, and a legacy level-1 Warrior save migrates to `18/18` HP and `14` MP, matching the original-client HP-only HUD evidence. Verification passed: focused `mir2-simulation start_game_` 15/15, `cargo +1.89.0 build --locked -p mir2-gateway`, web `tsc --noEmit`, R314 browser capture, `cargo +1.89.0 fmt --check`, and `git diff --check`. This is a backend vitals/source-data correction plus frontend evidence support; R300 remains the backend packet-acceptance gate.

> Latest sync: R310 frontend visual-watch support added server snapshot `questIds` to `WorldEntitySnapshot` so Web quest markers can be scoped to the actual Crystal NPC quest association instead of rendering for every NPC. This is a frontend visual-parity support change, not a backend packet acceptance change. Verification passed: `cargo fmt --check` and focused `mir2-simulation crystal_current_map_transfer_spawns_visible` 2/2.

> Latest sync: R305 completed for current-map visible Crystal respawn runtime population. R304 restored NPCs, but aligned Bichon snapshots still missed animals/guards because visible respawn `ObjectMonster` packets were not backed by ECS world entities and were overwritten by later snapshots. `apps/simulation/src/runtime.rs` now builds the current-map world spawn table from visible Crystal respawns at the player's current position. Evidence at `docs/generated/player-qa/r305-bichon-visible-world-snapshot.json` records `entityCount=17`, `npcCount=8`, `monsterCount=8`, including `Deer` and `Royal_Guard` at `0:284,607`; browser evidence confirms 8 monster sprite elements. Verification passed: focused R305 simulation regression, existing visible-respawn density regression, `cargo +1.89.0 fmt --check`, `cargo +1.89.0 build --locked -p mir2-gateway`, live WS probe, browser capture, gateway health, and web HTTP 200. This is Candidate integration evidence and does not change R300 backend/server packet acceptance wording.

> Latest sync: R304 completed for current-map Crystal NPC runtime population. The aligned Bichon web comparison showed saved-character `StartGame` on a real Crystal map was not instantiating current-map NPC-info manifest entries into the ECS world, so the web snapshot could contain only the player even though the source metadata existed. `apps/simulation/src/runtime.rs` now repopulates current Crystal maps on saved-character start and Crystal transfer, and live WS evidence at `docs/generated/player-qa/r304-bichon-npc-world-snapshot.json` confirms `QA0429A / QA0429Hero` at map `0`, `284,607` has `npcCount=8` with `Assistant_Jane` and `Merchant_Ruben`. Verification passed: `cargo +1.89.0 fmt --check`, focused R304 NPC regression, adjacent `transfer_map`, `start_game_emits_visible_object_packets`, `world_snapshot_marks_safe_zone_after_start_game`, `cargo +1.89.0 build --locked -p mir2-gateway`, and a live WS probe. This does not change the R300 backend/server packet acceptance wording and does not close full frontend visual 1:1.

> Latest sync: R302 completed for original-client live comparison tooling. `packet_trace` now supports `MIR2_PACKET_TRACE_KEEP_LIFECYCLE_CHARACTER=1` so `account_lifecycle` can retain the Crystal character it creates for original client visual QA fixtures. R302 used this to create retained character `R302HeroB` and capture original Crystal client select/game evidence under `docs/generated/player-qa/r302-original-client/summary.json`. The fresh R302 live matrix is diagnostic only (`stableDiffCleanCount=2/9`, `packetParityAccepted=false`) because local and Crystal fixture state was not aligned; R300/R298 remain the backend packet acceptance source. Verification: packet-trace bin 16/16 passed.

> Latest sync: R301 completed for the final automated Candidate acceptance-pack refresh. No backend/server acceptance wording changed: R300 remains the backend/server packet acceptance decision, and strict exact remains diagnostic. R301 reverified the current backend/gateway packages and surrounding automation without Docker: packet-trace bin 15/15, `mir2-game-data` 27/27, `mir2-gateway` 55/55 plus packet-trace bin 15/15, `mir2-admin-api` 22/22, `mir2-simulation` 674/674, web `tsc --noEmit`, web build, map API smoke 18/18, minimap smoke 0 failures with known 450/451 warning, WS load 64/64 ready with 0 errors, and Stage 5 UI smoke 88 screenshots with 0 critical console errors. Evidence summary: `docs/generated/player-qa/r301-summary.json`.

> Latest sync: R300 completed for stable-diff packet acceptance. The current tracked backend/server slice now treats stable live packet comparison as the accepted packet gate: R298 provides 9/9 local OK, 9/9 Crystal OK, `crystalMissingCount=0`, `stableDiffCleanCount=9`, and `acceptedStableLiveComparisonCount=9`; R299 payload-hex probing shows strict exact dirtiness is live Crystal dynamic state; and R300 adds explicit packet-trace acceptance mode plus acceptance documentation. Strict exact remains a diagnostic for deterministic Crystal fixture work. Verification passed: `cargo +1.89.0 test --locked -p mir2-gateway --bin packet_trace -- --test-threads=1` (15/15) and `cargo +1.89.0 fmt --check`. Backend/server tracked slice is now **100% Accepted for the tracked backend/server slice under stable-diff packet acceptance**.

> Previous sync: R298 completed on Windows for live Crystal stable packet-matrix refresh. With `MIR2_CRYSTAL_TCP_ADDR=127.0.0.1:7000`, local gateway `127.0.0.1:7310`, `CRYSTAL_CLIENT_ROOT=E:\mir2\Crystal\Build\Client\Debug`, and the `Cdx0428030348` fixture, `packet_trace --matrix` wrote `docs/generated/packet-traces/r298-live-matrix/latest-matrix.json` with 9/9 local OK, 9/9 Crystal OK, `crystalMissingCount=0`, `stableDiffCleanCount=9`, and `acceptedStableLiveComparisonCount=9` after classifying Crystal `TimeOfDay` payloads as stable-comparator volatile. Exact strict diff remains dirty (`diffDirtyCount=9`, `acceptedLiveComparisonCount=0`) due live AOI/world/volatile packet surfaces; after R300 it is diagnostic rather than the accepted packet gate. Validation passed without Docker: `mir2-simulation` 674/674, `mir2-gateway` 55/55 plus packet-trace bin 14/14, `mir2-admin-api` 22/22, `cargo +1.89.0 fmt --check`, `git diff --check`, and web `tsc --noEmit`.

> Latest sync: R297 completed on Windows for frontend/player QA refresh plus gateway persistence hardening. Account-store JSON saves now serialize/retry atomic replacement for concurrent Windows load, gateway `MapInformation` exposes minimap/big-map indices to the web client, and the WS load harness creates a character for Crystal-aligned empty accounts before `StartGame`. Validation passed without Docker: `mir2-simulation` 674/674, `mir2-gateway` 55/55 plus packet-trace bin 14/14, `mir2-admin-api` 22/22, `cargo +1.89.0 fmt --check`, `git diff --check`, web `tsc --noEmit`, web build, WS load 64/64 ready with 0 errors, map API smoke 18/18, minimap smoke 0 failures with known 450/451 warning, and Stage 5 UI smoke 88 screenshots with 0 critical console errors. R300 later closes the backend/server packet gate under stable-diff acceptance.

> Previous sync: R292 completed on Windows for the live Crystal stable packet-matrix gate. With `MIR2_CRYSTAL_TCP_ADDR=127.0.0.1:7000`, local gateway `127.0.0.1:7310`, `CRYSTAL_CLIENT_ROOT=E:\mir2\Crystal\Build\Client\Debug`, and the `Cdx0428030348` fixture, `packet_trace --matrix` wrote `docs/generated/packet-traces/r292-live-matrix/latest-matrix.json` with 9/9 local OK, 9/9 Crystal OK, `crystalMissingCount=0`, `stableDiffCleanCount=9`, and `acceptedStableLiveComparisonCount=9`. Exact strict diff remained dirty (`diffDirtyCount=9`, `acceptedLiveComparisonCount=0`) due live AOI/world/volatile packet surfaces, so backend/server tracked-slice parity remained **99.70% Candidate** and was not 100% Accepted. Validation passed: `mir2-simulation` 674/674, `mir2-gateway` 55/55 plus packet-trace bin 14/14, `mir2-admin-api` 22/22, `cargo +1.89.0 fmt --check`, `git diff --check`, and web `tsc --noEmit`.

> Latest sync: R248 completed on Windows for the backend/server data-import gate. The previously blocked `Server.MirDB` / `Envir\Routes` generator path now has local evidence: `generate-crystal-respawn-manifest.mjs` regenerated Crystal respawn/monster/item/NPC-info manifests from `E:\mir2\Crystal\Build\Server\Debug\Server.MirDB` and `E:\mir2\Crystal\Build\Server\Debug\Envir\Routes`, including map `NoThrowItem`, `NoDropPlayer`, and `NoDropMonster` flags. Validation passed: `mir2-game-data` 22/22, focused `no_drop_monster_map_rule` 2/2, full `mir2-simulation` 670/670, and `mir2-gateway` 55/55 plus packet-trace bin tests 7/7. R300 later closed the remaining backend packet gate through explicit stable-diff acceptance.

> Previous sync: R225 completed on the integration/global Candidate track; backend/server gameplay code was unchanged from R183 and remained green. `mir2-gateway` passed 54/54 including packet trace bin tests 7/7 after adding matrix summary coverage, local require-mode matrix evidence wrote 9/9 TCP-traceable artifacts with `localOk=true` under `docs/generated/packet-traces/r225-matrix`, and full package regressions passed (`mir2-game-data` 22/22, `mir2-simulation` 664/664). At R225 time backend/server tracked-slice parity estimate was 99.70%; R300 later closed the packet gate under explicit stable-diff acceptance.

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


Last updated: 2026-05-10

Scope: Rust backend parity against `E:\mir2\Crystal\Server`.
This tracker is for backend gameplay / server behavior only. It does not include frontend UI or asset alignment.

## Overall

Backend/server tracked-slice parity status: **100% Accepted for the tracked backend/server slice under stable-diff packet acceptance**. This is a backend/server tracked-slice status only, not the whole-project frontend-plus-backend 1:1 score, and not a claim that the whole Crystal universe or product-evolution systems are accepted. The full-project estimate is tracked in `docs/CRYSTAL-1TO1-ROADMAP.md`.

Backend rounds:
- Completed rounds: **R82**, **R83**, **R84**, **R85**, **R86**, **R87**, **R88**, **R89**, **R90**, **R91**, **R92**, **R93**, **R94**, **R95**, **R96**, **R97**, **R98**, **R99**, **R100**, **R101**, **R102**, **R103**, **R104**, **R105**, **R106**, **R107**, **R108**, **R109**, **R110**, **R111**, **R112**, **R113**, **R114**, **R115**, **R116**, **R117**, **R118**, **R119**, **R120**, **R121**, **R122**, **R123**, **R124**, **R125**, **R126**, **R127**, **R128**, **R129**, **R130**, **R131**, **R132**, **R133**, **R134**, **R135**, **R136**, **R137**, **R138**, **R139**, **R140**, **R141**, **R142**, **R143**, **R144**, **R145**, **R146**, **R147**, **R148**, **R149**, **R150**, **R151**, **R152**, **R153**, **R154**, **R155**, **R156**, **R157**, **R158**, **R159**, **R160**, **R161**, **R162**, **R163**, **R164**, **R165**, **R166**, **R167**, **R168**, **R169**, **R170**, **R171**, **R172**, **R173**, **R174**, **R175**, **R176**, **R177**, **R178**, **R179**, **R180**, **R181**, **R182**, **R183**
- Active round: **R305**
- Full-suite regression status: **674/674** passing after R301; R305 focused/adjacent `mir2-simulation` tests and `mir2-gateway` build passing; latest `mir2-gateway` **55/55** plus packet-trace bin **15/15** passing after R301; `mir2-admin-api` **22/22** passing after R301; `mir2-game-data` **27/27** passing after R301
- Historical Web/Stage5 automated-evidence status: **100.0% Candidate**. This
  does not include formal WN-CANDIDATE signing/attestation, native 30-minute
  client soak, real OS-DPI, independent-model review, or human acceptance.
- Whole-project real accepted 1:1 estimate: **roughly 90.0%**

Estimated completion context: current imported backend slice is green; full Crystal 1:1 expansion is tracked in `docs/CRYSTAL-1TO1-ROADMAP.md`.

- `Item-system deepening (best-effort)`: weapon refine is now a real mechanic instead of an unconditional `+1`. `RefineItem` computes a success chance from the deposited ingredients (count + DC/MC/SC bias from their Crystal templates) and a target stat, stored on `Stage5RefineState`; `CheckRefine` rolls deterministically and either adds the refined stat (DC→`added_attack`, MC/SC→`added_stats`) on success or costs durability on failure, with a soft cap (`REFINE_MAX_LEVEL`) and a no-materials/at-cap rejection that returns the ingredients. Exact ingredient weights remain Crystal `Settings` to re-validate against the C# reference. Verified by `mir2-simulation` lib refine suite (8/8: state machine, no-materials rejection, deterministic outcome, chance curve/cap, roll boundaries) with full lib at `837/837` of the non-environmental tests passing (70 pre-existing empty-Crystal-submodule failures unchanged) and `cargo check --workspace` green. Not a packet-acceptance claim.
- `100% tracked backend/server`: R300 accepts R298 stable live Crystal packet-matrix evidence as the packet gate for the current tracked slice: `docs/generated/packet-traces/r298-live-matrix/latest-matrix.json` has `artifactCount=9`, `crystalMissingCount=0`, `stableDiffCleanCount=9`, and `acceptedStableLiveComparisonCount=9`; `docs/generated/packet-traces/r299-movement-hex.json` shows strict exact dirtiness comes from live Crystal dynamic state; `docs/PACKET-PARITY-ACCEPTANCE.md` defines the accepted stable mode. Strict exact comparison remains dirty with `diffDirtyCount=9` and `acceptedLiveComparisonCount=0`, but is now diagnostic. Verified by prior R298 full regressions, R300 packet-trace bin 15/15 and `fmt --check`, and the R301 acceptance-pack refresh (`mir2-game-data` 27/27, `mir2-gateway` 55/55 plus packet-trace bin 15/15, `mir2-admin-api` 22/22, `mir2-simulation` 674/674).
- `Candidate integration`: R304 restores current-map Crystal NPC population for saved-character start and Crystal transfer snapshots. Live WS evidence for `QA0429A / QA0429Hero` at Bichon `0:284,607` records 8 NPCs around the player, closing the backend/runtime cause of the "no NPCs in web aligned Bichon snapshot" visual gap. This is not a new packet-acceptance claim and not full visual 1:1 acceptance.
- `Candidate integration`: R305 restores current-map visible Crystal respawns for saved-character start and Crystal transfer snapshots. Live WS/browser evidence for `QA0429A / QA0429Hero` at Bichon `0:284,607` records 8 monsters around the player, including Deer and Royal_Guard, closing the backend/runtime cause of the first "no animals/guards in web aligned Bichon snapshot" visual gap. This is not a new packet-acceptance claim and not full visual 1:1 acceptance.
- `99.70%`: Windows real Crystal server-data import is no longer blocked for the current map/drop-rule slice: generated manifests were refreshed from local `Server.MirDB` plus `Build/Server/Debug/Envir/Routes`, map records carry `NoThrowItem`, `NoDropPlayer`, and `NoDropMonster`, and runtime manifest-backed `NoThrowItem` / `NoDropMonster` paths remain green. Verified by `mir2-game-data` (22/22), focused `no_drop_monster_map_rule` (2/2), full `mir2-simulation` (670/670), and `mir2-gateway` (55/55 plus packet-trace bin 7/7).
- `99.70%`: Runtime interaction quest hints now use `custom.interaction.questHint` instead of the backend/runtime `sim.questHint`; generated localization bundles and importer are in sync, and `apps/simulation/src/runtime.rs` has no `sim.*` references. Verified by no-match runtime grep, `mir2-game-data` (22/22), focused snapshot test (1/1), `fmt --check`, and full `mir2-simulation` 664/664.
- `99.70%`: No-script/no-page NPC interaction now follows Crystal's silent `NPCScript.Call` no-response behavior instead of opening runtime-only idle dialog text. Verified by focused no-script NPC (1/1), adjacent `npc_interaction` (2/2), broad `crystal_npc` (52/52), `fmt --check`, and full `mir2-simulation` 664/664.
- `99.70%`: Quest-required drop feedback now uses Crystal `server.YouFound` and no longer emits runtime-only `sim.youSecuredQuestItem`, `sim.questReturnForReward`, or `sim.questProgressWasps` progress chats; `GainedItem` and quest state updates are preserved. Verified by focused quest-required drop (1/1), adjacent `quest_required_drop` (3/3), `fmt --check`, and full `mir2-simulation` 664/664.
- `99.70%`: Start-game welcome chat now uses Crystal `server.Welcome` with localized `server.GameName` and `Hint` chat type instead of runtime-only `sim.welcomeCharacter` System text. Verified by focused simulation/gateway `start_game_emits_bootstrap_sequence` (1/1 each), `fmt --check`, full `mir2-simulation` 664/664, and full `mir2-gateway` 47/47.
- `99.70%`: Normal `ClientPacket::Chat` now follows Crystal `Player.Chat` packet surface: pre-start normal chat is silent, and in-game normal chat emits only `ObjectChat` with `Name: message` instead of runtime-only `sim.echoChat` self `Chat` echo. Verified by simulation `chat_` (43/43), gateway `chat_` (2/2), `fmt --check`, full `mir2-simulation` 664/664, and full `mir2-gateway` 47/47.
- `99.70%`: High-level cast-skill failure paths no longer emit runtime-only `sim.skillNotKnown`, `sim.skillCooldown`, `sim.skillLogicNotWired`, `sim.playerNotInWorld`, or `sim.notEnoughMp`; successful buff/summon behavior remains intact. Verified by `casting` (9/9), `fmt --check`, and full `mir2-simulation` 663/663.
- `99.70%`: `MoveItem` unsupported-grid/missing-source fallback no longer emits runtime-only `sim.itemNotFoundInBag`; unsupported grids remain failed-ack only and Inventory/Storage missing-source keeps Crystal `server.ItemMoveErrorReport`. Verified by `move_item` (26/26), `fmt --check`, and full `mir2-simulation` 660/660.
- `99.70%`: Stale active NPC dialog missing-NPC/no-script handling no longer emits runtime-only `sim.targetNotGroundDrop` or `sim.npcNoMilestoneScript`, while ordinary no-script NPC idle fallback remains intact. Verified by focused stale-dialog tests (2/2), `npc_interaction` (2/2), broad `crystal_npc` (52/52), `fmt --check`, and full `mir2-simulation` 660/660.
- `99.70%`: NPC dialog helper no-active-dialog, invalid-target, and no-pending-input handling no longer emits runtime-only `sim.npcNoMilestoneScript` or `sim.itemNoActiveUse`, while successful dialog link/input/service flows remain intact. Verified by focused dialog-helper tests (3/3), `npc_interaction` (2/2), broad `crystal_npc` (52/52), `fmt --check`, and full `mir2-simulation` 658/658.
- `99.70%`: High-level direct NPC interaction invalid target/direction/range handling no longer emits runtime-only `sim.targetNoScriptedInteraction`, `sim.noValidInteractionDirection`, or `sim.moveCloserToTalkToNpc`, while successful NPC dialog/script/service flows remain intact. Verified by focused direct-interact tests (3/3), `npc_interaction` (2/2), broad `crystal_npc` (52/52), `fmt --check`, and full `mir2-simulation` 655/655.
- `99.70%`: High-level direct attack invalid target/state/range handling no longer emits runtime-only `sim.targetCannotBeAttackedYet`, `sim.monsterAlreadyDown`, `sim.noValidAttackDirection`, or `sim.targetOutOfRangeApproachFirst`, while normal attack, turn, hidden reveal, Zuma wake, and delayed hit surfaces remain intact. Verified by focused direct-attack tests (4/4), hidden/Zuma focused tests (2/2), adjacent `attack` (80/80), `fmt --check`, and full `mir2-simulation` 652/652.
- `99.70%`: Successful high-level NPC interaction no longer emits runtime-only `sim.talkingToNpc`, while NPC `ObjectChat`/dialog packet surfaces and Crystal NPC script/service flows remain intact. Verified by focused `npc_interaction` (2/2), `crystal_npc_dialog` (1/1), `crystal_npc_service` (1/1), broad `crystal_npc` (52/52), `fmt --check`, and full `mir2-simulation` 648/648.
- `99.70%`: Direct high-level ground-drop pickup invalid target/distance handling no longer emits runtime-only `sim.itemNoLongerOnGround`, `sim.targetNotGroundDrop`, or `sim.moveCloserToPickItem`, while Crystal owner/full-bag pickup messages and current-cell packet pickup behavior remain intact. Verified by focused direct-pickup tests (3/3), adjacent `pickup` (18/18), `drop` (42/42), `fmt --check`, and full `mir2-simulation` 648/648.
- `99.70%`: Missing defeated-monster entity handling no longer emits runtime-only `sim.defeatedMonsterEntityMissing`, while normal death/drop packet surfaces remain intact. Verified by focused missing-entity silent test (1/1), visible death packet test (1/1), adjacent `drop` (41/41), `fmt --check`, and full `mir2-simulation` 645/645.
- `99.70%`: Monster death drop success paths no longer emit runtime-only `sim.monsterDroppedGoldOnGround` / `sim.monsterDroppedItem` chats while preserving ground gold/item drops, quest-drop routing, owner windows, and pickup packet surfaces. Verified by focused item-drop no-chat (1/1), focused gold-drop no-chat/pickup (1/1), adjacent `drop` (41/41), `pickup` (15/15), `attack` (76/76), `fmt --check`, and full `mir2-simulation` 644/644.
- `99.70%`: Summoned VampireSpider death explosion no longer emits runtime-only `sim.targetDefeated` defeat chat while preserving explosion damage, summon despawn timing, and delayed health packet surfaces. Verified by focused vampire-spider no-chat explosion test (1/1), adjacent `spider` (6/6), `attack` (76/76), `fmt --check`, and full `mir2-simulation` 643/643.
- `99.70%`: Ordinary combat hit resolution no longer emits runtime-only damage narration (`sim.youHitTargetForDamage`, `sim.targetDefeated`, `sim.monsterPressuresYouForDamage`), while packet health/struck/death surfaces and Trainer DPS reporting remain intact. Verified by focused player-hit no-chat test (1/1), adjacent `attack` (76/76), `fmt --check`, and full `mir2-simulation` 643/643.

Imported-slice bar: `[####################]`

Remaining to full 1:1: Stage 4/5 depth work, especially broad AI, full skill/buff tables, imported item economy, exact packet behavior, live Crystal comparison, and long-duration production operations.

## Weighted Breakdown

| Area | Weight | Current | Weighted Progress | Notes |
| --- | ---: | ---: | ---: | --- |
| world runtime and map rules | 14% | 47% | 6.6% | starter region collision, imported respawns, route patrol, guard route handling, basic map bounds, and current config-backed map-rule handling for `NoThrowItem` / `NoDropMonster` / `NoTownTeleport` / `NoReincarnation` are in; full map import is not |
| player movement, combat, death, revive | 16% | 72% | 11.5% | movement/run/walk, player attack events, delayed player melee hit resolution, player `ObjectStruck`, monster `Struck`, delayed monster hit resolution, distance-scaled ranged hit timing, current guard-style melee packet shape, and bounded `ResurrectionScroll` revive/map-rule behavior are in; combat parity is still far from complete |
| monster AI, respawn, drops | 18% | 99% | 17.8% | Crystal respawn import, default `MonsterObject` imported template movement/chase/melee/respawn/drop/packet baseline, spread, delay, route, neutral guards, guard target selection, player target lock, guard hostile-kill baseline, guard back-tile melee packet shape, archer-guard immobility, `SpittingSpider` line-range attacks, `SandWorm` two-tile line attack plus harvest-corpse baseline, `CrystalSpider` three-tile type-1 line attack plus green-poison baseline, `KingScorpion` two-tile line/range MC baseline, `DarkDevil` three-tile DC*3 ranged burst baseline, `IncarnatedZT` active DC/paralysis melee baseline, `OmaKing` seven-tile type-1 MC baseline, `GreatFoxSpirit` static seven-tile DC ranged baseline, `Master_DragonYang` / ManectricKing three-tile MC line baseline, `SeedingsGeneral` two-tile MC ranged baseline, `RestlessJar` static zero-MC ranged packet baseline, `HellKeeper` static view-range locked-facing DC baseline, `GeneralMeowMeow` twelve-tile MC ranged baseline, `TucsonGeneral` rage packet plus delayed rock `ObjectSpell` scatter and type-1 SC ranged baseline, `TrapRock` hidden reveal and no-damage range baseline, `Armadillo` DigOut spell/reveal plus primary DC melee baseline, `ArmadilloElder` DigOut spell/reveal plus primary DC*2 melee baseline, `BoneLord` seven-tile ranged DC baseline, `MinotaurKing` six-tile ranged DC baseline, `Chieftain_Priest` / ManectricClaw three-tile thrust baseline, `MirStatue` static delayed ranged DC baseline, `GuardianRock` static immune range-pull packet baseline, `RedMoonEvil` static view-range DC baseline, `EvilCentipede` hidden/static DC baseline, `Yimoogi` seven-tile ranged DC baseline, `Lamia` / Kirin two-tile DC baseline, `Khazard` four-tile pull-packet baseline, `SandSnail` primary DC melee baseline, `Hen` / `Pig` / `Bull` passive two-skin-pass harvest with follow-up pending-drop transfer, `Deer` / `Deer1` / `Sheep` passive five-skin-pass harvest plus follow-up pending-drop transfer and run-away flee baseline, `HolyDeva` / `PKSpirit` six-tile range plus fear-kiting baseline, `RedThunderZuma` / `Ancient_RedThunderZuma` / `Frozen_RedZuma` nine-tile ranged Zuma baseline, `ZumaTaurus` stoned wake plus DC melee baseline, `FrostTiger` passive six-tile ranged baseline, `IceCrystalSolider` / `SinseokMiner` IceGuard eight-tile ranged baseline, `FrozenMiner` primary 600 ms DC attack baseline, `FrozenAxeman` two-tile type-1 DC*2 baseline, `FrozenMagician` nine-tile MC ranged baseline, `SnowWolf` primary 350 ms DC attack baseline, `FrozenWarewolf` SnowWolfKing primary 500 ms DC baseline, `SnowYeti` nine-tile DC ranged baseline, `DarkWraith` four-tile type-2 DC*3 baseline, `TucsonMage` three-tile zero-MC type-1 baseline, `TucsonWarrior` two-tile type-1 MC smash baseline, `CannibalPlant` hide/show state, `CannibalTentacles` non-adjacent view-range `ObjectRangeAttack` plus MC damage baseline, `Jar1` static one-tile/death-spawn baseline, `Jar2` static six-tile `ObjectRangeAttack` plus zero-MC no-damage gating baseline, `TurtleGrass` Zuma-style stone/wake plus two-tile DC attack baseline, `ManTree` / `FineSoul` Zuma-style stone/wake plus zero-DC attack-packet baseline, `CaveMaggot` DC-based melee plus paralysis poison baseline, `ToxicGhoul` DC-based melee plus green-poison status baseline, `ThunderElement` delayed two-tile area attack plus normal-damage immunity baseline, `DarkBeast` / `CatWidow` primary DC-based melee baseline, `FlamingWooma` 300 ms `ObjectAttack` plus DC damage baseline, `HedgeKekTal` near-vs-range `ObjectRangeAttack` plus DC damage baseline, `Trainer` static passive non-dying target-dummy baseline, `WoomaTaurus` FlamingWooma melee plus mad speed and surrounded-teleport baseline, `HarvestMonster` corpse harvest/drop semantics now persist pending `_drops`, transfer on the next harvest call, preserve leftovers across full-bag retries before `ObjectHarvested`, attach current-player harvest ownership on defeat, skip non-owner/non-group corpses during the Crystal front-centered scan, emit `NoNearbyOwnedCarcasses` only when no eligible corpse is found, and now suppress current death/quest/harvest loot on maps flagged `NoDropMonster`, `TucsonEgg` immobile one-HP damage and delayed death hook baseline, `Tree` static neutral/passive one-HP damage baseline, `DigOutZombie` hidden/reveal state, `RevivingZombie` delayed reduced-HP revival, `AxeSkeleton` range plus fear-window pressure, `ZumaMonster` stone/wake state, `BoneSpearman` two-tile line pressure, `RightGuard` / `LeftGuard` near-vs-range attack switching, `ShamanZombie` six-tile range/line pressure, `BlackFoxman` type-1 line attack, `RedFoxman` / `WhiteFoxman` six-tile ranged pressure, `WaterDragon` / `BlackTortoise` non-adjacent ranged pressure, `BlackHammerCat` / `StrayCat` / `CatShaman` packet-visible attack baselines, `YinDevilNode` / `YangDevilNode` immobile support-node baseline, HellFire `HellKnight` / `HellLord` / `HellBomb` baseline behavior, special-AI respawn state resets, player-friendly summon target selection, hostile-monster retargeting onto friendly summons, trap-priority hostile targeting, and DB-backed dynamic summon template import for `BugBat` / `BombSpider` / `Shinsu` / `StoneTrap` plus current `RootSpider -> BombSpider` and `SnakeTotem -> CharmedSnake` summon-body parity are in; wider AI/attack parity is not |
| inventory, equipment, item use, repair | 10% | 90% | 9.0% | Crystal item DB rows, item action acks, `UserItem` payloads, drop placement, stack/slot gain checks, `GROUP` drops, random-stat payloads, seal/socket/reseal metadata, buy-back/used-goods persistence, pickup/drop/gold packet behavior, and repair/durability baselines are in. Current `SellItem` requires active Crystal sell pages, enforces `DontSell` and script `[Types]`, uses `UserItem.Price() / 2` style sale value, preserves partial-stack overflow rejection and full-stack gold-cap clamping, keeps buy-back/used-goods persistence, and now resolves current bag items by Crystal-style unique id instead of raw slot aliases. Current packet `UseItem`, packet `EquipItem`, and `MergeItem` now also resolve the exact referenced current item by unique id instead of duplicate-key fallback or slot aliases, so duplicate-key items on different bag pages no longer mutate the wrong stack or equipment candidate. Current `EquipItem(grid=Storage)` now also resolves the exact current storage item through the active `@Storage` service and swaps replaced equipment back into the exact source slot, while current `RemoveItem(grid=Inventory|Storage)` now follows Crystal's exact destination-slot semantics with ack-only packet shape instead of accepting `grid=Equipment` or falling back into another bag slot. Current `RemoveSlotItem` now also keeps Crystal's bounded source-grid envelope for the modeled runtime, so invalid `grid=Equipment` requests and unmodeled `Mount` / `Fishing` / `Socket` slot-item requests ack-fail without falling through into whole-equipment removal when the packet ids only match the parent equipment. Current `UseItem` now also matches the bounded Crystal dead-state / scroll map-rule surface: ordinary items fail while dead, alive `ResurrectionScroll` emits `CannotResurrection`, dead `ResurrectionScroll` revives only on allowed maps, `TownTeleport` respects `NoTownTeleport`, `UseItem(grid=HeroInventory)` no longer falls back into player bag items, successful modeled use-equip no longer emits runtime-only `sim.equippedItem*` chat, non-inventory equipment-use failure is ack-only, unusable inventory fallback is chat-free, and missing-item/invalid-source `UseItem` now fails without `sim.itemNotFoundInBag`, unmodeled `UseItem(grid=HeroInventory)` returns a failed ack instead of empty packets, and missing-source `DropItem` fails without `sim.itemNotFoundInBag`. Current `SplitItem(grid=HeroInventory)` now likewise failed-acks without mutating matching player bag stacks. Current `SplitItem` now also follows Crystal single-array placement across local `Bag1` / `Bag2`, prefers eligible belt slots for inventory splits, supports only `Inventory` / `Storage`, requires active Crystal storage service for storage splits, and keeps unsupported/invalid/full/locked failures ack-only. Current `EquipItem(grid=HeroInventory)`, `RemoveItem(grid=HeroInventory)`, and `RemoveSlotItem(grid=HeroEquipment|HeroInventory)` now also failed-ack without mutating matching player inventory/equipment. Current `MergeItem` hero-grid requests now likewise failed-ack without extra chat or player-bag mutation, now also cover unsupported `Equipment` / `Fishing` / `Trade` / `Refine` / `QuestInventory` grids ack-only, keep current Inventory/Storage same-grid failures and success paths chat-free like Crystal, support Crystal-style `Inventory <-> Storage` plus modeled `Inventory <-> Belt` stack merges with the correct current storage-service / ack-only failure guards, keep the remaining unsupported `Storage <-> Belt` cross-grid requests ack-only without runtime-only chat, and now require the recorded storage NPC object to still exist and remain within Crystal `DataRange` for any storage-touching merge. Current `MoveItem(grid=HeroInventory)` now also failed-ack without extra chat or player-bag mutation, unsupported `Belt` / `QuestInventory` / `HeroEquipment` / `Equipment` / `Fishing` requests now ack-fail without extra chat or equipment mutation, storage-lock and invalid-slot failures now stay ack-only like Crystal, current `MoveItem(grid=Storage)` requires the active Crystal `@Storage` service context plus the recorded storage NPC remaining live and in range, successful current Inventory/Storage moves no longer emit runtime-only success chat, and slot-based current `MoveItem`, `StoreItem`, and `TakeBackItem` inventory paths now resolve Crystal single-array indices across local `Bag1` / `Bag2` instead of same-slot page aliases. Current `DeleteItem` now also matches Crystal's server-side `HeroInventory` quirk: the packet flag is ignored and deletion still searches only current player inventory by unique id, so matching bag items are removed while missing hero/player ids remain ack-only. Current `DropItem` now rejects base and rental `DontDrop`, respects current map `NoThrowItem` with localized `CanNotDrop` chat before the failed ack, and keeps the bounded hero-inventory `DropItem` / `CombineItem` packet guards so unavailable hero inventory ack-fails without mutating matching player inventory. Current `BuyItem` packet handling can purchase static trade, buy-back, and used goods with `LoseGold`/`GainedItem`, silently rejects invalid panel/count, no-service, non-buy-service, missing goods/metadata, insufficient-gold, and full-bag requests before mutation, and now also requires the recorded service NPC object to remain live and within Crystal `DataRange` for current buy-family actions. Current `StoreItem` / `TakeBackItem` now require active Crystal `@Storage` service context from the real `NPCStorage` flow, preserve password-lock/capacity/occupied-target rejection order, reject base and rental `DontStore` for store only, keep ack-only failure semantics, and now also reject stale/out-of-range storage service context when the recorded storage NPC disappears or leaves `CRYSTAL_DATA_RANGE`. Current storage password actions now require the active in-range Crystal storage service, enforce Crystal alphanumeric `5..=15` password format semantics, clear `LastSetTime` to `0` on successful removal, reset session unlock on repeated `@Storage` opens, successful current storage open/unlock now emit Crystal `UserStorage` follow-up packets, repeated unchanged current `@Storage` opens now suppress duplicate `UserStorage` after the first send, matching Crystal `Connection.StorageSent`, current storage-open `UserStorage` payloads now retain the full backing storage length even when expanded storage is inactive while higher-slot item actions continue to honor current accessible capacity, and expired expanded storage now downgrades inactive on `StartGame` before emitting the Crystal expiry chat plus `ResizeStorage` on the first world tick and persisting the account flag back to `false`. Current inventory-grid `CombineItem` packets now reach the existing shape-1/2/5/6 repair-hammer/sewing, shape-7 socket-growth, shape-8 seal, and bounded shape-3/4 gem/orb upgrade semantics through protocol, gateway, and runtime, including `ItemRepaired`, `ItemUpgraded`, persisted `gem_count`, equipment-backed player `GemRatePercent` success bonus, the corrected current-data `DurabilityGem` / `DurabilityOrb` `MaxDura` branch, focused regression coverage for the current-data durability, attack-speed, and magic-resist families, runtime rental `BindingFlags`, rental `DontUpgrade` rejection for current socket/upgrade branches, the shared Crystal target item-type gate across packet branches, and current inventory unique-id lookup cleanup that removes default `Bag1` / `Bag2` id collisions for current bag-item paths. NPC `SellItem`, `RepairItem`, and `SRepairItem` now likewise require the recorded Crystal service NPC object to remain live and within `CRYSTAL_DATA_RANGE`, while repair still uses current backpack item unique ids, requires the matching active repair service page, applies Crystal cost and normal max-dura loss semantics, and covers non-repairable/type/insufficient-gold rejection edges. Current equipment metadata now preserves `NeedIdentify` and `SoulBoundId` through runtime/item round-trips, auto-identifies items on equip/use-equip, rejects equipping items bound to another character, maps manifest-backed equipment item types to runtime slots, supports right-side explicit equip compatibility for manifest-backed ring/bracelet/amulet items, silently rejects dynamic manifest-backed explicit `EquipItem` attempts that fail Crystal requirements, and dynamic current `UseItem` now routes manifest-backed current-data HP/MP consumables, duration buffs, town teleports, and repair oils from Crystal template stats. Broader hero-inventory handling, imported credit product catalogs, remaining unsupported-grid `CombineItem` branches, and remaining storage economy edges remain. |
| skills, buffs, cooldowns, summons | 10% | 90% | 9.0% | starter skill/buff loop exists, summon skills are now defined in `mir2-game-data`, dynamic summon bodies can now be sourced from Crystal monster data instead of runtime literals, generalized summon metadata now carries Crystal-style visible `extra`, owner binding, timeout / out-of-range self-destruct, delayed dead-body cleanup, and player-friendly hostility/disposition overrides, and current summon-family parity now covers `BugBagMaggot -> BugBat`, `RootSpider -> BombSpider`, `SnakeTotem -> CharmedSnake`, player-cast `SummonShinsu`, `SummonVampire`, `SummonToad`, `SummonSnakes`, and `Stonetrap`; friendly summon-vs-monster combat, `SnakeTotem` minion cap/duration, `VampireSpider` / `CharmedSnake` death explosions, `Shinsu` target-driven mode/hide/show plus two-tile line pressure, and `StoneTrap` trap-priority aggro plus Crystal-style struck immunity are now in. Current buff totals also derive from `BuffState.stats`, so template-driven current-data duration buffs can stack time without resetting the applied stat payload, but edge-case summon timing and full spell table parity are still incomplete |
| NPC scripting and quests | 10% | 100% | 10.0% | starter guide chain now runs through a generic NPC script lookup/quest binding path, idle fallback is covered, Crystal NPC manifests now carry raw script text plus section bodies, runtime can bind a Crystal `script_key` to an NPC and render `@Main` `#SAY` text, links, and simple `#ACT GOTO` flow, and current interpreter now covers `CHECK`, `CHECKCALC`, `SET`, `MOV`, `CALC`, `CHECKCLASS`, `CHECKGENDER` / `GENDER`, `CHECKQUEST ACTIVE/COMPLETE`, `RANDOM`, `CHECKMON`, `CHECKEXACTMON`, `CHECKMAP`, `CHECKMAPLIGHT`, `CHECKRANGE`, `CHECKHUM`, `CHECKCONQUEST`, `CONQUESTOWNER`, `CHECKPERMISSION`, `AFFORDGUARD`, `AFFORDWALL`, `AFFORDGATE`, `HASBAGSPACE`, `HASGT`, `INGUILD`, `CHECKBUFF`, `DAYOFWEEK`, `HOUR`, `MIN`, `ISADMIN`, `GROUPLEADER`, `GROUPCOUNT`, `GROUPCHECKNEARBY`, `PARAM1/2/3`, `MONGEN`, `MONCLEAR`, `GIVEITEM`, `TAKEITEM`, `GIVEGOLD`, `GIVEEXP`, `GIVESKILL`, `GIVEPET`, `REMOVEPET`, `PETCOUNT`, `PETLEVEL`, `CHECKPET`, `LOCALMESSAGE`, `LINEMESSAGE`, `GLOBALMESSAGE`, `LOADVALUE`, `SAVEVALUE`, `GROUPGOTO`, `GROUPTELEPORT`, `CLEARPETS`, conquest/guild-territory actions, hero revive/seal actions, hair/level/buff/name-list actions, and timed-recall baselines; reserved Crystal service labels now open baseline service packet surfaces for buy, buy/sell, sell, repair, special repair, craft, refine, refine-check, wedding-ring replacement, and storage pages; parameter-page labels like `@Guess(1)`, `%ARG(n)`, embedded `%A1`, `<$OUTPUT(A1)>`, and input-driven `%INPUTSTR` now execute through the runtime plus gateway/web submission flow, NPC value storage persists through character saves, group/event branches now run against configured runtime members/state instead of stubs, and generated command coverage now reports 81/81 Crystal NPC command names and 7,044/7,044 occurrences covered |
| map transfer, AOI, safe zones, world events | 8% | 100% | 8.0% | AOI filtering preserves spawn-before-action ordering and same-tick tracked-object packets, hidden/visible monster state drives packet-visible show/hide, summon-body extra presentation is decoupled from Zuma wake logic, starter map-transfer/safe-zone state is modeled with `MapInformation`/`UserLocation` refresh plus snapshot safe-zone flags and saved current-map state, current event-script parity includes current-map `CHECKMAP`/`CHECKRANGE`/`CHECKHUM`, basic real-time `DAYOFWEEK`/`HOUR`/`MIN`, current-map `MONCLEAR`, configured conquest-state checks, group teleport flow across the current runtime-visible party model, and Stage 5 conquest/event state can start/end wars, assign castle ownership, and spawn runtime monsters |
| persistence and reconnect-safe state | 8% | 100% | 8.0% | shared account/character store is wired through simulation config, login/new-character/start-game/log-out use account state, gateway saves active character state after Web/TCP commands and socket-close paths, optional JSON-backed account storage survives fresh config/process reloads, NPC flag state is part of character saves, legacy save JSON loads without the new NPC fields, current save records persist player experience plus NPC `SAVEVALUE` / `LOADVALUE` key-value state and Stage 5 broad-system state across reloads, and the JSON account store now has `schemaVersion`, legacy migration, corrupt-source fallback, atomic temp-file replacement, and backup/restore APIs |
| protocol behaviour parity | 6% | 100% | 6.0% | gateway/session basics plus `ObjectAttack`, `ObjectRangeAttack`, `ObjectHarvest`, `ObjectHarvested`, item action acks, `CombineItem`, `ItemUpgraded`, `UserItem` serialization, split/gained/refresh item payload packets, item metadata request/response packets, delete-item packets, gold and credit delta packets, item slot/seal state packets, NPC service/craft packets, sell/repair entry packets, `DuraChanged`, `ItemRepaired`, `Struck`, `ObjectStruck`, `ObjectShow`, `ObjectHide`, imported monster AI in visible packets, delayed hit resolution for player and monster attacks, guard back-tile melee attack packets, ordinary-monster attack-without-preturn flow, Zuma wake-show packets, Shinsu show/hide mode packets, RightGuard/LeftGuard near-vs-range packet switching, improved AOI packet ordering/filtering, NPC input submission routed through gateway/web dialog snapshots, `UserInformation` experience fields reflecting persisted runtime state, stable packet trace entries, and the local/live TCP packet trace harness are all in place for the current backend target |

## Current Focus

1. maintain parity as new Crystal-backed content is imported
2. keep regression coverage current when runtime assumptions expand
3. extend beyond parity only under explicit new-scope work
4. Stage 4 expansion: full-map movement/safe-zone metadata is now runtime-backed, and monster AI families are classified for prioritization

## Milestones

- `40%`: imported monster behaviors stop looking "generic"; hostile vs neutral logic, player target lock, route patrol, guard target selection, struck/object-struck combat packets, special hide/show state, and packet-visible attack events are in place
- `50%`: item/equipment pipeline and AOI/map-transfer behavior are much closer to Crystal; starter-to-mid gameplay loop becomes less custom-scripted
- `55%`: summon-body packet presentation and lifetime rules stop being one-off hacks; Crystal-style summon ownership, timeout, range self-destruct, and delayed cleanup are shared runtime capabilities
- `60%`: broader quest/NPC runtime and more spell/buff behavior are in place; backend is no longer only a starter-scene parity slice
- `62%`: player summon spell entry points are no longer stubbed; friendly summon spawning, recall, visible `extra`, and ownership/disposition are part of the shared runtime
- `67%`: player archer summon family now has meaningful combat semantics in runtime, including friendly `SnakeTotem -> CharmedSnake`, `SpittingToad` ranged pressure, `VampireSpider` death explosion, summon skill-level minion cap/duration, and hostile-monster retargeting onto friendly summons
- `70%`: deeper summon/trap parity is in place: `Shinsu` is target-driven for show/hide and line pressure, `StoneTrap` has trap-priority hostile selection and Crystal-style struck immunity, and current summon death explosions resolve through delayed combat
- `73%`: equipment durability and repair now affect real backend state: Crystal-style weapon/non-weapon durability loss hooks are wired, broken gear stats are suppressed, repair powder restores equipped durability, and localization/test coverage is in place
- `73.5%`: current durability and repair flows now emit Crystal-shaped `DuraChanged` / ID 76 and `ItemRepaired` / ID 114 packets through protocol, runtime, and gateway conversion
- `74%`: current item packet grid/equipment slot values match Crystal for active paths, and current move/equip/remove/split/merge/drop/use/store/take-back flows emit Crystal action ack packets
- `74.5%`: current gold pickup/drop flows emit Crystal `GainedGold` / ID 67 and `LoseGold` / ID 68 packet updates
- `74.8%`: current sell/repair flows emit Crystal `SellItem` / ID 111 and `RepairItem` / ID 113 entry packets, and successful sell emits the paired `GainedGold` update
- `74.9%`: current inventory delete requests support Crystal client `DeleteItem` / ID 149 and server `DeleteItem` / ID 79 packet behavior
- `74.95%`: reusable Crystal `UserItem` serialization and current split-stack `SplitItem` / ID 44 payload packets are in place
- `74.98%`: current inventory pickup updates emit Crystal `GainedItem` / ID 66 payload packets
- `75%`: reconnect-safe character state now has a shared account-store baseline: characters, selected character saves, position, direction, HP/MP, gold, inventory, belt, equipment, quests, and skill cooldowns survive new sessions and gateway command boundaries
- `75.2%`: Crystal `Server.MirDB` item rows generate into `crystal_item_manifest.json`, and mapped starter `UserItem.item_index` values resolve through real Crystal item indices
- `75.3%`: Crystal `RefreshItem` / ID 148 payload packet support is available in protocol and gateway JSON
- `75.4%`: Crystal `RequestItemInfo` / ID 39 and `NewItemInfo` / ID 32 packet behavior is backed by the imported Crystal item manifest
- `75.5%`: current item gain and merge flows enforce imported Crystal `StackSize` for mapped stackable and non-stackable items
- `75.7%`: current pickup/shop/auction/NPC/quest item grants use StackSize-aware full-bag checks before mutating drops, gold, listings, quest state, or inventory
- `75.8%`: Crystal `GainedCredit` / ID 69 and `LoseCredit` / ID 70 packet support is available in protocol, trace names, and gateway JSON
- `75.85%`: current Crystal `CreditToken` scroll use mutates saved account credit, updates `UserInformation.credit`, and emits `GainedCredit`
- `75.88%`: current credit-shop purchase flow emits `LoseCredit` and preserves credit/items on insufficient-credit failures; later R21 work changed full-bag handling to Crystal game-shop-style mail delivery
- `75.9%`: Crystal `ItemSlotSizeChanged` / ID 115 and `ItemSealChanged` / ID 116 packet support is available in protocol, trace names, and gateway JSON
- `75.92%`: current `BenedictionOil` weapon Luck success use emits Crystal `RefreshItem` for the equipped weapon payload
- `75.95%`: current socket-slot growth updates equipped item socket state and emits Crystal `ItemSlotSizeChanged`
- `75.98%`: current item sealing records equipped-item expiry state and emits Crystal `ItemSealChanged`
- `76%`: Crystal NPC service/craft packet support is available in protocol, trace names, and gateway JSON, including `NPCGoods` with shared `UserItem` serialization and f32 service rates
- `76.2%`: current imported Crystal NPC service labels emit baseline runtime service-open packets for storage, buy/sell, repair, special repair, craft, refine, refine-check, and wedding-ring replacement pages
- `76.4%`: current buy/buy-sell/craft `NPCGoods` packets include manifest-backed goods parsed from imported Crystal NPC `[Trade]` / `[Recipe]` sections
- `76.45%`: current Crystal `RepairOil` and `WarGodOil` scroll use repairs the equipped weapon and emits `ItemRepaired`
- `76.5%`: current NPC buy/repair/special-repair service packets use imported Crystal `NPCInfo.Rate / 100F`
- `76.55%`: current buy/buy-sell/buy-used `NPCGoods` flags honor Crystal `GoodsHideAddedStats`, and empty buy-back panels no longer reuse static trade goods
- `76.6%`: current sell-service actions populate per-NPC buy-back `NPCGoods` with Crystal `UserItem` payloads and the 20-item cap
- `76.65%`: Crystal `BuyItem` / ID 51 can purchase current static trade and buy-back goods with imported price/rate, `LoseGold`, and `GainedItem`
- `76.7%`: current `SellItem` supports partial stack sales and imported Crystal `ItemInfo.Price / 2` sale gold
- `76.75%`: current runtime death and harvest rewards prefer imported Crystal `MonsterInfo.DropPath` drop tables before starter fallback, including grouped-section chance rolls, Crystal item metadata-backed harvest items, and imported gold drops
- `76.76%`: imported Crystal `Gold N` drop entries now use Crystal's `N / 2` inclusive through `N + N / 2` exclusive amount range instead of fixed table amounts
- `76.77%`: current harvest reward transfer emits Crystal `GainedItem` packets for item rewards before `ObjectHarvested`, and harvest transfer is item-only like Crystal `HarvestMonster`
- `76.78%`: imported Crystal item drops carry base durability into `UserItem`, and harvest meat applies Deer quality durability bonuses
- `76.79%`: monster death drops now carry Crystal-style pickup owner windows with expiry and configured group-member bypass
- `76.80%`: imported `ShowGroupPickup` metadata now emits Crystal-style grouped pickup system notices for marked items
- `76.81%`: current pickup and harvest reward transfer preserve drops when slot/stack capacity cannot accept an item; a later Crystal source audit clarified bag weight is not a pickup/harvest rejection gate
- `76.82%`: imported item drops now use Crystal `CreateDropItem` current-durability rolls before harvest meat quality and future random-stat upgrades
- `76.83%`: manifest-backed `UserItem.Identified` payloads now follow Crystal `NeedIdentify` for current gained, pickup, harvest, and equipment-refresh paths
- `76.84%`: player `PickUp` now uses Crystal current-cell semantics and no longer collects adjacent ground drops
- `76.85%`: ground drops now despawn on Crystal `ItemTimeOut=30` minute expiry and emit normal AOI removal
- `76.86%`: monster gold ground drops now split by Crystal `MaxDropGold=2000` chunking before pickup
- `76.87%`: ground gold pickup now preserves drops when `CanGainGold` would exceed Crystal's `uint.MaxValue` gold cap
- `76.88%`: player `DropGold` now matches Crystal zero-gold and insufficient-gold packet behavior
- `76.89%`: ground `ObjectItem` packets now expose imported Crystal grade and grade name-colour metadata
- `76.90%`: player `DropItem` now follows Crystal stack-count splitting, invalid-request failure acks, manifest-backed `DontDrop` rejection, and `DestroyOnDrop` delete-without-ground-object behavior
- `76.91%`: current Crystal `AddItem` gains now merge belt stacks, prioritize player potion/scroll/script and amulet belt slots before bag fallback, and consume referenced belt slots through `UseItem`
- `76.92%`: current ground item placement now follows Crystal `ItemObject.Drop(distance)` ring search, transfer-source rejection, and `DropStackSize=5` object-count limits for player item/gold drops and monster ground drops
- `76.93%`: Crystal quest-drop `Q` entries now roll normally, attempt active matching quest-inventory gain, suppress ground fallback when not needed or full, and share the gate across current death and harvest drop paths
- `76.94%`: current drop-created Crystal items now apply MaxDura, MaxAC, and MaxDC random-stat baselines from `random_stats_id` profiles and preserve added stats through pickup/harvest `GainedItem` payloads
- `76.95%`: current added-stat ground item drops now expose Crystal Cyan name-colour metadata through `ObjectItem` packets and world snapshots
- `76.96%`: current NPC buy-back entries now persist across save/reload, expire after Crystal `GoodsBuyBackTime=60`, move into used goods, and used goods can be bought back and persisted
- `76.97%`: current socket-slot growth now rejects items at imported socket capacity and only emits `ItemSlotSizeChanged` on successful capacity-backed growth
- `76.98%`: current seal flow now rejects already-sealed equipment without overwriting expiry and only emits `ItemSealChanged` on first active seal
- `76.99%`: current BenedictionOil can follow Crystal-shaped Luck, curse, and no-effect branches while consuming the oil for true outcomes
- `77.00%`: current seal flow validates optional source items against Crystal `Gem` shape-8 seal-source rules and consumes the source only on successful sealing
- `77.01%`: current socket-slot growth validates optional source items against Crystal `Gem` shape-7 socket-source and `ValidGemForItem` unique-flag rules, then consumes the source only on successful growth
- `77.02%`: current seal flow stores Crystal `SealedInfo.NextSealDate`, rejects reseal before `Settings.ItemSealDelay` has elapsed after expiry, serializes next-seal metadata, and preserves the field across save/reload with legacy defaults
- `77.03%`: current drop-created Crystal items now roll the full current Jev random-stat family baseline, carrying generic `UserItemStat` ids, curse flag, and socket slots through ground drops, pickup/harvest `GainedItem`, inventory/equipment state, and save/reload; the full `mir2-simulation` regression suite is green
- `77.04%`: generated `RandomItemStats.ini` manifest data now drives the current random-stat profile lookup, removing the remaining hardcoded runtime profile table while keeping full random-stat payload, game-data, drop, item, and full simulation regressions green
- `77.05%`: generated drop manifests and runtime resolution now preserve and execute nested Crystal `GROUP` semantics, including `GROUP*` random successful item selection, `GROUP^` first-success short-circuiting, and child gold accumulation; the full `mir2-simulation` regression suite is green
- `77.06%`: Crystal source audit confirmed normal owned item/gold drops are visible immediately and owner windows restrict pickup only; current `PickUp` now skips owner-blocked/full-bag/gold-cap candidates while still collecting later pickable current-cell drops, and pickup/harvest no longer reject gains solely because bag weight would exceed the movement limit
- `77.07%`: Crystal `HarvestMonster` rewards now materialize into pending `_drops` after the skin count reaches zero, transfer on the next harvest call, preserve untransferable leftovers across full-bag retries, and avoid re-rolling pending harvest rewards; the full `mir2-simulation` regression suite is green
- `77.08%`: Crystal harvest corpse ownership is now modeled for current harvest monsters: defeat attaches current-player ownership, harvest scanning skips non-owner/non-group corpses, grouped owners can harvest, and owner-blocked-only searches emit `NoNearbyOwnedCarcasses`; the full `mir2-simulation` regression suite is green
- `77.09%`: Crystal economy rejection coverage now includes active sell-service gating, partial-stack sell gold-cap rejection, credit-shop mail delivery without full-bag blocking, and mail attachment claim capacity checks; the full `mir2-simulation` regression suite is green
- `77.10%`: Crystal `BuyItem` rejection coverage now matches silent no-mutation behavior for invalid panel/count, missing active service, non-buy service pages, missing goods/metadata, insufficient gold, and full bags; the full `mir2-simulation` regression suite is green
- `77.11%`: Crystal NPC `RepairItem` / `SRepairItem` now follow backpack unique-id lookup, matching `@Repair` / `@SRepair` active-page gating, Crystal repair/special-repair cost, normal max-durability loss, special-repair max preservation, repairability/type rejection messages, and insufficient-gold silent return; the full `mir2-simulation` regression suite is green with 453 tests
- `77.12%`: Crystal NPC `SellItem` now follows `DontSell` and script `[Types]` rejection behavior, ack-only failure branches, Crystal `UserItem.Price() / 2` sale value, partial-stack gold overflow rejection, and full-stack gold-cap clamping; the full `mir2-simulation` regression suite is green with 457 tests
- `77.13%`: Crystal NPC `StoreItem` / `TakeBackItem` now follow active `@Storage` / `NPCStorage` service gating, `DontStore` store-only rejection, password-lock/capacity/occupied-target no-swap behavior, and ack-only failure semantics; the full `mir2-simulation` regression suite is green with 458 tests
- `77.14%`: Crystal inventory-grid `CombineItem` now has real client/server packet ids, codec, gateway JSON exposure, and runtime dispatch for the current shape-7 socket-growth and shape-8 seal branches; the full `mir2-simulation` regression suite is green with 461 tests, while full target-type/hero-inventory/other combine branches remain open
- `77.15%`: Crystal inventory-grid `CombineItem` now covers the bounded shape-3/4 gem/orb upgrade branch with `ItemUpgraded`, persisted `gem_count`, max-added-stat rejection, invalid-combination rejection, and destroy-on-failure behavior; the full `mir2-simulation` regression suite is green with 465 tests, while full target-type/hero-inventory/belt-id/rental-upgrade/player-rate gaps remain open
- `77.16%`: Crystal packet `CombineItem` now enforces the shared top-level target item-type gate across socket/seal/upgrade handling, so non-equipment targets ack-fail before branch-specific hints or mutations; the full `mir2-simulation` regression suite is green with 466 tests, while hero-inventory/belt-id/rental-upgrade/player-rate gaps remain open
- `77.17%`: Crystal packet `CombineItem` now also covers repair-hammer/sewing source shapes `1/2/5/6`, including `DontRepair` and wrong-family ack-only failures, `ItemNoRepairNeeded` hint rejection, `ItemRepaired`, and repair-combine durability mutation; the full `mir2-simulation` regression suite is green with 469 tests, while hero-inventory/belt-id/rental-upgrade/player-rate gaps remain open
- `77.18%`: runtime item/equipment state now preserves rental `BindingFlags` into `UserItem.RentalInformation`; Crystal storage rejects rental `DontStore`, and current socket/upgrade `CombineItem` branches reject rental `DontUpgrade` ack-only; the full `mir2-simulation` regression suite is green with 472 tests, while hero-inventory/belt-id/player-rate gaps remain open
- `77.19%`: current inventory-grid `CombineItem` shape-3/4 upgrade success chance now adds equipment-backed player `GemRatePercent` from non-broken equipped item stats, matching Crystal's `Stats[Stat.GemRatePercent]` success-rate hook; the full `mir2-simulation` regression suite is green with 473 tests, while hero-inventory/belt-id and other gem-family gaps remain open
- `77.27%`: current inventory unique-id lookup now follows Crystal across `CombineItem`, `SplitItem`, `DeleteItem`, `DropItem`, `SellItem`, and `RepairItem`; runtime fallback ids now distinguish `Bag1` / `Bag2` same-slot items, split-stack clones receive a fresh destination id, and the full `mir2-simulation` regression suite is green with 479 tests
- `77.35%`: current item packet unique-id lookup now also follows Crystal across packet `UseItem`, packet `EquipItem`, and `MergeItem`; duplicate-key bag items no longer mutate the wrong stack/equipment candidate, and `MergeItem` now resolves `Bag1` / `Bag2` items by unique id instead of slot aliases; the full `mir2-simulation` regression suite is green with 482 tests
- `77.36%`: current `DeleteItem` now matches Crystal's server-side `HeroInventory` quirk: the connection drops the flag and deletion still searches only current player inventory by unique id, so matching bag items are removed while missing hero/player ids remain ack-only; the full `mir2-simulation` regression suite is green with 484 tests
- `77.37%`: bounded current hero-inventory packet guards are now regression-locked for `DropItem(hero_inventory=true)` and `CombineItem(grid=HeroInventory)`, so unavailable hero inventory ack-fails without mutating matching player inventory; the full `mir2-simulation` regression suite is green with 486 tests
- `77.38%`: current `DropItem` now also rejects rental `BindingFlags.DontDrop` ack-only like Crystal, preserving inventory state and rental metadata; the full `mir2-simulation` regression suite is green with 487 tests
- `77.39%`: current `DropItem` now also respects map `CurrentMap.Info.NoThrowItem`, emitting localized `CanNotDrop` system chat before the failed ack and preserving inventory/ground state; the full `mir2-simulation` regression suite is green with 488 tests
- `77.40%`: current monster drop handling now also respects map `CurrentMap.Info.NoDropMonster`, suppressing normal monster drops, deterministic field-wasp quest drop, and harvest-corpse loot on blocked maps; the full `mir2-simulation` regression suite is green with 490 tests
- `77.41%`: current dead-state item mutation parity now short-circuits `BuyItem`, `DeleteItem`, `SellItem`, `RepairItem`, `DropItem`, and `CombineItem` without mutation; the full `mir2-simulation` regression suite is green with 496 tests
- `77.42%`: current `UseItem` now matches the bounded Crystal dead-state / `ResurrectionScroll` behavior, including alive `CannotResurrection` and dead-player revive-on-use semantics; the full `mir2-simulation` regression suite is green with 499 tests
- `77.43%`: current `TownTeleport` now respects map `CurrentMap.Info.NoTownTeleport`, emits `NoTownTeleport`, preserves the item, and suppresses teleport on blocked maps; the full `mir2-simulation` regression suite is green with 500 tests
- `77.44%`: current dead-player `ResurrectionScroll` now also respects map `CurrentMap.Info.NoReincarnation`, emitting `CannotUseOnMap`, preserving the item, and suppressing revive packets on blocked maps; the full `mir2-simulation` regression suite is green with 501 tests
- `77.45%`: current `UseItem(grid=HeroInventory)` no longer falls back into matching player bag items while hero inventory is unmodeled; the full `mir2-simulation` regression suite is green with 502 tests
- `77.46%`: current `SplitItem(grid=HeroInventory)` now failed-acks without mutating matching player bag stacks while hero inventory is unmodeled; the full `mir2-simulation` regression suite is green with 503 tests
- `77.47%`: current `EquipItem(grid=HeroInventory)`, `RemoveItem(grid=HeroInventory)`, and `RemoveSlotItem(grid=HeroEquipment|HeroInventory)` now failed-ack without mutating matching player inventory/equipment while hero grids are unmodeled; the full `mir2-simulation` regression suite is green with 506 tests
- `77.48%`: current `MergeItem` hero-grid requests now failed-ack without extra chat or player-bag mutation while hero inventory/equipment are unmodeled; the full `mir2-simulation` regression suite is green with 508 tests
- `77.49%`: current `MoveItem(grid=HeroInventory)` now failed-ack without extra chat or player-bag mutation while hero inventory is unmodeled; the full `mir2-simulation` regression suite is green with 509 tests
- `77.50%`: current `MoveItem` unsupported-grid parity now also covers `Trade` and `Refine` ack-only failures without extra chat or player-bag mutation; the full `mir2-simulation` regression suite is green with 511 tests
- `77.51%`: current `MergeItem` unsupported-grid parity now also covers `HeroInventory`, `HeroEquipment`, `Equipment`, `Fishing`, `Trade`, and `Refine` ack-only failures without extra chat or player-bag mutation; the full `mir2-simulation` regression suite is green with 517 tests
- `77.52%`: current `MergeItem` same-grid Inventory/Storage failure and success message shape now follows Crystal's ack-only surface, including storage-lock, missing-item, mismatched/full-stack, and success paths; the full `mir2-simulation` regression suite is green with 520 tests
- `77.53%`: current `MergeItem` now supports Crystal-style `Inventory <-> Storage` stack merges behind the active storage-service gate and preserves ack-only inactive/locked failures; the full `mir2-simulation` regression suite is green with 523 tests
- `77.54%`: current `MergeItem` now supports the next bounded modeled cross-grid surface via `Inventory <-> Belt` stack merges for Crystal belt-eligible items, keeps non-beltable belt cross-grid requests ack-only, and the full `mir2-simulation` regression suite is green with 529 tests
- `77.55%`: current `MoveItem` unsupported-grid parity now also covers `HeroEquipment`, `Equipment`, and `Fishing` ack-only failures without extra chat or player/equipment mutation; the full `mir2-simulation` regression suite is green with 529 tests
- `77.56%`: current `MoveItem` storage-lock and invalid-slot failures now follow Crystal's ack-only surface without extra chat; the full `mir2-simulation` regression suite is green with 533 tests
- `77.57%`: current `MoveItem(grid=Storage)` now requires the active Crystal storage service, and inactive-service requests fail ack-only without mutating storage items; the full `mir2-simulation` regression suite is green with 534 tests
- `77.58%`: current successful `MoveItem` current `Inventory` and `Storage` paths now follow Crystal's ack-only surface and no longer emit runtime-only `Item slot updated.` chat; the full `mir2-simulation` regression suite is green with 535 tests
- `77.59%`: current missing-source `MoveItem` Inventory/Storage failures now use Crystal's `ItemMoveErrorReport` chat surface before the failed ack instead of `sim.itemNotFoundInBag`; the full `mir2-simulation` regression suite is green with 537 tests
- `77.60%`: current `MoveItem` now rejects `Belt` / `QuestInventory` requests ack-only, enforces current inventory slot bounds, and keeps bag moves from mutating quest items; the full `mir2-simulation` regression suite is green with 542 tests
- `77.61%`: current `MergeItem` now rejects `QuestInventory` requests ack-only without extra chat or quest-item mutation; the full `mir2-simulation` regression suite is green with 544 tests
- `77.62%`: remaining unsupported `MergeItem` `Storage <-> Belt` cross-grid requests now follow Crystal's ack-only surface without runtime-only chat; the full `mir2-simulation` regression suite is green with 546 tests
- `77.63%`: slot-based current `MoveItem`, `StoreItem`, and `TakeBackItem` inventory paths now resolve Crystal single-array indices across local `Bag1` / `Bag2`, including `Bag2` swaps and storage transfers on slots `40+`; the full `mir2-simulation` regression suite is green with 549 tests
- `77.64%`: current `SplitItem(grid=Inventory)` now follows Crystal single-array placement across local `Bag1` / `Bag2`, including belt-first placement for belt-eligible items instead of source-container page scoping; the full `mir2-simulation` regression suite is green with 552 tests
- `77.65%`: current `SplitItem` now matches Crystal's supported-grid and failed-ack surface, so only `Inventory` / `Storage` are live, storage splits require active Crystal storage service, and unsupported/invalid/full/locked failures stay ack-only; the full `mir2-simulation` regression suite is green with 555 tests
- `77.66%`: current storage-family item actions now require the recorded Crystal storage NPC object to still exist and remain within `CRYSTAL_DATA_RANGE`, so stale/out-of-range storage service context now ack-fails across `StoreItem`, `TakeBackItem`, `MoveItem(grid=Storage)`, `SplitItem(grid=Storage)`, and any `MergeItem` touching `Storage`; the full `mir2-simulation` regression suite is green with 557 tests
- `77.67%`: current `BuyItem`, `SellItem`, and `RepairItem`/`SRepairItem` now require the recorded Crystal NPC object to still exist and remain within `CRYSTAL_DATA_RANGE`, so stale/out-of-range service context no longer mutates the implemented current NPC buy/sell/repair item surfaces; the full `mir2-simulation` regression suite is green with 561 tests
- `77.68%`: current inventory-grid `CombineItem` no longer misroutes current-data `DurabilityGem` / `DurabilityOrb` stat `48` control metadata into a fake added stat, so durability upgrades now follow Crystal's `MaxDura` branch and focused regressions lock the current-data durability, attack-speed, magic-resist, and durability-cap surfaces; the full `mir2-simulation` regression suite is green with 565 tests
- `77.69%`: current inventory-grid `CombineItem` current-data coverage now closes the remaining present-data shape-3/4 families and the shape-0 ack-only source surface for the current manifest slice; the full `mir2-simulation` regression suite is green with 571 tests
- `77.70%`: current storage password actions now require the active in-range Crystal storage service context, and successful password removal clears the persisted `LastSetTime` back to `0`; the full `mir2-simulation` regression suite is green with 572 tests
- `77.71%`: current storage password set/unlock/remove now enforce Crystal's `^[A-Za-z0-9]{5,15}$` password format semantics; the full `mir2-simulation` regression suite is green with 574 tests
- `77.72%`: reopening Crystal `@Storage` now resets the session unlock state before deciding whether storage contents can be sent, matching `ResetStorageUnlock()`; the full `mir2-simulation` regression suite is green with 575 tests
- `77.73%`: successful current `@Storage` open now emits Crystal `UserStorage` before `NPCStorage` when storage is available, and successful `UnlockStorage` now emits `StorageUnlockResult` followed by `UserStorage`, through protocol/gateway/runtime with full `mir2-simulation` regression green at 575 tests
- `77.74%`: repeated unchanged current `@Storage` opens now suppress duplicate `UserStorage` after the first send, matching Crystal `Connection.StorageSent` resend behavior while preserving the locked reopen/unlock resend path; the full `mir2-simulation` regression suite is green with 576 tests
- `77.75%`: current `@Storage` open now sends Crystal `UserStorage` with the full backing storage length even when expanded storage is inactive, while higher-slot storage actions remain gated by current accessible capacity; the full `mir2-simulation` regression suite is green with 577 tests
- `77.76%`: expired expanded storage now downgrades to inactive on current `StartGame`, then emits Crystal-style expiry chat plus `ResizeStorage` on the first world tick and persists the account flag back to `false` while preserving the 160-slot backing array; the full `mir2-simulation` regression suite is green with 579 tests
- `77.77%`: current `EquipItem(grid=Storage)` now resolves the exact storage item through the active `@Storage` service, and current `RemoveItem(grid=Inventory|Storage)` now follows Crystal's exact destination-slot semantics with ack-only packet shape instead of accepting `grid=Equipment` or falling back into another bag slot; the full `mir2-simulation` regression suite is green with 582 tests
- `80.00%`: current `MysteryWater` plus cursed current-equipment semantics now match Crystal's bounded runtime surface, so first use unlocks and consumes, repeat use hint-chats without consuming, cursed current `RemoveItem` and replacement `EquipItem` require the unlock, successful cursed removal/replacement clears it again, and storage-grid replacement rejects replaced equipment that cannot be stored; the full `mir2-simulation` regression suite is green with 590 tests
- `82.50%`: current equipment/item metadata now preserves Crystal `NeedIdentify` and `SoulBoundId` through runtime/item payload round-trips, auto-identifies items on equip/use-equip, and rejects equipping items soul-bound to another character; focused equip/item/storage regressions are green and later full-suite revalidation remains green with 599 tests
- `85.00%`: dynamic manifest-backed current-data `UseItem` now routes Crystal `SunPotion`, duration buffs, `TownTeleport`, `BenedictionOil`, `RepairOil`, and `WarGodOil` through template stats and scroll shapes, including same-key buff duration stacking and the current bounded `WarGodOil` shape-0 fallback; the full `mir2-simulation` regression suite is green with 599 tests
- `90.75%`: manifest-backed current `UseItem` now also includes the `RequiredType` expansion beyond level-only checks, enforcing modeled `MaxAC` / `MaxMAC` / `MaxDC` / `MaxMC` / `MaxSC` / `MinAC` / `MinMAC` / `MinDC` / `MinMC` / `MinSC` / `MaxLevel` requirements in `CanUseItem` against existing modeled equipment/buff totals; focused regressions for low/high requirement branches passed
- `91.00%`: manifest-backed current `UseItem` now supports scroll shape `0/2` for `DungeonEscape` and `TeleportHome` plus `RandomTeleport`; same-map occupied destinations are discovered via a current-map search, success consumes one scroll and emits `UseItem` success ack plus location/map refresh, while failures preserve state and emit failed ack without mutating; focused regressions passed with `use_item_packet_dynamic_crystal_dungeon_escape_teleports_same_map` (9/9) and `use_item_packet_dynamic_crystal_random_teleport_teleports_same_map` (30/30), and focused suites via `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_dynamic_crystal_ -- --test-threads=1 --nocapture` plus `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_ -- --test-threads=1 --nocapture` (32/32)
- `91.25%`: manifest-backed `ItemType.Food` now supports mount-feed for `RawMeat` and `LeanMeat` with full requirement and mutation parity: fails/does not consume when no mount equipped or mount is full, success consumes one meat and emits `server.MountFed` plus `ItemRepaired`, `RawMeat` shape `0` applies Crystal-style max-dura loss before feed and `LeanMeat` shape `1` skips max-dura loss; focused regressions passed with `use_item_packet_dynamic_crystal_food_requires_equipped_mount` and `use_item_packet_dynamic_crystal_food_feeds_equipped_mount`, and focused suite `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_dynamic_crystal_food_ -- --test-threads=1 --nocapture` plus `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_ -- --test-threads=1 --nocapture` (32/32)
- `91.50%`: manifest-backed normal-potion shape `0` now uses a modeled pending/timed recovery path on `UseItem`: `pending_pot_health_amount` / `pending_pot_mana_amount` are enqueued, the potion is consumed without immediate HP/MP mutation or hint chat, and `advance_world` drains recovery into timed `ObjectHealth` / `ObjectMana` packets; verified by `use_item_packet_dynamic_crystal_normal_potion_queues_timed_restore` and full `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_ -- --test-threads=1 --nocapture` (33/33); later full-suite revalidation is green at 620/620.
- `91.75%`: manifest-backed Crystal equipment item types now map automatically to runtime `EquipmentSlot` for item gain, test helper creation, and `UseItem` fallback, removing hand-coded/test-only equip-slot setup for current manifest equipment use; verified by `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_crystal_equipment_ -- --test-threads=1 --nocapture` (2/2) and `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_ -- --test-threads=1 --nocapture` (33/33).
- `92.00%`: manifest-backed scroll shape `0/2` now follows Crystal `CanUseItem` map-rule rejection for configured maps: `NoEscape` blocks `DungeonEscape` / `TeleportHome` with `server.CanNotDungeon`, and `NoRandom` blocks `RandomTeleport` with `server.CanNotRandom`; both failure paths preserve item and position and were verified with focused `use_item_packet_dynamic_crystal_dungeon_escape_rejects_on_no_escape_map`, `use_item_packet_dynamic_crystal_random_teleport_rejects_on_no_random_map`, and adjacent `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_ -- --test-threads=1 --nocapture` (35/35).
- `92.25%`: manifest-backed `RepairOil` / `WarGodOil` now reject equipped weapons carrying Crystal/rental `DontRepair`, and `WarGodOil` also rejects `NoSRepair`, matching Crystal's no-consume/no-mutation failure surface; verified by `use_item_packet_dynamic_crystal_repair_oils_respect_weapon_repair_binds` and adjacent `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_ -- --test-threads=1 --nocapture` (36/36).
- `92.50%`: successful dead-player `ResurrectionScroll` use now restores modeled MP to the current runtime cap together with full HP revive, matching Crystal's `MP = Stats[Stat.MP]` revive surface for the bounded runtime; verified by `use_item_packet_dead_player_resurrection_scroll_revives_and_consumes_item` and adjacent `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_ -- --test-threads=1 --nocapture` (36/36).
- `92.75%`: explicit `EquipItem` target-slot compatibility now follows Crystal item-type rules for manifest-backed ring/bracelet equipment, allowing rings and bracelets to equip into right-side slots instead of being blocked by the default left-slot mapping; verified by `equip_item_packet_manifest_ring_and_bracelet_can_target_right_slots` and adjacent `cargo +1.89.0 test --locked -p mir2-simulation equip_item_packet -- --test-threads=1 --nocapture` (9/9).
- `93.00%`: explicit `EquipItem` now applies Crystal gender/class/required-type rejection for dynamic manifest-backed equipment before mutation and without chat, matching `CanEquipItem`'s false-return surface while preserving localized `UseItem` requirement messages; verified by `equip_item_packet_manifest_equipment_rejects_unmet_requirements_silently`, adjacent `equip_item_packet` (11/11), adjacent `use_item_packet_crystal_equipment_` (2/2), `fmt --check`, `diff --check`, and full `mir2-simulation` 622/622.
- `93.00%`: storage-sourced explicit `EquipItem` is now covered for the same dynamic manifest-backed requirement rejection surface; focused `equip_item_packet_storage_manifest_equipment_rejects_unmet_requirements_silently`, adjacent `equip_item_packet` (12/12), `fmt --check`, `diff --check`, and full `mir2-simulation` 623/623 passed.
- `93.00%`: dynamic manifest-backed credit-token use is now covered directly through `CreditToken3`, including success ack, `GainedCredit`, localized `server.CreditsAddedToAccount` hint, credit-state update, and item consumption; focused regression, adjacent `use_item_packet_` (37/37), `fmt --check`, `diff --check`, and full `mir2-simulation` 624/624 passed.
- `93.00%`: dynamic manifest-backed explicit equipment use is now covered for the positive requirement path as well: `SpiritRing` equips to the right ring slot at level 15 with success ack, equipment mutation, and source inventory removal; focused regression, adjacent `equip_item_packet` (13/13), `fmt --check`, `diff --check`, and full `mir2-simulation` 625/625 passed.
- `93.25%`: successful modeled use-equip no longer emits runtime-only `sim.equippedItem*` chat; the success surface is now ack/refresh/equipment-state only, matching Crystal's explicit equip success packet surface for the bounded runtime. Verified by focused use-equip regression, adjacent `use_item_packet_` (37/37), adjacent `equip_item_packet` (13/13), `fmt --check`, `diff --check`, and full `mir2-simulation` 625/625.
- `93.50%`: non-inventory equipment `UseItem` attempts no longer emit literal runtime-only failure chat; belt-sourced equipment-like use now failed-acks without chat or mutation. Verified by `use_item_packet_belt_equipment_rejects_without_runtime_chat`, adjacent `use_item_packet_` (38/38), `fmt --check`, `diff --check`, and full `mir2-simulation` 626/626.
- `93.75%`: unusable inventory `UseItem` fallback no longer emits runtime-only `sim.itemNoActiveUse`; unknown/unusable items now failed-ack without chat or mutation. Verified by `use_item_packet_unusable_inventory_item_rejects_without_runtime_chat`, adjacent `use_item_packet_` (39/39), `fmt --check`, `diff --check`, and full `mir2-simulation` 627/627.
- `94.00%`: missing-item and invalid-source `UseItem` failures no longer emit runtime-only `sim.itemNotFoundInBag`; missing inventory ids now failed-ack without chat or mutation. Verified by `use_item_packet_missing_inventory_item_rejects_without_runtime_chat`, adjacent `use_item_packet_` (40/40), `fmt --check`, `diff --check`, and full `mir2-simulation` 628/628.
- `94.10%`: unmodeled `UseItem(grid=HeroInventory)` now emits a Crystal-shaped failed `UseItem` ack instead of empty packets while still avoiding fallback into matching player inventory. Verified by `use_item_packet_hero_inventory_grid_does_not_mutate_matching_player_item`, adjacent `use_item_packet_` (40/40), `fmt --check`, `diff --check`, and full `mir2-simulation` 628/628.
- `99.70%`: Successful cast-skill paths no longer emit runtime-only generic `sim.castSkill` chat; buff/heal and summon success preserve state mutation/spawns without generic success narration. Verified by focused `casting` (6/6), `fmt --check`, and full `mir2-simulation` 643/643.
- `99.70%`: Cast-skill high-level entrypoint (`cast_skill`) now silently rejects before `StartGame` instead of emitting runtime-only `sim.joinWorldBeforeCastingSkills` helper chat. Verified by focused pre-start cast-skill test (1/1), adjacent `casting` (6/6), `fmt --check`, and full `mir2-simulation` 643/643.
- `99.70%`: Interaction high-level/dialog entrypoints (`interact`, `select_npc_dialog_target`) now silently reject before `StartGame` instead of emitting runtime-only `sim.joinWorldBeforeInteracting` helper chat. Verified by focused pre-start interaction test (1/1), adjacent `npc_interaction` (2/2), `crystal_npc_dialog` (1/1), `crystal_npc_service` (1/1), `fmt --check`, and full `mir2-simulation` 642/642.
- `99.70%`: Harvest high-level and packet entrypoints (`harvest`, `Harvest`) now silently reject before `StartGame` instead of emitting runtime-only `sim.joinWorldBeforeHarvesting` helper chat. Verified by focused pre-start harvest test (1/1), adjacent `harvest` (9/9), `fmt --check`, and full `mir2-simulation` 641/641.
- `99.70%`: Attack high-level and packet entrypoints (`attack`, `Attack`, `RangeAttack`) now silently reject before `StartGame` instead of emitting runtime-only `sim.joinWorldBeforeAttacking` helper chat. Verified by focused pre-start attack test (1/1), adjacent `attack` (76/76), combat trace focused test (1/1), `fmt --check`, and full `mir2-simulation` 640/640.
- `99.70%`: Movement high-level and packet entrypoints (`move_to`, `Walk`, `Run`, `Turn`) now silently reject before `StartGame` instead of emitting runtime-only `sim.joinWorldBeforeMoving` / `sim.joinWorldBeforeTurning` helper chats. Verified by focused pre-start movement test (1/1), adjacent `walk` (6/6), `run_` (3/3), `transfer_map` (2/2), `fmt --check`, and full `mir2-simulation` 639/639.
- `99.70%`: Localization formatting now substitutes Crystal-style `{index:format}` placeholders, and trainer idle average damage chat uses Crystal `server.AverageDamageOnTrainer`. Verified by `mir2-game-data` (22/22), focused trainer test (1/1), `fmt --check`, and full `mir2-simulation` 638/638.
- `99.40%`: Benediction-oil no-effect, luck, and curse outcomes now use Crystal `server.WeaponNoEffect`, `server.WeaponLuck`, and `server.WeaponCurse` instead of hardcoded English. Verified by focused `benediction_oil` (4/4), adjacent `use_item` (42/42), `fmt --check`, and full `mir2-simulation` 638/638.
- `99.30%`: `@ADDSTORAGE` now emits modeled `ResizeStorage` without runtime-only hardcoded expanded-storage success chat. Verified by focused `addstorage` (2/2), adjacent `storage` (43/43), `fmt --check`, and full `mir2-simulation` 638/638.
- `99.20%`: `ShowGroupPickup` item notices now use Crystal `server.FriendlyPickedUpItem` from the generated localization bundle instead of hardcoded English formatting. Verified by focused group pickup test (1/1), adjacent `pickup` (14/14), `fmt --check`, and full `mir2-simulation` 638/638.
- `99.10%`: High-level `use_item(key)` and `drop_item(key)` before `StartGame` now emit no packets and no runtime-only `sim.joinWorldBeforeUsingItems` chat while preserving normal post-start behavior. Verified by adjacent `drop_item` (10/10), focused consumable helper (1/1), adjacent `use_item` (42/42), `fmt --check`, and full `mir2-simulation` 638/638.
- `99.00%`: High-level `drop_item(key)` missing-item helper now returns no packets and emits no runtime-only `sim.itemNotFoundInBag` chat, preserving no mutation and aligning with the packet `DropItem` missing-source no-chat surface. Verified by focused dropped-inventory-item test (1/1), adjacent `drop_item` (10/10), `fmt --check`, and full `mir2-simulation` 638/638.
- `98.90%`: Map-transfer not-in-world rejection now uses Crystal `server.NotFound` instead of `sim.joinWorldBeforeMoving`, with ordinary/debug missing-player transfer handling aligned to the same key. Verified by focused transfer-bound test (1/1), adjacent `transfer_map` (2/2), `fmt --check`, and full `mir2-simulation` 638/638.
- `98.80%`: Missing-template `RequestItemInfo` failure now uses Crystal `server.NotFound` instead of runtime-only `"Crystal item info ... was not found."`. Verified by focused request-item-info test (1/1), `fmt --check`, and full `mir2-simulation` 638/638.
- `98.70%`: Map-transfer bounds rejection now uses Crystal `server.CannotPositionMoveOnMap` instead of runtime-only `"You are not standing on this map transfer."`, preserving no-transfer/no-position-mutation behavior. Verified by focused transfer-bounds test (1/1), adjacent `transfer_map` (2/2), `fmt --check`, and full `mir2-simulation` 638/638.
- `98.60%`: Stage 5 `event.spawn` and `hero.behaviour` successes no longer emit runtime-only helper narration while preserving event spawn, conquest log, and hero behaviour state mutation. Verified by focused conquest/event/hero test (1/1), broader `stage5_` (26/26), `fmt --check`, and full `mir2-simulation` 638/638.
- `98.50%`: Debug Crystal transfer keys no longer emit runtime-only `"Transferred to Crystal map ..."` success chat, leaving `MapInformation` and `UserLocation` as the visible success surface. Verified by focused debug transfer test (1/1), adjacent `transfer_map` (2/2), `fmt --check`, and full `mir2-simulation` 638/638.
- `98.40%`: Generic runtime-only Stage 5 helper success chats were removed across group/social/mail/trade/auction/conquest/hero/profession helpers, preserving state mutations and existing localized Crystal failure/success surfaces. Verified by focused `stage5_` (26/26), `fmt --check`, and full `mir2-simulation` 638/638.
- `98.30%`: Stage 5 event-spawn missing-player/position rejections now use Crystal `server.NotFound`. Verified by focused `stage5_conquest_event_hero_mining_and_crafting_flow` (1/1), `fmt --check`, and full `mir2-simulation` 638/638.
- `98.20%`: Unknown map-transfer rejection now uses Crystal `server.NotFound`. Verified by focused `transfer_map_requires_player_on_transfer_bounds` (1/1), `fmt --check`, and full `mir2-simulation` 638/638.
- `98.10%`: Stage 5 unknown-command rejection now uses Crystal `server.InvalidPacketReceived`. Verified by focused `stage5_trade_shop_and_auction_cancel_error_paths_preserve_gold` (1/1), `fmt --check`, and full `mir2-simulation` 638/638.
- `98.00%`: Stage 5 inactive-trade rejections now use Crystal `server.NotFound`. Verified by focused `stage5_trade_shop_and_auction_cancel_error_paths_preserve_gold` (1/1), `fmt --check`, and full `mir2-simulation` 638/638.
- `97.90%`: Stage 5 `auction.buy` / `auction.cancel` missing-id rejections now use Crystal `server.InvalidPacketReceived`. Verified by focused `stage5_trade_shop_and_auction_cancel_error_paths_preserve_gold` (1/1), `fmt --check`, and full `mir2-simulation` 638/638.
- `97.80%`: Stage 5 `mail.claim` / `mail.delete` missing-id rejections now use Crystal `server.InvalidPacketReceived`. Verified by focused `stage5_social_group_guild_mail_persist_across_reload` (1/1), `fmt --check`, and full `mir2-simulation` 638/638.
- `97.70%`: Stage 5 `trade.offerGold` missing-amount rejection now uses Crystal `server.InvalidPacketReceived`. Verified by focused `stage5_trade_shop_and_auction_cancel_error_paths_preserve_gold` (1/1), `fmt --check`, and full `mir2-simulation` 638/638.
- `97.60%`: Stage 5 hero-behaviour missing-hero rejection now uses Crystal `server.NotFound`. Verified by focused `stage5_conquest_event_hero_mining_and_crafting_flow` (1/1), `fmt --check`, and full `mir2-simulation` 638/638.
- `97.50%`: Stage 5 event-spawn missing-template rejection now uses Crystal `server.NotFound`. Verified by focused `stage5_conquest_event_hero_mining_and_crafting_flow` (1/1), `fmt --check`, and full `mir2-simulation` 638/638.
- `97.40%`: Stage 5 guild creation success now uses Crystal `server.SuccessfullyCreatedGuild`. Verified by focused `stage5_social_group_guild_mail_persist_across_reload` (1/1), `fmt --check`, and full `mir2-simulation` 638/638.
- `97.30%`: Stage 5 craft no-ore rejection now uses Crystal `server.CraftingAttemptFailed` while preserving no crafted item mutation. Verified by focused `stage5_conquest_event_hero_mining_and_crafting_flow` (1/1), `fmt --check`, and full `mir2-simulation` 638/638.
- `97.20%`: Stage 5 credit-shop insufficient-credit rejection now uses Crystal `server.YouDontHaveEnoughCurrency` while preserving credit, mail, item, and `LoseCredit` no-mutation behavior. Verified by focused `stage5_trade_shop_and_auction_cancel_error_paths_preserve_gold` (1/1), `fmt --check`, and full `mir2-simulation` 638/638.
- `97.10%`: Stage 5 socket/seal missing-source rejection chats now use Crystal `server.NotFound` while preserving source lookup and no-mutation failure behavior. Verified by focused `stage5_item_` (13/13), `fmt --check`, and full `mir2-simulation` 633/633.
- `96.70%`: ordinary map-transfer success no longer emits runtime-only `"Transferred to ..."` chat, leaving `MapInformation` and `UserLocation` as the packet-visible success surface. Verified by focused `transfer_map` (2/2), `fmt --check`, and full `mir2-simulation` 633/633.
- `96.60%`: Stage 5 socket/seal invalid-source rejection chats now use Crystal `server.InvalidCombination` while preserving source item retention and no-mutation failure behavior. Verified by focused `stage5_item_` (13/13), `fmt --check`, and full `mir2-simulation` 633/633.
- `96.50%`: Stage 5 gold-shop purchase chat now uses Crystal `server.BoughtItemForGold` while preserving gold debit and item gain. Verified by focused `stage5_trade_shop_and_auction_are_transactional` (1/1), broader `stage5_` (22/22), `fmt --check`, and full `mir2-simulation` 633/633.
- `96.40%`: successful harvest-drop transfer no longer emits runtime-only `"Harvested ..."` chat, leaving `GainedItem` plus `ObjectHarvested` as the packet-visible success surface. Verified by `harvest` (8/8), `fmt --check`, and full `mir2-simulation` 633/633.
- `96.30%`: expanded-storage expiry notice now uses Crystal `server.ExpandedStorageExpired` while preserving one-shot resize notice and account flag persistence. Verified by focused `expired_expanded_storage_tick_emits_resize_notice_once_and_persists_flag` (1/1), broader `storage` (43/43), `fmt --check`, and full `mir2-simulation` 633/633.
- `96.20%`: Stage 5 item socket/seal success chats now use Crystal `server.ItemSocketsIncreased` and `server.ItemSealedFor`. Verified by focused `stage5_item_` (13/13), `fmt --check`, and full `mir2-simulation` 633/633.
- `96.10%`: Stage 5 item-seal reseal-delay rejection now uses Crystal `server.ItemCannotBeResealedFor` with the modeled remaining-duration label. Verified by focused `stage5_item_seal_rejects_before_next_seal_date_after_expiry` (1/1), broader `stage5_item_` (13/13), `fmt --check`, and full `mir2-simulation` 633/633.
- `96.00%`: Stage 5 credit-shop purchase chat now uses Crystal `server.BoughtItemForCredit` while preserving mailbox delivery and credit debit. Verified by focused `stage5_credit_shop_mails_purchase_and_claim_transfers_attachment` (1/1), broader `stage5_` (22/22), `fmt --check`, and full `mir2-simulation` 633/633.
- `95.90%`: Stage 5 successful trade completion now uses Crystal `server.TradeSuccessful`. Verified by focused `stage5_trade_shop_and_auction_are_transactional` (1/1), broader `stage5_` (22/22), `fmt --check`, and full `mir2-simulation` 633/633.
- `95.80%`: Stage 5 trade/shop/auction low-gold rejection messages now use Crystal `server.LowGold`. Verified by focused `stage5_trade_shop_and_auction_cancel_error_paths_preserve_gold` (1/1), broader `stage5_` (22/22), `fmt --check`, and full `mir2-simulation` 633/633.
- `95.70%`: direct ground-drop pickup full-bag rejection now uses Crystal `server.YouCannotCarryAnymore` while current-cell pickup still skips blocked drops for later candidates. Verified by focused `pickup` (14/14), `fmt --check`, and full `mir2-simulation` 633/633.
- `95.60%`: Stage 5 mail/shop/auction/craft full-bag rejection messages now use Crystal `server.YouCannotCarryAnymore`. Verified by focused `stage5_shop_and_auction_full_bag_preserve_gold_and_items` (1/1), broader `stage5_` (22/22), `fmt --check`, and full `mir2-simulation` 633/633.
- `95.50%`: Stage 5 item socket max-capacity and already-sealed rejections now use Crystal `server.ItemMaxSockets` and `server.ItemAlreadySealed` text keys. Verified by focused `stage5_item_` (13/13), `fmt --check`, and full `mir2-simulation` 633/633.
- `95.40%`: harvest no-drop and full-bag retry messages now use Crystal `server.NothingWasFound` and `server.YouCannotCarryAnymore`, preserving pending-drop retry and `ObjectHarvested` timing. Verified by focused `harvest` (8/8), `fmt --check`, and full `mir2-simulation` 633/633.
- `95.30%`: owner-blocked pickup rejection now uses Crystal `server.CannotPickupNotOwner` localization instead of runtime-only literal English, while preserving owner-window and scan-skip semantics. Verified by focused `pickup` (14/14), `fmt --check`, and full `mir2-simulation` 633/633.
- `95.20%`: normal item/gold pickup success no longer emits runtime-only `sim.pickedUpItem` chat, while Crystal `ShowGroupPickup` group notices remain preserved. Verified by focused `pickup` (14/14), `fmt --check`, and full `mir2-simulation` 633/633.
- `95.10%`: static starter and dynamic manifest-backed potion `UseItem` now honor Crystal `NoDrug` map-rule rejection with `server.YouCannotUsePotionsHere`, failed ack, item preservation, and no HP/MP recovery queue. Verified by focused `no_drug` (2/2), adjacent `use_item_packet_` (42/42), `fmt --check`, and full `mir2-simulation` 633/633.
- `95.00%`: static starter HP/MP potion use now queues Crystal-style timed recovery instead of immediately mutating HP/MP, while preserving successful `UseItem` ack and item consumption; follow-up ticks emit `ObjectHealth` as the pending restore drains. Verified by focused `crystal_use_item_packet_consumes_` (2/2), adjacent `use_item_packet_` (40/40), legacy `consumable_item_restores_hp`, `fmt --check`, and full `mir2-simulation` 631/631.
- `94.90%`: static `repair-powder` success/failure no longer emits runtime-only `sim.noEquipmentNeedsRepair` / `sim.repairedEquippedItems`; equipment repair mutation and `ItemRepaired` packets are preserved without extra generic chat. Verified by focused `repair_powder` (2/2), adjacent `use_item_packet_` (40/40), `fmt --check`, and full `mir2-simulation` 631/631.
- `94.80%`: static `town-teleport` success no longer emits runtime-only `sim.townTeleportReturnedToSpawn`; successful static teleports now route through movement/location packets without generic success chat. Verified by focused `town_teleport` (3/3), adjacent `use_item_packet_` (40/40), `fmt --check`, `diff --check`, and full `mir2-simulation` 631/631.
- `94.70%`: static `benediction-oil` no-weapon failure no longer emits hardcoded runtime-only chat and preserves the item on failed luck attempts. Verified by focused `benediction_oil` (4/4), adjacent `use_item_packet_` (40/40), `fmt --check`, `diff --check`, and full `mir2-simulation` 631/631.
- `94.60%`: successful `SplitItem` no longer emits runtime-only `"Item stack split."`; inventory/storage split success now returns Crystal-shaped `SplitItem1` plus `SplitItem` without extra chat. Verified by focused `split_item_packet` (7/7), `storage` (43/43), `fmt --check`, `diff --check`, and full `mir2-simulation` 630/630.
- `94.50%`: static `repair-oil` / `war-god-oil` now use Crystal localized weapon-repair Hint chat on success and no runtime-only failure chat on no-repair failures. Verified by focused `repair_oil` (3/3), adjacent `use_item_packet_` (40/40), `fmt --check`, `diff --check`, and full `mir2-simulation` 630/630.
- `94.40%`: successful `DropItem` no longer emits runtime-only `custom.itemDropped`; normal and split-stack inventory drops now success-ack with ground-object visibility and no generic chat. Verified by adjacent `drop_item_packet` (10/10), `fmt --check`, `diff --check`, and full `mir2-simulation` 629/629.
- `94.30%`: static HP/MP consumable `UseItem` success no longer emits runtime-only `sim.usedItem`; inventory/belt starter potions now heal, consume, and success-ack without chat. Verified by focused inventory/belt regressions, adjacent `use_item_packet_` (40/40), `fmt --check`, `diff --check`, and full `mir2-simulation` 629/629.
- `94.20%`: missing-source `DropItem` no longer emits runtime-only `sim.itemNotFoundInBag`; absent inventory ids now failed-ack without chat or mutation. Verified by `drop_item_packet_missing_inventory_item_rejects_without_runtime_chat`, adjacent `drop_item_packet` (10/10), `fmt --check`, `diff --check`, and full `mir2-simulation` 629/629.
- `78%`: NPC and persistence foundations are no longer hardcoded-only: starter NPC interactions flow through script lookup plus quest binding, Crystal NPC manifests include raw script lines for interpreter work, and account/character saves can persist to JSON across fresh config/process reloads
- `80%`: Crystal NPC scripts are now partially executable instead of manifest-only: scripts are sectionized, NPCs can bind `script_key`, and runtime can render current `@Main` dialog/links plus simple goto flow from imported Crystal text
- `82%`: map-state foundations are in place: current map title/file is runtime state and saved with characters, starter map-transfer rules can move the player and refresh `MapInformation` / `UserLocation`, and `worldSnapshot` exposes safe-zone state
- `85%`: current Crystal NPC execution is no longer just dialog and goto: runtime now covers persisted `CHECK/SET` flags, class/gender gating, Crystal-style `CHECKQUEST ACTIVE/COMPLETE`, reward actions, and the pet command cluster (`GIVEPET` / `PETCOUNT` / `PETLEVEL` / `CHECKPET` / `CLEARPETS`), while old character saves remain forward-compatible with new NPC flag persistence
- `87%`: Crystal event-script parity moved further into real runtime conditions: `RANDOM` now follows Crystal's pass-on-zero semantics via runtime RNG state, `CHECKMON` can query current-map live monster counts, and `PARAM1/2/3 + MONGEN` is now covered by condition/action integration tests instead of action-only smoke coverage
- `89%`: Crystal NPC script flow now handles argumentized page targets and variable-driven mini-games instead of only static labels: runtime supports `MOV`, `CHECKCALC`, `CHECKEXACTMON`, `REMOVEPET`, parameter-page matching like `@Guess(1)` -> `[@Guess()]`, `%ARG(n)`, and basic `%A1` / `<$OUTPUT(A1)>` variable substitution, with the imported Lottery script covered by an integration test
- `92%`: NPC/event script parity now crosses from mini-game-only into real Crystal world/event coverage: `CALC`, embedded variable/output substitution, `%INPUTSTR` dialog submission, `HASBAGSPACE`, `DAYOFWEEK` / `HOUR` / `MIN`, `CHECKMAP` / `CHECKRANGE` / `CHECKHUM`, and current-map `MONCLEAR` all execute through the Rust runtime, with gateway/web input wiring and regression tests in place
- `95%`: backend state and script persistence moved closer to real Crystal quest/event flows: `GIVEEXP`, `LOADVALUE`, and `SAVEVALUE` now execute in Rust, player experience is carried through `UserInformation` and character saves, and NPC key-value storage survives save/reload with regression coverage
- `97%`: current event/admin script condition coverage is now wide enough for more imported Crystal branches to evaluate without stubbing: `ISADMIN`, `CHECKMAPLIGHT`, and `GROUPCHECKNEARBY` have real baseline execution in Rust, while `GROUPLEADER` / `GROUPCOUNT` remain explicit single-player baselines instead of accidental pass-through
- `100%`: current backend target has full scripted parity for the imported Crystal gameplay slice: group-state checks/actions now run against configured runtime members, conquest-state checks are runtime-backed, and the remaining backend milestone gaps for this migration tracker are closed with regression coverage
- `100%`: current-map Crystal respawn bootstrap now preserves low-density visible representatives without stacking every representative on the same respawn origin. WoomyonWoods(S) coverage now spreads the imported Oma / OmaFighter / OmaWarrior / ForestYeti plus tree and mushroom monster rows across nearby walkable cells, with `start_game_visible_respawns_spread_representative_crystal_map_spawns` locking the regression. Production gateway `20260521T0830Z-spreadrep` was live-verified with BichonProvince, WoomyonWoods(S), NaturalCave, DeadMineEntrance, InsectCave_2F, and ZumaMaze screenshots and state captures.
- `Stage 4.1`: generated Crystal movement transfers, safe zones, mini-map/big-map/light metadata, and target-map collision-aware spawn placement are wired into runtime with regression coverage
- `Stage 4.2`: `crystal_monster_ai_summary.json` classifies 555 monster rows across 212 Crystal AI families; 87 families are present in respawns, 95 are currently special/guard-covered, and 0 spawned families still run generic baseline behavior after default `MonsterObject`, HellFire, ShamanZombie/BlackFoxman, HolyDeva, GreatFoxSpirit, ManectricKing/Master_DragonYang, SeedingsGeneral, RestlessJar, HellKeeper, GeneralMeowMeow, TucsonGeneral, TrapRock, Armadillo/ArmadilloElder, RedThunderZuma/ZumaTaurus, IncarnatedZT, BoneLord, MinotaurKing, KingScorpion, DarkDevil, OmaKing, Khazard, ManectricClaw/Chieftain_Priest, MirStatue, GuardianRock, RedMoonEvil, EvilCentipede, Yimoogi, Lamia/Kirin, FrostTiger, IceGuard, FrozenMiner/FrozenAxeman/FrozenMagician, SnowWolf/FrozenWarewolf/SnowYeti/DarkWraith, TucsonMage/TucsonWarrior, CrystalSpider, SandWorm, SandSnail, Hen/Pig/Bull, Deer, CannibalTentacles, Jar1/Jar2, TurtleGrass/ManTree, CaveMaggot, ToxicGhoul, ThunderElement, DarkBeast, FlamingWooma, HedgeKekTal, Trainer, WoomaTaurus, HarvestMonster, TucsonEgg, Tree, DigOutZombie, RevivingZombie, Red/White Foxman, WaterDragon/BlackTortoise, cat-family, and Devil Node coverage
- `Stage 4.5`: `crystal_npc_command_summary.json` classifies Crystal NPC script command coverage: 81/81 command names and 7,044/7,044 command occurrences are covered by the current Rust baseline, with runtime diagnostics for future unknown action/condition commands and a section-hop safety limit for script loops
- `Stage 4.6`: persistence hardening now includes account-store schema versioning, legacy migration, corrupt JSON fallback, atomic writes, and restart/reload regression coverage
- `Stage 5.1 baseline`: group/guild/social/mail state is persisted, exposed through snapshots, reachable by gateway/browser commands, and covered by restart regression tests
- `Stage 5.2 baseline`: player trade, shop buy, and auction list/buy/cancel flows are implemented with success, cancel, insufficient-funds, full-bag, and pre-accept disconnect no-loss regression tests
- `Stage 5.3 baseline`: conquest/castle state and event monster spawning are implemented through Stage 5 runtime state and gateway command paths
- `Stage 5.4 baseline`: hero recruitment/behavior plus mining/crafting state are persisted and tested
- `Stage 5.5 baseline`: `mir2-protocol` packet trace entries, runtime tests, and `apps/gateway/src/bin/packet_trace.rs` now lock representative bootstrap, combat delayed-hit, map-transfer ordering, and local TCP gateway trace capture; live side-by-side Crystal capture remains open until `MIR2_CRYSTAL_TCP_ADDR` is configured
- `Stage 5.6 baseline`: automated multi-client shared-store smoke, save/reload-under-load, account-store backup/restore, disconnect persistence, WebSocket/TCP socket-close saves, session panic boundaries, 1,200-tick bounded-entity soak tests, and real 64-client WebSocket/TCP gateway load harnesses with RSS samples now cover the current runtime/gateway surface
- `Stage 5.7 baseline`: Chrome CDP UI smoke now creates a fresh account plus temporary character, exercises login/select/game/inventory/character/storage/NPC/combat/map-transfer plus Stage 5 broad-system commands, and archives screenshots for the current implemented systems
- `80%+`: persistence, larger world systems, and edge-case packet parity become the main remaining gap

## Rule For Updates

Every time backend parity meaningfully moves, this file should be updated together with `docs/CRYSTAL-SERVER-PARITY.md`.

## 2026-07-23 Visual Capture Support Sync

- `WorldEntitySnapshot` now carries the Crystal monster AI id; simulation
  snapshots, shared-state merge, zone spawn packets, and observer re-seeding
  preserve it so AI 6 neutral guards stay green on the native minimap.
- `MIR2_SIMULATION_FIXED_LIGHT_SETTING=1..4` provides a server-only,
  process-start QA override for deterministic native/Web light pairing. Invalid
  or absent values retain the existing UTC Crystal light formula.
- Focused Gateway regressions cover template neutral-AI restoration, NPC side
  effect spawn packets, and shared observer movement re-seeding. Simulation
  regressions cover override resolution and visible current-map AI snapshots.
- This is deterministic QA/snapshot parity, not a production gameplay rule or
  a substitute for the remaining backend expansion queue.

## 2026-08-12 Production Login Incident Closure

- Root cause: a World Director restart restored session-owned Zone state while
  `ZoneHostServer.sessions` restarted empty. Twenty orphan players then
  accumulated 910,015 pending packet frames; repeated base-snapshot attempts
  expanded the 49.99 MB factory payload beyond the 64 MiB bound and held the
  per-Zone mutation lane long enough for Gateway `on_connect` to time out.
- `SharedInProcessZoneRuntimeFactory` now has a world-only checkpoint path that
  strips all session-scoped state without first encoding pending packets.
  Legacy images are consumed without cloning the large packet-frame vectors.
- `ZoneManager::leave_all_sessions` removes authoritative player occupancy while
  preserving map/world state. Strict roots remain mandatory for ordinary Zone
  checkpoints; only a World Director checkpoint whose enclosing commitment was
  already verified may re-anchor module-dependent collision roots.
- Pending packet queues retain only the newest 1,024 messages, including while
  decoding legacy checkpoints. The Zone Host journal compactor is now a
  reproducible source feature configured by
  `MIR2_ZONE_HOST_JOURNAL_COMPACT_ENTRIES` and
  `MIR2_ZONE_HOST_JOURNAL_COMPACT_INTERVAL_MS`.
- Offline replay of the exact production checkpoint SHA-256
  `b85aa711db02d6f1bddaf5f128cdae56d68fd67d080e99e6dc10673ad4f2fc0c`
  restored 16 Zones and emitted a 6,378,866-byte world-only factory image,
  down from 49,987,670 bytes.
## 2026-08-12 Shared monster cadence and corpse authority

- `ZoneMonsterSpawn` now carries Crystal movement and attack intervals in real
  milliseconds. Session snapshots, shared-map bootstrap, world-director spawns,
  summons, movement, turning, melee, and ranged attacks use those template
  values instead of the former fixed Zone cadence.
- Personal simulation ticks still advance private systems, but Gateway now
  removes `ObjectWalk`/`ObjectRun`/`ObjectTurn` packets for object ids owned by
  the shared Zone. This removes the second movement broadcaster that could make
  monsters appear too fast or visually jump between authorities.
- The death regression now verifies the authoritative monster remains at HP 0
  with `dead=true` and is replayed as a dead `ObjectMonster` to a later joiner,
  in addition to the existing `ObjectDied`, experience, and ground-drop checks.
- Verification: `cargo test -p mir2-simulation --test shared_zone` passed
  157/157; focused Crystal 1500 ms Scarecrow cadence and kill/corpse tests pass;
  `cargo check -p mir2-simulation -p mir2-gateway` passes. A browser QA-spawn is
  not used as server proof because Stage 5 `event.spawn` remains personal-session
  state while normal attacks are shared-Zone authoritative.

## 2026-08-26 Melee defence-type certificate fix

- `FlamingSword` and `Thrusting` packet attacks now carry their declared
  Crystal defence type into delayed damage scheduling instead of inheriting the
  ordinary melee `AcAgility` default. The previous leak could turn a valid
  FlamingSword hit into an Agility miss even though Crystal declares AC-only
  defence for that skill.
- The Platinum 1.76 combat-milestone fixture re-arms consumed one-shot melee
  skills for every bounded attempt. Two deterministic 15-case runs produced
  identical case data and all seven assertions passed; Warrior level 45 on
  D504 dealt 23 real-runtime damage to ZumaGuardian and survived at 788 HP.
- Focused FlamingSword/Slaying and Thrusting unit regressions pass. Whole-repo
  formatting remains independently blocked by unrelated local edits in
  `apps/simulation/tests/vertical_slice.rs`; both task files pass standalone
  Rustfmt and `git diff --check`.
- Backend Candidate evidence is improved, but strict 100% and final frontend
  acceptance remain gated by same-EXE UI/live-WebSocket execution, real DPI,
  native soak, human visual/feel review, and an official release certificate.

## 2026-08-26 Windows functional vertical-slice automation

- Full Crystal character creation now selects the source `Envir.StartItems`
  loadout for all five classes when the Crystal current-map runtime is active.
  This removes the legacy demo-equipment fallback while preserving the
  Platinum 1.76 three-class allowlist as a separate bounded contract.
- The integration slice now proves a level-four Wizard's original q10-q12 path
  across Bichon and map `0115`, including NPC links, ten Oma and ten RakingCat
  player-owned kill credits, exact rewards, `FireBall` as a retained book rather
  than an auto-learned spell, and save/reload stability.
- A PowerShell 5.1 fail-closed orchestrator now runs native host, complete
  `vertical_slice`, ordinary unprivileged loop, security lifecycle, shared Zone,
  Gateway reload, and Web typecheck controls from a clean Git revision. It
  hashes every log and emits one summary. Candidate CI includes this Windows
  lane and an aggregate job requiring both Candidate lanes.
- Revision `004549e9f15ca6fa4b7fad119cb305fcad7d3230` passed 7/7 fixed controls:
  native 312/312, vertical slice 10/10 in 440.27 s, ordinary 2/2, security
  18/18, shared Zone 195/195, Gateway 1/1, and Web typecheck. Summary SHA-256 is
  `0590F2CEA720E69FA8755C34A0D22580A3F631647351BBF3C6F4DC136631753B`.
- The summary's 100% value is only the declared automated functional-control
  set. Whole-game completion remains undefined while the semantic inventory is
  incomplete, and same-EXE UI/live WSS, real DPI, 30-minute native soak, human
  visual/feel, and formal release signing remain explicit external gates.

## 2026-08-26 Taoist instructor branch and hosted-runner hardening

- The original Taoist q13-q15 branch is now an integration journey rather than
  manifest-only evidence. It proves the level/class prerequisite, Assistant
  Jane and HighPriest Jude links, ten Oma plus ten RakingCat player-owned kill
  credits, exact EXP/gold, the retained `OldLoafer` and `Healing` book rewards,
  MirGuide Peter completion, and logout/new-session persistence.
- The Windows gate is now self-contained on a clean checkout. Its first fixed
  control builds the ignored map atlas from repository sources; the resulting
  local evidence used 2,305 source images, 57 atlas pages, and content hash
  `732065c9e021a7939b2797dc26b283310eb625c5869972c3da27a072eab0e7a7`.
- Native stderr is captured compatibly with Windows PowerShell 5.1 while the
  real process exit code remains authoritative. The self-test includes a native
  stderr probe, so normal Cargo progress cannot abort evidence publication.
- Revision `23ac6012adfd4132896f01642b96ab210320065b` passed 8/8 fixed
  controls in 640,891 ms. The functional journey is 11/11 in 561.116 s; the
  remaining results are native 312/312, ordinary 2/2, security 18/18, shared
  Zone 195/195, Gateway 1/1, and Web typecheck. Summary SHA-256:
  `23593A5E4CC564DA9D38729ED4FEE36C6EC93C54D7EEB9629377BDCFACE8EE80`.
- This is 100% only for the declared automated control set. The summary retains
  `globalParityPercent=null`, `accepted=false`, and `visualAccepted=false`; the
  same-EXE UI/live-WSS, real-DPI, 30-minute native-soak, human visual/feel,
  complete-inventory, and formal-signing gates are not closed.

## 2026-08-26 Gateway ordinary-client starter loop

- A new Gateway integration journey creates an ordinary account/Warrior and
  drives Bichon exclusively with normal client packets. It walks through shared
  Zone authority, opens Village Guide's server-owned dialog, accepts quest
  1001, attacks a real Field Wasp, picks up its tile gold, finishes the quest,
  logs out, and reloads through a fresh Gateway session.
- The boundary is fail closed: remote `AcceptQuest` and `FinishQuest` packets do
  not mutate state without the active nearby NPC dialog. The successful path
  proves Wasp Stinger grant/consumption, 300 quest gold, two `RareCopperOre`,
  `CopperRing`, completed quest state, inventory/equipment, and authoritative
  transform persistence. No direct runtime action or QA/admin command is used.
- Revision `f676a2a81f9fae949d6640df747dedf493d913e9` passed the clean Windows
  gate 8/8. Gateway persistence is now 2/2 in 33.917 s; the full run also passed
  native 312/312, functional 11/11, ordinary 2/2, security 18/18, shared Zone
  195/195, and Web typecheck. `SUMMARY.json` SHA-256 is
  `DE942CEB2D105AD039C3758FF00C7160880BC213BAA4FFC99A6AA826B118C0B6`.
- The summary still records `globalParityPercent=null`, `accepted=false`, and
  `visualAccepted=false`. The scoped eight-control 100% does not establish a
  whole-game denominator or close same-EXE UI/live-WSS, real-DPI, native soak,
  human visual/feel, complete semantic inventory, or formal signing.

## 2026-08-26 Assassin instructor branch

- The original q16-q18 Assassin journey now has integration coverage. It proves
  the Assassin-only class gate and prerequisite chain across Assistant Jane,
  loaded object 13 HighAssassin Cloud, and loaded object 26 MirGuide Peter on
  map `0`.
- Q17 receives player-owned credit from ten real Oma and ten real RakingCat
  deaths. Hand-ins award exactly 48/180/48 EXP and 60/45/60 gold;
  `OldLoafer` and `FatalSword` remain bag items, `FatalSword` is not
  auto-learned, and the complete quest/reward/equipment/transform state reloads
  through a new `SimulationSession`.
- Revision `82441f7b1257486d6f2b51206f5cffa4ef20f9b8` passed the clean Windows
  functional gate 8/8. The expanded journey is 12/12 in 670.046 s; native is
  312/312, ordinary 2/2, security 18/18, shared Zone 195/195, Gateway 2/2, and
  Web typecheck passes. `SUMMARY.json` SHA-256 is
  `D1A9ACE4920A834541B5798BBE53F38DEE1D37DD261226DD91394C82AA8BC105`.
- The evidence retains `globalParityPercent=null`, `accepted=false`, and
  `visualAccepted=false`. This expands the bounded automated journey; it does
  not close the semantic denominator, same-EXE UI/live-WSS, real-DPI, native
  soak, human visual/feel, or formal-signing gates.

## 2026-08-26 Archer instructor branch

- The original q19-q21 Archer journey now has integration coverage. It proves
  the Archer-only class gate and prerequisite chain across Assistant Jane,
  loaded object 14 Captain Jerald, and loaded object 26 MirGuide Peter on map
  `0`.
- Q20 receives player-owned credit from ten real Oma and ten real RakingCat
  deaths. Hand-ins award exactly 48/180/48 EXP and 60/45/60 gold;
  `OldLoafer` and `Focus` remain bag items, `Focus` is not auto-learned, and
  the complete quest/reward/equipment/transform state reloads through a new
  `SimulationSession`.
- Revision `d01910a1694d45e85dc54eafab6e61c43a063f5f` passed the clean Windows
  functional gate 8/8. The expanded journey is 13/13 in 773.360 s; native is
  312/312, ordinary 2/2, security 18/18, shared Zone 195/195, Gateway 2/2, and
  Web typecheck passes. `SUMMARY.json` SHA-256 is
  `BE47F67645A9DF165635C173CF2F04BB85895B5DC6666F8F8DE3E963BC721197`.
- The evidence retains `globalParityPercent=null`, `accepted=false`, and
  `visualAccepted=false`. The scoped 8/8 does not close the incomplete semantic
  denominator or same-EXE UI/live-WSS, real-DPI, soak, human, and signing gates.

## 2026-08-27 Gateway original Bichon q1-q4 boundary

- The Gateway integration journey now drives original q1-q4 from a fresh
  account with ordinary packets only. It proves NPC-dialog ownership and range,
  collision-aware `0.map` movement, q1 `CannibalLeaves` transfer, probabilistic
  q2 `GingerTea`, q3's three-option weapon dialog with `SharpDagger` selection,
  q4 neutral-Deer melee, multi-pass Crystal harvesting, probabilistic five-item
  `DeerMeat` collection, exact rewards, logout, and authoritative reload.
- A formal rerun exposed a real fifth-corpse rejection: the Gateway map action
  index could retain an older incarnation's harvested tombstone. The repaired
  boundary clears that marker on live native reconciliation and fresh death,
  while preserving it for duplicate death packets after the current corpse was
  harvested. Two focused unit regressions and the unchanged strict q1-q4
  end-to-end assertion pass.
- Revision `e1290bea3de1bdcd1663ee0f823c849c937eff3d` passed the clean Windows
  functional gate 8/8. Results are map-atlas preparation, native 312/312,
  functional 13/13 in 933.42 s, ordinary 2/2, security 18/18, shared Zone
  196/196, Gateway 5/5 in 472.95 s, and Web typecheck. The run lasted from
  `2026-08-26T21:16:58.2471471Z` to `2026-08-26T21:45:46.6861359Z`;
  `SUMMARY.json` SHA-256 is
  `8C942979F9D59178C33BC72D5BAAD0F3986348B76F331EAF6B0C0DF003714849`.
- The recorded Gateway-only `MIR2_QA_NATURAL_MOVEMENT_DELAY_MS=10` accelerates
  movement waits, not combat/drop/harvest/reward rules. This is follow-on source
  evidence and does not change Candidate-03's packaged EXE. The summary remains
  `globalParityPercent=null`, `accepted=false`, and `visualAccepted=false`; its
  100% is only the fixed eight-control set, not whole-game Crystal parity.

## 2026-08-27 Web runtime clean-checkout follow-up

- The Developer Handoff failure was outside Gateway semantics: current and
  fallback R2 prefixes both lacked the tracked `bevy-1813be587ef98bc1` package,
  while six later runtime changes had not refreshed the generated manifest.
- Two locked current-source builds on the same Windows evidence host generated
  `bevy-5046abca14947f40` and manifest SHA-256
  `4EC8644042F6926D7D724A7E7E500BA7DAFA1476B49780DF7EFAC7AEEC4806C1`.
  Runtime download tests pass 4/4, policy passes 5/5, both size budgets pass,
  and the full Web production pipeline completes through 13/13 static pages.
  This proves same-host repeatability, not cross-machine byte reproducibility.
- Handoff CI remains fail-closed: it prefers the four SHA-verified prebuilt
  files and keeps that exact content lock when they are available. When they are
  unavailable, it builds with pinned Rust/wasm-bindgen and requires the active
  manifest/file hashes, WASM validation, dual-backend budgets, and complete Web
  build to pass before restoring the tracked manifest. No production R2 or
  deployment was mutated. This does not expand backend parity or alter the
  remaining live database, remote Zone, crash-recovery, UI, soak, human, and
  signing gates.
- Exact-head run `33020542728` then exposed an installation-only runner race:
  pinned Cargo started rustc through the default stable rustup proxy while that
  proxy updated. The fallback now pins `RUSTUP_TOOLCHAIN` and `RUSTC` before
  installing wasm-bindgen. The developer image locks Rust `1.95.0`, wasm32, and
  wasm-bindgen `0.2.118`; Bash/PowerShell developer wrappers source-build the
  current runtime after an immutable-object failure. Both fault-injected wrapper
  contracts, Compose configuration, the developer-release checker, and runtime
  downloader 4/4 pass locally. A new exact-head real Docker/Compose CI run is
  still required; no CI-green or merge-ready claim is made here.
- Developer Environment run `33023066209` proved Linux source compilation but
  also showed that Linux host-local JS/WASM hashes differ from the canonical
  Windows release hashes (`bevy-a314c804ae9919d3` versus tracked
  `bevy-5046abca14947f40`). Exact-head run `33023784689` then proved all four
  source builds but showed Windows CI also emits a host-local
  `bevy-efe7c0554bdf9a45`; its JS wrappers matched and only the WASM hashes
  differed. The WASM contains absolute Cargo-registry source paths, invalidating
  the prior cross-machine zero-diff assumption. The matrix now validates each
  active source-built bundle and restores its generated manifest on every host
  before the clean-checkout assertion; the immutable prebuilt content lock
  remains exact.
- Exact-head Handoff run `33023777224` independently produced the same
  CI-host `bevy-efe7c0554bdf9a45`, passed the current-source runtime build and
  full Player Web build, and failed only the superseded tracked-manifest byte
  comparison. The corrected Handoff now runs the active-bundle budget/integrity
  gate before restoring a fallback manifest.

## 2026-08-27 Shared-Zone tick recovery

- Hosted q2 evidence exposed dual world authority: 121 aggregate
  `ObjectDied` packets were observed, but none was a confirmed player-owned
  kill. A shared Gateway `WorldCommand::Tick` still ran the complete personal
  `SimulationSession` monster/hazard tick after draining `ZoneRuntime`, so the
  private ECS could move, damage, or kill the same objects and player.
- Revision `fb7cd29e8a0afdd09cd7f3f3592ed5fa1c6c5dff` restricts that shared
  path to personal compatibility timers and pet maintenance. It skips private
  movement retry, pet ground-drop pickup, hero/monster combat, hazards, drops,
  status/vitals, dynamic spawns, activation, and monster AI. Public movement,
  combat, death, drops, experience, and hazards remain Zone-owned.
- The personal spawn table currently retains only the Crystal respawn schedule
  and emits the revive boundary consumed by the Zone. A Zone-native
  wall-clock/checkpointed respawn scheduler remains an open architecture item;
  this bounded recovery does not claim it as strict parity.
- The 16-tile q4 search grid closes AOI blind strips and is covered against all
  safe western Deer slots. Final-source validation passed Gateway unit 653
  passed, 0 failed, 1 ignored; authority regression 1/1; Deer geometry 1/1;
  and the strict ordinary-packet q1-q4 journey 1/1 in 455.10 s (q2: three confirmed
  kills; q4: seven confirmed kills). Windows gate self-test remains 8/8.
- Only `MIR2_QA_NATURAL_MOVEMENT_DELAY_MS=10` was set; combat, damage, drops,
  harvesting, rewards, and persistence were unmodified. The prior attested EXE
  is older-source evidence and must be rebuilt from the new exact head. Global
  parity remains undefined, and same-EXE UI/live WSS, real DPI, native soak,
  human acceptance, complete semantic inventory, and formal signing stay open.

## 2026-08-27 Exact-head nonvisual Candidate-04

- Revision `4074445ccac7c73adcf34c2e6fc775210d6c8a50` was clean-built and
  packaged as `WN-CANDIDATE-04-20260827`, so the shared-Zone correction is now
  present in the attested native artifact. EXE SHA-256 is
  `60A3C78D401385E6294FB129FABA50BA9E0EE0253F1C1A572FF0B9F2B70C6CB9`
  at 66,665,472 bytes; build-attestation SHA-256 is
  `25643FE5883152FBB7BE7EC6AE68340B5810FF9712A43DB6F579885047429765`.
- The real package path exercised the `npm.cmd` PowerShell fix, staged 10,258
  files / 325,281,417 bytes, and passed its built-in plus independent
  verification with source-repository checking, PE validation, detached CMS,
  `nonvisual=true`, and `launchRequested=false`. Manifest SHA-256 is
  `043F565024955ED4570D898FB7CE6C20CBEBE02D0993895E80A1E43CBB8ED2E9`;
  aggregate SHA-256 is
  `3DCEADF75D9EE64607B5322525886C0EF0946F170A40CAB8C105E6F17AC1A325`.
- The CMS signer is internal/self-signed and the EXE is not Authenticode-signed
  by a formal publisher. This closes the exact-head nonvisual packaging gap,
  not the UI/live/DPI/soak/human/formal-signing gates or global parity.
