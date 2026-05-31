# GM Commands & Ops Platform Expansion

Last updated: 2026-05-31

Scope: expanding the operations surface across four directions — the admin
command model, the in-game GM commands, the ops web UI, and live-world reach.
Complements `docs/ADMIN-OPERATIONS-ARCHITECTURE.md`.

## 1. Admin command set (server-side, audited) — landed

The audited command model already covered ~27 commands. This pass closed the
gaps versus `ADMIN-OPERATIONS-ARCHITECTURE.md`:

- **GrantExperience** — `UpdateCharacter` gained `experience` / `max_experience`
  fields plus a first-class `POST /admin/commands/grant-experience`.
- **AddSupportNote** — new low-risk command + `POST /admin/commands/add-support-note`.
  Notes append to a durable per-account ndjson trail
  (`<content_dir>/support-notes/<account>.ndjson`) with actor/reason/timestamp.
- **RollbackContentBundle** — new high-risk (approval-gated) command + route.
  Archives the published bundle JSON aside (`<bundle>.rolledback-<ts>.json`) and
  removes the live file so a world/zone reload stops serving it; recoverable.

All flow through the existing pipeline: required permission, payload validation,
approval gating (rollback requires peer approval; support note does not),
`command_type` mapping, executor, and the audited command/outbox records.
Covered by the crystal-console executor test (experience mutation, support-note
persistence, bundle rollback). 33 admin-api tests pass.

## 2. In-game GM `@` commands — first batch landed

Crystal's `@`-prefixed GM chat commands had no equivalent. Added:

- `PlayerPermissionResource.gm_level` + `is_gm()`; `AccountRecord.gm_level`
  (serde-default 0, never granted implicitly). StartGame sources the live GM
  rank from the authoritative account record.
- `runtime/gm_commands.rs`: an `@`-command dispatcher gated strictly on
  `is_gm()`. A non-GM typing `@foo` falls through to normal chat, so command
  existence never leaks. First batch: `@HELP`, `@WHERE`, `@LEVEL <n>`,
  `@GOLD <n>`, `@HEAL`, `@MOVE <x> <y>`, each acknowledged with a hint line + the
  correct client-refresh packet (`LevelChanged` / `GainedGold` / `ObjectHealth`
  / `UserLocation`).
- Wired into `handle_chat_packet` ahead of the chat pipeline / spam guard.

Tested: `@LEVEL` emits `LevelChanged` + sets level; `@GOLD`/`@MOVE` mutate
runtime; a non-GM `@` message stays normal chat.

To grant GM in-game: set `accounts.<id>.gm_level > 0` in the account store
(JSON or Postgres `raw_json`). Extending the batch (`@MAKE`, `@RECALL`,
`@KILL`, `@SUPERMAN`, etc.) is additive in `gm_commands.rs`.

## 3. Ops web (admin-web) real data — economy slice landed

The Economy page now surfaces the normalized-projection endpoint
`GET /admin/read/economy/aggregate` (PR #17) in a "Live aggregate (normalized
SQL)" panel: real gold supply / avg / max, active-auction count + escrow value,
unclaimed-mail count + escrow gold, top gold holders, gold-by-map — all from
indexed SQL, not deserialized blobs. Renders only when the projection is
Postgres-backed. Typechecks clean.

Remaining admin-web mock surfaces (gm-tools, accounts, players/[id], market,
namelists, console) can be wired to `/admin/read/mail|auctions|items` and the
new command routes incrementally using the same pattern.

## 4. Live-world GM reach — constraint documented (NOT a quick add)

Acting on **online** players (live teleport / grant / message / broadcast) is
**not** a small change today, and this is recorded honestly rather than
half-implemented:

- The gateway holds each player's live world in a per-WebSocket-task
  `SimulationSession`. There is **no server-push channel** into those task loops
  — a session only emits packets in response to its own client input/ticks
  (`web.rs`). The `session_cache` is a routing/presence cache, not a command bus.
- The one exception that already reaches online players is **mail/gold via the
  account store**: admin-delivered Stage 5 mail is merged by the online session
  before its next snapshot/save (`refresh_active_external_mail`). So
  `SendSystemMail` (incl. gold/items) is the supported live-reaching GM path
  today, and `KickPlayer` works by evicting the session-cache route.
- Arbitrary live mutation (teleport an online player, live stat edits) needs a
  per-session command channel from the gateway admin endpoint to the owning WS
  task — the same shared-command-bus / single-writer-zone work tracked in
  `WORLD-AUTHORITY-STATUS.md` (the `ZoneOwner` RPC effort). Until then, those
  admin commands act on the authoritative save and apply on next login.

Recommended next step for Direction 2: add an mpsc command queue per online
session (keyed in the session cache), drained on each gateway tick, that injects
a `WorldCommand` into the session runtime. That unlocks live teleport/grant/
message/broadcast on top of the executors from section 1.
