# Per-step map pause after smooth fallback repair

User clarifies the remaining symptom: the whole map pauses at every running
step. Process 11188 (`79997ee92`) live trace is
`C:/mir2-ui-repair-20260921/render-live/20260922-205515-007-render.jsonl`.
27 commands are confirmed; captured anchor drift/reverse candidates are absent.
During correlated running frames, individual frame intervals peak at 20.68 ms,
but position-change gaps reach 21–37 ms at command starts. Map sync is below
0.5 ms in this selection. These CPU observations do not measure GPU present.

Example command at 1790082104677: frames at 4686, 4699, 4709 retain exactly
(14928,13728); the latter two have entitySyncCpuUs=0. At 4720 the center changes
from (311,429) to (313,431), and smooth position resumes at
(14931.6416,13730.4277). Evidence: `render-live/center-wait-pause-evidence.json`.
This matches the early center-mismatch return and earlier camera-freeze repair.

Smooth mode now keeps advancing the camera in the committed old map center.
When an incoming entity frame names another center, the retained self composite
(body/hair/weapon actor layers) moves by the same camera-relative delta while
retaining its images, depth and applied-center provenance. The normal matching
snapshot commits later. This avoids both camera-only movement and freezing the
whole view. Legacy stepped mode retains its earlier freeze behavior.

The ECS regression supplies a pending new center and advances the camera over
three frames, asserting actual retained body transforms advance, the actor's
screen anchor stays fixed, and committed centers remain old/coherent.
Focused regression 1/1 passed. Full runtime and release logs are
`center-wait-motion-runtime.log` and `center-wait-motion-release.log` in
`C:/mir2-ui-repair-20260921`. Live pause-gap comparison and user acceptance remain
pending. No overall stutter acceptance is claimed.
