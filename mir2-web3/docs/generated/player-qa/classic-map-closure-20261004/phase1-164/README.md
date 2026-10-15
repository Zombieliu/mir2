# Admitted-world resource integration

Root reviewed and integrated the bounded seven-file change as `ca5eee0a5`
from worker commit `38a64de2985c82008a596a34768f36185980f62e`.
The handed-off source hashes were checked before commit. Source base is the
released R17 `6032ef8b3e27dd97bad0b20c8676ef9f185db64b`.

[Integration receipt](integration.json), [original implementation receipt](implementation-receipt.json)
and [actual PowerShell generated-resource guards](powershell-generated-guards.json)
record all 164 admitted maps, 67,554 covered drawable references and zero
missing drawable/omitted/unexpected maps. Original empty/out-of-range slots
remain classified. The 16 conditional entries are static paths, not real
NPC predicates or travel acceptance.

[Focused/adjacent checks](focused-and-adjacent-tests-passed.log) passed 60/60.
The [first failing test run](focused-and-adjacent-tests.log) and
[unchanged source6032 baseline](route-manifest-baseline-6032.log) preserve the
obsolete v25 route-test expectation; actual profile v26 was not changed.

The signed R17 release remains unchanged at its 32-map image scope. This phase
has **not** been built, packaged, published or player-accepted. The next phase
adds 45 excluded classical branches and Monster049, preserves D71653's original
no-entry classification, then obtains ordinary route/Boss/drop/save evidence.

Full technical scope is [documented here](../../../../CLASSIC-MAP-CLOSURE-20261004.md).
