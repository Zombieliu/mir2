# Shared skill cooldown projection

Taoist N11 ordinary receipts exposed a server projection defect. SoulFireBall
was accepted with an 1800ms shared Zone deadline, but accelerated personal
packet ticks reported zero cooldown after about 632ms. The ordinary client
then sent rejected casts before the Zone deadline and exhausted its original
20-action engagement budget. It did not spend this budget on self Healing.
The target remained alive at 3 HP; ordinary-pass4 preserves that failure.

The Zone retains admission authority. A read-only owner getter returns the
later of per-spell and global casting deadlines, relative to the same shared
clock. The public owner snapshot projects this duration into the existing
cooldownRemainingTicks field by rounding up to seconds. Passive and toggle
entries keep their existing semantics. No packet schema, MP cost, cooldown
rule or attack budget changes.

Fresh Simulation getter tests pass 3/3: actual accepted SoulFireBall at
0/1000/1799/1800ms and recast, global 300ms gate and owner isolation, and
uncast/rejected/duplicate casts preserving the original deadline.
Fresh Gateway bridge tests pass 11/11, including the new public cooldown test,
N4 Healing, all five safe arrival/profile regressions and four receipt lifecycle
tests. The new test exercises actual accepted Magic, accelerates private Tick
until legacy readiness is zero, proves public cooldown still positive and early
cast rejection without MP spend, waits the real remaining shared deadline,
then proves readiness zero and a second accepted cast.

The final Gateway harness was rebuilt after formatting only the newly changed
blocks/test source (18.25s); tests completed in 155.87s. The independent read-only
review passed. New test files pass rustfmt check; changed source passes diff check.
Formal release compilation is still in progress; this fix is not yet deployed.

Build recovery: an initial getter compile had two reference-match type errors,
which were corrected. Its original raw log was overwritten by the next failed
build and is not claimed retained. That build accidentally used the default
E: target and failed because the drive was full. The generated failed cache
and an older simulation incremental cache were moved, without deletion, to
C:/mir2-newcomer-v2-20260918/failed-default-target-artifacts. Fresh Simulation
compilation then passed with an explicit C: --target-dir. All subsequent
compilation uses explicit C: targets, offline/locked dependencies and one job.

Independent read-only Gateway review found no blocking issue in owner lookup,
canonical Spell parsing, lock order, shared clock or active/toggle handling.

The original three timing ledgers remain expired. A separate clean run will
use fresh normal accounts after the cooldown release build finishes; it will
not copy, reset or extend the old clocks or alter saved character rows.
No ordinary completion, native package or visual acceptance is implied here.

visualAccepted=false; measuredTime=false; globalParityPercent=null.
