# Shared HP image retry repair

2026-10-01. The previous frozen 16c767 build rendered the shared full-HP image on
WebGPU/shared GL2, retained ordinary UI/movement and normal public saves, and
passed a separately launched DPR2 closed-HUD/More check. Native Welcome remained
open and physical input/trusted image/normal Logout are unaccepted. Those old
artifacts remain at shared-hp-orb and are not evidence for this new source.

Actual Rust-only HP asset interruption correctly retains the DOM image, but
caused 3,682 aborted current-WASM fetches in about 64 seconds. The PostUpdate
observer calls AssetServer::load every frame; Bevy reissues failed requests.
Snapshot changes also load through the painter. This is a product failure to
resolve before this slice closes; no synthetic HP or Gateway command is needed.

One Sol/high writer owns only:
- apps/game-client/client-bevy/src/portable_hp_orb_ui.rs
- apps/game-client/client-bevy/src/crystal_ui/hud_orb.rs

Root owns integration, builds, packaging and actual clients. A second Sol/high
reviewer is read-only; Luna/medium audits fixed evidence. Preserve all other
staged work, assets, protocol/host/Web files, previous artifacts and saves.

Keep a strong image handle and an attempt lease per host generation and fixed
orb path, at most the two paths per current generation. Same-generation changes
to HP, revision, slot or window visibility cannot retry a failed image. Returning
to a previously attempted path cannot rearm it. A new generation/new App may
attempt again. If a new generation overlaps an older in-flight request, adopt
successful completion; if it fails, permit only the new generation's one retry.
Do not add automatic timers/backoff, asset reload APIs, gameplay replay or a new
canvas/App/Window. Observers only inspect existing handles/load state/geometry.

Add explicit-handle painter entry points so portable code can paint without
requesting another asset. Keep native AssetServer wrapper APIs and geometry,
crop, class thresholds, typed input/focus and asset paths unchanged. Failed or
stale handles never acquire HP ownership. Zero HP and window/menu fallback
retain the previous source contract.

Verify with a real failing/counting asset reader/loader fixture over many frames,
revision/HP/slot/hidden-window changes, two-path revisits, new generations and
overlapping old Loading -> Failed/Loaded transitions; pure mirrored policy
booleans are insufficient. Run affected native HUD and portable tests plus
native/WASM checks. Root then independently reviews, creates a new immutable
runtime/Next/portable package identity within unchanged byte/360MiB budgets, and
repeats actual Rust-only failure, bounded request count, DOM visibility,
ordinary movement/Logout/save and normal reload recovery. Current 16c767
screenshots cannot satisfy the repaired source gate.

Physical phones, native physical controls/provenance, full HUD/window composition,
partial-HP live gameplay, shortcut parity, public delivery, performance and human
acceptance remain separate. The overall cross-platform goal stays active.
