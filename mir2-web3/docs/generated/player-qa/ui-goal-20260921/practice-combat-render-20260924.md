# Class practice, Wooma guidance and combat candidate — 2026-09-24

The reported N21 card showed only `Complete your class practice 0/1`.
The shared native UI now reads the same V2 requirements as the server and
names the required skills/actions for the player's class in the task card.
Quest details put the full instructions first, including separate HalfMoon
and Thrusting activation for Warriors. All eight class-practice quests and
all three configured classes are covered; completion still comes only from
authoritative objective counters. Later quest details also identify the
ordinary Bichon potion shop (Samuel, 324,291) and scroll shop (Bull, 374,296).
No purchases, item grants or player-save edits were made.

N19–N21 hunting previously used the imported D022 whole-map spawn at
(250,250), spread 250, ignoring the authored V2 training footholds. This
could report arrival while 141 tiles from the displayed center. Navigation
and map markers now use the matching quest's three training points and their
bounded radius. N21's points are (280,340), (300,330), (300,385).
The guide retains hunting destinations when kills are complete but class
practice is pending. Arrival feedback expires after leaving the area,
changing map or changing the primary quest.

To address sparse task monsters, each V2 training point now has three actors
within spread 3: nine Dung, nine WoomaSoldier and nine WoomaFighter across the
nine separated points. The three imported D022 groups retain the existing
V2 cap of one roaming actor per group. Original cadence spawning is unchanged.
Training Wooma retain 120 HP and ordinary respawn timing. Static entry routes
and room for three actors at every point pass. This is an explicit newcomer
content adjustment, not original Crystal population parity; fresh three-class
survival and playthrough acceptance remain open.

## Weapon damage and experience

Original `Crystal/Server/MirObjects/HumanObject.cs` keeps the adjacent primary
hit at base physical damage (3028, 3163–3173); its Thrusting second cell applies
the spell multiplier and Agility-only defense (3209–3227).
`Envir.cs:319` gives 0.25 + 0.25 × level and `MagicInfo.cs:182` truncates.
The shared Zone had incorrectly multiplied the already armor-reduced primary
hit by 0.25, explaining ordinary hits above 20 versus Thrusting hits of 4–6.

Shared Zone now distinguishes primary and secondary cells for monster,
materialized monster, player and mixed targets. With fixed DC 40 and AC 16,
the adjacent hit is 24; the second cell is 10/20/30/40 at levels 0–3 and
bypasses AC. Off-axis selections fail and the selected actor is not hit twice.
HalfMoon's primary and three additional cells also use the original shape and
damage distinction. Gateway commits weapon-skill experience only after an
accepted swing, using Zone's legal primary-target snapshot before dispatch;
a second-cell-only hit, friendly occupant or rejected retry cannot grant it.
One-shot skill mana/toggle consumption occurs before skill progression.

The existing impact schedule is retained. AI49 Repulsion and the standalone
personal-world attack compatibility path are outside this correction. This
does not establish complete combat parity.

## Running and attacking

The old live trace's 81 attack requests all had zero pending moves; it did not
prove stale movement ACKs caused this screenshot. A deterministic presentation
case did reproduce attack poses over an active motion window when Bevy's
animation clock and the native motion clock diverged. Gateway projection,
local motion creation and entity animation now use the same native clock.
Attack issuance also waits for the visible motion window to end, while still
allowing an overdue ACK after that end. Camera position, collision validation
and authoritative movement remain unchanged. A bounded
`attackMotionOverlapRendered` event records any actual overlap that remains.

## D021 performance evidence and limits

The old live D021 trace had 1,099 frames in complete health windows, averaging
36.64 ms, with dense positions around 45–50 ms. D022 averaged about 13 ms.
Map/entity synchronization was below 1.1 ms, so this was not explained by those
two measured stages alone.

- Repeated resource-root discovery was a measured native hot path. Actor PNG
  fallback/highlights and effect masks could diagnose the entire bundle for
  each frame lookup. A one-entry successful-root cache, keyed by configured
  root and working directory, now revalidates once per second. Startup still
  validates the full bundle; missing files and later R2 arrivals are not cached.
