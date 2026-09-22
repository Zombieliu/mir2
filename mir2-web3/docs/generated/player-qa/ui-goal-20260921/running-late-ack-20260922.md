# Running rollback: delayed movement confirmation

The user reports visible player rubber-banding during sustained running on
client 7532ac047. No CPU-overload or unbounded command-queue cause is established:
the native controller permits one outstanding movement, and Zone Walk/Run
intents replace earlier queued movement. Both use a nominal 600ms step.

A deterministic presentation failure was reproduced before this repair
(late-ack-before.log): the prediction deadline expired before its matching ACK,
discarding the receipt and camera window. The retained source-centred state
could become visible again; a late matching movement then recreated animation.

The repair separates completion of animation from receipt of confirmation.
One local receipt survives its animation deadline without extending the step
or sending another command. The scene keeps its authoritative source centre
until ACK, while camera/self offsets hold at the predicted endpoint. A matching
late ACK consumes the completed step without replay. A real correction still
clears prediction and restores the authoritative transform. Session reset and
new movement retain their normal ownership boundaries.

Runtime camera interpolation now clamps at the completed endpoint relative to
the actually committed map centre. When that centre advances to the target,
the relative offset reaches zero without a screen jump. Regression checks
include the shared camera/self offsets and a fixed world object's composed
screen coordinate across that commit. The initial attempted solution of
publishing a predicted target centre was rejected during review because it
could conflict with the still source-centred map; it is not the final design.

Evidence is under C:/mir2-ui-repair-20260921. The final coherent-centre native
presentation suite passes 33/33 (late-ack-coherent-center.log); it includes
late confirmation, two runs, stale source snapshots and real correction.
Final source 91a3325b6 passes runtime 251/251 (late-ack-runtime-full.log) and
Windows 696/696 (late-ack-windows-full.log). The full checks include the final
coherent-centre changes and composed camera/player/world-position regression.
Release/package identity and live acceptance remain separate.

This demonstrates and repairs a code-level failure mode. It does not prove
that it is the only cause of the user's live symptom. Same-build continuous
running with movement trace and ordinary collision/turn/stop checks remains
required. No server cooldown, collision rule, save or character progress was
modified. The running user's client has not been interrupted.
