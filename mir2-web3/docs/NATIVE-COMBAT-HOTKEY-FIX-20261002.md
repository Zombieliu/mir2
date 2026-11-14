# Native movement and hotkey repair, 2026-10-02

This isolated branch starts at published client source
`f3c30036e36441b8e306aaac9b5e8d35488d8f85`. It does not activate the paused
periodic quest or capacity goals. Shared gateway/Zone paths at that source
match the currently deployed gateway's affected code.

## Causes and resulting behavior

- The client batch drain retained movement in a separate latest slot and
  appended it after explicit combat, reordering `Run, Attack` into
  `Attack, Run`. Explicit melee, ranged, magic and facing intents now retire
  earlier movement. New movement after the action still survives. Lifecycle
  priority and correlated inventory/service transactions retain their rules.
- Zone combat admission rejected cooldown-blocked attacks before clearing
  pending movement. Physical combat now cancels earlier movement before
  admission; an owner transform receipt also retires the client's earlier
  prediction. Swing, cast, action and movement deadlines are not shortened.
  Spell preparations/toggles do not cancel movement or manufacture an attack.
- Belt keys sent a slot without the item's real unique ID. Keyboard digits
  and numpad 1-6 now use the same exact item intent helper as mouse clicks,
  preserving the shared item-use clock and input focus/capture rules.
- Persistent weapon modes were blocked by generic spell readiness and MP.
  Thrusting, HalfMoon, CrossHalfMoon and DoubleSlash now follow Crystal's
  independent flags and shared 1000 ms ToggleTime. Each can be switched off
  by a later key press. Enabling HalfMoon does not disable Thrusting.
  FlamingSword/TwinDrakeBlade retain their 500 ms preparation timer and arm
  requests; CounterAttack retains its own readiness. Ordinary Wizard/Taoist
  casts retain MP, target and cooldown validation.

## Completed automated verification

- Native input: **144 passed, 0 failed, 0 ignored**, including 16 new tests
  using raw keyboard messages, release/repress, authoritative flag ACKs,
  actor changes and failed sends. Three classes, every belt digit/numpad key,
  UID zero/large values, empty slots, input focus, Hero separation, skill
  readiness and independent weapon flags are covered.
- Native sender: **14 passed**, covering action barriers, ordering, owner
  reconciliation, lifecycle priority and protected correlated queues.
- Zone: **30 unique selected tests passed**, covering rejected/accepted
  melee/range/magic, movement locks, delayed materialization, fresh chasing
  movement, owner ACK identity, damage and preparation neutrality.
- Diff whitespace and the three new test modules' formatting checks passed.

The first broad input attempt passed 115 and failed 29 because the isolated
checkout lacked required generated asset manifests. With unchanged source
and process-local `MIR2_NATIVE_ASSET_ROOT` set to the complete approved R10
public assets, all 144 passed. The first logs are retained, not overwritten.
The interrupted cold Cargo build ran no tests; the final checks reused the
released r33 cache with Rust 1.95.0 and one build job.

External retained evidence:

- `C:/mir2-client-delivery-20261002/run-combat-intent/RUN-COMBAT-INTENT-CHECKPOINT.json`
- `C:/mir2-build/platform-windows-hotkeys-20261002/validation-receipt.json`
- `C:/mir2-build/platform-windows-hotkeys-20261002/input-test-r10-assets-results.txt`

## Acceptance boundaries

These are source/test results. Release building, matched gateway deployment
and ordinary native gameplay acceptance are separate. Existing owner
correction applies a finite 400 ms guard and may briefly choose another
chase step; these tests do not prove perceptual smoothness under live delay.

After matched rollout: run continuously, select an adjacent monster during
swing/cast cooldown and verify earlier run commands do not resume; then issue
new chase movement. With Warrior, toggle Thrusting on/off after the 1 s guard,
enable HalfMoon independently and verify the next applicable attack. With
each class, use belt 1-6/top row/numpad and verify the correct item count and
HP/MP or scroll result. Chat fields and true modal dialogs must retain focus;
ordinary nonmodal spell/bag panels must not swallow world input elsewhere.
