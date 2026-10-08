# Gateway disconnect lifecycle Candidate — 2026-10-08

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
- Gateway debug build passes. This is not the Linux release binary or native QA.

The immutable evidence receipt binds base7fea plus all changed/new code file
hashes, the unabridged output archive, source patch/new files and debug binary.
Failed attempts are retained: original baseline7, Windows file-lock/link failures,
the in-memory missing-account classification bug, and corrected test-only recovery
read/refine carrier field/index errors. No existing business assertions were
removed or replaced with weaker gameplay thresholds.

[Candidate receipt and full originals](generated/player-qa/gateway-disconnect-20261008/candidate-01/README.md).

Public R22/source7fea, signed feed17 and Gatewaya41 were published by a separate
delivery slice. The last direct health read observed no sessions/route leases.
This Candidate has not yet changed that server or the installed client. Next:
commit/push, exact-source Linux release/security/PostgreSQL gates, strict drained
test-server rollout and normal public reconnect/save/relogin verification.

The original12-hour checkpoint was missed. The P1-P7 Goal remains active and is
not100%: dense-monster native escape, natural late-map/Boss/loot, shared NPC event
outbox/scheduling, several natural multiplayer lifecycle and human frontend gates
still require evidence. This transport/save fix does not close those gates.
