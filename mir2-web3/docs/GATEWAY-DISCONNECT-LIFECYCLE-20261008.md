# Gateway disconnect lifecycle — released fix, 2026-10-09

The native character-select error `character is already online or route lease
is unavailable` can persist when transport closure waits behind a full input
queue or a shared socket writer. This Candidate makes the terminal signal
independent of that queue, with a single10-second write budget across acquiring
the lock, readiness and flushing. The first terminal reason wins, including
closure during the final work poll. Reader lifetime also signals on early exit.

Cancellation waits for the actual admitted blocking movement closure to finish;
dropping its async join is insufficient. Teardown advances the movement epoch
and cancels pending Zone movement under its existing fence, preserving already
completed authority. It then follows the original checkpoint/source-save,
authenticated journal, resume-retention and lease-release decisions. It does not
evict another session, fabricate logout, or treat a DB/journal failure as saved.

## Frozen save and recovery

The unchanged7fea baseline reproduced seven failures in the original selected
regressions. A real recovery read previously used ordinary StartGame and could
run DefaultNPC/login writes, change the revision, or revive a dead character.
The new server-only, one-use recovery binding performs fresh account/eligibility
checks and reads the durable row. Ordinary packet/passkey/rebinding paths revoke
the capability. Missing identities remain conflicts; unavailable authority/access
is deferred with the authenticated journal retained.

Prepared teardown now captures a durable skill clock separately from ordinary
same-session rollback. Frozen saves preserve that clock and exact buff/refine
timer/item bytes through the original identity checks, source transaction,
revision CAS and LastLogoutDate update. Recovery prepares World authority from
the authenticated journal but commits the frozen row, rather than resnapshotting
elapsed timers. Verification checks the actual stored record. Shared saves require
a verified existing fence; a poisoned Zone lock refuses permission to save.

New journals compare the captured skill clock bytes as well as character state.
Legacy journals without captured clocks retain the previous typed-skill-wrapper
normalization; their missing original tick domain cannot be reconstructed. Generic
remote/test runtime fallback retains its prior save behavior. These limitations
are not hidden by equality checks or broad schema/packet changes.

## Verification and current delivery state

- Gateway270/270: actual local WebSocket close with a stalled writer, saturated
  queue signaling, write/ready/flush deadlines, late work-poll termination, real
  blocking movement drain, stale-epoch rejection, completed-position preservation,
  save/journal/resume/authentication and original ingress regressions.
- Recovery7/7: capability revocation/one-use identity, deferred authority,
  no-login-write/dead-character read, old tick100 with expired deadline99 or
  remaining deadline101, replay and ordinary cold login, clock alteration rejection.
- Running valid refine target UID and timed buff: primary save, journal replay,
  lost cleanup ACK, revision+1 and exact frozen bytes; no second source save.
- Gateway debug build and exact-source Linux release pass. Native rendering and gameplay acceptance remain open.

The immutable evidence receipt binds base7fea plus all changed/new code file
hashes, the unabridged output archive, source patch/new files and debug binary.
Failed attempts are retained: original baseline7, Windows file-lock/link failures,
the in-memory missing-account classification bug, and corrected test-only recovery
read/refine carrier field/index errors. No existing business assertions were
removed or replaced with weaker gameplay thresholds.

[Candidate receipt and full originals](generated/player-qa/gateway-disconnect-20261008/candidate-01/README.md).

Source536b4a59f3a8af43f6d4484238e2847573dfb106 is committed and pushed on
codex/gateway-disconnect-lifecycle-20261008. Exact-source [Linux CI37810114063](https://github.com/Zombieliu/mir2/actions/runs/37810114063)
passed both build-linux-x64 and siege-acceptance jobs, including the original
appearance, trusted recovery, classic source, PostgreSQL competing-writer/clock
and live protocol boundaries. The actual Gateway ELF is83430592 bytes with SHA256
755b02a845869551c79017ecb6392a1faee8007ebc117001443a294feb050dba.

The playtest Gateway changed froma41 to536 only after all six capacity counters,
native TCP connections and spectator viewers were zero. Private DB/state/config
backups precede normal old/Candidate stops and cold start. The source-pinned
running binary, original realm PID/release, environment, base unit, independently
changed spectator drop-in and resource limits were checked again afterward.
The first stale spectator-config preflight refused the switch and mutated no
service. Current spectator settings were preserved in the accepted second preflight.

Public WSS ordinary44 and final native-resume18 checks pass, with14 normal logout
acknowledgements. Three real socket terminations use native resume rather than
fresh StartGame; each verifies rotation, used-ticket refusal, one actor copy,
exact acknowledged position/direction/gold/full item fields, resumed live authority,
normal logout and fresh saved login. TLS/authentication stay enabled; existing
owned actors use no admin/QA grants, registrations or new character creation.
The first resume run completed15 checks before an unspecified WSS connection
failure; it is retained and excluded from the62 accepted checks. Retry02 changes
only its output directory and error diagnostics, preserving all business checks.
The intervening sandbox DNS failure ran no public tests. Its cause does not prove
the cause of the earlier WSS failure. No full saturated-queue public/native render
acceptance is claimed by this protocol verification.

The complete sanitized trace export also redacts historical credentialId metadata
without removing any events; original local file hashes and replacement paths are
recorded. Private ledger/config/database contents are never copied into Git.
[Release receipts and unabridged CI/sanitized traces](generated/player-qa/gateway-disconnect-20261008/release-01/README.md).

Windows R22/source7fea and signed feed17 were published separately and remain the
client pair. This is a compatible server-only fix; no reinstall is required.
The expired12-hour heartbeat is now PAUSED because its instructed deadline passed,
not because the broader Goal is complete. [Actual delivery and remaining gates](CLASSIC-12H-DELIVERY-20261007.md).

The original12-hour checkpoint was missed. The P1-P7 Goal remains active and is
not100%: dense-monster native escape, natural late-map/Boss/loot, shared NPC event
outbox/scheduling, several natural multiplayer lifecycle and human frontend gates
still require evidence. This transport/save fix does not close those gates.
