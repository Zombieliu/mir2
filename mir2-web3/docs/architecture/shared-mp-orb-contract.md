# Shared MP image contract

2026-10-01. The overall client convergence goal remains active. The HP retry
5a7c boundary is sealed at 1,232 matching indexed rows before this lease.
MP is a new image-only slice, not whole HUD or class/mobile acceptance.

## Behavior and source

Reuse native hud_orb.rs unchanged: split original-ui/Prguse/4.png, MP source
left51/width50, full image104x80, floor/clamp bottom-anchored height.
Warrior class (case-normalized) below26 has no MP picture. Wizard/Taoist and
Warrior26+ may render MP. Do not introduce another painter, canvas, App,
Window, input/intent owner, timer, gameplay packet or authoritative state.

The existing portable orb root/camera is shared. Add a typed MP image,
current model stamp and independent observation while retaining FocusPolicy
Pass. Both sides share the repaired strong path/generation image lease.
A hidden low-level Warrior must not request4.png merely for MP. Same-generation
revisions, slot/window changes or path revisits cannot rearm a failed image;
a new generation or App follows the existing bounded retry rule. Observers
never call AssetServer.load. HP remains independently available if MP is
unknown, unsupported, failed or has invalid geometry; Quest readiness remains
independent of orb readiness.

## Optional protocol and current-data proof

Add optional player.mp and player.maxMp to the existing Quest snapshot.
Rust must deserialize absent fields into Option values and retain both-value
knownness before into_models defaults; known0 with maxMp>0 is valid, missing,
negative MP or nonpositive/missing maxMp cannot own MP. Populate UiReadModel
from authoritative supplied values but do not infer knownness from default0.
Use the existing optional hpOrbSlot as the full104x80 origin for both sides.

Add optional status.mpOrb with fields:
supported, ready, generation, revision, mp, maxMp, hpOnly, slot, image,
source, destination, layout. hpOnly is false when MP is ready. Unready values
are null and ready is false; unsupported/lean omits mpOrb. Keep ABI1 and all
existing optional status/snapshot/intent behavior.

Supported sharedGL2/WebGPU hosts must publish mpOrb.supported=true before
any valid MP values or slot are received, so capability negotiation cannot
deadlock on its own fields or fonts. The Web hook probes this flag every
existing tick. Only supported runtimes receive nested player.mp/maxMp; for
an old or lean runtime remove those keys entirely, including supplied null.
Send hpOrbSlot only if HP or MP support is known. Hidden/catch/cleanup snapshots
must reuse the sanitized DTO, so old deny_unknown_fields never sees MP fields.

Ownership requires the latest acknowledged generation/revision, live player
MP/maxMP/class/level, knownness, shared slot, image, measured current typed tree,
loaded same-lease handle, target camera/window and logical layout. Recheck live
view values before hiding DOM. Valid known0 requires a loaded image and measured
zero-height destination, not an invented ready state. Rust measures ComputedNode
and UiGlobalTransform with inverse scale. Web verifies expected source x51/w50,
bottom anchoring and bounds against the same full logical stage. No stale ready
after logout, renderer stall, resize/geometry loss, model change or modal handoff.

## Web fallback and composition

Reuse the HP child image's stable full104x80 origin; the clipped MP image's
moving bounding box is not a slot. Preserve existing DOM MP geometry for this
lease and explicitly report any visual discrepancy rather than mixing a
fallback rewrite into adoption. Page and Shell retain immediate closed-window
gates for Bag/Diary/NPC/Tutorial/Menu/More and other current modal surfaces.

Use independent HP/MP readiness and owner attributes. Hide only a side's DOM
fill once its shared measured side is current. The other side, labels, counts,
caps, buttons/belt/world input and old runtime remain usable. Cover HP-only,
MP-only, both and neither owned CSS compositions; nonshared content remains
above the UI canvas. MP may make the existing sharedGL2 primary or WebGPU
secondary UI canvas visible, but pointerEvents/data-ui-interactive still depend
only on the existing Bag/Quest input owner. No new gameplay capture.

## Write leases and coordination

Root owns this contract, QA baseline/freeze, task queue/docs, integration,
canonical builds/packages, actual clients, credentials and stores.

Sol/high Rust worker /root/bag_ready_diagnostics owns only:
- apps/game-client/client-bevy/src/portable_hp_orb_ui.rs
- apps/game-client/runtime/src/quest_ui_host.rs

Sol/high Web worker /root/touch_bag_web owns only:
- apps/web/lib/bevy-quest-ui.ts
- apps/web/lib/bevy-hp-orb.ts
- apps/web/lib/use-bevy-quest-ui.ts
- apps/web/app/page.tsx
- apps/web/app/original-client-shell.tsx
- apps/web/app/components/original-client-shell-types.ts
- apps/web/app/globals.css
- apps/web/scripts/test-bevy-hp-orb-ui.mjs
- apps/web/scripts/test-bevy-quest-ui.mjs

Tests within leased Rust files are allowed. Native hud_orb.rs, Web overlays,
other product files, asset/ABI/intent/server code and account saves are read-only.
No source expansion without reporting a concrete blocker to root. Root does not
edit a worker's leased files while that lease runs. Luna/medium audits fixed
evidence; source review must be read-only and separately recorded. Preserve
unrelated staged/working changes and all rejected/older evidence.

## Meaningful verification

Rust: retain HP real counting reader/PNG loader regressions; add MP class
threshold, full/partial/known0 vs unknown, independent model/image/tree gates,
shared4.png single request, no4.png for hidden Warrior, same/new generation
failure/overlap/recovery and stale-layout/logout behavior. Run portable orb,
native HUD, runtime host and native/three-WASM checks using an isolated target.

Web: capability-safe nested DTO keys, old/null/lean compatibility, expected MP
geometry and current/live model rejection, independent owners/labels/input,
modal/More and resize recovery. Run the existing relevant Node suites and
TypeScript checks. Build current canonical runtime/Next only after source freeze;
retain unchanged JS/raw/gzip/package limits and a clean portable copy.

Actual QA: use isolated ordinary Wizard or Taoist credentials/character via
normal registration/create/login/StartGame; accepted Warrior save is untouched.
Require server-supplied maxMP>0, visible split HP+MP and current descriptor,
ordinary Bag/Diary/More/Menu transitions, resize and process-DPR1/2, sharedGL2,
functional GPU secondary, lean React and old-runtime negotiation, normal
Logout/save/relogin. Inject bounded asset transport faults only into exact current
Rust paths while DOM requests continue. No model/status/packet/admin/stat injection.
Ordinary lawful MP-spending/regen gameplay should be exercised if available;
otherwise partial-MP live gameplay remains unverified even if fixture math passes.

Record actual screenshots/ACKs/body hashes/public whitelist saves separately from
unit fixtures. Desktop touch/DPR emulation is not a phone or OS-native DPI test.
Native Welcome/physical inputs/trusted capture/normal Logout, full HUD/composition,
performance, public delivery, all caster journeys and human acceptance remain
open. No whole-game percentage or goal completion is inferred.
