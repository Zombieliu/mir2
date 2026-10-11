# Classic Windows escape and action clock, 2026-10-11

Exact parent: `1e343def08670e0e3fecee7e00088ff23c5c5153`. This is a Windows
component Candidate. New Windows release/CDN/feed, server pairing and actual
native/human crowded-combat acceptance remain independent.

## Resulting behavior

Accepting an AttackTarget into the UI FIFO used to set movement's next send
deadline to the full next-hit request interval. That acceptance proves neither
socket send nor a successful attack. It now paces only the next hit. Pointer,
keyboard and target-pursuit movement read the existing actual AnimationWorld
active/queued combat and life actions. Attack1-4, ranged attacks, DashAttack,
Spell and death/revival barriers retain their real frame lengths. Struck alone
and normal locomotion add no new movement lock. Once the admitted animation
ends, the same held right button can escape without waiting for the next hit.
This does not add a guessed550/600ms post-enqueue acceptance timer.

An unchanged UserLocation can correct a rejected attack/control action or a
sender-discarded move; it does not prove an obstructed tile. Correction retains
the complete original400ms guard and real unconfirmed movement slot handling.
It records a temporary blocked-path hint only when current collision proves
that full attempted path is blocked. A vacated monster cell no longer remains
blacklisted for3seconds. Static collision, all intermediate Run cells, pending
ACK requirements and original path selection remain. Full opaque action
receipt/packet-before-ACK lifecycle is still open; this is not its completion.

## Actual verification

Focused11 new checks pass; adjacent input/presentation/sender340 pass; final
native unit stage gate passes911 distinct tests with7 original native/external
checks ignored. Counts overlap and are not added. The final binary executed
every nonignored native unit test serially from the owned target
`C:/mir2-build/p1-native-escape-20261010-target`. Source hashes of16 selected
paths are equal before/after each final invocation, not whole input closure.
Final stage runtime10.31seconds and observed peak private memory240,726,016bytes
are measurements of this component test process, not game FPS or player-load
claims. The watcher stopped no process. Original GPU/native/external ignored
checks remain listed in the immutable receipt, not silently counted passed.

The sender failure check uses the real UI-forwarding system with a closed
command receiver. Escape still requires a genuine ACK for an unconfirmed move.
599/600ms and799/800ms actual frame boundaries, full400ms correction protection,
unchanged next-hit interval, death/revival and current occupancy are checked.
Three retained correction/path-cache fixtures now supply a real currently
blocking monster; their original clocks and alternate-step assertions remain.

The first compilation attempt reused a shared target containing another
worktree's Mir2 runtime/UI artifacts. It failed with foreign Jump/type errors;
no unrelated source was changed to accommodate that result. The owned target
copied registry-only products while excluding every Mir2/incremental product,
then rebuilt actual source crates. A bare PowerShell boolean caused the cache
copy receipt to fail after copying; separate read-only validation retained the
completed copy without repeating or modifying the shared target.

The next11-test run compiled but did not finish before the machine rebooted
at2026-10-10 16:53:05UTC. Seven partial pass lines are retained; no result/after
receipt exists and it is unqualified. That run observed about43GB working set;
the cause of the system reboot itself is not established. New fixture actors
were initialized at animation time0 and later sent Unix time by the normal
sender, causing years of idle-frame replay. Production packet/render/sender
entrances all use Unix for this same world. Only the three new fixtures now
use a current absolute animation origin; original controller clocks/boundary
assertions remain. A real Instant-bounded wait prevents the sender encountering
a future manually advanced frame. Additional motion-window assertions prove
prediction actually started, rather than swallowing TimeWentBackwards. Product
code is byte-identical to the originally integrated Candidate outside tests.
The first interrupted-run metadata parser omitted CRLF-terminated partial pass
lines; the original metadata and explicit7-line correction are both retained.

[Immutable receipt](generated/player-qa/client-escape-20261011/candidate-01/EVIDENCE.json)
and [all original logs/source/receipts](generated/player-qa/client-escape-20261011/candidate-01/original-evidence.zip)
retain compile/preparation/interruption records as well as final passes.
ZIP303609bytes, SHA256
`712cfe6e9c9f76410d580c7c0749d8b1cd64dfd370c266cfe84af9e1cb4cefb5`.

## Publication and remaining acceptance

Server parent1e is actually pushed, with own Linux run38102609192 dispatched;
its current completion/artifact/publication must be read, not inferred here.
Fresh read-only public Gateway observation2026-10-11 01:59:21UTC retains source
64b6, six capacity counters0/TCP7200=0/viewers0; remote Zone is unconfigured.
That is not a capacity/load claim or authorization to skip a fresh stage/apply
drain. Current released Windows remains R23/sourcefaef/feed18 at this checkpoint.
No client release, live character change, kick, other-game stop or acceptance
is claimed. New source requires own Windows artifact, signed update feed and
ordinary paired checks before publication. Strict real three-class seven-monster
held-right escape, continued attack, no drift and native/human screenshots are
still required. FullP1-P7/Mentor remains open; the expired heartbeat stays paused
and the blocked Goal is not marked complete.
