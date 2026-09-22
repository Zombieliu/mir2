# Running anchor mismatch: captured evidence and candidate repair

User reproduced running stutter with client `c6c0a0807`, process 7188, then
normally exited. Trace: `C:/mir2-ui-repair-20260921/render-live/20260922-194422-825-render.jsonl`.
Summary and focused evidence are beside it as `20260922-194422-825-summary.json`
and `running-anchor-evidence.json`. No lost trace records or malformed lines.

At unix ms 1790078396803 / 6814 and 1790078398132 / 8146, committed self-anchor
drift was 20 / 20 / 18.87 / 18.87 stage pixels. Frame intervals were 11.26,
11.33, 14.83 and 13.69 ms. Entity sync took zero microseconds at the first frame
of each pair, then applied the incoming center on the next frame. Map centers
advanced (302,445) -> (304,447) and (306,449) -> (308,451). The actual visual
world center did not reverse. All 19 recorded movement results were confirmed.

The 751 anomaly-selected captured frames in the running interval have maximum
17.55 ms frame interval, 707 us map sync, and 48 us entity sync. This does not
measure GPU presentation or uncaptured frames. Startup's 233 ms gap and the
later 69.95 ms gap must not be represented as these running-frame costs.
Likewise CPU-stage markers named world_snapshot_forwarded measure elapsed
milestone time, not an individual CPU blocking operation.

Source matches the two-frame signature: when incoming entity center differs
from the committed map center, `sync_entity_render_layers` retains the old
actor composite, but `begin_presentation_pose_frame` advanced its camera.
Candidate retains the previous camera offset/source in that same center-wait
condition. Matching-center frames continue through the normal motion path.
This preserves the previous complete scene during the wait rather than letting
the camera move past a retained actor.

Validation: repeated delayed-center regression 1/1; full runtime 262/262.
Release build log: `C:/mir2-ui-repair-20260921/retained-camera-release.log`.
New ordinary running reproduction and physical display smoothness remain open;
this is a repair for the captured anchor defect, not proof all stutter is gone.
