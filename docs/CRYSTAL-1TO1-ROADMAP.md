# Crystal / Mir2 1:1 Project Roadmap

> Latest map-environment roadmap sync: 2026-08-01 closes the source-to-browser
> path for Crystal map light overrides, `MapDarkLight`, and `WeatherParticles`.
> The generated 463-map manifest preserves the real fields, Simulation projects
> them into `MapInformation`, Gateway emits browser-facing camelCase values, and
> Web renders fixed-map/global-TimeOfDay precedence plus lazy original weather
> textures. Local Developer Compose and the current production Web test build
> default to readable Day; `?crystalLight=dynamic` restores the original UTC
> cycle, while map-specific fixed light remains authoritative. Focused GameData, Simulation, Gateway,
> frontend logic and TypeScript checks pass; final human weather appearance on
> `DogYoHyun` remains part of Player QA acceptance.

> Latest v5 failover roadmap sync: 2026-07-29 closes the final deterministic
> base-image restore gap found during main-branch landing. Standby Session
> rebinding now restores only its local movement ingress, cached transfers, and
> move sequence from the installed Zone image instead of running a second full
> world synchronization over that image. Re-exporting an installed base
> snapshot is therefore byte-identical, including static Crystal entity fields.
> The complete Gateway matrix is green: 390 library, 17 packet-trace, 2 Gate
> 11, 4 Home Tunnel, and 28 Zone RPC tests.

> Latest cross-platform delivery roadmap sync: 2026-07-25 replaces the
> Windows-only handoff path with a version-locked Docker/Dev Container contract
> for Windows, Intel Mac, Apple Silicon, and Linux. One release lock binds the
> Crystal gitlink, Node 22.18.0, npm 11.13.0, Rust 1.89.0, GitHub CLI 2.96.0,
> digest-pinned base image, and full-asset tag/content hash. Windows and
> macOS/Linux now have matching doctor/build/up/down/logs/verify/assets commands,
> transactional full-pack installers, and clean-room clone/start acceptance
> scripts. GitHub CI checks clean checkouts on all three host OS families and
> runs the real repository Dockerfile plus Gateway/Web Compose smoke on Linux;
> a separate workflow publishes amd64/arm64 GHCR images, while a Caddy overlay
> provides authenticated HTTPS/WSS for the shared second-server acceptance
> environment. Follow-up hardening isolates private GitHub credentials from the
> default workspace and every local-build image, downloads only through the
> release-locked digest/revision fetcher after remote witness validation, uses
> ephemeral Docker authentication plus standard-input credential delivery,
> supports transactional full-pack upgrades with locking/recovery, executes
> host-wrapper contracts on Windows/macOS/Linux, separates Starter and Full
> Assets acceptance witnesses, and verifies the live Gateway `/health` plus
> Player Web `/version` revisions against Git HEAD.
> Local verification passes script parsing, both Compose models, installer
> recovery/safety fixtures, all 1,440 full-pack libraries and 4,446 unique page
> hashes, native Windows startup, and a complete browser account-to-Bichon flow
> with no console warning/error. The current Windows host cannot execute Linux
> containers because firmware/WSL virtualization is disabled, so the real
> container build remains a required hosted-CI gate after push; macOS native
> Crystal `Client.exe` remains intentionally unsupported.
>
> The same round removes a StartGame hot-path copy amplification: immutable
> full/world collision caches now return shared `Arc` values instead of cloning
> complete map cell sets for every respawn query. Four focused collision and
> StartGame-spawn regressions pass without changing authoritative behavior.

> Latest distributed Mir2 workload roadmap sync: 2026-07-23 completes Gate
> 11.1-11.4. Checkpoint v4 combines the journal with a canonical complete Zone
> image and rebuilds derived occupancy/AOI/ECS state on restore. Real Crystal
> combat, item drop/pickup, cross-map handoff, monster HP, retained drops, and
> player vitals survive takeover. Four sessions on two maps then survive two
> consecutive host failures with old generations rejected. A unified operations
> binary writes atomic, versioned JSON release evidence including size and RTO.

> Final 100% Candidate roadmap sync: 2026-07-23 closes the deterministic GDI,
> chat-state, and per-object animation-phase slice. A source-audited Windows
> exporter records exact Arial 8pt/96-DPI TextRenderer output and ARGB hashes for
> eight fixed acceptance strings. The Web client uses those images only on an
> exact text/style key match and keeps accessible fallback text for dynamic
> strings. Crystal's four-line chat history, 17 chat types, channel colours,
> filtering, scrolling, and no-timestamp behavior are now modeled from real
> packets. Gateway TCP/WebSocket sessions register one shared online presence
> after StartGame and receive Crystal-cadence online-count and LineMessage
> broadcasts; QA cadence/fixed-line controls require an explicitly configured
> control token.
>
> Entity animation is now one persistent Rust/Bevy state machine per object and
> incarnation, covering standing, walking, running, attack, struck, spell,
> harvest, die, and revive action lifecycles with bounded FIFO events and seeded
> Crystal idle selection. Both GPU backends consume the same pose bridge. Final
> r40 at Bichon `0 @ 328,275`, light 1, reports a 100% automated Candidate score,
> zero critical errors/404s, 89% world similarity, 91% HUD UI, 84% chat, and 87%
> MiniMap. Current WebGPU/WebGL2 strict movement, native/Web four-action temporal
> alignment, 126/126 runtime tests, Gateway 307/307, complete frontend logic,
> typecheck, and the 1,440-library full-asset hash gate pass. Roadmap implementation status is now
> **100% Candidate**. Only the explicit final human **Accepted** decision remains;
> raw pixel identity is not asserted across independently sampled roaming
> actors, random idle frames, particles, and native/browser compositors.

> Latest developer-handoff roadmap sync: 2026-07-22 establishes a reproducible
> code-and-assets entry point for another developer. A recursive clone resolves
> the maintained Crystal handoff branch, Windows scripts bootstrap Node 22 and
> Rust 1.89 dependencies and start Gateway/Web on the documented ports, and a
> tracked manifest pins the private full-asset Release. The full pack is an
> exact, page-hash-verified closure of 1,440 shards and 4,446 unique pages,
> packaged as deterministic USTAR with safe transactional installation. The
> remote release manifest independently covers 45,398 objects with zero local
> misses and exactly 5,887 full-pack objects. The private Release now contains
> all seven parts plus its manifest, and a filtered clean clone has downloaded,
> reconstructed, extracted, and page-hash-verified all 5,887 full-pack files.
> That same clone passes both frontend production builds, starts Gateway and
> Player Web, serves health/index/shard/page probes with HTTP 200, and releases
> all ports after shutdown. This bundle is the complete converted visual atlas,
> not the native client or the separate 316-file full sound library.
> Credentialed R2 publication, Crystal-fork
> rights/visibility resolution, and physical Brazil low-end testing remain
> separate open work.

> Latest original Bichon level-6 quest-chain roadmap sync: 2026-07-18 closes
> q1-q9 as one deterministic fresh-Warrior Candidate route, superseding the
> earlier q1-q4/q5-available checkpoint below. Tasks, prerequisite links,
> hand-in XP/gold, fixed rewards, q3/q6 class reward selection, Scarecrow
> `GingerTea` Q drops, Deer harvest `DeerMeat` Q drops, and Crystal template
> equipment stats now follow the original server scripts. Monster EXP and
> quest kill credit are emitted only from player-owned melee, skill, or poison
> deaths; ambient and NPC damage cannot advance the route. The release
> vertical slice completes all nine quests with exact hand-in totals and
> reaches level 6 naturally. Gateway reward metadata, tracked objectives, and
> selectable reward UI contracts are covered by focused Rust and Web tests.
> Remaining acceptance is a human dialog/route/feel pass, then expansion from
> q10 into the broader level 1-45 route; q1-q9 backend completion is no longer
> an open roadmap item.

> Latest fresh-native visual roadmap sync: 2026-07-18 closes the visible
> TrapHexagon and Belt-compositing regressions against the post-NPC-fix Crystal
> client. r05 first established a live Lime-name baseline at `0 @ 332,275`:
> 15.0% full-window / 14.8% world changed pixels and Belt MAE 38.05. The Web
> effect nodes had correct packets, Magic/1397 frame, coordinates, and blend
> metadata but were below the z-index 2 GPU renderer inside a translated
> auto-level sprite stacking context. Effects now participate through an
> untransformed parent and receive imperative camera translation individually;
> the Belt's nearly opaque overlay frame no longer darkens transparent slots.
> Live r16 reaches 7.1% / 6.0% changed pixels, 90.1% / 91.4% similarity, world
> MAE 4.499, and Belt MAE 10.765. WebGPU and forced WebGL2 both pass a new
> effect-pixel A/B guard with 0 critical errors / 0 404s. Automated Candidate
> remains green; Accepted remains open for clean native recapture without the
> external Computer Use status bubble, chat-state alignment, residual HUD and
> minimap text, and human visual/feel review.

> Latest deterministic visual-gate roadmap sync: 2026-07-18 closes the
> compiled-`ws` Edge 150 CDP runtime blocker with complete r03/r04 Web-only
> packs at Bichon `0.map @ 332,275`. r04 filters only the known extension
> message-port closure noise, keeps real JavaScript/network errors critical,
> and records 100% automated weighted Candidate trend with 0 critical console
> errors and 0 404s. It still shows 10% full-window and 9% world thresholded
> pixel change against the fixed r01 native frame, so final human acceptance
> remains open. The same round removes a Rust packet split where a manifest NPC
> was first Lime and then overwritten White: initial ObjectNpc, snapshot, and
> visible-object bundle now share Crystal `Color.Lime`; Web correctly renders
> the first underscore-delimited line Lime and later lines White. The old fixed
> native frame predates this packet fix and must be refreshed before name-color
> pixels are judged. Next: fresh native/Web pair, then HUD/chat/effect deltas.

> Latest full-pack and low-end delivery sync: 2026-07-14 classifies and
> hash-verifies every frame slot in all 1,440 Crystal libraries, publishing
> 1,440 lazy library shards backed by 4,446 unique immutable PNG pages. Entity
> presentation now prefers those shards with legacy fallback and bounded
> Bevy/WebGL2 residency; it does not load the 62,972,592,128-byte decoded source
> set at once. Forced-low WebGL2 Bichon QA used 58,379,430 decoded atlas bytes,
> passed 28/28 movement/render assertions, and completed a reduced 403/403
> prewarm with zero failures, 404s, or critical console errors. This closes full
> offline `.Lib` conversion and the local low-tier runtime gate. Maps keep their
> regional atlas path, while HUD, audio, and effect-specific consumers remain
> dedicated. Brazil release gates still require CDN publication and physical
> 2/4 GiB Android throttled-4G soak; optional KTX2/UASTC remains a measured
> follow-up, not a WebGPU-only requirement. See
> `docs/CRYSTAL-FULL-ASSET-PIPELINE.md`.

> Latest temporal-parity roadmap sync: 2026-07-13 adds a repeatable,
> fail-closed native/Web recording and frame-diff gate for Bichon
> `0.map @ 332,275`. It exposed an applied-vs-requested scene-center race and a
> standalone-animation residency split: Rust evicted inactive image frames but
> TypeScript still suppressed their future uploads, freezing map commits and
> publishing `ready:false` poses. Pose selection now uses only the coherent
> applied map/entity center, and a validated map ACK makes Rust's resident image
> list authoritative for Web upload deduplication. On
> `bevy-90fb96239f221a47`, WebGPU passes the four-step route at 4/4 pose coverage
> with a 41ms maximum sink delay; Bevy WebGL2 passes at 4/4 and 42ms. The native
> and Web evidence emits four overlays plus heatmaps at 1-13ms action-alignment
> error. Movement/map transaction continuity is closed by automation, while
> 75.0%-76.8% full-window pixel deltas keep lighting, population, effects, and
> HUD composition explicitly open for the next visual-parity slice. Evidence:
> `docs/generated/player-qa/movement-jitter/temporal-packs/bichon-332275-left4/`.

> Latest asset/render Candidate sync: 2026-07-13 completes the first-principles
> Crystal resource pipeline and its automated dual-backend gate. All 1,440
> source libraries are deterministically hashed and parsed, v3 FrameSet action
> and secondary-effect semantics now drive production actor presentation,
> player Spell uses Crystal frame 296, and real packet effects resolve 62
> spells plus object/map effect families with directional, mask, light, and
> blend metadata. Bevy loads packed map atlas pages directly by URL through
> `AssetServer`, swaps only complete generations under an exact ACK, and shares
> ref-counted decoded-byte residency with entity atlases. Immutable CAS blobs
> and releases are uploaded before the mutable channel pointer. The complete
> offline gate is green at 38,846 assets, 99.76% renderable map references with
> 100% accounted semantics, zero missing minimaps, 450/450 SoundList entries,
> and 99.88% headline render coverage. Release WebGPU/WebGL2 browser smokes are
> green with no failed assertions or critical console errors; evidence is in
> `docs/generated/assets/latest-asset-coverage-summary.json`,
> `docs/generated/player-qa/bevy-runtime-backends/bevy-runtime-backends-asset-pipeline-final-20260713.json`,
> `docs/generated/player-qa/bevy-map-standalone/bevy-map-standalone-webgpu-20260713001211-8821c193-report.json`,
> and `docs/generated/player-qa/bevy-map-standalone/bevy-map-standalone-webgl2-20260713001238-3c4011f6-report.json`.
> The automated
> asset/render Candidate gate is closed. Final Crystal-vs-Web human visual and
> feel acceptance remains an acceptance activity, not a reason to replace the
> Web/Bevy architecture.

> Latest combat-presentation Candidate update: 2026-07-13 closes the missing
> local melee action with Crystal's actual ownership model. The browser owner
> keeps personal self id `1000`, while shared Zone action broadcasts identify
> that player as `50001`; waiting for `ObjectAttack` therefore never selected
> the local entity. Crystal instead queues `Attack1` locally before it sends
> `C.Attack` and ignores its own server action echo. Web now starts adjacent
> `melee1` locally, submits its rAF-coalesced world at normal priority, and keeps
> packet attack/struck/death state authoritative for non-local actors. Browser
> A/B moved from accepted damage plus a 900ms animation timeout to a visible
> attack in 123ms with clean 600ms completion. Evidence is
> `docs/generated/player-qa/combat/web-local-melee-attack-20260713.{json,png}`;
> frontend logic and TypeScript pass. Stable action atlases, Bevy layout refresh,
> and shared moving-object AOI relocation remain green (Bevy 100/100, Zone
> 153/153). Remaining nearby gaps are alpha-aware entity hit bounds and the
> separate moving-monster visual acceptance pass.

> Latest movement-presentation roadmap sync: 2026-07-12 closes the duplicated
> renderer-ownership slice left after mounted movement semantics were fixed.
> Local movement no longer eases once in page state and again in Bevy. Map and
> entity provenance publish atomically; one rejected frame holds the last valid
> pose for a bounded 250ms instead of changing clocks; shadow Run distance comes
> from the explicit intent target; and each local phase receives a full 100ms
> from command start without global-pulse catch-up.
>
> WebGPU r12 is 33/33 and WebGL2 r16 is 33/33 on runtime
> `bevy-bd9004a17f2873ea`. They reproduce all eight mounted-Walk offsets
> (`-6..-48px`) and all six three-cell Run offsets (`-24..-144px`) with zero
> center tearing, synthetic logical centers, shadow mismatches, pose warnings,
> console errors, or missing movement ACKs. Movement presentation is no longer
> the reason to consider a PC-only rewrite. Remaining Candidate gates are the
> multiplicative light/effect compositor, scene population and draw details,
> broader non-movement ownership, and final human visual/feel acceptance.

> Latest movement roadmap sync: 2026-07-12 completes mounted eight-phase and
> true three-cell sprint parity. Crystal source establishes six 100ms phases for
> foot movement, eight for mounted Walk, six for mounted Run, and three-cell Run
> distance for a mount or active unpaused Swift Feet when not sneaking. These
> semantics now cross one explicit `phaseCount` contract through Web prediction,
> Bevy motion, presentation Pose JSON, and frame parsing. Gateway forwards
> Session-owned mount/sneak/buff state to shared Zone authority before movement.
>
> Release report
> `docs/generated/player-qa/movement-jitter/movement-mounted-walk8-run3-webgpu-20260712-r6.json`
> is 27/27 green: two real keyboard commands move one then three cells, ACK in
> 18/22ms, observe 8/6 phases, reach the Pose sink 2/2 within 26ms, and retain
> zero pose-atomicity, rollback, queue, console, or 404 warnings. Runtime
> `bevy-78d40eb80133609c`, shared Zone 152/152, frontend logic, TypeScript, and
> dual WebGPU/WebGL2 smoke pass. Mounted movement is no longer an open roadmap
> gate. Remaining Candidate work is scene lighting/effects, actor draw details,
> broader non-movement actor ownership, and final human acceptance.

> Latest Zone cadence/live-outbound roadmap sync: 2026-07-12 completes the next
> architecture slice named below. The bounded per-Zone owner now combines
> authenticated Walk/Run/Turn execution with one monotonic 300ms global cadence;
> late cadence work is coalesced, never replayed as a burst. Personal Session
> ticks no longer advance the shared Zone, while token-fenced bounded socket
> channels push realtime owner/AOI movement, appearance, and removal packets
> independently of an observer's private runtime. Full/closed live queues retain
> reliable mailbox fallback, and critical personal side effects stay mailbox
> committed.
>
> Strict Release proof deliberately sets personal Tick to 5000ms and disables
> observer pulses:
> `docs/generated/player-qa/two-client-zone/two-client-zone-zone-owned-cadence-tick5000-release-20260712.json`
> passes every assertion with 12ms observer movement latency, 16 entities on
> both clients, one Bevy remote-motion event, 29 packed-offset matches, and zero
> decode errors, queue drops, console errors, or 404s. Both screenshots are
> scene-ready. Focused Gateway cadence/observer/combat regressions, Simulation
> 148/148, full frontend logic, TypeScript, fmt/check, and Release build pass.
>
> This is not total shared-world completion. Movement plus global cadence are
> Zone-owned, but non-movement commands and some personal side-effect commits
> still use Session paths into shared state. Full command actorization, mounted
> eight-phase motion, true three-cell sprint, scene lighting/effects, final
> human feel acceptance, and host WHEA/BIOS stabilization remain open gates.
>
> Latest movement protocol roadmap sync: 2026-07-12 verifies both sides of
> Crystal's first-run transition. Web ACK classification is now shared between
> the early page path and the movement controller, and a one-cell Run first-step
> ACK remains a confirmed degradation rather than a correction. Shared Zone
> rejects full two-cell running from standstill and emits the effective Walk;
> `shared_zone` passes 148/148. Release protocol evidence records the expired
> Walk -> Run case at 16/99ms with one degradation, zero corrections, and delta
> `(2,0)`, while the normal primed UI chain records 22/28ms ACKs, 17/1ms local
> pose latency, and exact delta `(3,0)`. This closes the clean/degraded semantic
> slice but not the scheduling architecture: a private heavy world Tick can
> still occupy the same WebSocket future. The next milestone is independent,
> bounded Zone movement ingress plus blocked-private-tick and observer-delivery
> regressions, followed by mounted eight-phase and three-cell sprint parity.
> Evidence:
> `docs/generated/player-qa/movement-jitter/movement-protocol-expired-run-degrades-release-202607120745.json`
> and
> `docs/generated/player-qa/movement-jitter/movement-normal-walk-run-chain-release-202607120750.json`.
>
> Latest default shared-clock roadmap sync: 2026-07-12 closes the clean local
> movement presentation gate. Normal URLs now enable Bevy local self/camera
> ownership and synchronous pose commit by default, with
> `?bevyLocalMotion=0&bevyPoseCommit=0` retained and end-to-end verified as the
> legacy rollback. One Crystal-compatible 100ms scene pulse advances six walk
> phases independently of ACK arrival, while shared Zone continues to own
> acceptance, collision, occupancy, correction, AOI, cooldown, and persistence.
> Default continuous and keyboard evidence report 10ms and 15ms maximum
> command-to-pose delays respectively, zero long tasks/errors, exact final
> coordinates, and native/Web four-action spans of 2701ms on both sides. The
> final 25 additive map sprites also moved from DOM fallback to a custom Bevy
> `SrcAlpha + One` material, with zero DOM world sprites in both WebGPU and
> WebGL2 smokes. Runtime `bevy-630a77b3535f95bd` passes 94/94 Rust tests and
> dual-backend coverage. This is a clean-route renderer milestone, not total
> 1:1 completion: correction/degraded-run capture, mounted eight-frame motion,
> sprint prediction, native scene population, lighting/ambient effects, and
> combat VFX remain active roadmap gates. Evidence:
> `docs/generated/player-qa/movement-jitter/temporal-crystal-native-vs-web-default-shared-clock-horizontal-20260712-001.md`,
> `docs/generated/player-qa/movement-jitter/movement-explicit-legacy-rollback-202607120623.json`,
> `docs/generated/player-qa/bevy-map-standalone/bevy-map-standalone-webgpu-20260711213830-dee09cfc-report.json`,
> and
> `docs/generated/player-qa/bevy-runtime-backends/bevy-runtime-backends-default-shared-clock-202607120620.json`.
>
> Latest early-presentation/performance roadmap sync: 2026-07-10 makes a clean
> command eligible for Bevy self/camera/DOM ownership on its first release-WASM
> frame instead of waiting for a TypeScript motion snapshot. One shared
> map/entity center and provenance-gated synchronous pose commit keep the frame
> atomic; correction, degraded-run, target mismatch, and path mismatch retain the
> old fallback. Shared Zone still owns all acceptance and authoritative state.
> Both local motion and pose commit remain separately rollback-gated. The map
> producer now emits only semantic changes, while Rust retains tiles by key,
> updates transforms in place, rebinds sprites only when images change, and stores
> a revision fingerprint instead of cloning the full state. The exact four-walk
> route now has five sampled map states rather than 53 and reaches every accepted
> pose sink in `14/18/32/16ms` under a hard 75ms budget. Evidence:
> `docs/generated/player-qa/bevy-movement-shadow/bevy-movement-shadow-webgpu-20260710220403-44ba1f45-report.json`,
> default-off compatibility
> `docs/generated/player-qa/bevy-movement-shadow/bevy-movement-shadow-webgpu-20260710221024-ce1066ce-report.json`,
> and dual backend
> `docs/generated/player-qa/bevy-runtime-backends/bevy-runtime-backends-20260710221430.json`.
> Runtime `bevy-9ce93936c0841d7e` passes 86/86 Rust tests and all focused Web
> tests/type checks. The next renderer slice is stable world-space map cells,
> perimeter/chunk deltas, and one-time atlas metadata. Default promotion still
> requires exact native clean/correction/degraded-run temporal evidence and human
> feel acceptance; this milestone is not a claim that total 1:1 parity is done.
>
> Latest local-presentation roadmap sync: 2026-07-10 lands the shadow-first,
> rollback-gated Bevy self/camera slice. The bounded Rust local-motion resource
> consumes copies of normalized commands and ACKs in `PreUpdate`, then can drive
> packed self, camera, and DOM overlays through one `localCommand` pose. Shared
> Zone remains authoritative for movement acceptance, correction, collision,
> occupancy, cooldown, AOI, and persistence. The feature remains default-off via
> `?bevyLocalMotion=1` / `mir2-bevy-local-motion`. Takeover requires matching
> object, target, and from/to path; correction clears the segment and any
> degraded/rebased path mismatch stays on the previous TypeScript `selfWindow`.
> Runtime `bevy-e50cfdd1e6c8d229` passes 83/83 Rust tests, 6/6 pose-parser tests,
> 9/9 movement-bridge tests, TypeScript, and validated WebGPU/WebGL2 releases.
> Evidence:
> `docs/generated/player-qa/bevy-runtime-backends/bevy-runtime-backends-20260710173210.json`,
> default-off route
> `docs/generated/player-qa/bevy-movement-shadow/bevy-movement-shadow-webgpu-20260710173245-17db8e6b-report.json`,
> forced-on route
> `docs/generated/player-qa/bevy-movement-shadow/bevy-movement-shadow-webgpu-20260710173356-7b3abddd-report.json`,
> and map regression
> `docs/generated/player-qa/bevy-map-standalone/bevy-map-standalone-webgpu-20260710173500-ca321fe7-report.json`.
> Both real routes have 4/4 commands, 4/4 ACKs, 0 jumps and exact geometry
> (76/76 off and on); forced-on finishes with self and camera owned by
> `localCommand`. The next acceptance gate is temporal rather than geometry:
> command timing leads the delayed TS window by as much as 32px / 326ms. Capture
> exact native Crystal vs Web off/on frame sequences, include correction and
> degraded-run cases, and only then decide whether to make local takeover the
> default. Additive Bevy materials and the native Gateway multi-session crash
> remain separate roadmap lanes.
>
> Latest unified-presentation roadmap sync: 2026-07-10 closes the duplicate
> sprite/camera/DOM interpolation stage. The packed state now identifies self;
> Rust computes one camera screen pose per frame, derives self as its exact
> inverse, records each remote sprite's actual selected offset, and publishes a
> bounded/versioned pose snapshot for DOM overlays. The DOM consumer validates
> schema, source, bounds, clock domain, and <=250ms freshness before use, then
> safely falls back to TypeScript. Bridge disable is isolated from Bevy render
> ownership. A real movement run initially found two 20/22px jumps from the old
> independently sampled self/camera windows; the unified derivation removed the
> race and the strengthened rerun passes with 0 jumps. Runtime
> `bevy-8a40d0bdcf0dc14a` passes 72/72 Rust tests, 5/5 pose-parser tests, 9/9
> movement-bridge tests, TypeScript, and dual release packages. Evidence:
> `docs/generated/player-qa/bevy-runtime-backends/bevy-runtime-backends-unified-pose-20260710.json`,
> `docs/generated/player-qa/bevy-movement-shadow/bevy-movement-shadow-webgpu-20260710163125-1a4aff1b-report.json`,
> and
> `docs/generated/player-qa/bevy-map-standalone/bevy-map-standalone-webgpu-20260710162936-ca18422e-report.json`.
> The next roadmap slice is a shadow-first, rollback-gated migration of local
> self prediction and ACK reconciliation into Bevy presentation. Shared Zone
> remains the sole authority for command acceptance, correction, collision,
> occupancy, cooldown, AOI, and persistence. The native Gateway two-client crash
> remains a separate network-evidence blocker, not a renderer result.

> Latest Bevy movement roadmap sync: 2026-07-10 promotes remote packet motion
> from read-only shadow state into a guarded presentation source. Normalized
> remote motion/remove events are copied into a bounded `PreUpdate` resource;
> packed Bevy sprites use its Crystal-stepped offset only when the segment target
> equals the latest packed grid target, with the previous TypeScript window as a
> fallback. Connected segments continue from the displayed fractional pose,
> stale/out-of-order events are ignored, discontinuities snap, remove/disable
> clears state, and `?bevyRemoteMotion=0` provides a rollback switch. This is
> presentation-only: shared Zone still owns collision, occupancy, cooldown, AOI,
> correction, and persistence. Runtime `bevy-63449641a633efc2` passes 67/67 Rust
> tests (13 focused remote-presentation) and 9/9 TypeScript bridge tests. Real
> Chrome/WASM backend evidence
> `docs/generated/player-qa/bevy-runtime-backends/bevy-runtime-backends-remote-motion-probe-20260710.json`
> proves target-mismatch fallback, matched-target Bevy takeover, disable cleanup,
> and zero decode/event drops in default/forced WebGPU and forced WebGL2. Latest
> map and shadow regressions are also green. Unified packed sprite, self-camera,
> and DOM pose is complete in the sync above; local-prediction ownership remains
> the next guarded migration.
> Real two-client packet-to-render evidence remains blocked by the separately
> recorded native Gateway multi-session/reconnect crash at
> `docs/generated/player-qa/two-client-zone/two-client-zone-native-crash-20260710.json`;
> renderer-probe success is not being misreported as network proof.

> Latest Bevy/WebGPU architecture sync: 2026-07-10 moves every normal-blend
> map atlas miss into Bevy standalone textures while retaining only
> Crystal-additive glows in the DOM. A stable runtime-started state now drives
> map readiness and world-snapshot emission; diagnostic status phases no longer
> collapse the running lifecycle. WebGL2/DOM map ownership remains active until
> Rust acknowledges a complete `map-render-synced` frame for all viewport atlas
> pages, and standalone tiles use the same acknowledgement before hiding their
> DOM fallback. Evidence
> `docs/generated/player-qa/bevy-map-standalone/bevy-map-standalone-webgpu-20260710162936-ca18422e-report.json`
> passes with 421 atlas tiles, 109 standalone draws, 108 decoded standalone
> images, 7 atlas pages / 115 images, 0 image failures, 0 map 404s, and 0
> critical console errors; the 25 remaining DOM world sprites are all additive.
> `docs/generated/player-qa/bevy-runtime-backends/bevy-runtime-backends-unified-pose-20260710.json`
> also passes default/forced WebGPU and forced WebGL2 for runtime
> `bevy-8a40d0bdcf0dc14a`. Remote motion and unified presentation pose are now
> complete in the syncs above; next comes local self prediction/reconciliation.
> Shared Zone collision, cooldown, occupancy, AOI, and final transforms remain
> server-authoritative and are not duplicated into the client renderer.

> Latest main-scene light render roadmap sync: 2026-07-09 converts dynamic
> `lightSetting` from data into visible world ambience. Web now renders
> `.viewport-crystal-light-overlay` for Dawn/Evening/Night while leaving
> Day/Normal untouched, and the layer sits after sprites but before nameplates
> so scene pixels darken without dimming labels or UI. Evidence
> `docs/generated/player-qa/visual-parity/scene-light-render-20260709/`
> records the clean Night screenshot and DOM state:
> `overlayClass=viewport-crystal-light-overlay night`, `overlayLight=4`,
> `z-index=6`, `pointer-events=none`, `tutorialOpen=false`, and browser console
> errors `0`. The same round now carries Crystal map-cell `light` values into
> `OriginalMapCell.light` and renders map-cell light nodes when non-Day overlays
> are active; `map-light-export-probe-20260709.json` confirms map `0` samples
> with 127 / 127 / 25 / 26 light cells. A fresh map-light DOM screenshot remains
> pending because the real Crystal UTC light window rotated to Day. Roadmap
> priority now moves to Night/Evening/Dawn recapture or a safe QA-only light
> override, intensity tuning against native same-time screenshots, and
> object/equipment/effect light sources.

> Latest dynamic TimeOfDay/lightSetting roadmap sync: 2026-07-09 closes the
> data/packet side of world light parity. Crystal source seeds `Envir.Now` from
> `DateTime.UtcNow` and maps `Now.Hour * 2 % 24` to Dawn/Day/Evening/Night in
> `AdjustLights()` before broadcasting `S.TimeOfDay`. Simulation StartGame and
> `WorldSnapshot.lightSetting` now follow that same UTC-hour formula, and Web
> applies the snapshot field plus exposes `window.__mir2Stage5.state.lightSetting`
> for automated capture. Evidence
> `docs/generated/player-qa/visual-parity/light-setting-snapshot-20260709/`
> records direct WS `TimeOfDay.lights=4`, `worldSnapshot.lightSetting=4`, and
> browser state `lightSetting=4` with 0 critical console errors / 0 non-favicon
> 404s. The later main-scene light render pass above starts the visual layer;
> remaining work is object/map light sources and intensity tuning before
> same-coordinate visual packs measure camera/world/HUD transparent-slot deltas.

> Latest Crystal/Web visual-feel roadmap sync: 2026-07-09 established a newer
> same-coordinate evidence ladder at `0 @ 335,266`. Pack
> `docs/generated/player-qa/visual-parity/crystal-web-pack-20260709-0060-minimap-source-panel-viewrect-native335266-clean/`
> validates the rebuilt Gateway/native-state path with runtime/layout/entities
> `100%`, MiniMap `86%`, HUD UI `86%`, and matched Web vitals/items/gold/belt.
> Web capture now treats `crystalVisibleChatLines` as a replacement snapshot
> instead of appending over startup logs, and string inference maps Crystal
> `[Mode: ...]`, `[Pet: ...]`, and `Now in Net:N` to the green/blue ChatDialog
> styles. Belt parity also advanced: Web now renders belt quantity `1` for
> consumables and uses Crystal-like black shortcut labels plus yellow counts.
> Pack 0065 shows the Belt text/data correction is visible, but remaining
> `hud-belt=78%` is mostly transparent-slot exposure of world camera/light
> mismatch. Roadmap priority should therefore move from chat/Belt text tweaks to
> camera viewport alignment, shared visible object/AOI parity, world lighting
> render, and movement/video feel capture.

> Latest fair-coordinate/MiniMap light roadmap sync: 2026-07-09 improves both
> the evidence lane and a visible MiniMap mismatch. The capture harness now
> treats `qa.applyNativeState` as incomplete until Web matches the requested
> `mapFileName` and `position.x/y`, and the Gateway shared-Zone route now syncs
> `qa.applyNativeState` transforms back into Zone presence instead of allowing
> stale authoritative coordinates to overwrite the personal session. Evidence
> `docs/generated/player-qa/visual-parity/crystal-web-pack-20260709-0056-main-hud-fair-visible-coord/`
> proves the coordinate lock with Web `player` and `authoritativePlayer` both
> at `334,263`, runtime/layout/entities `100%`, and overall `99.5%`. The next
> historical pass changed Simulation StartGame's fixed `TimeOfDay` bootstrap
> from Night (`lights=4`, MiniMap `2092`) to Day (`lights=2`, MiniMap `2093`),
> matching that Crystal Bichon capture; the dynamic TimeOfDay/lightSetting
> roadmap sync above now supersedes fixed Day. Evidence
> `docs/generated/player-qa/visual-parity/crystal-web-pack-20260709-0057-minimap-light-day-bootstrap/`
> records Web `miniMapLight.originalSrc=/original-ui/Prguse/2093.png`, 0
> network 404s, 0 critical console errors, runtime/layout/entities `100%`, and
> MiniMap `0.786` / meanAbsDelta `32.545`. Remaining roadmap work in this lane
> is true MiniMap raster/color/marker parity and world camera/object-frame
> parity, not coordinate-lock or light-icon drift.

> Latest Main HUD content-y roadmap sync: 2026-07-09 closes the stable
> main-HUD 2px vertical drift while preserving the stage anchor. Web keeps
> `.main-hud-shell` at `0,616` for layout parity, but shifts the inner
> `.main-hud` content by `top: 2px`. 0050/0054 crop analysis showed
> `hud-left`, `hud-right-controls`, `hud-right-status`, and
> `hud-bottom-center` all aligned best with that main-HUD-only downward shift,
> while independent `hud-belt` and `chat` did not. Evidence
> `docs/generated/player-qa/visual-parity/crystal-web-pack-20260709-0055-main-hud-content-y-offset/`
> records rightControls improving from `0.720/49.436` to `0.986/0.303`,
> rightStatus from `0.734/42.642` to `0.824/14.189`, bottomCenter from `0.800`
> to `0.886`, and hudUi from `0.782/34.113` to `0.856/15.453`, with 0 network
> 404s, 0 critical console errors, and runtime/layout/entities `100%`. 0055 is
> kept as a HUD-specific proof because dynamic world/minimap/chat state makes
> it unfair as the new overall baseline (`overall=95.9%`).

> Latest Belt/HUD roadmap sync: 2026-07-09 fixes the Belt panel overlay draw
> order against Crystal source. Crystal `BeltDialog` draws the half-opacity
> overlay (`1933` / `1945`) inside `BeltPanel_BeforeDraw`, and
> `MirControl.Draw()` invokes `BeforeDrawControl()` before `DrawControl()`, so
> the overlay belongs behind the main Belt frame (`1932` / `1944`). Web had
> the DOM order reversed, placing the overlay above the base frame and
> visibly darkening the Belt. Evidence
> `docs/generated/player-qa/visual-parity/crystal-web-pack-20260709-0054-belt-overlay-draw-order/`
> records `hudBelt` improving from 0050's `0.765` / meanAbsDelta `48.963` to
> `0.791` / `38.920`, with `hudUi` moving from `0.778` / `35.215` to
> `0.782` / `34.113`. 0054 is kept as a Belt-specific proof because dynamic
> chat prevented it from becoming the new overall fair baseline.

> Latest same-scene evidence-tooling roadmap sync: 2026-07-09 makes the visual
> pack output easier to audit by having `capture-crystal-web-pack.mjs`
> automatically emit native/Web crop pairs for the exact report regions:
> `world`, `hud-full`, `hud-left`, `hud-belt`, `hud-right-controls`,
> `hud-right-status`, `hud-bottom-center`, `minimap`, and `chat`. Evidence
> `docs/generated/player-qa/visual-parity/crystal-web-pack-20260709-0053-auto-region-crops/`
> records 9 generated crop pairs in `summary.cropSet`. This is not promoted as
> a new fair visual baseline because native chat state rotated during the run
> (`chat=67%`, overall `96.9%`), but it preserves the 0050 right-status metric
> (`hudRightStatus=0.734`) and gives future HUD/MiniMap/world passes immediate
> side-by-side crop artifacts. 0051/0052 showed that the attempted CSS
> GDI-outline HUD text variants did not improve over 0050, so that experiment
> was left as diagnostic evidence only.

> Latest clean visual-baseline roadmap sync: 2026-07-09 adds
> `crystalVisibleChatLines` support to the same-scene capture lane and records
> `docs/generated/player-qa/visual-parity/crystal-web-pack-20260709-0050-chat-visible-slots-current/`
> as the latest fair automated baseline. This avoids comparing Web against a
> random native `LineMessage.txt` rotation or stale native chat scroll state
> while preserving normal client behavior outside explicit capture URLs. 0050
> reaches overall `98.5%`, pixel trend `96%`, runtime/layout/entities `100%`,
> chat `83%`, HUD full/UI `78%`, world `83%`, MiniMap `80%`, and keeps the
> 0046 source-backed weight-bar fill (`fillWidth=16`, `hudRightStatus=0.734`).
> The remaining roadmap work now points at true visual deltas: HUD
> asset/color/antialias drift, MiniMap raster/color sampling, world
> scene/object-frame mismatch, and movement/video feel evidence.

> Latest chat capture roadmap diagnostic: 2026-07-09 makes the same-scene pack
> less brittle by adding explicit `--gatewayWs` and `--crystalLineMessage`
> controls to `capture-crystal-web-pack.mjs`. The 0047 evidence pack proves Web
> can render the intended native startup LineMessage and preserve the 0046
> weight-bar fix in the same run, with 0 network 404s and 0 critical console
> errors. The remaining chat gap is now more precise: native Crystal's
> `ChatDialog` line slots can include an empty/filtered slot before the visible
> LineMessage, while Web currently renders the seeded startup lines
> contiguously. Roadmap priority for chat is therefore `History` / `StartIndex`
> behavior, not only LineMessage text selection.

> Latest HUD weight-bar roadmap sync: 2026-07-09 closes a source-backed
> rightStatus semantic/render mismatch in the main HUD. Crystal
> `MainDialogs.cs` sets the `WeightBar` control to `DrawImage=false` and then
> clips the fill in `WeightBar_BeforeDraw` to
> `(WeightBar.Size.Width - 2) * CurrentBagWeight / Stats[BagWeight]`, using
> `Prguse/76` under 50%, `UI_32bit/473` up to 75%, and `UI_32bit/472` above
> 75%. Web now follows that fill-width and color-resource decision instead of
> drawing a full green 76px bar, and the missing `UI_32bit` frames are exported.
> Evidence
> `docs/generated/player-qa/visual-parity/crystal-web-pack-20260709-0046-weightbar-source-fill/`
> records `currentWeight=14`, `maxWeight=62`, `weightRatio=0.2258`,
> `fillWidth=16`, 0 network 404s, 0 critical console errors, and a measured
> `hudRightStatus` improvement from `0.727/45.137` similarity/meanAbsDelta in
> 0045 to `0.734/42.642`. Remaining HUD roadmap work is now true
> asset/color/brightness and antialias parity around the right-status cluster,
> not the weight-bar fill semantics.

> Latest HUD right-button roadmap sync: 2026-07-09 closes the source-backed
> 1px coordinate drift in the main-HUD rightControls cluster. Crystal
> `MainDialogs.cs` positions the 1024px HUD buttons at
> `Size.Width - 105/55/119/96/73/50/27` (`919`, `969`, `905`, `928`, `951`,
> `974`, `997`); Web previously rendered the same buttons 1px left of those
> anchors. Web CSS now uses the Crystal coordinates. Evidence
> `docs/generated/player-qa/visual-parity/crystal-web-pack-20260709-0045-hud-right-button-source-coords/`
> records 0 network 404s, 0 critical console errors, runtime/layout/entities
> `100%`, and a modest rightControls metric improvement from 0042's
> `0.715/51.576` similarity/meanAbsDelta to `0.720/49.436`. Remaining HUD
> work in this area is now asset/color/brightness parity rather than button
> coordinate parity.

> Latest Belt/HUD roadmap sync: 2026-07-09 closes a small but visible
> source-backed Belt mismatch and improves HUD measurement. Crystal
> `BeltDialog` puts shortcut labels directly on the belt at `(8 + i*35, 2)`
> and item cells at `(i*35 + 12, 3)`, so labels `1` and `2` remain visible over
> occupied potion slots. Web now renders those labels as direct belt children
> with Crystal parent coordinates and a higher z-index instead of nesting them
> inside slots. The same pass extends visual reports with `hudLeft`, `hudBelt`,
> `hudRightControls`, `hudRightStatus`, `hudBottomCenter`, and aggregate
> `hudUi` metrics so HUD fixes can target clean UI areas rather than only the
> full transparent HUD crop. Evidence
> `docs/generated/player-qa/visual-parity/crystal-web-pack-20260709-0044-belt-key-label-diagnostics/`
> records label rects `1 @ 238,620 26x14` and `2 @ 273,620 26x14`, 0 network
> 404s, 0 critical console errors, runtime/layout/entities `100%`, and
> `hudUi=78%` with subregions `left=79%`, `belt=77%`, `rightControls=72%`,
> `rightStatus=73%`, and `bottomCenter=80%`. The roadmap priority for HUD now
> narrows to rightControls/rightStatus asset/color drift, belt background
> brightness/overlay parity, and chat-line capture stability before broader
> world/MiniMap/video work.

> Latest MiniMap roadmap sync: 2026-07-09 closes the source-backed MiniMap
> coordinate-format, label-box, light-icon, and radar-dot mismatches in the
> native-state same-scene lane. Web now uses Crystal `Functions.PointToString`
> style coordinates (`335, 262`), pins MiniMap labels to Arial, keeps the
> coordinate label in Crystal's `56x18` vertically centered box, exports missing
> `Prguse` light frames `2092`, `2094`, and `2095`, and maps Crystal light
> states to the same icon indices as native (`Normal/Day=2093`, `Dawn=2095`,
> `Evening=2094`, `Night=2092`). The radar overlay now draws Crystal-style 2x2
> `RadarTexture` rects at `(x - 0.5, y - 0.5)`, skips dead entities, and keeps
> player/NPC/other/owned-object colors aligned where Web state exposes
> ownership. Evidence
> `docs/generated/player-qa/visual-parity/crystal-web-pack-20260709-0042-minimap-radar-dot-label-welcome/`
> keeps runtime/layout/entities at `100%`, with overall `98%`, estimated human
> band `91-100%`, pixel trend `96%`, HUD `78%`, world `83%`, minimap `80%`,
> chat `83%`, and MiniMap meanAbsDelta `29.535` versus 0039's `29.718`. Roadmap
> priority remains the larger visible deltas: bottom-panel HUD asset/layout and
> text placement, true MiniMap raster crop/color sampling parity, world
> scene/frame mismatch, and then movement/video evidence on this native-state
> lane.

> Latest HUD text roadmap sync: 2026-07-09 closes the current bottom-right gold
> text-format mismatch and removes a Web-only serif-font leak from the main HUD.
> Web now mirrors Crystal `MainDialogs.cs` by formatting `GoldLabel` with the
> `###,###,##0` grouping pattern, so the native-state character renders
> `3,457` instead of raw `3457`; the Web main HUD also explicitly uses Arial,
> matching Crystal `Settings.FontName = "Arial"`. Evidence
> `docs/generated/player-qa/visual-parity/crystal-web-pack-20260709-0036-hud-font-arial-cleanline/`
> keeps the same-scene pack at overall `98%`, estimated human band `91-100%`,
> pixel trend `95%`, HUD `77%`, chat `82%`, world `83%`, minimap `79%`, and
> runtime/layout/entities `100%`, with crop pairs attached. The earlier
> gold-only pass
> `docs/generated/player-qa/visual-parity/crystal-web-pack-20260709-0034-hud-gold-format/`
> scored HUD `78%`; the Arial step is therefore kept as source-backed visual
> cleanup while roadmap priority remains the larger visible deltas:
> bottom-panel HUD asset/layout and text placement, minimap crop/color, world
> scene/frame mismatch, and then movement/video evidence on this native-state
> lane.

> Latest ChatDialog/HUD orb roadmap sync: 2026-07-09 closes the current
> startup chat-panel content/state slice and fixes the most visible left-HUD
> orb crop defect. Web now keeps Crystal's packet-visible `Welcome` in older
> history while matching the native visible four-line window with rotating
> `LineMessage` support through `?crystalLineMessage=...`;
> `ChatType.LineMessage` uses the Crystal blue/white label style; chat rows
> render as AutoSize-width labels; the empty input box is hidden until chat text
> exists, matching `ChatTextBox.Visible=false`; and low-level Warrior HP-only
> mode uses the full red orb instead of the old half-width HP/MP crop. Evidence
> `docs/generated/player-qa/visual-parity/crystal-web-pack-20260709-0033-chat-and-hp-orb-clean/`
> keeps the same-scene pack at overall `98%`, estimated human band `91-100%`,
> pixel trend `96%`, chat similarity `83%`, HUD `78%`, and
> runtime/layout/entities still `100%`. Roadmap priority now moves to the
> remaining visible deltas: bottom-panel HUD asset/layout and text placement,
> minimap crop/color, world scene/frame mismatch, and then movement/video
> evidence on this same native-state lane.

> Latest bottom-right HUD roadmap sync: 2026-07-09 resolves the most concrete
> HUD semantic mismatch left by the native-state comparison. Web now mirrors
> Crystal `MainDialogs.cs` for the right-side main-HUD readouts: remaining bag
> weight is computed from Crystal player `BagWeight` stats, free space uses the
> Crystal 46-slot inventory view including belt slots, and the gold row is
> visible below the readouts. Evidence
> `docs/generated/player-qa/visual-parity/crystal-web-pack-20260709-0027-hud-weight-diagnostics/`
> records `currentWeight=14`, `maxWeight=62`, HUD `48 / 38`, gold `3457`, and
> expected HUD `48 / 38` with runtime/layout/entities still `100%`. Roadmap
> priority remains visual-feel work, but the next HUD slice should focus on
> chat panel state/overlap and bottom-panel asset drift rather than these
> bottom-right status numbers.

> Latest native-state/max-MP/EXP same-scene roadmap sync: 2026-07-09 closes the immediate
> Web-vs-Crystal account-state blocker for automated Candidate scoring. The
> pack harness now converts native `Server.MirADB` account state into both a
> Web account-store save and a live QA character payload, then applies that
> payload through the token-gated local QA-control wrapper without exposing raw
> debug commands to normal clients. `WorldSnapshot` now also carries
> `playerMaxMp`, so the Web state remains `MP 32/32` after transfer instead of
> degrading to `32/?`. The Web upsert path now reads Crystal `ExpList.ini`,
> giving the native level-6 character EXP `435/900` and HUD `48.33%` instead
> of the placeholder Web `100.00%`. Evidence
> `docs/generated/player-qa/visual-parity/crystal-web-pack-20260709-0025-exp-debug/`
> aligns the same character at `0 @ 335,262` with native HP/MP/gold,
> EXP, inventory, belt, and equipment, clears the missing potion icon 404s, and
> scores overall `94%` with runtime/layout/entities `100%` and estimated human
> band `87-100%`. Roadmap priority now moves from "make the comparison fair" to
> the real client deltas surfaced by that fair comparison: tune HUD assets,
> align minimap crop/color and chat contents, reduce world scene/frame mismatch,
> then attach movement/video feel evidence to the same pack family. The
> bottom-right status semantics called out by this pass are superseded by the
> 0027 HUD sync above.

> Latest HUD-state roadmap sync: 2026-07-09 adds the missing evidence bridge
> between native Crystal account state and Web visual scoring. The same-scene
> report now emits dynamic-state diagnostics, and the new read-only
> `extract-crystal-account-state.mjs` parser can consume local
> `Server.MirADB` plus the generated Crystal item manifest to identify the
> actual native character state used by screenshots. Evidence
> `docs/generated/player-qa/visual-parity/crystal-web-pack-20260709-0019-hud-state-diagnostics/`
> proves the current HUD/chat penalty is heavily state-polluted: Web captured
> the same visible character name as a fresh level-1 starter with HP `18/18`,
> MP `14/?`, gold 0, empty belt/inventory, and starter Web equipment, while
> native Crystal has level 6, HP 51, MP 32, gold 3457, potion belt slots, and
> eight equipped Crystal items. Roadmap priority is now to align Web capture
> state from native account data before spending more time on HUD art tuning;
> after state is matched, rerun the same-scene pack and then address residual
> HUD assets, minimap crop/color, and chat content.

> Latest Crystal/Web visual-effect roadmap sync: 2026-07-09 lands the current
> additive map-effect parity pass and a cleaner same-account evidence lane. The
> Web renderer now treats Crystal glow frames as a special DOM blend layer over
> the Bevy map atlas, matching Crystal's `SourceAlpha + One` intent more
> closely than normal alpha quads. The capture harness can now create/use a Web
> character with the same visible name as the native client via
> `--createAccount` / `--characterName`. Evidence
> `docs/generated/player-qa/visual-parity/crystal-web-pack-20260709-0017-same-account-native335/`
> records same-name map `0 @ 335,262` with overall `97%`,
> runtime/layout/entity health `100%`, pixel trend `92%`,
> `domBlendSpriteCount=12`, and no 404 or critical console errors. This is
> still automated Candidate evidence, not final human acceptance: roadmap
> priority now moves to the remaining visible client deltas called out by the
> report, especially HUD state/assets, minimap crop/color, chat panel state,
> and Web HP/MP/equipment/belt state alignment against the native character.

> Latest QA-control roadmap sync: 2026-07-08 lands the safe automation control
> lane needed for overnight parity work. The gateway now accepts local
> `qaControl` commands only with `MIR2_GATEWAY_QA_CONTROL_TOKEN`, while normal
> production clients still reject debug movement, raw Stage5 commands, and debug
> transfer. Evidence
> `docs/generated/player-qa/combat-survival-default-selfcamera-rust7111-qacontrol2-20260708/report.md`
> confirms the route can drive Rust `7111` with production safety on and pass
> incoming damage plus death/revive. Roadmap priority now moves to making that
> control lane deterministic with explicit ACK/settle semantics, then closing
> the visible client gaps: DOM damage floaters, seeded pickup movement, and
> normal kill/XP/drop.

> Latest hostile-retaliation roadmap sync: 2026-07-08 upgrades the combat lane
> from "incoming damage unproven" to "incoming damage verified, completion
> evidence blocked by control stability." The attack-trace harness now records
> target map/object id, sent attack frames, melee approach, delayed server
> combat packets, and `StartGame` retry attempts. Evidence
> `docs/generated/player-qa/combat-survival-default-selfcamera-rust7111-survivalattacktrace5-20260708/report.md`
> reached melee with natural `ForestYeti` object `258949`, sent 24 attack
> frames, observed target `ObjectAttack` / `ObjectStruck` /
> `DamageIndicator`, and dropped player HP `18 -> 3`. Roadmap priority now
> shifts to deterministic test control and completion parity: make safe
> transfer/spawn/death-revive isolation reliable without exposing debug paths to
> normal clients, then rerun normal attack-kill, `ObjectDied`, XP, and loot
> evidence from the same Web/Rust route.

> Latest Web/Zone action-parity roadmap sync: 2026-07-08 moves the current
> default self-camera loop forward from "pickup/death red" to "deterministic
> pickup and death/revive green, combat completion still open." The Web client
> now exposes packet-ACK `authoritativePlayer` and uses it for pickup/action
> gating instead of predicted/render self. The combat QA harness records
> authoritative self, inventory plus belt carried items, WS frames, pickup
> attempts, and an opt-in QA Blue Potion seed. Gateway now syncs personal
> session ground drops into shared Zone pickup and aligns session fallback item
> commands to the current Zone transform; shared normal chat stays Zone-native
> while `@DIE` falls back to personal-session GM handling. Evidence
> `docs/generated/player-qa/combat-survival-default-selfcamera-rust7111-authpickupseed7-20260708/report.md`
> passed pickup (`carried 0 -> 1`, `GainedItem x1`) and death/revive
> (`playerHp 0 -> 18`, respawn `0:330,270`) on Rust `7111`. Roadmap priority
> now narrows to unseeded combat progression: make a fresh player reliably
> find/kill a field monster, receive retaliation or explain neutral monsters,
> emit `ObjectDied`/XP/drop without QA seed, and clear remaining sound 404s.

> Latest Rust-gateway combat/effect roadmap sync: 2026-07-07 upgrades combat
> evidence from "attack packets produce no outcomes" to "authoritative damage
> and visible damage numbers work, but kill/death/loot/XP are still not
> accepted." The Web client now schedules targeted combat-confirm ticks after
> attack/range/cast commands, and the combat QA harness can move through normal
> `walk` packets when the WebGL2 scene has no DOM tile hitbox. Evidence
> `docs/generated/player-qa/combat-survival-default-selfcamera-rust7111-floaterfix30s-20260707/report.md`
> connected to Rust `7111`, completed 11/11 beats, landed melee damage, drove
> target HP below 100%, and passed the DOM damage-floater gate (`4`
> `DamageIndicator` packets, `.scene-damage-floater` peak `1`). Roadmap
> priority now narrows to gameplay completion parity: make a fresh level-1
> player reliably kill a field monster within an accepted window, emit
> `ObjectDied` / XP / loot evidence, and fix the normal-client death/revive
> lifecycle (`@DIE` currently does not transition to dead). Asset follow-up
> remains missing original UI sound/monster metadata coverage.

> Latest Rust-gateway combat/effect roadmap sync: 2026-07-07 turns the combat
> lane from "needs a valid harness" into a concrete Rust `7111` backend parity
> blocker. The hardened default self-camera harness writes per-beat partial
> reports, uses atomic report writes, avoids known field safe-zone circles, and
> now starts combat from Woomyon anchor `1:315,100`. Evidence
> `docs/generated/player-qa/combat-survival-default-selfcamera-rust7111-anchor-20260707/report.md`
> connected to `ws://127.0.0.1:7111/ws` with `gatewayIsRust=true` and attacked
> a non-safe-zone `ForestYeti` in melee, but combat outcomes did not advance:
> no `ObjectStruck`, no `DamageIndicator`, no target `ObjectHealth` drop, no
> `ObjectDied`, no player HP loss from retaliation, and no dead-state after
> `@DIE`. Roadmap priority now moves below movement smoothing into the
> gateway/Zone/server lifecycle: route real client `attack` intents into
> authoritative damage, surface damage-floater packets to Web, wire player
> death/revive for normal client commands, then rerun this same anchor evidence
> until combat/effect survival is green. Asset follow-up from the same run:
> fill `Sound/103.wav` and Monster `007` original-ui metadata coverage.

> Latest combat/effect roadmap probe: 2026-07-07 opens the next parity lane
> after movement. `docs/generated/player-qa/combat-survival-default-selfcamera-20260707/report.md`
> captured 11 default self-camera combat screenshots and completed the harness,
> but remains red evidence: `7111` Rust gateway was unavailable, the client ran
> via `7110`, hunting-field transfer/engagement was unreliable, attack-kill and
> damage-floater checks did not pass, and death/revive failed. The run did prove
> one useful surface (`playerHp 18 -> 9` during survival), so the next roadmap
> item is to stabilize Rust-gateway combat/effect automation until
> attack-kill, damage floaters, death/revive, loot, and XP can be judged from
> strong same-scene evidence.

> Latest held/chorded roadmap sync: 2026-07-07 promotes default self-camera
> verification from the four-click Bichon route to keyboard-held and chorded
> movement. Chorded cardinal evidence
> `docs/generated/player-qa/movement-jitter/web-motion-keyseq-bichon-cardinal-default-selfcamera-windowfps-content-jpeg-20260707-2000.json`
> is strict-green with 148 JPEG frames, 8 movement commands, no rollback, and no
> interaction pollution. Held Shift+Right first found a prediction cleanup gap
> (`predicted 332,270` briefly fell back to server `331,270` between run ACKs);
> `apps/web/app/page.tsx` now keeps fresh, unconsumed direction
> `queuedMoveIntent` state as movement transport evidence. Verified held rerun
> `docs/generated/player-qa/movement-jitter/web-motion-heldrun-bichon-right-default-selfcamera-windowfps-content-queuedintentfix-jpeg-20260707-2000.json`
> is `ok=true` with 122 JPEG frames, 8 movement commands, average ACK
> `198.5ms`, max ACK `439ms`, final `345,270`, no logical rollback, no failed
> assertions, and no console/network failures. Roadmap next: equal-duration
> native held/video evidence, then busier combat/effect scenes and HUD/chat
> temporal polish.

> Latest default self-camera roadmap sync: 2026-07-07 closes the measured
> equal-cadence movement-intensity gap for the current four-click Bichon route.
> The Bevy self-camera + per-entity interpolation path is now requested by
> default and only activates when the Bevy entity/map renderer is live; URL and
> localStorage escape hatches remain. The residual DOM self overlay now cancels
> the parent camera transform, eliminating the jump failures seen in the first
> opt-in self-camera probe. Native evidence
> `docs/generated/player-qa/movement-jitter/original-motion-computeruse-route-bichon-4click-highfps-20260707-2000.json`
> is `ok=true`, 104 JPEG frames at `50.17ms` average cadence, and 4 real native
> clicks. Matching default-URL Web content-only evidence
> `docs/generated/player-qa/movement-jitter/web-motion-clicksequence-bichon-samedir-4click-windowfps-content-default-selfcamera-jpeg-20260707-2000.json`
> is strict-green with 105 JPEG frames at ~50ms cadence, 4/4 Walk ACKs
> averaging `139.25ms` with max `369ms`, no visual jumps, no interaction
> pollution, and no console/network failures. The temporal report
> `docs/generated/player-qa/movement-jitter/temporal-native-highfps-route-vs-web-windowfps-content-default-selfcamera-clicksequence-bichon-20260707.md`
> records normalized delta/sec Crystal `63.7831` vs Web `62` (Web ratio
> `0.972`) and changed-pixel/sec Crystal `1.718936` vs Web `1.7788` (Web ratio
> `1.0348`). Roadmap next: broaden default self-camera evidence to held/chorded
> movement plus combat/effect-heavy scenes, then tune HUD/chat temporal polish
> and effect-layer motion.

> Latest 4-click temporal-feel roadmap sync: 2026-07-07 upgrades the native
> real-input evidence from a single click to a four-click Crystal/Web movement
> route. Native Computer Use evidence
> `docs/generated/player-qa/movement-jitter/original-motion-computeruse-route-bichon-4click-20260707-2000.json`
> captured 23 frames and 4 real native clicks. Web evidence now uses the new
> `clickSequence` harness path; a polluted first sample
> `docs/generated/player-qa/movement-jitter/web-motion-clicksequence-bichon-4click-left-jpeg-20260707-2000.json`
> correctly failed after hitting `Teleport_Gilbert`, while the clean accepted
> sample
> `docs/generated/player-qa/movement-jitter/web-motion-clicksequence-bichon-leftclean-4click-jpeg-20260707-2000.json`
> passed with 29 JPEG frames, 4/4 ACKs, average ACK `204.25ms`, max `590ms`,
> and no interaction pollution. Report
> `docs/generated/player-qa/movement-jitter/temporal-native-computeruse-route-vs-web-clicksequence-bichon-leftclean-20260707.md`
> records Crystal visual delta `11.42` vs Web `10.11` (ratio `0.8853`).
> Roadmap next: video/higher-cadence native capture and exact clean-route
> replay, then tune render/camera/HUD temporal polish from measured deltas.

> Latest native temporal-feel roadmap sync: 2026-07-07 promotes the smoothness
> work from "need native real input" to repeatable same-scene Crystal/Web click
> evidence. `capture-original-computer-use.mjs` drives and captures the native
> Crystal window through Computer Use, producing real movement frames in
> `docs/generated/player-qa/movement-jitter/original-motion-computeruse-click-620-520-20260707-2000.json`.
> Web same-scene `clickTarget` evidence
> `docs/generated/player-qa/movement-jitter/web-motion-clicktarget-bichon-287-611-plus1-left-jpeg-1800ms-20260707-2000.json`
> reached `288,612` with one clean `walk DownRight`, 0 failures, and 10 JPEG
> frames. The aligned temporal report
> `docs/generated/player-qa/movement-jitter/temporal-native-computeruse-click-vs-web-clicktarget-bichon-1800ms-20260707.md`
> reports native mean visual delta `7.09` vs Web `4.51` and native changed-pixel
> ratio `0.16855` vs Web `0.108783`. Roadmap next: expand from a one-step
> click to longer Crystal/Web run routes and/or video-derived frames so the
> remaining "Crystal is smoother" claim is judged on equivalent temporal
> evidence, not screenshots.

> Latest temporal-feel roadmap sync: 2026-07-07 adds the first reusable
> frame-cadence scoring pass for the Crystal-vs-Web smoothness gap. Web held
> keyboard capture now has valid full-stage JPEG frame evidence:
> `docs/generated/player-qa/movement-jitter/web-motion-keyhold-right-jpeg-cadence-20260707-2000.json`
> is `ok=true`, records 23 frames at about 98ms average spacing, sends
> `Walk, Run, Run`, reaches `335,270`, and records no ACK/capture/assertion
> failures. The temporal report
> `docs/generated/player-qa/movement-jitter/temporal-keyhold-native-static-vs-webjpeg-cadence-20260707.md`
> compares those frames against the current native Crystal synthetic-input
> capture and reports aggregate visual delta `Crystal 0.37` vs `Web 7.09`.
> This is a measurement milestone, not visual acceptance: the native Crystal
> Win32 keyboard/click samples did not reliably move the real client, so the
> roadmap next item is native real-input/video automation before claiming
> Crystal animation-cadence parity. Follow-up SendInput scan-code keyboard,
> right-click target, and left-click target probes still produced near-static
> Crystal deltas (`0.43`, `0.33`, `0.46`), which rules out the current
> synthetic input path as accepted evidence.

> Latest held/chorded movement roadmap sync: 2026-07-07 advances the
> "Crystal feels smoother" work from click-route cleanup into longer keyboard
> runs. The red WebGL2 held Shift+Right repro was not a Bevy/Web rendering
> hitch: Gateway move logging showed the fifth run to `0:339,270` returning a
> six-packet batch with transfer/reset traffic, because `with_crystal_world_runtime`
> still retained the starter demo `starter-east-field-gate` same-map transfer.
> Full Crystal world runtime now clears starter demo map transfers and keeps
> world travel source-of-truth in generated Crystal movement records. Before
> evidence:
> `docs/generated/player-qa/movement-jitter/web-motion-heldrun-bichon-right-webgl2-movelog-20260707.json`
> showed `ok=false`, rollback, and ACK warnings `7481/4066ms`; after evidence
> `docs/generated/player-qa/movement-jitter/web-motion-heldrun-bichon-right-worldtransferfix-20260707.json`
> is `ok=true`, 8/8 movement ACKs at
> `359/152/200/247/91/57/92/146ms`, final `345,270`, no rollback, no stale
> prediction, no command queue warnings, and Bevy WebGL2 packed rendering.
> The cardinal chorded sequence
> `docs/generated/player-qa/movement-jitter/web-motion-keyseq-bichon-cardinal-worldtransferfix-rerun-20260707.json`
> is also `ok=true` with all eight ACKs under 300ms. Roadmap next: native
> Crystal held/chorded frame recording and animation-cadence scoring, because
> the server-side long-run rollback class is now covered by automated evidence.

> Latest crowded-town movement sync: 2026-07-07 closes the local Bichon
> click-route ACK/pollution gap that remained after the first temporal pass.
> The harness now produces clean same-scene mouse-route evidence with route
> patterns, entity-hit avoidance, interaction-pollution assertions, and Bevy
> WebGL2 readiness waits. The self entity no longer steals ground clicks, shared
> Zone movement consumes late-ready input immediately, and the post-ACK
> input-priority window is now Crystal run grace plus one Crystal tick (1.5s)
> so heavy world ticks do not block the next chained Walk/Run. Evidence
> `docs/generated/player-qa/movement-jitter/web-motion-clickroute-bichon-leftclean-postgrace1500-20260707.json`
> is `ok=true`, clean settle, 4/4 ACKs at `490/164/33/5ms`, no entity-hit or
> non-movement pollution, Bevy WebGL2 packed/no DOM fallback. Temporal summary:
> `docs/generated/player-qa/movement-jitter/temporal-clickroute-postgrace1500-20260707.md`.
> Repeat evidence
> `docs/generated/player-qa/movement-jitter/web-motion-clickroute-bichon-leftclean-postgrace1500-rerun-20260707.json`
> also passed with ACKs `582/78/109/7ms`.
> Roadmap next: longer held/chorded movement and animation-frame cadence versus
> Crystal, because the single crowded-town click-route repro is now green.

> Latest Crystal/Web temporal movement sync: 2026-07-07 adds the first
> repeatable native-vs-Web short-sequence movement evidence loop. Native
> Crystal Win32 capture
> `docs/generated/player-qa/movement-jitter/original-motion-frames-20260707-183007.json`
> records 16 frame images for the four-step mouse route, while Web capture now
> supports per-sample frame images, route-step timing, click-hold timing, and
> self-only movement ACK latency matching. A Web input-semantics gap is fixed:
> right-click target movement now primes run immediately instead of letting the
> first target route degrade into all-walk packets. Evidence summary
> `docs/generated/player-qa/movement-jitter/temporal-clickroute-runfix-20260707-183748.md`
> reports WoomyonWoods(S) click-route strict-green with `ok=true`, 8/8 self
> ACKs, average ACK 164.75ms, max ACK 301ms, Bevy WebGL2 drawn, and no console
> errors or non-favicon 404s. Remaining roadmap risk: Bichon crowded
> click-route evidence
> `docs/generated/player-qa/movement-jitter/web-motion-clickroute-runfix-clean-20260707-183601.json`
> still fails strict ACK responsiveness after the first run because of
> crowded-AOI / blocked-route conditions, so town mouse-route feel is the next
> movement target rather than static screenshot parity.

> Latest Crystal/Web movement evidence sync: 2026-07-07 cleans the local Bichon
> `0:286,610` Web movement sample after expanding exported Crystal UI resources
> with `NPC/09`, `Monster/011`, and `Monster/013`. The new capture
> `docs/generated/player-qa/movement-jitter/local-crystal-visual-baseline-keyseq-clean-20260707-181953.json`
> passes with `ok=true`, `strictStatus="settled"`, 4/4 movement packets ACKed,
> all 15 movement assertions green, Bevy WebGL2 gameplay layers drawn, 0
> critical console errors, and 0 non-favicon 404s. This separates real
> Crystal/Web smoothness work from the earlier polluted 367-resource-404 run.
> Roadmap next: record/compare temporal animation cadence and remove Web-only
> overlay/UI silhouette gaps rather than treating resource noise as movement
> feel.

> Latest Crystal/Web static visual parity sync: 2026-07-07 adds the repeatable
> `qa:visual-parity` report over Windows Crystal + Web same-scene screenshot
> pairs. The capture path now waits for visual scene readiness, suppresses the
> Web-only beginner tutorial during parity capture, emits Bevy map/entity
> renderer diagnostics, and filters transient headless `net::ERR_FAILED` noise
> out of critical console health. The Web-only objective tracker now defaults
> off unless explicitly enabled with `?objectiveTracker=1` or
> `localStorage["mir2:objectiveTracker"]="1"`, closing the previous automated
> P1 silhouette gap. Current local Bichon `0:286,610` evidence
> `docs/generated/player-qa/visual-parity/current-20260707-181734-report.md`
> reports weighted 95%, runtime/layout/entities 100%, pixel trend 86%, estimated
> human visual/feel parity band 88-100%, and no recurring automated top gaps.
> Roadmap next: compare temporal animation cadence, Crystal lighting/shadow
> timing, and live HUD/chat state because Crystal's "smooth" feel is a temporal
> problem, not a static screenshot problem.

> Latest Gateway movement ACK/input-priority roadmap sync: 2026-07-06 records
> the local Web/Gateway fix for the current stop/go movement repro. The issue
> was not PR #123 or a UI-only regression: a heavy shared in-process Zone world
> tick could run on the same WebSocket task immediately after a movement ACK,
> causing the browser's next chained Walk/Run to arrive after Crystal run
> grace and degrade into a one-tile step. Shared Zone now treats player
> movement ACKs as an input-priority window: `TickPlayerMovement` is drained
> before heavy ticks, pending movement blocks heavy world tick work, and a
> 1.2s post-`UserLocation` window keeps the task free for follow-up Crystal
> input while Gateway input wake remains 75ms. Evidence:
> `docs/generated/player-qa/startgame-debug-20260706-213036/current-web-jitter-r2-gateway-postackgrace1200-click.json`
> passed with `ok=true`, Run ACK about 205ms, no rollback, settled movement,
> and Bevy WebGL2 packed/no-DOM-fallback rendering. Roadmap next: keep PR
> #123's uncovered-map Bevy work out of this branch until the movement fix is
> safely isolated, then continue longer held/chorded/crowded-AOI feel sampling.

> Latest gameplay-feel + world-authority roadmap sync: 2026-06-15 records the
> June `main` landings: combat juice (floating damage numbers + hit flash, #98),
> all Crystal sound effects wired (#99), real item icons on ground drops +
> walk-to-pick-up (#97), full Crystal world activation (#80) with an on-demand
> monster pool (#83), Crystal-faithful zone-authoritative combat numerics
> (`Random(MinDC..=MaxDC)` + AC/MAC + crit), mining + dynamic doors, the full GM
> @-command set, the security remediation pass (#77), and on-chain mine M1–M4
> (#92). The active asset release `mir2/v/20260601-fullcrystal-a2f10be0` is a
> complete full-Crystal upload (0 missing), so the long-standing sprite-404
> blocker is closed. Roadmap next: per-monster AI breadth (~35 handlers → toward
> Crystal's 212 subclasses) is now the single largest gameplay-depth gap, plus
> cross-process Zone sharding and persistence normalization.

> Latest entity-atlas resource roadmap sync: 2026-06-02 closes a code-side
> gap where the Bichon starter entity atlas was listed in the critical cache
> pack but was not part of the remote R2 release roots or service-worker
> static asset class. `/bevy-entity-atlases/` is now included in the asset
> manifest, service-worker remote/cache handling, release doctor, production
> smoke, and remote release builder; the starter pack prewarms both
> `manifest.json` and `starter-bichon-base.png`. Scene readiness also uses
> entity walk/run/equipment `preloadPaths` for DOM fallback while avoiding
> duplicate scatter-fetches when the GPU atlas is already ready. Roadmap next:
> rebuild/upload the remote asset release so the current production CDN 404s
> for `/bevy-entity-atlases/*` become 200, then rerun the CDN target smoke.

> Latest player/monster state roadmap sync: 2026-05-27 closes the first
> Crystal death/vitals authority slice. Player damage can now reach 0 HP and
> emits authoritative health/death packets, snapshots expose self `dead=true`,
> dead players are blocked from movement/combat/magic/normal consumables,
> resurrection restores action, MP spend/healing syncs runtime vitals, player
> poison/control statuses now affect gameplay, and monster death regressions
> lock non-blocking/no-repeat/respawn behavior. Roadmap next: resolve the
> already-exposed broader Skill preflight/effect failures before using full
> `mir2-simulation` suite green as a candidate gate.

> Latest minimap raster roadmap sync: 2026-05-27 moves MiniMap/BigMap position
> rendering from naive world-size linear scaling to per-map Crystal MMap
> transforms. Bichon map `0` / MMap `101` is locked as a 1052x700 isometric
> projection, with shared world-to-image helpers, debug overlays via
> `?mapDebug=1`, and a regression script covering Bichon `347,285`, mini/big
> index separation, and linear fallback parity. Roadmap next: run headed
> browser/production visual acceptance before declaring this map-panel slice
> live.

> Latest Crystal movement authority roadmap sync: 2026-05-27 deploys the
> server-authority Web movement convergence to production. UCloud Gateway
> release `20260527T0020CST-crystal-movement-authority` and Web deployment
> `dpl_5rwcVtQcNBnZy5XiXvaS4axpPJSD` are live. Production headed Chrome WebGL2
> evidence
> `docs/generated/player-qa/movement-jitter/prod-crystal-movement-authority-walk-run-reverse-webgl2-skiptransfer3-20260527.json`
> passed the current walk-run-reverse repro with UI sends
> `walk Right -> run Right -> run Right -> walk Left -> run Left -> run Right`,
> no `moveTo`, send intervals `724/718/722/742/736ms`, ACK latencies
> `449/57/131/55/38/38ms`, raw WebGL2 atlas `renderedLayers=17`, final player
> `343,270 Right`, no pending plan or prediction, no visual jumps, no logical
> rollback, no stale prediction, no command queue warnings, no critical console
> errors, and no non-favicon 404s. Roadmap next: continue broader manual feel
> acceptance under long held/chorded keys, but the deployed authority model is
> now the Crystal-style single-pending server-coordinate path.

> Latest movement input-buffer roadmap sync: 2026-05-26 closes the current
> "先走、再跑、再换方向" production rollback/drift repro. The fix is split across
> the true failure boundaries: Player Web preserves discrete key-up/reverse
> edges and one queued reverse backlog, the movement harness now fails if an
> expected keyboard sequence does not actually send its `walk/run` frames,
> Gateway gives movement packets a small input-grace window over background
> runtime ticks, and shared Zone consumes ready pending movement before
> accepting follow-up intent while preserving late-consumed Run commands whose
> packet arrived during Crystal run grace. UCloud Gateway release
> `20260526T1918CST-move-input-buffer` and Web deployment
> `dpl_HttHWiP21hufr1d3mm6fMsHNwcmW` are live. Production WebGL2 evidence
> `docs/generated/player-qa/movement-jitter/prod-move-input-buffer-walk-run-turn-webgl2-20260526b.json`
> passed with ordered ACK latencies `251/51/50ms`; the faster 180ms stress
> `docs/generated/player-qa/movement-jitter/prod-move-input-buffer-walk-run-turn-fast-webgl2-20260526a.json`
> passed at `73/54/55ms`. Both settled cleanly at `332,270 Left`, with no
> rollback, no pending movement queue, raw WebGL2 atlas rendering, no critical
> console errors, and no non-favicon 404s. Roadmap next: continue broader
> Crystal movement feel sampling under held/chorded keys and the remaining
> Shared MMO authority gaps.

> Latest production movement/asset roadmap sync: 2026-05-26 closes the current
> live "走路发送指令处理有延迟" repro. The resource console storm was not a
> renderer bug: the active immutable remote asset prefix missed several
> current-scene original-map files, and the web retry loop kept cache-busting
> immutable 404s. The current missing `Objects/2652..2661` and
> `Objects23/1418/1420/1423/1425/1429` files are now present in R2, the web
> bundle negative-caches immutable asset failures, and Gateway release
> `20260526T1435CST-move-tick-grace0` removes the old 1200ms movement input
> tick defer. Production headed Chrome WebGL2 evidence
> `docs/generated/player-qa/movement-jitter/prod-move-tick-grace0-webgl2-existing-20260526.json`
> passed with `ok=true`, raw WebGL2 atlas `renderedLayers=21`, two Walk ACKs at
> `398ms` and `609ms`, no critical console errors, and no non-favicon 404s.
> Roadmap next: investigate the remaining isolated `Objects/289.png` source or
> map-library mapping gap, then continue Shared MMO ZoneOwner/service authority
> gaps.

> Latest raw WebGL2 atlas gameplay roadmap sync: 2026-05-26 moves the
> browser-native WebGL2 entity-atlas path from synthetic probe to production
> headed gameplay evidence. Player Web now builds the atlas for the raw WebGL2
> path under the shared GPU renderer condition, keeps initial scene interaction
> gated until the raw atlas is ready, and targets hosted custom-domain sessions
> at `wss://165.154.65.136.sslip.io/ws` instead of the high-jitter
> custom-domain `/ws` route. Production deployment
> `dpl_Q1k4QFSbGigw9gJ64cfBNcAehjEQ` is live behind
> `https://mir2.obelisk.build`; bundle probing found the direct WSS host in the
> shipped JS. Headed Chrome forced-WebGL2 evidence
> `docs/generated/player-qa/movement-jitter/prod-webgl2-raw-atlas-gameplay-focused-direct-default3-20260526.json`
> passed with selected/compiled backend `webgl2`, hidden Bevy canvas, raw
> WebGL2 enabled, prebuilt `starter-bichon-base` atlas packed,
> `textureReady=true`, `renderedLayers=21`, three Walk ACKs at `93/51/46ms`,
> clean settle, no camera-offset stair-step warnings after foregrounded headed
> sampling, no critical console errors, and no non-favicon 404s. Roadmap next:
> continue Shared MMO ZoneOwner/service authority gaps.

> Latest WebGPU/WebGL2 runtime roadmap sync: 2026-05-26 adds a repeatable
> Chrome smoke for Bevy runtime backend selection. Player Web now has
> `smoke:bevy-runtime-backends`, covering default WebGPU-first selection,
> forced WebGPU, forced WebGL2 package loading, runtime JS/WASM fetches, and
> post-boot critical console errors. Local evidence selected/compiled WebGPU
> for default and forced WebGPU, selected/compiled WebGL2 for forced WebGL2,
> and passed with zero critical console errors. The newer raw WebGL2 atlas
> renderer probe above starts closing the true WebGL2-renderer fallback target.

> Latest ZoneOwner handoff roadmap sync: 2026-05-26 turns owner takeover from
> fencing-only into a tested runtime-state move. A hosted owner can hand off
> its owned `ZoneRuntimeHandle` once, the old host becomes unavailable, and a
> replacement owner resumes active session state under the new fencing token.
> Roadmap next: replace the in-process handle transfer with a durable Zone
> state snapshot/log, wire it through real process/network RPC, and make
> Account/Inventory plus NPC world-service commits participate in owner
> takeover and rollback semantics.

> Latest ZoneOwner RPC transport roadmap sync: 2026-05-26 turns the owner
> command-client boundary into a transport-facing seam. `RpcZoneOwnerCommandClient`
> now delegates command execution and owner views through `ZoneOwnerRpcTransport`,
> and the hosted owner implements that transport as the current loopback host.
> Roadmap next: swap the loopback transport for a real process/network
> ZoneOwner service, persist/migrate Zone state during handoff, and bind
> Account/Inventory plus NPC world-service commits to that owner fencing.

> Latest SkillItemConsume request-id roadmap sync: 2026-05-26 closes the next
> Account/Inventory service boundary prerequisite. Item-consuming Zone skill
> casts now carry a stable per-session `request_id` in
> `SkillItemConsume`, and the idempotency key includes account, character,
> spell, and request id. Roadmap next: persist those committed receipts in an
> external Account/Inventory actor, pass them through real ZoneOwner RPC, and
> connect failure/rollback semantics to owner fencing.

> Latest ZoneOwner hosted-runtime roadmap sync: 2026-05-26 moves owner command
> execution from "Gateway passes a request through a replaceable client" to
> "the owner host can own the runtime being mutated." The new hosted owner
> command client executes fenced requests inside its own `ZoneRuntimeHandle`
> and validates stale leases at the owner boundary after handoff.
> `GatewaySession` now routes snapshot, identity, save, and mail-refresh reads
> through that owner-client surface as well. The newer RPC transport seam above
> lifts this from a direct hosted client into a replaceable transport surface.
> Roadmap next: add durable state handoff/takeover for ZoneOwner migration and
> replace the loopback transport with real process/network RPC.

> Latest Account/Inventory idempotency roadmap sync: 2026-05-26 moves shared
> reward commits closer to the durable economy actor target. Zone monster kill
> awards and ground-drop pickups now have deterministic committed-receipt keys
> in the default Account/Inventory service, preventing duplicate reward or
> pickup mutation when a shared Zone command is retried. The newer
> SkillItemConsume request-id sync above adds the missing cast-command identity.
> Roadmap next: move the command store out of process and bind commits to
> ZoneOwner fencing/RPC handoff.

> Latest NPC world-service atomic outcome roadmap sync: 2026-05-26 moves the
> NPC/quest side-effect track from separate bridge commits toward a single
> authoritative world-service transaction. Shared NPC execution now packages
> script saved values, shared random seed, and entity mutation packets into
> `ApplyScriptOutcome`; Gateway merges or forwards those results only after a
> committed world-service receipt validates the side-effect payload. Roadmap
> next: replace the in-process world service with a durable process/RPC actor,
> expand quest/economy/account side effects into the same command surface, and
> continue ZoneOwner handoff plus long gameplay acceptance.

> Latest Zone-native CharmedSnake roadmap sync: 2026-05-26 closes the
> remaining Crystal `CharmedSnake` successful-hit status behavior in the
> current Archer summon family. Shared Zone now applies the minion's
> post-damage paralysis poison with deterministic Zone authority and the
> Crystal PetLevel-based chance/duration shape. Roadmap next: continue through
> broader monster AI/status families, then expand durable skill/Buff service
> ownership and process-external NPC/economy/account authority toward true
> distributed ZoneOwner operation.

> Latest Zone self-Buff roadmap sync: 2026-05-26 advances the durable
> skill-state track from "open risk" to a verified bridge. Gateway now applies
> Zone-owned self `AddBuff` / `RemoveBuff` packet outcomes into the owning
> personal runtime `BuffResource`, including pending Zone packets, so accepted
> shared-Zone Buffs such as MagicShield are reflected in snapshots instead of
> only in transient client packets. Roadmap next: convert more Buff families
> from packet mirroring into Zone-owned lifetime/service state, then connect
> that state to process-external services alongside NPC/economy/account
> authority and ZoneOwner handoff.

> Latest Zone-native SnakeTotem roadmap sync: 2026-05-26 completes the
> remaining Archer summon-family follow-through for this shared-Zone pass.
> `SnakeTotem` now keeps the Crystal `PetLevel + 1` swarm cap, refreshes
> expired `CharmedSnake` minions, self-destructs on missing/far master, and
> kills its owned minions when it dies. `CharmedSnake` now dies on lifetime
> expiry or missing/far Totem and applies the Crystal 3x3 death explosion via
> Zone-native monster-hit resolution. Roadmap next: durable skill-state,
> process-external NPC/economy/account services, full monster AI coverage,
> ZoneOwner handoff, and long production gameplay acceptance.

> Latest Zone-native VampireSpider roadmap sync: 2026-05-26 moves the
> `SummonVampire` tail behavior from "remaining summon polish" into verified
> shared-Zone authority. `VampireSpider` attacks now emit Crystal Bleeding
> `ObjectEffect` effect 18 and heal the Archer master through Zone-owned
> health state. Zone expiry now models Crystal self-destruct for lifetime
> expiry or a missing/far master, emits `ObjectDied`, and applies 3x3 explosion
> damage to nearby hostile Zone monsters through the native hit resolver while
> preserving owner heal/effect behavior and avoiding player damage. Roadmap
> next: SnakeTotem swarm cap/expiry hardening, durable skill-state persistence,
> process-external services, full NPC/economy authority, and ZoneOwner
> handoff.

> Latest Zone-native Archer summon roadmap sync: 2026-05-26 advances the
> remaining summon authority track beyond Taoist/HolyDeva/PetEnhancer into the
> first Archer family slice. Shared Zone now recognizes `SummonVampire`,
> `SummonToad`, `SummonSnakes`, and `Stonetrap` as native summon spells with
> Crystal-style target-point/projectile-delay validation, retained friendly
> `ObjectMonster` state, `extra` visibility, master binding, summon caps, and
> lifetime expiry; Gateway routes those spells to Zone without applying the
> Taoist amulet item boundary. Verified behavior covers `VampireSpider`
> target-point spawn plus recast recall, stationary `SpittingToad` ranged
> attack, retained static `SnakeTotem` spawn plus owned `CharmedSnake` minion
> attack, and static expiring `StoneTrap` decoy aggro that pulls hostile
> monsters off the player. Roadmap next: finish SnakeTotem swarm cap/expiry
> hardening, VampireSpider self-destruct/vampire-heal details, durable
> skill-state persistence, and process-external service boundaries.

> Latest Zone-native summon/PetEnhancer roadmap sync: 2026-05-25 advances the
> shared-Zone summon ownership track from first spawn/recall to melee and
> ranged summon-vs-monster combat plus pet Buff stats. `SummonSkeleton` /
> `SummonShinsu` / `SummonHolyDeva` use the targetless Zone magic route and
> Account/Inventory skill-item command boundary; the verified `SummonSkeleton`
> path schedules a Crystal-delay
> friendly `BoneFamiliar`, binds `master_object_id` to the Zone player, retains
> `extra=true` for late AOI joins, recalls the existing owned summon on recast
> without a second item commit or duplicate spawn, and now lets the
> `BoneFamiliar` attack hostile native monsters without targeting players.
> `SummonShinsu` now shares the one-amulet Zone item boundary, delayed retained
> `Shinsu` spawn, master binding, and hostile-monster melee behavior.
> `SummonHolyDeva` now covers the 1.5s delayed retained `HolyDeva` spawn and
> six-tile ranged `ObjectRangeAttack` with delayed DC damage against hostile
> monsters. `PetEnhancer` now applies and retains the visible Crystal pet Buff
> type 22 on owned Zone summons and feeds its DC stats back into summon damage.
> Roadmap next: HolyDeva kiting polish, archer summon families, and durable
> skill-state persistence.

> Latest Zone-native area-healing roadmap sync: 2026-05-25 expands the
> self/friendly recovery track from single-target Healing to MassHealing and
> HealingCircle. Zone now validates near-target casts, selects wounded Zone
> players inside the recovery radius, owns each delayed HP restore, emits
> `PlayerHealed` for Gateway personal-runtime synchronization, and emits
> HealingCircle's delayed `ObjectSpell` from Zone state. Roadmap next: party /
> group membership filtering, summon ownership, and durable skill-state
> persistence.

> Latest Zone-native Healing roadmap sync: 2026-05-25 adds the first
> self/friendly recovery spell to Zone-native authority. Healing now accepts
> self-target casts, validates HP/MP/cooldown/action state in Zone, publishes
> owner/observer magic plus the Crystal healing effect, schedules the delayed
> Zone-owned HP restore, and returns `PlayerHealed` for Gateway to synchronize
> the personal runtime. Roadmap next: expand this pattern to MassHealing,
> HealingCircle/friendly target validation, group recovery, and summons while
> durable skill-state persistence is built out.

> Latest Zone-native MagicShield roadmap sync: 2026-05-25 advances
> skill/Buff authority from observer-synchronized Buff packets into native
> self-target Zone magic. `MagicShield` casts with `target_id=0` now resolve in
> `ZoneRuntime`: Zone validates the self target, action window, MP/cooldown,
> and duplicate Buff state; emits `Magic`, `ObjectMagic`, visible `AddBuff`,
> and the Crystal shield-up effect; stores the Buff for late AOI joins; and
> applies damage-reduction-percent stats when native monsters hit the player.
> Gateway preparation recognizes self-target Zone magic so learned MagicShield
> can use this path. Roadmap next: extend the same Zone-owned Buff/stat pattern
> to more self/friendly Buffs, healing, summons, and durable skill-state
> persistence.

> Latest production movement/input roadmap sync: 2026-05-25 closes the live
> movement rollback and input-stall investigation. Gateway release
> `20260525T0334CST-starter-transfer-cleanup` is installed at
> `/opt/mir2/gateway/releases/20260525T0334CST-starter-transfer-cleanup` on
> the UCloud host and health/WSS smoke passed. Production headed WebGPU
> packet-walk crossed the old `339..341 -> 330,270` demo-gate area with ACKs
> `339..343`, no map-change packet, and no rollback. Player Web deployment
> `dpl_7iG3bPgA7HTxkvEzN4LxP2rmFmFC` then shipped the scene-input unlock:
> after the first playable scene becomes ready, later viewport asset preloads
> no longer block keyboard/pointer movement. Headed Chrome evidence
> `docs/generated/player-qa/movement-jitter/prod-scene-input-unlocked2-webgpu-headed-keyboard-a-nosample-hold-20260525.json`
> passed with WebGPU selected, packed prebuilt atlas active,
> `sceneInteractionReady=true` during background asset loading, five held-Walk
> commands, five authoritative ACKs, no critical console errors, and no
> non-favicon 404s. Roadmap next: continue the remaining 100% Candidate shared
> MMO gaps, especially durable ZoneOwner RPC/handoff, durable
> Account/Inventory/NPC services, full Buff/skill state, full monster AI, and
> 30-active long gameplay acceptance.

> Latest Crystal runtime starter-transfer roadmap sync: 2026-05-25 removes
> the starter-demo same-map gate from production Crystal runtime config.
> `with_crystal_map_runtime()` now clears the hard-coded
> `starter-east-field-gate`, leaving normal starter demo behavior intact while
> production uses generated Crystal movement records only. Roadmap next:
> continue the shared MMO 100% Candidate gaps after the deployed production
> movement/input closeout above.

> Latest PoisonCloud live item-route roadmap sync: 2026-05-25 turns the
> PoisonCloud Account/Inventory boundary into a live Gateway route. Gateway now
> treats PoisonCloud as a targetless ground spell, prechecks Zone acceptance
> before consuming the required amulet and green poison, then dispatches the
> Zone cast only after the item-cost commit succeeds. Roadmap next: make this
> account/inventory command surface durable and RPC-fenced for multi-process
> ZoneOwner deployment.

> Latest Zone-native ExplosiveTrap roadmap sync: 2026-05-25 closes another
> Trap-family shared-Zone authority gap. Native ExplosiveTrap now derives its
> front-row trap cells from caster direction, emits the delayed Crystal-style
> `ObjectSpell` trap row, applies contact damage from Zone-owned state, and
> clears after detonation. Roadmap next: continue broader profession bespoke
> controls, summons, and durable skill-state persistence.

> Latest Zone-native TrapHexagon roadmap sync: 2026-05-25 closes the next
> Trap-family shared-Zone slice. Native TrapHexagon now applies area root
> control to hostile Zone monsters around the target and schedules the delayed
> eight-point ring of `ObjectSpell` packets from Zone-owned ground-spell state.
> Roadmap next: continue broader profession bespoke controls, summons, and
> durable skill-state persistence.

> Latest Skill item-consumption roadmap sync: 2026-05-25 starts moving
> item-consuming spell costs behind the Account/Inventory command boundary.
> `SkillItemConsume` now exists as an identity-bearing shared command, and the
> in-process service can transact PoisonCloud amulet + green-poison consumption
> into receipt packets. Roadmap next: replace the in-process implementation
> with a durable actor/transaction service.

> Latest targetless ground-magic roadmap sync: 2026-05-25 closes the first
> object-target assumption in shared-Zone magic routing. Zone now accepts
> `PlayerCastMagic` with `target_id=0` for ground-target spells such as
> FireWall/Blizzard/MeteorStrike/PoisonCloud, and Gateway preparation can route
> learned targetless ground Magic to Zone without fabricating a monster target;
> PoisonCloud now uses the Account/Inventory item-cost route after Zone
> precheck. Roadmap next: broaden live Gateway regressions and move the command
> service out of process.

> Latest Zone-native Trap roadmap sync: 2026-05-25 adds the first
> Trap-family control spell to the shared-Zone authority track. Zone native
> monsters now retain level from Crystal spawn data, and native Trap uses that
> level to enforce Crystal's lower-level gate before rooting the hostile monster
> and queuing the delayed Trap `ObjectSpell` with direction/param semantics.
> Roadmap next: extend this to broader root/control skills and durable
> skill-state persistence.

> Latest Zone-native PoisonCloud roadmap sync: 2026-05-25 extends persistent
> ground-spell authority to Taoist poison cloud monster effects. The Zone now
> owns PoisonCloud's delayed visible cloud object, 3x3 occupied-cell damage,
> and green monster poison projection. Gateway now also routes the required
> amulet and poison item consumption through the Account/Inventory command
> boundary after Zone precheck. Roadmap next: make that command boundary
> durable and process-external.

> Latest Zone-native chain/splash roadmap sync: 2026-05-25 expands routed
> magic from direct/area/ground spells into chain and splash branches. Native
> MeteorShower now owns secondary-target selection and half-damage commits in
> Zone, and native FireBounce now owns chained projectile hops plus delayed
> damage between Zone monsters. Roadmap next: close PoisonCloud item commits,
> remaining Trap-family actions, profession bespoke skills, summons, and
> durable skill-state persistence before retiring the
> personal-session skill bridge.

> Latest Zone-native ground-spell roadmap sync: 2026-05-25 advances persistent
> spell ownership from routed casts into shared-Zone state. Native FireWall now
> schedules the delayed cross-shaped `ObjectSpell` cells and recurring
> same-cell damage from Zone state, while Blizzard/MeteorStrike schedule their
> delayed 5x5 ground spell cells, center marker, and later recurring damage
> tick inside Zone. Roadmap next: promote Trap-style ground actions and the
> remaining profession-specific spell formulas into the same authority
> boundary.

> Latest Zone-native area magic roadmap sync: 2026-05-25 starts replacing
> personal-session multi-target spell effects with Zone-owned target collection.
> Native FireBang/IceStorm casts now include secondary target ids and commit
> damage to nearby Zone monsters. Roadmap next: broaden the same mechanism to
> persistent ground spells, chain projectiles, splash-specific formulas, and
> late profession spell variants.

> Latest Zone-native special arrow Buff roadmap sync: 2026-05-25 expands
> native skill authority from PoisonShot poison ticking into player-held Buff
> state. Zone now owns the visible PoisonShot arrow marker Buff, exposes it to
> late AOI joins, and lets CrippleShot consume that Buff to spread green poison
> to nearby native monsters. VampireShot healing now also resolves in Zone and
> synchronizes back to Gateway/personal runtime through `PlayerHealed`;
> CrippleShot can consume that VampireShot Buff for the same Zone-owned heal
> follow-up. Roadmap next: finish full Buff stat families, summons, and
> AoE/ground spells in Zone before removing the remaining personal-session
> skill side-effect bridge.

> Latest Gateway Magic route roadmap sync: 2026-05-25 closes the practical
> command-routing proof for shared-Zone Magic launches. Gateway focused tests
> now show both RangeAttack and Magic commands leave the personal session path,
> execute through shared Zone authority, and reach observers through Zone
> outbounds. Roadmap next: expand from routed seeded magic to complete
> Zone-owned Crystal spell effects, Buff state, projectile/damage variants, and
> durable skill-state persistence.

> Latest Zone-native poison tick roadmap sync: 2026-05-25 moves the first
> player-applied poison damage loop from personal-session behavior into shared
> Zone. Native `PoisonShot` now attaches green poison state to
> `ZoneNativeMonster`, broadcasts the poison bit, ticks 2-second poison damage
> in Zone, and can complete death/drop/award through the Zone-native reward
> path. Roadmap next: expand this pattern to CrippleShot, PoisonCloud, broader
> poison variants, monster-applied poison damage, and Boss/status AI while the
> remaining Account/Inventory, NPC world-service, and ZoneOwner process
> adapters are replaced with durable authorities.

> Latest ZoneOwner heartbeat roadmap sync: 2026-05-25 turns the optional TTL
> lease slice into a scheduled Gateway-session heartbeat. Web sessions now
> configure a ZoneOwner renewal interval, and the runtime tick renews the owner
> lease before any deferred world tick work so high-frequency movement input
> does not starve owner liveness. Roadmap next: swap the in-process
> `ZoneOwnerCommandClient` for real Gateway -> ZoneOwner RPC, keep fencing at
> the owner boundary, and implement handoff/takeover around the same lease
> heartbeat contract.

> Latest Zone-native player action-window roadmap sync: 2026-05-25 closes the
> shared-Zone combat timing gap left after moving melee/range/magic authority
> into Zone. Zone-owned players now track Crystal-style attack and spell action
> readiness; native melee/range reject early launches, and native magic rejects
> early casts across different spells before MP/cooldown/impact commits. Gateway
> shared-runtime coverage now proves the practical RangeAttack route also stops
> early relaunch at the Zone boundary. Roadmap next: expand native skill/Buff
> side effects and monster status/poison/Boss AI,
> then replace the remaining in-process Account/Inventory, NPC world-service,
> and ZoneOwner command adapters with durable process boundaries.

> Latest NPC world-service command-envelope roadmap sync: 2026-05-25 turns the
> existing shared NPC saved-value/random-seed/entity-side-effect bridge into a
> replaceable command surface. Gateway now sends
> `SharedNpcWorldCommandEnvelope` values with active account/character identity
> to `SharedNpcWorldService`, and only applies the returned committed command to
> shared Zone NPC/map state. Roadmap next: promote `MONGEN`, `MONCLEAR`, event
> flags, NPC service trades, quest rewards, and rollback-sensitive economy
> changes from diff-derived packets onto first-class NPC/world-service commands.

> Latest Account/Inventory command-envelope roadmap sync: 2026-05-25 makes
> reward/economy commits actor-shaped. Gateway now submits a
> `SharedAccountInventoryCommandEnvelope` carrying active account/character
> identity plus the shared monster kill award or ground-drop pickup command,
> while the default implementation adapts that envelope back to the current
> session-backed commit functions after rejecting identity mismatches. Roadmap
> next: replace the adapter with a durable Account/Inventory actor or
> transactional store, then expand the same command surface to NPC trades, quest
> rewards, and rollback-sensitive economy side effects.

> Latest ZoneOwner command-client roadmap sync: 2026-05-25 adds a concrete
> command client boundary after the existing owner-lease validation step.
> Gateway sessions now pass `ZoneOwnerCommandRequest` envelopes through
> `ZoneOwnerCommandClient`, with the default in-process client preserving
> current runtime behavior and tests proving stale requests stop before that
> boundary. The same slice adds a renewal hook that accepts the current lease
> and rejects old owners after handoff; the in-process owner client also
> validates fencing against the same authority at the owner boundary. The
> in-memory authority now supports optional TTL renewal semantics, including
> expired-renewal rejection and fencing-token advancement for takeover. Roadmap
> next: turn this client into real Gateway -> ZoneOwner RPC with scheduled
> heartbeat renewal, fencing-token enforcement at the owner process, handoff,
> and takeover recovery.

> Latest Zone-native monster status roadmap sync: 2026-05-25 closes the
> first special AI player-status gap in the shared Zone path. Zone-native
> delayed monster hits now evaluate Crystal-style paralysis for AI 7/22 and
> green poison for AI 28/37, commit the Zone player's poison bitfield,
> broadcast `ObjectPoisoned`, prevent movement while Zone-owned paralysis is
> active, and clear the status after its Crystal-duration window. Roadmap
> next: add poison damage ticks, expand the Boss/area/status AI matrix, and
> keep moving remaining combat/drop/NPC/economy side effects behind Zone or
> world-service authority.

> Latest Account/Inventory service-boundary roadmap sync: 2026-05-25 turns
> the previous reward receipt contract into a replaceable Gateway service.
> Zone monster kill awards and shared ground-drop claim commits now go
> through `SharedAccountInventoryService`; the default implementation keeps
> current session-backed behavior, and injected services can act like a
> future Account/Inventory actor. Roadmap next: implement the real actor or
> transaction service behind that interface, then route NPC service trades,
> quest rewards, gold/items/experience persistence, and rollback through the
> same authority boundary.

> Latest NPC entity side-effect roadmap sync: 2026-05-25 makes NPC script
> monster world mutations visible to the shared Zone boundary. Gateway now
> diffs monster entities around NPC command execution and converts new
> monsters into Crystal-backed `ObjectMonster` packets, while cleared/dead
> monsters produce `ObjectHealth(0)` plus `ObjectDied` and removed monsters
> produce `ObjectRemove`. Shared observer routing now accepts those
> health/death/remove packets as shared-object updates. Roadmap next: replace
> this bridge diff with first-class Zone/world-service commands for NPC
> map/event mutations, service trades, and rollback-sensitive quest/economy
> commits.

> Latest NPC random shared-state roadmap sync: 2026-05-25 closes another
> NPC bridge divergence by moving Crystal NPC `RANDOM` seed progression onto
> the shared Zone state path. `SimulationSession` now exposes read/apply hooks
> for the NPC random seed, and Gateway applies the shared seed before NPC
> commands then publishes the post-command seed back to the shared state. This
> does not make quests global; per-character quest progress remains personal.
> Roadmap next: turn NPC `MONGEN` / `MONCLEAR`, map event flags, service
> trades, and rollback-sensitive quest/economy side effects into explicit
> Zone/world-service submissions.

> Latest Zone-owner command fencing roadmap sync: 2026-05-25 turns the
> previous owner metadata into an executable guardrail. `GatewaySession`
> wraps commands in `ZoneOwnerCommandRequest` envelopes that require a
> matching `ZoneOwnerLease` before calling the underlying runtime, and the
> production Web action path now uses that guard for normal authenticated
> player commands. The new shared `ZoneOwnerLeaseAuthority` also lets a zone
> handoff advance the current owner fencing token, so a pre-handoff session
> can no longer execute commands with its saved lease. Focused tests cover
> successful execution with the current in-process owner, rejection of a stale
> fencing token before runtime mutation or gameplay-event publication,
> rejection of a mismatched owner id before production command execution, and
> rejection after shared-authority handoff. Roadmap next: move the command
> receiver behind a real Gateway -> ZoneOwner RPC boundary, then add TTL
> renewal, takeover recovery, and stale-owner rejection across processes.

> Latest Zone-owner fencing metadata roadmap sync: 2026-05-25 adds the first
> concrete owner/fencing contract to the Gateway routing layer. Routed
> runtimes, live Gateway sessions, and session-cache routes now carry
> `ZoneOwnerLease` metadata (`zoneOwnerId`, `fencingToken`) in addition to
> `zoneId`, with the current in-process owner represented as
> `in-process:<zoneId>` token `1`. Verification passed focused registry/cache
> owner metadata regressions, admin session record coverage, fmt/diff checks,
> and locked Simulation/Gateway check. Roadmap next: use these fields to build
> the real Gateway -> ZoneOwner command path with fencing-token validation,
> owner renewal, migration/handoff, and stale-command rejection.

> Latest NPC saved-value shared-state roadmap sync: 2026-05-25 starts moving
> NPC/quest side effects from personal-session bridge behavior toward a shared
> world-service boundary. Crystal NPC `SAVEVALUE` / `LOADVALUE` data now uses
> `SharedNpcSavedValue`, with Gateway shared Zone state applying values before
> NPC commands and publishing updated values after them. Verification passed
> the new cross-session saved-value regression, existing shared sparse-NPC and
> guide-quest regressions, the Account/Inventory receipt regression, fmt/diff
> checks, and locked Simulation/Gateway check. Roadmap next: expand this from
> saved script values into quest progress/acceptance, NPC service/economy
> commits, map/event flags, and rollback-capable world-service submissions.

> Latest Account/Inventory transaction-boundary roadmap sync: 2026-05-25
> turns the shared reward bridge into one explicit Account/Inventory receipt
> contract. Shared ground-drop pickup and Zone-native monster kill awards both
> now return `SharedAccountInventoryTransactionReceipt { kind, committed,
> packets }`, and Gateway uses that receipt for reward packets plus Zone drop
> claim commit/cancel decisions. Verification passed the new Gateway receipt
> regression, adjacent shared kill-award and rollback tests, the Simulation
> ground-drop receipt test, fmt/diff checks, and locked Simulation/Gateway
> check. Roadmap next: replace the personal-session storage behind this
> receipt with a real Account/Inventory actor or transaction service, then
> route NPC/quest world side effects, special monster AI rewards, and
> distributed Zone-owner handoff through the same authority boundary.

> Latest 30-active movement/chat roadmap sync: 2026-05-25 moves production
> gameplay feel from "30 sockets are reachable but active cap stays 15" to
> accepted `60 ws / 30 active / 30 reconnect leases` on the current single
> UCloud Gateway. Release `20260525T1348CST-route-refresh-background-task`
> fixes the movement-delay root cause by decoupling route-lease refresh from
> the socket hot loop, caching same-map transfer tiles, reducing Zone lock
> churn, coalescing observer movement packets, and lazily generating retained
> AOI visibility packets. Public proof passed 30-active movement-only and
> move/chat pressure:
> `docs/generated/load/public-route-refresh-background-task-30active-movementonly1m-settle30s-20260525.json`,
> `docs/generated/load/public-route-refresh-background-task-30active-movechat1m-chat30-settle30s-20260525.json`,
> and
> `docs/generated/load/public-route-refresh-background-task-30active-movechat1m-chat10-settle30s-20260525.json`.
> Roadmap next: keep 30-active as the current single-Gateway target, then
> finish the still-open Shared MMO pieces: transactional Account/Inventory
> rewards, NPC/quest side effects, special monster AI, and cross-Gateway Zone
> owner fencing/handoff.

> Latest shared ground-drop commit receipt roadmap sync: 2026-05-25 turns the
> shared pickup bridge into a clearer transaction boundary. Shared
> ground-drop pickup now returns an explicit `committed` receipt from the
> character/economy side; Gateway uses that receipt to commit or cancel the
> Zone claim instead of deriving success from `GainedGold` / `GainedItem`
> packets. Verification passed focused Simulation/Gateway regressions, normal
> shared pickup coverage, local and UCloud locked checks, and production
> release `20260525T0843CST-grounddrop-commit-receipt` with WSS smoke plus
> 30-client safe-cap evidence
> `docs/generated/load/remote-grounddrop-commit-receipt-30active-timeout60-20260525.json`.
> Roadmap next: move this receipt contract behind a real Account/Inventory
> transaction service for gold/items/quest side effects, then continue
> Zone-owner fencing/handoff and accepted 30-active gameplay feel.

> Latest shared kill-award commit roadmap sync: 2026-05-25 tightens the
> reward commit boundary after Zone-native monster death. Zone now owns the
> kill/drop decision and emits `MonsterKillAward`, while the Gateway-side
> character commit applies experience and emits `GainExperience` only after
> state mutation. Verification passed focused Simulation/Gateway regressions,
> shared routing/fallback drop tests, local and UCloud locked checks, and
> production release `20260525T0827CST-zone-award-commit` with WSS smoke plus
> 30-client safe-cap evidence
> `docs/generated/load/remote-zone-award-commit-30active-timeout60-20260525.json`.
> Roadmap next: generalize this from experience to a real transactional
> reward service covering gold, inventory items, quest side effects, and
> rollback, then continue Zone-owner fencing/handoff and accepted 30-active
> gameplay feel.

> Latest shared fallback drop-template roadmap sync: 2026-05-25 advances the
> drop/economy track by removing a personal-session dependency from sparse
> shared monster combat. Fallback `ZoneMonsterSpawn` creation from shared
> entity snapshots now restores Crystal/starter drop templates, so a
> Zone-native kill reached through that path can still spawn ground drops with
> the normal owner window. Verification passed focused Gateway helper tests,
> Simulation native kill/drop coverage, shared routing/rollback regressions,
> local and UCloud locked checks, and production release
> `20260525T0804CST-zone-fallback-drops` with WSS smoke plus 30-client
> safe-cap evidence
> `docs/generated/load/remote-zone-fallback-drops-30active-timeout60-20260525.json`.
> Roadmap next: replace the remaining personal-session economy bridge with
> Zone/world-service drop generation and transactional Account/Inventory
> reward commit, then continue NPC/quest side effects, Zone owner
> fencing/handoff, and accepted 30-active gameplay feel.

> Latest shared drop/economy rollback roadmap sync: 2026-05-25 converts one
> risky bridge assumption into covered behavior. Gateway now has a focused
> regression proving that if a shared Zone ground-drop claim succeeds but the
> personal economy commit rejects the pickup, the claim is canceled, the
> shared/Zone drop is restored, and the client sees the ground gold remain
> instead of a false `ObjectRemove`. Verification passed the new rollback
> test, adjacent shared-drop pickup tests, locked Gateway check, and Gateway
> fmt check. Roadmap next: keep this rollback guard while moving drop
> generation and inventory/gold mutation out of personal sessions into a
> Zone/world-service transaction, then continue Zone owner fencing/handoff and
> accepted 30-active gameplay feel.

> Latest Zone-native ranged monster AI roadmap sync: 2026-05-25 advances the
> monster-AI authority track beyond melee. Native Zone monsters now retain
> Crystal `ai`; ranged/magic-style AI no longer has to chase until adjacent
> before attacking, and instead launches `ObjectRangeAttack` plus a delayed
> Zone-owned player hit from its current non-adjacent tile. Verification passed
> the focused ranged-monster regression, adjacent melee/Buff/Gateway routing
> tests, local and UCloud locked checks, and production release
> `20260525T0734CST-zone-monster-ranged` with WSS smoke plus 30-client
> safe-cap evidence
> `docs/generated/load/remote-zone-monster-ranged-30active-timeout60-20260525.json`.
> Roadmap next: expand this first ranged branch into special ranged/magic/Boss
> AI, then continue through AoE/ground spells, summon lifecycle, NPC/quest
> side effects, economy transactions, Zone owner fencing/handoff, and accepted
> 30-active gameplay feel.

> Latest Zone-owned defensive Buff roadmap sync: 2026-05-25 closes the first
> incoming-damage Buff stat slice. `ZoneRuntime` now applies retained player
> `MAX_AC` Buff stats while resolving delayed native monster hits, so Zone
> owns both outgoing attack-stat and incoming defence-stat participation in the
> current native combat MVP. Verification passed focused and adjacent
> Simulation regressions, Gateway shared routing coverage, local/UCloud locked
> checks, and production release `20260525T0720CST-zone-buff-defence` with
> WSS smoke and 30-client safe-cap baseline
> `docs/generated/load/remote-zone-buff-defence-30active-timeout60-20260525.json`.
> Roadmap next: expand from simple DC/AC stat effects into rate/status Buffs,
> AoE/ground spell authority, summon lifecycle, monster ranged/magic/Boss AI,
> NPC/quest side effects, economy transactions, and real Zone-owner
> fencing/handoff.

> Latest Zone-owned Buff stat roadmap sync: 2026-05-25 moves the first stat
> Buff effect out of personal-session combat math and into shared Zone commit.
> Zone-native attack profiles now start from unbuffed base damage, then
> `ZoneRuntime` applies retained player Buff stat payloads when resolving
> native monster hits; once the Zone-held Buff expires, the same attack returns
> to base damage. Verification passed the focused Buff-stat authority test,
> existing Zone object-Magic tests, Gateway shared routing coverage, local and
> UCloud locked Simulation/Gateway checks, and production release
> `20260525T0709CST-zone-buff-stats` with WSS smoke plus the 30-client
> safe-cap baseline
> `docs/generated/load/remote-zone-buff-stats-30active-timeout60-20260525.json`.
> Roadmap next: extend Zone-owned Buff semantics beyond max-DC damage into
> defense/rate/status stats, then tackle AoE/ground spells, summon lifecycle,
> monster ranged/magic/Boss AI, NPC/quest side effects, economy transactions,
> and Zone-owner fencing/handoff.

> Latest Zone-native Magic control roadmap sync: 2026-05-25 closes the first
> shared-Zone skill-control gap after object Magic MP/cooldown. Targeted
> ElectricShock, Entrapment, and CatTongue now apply control state inside
> `ZoneRuntime`; controlled native monsters stop moving/attacking until the
> Zone-owned expiry, Entrapment/CatTongue publish retained Crystal control
> packets, poison clears on expiry, and ElectricShock/Entrapment are treated as
> zero-damage control profiles. Verification passed local and UCloud focused
> Simulation tests, Gateway shared routing coverage, and locked
> Simulation/Gateway check. Gateway release
> `20260525T0651CST-zone-magic-control` is live; WSS smoke
> `docs/generated/load/remote-zone-magic-control-wss-smoke-20260525.json`
> passed, and the updated 30-client baseline
> `docs/generated/load/remote-zone-magic-control-30active-timeout60-20260525.json`
> confirms the production policy remains `15 active / 15 rejected` with no
> client errors. Roadmap next: move stat Buff application and AoE/ground spell
> resolution into Zone-owned skill state, then continue into richer monster
> ranged/magic/Boss AI, NPC/quest side effects, transactional economy commit,
> and real cross-Gateway Zone-owner fencing/handoff.

> Latest Zone-native ranged combat roadmap sync: 2026-05-25 extends the shared
> combat migration beyond melee. `ZoneCommand::PlayerRangeAttackObject` and
> `ZoneCommand::PlayerCastMagic` now give the Zone authority over live monster
> target validation, target-tile validation, launch packet AOI fanout, and
> delayed tick-time damage/health/death/drop/experience commit for targeted
> ranged and object magic attacks. Zone also now owns object-magic MP spend,
> per-Spell cooldown rejection, and `ObjectMana` AOI fanout. Gateway routes
> shared `RangeAttack` through Zone and has learned object-target Magic routing
> wired from the same command path. Verification passed focused Zone
> ranged/magic authority tests, MP/cooldown rejection, invalid target
> rejection, a Gateway shared `RangeAttack` routing regression, the existing
> delayed melee regression, and locked Gateway check. The matching UCloud
> Gateway release `20260525T0630CST-zone-magic-mp-cooldown` is live, and
> public health plus WSS smoke
> `docs/generated/load/remote-zone-magic-mp-cooldown-wss-smoke-20260525.json`
> passed with `ready=1/1`, `capacityRejected=0`, `errors=0`, `messages=623`,
> and `ok=true`. A same-release 30-simultaneous baseline
> `docs/generated/load/remote-zone-magic-mp-cooldown-30active-baseline-20260525.json`
> confirms the current production policy still accepts 15 active and rejects
> 15 (`ok=true`, keepalive p95 `22076ms`), so 30-active gameplay feel remains
> open. Roadmap next: move Buff/stat/control/summon/AoE effects into
> Zone-owned skill state, then expand monster AI from melee to
> ranged/magic/Boss mechanisms, replace personal-session drop/economy mutation
> with a Zone/world-service transaction, and lower the 30-active burst latency
> before raising the live active cap.

> Latest blocked-source map-transfer roadmap sync: 2026-05-25 turns the live
> Chrome Library-door failure into a covered backend parity case. Manual
> production testing moved `Scout` from `BichonProvince 322:248` onto the
> Crystal direct movement source `322:247`, but the live Gateway stayed on map
> `0` instead of entering `0104 Library`. The source fix distinguishes ordinary
> blocked tiles from manifest-backed direct movement source cells: player
> movement may step onto the transfer source so the authoritative walk-on
> transfer can fire, while closed/static collision still blocks non-transfer
> movement and object occupancy. Shared Zone map collision now also prefers full
> original Crystal map collision data for map `0` instead of the starter
> fragment. Verification passed focused Simulation and Gateway Library
> regressions, existing direct walk-on transfer regressions, adjacent
> `crystal_manifest_movements`, Simulation/Gateway fmt check, and locked
> Simulation/Gateway check. Roadmap next: deploy a Gateway release with this
> patch, then retest the same Chrome route through `BichonProvince 322:247` to
> prove live transfer into `0104 Library`.

> Latest movement latency roadmap sync: 2026-05-24 closes the current
> user-reported "walk command delay / no frontend print" slice. The command was
> being sent, but the web client did not expose console movement logs by
> default, and the production Gateway still spent movement hot-path time
> generating full outcome snapshots. Player Web now exposes movement send/ack
> console logs behind `?movementLog=1`, mitigates the React #418 hydration
> warning path, and is deployed as `dpl_BommXyKsMcAX3Lmw4TYcg82a7Rsw`.
> Simulation/Gateway now skip outcome snapshots for low-latency movement/tick
> commands, and the live Gateway is release `20260524Tmovelowlatency`. Normal
> `https://mir2.obelisk.build` sessions now use
> `wss://165.154.65.136.sslip.io/ws` rather than the high-jitter custom-domain
> Worker WebSocket route. Verification passed focused Rust regressions, Gateway
> check, Web typecheck, public health, WSS smoke, and production browser
> movement capture
> `docs/generated/player-qa/movement-jitter/prod-normal-directws-keyboard-d-20260524T1513.json`
> with six walk ACKs at `555/522/516/523/517/517ms`, clean settle, and no
> rollback/blackout/critical-console/404 failures. Roadmap next: keep this
> direct Gateway WSS shape while broadening movement acceptance to multi-player
> and higher-pressure map clusters.

> Latest movement rollback roadmap sync: 2026-05-24 narrows the remaining
> "walks then snaps back" symptom to legitimate server corrections and removes
> two client/server mismatches that made those corrections look like rubber-band
> movement. Shared Zone now degrades standstill Run into an effective one-tile
> Walk in source, matching the current Crystal action goal, instead of
> hard-correcting the player at origin. Player Web keeps local prediction out of
> authoritative `world.entities` and waits for server ACK when map-region data
> is missing, out-of-region, or known-blocked. Verification passed Web
> typecheck, scoped diff check, local smoke
> `docs/generated/player-qa/movement-jitter/local-left-walk-wait-map-20260523T233000.json`,
> and production Web smoke
> `docs/generated/player-qa/movement-jitter/prod-left-walk-web-rollback-fix-20260524T0034.json`
> on deployment `dpl_3BwwKyjXY9UFZS3jSZk3vCsybCrW`, with zero visual jumps,
> zero logical rollback, zero scene blackout, no critical console errors, and
> no non-favicon 404s. The remote Gateway was then restarted to release
> `20260524T0310Z-rollbackfix`; remote WSS smoke
> `docs/generated/load/remote-rollbackfix-wss-smoke-20260524.json` passed, and
> post-Gateway production Web movement smoke
> `docs/generated/player-qa/movement-jitter/prod-left-walk-gateway-rollbackfix-20260524T0320.json`
> passed with the same rollback/blackout/error assertions green. Roadmap next:
> broaden production Chrome movement around NPC/monster clusters and region
> edges as a longer soak, rather than treating normal occupied-tile correction
> as a client rubber-band regression.

> Latest map-transfer roadmap sync: 2026-05-22 closed the direct walk-on
> movement trigger for Crystal map transfers. The previous 807-scene
> production QA screenshot set remains the resource/render proof, but it is not
> a traversal proof. `docs/generated/map/latest-crystal-map-reachability.json`
> now records the live traversal graph from Bichon map `0`: 463 maps,
> 1999 movement rows, 1906 direct rows, 93 special/ignored rows, 268 maps
> reachable by direct movement, and 185/284 positive-respawn maps reachable by
> that direct path. Runtime now transfers on Walk/Run arrival at a Crystal
> movement tile in both the personal session and the production shared-Zone
> Gateway route, while debug `crystal:<map>:<x>:<y>` remains blocked for normal
> production clients. Remote UCloud Gateway release
> `20260522T064157Z-walktransfer` is deployed and passed host/public health plus
> 1-client WSS smoke
> `docs/generated/load/remote-walktransfer-wss-smoke-20260522.json`. Roadmap next:
> add live production browser screenshots for
> representative direct-walk routes and separately classify the remaining
> non-direct maps by NPC/script/event/item/special entry path.

> Latest original-map spawn roadmap sync: 2026-05-21 closed the reported
> non-original Bichon monster leak by moving production Gateway bootstrap to the
> Crystal map runtime. Original map sessions now normalize map metadata from the
> Crystal respawn manifest and spawn current-map original NPC/monster rows
> instead of the starter scene's `Training Dummy` / `Field Wasp`. Saved
> non-default starts now also surface representative monsters from broad
> original respawn rectangles while the player is inside that data range, which
> closes the live QA case where a forest/cave map could load with no local
> roster visible. The all-map
> gameplay audit also now auto-detects the local full Crystal client root and
> passed strict mode with 463 maps, 6341 respawns, respawn failures 0, NPC
> failures 0, movement failures 0, and static failures 0. Roadmap next: deploy
> the Gateway/Web stack and run a live production smoke around Bichon plus a
> few respawn-heavy maps to verify the browser receives only original spawn
> surfaces.

> Latest Gateway scheduling roadmap sync: 2026-05-19 moved the 30-player server question from "health endpoint starves under load" to "gameplay burst latency still needs tuning". Release `20260519T141920Z-fastka` adds per-socket Redis route-refresh throttling, idle route-lease keepalive, env-tunable runtime tick cadence, configurable Tokio worker threads, blocking isolation for synchronous simulation action/tick/snapshot/save work, and a fast Web KeepAlive ACK path. The UCloud 4H8G host passed a 30-client WSS 5-minute soak with `ready=30/30`, no capacity rejection, no client errors, and 30/30 successful 5s health probes while Redis record/lease counts reached 30. Roadmap next: keep live capacity at 15 active players for feel safety, then reduce StartGame/bootstrap and movement-burst latency before promoting 30 active to the normal internal-test target.

> Latest Gateway health-soak roadmap sync: 2026-05-19 moved the 30-player question from "can 30 sockets enter" to "can the Gateway stay observable under 30-player entry pressure". Release `20260519T124942Z-healthfast` is now deployed and reduces Redis `/health` work to one key scan plus one `MGET`, with session-cache status on the blocking pool. Verification proved 30/30 clients for a 20-minute soak before the release and a 5-minute soak after it, but health probes still timed out during entry/runtime pressure and keepalive p95 remained high. Roadmap next: keep the live cap at 15 active players, then optimize Login/NewCharacter/StartGame scheduling or enforce small in-flight caps before retesting 30 active as the normal internal-test target.

> Latest Gateway Postgres-pool roadmap sync: 2026-05-19 moved the internal-test Gateway persistence path from one-connection-per-save pressure toward a bounded pooled Postgres profile. Account-store Postgres now uses reusable connections, one migration pass per pool, serialized source writes in the process, and account-scoped writes for hot account/character saves. The UCloud 4H8G Gateway is running release `20260519T105412Z-nogit` with pool size 8, and the 30-client WSS concurrent handshake artifact passed with `ready=30/30 capacityRejected=0 errors=0 ok=true`. Roadmap next: keep the live cap at 15 active players, then run a longer 30-player soak that proves `/health` stays responsive before treating 30 active as the normal internal-test target.

> Latest new-account roadmap sync: 2026-05-19 aligned first-login character selection with the expected Crystal account flow. New password accounts already had an empty select list; now missing password logins no longer auto-create the `Scout` Warrior template, and first-time Passkey/Wallet accounts also receive an empty character list before using the original select-screen `NEW` class/gender/name picker. Roadmap next: deploy this account-lifecycle fix to the live Gateway/Web stack and smoke a fresh Passkey account through NEW -> selected created character.

> Latest original Bichon intro quest-chain roadmap sync: 2026-05-18 converted the first visible original new-player chain from "backend quest data exists" into a repeatable gameplay route. The vertical slice now starts on original Bichon map `0`, talks to Assistant Jane/CraftLady/Merchant John, handles q1 carry-item readiness, q2 Scarecrow `GingerTea` Q drops, q3 talk handoff, q4 passive Deer close-melee plus Harvest `DeerMeat`, q4 turn-in, and q5 availability. Verification passed focused original Bichon intro coverage, full Simulation `vertical_slice` 6/6, shared Zone 77/77, security lifecycle 9/9, and Simulation/Gateway locked check. Roadmap next: broaden this from q1-q4/q5 availability into representative q5+ and mid/late 1-45 live-client route acceptance, including dialog text, route hints, quest markers, and reward UI feel.

> Latest Zone-native monster combat/drop roadmap sync: 2026-05-18 moved the first normal melee attack route into the shared-Zone producer while keeping Crystal launch/hit-frame timing, added the first native monster AI tick, and completed the monster-to-player damage write-back outbound. Gateway now seeds live map monsters into Zone, routes explicit shared monster `WorldCommand::Attack` to `PlayerAttackObject`, and Zone emits attack launch packets separately from tick-resolved strike/damage/health/death/drop/experience/kill-award packets. Zone-native monsters can walk toward nearby players, fire adjacent delayed melee-hit visuals, mutate Zone-held player HP, broadcast player `ObjectHealth`, and send `PlayerDamaged` so the Gateway applies the same HP loss to personal `SimulationSession` state for snapshots/save. Verification passed Simulation `shared_zone` 77/77, Gateway `shared_in_process` 40/40, Simulation `security_lifecycle` 9/9, focused delayed-hit/native-attack/AI-tick/HP-writeback regressions, and Simulation/Gateway locked check. Roadmap next: extend native routing to RangeAttack/Magic with real skill semantics, and expand from current seed drops into full Crystal drop-table exactness.

> Latest Postgres+Redis roadmap sync: 2026-05-18 moved prod-like runtime policy from "can use Postgres/Redis" to "must use Postgres/Redis". Gateway still supports JSON/in-memory for local development, but production/staging envs and explicit require flags now reject missing Redis route/session cache, validate required Redis availability on startup, and pair that with the existing Postgres source-of-truth account-store requirement. Staging and systemd examples now advertise Postgres account persistence plus Redis session/routing leases as the default internal-test profile. Roadmap next: normalize deeper character subsystems into database tables and add cross-Gateway owner handoff/shared Zone process work after the current single-process Gateway policy is stable.

> Latest ranking-system roadmap sync: 2026-05-18 promoted rankings from a placeholder/social-menu surface into a working Crystal packet/UI path. Source audit confirmed the original client/server ranking model uses `GetRanking { rankType, rankIndex, onlineOnly }` and `Rankings { myRank, listings, listingDetails, count }` across Overall plus Warrior/Wizard/Taoist/Assassin/Archer classes. The Rust runtime now builds those rankings from persisted characters with active-session overlay, Gateway exposes the Web `getRanking` command, and Player Web renders the ranking panel through the original System Menu social surface instead of static copy. Verification passed Rust fmt/check, Simulation ranking regression, Gateway command/event regressions, Web typecheck, and a live Browser smoke showing Overall and Online rankings for Scout with evidence at `docs/generated/player-qa/ranking-system/ranking-panel.png` and `ranking-smoke.json`. Roadmap next: extend ranking acceptance to real multi-account online/store scenarios after production persistence/shared roster work, then tune any original-client pagination and statue/NPC entry feel.

> Latest shared Zone drop-claim roadmap sync: 2026-05-18 completed the next queued shared-native drop arbitration slice. Shared ground drops are now synchronized into Zone state, manual pickup and IntelligentCreature pickup acquire Zone claims before the personal session awards gold/items, successful commits remove the object for observers and late joiners, and failed personal award/filter/full-bag paths cancel the claim and restore visibility instead of leaving stale `ObjectRemove` packets. Zone also owns nearest eligible drop selection for auto pickup, while Gateway's map layer is reduced to a mirrored read model for drop visibility/removal. Verification passed Simulation `shared_zone` 74/74 and Gateway `shared_in_process` 38/38. Roadmap next: move the producer side of drops, monster AI/combat, and item/gold award mutation further into shared Zone or an actor-owned world service so personal sessions stop being the gameplay authority behind those outcomes.

> Latest vertical-slice roadmap sync: 2026-05-18 promoted the current "playable slice" from a status description into a repeatable gate. The new Simulation vertical-slice suite covers the four requested main lines: all five classes can be created and enter game with correct class/gender/vitals and empty personal state, each class has a basic skill/combat loop, Bichon starter NPC/monster/quest/drop/reward flow completes, and shared multiplayer presence/movement/chat/drop ownership remains stable. Verification passed Simulation fmt, vertical slice 4/4, shared Zone 74/74, and security lifecycle 9/9. Roadmap next: use this gate as the regression floor while expanding from "one basic skill/task path per class/zone" to full Crystal skill trees, broader 1-45 live NPC walkthroughs, and native shared Zone monster AI/combat/drop ownership.

> Latest Redis route-admission roadmap sync: 2026-05-18 made Redis/session-cache routing part of the actual Web `StartGame` admission path. The Gateway now acquires a per-account/character route lease before entering the world, rejects duplicate online entry while another fresh owner holds that lease, releases failed pending leases, and keeps successful sessions on the existing refresh/owned-remove lifecycle. Redis status now counts lease keys directly, making pending entry locks visible in `/health`. Verification passed new route-admission regressions, existing in-memory and Redis route-lease tests, production Web path safety, session-cache suite, and health boundary coverage. Roadmap next: add a real owner handoff channel for cross-Gateway reconnect/Admin kick, then move distributed WS/Login/StartGame counters from process-local capacity to Redis-backed leases when multiple Gateway processes are used.

> Latest Gateway hot-path roadmap sync: 2026-05-18 split the next capacity optimization into two runtime controls. First, account lifecycle bursts are now independently limitable with `MIR2_GATEWAY_MAX_LOGIN_IN_FLIGHT`, `MIR2_GATEWAY_MAX_NEW_CHARACTER_IN_FLIGHT`, and `MIR2_GATEWAY_MAX_START_GAME_IN_FLIGHT`, and `/health` exposes those in-flight counters so registration/login/start-game pressure can be sized separately from online Zone tick load. Second, active character persistence now uses a dirty save queue: Web actions mark sessions dirty, saves flush by debounce/checkpoint/queue-pressure policy, and disconnect still forces the final active-character save so the last authoritative movement transform survives reconnect/logout. Verification covered focused in-flight capacity and save-queue regressions, production Web path safety, reconnect retention, Web typecheck/script syntax, and a live 4-client hot-path smoke under `docs/generated/load/gateway-hotpath-codex-smoke.json` with `ready=4/4 capacityRejected=0 errors=0 ok=true`. Roadmap next: replace the JSON account store with production persistence or an actor-owned store, then repeat the 2H2G/2H4G/4H8G bandwidth matrix as a longer soak with CPU/RSS/network evidence.

> Latest Gateway capacity roadmap sync: 2026-05-18 turned Gateway capacity from an implicit OS/resource outcome into explicit runtime policy. The web Gateway now has independent caps for WebSocket connections, active in-game sessions, and reconnect-grace leases via `MIR2_GATEWAY_MAX_WS_CONNECTIONS`, `MIR2_GATEWAY_MAX_ACTIVE_SESSIONS`, and `MIR2_GATEWAY_MAX_RECONNECT_LEASES`, with live `/health` capacity counters. The load harness now defaults to real production-safe player traffic instead of debug Stage5 commands and can assert expected capacity rejection counts. Verification covered unit capacity accounting, reconnect permit transfer/release, production command safety, Web typecheck/script syntax, and two live 4-client smokes: active-session cap `ready=2/4 capacityRejected=2 ok=true` and WebSocket handshake cap `ready=2/4 capacityRejected=2 ok=true` under `docs/generated/load/`. Roadmap next: choose production caps per deployment size and run longer soak/RSS/network evidence before checking the final accepted production-smoke concurrency gate.

> Latest reconnect grace roadmap sync: 2026-05-18 moved the in-game disconnect experience from frontend-only replay into a backed, repeatable Gateway grace path. The Web client already snapshots auth/character and replays login/start after unexpected socket close; Gateway now retains the active `GatewaySession` for a short reconnect lease, refreshes the session-cache route lease for that same grace window, and restores the retained session on the next authenticated `StartGame` for the same account/character. A new `npm run smoke:reconnect-resume` harness proves the player stays in game, sees bounded reconnect status, returns to `wsState=open`/`reconnectStatus=idle`, and keeps the same map/player position. Verification passed Gateway reconnect/production-safety/route-lease tests, Web typecheck/script syntax, and live smoke evidence under `docs/generated/player-qa/reconnect/`. Roadmap next: decide whether production deployment needs cross-process durable session ownership or a shared Zone service for reconnects beyond a single Gateway process.

> Latest original quest-chain roadmap sync: 2026-05-18 closed the current automated backend acceptance slice for normal Crystal quests from level 1 through 45. The roadmap state now includes generated Crystal quest packets plus parsed source quest-text tasks, runtime availability in the Quest Diary model, NPC quest links, accept/finish/share command handling, prerequisite-chain/level/class gates, kill/item/flag progress, carry and task-item lifecycle, reward option semantics, and backend regressions covering original quest availability/progress/rewards plus adjacent starter-quest behavior. Roadmap next: live human walkthrough for representative 1-45 NPC dialogs, translated wording, map-route hints, and any source-script branch nuance that only appears in the visual client.

> Latest all-map resource/gameplay roadmap sync: 2026-05-16 closed the current automated map-source and map-semantic gate. Web map coverage now records 463/463 Crystal manifest maps present/parseable, missing minimap indices `[]`, missing sampled map libraries 0, and `visualFallbackRisk.mapCount=0`, with 453 empty/out-of-range frame references tracked as Crystal source no-draw behavior instead of frontend fallback. Added the all-map gameplay audit for movements, respawns, NPC scripts, safe zones, safe-zone spell flags, doors, cell lights, fishing cells, drop rules, light/feature flags, and static map semantics: movement failures 0, respawn failures 0, static failures 0, unimplemented NPC commands 0. Simulation now finds the local full Crystal client root, fixes type-1 map cell stride parsing, suppresses invalid/special Crystal movement rows from runtime direct transfers, and leaves no-candidate respawn rows inert like Crystal. Verification passed both map audits, Web `npx tsc --noEmit`, Simulation fmt check, focused `crystal_manifest_movements` 2/2, and focused `spread_slots` 2/2. Roadmap next: final human visual walk-through across representative maps, doors/mechanisms/weather/light feel, and any map-specific script branches that require live gameplay judgment.

> Latest shared object-action roadmap sync: 2026-05-14 moved shared monster/generated-object action observer delivery onto Zone retained-object AOI. Gateway no longer directly queues those packets to every same-map observer; it seeds shared Monster/NPC objects into Zone and asks Zone to broadcast shared-object packets. Zone preserves object actor ids, rebases current-player local self result ids to authoritative Zone player ids, applies retained lifecycle guards, and filters delivery by object visibility. Verification passed focused Simulation shared-object regressions 3/3, Simulation `shared_zone` 69/69, Gateway `shared_in_process` 35/35, and Simulation/Gateway fmt/check. Roadmap next: Zone-owned shared drop claims/expiry and then deeper native monster combat/drop generation.

> Latest retained object authority roadmap sync: 2026-05-14 advanced shared Zone retained-object parity from visibility toward authoritative lifecycle/occupancy. Full retained Buff payloads now replay for late joiners and object-AOI entrants, stale post-death movement/mana/positive-health packets are suppressed, retained health cannot rise from stale personal-runtime snapshots before revive, and retained NPC/live-monster tiles now block player movement while dead/removed/drop/deco objects stay passable. Verification passed Simulation `shared_zone` 66/66, Gateway `shared_in_process` 35/35, and Simulation/Gateway fmt/check. Roadmap next: move the actual monster AI/combat/drop/NPC side-effect source of truth from personal-session bridges into native shared Zone authority.

> Latest retained object-vitals roadmap sync: 2026-05-14 added current health/mana as retained shared Zone object visibility state. Retained `ObjectHealth` packets now follow `ObjectMonster` for late joiners and players entering object AOI, update to the latest percent/expire payload, keep zero-health death ordering intact, and clear on revive. Retained `ObjectMana` now does the same for MP-bearing heroes/generated objects. Verification passed focused retained-object health/mana regressions 3/3, Simulation shared_zone 60/60, Gateway shared_in_process 35/35, and Simulation/Gateway fmt/check. Roadmap next: migrate the underlying damage/death/drop/mana resolution into native Zone authority.

> Latest retained harvest-corpse roadmap sync: 2026-05-14 moved another corpse/drop consistency fact into the shared Zone retained-object lifecycle. Non-player `ObjectHarvested` packets now mark the retained object harvested/dead, duplicate harvest completion is suppressed, stale later live spawns cannot erase the harvested corpse state, late joiners see the harvested dead anchor/direction, and player harvest animation packets remain player-state-neutral. Verification passed focused harvested retained-object regressions 3/3, Simulation shared_zone 57/57, Gateway shared_in_process 35/35, and Simulation/Gateway fmt/check. Roadmap next: native Zone ownership for harvest reward transfer and drop generation.

> Latest Crystal action-queue roadmap sync: 2026-05-23 moves shared movement
> from "latest intent plus run grace" to Crystal's ordered action semantics in
> both code and production evidence. Zone now owns a bounded ordered
> Walk/Run/Turn action queue per player, consumes actions on Crystal
> `ActionTime`, applies the 350ms Turn delay and 600ms Walk/Run windows,
> now degrades raw standstill Run into an effective Walk per the latest movement
> rollback correction above, and preserves
> Crystal's different failed-Walk versus failed-Run direction behavior. Player
> Web treats self `UserLocation` as confirmation/correction rather than a fresh
> movement animation, renders packet Walk/Run as one Crystal 600ms action even
> when Run spans two tiles, and caps local ActionFeed lead to the one-Run
> two-tile window so real corrections are not masked as stale echoes.
> Verification passed Simulation `shared_zone` 78/78, focused Gateway
> Walk+Run/Turn routing regressions, Simulation/Gateway fmt-check, Web
> typecheck, Web production build, remote Gateway release
> `20260523T071900Z-actionqueue`, Player Web action-queue verification deployment
> `dpl_HmHQ4CXfy7d895kHFMfiNLHWespN`, custom-domain production `/health`, and production
> walk/run captures
> `docs/generated/player-qa/movement-jitter/prod-action-queue-keyboard-walk-fix2-20260523T1331.json`
> plus
> `docs/generated/player-qa/movement-jitter/prod-action-queue-keyboard-run-fix2-20260523T1332.json`,
> both `ok=true` with zero visual jumps, logical rollback, scene blackouts,
> critical console errors, and non-favicon 404s. Roadmap next: continue
> shared-native combat/drop/NPC authority work and final human feel acceptance
> for broader blocked-tile/collision edges.

> Latest delayed combat status-result roadmap sync: 2026-05-14 broadened the delayed player combat bridge so status and buff result packets are not lost. Local-player-owned delayed strike bundles now carry matching `ObjectPoisoned`, `AddBuff`, `RemoveBuff`, and `PauseBuff` results for the struck target or actor, while unrelated tick combat from other attackers remains filtered. Verification passed the focused delayed-player-action filter regression. Roadmap next: use the retained Zone object state to move these combat-result facts out of personal-runtime mirroring.

> Latest retained Zone object roadmap sync: 2026-05-14 advanced the shared Zone from player-only AOI plus transient packet fanout toward a retained online-world read model. `ZoneRuntime` now stores rebased monster/hero/NPC/item/gold/deco spawn surfaces from `BroadcastPackets`, keeps them updated through movement, death/revive, zero-health death, hidden/effect, poison, buff, name/colour, and NPC image packets, expires retained visible object Buffs on Zone tick, tombstones objects on explicit object remove or intelligent-creature pickup, sends spawn/remove packets when players join or move into/out of retained-object AOI, dispatches retained object spawn/update/remove by object AOI instead of actor AOI, cleans owner-generated retained objects on owner Leave, despawns retained item/gold drops from Zone tick, blocks stale retained spawns from reintroducing removed objects or overriding dead retained lifecycle facts, and preserves `ObjectRevived` ordering against stale dead spawns. Verification passed focused retained-object regressions 16/16, Simulation shared_zone 55/55, Gateway shared_in_process 35/35, and Simulation/Gateway fmt/check. Roadmap next: use the retained object state as the stepping stone for native Zone combat/drop/NPC authority rather than relying on personal-runtime/Gateway mirroring.

> Latest shared entity-action observer roadmap sync: 2026-05-14 extended shared Gateway observer delivery for non-player action packets. Shared monster/generated-object `ObjectAttack`, `ObjectRangeAttack`, `ObjectMagic`, `ObjectProjectile`, and attacker-anchored `ObjectStruck` packets now reach same-map observers when their actor/source exists in the shared map, while player-origin combat remains on the existing Zone rebasing path; current-player local target ids are rewritten to the Zone object id for observer correctness, including same-batch local `ObjectHealth` / `DamageIndicator` / `ObjectDied` / `ObjectPoisoned` / `AddBuff` / `RemoveBuff` / `PauseBuff` results anchored by a shared-actor strike. Verification passed focused shared entity movement/action regressions 2/2 and Gateway shared_in_process 35/35. Roadmap next: make the resulting health/death/drop state native to shared Zone instead of mirrored from personal-runtime outcomes.

> Latest shared entity-movement observer roadmap sync: 2026-05-14 moved another monster/generated-object visibility edge into the shared Gateway layer. `ObjectTurn`, `ObjectWalk`, and `ObjectRun` packets for objects already present in the shared map now reach same-map observers even when produced by a personal runtime Tick rather than by Zone player movement, and the bridge avoids expensive snapshot reads on non-movement packets so player Run timing stays stable. Verification passed focused entity movement and Run timing regressions, Gateway shared_in_process 34/34, Gateway shared_zone_state 36/36, and Simulation/Gateway fmt/check. Roadmap next: replace the remaining personal-runtime source of monster AI movement/combat/drop generation with native shared Zone authority.

> Latest shared drop despawn-expiry roadmap sync: 2026-05-14 completed shared Gateway despawn deadlines for ground drops. Drops synced from personal snapshots, restored after blocked shared pickup, or committed from death packets now receive a Crystal-tick lifetime; Tick/KeepAlive removes expired shared drops, tombstones them, clears owner/despawn metadata, and broadcasts `ObjectRemove` to same-map clients. Verification passed focused shared expiry regressions 4/4, Gateway shared_zone_state 36/36, Gateway shared_in_process 33/33, and Simulation/Gateway fmt/check. Roadmap next: move the actual drop generation/award source of truth from personal runtime reconciliation into native shared Zone state.

> Latest shared drop ownership-expiry roadmap sync: 2026-05-14 added shared Gateway deadlines for Crystal ground-drop owner windows. Shared drops merged from personal snapshots or committed from death drops now carry a local expiry derived from `ownership_remaining_ticks`, and both manual pickup and IntelligentCreature auto pickup clear expired ownership before enforcing owner/group rules. Verification passed focused manual/auto expiry regressions, Gateway shared_zone_state 35/35, Gateway shared_in_process 32/32, and Simulation/Gateway fmt/check. Roadmap next: native shared Zone ownership for drop generation plus full shared drop despawn/expiry.

> Latest shared object-movement cache roadmap sync: 2026-05-14 tightened shared map-cache handling for ordinary Crystal object movement packets. `ObjectTurn`, `ObjectWalk`, and `ObjectRun` now refresh shared entity coordinates/direction through the same guarded transform path as push/backstep/dash packets, reducing stale shared coordinates for moved monsters or generated objects before native Zone-owned movement is complete. Verification passed a focused Gateway shared-zone-state movement regression. Roadmap next: native shared Zone ownership for monster AI movement, combat results, drops, and NPC side effects.

> Latest shared owned-generated lifecycle roadmap sync: 2026-05-14 moved player-owned generated object cleanup into the shared Gateway lifecycle. Summoned monsters now retain owner identity from `master_object_id` even when the personal runtime emits the local self id, stale ownerless snapshot merges preserve that owner identity, and player leave or map change removes owner-linked shared generated rows from the old map, tombstones them, clears stale lifecycle/drop anchors, and queues observer `ObjectRemove` packets on the same map. Verification passed Gateway shared_zone_state 33/33, Gateway shared_in_process 32/32, plus focused runtime/state regressions for hero disconnect, local-master summon disconnect, owner-preserving snapshot merge, and owner map-change cleanup. Roadmap next: native shared Zone ownership for generated object behavior, combat results, drops, and NPC side effects.

> Latest shared intelligent-creature pickup roadmap sync: 2026-05-14 closed the shared manual/auto pet pickup fallback that still depended on local personal-session ground drops. Gateway now performs shared-map target-location pickup for `IntelligentCreaturePickup`, and Tick-driven auto pickup scans shared drops by Crystal range/ownership/filter/grade/fullness rules. Successful shared pet pickup awards through the personal inventory/gold layer, removes the shared drop, and delivers `IntelligentCreaturePickup` to AOI observers even when unrelated pending packets are also waiting; filter-blocked items remain on the shared ground. Verification passed focused Gateway intelligent-creature coverage 6/6, Simulation shared_zone 38/38, Gateway shared_zone_state 29/29, Gateway shared_in_process 30/30, and fmt/check. Roadmap next: native shared Zone ownership for the remaining combat/drop/NPC side-effect generation.

> Latest shared spawn/skill-target roadmap sync: 2026-05-14 moved more generated-object and skill-reference behavior into the shared multiplayer bridge. Zone AOI delivery now includes hero, summoned monster, NPC spawn/update, and intelligent-creature pickup packets, with owner-local ids rebased in summoned-monster masters and self-target skill/projectile/struck references. Gateway shared map state now removes pet-picked drops, records generated ObjectHero/ObjectMonster/ObjectNpc rows for late snapshots, preserves dead-marker authority over late ObjectMonster packets, and applies pushed/backstep-style object transforms to shared entities without moving dead objects. Verification passed focused Simulation and Gateway shared-zone regressions for these packet families. Roadmap next: replace the remaining personal-runtime packet reconciliation for combat/drop/NPC effects with native shared Zone state and resolution.

> Latest shared dead-marker roadmap sync: 2026-05-13 removed snapshot-order dependencies from shared monster lifecycle state. Gateway now records `ObjectDied` / zero-health as an independent dead marker, rejects actions before the entity row exists, forces later live snapshots back to the dead packet location/direction, and can commit death drops from packet location alone. Out-of-order `ObjectRevived` and `ObjectHarvested` are covered so stale later snapshots cannot re-kill revived objects or reopen harvested corpses. Verification passed Gateway `shared_zone_state_` 23/23. Roadmap next: continue toward Zone-native combat/drop generation instead of relying on personal runtime result reconciliation.

> Latest shared delayed-damage roadmap sync: 2026-05-13 advanced delayed combat visibility in the shared multiplayer path. Gateway now recognizes player-owned delayed Tick damage by `ObjectStruck.attacker_id`, forwards that bundle and matching target health/death/remove/drop surfaces through Zone AOI, and avoids treating unrelated monster AI Tick packets as player-origin action packets. Shared zero-health packets now mark dead state even without max HP, and the stable shared-runtime pair fixture proves delayed `ObjectStruck/ObjectHealth` reaches the observer after `Attack -> Tick -> observer drain`. Verification passed the focused delayed-damage filter, focused no-max-HP death regression, focused delayed combat regression, Gateway `shared_zone_state_` 19/19, and Gateway `shared_in_process` 26/26. Roadmap next: continue migrating damage/drop generation into shared Zone authority.

> Latest shared transform-cache roadmap sync: 2026-05-13 removed another one-command lag between Zone authority and Gateway snapshots. `SaveTransform` now immediately refreshes shared player presence for both current and queued sessions, while `world_snapshot()` overlays `SelfPlayer` from Zone presence before returning the shared read model. Verification passed the focused transform-cache regression, Gateway `shared_zone_state_` 18/18, and Gateway `shared_in_process` 25/25. Roadmap next: keep pushing monster/combat/drop/NPC source-of-truth from personal-session bridges into shared Zone authority.

> Latest shared viewport/transform roadmap sync: 2026-05-13 hardened shared-world map/lifecycle semantics. Gateway scene snapshots are no longer treated as complete map snapshots, so monsters and drops outside one player's viewport remain shared until explicit `ObjectRemove`, shared pickup, or duplicate death-drop tombstones remove them; death-drop anchors now persist independently of corpse entity rows. Zone also now supports `TransformUpdate` rebasing plus retained `ObjectPlayer.transform_type` for late observers. Verification passed Simulation shared Zone 35/35, Gateway `shared_zone_state_` 17/17, and Gateway `shared_in_process` 25/25. Roadmap next: continue replacing personal-runtime generation/reconciliation with native shared authority.

> Latest shared revive-state roadmap sync: 2026-05-13 aligned revive packets with the shared death/harvest/drop lifecycle guards. Gateway now applies `ObjectRevived` to shared map state, clears dead/harvested/death-drop/remove tombstone markers for that object, restores HP when max HP is known, and prevents stale dead snapshots from undoing the revive. Verification passed focused Gateway revive/remove-tombstone coverage and Gateway `shared_zone_state_` 15/15. Roadmap next: keep moving monster respawn/lifecycle and corpse/drop authority out of personal runtime reconciliation.

> Latest shared harvest-corpse roadmap sync: 2026-05-13 moved harvested-corpse state into the shared Gateway map layer. Shared `ObjectHarvested` packets now mark the corpse harvested, stale personal snapshots cannot clear that marker, and later `Harvest` commands targeting the same corpse are rejected before duplicate personal-session harvest rewards can be produced. Verification passed focused Gateway reharvest coverage and Gateway `shared_zone_state_` 13/13. Roadmap next: continue migrating harvest/drop generation itself into shared authority.

> Latest shared death-drop roadmap sync: 2026-05-13 advanced shared monster death/drop handling from stale snapshot reconciliation toward explicit shared commit. Gateway now detects monster death through `ObjectDied` or zero-percent `ObjectHealth`, commits matching newly generated drops from the acting runtime into the shared map once, and tombstones duplicate stale drops from later personal-session syncs. Verification passed focused Gateway death-drop commit/spawn tests 3/3 and Gateway `shared_zone_state_` 12/12. Roadmap next: continue moving the actual damage/drop generation path into Zone/shared authority.

> Latest shared late-join status roadmap sync: 2026-05-13 retained late-status player visuals in Zone. After live packets update name colour/display name/guild name, player update equipment/light/wing state, poison, mount/riding, fishing, and level effects, future `ObjectPlayer` packets for late joiners/new AOI observers now carry those values instead of defaulting them away. Verification passed focused Simulation late-join retention and full Simulation shared Zone 35/35. Roadmap next: move monster damage/death/drop source-of-truth into shared authority.

> Latest shared late-status roadmap sync: 2026-05-13 expanded Zone observer delivery for Crystal player status and late-system visuals. `PlayerUpdate`, `DamageIndicator`, `ObjectColourChanged`, `ObjectGuildNameChanged`, `ObjectLeveled`, `ObjectName`, `MagicDelay`, `PauseBuff`, `MountUpdate`, `FishingUpdate`, `ObjectTeleportOut`, `ObjectTeleportIn`, and `ObjectDeco` now rebase to the authoritative Zone player id before AOI fanout. Verification passed focused Simulation late-status coverage and full Simulation shared Zone 34/34. Roadmap next: retain more late-status fields for late joiners and continue the larger monster damage/death/drop authority migration.

> Latest shared teleport/poison roadmap sync: 2026-05-13 added Zone authority for player-origin `UserLocation` action results and rebased poison visuals. Teleport/Blink-style skill outputs now update the shared player transform and occupancy before observer effects are sent, and `ObjectPoisoned` now carries the authoritative Zone player id to observers. Verification passed focused Simulation transform/poison tests and full Simulation shared Zone 33/33. Roadmap next: apply the same source-of-truth migration to monster damage, death, and drop generation.

> Latest shared skill-transform roadmap sync: 2026-05-13 advanced movement-skill outcomes from packet rebasing into shared Zone authority. The Zone bridge now extracts owner transforms from BackStep/Dash/DashAttack/AttackMove/Pushed-style packets, applies position/direction to `ZonePlayer`, updates occupancy, clears stale movement intent, emits `SaveTransform`, and rejects occupied/static destinations with an owner correction instead of observer fanout. Verification passed focused Simulation transform success/reject tests and full Simulation shared Zone 32/32. Roadmap next: repeat this authority migration for monster damage/death/drop outcomes instead of leaving them as personal-session reconciliation.

> Latest shared skill-movement roadmap sync: 2026-05-13 expanded the shared Zone bridge for visible Crystal movement-skill and special-skill packets. `ObjectBackStep`, `ObjectDash`, `ObjectDashFail`, `ObjectDashAttack`, `ObjectSitDown`, `SetConcentration`, `SetElemental`, `SetBindingShot`, `RemoveDelayedExplosion`, `ObjectSneaking`, and `ObjectLevelEffects` are now rebased to the authoritative Zone player id before AOI fanout. Verification passed focused Simulation coverage for movement/special skill observer packets and full Simulation shared Zone 30/30. Roadmap next: move from rebased visual fanout toward Zone-owned skill transforms, damage, monster state, and drop authority.

> Latest shared harvest roadmap sync: 2026-05-13 closed a narrow but player-visible harvest fanout gap in the shared Zone bridge. `ObjectHarvest` and `ObjectHarvested` are now rebased from the personal self object id to the authoritative Zone player object id before AOI delivery, and observer movement anchors come from Zone state. Verification passed focused Simulation harvest observer coverage plus full Simulation shared Zone 28/28. Roadmap next: continue replacing bridged personal-session harvest/combat outcomes with shared-native monster, harvest, and drop authority.

> Latest shared NPC/task roadmap sync: 2026-05-13 closed another shared multiplayer edge around NPC, group task semantics, and stale monster health. Gateway now proves shared-snapshot Village Guide `CallNpc @Main` can mutate the sparse session quest log, relays `ShareQuest` packets to online group members, lets group members pick owner-window shared drops when the owner is an online Zone player, prevents stale personal-session `ObjectHealth` from raising shared monster HP, and pushes shared monster snapshots back into the acting personal runtime before target-based and direction-only combat/harvest resolution. Verification passed focused Gateway regressions, Gateway shared state 9/9, Gateway shared registry 25/25, focused Simulation shared-monster snapshot application, and focused Gateway current-map shared-monster application. Roadmap next: push the remaining monster damage/death/drop source of truth from personal sessions toward shared-native Zone authority.

> Latest shared drop ownership roadmap sync: 2026-05-13 moved a concrete drop-authority edge into the shared layer. `GroundDropSnapshot` now preserves active ownership metadata, Gateway rebases personal owner object ids to Zone player object ids when syncing shared drops, shared pickup blocks non-owners during the owner window without tombstoning, and `ObjectItem` / `ObjectGold` spawn packets are included in Zone AOI fanout. Verification passed focused Simulation drop fanout and Gateway shared-zone-state 7/7. Roadmap next: continue toward shared-native monster damage/death/drop resolution and NPC/task side effects.

> Latest shared player appearance roadmap sync: 2026-05-13 retained hidden/dead/effect player state in Zone for late AOI visibility. The shared observer bridge now updates `ZonePlayer` from rebased self `ObjectHidden`, `ObjectHide`, `ObjectShow`, `ObjectDied`, `ObjectRevived`, and `ObjectEffect`, and subsequent `ObjectPlayer` packets carry the current fields. Verification passed Simulation shared Zone 25/25. Roadmap next: move the underlying combat/death/effect causes, monster damage, drops, and NPC/task mutations into shared authority instead of mirroring personal-session outputs.

> Latest shared Buff expiry roadmap sync: 2026-05-13 added a Zone-owned lifecycle step for retained visible player Buffs. `BroadcastPackets` now timestamps personal-runtime Buff packets, Zone converts Crystal relative `expire_time` into local expiry, `tick` sends `RemoveBuff` to AOI observers, and late joiners no longer receive expired Buff markers. Verification passed Simulation shared Zone 24/24. Roadmap next remains deeper shared authority for real skill effects, monster damage/drop ownership, and NPC/task state mutation.

> Latest shared Buff state roadmap sync: 2026-05-13 persisted active visible player Buff state inside the shared Zone. The observer bridge now updates each `ZonePlayer` from rebased self `AddBuff` / `RemoveBuff`, includes active visible buff types in `ObjectPlayer`, and sends rebased `AddBuff` details when another player first sees that actor. Verification passed Simulation shared Zone 23/23. Roadmap next: replace the remaining personal-session skill/Buff timing and monster/NPC side effects with shared Zone-owned authority.

> Latest shared skill/Buff roadmap sync: 2026-05-13 widened the shared Zone observer bridge for player-origin skill visuals. `BroadcastPackets` now rebases visible mana/Buff/effect/spell/push/revive/hide/show/toggle packets onto the shared Zone player object id, so other clients see more of the same Crystal skill/Buff packet surface that the acting client receives. Verification passed Simulation shared Zone 22/22. Roadmap next remains deeper authority migration: native Zone-owned skill effects, monster damage/drop ownership, and NPC/task state mutation.

> Latest shared NPC roadmap sync: 2026-05-13 advanced the shared Zone migration into NPC/task entry points. Crystal `CallNpc` packet handling now opens the runtime NPC dialog/script path, and shared Gateway sessions can interact with NPCs that come from the shared map snapshot even when the personal session's sparse ECS did not spawn that NPC. This directly addresses the "NPC visible but click has no response" multiplayer failure mode without rewriting the full quest engine in this slice. Verification passed Simulation shared Zone 21/21 and Gateway shared registry 20/20. Roadmap next: migrate NPC/task mutations and monster damage/drop ownership from personal-session fallback/reconciliation toward native shared authority.

> Latest shared-authority roadmap sync: 2026-05-13 advanced the Zone migration beyond movement/chat by adding AOI fanout for successful combat/skill packet surfaces and shared monster snapshot reconciliation. Gateway now forwards successful personal-session Attack/RangeAttack/Harvest/Magic action results to `ZoneRuntime`; Zone rewrites local self actor ids to the shared Zone player object id and sends `ObjectAttack`, `ObjectRangeAttack`, `ObjectMagic`, `ObjectProjectile`, `ObjectStruck`, `ObjectHealth`, `ObjectDied`, and `ObjectRemove` to nearby observers. The shared map layer now consumes health/death/remove packets so lower HP, dead state, and removed tombstones survive stale personal-session snapshot syncs, and stale attacks against shared dead/removed targets are rejected before local execution. This improves visible multiplayer consistency without rewriting the high-conflict combat/skills modules in this slice. Verification passed Simulation shared Zone 20/20, Gateway shared entity state 5/5, Gateway shared registry 20/20, and Simulation/Gateway fmt/check. Roadmap next: lift monster damage/drop ownership out of personal sessions and into native Zone/shared authority, then continue NPC/task and broader gameplay authority migration.

> Latest chat roadmap sync: 2026-05-13 completed the current Crystal chat parity layer for shared online play. Protocol coverage now includes `C.Chat` linked item payloads and all observed Crystal `ChatType` values through LineMessage. Runtime coverage now splits personal Session responsibilities from shared Zone delivery: Session handles bans/spam gate/local commands/item resolution, and Zone handles live recipients for normal AOI chat, whisper, group, guild, mentor, relationship, GM announcement, local/map/server shout, `$pos`, shout cooldown, and shout-scroll consumption. Gateway now propagates server shout globally through `ZoneOutbound::ToAll`, and Web recognizes Mentor/Relationship chat channels. Verification passed the focused Protocol, Simulation, Gateway, Rust fmt/check, and Web typecheck commands listed in the backend progress log. Next roadmap work remains broader shared-authority gameplay migration plus human Crystal visual/feel acceptance.

> Latest architecture roadmap sync: 2026-05-13 completed the Gateway routing, production WebSocket/player command-boundary, and live two-client browser-smoke phases of the shared multiplayer Zone MVP. Gateway shared sessions now join the simulation `ZoneManager`, route Walk/Run/Turn/Chat through the shared Zone, consume latest movement intents through Tick/KeepAlive, deliver Zone outbounds to other online sessions through pending queues, and apply Zone `SaveTransform` to the personal session before saves with a tick refresh for session-cache freshness. The WebSocket path can now switch into production-safe execution through production-like envs or `MIR2_GATEWAY_ENFORCE_PLAYER_COMMAND_SAFETY`, rejecting unauthenticated StartGame and blocking normal-client `MoveTo`, `Stage5Command`, and debug `crystal:<map>:<x>:<y>` transfers while preserving HMAC-verified passkey login. This turns the previous same-process multiplayer projection into a real shared online-world path for player presence and movement broadcasts while preserving existing shared drop, Trade, and ItemRental tests. Verification passed Gateway lib 121/121, Gateway shared registry 20/20, production WebSocket safety 3/3, Simulation shared Zone 12/12, security lifecycle 9/9, Simulation/Gateway fmt/check, Gateway health, two-client WebSocket smoke 2/2, ad hoc browser two-client Zone smoke, and the committed `npm run smoke:two-client-zone` harness with mutual player visibility, movement broadcast delivery, chat broadcast delivery, no console errors, and no non-favicon 404s. Next roadmap item: broaden human Crystal visual/feel acceptance and continue moving more non-movement gameplay authority into the shared Zone.

> Latest architecture roadmap sync: 2026-05-12 began the shared multiplayer Zone MVP as a product-architecture correction beyond the previous single-session simulation boundary. Added root AGENTS guidance that locks the rule "Session is not world. Zone is world." The simulation crate now has a synchronous `ZoneRuntime` / `ZoneManager` foundation with Join/Leave, AOI visibility, occupancy, static collision, latest movement intent consumption, Run intermediate-tile validation, movement/turn/chat broadcasts, unique object ids, `SaveTransform`, and session authoritative transform write-back. Security lifecycle tests lock the production-player rejection boundary for unauthenticated character lifecycle commands plus `PasskeyLogin`, `MoveTo`, `Stage5Command`, and debug crystal teleports. Verification passed shared Zone 12/12, security lifecycle 9/9, Simulation fmt, and locked Simulation check. Remaining roadmap work is the Gateway integration that routes StartGame/Walk/Run/Turn/Chat through Zone in production.

> Latest backend worker roadmap sync: 2026-05-11 closed a bounded Hero book/stat requirement exactness slice. Crystal source evidence from `HeroObject.UseItem` shows HeroInventory item use calls `CanUseItem` before the book branch, and `HumanObject.CanUseItem` rejects gender/class mismatches, all `RequiredType` stat gates (`MaxAC/MAC/DC/MC/SC`, `MinAC/MAC/DC/MC/SC`), level/max-level gates, and duplicate learned books before a Hero learns `UserMagic` and sends `NewMagic(hero=true)`. Rust Hero book learning now routes through a Hero CanUseItem-style requirement check that sums non-broken equippable HeroInventory stat totals before adding `heroLearnedMagics`, so stat-required books fail without consuming/learning. Verification passed focused Hero stat-book regression 1/1, focused `hero_inventory` 16/16 plus book/key integration 1/1, Hero AI integration 28/28, Simulation fmt check, and locked Simulation check.

> Latest backend worker roadmap sync: 2026-05-11 closed a low-conflict Friend/blacklist Stage 5 command-state gap. Crystal source evidence from `PlayerObject.AddFriend` shows normal friends and blocked entries share the same `Info.Friends` table, so adding a target already present in either view returns `PlayerAlreadyAdded` and does not convert friend <-> blacklist state. Rust high-level `social.friend` / `social.block` now follow that single-entry rule for modeled social state, keep self-target rejection localized through `CannotAddYourself`, and persist the resulting list through save/reload. Verification passed focused social/economy integration 3/3, adjacent social/mail lib regressions, Simulation fmt check, and locked Simulation check.

> Latest coordinator roadmap sync: 2026-05-11 advanced the post-frontend backend parity queue with two 5.5 xhigh workers plus local reconciliation. Hero learned magic now progresses level/experience from successful keyed Hero AI casts, persists through Stage 5 save state, emits `MagicLeveled` and level-up `MagicDelay`, and uses progressed learned level on later Hero AI decisions. Player Crystal movement skills now have source-shaped practice gates: `BackStep` and `ShoulderDash` no longer level on blocked/fail paths, and `FlashDash` only levels when the front target hit path is established. Mail exact parcel claims now use an all-or-nothing capacity preflight and consume serialized attachment payload after success, closing a rollback/persistence hole in late social/economy semantics. Verification passed `magic_packet_crystal_` 73/73, Hero AI 28/28, focused Hero progression 2/2, Mail 9+2, Simulation fmt/check, and targeted diff checks. Remaining roadmap work: Hero book/stat requirement exactness, mentor/stat skill-gain modifiers, broader late social/economy exactness, and final human visual/feel acceptance.

> Latest Hero progression sync: 2026-05-11 closed the bounded Hero learned-magic level/experience loop after the book/key/save slice. Stage 5 `heroLearnedMagics` now starts book-learned Hero spells at Crystal `Level=0`, `Key=0`, `Experience=0`, assigns keys through the existing Hero `MagicKey` path, advances learned experience when Hero AI successfully uses a keyed learned spell, emits `MagicLeveled` for each practice update plus `MagicDelay` on level-up, persists the updated level/experience, and feeds the progressed learned level back into existing Hero AI level gates, damage, and cooldown selection. Crystal source evidence: `MagicInfo.cs::UserMagic`, `HeroObject.cs::UseItem` / `CanUseMagic`, Hero class `CanUseMagic` selection, and `HumanObject.cs::LevelMagic`. Verification passed focused Hero progression 2/2, Hero AI integration 28/28, focused `hero_inventory` 15 lib tests plus book/key integration 1/1, Simulation fmt, and locked Simulation check. Remaining roadmap risk is wider Hero book/stat requirement parity and exact Crystal skill-gain multiplier/random/mentor tuning.

> Latest coordinator roadmap sync: 2026-05-11 closed the current Hero/Guild/frontend-feel verification pass. Hero learned-magic creation/key/save and Guild alliance runtime-only persistence remain green under the current backend gate, while Player Web now fixes the held-run plus repeated target-click rollback reported by the live movement harness. Evidence: `r-direction-lag-logical-rollback-0511-fix-bust-063119.json` records `ok=true`, `settle.status="settled"`, `pendingPlanAtEnd=null`, final `{338,270}`, `predictedPlayer=null`, no visual/logical rollback warnings, and no direction-lag warnings; route-spam and blocked-target regressions stayed green with explicit target-blocked non-failure. Verification passed locked GameData/Protocol/Simulation/Gateway check, Simulation/Gateway fmt, full locked `mir2-simulation` 856/856 plus Hero AI 26/26, focused Hero AI 26/26, focused `guild_` 16/16, Gateway shared registry 15/15, Web typecheck, movement/NPC script syntax checks, live movement captures, screenshot inspection, and targeted diff checks. After the later Hero progression sync above, remaining roadmap risk: Crystal's stricter single-action input queue / immediate hard-correction feel, wider Hero book/stat requirement parity, exact skill-gain tuning, any future Guild alliance UI evidence, and human visual/feel acceptance.

> Latest 5.5 Hero learned-magic closure sync: 2026-05-11 moved the Hero learned-magic work from AI-only consumption to a real Crystal-shaped creation/update loop. HeroInventory `UseItem` on a valid Crystal book now creates `heroLearnedMagics` with level 0/key 0, emits `NewMagic(hero=true)`, and consumes the Hero book, matching the audited `HeroObject.UseItem` / `UserMagic.GetInfo(hero)` path. `MagicKey` now routes `key > 16` or `oldKey > 16` to Hero learned state like Crystal `MirConnection.MagicKey`, persists the assigned Hero key, and Hero AI remains gated until that key is nonzero. Verification passed the new focused loop, Hero AI 26/26, focused `hero_inventory`, Simulation fmt, and locked Simulation check. Remaining roadmap risk is precise Hero magic level/experience progression and wider Hero book/stat requirement coverage beyond this bounded book/key path.

> Latest 5.5 Guild alliance deep-surface sync: 2026-05-11 re-audited Crystal `GuildObject`, `GuildDialog`, `RequestGuildInfo`, packet enums, and `GuildInfo.Save/Load`. The current Crystal tree exposes alliance only as runtime `GuildObject.AllyGuilds` / `AllyCount` plus the `CanAlterAlliance` rank bit; it has no alliance packet/dialog action and does not save alliance state in `GuildInfo`. Rust now keeps the in-session Stage 5 alliance readback but does not rehydrate alliance fields from saved Stage 5 JSON after reload. Verification passed focused `guild_` 16/16, the new alliance save/reload regression, `cargo +1.89.0 fmt --check -p mir2-simulation`, and `cargo +1.89.0 check --locked -p mir2-simulation`.

> Latest 5.5 xhigh roadmap sync: 2026-05-10 closed the Hero learned-magic and Guild alliance visible-info tranche and refreshed NPC click/marker evidence. Hero AI now honors optional saved `heroLearnedMagics` as the Crystal learned-spell source, requiring `key > 0` and learned-level eligibility before a Wizard Hero can choose a spell from the priority chain, while preserving default behavior for older saves with no learned list. Guild alliance now has both mutation state and visible readback: `RequestGuildInfo` type 0/1 keeps Crystal notice/member responses and exposes ally count/list/recent broadcasts as guild chat visibility instead of adding an unsupported typed packet. NPC automation proves MirGuide out-of-range clicks approach then interact, adjacent clicks interact without movement, and the quest marker is centered over the NPC body anchor. Verification passed `guild_` 15/15, Hero AI 25/25, full locked `mir2-simulation` 855/855 plus Hero AI 25/25, Gateway shared registry 15/15, package fmt/check, Web typecheck, NPC/movement script syntax, NPC evidence parse, and targeted diff checks. Remaining roadmap risk is Hero book/key progression into learned magic state, deeper client/dialog presentation for Guild alliance if Crystal source demands it, broader Hero exactness, and final human visual/feel acceptance.

> Latest 5.5 xhigh roadmap sync: 2026-05-10 closed the next Guild/Hero/client-feel tranche. Guild alliance is now a verified Stage 5 semantic surface matching Crystal's `AllyGuilds`, `AllyCount`, and `CanAlterAlliance` data/permission shape where this source tree does not expose a typed alliance packet: `guild.ally` / `guild.unally` handle known-guild lookup, duplicate/self/missing/war rejection, broadcast logging, and ally count maintenance. Wizard Hero `ProcessAttack` now covers the late Crystal priority chain through `TurnUndead`, `FlameDisruptor`, `Vampirism`, and `FrostCrunch` before the classic single-target spell fallbacks, including Vampirism heal and FrostCrunch freeze evidence. Client input-feel automation now includes blocked/unreachable repeated target clicks with `r-blocked-target-nonfailure-0511-fixed6`, proving final settle with no prediction residue, no route spam, no jumps, no console errors, and no non-favicon 404s. Verification passed `guild_` 14/14, Hero AI 23/23, full locked `mir2-simulation` 854/854 plus Hero AI 23/23, Gateway shared registry 15/15, package fmt/check, Web typecheck, movement script syntax, evidence parse, and diff checks. Remaining roadmap risk is exact Hero learned-magic inventory/progression, any deeper Crystal alliance packet surface if discovered, broader class Hero AI exactness, and final human Crystal visual/dialog/feel acceptance.

> Latest 5.5 xhigh roadmap sync: 2026-05-10 closed the second Guild/Hero/client-feel tranche in this continuation. Guild war behavior now has timed lifecycle semantics matching Crystal's `GuildAtWar`: start changes colour, active war state expires after modeled duration, end-war chat is recorded, and colour returns to normal. Wizard Hero behavior now matches the next Crystal `ProcessAttack` priority section for Repulsion and area spells before single-target fallback, with packet/state regressions for Repulsion, FireBang, and ThunderStorm. Client route-spam obstacle handling now proves final settle rather than merely no-jump sampling: `r-route-spam-obstacle-settle-followup5` reports `settle.status="settled"`, `movementPlan=null`, `predictedPlayer=null`, empty direction queue, `jumps=[]`, and `routeSpamWarnings=[]`. Verification passed `guild_` 12/12, Hero AI 20/20, full locked `mir2-simulation` 852/852 plus Hero AI 20/20, Gateway shared registry 15/15, package fmt/check, Web typecheck, movement script syntax, live capture, and diff checks. Remaining roadmap risk is now focused on full Guild alliance semantics, later Wizard Hero spell branches, exact Hero learned-magic state, and final human Crystal visual/dialog/feel acceptance.

> Latest 5.5 xhigh roadmap sync: 2026-05-10 closed another concrete Guild/Hero/client-feel tranche. Guild behavior now reaches beyond storage into Crystal war and territory packet semantics: request-war prompt, `GuildWarReturn` validation/failures, bank-cost deduction, active-war state, duplicate rollback, and territory listing/purchase rollback are covered by focused regressions. Wizard Heroes now support Crystal `ProcessFriend` self buffs, casting `MagicShield` before `MagicBooster` with MP/cooldown/ObjectMagic/AddBuff evidence and level/low-mana gates, on top of the earlier ranged FireBall/GreatFireBall/ThunderBolt loop. Client input feel now has route-block correction memory and a `routeSpamObstacle` capture path proving repeated target changes around blocked routes no longer produce jump/route-spam warnings. Verification passed focused `guild_` 10/10, Hero AI 17/17, full locked `mir2-simulation` 850/850 plus Hero AI 17/17, Gateway shared registry 15/15, Simulation/Gateway fmt, locked four-package check, Web typecheck, movement script syntax, live route-spam obstacle capture, and targeted diff checks. Remaining roadmap risk is fuller alliance/war lifecycle broadcasting, Wizard Hero full attack-spell priority and learned-magic inventory semantics, and final human Crystal visual/dialog/feel acceptance.

> Latest 5.5 xhigh roadmap sync: 2026-05-10 closed the next concrete Guild/Hero/client-feel risks. Guild rank/storage is no longer only a permissive Stage 5 shell for the covered operations: notice edits, guild storage gold, store/retrieve/move, permission rejections, safe-zone gates, and `DontStore` / rental `DontStore` item rejection now have Crystal-shaped state and packet regressions, with exact stored item payload preservation. Wizard Heroes now have a bounded Crystal ranged spell loop with MP/cooldown/ObjectMagic/delayed damage evidence. Client input feel now keeps Crystal-style 600ms visual continuity when server confirmation arrives for the same tile, reducing the visible snap-back during hold-to-target-click transitions. Verification passed focused `guild_` 5/5, `trade_` 12/12, Hero AI 13/13, full locked `mir2-simulation` 845/845 plus Hero AI 13/13, Simulation/Gateway fmt, locked four-package check, Web typecheck, movement script syntax, and targeted diff checks. Remaining roadmap risk: guild war/alliance/territory broadcasting depth, Wizard Hero defensive/friend skills and higher spell priorities, broader route spam/obstacle feel, and final human Crystal visual/dialog/feel acceptance.

> Latest coordinator verification sync: 2026-05-10 consolidated the current 5.5 xhigh worker round into roadmap evidence. TradeEscrow now has true two-account delivery/rollback/full-bag/disconnect coverage, Taoist Hero owner healing is covered by class-priority regressions, and the Web action-feel bridge has a live hold-run plus repeated target-click capture. Verification passed focused Simulation `trade_` 12/12, Gateway shared registry 15/15, Hero AI 11/11, full locked `mir2-simulation` 843/843 plus Hero AI 11/11, Simulation/Gateway fmt, locked GameData/Protocol/Simulation/Gateway check, Web typecheck, movement script syntax, and targeted diff check. Remaining roadmap risk is Guild rank/permission/storage depth, remaining Hero class AI breadth, Crystal frame/input feel acceptance, and final human visual/dialog acceptance.

> Latest Worker TradeEscrow sync: 2026-05-10 closed the concrete true two-account Trade escrow roadmap risk. The runtime now blocks bound, soulbound, rental-bound, rental-owned, rental-expiring, rental-locked, and otherwise invalid offered items before escrow lock; gateway shared settlement checks both parties' bag capacity before committing delivery; full-bag delivery failures roll back both locked offers; and partner cancel/disconnect restores locked gold/items. Successful two-session trades continue to deliver real gold and serialized item state through the existing Crystal-shaped trade packet surface. Verification passed locked simulation `trade_` tests 12/12, locked gateway `shared_in_process_registry_` tests 15/15, package fmt for Simulation/Gateway, and locked Simulation/Gateway check. Remaining roadmap risk is now frame-driven client input queue feel, guild rank/storage permission depth, remaining Hero class breadth, and final human Crystal visual/dialog/feel acceptance.

> Latest Worker HeroClassBreadth sync: 2026-05-10 added the next non-Warrior/non-Archer Hero AI class-priority slice for Taoist. Taoist Heroes now prioritize owner support before hostile melee selection, unlock Crystal `Healing` from manifest level gates, spend Hero MP with `ObjectMana`, expose a Hero `ObjectMagic(Healing)` cast, emit owner `ObjectHealth`, and retain a cooldown gate so the heal is not repeated on the next tick. Focused Hero AI regressions now cover the positive level-7 owner-heal path and the below-gate level-6 rejection path. Verification passed Hero AI integration 11/11, locked `mir2-simulation` check, and the later coordinator Simulation/Gateway fmt plus full Simulation regression pass.

> Latest coordinator sync: 2026-05-10 advanced the skill-system / late-semantic track with a verified backend timing close. The runtime now enforces Crystal-style packet action timing for `Walk`, `Run`, `Attack`, `RangeAttack`, and `Magic`, returning `UserLocation` and suppressing duplicate action packets when commands arrive before the modeled move/attack/spell delay. The same close reconciled Archer Hero AI (`Concentration`, `StraightShot`, Hero MP, `ObjectMana`, `SetConcentration`, ranged damage) and Mail-Parcel fidelity (serialized attachments, opened/locked flags, remote delivery, exact claim item state). Verification passed action-timing regressions, `magic_packet_crystal_` 73/73, `packet_` 280/280, Hero AI 9/9, Mail 9/9, full locked `mir2-simulation` 841/841 plus Hero AI 9/9, package fmt, locked four-package check, and targeted diff checks. Remaining roadmap risk is now concentrated in frame-driven client input queue feel, true two-account Trade escrow semantics, deeper guild rank/storage permissions, remaining Hero class breadth, and final human Crystal visual/dialog/feel acceptance.

> Latest Mail-Parcel sync: 2026-05-10 moved player mail parcels closer to Crystal behavior. The runtime now accepts `SendMail` attachment unique IDs, serializes attached item state into mail, validates recipient/items/cost before mutation, removes sender item/gold on success, persists remote recipient mail via account-store Stage 5 state, returns parcel item previews plus opened/locked flags in `ReceiveMail`, and claims exact serialized item state through `GainedItem` / `ParcelCollected`. Focused mail regressions passed 9/9, locked `mir2-simulation` check passed, and coordinator-reconciled package fmt passed.

> Latest Hero class AI sync: 2026-05-10 extended Hero combat beyond the prior Warrior skill slice with bounded Archer semantics. Archer Heroes now level-gate Crystal `Concentration` and `StraightShot`, maintain private Hero AI cooldown/buff state, spend Hero MP with `ObjectMana`, emit `SetConcentration` once while active, and tag ranged Hero attacks with `StraightShot` plus Crystal magic-level damage scaling. Verification passed Hero AI integration 9/9, locked `mir2-simulation` check, and coordinator-reconciled package fmt.

> Latest Worker Agility sync: 2026-05-10 closed the code-side roadmap gap for broad monster Agility import/application. Crystal monster generation now reads stat 11 into `CrystalMonsterTemplate.agility`, projects it to respawn templates as `monster_agility`, and the runtime attaches `MonsterCombatStats` from that imported value across Crystal spawn tables, current-map visible monster imports, respawns, and dynamic template spawns. Focused coverage proves a nonzero-agility Crystal template spawned through the production runtime helper can miss without passive accuracy and hit with the modeled `Fencing` / `SpiritSword` accuracy path. Verification passed the focused Simulation regression, game-data manifest loads, JS syntax check, Rust fmt, and locked GameData/Simulation check; source-data refresh from `Server.MirDB` remains a Windows data-refresh follow-up because the DB is absent on this Mac workspace.

> Latest Hero deep follow-up: 2026-05-10 moved Hero combat past carried-equipment projection with a bounded Warrior skill surface. Saved/spawned Warrior Heroes now unlock modeled `Slaying` / `FlamingSword` at Crystal magic level gates, emit the matching spell and level on Hero melee `ObjectAttack`, add Slaying's passive DC bonus, and scale scheduled Hero hit damage for FlamingSword. Verification passed Hero AI integration 7/7, `cargo +1.89.0 fmt --check -p mir2-simulation`, and `cargo +1.89.0 check --locked -p mir2-simulation`. Remaining roadmap risk is full Hero magic inventory/learning, mana/cooldown persistence, wider class-specific Hero skill exactness, broader data/late-system semantics, and human Crystal client acceptance.

> Latest 5.5 xhigh closure: 2026-05-10 completed the next backend parity hardening slice after the skill/Hero/Fishing continuation. Crystal passive accuracy is now modeled for `Fencing`, `Slaying`, and `SpiritSword` with equipment `Accuracy`, monster `Agility` hit rolls, Crystal miss `DamageIndicator`, melee passive progression, and accuracy-based `MPEater` recovery. Hero equipment/stat projection now feeds both Hero AI damage and `HeroInformation`, Fishing now uses real slot-backed bait/hook/float/finder/reel items and durability/autocast gates, and Market settlement now covers underbid rejection plus gross accepted bid and 5% commission net seller payout. Verification passed focused passive accuracy 1/1, `magic_packet_crystal_` 73/73, Fishing 11/11, Market 1/1, Auction 6/6, Hero AI 5/5, full locked `mir2-simulation` 836/836 plus Hero AI 5/5, Rust fmt, locked GameData/Protocol/Simulation/Gateway check, and targeted diff checks. Current remaining roadmap risk is no longer these four concrete gaps; it is broader data population for monster Agility, deeper guild/market multi-account production semantics, full Hero equipment/skill family exactness, and human Crystal client visual/dialog/feel acceptance.

> Latest 5.5 xhigh multi-agent continuation: 2026-05-10 extended the Crystal skill, Hero, Fishing, and social-economy closure after the Hero/ItemRental sync. The remaining generated Crystal magic-manifest spells now have explicit runtime routing instead of falling through to generic cast damage; a local manifest scan reports `unmatched manifest spells: 0`. Player melee/ranged packet paths now attach Crystal spell surfaces for `Thrusting`, `FlamingSword`, `Slaying`, `Focus`, and incoming-hit `CounterAttack`, and model bounded passive follow-ups for `FatalSword`, `MPEater`, `Hemorrhage`, and `Meditation` with `ObjectAttack`, `ObjectRangeAttack`, `ObjectMagic`, `ObjectEffect`, `ObjectMana`, `ObjectPoisoned`, and `SetElemental` evidence. Hero now has a bounded combat AI slice for Attack/Follow/CounterAttack behavior, Warrior/basic `ObjectAttack`, Archer `ObjectRangeAttack`, scheduled monster damage, and chase fallback. Fishing now resolves Crystal fishing drops/events instead of fixed `Walleye`, models rod/bait stat chance, reel miss/no-space/gold handling, reel durability proxy, and `GiantKeratoid` fishing event spawn. Mail now rejects sending to blocked friends with the Crystal blacklist system message without deducting gold or creating mail. Verification passed focused `magic_packet_crystal_` 72/72, Hero AI 3/3, Fishing 7/7, blacklist mail 1/1, full locked `mir2-simulation` 831/831 plus integration Hero AI 3/3, `mir2-simulation` fmt/check, and targeted diff checks. Remaining risk is exact Crystal tuning for hit-rate/stat passives such as `Fencing` / `SpiritSword`, Hero equipment/stat math, embedded fishing slot item fidelity, deeper market/guild semantics, and human visual/effect acceptance.

> Latest uninterrupted parity sync: 2026-05-10 advanced the 100% Candidate backend track. Hero map gating now follows Crystal `NoHero` rules for transfer, `NewHero`, and `ChangeHero`; Hero inventory transfer/take-back/use now moves, persists, and consumes Hero-bag items; invalid Hero auto-pot item indexes normalize like Crystal and auto-pot consumes matching Hero inventory potions. ItemRental now has a tested shared two-account path for adjacent invite, borrower fee lock, lender item lock, confirm-time gold/item delivery, lender rented-record updates, partner-cancel rollback, expiry return mail, and death-return-before-drop semantics with exact returned `ItemState` preservation. Skill-system parity now rejects unlearned player `SpellToggle` packets, persists stateful toggles, models `FlamingSword` MP/state behavior, models `CounterAttack` buff type 18 AC/MAC payloads, cycles MentalState buff type 19 values with archer shot damage penalties, adds Crystal repulsion-family semantics for `Repulsion` / `EnergyRepulsor` / `FireBurst` with adjacent lower-level `ObjectPushed` movement and ThunderElement repulsion-only damage, adds `StormEscape` target relocation, `ObjectEffect`, TemporalFlux teleport penalty, and nearby delayed damage, adds Crystal archer semantics for `Concentration`, `ElementalShot`, `ElementalBarrier`, `StraightShot`, `DoubleShot`, `BackStep`, `BindingShot`, `VampireShot`, `PoisonShot`, `CrippleShot`, `NapalmShot`, `DelayedExplosion`, and `Trap` with `SetConcentration`, `SetElemental`, orb gather/spend, one/two delayed ranged hits, opposite-facing relocation, `UserBackStep` / `ObjectBackStep`, blocked distance-0 reporting, delayed damage/heal, Green poison ticks, visible buff types 16/17, active-buff consumption, `RemoveBuff`, buff type 25, target-centered area damage, delayed marker/effect/removal, and Trap `ObjectSpell` root behavior, adds Wizard HellFire / FireBang / IceStorm / Blizzard / MeteorStrike / FireBounce / MeteorShower / ThunderBolt / ElectricShock / FlameDisruptor / IceThrust packet-state surfaces for forward/side-lane fire, target 3x3 damage, 5x5 ground spell spawn/persistent damage, chain projectile bounces, primary/secondary meteor damage, undead thunderbolt bonus, electric-shock root, non-undead bonus damage, and three-column ice/Frozen poison behavior, adds Taoist MassHealing / HealingCircle / Curse / Purification / Revelation / Poisoning / PoisonCloud / Plague / TrapHexagon packet-state surfaces for delayed area healing, HealingCircle `ObjectSpell`, amulet/poison consumption, hostile debuff payloads, debuff removal, target-health reveal packets, monster poison projection/ticks, poison cloud ground ticks, plague debuff branches, hostile area root, and eight-point TrapHexagon `ObjectSpell` rings, adds LightBody / MoonLight / DarkBody / Hiding / MassHiding buff-hidden semantics with Agility stats and `ObjectHidden` hide/reveal packets, and adds FrostCrunch freeze/root, Vampirism damage-to-heal, TurnUndead undead-only level-gated kill behavior, EnergyShield buff type 20 shield stats, ImmortalSkin buff type 23 defence payloads, PetEnhancer friendly/summoned monster buffing, LionRoar `LRParalysis`, and BattleCry hostile reacquire behavior. Verification passed focused Hero/NoHero/auto-pot, focused Hero inventory/auto-pot 25/25, ItemRental expiry/death/mail, focused skill toggle 6/6, casting 13/13, magic-packet Crystal skill tests 54/54 after adding Hiding/FrostCrunch/Vampirism/TurnUndead, EnergyShield/ImmortalSkin/PetEnhancer/LionRoar/BattleCry, MentalState/NapalmShot/DelayedExplosion/Trap/ExplosiveTrap/PoisonSword/PoisonCloud/Plague, and HellFire/FireBang/IceStorm/Blizzard/MeteorStrike/FireBounce/MeteorShower/ThunderBolt/ElectricShock/FlameDisruptor/IceThrust on top of FireWall/Lightning/ThunderStorm, shared Gateway registry 13/13, Rust fmt, and locked four-package check for GameData/Protocol/Simulation/Gateway. Remaining roadmap items stay explicit: broader Crystal per-profession skill semantics beyond the newly covered archer/Taoist/Wizard/stealth/control packet slices, Hero combat/equipment AI, and final human Crystal visual/dialog/feel acceptance.
> Follow-up skill slices: `ShoulderDash`, `FlashDash`, and `SlashingBurst` now cover Crystal dash / dash-attack / attack-move packet surfaces, target push or stun side effects, and focused regressions for move, block, and delayed damage cases. `FireWall`, `Lightning`, `ThunderStorm`, `HellFire`, `FireBang`, `IceStorm`, `Blizzard`, `MeteorStrike`, `FireBounce`, `MeteorShower`, `ThunderBolt`, `ElectricShock`, `FlameDisruptor`, and `IceThrust` now cover Crystal delayed ground/line/range damage behavior. `Hiding`, `MassHiding`, `FrostCrunch`, `Vampirism`, `TurnUndead`, `EnergyShield`, `ImmortalSkin`, `PetEnhancer`, `LionRoar`, `BattleCry`, `MentalState`, `NapalmShot`, `DelayedExplosion`, `Trap`, `ExplosiveTrap`, `PoisonSword`, `PoisonCloud`, and `Plague` now have focused packet/state regressions.

> Latest skill and late-system semantic sync: 2026-05-08 completed. The next requested parity slice moves Hero from Stage 5 state-only control surface to a visible Crystal-shaped actor: `ObjectHero` now preserves owner name, the runtime spawns and snapshots the Hero entity, frontend labels it as the owner's Hero, movement emits Hero `ObjectWalk`/`ObjectRun` follow packets, spawn-state uses Crystal `Summoned=2`, default Hero `SpellToggle` routes to the Hero actor, and Hero auto-pot settings round-trip through `HeroInformation`, `SetAutoPotValue`, and `SetAutoPotItem`. The skill path now emits server `ObjectProjectile` for the currently modeled targeted projectile spells, and MagicBooster applies Crystal buff type 21 with MinMC/MaxMC plus ManaPenaltyPercent stats. Verification passed focused Hero protocol/runtime tests, Hero 18/18, SpellToggle 2/2, MagicBooster 1/1, focused projectile skill regression, locked Protocol/Simulation/Gateway fmt/check, and Web typecheck. Remaining 100% Accepted work is now narrower: exact Hero combat/equipment AI, broader per-spell tuning across every Crystal profession, and final human visual/feel acceptance.

> Latest Stage 5 dirty-save full-smoke sync: 2026-05-08 completed. The Candidate automation path is stable on reused demo saves when explicitly run with `MIR2_STAGE5_ACCOUNT_MODE=demo`, while the default smoke run now uses a fresh throwaway account so human `demo/Scout` acceptance state is not polluted. Runtime normalizes known potion metadata and dirty duplicate inventory/storage unique IDs, `qa.giveItem` seeds usable red/blue potions, and Player Web smoke verifies split/use/drop/store/take-back/pickup through exact `uniqueId` / `objectId` checks with all bag containers and belt included where Crystal stacking can move consumables. The live local Gateway/Web full smoke captured 114 screenshots with `criticalConsoleErrorCount=0`, `compactMatrixCount=3`, `systemMenuSocial=44`, and verified inventory split, storage take-back, blue-potion pickup, gold pickup, and belt use on real command paths. Verification passed Web script syntax/typecheck, focused Simulation `stage5_qa_give_item_seeds_usable_healing_metadata` 1/1, focused `unique_id` 13/13, locked Simulation/Gateway check, Rust fmt, and the full Stage 5 UI smoke. Remaining 100% Accepted work is human Crystal visual/feel acceptance and deeper late-system exact semantics, not the smoke-blocking item identity/metadata issues closed here.

> Latest late-dialog command/readiness sync: 2026-05-08 completed. The Player Web System Menu now has real command-backed Hero and Item Rental panels, and the previously static Creature/Mount/Fishing controls now dispatch typed Gateway browser commands. The smoke harness records recent command history so automatic tick traffic cannot hide button commands, then verifies Creature summon, Mount use, Fishing cast/autocast, Hero create, ItemRental request, and the expanded social late-system panels. Simulation snapshots now include ItemRental active/record state from the runtime resource, allowing the UI to observe partner/fee/period/deposit/lock/rented-record readiness. Verification passed Web script syntax/typecheck, live local Gateway/Web fast Stage 5 smoke with 22 screenshots (`systemMenuFeature=10`, `systemMenuSocial=44`), focused Simulation item-rental tests 3/3, locked Simulation/Gateway check, and Gateway command mapping tests 7/7. This moves Hero/ItemRental/Creature/Mount/Fishing frontend reachability from open implementation gap to verified Candidate surface; remaining work is exact Crystal dialog pixel/feel acceptance and deeper production late-system semantics.

> Latest frontend 2/4/5/6 closure sync: 2026-05-07 completed. The Player Web Candidate surface now closes the automatable frontend gaps called out for combat/skills/buffs/projectiles, late-system windows/interactions, NPC/quest path validation, and compact/text responsiveness. Live Crystal combat packets now drive magic, projectile, buff, map-effect, cooldown, and Crystal-like action timing state in the browser; late System Menu panels include state-backed trade/market/marriage/social surfaces plus a real trade chat filter; the NPC/quest route opens InnKeeper_Brittney through Crystal dialog links without storage QA fallback, strips raw script markup, and verifies Quest Diary detail rows; compact smoke now runs a three-viewport matrix with repo-stable screenshot output. Verification passed Web `npx tsc --noEmit`, smoke script syntax, and a full live isolated-Gateway Stage 5 UI smoke capturing 113 screenshots with `criticalConsoleErrorCount=0`, `compactMatrixCount=3`, `systemMenuSocial=36`, `systemMenuFeature=6`, `storagePassword=9`, `npcDialogFlow=11`, and `combatFlow=2`. Remaining work is no longer these frontend automation holes; it is human Crystal visual/feel acceptance plus deeper full per-skill bitmap/effect fidelity.

> Latest full Stage 5 UI smoke stabilization sync: 2026-05-07 completed. The frontend/runtime smoke path now disables automatic keep-alive/tick flooding with `autoTick=0`, records WebSocket frame diagnostics on timeout, opens a real Crystal NPC dialog through deterministic `qa.openNpcDialog`, and uses Crystal-backed `BugBat` event spawns placed on nearby spawnable tiles for combat verification. Runtime `event.spawn` now defaults to a Crystal-resolvable monster and searches the current map collision data before spawning, so Crystal map positions such as `crystal:0:330:270` produce visible monsters instead of silent zero-spawn logs. Full Stage 5 UI smoke captured 102 screenshots with `criticalConsoleErrorCount=0`, including service-backed storage password/store/take-back, equipped repair/special repair, GameShop Gold buy, belt keyboard/mouse use, NPC dialog, and combat flows. Verification passed Node script syntax, Web `npx tsc --noEmit`, Rust fmt/check for Simulation/Gateway, focused drop/QA/NPC/event/gateway regressions, shared in-process registry 11/11, full locked Gateway 107/107 plus packet-trace bin 17/17, full locked Simulation 731/731, and the live isolated-Gateway Stage 5 UI smoke.

> Latest frontend/runtime service-backed UI sync: 2026-05-07 completed. Player Web can now run Stage 5 UI smoke against an isolated Gateway via `?gatewayWs=` / `NEXT_PUBLIC_MIR2_GATEWAY_WS_URL`, and the smoke now verifies live Crystal service flows instead of no-service placeholders for storage, equipped-item repair, and GameShop Gold buys. Runtime repair accepts equipped Crystal slot unique IDs for `RepairItem` / `SRepairItem`, preserves normal max-durability loss vs special-repair max preservation, and deterministic smoke setup can damage equipped gear through `qa.damageEquipment`. Web fixes the Belt/Boots equipment slot id mapping, avoids nested Mail row buttons that caused hydration errors, and verifies Gold-funded `AccuracyPotion` purchase delivery to carry slots. Full Stage 5 UI smoke captured 101 screenshots with `criticalConsoleErrorCount=0`, including `characterRepairFlow=8`, `gameShopFlow=4`, `storageStoreFlow=4`, `storageTakeBackFlow=4`, and `beltUseFlow=7`. Verification passed focused Simulation repair/damage regressions, Rust fmt/check for Simulation/Gateway, Web `npx tsc --noEmit`, smoke script syntax, `git diff --check`, and the live isolated-Gateway Stage 5 UI smoke.

> Latest typed-observability sync: 2026-05-07 completed. After the full server-packet typed pass, Gateway/Web no longer collapses newly typed packet families into Debug-only payload summaries: fallback packet events now serialize typed fields for map/ranking/guild/status and other newly covered Crystal packets, while packet traces use display names derived from typed server IDs instead of reporting those IDs as `Raw`. Game-data acceptance tests now hard-fail if the generated NPC command summary regresses from `81/81` command names / `7,044/7,044` occurrences or if the monster AI summary reintroduces spawned runtime priorities. Verification passed fmt/diff/check plus full locked GameData/Gateway/Protocol/Simulation tests: GameData 27/27, Gateway lib 105/105 plus packet-trace bin 17/17, Protocol lib 33/33 plus codec 33/33, and Simulation 722/722. This closes the automatable observability residue after packet typing; full-project 100% Accepted still depends on semantic gameplay tuning plus human Crystal client visual/dialog/feel acceptance.

> Latest full server-packet typed sync: 2026-05-07 completed. The protocol layer no longer has known Crystal server packets that fall through to Raw decode: all `ServerPacketId` values `0..278` now have explicit typed decode/encode coverage, with the final 58 payload families added for map/world-map/search/user-slot refresh, player inspect/update/status/death/map-change, guild notice/member/status/storage/war, auto-pot, NPC image/input/pearl goods, quest inventory, reincarnation, dash/attack-move/concentration/elemental, awakening materials, transform, game-shop stock, ranking, notice, and guild territory pages. A local Crystal packet scan reports `explicit=279 remaining=0`. Verification passed protocol round trips, Gateway/Web compile surfaces, and full locked Protocol/Gateway/Simulation tests: Gateway lib 104/104 plus packet-trace bin 17/17, Protocol lib 32/32 plus codec 33/33, and Simulation 722/722. This moves the remaining server protocol gap from “Raw payload coverage” to “exact runtime/client behavior behind fully typed packets”; full-project 100% Accepted still needs human Crystal client visual/dialog/feel acceptance and deeper gameplay tuning.

> Latest P1/P2 packet-runtime sync: 2026-05-07 completed. The next parity slice now has tested typed packet and modeled gameplay coverage for Group utility responses, Quest change/complete/share responses, Refine deposit/retrieve/cancel/start/check responses, stateful group packet updates, quest accept/finish/abandon/share, Stage 5 market consign/buy/get-back/sell-now flows, refine slot/current-item/check state, `OpenDoor`, and manifest-backed map/monster/NPC info requests. Gateway Web and packet trace expose these packet names/events, and the player System Menu social panels now show group/guild/mentor/ranking surfaces without visible Web placeholder wording. Verification passed focused regressions, Rust fmt/check, Web typecheck, fast live Stage 5 UI smoke, and full locked Protocol/Gateway/Simulation tests: Gateway lib 103/103 plus packet-trace bin 17/17, Protocol lib 29/29 plus codec 32/32, and Simulation 722/722. This moves another P1/P2 block from “missing/empty packet surfaces” to “modeled and regression-protected”; exact Crystal market listing pages, refine economics/timers, bid/commission settlement, Quest Diary visual acceptance, and final human feel remain open.

> Latest P1/P2 exact-gate sync: 2026-05-07 completed. The multi-agent closure was reconciled into tested source changes: Raw/known-raw server payloads now have copyable hex/detail fields in Gateway Web and `packet_trace`; IntelligentCreature pickup now follows Crystal default pet rules, mode gates, category/grade filters, fullness pickup blocking, and blackstone progress behavior; Fishing now enforces Crystal rod/bait/hook/reel/fishing-cell/durability gates while preserving reel loot and autocast; Mount ride toggling now honors `NoMount`, `NeedBridle`, saddle, and reins state, and the Crystal respawn manifest path now preserves `NoMount` / `NeedBridle`; System Menu creature/mount/fishing panels no longer display Web placeholder text. The Web original-scene sprite loader also now checks the generated asset manifest before fetching, so unexported Crystal libraries such as `NPC/09` degrade quietly instead of producing browser 404s. Verification covered focused Protocol/Gateway/Simulation regressions, Rust fmt/check/diff gates, Web typecheck, Node script checks, live Stage 5 UI smoke with 83 screenshots and 0 critical console errors, and full locked package tests: GameData 27/27, Gateway lib 100/100 plus packet-trace bin 17/17, Protocol lib 26/26 plus codec 32/32, and Simulation 716/716. This moves the current P1/P2 backlog from “missing backend gates” to “human visual/client acceptance plus deeper exact tuning”: final Crystal dialog review, fishing slot-stat math, hero combat/equipment AI, and remaining production-grade late-system edge cases.

> Latest multi-agent gameplay closure sync: 2026-05-07 completed. The coordinator used separate Simulation, Gateway/Web, and verification agents, then reconciled the result locally. The late-system gameplay slice now has tested modeled flows for shared two-account Trade commit/rollback, IntelligentCreature automatic pickup/fullness/blackstone ticking, Fishing find/reel/autocast loot, equipped Mount ride toggling, Hero create/change/behaviour state, and Gateway BrowserCommand/packet-trace coverage for those paths. Verification passed with locked Protocol/Simulation/Gateway fmt/check and full package tests: Gateway lib 99/99 plus packet-trace bin 16/16, Protocol lib 25/25 plus codec 32/32, and Simulation 711/711. This closes the concrete backend gaps the previous audit named for Trade delivery/rollback and IntelligentCreature fullness/blackstone/automatic pickup; whole-project 100% Accepted still depends on human Crystal client visual/dialog/feel acceptance and exact late-system tuning beyond the modeled backend surface.

> Latest IntelligentCreature stateful protocol sync: 2026-05-06 completed. The pet/intelligent-creature packet family now has a real Stage 5 state model behind the Crystal packets. `UpdateIntelligentCreature` registers and updates `ClientIntelligentCreature` rows, honors summon/unsummon/release flags, emits `NewIntelligentCreature` for new creatures, and returns a non-empty `UpdateIntelligentCreatureList`; `RequestIntelligentCreatureUpdates` reads the stored list; active creatures can now execute `IntelligentCreaturePickup` against a targeted ground drop and deliver the same gain packets as player pickup. Verification passed with focused creature update/pickup coverage, locked three-package fmt/check, and full Protocol/Simulation/Gateway validation: Gateway lib 96/96 plus packet-trace bin 16/16, Protocol lib 25/25 plus codec 32/32, and Simulation 708/708. This moves IntelligentCreature past empty shell behavior; remaining depth is fullness/feeding, blackstone timers, automatic pickup rules/filters, visible pet movement, and final Crystal dialog acceptance.

> Latest Trade stateful protocol sync: 2026-05-06 completed. The late-system Trade slice now has a packet-backed Stage 5 session instead of only empty/no-partner behavior. Gateway shared in-process sessions can resolve an adjacent player for `TradeRequest`, and Simulation now drives `TradeRequest`/`TradeReply`/`TradeGold`/`DepositTradeItem`/`RetrieveTradeItem`/`TradeConfirm`/`TradeCancel` through persistent trade state: partner name, offered gold, trade slots, item echo payloads, lock/accept/complete flags, gold deduction, offered item removal, and cancellation cleanup. Verification passed with focused Simulation trade packet tests, existing Stage 5 trade command regressions, a Gateway adjacent-player trade request regression, locked three-package fmt/check, and full Protocol/Simulation/Gateway validation: Gateway lib 96/96 plus packet-trace bin 16/16, Protocol lib 25/25 plus codec 32/32, and Simulation 708/708. This closes the current single-offer Trade packet loop; remaining Trade parity is partner-side delivery/rollback, disconnect race semantics, and final Crystal dialog acceptance.

> Latest Mail/Friend stateful protocol sync: 2026-05-06 completed. The packet-backed Mail/Friend slice now has real state behind the Crystal surfaces instead of only bounded empty responses. `SendMail` validates recipients/cost/gold, deducts through `LoseGold`, creates a Stage 5 mail row, returns `MailSent`, and immediately exposes the delivery through `ReceiveMail`; `ReadMail`, `CollectParcel`, `DeleteMail`, `LockMail`, and `MailCost` now reflect mailbox state, parcel gold transfer, deletion filtering, and Crystal-shaped failure acks. `AddFriend`, `RemoveFriend`, `RefreshFriends`, and `AddMemo` now update/read persisted Stage 5 social lists and return `FriendUpdate` with `ClientFriend` payloads, including memo state. Verification passed with focused and adjacent mail/social tests, locked three-package fmt/check, and full Protocol/Simulation/Gateway validation: Gateway lib 95/95 plus packet-trace bin 16/16, Protocol lib 25/25 plus codec 32/32, and Simulation 707/707. This advances Mail/Friend beyond Stage 5 shell behavior; remaining depth is exact live item-attachment semantics, persistent lock/reply details, online delivery fanout, and final Crystal dialog acceptance.

> Latest full-protocol coverage sync: 2026-05-06 completed. The protocol gap called out by the truth audit is now closed at the packet-ID level: `ClientPacketId` covers Crystal client IDs `0..152` and typed `ClientPacket` covers all 153 client packets, while `ServerPacketId` covers Crystal server IDs `0..278` with typed variants for the implemented semantics and Raw-safe fallback for known complex server payloads that still need deeper modeling. Critical Crystal ID corrections are now tested: client `CombineItem=110`, `AwakeningNeedMaterials=111`, server `CombineItem=214`, and `ItemUpgraded=215`. The typed server surface now includes the extra magic/visual/late packet families for projectile, range attack, push, map effect, observe, pause buff, hidden state, dash/fail dash, delayed explosion removal, deco/sneak/level effects, binding shot, output message, awakening NPC dialogs/results/locked item, and `ResizeInventory`; Gateway Web event JSON and packet trace naming expose those variants. Verification passed with focused round-trip and Raw-fallback regressions plus full three-package validation: `cargo +1.89.0 fmt --check -p mir2-protocol -p mir2-simulation -p mir2-gateway`, `cargo +1.89.0 check --locked -p mir2-protocol -p mir2-simulation -p mir2-gateway`, and `CARGO_CACHE_AUTO_CLEAN_FREQUENCY=never cargo +1.89.0 test --locked -p mir2-protocol -p mir2-simulation -p mir2-gateway -- --test-threads=1` covering Gateway lib 95/95 plus packet-trace bin 16/16, Protocol lib 25/25 plus codec 32/32, and Simulation 707/707. This removes missing packet IDs as a class of risk; remaining Crystal 1:1 work is deeper gameplay/state/UI acceptance behind some packets, including complex guild/market/ranking/hero-info payloads, multi-player transaction semantics, full per-spell fidelity, and human visual/feel acceptance.

> Latest late-system packet parity sync: 2026-05-06 completed. The Rust protocol/runtime/gateway surface now covers the newly audited Crystal late-system packet families that were still mostly Raw or Stage 5-only: Trade (`DepositTradeItem`, `RetrieveTradeItem`, `TradeRequest`, `TradeReply`, `TradeGold`, `TradeConfirm`, `TradeCancel` plus server trade responses/items), Fishing/Mount (`FishingCast`, `FishingChangeAutocast`, `MountUpdate`, `FishingUpdate`), Mail/Friend (`SendMail`, `ReadMail`, `CollectParcel`, `DeleteMail`, `LockMail`, `MailLockedItem`, `MailCost`, `AddFriend`, `RemoveFriend`, `RefreshFriends`, `AddMemo`, `ReceiveMail`, `MailSent`, `ParcelCollected`, `FriendUpdate`), and IntelligentCreature (`UpdateIntelligentCreature`, `IntelligentCreaturePickup`, `RequestIntelligentCreatureUpdates`, `NewIntelligentCreature`, `UpdateIntelligentCreatureList`, `IntelligentCreatureEnableRename`). Protocol now has Crystal packet IDs, payload structs, trace names, and round-trip regressions for these families; Gateway Web can drive the new client commands and serialize the server events; Simulation preserves Crystal-safe empty/no-partner/failure surfaces where the deeper persistent gameplay model is still pending. Verification passed: `cargo +1.89.0 fmt --check -p mir2-protocol -p mir2-simulation -p mir2-gateway`, locked `check` for the same packages, focused package regressions for each new packet family, and full `cargo +1.89.0 test --locked -p mir2-protocol -p mir2-simulation -p mir2-gateway -- --test-threads=1` with Gateway lib 91/91 plus packet-trace bin 16/16, Protocol lib 15/15 plus codec 32/32, and Simulation 705/705. Remaining work is the actual persistent gameplay depth behind those now-typed surfaces: two-player trade transaction close, real mailbox/friend lists, pet lifecycle and pickup behavior, full fishing rod/bait/drop/durability flow, mount equip/ride progression, and final client-dialog acceptance.

> Latest item-rental parity sync: 2026-05-06 completed. The Rust protocol/runtime now covers the Crystal item-rental packet family instead of leaving `ItemRental` as an untyped late-system gap: client packet IDs `GetRentedItems=137` through `ConfirmItemRental=146` and server packet IDs `GetRentedItems=254` through `ConfirmItemRental=265` decode/encode with Crystal-shaped payloads, including `ItemRentalInformation` and rental metadata on loaned `UserItem`s. Simulation handles the request/fee/period/deposit/retrieve/cancel/lock/confirm flow, applies Crystal rental binding restrictions, returns locked items and fees on cancel, records confirmed lender-side rental entries, persists rental records across save/reload, and exposes the same surfaces through Gateway Web JSON and packet traces. Shared in-process Gateway sessions can now resolve an adjacent remote player for the Crystal `ItemRentalRequest` handshake. Verification passed for focused protocol/simulation/gateway rental regressions and full package tests: Gateway lib 83/83 plus packet-trace bin 16/16, Protocol lib 7/7 plus codec 32/32, and Simulation 701/701, along with locked check/fmt across Protocol/Simulation/Gateway. Remaining rental work is the deeper cross-account borrower inventory transfer/expiry-return/mail/death-return behavior and final front-end dialog acceptance.

> Latest gameplay magic/buff parity sync: 2026-05-06 completed. The Rust protocol/runtime now covers the Crystal magic and buff packet family instead of relying only on Web high-level `castSkill`: client packet IDs `MagicKey=57`, `Magic=58`, and `SpellToggle=69` decode/encode; server packet IDs `NewMagic=117` through `ObjectMagic=123`, `SpellToggle=138`, `ObjectMana=140`, `AddBuff=144`, and `RemoveBuff=145` decode/encode; and the `Spell`, `ClientMagic`, `ClientBuff`, and `ObjectMana` payload shapes are available to Gateway/Web. Simulation handles real `Magic` packets with Crystal-shaped `UserLocation` fallback, successful MP/magic/buff response packets, `MagicKey` hotkey assignment, `SpellToggle` ack, book-learning `NewMagic`, timed potion `ObjectMana`, expired-buff `RemoveBuff`, MagicShield/Fury-style `AddBuff`, deterministic `MagicLeveled`/`MagicDelay`, Teleport movement, and target-damage scheduling for manifest-backed spells whose Crystal data exists before full bespoke gameplay fidelity is implemented. Gateway Web and packet trace expose matching packet commands/events for admin/session QA. Verification passed for protocol round trips, focused simulation magic/buff regressions, focused gateway web regressions, packet-trace flow-name coverage, Player Web `npx tsc --noEmit`, locked check across Protocol/Simulation/Gateway, fmt, `git diff --check`, and full Protocol/Simulation/Gateway package tests: Gateway lib 82/82 plus packet-trace bin 16/16, Protocol lib 5/5 plus codec 32/32, and Simulation 698/698. This closes the current magic/buff protocol-runtime parity slice; exact per-spell combat/AI edge cases remain tracked under the broader Crystal skill/monster behavior queue.

> Latest admin-console parity sync: 2026-05-06 completed. The existing Admin backend now covers the Crystal server engine-console operating surface instead of requiring the legacy WinForms console for routine GM/server operations. Admin API adds a generic audited `/admin/commands/console` execution path with RBAC and approval gates for account create/update/delete, unban and storage-password clear, character rename/stat/currency/location/vital/PK edits, chat ban apply/clear, safe-zone return, kill player, kill pets, NPC flag set/clear, direct GM message, world broadcast, market listing cancel/expire/delete, guild member/message moderation, NameLists create/add/remove/delete, content override bundle publish, and server control. Gateway adds `/admin/sessions` and `/admin/control`, while Admin Web adds Console, Accounts, Market, Guilds, NameLists, Content, and player-detail editor/flag/chat-ban pages. Runtime persistence now stores Crystal PK/chat-ban fields, normal chat honors active bans, and Stage 5 auctions persist Crystal-style expired listings. Verification passed: Rust fmt/check locked for Simulation/Admin API/Gateway, full `mir2-simulation` 692/692, full `mir2-admin-api` tests, focused Gateway admin endpoint test, Admin Web typecheck/build, live HTTP mutation/readback smoke for the new console operations, SSR page probes, and Playwright page snapshots confirming Market, NameLists, and player-detail controls/readback. This closes the Admin-console functional parity slice and leaves human frontend acceptance as the next review gate.

> Latest runtime/frontend comparison sync: 2026-05-01-R327 completed. Web now covers the user-requested Gameshop Buy and map-click arrival paths. Gameshop product cells route Buy to backend `gameShop.buyCredit` / `gameShop.buyGold` commands using the generated Crystal game-shop manifest, and the runtime delivers credit purchases through Stage 5 mail after deducting currency. Browser evidence for `QA0429A / QA0429Hero` records `AccuracyPotion`, command `gameShop.buyCredit(20,1)`, expected zero-credit rejection, `network404Count=0`, and `consoleErrorCount=0`; focused simulation coverage verifies the positive credit/mail path. The map-click movement loop now waits for pending self movement confirmation, reconciles player `ObjectRun` / `ObjectWalk` packets immediately, and stops the movement-time 180ms tick flood that delayed queued move commands behind monster updates. Evidence `docs/generated/player-qa/movement-jitter/r327-map-click-target-arrival-fixed3.json` reaches `338,270` with `movementPlan=null` and `jumps=[]`. Verification passed: web `tsc --noEmit`, capture-script syntax checks, focused game-shop simulation test, `mir2-gateway` check, and targeted CDP captures. R327 also exports `NPC/25` to remove the prior same-scene resource 404.

> Latest runtime/frontend comparison sync: 2026-04-30-R319 completed. Web now tightens the latest same-scene visual gaps for object labels, BigMap NPC rows, Mail empty state, and mouse cursors. Entity labels keep Crystal object-centered behavior with stacked underscore names and no Web selected-target helper lines inside the nameplate; BigMap NPC rows are sourced from the Crystal whole-map NPC manifest, use `MapLinkIcon` assets, and format text like `(Teleport)Gilbert`; empty Mail no longer shows Web `No mail`; and Crystal `.CUR` files are applied for default, NPC, monster/attack, and text cursors. Evidence: `docs/generated/player-qa/r319-label-bigmap-mail-cursor/r319-label-bigmap-mail-cursor-final.png` and `docs/generated/player-qa/r319-label-bigmap-mail-cursor/r319-label-bigmap-mail-cursor-final-state.json`; state records `mailPanel.emptyVisible=false`, `bigMap.npcRowCount=18`, first NPC rows with `MapLinkIcon` paths, stage/NPC/monster cursor URLs using original cursor files, `nonFaviconNetwork404s=[]`, and `consoleErrors=[]`. Verification passed: UI asset export, web `tsc --noEmit`, capture script syntax check, and focused CDP capture with `--openMail true --openBigMap true`.

> Latest runtime/frontend comparison sync: 2026-04-30-R318 completed. BigMap and MailList now use original Crystal dialog shells instead of Web-style UI. Web persists `MapInformation.bigMapIndex`, the minimap BigMap button opens the exported `Title/820` `BigMapDialog` with Crystal controls, raster viewport, coordinate label, NPC list, and radar dots, and the Mail button opens the exported `Title/670` `MailListDialog` at the original `562,5,312,444` position with `Title/7`, close/help/page/action buttons, and 10-row layout. Evidence: `docs/generated/player-qa/r318-mail-bigmap/r318-mail-bigmap-final.png` and `docs/generated/player-qa/r318-mail-bigmap/r318-mail-bigmap-final-state.json`; state records `mailPanel.bounds=562,5,312,444`, `mailPanel.hasFrame=true`, `mailPanel.visibleOverlayHead=false`, `bigMap.bounds=132,134,760,500`, `bigMap.viewport=146,186,568,380`, `bigMap.hasRaster=true`, `bigMap.coordinate="[ 287, 618 ]"`, `nonFaviconNetwork404s=[]`, and `consoleErrors=[]`. Verification passed: UI asset export, web `tsc --noEmit`, capture/smoke script syntax checks, focused CDP capture with `--openMail true --openBigMap true`, and `git diff --check`. Remaining map/mail work is deeper interaction parity and final human visual acceptance.

> Latest runtime/frontend comparison sync: 2026-04-30-R317 completed. The Gameshop dialog interior now uses Crystal product data and exported original assets instead of Web placeholder cells. Web renders the 105 generated Crystal Gameshop products with original `Title/750` cell frames, `Title/778-783` buy/preview buttons, item icons exported from `Items.Lib`, Crystal cell label coordinates, category/class/search filtering, page controls, stock/count/price labels, and gold/credit payment state. Evidence: `docs/generated/player-qa/r317-gameshop-products/r317-gameshop-products.png` and `docs/generated/player-qa/r317-gameshop-products/r317-gameshop-products-state.json`; state records `gameShop.bounds=164,70,696,476`, `cellCount=8`, `firstCellName="AccuracyPotion"`, `pageLabel="1 / 14"`, `categoryCount=10`, `loadedIconCount=8`, `buyButtonCount=8`, `previewButtonCount=1`, `oldPlaceholderCellCount=0`, `inventoryVisible=false`, `nonFaviconNetwork404s=[]`, and `consoleErrors=[]`. Verification passed: web `tsc --noEmit`, capture-script `node --check`, focused CDP capture with `--openGameShop true`, UI asset export, and `git diff --check`. Remaining Gameshop work is service-backed buy/preview behavior and final human visual acceptance.

> Latest runtime/frontend comparison sync: 2026-04-30-R316 completed. The latest Gameshop/Menu screenshots exposed two Web UI parity bugs: the HUD Gameshop button opened Inventory/Quest because it was wired to `onOpenInventoryTab("quest")`, and the HUD Menu button opened a Web QA/debug transfer panel instead of Crystal's narrow icon `MenuDialog`. Web now follows Crystal source behavior: Gameshop toggles a Crystal-framed `GameShopDialog` shell, while Menu renders the exported `Title/567` 36x282 icon strip with 13 original sprite buttons at Crystal offsets. The QA transfer form remains available for automation but is offscreen and no longer appears as normal player UI. Evidence: `docs/generated/player-qa/r316-gameshop-menu/r316-gameshop-open.png`, `docs/generated/player-qa/r316-gameshop-menu/r316-menu-open.png`, and `docs/generated/player-qa/r316-gameshop-menu/r316-gameshop-menu-state.json`; state records `shopVisible=true`, `inventoryVisible=false`, `shopBounds=164,70,696,476`, `menuBounds=988,349,36,282`, `iconCount=13`, `oldOverlayHeadVisible=false`, `nonFaviconNetwork404s=[]`, and `consoleErrors=[]`. Verification passed: web `tsc --noEmit`, capture-script `node --check`, focused CDP click capture, and `git diff --check`. Full Gameshop product data/buy interaction remains open.

> Latest runtime/frontend comparison sync: 2026-04-30-R315 completed. The latest panel screenshots exposed Web demo seed state in the Crystal QA character: starter items/equipment/skills/quest/storage/gold were being injected where the original new character is empty. Runtime now makes real `NewCharacter` saves Crystal-empty for bag, belt, storage, equipment, quests, skills, and gold; empty save arrays load as explicit empty; old level-1 saves that exactly match the former Web seed set migrate to empty; and `demo/Scout` keeps its Stage 5 seed state for automation. Frontend no longer fills empty spell rows with Web hints/buffs and removed the web-only Character repair buttons. Evidence for `QA0429A / QA0429Hero` at `BichonProvince` map `0`, `287,618` records `gold=0`, all panel-state counts at 0 (`inventoryItemCount`, `beltItemCount`, `storageItemCount`, `equipmentItemCount`, `questCount`, `skillCount`), `playerHp=18`, `playerMaxHp=18`, `playerMp=14`, `hudHealthOnlyLabel="HP 18/18"`, `nonFaviconNetwork404s=[]`, and `consoleErrors=[]`. Screenshot/state: `docs/generated/player-qa/r315-empty-new-character-panels/r315-empty-new-character-panels.png` and `docs/generated/player-qa/r315-empty-new-character-panels/r315-empty-new-character-panels-state.json`. Verification passed: focused `mir2-simulation start_game_` 16/16, `cargo +1.89.0 build --locked -p mir2-gateway`, web `tsc --noEmit`, R315 capture, `cargo +1.89.0 fmt --check`, and capture-script `node --check`. This closes the incorrect seeded content mismatch; exact Quest Diary/Storage dialog bitmaps and paperdoll base rendering remain open visual work.

> Latest runtime/frontend comparison sync: 2026-04-30-R314 completed. The aligned Bichon Web HUD now follows the Crystal low-level Warrior HP-only path: Crystal `BaseStats` formulas drive default and migrated legacy save vitals, `QA0429Hero` level 1 records `playerHp=18`, `playerMaxHp=18`, `playerMp=14`, and the HUD label is `HP 18/18` using exported `Prguse` frame 6. R314 also keeps the R311 bitmap orb fill, applies the Crystal 4-line chat feed sizing/colors, and layers the belt bar from `Prguse` 1932 with the 0.5-opacity 1933 overlay. Evidence: `docs/generated/player-qa/r314-crystal-vitals-hud/r314-bichon-287-618-vitals-hud.png` and `docs/generated/player-qa/r314-crystal-vitals-hud/r314-bichon-287-618-vitals-hud-state.json`; the state records exact 1024x768 stage/HUD/minimap/chat bounds, `visibleChatLines` count 4, `nonFaviconNetwork404s=[]`, and `consoleErrors=[]`. Verification passed: focused `mir2-simulation start_game_` 15/15, `cargo +1.89.0 build --locked -p mir2-gateway`, web `tsc --noEmit`, R314 capture, `cargo +1.89.0 fmt --check`, and `git diff --check`. This closes the specific blood-bar/font/hotbar value mismatch raised in the latest screenshot pass; exact effects/lighting, dynamic placement, and human visual acceptance remain open.

> Latest runtime/frontend comparison sync: 2026-04-30-R312 completed. The aligned Bichon Web projection now follows Crystal source anchors for the next visible parity pass: `MapControl.OffSetY` is restored to `Settings.ScreenHeight / 2 / CellHeight - 1`, floor/object map layers keep the source map-layer `drawX` offset path, and entity sprites/nameplates/health bars use Crystal `DrawLocation` / `DisplayRectangle` placement instead of tile-center/self-nameplate web offsets. Evidence for `QA0429A / QA0429Hero` at `BichonProvince` map `0`, `287,618` records self nameplate `top=275`, exact `1024x768` stage and `0,616,1024,768` HUD bounds, `questMarkerCount=0`, `nonFaviconNetwork404s=[]`, and `consoleErrors=[]`. Screenshot/state: `docs/generated/player-qa/r312-entity-crystal-anchor/r312-bichon-287-618-entity-anchor.png` and `docs/generated/player-qa/r312-entity-crystal-anchor/r312-bichon-287-618-entity-anchor-state.json`. R311's Crystal bitmap HP/MP orb fill remains active; R312 supersedes the R311 playfield-centered camera experiment for projection math.

> Latest runtime/frontend comparison sync: 2026-04-30-R311 completed. The aligned Bichon Web viewport now uses Crystal playfield-centered camera math above the fixed 152px HUD instead of centering on the whole 1024x768 stage. Same-scene evidence for `QA0429A / QA0429Hero` at `BichonProvince` map `0`, `287,618` records the self nameplate at `top=325` instead of the earlier R310 web `top=389`, exact `1024x768` stage and `0,616,1024,768` HUD bounds, `questMarkerCount=0`, `nonFaviconNetwork404s=[]`, and `consoleErrors=[]`. R311 also switches the HP/MP HUD orb fill from CSS gradients to exported Crystal `Prguse` frame 4 bitmap slices and adds `Prguse` frames 4/6 to the UI export manifest. Evidence: `docs/generated/player-qa/r311-playfield-camera/r311-bichon-287-618-playfield-camera.png`, `docs/generated/player-qa/r311-playfield-camera/r311-bichon-287-618-hud-orb.png`, and `docs/generated/player-qa/r311-playfield-camera/r311-bichon-287-618-hud-orb-state.json`. This improves the user-reported original-vs-Web mismatch but remains Candidate visual evidence, not final human visual acceptance.

> Latest runtime/frontend comparison sync: 2026-04-29-R310 completed/monitoring. The Web client no longer lets the login-success transition overlay cover the game scene: same-scene evidence records `screen=game` with `transitionOverlayVisible=false`. NPC quest markers are now scoped by server-provided `questIds`, removing the previous all-NPC marker noise for the current Bichon comparison. R310 also adds repeatable same-scene capture tooling (`apps/web/scripts/capture-crystal-parity.mjs`) and a six-hour-capable original/Web sampler (`apps/web/scripts/r310-visual-watch.ps1`). Evidence: `docs/generated/player-qa/r310-visual-watch/r310-final-web-scene-state.json` and `docs/generated/player-qa/r310-visual-watch/r310-final-web-scene.png`; one-sample watch evidence includes `watch-20260429-042013-original.png`, `watch-20260429-042013-web.png`, and `r310-visual-watch-log.jsonl`. This is Candidate automation evidence, not final human visual acceptance.

> Latest runtime/frontend comparison sync: 2026-04-29-R309 completed. The aligned Bichon minimap/HUD boundary now stays inside the exact 1024x768 Crystal-size stage. `.mini-map-panel` moved from `right=-2px` to `right=0`; evidence at `docs/generated/player-qa/r309-minimap-bounds-web-page-state.json` records desktop minimap bounds `left=896`, `right=1024`, `width=128`, `desktopOverflows=[]`, compact `820x640` `compactOverflows=[]`, `nonFaviconNetwork404s=[]`, and `consoleErrors=[]`. Screenshots: `docs/generated/player-qa/r309-minimap-bounds-web-page.png` and `docs/generated/player-qa/r309-minimap-bounds-compact-web-page.png`. This closes the measured minimap 2px overflow but remains Candidate evidence, not final visual 1:1 acceptance.

> Latest runtime/frontend comparison sync: 2026-04-29-R308 completed. The Bichon browser comparison now uses an exact 1024x768 Crystal-size stage at original comparison viewports instead of the previous web-only 0.9 downscale, with black outer background and no frame shadow. Compact evidence keeps the stage inside `820x640` at `798.72x599.04`. R308 also exports missing visible-object sprite libraries from the Crystal client (`NPC/00`, `NPC/01`, `NPC/03`, `NPC/11`, `NPC/15`, `Monster/003`, `Monster/004`, `Monster/005`), eliminating non-favicon sprite 404s in the aligned Bichon view. Evidence: `docs/generated/player-qa/r308-stage-scale-web-page-state.json`, `docs/generated/player-qa/r308-stage-scale-web-page.png`, and `docs/generated/player-qa/r308-stage-scale-compact-web-page.png` for `QA0429A / QA0429Hero` at map `0`, `287,618`: desktop stage bounds are `0,0,1024,768`, `hasGuard=true`, `hasArcherGuard=true`, `questTrackerVisible=false`, `nonFaviconNetwork404s=[]`, and `consoleErrors=[]`. This closes the browser-only scale/frame and Bichon visible sprite 404 gaps but remains Candidate evidence, not final visual 1:1 acceptance.

> Latest runtime/frontend comparison sync: 2026-04-29-R307 completed. The second aligned Bichon comparison point now has explicit ordinary Guard/ArcherGuard coverage. Added a focused `mir2-simulation` regression for `crystal:0:287:618` requiring `Guard` at `291,620` and `ArcherGuard` at `295,624` in both `ObjectMonster` packets and `worldSnapshot`. Browser evidence at `docs/generated/player-qa/r307-bichon-guard-archer-web-page-state.json` records `QA0429A / QA0429Hero` at `BichonProvince` map `0`, `287,618` with `hasGuard=true`, `hasArcherGuard=true`, `monsterCount=7`, `npcCount=5`, `hasUnderscoreNameplate=false`, and `questTrackerVisible=false`; screenshot evidence is `docs/generated/player-qa/r307-bichon-guard-archer-web-page.png`. Verification passed: focused simulation regression and CDP browser capture with zero console errors. This closes ordinary Guard/ArcherGuard visibility evidence for that comparison point but is still Candidate evidence, not final visual 1:1 acceptance.

> Latest runtime/frontend comparison sync: 2026-04-29-R306 completed. The aligned Bichon browser view now removes the default web-only quest tracker overlay from the playfield and normalizes visible NPC/monster nameplates by displaying spaces instead of raw underscore names. Evidence: `docs/generated/player-qa/r306-bichon-display-web-page-state.json` records `QA0429A / QA0429Hero` at `BichonProvince` map `0`, `284,607` with `entityCount=17`, `npcCount=8`, `monsterCount=8`, `npcSpriteElementCount=8`, `monsterSpriteElementCount=8`, `hasUnderscoreNameplate=false`, and `questTrackerVisible=false`; screenshot evidence is `docs/generated/player-qa/r306-bichon-display-web-page.png`. Verification passed: web `tsc --noEmit`, CDP login/start/transfer/browser capture, and zero browser console errors. This closes the display-only name/quest-overlay gap for the Bichon comparison but is still Candidate evidence, not final visual 1:1 acceptance.

> Latest runtime/frontend comparison sync: 2026-04-29-R305 completed. The Bichon same-scene snapshot now includes current-map visible Crystal respawns in ECS/worldSnapshot, not only `ObjectMonster` bootstrap packets. Evidence: `docs/generated/player-qa/r305-bichon-visible-world-snapshot.json` records `QA0429A / QA0429Hero` at `BichonProvince` map `0`, `284,607` with `entityCount=17`, `npcCount=8`, `monsterCount=8`, including `Deer`, `Scarecrow`, `Hen`, and two `Royal_Guard` entities; `docs/generated/player-qa/r305-bichon-visible-web-page.png` and `docs/generated/player-qa/r305-bichon-visible-web-page-state.json` record 8 NPC sprite elements plus 8 monster sprite elements in the browser. Verification passed: focused R305 simulation regression, existing visible-respawn density regression, `cargo +1.89.0 fmt --check`, `cargo +1.89.0 build --locked -p mir2-gateway`, live WS probe, browser state/screenshot capture, gateway health, and web HTTP 200. This closes the first Deer/Royal Guard screenshot gap but is still Candidate evidence, not final visual 1:1 acceptance.

> Latest runtime/frontend comparison sync: 2026-04-29-R304 completed. The Bichon same-scene comparison exposed that saved web characters entering a real Crystal map were not repopulating the ECS world with current-map NPC-info manifest entries, leaving the web snapshot with only the player. `apps/simulation/src/runtime.rs` now rebuilds Crystal current-map world population on saved-character start and transfer, and live WS evidence at `docs/generated/player-qa/r304-bichon-npc-world-snapshot.json` confirms `QA0429A / QA0429Hero` at `BichonProvince` map `0`, `284,607` has `entityCount=9`, `npcCount=8`, including `Assistant_Jane` and `Merchant_Ruben`. Browser evidence at `docs/generated/player-qa/r304-bichon-npc-web-page.png` and `docs/generated/player-qa/r304-bichon-npc-web-page-state.json` records `npcSpriteElementCount=8` with expected visible nameplates. Verification passed: `cargo +1.89.0 fmt --check`, focused R304 NPC regression, adjacent `transfer_map`, `start_game_emits_visible_object_packets`, `world_snapshot_marks_safe_zone_after_start_game`, `cargo +1.89.0 build --locked -p mir2-gateway`, a live WS probe on `127.0.0.1:7110`, and browser state/screenshot capture on `http://127.0.0.1:3002`. This is Candidate/integration evidence, not full visual 1:1 acceptance; deer/guard/monster density, quest panel/HUD differences, NPC display-name normalization, and human screenshot acceptance remain open.

> Historical map-resource audit sync: 2026-04-29-R303 completed. The first `audit:crystal-map-coverage` evidence (`docs/generated/map/r303-crystal-map-coverage.json`) confirmed the current Crystal client source had all 463 manifest map files available and parseable by the frontend map parser, with 0 unsupported map types and 0 parse errors. The 2026-05-16 all-map audit above supersedes R303's then-open source-frame/minimap warnings by classifying Crystal no-draw frames separately and adding gameplay semantic checks.

> Latest original-client comparison sync: 2026-04-28-R302 completed. Windows launched original Crystal `Server.exe`/`Client.exe`, created retained character `R302HeroB`, and archived original select/game screenshots plus web Stage 5 comparison evidence at `docs/generated/player-qa/r302-original-client/summary.json`. R302 adds a packet-trace fixture helper (`MIR2_PACKET_TRACE_KEEP_LIFECYCLE_CHARACTER=1`) for retained live QA characters. The fresh current-live matrix in the R302 pack is diagnostic only (`stableDiffCleanCount=2/9`, `packetParityAccepted=false`) because local and Crystal fixture state was not aligned; it does not supersede R300/R298 accepted packet evidence. Whole-project accepted Crystal 1:1 remains **roughly 90%** until human visual/feel acceptance or explicit accepted differences close.

> Latest frontend/player QA sync: 2026-04-28-R301 completed. Windows refreshed the final automated Candidate acceptance pack after R300 stable-diff packet acceptance. Evidence is archived at `docs/generated/player-qa/r301-summary.json`: packet-trace bin 15/15, web `tsc --noEmit`, web `npm.cmd run build`, map API smoke 18/18 with 0 failures, minimap smoke 0 failures with a historical preview-index warning later closed by the 2026-05-16 map audit, WS load 64/64 ready with 0 errors and keepalive p95 637 ms, Stage 5 UI smoke 88 screenshots with 0 critical console errors and 32 compact text nodes checked without overflow, `mir2-game-data` 27/27, `mir2-gateway` 55/55 plus packet-trace bin 15/15, `mir2-admin-api` 22/22, and `mir2-simulation` 674/674. Temporary gateway/web services were stopped and ports 7000/7110/3002 verified closed. Automated parity evidence remains **100% Candidate**; backend/server tracked slice remains **100% Accepted under stable-diff packet acceptance**; whole-project accepted Crystal 1:1 remains **roughly 90%** until human visual/feel acceptance closes.

> Latest backend parity sync: 2026-04-28-R300 completed. The current tracked backend/server packet matrix is now **100% Accepted under explicit stable-diff packet acceptance**. R298 supplies the live Crystal stable matrix evidence under `docs/generated/packet-traces/r298-live-matrix/latest-matrix.json` with 9/9 local OK, 9/9 Crystal OK, `crystalMissingCount=0`, `stableDiffCleanCount=9`, and `acceptedStableLiveComparisonCount=9`; R299 identifies strict exact dirtiness as Crystal dynamic state; and R300 makes stable acceptance enforceable through `MIR2_PACKET_TRACE_ACCEPT_STABLE_DIFF=1` / `MIR2_PACKET_TRACE_REQUIRE_CRYSTAL=1`. Strict exact diff remains diagnostic until a deterministic Crystal fixture controls volatile state. Automated parity evidence remains **100% Candidate**; backend/server tracked slice is **100% Accepted under stable-diff packet acceptance**; whole-project accepted Crystal 1:1 remains **roughly 90%** until human visual/feel acceptance closes.

> Previous backend parity sync: 2026-04-28-R298 completed. Windows refreshed live Crystal stable packet matrix evidence under `docs/generated/packet-traces/r298-live-matrix/latest-matrix.json`: 9/9 local OK, 9/9 Crystal OK, `crystalMissingCount=0`, `stableDiffCleanCount=9`, and `acceptedStableLiveComparisonCount=9` using Crystal `127.0.0.1:7000`, local gateway `127.0.0.1:7310`, full client resources at `E:\mir2\Crystal\Build\Client\Debug`, and fixture character `Cdx0428030348` index `8`. Strict exact diff remained dirty (`diffDirtyCount=9`, `acceptedLiveComparisonCount=0`), which R300 now treats as diagnostic rather than blocking for the accepted stable-diff packet gate.

> Latest frontend/player QA sync: 2026-04-28-R297 completed. Windows automated evidence now covers full client resources at `E:\mir2\Crystal\Build\Client\Debug`, web build/typecheck, map API smoke 18/18, minimap smoke 0 failures with a historical preview-index warning later closed by the 2026-05-16 map audit, WS load 64/64 ready with 0 errors, and Stage 5 UI smoke 88 screenshots with `criticalConsoleErrorCount=0`. Missing original scene sprite libs used by the smoke were exported from Crystal `Data`, gateway `MapInformation` now carries minimap indices for transfer maps, and concurrent account-store file saves are hardened for Windows load. Rust validation passed `mir2-simulation` 674/674, `mir2-gateway` 55/55 plus packet-trace bin 14/14, `mir2-admin-api` 22/22, `fmt --check`, and `git diff --check`. Automated parity evidence remains **100% Candidate**; R300 later closes the backend/server packet gate under stable-diff acceptance; whole-project accepted Crystal 1:1 remains **roughly 90%** until human visual/feel acceptance closes.

> Previous backend parity sync: 2026-04-28-R292 completed. Windows live Crystal stable packet matrix evidence was recorded under `docs/generated/packet-traces/r292-live-matrix/latest-matrix.json`: 9/9 local OK, 9/9 Crystal OK, `crystalMissingCount=0`, `stableDiffCleanCount=9`, and `acceptedStableLiveComparisonCount=9` using Crystal `127.0.0.1:7000`, local gateway `127.0.0.1:7310`, full client resources at `E:\mir2\Crystal\Build\Client\Debug`, and fixture character `Cdx0428030348` index `8`. Strict exact diff was still not clean (`diffDirtyCount=9`, `acceptedLiveComparisonCount=0`), and human player QA was still open, so automated parity evidence remained **100% Candidate**, backend/server tracked slice remained **99.70% Candidate**, and whole-project accepted Crystal 1:1 remained **roughly 90%**.

> Latest backend parity sync: 2026-04-28-R248 completed. The Windows Crystal server-data import gate is no longer blocked for the current backend slice: `generate-crystal-respawn-manifest.mjs` regenerated manifests from `E:\mir2\Crystal\Build\Server\Debug\Server.MirDB` and `E:\mir2\Crystal\Build\Server\Debug\Envir\Routes`, including real map drop-rule flags. Verification passed: `mir2-game-data` 22/22, focused `mir2-simulation no_drop_monster_map_rule` 2/2, full `mir2-simulation` 670/670, and `mir2-gateway` 55/55 plus packet-trace bin tests 7/7. R300 later closed the backend/server packet gate through explicit stable-diff acceptance; whole-project accepted Crystal 1:1 remains **roughly 90%** until human visual/feel acceptance closes.

> Latest truth-audit sync: 2026-04-28. `docs/PARITY-TRUTH-AUDIT.md` is now the authoritative status split for Accepted vs Candidate vs Fallback vs Blocked. The project remains **100% Candidate** for automated evidence, backend/server tracked slice is **100% Accepted under explicit stable-diff packet acceptance**, and real full-project accepted Crystal 1:1 remains **roughly 90%** until human visual/feel acceptance closes.

> Previous sync: R225 completed. Mac-local Candidate evidence was refreshed and green: web `tsc --noEmit`, direct `next build`, Stage 5 UI smoke (88 screenshots with manifest summary counts), map API smoke 18/18, minimap asset smoke 0 failures with a historical preview-index warning later closed by the 2026-05-16 map audit, WS load 64/64, `mir2-game-data` 22/22, `mir2-gateway` 54/54, `mir2-simulation` 664/664, require-local packet trace matrix 9/9 local artifacts under `docs/generated/packet-traces/r225-matrix`, `cargo +1.89.0 fmt --check`, and `git diff --check`. At R225 time backend/server tracked slice was 99.70%; R300 later closed the packet gate under explicit stable-diff acceptance.

> Previous sync: R224 completed. The project remained **100% Candidate** (not 100% Accepted), and the local packet trace matrix blocker was closed. `apps/gateway/src/bin/packet_trace.rs` was restored; `--list-flows` worked; `mir2-gateway` passed 53/53 including packet trace bin tests 6/6; require-local `packet_trace --matrix` wrote 9/9 TCP-traceable artifacts with `localOk=true` under `docs/generated/packet-traces/r224-matrix`. Later Windows rounds closed the server-data import gate and accepted the stable-diff live packet gate.

> Latest sync: R219-R222 completed. Frontend/global evidence advanced across login language/View Key/Enter submit, select language/Credits/Delete cancel/New Character/Delete confirmed/recreate/Start lifecycle, archived map API and minimap asset smoke JSON, refreshed WS load, compact inventory/storage/character/system-menu/chat-settings panel bounds, system-menu compact overflow fix, and NPC dialog link rendering support. Stage 5 UI smoke now captures 85 screenshots. Validation: web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke (85 screenshots), map API smoke (18/18 requests), minimap asset smoke (0 failures, a historical preview-index warning later closed by the 2026-05-16 map audit), WS load 64/64, `cargo +1.89.0 fmt --check`, and `git diff --check`. Active backend/global round is R223; backend/server parity estimate is 99.70%, whole-project 1:1 estimate is 90.0%.


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
Backend/server tracked-slice status: 100% Accepted under explicit stable-diff packet acceptance
Active backend/global round: R309
Completed backend rounds: R82, R83, R84, R85, R86, R87, R88, R89, R90, R91, R92, R93, R94, R95, R96, R97, R98, R99, R100, R101, R102, R103, R104, R105, R106, R107, R108, R109, R110, R111, R112, R113, R114, R115, R116, R117, R118, R119, R120, R121, R122, R123, R124, R125, R126, R127, R128, R129, R130, R131, R132, R133, R134, R135, R136, R137, R138, R139, R140, R141, R142, R143, R144, R145, R146, R147, R148, R149, R150, R151, R152, R153, R154, R155, R156, R157, R158, R159, R160, R161, R162, R163, R164, R165, R166, R167, R168, R169, R170, R171, R172, R173, R174, R175, R176, R177, R178, R179, R180, R181, R182, R183
Whole-project automation status: 100.0% Candidate
Whole-project real accepted 1:1 estimate: roughly 90.0%
Backend full-suite status: latest locked Simulation regression passed on 2026-05-10 with `mir2-simulation` 836/836 plus Hero AI integration 5/5; latest locked GameData/Protocol/Simulation/Gateway check passed on 2026-05-10, while broader package-specific full-suite evidence remains tracked in the dated sync entries above.

Purpose: this document is the working checklist for moving `mir2-web3` from the current migrated slice toward full Crystal / Mir2 1:1 parity across backend, frontend, assets, integration, and playable operations. It is meant to be updated continuously. When a task is completed and verified, check it off. When a stage gate passes, check the stage gate. Then loop back to the next unchecked item.

Truth source: `docs/PARITY-TRUTH-AUDIT.md` controls status wording. Do not convert Candidate, fallback, or blocked rows into Accepted 1:1 based only on local automation.

This roadmap uses four meanings of completion:

- Slice completion: the currently imported gameplay slice behaves correctly and has passing regression coverage.
- Backend/server parity completion: Rust backend and gateway behavior match Crystal server behavior for the tracked gameplay/server slice. This is tracked separately in `docs/BACKEND-1TO1-PROGRESS.md`.
- Automation Candidate completion: local automated evidence, screenshots, traces, and regression commands are green against the current acceptance bundle.
- Accepted full-project completion: the Rust/Web project is accepted as Crystal 1:1 across backend behavior, frontend/client UI and controls, assets/data fidelity, end-to-end integration, protocol-visible behavior, persistence, and operational edge cases.

Post-1:1 product evolution is tracked separately in `docs/POST-1TO1-EVOLUTION-PLAN.md`. Future database, cache, login UI, NPC script parser, and gameplay redesign work may intentionally diverge from Crystal. Such work should preserve this roadmap as the compatibility baseline instead of rewriting Candidate evidence as product acceptance.

Current verified state as of 2026-04-29:
- [x] 2026-04-29 `R309` Bichon minimap/HUD bounds cleanup: `.mini-map-panel` no longer uses `right=-2px`, closing the measured desktop minimap overflow from `right=1026` to `right=1024`. Evidence: `docs/generated/player-qa/r309-minimap-bounds-web-page-state.json`, `docs/generated/player-qa/r309-minimap-bounds-web-page.png`, and `docs/generated/player-qa/r309-minimap-bounds-compact-web-page.png` record `QA0429A / QA0429Hero` at `0:287,618`, desktop minimap `left=896`, `right=1024`, compact minimap inside `820x640`, `desktopOverflows=[]`, `compactOverflows=[]`, `nonFaviconNetwork404s=[]`, and `consoleErrors=[]`. Exact dynamic animal density/placement, and human visual acceptance remain open.
- [x] 2026-04-29 `R308` Bichon viewport/resource cleanup: original-size browser viewports no longer receive the previous 0.9 CSS stage downscale, the outer page/frame is plain black with no decorative shadow, compact scaling is limited to sub-1024x768 viewports, and missing Bichon visible-object sprite libraries (`NPC/00`, `NPC/01`, `NPC/03`, `NPC/11`, `NPC/15`, `Monster/003`, `Monster/004`, `Monster/005`) are exported from Crystal client data. Evidence: `docs/generated/player-qa/r308-stage-scale-web-page-state.json`, `docs/generated/player-qa/r308-stage-scale-web-page.png`, and `docs/generated/player-qa/r308-stage-scale-compact-web-page.png` record `QA0429A / QA0429Hero` at `0:287,618`, desktop stage bounds `0,0,1024,768`, compact stage `798.72x599.04` inside `820x640`, `hasGuard=true`, `hasArcherGuard=true`, `nonFaviconNetwork404s=[]`, and `consoleErrors=[]`. Verification: web `tsc --noEmit`, JSON parse check, focused `mir2-simulation` R307 regression, `cargo fmt --check`, targeted `git diff --check`, gateway health, and web HTTP 200. Exact dynamic animal density/placement, and human visual acceptance remain open.
- [x] 2026-04-29 `R307` Bichon ordinary Guard/ArcherGuard evidence: added a focused simulation regression for the `0:287,618` comparison point, requiring `Guard` at `291,620` and `ArcherGuard` at `295,624` through both bootstrap `ObjectMonster` packets and `worldSnapshot`. Browser evidence: `docs/generated/player-qa/r307-bichon-guard-archer-web-page.png` plus `docs/generated/player-qa/r307-bichon-guard-archer-web-page-state.json` records `hasGuard=true`, `hasArcherGuard=true`, `monsterCount=7`, `npcCount=5`, `hasUnderscoreNameplate=false`, and `questTrackerVisible=false`. Verification: focused `mir2-simulation` regression and CDP browser capture with zero console errors. Exact dynamic animal density/placement, HUD scale/letterboxing, and human visual acceptance remain open.
- [x] 2026-04-29 `R306` Bichon same-scene display cleanup: the default web quest tracker overlay is no longer rendered over the Crystal playfield, and NPC/monster nameplates display Crystal-style spaces instead of raw underscores while runtime entity names remain unchanged. Evidence: `docs/generated/player-qa/r306-bichon-display-web-page.png` plus `docs/generated/player-qa/r306-bichon-display-web-page-state.json` records the same R305 population count at `0:284,607` (`entityCount=17`, `npcCount=8`, `monsterCount=8`) with `hasUnderscoreNameplate=false` and `questTrackerVisible=false`. Verification: web `tsc --noEmit`, CDP login/start/transfer/browser capture, and zero console errors. Exact placement/density, HUD scale/letterboxing, and human visual acceptance remain open.
- [x] 2026-04-29 `R305` Bichon same-scene visible respawn runtime population: current-map visible Crystal respawns now populate ECS/worldSnapshot using the same visible-respawn helper as bootstrap object packets. Evidence: `docs/generated/player-qa/r305-bichon-visible-world-snapshot.json` records 8 NPCs and 8 monsters at `0:284,607`, including `Deer` and `Royal_Guard`; browser evidence at `docs/generated/player-qa/r305-bichon-visible-web-page.png` plus `docs/generated/player-qa/r305-bichon-visible-web-page-state.json` records 8 NPC sprite elements and 8 monster sprite elements. Verification: focused R305 regression, visible-respawn density regression, `fmt --check`, gateway build, live WS probe, browser capture, gateway health, and web HTTP 200. This closes the first visible Deer/Royal Guard gap only; final visual 1:1 remains open.
- [x] 2026-04-29 `R304` Bichon same-scene current-map NPC runtime population: saved-character `StartGame` and Crystal transfers now populate the current Crystal map with NPC-info manifest entries before snapshots/visible-object packets. Evidence: `docs/generated/player-qa/r304-bichon-npc-world-snapshot.json` for `QA0429A / QA0429Hero` at map `0`, `284,607` records `npcCount=8` and visible `Assistant_Jane` / `Merchant_Ruben`; `docs/generated/player-qa/r304-bichon-npc-web-page.png` plus `docs/generated/player-qa/r304-bichon-npc-web-page-state.json` show the browser page also has 8 NPC sprite elements. Verification: `cargo +1.89.0 fmt --check`, focused NPC regression, adjacent `transfer_map`, `start_game_emits_visible_object_packets`, `world_snapshot_marks_safe_zone_after_start_game`, gateway build, live WS probe, and browser capture. This closes the "no NPCs in aligned Bichon snapshot" gap only; full visual 1:1 remains open.
- [x] 2026-04-29 `R303` all-map frontend source-resource audit: `apps/web/scripts/audit-crystal-map-coverage.mjs` and `npm.cmd run audit:crystal-map-coverage --prefix apps\web` produced the first full manifest coverage evidence at `docs/generated/map/latest-crystal-map-coverage.json` and `docs/generated/map/r303-crystal-map-coverage.json`. Results: 463/463 Crystal manifest maps had local map files, 0 unsupported map types, 0 parse errors, every sampled viewport referenced source frames, and no map libraries were missing. The 2026-05-16 audit above supersedes the historical R303 warnings and adds gameplay semantic coverage; full visual 1:1 acceptance remains a human comparison gate.
- [x] 2026-04-28 `R302` original Crystal client/server visual-reference pass: original `Server.exe` listened on `127.0.0.1:7000`, visible `Client.exe` reached select/game with retained character `R302HeroB`, and evidence was archived at `docs/generated/player-qa/r302-original-client/summary.json`. Web Stage 5 UI smoke captured 88 screenshots with 0 critical console errors. Packet-trace bin 16/16 passed after adding `MIR2_PACKET_TRACE_KEEP_LIFECYCLE_CHARACTER=1`. The fresh live matrix is diagnostic only (`stableDiffCleanCount=2/9`, `packetParityAccepted=false`) and does not change the accepted status wording.
- [x] 2026-04-28 `R301` final automated Candidate acceptance-pack refresh: `docs/generated/player-qa/r301-summary.json` records packet-trace bin 15/15, web `tsc --noEmit`, web `npm.cmd run build`, map API smoke 18/18 with 0 failures, minimap smoke 0 failures with a historical preview-index warning later closed by the 2026-05-16 map audit, WS load 64/64 ready with 0 errors and keepalive p95 637 ms, Stage 5 UI smoke 88 screenshots with 0 critical console errors and 32 compact text nodes checked without overflow, `mir2-game-data` 27/27, `mir2-gateway` 55/55 plus packet-trace bin 15/15, `mir2-admin-api` 22/22, and `mir2-simulation` 674/674. This refresh keeps automation at 100% Candidate and does not close human visual/feel acceptance for whole-project 100% Accepted.
- [x] 2026-04-28 `R300` stable-diff packet acceptance: `apps/gateway/src/bin/packet_trace.rs` now separates strict exact diagnostics from accepted packet parity and supports `MIR2_PACKET_TRACE_ACCEPT_STABLE_DIFF=1` / `MIR2_PACKET_TRACE_REQUIRE_CRYSTAL=1` for the current live Crystal matrix. The accepted gate uses R298's 9/9 stable-clean live matrix and R299's payload probe showing strict exact dirtiness is Crystal dynamic state, not semantic packet drift. Evidence is recorded in `docs/PACKET-PARITY-ACCEPTANCE.md` and `docs/generated/packet-traces/r300-stable-acceptance.json`. Verification passed: `cargo +1.89.0 test --locked -p mir2-gateway --bin packet_trace -- --test-threads=1` (15/15), `cargo +1.89.0 fmt --check`.
- [x] 2026-04-28 `R298` Windows live Crystal stable packet-matrix refresh: `MIR2_CRYSTAL_TCP_ADDR=127.0.0.1:7000`, local gateway `127.0.0.1:7310`, `CRYSTAL_CLIENT_ROOT=E:\mir2\Crystal\Build\Client\Debug`, and fixture character `Cdx0428030348` index `8` produced 9/9 local and Crystal TCP artifacts under `docs/generated/packet-traces/r298-live-matrix` with `stableDiffCleanCount=9` and `acceptedStableLiveComparisonCount=9`. `TimeOfDay` is now treated as stable-comparator volatile, matching the live evidence model. This was stable deterministic evidence only at R298 time; strict exact remained dirty with `diffDirtyCount=9`, and R300 later made this matrix the accepted packet parity gate through explicit stable-diff acceptance. Verification passed: `mir2-simulation` 674/674, `mir2-gateway` 55/55 plus packet-trace bin 14/14, `mir2-admin-api` 22/22, `fmt --check`, `git diff --check`, and web `tsc --noEmit`.
- [x] 2026-04-28 `R297` Windows frontend/player QA refresh: `CRYSTAL_CLIENT_ROOT=E:\mir2\Crystal\Build\Client\Debug` was used for web build and smokes; map API smoke served 18/18 representative requests, minimap smoke had 0 failures with a historical preview-index warning later closed by the 2026-05-16 map audit, WS load reported 64/64 ready with 0 errors and keepalive p95 632 ms, and Stage 5 UI smoke captured 88 screenshots with 0 critical console errors. The round exported missing original scene `NPC/*` and `Monster/*` sprite libs, fixed map-transfer minimap state, and hardened concurrent account-store saves. Verification passed `mir2-simulation` 674/674, `mir2-gateway` 55/55 plus packet-trace bin 14/14, `mir2-admin-api` 22/22, `fmt --check`, `git diff --check`, and web `tsc --noEmit`. This is automated Candidate evidence only; human Crystal visual/feel acceptance remains open.
- [x] 2026-04-28 `R292` Windows live Crystal stable packet-matrix evidence: `MIR2_CRYSTAL_TCP_ADDR=127.0.0.1:7000` and local gateway `127.0.0.1:7310` produced 9/9 local and Crystal TCP artifacts under `docs/generated/packet-traces/r292-live-matrix` with `stableDiffCleanCount=9` and `acceptedStableLiveComparisonCount=9`. This was stable deterministic evidence only at R292 time; strict exact diff remained dirty with `diffDirtyCount=9`, and R300 later made stable-diff the accepted packet gate. Verification passed: `mir2-simulation` 674/674, `mir2-gateway` 55/55 plus packet-trace bin 14/14, `mir2-admin-api` 22/22, `fmt --check`, `git diff --check`, and web `tsc --noEmit`.
- [x] 2026-04-28 `R248` Windows Crystal server-data import closure: local `Server.MirDB` and matching `Build/Server/Debug/Envir/Routes` were available, `generate-crystal-respawn-manifest.mjs` regenerated the Crystal respawn/monster/item/NPC-info manifests, and map records now carry real `NoThrowItem`, `NoDropPlayer`, and `NoDropMonster` flags. Verification passed: `mir2-game-data` 22/22, focused `mir2-simulation no_drop_monster_map_rule` 2/2, full `mir2-simulation` 670/670, and `mir2-gateway` 55/55 plus packet-trace bin tests 7/7. R298/R300 later closed the tracked live packet gate under explicit stable-diff acceptance.
- [x] 2026-04-26 `R225` Mac-local Candidate regression refresh: Stage 5 UI smoke captures 88 screenshots and now records summary counts for screenshot total, 8 compact panel bounds, 34 compact text nodes, 0 critical console errors, and major flow counts. Map API smoke writes 18/18 successful requests, minimap asset smoke writes 0 failures with a historical preview-index warning later closed by the 2026-05-16 map audit, WS load reports 64/64 ready and 0 errors, and Rust package regressions are green (`mir2-game-data` 22/22, `mir2-gateway` 54/54, `mir2-simulation` 664/664). Local require-mode `packet_trace --matrix` wrote 9 artifacts under `docs/generated/packet-traces/r225-matrix`; live Crystal comparison remains blocked until `MIR2_CRYSTAL_TCP_ADDR` is provided.
- [x] 2026-04-26 `R224` packet trace matrix closure: restored `mir2-gateway` `packet_trace` with `--list-flows`, single-flow capture, matrix artifact writing, local/Crystal endpoint capture, diff summaries, fixture metadata, and require-mode enforcement. `mir2-gateway` passes 53/53 including packet trace bin tests 6/6. Local gateway require-mode matrix wrote 9 artifacts under `docs/generated/packet-traces/r224-matrix` with `localOk=true`; 17 matrix entries without TCP `traceFlow` were intentionally skipped. Live Crystal trace comparison is still blocked until `MIR2_CRYSTAL_TCP_ADDR` is provided.
- [x] 2026-04-26 `R223` 100% Candidate automated evidence gate: Stage 5 UI smoke now captures 88 screenshots and records advanced systems state for trade item/cancel, shop gold purchase, auction buy/cancel, conquest end, hero behaviour, mining/craft, and mail delete state, plus compact Mail/Report panel bounds. Map API smoke writes 18/18 successful requests, minimap asset smoke writes 0 failures with a historical preview-index warning later closed by the 2026-05-16 map audit, WS load reports 64/64 ready and 0 errors, and full Rust package regressions are green. Human Crystal visual/feel review is still required for 100% Accepted.
- [x] 2026-04-26 `R219-R222` frontend/global evidence batch: Stage 5 UI smoke now captures 85 screenshots and records login/select lifecycle flows, compact multi-panel layout bounds, NPC dialog link-capable state, and existing broad Stage 5 gameplay/system flows. Map API smoke writes `docs/generated/map/latest-crystal-map-api.json` with 18/18 successful requests, minimap asset smoke writes `docs/generated/assets/latest-minimap-assets.json` with 0 failures and the existing 450/451 missing-index warning, and WS load refresh reports 64/64 ready with 0 errors. Verified by web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke, map/minimap smokes, WS load, `fmt --check`, and `diff --check`.
- [x] 2026-04-26 `R210-R218` frontend/global evidence batch: Stage 5 UI smoke now captures 71 screenshots and records Mail/Report/NPC panel state, broad Stage 5 systems state, guild/group chat filters, Character repair/special-repair, ground item/gold pickup, combat target state, system-menu QA and transfer-list routing, Battle Focus spell casting, and compact inventory panel bounds. Verified by web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 71 screenshots, screenshot visual inspection, `fmt --check`, and `diff --check`.
- [x] 2026-04-26 `R209` frontend/storage-password submit evidence: Stage 5 UI smoke fills Set Storage Password, verifies mismatched confirmation keeps submit disabled with the mismatch warning, submits matching `Safe123` without an active storage service, verifies `hasStoragePassword` remains false with no-service feedback, captures `stage5-storage-password-mismatch.png` and `stage5-storage-password-submit-no-service.png`, and records the extended `storagePasswordFlow`. Verified by web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 60 screenshots, screenshot visual inspection, `fmt --check`, and `diff --check`.
- [x] 2026-04-26 `R208` frontend/storage-password evidence: Protect is now reachable when no storage password is set. Stage 5 UI smoke opens Set Storage Password, verifies title/prompt/input count/disabled submit/debug storage password state, closes it without submitting credentials, captures `stage5-storage-password-panel.png`, and records `storagePasswordFlow`. Verified by web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 58 screenshots, screenshot visual inspection, `fmt --check`, and `diff --check`.
- [x] 2026-04-26 `R207` frontend/storage-takeback evidence: Stage 5 UI smoke opens Take Back for stored Red Potion, selects an inventory slot without an active storage service, verifies bag1 Red Potion remains quantity 3 and storage Red Potion remains quantity 10, captures `stage5-storage-takeback-red-potion-selected.png`, `stage5-storage-takeback-red-potion-result.png`, and `stage5-storage-takeback-red-potion-feedback.png`, and records `storageTakeBackFlow`. Verified by web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 57 screenshots, screenshot visual inspection, `fmt --check`, and `diff --check`.
- [x] 2026-04-26 `R206` frontend/storage-store evidence: Stage 5 UI smoke opens Store Item for Dagger, selects a warehouse slot without an active storage service, verifies Dagger remains in bag1 slot 4 and existing storage contents are unchanged, exposes `storageItems` in Stage 5 debug state, captures `stage5-storage-store-dagger-selected.png`, `stage5-storage-store-dagger-result.png`, and `stage5-storage-store-dagger-feedback.png`, and records `storageStoreFlow`. Verified by web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 54 screenshots, screenshot visual inspection, `fmt --check`, and `diff --check`.
- [x] 2026-04-26 `R205` frontend/inventory-sell evidence: Stage 5 UI smoke opens Sell Item for Dagger, confirms without an active sell service, verifies Dagger remains in bag1 slot 4 and gold stays at 1180, captures `stage5-inventory-sell-dagger-panel.png` and `stage5-inventory-sell-dagger-no-service.png`, and records `inventorySellFlow`. Verified by web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 51 screenshots, screenshot visual inspection, `fmt --check`, and `diff --check`.
- [x] 2026-04-26 `R204` frontend/belt mouse-use evidence: Stage 5 UI smoke clicks Red Potion directly in the belt, verifies quantity decreases from 5 to 4 before the hotkey path decreases it from 4 to 3, captures `stage5-belt-mouse-use-red-potion.png`, and records `beltMouseUseFlow`. Verified by web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 49 screenshots, screenshot visual inspection, `fmt --check`, and `diff --check`.
- [x] 2026-04-26 `R203` frontend/character-remove evidence: Character RemoveItem now targets the inventory grid with a free bag1 slot, and Stage 5 UI smoke verifies Dagger leaves the weapon equipment slot and returns to bag1 slot 4. Captures `stage5-character-remove-dagger.png` and records `characterRemoveFlow`. Verified by web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 48 screenshots, screenshot visual inspection, `fmt --check`, and `diff --check`.
- [x] 2026-04-26 `R202` frontend/inventory-drop evidence: Stage 5 UI smoke opens Delete Item for Blue Potion, confirms the drop, verifies quantity drops from 3 to 2 plus a `Blue Potion` ground label, captures `stage5-inventory-drop-blue-potion-panel.png` and `stage5-inventory-drop-blue-potion.png`, and records `inventoryDropFlow`. Verified by web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 47 screenshots, screenshot visual inspection, `fmt --check`, and `diff --check`.
- [x] 2026-04-26 `R201` frontend/inventory-split evidence: Stage 5 UI smoke opens Split Item for Red Potion, confirms count 1, verifies Crystal-style belt placement with total Red Potion quantity preserved, captures `stage5-inventory-split-red-potion-panel.png` and `stage5-inventory-split-red-potion.png`, and records `inventorySplitFlow`. Verified by web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 45 screenshots, screenshot visual inspection, `fmt --check`, and `diff --check`.
- [x] 2026-04-26 `R200` frontend/inventory-move evidence: Stage 5 UI smoke context-clicks Wooden Sword in bag1, moves it from slot 4 to slot 10, captures `stage5-inventory-move-wooden-sword.png`, and records `inventoryMoveFlow`. Verified by web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 43 screenshots, screenshot visual inspection, `fmt --check`, and `diff --check`.
- [x] 2026-04-26 `R199` frontend/inventory-gold evidence: Stage 5 UI smoke opens Drop Gold, confirms 100 gold, verifies gold drops from 1280 to 1180 plus a `100 Gold x100` ground label, fixes missing `ui.confirm` fallback text, captures two gold-drop screenshots, and records `inventoryGoldFlow`. Verified by web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 42 screenshots, screenshot visual inspection, `fmt --check`, and `diff --check`.
- [x] 2026-04-26 `R198` frontend/HUD button evidence: Stage 5 UI smoke opens Character Spells from HUD Skill and Stats II from HUD Option, captures `stage5-hud-skill-spells.png` and `stage5-hud-option-stats2.png`, and records `hudButtonFlow`. Verified by web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 40 screenshots, screenshot visual inspection, `fmt --check`, and `diff --check`.
- [x] 2026-04-26 `R197` frontend/inventory-equip evidence: Stage 5 UI smoke clicks Dagger from inventory bag1, verifies Dagger moves into the weapon equipment slot, captures `stage5-inventory-equip-dagger.png`, and records `inventoryEquipFlow`. Verified by web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 38 screenshots, screenshot visual inspection, `fmt --check`, and `diff --check`.
- [x] 2026-04-26 `R196` frontend/inventory-use evidence: Stage 5 UI smoke clicks Red Potion from inventory bag1, verifies quantity drops from 5 to 4, captures `stage5-inventory-use-red-potion.png`, and records `inventoryUseFlow`. Verified by web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 37 screenshots, screenshot visual inspection, `fmt --check`, and `diff --check`.
- [x] 2026-04-26 `R195` frontend/expanded-storage evidence: Stage 5 UI smoke rents expanded storage from locked page 2, verifies active expanded storage, unlocked page 2, 160-slot capacity, expiry copy, captures the rented page screenshot, and records the rented state in `storageFlow`. Verified by web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 36 screenshots, screenshot visual inspection, `fmt --check`, and `diff --check`.
- [x] 2026-04-26 `R194` frontend/system-menu evidence: Stage 5 UI smoke opens the system menu, verifies transfer/action labels, routes Character/Inventory/Quest actions, captures four system-menu screenshots, and records `systemMenuFlow`. Verified by web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 35 screenshots, screenshot visual inspection, `fmt --check`, and `diff --check`.
- [x] 2026-04-26 `R193` frontend/chat-control evidence: Stage 5 UI smoke exercises chat Shout filter, All restore, Settings, collapse/restore size, and Report paths, captures four chat-control screenshots, and records `chatFlow`. Verified by web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 31 screenshots, screenshot visual inspection, `fmt --check`, and `diff --check`.
- [x] 2026-04-26 `R192` frontend/storage evidence: Stage 5 UI smoke switches storage page 1 -> locked page 2 -> page 1, captures two storage page screenshots, and records `storageFlow`. Verified by web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 27 screenshots, screenshot visual inspection, `fmt --check`, and `diff --check`.
- [x] 2026-04-26 `R191` frontend/character evidence: Stage 5 UI smoke exposes active character tab and known skills in debug state, switches char -> stats1 -> stats2 -> spells -> char, captures four character tab screenshots, and records `characterFlow`. Verified by web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 25 screenshots, screenshot visual inspection, `fmt --check`, and `diff --check`.
- [x] 2026-04-26 `R190` frontend/inventory evidence: Stage 5 UI smoke exposes inventory items and active tab in debug state, switches bag1 -> bag2 -> quest -> bag1, captures three inventory tab screenshots, and records `inventoryFlow`. Verified by web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 21 screenshots, screenshot visual inspection, `fmt --check`, and `diff --check`.
- [x] 2026-04-26 `R189` frontend/belt use evidence: Stage 5 UI smoke exposes belt items in debug state, presses hotkey `1`, verifies slot-1 Red Potion quantity drops from 5 to 4, captures `stage5-belt-hotkey-use.png`, and records `beltUseFlow`. Verified by web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 18 screenshots, screenshot visual inspection, `fmt --check`, and `diff --check`.
- [x] 2026-04-26 `R188` frontend/belt evidence: Stage 5 UI smoke now exercises belt horizontal, vertical, rotate-back, and close states; fixes belt slot-label offsets and vertical Quest overlap; captures three belt screenshots; and records `beltFlow`. Verified by web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 17 screenshots, screenshot visual inspection, `fmt --check`, and `diff --check`.
- [x] 2026-04-26 `R187` frontend/minimap evidence: Stage 5 UI smoke now exercises minimap collapse, BigMap re-expand, and Mail open paths; captures three minimap screenshots; and records `minimapFlow` state. Verified by web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 14 screenshots, screenshot visual inspection, `fmt --check`, and `diff --check`.
- [x] 2026-04-26 `R186` frontend/global text-layout evidence: Stage 5 UI smoke now checks visible compact quest/HUD/minimap/belt/chat/entity text for overflow, records `compactTextLayout`, and the compact minimap title/Safe Zone label is stable two-line text. Verified by web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 11 screenshots and 33 compact text nodes checked, compact screenshot visual inspection, `fmt --check`, and `diff --check`.
- [x] 2026-04-26 `R185` frontend/global screenshot evidence: Stage 5 UI smoke now records named desktop 1024x768 and compact 820x640 viewports, captures `stage5-compact-game.png`, writes compact core UI bounds to the manifest, and fails if stage/HUD/chat/minimap overflow the compact viewport. Verified by `node --check`, gateway/web health, Stage 5 UI smoke with 11 screenshots and zero critical console errors, compact screenshot visual inspection, `fmt --check`, and `diff --check`.
- [x] 2026-04-26 `R184` frontend/global smoke parity: chat latest-line/scroll behavior is implemented, headless/no-WebGL UI uses DOM fallback, Crystal map API no longer recursively fails when local Crystal map files are absent, and Stage 5 UI smoke detects macOS Chrome. Verified by web `tsc --noEmit`, direct `next build`, minimap smoke, map API smoke, Stage 5 UI smoke with 10 screenshots and zero critical console errors, gateway health on 7110, WS load 64/64 ready, `fmt --check`, and `diff --check`.
- [x] 2026-04-26 `R183` backend/global namespace cleanup moved runtime interaction quest hints from `sim.questHint` to `custom.interaction.questHint`, synchronized the importer and generated localization bundles, and left no `sim.*` references in `apps/simulation/src/runtime.rs`. Verified by no-match runtime grep, `mir2-game-data` (22/22), focused snapshot test (1/1), `fmt --check`, and full `mir2-simulation` 664/664.
- [x] 2026-04-26 `R182` backend NPC packet-surface parity removed runtime-only no-script/no-page idle dialog fallback, matching Crystal `NPCScript.Call` no-response behavior when no page is found. Verified by focused no-script NPC (1/1), adjacent `npc_interaction` (2/2), broad `crystal_npc` (52/52), `fmt --check`, and full `mir2-simulation` 664/664.
- [x] 2026-04-26 `R181` backend quest-required drop localization parity replaced runtime-only quest drop/progress chats with Crystal `server.YouFound` feedback while preserving `GainedItem` and quest state updates. Verified by focused quest-required drop (1/1), adjacent `quest_required_drop` (3/3), `fmt --check`, and full `mir2-simulation` 664/664.
- [x] 2026-04-26 `R180` backend start-game localization parity replaced runtime-only `sim.welcomeCharacter` System text with Crystal `server.Welcome` using localized `server.GameName` and `ChatType::Hint`. Verified by focused simulation/gateway `start_game_emits_bootstrap_sequence` (1/1 each), `fmt --check`, full `mir2-simulation` 664/664, and full `mir2-gateway` 47/47.
- [x] 2026-04-26 `R179` backend chat packet-surface parity removed runtime-only normal chat self echo. Pre-start normal chat now returns no packets, and in-game normal chat emits only Crystal-shaped `ObjectChat` with `Name: message`. Verified by simulation `chat_` (43/43), gateway `chat_` (2/2), `fmt --check`, full `mir2-simulation` 664/664, and full `mir2-gateway` 47/47.
- [x] 2026-04-26 `R178` backend cast-skill helper packet-surface parity removed runtime-only failure chats from high-level casting unknown-skill, cooldown, unwired-definition, missing-player, no-MP, unwired summon-spell, and missing summon-template branches while preserving successful buff/summon behavior. Verified by `casting` (9/9), `fmt --check`, and full `mir2-simulation` 663/663.
- [x] 2026-04-26 `R177` backend MoveItem packet-surface parity removed the runtime-only unsupported-grid/missing-source `sim.itemNotFoundInBag` fallback while preserving failed-ack-only unsupported grids and Crystal `server.ItemMoveErrorReport` for Inventory/Storage. Verified by `move_item` (26/26), `fmt --check`, and full `mir2-simulation` 660/660.
- [x] 2026-04-26 `R176` backend NPC dialog helper packet-surface parity removed runtime-only stale active-dialog missing-NPC/no-script chats while preserving ordinary no-script NPC idle fallback. Verified by focused stale-dialog tests (2/2), `npc_interaction` (2/2), broad `crystal_npc` (52/52), `fmt --check`, and full `mir2-simulation` 660/660.
- [x] 2026-04-26 `R175` backend NPC dialog helper packet-surface parity removed runtime-only no-active-dialog, invalid-target, and no-pending-input chats while preserving successful dialog link/input/service flows. Verified by focused dialog-helper tests (3/3), `npc_interaction` (2/2), broad `crystal_npc` (52/52), `fmt --check`, and full `mir2-simulation` 658/658.
- [x] 2026-04-26 `R174` backend NPC interaction helper packet-surface parity removed runtime-only direct interaction invalid target/direction/range chats while preserving successful NPC dialog/script/service flows. Verified by focused direct-interact tests (3/3), `npc_interaction` (2/2), broad `crystal_npc` (52/52), `fmt --check`, and full `mir2-simulation` 655/655.
- [x] 2026-04-26 `R173` backend attack helper packet-surface parity removed runtime-only direct attack invalid target/state/range chats while preserving turn packets, normal attacks, hidden reveal, Zuma wake, and delayed hit behavior. Verified by focused direct-attack tests (4/4), hidden/Zuma focused tests (2/2), adjacent `attack` (80/80), `fmt --check`, and full `mir2-simulation` 652/652.
- [x] 2026-04-26 `R172` backend NPC interaction packet-surface parity removed runtime-only successful interaction chat while preserving NPC `ObjectChat`/dialog packet surfaces and Crystal NPC script/service flows. Verified by focused `npc_interaction` (2/2), `crystal_npc_dialog` (1/1), `crystal_npc_service` (1/1), broad `crystal_npc` (52/52), `fmt --check`, and full `mir2-simulation` 648/648.
- [x] 2026-04-26 `R171` backend pickup helper packet-surface parity removed runtime-only direct pickup invalid target/distance chats while preserving Crystal owner/full-bag pickup messages and current-cell packet pickup behavior. Verified by focused direct-pickup tests (3/3), adjacent `pickup` (18/18), `drop` (42/42), `fmt --check`, and full `mir2-simulation` 648/648.
- [x] 2026-04-26 `R170` backend death-drop packet-surface parity removed runtime-only missing defeated-entity chat while preserving normal death/drop packet behavior. Verified by focused missing-entity silent test (1/1), visible death packet test (1/1), adjacent `drop` (41/41), `fmt --check`, and full `mir2-simulation` 645/645.
- [x] 2026-04-26 `R169` backend monster-drop packet-surface parity removed runtime-only gold/item drop success chats while preserving ground drop creation, quest-drop routing, owner windows, and pickup packet surfaces. Verified by focused item-drop no-chat (1/1), focused gold-drop no-chat/pickup (1/1), adjacent `drop` (41/41), `pickup` (15/15), `attack` (76/76), `fmt --check`, and full `mir2-simulation` 644/644.
- [x] 2026-04-26 `R168` backend summon packet-surface parity removed runtime-only `sim.targetDefeated` from summoned VampireSpider death explosion while preserving explosion damage and despawn behavior. Verified by focused vampire-spider no-chat explosion test (1/1), adjacent `spider` (6/6), `attack` (76/76), `fmt --check`, and full `mir2-simulation` 643/643.
- [x] 2026-04-26 `R167` backend combat packet-surface parity removed runtime-only ordinary damage narration from player/monster hit resolution while preserving packet health/struck/death surfaces and Trainer DPS reporting. Verified by focused player-hit no-chat test (1/1), adjacent `attack` (76/76), `fmt --check`, and full `mir2-simulation` 643/643.
- [x] 2026-04-26 `R166` backend cast-skill packet-surface parity removed runtime-only generic `sim.castSkill` success chat from buff/heal and summon paths while preserving state mutation and spawns. Verified by focused `casting` (6/6), `fmt --check`, and full `mir2-simulation` 643/643.
- [x] 2026-04-26 `R165` backend cast-skill helper packet-surface parity removed runtime-only pre-start chat from high-level `cast_skill`, preserving started-world casting behavior. Verified by focused pre-start cast-skill test (1/1), adjacent `casting` (6/6), `fmt --check`, and full `mir2-simulation` 643/643.
- [x] 2026-04-26 `R164` backend interaction helper packet-surface parity removed runtime-only pre-start chats from high-level `interact` and dialog target follow-up, preserving started-world NPC dialog behavior. Verified by focused pre-start interaction test (1/1), adjacent `npc_interaction` (2/2), `crystal_npc_dialog` (1/1), `crystal_npc_service` (1/1), `fmt --check`, and full `mir2-simulation` 642/642.
- [x] 2026-04-26 `R163` backend harvest helper packet-surface parity removed runtime-only pre-start chats from high-level `harvest` and packet `Harvest`, preserving started-world harvest behavior. Verified by focused pre-start harvest test (1/1), adjacent `harvest` (9/9), `fmt --check`, and full `mir2-simulation` 641/641.
- [x] 2026-04-26 `R162` backend attack helper packet-surface parity removed runtime-only pre-start chats from high-level `attack` and packet `Attack` and `RangeAttack`, preserving started-world attack traces. Verified by focused pre-start attack test (1/1), adjacent `attack` (76/76), combat trace focused test (1/1), `fmt --check`, and full `mir2-simulation` 640/640.
- [x] 2026-04-26 `R161` backend movement helper packet-surface parity removed runtime-only pre-start chats from high-level `move_to` and packet `Walk`, `Run`, and `Turn`, preserving started-world movement behavior. Verified by focused pre-start movement test (1/1), adjacent `walk` (6/6), `run_` (3/3), `transfer_map` (2/2), `fmt --check`, and full `mir2-simulation` 639/639.
- [x] 2026-04-26 `R158` backend trainer localization parity added Crystal-style `{index:format}` placeholder substitution and routed trainer average damage chat through `server.AverageDamageOnTrainer`. Verified by `mir2-game-data` (22/22), focused trainer test (1/1), `fmt --check`, and full `mir2-simulation` 638/638.
- [x] 2026-04-26 `R157` backend item-use localization parity routed benediction-oil no-effect/luck/curse outcomes through Crystal `server.WeaponNoEffect`, `server.WeaponLuck`, and `server.WeaponCurse`. Verified by focused `benediction_oil` (4/4), adjacent `use_item` (42/42), `fmt --check`, and full `mir2-simulation` 638/638.
- [x] 2026-04-26 `R156` backend storage helper packet-surface parity removed runtime-only `@ADDSTORAGE` expanded-storage success chat, leaving the modeled `ResizeStorage` packet surface. Verified by focused `addstorage` (2/2), adjacent `storage` (43/43), `fmt --check`, and full `mir2-simulation` 638/638.
- [x] 2026-04-26 `R155` backend pickup localization parity routed `ShowGroupPickup` item notices through Crystal `server.FriendlyPickedUpItem` instead of hardcoded English formatting. Verified by focused group pickup test (1/1), adjacent `pickup` (14/14), `fmt --check`, and full `mir2-simulation` 638/638.
- [x] 2026-04-26 `R154` backend item helper packet-surface parity removed runtime-only high-level `use_item(key)` / `drop_item(key)` before-start chats, preserving no-packet/no-chat behavior and normal post-start behavior. Verified by adjacent `drop_item` (10/10), focused consumable helper (1/1), adjacent `use_item` (42/42), `fmt --check`, and full `mir2-simulation` 638/638.
- [x] 2026-04-26 `R153` backend drop helper packet-surface parity removed runtime-only high-level `drop_item(key)` missing-item chat, preserving no-mutation/no-packet behavior. Verified by focused drop helper test (1/1), adjacent `drop_item` (10/10), `fmt --check`, and full `mir2-simulation` 638/638.
- [x] 2026-04-26 `R152` backend map-transfer localization parity routed not-in-world transfer attempts through Crystal `server.NotFound`, including ordinary and debug transfer keys. Verified by focused transfer-bound test (1/1), adjacent `transfer_map` (2/2), `fmt --check`, and full `mir2-simulation` 638/638.
- [x] 2026-04-26 `R151` backend request-item-info localization parity routed missing-template `RequestItemInfo` failure through Crystal `server.NotFound`. Verified by focused request-item-info test (1/1), `fmt --check`, and full `mir2-simulation` 638/638.
- [x] 2026-04-26 `R150` backend map-transfer localization parity routed source-tile/bounds rejection through Crystal `server.CannotPositionMoveOnMap` while preserving no-transfer/no-position-mutation behavior. Verified by focused transfer-bounds test (1/1), adjacent `transfer_map` (2/2), `fmt --check`, and full `mir2-simulation` 638/638.
- [x] 2026-04-26 `R149` backend Stage 5 helper packet-surface parity removed runtime-only `event.spawn` and `hero.behaviour` success narration while preserving state mutation. Verified by focused conquest/event/hero test (1/1), broader `stage5_` (26/26), `fmt --check`, and full `mir2-simulation` 638/638.
- [x] 2026-04-26 `R148` backend map-transfer packet-surface parity removed the runtime-only debug Crystal transfer success chat, leaving `MapInformation` and `UserLocation` as the modeled success surface. Verified by focused debug transfer test (1/1), adjacent `transfer_map` (2/2), `fmt --check`, and full `mir2-simulation` 638/638.
- [x] 2026-04-26 `R147` backend Stage 5 helper packet-surface parity removed generic runtime-only helper success chats across group/social/mail/trade/auction/conquest/hero/profession helpers while preserving state mutation. Verified by `stage5_` (26/26), `fmt --check`, and full `mir2-simulation` 638/638.
- [x] 2026-04-25 `R82` backend item-use parity completed for manifest-backed `UseItem` restrictions and skill-book learn behavior: `RequiredGender` / `RequiredClass` checks, `RequiredType == Level` gating, repeated skill-book attempts, and consume-on-success for valid learns.
- [x] 2026-04-25 `R83` backend item-use parity completed for remaining manifest-backed use surfaces including `AncientBanga[Green]` and `AncientBanga[Purple]` via scroll shape 8/9, `free_map_shout` / `free_server_shout` emissions, Crystal hint-chat, and localized `server.CreditsAddedToAccount` credit-token handling.
- [x] 2026-04-25 focused validation includes `use_item_packet_`, `equip_item_packet`, `item`, and `storage` test suites plus full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` with 631 tests passing after R111.
- [x] 2026-04-25 `R84` backend item-use parity completed for manifest-backed `UseItem` scroll shape 26/27 (`GtInvite` / `GTTeleport`) with no chat/teleport side effect and success ack-only behavior after `CanUseItem`; focused regressions added for both branches: `use_item_packet_dynamic_crystal_gt_invite_consumes_without_active_effect` and `use_item_packet_dynamic_crystal_gt_teleport_consumes_without_teleporting`.
- [x] 2026-04-25 `R85` backend item-use parity expanded modeled `CanUseItem` required-type checks for `MaxAC` / `MaxMAC` / `MaxDC` / `MaxMC` / `MaxSC` / `MinAC` / `MinMAC` / `MinDC` / `MinMC` / `MinSC` / `MaxLevel` against existing equipment/buff totals; focused and adjacent suite coverage passed with `use_item_packet_crystal_equipment_rejects_low_max_dc_requirement` and `use_item_packet_crystal_equipment_allows_modeled_max_mc_requirement`.
- [x] 2026-04-25 `R86` backend item-use parity added manifest-backed support for `DungeonEscape` / `TeleportHome` and `RandomTeleport` through scroll-shape `0/2`, with same-map destination validation and bounded success/failure `UseItem` surfaces.
- [x] 2026-04-25 `R87` backend item-use parity expanded `ItemType.Food` mount-feed support for `RawMeat` and `LeanMeat`, including equipped mount/dura gating, Crystal-style max-dura loss for shape `0`, mount-fed hint surfaces, and `ItemRepaired`.
- [x] 2026-04-25 `R88` backend item-use parity added Crystal normal-potion shape `0` pending/timed recovery as a modeled subset: `crystal-item-*` consumable flow now queues `pending_pot_health_amount` / `pending_pot_mana_amount` on `UseItem`, consumes the potion without immediate HP/MP mutation or hint chat, and drains queued recovery on `advance_world` via `ObjectHealth` / `ObjectMana` packets. Verified by focused new test `use_item_packet_dynamic_crystal_normal_potion_queues_timed_restore` plus full `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_ -- --test-threads=1 --nocapture` (33/33).
- [x] 2026-04-25 `R89` backend item-use parity mapped Crystal manifest equipment item types to runtime `EquipmentSlot` for item gain/test helper creation and `UseItem` fallback, eliminating manual slot setup in current manifest equipment tests.
- [x] 2026-04-25 `R90` backend item-use parity added Crystal `NoEscape` / `NoRandom` map-rule rejections for manifest-backed scroll shape `0/2`, preserving item/position and emitting localized `server.CanNotDungeon` / `server.CanNotRandom` system messages. Verified by focused no-escape/no-random regressions plus `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_ -- --test-threads=1 --nocapture` (35/35).
- [x] 2026-04-25 `R91` backend item-use parity added Crystal repair-bind rejection for manifest-backed `RepairOil` / `WarGodOil`, including `DontRepair` and `NoSRepair` failure surfaces with no consume and no weapon mutation. Verified by `use_item_packet_dynamic_crystal_repair_oils_respect_weapon_repair_binds` plus `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_ -- --test-threads=1 --nocapture` (36/36).
- [x] 2026-04-25 `R92` backend item-use parity restored modeled MP on successful dead-player `ResurrectionScroll` revive, matching Crystal's revive vitals surface within the current runtime cap. Verified by `use_item_packet_dead_player_resurrection_scroll_revives_and_consumes_item` plus `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_ -- --test-threads=1 --nocapture` (36/36).
- [x] 2026-04-25 `R93` backend equipment parity fixed explicit `EquipItem` compatibility for manifest-backed ring/bracelet items so right-side ring/bracelet slots are accepted by imported item type, rather than blocked by default left-slot metadata. Verified by focused right-slot regression plus `cargo +1.89.0 test --locked -p mir2-simulation equip_item_packet -- --test-threads=1 --nocapture` (9/9).
- [x] 2026-04-25 `R95` backend equipment parity added explicit `ItemType.Amulet` to right-bracelet compatibility coverage.
- [x] 2026-04-25 `R96` backend equipment parity added explicit `EquipItem` requirement gating for dynamic manifest-backed equipment: Crystal gender/class/required-type failures silently fail before mutation like `CanEquipItem`, while `UseItem` keeps localized requirement messages. Verified by focused regression, `equip_item_packet` (11/11), `use_item_packet_crystal_equipment_` (2/2), `fmt --check`, `diff --check`, and full `mir2-simulation` 622/622.
- [x] 2026-04-25 `R97` backend equipment parity locked storage-grid coverage for the dynamic manifest-backed explicit `EquipItem` requirement rejection surface. Verified by focused regression, `equip_item_packet` (12/12), `fmt --check`, `diff --check`, and full `mir2-simulation` 623/623.
- [x] 2026-04-25 `R98` backend item-use parity locked dynamic manifest-backed `CreditToken3` use coverage for success ack, `GainedCredit`, localized `server.CreditsAddedToAccount` hint chat, credit-state update, and item consumption. Verified by focused regression, `use_item_packet_` (37/37), `fmt --check`, `diff --check`, and full `mir2-simulation` 624/624.
- [x] 2026-04-25 `R99` backend equipment parity locked the positive explicit `EquipItem` requirement path for dynamic manifest-backed equipment, using `SpiritRing` at level 15 into the right ring slot. Verified by focused regression, `equip_item_packet` (13/13), `fmt --check`, `diff --check`, and full `mir2-simulation` 625/625.
- [x] 2026-04-25 `R100` backend item/equipment packet-surface parity removed runtime-only `sim.equippedItem*` chat from successful modeled use-equip. The success surface is now ack/refresh/equipment-state only, matching Crystal's explicit equip success surface for the bounded runtime. Verified by focused regression, `use_item_packet_` (37/37), `equip_item_packet` (13/13), `fmt --check`, `diff --check`, and full `mir2-simulation` 625/625.
- [x] 2026-04-25 `R101` backend item/equipment packet-surface parity removed the literal runtime-only non-inventory use-equip failure chat. Belt-sourced equipment-like use now failed-acks without chat or mutation. Verified by focused regression, `use_item_packet_` (38/38), `fmt --check`, `diff --check`, and full `mir2-simulation` 626/626.
- [x] 2026-04-25 `R102` backend item-use packet-surface parity removed runtime-only `sim.itemNoActiveUse` from the unusable inventory item fallback. Unknown/unusable items now failed-ack without chat or mutation. Verified by focused regression, `use_item_packet_` (39/39), `fmt --check`, `diff --check`, and full `mir2-simulation` 627/627.
- [x] 2026-04-25 `R103` backend item-use packet-surface parity removed runtime-only `sim.itemNotFoundInBag` from missing-item and invalid-source `UseItem` failures. Missing inventory ids now failed-ack without chat or mutation. Verified by focused regression, `use_item_packet_` (40/40), `fmt --check`, `diff --check`, and full `mir2-simulation` 628/628.
- [x] 2026-04-25 `R104` backend item-use packet-surface parity made unmodeled `UseItem(grid=HeroInventory)` return a Crystal-shaped failed ack instead of empty packets while preserving the no-fallback/no-mutation guard. Verified by focused regression, `use_item_packet_` (40/40), `fmt --check`, `diff --check`, and full `mir2-simulation` 628/628.
- [x] 2026-04-25 `R114` backend item-use map-rule parity added Crystal `NoDrug` rejection for static starter and dynamic manifest-backed potion `UseItem`. Blocked maps now emit `server.YouCannotUsePotionsHere`, failed ack, preserve items, and avoid HP/MP recovery queueing. Verified by `no_drug` (2/2), `use_item_packet_` (42/42), `fmt --check`, and full `mir2-simulation` 633/633.
- [x] 2026-04-25 `R115` backend pickup packet/chat parity removed runtime-only normal item/gold pickup success chat while preserving Crystal `ShowGroupPickup` group notices. Verified by `pickup` (14/14), `fmt --check`, and full `mir2-simulation` 633/633.
- [x] 2026-04-25 `R116` backend pickup localization parity routed owner-blocked pickup rejection through Crystal `server.CannotPickupNotOwner` while preserving owner-window and scan-skip behavior. Verified by `pickup` (14/14), `fmt --check`, and full `mir2-simulation` 633/633.
- [x] 2026-04-25 `R117` backend harvest localization parity routed no-drop/full-bag harvest messages through Crystal `server.NothingWasFound` and `server.YouCannotCarryAnymore`. Verified by `harvest` (8/8), `fmt --check`, and full `mir2-simulation` 633/633.
- [x] 2026-04-25 `R118` backend Stage 5 item localization parity routed socket max-capacity and already-sealed rejections through Crystal `server.ItemMaxSockets` and `server.ItemAlreadySealed`. Verified by `stage5_item_` (13/13), `fmt --check`, and full `mir2-simulation` 633/633.
- [x] 2026-04-25 `R119` backend Stage 5 economy/helper localization parity routed mail/shop/auction/craft full-bag rejections through Crystal `server.YouCannotCarryAnymore`. Verified by `stage5_shop_and_auction_full_bag_preserve_gold_and_items` (1/1), `stage5_` (22/22), `fmt --check`, and full `mir2-simulation` 633/633.
- [x] 2026-04-25 `R120` backend pickup localization parity routed direct ground-drop pickup full-bag rejection through Crystal `server.YouCannotCarryAnymore` while preserving current-cell skip behavior. Verified by `pickup` (14/14), `fmt --check`, and full `mir2-simulation` 633/633.
- [x] 2026-04-25 `R121` backend Stage 5 economy localization parity routed trade/shop/auction low-gold rejections through Crystal `server.LowGold`. Verified by `stage5_trade_shop_and_auction_cancel_error_paths_preserve_gold` (1/1), `stage5_` (22/22), `fmt --check`, and full `mir2-simulation` 633/633.
- [x] 2026-04-25 `R122` backend Stage 5 trade localization parity routed successful trade completion through Crystal `server.TradeSuccessful`. Verified by `stage5_trade_shop_and_auction_are_transactional` (1/1), `stage5_` (22/22), `fmt --check`, and full `mir2-simulation` 633/633.
- [x] 2026-04-25 `R123` backend Stage 5 credit-shop localization parity routed purchase chat through Crystal `server.BoughtItemForCredit` while preserving mailbox delivery. Verified by `stage5_credit_shop_mails_purchase_and_claim_transfers_attachment` (1/1), `stage5_` (22/22), `fmt --check`, and full `mir2-simulation` 633/633.
- [x] 2026-04-25 `R124` backend Stage 5 item-seal localization parity routed reseal-delay rejection through Crystal `server.ItemCannotBeResealedFor` with the modeled remaining-duration label. Verified by `stage5_item_seal_rejects_before_next_seal_date_after_expiry` (1/1), `stage5_item_` (13/13), `fmt --check`, and full `mir2-simulation` 633/633.
- [x] 2026-04-25 `R125` backend Stage 5 item success-chat localization parity routed socket/seal success messages through Crystal `server.ItemSocketsIncreased` and `server.ItemSealedFor`. Verified by `stage5_item_` (13/13), `fmt --check`, and full `mir2-simulation` 633/633.
- [x] 2026-04-25 `R126` backend storage localization parity routed expanded-storage expiry notice through Crystal `server.ExpandedStorageExpired` while preserving one-shot resize and persistence behavior. Verified by `expired_expanded_storage_tick_emits_resize_notice_once_and_persists_flag` (1/1), `storage` (43/43), `fmt --check`, and full `mir2-simulation` 633/633.
- [x] 2026-04-25 `R127` backend harvest packet-surface parity removed runtime-only harvest success chat so successful transfer emits `GainedItem` plus `ObjectHarvested` without generic `"Harvested ..."` text. Verified by `harvest` (8/8), `fmt --check`, and full `mir2-simulation` 633/633.
- [x] 2026-04-26 `R128` backend Stage 5 gold-shop localization parity routed purchase chat through Crystal `server.BoughtItemForGold` while preserving gold debit and item gain. Verified by `stage5_trade_shop_and_auction_are_transactional` (1/1), `stage5_` (22/22), `fmt --check`, and full `mir2-simulation` 633/633.
- [x] 2026-04-26 `R129` backend Stage 5 item localization parity routed socket/seal invalid-source rejections through Crystal `server.InvalidCombination` while preserving source item retention and no-mutation failure behavior. Verified by `stage5_item_` (13/13), `fmt --check`, and full `mir2-simulation` 633/633.
- [x] 2026-04-26 `R130` backend map-transfer packet-surface parity removed runtime-only ordinary transfer success chat while preserving `MapInformation`, `UserLocation`, and safe-zone/map snapshot updates. Verified by `transfer_map` (2/2), `fmt --check`, and full `mir2-simulation` 633/633.
- [x] 2026-04-26 `R131` backend Stage 5 item localization parity routed socket/seal missing-source rejections through Crystal `server.NotFound` while preserving source lookup and no-mutation failure behavior. Verified by `stage5_item_` (13/13), `fmt --check`, and full `mir2-simulation` 633/633.
- [x] 2026-04-26 `R132` backend Stage 5 item localization parity routed socket/seal missing-equipped-item rejections through Crystal `server.NotFound` while preserving no-mutation failure behavior. Verified by `stage5_item_` (15/15), `fmt --check`, and full `mir2-simulation` 635/635.
- [x] 2026-04-26 `R133` backend Stage 5 item localization parity routed socket metadata-missing rejection through Crystal `server.NotFound` while preserving no-mutation failure behavior. Verified by `stage5_item_` (16/16), `fmt --check`, and full `mir2-simulation` 636/636.
- [x] 2026-04-26 `R134` backend Stage 5 missing-entity localization parity routed mail/trade/auction missing-entity rejections through Crystal `server.NotFound` while preserving no-mutation failure behavior. Verified by `stage5_` (26/26), `fmt --check`, and full `mir2-simulation` 638/638.
- [x] 2026-04-26 `R135` backend Stage 5 credit-shop localization parity routed insufficient-credit rejection through Crystal `server.YouDontHaveEnoughCurrency` while preserving credit/mail/item no-mutation behavior. Verified by `stage5_trade_shop_and_auction_cancel_error_paths_preserve_gold` (1/1), `fmt --check`, and full `mir2-simulation` 638/638.
- [x] 2026-04-26 `R136` backend Stage 5 craft localization parity routed no-ore rejection through Crystal `server.CraftingAttemptFailed` while preserving no crafted-item mutation. Verified by `stage5_conquest_event_hero_mining_and_crafting_flow` (1/1), `fmt --check`, and full `mir2-simulation` 638/638.
- [x] 2026-04-26 `R137` backend Stage 5 guild localization parity routed guild creation success through Crystal `server.SuccessfullyCreatedGuild`. Verified by `stage5_social_group_guild_mail_persist_across_reload` (1/1), `fmt --check`, and full `mir2-simulation` 638/638.
- [x] 2026-04-26 `R138` backend Stage 5 event localization parity routed missing monster-template rejection through Crystal `server.NotFound`. Verified by `stage5_conquest_event_hero_mining_and_crafting_flow` (1/1), `fmt --check`, and full `mir2-simulation` 638/638.
- [x] 2026-04-26 `R139` backend Stage 5 hero localization parity routed missing-hero behaviour rejection through Crystal `server.NotFound`. Verified by `stage5_conquest_event_hero_mining_and_crafting_flow` (1/1), `fmt --check`, and full `mir2-simulation` 638/638.
- [x] 2026-04-26 `R140` backend Stage 5 trade localization parity routed missing `trade.offerGold` amount rejection through Crystal `server.InvalidPacketReceived`. Verified by `stage5_trade_shop_and_auction_cancel_error_paths_preserve_gold` (1/1), `fmt --check`, and full `mir2-simulation` 638/638.
- [x] 2026-04-26 `R141` backend Stage 5 mail localization parity routed missing `mail.claim` / `mail.delete` id rejections through Crystal `server.InvalidPacketReceived`. Verified by `stage5_social_group_guild_mail_persist_across_reload` (1/1), `fmt --check`, and full `mir2-simulation` 638/638.
- [x] 2026-04-26 `R142` backend Stage 5 auction localization parity routed missing `auction.buy` / `auction.cancel` id rejections through Crystal `server.InvalidPacketReceived`. Verified by `stage5_trade_shop_and_auction_cancel_error_paths_preserve_gold` (1/1), `fmt --check`, and full `mir2-simulation` 638/638.
- [x] 2026-04-26 `R143` backend Stage 5 trade localization parity routed inactive-trade rejections through Crystal `server.NotFound`. Verified by `stage5_trade_shop_and_auction_cancel_error_paths_preserve_gold` (1/1), `fmt --check`, and full `mir2-simulation` 638/638.
- [x] 2026-04-26 `R144` backend Stage 5 command localization parity routed unknown Stage 5 commands through Crystal `server.InvalidPacketReceived`. Verified by `stage5_trade_shop_and_auction_cancel_error_paths_preserve_gold` (1/1), `fmt --check`, and full `mir2-simulation` 638/638.
- [x] 2026-04-26 `R145` backend map-transfer localization parity routed unknown map-transfer keys through Crystal `server.NotFound`. Verified by `transfer_map_requires_player_on_transfer_bounds` (1/1), `fmt --check`, and full `mir2-simulation` 638/638.
- [x] 2026-04-26 `R146` backend Stage 5 event localization parity routed missing player/position event-spawn rejections through Crystal `server.NotFound`. Verified by `stage5_conquest_event_hero_mining_and_crafting_flow` (1/1), `fmt --check`, and full `mir2-simulation` 638/638.
- [x] 2026-04-25 `R113` backend item-use packet-surface parity aligned static starter HP/MP potion use with Crystal normal-potion timed recovery. Successful use still consumes and acks immediately, but HP/MP restore is queued and emitted on follow-up ticks instead of mutating immediately. Verified by `crystal_use_item_packet_consumes_` (2/2), `use_item_packet_` (40/40), `consumable_item_restores_hp`, `fmt --check`, and full `mir2-simulation` 631/631.
- [x] 2026-04-25 `R112` backend item-use packet-surface parity removed runtime-only static `repair-powder` success/failure chat. Starter equipment repair use now preserves repair mutation and `ItemRepaired` packets without `sim.noEquipmentNeedsRepair` / `sim.repairedEquippedItems`. Verified by `repair_powder` (2/2), `use_item_packet_` (40/40), `fmt --check`, and full `mir2-simulation` 631/631.
- [x] 2026-04-25 `R111` backend item-use packet-surface parity removed runtime-only static `town-teleport` success chat. Successful static teleports now emit movement/location packets without `sim.townTeleportReturnedToSpawn`. Verified by `town_teleport` (3/3), `use_item_packet_` (40/40), `fmt --check`, `diff --check`, and full `mir2-simulation` 631/631.
- [x] 2026-04-25 `R110` backend item-use packet-surface parity removed hardcoded static `benediction-oil` no-weapon failure chat. Invalid luck attempts now fail without runtime-only chat or item consumption. Verified by `benediction_oil` (4/4), `use_item_packet_` (40/40), `fmt --check`, `diff --check`, and full `mir2-simulation` 631/631.
- [x] 2026-04-25 `R109` backend item packet-surface parity removed runtime-only `SplitItem` success chat. Inventory/storage split success now emits only Crystal-shaped `SplitItem1` plus `SplitItem`. Verified by `split_item_packet` (7/7), `storage` (43/43), `fmt --check`, `diff --check`, and full `mir2-simulation` 630/630.
- [x] 2026-04-25 `R108` backend item-use packet-surface parity aligned static `repair-oil` / `war-god-oil` with Crystal localized weapon-repair Hint success chat and no runtime-only no-repair failure chat. Verified by focused `repair_oil` (3/3), `use_item_packet_` (40/40), `fmt --check`, `diff --check`, and full `mir2-simulation` 630/630.
- [x] 2026-04-25 `R107` backend item packet-surface parity removed runtime-only `custom.itemDropped` from successful `DropItem`. Normal and split-stack inventory drops now success-ack with ground-object visibility and no generic success chat. Verified by `drop_item_packet` (10/10), `fmt --check`, `diff --check`, and full `mir2-simulation` 629/629.
- [x] 2026-04-25 `R106` backend item-use packet-surface parity removed runtime-only `sim.usedItem` from static HP/MP consumable success. Inventory/belt starter potions now heal, consume, and success-ack without chat. Verified by focused inventory/belt regressions, `use_item_packet_` (40/40), `fmt --check`, `diff --check`, and full `mir2-simulation` 629/629.
- [x] 2026-04-25 `R105` backend item packet-surface parity removed runtime-only `sim.itemNotFoundInBag` from missing-source `DropItem`. Absent inventory ids now failed-ack without chat or mutation. Verified by focused regression, `drop_item_packet` (10/10), `fmt --check`, `diff --check`, and full `mir2-simulation` 629/629.

- [x] `apps/web` production build passed with `npm.cmd run build`.
- [x] `http://127.0.0.1:3002` responded with HTTP 200.
- [x] gateway health responded with `{"ok":true,"http":"ready","ws":"ready","tcp_stub":"ready"}`.
- [x] representative map API calls returned real cells and sprites after warm cache for `0`, `1`, `2`, `n0`, `HF1`, `HF2`, `HF3`, `D1801`, and `HKR`.
- [x] Rust workspace regression is green. The previous `mir2-simulation` 13-test blocker was resolved on 2026-04-21.
- [x] Latest autonomous Stage 4 verification passed with `cargo test --workspace`, `cargo test --workspace -- --test-threads=1`, `npm.cmd run build`, `npm.cmd run smoke:crystal-minimap-assets`, and `npm.cmd run smoke:crystal-map-api` against `http://127.0.0.1:3004`.
- [x] Latest autonomous Stage 5 hardening pass added account-store backup/restore, disconnect persistence, socket-close save hooks, WebSocket/TCP session panic boundaries, reproducible packet traces, and a Chrome CDP UI smoke that archives login/select/game/inventory/character/storage/NPC/combat/map-transfer screenshots with zero critical console errors.
- [x] 2026-04-22 Stage 5 broad-system pass added persisted runtime state, gateway commands, snapshot exposure, and automated tests for group/guild/social/mail, trade/shop/auction, conquest/events, hero, mining, and crafting.
- [x] 2026-04-22 NPC command-surface pass regenerated Crystal NPC diagnostics at 81/81 command names and 7,044/7,044 command occurrences covered by the current Rust baseline.
- [x] 2026-04-22 packet trace harness can capture local TCP gateway traces and optionally diff live Crystal traces when `MIR2_CRYSTAL_TCP_ADDR` is set; current local baseline captured 16 decoded entries.
- [x] 2026-04-22 real gateway load harnesses passed against the running process: WebSocket 64/64 ready, 0 errors, 1,293 messages, 3,072 commands; TCP 64/64 ready, 0 failures, 656 packets, 0 decode errors.
- [x] 2026-04-22 Crystal `MonsterInfo.DropPath` now resolves imported drop tables for current runtime death and harvest rewards, with verified OmaFighter gold, Hen chicken, and Deer venison paths.
- [x] 2026-04-22 imported Crystal gold drops now use Crystal's `base/2 .. base+base/2` amount range instead of fixed table values.
- [x] 2026-04-22 harvest rewards now emit Crystal `GainedItem` packets for transferred items, verified on Training Dummy, Hen, and Deer harvest paths.
- [x] 2026-04-22 imported Crystal item drops now carry base durability into `UserItem`, and harvest meat applies Deer quality durability bonuses.
- [x] 2026-04-22 monster death drops now carry Crystal-style owner pickup windows with group-member bypass and expiry.
- [x] 2026-04-22 `ShowGroupPickup` item pickups now emit Crystal-style group pickup system notices.
- [x] 2026-04-22 ground-drop pickup and harvest transfer now follow Crystal slot/stack gain checks: bag weight refreshes after gain and does not block pickup/harvest acceptance.
- [x] 2026-04-22 imported Crystal item drops now use Crystal `CreateDropItem` current-durability roll before harvest meat quality and future random-stat upgrades.
- [x] 2026-04-22 manifest-backed `UserItem` payloads now set `Identified` from Crystal `NeedIdentify`, including current pickup and harvest rewards.
- [x] 2026-04-22 player `PickUp` now matches Crystal current-cell pickup semantics instead of collecting adjacent ground drops.
- [x] 2026-04-22 ground drops now expire using Crystal `ItemTimeOut=30` minute semantics and emit removal through normal AOI finalization.
- [x] 2026-04-22 monster gold drops now split into Crystal `MaxDropGold=2000` ground chunks before pickup.
- [x] 2026-04-22 gold pickup now respects Crystal `CanGainGold` `uint.MaxValue` cap and preserves ground gold when full.
- [x] 2026-04-22 player `DropGold` now mirrors Crystal zero-gold and insufficient-gold packet behavior.
- [x] 2026-04-22 ground `ObjectItem` now uses imported Crystal item `Grade` and grade name-colour mapping for manifest-backed drops.
- [x] 2026-04-22 player `DropItem` now mirrors Crystal stack-count splitting plus `DontDrop` and `DestroyOnDrop` bind semantics for current inventory items.
- [x] 2026-04-22 Crystal `AddItem` now prioritizes player potion/scroll/script and amulet belt slots before bag fallback, and belt `UseItem` consumes the belt slot instead of same-key inventory items.
- [x] 2026-04-22 ground drop placement now follows Crystal `ItemObject.Drop(distance)` ring search, skips transfer source tiles, caps same-cell item objects at `DropStackSize=5`, and uses Crystal player item/gold and monster drop ranges.
- [x] 2026-04-22 Crystal quest-drop `Q` entries now roll normally, route into active matching quest inventory, suppress ground fallback, and preserve full quest-inventory failures.
- [x] 2026-04-22 frontend shell first patch landed: login inputs submit on Enter and scene tile hit buttons stop pointer bubbling to avoid double-dispatch while preserving empty-space scene clicks.
- [x] 2026-04-22 current Crystal random-stat drop baseline now rolls MaxDura, MaxAC, and MaxDC from `random_stats_id` profiles and preserves the resulting added stats through pickup/harvest `GainedItem` payloads.
- [x] 2026-04-22 current added-stat ground drops now expose Crystal Cyan item-name colour through backend packets, world snapshots, and web ground-drop labels.
- [x] 2026-04-22 NPC buy-back entries now persist across save/reload, expire into NPC used goods after Crystal `GoodsBuyBackTime`, and used goods can be purchased back.
- [x] 2026-04-22 current socket-slot growth now rejects items at imported socket capacity and only emits `ItemSlotSizeChanged` on successful capacity-backed growth.
- [x] 2026-04-22 current seal flow now rejects already-sealed equipment without overwriting expiry and only emits `ItemSealChanged` on first active seal.
- [x] 2026-04-22 current BenedictionOil can add Luck, curse the weapon with negative Luck, or consume with no effect using Crystal-shaped branch rules.
- [x] 2026-04-22 frontend selected scene targets now route keyboard approach and primary actions through existing runtime handlers with localized action/distance feedback.
- [x] 2026-04-22 current seal flow validates optional source items against Crystal `Gem` shape-8 seal-source rules and consumes the source only on successful sealing.
- [x] 2026-04-22 current socket-slot growth validates optional source items against Crystal `Gem` shape-7 socket-source and `ValidGemForItem` unique-flag rules before consuming the source.
- [x] 2026-04-22 current seal flow stores and serializes Crystal `SealedInfo.NextSealDate`, rejects reseal before `Settings.ItemSealDelay=60` minutes after expiry, and preserves the metadata through save/reload with legacy defaults.
- [x] 2026-04-22 current Crystal random-stat drops now carry full current Jev profile-family metadata: generic `UserItemStat` entries, curse flag, and socket slots survive drop resolution, pickup/harvest `GainedItem`, inventory/equipment state, and save/reload.
- [x] 2026-04-22 generated `RandomItemStats.ini` manifest data now drives Crystal random-stat profile lookup, removing the remaining hardcoded runtime profile table while preserving the full random-stat payload behavior.
- [x] 2026-04-22 generated drop manifests now preserve nested Crystal `GROUP` trees, and runtime applies `GROUP*` random-one-item, `GROUP^` first-success, child gold accumulation, and nested group composition.
- [x] 2026-04-22 Crystal source audit confirmed owned item/gold drops are visible immediately; current `PickUp` now scans the current cell, skips owner-blocked/full-bag/gold-cap candidates when a later drop is pickable, and emits the owner warning only when no later pickable candidate exists.
- [x] 2026-04-22 Crystal `HarvestMonster` now generates pending `_drops` after the skin count reaches zero, transfers on the next harvest call, preserves leftovers across full-bag retries, and avoids re-rolling pending harvest rewards.
- [x] 2026-04-22 Crystal harvest corpse owner/EXPOwner scanning now skips non-owner/non-group corpses, allows grouped owners, and emits `NoNearbyOwnedCarcasses` only when no eligible corpse is found.
- [x] 2026-04-22 current `SellItem` now requires an active Crystal sell service and rejects partial-stack sales that would overflow the Crystal gold cap.
- [x] 2026-04-22 current credit-shop purchases now follow Crystal game-shop mailbox delivery: `LoseCredit` is emitted after balance checks, items are mailed, and full bags block only mail attachment claim.
- [x] 2026-04-22 current `BuyItem` now silently rejects invalid panel/count, missing active service, non-buy service pages, missing goods/metadata, insufficient gold, and full bags before mutation.
- [x] 2026-04-23 current NPC `RepairItem` / `SRepairItem` now follow Crystal backpack unique-id lookup, active `@Repair` / `@SRepair` service gating, repair/special-repair costs, normal max-dura loss, and repairability/type/insufficient-gold rejection behavior.
- [x] 2026-04-23 current NPC `SellItem` now follows Crystal `DontSell`, script `[Types]`, ack-only failure, `UserItem.Price() / 2`, partial-stack overflow rejection, and full-stack gold-cap clamping semantics.
- [x] 2026-04-23 current NPC `StoreItem` / `TakeBackItem` now follow Crystal active `@Storage` / `NPCStorage` service gating, `DontStore` store-only rejection, accessible storage capacity, occupied-target no-swap behavior, and ack-only failure semantics.
- [x] 2026-04-23 current inventory-grid `CombineItem` packets now follow Crystal client/server ids and payloads for the currently modeled shape-7 socket-growth and shape-8 seal branches, including gateway JSON exposure, runtime dispatch, and persisted seal metadata flow-through; full target-type, hero-inventory, and other combine branches remain queued.
- [x] 2026-04-23 current inventory-grid `CombineItem` now also follows the bounded Crystal shape-3/4 gem/orb upgrade branch, including `ItemUpgraded`, persisted `gem_count`, max-added-stat rejection, and destroy-on-failure handling.
- [x] 2026-04-23 current inventory-grid `CombineItem` now also applies Crystal's shared target `ItemType` gate across socket/seal/upgrade packet branches, so non-equipment targets ack-fail before branch-specific hints or mutations.
- [x] 2026-04-23 current inventory-grid `CombineItem` now also follows Crystal repair-hammer/sewing source shapes `1/2/5/6`, including hammer-vs-sewing target-family gating, `ItemNoRepairNeeded`, `ItemRepaired`, and success-path durability mutation.
- [x] 2026-04-23 current item runtime state now preserves rental `BindingFlags` into `UserItem.RentalInformation`; storage rejects rental `DontStore`, and current socket/upgrade `CombineItem` branches reject rental `DontUpgrade` ack-only.
- [x] 2026-04-23 current inventory-grid `CombineItem` shape-3/4 upgrade success chance now applies equipment-backed player `GemRatePercent`, matching Crystal's `Stats[Stat.GemRatePercent]` hook.
- [x] 2026-04-23 current bag-item `CombineItem` / `SplitItem` / `DeleteItem` / `DropItem` / `SellItem` / `RepairItem` lookup now follows Crystal inventory unique ids instead of slot aliases, and default `Bag1` / `Bag2` same-slot items no longer collide in runtime fallback ids.
- [x] 2026-04-23 current packet `UseItem` / `EquipItem` / `MergeItem` now resolve current bag items by Crystal unique ids instead of duplicate-key fallback or slot aliases.
- [x] 2026-04-23 current `DeleteItem` now ignores packet `HeroInventory` like Crystal's server and still searches only current player inventory by unique id, deleting matching bag items while leaving missing hero/player ids ack-only.
- [x] 2026-04-23 bounded current hero-inventory packet guards are now regression-locked for `DropItem(hero_inventory=true)` and `CombineItem(grid=HeroInventory)`: when hero inventory is unavailable or unmodeled, both paths ack-fail without mutating matching player inventory.
- [x] 2026-04-23 current `DropItem` now also rejects rental `BindingFlags.DontDrop` ack-only like Crystal, preserving inventory state and rental metadata while spawning no ground drop.
- [x] 2026-04-24 current dead-state item mutation parity now short-circuits `BuyItem`, `DeleteItem`, `SellItem`, `RepairItem`, `DropItem`, and `CombineItem` without mutation.
- [x] 2026-04-24 current `UseItem` now matches the bounded Crystal dead-state / `ResurrectionScroll` behavior, including alive `CannotResurrection`, dead-player revive-on-use, and dead-player `ResurrectionScroll` rejection on maps flagged `NoReincarnation`.
- [x] 2026-04-24 current `TownTeleport` now respects map `CurrentMap.Info.NoTownTeleport`, emits `NoTownTeleport`, preserves the item, and suppresses teleport on blocked maps.
- [x] 2026-04-24 current `UseItem(grid=HeroInventory)` no longer falls back into player bag items while hero inventory is unmodeled.
- [x] 2026-04-24 current `SplitItem(grid=HeroInventory)` now failed-acks without mutating matching player bag stacks while hero inventory is unmodeled.
- [x] 2026-04-24 current `EquipItem(grid=HeroInventory)`, `RemoveItem(grid=HeroInventory)`, and `RemoveSlotItem(grid=HeroEquipment|HeroInventory)` now failed-ack without mutating matching player inventory/equipment while hero grids are unmodeled.
- [x] 2026-04-24 current `MergeItem` hero-grid requests now failed-ack without extra chat or player-bag mutation while hero inventory/equipment are unmodeled.
- [x] 2026-04-24 current `MoveItem` unsupported-grid parity now covers `HeroInventory`, `Trade`, and `Refine` ack-only failures without extra chat or player-bag mutation.
- [x] 2026-04-24 current `MergeItem` now supports modeled `Inventory <-> Belt` stack merges for Crystal belt-eligible items, with ack-only non-beltable belt cross-grid failures.
- [x] 2026-04-24 current `MoveItem` unsupported-grid parity now also covers `HeroEquipment`, `Equipment`, and `Fishing` ack-only failures without extra chat or player/equipment mutation.
- [x] 2026-04-24 current `MoveItem` storage-lock and invalid-slot failures now ack-fail without extra chat, matching Crystal's message shape more closely.
- [x] 2026-04-24 current `MoveItem(grid=Storage)` now requires active `@Storage` / `NPCStorage` service context, keeping inactive-service failures ack-only.
- [x] 2026-04-24 current successful `MoveItem` current `Inventory` / `Storage` paths now no longer emit runtime-only `Item slot updated.` chat.
- [x] 2026-04-24 slot-based current `MoveItem`, `StoreItem`, and `TakeBackItem` inventory paths now resolve Crystal single-array indices across local `Bag1` / `Bag2`, including `Bag2` swaps and storage transfers on slots `40+`.
- [x] 2026-04-24 current `SplitItem(grid=Inventory)` now follows Crystal single-array placement across local `Bag1` / `Bag2`, including belt-first placement for belt-eligible items instead of source-container page scoping.
- [x] 2026-04-24 current `SplitItem` now matches Crystal's supported-grid and failed-ack surface: only `Inventory` / `Storage` are live, storage splits require active Crystal storage service, and unsupported/invalid/full/locked failures stay ack-only.
- [x] 2026-04-25 current storage-family item actions now require the recorded Crystal storage NPC object to still exist and remain within `CRYSTAL_DATA_RANGE`, so stale/out-of-range storage service context now ack-fails across `StoreItem`, `TakeBackItem`, `MoveItem(grid=Storage)`, `SplitItem(grid=Storage)`, and any `MergeItem` touching `Storage`.
- [x] 2026-04-25 current `BuyItem`, `SellItem`, and `RepairItem`/`SRepairItem` now require the recorded Crystal NPC object to still exist and remain within `CRYSTAL_DATA_RANGE`, so stale/out-of-range NPC service context no longer mutates the implemented current buy/sell/repair item surfaces.
- [x] 2026-04-25 current inventory-grid `CombineItem` now routes current-data `DurabilityGem` / `DurabilityOrb` through Crystal's `MaxDura` branch instead of misusing stat `48` as the applied upgrade stat, and focused regressions now lock the current-data durability, attack-speed, magic-resist, and durability-cap surfaces.
- [x] 2026-04-25 current inventory-grid `CombineItem` current-data coverage now closes the remaining present-data shape-3/4 families and the shape-0 ack-only source surface for the current manifest slice.
- [x] 2026-04-25 current equipment/item metadata now preserves Crystal `NeedIdentify` and `SoulBoundId` through runtime/item payload round-trips, auto-identifies items on equip/use-equip, and rejects equipping items soul-bound to another character.
- [x] 2026-04-25 dynamic manifest-backed current-data `UseItem` now routes Crystal `SunPotion`, duration buffs, `TownTeleport`, `BenedictionOil`, `RepairOil`, and `WarGodOil` through template stats and scroll shapes, including same-key buff duration stacking and the current bounded `WarGodOil` shape-0 fallback.
- [x] 2026-04-25 current storage password actions now require the active in-range Crystal storage service context, and successful password removal clears the persisted `LastSetTime` back to `0`.
- [x] 2026-04-25 current storage password set/unlock/remove now enforce Crystal's `^[A-Za-z0-9]{5,15}$` password format semantics.
- [x] 2026-04-25 reopening Crystal `@Storage` now resets the session unlock state before deciding whether storage contents can be sent, matching `ResetStorageUnlock()`.
- [x] 2026-04-25 successful current `@Storage` open now emits Crystal `UserStorage` before `NPCStorage` when storage is available, and successful `UnlockStorage` now emits `StorageUnlockResult` followed by `UserStorage`.
- [x] 2026-04-25 repeated unchanged current `@Storage` opens now suppress duplicate `UserStorage` after the first send, matching Crystal `Connection.StorageSent` resend behavior while preserving the locked reopen/unlock resend path.
- [x] 2026-04-25 current `@Storage` open now sends Crystal `UserStorage` with the full backing storage length even when expanded storage is inactive, while higher-slot storage actions remain gated by current accessible capacity.
- [x] 2026-04-25 expired expanded storage now downgrades inactive on current `StartGame`, then emits Crystal-style expiry chat plus `ResizeStorage` on the first world tick and persists the account flag back to `false` while preserving the 160-slot backing array.
- [x] 2026-04-25 current `EquipItem(grid=Storage)` now resolves the exact storage item through the active `@Storage` service, and current `RemoveItem(grid=Inventory|Storage)` now follows Crystal's exact destination-slot semantics with ack-only packet shape instead of accepting `grid=Equipment` or falling back into another bag slot.
- [x] 2026-04-25 current `RemoveSlotItem` now keeps Crystal's bounded source-grid envelope for the modeled runtime: invalid `grid=Equipment` requests and unmodeled `Mount` / `Fishing` / `Socket` slot-item requests ack-fail without falling through into whole-equipment removal, including socket requests that only match the parent equipment id.
- [x] 2026-05-27 frontend/resource-loading hardening evidence: Player Web Crystal resource loading now parses `.Lib` sources index-only, lazily decodes requested frames through a byte-capped LRU, caps server map/library caches, blocks production request-time original-map writes and synthetic map fallback unless explicitly opted in, returns visible `resource_missing` for missing required resources, quantizes scene blueprint cache keys with disk TTL/size trim, preloads visible scene asset URLs before visual readiness, and exposes cache/readiness metrics for scene cache key, original-map sprite/cell counts, sprite-library count, DOM image count, Bevy atlas bytes, and alpha-keyed blob bytes. Verified by `MIR2_CANDIDATE_SCOPE=local bash infra/check-candidate-gate.sh`, including Web typecheck, movement-controller tests, resource-loading tests, focused Rust gateway/simulation/admin gates, and `git diff --check`; production browser acceptance remains open.
- [x] 2026-04-24 current `MergeItem` unsupported-grid parity now also covers `Equipment`, `Fishing`, `Trade`, and `Refine` ack-only failures without extra chat or player-bag mutation.
- [x] 2026-04-24 current `MergeItem` same-grid Inventory/Storage failure and success message shape now follows Crystal's ack-only surface, removing runtime-only chat for storage-lock, missing-item, mismatched/full-stack, and success paths.
- [x] 2026-04-24 current `MergeItem` now supports Crystal-style `Inventory <-> Storage` stack merges behind the active storage-service gate, with ack-only inactive/locked failures.

## Progress Summary

Backend/server parity status: **100% Accepted for the tracked backend/server slice under explicit stable-diff packet acceptance**. This is not the whole-project score; it only covers the tracked Rust backend, gateway, protocol, persistence, and server gameplay behavior.

Full project 1:1 estimate: **roughly 90.0%**. This replaces the older rough 45% architecture-only estimate by explicitly scoring frontend, resources, integration, and playable operations in addition to backend parity.

| Project Area | Weight | Current | Weighted Progress | Notes |
| --- | ---: | ---: | ---: | --- |
| Backend/server behavior parity | 45% | 100% Accepted for tracked slice | 45.0% | Current backend tracker is green through the imported gameplay/server slice and R300 closes the current live packet gate under explicit stable-diff acceptance. Strict exact packet comparison remains diagnostic until deterministic Crystal volatile-state fixtures exist; untracked Crystal branches and product-evolution systems are outside this tracked-slice score. |
| Frontend/client UI and interaction parity | 25% | 88.0% | 22.0% | Web client builds and smoke tests pass, map/UI pipeline exists, current gameplay panels are usable, and R301 Stage 5 UI smoke covers 88 screenshots across login/select lifecycle, compact layout/text/multi-panel bounds, minimap, belt, chat/channel filters, storage, storage password panel entry/mismatch/no-service submit, system menu QA/transfer routes, Mail/Report/NPC panels with link-capable rendering, character repair/special-repair/remove/spell-cast flows, HUD buttons, inventory use/equip/gold/move/split/drop/sell, ground item/gold pickup, Store Item no-service preservation, and Take Back no-service preservation evidence; exact Crystal visual layout, animation timing, sound/effects, service-backed economy/storage flows, and pixel-level UI parity are still incomplete. |
| Crystal assets and data coverage | 15% | 72% | 10.8% | Map, minimap, item, monster, NPC, drop, magic, and buff manifests are imported for major backend use; map API and minimap smoke evidence is now archived under `docs/generated/map` and `docs/generated/assets`; full asset coverage, all visual resources, event bindings, weather/light/fire metadata, and remaining economy tables are not complete. |
| End-to-end integration and live parity harness | 10% | 80% | 8.0% | Local packet trace, load, smoke, gateway harnesses, route screenshots, login/select lifecycle, map API/minimap JSON evidence, ground pickup, menu transfer, spell-cast, and broad Stage 5 systems manifest evidence exist; R298/R300 provide accepted stable-diff live Crystal packet evidence for the tracked matrix, and R301 refreshes map/minimap/load/UI automation, while human visual/feel QA and deeper late-game route acceptance are still missing. |
| Playability, operations, and hardening | 5% | 88% | 4.4% | Save/reload, reconnect, load harnesses, smoke route stability, broad-system baselines, compact multi-panel layout checks, and refreshed R301 WS load 64/64 evidence exist; long-duration production acceptance, telemetry/rollback readiness, and late-game multi-system playability remain open. |
| **Total** | **100%** |  | **roughly 90.0%** | Whole-project estimate, not interchangeable with the backend tracked-slice 100% Accepted packet status. |

Stage execution summary:

| Stage | Target | Status | Estimated Completion |
| --- | --- | --- | ---: |
| 1 | Restore regression baseline | Complete | 100% |
| 2 | Finish map/UI/data pipeline | Complete | 100% |
| 3 | Expand current playable server slice | Complete | 100% |
| 4 | Broaden Crystal system parity | In progress | 65% |
| 5 | Full 1:1 hardening and production parity | In progress | 60% |

## 100% Completion Closure Roadmap

Goal: move the overall full 1:1 estimate from roughly 62.2% to 100% without losing the current green migrated slice.

Execution rule: continue autonomously through the highest-priority unchecked item. Do not pause for confirmation for normal implementation, testing, documentation updates, local service starts, generated data refreshes, or focused refactors needed by an unchecked item. Only stop for explicit destructive operations, missing credentials or private endpoints, unavailable Crystal source/assets, or a product-scope decision that cannot be inferred from Crystal behavior.

Progress gates:

| Gate | Target State | Completion Meaning |
| --- | --- | --- |
| 55% | Live parity harness accepted | Local traces, Crystal traces, and diff reports exist for representative flows. |
| 65% | High-impact spawned AI cleared | Highest-spawn generic AI families have Crystal-specific behavior and regression coverage. |
| 75% | Full combat / skill / item foundations | Spell, buff, projectile, item roll, durability, repair, and economy data are table-driven. |
| 85% | Broad system behavior parity | NPC, quest, map events, social, guild, trade, auction, conquest, hero, mining, and crafting match representative Crystal flows. |
| 92% | Packet-visible parity matrix green | Login through late-game representative flows pass local-vs-Crystal packet and behavior comparison. |
| 97% | Production hardening accepted | Long soak, high-concurrency load, persistence recovery, telemetry, and rollback evidence are green. |
| 100% | No open blocker gaps | All full 1:1 gates are checked, remaining deviations are either zero or explicitly approved as out of scope. |

### A. Parity Measurement Harness

- [x] Define the full packet / behavior parity matrix for login, account, character select, start game, movement, chat, combat, NPC, inventory, storage, item use, map transfer, death/revive, skills, summons, trade, shop, auction, guild, mail, conquest, hero, mining, and crafting.
- [x] Make `MIR2_CRYSTAL_TCP_ADDR` live-capture runs reproducible with stable account fixtures, character fixtures, and reset instructions.
- [x] Extend `apps/gateway/src/bin/packet_trace.rs` so each TCP-traceable matrix flow can capture local and Crystal traces into separate JSON artifacts.
- [x] Add a diff reporter that separates packet id mismatch, packet order mismatch, payload mismatch, timing tolerance, and known nondeterministic fields.
- [x] Add CI/local commands that fail when an accepted representative flow regresses.
- [ ] Check the 55% gate only when representative live Crystal traces are captured/reviewed or explicitly accepted under the stable-diff policy for the core flows.

### B. Monster AI Full Parity

- [x] Regenerate `crystal_monster_ai_summary.json` and sort remaining `generic_baseline` spawned families by respawn count, map importance, and player-facing risk.
- [x] Implement Crystal-specific behavior for high-count generic families first, including CaveMaggot, HarvestMonster, FlamingWooma, WoomaTaurus, ToxicGhoul, ThunderElement, Tucson, Frozen, Snow, Jar, and common dungeon families.
  - [x] AI 7 `CaveMaggot`: Crystal melee timing, DC-based damage, 1/20 five-second paralysis movement stop, and HarvestMonster two-pass corpse harvest baseline.
  - [x] AI 28 `ToxicGhoul`: Crystal melee timing, DC-based damage, 1/8 five-second green-poison status, and HarvestMonster two-pass corpse harvest baseline.
  - [x] AI 49 `ThunderElement`: two-tile CompleteAttack baseline, due-time `ObjectAttack`, DC damage, random near-target repositioning, opposing-target fanout, normal-damage immunity, and player `Repulsion` / `EnergyRepulsor` / `FireBurst` push-damage with `ObjectPushed`.
  - [x] AI 9 `HarvestMonster`: harvest/drop corpse semantics.
  - [x] AI 112 `DarkBeast`: primary DC melee plus secondary type-1/bleeding hook with current CatWidow data gating.
  - [x] AI 10 `FlamingWooma`: Crystal 300 ms melee timing, `ObjectAttack`, and imported DC-based damage baseline.
  - [x] AI 11 `WoomaTaurus`: FlamingWooma melee, HP-threshold mad speed phase, surrounded teleport baseline, and `ObjectTeleportOut` / `ObjectTeleportIn` effect packets.
  - [x] AI 16 `RedThunderZuma`: Zuma-style stone/wake state, nine-tile range, non-adjacent `ObjectRangeAttack`, fixed ranged delay, and DC damage gating.
  - [x] AI 17 `ZumaTaurus`: Zuma-style stone/wake state, adjacent `ObjectAttack` melee with imported DC damage, and seven-stage HP slave waves using Crystal's Zuma minion set and 8/40 caps.
  - [x] AI 20 `DarkDevil`: three-tile `ObjectRangeAttack`, 500 ms imported DC*3 damage, 2-4 second cooldown, and forward one-tile fanout.
  - [x] AI 19 `KingScorpion`: two-tile line/range MC branch, line fanout, adjacent DC fallback, and random/second-tile range override.
  - [x] AI 22 `IncarnatedZT`: active non-stoned `ObjectAttack` melee branch with 300 ms delayed imported DC damage and paralysis chance.
  - [x] AI 34 `FrostTiger`: passive-until-targeted acquisition, six-tile range, non-adjacent `ObjectRangeAttack`, distance-scaled ranged delay, DC damage gating, ranged bleeding/slow poison rolls, and `ObjectSitDown` sitting/standing presentation.
  - [x] AI 102 `IceGuard`: eight-tile near/ranged switching, fixed ranged delay, imported MC ranged damage, DC melee gating, fire type-1 range branch, and ice slow/frozen poison rolls.
  - [x] AI 187 `FrozenMiner`: primary `ObjectAttack` branch with Crystal 600 ms delayed imported DC damage plus type-1 80% DC area branch and adjacent opposing-monster fanout.
  - [x] AI 188 `FrozenAxeman`: two-tile line/diagonal type-1 `ObjectAttack` branch with Crystal 500 ms delayed DC*2 damage plus adjacent type-2 pull/push branch.
  - [x] AI 189 `FrozenMagician`: nine-tile non-adjacent `ObjectRangeAttack` type-0 imported MC branch and type-1 boosted MC*3/2 branch.
  - [x] AI 179 `SnowWolf`: primary `ObjectAttack` branch with Crystal 350 ms delayed imported DC damage, type-1 MC/Slow/Frozen branch, and `FindAllTargets(2)` fanout.
  - [x] AI 180 `SnowWolfKing` / `FrozenWarewolf`: type-0/type-1/type-2/type-3 `ObjectAttack` branches with 500 ms delayed imported DC damage, below-70% SnowWolf slave spawn, and delayed one-tile death explosion.
  - [x] AI 126 `TucsonMage`: three-tile type-1 `ObjectAttack` WideLine branch with zero-MC no-damage gating, adjacent branch selection, and multi-target fanout.
  - [x] AI 127 `TucsonWarrior`: two-tile reach, non-adjacent/adjacent type-1 MC smash, adjacent halfmoon DC branch, and target-area fanout.
  - [x] AI 190 `SnowYeti`: nine-tile non-adjacent `ObjectRangeAttack` branch with distance-scaled delayed timing, imported DC damage, and frozen poison roll plus adjacent type-0/type-1 double-hit melee branch.
  - [x] AI 192 `DarkWraith`: four-tile type-2 line attack with DC*3 damage, line fanout, cooldown, plus adjacent type-1 area fanout branch.
  - [x] AI 128 `TucsonEgg`: immobile/no-attack egg baseline, fixed one-HP damage intake, and delayed death poison/spawn hook.
  - [x] AI 3 `Tree`: static neutral/passive tree object baseline with fixed one-HP damage intake.
  - [x] AI 51 `HedgeKekTal`: eight-tile near-vs-range switching, `ObjectRangeAttack`, and imported DC-based damage baseline.
  - [x] AI 56 `Trainer`: static passive training target baseline with no attack, movement, HP loss, death, plus `ChatType.Trainer` damage/DPS and idle average reporting.
  - [x] AI 130 `CannibalTentacles`: view-range non-adjacent `ObjectRangeAttack`, imported MC damage, adjacent type-1 halfmoon, green poison, and arc fanout.
  - [x] AI 119 `Jar1`: static one-tile baseline and delayed regular-monster slave spawn on death.
  - [x] AI 120 `Jar2`: static six-tile `ObjectRangeAttack`, adjacent random DC melee/range split, zero-MC no-damage gating, and Frozen poison hook.
  - [x] AI 173 `TurtleGrass`: Zuma-style stone/wake state, two-tile attack shape, imported DC damage, and type-1 single-push branch.
  - [x] AI 174 `ManTree`: Zuma-style stone/wake state, type-0/type-1/type-2 `ObjectAttack` packet branches, and data-gated boulder Stun hook.
  - [x] AI 35 `SandWorm`: SpittingSpider-style two-tile line `ObjectAttack`, delayed DC hit, forward-line fanout, and harvest-corpse baseline.
  - [x] AI 37 `CrystalSpider`: three-tile row/column/diagonal type-1 `ObjectAttack`, distance-delayed DC hit, forward-line fanout, and green poison baseline.
  - [x] AI 30 `BoneLord`: seven-tile `ObjectRangeAttack` branch with distance-delayed imported DC damage, plus HP-stage type-1 slave waves using Crystal's bone minion set and 8/40 caps.
  - [x] AI 33 `MinotaurKing`: six-tile RightGuard-derived `ObjectRangeAttack`, 500 ms imported DC damage, and three-tile target fanout.
  - [x] AI 86 `ManectricClaw` / `Chieftain_Priest`: random pre-thrust movement, three-tile `ObjectRangeAttack` thrust baseline with near-DC / far-MC damage, player slow/frozen poison rolls, and cone fanout.
  - [x] AI 54 `DragonStatue` / `MirStatue`: static delayed `ObjectRangeAttack` baseline with imported DC damage, target-radius fanout, lethal-damage sleep, sleeping immunity, and full-HP wake.
  - [x] AI 48 `GuardianRock`: static immune delayed range-pull packet baseline with no direct damage and capped pull movement.
  - [x] AI 13 `RedMoonEvil`: static view-range `ObjectAttack`, delayed imported DC damage, multi-target fanout, and `ObjectEffect RedMoonEvil` target broadcasts.
  - [x] AI 14 `EvilCentipede`: hidden reveal/hide baseline, static view-range `ObjectAttack`, imported DC damage, fanout, and green/paralysis poison.
  - [x] AI 36 `Yimoogi`: seven-tile `ObjectRangeAttack`, 500 ms imported DC damage, four-tile type-1 red-poison branch, four-second sister child spawn, final low-HP teleport with two `WhiteSerpent` spawns, and paired drop suppression.
  - [x] AI 186 `Kirin` / `Lamia`: two-tile row/diagonal `ObjectAttack` baseline, type-1 500 ms branch with imported DC damage, and nonzero-MC type-2 IceThrust cone with slow poison and opposing-target fanout.
  - [x] AI 27 `Khazard`: four-tile line/diagonal `ObjectRangeAttack` pull branch with no direct damage, pull movement, and 5s cooldown.
  - [x] AI 115 `SandSnail`: primary type-0 DC attack, type-1 halfmoon fanout, and type-2 MC Green-poison area branch.
  - [x] AI 1 `Deer`: Hen/Pig/Bull passive no-player-target baseline and Crystal two-pass harvest skin count.
  - [x] AI 2 `Deer`: passive no-player-target baseline, Crystal five-pass harvest skin count, and run-away flee movement.
  - [x] AI 38 `HolyDeva`: six-tile `ObjectRangeAttack`, summoned `extra`, delayed imported DC damage, and Crystal fear-window kiting movement.
  - [x] AI 43 `OmaKing`: seven-tile type-1 magic branch, close push/paralysis handling, and two-tile line fanout.
  - [x] AI 50 `GreatFoxSpirit`: static seven-tile `ObjectRangeAttack`, 300 ms imported DC damage, `FindAllTargets` fanout, `ObjectEffect GreatFoxSpirit`, slow/paralysis poison, HP-stage `extra_byte` broadcasts, nearby `GuardianRock` activation/deactivation, and far-target recall movement with `ObjectTeleportOut` / `ObjectTeleportIn` effect 11.
  - [x] AI 88 `ManectricKing` / `Master_DragonYang`: three-tile line/diagonal `ObjectAttack` magic branch, close type-1 DC push line, and low-HP seven-tile mass attack.
  - [x] AI 121 `SeedingsGeneral`: two-tile `ObjectRangeAttack` magic branch with 300 ms delayed imported MC damage, Echo slow poison, Stomp frozen AOE fanout, and close mixed DC/MC melee branches.
  - [x] AI 122 `RestlessJar`: static six-tile range packet, Crystal ProjectileAttack distance*50+500ms timing, zero-MC gating, adjacent spin fanout, tornado/blindness, and low-HP stomp push/fanout.
  - [x] AI 79 `HellKeeper`: static view-range locked-facing `ObjectAttack` branches with type-0 DC, type-1 MC/Dazed gating, and `FindAllTargets` fanout.
  - [x] AI 123 `GeneralMeowMeow`: twelve-tile `ObjectRangeAttack`, 500 ms imported MC damage, two-tile target-area fanout, close type-1 triple-DC slam, HP shield windows, `ObjectSpell` mass thunder, and periodic Crystal cat-minion slave spawning.
  - [x] AI 131 `TucsonGeneral`: opening rage `ObjectRangeAttack` packet, delayed `TucsonGeneralRock` `ObjectSpell` scatter/impact pass, type-1/type-2 ranged branches, and close MC stomp/paralysis area hit.
  - [x] AI 47 `TrapRock`: hidden reveal, deterministic `SpawnCorner` target teleport, child rocks, reveal paralysis, parent no-damage `ObjectRangeAttack`, child `ObjectAttack`, target-move death, first-hit collapse, and repeated parent-attack paralysis roll.
  - [x] AI 124 `Armadillo`: DigOut-style hidden reveal, `DigOutArmadillo` `ObjectSpell`, primary DC `ObjectAttack`, type-1 three-hit combo branch, retreat `ObjectBackStep`, retreat radius damage, and run-away after failed retreat damage.
  - [x] AI 125 `ArmadilloElder`: DigOut-style hidden reveal, `DigOutArmadillo` `ObjectSpell`, primary DC*2 `ObjectAttack`, type-1 two-tile push/no-direct-damage branch, retreat `ObjectBackStep`, and run-away movement.
  - [x] AI 0 `MonsterObject`: default imported template movement/chase/melee/respawn/drop/packet baseline.
- [ ] Replace remaining wildlife/harvest partial behavior with Crystal-accurate passive, harvest, loot, blocking, flee, and guard-ignore semantics.
- [ ] Implement remaining boss and elite AI families, including spawn phases, summons, area attacks, immunity states, teleport, fear, poison, slow, push, and scripted transitions.
- [ ] Add focused simulation tests for every AI family promoted out of `generic_baseline`.
- [x] Update generated AI summary after each pass and check this section only when spawned `generic_baseline` families are zero or explicitly scoped out.

### C. Combat, Skills, Buffs, and Projectiles

- [ ] Import and validate the complete Crystal magic, buff, poison, projectile, cooldown, level requirement, MP cost, and class restriction data.
- [ ] Implement warrior skill parity, including hit shape, delay, durability, target rules, and packet-visible animation behavior.
- [ ] Implement wizard skill parity, including projectile lifecycle, area damage, element/status effects, and map-object interactions.
- [ ] Implement taoist skill parity, including healing, poison, summons, buffs, pets, and corpse/target constraints.
- [ ] Implement assassin and archer skill parity for the Crystal classes represented by the current client assets.
- [ ] Add packet and behavior tests for cast failure, cooldown, insufficient MP, invalid targets, safe zone restrictions, death state, and reconnect persistence.
- [ ] Check the combat/skill gate only when representative skill traces match Crystal packet-visible behavior.

### D. Items, Economy, Drops, and Storage

- [x] Import Crystal item DB manifest with item index, type, grade, stack, price, durability, restrictions, bind/unique flags, random-stat id, slots, stats, tooltip, and lookup tests.
- [ ] Apply Crystal item tables to runtime behavior, including random roll generation, stat ranges, special item types, stack/weight/restriction enforcement, and durability rules.
- [x] Enforce imported Crystal `StackSize` for current item gain and stack merge flows.
- [ ] Replace starter economy constants with generated Crystal shop, repair, special repair, storage, and NPC service data.
- [x] Align current item packet grid and equipment-slot numeric mappings with Crystal `MirGridType` / `EquipmentSlot` values.
- [x] Implement exact `DuraChanged` and `ItemRepaired` packet behavior for current durability loss and repair flows.
- [x] Implement Crystal ack packet behavior for current move, equip, remove, split, merge, drop, use, store, take-back, and remove-slot item flows.
- [x] Implement Crystal `GainedGold` and `LoseGold` packet behavior for current gold pickup and drop flows.
- [x] Implement Crystal `GainedCredit` and `LoseCredit` packet protocol/gateway support.
- [x] Wire current Crystal credit scroll/token use to account credit state, `GainedCredit`, `UserInformation.credit`, and save/reload persistence.
- [x] Wire current credit-shop spend flow to `LoseCredit` with balance checks and Crystal-style mailbox delivery.
- [x] Implement Crystal `SellItem` and `RepairItem` entry packet behavior for current sell and repair flows.
- [x] Implement Crystal `ItemSlotSizeChanged` and `ItemSealChanged` packet protocol/gateway support.
- [x] Wire current socket-slot growth flow to runtime equipment state and Crystal `ItemSlotSizeChanged`.
- [x] Wire current item seal flow to runtime equipment seal state and Crystal `ItemSealChanged`.
- [x] Store Crystal item seal `NextSealDate` reseal-delay metadata, block reseal before `Settings.ItemSealDelay`, and preserve the field across save/reload.
- [x] Implement Crystal NPC service and `CraftItem` packet protocol/gateway support for current shop/repair/storage/refine/craft surfaces.
- [x] Wire current Crystal NPC reserved service labels (`@Buy`, `@BuySell`, `@Sell`, `@Repair`, `@SRepair`, `@Craft`, `@Refine`, `@RefineCheck`, `@ReplaceWeddingRing`, and `@Storage`) to baseline service-open packets.
- [x] Populate current `NPCGoods` packets from imported Crystal NPC `[Trade]` and `[Recipe]` sections for buy/buy-sell/craft service pages.
- [x] Import Crystal `NPCInfo.Rate` data and apply it to current `NPCGoods`, `NPCRepair`, and `NPCSRepair` service packets.
- [x] Apply Crystal `GoodsHideAddedStats` and initial empty buy-back panel behavior to current `NPCGoods` service packets.
- [x] Track current-session NPC sell buy-back goods and expose them through `@BuyBack` `NPCGoods`.
- [x] Implement Crystal `BuyItem` client packet support and current NPC buy-back purchase runtime flow.
- [x] Align current Crystal `BuyItem` silent rejection behavior for invalid panel/count, inactive service, non-buy service pages, missing goods/metadata, insufficient gold, and full bags.
- [x] Align current NPC `RepairItem` / `SRepairItem` with Crystal backpack unique-id lookup, active repair-service gating, repair/special-repair cost, normal max-dura loss, and rejection behavior.
- [x] Align current NPC `SellItem` stack-count handling and sale gold with Crystal `ItemInfo.Price / 2`.
- [x] Align current NPC `SellItem` item flag/type/price failure behavior with Crystal `DontSell`, script `[Types]`, `UserItem.Price() / 2`, ack-only failures, and gold-cap edge cases.
- [x] Implement Crystal `DeleteItem` client/server packet behavior for current inventory delete flows.
- [x] Implement Crystal `UserItem` serialization and `SplitItem` payload packet behavior for current split-stack flows.
- [x] Align current packet `UseItem` / `EquipItem` / `MergeItem` current-inventory resolution with Crystal `UserItem.UniqueID`, including duplicate-key bag items across `Bag1` / `Bag2`.
- [x] Implement Crystal `GainedItem` payload packet behavior for current pickup inventory updates.
- [x] Implement Crystal `RefreshItem` payload packet protocol and gateway support.
- [x] Implement Crystal `RequestItemInfo` / `NewItemInfo` packet behavior backed by the imported Crystal item manifest.
- [x] Wire runtime `RefreshItem` triggers against representative Crystal item mutation traces, starting with `BenedictionOil` weapon Luck success refresh.
- [x] Wire current Crystal `RepairOil` and `WarGodOil` weapon repair scroll use to `ItemRepaired`.
- [x] Preserve current Crystal `NeedIdentify` / `SoulBoundId` metadata through runtime item/equipment state, `UserItem` round-trips, and equip/use-equip behavior.
- [x] Route dynamic manifest-backed current-data `UseItem` consumables and scrolls through Crystal template stats, including HP/MP restore, same-key buff duration stacking, town teleport, and repair-oil surfaces.
- [ ] Expand drop-table parity to include gold, grouped drops, rare rolls, quest drops, ownership timing, visibility timing, and inventory-full behavior.
  - [x] Resolve Crystal `MonsterInfo.DropPath` to imported `Envir/Drops` tables and prefer those tables over starter fallback for current monster death and harvest rewards.
  - [x] Convert current imported Crystal drop entries into runtime gold/item rewards, including grouped table sections, deterministic chance rolls, Crystal item metadata lookup, and fallback preservation for starter-only drops.
  - [x] Apply Crystal gold drop amount range semantics for imported `Gold N` entries (`N/2` inclusive through `N + N/2` exclusive) with deterministic runtime rolls.
  - [x] Emit Crystal `GainedItem` payloads when harvest transfers items into the player's bag, and keep harvest transfer item-only like Crystal `HarvestMonster`.
  - [x] Carry imported Crystal item durability through ground-drop pickup and harvest rewards, including `ItemType.Meat` quality durability for AI 2 Deer harvests.
  - [x] Apply Crystal `CreateDropItem` current-durability roll for imported item drops before meat quality and future random-stat upgrades.
  - [x] Set manifest-backed `UserItem.Identified` from Crystal `NeedIdentify` for current gained, pickup, harvest, and equipment payloads.
  - [x] Add Crystal-style death-drop pickup ownership windows with owner-only access until expiry and group-member pickup bypass.
  - [x] Carry imported `ShowGroupPickup` item metadata into pickup flow and emit Crystal-style group pickup notices when the player is grouped.
  - [x] Apply Crystal pickup/harvest gain checks for current drops: slot/stack capacity can block item transfer, bag weight does not block pickup/harvest acceptance, and overweight state is reflected after gain.
  - [x] Restrict player `PickUp` to the current map cell like Crystal `CurrentMap.GetCell(CurrentLocation)`, leaving adjacent drops untouched.
  - [x] Apply Crystal `ItemTimeOut` ground-drop expiry so item and gold drops despawn after the default 30-minute timeout.
  - [x] Split monster ground gold by Crystal `MaxDropGold=2000`, including the source-compatible zero remainder chunk on exact division.
  - [x] Apply Crystal `CanGainGold` cap checks to ground gold pickup so full-gold players do not consume drops.
  - [x] Align player `DropGold` edge behavior with Crystal: zero-gold drops are allowed and insufficient-gold requests return without packets.
  - [x] Populate ground `ObjectItem` grade and name colour from imported Crystal item grade metadata for manifest-backed drops.
  - [x] Align player `DropItem` semantics with Crystal stack-count splitting, failure ack behavior, `DontDrop` rejection, and `DestroyOnDrop` no-ground-object deletion.
  - [x] Align Crystal `AddItem` belt-priority placement for Potion/Scroll/Script effect 1 and Amulet gains, including belt `UseItem` consumption for current player belt slots.
  - [x] Implement Crystal ground-drop position search and `DropStackSize` object-count limits for current player item drops, player gold drops, and monster ground drops.
  - [x] Implement Crystal quest-drop (`Q`) gating for current runtime death and harvest drops.
  - [x] Implement current Crystal random-stat roll baseline for MaxDura, MaxAC, and MaxDC on drop-created items.
  - [x] Implement full current Jev random-stat family payloads, curse flag, socket slots, and save/reload coverage for drop-created items.
  - [x] Replace the remaining hardcoded random-stat profile table with generated `RandomItemStats.ini` data.
  - [x] Finish Crystal `GROUP` drop semantics, including nested groups, `GROUP*` random-one-item selection, `GROUP^` first-success short-circuiting, and child gold accumulation.
  - [x] Source-audit delayed visibility and current pickup rejection semantics: item/gold drops broadcast immediately, owner windows restrict pickup only, and current-cell scan skips owner-blocked/full-bag/gold-cap candidates when later drops can be picked.
  - [x] Persist Crystal `HarvestMonster` pending `_drops` after the skin count reaches zero, transfer them on the next harvest call, and retain untransferable leftovers for later retries.
  - [x] Apply Crystal harvest owner/EXPOwner corpse scan rejection semantics, including grouped-owner bypass and `NoNearbyOwnedCarcasses`.
  - [x] Require an active Crystal sell service for `SellItem`, and reject partial-stack sales that would overflow the Crystal gold cap.
  - [x] Mail credit-shop purchases like Crystal game-shop buys, and block only mail attachment claim when the bag cannot accept the item.
  - [x] Align `BuyItem` silent no-mutation rejection for invalid panel/count, no-service, non-buy service, missing goods, insufficient gold, and full bags.
  - [x] Align NPC `RepairItem` / `SRepairItem` semantics: entry ack, backpack unique-id item lookup, matching `@Repair` / `@SRepair` page requirement, Crystal cost, normal max-dura loss, special-repair max preservation, repairability/type messages, and insufficient-gold silent return.
  - [x] Align NPC `SellItem` semantics for `DontSell`, script `[Types]`, zero-count/missing-item/count failures, partial-stack gold overflow, full-stack gold-cap clamping, and Crystal `UserItem.Price() / 2` sale value.
  - [ ] Finish broader inventory/economy rejection semantics outside current ground-drop and harvest paths.
- [ ] Make trade, shop, auction, mail attachments, and storage packet-perfect against representative Crystal traces.
- [x] Add Crystal `StackSize`-aware inventory-full checks for current pickup, shop, auction, NPC reward, and quest reward item grants.
- [ ] Add no-duplication and no-loss tests for every transactional item/economy path.

### E. NPC, Quest, and Script Semantics

- [ ] Build a script path coverage report that goes beyond command-name coverage and records which imported sections, branches, inputs, and actions have behavior tests.
- [ ] Execute representative Crystal NPC scripts end-to-end and compare Rust runtime results for dialog text, links, inputs, rewards, flags, messages, teleport, pets, guild territory, conquest, and timed recall behavior.
- [ ] Implement remaining semantic gaps where command names exist but Crystal edge behavior differs.
- [ ] Import quest state, quest UI status, repeatability, prerequisites, progress counters, reward choices, and failure branches.
- [ ] Add loop-safety and diagnostic checks for every script path that cannot execute because of missing data or unsupported runtime state.
- [ ] Check this gate only when no high-severity NPC/quest semantic gap remains open.

### F. Maps, World Events, and AOI

- [x] Resolve missing or indirect map assets for the current automated gate: minimap indices are complete, Crystal no-draw source frames are tracked separately, and invalid/special movement targets are filtered from runtime direct transfers by the 2026-05-16 all-map audit.
- [ ] Import map event script bindings and exact weather, light, lightning, fire, door, wall, gate, and object-state behavior.
- [ ] Complete all map transfer, safe zone, revive point, random spawn, collision, blocking, and occupancy behavior against Crystal source.
- [ ] Expand AOI enter/leave parity beyond current spawn/action/remove ordering, including object despawn, hidden state, projectile visibility, drops, NPCs, pets, heroes, and event objects.
- [ ] Add representative screenshot/API/packet evidence for every high-traffic map family.
- [ ] Check this gate only when all imported maps either render and transfer correctly or have a documented missing-source blocker.

### G. Broad Social and Late-Game Systems

- [ ] Replace Stage 5 functional baselines for group, guild, social, mail, trade, shop, auction, conquest, hero, mining, crafting, refining, and guild territory with Crystal data-driven behavior.
- [ ] Implement packet-perfect group, guild, friend/block, mail, whisper, guild chat, party chat, rankings, permissions, and guild territory flows.
- [ ] Implement conquest scheduling, castle ownership, gates, walls, guards, tax, war start/end, rewards, and NPC control against Crystal behavior.
- [ ] Implement hero equipment, inventory, AI, death/revive, seal/unseal, follow/attack modes, and persistence.
- [ ] Implement mining, crafting, refining, success probability, failure behavior, material consumption, and packet-visible feedback from Crystal tables.
- [ ] Add multi-client transactional tests for every player-to-player and guild/conquest path.

### H. Persistence, Operations, and Recovery

- [ ] Decide and document the production persistence target for accounts, characters, world state, mail, guilds, auctions, conquest, heroes, pets, and event state.
- [ ] Implement durable migrations, schema validation, backup, restore, corrupt-source preservation, cross-process locking or equivalent deployment-safe ownership rules.
- [ ] Add reconnect, crash, socket-close, process-restart, and partial-write tests for every persistent subsystem.
- [ ] Run long-duration soak beyond the current 1,200-tick simulation baseline, including real gateway WebSocket/TCP clients, RSS/handle monitoring, and entity-count bounds.
- [ ] Add production telemetry, structured error logs, panic boundaries, health checks, and rollback notes.
- [ ] Check the 97% gate only after long soak and recovery evidence is archived under `docs/generated`.

### I. UI, Visual, and Asset 1:1

- [ ] Build a screenshot matrix for login, select, game HUD, inventory, character, storage, NPC, chat, combat, map transfer, trade, auction, guild, conquest, hero, mining, and crafting.
- [ ] Compare representative screens against Crystal/original client assets for layout, sprite library id, animation frame, minimap, text placement, buttons, and panel behavior.
- [ ] Replace debug-only UI entry points with original-like user flows where Crystal exposes a real client interaction.
- [ ] Ensure 1024x768 remains the primary no-overlap acceptance resolution, then add wider and smaller viewport sanity screenshots.
- [ ] Check this gate only when visual differences are documented as either fixed, intentional, or blocked by missing source assets.

### J. Final 100% Gate

- [x] `infra/check-candidate-gate.sh` provides the repeatable local/full/live
  Candidate gate command bundle. `MIR2_CANDIDATE_SCOPE=local bash
  infra/check-candidate-gate.sh` passed on 2026-05-06; `full` and `live`
  scopes are the explicit entry points for build/static-smoke and running
  Gateway/Web evidence refreshes. `.github/workflows/mir2-candidate-gate.yml`
  runs the local scope on pull requests and pushes to `main`.
- [ ] `cargo fmt --check` passes.
- [ ] `cargo test --workspace` passes.
- [ ] `cargo test --workspace -- --test-threads=1` passes.
- [ ] `npm.cmd run build` passes.
- [ ] `npm.cmd run smoke:crystal-minimap-assets` passes with no unresolved required asset warnings.
- [ ] `npm.cmd run smoke:crystal-map-api` passes for the expanded representative map set.
- [ ] `npm.cmd run smoke:stage5-ui` passes for the expanded UI/system matrix.
- [ ] `npm.cmd run load:gateway-ws` and `target\debug\tcp_load.exe` pass at the accepted production-smoke concurrency.
- [ ] `cargo run -p mir2-gateway --bin packet_trace` passes with accepted live Crystal comparisons for the full representative matrix.
- [ ] `docs/BACKEND-1TO1-PROGRESS.md`, `docs/CRYSTAL-SERVER-PARITY.md`, and this roadmap all report 100% with no unapproved open high/medium gaps.

## Operating Loop

Use this loop until every stage gate is checked:

1. Pick the highest-priority unchecked item in the current stage.
2. Read Crystal source or generated manifests before changing Rust/TypeScript behavior.
3. Write or update a focused regression test that describes the Crystal-observed behavior.
4. Implement the smallest compatible change in the relevant module.
5. Run the local verification commands for that item.
6. If verification passes, check the item and add a short note with date and command.
7. If verification fails, leave the item unchecked and add a blocker note.
8. When all items in a stage pass, run the stage gate commands.
9. Check the stage gate only when all required commands pass and the UI/API acceptance items are verified.
10. Move to the next stage.

Checklist rule:

- `[ ]` means not started or not verified.
- `[~]` means partially implemented but not accepted. Markdown has no native partial checkbox, so use `[ ]` plus a note if the renderer does not support `[~]`.
- `[x]` means implemented and verified with the listed acceptance criteria.

Do not check a box for code that only "looks right". Check it after a command, API response, screenshot, manual comparison, or Crystal source comparison proves it.

## Global Verification Commands

Run these before checking a stage gate:

```powershell
cd E:\mir2\mir2-web3
cargo test --workspace
```

```powershell
cd E:\mir2\mir2-web3\apps\web
npm.cmd run build
```

```powershell
Invoke-WebRequest -UseBasicParsing -Uri 'http://127.0.0.1:7110/health' -TimeoutSec 3
Invoke-WebRequest -UseBasicParsing -Uri 'http://127.0.0.1:3002' -TimeoutSec 3
```

Representative map API smoke:

```powershell
cd E:\mir2\mir2-web3\apps\web
$maps=@('0','1','2','n0','HF1','HF2','HF3','D1801','HKR')
foreach($m in $maps){
  $u="http://127.0.0.1:3002/api/scene/crystal?map=$m&x=200&y=200&width=24&height=18"
  $r=Invoke-WebRequest -UseBasicParsing -Uri $u -TimeoutSec 60
  $j=$r.Content | ConvertFrom-Json
  [pscustomobject]@{
    map=$m
    status=$r.StatusCode
    title=$j.mapTitle
    mini=$j.miniMapIndex
    cells=($j.originalMapRegion.cells | Measure-Object).Count
    sprites=($j.originalMapRegion.sprites.PSObject.Properties | Measure-Object).Count
    width=$j.originalMapRegion.mapWidth
    height=$j.originalMapRegion.mapHeight
  }
}
```

## Stage 1: Restore Regression Baseline

Goal: make the current migrated slice trustworthy again before expanding scope.

Estimated time: 0.5 to 2 days.

Exit condition: `cargo test --workspace` and `npm.cmd run build` both pass, and the current demo still enters the game.

### 1.1 Triage Current Simulation Failures

Current failing tests from 2026-04-21:

- [x] `runtime::tests::attack_reduces_monster_hp`
- [x] `runtime::tests::consumable_item_restores_hp`
- [x] `runtime::tests::crystal_npc_group_and_conquest_runtime_use_configured_members_and_state`
- [x] `runtime::tests::crystal_pickup_packet_collects_nearest_adjacent_ground_drop`
- [x] `runtime::tests::crystal_use_item_packet_consumes_inventory_slot`
- [x] `runtime::tests::defeating_field_wasp_advances_and_turns_in_quest`
- [x] `runtime::tests::dropped_gold_can_be_picked_up`
- [x] `runtime::tests::dropped_item_can_be_picked_up_into_inventory`
- [x] `runtime::tests::npc_interaction_assigns_quest_and_dialog`
- [x] `runtime::tests::npc_interaction_uses_script_template_lookup`
- [x] `runtime::tests::player_attack_damages_weapon_durability`
- [x] `runtime::tests::player_attack_delays_health_change_until_followup_tick`
- [x] `runtime::tests::world_snapshot_filters_outside_player_aoi`

Acceptance:

- [x] Each failure has a root-cause note: behavior regression, stale test expectation, imported data change, timing change, or fixture mismatch.
- [x] No failing test is deleted to make the suite pass.
- [x] If a test expectation is changed, the new expectation is tied to Crystal source behavior or a deliberate updated contract.

Root cause note:

- 12 failures came from test fixture setup using `session.move_to(...)` as if it teleported the player. The runtime contract is step-based movement because the browser/gateway issues repeated movement commands; tests now use `set_player_position(...)` for preconditions.
- 1 failure came from `GROUPTELEPORT` clearing remote group-member entities during map relocation before moving the group. The runtime now snapshots remote group members before relocation and restores them at the group teleport destination.

Verification:

```powershell
cd E:\mir2\mir2-web3
cargo test -p mir2-simulation --lib -- --test-threads=1
```

### 1.2 Restore Player Combat Baseline

Scope:

- [x] Player attack reduces monster HP after the intended delayed-hit tick.
- [x] `ObjectAttack` / `ObjectStruck` / `ObjectHealth` packet ordering matches current protocol expectation.
- [x] Weapon durability decreases on player attack when the attack is accepted.
- [x] Dead or untargetable monsters do not incorrectly consume durability.
- [x] Existing special AI rules still hold: hidden plant, stoned Zuma, guard neutrality, line attacks.

Acceptance:

- [x] `attack_reduces_monster_hp` passes.
- [x] `player_attack_damages_weapon_durability` passes.
- [x] `player_attack_delays_health_change_until_followup_tick` passes.
- [x] Existing combat-special tests still pass.

### 1.3 Restore Consumable Item Baseline

Scope:

- [x] Basic HP consumable restores HP only when applicable.
- [x] Crystal-style use item command consumes the inventory slot when the item is used.
- [x] No-op use should not destroy item unless Crystal behavior says so.
- [x] System feedback remains localized and packet-visible where expected.

Acceptance:

- [x] `consumable_item_restores_hp` passes.
- [x] `crystal_use_item_packet_consumes_inventory_slot` passes.

### 1.4 Restore Drop and Pickup Baseline

Scope:

- [x] Visible monster death can generate item and gold drops.
- [x] Adjacent pickup collects the nearest legal drop.
- [x] Inventory capacity and slot placement remain deterministic.
- [x] Gold pickup updates player gold and removes the ground drop.

Acceptance:

- [x] `crystal_pickup_packet_collects_nearest_adjacent_ground_drop` passes.
- [x] `defeating_field_wasp_advances_and_turns_in_quest` passes.
- [x] `dropped_gold_can_be_picked_up` passes.
- [x] `dropped_item_can_be_picked_up_into_inventory` passes.
- [x] `world_snapshot_filters_outside_player_aoi` passes.

### 1.5 Restore NPC Dialog and Script Baseline

Scope:

- [x] Starter NPC interaction opens the expected active dialog.
- [x] NPC script template lookup binds the correct script key.
- [x] Quest assignment and dialog presentation remain compatible.
- [x] `ObjectChat` or dialog snapshot behavior is aligned with the current client contract.

Acceptance:

- [x] `npc_interaction_assigns_quest_and_dialog` passes.
- [x] `npc_interaction_uses_script_template_lookup` passes.

### 1.6 Restore Group and Conquest Script Baseline

Scope:

- [x] Group runtime membership checks use configured party state.
- [x] Group actions affect the expected number of configured members.
- [x] Conquest state checks are runtime-backed and deterministic in tests.

Acceptance:

- [x] `crystal_npc_group_and_conquest_runtime_use_configured_members_and_state` passes.

### Stage 1 Gate

- [x] `cargo test --workspace` passes.
- [x] `npm.cmd run build` passes.
- [x] `http://127.0.0.1:3002` loads.
- [x] `http://127.0.0.1:7110/health` reports ready HTTP and WS.
- [x] No known regression is hidden by ignored tests.

Completion note:

- Date: 2026-04-21
- Commands: `cargo test -p mir2-simulation --lib -- --test-threads=1`; `cargo test --workspace`; `npm.cmd run build`; web/gateway health checks; representative map API smoke.
- Remaining risk: one deprecated Bevy ECS API warning remains; Stage 2 still needs full map manifest regeneration and cold-start map API performance work.

## Stage 2: Finish Map, UI, and Data Pipeline

Goal: make the original-client UI and map loading pipeline stable enough for broad manual visual comparison.

Estimated time: 3 to 7 days after Stage 1 is green.

Exit condition: representative maps can be selected from the UI, mini-map metadata is generated and consumed, map API cold/warm performance is acceptable, and production build remains green.

### 2.1 Full Map Manifest Regeneration

Scope:

- [x] Regenerate full `crystal_respawn_manifest.json` from Crystal data.
- [x] Include `map_file_name`, title, respawns, route references, safe-zone/transfer metadata when available.
- [x] Include `mini_map` metadata for all maps that have a Crystal mini-map.
- [x] Preserve existing imported respawn behavior.
- [x] Add a manifest validation command or test.

Acceptance:

- [x] `packages/game-data` can load the regenerated manifest.
- [x] Known maps report expected `map_title`.
- [x] Known mini-map maps report non-null `miniMapIndex`.
- [x] Map entries without mini-maps use the small original UI frame instead of fake raster mini-maps.

Completion note:

- Date: 2026-04-21
- Generated maps: 463 total, 6,341 total respawns, 179 maps without respawns, 294 maps with Crystal `mini_map`, 412 maps with movements, 21 maps with safe zones.
- Commands: `node packages\tooling\scripts\generate-crystal-respawn-manifest.mjs`; `cargo test -p mir2-game-data`.
- Remaining risk: UI mini-map assets currently only include exported `MMap/0.png`, `MMap/1.png`, and `MMap/8.png`; Stage 2.4 must export or map true Crystal mini-map indexes such as `101`.

Verification:

```powershell
cd E:\mir2\mir2-web3
cargo test -p mir2-game-data
```

### 2.2 Map Switcher for Visual QA

Scope:

- [x] Add a UI or debug control to switch directly to representative maps.
- [x] Include `0`, `1`, `2`, `n0`, `HF1`, `HF2`, `HF3`, `D1801`, and `HKR`.
- [x] Support direct map, x, y selection for debugging.
- [x] Keep this tool out of normal gameplay flow or label it as QA/debug.

Acceptance:

- [x] User can switch maps without editing code.
- [x] Current player position and scene center update.
- [x] Map title, coordinate display, and mini-map panel update.
- [x] No shell reload is required for manual comparison.

Completion note:

- Date: 2026-04-21
- Implementation: system menu now includes representative Crystal map jump buttons and a QA `map/x/y` jump form that sends existing `crystal:<map>:<x>:<y>` transfer keys.
- Commands: `npm.cmd run build`; `cargo test -p mir2-simulation debug_crystal_transfer_key_updates_map_information_and_location`; `cargo test -p mir2-simulation --lib -- --test-threads=1`; representative map API smoke for `0`, `1`, `2`, `n0`, `HF1`, `HF2`, `HF3`, `D1801`, and `HKR`.
- Remaining risk: automated browser click coverage is not present because this workspace has no Playwright dependency; production build and runtime transfer regression cover the wiring.

### 2.3 Map API Performance and Cache

Scope:

- [x] Measure cold map API response for large/asset-heavy maps.
- [x] Measure warm map API response.
- [x] Cache parsed map files.
- [x] Cache parsed `.Lib` files.
- [x] Avoid re-exporting PNG frames that already exist.
- [x] Identify maps that still exceed a practical cold-start threshold.

Acceptance:

- [x] Representative maps return within 60 seconds cold.
- [x] Representative maps return within 2 seconds warm.
- [x] API returns non-empty `cells` and `sprites` for known populated regions.
- [x] Timeout behavior is explicit and logged.

Completion note:

- Date: 2026-04-21
- Implementation: `exportMapRegion` now walks only the requested bounds by direct cell index instead of scanning every cell in the full map. Existing process caches for parsed maps, parsed `.Lib` files, and exported PNG existence are preserved.
- Commands: `npm.cmd run smoke:crystal-map-api`; `npm.cmd run build`.
- Performance evidence: first pass max was `0` at 51,571 ms and `n0` at 24,181 ms; all first-pass maps were under 60 seconds. Warm pass max was `HF1` at 67 ms; all warm maps were under 2 seconds.
- Remaining risk: first export of very asset-heavy maps still spends tens of seconds writing/reading many PNGs; acceptable for this stage but worth revisiting with offline pre-export if broader map QA becomes slow.

### 2.4 Mini-map 1:1 Completion

Scope:

- [x] Confirm maps with no mini-map use `Prguse/2091.png` small frame.
- [x] Confirm maps with mini-map use 120x108 original clipped window.
- [x] Confirm coordinates render in the original-like position.
- [x] Confirm mail/collapse buttons do not overlap.
- [x] Confirm current player marker is scaled and positioned correctly.

Acceptance:

- [x] Bichon Province `0` displays Crystal raster mini-map `101`. Earlier no-mini-map expectation was corrected by the full Crystal DB manifest.
- [x] `1` and `2` display raster mini-map where metadata exists.
- [x] At least 5 maps are screenshot-archived for automated regression evidence. Manual Crystal/original comparison is intentionally non-blocking per current operating mode.

Completion note:

- Date: 2026-04-21
- Implementation: `MMap` export now automatically merges all positive `mini_map` indices from `crystal_respawn_manifest.json`; the client reads `MMap/meta.json` for per-index raster dimensions instead of hard-coding only indices `1` and `8`.
- Commands: `npm.cmd run export:crystal-ui`; `npm.cmd run smoke:crystal-minimap-assets`; `npm.cmd run build`.
- Evidence: representative mini-map frames `101`, `102`, `105`, `406`, `407`, `408`, and `409` are exported with dimensions; `D1801` has `mini_map=0` and uses the small frame path. Headless screenshots were generated under `docs/stage2-screenshots`, but automated visual comparison is still pending.
- Remaining risk: Crystal `mini_map` indices `450` and `451` are referenced by `DogYoArena2` and `DogYoHyun` but no exportable frames were present in `MMap.Lib`.

### 2.5 Main Game UI 1:1 Pass

Scope:

- [x] HP/MP bars align with original frame.
- [x] Belt and chat panel align with original assets.
- [x] Inventory panel tabs and slots are stable.
- [x] Character panel tabs and equipment slots are stable.
- [x] NPC dialog panel supports imported links and input prompts.
- [x] Storage/password UI remains functional.
- [x] All text fits without overlap at 1024x768.

Acceptance:

- [x] Automated screenshot archive exists for representative HUD/map states; manual visual approval is intentionally non-blocking.
- [x] `npm.cmd run build` passes.
- [x] No new TypeScript errors.

Completion note:

- Date: 2026-04-21
- Commands: `npm.cmd run build`; `npm.cmd run smoke:crystal-minimap-assets`; `cargo test --workspace`.
- Screenshots: `docs/stage2-screenshots/stage2-minimap-0.png`, `stage2-minimap-1.png`, `stage2-minimap-2.png`, `stage2-minimap-HF1.png`, `stage2-minimap-D1801.png`, `stage2-minimap-HKR.png`.
- Remaining risk: screenshot files are regression evidence, not human-approved pixel parity; this is accepted for the current autonomous mode.

### Stage 2 Gate

- [x] Full map manifest generated.
- [x] Map switcher exists.
- [x] Representative map API smoke passes.
- [x] Mini-map behavior accepted.
- [x] Main UI visual pass accepted.
- [x] `cargo test --workspace` passes.
- [x] `npm.cmd run build` passes.

Completion note:

- Date: 2026-04-21
- Commands: `cargo test --workspace`; `npm.cmd run build`; `npm.cmd run smoke:crystal-map-api`; `npm.cmd run smoke:crystal-minimap-assets`; gateway health and web/API checks.
- Screenshots: archived under `docs/stage2-screenshots`.
- Remaining risk: exact human pixel comparison is deferred by request. The historical `MMap` preview-index gap was closed by the later full-client/minimap audit.

## Stage 3: Expand Current Playable Server Slice

Goal: turn the current slice into a broader playable server baseline: login, select, enter map, move, fight, loot, use items, interact with NPCs, save, reconnect.

Estimated time: 1 to 3 weeks after Stage 2.

Exit condition: a player can run a repeatable 15-30 minute gameplay loop with persistence and without manual intervention.

### 3.1 Login, Account, Character, Reconnect

Scope:

- [x] Account creation and login are deterministic.
- [x] Character creation supports all available classes/genders that are represented by UI assets.
- [x] Delete character behavior matches current protocol expectation.
- [x] Start game restores saved position, map, direction, HP/MP, gold, inventory, equipment, skills, quests.
- [x] Logout saves active character.
- [x] Fresh gateway process can reload JSON-backed account state.

Acceptance:

- [x] Login/select/start-game tests pass.
- [x] Reconnect test covers map and position persistence.
- [x] Automated runtime loop covers create/login/start, move, save, reload, and re-enter.

### 3.2 Movement, Collision, AOI

Scope:

- [x] Walk and run honor map collision.
- [x] Blocking objects and occupied tiles prevent invalid movement.
- [x] Diagonal movement follows Crystal constraints where applicable.
- [x] AOI spawn/remove ordering is stable.
- [x] Visible action packets are not dropped when object enters AOI same tick.
- [x] Map bounds are enforced for imported maps.

Acceptance:

- [x] Movement tests pass.
- [x] AOI tests pass.
- [x] Automated movement/collision regressions cover wall, occupied tile, and imported map bounds.

### 3.3 Combat and Death Loop

Scope:

- [x] Player melee timing and damage follows current Crystal target.
- [x] Monster melee and ranged attacks use delayed hit resolution.
- [x] Death packets and corpse cleanup are deterministic.
- [x] Revive and respawn states are represented.
- [x] Weapon and armor durability update on relevant events.
- [x] Basic PvE loop is playable without fixture-only assumptions.

Acceptance:

- [x] Combat regression tests pass.
- [x] Automated PvE loop kills a monster, takes/uses HP flow, and persists state.
- [x] Death/respawn behavior has automated regressions.

### 3.4 Drops, Inventory, Belt, Equipment

Scope:

- [x] Drop tables resolve from imported Crystal data where available.
- [x] Item pickup respects adjacency, capacity, and stack rules.
- [x] Item use supports common consumables.
- [x] Belt shortcuts work through client command path.
- [x] Equip/remove/swap equipment works.
- [x] Broken gear stat suppression remains covered.
- [x] Repair powder and repair commands are stable.

Acceptance:

- [x] Pickup/use/equip/drop/repair tests pass.
- [x] Automated gameplay loop includes loot, pickup, use potion, equipment reward, and persistence.

### 3.5 NPC and Quest Starter Loop

Scope:

- [x] Starter guide quest is data-driven.
- [x] NPC links and follow-up sections work.
- [x] NPC input prompt loop works.
- [x] Quest flags persist.
- [x] Rewards are applied through item/gold/exp/skill action paths.
- [x] Idle fallback exists for unscripted NPCs.

Acceptance:

- [x] NPC script tests pass.
- [x] Automated NPC interaction accepts/advances/turns in a quest.

### 3.6 Map Transfer and Safe Zones

Scope:

- [x] Transfer rules can move player between multiple imported maps.
- [x] `MapInformation` and `UserLocation` refresh after transfer.
- [x] Safe-zone state is available in snapshot and relevant packets.
- [x] Manual map switch and gameplay transfer paths do not conflict.

Acceptance:

- [x] Transfer tests pass.
- [x] Automated transfer between representative maps works.

### Stage 3 Gate

- [x] Automated PvE loop completed.
- [x] Save/reconnect verified.
- [x] All current workspace tests pass.
- [x] Frontend build passes.
- [x] Known gameplay limitations are documented in this file or parity docs.

Completion note:

- Date: 2026-04-21
- Commands: `cargo test -p mir2-simulation stage3_playable_pve_loop_persists_after_reconnect`; `cargo test -p mir2-simulation --lib -- --test-threads=1`; `cargo test --workspace`; `npm.cmd run build`.
- Manual route: replaced with automated route by request.
- Remaining risk: Stage 3 is a broad playable slice, not exhaustive full 1:1 parity; broad Crystal systems remain in Stage 4/5.

## Stage 4: Broaden Crystal System Parity

Goal: move beyond the starter/midgame slice into broad Crystal behavior coverage.

Estimated time: 3 to 6 weeks after Stage 3.

Exit condition: most common Crystal gameplay systems are imported or explicitly tracked as remaining gaps.

### 4.1 Full Map Server Metadata

Scope:

- [x] Import map transfer definitions.
- [x] Import safe-zone definitions.
- [x] Import map mini-map, big-map, and light settings into runtime `MapInformation`.
- [ ] Import map weather, lightning/fire, and exact time-of-day settings where applicable.
- [x] Import respawn zones for all maps.
- [x] Import route patrol data for all route-enabled monsters.
- [ ] Import event script bindings per map.
- [x] Add validation for missing movement targets and monster AI source mappings.
- [ ] Add validation for missing referenced map event scripts once event bindings are imported.

Acceptance:

- [x] Generated manifest covers all expected Crystal maps.
- [x] Missing references are listed and triaged.
- [x] Runtime can load multiple real maps without hardcoded starter-only assumptions.

Completion note:

- Date: 2026-04-21
- Implementation: runtime map transfers and safe-zone checks now consume generated Crystal movement/safe-zone metadata. Runtime `MapInformation` now carries Crystal `mini_map`, `big_map`, and `light`; representative map spawn tables now use target-map collision data instead of starter-map collision, with runtime collision parsing cached for performance.
- Commands: `cargo test -p mir2-simulation crystal_manifest_`; `cargo test -p mir2-simulation crystal_manifest_map_information_includes_minimap_bigmap_and_light`; `cargo test -p mir2-simulation crystal_current_map_spawn_table_uses_representative_map_rosters`; `cargo test --workspace`; `cargo test --workspace -- --test-threads=1`.
- Remaining risk: map event script bindings, weather/lightning/fire flags, and exact time-of-day packet behavior are still open.

### 4.2 Monster Roster and AI Families

Scope:

- [x] Import full monster roster metadata.
- [x] Classify AI families by Crystal behavior.
- [x] Implement HellFire boss cluster AI families: HellKnight / HellLord / HellBomb.
- [x] Implement HellBomb Frozen/Dazed/Bleeding poison variants.
- [x] Implement high-count line/range AI families: ShamanZombie / BlackFoxman.
- [x] Implement DigOutZombie hidden/reveal behavior.
- [x] Implement RevivingZombie delayed revival behavior.
- [x] Implement RedFoxman / WhiteFoxman ranged attack baseline.
- [x] Implement RedFoxman type-0/type-1 ranged packet split and imported DC ranged damage.
- [x] Implement WhiteFoxman type-1 delayed status-only slow branch.
- [x] Implement RedFoxman / WhiteFoxman fear-window kiting and RedFoxman adjacent teleport effect.
- [x] Implement WaterDragon / BlackTortoise ranged attack baseline.
- [x] Implement WaterDragon/BlackTortoise ranged green-poison hook and current SmallDrake zero-MC gating.
- [x] Implement BlackTortoise close type-1 halfmoon fanout.
- [x] Implement BlackHammerCat / StrayCat / CatShaman attack packet baseline.
- [x] Implement StrayCat close type-1 push variant and current zero-MC follow-up gating.
- [x] Implement CatShaman type-1 red-poison packet branch and current zero-MC gating.
- [x] Implement Yin/Yang Devil Node immobile support-node baseline.
- [ ] Implement remaining common AI families.
- [ ] Add target acquisition parity for hostile, neutral, guard, summon, trap, and special monsters.
- [ ] Add ranged/projectile behavior where packet-visible.
- [ ] Add hide/show/stone/wake/special states for all relevant families.
- [x] Add respawn state reset parity.

Acceptance:

- [x] Each currently implemented AI family has at least one regression.
- [x] Representative maps spawn correct monster types.
- [x] Guard/town monster behavior remains neutral or hostile as in Crystal.

Completion note:

- Date: 2026-04-21
- Implementation: added `packages/tooling/scripts/generate-crystal-monster-ai-summary.mjs`, generated `packages/game-data/data/generated/crystal_monster_ai_summary.json`, and generated `docs/generated/crystal-monster-ai-summary.md`. The summary cross-references Crystal `MonsterObject.GetMonster`, the monster manifest, and all map respawns: 555 monster rows, 212 AI families, 87 spawned AI families, 35 currently special/guard-covered runtime families, 57 generic runtime families, and 117 data-only families. HellFire AI 97/98/99, high-count AI 26/42/44/45/46/116/117/118/181/182, DigOutZombie AI 24, and RevivingZombie AI 25 now have runtime behavior and regression coverage.
- Commands: `node packages\tooling\scripts\generate-crystal-monster-ai-summary.mjs`; `cargo test -p mir2-game-data crystal_monster_ai_summary_classifies_manifest_families`; `cargo test -p mir2-simulation hell_ -- --test-threads=1`; `cargo test -p mir2-simulation line_attack -- --test-threads=1`; `cargo test -p mir2-simulation shaman_zombie -- --test-threads=1`; `cargo test -p mir2-simulation dig_out_zombie -- --test-threads=1`; `cargo test -p mir2-simulation reviving_zombie -- --test-threads=1`; `cargo test -p mir2-simulation foxmen -- --test-threads=1`; `cargo test -p mir2-simulation water_dragon -- --test-threads=1`; `cargo test -p mir2-simulation cat_family -- --test-threads=1`; `cargo test -p mir2-simulation yin_devil_node -- --test-threads=1`; `cargo test -p mir2-simulation crystal_current_map_spawn_table_uses_representative_map_rosters`; `cargo test --workspace`; `npm.cmd run build`.
- Remaining risk: broad generic AI families are classified and spawned, but not yet behavior-complete.

### 4.3 Skills, Buffs, Projectiles, Summons

Scope:

- [ ] Import full magic table.
- [ ] Implement class-specific cast rules.
- [ ] Implement MP, cooldown, range, line-of-sight, target validation.
- [ ] Implement buff lifecycle and stat effects.
- [ ] Implement projectile/delayed impact packet behavior.
- [ ] Complete summon families and edge-case cleanup.
- [ ] Persist skill levels and cooldowns.

Acceptance:

- [ ] Common warrior, wizard, taoist, assassin, and archer skills have tests.
- [ ] Summon ownership and cleanup tests pass.
- [ ] Manual cast loop works in browser.

### 4.4 Item System Expansion

Scope:

- [ ] Import item definitions beyond starter templates.
- [ ] Implement item grades, random rolls, stat ranges, special flags.
- [ ] Implement more consumable types.
- [ ] Implement scrolls, books, repair items, bundle/stack behavior.
- [ ] Implement sell/store/repair price behavior.
- [x] Implement exact durability and repair packet behavior for current durability loss and repair flows.

Acceptance:

- [ ] Item manifest validation passes.
- [x] Random stat persistence has regression coverage.
- [ ] Common item flows pass through gateway and UI.

### 4.5 NPC Script Engine Expansion

Scope:

- [x] Expand command coverage beyond current implemented list.
- [x] Classify unimplemented commands by frequency in Crystal scripts.
- [x] Add parser support for remaining label/argument/input forms.
- [x] Add script execution tracing for debugging imported scripts.
- [x] Add script safety limits to avoid infinite loops.
- [x] Add automated tests for high-value real Crystal NPC command families.

Acceptance:

- [x] Top-used imported scripts execute without unknown-command blockers.
- [x] Unknown commands are reported with script key and line number.
- [ ] Regression suite covers representative quest, shop, travel, event, and admin scripts.

Completion note:

- Date: 2026-04-22
- Implementation: regenerated `crystal_npc_command_summary.json` and `docs/generated/crystal-npc-command-summary.md` after adding runtime baselines for conquest, guild territory, hero, hair, buff, recall, name-list, and message/admin command families. Runtime command coverage is now 81/81 command names and 7,044/7,044 command occurrences.
- Commands: `node packages\tooling\scripts\generate-crystal-npc-command-summary.mjs`; `cargo test -p mir2-game-data crystal_npc_command_summary_classifies_runtime_coverage`; `cargo test -p mir2-simulation crystal_npc_stage5_ -- --test-threads=1`.
- Remaining risk: command-name blockers are closed, but exact semantic parity for every imported NPC path is still broader than command coverage and remains tied to representative script-flow tests plus live Crystal behavior comparison.

### 4.6 Persistence Model Hardening

Scope:

- [x] Persist full character state needed by implemented systems.
- [x] Add schema/version handling for save files.
- [x] Add migration for older saves.
- [x] Add atomic write behavior for JSON-backed store or replace with a stronger storage layer.
- [x] Add crash/restart tests.

Acceptance:

- [x] Saves survive process restart.
- [x] Old saves load with defaults.
- [x] Corrupt save behavior is explicit and safe.

Completion note:

- Date: 2026-04-21
- Implementation: JSON-backed account store now carries `schemaVersion`, migrates legacy saves to the current schema, writes through same-directory temporary files plus atomic replace, and preserves corrupt source files while explicitly falling back to a default account store.
- Commands: `cargo test -p mir2-simulation account_store -- --test-threads=1`; `cargo test -p mir2-simulation multi_client_shared_store_smoke -- --test-threads=1`; `cargo test -p mir2-simulation long_running_tick_soak -- --test-threads=1`.
- Remaining risk: this is still a JSON file store; production-scale locking, backups, and cross-process conflict handling remain Stage 5 operational work.

### 4.7 Protocol Parity Expansion

Scope:

- [ ] Map all currently emitted packets to Crystal IDs and payload shapes.
- [ ] Add missing packet IDs for implemented systems.
- [ ] Add codec roundtrips for new packets.
- [x] Add packet ordering tests for combat, AOI, map transfer, NPC, item use.
- [x] Add browser/gateway command mapping for new client actions.

Acceptance:

- [x] Protocol tests pass.
- [x] Gateway command tests pass.
- [x] Packet ordering is covered for high-risk flows.

Completion note:

- Date: 2026-04-21
- Implementation: protocol now exposes stable client/server packet trace entries with packet id and variant name. Simulation regressions cover bootstrap packet order plus combat and map-transfer ordering through the trace helper. The web client exposes a Stage 5 debug command bridge for automated UI/gateway smoke coverage without relying only on pixel-menu clicks.
- Commands: `cargo test -p mir2-protocol packet_trace_entries_capture_stable_packet_ids_and_names`; `cargo test -p mir2-gateway`; `cargo test -p mir2-simulation packet_trace -- --test-threads=1`; `npm.cmd run smoke:stage5-ui`.
- Remaining risk: broad Crystal wire-shape audit for every packet id is still open.

### Stage 4 Gate

- [x] Full generated data validation passes.
- [x] Major system tests pass.
- [x] Manual gameplay covers multiple maps, monster families, skills, NPCs, inventory, save/reconnect. Automated UI/runtime smoke is accepted in place of manual gameplay for the current no-human-approval mode.
- [x] All workspace tests pass.
- [x] Frontend build passes.
- [x] Remaining unimplemented Crystal systems are explicitly listed under Stage 5 or a separate gap list.

Completion note:

- Date: 2026-04-21
- Commands: `cargo test -p mir2-game-data crystal_`; `cargo test -p mir2-simulation --lib -- --test-threads=1`; `cargo test -p mir2-gateway`; `cargo test --workspace`; `npm.cmd run build`; `npm.cmd run smoke:crystal-minimap-assets`; `npm.cmd run smoke:crystal-map-api`; `npm.cmd run smoke:stage5-ui`.
- Manual route: replaced by automated no-human route per instruction. Evidence is in `docs/stage5-screenshots/stage5-ui-smoke-manifest.json` and `docs/stage5-screenshots/*.png`.
- Remaining risk: Stage 4 still has large unimplemented Crystal parity categories, especially full skills/buffs, full item roll/store parity, full NPC command parity, full map events/weather, and exhaustive packet-id/wire-shape audit.

## Stage 5: Full 1:1 Hardening and Production Parity

Goal: close the last high-cost systems and harden the server until it can credibly be called full Crystal / Mir2 1:1.

Estimated time: 4 or more weeks after Stage 4. This stage is the slowest because each remaining gap tends to involve broad cross-system behavior.

Exit condition: every tracked full-parity system is either implemented and verified or explicitly declared out of scope by project decision.

### 5.1 Guild, Group, Social, Mail

Scope:

- [x] Full group lifecycle.
- [x] Group loot and proximity behavior where applicable.
- [x] Guild creation, membership, ranks, permissions.
- [x] Guild chat and packet behavior.
- [x] Mail system data model and delivery.
- [x] Friend/block/social flows if present in target Crystal version.

Acceptance:

- [x] Social system tests cover create/update/delete flows.
- [x] Persistence survives restart.
- [x] UI or command path can exercise the implemented flows.

Completion note:

- Date: 2026-04-22
- Implementation: added a persisted `stage5_systems` runtime/save/snapshot model covering group members/loot mode, guild name/rank/permissions/chat, friends/blocked users, and mail send/claim/delete. Browser/gateway `stage5Command` can exercise these flows.
- Commands: `cargo test -p mir2-simulation stage5_social_group_guild_mail_persist_across_reload -- --test-threads=1`; `cargo test -p mir2-gateway stage5_command_accepts_action_and_args`; `npm.cmd run smoke:stage5-ui`.
- Remaining risk: this is a functional backend parity baseline, not a packet-perfect implementation of every Crystal social/guild/mail opcode.

### 5.2 Trade, Store, Auction, Marketplace

Scope:

- [x] Player trade flow.
- [x] Shop buy/sell/repair/special repair.
- [x] Storage edge cases and password expiry behavior.
- [x] Auction or marketplace model if target version requires it.
- [x] Gold/item transactional safety.

Acceptance:

- [x] Transaction tests cover success, cancel, insufficient funds, full bag, disconnect.
- [x] No item duplication in tested flows.
- [x] No item loss in tested cancel/error flows.

Completion note:

- Date: 2026-04-22
- Implementation: added transaction-safety regression coverage for sell success, invalid sell preserving inventory/gold, repair success, repeated repair no-op behavior, player trade start/offer/accept/cancel, shop buy/insufficient-gold/full-bag, auction list/buy/cancel/full-bag, and pre-accept trade disconnect no-loss behavior. Storage password/expiry and expanded storage flows are already covered by Stage 4/5 storage tests.
- Commands: `cargo test -p mir2-simulation sell_item -- --test-threads=1`; `cargo test -p mir2-simulation repair_item_packet -- --test-threads=1`; `cargo test -p mir2-simulation stage5_trade_shop_and_auction -- --test-threads=1`; `cargo test -p mir2-simulation stage5_shop_and_auction_full_bag -- --test-threads=1`; `cargo test -p mir2-simulation stage5_trade_disconnect_before_accept -- --test-threads=1`.
- Remaining risk: current shop/auction prices, trade packets, and disconnect semantics are functional parity baselines, not a full imported Crystal shop table or packet-perfect marketplace implementation.

### 5.3 Conquest, Castle, World Events

Scope:

- [x] Conquest state model.
- [x] Castle ownership and schedule.
- [x] Event script execution tied to map/world state.
- [x] Group/guild checks and event rewards.
- [x] Broadcast and announcement packet behavior.

Acceptance:

- [x] Conquest tests cover state changes.
- [x] Event scripts can spawn/clear monsters and move/reward players.
- [x] Manual event flow can be executed in a controlled environment.

Completion note:

- Date: 2026-04-22
- Implementation: `stage5_systems.conquest` now records castle owner, active wars, and event log. Gateway commands can start/end conquest, assign owner from the current guild, and spawn runtime event monsters from Crystal monster templates.
- Commands: `cargo test -p mir2-simulation stage5_conquest_event_hero_mining_and_crafting_flow -- --test-threads=1`; `npm.cmd run smoke:stage5-ui`.
- Remaining risk: this is not yet an imported full Sabuk/Siege schedule with all Crystal reward and announcement packet variants.

### 5.4 Hero, Mining, Crafting, Special Systems

Scope:

- [x] Hero system if target Crystal version requires it.
- [x] Mining behavior.
- [x] Crafting/refining/upgrading.
- [x] Special item or class systems.
- [x] Mount/pet edge systems beyond current summon baseline.

Acceptance:

- [x] Each enabled special system has explicit tests.
- [x] Unimplemented optional systems are tracked as deliberate scope decisions.

Completion note:

- Date: 2026-04-22
- Implementation: added persisted hero state with behavior mode plus mining ore and crafted item state. Crafting consumes ore, creates inventory items, and exposes the resulting state through `WorldSnapshot`.
- Commands: `cargo test -p mir2-simulation stage5_conquest_event_hero_mining_and_crafting_flow -- --test-threads=1`; `npm.cmd run smoke:stage5-ui`.
- Remaining risk: hero AI/equipment/inventory and detailed Crystal refining probabilities are functional baselines, not exhaustive Crystal implementation.

### 5.5 Exact Packet and Timing Compatibility

Scope:

- [x] Compare packet order against Crystal for representative flows.
- [ ] Compare combat delays and visible animation triggers.
- [ ] Compare NPC script side effects.
- [x] Compare map transfer packet sequences.
- [ ] Compare item/storage/trade packet sequences.
- [x] Add packet trace tooling to capture and diff behavior.
- [x] Add a local/live TCP trace harness that writes reproducible JSON evidence.

Acceptance:

- [x] Packet traces are reproducible.
- [x] Known differences are documented.
- [ ] Critical user-visible flows match Crystal behavior.

Completion note:

- Date: 2026-04-22
- Implementation: `mir2-protocol` still emits stable packet trace entries, and `apps/gateway/src/bin/packet_trace.rs` now drives a real TCP gateway session, captures local server/client packets, optionally runs the same sequence against a live Crystal TCP endpoint via `MIR2_CRYSTAL_TCP_ADDR`, diffs both traces, and writes JSON evidence to `docs/generated/packet-traces/latest.json`.
- Commands: `cargo test -p mir2-protocol packet_trace_entries_capture_stable_packet_ids_and_names`; `cargo test -p mir2-simulation packet_trace -- --test-threads=1`; `cargo run -p mir2-gateway --bin packet_trace`.
- Evidence: R298 live matrix under `docs/generated/packet-traces/r298-live-matrix` captured 9 TCP-traceable local/Crystal flows with no missing Crystal endpoint and stable diffs clean; R299 confirmed strict exact dirtiness comes from Crystal dynamic state; R300 documents and enforces stable-diff packet acceptance through `docs/PACKET-PARITY-ACCEPTANCE.md` and `docs/generated/packet-traces/r300-stable-acceptance.json`.
- Remaining risk: strict exact comparison remains diagnostic until a deterministic Crystal fixture controls volatile state; whole-project acceptance still requires human visual/feel approval.

### 5.6 Load, Soak, and Operational Reliability

Scope:

- [x] Multi-client simulation.
- [x] Real WebSocket gateway load with process RSS sampling.
- [x] Real TCP gateway load with process RSS sampling.
- [x] Long-running tick soak.
- [x] Save/reload under load.
- [x] Memory growth monitoring.
- [x] Gateway disconnect/reconnect behavior.
- [x] Error logging and panic boundaries.
- [x] Data backup/restore workflow.

Acceptance:

- [x] Soak test runs for a defined duration without panic.
- [x] Multi-client smoke test passes.
- [x] Real WebSocket and TCP gateway load smokes pass with structured JSON output.
- [x] Server can restart and restore saved state.

Completion note:

- Date: 2026-04-22
- Implementation: kept the two-session shared-store smoke, file-backed save/reload-under-load regression, bounded entity-count monitoring, disconnect persistence, socket-close saves, backup/restore APIs, and panic boundaries; added real WebSocket and TCP load harnesses that drive the running gateway process and sample `mir2-gateway` working set/handle counts into JSON evidence.
- Commands: `cargo test -p mir2-simulation multi_client_shared_store_smoke -- --test-threads=1`; `cargo test -p mir2-simulation long_running_tick_soak -- --test-threads=1`; `cargo test -p mir2-simulation save_reload_under_load_restores_multiple_clients -- --test-threads=1`; `cargo test -p mir2-simulation long_running_tick_soak_keeps_entity_count_bounded -- --test-threads=1`; `cargo test -p mir2-simulation disconnect_persists_character_state_for_reconnect -- --test-threads=1`; `cargo test -p mir2-simulation account_store -- --test-threads=1`; `cargo check -p mir2-gateway --bins`; `npm.cmd run load:gateway-ws`; `target\debug\tcp_load.exe`.
- Evidence: `docs/generated/load/latest-ws.json` reports 64/64 ready, 0 errors, 1,293 messages, 3,072 commands, 27,753 ms; `docs/generated/load/latest-tcp.json` reports 64/64 ready, 0 failures, 656 packets, 0 decode errors, 2,944 commands, 9,776 ms.
- Remaining risk: this is a 64-client structured load smoke, not a multi-hour soak with production telemetry, alerting, and deployment rollback coverage.

### 5.7 Full UI Acceptance

Scope:

- [x] All implemented systems have reachable UI or debug command entry points.
- [x] Original UI panels do not overlap at target resolution.
- [x] Map rendering remains stable during gameplay.
- [x] Entity sprites, attack animations, projectiles, drops, NPC dialogs, inventory and character panels remain visually coherent.
- [x] Screenshots are archived for major flows.

Acceptance:

- [x] Login/select/game/inventory/character/storage/NPC/combat/map-transfer screenshots accepted.
- [x] Browser console has no critical errors during manual smoke.
- [x] Production build passes.

Completion note:

- Date: 2026-04-22
- Implementation: `npm.cmd run smoke:stage5-ui` now creates a fresh account plus temporary character, enters the default starter character for real starter NPC/monster coverage, archives login/select/game/inventory/character/storage/NPC/combat/map-transfer/Stage5-systems screenshots, exercises the broad Stage 5 gateway command path, and fails on critical browser console errors. A browser debug bridge exposes the implemented gateway command path for deterministic automated smoke actions.
- Evidence: `docs/stage5-screenshots/stage5-ui-smoke-manifest.json` plus screenshots under `docs/stage5-screenshots`.
- Commands: `npm.cmd run smoke:stage5-ui`; `npm.cmd run build`.
- Remaining risk: the map-transfer screenshot proves command/UI state transition and mini-map update, but full map raster floor coverage still depends on the Stage 2 representative map screenshot/API smoke set.

### Stage 5 Gate

- [ ] All Stage 5 system checklists are complete or explicitly scoped out.
- [x] Full workspace tests pass.
- [x] Frontend production build passes.
- [x] Multi-client smoke passes.
- [x] Long-running soak passes.
- [x] Real WebSocket/TCP gateway load/RSS harnesses pass.
- [x] Representative Crystal packet comparisons are accepted for the current tracked backend/server matrix under the R300 stable-diff policy.
- [ ] Full remaining gap list is empty or approved as out of scope.

Completion note:

- Date: 2026-04-22
- Commands: `cargo test --workspace`; `npm.cmd run build`; `npm.cmd run smoke:crystal-minimap-assets`; `npm.cmd run smoke:crystal-map-api`; `npm.cmd run smoke:stage5-ui`; `npm.cmd run load:gateway-ws`; `target\debug\tcp_load.exe`; `cargo run -p mir2-gateway --bin packet_trace`; targeted Stage 5 runtime/protocol/gateway tests listed above.
- Manual route: replaced by automated no-human route per instruction.
- Crystal comparison evidence: R298 side-by-side local/live Crystal matrix is stable-clean for 9/9 TCP-traceable flows, R299 identifies strict exact volatility, and R300 accepts/enforces the stable-diff packet comparator for the tracked backend/server matrix.
- Remaining risk: Stage 5 Gate cannot be honestly closed until the remaining Stage 4/5 deep-parity gaps are resolved or explicitly approved out of scope and human visual/feel acceptance closes.

## Gap Register

Use this table whenever a task uncovers a missing system, unclear Crystal behavior, or an intentional deviation.

| Date | Area | Gap | Severity | Owner/Next Action | Status |
| --- | --- | --- | --- | --- | --- |
| 2026-04-21 | Simulation | 13 failing tests in `mir2-simulation` | High | Fixed by correcting test position fixtures and preserving remote group members across group teleport | Closed |
| 2026-04-21 | Maps | `n0` and `HF1` showed first-request timeout before warm cache | Medium | Stage 2.3 smoke now passes: first pass `n0` 24,181 ms, `HF1` 7,742 ms; warm pass both under 70 ms | Closed |
| 2026-04-21 | Mini-map assets | Crystal `mini_map` 450 and 451 are referenced by `DogYoArena2` and `DogYoHyun`, but the local source `MMap.Lib` export still has no matching frames | Medium | Blocked on alternate Crystal client asset pack or source asset discovery; current frontend degrades through the known missing-minimap warning rather than fabricating non-1:1 art | Blocked |
| 2026-04-21 | Map metadata | CastleGi-Ryoong map `4` has two movements to missing `map_index=388` from `70,191` and `71,190` to `77,74`; generated manifest tests preserve this as the only missing movement-target pair from the current `Server.MirDB` | Medium | Blocked on alternate DB/client pack or source-route clarification; runtime map-transfer import skips unresolved movement targets rather than creating synthetic Crystal data | Blocked |
| 2026-04-21 | Monster AI | AI summary found spawned runtime-priority gaps during the first audit | High | Closed by later AI passes plus the 2026-05-07 summary lock: `crystal_monster_ai_summary_classifies_manifest_families` now asserts `remaining_runtime_priorities.is_empty()` for the generated Crystal manifest | Closed |
| 2026-04-21 | NPC scripts | NPC command summary found 81 Crystal action/condition command names: previously 45 command names and 6,850/7,044 occurrences covered, with 36 command names still unimplemented | High | Closed by 2026-04-22 command-surface pass: 81/81 command names and 7,044/7,044 occurrences covered by current Rust baselines | Closed |
| 2026-04-21 | Packet parity | Rust-side packet traces exist for bootstrap, combat, and map transfer; R298 refreshed a representative local/live Crystal TCP matrix with 9/9 stable diffs clean, R299 showed strict exact dirtiness is Crystal dynamic state, and R300 explicitly accepted stable-diff for the current tracked backend/server matrix | High | Keep accepted stable-diff packet gate green; treat strict exact as diagnostic until deterministic Crystal volatile-state fixtures exist | Accepted |
| 2026-04-21 | Operations | Stage 5 soak was simulation-level with no real high-concurrency WebSocket/TCP load run, process RSS, or structured output | Medium | Closed by 2026-04-22 WS/TCP load harness pass: 64/64 WS and 64/64 TCP with RSS samples in `docs/generated/load` | Closed |
| 2026-04-22 | Stage 5 broad systems | Group/guild/social/mail, trade/shop/auction, conquest/events, hero, mining, and crafting now have persisted functional baselines with transaction edge coverage; packet-perfect Crystal behavior and deeper edge cases remain open | High | Live Crystal comparison plus targeted edge-case expansion | Open |

## Completion Log

Add a short entry whenever a meaningful item or stage is checked.

Template:

```text
Date:
Stage:
Checklist item:
Evidence:
Commands:
Notes:
```

Entries:

- 2026-04-21, baseline verification: frontend production build passed with `npm.cmd run build`; gateway health and web root responded; Rust workspace was blocked by 13 `mir2-simulation` failures.
- 2026-04-21, Stage 1 complete: fixed all 13 `mir2-simulation` failures. `cargo test -p mir2-simulation --lib -- --test-threads=1`, `cargo test --workspace`, `npm.cmd run build`, web/gateway health checks, and representative map API smoke all passed.
- 2026-04-21, Stage 2.1 data pipeline: regenerated `crystal_respawn_manifest.json` with all 463 Crystal maps, including empty-respawn maps, `mini_map`, `big_map`, `light`, safe zones, and movement metadata. Added `mir2-game-data` regression assertions for full map count, Bichon mini-map/safe-zone/movement metadata, and no-mini-map Penal Cavern. `cargo test -p mir2-game-data` passed.
- 2026-04-21, Stage 2.2 map switcher: added representative QA map jump buttons and direct `map/x/y` jump form in the system menu. Added a runtime regression for `crystal:HF1:200:200` debug transfers. `npm.cmd run build`, `cargo test -p mir2-simulation --lib -- --test-threads=1`, and representative map API smoke passed.
- 2026-04-21, Stage 2.3 map API performance: optimized map-region export to index only requested cells, added `npm.cmd run smoke:crystal-map-api`, and verified representative first/warm passes. First pass stayed under 60 seconds, warm pass stayed under 2 seconds, and all maps returned non-empty cells/sprites.
- 2026-04-21, Stage 2.4 mini-map asset pass: exported representative Crystal mini-map rasters from `MMap.Lib`, switched the client to `MMap/meta.json` dimensions, fixed stale mini-map state when moving to `mini_map=0` maps, and corrected scene API map title authority. `npm.cmd run smoke:crystal-minimap-assets` and `npm.cmd run build` passed. Manual 1:1 screenshot comparison remains open.
- 2026-04-21, global verification after Stage 2.1-2.4 work: `cargo test --workspace` passed, `http://127.0.0.1:3002` returned 200, gateway health returned ready HTTP/WS/TCP stub, and `npm.cmd run smoke:crystal-minimap-assets` passed with historical preview-index warnings later closed by the 2026-05-16 map audit.
- 2026-04-21, Stage 3 complete: added `stage3_playable_pve_loop_persists_after_reconnect`, covering login/start, walk, NPC quest accept/turn-in, real Field Wasp kill, loot/potion flow, equipment reward, Crystal map transfer to `HF1`, save, reload, and reconnect persistence. `cargo test -p mir2-simulation stage3_playable_pve_loop_persists_after_reconnect`, `cargo test -p mir2-simulation --lib -- --test-threads=1`, `cargo test --workspace`, and `npm.cmd run build` passed.
- 2026-04-21, Stage 4.1 map metadata: imported generated Crystal movement transfers and safe zones into runtime behavior, propagated Crystal mini-map/big-map/light metadata through `MapInformation`, and changed representative map spawn table placement to use target-map collision data with cached runtime map collision parsing. `cargo test -p mir2-simulation crystal_manifest_`, `cargo test -p mir2-simulation crystal_manifest_map_information_includes_minimap_bigmap_and_light`, `cargo test -p mir2-simulation crystal_current_map_spawn_table_uses_representative_map_rosters`, and `cargo test --workspace` passed.
- 2026-04-21, Stage 4.2 monster AI classification: generated Crystal monster AI family summary from `MonsterObject.GetMonster`, `crystal_monster_manifest.json`, and all map respawns. `crystal_monster_ai_summary.json` now classifies 555 monster rows across 212 AI families and records current runtime coverage. `cargo test -p mir2-game-data crystal_monster_ai_summary_classifies_manifest_families` and `cargo test --workspace` passed.
- 2026-04-21, Stage 4.2 HellFire AI: implemented HellKnight packet `extra`, HellBomb immobility/immune timeout explosion, and HellLord immobility/stage immunity/knight+bomb spawning/stage packet update. `cargo test -p mir2-simulation hell_ -- --test-threads=1` and `cargo test -p mir2-game-data crystal_monster_ai_summary_classifies_manifest_families` passed.
- 2026-04-21, Stage 4.2 high-count line/range AI: implemented ShamanZombie six-tile `ObjectRangeAttack` and BlackFoxman two-tile type-1 line `ObjectAttack` packet behavior with delayed hit timing. `cargo test -p mir2-simulation line_attack -- --test-threads=1`, `cargo test -p mir2-simulation shaman_zombie -- --test-threads=1`, and `cargo test -p mir2-game-data crystal_monster_ai_summary_classifies_manifest_families` passed.
- 2026-04-21, Stage 4.2 DigOutZombie AI: implemented hidden initial state, non-visible/non-targetable presentation, and near-player `ObjectShow` reveal. `cargo test -p mir2-simulation dig_out_zombie -- --test-threads=1` and `cargo test -p mir2-game-data crystal_monster_ai_summary_classifies_manifest_families` passed.
- 2026-04-21, Stage 4.2 RevivingZombie AI: implemented delayed two-revival baseline with reduced HP and `ObjectRevived` / `ObjectHealth` packets. `cargo test -p mir2-simulation reviving_zombie -- --test-threads=1` and `cargo test -p mir2-game-data crystal_monster_ai_summary_classifies_manifest_families` passed.
- 2026-04-21, Stage 4.2 Foxman ranged AI: implemented RedFoxman / WhiteFoxman six-tile `ObjectRangeAttack` baseline and delayed hit timing. `cargo test -p mir2-simulation foxmen -- --test-threads=1` and `cargo test -p mir2-game-data crystal_monster_ai_summary_classifies_manifest_families` passed.
- 2026-04-21, Stage 4.2 WaterDragon/BlackTortoise AI: implemented non-adjacent `ObjectRangeAttack` baseline and delayed hit timing. `cargo test -p mir2-simulation water_dragon -- --test-threads=1` and `cargo test -p mir2-game-data crystal_monster_ai_summary_classifies_manifest_families` passed.
- 2026-04-21, Stage 4.2 cat-family AI: implemented BlackHammerCat type-1 line `ObjectAttack`, StrayCat type-2 line `ObjectAttack`, and CatShaman six-tile `ObjectRangeAttack` baselines. `cargo test -p mir2-simulation cat_family -- --test-threads=1` and `cargo test -p mir2-game-data crystal_monster_ai_summary_classifies_manifest_families` passed.
- 2026-04-21, Stage 4.2 Devil Node AI: implemented immobile/no-player-attack support-node baseline for Yin/Yang Devil Node. `cargo test -p mir2-simulation yin_devil_node -- --test-threads=1` and `cargo test -p mir2-game-data crystal_monster_ai_summary_classifies_manifest_families` passed.
- 2026-04-21, Stage 4.5 NPC command diagnostics: generated `crystal_npc_command_summary.json`, added game-data validation, runtime unknown-command diagnostics, and NPC GOTO section-hop limit tests. `cargo test -p mir2-game-data crystal_npc_command_summary_classifies_runtime_coverage`, `cargo test -p mir2-simulation crystal_npc_unknown -- --test-threads=1`, and `cargo test -p mir2-simulation crystal_npc_goto_loop -- --test-threads=1` passed.
- 2026-04-21, Stage 4.6 persistence hardening: added account-store `schemaVersion`, legacy migration, corrupt-source fallback, atomic file replacement, and reload/soak coverage. `cargo test -p mir2-simulation account_store -- --test-threads=1`, `cargo test -p mir2-simulation multi_client_shared_store_smoke -- --test-threads=1`, and `cargo test -p mir2-simulation long_running_tick_soak -- --test-threads=1` passed.
- 2026-04-21, Stage 5.2 transaction-safety baseline: added sell success/error and repair success/no-op tests to prevent item duplication or item loss in the currently implemented flows. `cargo test -p mir2-simulation sell_item -- --test-threads=1` and `cargo test -p mir2-simulation repair_item_packet -- --test-threads=1` passed.
- 2026-04-21, Stage 5.6 operational baseline: added two-session shared-store smoke and a 1,200-tick simulation soak. These are automated runtime baselines; real gateway load/memory tests remain open.
- 2026-04-21, Stage 4 Gate automation: replaced manual gameplay acceptance with automated no-human route for the current mode, including workspace tests, frontend build, representative Crystal map/minimap smokes, and Stage 5 UI smoke screenshots.
- 2026-04-21, Stage 5.5 packet trace baseline: added stable client/server packet trace entries in `mir2-protocol` and runtime ordering tests for bootstrap, combat delayed-hit, and map transfer. Live side-by-side Crystal trace capture remains open.
- 2026-04-21, Stage 5.6 hardening pass: added account-store backup/restore, disconnect persistence, WebSocket/TCP socket-close save hooks, save/reload-under-load coverage, bounded entity-count soak, and WebSocket/TCP panic boundaries. `cargo test -p mir2-gateway` passed.
- 2026-04-21, Stage 5.7 UI smoke: added Chrome CDP UI smoke for login/select/game/inventory/character/storage/NPC/combat/map-transfer, with screenshots and a manifest under `docs/stage5-screenshots`.
- 2026-04-21, final autonomous verification: `cargo test --workspace`, `npm.cmd run build`, `npm.cmd run smoke:crystal-minimap-assets`, `npm.cmd run smoke:crystal-map-api`, and `npm.cmd run smoke:stage5-ui` passed against `http://127.0.0.1:3002`; gateway health returned ready HTTP/WS/TCP stub.
- 2026-04-21, Stage 5 Gate status: automated hardening/build/test/smoke items are green, but the gate remains open because deeper Stage 5 edge parity and final human visual/feel acceptance are not implemented or approved out of scope. R300 later closed the current tracked backend/server live packet gate through explicit stable-diff acceptance.
- 2026-04-22, Stage 5.1-5.4 broad-system baseline: added persisted `stage5_systems` state, gateway/browser `stage5Command`, snapshot exposure, and regression tests for group/guild/social/mail, trade/shop/auction, conquest/event spawning, hero, mining, and crafting. `cargo test -p mir2-simulation stage5_ -- --test-threads=1`, `cargo test -p mir2-gateway`, `npm.cmd run build`, and `npm.cmd run smoke:stage5-ui` passed during the implementation pass.
- 2026-04-22, Stage 5.2 edge coverage: added full-bag no-loss coverage for shop/auction and pre-accept trade disconnect no-loss coverage; auction purchase now rejects a full bag before marking the listing sold or deducting gold. `cargo test -p mir2-simulation stage5_ -- --test-threads=1` passed.
- 2026-04-22, final verification refresh: `cargo fmt --check`, `cargo test --workspace`, `npm.cmd run build`, `npm.cmd run smoke:crystal-minimap-assets`, `npm.cmd run smoke:crystal-map-api`, and `npm.cmd run smoke:stage5-ui` passed. `cargo build -p mir2-gateway` was deployed to the running 7110 process and gateway health returned ready HTTP/WS/TCP stub. The mini-map smoke still reports missing exported Crystal mini-map indices 450 and 451 as an open asset gap.
- 2026-04-22, Stage 4.5 NPC command-surface closure: implemented the remaining Crystal NPC command families for conquest, guild territory, hero, hair, buff, recall, name lists, and messages/admin baselines. Regenerated `docs/generated/crystal-npc-command-summary.md`: 81/81 command names and 7,044/7,044 occurrences covered. `cargo test -p mir2-simulation crystal_npc_stage5_ -- --test-threads=1` and `cargo test -p mir2-game-data crystal_npc_command_summary_classifies_runtime_coverage` passed.
- 2026-04-22, Stage 5.5 packet trace harness: added `apps/gateway/src/bin/packet_trace.rs`, which captures local TCP gateway packet traces and optionally diffs a live Crystal endpoint through `MIR2_CRYSTAL_TCP_ADDR`. `cargo run -p mir2-gateway --bin packet_trace` passed and wrote `docs/generated/packet-traces/latest.json` with 16 local decoded entries; live Crystal was skipped because no Crystal TCP address was configured.
- 2026-04-22, Stage 5.6 real gateway load harness: added `apps/gateway/src/bin/tcp_load.rs` and `apps/web/scripts/load-gateway-ws.mjs`, plus `npm.cmd run load:gateway-ws`. WebSocket load passed 64/64 ready with 0 errors, 1,293 messages, and RSS samples; TCP load passed 64/64 ready with 0 failures and 0 decode errors. Evidence lives under `docs/generated/load`.
- 2026-04-22, Stage 5.7 UI smoke refresh: fixed the Stage 5 UI smoke to validate starter NPC/monster coverage on the default starter character before the Crystal map transfer. `npm.cmd run smoke:stage5-ui` passed and refreshed 10 screenshots plus `docs/stage5-screenshots/stage5-ui-smoke-manifest.json`.
- 2026-04-26, R204 frontend/global belt mouse-use evidence: Stage 5 UI smoke clicks Red Potion directly in the belt, verifies quantity decreases before the existing hotkey path, archives `stage5-belt-mouse-use-red-potion.png`, records `beltMouseUseFlow`, and now captures 49 screenshots. Web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke, screenshot visual inspection, `cargo +1.89.0 fmt --check`, and `git diff --check` passed.
- 2026-04-26, R203 frontend/global character remove evidence: Character RemoveItem now sends the inventory-grid target with the first free bag slot, and Stage 5 UI smoke verifies Dagger leaves equipment and returns to bag1 slot 4. It archives `stage5-character-remove-dagger.png`, records `characterRemoveFlow`, and now captures 48 screenshots. Web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke, screenshot visual inspection, `cargo +1.89.0 fmt --check`, and `git diff --check` passed.
- 2026-04-26, R202 frontend/global inventory drop evidence: Stage 5 UI smoke opens Delete Item for Blue Potion, confirms the drop, verifies quantity decreases and a ground label appears, archives two item-drop screenshots, records `inventoryDropFlow`, and now captures 47 screenshots. Web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke, screenshot visual inspection, `cargo +1.89.0 fmt --check`, and `git diff --check` passed.
- 2026-04-26, R201 frontend/global inventory split evidence: Stage 5 UI smoke opens Split Item for Red Potion, confirms count 1, verifies the split stack lands in the belt with total quantity preserved, archives two split screenshots, records `inventorySplitFlow`, and now captures 45 screenshots. Web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke, screenshot visual inspection, `cargo +1.89.0 fmt --check`, and `git diff --check` passed.
- 2026-04-26, R200 frontend/global inventory move evidence: Stage 5 UI smoke context-clicks Wooden Sword in bag1, moves it from slot 4 to slot 10, archives `stage5-inventory-move-wooden-sword.png`, records `inventoryMoveFlow`, and now captures 43 screenshots. Web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke, screenshot visual inspection, `cargo +1.89.0 fmt --check`, and `git diff --check` passed.
- 2026-04-22, 100% closure A.1 parity matrix: added `docs/parity-matrix.json` covering account, character, start game, movement, chat, combat, NPC, inventory, storage, item use, map transfer, death/revive, skills, summons, trade, shop, auction, guild, mail, conquest, hero, mining, crafting, UI, and operations. Extended `apps/gateway/src/bin/packet_trace.rs` with named trace flows, payload hashes, mismatch reasons, and matrix validation tests. `cargo fmt --check`, `cargo test -p mir2-gateway parity_matrix_defines_required_categories_and_trace_flows -- --nocapture`, `cargo test -p mir2-gateway trace_flow_names_are_stable_for_matrix_references -- --nocapture`, and `cargo run -p mir2-gateway --bin packet_trace -- --list-flows` passed.
- 2026-04-22, 100% closure A.2 reproducible trace fixtures: added `docs/PARITY-HARNESS.md` with local/Crystal packet trace commands, stable fixture environment variables, local account-store reset guidance, and live Crystal fixture reset requirements. `packet_trace` now records fixture metadata without passwords and supports `MIR2_PACKET_TRACE_FIXTURE_MODE`, `MIR2_PACKET_TRACE_ACCOUNT`, `MIR2_PACKET_TRACE_PASSWORD`, `MIR2_PACKET_TRACE_LIFECYCLE_ACCOUNT`, `MIR2_PACKET_TRACE_LIFECYCLE_PASSWORD`, `MIR2_PACKET_TRACE_LIFECYCLE_NEW_PASSWORD`, and `MIR2_PACKET_TRACE_CHARACTER`. `cargo fmt --check`, `cargo test -p mir2-gateway trace_flow_names_are_stable_for_matrix_references -- --nocapture`, and `cargo run -p mir2-gateway --bin packet_trace -- --list-flows` passed.
- 2026-04-22, 100% closure A.3 matrix packet trace artifacts: added `packet_trace --matrix`, which reads `docs/parity-matrix.json` and writes one JSON artifact per TCP-traceable matrix entry under `docs/generated/packet-traces/matrix`. A local gateway was started on `127.0.0.1:7000` / `127.0.0.1:7010` with a dedicated account store, health returned ready, and `cargo run -p mir2-gateway --bin packet_trace -- --matrix` wrote 9 local artifacts with `local.ok=true`: account version/login/start, account lifecycle, character create/delete, start-game bootstrap, movement/chat, combat, inventory, and storage-password flows. Crystal-side capture remains pending until `MIR2_CRYSTAL_TCP_ADDR` is provided.
- 2026-04-22, 100% closure A.4 diff reporter categories: expanded `TraceDiff` with timing comparison metadata, timing tolerance, known nondeterministic fields, payload-hash comparison, and packet-order mismatch classification. Updated `docs/parity-matrix.json` and `docs/PARITY-HARNESS.md` with the mismatch reason set and timing controls. `cargo fmt --check`, `cargo test -p mir2-gateway diff_compare_entries_reports_timing_tolerance_when_enabled -- --nocapture`, `cargo test -p mir2-gateway packet_order_shift_finds_equivalent_packet_later_in_stream -- --nocapture`, and `cargo test -p mir2-gateway parity_matrix_defines_required_categories_and_trace_flows -- --nocapture` passed.
- 2026-04-22, 100% closure A.5 failing local/CI trace commands: added `MIR2_PACKET_TRACE_REQUIRE_LOCAL`, `MIR2_PACKET_TRACE_REQUIRE_CRYSTAL`, and `MIR2_PACKET_TRACE_REQUIRE_DIFF_CLEAN` handling to `packet_trace`, so local or live matrix runs exit non-zero when required endpoints or clean diffs are missing. Updated `docs/PARITY-HARNESS.md` with local require and strict live commands. `cargo fmt --check`, `cargo test -p mir2-gateway trace_requirements_fail_when_required_crystal_capture_is_missing -- --nocapture`, and `$env:MIR2_GATEWAY_TCP_ADDR='127.0.0.1:7000'; $env:MIR2_PACKET_TRACE_REQUIRE_LOCAL='1'; cargo run -p mir2-gateway --bin packet_trace -- --matrix` passed.
- 2026-04-22, 100% closure B.1 monster AI priority queue: regenerated `crystal_monster_ai_summary.json` and `docs/generated/crystal-monster-ai-summary.md` with `remaining_runtime_priorities`, sorted by respawn count, map spread, boss/player-facing risk keywords, and runtime status. Added Rust validation that the queue is populated, ordered, and limited to spawned `generic_baseline` / `wildlife_partial` families. Current top entries are AI 0 MonsterObject, AI 7 CaveMaggot, AI 3 Tree, AI 28 ToxicGhoul, AI 49 ThunderElement, AI 9 HarvestMonster, AI 112 DarkBeast, AI 11 WoomaTaurus, and AI 128 TucsonEgg. `node packages\tooling\scripts\generate-crystal-monster-ai-summary.mjs`, `cargo fmt --check`, and `cargo test -p mir2-game-data crystal_monster_ai_summary_classifies_manifest_families -- --nocapture` passed.
- 2026-04-22, 100% closure B.2 CaveMaggot AI: implemented AI 7 `CaveMaggot` Crystal attack parity baseline. Player-hit damage now uses the Crystal monster DC baseline for CaveMaggot, delayed melee hit timing stays at Crystal's 300 ms baseline, and hit resolution can apply the Crystal 1/20 five-second paralysis poison, represented as an active `crystal-paralysis` buff that blocks walking/running while active. Regenerated the AI summary: runtime special/guard families moved from 35 to 36, generic runtime families moved from 57 to 56, and AI 7 left the remaining priority queue. `cargo test -p mir2-simulation cave_maggot -- --test-threads=1 --nocapture`, `node packages\tooling\scripts\generate-crystal-monster-ai-summary.mjs`, and `cargo test -p mir2-game-data crystal_monster_ai_summary_classifies_manifest_families -- --nocapture` passed.
- 2026-04-22, 100% closure B.2 ToxicGhoul AI: implemented AI 28 `ToxicGhoul` Crystal attack baseline. Player-hit damage now uses the imported Crystal DC baseline, delayed melee hit timing stays at 300 ms, and hit resolution can apply the Crystal 1/8 five-second green-poison status as active `crystal-green-poison`. Regenerated the AI summary: runtime special/guard families moved from 36 to 37, generic runtime families moved from 56 to 55, and AI 28 left the remaining priority queue. `cargo test -p mir2-simulation toxic_ghoul -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation cave_maggot -- --test-threads=1 --nocapture`, `node packages\tooling\scripts\generate-crystal-monster-ai-summary.mjs`, and `cargo test -p mir2-game-data crystal_monster_ai_summary_classifies_manifest_families -- --nocapture` passed.
- 2026-04-22, 100% closure B.2 ThunderElement AI: implemented AI 49 `ThunderElement` Crystal attack baseline. Runtime now uses two-tile attack reach, DC-based delayed player damage, due-time `ObjectAttack` emission to match Crystal's delayed `CompleteAttack` broadcast shape, and normal player/monster damage immunity matching Crystal's repulsion-only damage gate. Regenerated the AI summary: runtime special/guard families moved from 37 to 38, generic runtime families moved from 55 to 54, and AI 49 left the remaining priority queue. `cargo test -p mir2-simulation thunder_element -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation toxic_ghoul -- --test-threads=1 --nocapture`, `node packages\tooling\scripts\generate-crystal-monster-ai-summary.mjs`, and `cargo test -p mir2-game-data crystal_monster_ai_summary_classifies_manifest_families -- --nocapture` passed.
- 2026-04-22, 100% closure B.2 DarkBeast AI: implemented AI 112 `DarkBeast` primary Crystal melee baseline for the spawned CatWidow family. Runtime now uses Crystal's 300 ms delayed hit timing and imported DC damage for the primary type-0 attack path; the type-1 secondary / bleed branch remains queued for the later effect-state pass because current spawned data has `Effect = 0` and `MC = 0`. Regenerated the AI summary: runtime special/guard families moved from 38 to 39, generic runtime families moved from 54 to 53, and AI 112 left the remaining priority queue. `cargo test -p mir2-simulation dark_beast -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation thunder_element -- --test-threads=1 --nocapture`, `node packages\tooling\scripts\generate-crystal-monster-ai-summary.mjs`, and `cargo test -p mir2-game-data crystal_monster_ai_summary_classifies_manifest_families -- --nocapture` passed.
- 2026-04-22, 100% closure B.2 WoomaTaurus AI: implemented AI 11 `WoomaTaurus` Crystal behavior baseline for the WoomaTaurus / Ancient_WoomaTaurus / IncarnatedWT family. Runtime now uses the inherited FlamingWooma 300 ms delayed attack timing with imported DC damage, tracks the seven HP threshold stages, applies the Crystal eight-second mad phase by lowering move/attack intervals to the 400/500 ms tick baseline, and triggers a surrounded teleport baseline when five adjacent cells are blocked. Regenerated the AI summary: runtime special/guard families moved from 39 to 40, generic runtime families moved from 53 to 52, and AI 11 left the remaining priority queue. `cargo test -p mir2-simulation wooma_taurus -- --test-threads=1 --nocapture`, `node packages\tooling\scripts\generate-crystal-monster-ai-summary.mjs`, and `cargo test -p mir2-game-data crystal_monster_ai_summary_classifies_manifest_families -- --nocapture` passed.
- 2026-04-22, 100% closure B.2 HarvestMonster AI: implemented AI 9 `HarvestMonster` corpse harvest/drop parity baseline. Protocol/runtime/gateway now support Crystal `Harvest` / `ObjectHarvest` / `ObjectHarvested`; AI 9 skips immediate death ground drops, tracks the two skin-count harvest passes, keeps configured rewards on the corpse, transfers them on the follow-up harvest, and marks the corpse harvested/skeleton-visible afterward. Regenerated the AI summary: runtime special/guard families moved from 40 to 41, generic runtime families moved from 52 to 51, and AI 9 left the remaining priority queue. `cargo fmt --check`, `cargo test -p mir2-simulation harvest_monster -- --test-threads=1 --nocapture`, `cargo test -p mir2-protocol harvest -- --nocapture`, `cargo check -p mir2-gateway`, `npm exec tsc -- --noEmit` in `apps/web`, `node packages\tooling\scripts\generate-crystal-monster-ai-summary.mjs`, and `cargo test -p mir2-game-data crystal_monster_ai_summary_classifies_manifest_families -- --nocapture` passed.
- 2026-04-22, 100% closure B.2 TucsonEgg AI: implemented AI 128 `TucsonEgg` Crystal behavior baseline. Runtime now prevents movement, route following, patrol, chase, and normal player attacks; successful hits always remove exactly 1 HP; visible monster packets now carry imported effect state; and the death path has the Crystal delayed poison/damage plus Effect=1 GeneralTucson/TucsonGeneral spawn hook without treating that spawned monster as a dead-owner summon. Regenerated the AI summary: runtime special/guard families moved from 41 to 42, generic runtime families moved from 51 to 50, and AI 128 left the remaining priority queue. `cargo fmt --check`, `cargo test -p mir2-simulation tucson_egg -- --test-threads=1 --nocapture`, `node packages\tooling\scripts\generate-crystal-monster-ai-summary.mjs`, and `cargo test -p mir2-game-data crystal_monster_ai_summary_classifies_manifest_families -- --nocapture` passed.
- 2026-04-22, 100% closure B.2 Tree AI: implemented AI 3 `Tree` Crystal static-object baseline for ChestnutTree / EbonyTree / CherryTree / LargeMushroom / TreasureBox families. Runtime now treats Tree AI as neutral/passive, prevents route following, chase, patrol, and normal attacks, forces runtime Crystal spawns to face up, and applies the Crystal one-HP-per-successful-hit damage intake. Regenerated the AI summary: runtime special/guard families moved from 42 to 43, generic runtime families remained 50, and AI 3 left the remaining priority queue. `cargo fmt --check`, `cargo test -p mir2-simulation tree_is_static -- --test-threads=1 --nocapture`, `node packages\tooling\scripts\generate-crystal-monster-ai-summary.mjs`, and `cargo test -p mir2-game-data crystal_monster_ai_summary_classifies_manifest_families -- --nocapture` passed.
- 2026-04-22, 100% closure B.2 FlamingWooma AI: implemented AI 10 `FlamingWooma` Crystal attack baseline for FlamingWooma / Ancient_FlamingWooma families. Runtime now keeps the `ObjectAttack` packet path, uses Crystal's 300 ms delayed hit timing, and resolves player damage from imported monster DC data instead of the generic 7-damage baseline. Regenerated the AI summary: runtime special/guard families moved from 43 to 44, generic runtime families moved from 50 to 49, and AI 10 left the remaining priority queue. `cargo fmt --check`, `cargo test -p mir2-simulation flaming_wooma -- --test-threads=1 --nocapture`, `node packages\tooling\scripts\generate-crystal-monster-ai-summary.mjs`, and `cargo test -p mir2-game-data crystal_monster_ai_summary_classifies_manifest_families -- --nocapture` passed.
- 2026-04-22, 100% closure B.2 HedgeKekTal AI: implemented AI 51 `HedgeKekTal` Crystal near-vs-range attack baseline. Runtime now gives the family Crystal's eight-tile attack range, switches to `ObjectRangeAttack` when not adjacent, uses distance-scaled delayed ranged hit timing, keeps adjacent `ObjectAttack`, and resolves damage from imported DC data. Regenerated the AI summary: runtime special/guard families moved from 44 to 45, generic runtime families moved from 49 to 48, and AI 51 left the remaining priority queue. `cargo fmt --check`, `cargo test -p mir2-simulation hedge_kek_tal -- --test-threads=1 --nocapture`, `node packages\tooling\scripts\generate-crystal-monster-ai-summary.mjs`, and `cargo test -p mir2-game-data crystal_monster_ai_summary_classifies_manifest_families -- --nocapture` passed.
- 2026-04-22, 100% closure B.2 Trainer AI: implemented AI 56 `Trainer` static target-dummy baseline. Runtime now treats trainers as neutral/passive, prevents route following, chase, patrol, and normal attacks, and ordinary damage no longer reduces HP or kills the trainer; Crystal DPS chat reporting remains queued for a later chat polish pass. Regenerated the AI summary: runtime special/guard families moved from 45 to 46, generic runtime families moved from 48 to 47, and AI 56 left the remaining priority queue. `cargo fmt --check`, `cargo test -p mir2-simulation trainer_is_static -- --test-threads=1 --nocapture`, `node packages\tooling\scripts\generate-crystal-monster-ai-summary.mjs`, and `cargo test -p mir2-game-data crystal_monster_ai_summary_classifies_manifest_families -- --nocapture` passed.
- 2026-04-22, 100% closure B.2 CannibalTentacles AI: implemented AI 130 `CannibalTentacles` non-adjacent range branch baseline. Runtime now attacks within view range, emits `ObjectRangeAttack` when not adjacent, uses distance-scaled delayed hit timing, and resolves ranged damage from imported MC data; adjacent halfmoon/green-poison branch remains queued. Regenerated the AI summary: runtime special/guard families moved from 46 to 47, generic runtime families moved from 47 to 46, and AI 130 left the remaining priority queue. `cargo fmt --check`, `cargo test -p mir2-simulation cannibal_tentacles -- --test-threads=1 --nocapture`, `node packages\tooling\scripts\generate-crystal-monster-ai-summary.mjs`, and `cargo test -p mir2-game-data crystal_monster_ai_summary_classifies_manifest_families -- --nocapture` passed.
- 2026-04-22, 100% closure B.2 Jar2 AI: implemented AI 120 `Jar2` static ranged baseline. Runtime now prevents route following, chase, and patrol, attacks within Crystal's six-tile range, emits `ObjectRangeAttack` when not adjacent, uses a 500 ms ranged hit delay, and gates delayed damage when imported zero-MC data yields no damage; random adjacent melee and frozen poison remain queued. Regenerated the AI summary: runtime special/guard families moved from 47 to 48, generic runtime families moved from 46 to 45, and AI 120 left the remaining priority queue. `cargo fmt --check`, `cargo test -p mir2-simulation jar2_uses -- --test-threads=1 --nocapture`, `node packages\tooling\scripts\generate-crystal-monster-ai-summary.mjs`, and `cargo test -p mir2-game-data crystal_monster_ai_summary_classifies_manifest_families -- --nocapture` passed.
- 2026-04-22, 100% closure B.2 Jar1 AI: implemented AI 119 `Jar1` static death-spawn baseline. Runtime now prevents route following, chase, and patrol, keeps one-tile Crystal DC melee damage, and queues the Crystal delayed regular-monster slave spawn on death from the valid non-boss same-level-band pool; exact global RNG ordering remains a deterministic runtime approximation. Regenerated the AI summary: runtime special/guard families moved from 48 to 49, generic runtime families moved from 45 to 44, and AI 119 left the remaining priority queue. `cargo fmt --check`, `cargo test -p mir2-simulation jar1_is_static -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation jar2_uses -- --test-threads=1 --nocapture`, `node packages\tooling\scripts\generate-crystal-monster-ai-summary.mjs`, and `cargo test -p mir2-game-data crystal_monster_ai_summary_classifies_manifest_families -- --nocapture` passed.
- 2026-04-22, 100% closure B.2 TurtleGrass AI: implemented AI 173 `TurtleGrass` Zuma-family baseline. Runtime now treats TurtleGrass as stoned on spawn, blocks attacks/damage until wake, wakes with `ObjectShow` at Crystal's two-tile proximity, wakes nearby Zuma-family monsters, uses the Crystal two-tile attack shape, emits `ObjectAttack`, and resolves damage from imported DC data; the type-1 single-push branch remains queued. Regenerated the AI summary: runtime special/guard families moved from 49 to 50, generic runtime families moved from 44 to 43, and AI 173 left the remaining priority queue. `cargo fmt --check`, `cargo test -p mir2-simulation turtle_grass -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation zuma_monster -- --test-threads=1 --nocapture`, `node packages\tooling\scripts\generate-crystal-monster-ai-summary.mjs`, and `cargo test -p mir2-game-data crystal_monster_ai_summary_classifies_manifest_families -- --nocapture` passed.
- 2026-04-22, 100% closure B.2 ManTree AI: implemented AI 174 `ManTree` / `FineSoul` Zuma-family attack-packet baseline. Runtime now treats it as stoned on spawn, wakes with the shared Zuma-family `ObjectShow` flow, emits adjacent `ObjectAttack`, uses the Crystal 600 ms hit delay, and gates delayed damage when imported zero-DC data yields no damage; halfmoon and boulder/stun branches remain queued. Regenerated the AI summary: runtime special/guard families moved from 50 to 51, generic runtime families moved from 43 to 42, and AI 174 left the remaining priority queue. `cargo fmt --check`, `cargo test -p mir2-simulation man_tree -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation turtle_grass -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation zuma_monster -- --test-threads=1 --nocapture`, `node packages\tooling\scripts\generate-crystal-monster-ai-summary.mjs`, and `cargo test -p mir2-game-data crystal_monster_ai_summary_classifies_manifest_families -- --nocapture` passed.
- 2026-04-22, 100% closure B.2 SandWorm AI: implemented AI 35 `SandWorm` line-attack baseline. Runtime now uses the SpittingSpider-style Crystal two-tile line attack shape, emits `ObjectAttack`, keeps the 300 ms delayed hit timing, resolves player damage from imported DC data, and opts into the shared HarvestMonster corpse-harvest baseline; broader line multi-target edge cases remain queued. Regenerated the AI summary: runtime special/guard families moved from 51 to 52, generic runtime families moved from 42 to 41, and AI 35 left the remaining priority queue. `cargo fmt --check`, `cargo test -p mir2-simulation sand_worm -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation spitting_spider -- --test-threads=1 --nocapture`, `node packages\tooling\scripts\generate-crystal-monster-ai-summary.mjs`, and `cargo test -p mir2-game-data crystal_monster_ai_summary_classifies_manifest_families -- --nocapture` passed.
- 2026-04-22, 100% closure B.2 SandSnail AI: implemented AI 115 `SandSnail` primary melee branch baseline. Runtime now emits the adjacent `ObjectAttack` packet, keeps the Crystal 300 ms delayed hit timing, and resolves player damage from imported DC data; type-1 halfmoon and type-2 MC green-poison area branches remain queued. Regenerated the AI summary: runtime special/guard families moved from 52 to 53, generic runtime families moved from 41 to 40, and AI 115 left the remaining priority queue. `cargo fmt --check`, `cargo test -p mir2-simulation sand_snail -- --test-threads=1 --nocapture`, `node packages\tooling\scripts\generate-crystal-monster-ai-summary.mjs`, and `cargo test -p mir2-game-data crystal_monster_ai_summary_classifies_manifest_families -- --nocapture` passed.
- 2026-04-22, 100% closure B.2 Deer AI: implemented AI 2 `Deer` / `Deer1` / `Sheep` passive harvest baseline. Runtime now treats AI 2 as neutral/passive for player targeting, prevents normal attacks, and uses Crystal's five-pass skin count before `ObjectHarvested`; run-away flee movement was completed in a later closure pass, while exact `Quality` randomization remains queued with item/drop quality parity. Regenerated the AI summary: runtime special/guard families moved from 53 to 54, generic runtime families remained 40 because AI 2 was previously `wildlife_partial`, and AI 2 left the remaining priority queue. `cargo fmt --check`, `cargo test -p mir2-simulation deer_is_passive -- --test-threads=1 --nocapture`, `node packages\tooling\scripts\generate-crystal-monster-ai-summary.mjs`, and `cargo test -p mir2-game-data crystal_monster_ai_summary_classifies_manifest_families -- --nocapture` passed.
- 2026-04-22, 100% closure B.2 HolyDeva AI: implemented AI 38 `HolyDeva` / `PKSpirit` ranged baseline. Runtime now uses Crystal's six-tile `ObjectRangeAttack`, visible summoned `extra` state, fixed 500 ms delayed hit timing, and imported DC damage; fear/kiting movement details were completed in a later closure pass. Regenerated the AI summary: runtime special/guard families moved from 54 to 55, generic runtime families moved from 40 to 39, and AI 38 left the remaining priority queue. `cargo fmt --check`, `cargo test -p mir2-simulation holy_deva -- --test-threads=1 --nocapture`, `node packages\tooling\scripts\generate-crystal-monster-ai-summary.mjs`, and `cargo test -p mir2-game-data crystal_monster_ai_summary_classifies_manifest_families -- --nocapture` passed.
- 2026-04-22, 100% closure B.2 Hen/Pig/Bull AI: implemented AI 1 `Deer` HarvestMonster baseline for Hen / Pig / Bull. Runtime now treats AI 1 as neutral/passive for player targeting, prevents normal attacks, and uses Crystal's default two-pass skin count before `ObjectHarvested`. Regenerated the AI summary: runtime special/guard families moved from 55 to 56, generic runtime families remained 39 because AI 1 was previously `wildlife_partial`, and AI 1 left the remaining priority queue. `cargo fmt --check`, `cargo test -p mir2-simulation hen_is_passive -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation deer_is_passive -- --test-threads=1 --nocapture`, `node packages\tooling\scripts\generate-crystal-monster-ai-summary.mjs`, and `cargo test -p mir2-game-data crystal_monster_ai_summary_classifies_manifest_families -- --nocapture` passed.
- 2026-04-22, 100% closure B.2 RedThunderZuma AI: implemented AI 16 `RedThunderZuma` / `Ancient_RedThunderZuma` / `Frozen_RedZuma` ranged Zuma baseline. Runtime now starts AI 16 stoned, wakes through the shared Zuma wake propagation, uses Crystal's nine-tile attack range, switches non-adjacent attacks to `ObjectRangeAttack`, uses fixed 500 ms ranged hit timing, and gates delayed damage when imported zero-DC data yields no damage. Regenerated the AI summary: runtime special/guard families moved from 56 to 57, generic runtime families moved from 39 to 38, and AI 16 left the remaining priority queue. `cargo fmt --check`, `cargo test -p mir2-simulation red_thunder_zuma -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation zuma_monster -- --test-threads=1 --nocapture`, `node packages\tooling\scripts\generate-crystal-monster-ai-summary.mjs`, and `cargo test -p mir2-game-data crystal_monster_ai_summary_classifies_manifest_families -- --nocapture` passed.
- 2026-04-22, 100% closure B.2 FrostTiger AI: implemented AI 34 `FrostTiger` passive ranged baseline. Runtime now follows Crystal's empty `FindTarget()` behavior by not auto-targeting players, still allows target lock after being targeted, uses six-tile attack reach, switches non-adjacent attacks to `ObjectRangeAttack`, applies distance-scaled ranged hit timing, and gates delayed damage from imported DC data; `ObjectSitDown` packet support and bleed/slow poison rolls remain queued. Regenerated the AI summary: runtime special/guard families moved from 57 to 58, generic runtime families moved from 38 to 37, and AI 34 left the remaining priority queue. `cargo fmt --check`, `cargo test -p mir2-simulation frost_tiger -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation red_thunder_zuma -- --test-threads=1 --nocapture`, `node packages\tooling\scripts\generate-crystal-monster-ai-summary.mjs`, and `cargo test -p mir2-game-data crystal_monster_ai_summary_classifies_manifest_families -- --nocapture` passed.
- 2026-04-22, 100% closure B.2 IceGuard AI: implemented AI 102 `IceCrystalSolider` / `SinseokMiner` near/ranged baseline. Runtime now uses Crystal's eight-tile attack reach, adjacent `ObjectAttack` vs non-adjacent `ObjectRangeAttack` switching, fixed 500 ms ranged hit timing, imported MC ranged damage, and imported DC melee gating; random fire type and slow/frozen poison details remain queued. Regenerated the AI summary: runtime special/guard families moved from 58 to 59, generic runtime families moved from 37 to 36, and AI 102 left the remaining priority queue. `cargo fmt --check`, `cargo test -p mir2-simulation ice_guard -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation frost_tiger -- --test-threads=1 --nocapture`, `node packages\tooling\scripts\generate-crystal-monster-ai-summary.mjs`, and `cargo test -p mir2-game-data crystal_monster_ai_summary_classifies_manifest_families -- --nocapture` passed.
- 2026-04-22, 100% closure B.2 FrozenMiner AI: implemented AI 187 `FrozenMiner` primary branch baseline. Runtime now emits the Crystal primary `ObjectAttack` packet, uses 600 ms delayed hit timing, and resolves player damage from imported DC data; the random type-1 area branch remains queued. Regenerated the AI summary: runtime special/guard families moved from 59 to 60, generic runtime families moved from 36 to 35, and AI 187 left the remaining priority queue. `cargo fmt --check`, `cargo test -p mir2-simulation frozen_miner -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation ice_guard -- --test-threads=1 --nocapture`, `node packages\tooling\scripts\generate-crystal-monster-ai-summary.mjs`, and `cargo test -p mir2-game-data crystal_monster_ai_summary_classifies_manifest_families -- --nocapture` passed.
- 2026-04-22, 100% closure B.2 FrozenAxeman AI: implemented AI 188 `FrozenAxeman` two-tile branch baseline. Runtime now uses Crystal's two-tile line/diagonal attack shape, emits `ObjectAttack` with type 1 for non-adjacent targets, uses 500 ms delayed hit timing, and resolves player damage as imported DC*2; adjacent pull/push remains queued. Regenerated the AI summary: runtime special/guard families moved from 60 to 61, generic runtime families moved from 35 to 34, and AI 188 left the remaining priority queue. `cargo fmt --check`, `cargo test -p mir2-simulation frozen_axeman -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation frozen_miner -- --test-threads=1 --nocapture`, `node packages\tooling\scripts\generate-crystal-monster-ai-summary.mjs`, and `cargo test -p mir2-game-data crystal_monster_ai_summary_classifies_manifest_families -- --nocapture` passed.
- 2026-04-22, 100% closure B.2 FrozenMagician AI: implemented AI 189 `FrozenMagician` primary ranged branch baseline. Runtime now gives the family Crystal's nine-tile non-adjacent ranged reach, emits `ObjectRangeAttack`, uses distance-scaled delay with a 600 ms base, and resolves player damage from imported MC data; the random type-1 boosted ranged branch remains queued. Regenerated the AI summary: runtime special/guard families moved from 61 to 62, generic runtime families moved from 34 to 33, and AI 189 left the remaining priority queue. `cargo fmt --check`, `cargo test -p mir2-simulation frozen_magician -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation frozen_axeman -- --test-threads=1 --nocapture`, `node packages\tooling\scripts\generate-crystal-monster-ai-summary.mjs`, and `cargo test -p mir2-game-data crystal_monster_ai_summary_classifies_manifest_families -- --nocapture` passed.
- 2026-04-22, 100% closure B.2 SnowWolf AI: implemented AI 179 `SnowWolf` primary branch baseline. Runtime now emits the Crystal primary `ObjectAttack` packet, uses 350 ms delayed hit timing, and resolves player damage from imported DC data; the random type-1 slow/frozen branch remains queued and current SnowWolf data has zero MC. Regenerated the AI summary: runtime special/guard families moved from 62 to 63, generic runtime families moved from 33 to 32, and AI 179 left the remaining priority queue. `cargo fmt --check`, `cargo test -p mir2-simulation snow_wolf -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation frozen_magician -- --test-threads=1 --nocapture`, `node packages\tooling\scripts\generate-crystal-monster-ai-summary.mjs`, and `cargo test -p mir2-game-data crystal_monster_ai_summary_classifies_manifest_families -- --nocapture` passed.
- 2026-04-22, 100% closure B.2 TucsonMage AI: implemented AI 126 `TucsonMage` non-adjacent branch baseline. Runtime now uses Crystal's three-tile square attack reach, emits type-1 `ObjectAttack` for non-adjacent targets, and gates delayed damage when current imported zero-MC data yields no damage; full wide-line multi-target coverage remains queued. Regenerated the AI summary: runtime special/guard families moved from 63 to 64, generic runtime families moved from 32 to 31, and AI 126 left the remaining priority queue. `cargo fmt --check`, `cargo test -p mir2-simulation tucson_mage -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation snow_wolf -- --test-threads=1 --nocapture`, `node packages\tooling\scripts\generate-crystal-monster-ai-summary.mjs`, and `cargo test -p mir2-game-data crystal_monster_ai_summary_classifies_manifest_families -- --nocapture` passed.
- 2026-04-22, 100% closure B.2 SnowYeti AI: implemented AI 190 `SnowYeti` non-adjacent ranged baseline. Runtime now gives the family Crystal's nine-tile ranged reach, emits `ObjectRangeAttack`, uses distance-scaled delayed timing, and resolves player damage from imported DC data; adjacent double-hit and frozen poison details remain queued. Regenerated the AI summary: runtime special/guard families moved from 64 to 65, generic runtime families moved from 31 to 30, and AI 190 left the remaining priority queue. `cargo fmt --check`, `cargo test -p mir2-simulation snow_yeti -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation tucson_mage -- --test-threads=1 --nocapture`, `node packages\tooling\scripts\generate-crystal-monster-ai-summary.mjs`, and `cargo test -p mir2-game-data crystal_monster_ai_summary_classifies_manifest_families -- --nocapture` passed.
- 2026-04-22, 100% closure B.2 DarkWraith AI: implemented AI 192 `DarkWraith` four-tile line branch baseline. Runtime now uses Crystal's four-tile row/column/diagonal reach, emits `ObjectAttack` type 2 for non-adjacent targets, and resolves player damage from imported DC*3; exact same-method Crystal hit timing and adjacent area branch remain queued. Regenerated the AI summary: runtime special/guard families moved from 65 to 66, generic runtime families moved from 30 to 29, and AI 192 left the remaining priority queue. `cargo fmt --check`, `cargo test -p mir2-simulation dark_wraith -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation snow_yeti -- --test-threads=1 --nocapture`, `node packages\tooling\scripts\generate-crystal-monster-ai-summary.mjs`, and `cargo test -p mir2-game-data crystal_monster_ai_summary_classifies_manifest_families -- --nocapture` passed.
- 2026-04-22, 100% closure B.2 CrystalSpider AI: implemented AI 37 `CrystalSpider` line branch baseline. Runtime now uses Crystal's three-tile row/column/diagonal reach, emits non-adjacent `ObjectAttack` type 1, applies distance-scaled delayed DC damage, and can apply the Crystal 1/8 green-poison status on hit; full multi-target `LineAttack` edge cases remain queued. Regenerated the AI summary: runtime special/guard families moved from 66 to 67, generic runtime families moved from 29 to 28, and AI 37 left the remaining priority queue. `cargo fmt`, `cargo test -p mir2-simulation crystal_spider -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation dark_wraith -- --test-threads=1 --nocapture`, `node packages\tooling\scripts\generate-crystal-monster-ai-summary.mjs`, and `cargo test -p mir2-game-data crystal_monster_ai_summary_classifies_manifest_families -- --nocapture` passed.
- 2026-04-22, 100% closure B.2 TucsonWarrior AI: implemented AI 127 `TucsonWarrior` non-adjacent smash baseline. Runtime now uses Crystal's two-tile row/column/diagonal attack reach, emits `ObjectAttack` type 1 for non-adjacent targets, keeps the 300 ms delayed hit timing, and resolves player damage from imported MC data; adjacent random halfmoon/type-1 selection and full area multi-target coverage remain queued. Regenerated the AI summary: runtime special/guard families moved from 67 to 68, generic runtime families moved from 28 to 27, and AI 127 left the remaining priority queue. `cargo fmt --check`, `cargo test -p mir2-simulation tucson_warrior -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation crystal_spider -- --test-threads=1 --nocapture`, `node packages\tooling\scripts\generate-crystal-monster-ai-summary.mjs`, and `cargo test -p mir2-game-data crystal_monster_ai_summary_classifies_manifest_families -- --nocapture` passed.
- 2026-04-22, 100% closure B.2 ZumaTaurus AI: implemented AI 17 `ZumaTaurus` / `ZumaTaurus9` / `Ancient_ZumaTaurus` baseline. Runtime now includes AI 17 in the shared Zuma stoned wake/show state, wakes nearby Zuma-family monsters through the existing propagation path, emits adjacent `ObjectAttack`, uses Crystal's 300 ms delayed hit timing, and resolves player damage from imported DC data; HP-stage Zuma slave spawning remains queued. Regenerated the AI summary: runtime special/guard families moved from 68 to 69, generic runtime families moved from 27 to 26, and AI 17 left the remaining priority queue. `cargo fmt --check`, `cargo test -p mir2-simulation zuma_taurus -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation red_thunder_zuma -- --test-threads=1 --nocapture`, `node packages\tooling\scripts\generate-crystal-monster-ai-summary.mjs`, and `cargo test -p mir2-game-data crystal_monster_ai_summary_classifies_manifest_families -- --nocapture` passed.
- 2026-04-22, 100% closure B.2 BoneLord AI: implemented AI 30 `BoneLord` / `Ancient_BoneLord` ranged baseline. Runtime now gives the family Crystal's seven-tile attack reach, switches non-adjacent attacks to `ObjectRangeAttack`, applies distance-scaled delayed hit timing, and resolves player damage from imported DC data; HP-stage bone slave spawning remains queued. Regenerated the AI summary: runtime special/guard families moved from 69 to 70, generic runtime families moved from 26 to 25, and AI 30 left the remaining priority queue. `cargo fmt --check`, `cargo test -p mir2-simulation bone_lord -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation zuma_taurus -- --test-threads=1 --nocapture`, `node packages\tooling\scripts\generate-crystal-monster-ai-summary.mjs`, and `cargo test -p mir2-game-data crystal_monster_ai_summary_classifies_manifest_families -- --nocapture` passed.
- 2026-04-22, 100% closure B.2 ManectricClaw AI: implemented AI 86 `ManectricClaw` baseline for the spawned `Chieftain_Priest` row. Runtime now uses Crystal's three-tile attack reach, emits non-adjacent `ObjectRangeAttack`, applies a 500 ms delayed thrust hit, and resolves far targets from imported MC damage while keeping near targets on imported DC damage; random thrust movement and slow/frozen cone details remain queued. Regenerated the AI summary: runtime special/guard families moved from 70 to 71, generic runtime families moved from 25 to 24, and AI 86 left the remaining priority queue. `cargo fmt --check`, `cargo test -p mir2-simulation manectric_claw -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation bone_lord -- --test-threads=1 --nocapture`, `node packages\tooling\scripts\generate-crystal-monster-ai-summary.mjs`, and `cargo test -p mir2-game-data crystal_monster_ai_summary_classifies_manifest_families -- --nocapture` passed.
- 2026-04-22, 100% closure B.2 KingScorpion AI: implemented AI 19 `KingScorpion` two-tile line/range baseline. Runtime now uses Crystal's two-tile row/column/diagonal reach, emits `ObjectRangeAttack` for non-adjacent line targets, applies Crystal's delayed line hit timing, and resolves player damage from imported MC data; adjacent random `ObjectAttack` DC branch remains queued. Regenerated the AI summary: runtime special/guard families moved from 71 to 72, generic runtime families moved from 24 to 23, and AI 19 left the remaining priority queue. `cargo fmt --check`, `cargo test -p mir2-simulation king_scorpion -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation manectric_claw -- --test-threads=1 --nocapture`, `node packages\tooling\scripts\generate-crystal-monster-ai-summary.mjs`, and `cargo test -p mir2-game-data crystal_monster_ai_summary_classifies_manifest_families -- --nocapture` passed.
- 2026-04-22, 100% closure B.2 MirStatue AI: implemented AI 54 `DragonStatue` baseline for the spawned `MirStatue` row. Runtime now treats the family as static, blocks route/chase/patrol movement, attacks across imported view range, delays both damage and `ObjectRangeAttack` packet emission to the due tick, and resolves player damage from imported DC data; sleeping wake/revive state and radius multi-target damage remain queued. Regenerated the AI summary: runtime special/guard families moved from 72 to 73, generic runtime families moved from 23 to 22, and AI 54 left the remaining priority queue. `cargo fmt`, `cargo test -p mir2-simulation mir_statue -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation king_scorpion -- --test-threads=1 --nocapture`, `node packages\tooling\scripts\generate-crystal-monster-ai-summary.mjs`, and `cargo test -p mir2-game-data crystal_monster_ai_summary_classifies_manifest_families -- --nocapture` passed.
- 2026-04-22, 100% closure B.2 GuardianRock AI: implemented AI 48 `GuardianRock` static range-pull packet baseline. Runtime now blocks route/chase/patrol movement, emits `ObjectRangeAttack` while leaving player HP unchanged, and treats the rock as normal-damage immune; exact 500 ms delayed pull timing, resist checks, and pull-distance movement remain queued. Regenerated the AI summary: runtime special/guard families moved from 73 to 74, generic runtime families moved from 22 to 21, and AI 48 left the remaining priority queue. `cargo fmt`, `cargo test -p mir2-simulation guardian_rock -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation mir_statue -- --test-threads=1 --nocapture`, `node packages\tooling\scripts\generate-crystal-monster-ai-summary.mjs`, and `cargo test -p mir2-game-data crystal_monster_ai_summary_classifies_manifest_families -- --nocapture` passed.
- 2026-04-22, 100% closure B.2 FrozenWarewolf AI: implemented AI 180 `SnowWolfKing` primary branch baseline for the spawned `FrozenWarewolf` row. Runtime now emits primary `ObjectAttack` type 0, uses Crystal's 500 ms delayed hit timing, and resolves player damage from imported DC data; HP-threshold attack variants, weaker-target teleport, slave spawning, delayed death explosion, and pet transfer remain queued. Regenerated the AI summary: runtime special/guard families moved from 74 to 75, generic runtime families moved from 21 to 20, and AI 180 left the remaining priority queue. `cargo fmt --check`, `cargo test -p mir2-simulation frozen_warewolf -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation guardian_rock -- --test-threads=1 --nocapture`, `node packages\tooling\scripts\generate-crystal-monster-ai-summary.mjs`, and `cargo test -p mir2-game-data crystal_monster_ai_summary_classifies_manifest_families -- --nocapture` passed.
- 2026-04-22, 100% closure B.2 RedMoonEvil AI: implemented AI 13 `RedMoonEvil` / `RedMoonEvil1` static view-range baseline. Runtime now blocks route/chase/patrol movement, attacks across imported view range with `ObjectAttack`, keeps Crystal's 300 ms delayed hit timing, and resolves player damage from imported DC data; multi-target fanout and `ObjectEffect RedMoonEvil` packet parity remain queued. Regenerated the AI summary: runtime special/guard families moved from 75 to 76, generic runtime families moved from 20 to 19, and AI 13 left the remaining priority queue. `cargo fmt`, `cargo test -p mir2-simulation red_moon_evil -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation frozen_warewolf -- --test-threads=1 --nocapture`, `node packages\tooling\scripts\generate-crystal-monster-ai-summary.mjs`, and `cargo test -p mir2-game-data crystal_monster_ai_summary_classifies_manifest_families -- --nocapture` passed.
- 2026-04-22, 100% closure B.2 Yimoogi AI: implemented AI 36 `Yimoogi` ranged branch baseline. Runtime now gives Yimoogi Crystal's seven-tile attack reach, emits `ObjectRangeAttack` beyond the close two-tile shape, uses 500 ms delayed hit timing, and resolves player damage from imported DC data; poison branch, child/sister spawning, final teleport, and paired drop rules remain queued. Regenerated the AI summary: runtime special/guard families moved from 76 to 77, generic runtime families moved from 19 to 18, and AI 36 left the remaining priority queue. `cargo fmt --check`, `cargo test -p mir2-simulation yimoogi -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation red_moon_evil -- --test-threads=1 --nocapture`, `node packages\tooling\scripts\generate-crystal-monster-ai-summary.mjs`, and `cargo test -p mir2-game-data crystal_monster_ai_summary_classifies_manifest_families -- --nocapture` passed.
- 2026-04-22, 100% closure B.2 Lamia/Kirin AI: implemented AI 186 `Kirin` two-tile attack baseline for the spawned `Lamia` row. Runtime now uses Crystal's two-tile row/diagonal attack shape, emits `ObjectAttack` type 0, and resolves player damage from imported DC data; current Lamia MC=0 gates the Crystal IceThrust branch, and type-1/slow details remain queued. Regenerated the AI summary: runtime special/guard families moved from 77 to 78, generic runtime families moved from 18 to 17, and AI 186 left the remaining priority queue. `cargo fmt --check`, `cargo test -p mir2-simulation lamia -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation yimoogi -- --test-threads=1 --nocapture`, `node packages\tooling\scripts\generate-crystal-monster-ai-summary.mjs`, and `cargo test -p mir2-game-data crystal_monster_ai_summary_classifies_manifest_families -- --nocapture` passed.
- 2026-04-22, 100% closure B.2 Khazard AI: implemented AI 27 `Khazard` ranged pull-packet baseline. Runtime now uses Crystal's four-tile row/column/diagonal reach, emits `ObjectRangeAttack` for non-adjacent pull targets, and leaves player HP unchanged because the Crystal branch is displacement-only; exact pull movement, resist checks, and `PullTime` cooldown remain queued. Regenerated the AI summary: runtime special/guard families moved from 78 to 79, generic runtime families moved from 17 to 16, and AI 27 left the remaining priority queue. `cargo fmt --check`, `cargo test -p mir2-simulation khazard -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation lamia -- --test-threads=1 --nocapture`, `node packages\tooling\scripts\generate-crystal-monster-ai-summary.mjs`, and `cargo test -p mir2-game-data crystal_monster_ai_summary_classifies_manifest_families -- --nocapture` passed.
- 2026-04-22, 100% closure B.2 EvilCentipede AI: implemented AI 14 `EvilCentipede` hidden/static attack baseline. Runtime now starts the family hidden, reveals with `ObjectShow` when the player is within Crystal's three-tile trigger, hides and restores HP after leaving the seven-tile active radius, blocks route/chase/patrol movement, emits `ObjectAttack` while visible, uses Crystal's 500 ms delayed hit timing, and resolves player damage from imported DC data; multi-target fanout plus green/paralysis poison details remain queued. Regenerated the AI summary: runtime special/guard families moved from 79 to 80, generic runtime families moved from 16 to 15, and AI 14 left the remaining priority queue. `cargo fmt`, `cargo test -p mir2-simulation evil_centipede -- --test-threads=1 --nocapture`, `node packages\tooling\scripts\generate-crystal-monster-ai-summary.mjs`, and `cargo test -p mir2-game-data crystal_monster_ai_summary_classifies_manifest_families -- --nocapture` passed.
- 2026-04-22, 100% closure B.2 MinotaurKing AI: implemented AI 33 `MinotaurKing` RightGuard-derived ranged baseline. Runtime now gives the family Crystal's six-tile attack reach, emits `ObjectRangeAttack` for non-adjacent targets, uses the inherited 500 ms ranged hit timing, and resolves player damage from imported DC data; the three-tile `CompleteRangeAttack` fanout around the target remains queued. Regenerated the AI summary: runtime special/guard families moved from 80 to 81, generic runtime families moved from 15 to 14, and AI 33 left the remaining priority queue. `cargo fmt`, `cargo test -p mir2-simulation minotaur_king -- --test-threads=1 --nocapture`, `node packages\tooling\scripts\generate-crystal-monster-ai-summary.mjs`, and `cargo test -p mir2-game-data crystal_monster_ai_summary_classifies_manifest_families -- --nocapture` passed.
- 2026-04-22, 100% closure B.2 IncarnatedZT AI: implemented AI 22 `IncarnatedZT` active Zuma melee baseline. Runtime now keeps the family out of the stoned Zuma wake state, emits adjacent `ObjectAttack`, uses Crystal's 300 ms delayed hit timing, resolves player damage from imported DC data, and wires Crystal's 1/12 five-second paralysis poison chance into hit resolution. Regenerated the AI summary: runtime special/guard families moved from 81 to 82, generic runtime families moved from 14 to 13, and AI 22 left the remaining priority queue. `cargo fmt`, `cargo test -p mir2-simulation incarnated_zt -- --test-threads=1 --nocapture`, `node packages\tooling\scripts\generate-crystal-monster-ai-summary.mjs`, and `cargo test -p mir2-game-data crystal_monster_ai_summary_classifies_manifest_families -- --nocapture` passed.
- 2026-04-22, 100% closure B.2 DarkDevil AI: implemented AI 20 `DarkDevil` three-tile ranged burst baseline. Runtime now gives DarkDevil Crystal's opening three-tile attack reach, emits `ObjectRangeAttack` for non-adjacent targets, uses the 500 ms ranged hit timing, and resolves player damage as imported DC*3; random 2-4 second area cooldown plus the Crystal forward one-tile fanout remain queued. Regenerated the AI summary: runtime special/guard families moved from 82 to 83, generic runtime families moved from 13 to 12, and AI 20 left the remaining priority queue. `cargo fmt`, `cargo test -p mir2-simulation dark_devil -- --test-threads=1 --nocapture`, `node packages\tooling\scripts\generate-crystal-monster-ai-summary.mjs`, and `cargo test -p mir2-game-data crystal_monster_ai_summary_classifies_manifest_families -- --nocapture` passed.
- 2026-04-22, 100% closure B.2 OmaKing AI: implemented AI 43 `OmaKing` ranged magic branch baseline. Runtime now gives OmaKing Crystal's seven-tile attack reach, emits type-1 `ObjectAttack` instead of `ObjectRangeAttack` for distant targets, uses the 500 ms ranged hit timing, and resolves player damage from imported MC data; close push/paralysis and line-attack fanout remain queued. Regenerated the AI summary: runtime special/guard families moved from 83 to 84, generic runtime families moved from 12 to 11, and AI 43 left the remaining priority queue. `cargo fmt --check`, `cargo test -p mir2-simulation oma_king -- --test-threads=1 --nocapture`, `node packages\tooling\scripts\generate-crystal-monster-ai-summary.mjs`, and `cargo test -p mir2-game-data crystal_monster_ai_summary_classifies_manifest_families -- --nocapture` passed.
- 2026-04-22, 100% closure B.2 GreatFoxSpirit AI: implemented AI 50 `GreatFoxSpirit` static ranged baseline. Runtime now treats the family as static, blocks route/chase/patrol movement, gives it Crystal's seven-tile attack reach, emits `ObjectRangeAttack` for distant targets, uses 300 ms delayed hit timing, and resolves player damage from imported DC data; HP-stage extra, recall teleport, GuardianRock activation, target `ObjectEffect`, and slow/paralysis poison remain queued. Regenerated the AI summary: runtime special/guard families moved from 84 to 85, generic runtime families moved from 11 to 10, and AI 50 left the remaining priority queue. `cargo fmt`, `cargo test -p mir2-simulation great_fox_spirit -- --test-threads=1 --nocapture`, `node packages\tooling\scripts\generate-crystal-monster-ai-summary.mjs`, and `cargo test -p mir2-game-data crystal_monster_ai_summary_classifies_manifest_families -- --nocapture` passed.
- 2026-04-22, 100% closure B.2 ManectricKing AI: implemented AI 88 `ManectricKing` baseline for spawned `Master_DragonYang`. Runtime now uses Crystal's three-tile row/column/diagonal attack shape, emits type-0 `ObjectAttack`, uses 500 ms delayed hit timing, and resolves player damage from imported MC data; low-HP mass attack plus close type-1 push line remain queued. Regenerated the AI summary: runtime special/guard families moved from 85 to 86, generic runtime families moved from 10 to 9, and AI 88 left the remaining priority queue. `cargo fmt --check`, `cargo test -p mir2-simulation manectric_king -- --test-threads=1 --nocapture`, `node packages\tooling\scripts\generate-crystal-monster-ai-summary.mjs`, and `cargo test -p mir2-game-data crystal_monster_ai_summary_classifies_manifest_families -- --nocapture` passed.
- 2026-04-22, 100% closure B.2 SeedingsGeneral AI: implemented AI 121 `SeedingsGeneral` two-tile ranged magic baseline. Runtime now gives the family Crystal's two-tile attack reach, emits `ObjectRangeAttack` for non-adjacent targets, uses 300 ms delayed hit timing, and resolves player damage from imported MC data; random stomp type, close mixed attacks, slow poison, and frozen poison remain queued. Regenerated the AI summary: runtime special/guard families moved from 86 to 87, generic runtime families moved from 9 to 8, and AI 121 left the remaining priority queue. `cargo fmt`, `cargo test -p mir2-simulation seedings_general -- --test-threads=1 --nocapture`, `node packages\tooling\scripts\generate-crystal-monster-ai-summary.mjs`, and `cargo test -p mir2-game-data crystal_monster_ai_summary_classifies_manifest_families -- --nocapture` passed.
- 2026-04-22, 100% closure B.2 RestlessJar AI: implemented AI 122 `RestlessJar` static ranged packet baseline. Runtime now blocks route/chase/patrol movement, gives the family Crystal's six-tile attack reach, emits `ObjectRangeAttack` for non-adjacent targets, and gates delayed damage when current imported zero-MC data yields no damage; melee spin, tornado/blindness, stomp push, and exact projectile timing details remain queued. Regenerated the AI summary: runtime special/guard families moved from 87 to 88, generic runtime families moved from 8 to 7, and AI 122 left the remaining priority queue. `cargo fmt`, `cargo test -p mir2-simulation restless_jar -- --test-threads=1 --nocapture`, `node packages\tooling\scripts\generate-crystal-monster-ai-summary.mjs`, and `cargo test -p mir2-game-data crystal_monster_ai_summary_classifies_manifest_families -- --nocapture` passed.
- 2026-04-22, 100% closure B.2 HellKeeper AI: implemented AI 79 `HellKeeper` static view-range attack baseline. Runtime now blocks route/chase/patrol movement, uses Crystal's view-range reach, keeps the monster's initial attack facing instead of turning, emits type-0 `ObjectAttack` instead of `ObjectRangeAttack`, and applies 300 ms delayed imported DC damage; random type-1 MC/dazed branch, no-regen detail, and full `FindAllTargets` fanout remain queued. Regenerated the AI summary: runtime special/guard families moved from 88 to 89, generic runtime families moved from 7 to 6, and AI 79 left the remaining priority queue. `cargo fmt --check`, `cargo test -p mir2-simulation hell_keeper -- --test-threads=1 --nocapture`, `node packages\tooling\scripts\generate-crystal-monster-ai-summary.mjs`, and `cargo test -p mir2-game-data crystal_monster_ai_summary_classifies_manifest_families -- --nocapture` passed.
- 2026-04-22, 100% closure B.2 GeneralMeowMeow AI: implemented AI 123 `GeneralMeowMeow` twelve-tile ranged magic baseline. Runtime now gives the family Crystal's 12-tile attack reach, switches to `ObjectRangeAttack` beyond the close two-tile shape, uses 500 ms delayed hit timing, resolves ranged damage from imported MC data, and keeps close two-tile attacks on `ObjectAttack`/DC semantics; shield phases, mass thunder spell objects, slave spawning, random slam, and range fanout remain queued. Regenerated the AI summary: runtime special/guard families moved from 89 to 90, generic runtime families moved from 6 to 5, and AI 123 left the remaining priority queue. `cargo fmt`, `cargo test -p mir2-simulation general_meow_meow -- --test-threads=1 --nocapture`, `node packages\tooling\scripts\generate-crystal-monster-ai-summary.mjs`, and `cargo test -p mir2-game-data crystal_monster_ai_summary_classifies_manifest_families -- --nocapture` passed.
- 2026-04-22, 100% closure B.2 TucsonGeneral AI: implemented AI 131 `TucsonGeneral` rage/ranged baseline. Runtime now gives TucsonGeneral Crystal's view-range attack reach, emits the opening type-0 `ObjectRangeAttack` rage packet without direct damage, applies the 20-second rage cooldown and 8-second attack pause, and uses the normal type-1 `ObjectRangeAttack` branch with distance-scaled delay plus imported SC damage while rage is cooling down; rock spell objects, random type-2 ranged hit, melee stomp/paralysis, and rage target scattering were completed in later closure passes. Regenerated the AI summary: runtime special/guard families moved from 90 to 91, generic runtime families moved from 5 to 4, and AI 131 left the remaining priority queue. `cargo fmt`, `cargo test -p mir2-simulation tucson_general -- --test-threads=1 --nocapture`, `node packages\tooling\scripts\generate-crystal-monster-ai-summary.mjs`, and `cargo test -p mir2-game-data crystal_monster_ai_summary_classifies_manifest_families -- --nocapture` passed.
- 2026-04-22, 100% closure B.2 TrapRock AI: implemented AI 47 `TrapRock` / `TrapRock1` hidden reveal baseline. Runtime now starts the family hidden and non-visible, blocks route/chase/patrol movement, waits through Crystal's visibility delay, reveals near a player target, moves to an adjacent target tile, emits `ObjectShow`, and uses the parent `ObjectRangeAttack` packet with no direct damage; child rocks, four-corner spawn layout, target-move death, first-hit collapse, and paralysis details remain queued. Regenerated the AI summary: runtime special/guard families moved from 91 to 92, generic runtime families moved from 4 to 3, and AI 47 left the remaining priority queue. `cargo fmt`, `cargo test -p mir2-simulation trap_rock -- --test-threads=1 --nocapture`, `node packages\tooling\scripts\generate-crystal-monster-ai-summary.mjs`, and `cargo test -p mir2-game-data crystal_monster_ai_summary_classifies_manifest_families -- --nocapture` passed.
- 2026-04-22, 100% closure B.2 Armadillo AI: implemented AI 124 `Armadillo` and AI 125 `ArmadilloElder` DigOut-style baseline. Runtime now starts both families hidden/non-visible, reveals with `ObjectShow` when the player is close, delays attacks after reveal, uses Armadillo's primary `ObjectAttack`/DC branch, and uses ArmadilloElder's primary `ObjectAttack`/DC*2 branch with 400 ms hit timing; `DigOutArmadillo` spell-object presentation, retreat/backstep/branch details, and Armadillo run-away after failed retreat damage were completed in later closure passes. Regenerated the AI summary: runtime special/guard families moved from 92 to 94, generic runtime families moved from 3 to 1, and AI 124/125 left the remaining priority queue. `cargo fmt --check`, `cargo test -p mir2-simulation armadillo -- --test-threads=1 --nocapture`, `node packages\tooling\scripts\generate-crystal-monster-ai-summary.mjs`, and `cargo test -p mir2-game-data crystal_monster_ai_summary_classifies_manifest_families -- --nocapture` passed.
- 2026-04-22, 100% closure B.2 default MonsterObject AI: promoted AI 0 `MonsterObject` from implicit generic to explicit default-branch coverage. Runtime now has focused coverage for imported AI0 templates using Crystal stats, adjacent `ObjectAttack` melee, delayed `Struck` damage, hostile target tracking, movement/chase baseline, respawn/drop plumbing, and packet visibility; subclass-specific behaviors continue to be tracked by their own AI rows. Regenerated the AI summary: runtime special/guard families moved from 94 to 95, generic runtime families moved from 1 to 0, and the remaining spawned runtime priority queue is empty. `cargo fmt --check`, `cargo test -p mir2-simulation ai0_default -- --test-threads=1 --nocapture`, `node packages\tooling\scripts\generate-crystal-monster-ai-summary.mjs`, and `cargo test -p mir2-game-data crystal_monster_ai_summary_classifies_manifest_families -- --nocapture` passed.
- 2026-04-22, 100% closure B.2 TrapRock follow-up: deepened AI 47 `TrapRock` parent-rock parity beyond the initial reveal baseline. Runtime now stores the trapped target location after reveal, kills the visible parent rock when the target moves away from that Crystal `TargetLocation`, and implements the parent `FirstAttack` collapse path so the first player hit immediately kills the rock instead of applying normal HP damage. Child rocks, four-corner spawn layout, and paralysis poison details remain queued. `cargo fmt`, `cargo test -p mir2-simulation trap_rock -- --test-threads=1 --nocapture`, `node packages\tooling\scripts\generate-crystal-monster-ai-summary.mjs`, and `cargo test -p mir2-game-data crystal_monster_ai_summary_classifies_manifest_families -- --nocapture` passed.
- 2026-04-22, 100% closure B.2 TrapRock child-rock pass: added the AI 47 child-rock surround and reveal paralysis baseline. Parent TrapRock reveal now spawns the other three cardinal child rocks around the trapped target, marks them as parent-owned summons, emits `ObjectShow` for each child, applies the Crystal three-second paralysis baseline on reveal, lets child rocks emit `ObjectAttack`, and clears the parent `FirstAttack` state when a child rock is attacked. Exact random spawn-corner ordering and repeated attack poison rolls remain queued. `cargo fmt --check`, `cargo test -p mir2-simulation trap_rock -- --test-threads=1 --nocapture`, `node packages\tooling\scripts\generate-crystal-monster-ai-summary.mjs`, and `cargo test -p mir2-game-data crystal_monster_ai_summary_classifies_manifest_families -- --nocapture` passed.
- 2026-04-22, 100% closure B.2 Armadillo branch pass: deepened AI 124/125 beyond the primary DigOut reveal baseline. AI 124 `Armadillo` now has the Crystal type-1 three-hit combo packet with three imported half-DC delayed hits, while AI 125 `ArmadilloElder` now emits the type-1 push packet branch without direct damage; retreat/backstep movement, exact push displacement, run-away state, and `DigOutArmadillo` spell-object presentation were completed in later closure passes. `cargo fmt --check`, `cargo test -p mir2-simulation armadillo -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation trap_rock -- --test-threads=1 --nocapture`, `node packages\tooling\scripts\generate-crystal-monster-ai-summary.mjs`, and `cargo test -p mir2-game-data crystal_monster_ai_summary_classifies_manifest_families -- --nocapture` passed.
- 2026-04-22, 100% closure B.2 HellKeeper type-1 pass: deepened AI 79 `HellKeeper` attack branch coverage. Runtime now emits the Crystal type-1 locked-facing `ObjectAttack` branch and resolves it through imported raw MC data; current Crystal HellKeeper data has `MC=0`, so the branch correctly emits the packet without direct damage or Dazed poison, while nonzero-MC Dazed effect coverage and full `FindAllTargets` fanout remain queued. `cargo fmt --check`, `cargo test -p mir2-simulation hell_keeper -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation armadillo -- --test-threads=1 --nocapture`, `node packages\tooling\scripts\generate-crystal-monster-ai-summary.mjs`, and `cargo test -p mir2-game-data crystal_monster_ai_summary_classifies_manifest_families -- --nocapture` passed.
- 2026-04-22, 100% closure B.2 SnowWolf type-1 pass: deepened AI 179 `SnowWolf` beyond the primary DC attack. Runtime now emits the Crystal type-1 `ObjectAttack` branch and gates delayed damage through imported raw MC data; current SnowWolf data has `MC=0`, so the branch correctly emits the packet without direct damage, while nonzero-MC slow/frozen poison fanout remains queued. `cargo fmt --check`, `cargo test -p mir2-simulation snow_wolf -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation hell_keeper -- --test-threads=1 --nocapture`, `node packages\tooling\scripts\generate-crystal-monster-ai-summary.mjs`, and `cargo test -p mir2-game-data crystal_monster_ai_summary_classifies_manifest_families -- --nocapture` passed.
- 2026-04-22, 100% closure B.2 FrozenMiner type-1 pass: deepened AI 187 `FrozenMiner` beyond the primary DC attack. Runtime now emits the Crystal type-1 `ObjectAttack` branch and applies the imported 80% DC delayed hit at the 1000 ms baseline for the player target; full `FindAllTargets(1)` area fanout remains queued. `cargo fmt --check`, `cargo test -p mir2-simulation frozen_miner -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation snow_wolf -- --test-threads=1 --nocapture`, `node packages\tooling\scripts\generate-crystal-monster-ai-summary.mjs`, and `cargo test -p mir2-game-data crystal_monster_ai_summary_classifies_manifest_families -- --nocapture` passed.
- 2026-04-22, 100% closure B.2 FrozenMagician type-1 pass: deepened AI 189 `FrozenMagician` ranged branch coverage. Runtime now emits `ObjectRangeAttack` type 1 for the boosted Crystal ranged branch, applies the distance-scaled 750 ms base delay, and resolves damage as imported MC*3/2; the existing type-0 branch remains on the 600 ms base delay with imported MC damage. `cargo fmt --check`, `cargo test -p mir2-simulation frozen_magician -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation frozen_miner -- --test-threads=1 --nocapture`, `node packages\tooling\scripts\generate-crystal-monster-ai-summary.mjs`, and `cargo test -p mir2-game-data crystal_monster_ai_summary_classifies_manifest_families -- --nocapture` passed.
- 2026-04-22, 100% closure B.2 SnowYeti adjacent pass: deepened AI 190 `SnowYeti` beyond the non-adjacent ranged baseline. Runtime now emits the Crystal adjacent melee sequence as same-tick `ObjectAttack` type 0 and type 1 packets, then applies two imported DC hits at the 500 ms and 1500 ms baselines; ranged frozen-poison rolls remain queued. `cargo fmt --check`, `cargo test -p mir2-simulation snow_yeti -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation dark_wraith -- --test-threads=1 --nocapture`, `node packages\tooling\scripts\generate-crystal-monster-ai-summary.mjs`, and `cargo test -p mir2-game-data crystal_monster_ai_summary_classifies_manifest_families -- --nocapture` passed.
- 2026-04-22, 100% closure B.2 IceGuard branch pass: deepened AI 102 `IceCrystalSolider` / `SinseokMiner` beyond the initial near/ranged baseline. Runtime now selects the Crystal fire ranged branch as `ObjectRangeAttack` type 1 without poison, keeps the ice ranged branch as type 0, and applies the Crystal slow/frozen poison rolls after successful ice-branch damage. `cargo fmt --check`, `cargo test -p mir2-simulation ice_guard -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation frost_tiger -- --test-threads=1 --nocapture`, `node packages\tooling\scripts\generate-crystal-monster-ai-summary.mjs`, and `cargo test -p mir2-game-data crystal_monster_ai_summary_classifies_manifest_families -- --nocapture` passed.
- 2026-04-22, 100% closure B.2 SnowYeti frozen pass: completed the AI 190 `SnowYeti` ranged poison detail. Runtime now applies the Crystal frozen-poison roll after successful ranged damage, sharing the runtime frozen buff representation already used by IceGuard. `cargo fmt --check`, `cargo test -p mir2-simulation snow_yeti -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation ice_guard -- --test-threads=1 --nocapture`, `node packages\tooling\scripts\generate-crystal-monster-ai-summary.mjs`, and `cargo test -p mir2-game-data crystal_monster_ai_summary_classifies_manifest_families -- --nocapture` passed.
- 2026-04-22, 100% closure B.2 FrozenAxeman adjacent pass: deepened AI 188 `FrozenAxeman` beyond the two-tile type-1 branch. Runtime now covers the adjacent Crystal type-2 pull/push branch with a deterministic 2/3 trigger, 10s cooldown, immediate 2-4 tile player push, and 500 ms delayed imported DC hit; cooldown fallback remains on the existing type-0 melee branch. `cargo fmt --check`, `cargo test -p mir2-simulation frozen_axeman -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation frozen_miner -- --test-threads=1 --nocapture`, `node packages\tooling\scripts\generate-crystal-monster-ai-summary.mjs`, and `cargo test -p mir2-game-data crystal_monster_ai_summary_classifies_manifest_families -- --nocapture` passed.
- 2026-04-22, 100% closure B.2 FrozenMiner fanout pass: completed the AI 187 `FrozenMiner` type-1 area branch beyond the player-target hit. Runtime now applies the 1000 ms imported 80% DC hit to adjacent opposing monster targets as a `FindAllTargets(1)`-style fanout while keeping the existing player hit. `cargo fmt --check`, `cargo test -p mir2-simulation frozen_miner -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation frozen_axeman -- --test-threads=1 --nocapture`, `node packages\tooling\scripts\generate-crystal-monster-ai-summary.mjs`, and `cargo test -p mir2-game-data crystal_monster_ai_summary_classifies_manifest_families -- --nocapture` passed.
- 2026-04-22, 100% closure B.2 DarkWraith adjacent pass: deepened AI 192 `DarkWraith` beyond the four-tile type-2 line branch. Runtime now covers the adjacent Crystal type-1 area branch with 600 ms delayed imported DC damage against the player and nearby opposing monster targets; exact line-attack cooldown/timing remains queued. `cargo fmt --check`, `cargo test -p mir2-simulation dark_wraith -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation crystal_spider -- --test-threads=1 --nocapture`, `node packages\tooling\scripts\generate-crystal-monster-ai-summary.mjs`, and `cargo test -p mir2-game-data crystal_monster_ai_summary_classifies_manifest_families -- --nocapture` passed.
- 2026-04-22, 100% closure B.2 KingScorpion adjacent pass: pinned AI 19 `KingScorpion` adjacent branch parity with a focused test. Runtime already used adjacent `ObjectAttack` type 0 with imported DC damage while keeping the non-adjacent two-tile `ObjectRangeAttack` MC branch; docs now track the remaining work as adjacent random range override and line multi-target edge cases instead of the covered DC melee branch. `cargo fmt --check`, `cargo test -p mir2-simulation king_scorpion -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation dark_devil -- --test-threads=1 --nocapture`, `node packages\tooling\scripts\generate-crystal-monster-ai-summary.mjs`, and `cargo test -p mir2-game-data crystal_monster_ai_summary_classifies_manifest_families -- --nocapture` passed.
- 2026-04-22, 100% closure B.2 Khazard pull pass: deepened AI 27 `Khazard` beyond the pull packet baseline. Runtime now performs the Crystal pull movement toward Khazard, keeps the pull branch damage-free, and stores the 5s `PullTime` cooldown so non-adjacent pulls cannot repeat immediately; exact magic-resist checks remain queued. `cargo fmt --check`, `cargo test -p mir2-simulation khazard -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation lamia -- --test-threads=1 --nocapture`, `node packages\tooling\scripts\generate-crystal-monster-ai-summary.mjs`, and `cargo test -p mir2-game-data crystal_monster_ai_summary_classifies_manifest_families -- --nocapture` passed.
- 2026-04-22, 100% closure B.2 Lamia/Kirin type-1 pass: deepened AI 186 `Kirin` / spawned `Lamia` beyond the type-0 DC baseline. Runtime now emits the Crystal type-1 `ObjectAttack` branch and applies the 500 ms imported DC hit; current Lamia MC=0 still gates the IceThrust/slow branch. `cargo fmt --check`, `cargo test -p mir2-simulation lamia -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation khazard -- --test-threads=1 --nocapture`, `node packages\tooling\scripts\generate-crystal-monster-ai-summary.mjs`, and `cargo test -p mir2-game-data crystal_monster_ai_summary_classifies_manifest_families -- --nocapture` passed.
- 2026-04-22, 100% closure B.2 SeedingsGeneral branch pass: deepened AI 121 `SeedingsGeneral` beyond the primary ranged MC baseline. Runtime now covers type-0 Echo Shout slow poison and type-1 Stomp frozen poison, with Stomp also fanning out imported MC damage to nearby opposing monster targets; close mixed melee branches remain queued. `cargo fmt --check`, `cargo test -p mir2-simulation seedings_general -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation hell_keeper -- --test-threads=1 --nocapture`, `node packages\tooling\scripts\generate-crystal-monster-ai-summary.mjs`, and `cargo test -p mir2-game-data crystal_monster_ai_summary_classifies_manifest_families -- --nocapture` passed.
- 2026-04-22, 100% closure B.2 ManectricClaw poison pass: deepened AI 86 `ManectricClaw` / spawned `Chieftain_Priest` thrust coverage. Runtime now applies the Crystal player slow/frozen poison rolls after the range-thrust hit while preserving near-DC/far-MC damage; thrust movement and full cone fanout remain queued. `cargo fmt --check`, `cargo test -p mir2-simulation manectric_claw -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation bone_lord -- --test-threads=1 --nocapture`, `node packages\tooling\scripts\generate-crystal-monster-ai-summary.mjs`, and `cargo test -p mir2-game-data crystal_monster_ai_summary_classifies_manifest_families -- --nocapture` passed.
- 2026-04-22, 100% closure B.2 FrostTiger poison pass: deepened AI 34 `FrostTiger` ranged branch coverage. Runtime now applies the Crystal ranged poison roll using imported `Effect`: current `FrostTiger` effect 0 maps to Bleeding, while effect 1 maps to Slow; `ObjectSitDown` presentation remains queued. `cargo fmt --check`, `cargo test -p mir2-simulation frost_tiger -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation ice_guard -- --test-threads=1 --nocapture`, `node packages\tooling\scripts\generate-crystal-monster-ai-summary.mjs`, and `cargo test -p mir2-game-data crystal_monster_ai_summary_classifies_manifest_families -- --nocapture` passed.
- 2026-04-22, 100% closure B.2 TurtleGrass single-push pass: completed the AI 173 `TurtleGrass` type-1 branch beyond the Zuma-family baseline. Runtime now follows Crystal's 1/4 type-1 `ObjectAttack` path, immediately pushes the player three tiles along the attack direction, and applies the imported DC hit on the 500 ms single-push delay. `cargo fmt --check` and `cargo test -p mir2-simulation turtle_grass -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure B.2 CannibalTentacles halfmoon pass: completed the AI 130 `CannibalTentacles` adjacent type-1 branch beyond the non-adjacent range baseline. Runtime now follows Crystal's 1/5 type-1 `ObjectAttack` path, applies the fixed 500-damage halfmoon hit on the 500 ms delay, fans that halfmoon arc out to adjacent opposing monsters, and applies Green Poison to the player on successful hit. `cargo fmt --check` and `cargo test -p mir2-simulation cannibal_tentacles -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure B.2 Jar2 adjacent branch pass: completed the AI 120 `Jar2` adjacent random split beyond the static range baseline. Runtime now follows Crystal's adjacent 1/3 DC `ObjectAttack` path and adjacent 2/3 `ObjectRangeAttack` path, keeps current generated zero-MC range hits damage-free, and wires the Frozen poison hook for successful Jar2 ranged damage when data allows it. `cargo fmt --check` and `cargo test -p mir2-simulation jar2 -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure B.2 SandSnail branch pass: completed AI 115 `SandSnail` beyond the primary DC melee baseline. Runtime now covers the Crystal type-1 DC halfmoon arc fanout and type-2 MC one-tile area branch, including Green Poison on successful player hit, while preserving the 300 ms delayed attack timing. `cargo fmt --check` and `cargo test -p mir2-simulation sand_snail -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure B.2 ManTree branch pass: deepened AI 174 `ManTree` / spawned `FineSoul` beyond the baseline type-0 packet. Runtime now covers the Crystal type-1 halfmoon and type-2 boulder packet branches and wires the Stun hook for successful boulder hits; current generated `FineSoul` data has `DC=0` and `MC=0`, so damage, halfmoon fanout, and Stun remain correctly data-gated. `cargo fmt --check` and `cargo test -p mir2-simulation man_tree -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure B.2 MinotaurKing fanout pass: completed the AI 33 `MinotaurKing` `CompleteRangeAttack` fanout beyond the six-tile ranged baseline. Runtime now applies the imported DC ranged hit to the player and to opposing monsters within Crystal's three-tile radius around the target location. `cargo fmt --check` and `cargo test -p mir2-simulation minotaur_king -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure B.2 DarkDevil fanout pass: completed the AI 20 `DarkDevil` area burst details beyond the opening ranged baseline. Runtime now stores the Crystal 2-4 second area cooldown after a burst, suppresses repeated non-adjacent range bursts while cooling down, and applies the delayed imported DC*3 hit to opposing monsters in the one-tile fanout around the point two tiles in front of the monster. `cargo fmt --check` and `cargo test -p mir2-simulation dark_devil -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure B.2 OmaKing close pass: completed AI 43 `OmaKing` close-branch details beyond the seven-tile type-1 magic baseline. Runtime now preserves the close random split, applies the type-0 push before line resolution, can apply the Crystal paralysis hook after a successful push, and resolves the imported DC line hit across the two forward tiles; close/ranged type-1 remains on imported MC with the 500 ms delay. `cargo fmt --check` and `cargo test -p mir2-simulation oma_king -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure B.2 RedMoonEvil fanout pass: completed AI 13 `RedMoonEvil` multi-target damage beyond the static view-range baseline. Runtime now applies the delayed imported DC hit to opposing monsters inside the Crystal view-range fanout while preserving the single `ObjectAttack` packet shape; `ObjectEffect RedMoonEvil` remains queued on protocol support. `cargo fmt --check` and `cargo test -p mir2-simulation red_moon_evil -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure B.2 EvilCentipede poison pass: completed AI 14 `EvilCentipede` fanout and poison details beyond the hidden/static baseline. Runtime now applies the delayed imported DC hit to all opposing targets in the seven-tile Crystal fanout and applies the Crystal Green Poison plus Paralysis rolls on successful player hit. `cargo fmt --check` and `cargo test -p mir2-simulation evil_centipede -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure B.2 GreatFoxSpirit fanout/poison pass: deepened AI 50 `GreatFoxSpirit` beyond the static ranged baseline. Runtime now applies the delayed imported DC hit to Crystal `FindAllTargets` fanout targets, using the seven-tile ranged radius or two-tile close radius, and applies the Crystal Slow plus Paralysis rolls on successful player hit. `cargo fmt --check` and `cargo test -p mir2-simulation great_fox_spirit -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure B.2 Yimoogi poison pass: deepened AI 36 `Yimoogi` beyond the seven-tile ranged baseline. Runtime now preserves Crystal's 500 ms ranged attack branch and adds the four-tile type-1 `ObjectAttack` red-poison path without direct damage; child/sister spawning, final teleport, and paired drop rules remain queued. `cargo fmt --check` and `cargo test -p mir2-simulation yimoogi -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure B.2 DarkWraith line pass: completed AI 192 `DarkWraith` line-branch details beyond the initial type-2 player hit. Runtime now stores the Crystal 3-7s line cooldown approximation after a non-adjacent line attack and applies the imported DC*3 hit to opposing monsters along the four forward line tiles while preserving the adjacent type-1 area branch. `cargo test -p mir2-simulation dark_wraith -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure B.2 TucsonWarrior branch pass: completed AI 127 `TucsonWarrior` adjacent selection and area coverage beyond the non-adjacent type-1 smash baseline. Runtime now covers the Crystal adjacent 4/5 type-0 halfmoon DC branch, adjacent 1/5 type-1 MC smash branch, and one-tile target-area multi-target fanout for smash hits. `cargo test -p mir2-simulation tucson_warrior -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure B.2 spider line fanout pass: completed forward-line multi-target coverage for AI 35 `SandWorm` and AI 37 `CrystalSpider`. Runtime now applies the delayed line hit to opposing monsters on the same two-tile SpittingSpider/SandWorm line or three-tile CrystalSpider line while preserving existing player damage and CrystalSpider green-poison behavior. `cargo fmt --check`, `cargo test -p mir2-simulation sand_worm -- --test-threads=1 --nocapture`, and `cargo test -p mir2-simulation crystal_spider -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure B.2 KingScorpion line pass: completed AI 19 `KingScorpion` line fanout and adjacent range override details beyond the prior range/DC baselines. Runtime now applies the two-tile line hit to opposing monsters, uses the adjacent random `ObjectRangeAttack` MC override, and forces the range branch when an attack target is on the second forward tile. `cargo test -p mir2-simulation king_scorpion -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure B.2 RestlessJar adjacent pass: completed AI 122 `RestlessJar` adjacent branch coverage beyond the static ranged packet baseline. Runtime now covers the adjacent spin branch with one-tile fanout, the high-HP tornado `ObjectRangeAttack` type-1 branch with Blindness poison, and the low-HP stomp type-2 branch with one-tile push plus fanout while preserving current zero-MC ranged no-damage gating. `cargo test -p mir2-simulation restless_jar -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure B.2 GeneralMeowMeow fanout pass: completed AI 123 `GeneralMeowMeow` range fanout beyond the twelve-tile ranged baseline. Runtime now applies the delayed imported MC hit to opposing monsters in the Crystal two-tile target-area fanout around the player target while keeping close attacks on the existing DC path. `cargo fmt --check` and `cargo test -p mir2-simulation general_meow_meow -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure B.2 DarkBeast secondary pass: completed AI 112 `DarkBeast` secondary type-1 branch coverage beyond the primary DC melee baseline. Runtime now emits the Crystal type-1 packet, resolves it through imported raw MC, and wires the Effect=1 bleeding hook; current spawned `CatWidow` data has `MC=0` and `Effect=0`, so the branch correctly emits without damage or bleeding. `cargo fmt --check` and `cargo test -p mir2-simulation dark_beast -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure B.2 GeneralMeowMeow close slam pass: completed AI 123 `GeneralMeowMeow` close random slam branch. Runtime now maps the original 1-in-9 close `ObjectAttack` type-1 branch to deterministic `tick % 9 == 0` coverage and resolves damage as imported Crystal DC * 3 after defence, while keeping the normal close attack and ranged MC fanout paths intact. `cargo fmt --check` and `cargo test -p mir2-simulation general_meow_meow -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure B.2 TucsonGeneral type-2 ranged pass: completed AI 131 `TucsonGeneral` random ranged heavy-hit branch. Runtime now maps the original 1-in-4 ranged `ObjectRangeAttack` type-2 branch to deterministic `tick % 4 == 0` coverage, applies the Crystal 500 ms delayed hit, and resolves imported SC * 2 after defence while leaving the existing rage packet and normal type-1 SC branch intact. `cargo fmt --check` and `cargo test -p mir2-simulation tucson_general -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure B.2 TucsonGeneral close stomp pass: completed AI 131 `TucsonGeneral` close type-1 MC stomp branch. Runtime now maps the original close stomp/paralysis path to deterministic `tick % 4 == 0` coverage, emits `ObjectAttack` type 1, applies the 500 ms delayed imported MC hit to the player and opposing monsters within three tiles of TucsonGeneral, and wires the Crystal 3-denominator / 5-tick paralysis poison. `cargo fmt` and `cargo test -p mir2-simulation tucson_general -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure B.2 CatShaman red poison pass: completed AI 118 `CatShaman` red-poison ranged variant. Runtime now maps the original 1-in-5 ranged branch to deterministic `tick % 5 == 0` coverage, emits `ObjectRangeAttack` type 1, resolves ranged damage from imported raw MC data, and wires a 5-denominator / 5-tick red poison hook for future nonzero-MC data; current spawned CatShaman data has `MC=0`, so damage and poison are correctly gated. `cargo fmt --check` and `cargo test -p mir2-simulation cat_ -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure B.2 WaterDragon/BlackTortoise poison pass: completed AI 181/182 ranged poison coverage. Runtime now maps WaterDragon ranged damage to imported MC data and applies the Crystal 7-denominator / 5-tick green poison roll on successful ranged hits; BlackTortoise uses imported raw MC for ranged damage so current spawned `SmallDrake` data with `MC=0` correctly emits the range packet without damage or poison. `cargo fmt --check`, `cargo test -p mir2-simulation water_dragon -- --test-threads=1 --nocapture`, and `cargo test -p mir2-simulation black_tortoise -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure B.2 BlackTortoise halfmoon pass: completed AI 182 `BlackTortoise` close halfmoon branch. Runtime now maps the original close 1-in-5 branch to deterministic `tick % 5 == 0` coverage, emits `ObjectAttack` type 1, keeps imported DC damage, and fans damage through the Crystal halfmoon arc to opposing monsters. `cargo fmt --check` and `cargo test -p mir2-simulation black_tortoise -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure B.2 RedFoxman packet split pass: completed AI 45 `RedFoxman` ranged packet variant coverage and AI45/46 imported DC ranged damage mapping. Runtime now maps RedFoxman's original random spell type to deterministic type-0/type-1 `ObjectRangeAttack` coverage and resolves RedFoxman/WhiteFoxman ranged hits from imported Crystal DC data rather than the generic 7-damage fallback; teleport/fear and WhiteFoxman slow remain queued. `cargo fmt --check` and `cargo test -p mir2-simulation fox -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure B.2 WhiteFoxman slow pass: completed AI 46 `WhiteFoxman` type-1 slow branch. Runtime now maps the original 1-in-8 branch to deterministic `tick % 8 == 0` coverage, emits `ObjectRangeAttack` type 1, schedules a 300 ms delayed status-only pending action without `Struck`/damage messaging, and applies the Crystal level-scaled slow poison check. `cargo fmt` and `cargo test -p mir2-simulation fox -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure B.2 StrayCat push pass: completed AI 117 `StrayCat` close push variant. Runtime now maps the original close type-1 branch to deterministic `tick % 10 == 0` coverage, emits `ObjectAttack` type 1, pushes the player one tile in the attack direction, and resolves follow-up line damage from imported raw MC data; current spawned StrayCat data has `MC=0`, so the push is visible while follow-up damage is correctly gated. `cargo fmt --check` and `cargo test -p mir2-simulation cat_ -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure B.2 HellBomb poison pass: completed AI 99 `HellBomb` poison variants. Runtime now applies HellBomb1 Frozen, HellBomb2 Dazed, and HellBomb3 Bleeding for five ticks on successful player explosion hits, while preserving immobility, damage immunity, delayed attack/death packets, and radius damage. `cargo fmt --check` and `cargo test -p mir2-simulation hell_bomb -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure B.2 ManectricKing close push pass: completed AI 88 `ManectricKing` / spawned `Master_DragonYang` close type-1 push line branch. Runtime now maps the original close-range random branch to deterministic `tick % 3 == 0` coverage, emits `ObjectAttack` type 1, resolves damage from imported DC data, and pushes the player along the Crystal line distance before delayed damage; low-HP mass attack remains queued. `cargo fmt`, `cargo test -p mir2-simulation manectric_king -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure B.2 SeedingsGeneral close melee pass: completed AI 121 `SeedingsGeneral` close mixed melee coverage. Runtime now keeps the normal close type-0 Blood Attack on imported DC and maps the original close Green Splash branch to deterministic `tick % 5 == 0` coverage, emitting `ObjectAttack` type 1 and resolving delayed imported MC damage without the ranged slow/frozen poison hooks. `cargo fmt --check` and `cargo test -p mir2-simulation seedings_general -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure B.2 ManectricKing mass pass: completed AI 88 `ManectricKing` / spawned `Master_DragonYang` low-HP mass attack. Runtime now triggers the Crystal HP<20% type-0 `ObjectRangeAttack` branch behind a 2-6 second cooldown, schedules imported MC damage with the original `distance * 50 + 750ms` delay, and fans the hit to opposing monsters within seven tiles. `cargo fmt`, `cargo test -p mir2-simulation manectric_king -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure B.2 TucsonMage WideLine pass: completed AI 126 `TucsonMage` multi-target WideLine coverage. Runtime now keeps the non-adjacent type-1 branch, adds the adjacent 1-in-3 type-1 selection, preserves current zero-MC no-damage gating for imported TucsonMage data, and fans nonzero-MC WideLine damage through the Crystal forward cell plus three two-step lanes with per-target delay. `cargo fmt --check` and `cargo test -p mir2-simulation tucson_mage -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure B.2 HellKeeper fanout/Dazed pass: completed AI 79 `HellKeeper` view-range target fanout and nonzero-MC type-1 semantics. Runtime now applies both type-0 DC and type-1 MC branches to opposing monsters in Crystal `FindAllTargets(ViewRange)` style, keeps current zero-MC HellKeeper type-1 packets damage-free, and applies Dazed only after a successful nonzero-MC player hit with defence mitigation. `cargo fmt --check` and `cargo test -p mir2-simulation hell_keeper -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure B.2 SnowWolf fanout/poison pass: completed AI 179 `SnowWolf` nonzero-MC type-1 semantics and `FindAllTargets(2)` coverage. Runtime now keeps current zero-MC SnowWolf type-1 packets damage-free, applies defence-mitigated raw MC when nonzero data is available, fans both branches to nearby opposing monsters, and applies the Crystal Slow/Frozen rolls after successful type-1 player hits. `cargo fmt --check` and `cargo test -p mir2-simulation snow_wolf -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure B.2 ManectricClaw cone pass: completed AI 86 `ManectricClaw` / spawned `Chieftain_Priest` full thrust cone fanout. Runtime now applies the 500 ms IceThrust hit to opposing monsters in the Crystal three-column, three-row cone, using near DC for the first two rows and far MC for the third row, while preserving the existing player slow/frozen rolls; random thrust movement remains queued. `cargo fmt --check` and `cargo test -p mir2-simulation manectric_claw -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure B.2 GuardianRock delayed pull pass: completed AI 48 `GuardianRock` delayed packet and pull-distance behavior. Runtime now queues the Crystal 500 ms delayed `ObjectRangeAttack`, keeps the branch damage-free, and pulls the player toward the rock by `min(distance - 1, 4)` tiles on the due tick; magic-resist checks remain queued because the simulation has no MagicResist stat yet. `cargo fmt --check` and `cargo test -p mir2-simulation guardian_rock -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure B.2 MirStatue fanout pass: completed AI 54 `DragonStatue` / spawned `MirStatue` target-radius damage fanout. Runtime now keeps the due-time `ObjectRangeAttack` packet and imported DC player hit, then applies the same delayed hit to opposing monsters within two tiles of the player target in Crystal `FindAllTargets(2)` style; sleeping wake/revive state remains queued. `cargo fmt --check` and `cargo test -p mir2-simulation mir_statue -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure B.2 GreatFoxSpirit HP-stage pass: completed AI 50 `GreatFoxSpirit` HP-stage presentation. Runtime now computes Crystal's four-stage HP threshold during monster AI processing, advances `extra_byte` monotonically, and emits an existing-object `ObjectMonster` update so clients receive the stage change; recall teleport, GuardianRock activation, and target `ObjectEffect` remain queued. `cargo test -p mir2-simulation great_fox_spirit -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure B.2 GreatFoxSpirit GuardianRock pass: completed AI 50 `GreatFoxSpirit` nearby GuardianRock activation semantics. Runtime now activates AI 48 `GuardianRock` units within 20 tiles while the fox is spawned/alive and deactivates them when the fox dies, with AI 48 attack gating bound to that active flag; recall teleport and target `ObjectEffect` remain queued. `cargo test -p mir2-simulation great_fox_spirit -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure B.2 GreatFoxSpirit recall pass: completed AI 50 `GreatFoxSpirit` far-player recall baseline. Runtime now follows Crystal's `>3` and `<=30` target distance window, deterministic 1-in-10 recall trigger, 10-second cooldown, 3-in-4 actual recall branch, and moves the player to an occupiable tile adjacent to the fox while skipping the attack tick after a successful recall; multi-target ordering and MagicResist remain queued, while teleport effect packets were completed in a later closure pass. `cargo test -p mir2-simulation great_fox_spirit -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure B.2 monster `ObjectEffect` packet pass: added protocol/runtime support for Crystal `ObjectEffect` packet ID 124 and wired target effect broadcasts for AI 13 `RedMoonEvil` and ranged AI 50 `GreatFoxSpirit` fanout targets. `cargo test -p mir2-protocol object_effect_packet_uses_crystal_payload -- --nocapture`, `cargo test -p mir2-simulation great_fox_spirit -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation red_moon_evil -- --test-threads=1 --nocapture`, and `cargo test -p mir2-game-data crystal_monster_ai_summary_classifies_manifest_families -- --nocapture` passed.
- 2026-04-22, 100% closure B.2 ManectricClaw movement pass: completed AI 86 `ManectricClaw` random pre-thrust movement. Runtime now maps Crystal's 1-in-2 ranged `MoveTo(Target.CurrentLocation)` branch to a deterministic one-step move toward the target with next/previous-direction fallback, emits `ObjectWalk`, applies the short action delay, and skips the range attack on that tick. `cargo test -p mir2-simulation manectric_claw -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure B.2 BoneLord slave pass: completed AI 30 `BoneLord` HP-stage slave spawning. Runtime now initializes the Crystal three-stage HP tracker, emits the type-1 `ObjectAttack` wave when HP crosses a stage, spawns up to eight BoneSpearman/BoneBlademan/BoneArcher/BoneCaptain minions immediately with a 40-slave cap, assigns the original target, and skips the normal attack tick after the wave. `cargo test -p mir2-simulation bone_lord -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure B.2 ZumaTaurus slave pass: completed AI 17 `ZumaTaurus` HP-stage slave spawning. Runtime now initializes the Crystal seven-stage HP tracker, spawns up to eight ZumaStatue/ZumaGuardian/ZumaArcher/WedgeMoth/ZumaArcher3/ZumaStatue3/ZumaGuardian3 minions with a 40-slave cap when HP crosses a stage, assigns the original target, and preserves Crystal's behavior of continuing into the normal attack path after the spawn wave. `cargo test -p mir2-simulation zuma_taurus -- --test-threads=1 --nocapture` and `cargo fmt --check` passed.
- 2026-04-22, 100% closure B.2 ThunderElement movement/fanout pass: deepened AI 49 `ThunderElement` beyond the due-time attack baseline. Runtime now maps Crystal's 1-in-3 near-target repositioning to deterministic one-step movement before the attack, emits `ObjectWalk`, broadcasts the delayed `ObjectAttack` from the post-move location, applies the two-tile CompleteAttack damage to nearby opposing monsters as well as the player, and the later skill parity pass closes player-skill Repulsion push-damage with `ObjectPushed`. `cargo test -p mir2-simulation thunder_element -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure B.2 FrozenWarewolf HP/spawn pass: deepened AI 180 `SnowWolfKing` / spawned `FrozenWarewolf` beyond the primary attack. Runtime now maps the Crystal random HP-threshold packet variants to type 1 at >=60%, type 2 at >=30%, and type 3 below 30%, keeps the shared 500 ms imported DC hit, and performs the one-time below-70% SnowWolf slave spawn with three minions and 2s activation delay. Weak-target teleport, delayed death explosion, and pet transfer remain queued. `cargo test -p mir2-simulation frozen_warewolf -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure B.2 FrozenWarewolf death pass: completed AI 180 `SnowWolfKing` delayed death explosion damage. Runtime now schedules the Crystal 500 ms post-death one-tile explosion using imported DC damage against adjacent player and opposing-monster targets while preserving the normal death packet; weak-target teleport and pet transfer remain queued. `cargo test -p mir2-simulation frozen_warewolf -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure B.2 GeneralMeowMeow shield/thunder/slave pass: completed the remaining AI 123 `GeneralMeowMeow` shield and summon details. Runtime now tracks the Crystal HP shield windows, exposes `GeneralMeowMeowShield` in monster presentation, absorbs 100 incoming damage while shielded, emits protocol `ObjectSpell` ID 149 with `GeneralMeowMeowThunder` for shield-phase mass thunder, applies delayed MC damage to the player and opposing monsters near the target, and performs the 60s periodic StainHammerCat/BlackHammerCat/StrayCat/CatShaman slave spawn with the original 3-per-wave / 6-slave cap. `cargo test -p mir2-protocol object_spell_packet_uses_crystal_payload -- --nocapture` and `cargo test -p mir2-simulation general_meow_meow -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure B.2 Yimoogi lifecycle pass: completed the remaining AI 36 `Yimoogi` lifecycle details. Runtime now tracks parent/child sister state, emits the Crystal type-2 spawn attack after the four-second delay, spawns the same-name child with a sister link and no slave master presentation, triggers the final <=10% HP random teleport once, spawns two `WhiteSerpent` mobs at the old location, clears the player target after teleport, and suppresses configured death drops while the linked sister is alive. `cargo test -p mir2-simulation yimoogi -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure B.2 Lamia/Kirin IceThrust pass: completed AI 186 `Kirin` nonzero-MC IceThrust semantics while preserving current `Lamia` MC=0 gating. Runtime now emits type-2 `ObjectAttack` for the three-tile IceThrust branch when imported MC is nonzero, applies delayed MC damage with Crystal slow poison to the player, and fans the same cone damage to opposing monsters in the three-column/three-row thrust area. `cargo test -p mir2-simulation lamia -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure B.2 MirStatue sleep/wake pass: completed the remaining AI 54 `DragonStatue` / spawned `MirStatue` death-sleep lifecycle. Runtime now turns lethal damage into a sleeping 0-HP state instead of `ObjectDied`, blocks further damage while sleeping, suppresses death handling/drops/respawn, and wakes after Crystal's 15-minute delay with full HP plus `ObjectHealth`. `cargo test -p mir2-simulation mir_statue -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure B.2 TrapRock SpawnCorner/poison pass: completed the remaining AI 47 `TrapRock` spawn-corner ordering and repeated parent-attack paralysis details. Runtime now chooses the visible parent location through a deterministic Crystal-style cardinal `SpawnCorner` ordering, preserves child-rock placement around the trapped target, and rolls the parent `ObjectRangeAttack` branch's 1-in-8 three-second paralysis without adding damage. `cargo test -p mir2-simulation trap_rock -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure B.2 Armadillo retreat/backstep pass: added Crystal `ObjectBackStep` packet support and completed AI 124/125 retreat movement details. Runtime now emits Armadillo/Elder two-tile backstep packets, applies Armadillo's delayed retreat radius damage to nearby opposing monsters, pushes ArmadilloElder's type-1 target exactly two tiles without damage, and drives Elder's post-retreat run-away movement. `cargo test -p mir2-protocol object_back_step_packet_uses_crystal_payload -- --nocapture` and `cargo test -p mir2-simulation armadillo -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure B.2 Armadillo DigOutArmadillo spell pass: added Crystal `Spell::DigOutArmadillo` / value 206 support for `ObjectSpell` and wired AI 124/125 to emit the delayed drill-out spell object after `ObjectShow`. `cargo test -p mir2-protocol object_spell_packet_uses_crystal_payload -- --nocapture` and `cargo test -p mir2-simulation armadillo -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure B.2 TucsonGeneral rock spell pass: added Crystal `Spell::TucsonGeneralRock` / value 213 support and wired AI 131 rage to schedule 15 delayed rock `ObjectSpell` presentations, deterministic target-biased scatter, and raw-DC impact damage on player/opposing-monster targets at the struck location. `cargo test -p mir2-protocol object_spell_packet_uses_crystal_payload -- --nocapture` and `cargo test -p mir2-simulation tucson_general -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure B.2 Armadillo failed-retreat run-away pass: completed AI 124 `Armadillo` `_runAway` activation after retreat range damage fails against an immune target. Runtime now enters Armadillo run-away mode when the delayed retreat area hit resolves against a monster that ignores normal monster damage, matching Crystal's `Attacked <= 0` branch. `cargo test -p mir2-simulation armadillo -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure B.2 GreatFoxSpirit recall teleport packet pass: wired AI 50 far-player recall to emit Crystal `ObjectTeleportOut` / `ObjectTeleportIn` effect 11 around the existing recall movement. `cargo test -p mir2-simulation great_fox_spirit -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure B.2 CaveMaggot/ToxicGhoul harvest pass: added AI 7 and AI 28 to the shared Crystal `HarvestMonster` corpse state so death skips immediate drops and the corpse requires the default two skin-count harvest passes before `ObjectHarvested`. `cargo test -p mir2-simulation cave_maggot -- --test-threads=1 --nocapture` and `cargo test -p mir2-simulation toxic_ghoul -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure B.2 FrostTiger sit-down pass: added Crystal `ObjectSitDown` packet support and completed AI 34 sitting presentation. Runtime now schedules FrostTiger's passive sit-down state, blocks movement/attacks while sitting, emits sitting/standing `ObjectSitDown` packets, preserves `ObjectMonster.extra` for sitting presentation, and stands up when target-locked before resuming the existing ranged branch. `cargo test -p mir2-protocol object_sit_down_packet_uses_crystal_payload -- --nocapture` and `cargo test -p mir2-simulation frost_tiger -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure B.2 WoomaTaurus teleport packet pass: added Crystal `ObjectTeleportOut` / `ObjectTeleportIn` packet support and wired AI 11 surrounded teleport to emit both effect packets around the existing deterministic position update. `cargo test -p mir2-protocol object_teleport_packets_use_crystal_payloads -- --nocapture` and `cargo test -p mir2-simulation wooma_taurus -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure B.2 Foxman fear/kiting pass: completed AI 45/46 fear-window movement details. Runtime now makes RedFoxman and WhiteFoxman kite away or close in before opening a five-second attack window, keeps ranged attacks inside that window, and gives adjacent RedFoxman the Crystal effect-2 teleport branch using `ObjectTeleportOut` / `ObjectTeleportIn`. `cargo test -p mir2-simulation fox -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure B.2 HolyDeva fear/kiting pass: completed AI 38 `HolyDeva` / `PKSpirit` fear-window movement details. Runtime now refreshes the Crystal five-second attack window, kites away when the target is inside the six-tile attack range before that window, avoids generic close-chasing when outside range, and preserves the existing ranged attack/damage path inside the window. `cargo test -p mir2-simulation holy_deva -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure B.2 AxeSkeleton fear-window pass: completed AI 8 `AxeSkeleton` common ranged fear movement. Runtime now refreshes the Crystal five-second attack window, moves closer before attacking at six tiles when the window is expired, kites away when the target is inside six tiles, and preserves six-tile `ObjectRangeAttack` inside the window. `cargo test -p mir2-simulation axe_skeleton -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure B.2 Deer run-away pass: completed AI 2 `Deer` / `Deer1` / `Sheep` run-away flee movement. Runtime now marks the Crystal 1-in-7 run-away subset deterministically at spawn/respawn, lets those deer acquire nearby player targets only for fleeing, and emits `ObjectWalk` away from the player without enabling normal attacks. Exact `Quality` randomization remains queued with item/drop quality parity. `cargo test -p mir2-simulation deer -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure B.2 RestlessJar projectile timing pass: pinned AI 122 `RestlessJar` non-adjacent `ObjectRangeAttack` timing to Crystal's inherited `ProjectileAttack` formula (`distance * 50 + 500ms`) while preserving the current imported zero-MC no-damage gate. `cargo test -p mir2-simulation restless_jar -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure B.2 Trainer DPS chat pass: added protocol `ChatType::Trainer` / value 9 and wired AI 56 `Trainer` damage reports to emit Crystal-style damage/DPS chat without reducing trainer HP or killing the target, plus a five-second idle average report. `cargo test -p mir2-protocol server_packet_encoder_roundtrip_for_chat -- --nocapture` and `cargo test -p mir2-simulation trainer -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure D.1 durability/repair packet pass: added protocol, runtime, and gateway support for Crystal `DuraChanged` / ID 76 and `ItemRepaired` / ID 114. Player attacks now emit `DuraChanged` for weapon durability loss, delayed monster hits emit `DuraChanged` for worn equipment loss, repair powder emits one `ItemRepaired` per repaired equipped item, and single-item repair emits `ItemRepaired` before its existing feedback. `cargo test -p mir2-protocol durability_repair_server_packets_use_crystal_ids -- --nocapture`, `cargo check -p mir2-gateway`, `cargo test -p mir2-simulation durability -- --test-threads=1 --nocapture`, and `cargo test -p mir2-simulation repair -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure D.1 item grid/action ack pass: aligned protocol `MirGridType` values with Crystal for Inventory/Equipment/Trade/Storage/QuestInventory/Refine/HeroEquipment/HeroInventory and fixed runtime equipment slot indices for Crystal Belt=10, Boots=11, Mount=13. Added protocol/runtime/gateway support for current Crystal item action ack packets: `MoveItem` / 37, `EquipItem` / 38, `MergeItem` / 39, `RemoveItem` / 40, `RemoveSlotItem` / 41, `TakeBackItem` / 42, `StoreItem` / 43, `SplitItem1` / 45, `UseItem` / 52, and `DropItem` / 53. Current runtime item move/equip/remove/split/merge/drop/use/store/take-back flows now emit success/failure ack packets around the existing state changes and messages; full `UserItem` payload packets remain queued. `cargo test -p mir2-protocol mir_grid_type_values_match_crystal -- --nocapture`, `cargo test -p mir2-protocol item_action_ack_server_packets_use_crystal_ids -- --nocapture`, `cargo test -p mir2-simulation equipment_slot_indices_match_crystal -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation crystal_use_item_packet_consumes_inventory_slot -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation crystal_drop_item_packet_reduces_stack_and_spawns_ground_item -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation equip_and_remove_item_packets_emit_crystal_acks -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation storage -- --test-threads=1 --nocapture`, and `cargo check -p mir2-gateway` passed.
- 2026-04-22, 100% closure D.1 gold delta packet pass: added protocol/runtime/gateway support for Crystal `GainedGold` / ID 67 and `LoseGold` / ID 68. Current gold pickup emits `GainedGold` alongside the existing pickup feedback/removal path, and current `DropGold` emits `LoseGold` alongside the ground gold object spawn. `cargo test -p mir2-protocol gold_delta_server_packets_use_crystal_ids -- --nocapture`, `cargo test -p mir2-simulation dropped_gold_can_be_picked_up -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation drop_gold_packet_emits_lose_gold -- --test-threads=1 --nocapture`, and `cargo check -p mir2-gateway` passed.
- 2026-04-22, 100% closure D.1 sell/repair entry packet pass: added protocol/runtime/gateway support for Crystal `SellItem` / ID 111 and `RepairItem` / ID 113. Current sell flows now emit `SellItem` success/failure acks and `GainedGold` on success, while current repair and special-repair requests emit `RepairItem` before existing repair validation and `ItemRepaired` output. `cargo test -p mir2-protocol sell_and_repair_server_packets_use_crystal_ids -- --nocapture`, `cargo test -p mir2-simulation sell_item -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation repair_item_packet -- --test-threads=1 --nocapture`, and `cargo check -p mir2-gateway` passed.
- 2026-04-22, 100% closure D.1 delete-item packet pass: added protocol/runtime/gateway support for Crystal client `DeleteItem` / ID 149 and server `DeleteItem` / ID 79. Current inventory delete requests now echo the Crystal-shaped delete packet, reduce partial stacks, and remove full stacks while preserving Crystal's no-success-field response shape. `cargo test -p mir2-protocol delete_item_client_packet_uses_crystal_payload -- --nocapture`, `cargo test -p mir2-protocol item_action_ack_server_packets_use_crystal_ids -- --nocapture`, `cargo test -p mir2-protocol item_and_combat_client_packets_use_crystal_payloads -- --nocapture`, `cargo test -p mir2-simulation delete_item_packet_reduces_or_removes_inventory_stack -- --test-threads=1 --nocapture`, and `cargo test -p mir2-gateway delete_item_command_maps_to_protocol_packet -- --nocapture` passed.
- 2026-04-22, 100% closure D.1 split-item payload pass: added reusable Crystal `UserItem.Save`-order serialization in `mir2-protocol` and implemented server `SplitItem` / ID 44 alongside the existing `SplitItem1` ack. Current split-stack flows now emit `SplitItem1` success followed by a Crystal-shaped `SplitItem` payload for the newly created stack, with gateway JSON conversion. `cargo test -p mir2-protocol split_item_server_packet_uses_crystal_user_item_payload -- --nocapture`, `cargo test -p mir2-simulation storage_split_item_stack_creates_new_storage_slot -- --test-threads=1 --nocapture`, and `cargo check -p mir2-gateway` passed.
- 2026-04-22, 100% closure D.1 gained-item payload pass: implemented server `GainedItem` / ID 66 on the reusable Crystal `UserItem` serializer and wired current inventory pickup updates to emit `GainedItem` while gold pickup continues to emit `GainedGold`. Gateway JSON conversion now exposes the item payload. `cargo test -p mir2-protocol gained_item_server_packet_uses_crystal_user_item_payload -- --nocapture`, `cargo test -p mir2-simulation crystal_pickup_packet_collects_nearest_adjacent_ground_drop -- --test-threads=1 --nocapture`, and `cargo check -p mir2-gateway` passed.
- 2026-04-22, 100% closure D.0 item manifest import pass: extended the Crystal `Server.MirDB` generator to emit `crystal_item_manifest.json` with 1,628 item rows and core `ItemInfo.Save` fields, exposed item lookup helpers in `mir2-game-data`, and switched current mapped starter `UserItem.item_index` values to real Crystal item indices such as `(HP)DrugSmall` / 658. `node packages\tooling\scripts\generate-crystal-respawn-manifest.mjs`, `cargo test -p mir2-game-data crystal_item_manifest_loads -- --nocapture`, `cargo test -p mir2-simulation storage_split_item_stack_creates_new_storage_slot -- --test-threads=1 --nocapture`, `cargo check -p mir2-gateway`, and `cargo fmt --check` passed.
- 2026-04-22, 100% closure D.1 refresh-item payload pass: implemented server `RefreshItem` / ID 148 on the shared Crystal `UserItem` serializer and added gateway JSON conversion. Runtime trigger parity remains tracked separately because Crystal sends `RefreshItem` from specific item mutation flows such as weapon luck/curse and later equipment-affecting actions. `cargo test -p mir2-protocol refresh_item_server_packet_uses_crystal_user_item_payload -- --nocapture`, `cargo check -p mir2-gateway`, and `cargo fmt --check` passed.
- 2026-04-22, 100% closure D.1 refresh-item runtime trigger pass: added a representative Crystal `BenedictionOil` item-use branch that mutates equipped weapon Luck, serializes the equipped weapon as a Crystal `UserItem`, and emits `RefreshItem` with stat 15 / Luck. The current branch covers the deterministic success path; Crystal's random curse/no-effect outcomes remain future detail work. `cargo fmt --check`, `cargo test -p mir2-simulation benediction_oil_refreshes_weapon_after_luck_gain -- --test-threads=1 --nocapture`, and `cargo test -p mir2-simulation crystal_use_item_packet_consumes_inventory_slot -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure D.1 item-info packet pass: implemented client `RequestItemInfo` / ID 39 and server `NewItemInfo` / ID 32 with Crystal `ItemInfo.Save`-order serialization, gateway command/JSON conversion, and runtime lookup from `crystal_item_manifest.json`. `RequestItemInfo(658)` now returns the Crystal `(HP)DrugSmall` metadata packet. `cargo test -p mir2-protocol item_info_packets_use_crystal_payloads -- --nocapture`, `cargo test -p mir2-simulation request_item_info_packet_returns_crystal_item_info -- --test-threads=1 --nocapture`, `cargo test -p mir2-gateway request_item_info_command_maps_to_protocol_packet -- --nocapture`, `cargo check -p mir2-gateway`, and `cargo fmt --check` passed.
- 2026-04-22, 100% closure D.1 item StackSize pass: current item gain and stack merge paths now consult the imported Crystal item manifest for `StackSize`. Mapped stackables such as `(HP)DrugSmall` split into capped stacks of 20, partial merges cap the target stack and keep source leftovers, and mapped non-stackables such as `BronzeHelmet` stay at one item per stack. `cargo fmt --check`, `cargo test -p mir2-simulation add_or_increment_item_respects_crystal_stack_size -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation add_or_increment_item_keeps_crystal_non_stackables_single_count -- --test-threads=1 --nocapture`, and `cargo test -p mir2-simulation storage_merge_item_stack_caps_at_crystal_stack_size -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure D.1 inventory-full StackSize pass: current bag-capacity checks now account for existing partial stacks and imported Crystal `StackSize` before item grants mutate state. Pickup no longer removes a ground item when the bag cannot accept it, pickup can still stack into a full bag when an existing mapped stack has room, and shop/auction/NPC/quest reward gates use the same StackSize-aware capacity calculation before mutating gold, listings, quest state, or inventory. `cargo fmt --check`, `cargo test -p mir2-simulation pickup_preserves_ground_drop_when_inventory_is_full -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation pickup_can_stack_into_full_inventory_when_stack_has_room -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation stage5_shop_and_auction_full_bag_preserve_gold_and_items -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation crystal_npc_giveitem_full_bag_preserves_inventory -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation quest_turn_in_full_bag_preserves_quest_state_and_rewards -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation crystal_npc_giveitem_adds_reward_to_inventory -- --test-threads=1 --nocapture`, and `cargo test -p mir2-simulation add_or_increment_item_respects_crystal_stack_size -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure D.1 credit delta packet pass: implemented Crystal `GainedCredit` / ID 69 and `LoseCredit` / ID 70 server packet support in protocol IDs, encode/decode, trace names, and gateway JSON event conversion. Runtime credit mutation flows remain future work, but the packet surface is ready for imported credit shop/account systems. `cargo fmt --check`, `cargo test -p mir2-protocol currency_delta_server_packets_use_crystal_ids -- --nocapture`, `cargo test -p mir2-gateway credit_delta_server_events_expose_crystal_payload_fields -- --nocapture`, and `cargo check -p mir2-gateway` passed.
- 2026-04-22, 100% closure D.1 credit runtime gain pass: added runtime account credit state to world snapshots, `UserInformation.credit`, character saves, and legacy-save-compatible reloads, then wired mapped Crystal `CreditToken` scroll use to consume the token, add imported Crystal `ItemInfo.Price` credit value, and emit `GainedCredit`. Credit-spend / `LoseCredit` runtime paths remain pending with imported credit shop flows. `cargo fmt --check`, `cargo test -p mir2-simulation crystal_credit_token_use_adds_credit_and_emits_packet -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation crystal_credit_persists_and_updates_user_information -- --test-threads=1 --nocapture`, and `cargo test -p mir2-simulation legacy_character_save_without_npc_flag_states_uses_default -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure D.1 credit runtime spend pass: added a current `shop.buyCredit` runtime path that checks credit balance and StackSize-aware bag capacity before mutating state, emits `LoseCredit` on successful credit purchase, and preserves credit/items on insufficient-credit or full-bag failures. Imported Crystal game-shop product catalogs remain pending. `cargo fmt --check`, `cargo test -p mir2-simulation stage5_trade_shop_and_auction_are_transactional -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation stage5_trade_shop_and_auction_cancel_error_paths_preserve_gold -- --test-threads=1 --nocapture`, and `cargo test -p mir2-simulation stage5_shop_and_auction_full_bag_preserve_gold_and_items -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure D.1 item slot/seal packet pass: implemented Crystal `ItemSlotSizeChanged` / ID 115 and `ItemSealChanged` / ID 116 server packet support in protocol IDs, encode/decode, trace names, and gateway JSON event conversion. Runtime seal mutation flows remain future work, but the item packet surface now matches the adjacent Crystal durability/repair packet block. `cargo fmt --check`, `cargo test -p mir2-protocol item_slot_and_seal_server_packets_use_crystal_ids -- --nocapture`, `cargo test -p mir2-gateway item_slot_and_seal_server_events_expose_crystal_payload_fields -- --nocapture`, and `cargo check -p mir2-gateway` passed.
- 2026-04-22, 100% closure D.1 item slot-size runtime pass: added current equipment socket slot state and a Stage 5 `item.addSocket` path that mirrors Crystal's successful socket-size mutation by increasing the equipped item slot count and emitting `ItemSlotSizeChanged` with the equipment unique id. Full Crystal gem/socket validation remains pending with imported socket item flows. `cargo fmt --check`, `cargo test -p mir2-simulation stage5_item_add_socket_emits_item_slot_size_changed -- --test-threads=1 --nocapture`, and `cargo test -p mir2-simulation crystal_npc_service_links_emit_packets_and_close_dialog -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure D.1 item seal runtime pass: added current equipped-item seal expiry state and a Stage 5 `item.seal` path that records a .NET binary datetime expiry and emits `ItemSealChanged` with the equipment unique id. Full Crystal seal-item validation, reseal delay, and seal-source item handling remain pending. `cargo fmt --check`, `cargo test -p mir2-simulation stage5_item_seal_emits_item_seal_changed -- --test-threads=1 --nocapture`, and `cargo test -p mir2-simulation stage5_item_add_socket_emits_item_slot_size_changed -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure D.1 NPC service packet pass: implemented protocol/gateway support for Crystal `TeleportIn` / ID 101, `NPCGoods` / ID 102, `NPCSell` / ID 103, `NPCRepair` / ID 104, `NPCSRepair` / ID 105, `NPCRefine` / ID 106, `NPCCheckRefine` / ID 107, `NPCCollectRefine` / ID 108, `NPCReplaceWedRing` / ID 109, `NPCStorage` / ID 110, and `CraftItem` / ID 112. Protocol IO now supports little-endian .NET `Single` / `f32` fields for service rates, and `NPCGoods` uses the shared `UserItem` serializer. Runtime NPC service triggers remain future work. `cargo fmt --check`, `cargo test -p mir2-protocol npc_service_server_packets_use_crystal_ids -- --nocapture`, `cargo test -p mir2-gateway npc_service_server_events_expose_crystal_payload_fields -- --nocapture`, and `cargo check -p mir2-gateway` passed.
- 2026-04-22, 100% closure D.1 NPC service runtime trigger pass: wired imported Crystal NPC reserved service labels to baseline runtime packets. `@Storage` now emits `NPCStorage`, `@BuySell` emits `NPCGoods` plus `NPCSell`, `@Repair` emits `NPCRepair`, and the same mapper covers current buy, sell, special repair, craft, refine, refine-check, and wedding-ring service pages; generated shop goods, real service rates, and refine result state remain tracked under imported service data. `cargo fmt`, `cargo test -p mir2-simulation crystal_npc_reserved_service_labels_map_to_crystal_packets -- --test-threads=1 --nocapture`, and `cargo test -p mir2-simulation crystal_npc_service_links_emit_packets_and_close_dialog -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure D.1 NPC goods list pass: populated current buy/buy-sell/craft `NPCGoods` packets from imported Crystal NPC `[Trade]` and `[Recipe]` sections using manifest-backed `UserItem` payloads. The WickedTrader `@BuySell` integration test verifies Crystal `(HP)DrugSmall` item index 658 entries with counts 1 and 5, and the CraftLady `@Craft` path verifies Crystal `(HP)DrugXL` item index 664 in the craft panel. Service rates, hide-added-stat settings, and buy-back lists remain pending. `cargo fmt --check`, `cargo test -p mir2-simulation crystal_npc_reserved_service_labels_map_to_crystal_packets -- --test-threads=1 --nocapture`, and `cargo test -p mir2-simulation crystal_npc_service_links_emit_packets_and_close_dialog -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure D.1 repair oil scroll pass: mapped Crystal `RepairOil` and `WarGodOil` scroll items through the imported item manifest and wired their current weapon-use branches to emit `ItemRepaired`. `RepairOil` now partially repairs the equipped weapon in the runtime durability scale, and `WarGodOil` fully repairs it while preserving failure/no-consume behavior when the weapon does not need repair. `cargo fmt --check` and `cargo test -p mir2-simulation repair_and_war_god_oil_emit_item_repaired_for_weapon -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure D.1 NPC service rate pass: extended the Crystal `Server.MirDB` generator to emit `crystal_npc_info_manifest.json` with 375 NPC placement rows including `NPCInfo.Rate`, exposed NPC info lookup helpers in `mir2-game-data`, and applied `Rate / 100F` to current `NPCGoods`, `NPCRepair`, and `NPCSRepair` packets. The Natural Cave `WickedTrader` integration now verifies Crystal rate 200 as packet rate 2.0. `node packages\tooling\scripts\generate-crystal-respawn-manifest.mjs`, `cargo fmt --check`, `cargo test -p mir2-game-data crystal_npc_info_manifest_loads -- --nocapture`, `cargo test -p mir2-simulation crystal_npc_reserved_service_labels_map_to_crystal_packets -- --test-threads=1 --nocapture`, and `cargo test -p mir2-simulation crystal_npc_service_links_emit_packets_and_close_dialog -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure D.1 NPC goods flags pass: aligned current `NPCGoods.HideAddedStats` with Crystal's `Settings.GoodsHideAddedStats = true` for buy, buy-new, buy-used, and buy-sell service packets, while craft stays on Crystal's default `false`. `@BuyBack` now opens an empty buy panel until real buy-back state is implemented instead of reusing static `[Trade]` goods. `cargo fmt --check`, `cargo test -p mir2-simulation crystal_npc_reserved_service_labels_map_to_crystal_packets -- --test-threads=1 --nocapture`, and `cargo test -p mir2-simulation crystal_npc_service_links_emit_packets_and_close_dialog -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure D.1 NPC buy-back goods pass: added a current-session NPC sell-service context and per-script buy-back list. Selling through an active `@BuySell` / `@Sell` service now stores the sold `UserItem` under that NPC with Crystal's 20-item cap, and `@BuyBack` returns those goods with the NPC's imported rate. Buy-back purchase and expiry timing remain pending. `cargo fmt --check`, `cargo test -p mir2-simulation crystal_npc_sell_updates_buy_back_goods_for_active_service -- --test-threads=1 --nocapture`, and `cargo test -p mir2-simulation crystal_npc_service_links_emit_packets_and_close_dialog -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure D.1 NPC buy-item packet pass: added Crystal `BuyItem` / ID 51 client-packet support with `ItemIndex`, `Count`, and `PanelType`, gateway `buyItem` command mapping, and current runtime purchase handling for static NPC `[Trade]` goods plus NPC buy-back goods. Buying now checks imported Crystal price/rate and bag capacity, deducts gold with `LoseGold`, emits `GainedItem`, removes buy-back entries when applicable, and refreshes the buy-back `NPCGoods` list. Full used-goods persistence and exact purchase edge cases remain pending. `cargo fmt --check`, `cargo test -p mir2-protocol item_and_combat_client_packets_use_crystal_payloads -- --nocapture`, `cargo test -p mir2-gateway buy_item_command_maps_to_protocol_packet -- --nocapture`, `cargo test -p mir2-simulation crystal_npc_buy_item_packet_purchases_trade_goods -- --test-threads=1 --nocapture`, and `cargo test -p mir2-simulation crystal_npc_sell_updates_buy_back_goods_for_active_service -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure D.1 NPC sell price/count pass: aligned current `SellItem` runtime semantics with Crystal stack-count behavior and imported item pricing. Stackable items now sell only the requested `Count`, leave the remaining stack in the bag, grant gold from `ItemInfo.Price / 2` for mapped Crystal items, and keep buy-back entries scoped to the sold quantity. `cargo fmt --check`, `cargo test -p mir2-simulation sell_item_packet_removes_item_and_adds_gold_without_duplication -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation sell_item_invalid_slot_preserves_inventory_and_gold -- --test-threads=1 --nocapture`, and `cargo test -p mir2-simulation crystal_npc_sell_updates_buy_back_goods_for_active_service -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure D.1 Crystal drop-table runtime pass: wired imported Crystal `MonsterInfo.DropPath` to `Envir/Drops` table lookup and made current runtime death/harvest rewards prefer those tables before starter fallback. Crystal drop entries now resolve grouped sections, deterministic chance rolls, gold amounts, and Crystal item metadata-backed rewards; verified OmaFighter `Gold 2000` death pickup, Hen `Chicken`, Deer `Venison`, and fallback Field Wasp/Training Dummy behavior. Quest-drop `Q` gating, ownership timing, visibility source audit, quality/random-stat rolls, and full inventory capacity semantics remained queued under the broader drop-table item at that point. `cargo test -p mir2-game-data crystal_monster_drop_path_resolves_imported_drop_table -- --nocapture`, `cargo test -p mir2-game-data crystal_drop -- --nocapture`, `cargo test -p mir2-simulation harvest -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation dropped_gold_can_be_picked_up -- --test-threads=1 --nocapture`, and `cargo test -p mir2-simulation crystal_drop_table_gold_entry_spawns_pickup_gold_for_monster_death -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure D.1 Crystal gold drop range pass: aligned imported Crystal `Gold N` drop entries with `DropInfo.AttemptDrop`, using `N / 2` as the inclusive lower bound and `N + N / 2` as the exclusive upper bound instead of spawning the raw table amount. The OmaFighter gold-drop regression now verifies the deterministic ranged amount is picked up through `GainedGold`. `cargo fmt --check` and `cargo test -p mir2-simulation crystal_drop_table_gold_entry_spawns_pickup_gold_for_monster_death -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure D.1 Crystal harvest gained-item packet pass: harvest reward transfer now mirrors Crystal `player.GainItem(...)` by emitting `GainedItem` for each transferred item before `ObjectHarvested`, and harvest transfer ignores gold rewards because Crystal `HarvestMonster` only moves `reward.Items` into `_drops`. Training Dummy, Hen, and Deer harvest tests now assert the gained item payloads. `cargo fmt --check`, `cargo test -p mir2-simulation harvest -- --test-threads=1 --nocapture`, and `cargo test -p mir2-simulation crystal_drop_table_gold_entry_spawns_pickup_gold_for_monster_death -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure D.1 Crystal meat durability pass: imported Crystal item drops now carry `ItemInfo.Durability` into `UserItem` current/max durability through ground-drop pickup and harvest reward transfer. `ItemType.Meat` harvest rewards now apply the Deer AI 2 quality bonus to current durability, matching Crystal's `item.CurrentDura += Quality` path. Hen and Deer harvest regressions verify max durability plus the current-durability path that was completed in the later `CreateDropItem` pass. `cargo fmt --check`, `cargo test -p mir2-simulation harvest -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation pickup -- --test-threads=1 --nocapture`, and `cargo test -p mir2-simulation crystal_drop_table_gold_entry_spawns_pickup_gold_for_monster_death -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure D.1 Crystal drop ownership pass: monster death drops now attach a Crystal-style owner pickup window for the current player, and pickup rejects non-owner/non-group attempts until the window expires while allowing configured group-owner bypass. Player-owned monster gold drops remain immediately collectible. `cargo fmt --check`, `cargo test -p mir2-simulation pickup -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation harvest -- --test-threads=1 --nocapture`, and `cargo test -p mir2-simulation crystal_drop_table_gold_entry_spawns_pickup_gold_for_monster_death -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure D.1 Crystal group pickup notice pass: imported `ItemInfo.ShowGroupPickup` now travels with ground-drop payloads, and grouped pickup of marked items emits a Crystal-style system notice (`Scout Picked up: {SpiritBlade}`) alongside `GainedItem`. Non-grouped and unmarked pickups keep the existing local pickup message only. `cargo fmt --check`, `cargo test -p mir2-simulation pickup -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation harvest -- --test-threads=1 --nocapture`, and `cargo test -p mir2-simulation crystal_drop_table_gold_entry_spawns_pickup_gold_for_monster_death -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure D.1 pickup/harvest capacity pass: ground-drop pickup and harvest reward transfer preserve item rewards when slot/stack capacity cannot accept them, while still allowing stack-into-full-bag cases that fit existing partial stacks. A later source audit corrected this pass by removing the runtime weight gate, because Crystal `CanGainItem` does not reject pickup/harvest gains by bag weight. `cargo fmt --check`, `cargo test -p mir2-simulation pickup -- --test-threads=1 --nocapture`, and `cargo test -p mir2-simulation harvest -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure D.1 Crystal drop current durability pass: imported item drops now set max durability from `ItemInfo.Durability` and current durability from Crystal-style deterministic `min(max, roll + 1000)` before harvest meat quality. Hen/Deer harvest tests now verify current/max durabilities through `GainedItem`; Crystal random stat upgrade tables remain pending. `cargo fmt --check`, `cargo test -p mir2-simulation harvest -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation pickup -- --test-threads=1 --nocapture`, and `cargo test -p mir2-simulation crystal_drop_table_gold_entry_spawns_pickup_gold_for_monster_death -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure D.1 Crystal identified flag pass: manifest-backed runtime `UserItem` payloads now derive `Identified` from Crystal `ItemInfo.NeedIdentify`, matching `CreateDropItem`'s `if (!info.NeedIdentify) item.Identified = true` behavior for current pickup, harvest, gained-item, and equipment refresh payloads. Added positive/negative coverage for a normal potion and `MysteryHelmet`, and Hen/Deer harvest rewards now assert identified item payloads. `cargo fmt --check`, `cargo test -p mir2-simulation user_item_identified_flag_follows_crystal_need_identify -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation harvest -- --test-threads=1 --nocapture`, and `cargo test -p mir2-simulation pickup -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure D.1 Crystal current-cell pickup pass: player `PickUp` now scans only drops on the player's current tile, matching Crystal `CurrentMap.GetCell(CurrentLocation)`, and direct pickup rejects off-cell targets. Adjacent ground drops remain on the map until the player stands on the drop cell. `cargo fmt --check`, `cargo test -p mir2-simulation pickup -- --test-threads=1 --nocapture`, and `cargo test -p mir2-simulation crystal_drop_table_gold_entry_spawns_pickup_gold_for_monster_death -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure D.1 Crystal ground-drop expiry pass: new ground drops now carry a Crystal `ItemTimeOut=30` minute expiry, represented as 1,800 simulation ticks, and world ticking despawns expired drops so visible clients receive the normal `ObjectRemove` through AOI finalization. `cargo fmt --check`, `cargo test -p mir2-simulation ground_drop_expires_after_crystal_item_timeout -- --test-threads=1 --nocapture`, and `cargo test -p mir2-simulation pickup -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure D.1 Crystal MaxDropGold pass: monster ground gold now follows Crystal `DropGold` chunking with `Settings.MaxDropGold=2000`, so large gold rewards spawn as 2,000-gold chunks plus the final modulo chunk. The helper intentionally preserves Crystal's exact-division zero remainder behavior. `cargo fmt --check`, `cargo test -p mir2-simulation crystal_ground_gold_chunks_follow_max_drop_gold_formula -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation crystal_drop_table_gold_entry_spawns_pickup_gold_for_monster_death -- --test-threads=1 --nocapture`, and `cargo test -p mir2-simulation pickup -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure D.1 Crystal gold cap pickup pass: ground gold pickup now checks Crystal `CanGainGold` semantics before mutating state, so a player at `uint.MaxValue` gold leaves the ground drop in place and receives no `GainedGold` packet instead of overflowing. `cargo fmt --check`, `cargo test -p mir2-simulation pickup_preserves_gold_drop_when_gold_cap_is_full -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation pickup -- --test-threads=1 --nocapture`, and `cargo test -p mir2-simulation dropped_gold_can_be_picked_up -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure D.1 Crystal player DropGold edge pass: player `DropGold` now follows Crystal's edge behavior by allowing a 0-gold ground object with `LoseGold(0)` and returning no packets when the requested amount exceeds the player's current gold. `cargo fmt --check`, `cargo test -p mir2-simulation drop_gold_packet -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation dropped_gold_can_be_picked_up -- --test-threads=1 --nocapture`, and `cargo test -p mir2-simulation pickup -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure D.1 Crystal ground item grade display pass: ground `ObjectItem` packets now use the imported Crystal item `Grade` and grade-to-name-colour mapping for manifest-backed drops. SpiritBlade now spawns as grade 2 with Crystal rare `DeepSkyBlue` name colour instead of a flat white grade-0 item. Random-added-stat cyan remains pending with random stat generation. `cargo fmt --check`, `cargo test -p mir2-simulation ground_item_object_uses_crystal_grade_and_name_colour -- --test-threads=1 --nocapture`, and `cargo test -p mir2-simulation pickup -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure D.1 Crystal player DropItem semantics pass: player `DropItem` now routes through slot/count packet semantics, splits stackable items by requested count, returns Crystal-shaped failure acks for invalid counts or missing items, rejects manifest-backed `DontDrop` items, and applies `DestroyOnDrop` by deleting the requested quantity without spawning a ground item. `cargo fmt --check`, `cargo test -p mir2-simulation drop_item_packet -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation dropped_inventory_item_can_be_removed_from_bag_and_spawned_on_ground -- --test-threads=1 --nocapture`, and `cargo test -p mir2-simulation crystal_drop_item_packet_reduces_stack_and_spawns_ground_item -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure D.1 Crystal AddItem belt-priority pass: current item gains now merge stackables across player belt stacks before inventory stacks, place Potion/Scroll/Script effect 1 gains into belt slots 0..3 first, place Amulets into belt slots 4..5 first, then fall back to normal bag slots. `UseItem` now consumes the referenced belt slot for Crystal belt packets instead of consuming a same-key inventory item. `cargo fmt --check`, `cargo test -p mir2-simulation crystal_add_item -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation use_item -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation pickup -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation add_or_increment_item -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation crystal_npc_giveitem -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation quest_turn_in_full_bag_preserves_quest_state_and_rewards -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation stage5_shop_and_auction_full_bag_preserve_gold_and_items -- --test-threads=1 --nocapture`, and `cargo test -p mir2-simulation crystal_npc_buy_item_packet_purchases_trade_goods -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure D.1 Crystal drop placement pass: current player item drops, player gold drops, and monster ground drops now route through Crystal-style `ItemObject.Drop(distance)` placement. Runtime scans the same ring order, rejects blocked cells and map transfer source tiles, enforces `DropStackSize=5` by ground item object count, chooses the least-populated fallback cell, and preserves Crystal ranges for manual item drops (`1`), manual gold drops (`5`), and monster drops (`Settings.DropRange=4`). `cargo fmt --check`, `cargo test -p mir2-simulation crystal_drop_search -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation drop -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation pickup -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation harvest -- --test-threads=1 --nocapture`, and `cargo test -p mir2-simulation crystal_drop_table_gold_entry_spawns_pickup_gold_for_monster_death -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure D.1 Crystal quest-drop `Q` pass: imported `Q` drop entries now preserve the marker after chance rolls, normal monster death drops attempt quest-inventory gain before suppressing ground fallback, harvest transfers use the same quest gate, and the Field Wasp quest path now shares the same active-quest/full-quest-inventory semantics. `cargo fmt --check`, `cargo test -p mir2-simulation crystal_q_drop_marker -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation crystal_quest_required_drop -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation drop -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation harvest -- --test-threads=1 --nocapture`, and `cargo test -p mir2-simulation quest -- --test-threads=1 --nocapture` passed.
- 2026-04-22, frontend shell first patch: login account/password inputs now submit on Enter through the existing login handler; viewport tile hit controls now mark themselves UI-interactive and stop pointer bubbling to prevent scene-level double dispatch while leaving empty-space clicks on the scene frame. `npm.cmd run build --prefix E:\mir2\mir2-web3\apps\web` passed.
- 2026-04-22, 100% closure D.1 Crystal random-stat baseline: current imported Crystal drop-created items now use `random_stats_id` profiles to apply deterministic Crystal-style `RandomomRange` rolls for MaxDura, MaxAC, and MaxDC. Runtime adds durability bonuses before pickup/harvest transfer and preserves added attack/defence into `UserItem.added_stats` on `GainedItem`; this baseline was superseded later the same day by the full random-stat payload pass below. `cargo fmt --check`, `cargo test -p mir2-simulation crystal_resolved_drop_applies_random_attack_defence_and_durability -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation pickup_preserves_random_added_stats -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation drop -- --test-threads=1 --nocapture`, and `cargo test -p mir2-simulation harvest -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure D.1 full random-stat payload pass: current imported Crystal drop-created items now roll the full current Jev profile family baseline into generic `UserItemStat` payloads, curse flag, and socket slots while preserving legacy added-attack/defence compatibility. The metadata survives ground drops, pickup, harvest transfer, equipment/inventory state, `GainedItem`, and JSON save/reload; data-driven `RandomItemStats.ini` generation was completed in the follow-up manifest pass. `cargo fmt --check`, `cargo test -p mir2-simulation random -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation item_roll_fields_persist_through_save_and_reload -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation drop -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation item -- --test-threads=1 --nocapture`, and `cargo test -p mir2-simulation -- --test-threads=1` passed.
- 2026-04-22, 100% closure D.1 RandomItemStats manifest pass: generated `crystal_random_item_stats_manifest.json` from Crystal `RandomItemStats.ini`, added typed game-data accessors, and moved runtime random-stat profile lookup off the hardcoded table while keeping `random_stats_id == 0` as the no-profile path. `cargo test -p mir2-game-data crystal_random_item_stats_manifest_loads -- --nocapture`, `cargo test -p mir2-game-data -- --nocapture`, `cargo test -p mir2-simulation random -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation drop -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation item -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation -- --test-threads=1`, and `cargo fmt --check` passed.
- 2026-04-22, 100% closure D.1 Crystal GROUP drop semantics pass: generated drop manifests now preserve group-shaped entries with nested children, and runtime recursively executes Crystal `GROUP`, `GROUP*`, and `GROUP^` rules. Successful child gold accumulates, `GROUP*` keeps one successful child item after all child rolls, `GROUP^` stops after the first successful child, and nested groups compose through the same evaluator. `cargo test -p mir2-game-data crystal_drop -- --nocapture`, `cargo test -p mir2-game-data -- --nocapture`, `cargo test -p mir2-simulation crystal_group -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation crystal_nested_group -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation drop -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation item -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation -- --test-threads=1`, and `cargo fmt --check` passed.
- 2026-04-22, 100% closure D.1 Crystal pickup visibility/rejection pass: source audit confirmed normal `ItemObject.Drop()` / `Spawned()` broadcasts item and gold drops immediately, including owned monster drops; owner windows restrict pickup, not visibility. Runtime `PickUp` now scans only the current cell, skips owner-blocked/full-bag/gold-cap candidates when a later drop is pickable, emits the owner warning only if no later pickable candidate exists, and allows overweight pickup/harvest gains because Crystal `CanGainItem` gates by slot/stack rather than bag weight. `cargo test -p mir2-simulation pickup_packet_skips -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation pickup_respects_crystal_drop_owner_window -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation pickup_allows_overweight_item_like_crystal -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation pickup -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation drop -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation harvest -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation -- --test-threads=1`, and `cargo fmt --check` passed.
- 2026-04-22, 100% closure D.1 Crystal HarvestMonster pending-drop pass: current harvest monsters now mirror Crystal's `_drops` lifecycle. The final skin pass only materializes pending rewards; the follow-up harvest transfers them and emits `ObjectHarvested`; pending rewards are not re-rolled on transfer; and full-bag leftovers remain pending for a later retry. Hen/Deer/CaveMaggot/ToxicGhoul tests now cover the follow-up transfer timing, and a full-bag regression covers retained pending drops. `cargo test -p mir2-simulation harvest -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation hen_is_passive -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation drop -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation -- --test-threads=1`, and `cargo fmt --check` passed.
- 2026-04-22, 100% closure D.1 Crystal harvest owner/EXPOwner pass: current harvest corpse scanning now mirrors Crystal owner rejection for the front-centered scan. Defeated harvest monsters attach current-player ownership, non-owner/non-group corpses are skipped while later eligible corpses remain harvestable, grouped owners can harvest, and owner-blocked-only searches emit `NoNearbyOwnedCarcasses`. `cargo test -p mir2-simulation harvest_owner -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation harvest_skips_owner -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation harvest_allows_owner_group -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation harvest -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation drop -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation -- --test-threads=1`, and `cargo fmt --check` passed.
- 2026-04-22, 100% closure D.1 Crystal economy rejection pass: current `SellItem` now requires an active Crystal sell service before mutating inventory/gold, rejects partial-stack sales that would overflow Crystal's `uint.MaxValue` gold cap, and preserves failure ack semantics. Current credit-shop purchases now follow Crystal game-shop mailbox delivery by emitting `LoseCredit`, creating a mail attachment, and deferring bag capacity checks until mail claim. `cargo fmt --check`, `cargo test -p mir2-simulation sell_item -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation stage5_credit_shop -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation stage5_trade_shop_and_auction -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation item -- --test-threads=1 --nocapture`, and `cargo test -p mir2-simulation -- --test-threads=1` passed.
- 2026-04-22, 100% closure D.1 Crystal BuyItem rejection pass: current `BuyItem` now follows Crystal silent-return semantics for invalid panel type, zero/invalid count, missing active NPC service, non-buy pages such as `@Repair`, missing goods, missing item metadata, insufficient gold, and full bags. These branches preserve inventory and gold and emit no packets. `cargo fmt --check`, `cargo test -p mir2-simulation crystal_npc_buy_item -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation item -- --test-threads=1 --nocapture`, and `cargo test -p mir2-simulation -- --test-threads=1` passed.
- 2026-04-23, 100% closure D.1 Crystal NPC repair rejection/cost pass: current `RepairItem` / `SRepairItem` now emit the Crystal entry ack, require a matching active `@Repair` / `@SRepair` service page, find the current backpack item by unique id, apply `DontRepair` / `NoSRepair` and script `[Types]` rejection messages, calculate normal and triple special-repair cost from Crystal item price/rate, silently return on insufficient gold, emit `LoseGold` plus `ItemRepaired` on success, reduce max durability only for normal repair, and keep item-use repair powder/oil flows separate. `cargo fmt --check`, `cargo test -p mir2-simulation repair_item -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation repair -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation crystal_npc_service_links -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation item -- --test-threads=1 --nocapture`, and `cargo test -p mir2-simulation -- --test-threads=1` passed with 453 tests.
- 2026-04-23, 100% closure D.1 Crystal SellItem flag/type/price pass: current `SellItem` now follows Crystal `DontSell` and script `[Types]` rejection behavior, returns ack-only failures for zero count, inactive service, missing item, oversized count, `DontSell`, and partial-stack gold overflow, emits `CannotSellItemHere` only for type mismatch, uses Crystal `UserItem.Price() / 2` style sale value, preserves full-stack sale success with capped zero-gold gain at gold cap, and keeps `@SELL` / `@BUYSELL` as the only accepted sell pages. `cargo fmt --check`, `cargo test -p mir2-simulation sell_item -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation sell -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation item -- --test-threads=1 --nocapture`, and `cargo test -p mir2-simulation -- --test-threads=1` passed with 457 tests.
- 2026-04-22, 100% closure D.1 added-stat ground item colour pass: current added-stat ground items now use Crystal `ItemObject` Cyan name-colour semantics through `ObjectItem.name_colour_argb`, `GroundDropSnapshot.name_colour_argb`, gateway/web snapshot mapping, and the web ground-drop label. `cargo fmt --check`, `cargo test -p mir2-simulation ground_item_object_uses_cyan_name_colour_for_added_stats -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation ground_item_object_uses_crystal_grade_and_name_colour -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation drop -- --test-threads=1 --nocapture`, and `npm.cmd run build --prefix apps\web` passed.
- 2026-04-22, 100% closure D.1 NPC buy-back / used-goods pass: current sell-service entries are now player-scoped, survive save/reload, carry Crystal `GoodsBuyBackTime=60` expiry, move into NPC used goods on expiry, persist used goods in character save state, and remove buy-back/used entries after resale purchase while preserving durability and current added attack/defence payloads. `cargo fmt --check`, `cargo test -p mir2-simulation crystal_npc_buy_back -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation crystal_npc_buy_item_packet_purchases_trade_goods -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation sell -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation npc -- --test-threads=1 --nocapture`, and `cargo test -p mir2-simulation legacy_character_save_without_npc_flag_states_uses_default -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure D.1 socket capacity validation pass: current `item.addSocket` now checks imported item socket capacity before mutating equipment, rejects maxed or zero-capacity items without `ItemSlotSizeChanged`, and keeps the success path covered on a capacity-backed manifest item. Full Crystal source gem validation remains queued. `cargo fmt --check`, `cargo test -p mir2-simulation stage5_item_add_socket -- --test-threads=1 --nocapture`, `cargo test -p mir2-simulation stage5_item_seal_emits_item_seal_changed -- --test-threads=1 --nocapture`, and `cargo test -p mir2-simulation item -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure D.1 seal already-sealed validation pass: current `item.seal` now rejects active already-sealed equipment, preserves the existing expiry, and only emits `ItemSealChanged` on the first successful active seal. Full seal-source item validation and reseal-delay metadata remain queued. `cargo fmt --check`, `cargo test -p mir2-simulation stage5_item_seal -- --test-threads=1 --nocapture`, and `cargo test -p mir2-simulation item -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure D.1 BenedictionOil branch pass: current BenedictionOil now mirrors Crystal's three true outcomes: Luck gain emits `RefreshItem`, curse decrements weapon Luck and emits `RefreshItem`, and no-effect consumes the oil without a refresh. `cargo fmt --check`, `cargo test -p mir2-simulation benediction_oil -- --test-threads=1 --nocapture`, and `cargo test -p mir2-simulation item -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure D.1 seal source validation pass: current `item.seal` now supports optional source-key validation and consumption. Manifest-backed source items must match Crystal `ItemType.Gem` with `Shape == 8`, source durability drives the seal duration, missing/wrong sources fail without mutation, and the old no-source Stage 5 path remains compatible. `cargo fmt --check`, `cargo test -p mir2-simulation stage5_item_seal -- --test-threads=1 --nocapture`, and `cargo test -p mir2-simulation item -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure D.1 socket source validation pass: current `item.addSocket` now supports optional source-key validation and consumption. Manifest-backed source items must match Crystal `ItemType.Gem` with `Shape == 7`, target compatibility follows Crystal `ValidGemForItem` unique flags, missing/wrong sources fail without mutation, and the old no-source Stage 5 path remains compatible. `cargo fmt --check`, `cargo test -p mir2-simulation stage5_item_add_socket -- --test-threads=1 --nocapture`, and `cargo test -p mir2-simulation item -- --test-threads=1 --nocapture` passed.
- 2026-04-22, 100% closure D.1 seal reseal-delay metadata pass: current `item.seal` now stores Crystal `SealedInfo.NextSealDate` as `ExpiryDate + Settings.ItemSealDelay`, exposes it through the Crystal `UserItem.SealedInfo` payload, rejects reseal after expiry until the next-seal date has elapsed, and preserves the metadata through JSON save/reload while old saves default safely. `cargo fmt --check`, `cargo test -p mir2-simulation stage5_item_seal -- --test-threads=1 --nocapture`, and `cargo test -p mir2-simulation item -- --test-threads=1 --nocapture` passed.
- 2026-04-23, 100% closure D.1 bounded `CombineItem` packet parity pass: added Crystal `CombineItem` client/server ids, codec, trace names, gateway JSON conversion, and inventory-grid runtime dispatch into the current shape-7 socket-growth and shape-8 seal semantics. The round intentionally stops short of full Crystal target-type, hero-inventory, and other combine-branch parity because the current imported manifest does not yet support a meaningful strict target-type success path. `cargo +1.89.0 fmt --check`, `cargo +1.89.0 test -p mir2-protocol item_and_combat_client_packets_use_crystal_payloads -- --nocapture`, `cargo +1.89.0 test -p mir2-protocol item_action_ack_server_packets_use_crystal_ids -- --nocapture`, `cargo +1.89.0 test -p mir2-gateway combine_item_server_event_exposes_crystal_payload_fields -- --nocapture`, `cargo +1.89.0 test -p mir2-simulation combine_item_packet -- --test-threads=1 --nocapture`, `cargo +1.89.0 test -p mir2-simulation item -- --test-threads=1 --nocapture`, and `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` passed.
- 2026-04-23, 100% closure D.1 bounded `CombineItem` shape-3/4 gem/orb upgrade pass: added Crystal `ItemUpgraded` / ID 216 protocol/gateway/runtime support, extended inventory-grid `CombineItem` handling to the current shape-3/4 gem/orb upgrade semantics, persisted `gem_count` through runtime/equipment/inventory `UserItem` round-trips, and covered success, max-added-stat rejection, invalid combination rejection, and destroy-on-failure branches. The round intentionally remains bounded to inventory-grid handling; hero inventory, broader target-type gating, rental `DontUpgrade`, player `GemRatePercent`, and belt/id-collision cleanup remain queued. `cargo +1.89.0 fmt --check`, `cargo +1.89.0 test -p mir2-protocol item_slot_seal_and_upgrade_server_packets_use_crystal_ids -- --nocapture`, `cargo +1.89.0 test -p mir2-gateway item_slot_and_seal_server_events_expose_crystal_payload_fields -- --nocapture`, `cargo +1.89.0 test -p mir2-simulation combine_item_packet -- --test-threads=1 --nocapture`, `cargo +1.89.0 test -p mir2-simulation storage -- --test-threads=1 --nocapture`, `cargo +1.89.0 test -p mir2-simulation item -- --test-threads=1 --nocapture`, and `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` passed with 465 tests.
- 2026-04-23, 100% closure D.1 bounded `CombineItem` target-type gate pass: aligned packet `CombineItem` with Crystal's shared top-level target `ItemType` check, so shape-7 socket, shape-8 seal, and shape-3/4 upgrade attempts now ack-fail immediately when the target is outside item types `1..=11`. This removes the prior non-Crystal behavior where non-equipment packet targets could fall into `InvalidCombination` or seal mutation paths. The round remains bounded: hero inventory, rental `DontUpgrade`, player `GemRatePercent`, and belt/id-collision cleanup remain queued. `cargo +1.89.0 fmt --check`, `cargo +1.89.0 test -p mir2-simulation combine_item_packet -- --test-threads=1 --nocapture`, `cargo +1.89.0 test -p mir2-simulation storage -- --test-threads=1 --nocapture`, `cargo +1.89.0 test -p mir2-simulation item -- --test-threads=1 --nocapture`, and `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` passed with 466 tests.
- 2026-04-23, 100% closure D.1 bounded `CombineItem` repair-hammer/sewing pass: aligned packet `CombineItem` with Crystal source shapes `1/2/5/6`, so hammer-vs-sewing target-family mismatches and `DontRepair` fail ack-only, full-durability targets emit `ItemNoRepairNeeded`, and successful repair-combine mutates durability, consumes the source, and emits `ItemRepaired` before the success ack. The round remains bounded: hero inventory, rental `DontUpgrade`, player `GemRatePercent`, and belt/id-collision cleanup remain queued. `cargo +1.89.0 fmt --check`, `cargo +1.89.0 test -p mir2-simulation combine_item_packet -- --test-threads=1 --nocapture`, `cargo +1.89.0 test -p mir2-simulation storage -- --test-threads=1 --nocapture`, `cargo +1.89.0 test -p mir2-simulation item -- --test-threads=1 --nocapture`, and `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` passed with 469 tests.
- 2026-04-23, 100% closure D.1 bounded rental binding flag pass: runtime item/equipment state now preserves rental `BindingFlags` and emits them through `UserItem.RentalInformation`; `StoreItem` rejects rental `DontStore`, and current socket/upgrade `CombineItem` branches reject rental `DontUpgrade` ack-only while preserving source/target state. The round intentionally remains bounded to the audited Crystal storage, socket, and upgrade paths; hero inventory, player `GemRatePercent`, belt/id-collision cleanup, and other gem-family branches remain queued. `cargo +1.89.0 fmt --check`, `cargo +1.89.0 test -p mir2-simulation combine_item_packet -- --test-threads=1 --nocapture`, `cargo +1.89.0 test -p mir2-simulation storage -- --test-threads=1 --nocapture`, `cargo +1.89.0 test -p mir2-simulation item -- --test-threads=1 --nocapture`, and `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` passed with 472 tests.
- 2026-04-23, 100% closure D.1 bounded `CombineItem` player `GemRatePercent` pass: current inventory-grid shape-3/4 upgrade success chance now adds equipment-backed player `GemRatePercent` from non-broken equipped item stats, matching Crystal's `Stats[Stat.GemRatePercent]` formula hook. The round remains bounded: hero inventory, belt/id-collision cleanup, and other gem-family branches remain queued. `cargo +1.89.0 fmt --check`, `cargo +1.89.0 test -p mir2-simulation combine_item_packet_upgrade_branch_applies_player_gem_rate_percent_bonus -- --test-threads=1 --nocapture`, `cargo +1.89.0 test -p mir2-simulation combine_item_packet -- --test-threads=1 --nocapture`, `cargo +1.89.0 test -p mir2-simulation storage -- --test-threads=1 --nocapture`, `cargo +1.89.0 test -p mir2-simulation item -- --test-threads=1 --nocapture`, and `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` passed with 473 tests.
- 2026-04-23, 100% closure D.1 bounded current inventory unique-id cleanup pass: Crystal source confirmed `CombineItem`, `SplitItem`, `DeleteItem`, `DropItem`, `SellItem`, and `RepairItem` all resolve current inventory items by `UserItem.UniqueID`; runtime now carries compatible `ItemState.unique_id` state, resolves those current bag-item paths by unique id instead of slot aliasing, assigns distinct fallback ids to `Bag1` / `Bag2` same-slot items, and gives split-stack clones a fresh destination id. The round remains bounded: hero inventory, move/merge unique-id parity, and other gem-family branches remain queued. `cargo +1.89.0 test --locked -p mir2-simulation unique_id -- --test-threads=1 --nocapture`, `cargo +1.89.0 fmt --check`, and `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` passed with 479 tests.
- 2026-04-23, 100% closure D.1 bounded current item packet unique-id cleanup pass: current packet `UseItem`, packet `EquipItem`, and `MergeItem` now resolve the exact referenced bag item by Crystal `UserItem.UniqueID` instead of duplicate-key fallback or slot aliases, so `Bag1` / `Bag2` duplicate-key items no longer consume/equip/merge the wrong candidate. `cargo +1.89.0 test --locked -p mir2-simulation unique_id -- --test-threads=1 --nocapture`, `cargo +1.89.0 test --locked -p mir2-simulation item -- --test-threads=1 --nocapture`, `cargo +1.89.0 fmt --check`, and `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` passed with 482 tests.
- 2026-04-23, 100% closure D.1 bounded `DeleteItem` hero-flag pass: Crystal source confirmed `MirConnection.DeleteItem` discards the packet `HeroInventory` flag and `PlayerObject.DeleteItem` still scans only player `Info.Inventory` by `UserItem.UniqueID`; runtime now mirrors that quirk by deleting matching player bag items even when `hero_inventory=true`, while missing hero/player ids remain ack-only with the normal Crystal-shaped `DeleteItem` response. The round remains intentionally bounded: full hero-inventory item handling, hero-grid `CombineItem`, and other gem-family branches stay queued. `cargo +1.89.0 test --locked -p mir2-simulation delete_item_packet -- --test-threads=1 --nocapture`, `cargo +1.89.0 test --locked -p mir2-simulation item -- --test-threads=1 --nocapture`, `cargo +1.89.0 fmt --check`, and `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` passed with 484 tests.
- 2026-04-23, 100% closure D.1 bounded hero-inventory guard pass: Crystal source confirmed `DropItem(hero_inventory=true)` and `CombineItem(grid=HeroInventory)` both fail through hero inventory only and, when no current hero inventory is available, return the failed ack without mutating matching player bag items. Runtime already matched that bounded behavior, and focused regressions now lock both guards so unavailable hero inventory cannot consume or mutate player inventory by accident. `cargo +1.89.0 test --locked -p mir2-simulation hero_inventory -- --test-threads=1 --nocapture`, `cargo +1.89.0 test --locked -p mir2-simulation item -- --test-threads=1 --nocapture`, `cargo +1.89.0 fmt --check`, and `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` passed with 486 tests.
- 2026-04-23, 100% closure D.1 bounded rental `DropItem` bind pass: Crystal source confirmed `PlayerObject.DropItem` rejects rental `RentalInformation.BindingFlags.DontDrop` the same way it rejects base `DontDrop`; runtime now reuses the shared Crystal-or-rental bind helper so current `DropItem` returns the failed ack, preserves inventory state, preserves rental metadata, and spawns no ground drop for rental `DontDrop` items. `cargo +1.89.0 fmt --check`, `cargo +1.89.0 test --locked -p mir2-simulation drop_item_packet -- --test-threads=1 --nocapture`, `cargo +1.89.0 test --locked -p mir2-simulation item -- --test-threads=1 --nocapture`, and `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` passed with 487 tests.
- 2026-04-25, 100% closure D.1 current equipment metadata pass: current equipment/item runtime state now preserves Crystal `NeedIdentify` and `SoulBoundId` through `UserItem` round-trips, successful equip/use-equip identifies the item before the visible refresh, and equipping an item bound to another character now fails without mutation. `cargo +1.89.0 test --locked -p mir2-simulation equip_item_packet -- --test-threads=1 --nocapture`, `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_equipping_need_identify_item_emits_refresh_item -- --test-threads=1 --nocapture`, `cargo +1.89.0 test --locked -p mir2-simulation item -- --test-threads=1 --nocapture`, `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture`, and later `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` revalidation passed with 599 tests.
- 2026-04-25, 100% closure D.1 dynamic current-data `UseItem` pass: manifest-backed `crystal-item-*` consumables now route through Crystal template stats instead of starter-only local tables, covering `SunPotion` HP/MP restore, same-key duration buff stacking for current consumables like `ImpactDrug`, multi-buff items like `Apple`, current-data `TownTeleport`, and current-data `BenedictionOil` / `RepairOil` / `WarGodOil`. The bounded `WarGodOil` path currently falls back on the template name because the generated manifest still reports `shape = 0`. `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_dynamic_crystal -- --test-threads=1 --nocapture`, `cargo +1.89.0 test --locked -p mir2-simulation use_item -- --test-threads=1 --nocapture`, `cargo +1.89.0 test --locked -p mir2-simulation item -- --test-threads=1 --nocapture`, `cargo +1.89.0 test --locked -p mir2-simulation equip_item_packet -- --test-threads=1 --nocapture`, `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture`, and `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` passed with 599 tests.
- 2026-05-21, production Crystal map respawn screenshot pass: current-map StartGame visible respawns now keep representative low-density Crystal map spawns while distributing them over nearby walkable cells instead of stacking all representatives at one origin. Production gateway release `20260521T0830Z-spreadrep` was deployed and verified with live screenshots/states for BichonProvince, WoomyonWoods(S), NaturalCave, DeadMineEntrance, InsectCave_2F, and ZumaMaze under `docs/generated/player-qa/live-map-monsters/`; the state captures report `network404=0`, Monster meta `503=0`, and Monster PNG failed count `0`. Verification passed: `cargo +1.89.0 fmt --check --package mir2-simulation --package mir2-gateway`, `cargo +1.89.0 check -p mir2-simulation`, `cargo +1.89.0 check -p mir2-gateway`, and `cargo +1.89.0 test -p mir2-simulation start_game_visible_respawns_spread_representative_crystal_map_spawns -- --nocapture`.

## Working Rules

- Crystal source and exported Crystal data are the behavior reference.
- Do not mark a task complete based only on code inspection.
- Do not delete failing tests to claim progress.
- Prefer adding focused regression coverage before changing shared runtime behavior.
- Keep generated data changes tied to generator changes whenever possible.
- Keep UI debug controls useful but isolated from normal gameplay where needed.
- Update this document after each completed task, not only at the end of a stage.
- Update `docs/BACKEND-1TO1-PROGRESS.md` and `docs/CRYSTAL-SERVER-PARITY.md` when backend parity meaningfully changes.

## 2026-07-23 Deterministic Visual-Parity Milestone

The camera/HUD/light/effect normalization slice is complete. Final Dawn r33
reduces full/world changed pixels from r29 `36.4%/40.2%` to `24.2%/26.1%` and
world MAE from `18.845` to `11.987`. Final Night r32 remains at
`12.5%/12.6%`, matching r26 and proving the Dawn/Evening compositor correction
does not regress the Night path. Both pairs are same-account, same-coordinate,
fixed-light, overlay-free, and zero-error.

The milestone also closes the capture handshake race, stale-socket event race,
secret redaction, cursor parking verification, Crystal minimap quantization,
AI 6 radar propagation, and HUD experience/HP clipping. Movement/map
transaction unit gates and WebGPU/WebGL runtime smokes remain green. The next
visual milestone is intentionally narrower: GDI text rasterization,
deterministic chat content, roaming entity/animation phase, and final human
visual/feel acceptance.
