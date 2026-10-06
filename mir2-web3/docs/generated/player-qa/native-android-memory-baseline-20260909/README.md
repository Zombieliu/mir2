# Offline UI regression and editing memory baseline

Tested source HEAD `490928e484f17c600a63223112edc9239831b3f4`, clean worktree.
No source fix in this checkpoint. Existing final preview APK from `bc392f889`:
SHA256 `87a507d2de5280ec45ea0faa61e74d99c37e20a4ab9100c829e1d376313a3423`.
API31 Pixel5 arm64 emulator, not a physical device; no Gateway connection.

## Completed regression

The capture script started during the user-interrupted turn continued running.
Its run log confirms all 33 offline specimens captured and passed the script's
ready/panic/FATAL/PathNotFound checks. This is not all-interaction/visual parity.
Full captures and source/device metadata remain locally at
`/tmp/android-ui-acceptance-Bipgw4`; they are not duplicated in Git.

24 memory samples across the cold-launch scene run and final Help idle phase
showed roughly 297116–313044 KB RSS. Later Help PID12722 was still running at
310288 KB RSS. Each scene is force-stopped by the capture script, so this does
not prove in-process scene-transition stability.

## Targeted editing reproduction

Cold-started mail-compose; tapped message at physical `(1450,110)`.
Sampled before, then 20 rounds of `adb shell input text abcdefghijklmnopqrstuvwxyz`
and `adb shell input keyevent --longpress 67`, sampling dumpsys meminfo each round.
The delete event is not a full draft reset; this is repeated editing/appending,
not a constant-length A/B benchmark. No mail was sent.

Before-loop RSS 525200 KB grew to 839772 KB on the last loop sample (about
513→820 MiB). The native heap accounts for most of the increase. Screenshot
confirms actual mail body editing; no world or network activity was involved.
Post-loop RSS then reached 976644 KB. At 23:04:28 lowmemorykiller killed
preview PID13032 at 1171124 KB RSS, oom_score_adj0. The post-Cancel sample
reported no process, so Cancel is NOT accepted in this run. Logs are retained.

This localizes a reproducible growth path to editing, but does not prove the
allocation owner or a leak. UI/entity counts, font/layout caches, image assets
and allocator retention need instrumentation before proposing a fix. Earlier
lowmemorykiller evidence remains valid; no stability or process-death recovery
acceptance is claimed. No AVD wipe, RAM increase or package data deletion.

After collecting evidence a force-stop was issued for the test preview, but
the process had already been killed by Android. The emulator was not stopped.

## Next

Instrument the preview-only editing path with entity/text/asset counts and
compare repeated fixed-length edits versus idle and panel close. Then fix the
identified lifetime owner with a failing regression and repeat the same memory
workload. Full phone HUD/chat/dialog layout, multiplayer/network state and
physical-device acceptance remain open. No new test Gateway or phone supplied.
Local-only evidence; no push, PR mutation or deployment this checkpoint.
