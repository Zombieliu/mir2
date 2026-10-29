# Running map handoff repair — 2026-09-21

User observed screen flashing while running. Two renderer defects reproduced:

- Replacing an atlas layout explicitly removed the old asset while retained
  sprites still needed it during an image-readiness wait.
- Standalone URL images were treated as ready when their handles existed,
  before pixels were loaded. Retained animation images had the same weakness.

The renderer now requires loaded URL images and their dependencies for every
active map image, and lets sprite strong handles retain superseded layouts
until rebinding. Failed loading keeps the previous frame and reports an error.

Validation: two regressions failed on the old implementation; the fixed runtime
library passes 230/230 tests, including retained animation readiness and eventual
old-layout reclamation. Windows offline Release build passes (existing warnings).
Package: `C:/numeron-legend-of-rebirth-20260921-run-flicker`.

Not yet deployed over the user's active session; sustained-running visual
verification remains open. These tests do not establish the exact appearance
or sole cause of the user's reported flash.

Separate diagnostic finding: the current process samples grow from approximately
1.41 GB private bytes at startup to 12.75 GB, then plateau; working set is about
6.48 GB. Read-only review identifies missing additive map-material eviction on
tile removal and scene-long entity atlas residency. Their contribution requires
measurement; memory growth is not fixed or explained by this patch.