- An old/new/new/old test using the actual installed directory and 120 frames
  per batch measured 16 lookups/frame at 6.66–6.86 → 0.235 ms, 32 at
  14.49–14.53 → 0.481–0.526 ms and 64 at 33.62–34.30 → 1.42–2.13 ms.
  This isolates lookup cost; old telemetry does not count D021's exact lookups
  per frame, so these numbers are not a measured whole-map FPS improvement.
- Map animation visibility now changes only when a phase changes. Immutable
  additive materials share texture/opacity/UV bindings with reference-counted
  aliases, avoiding repeated asset mutation and preserving independent fades.
- Offscreen D021 (58,57) replay used 712 actual tile-phase entities, including
  192 additive phases. Old/new GPU pixels match exactly and materials fall
  from 192 to 40. Draw batches remain 134. Timing had ordering bias, so this
  test proves image equivalence and resource reduction, not a speedup.

The candidate adds bounded `mapAnimationVisibility` timing. D021 gameplay
stutter must be compared in the newly packaged client before being marked
resolved.

## Validation and handoff

- Shared UI: 1,083/1,083.
- V2 Node route/content tests: 64/64, including nine reachable training points.
- Shared Zone existing integration: 209/209.
- New melee formula/primary legality integration: 10/10.
- Training spawn integration: 1/1.
- Gateway progression integration: 2/2.
- Renderer: 275/275, plus explicit D021 offscreen GPU comparison.
- Asset-root focused regression: 8/8, plus the explicit offline lookup benchmark.
- Final native Windows regression: 738/738, three explicit GPU/performance
  tests ignored in the normal run. Serial execution follows the repository's
  shared-state test policy; a preliminary four-thread run exposed an existing
  global-reset counter race, and its isolated and full serial reruns pass.
- Release client and Gateway compiled successfully offline.

The Gateway test initially requested a blocked two-cell stand position; Zone
corrected (336,273) to (335,272). Its fixture now uses a verified legal stand
position and still checks exact target ID, progression and rejected retries.
No production acceptance check was weakened.

The client and Gateway are copied and hash-verified in
`C:/numeron-legend-of-rebirth-20260924-practice-combat`. Client SHA-256:
`6D20DFE81BD21EEF59C8C0A2E6F19FC3BEEB01C3E3FED6848D3BA81D3E42D753`;
Gateway SHA-256:
`A79307FB7F7ED779771E5C6C49A3E47504196C59F2353D6B5C60B010FFA5D201`.
The launcher preserves the existing store and identity keys, shared assets,
fixed-daylight setting and movement/render diagnostics.

### Live handoff: September 24, 06:02 +08:00

After the user confirmed normal exit, the previous client was absent and the
old Gateway had no established connections. Source
`abee09b21d33a7e5a23f36609ee31f0da3246f83` is now deployed as client PID 55648
and Gateway PID 61900. Both binary hashes match the packaged manifest.
Settings were copied again after logout, preserving the latest user settings.

The original account store was backed up before switching to
`C:/mir2-ui-repair-20260921/player-save-backups/20260924-060127-726-practice-combat-accounts.json`.
The copy and live store SHA-256 matched:
`5B5EB20EF4892DAEE3BDA0F8ECCCE78C2C3649CDD341A4C535F272FB75E73C95`.
No character records were edited and no items or progression were granted.

The Gateway owns both local listeners (19900 and 19910); the responsive native
client has an established WebSocket connection and received its first snapshot.
Resource-index/map-pack discovery succeeded without a startup crash.
Movement and render traces use
`C:/mir2-ui-repair-20260921/render-live/20260924-060209-524-{movement,render}.jsonl`;
native soak metrics are also enabled. Gateway stderr is
`C:/mir2-ui-repair-20260921/render-live/20260924-060128-531-gateway.stderr.log`.

This verifies launch and connection only. Physical acceptance, D021 whole-frame
improvement, three-class survival and whole-game stability remain open;
the package manifest retains false acceptance flags. The client is open for
the user's gameplay check.

Evidence logs live in `C:/mir2-ui-repair-20260921`: shared/Node test logs,
`thrusting-gateway-fixture-tests.log`, `thrusting-training-spawns-tests.log`,
and release build logs; render evidence is under `render-live/20260924-*`,
including `wooma-render-audit.json`, `d021-gpu-reverse-ab.log` and
`asset-root-ab.log`.
