# Unavailable ordinary map collision

Map collision read/parse failure previously became `ZoneCollision::unbounded()`.
The normal Zone Join → Walk/Run → tick path therefore allowed movement on missing
terrain; ordinary strict checkpoint restore reproduced it. This round makes that
absence a private closed collision identity. Known terrain and trusted explicit
open arenas retain their original live movement/serialization bytes because
`unavailable=false` is omitted. Doors, builder changes and transaction clones do
not clear unavailable; transfer-cell bypass runs only after this guard. Mining
also rejects unknown terrain as a wall.

The corrected real before-fix run is1 pass/3 failures. The earlier0/4 contains one
new550ms positive-fixture timing error, retained separately. Product cadence stays
600ms. The old cold fixture used explicit unbounded with a missing key; the exact
frozen9fe7 parent passes19, while the new round initially passes18/fails1 at strict
root verification. Only its constructor now uses the ordinary map collision
identity. Every original cold payload, online revocation and tamper assertion is
retained. A new test explicitly refuses strict restore of an open missing-map
checkpoint; no ignore-root, fallback arena or weakened restoration is added.

Final73 distinct selected checks pass: collision/door/checkpoint25, normal missing
and explicit-open cold5, original movement14, mining5/security20, original release
Gateway4. Nine are new;64 are retained regressions. Pre/post/current hashes cover
19 selected paths, not a whole toolchain/input closure. Independent read-only
review covers the final four files. The original failed runs remain in the archive.

This does not reject Join or trusted transforms. Missing-map prior open roots can
be rejected by strict cold restore. Existing verified-world reanchor policy is not
changed. Gateway failed Walk/Run/Turn still has a current-foot transfer path; this
round is not complete transfer safety or successful Source Map.Load/population.
Map0 can also use its known embedded starter fragment. Full P1-P7/Mentor and native
human acceptance remain unfinished, with the expired heartbeat still paused.

Separately, source9fe7 is actually pushed with exact guarded Git readbacks; its
original63 local checks and actual463 raw/1620 gzip byte probes are recorded in
[the preceding I/O round](CLASSIC-SOURCE-MAP-IO-20261010.md). New I/O Linux
38006259110 remains distinct. Predecessor terrain64b6 passed Linux38001876065;
its first partial artifact download hit unexpected EOF and remains retained.
The fresh artifact02 passed the original ZIP/TAR/ELF/full gate verifier.

Terrain64b6 was then published after fresh six-zero/TCP/viewer checks, private
DB/state/recovery/config backup, normal c704 stop and candidate normal stop/cold
restart. Only isolated Gateway ExecStart changed; original realm, resources,
environment, authentication and R23/feed18 delivery were not modified. Original
44 ordinary public WSS and18 protocol/native-resume checks pass. These are normal
public packet stories, not GUI/human acceptance; this release does not include
9fe7 I/O or the new unavailable guard. Existing operator has two snapshot-to-stop
windows without an admission fence; no claim is made that future arrivals are
atomically excluded. Apply success is separate from complete public verification;
candidate-started failures do not trigger a blind schema/DB rollback.

[Original local, Git, Linux and public evidence](generated/player-qa/unavailable-collision-20261010/candidate-01/EVIDENCE.json).
