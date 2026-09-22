//! Opt-in CPU/main-world telemetry. This does NOT measure GPU execution,
//! swapchain presentation, display refresh, or video frame delivery.
use super::*;
use serde_json::{json, Value};
use std::collections::VecDeque;
use std::io::{BufWriter, Write};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, OnceLock};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

const PRE_MS: u64 = 10_000;
const POST_MS: u64 = 2_000;
const MAX_FRAMES: usize = 1_200;
const QUEUE_CAPACITY: usize = 2_048;
const MAX_FILE_BYTES: u64 = 256 * 1024 * 1024;
const STOP_RECORD_RESERVE: u64 = 512;
const ANCHOR_THRESHOLD_PX: f32 = 4.0;
static SINK: OnceLock<Option<Sink>> = OnceLock::new();
static MAP_US: AtomicU64 = AtomicU64::new(0);
static ENTITY_US: AtomicU64 = AtomicU64::new(0);
static BOUNDARY_UNTIL: AtomicU64 = AtomicU64::new(0);
static ENTITY_CENTER: AtomicU64 = AtomicU64::new(0);
static ENTITY_CENTER_VALID: AtomicBool = AtomicBool::new(false);
static WRITER_STOPPED: AtomicBool = AtomicBool::new(false);
struct Sink {
    sender: mpsc::SyncSender<Value>,
    dropped: AtomicU64,
}

fn unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(u128::from(u64::MAX)) as u64
}

fn sink() -> Option<&'static Sink> {
    SINK.get_or_init(|| {
        let path = std::env::var_os("MIR2_NATIVE_RENDER_TRACE_PATH").filter(|v| !v.is_empty())?;
        let (sender, receiver) = mpsc::sync_channel::<Value>(QUEUE_CAPACITY);
        let spawned=std::thread::Builder::new().name("mir2-render-trace".into()).spawn(move || {
            if let Err(error)=run_writer(std::path::Path::new(&path),receiver) {
                eprintln!("[render-diagnostics] writer stopped: {error}; gameplay continues without render tracing");
            }
            WRITER_STOPPED.store(true,Ordering::Relaxed);
        });
        if let Err(error)=spawned { eprintln!("[render-diagnostics] cannot start writer: {error}"); return None; }
        Some(Sink { sender, dropped: AtomicU64::new(0) })
    }).as_ref().filter(|_| !WRITER_STOPPED.load(Ordering::Relaxed))
}

fn run_writer(path: &std::path::Path, receiver: mpsc::Receiver<Value>) -> std::io::Result<()> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    let file = std::fs::OpenOptions::new()
        .create(true)
        // Windows cannot lock an append-only handle; include read access.
        .read(true)
        .append(true)
        .open(path)?;
    // Never let two processes race the same file budget. Failure affects only
    // diagnostics; use separate trace paths for simultaneous clients.
    file.try_lock().map_err(|error| {
        std::io::Error::other(format!("trace file already locked or unavailable: {error}"))
    })?;
    let existing = file.metadata()?.len();
    let mut output = BudgetWriter {
        writer: BufWriter::new(file),
        written: existing,
        limit: MAX_FILE_BYTES,
    };
    output.record(&json!({"type":"renderTraceSession","unixMs":unix_ms(),"processId":std::process::id(),
        "schema":"mir2.native.render-cpu.v1","measurementStage":"main_world_last_after_transform_propagation",
        "gpuPresentMeasured":false,"videoMeasured":false,"preMs":PRE_MS,"postMs":POST_MS,"maxPreFrames":MAX_FRAMES,"maxFileBytes":MAX_FILE_BYTES}))?;
    output.writer.flush()?;
    let mut ring = CaptureRing::default();
    let mut health = HealthWindow::default();
    let mut movement = MovementCorrelation::default();
    for mut event in receiver {
        movement.observe(&mut event);
        if let Some(summary) = health.observe(&event) {
            output.record(&summary)?;
        }
        let events = if event["type"] == "renderFrame" {
            ring.push(event)
        } else {
            vec![event]
        };
        for event in events {
            output.record(&event)?;
        }
        output.writer.flush()?;
    }
    output.writer.flush()
}

struct BudgetWriter<W: Write> {
    writer: W,
    written: u64,
    limit: u64,
}
impl<W: Write> BudgetWriter<W> {
    fn record(&mut self, event: &Value) -> std::io::Result<()> {
        let mut bytes = serde_json::to_vec(event)?;
        bytes.push(b'\n');
        if self.written.saturating_add(bytes.len() as u64)
            > self.limit.saturating_sub(STOP_RECORD_RESERVE)
        {
            let mut stop = serde_json::to_vec(
                &json!({"type":"renderTraceStopped","unixMs":unix_ms(),"processId":std::process::id(),"reason":"fileBudgetReached","maxFileBytes":self.limit}),
            )?;
            stop.push(b'\n');
            if self.written.saturating_add(stop.len() as u64) <= self.limit {
                self.writer.write_all(&stop)?;
                self.written += stop.len() as u64;
            }
            self.writer.flush()?;
            return Err(std::io::Error::other(
                "fileBudgetReached: trace file budget exhausted",
            ));
        }
        self.writer.write_all(&bytes)?;
        self.written += bytes.len() as u64;
        Ok(())
    }
}

// Correlate on the writer thread: no locks or additional channels on the
// gameplay/render paths. These are visual candidates, never server verdicts.
#[derive(Default)]
struct MovementCorrelation {
    command: Option<CorrelatedCommand>,
    dropped: u64,
}
struct CorrelatedCommand {
    from: Vec2,
    to: Vec2,
    unix_ms: u64,
    last_projection: Option<f32>,
}
impl MovementCorrelation {
    fn observe(&mut self, event: &mut Value) {
        let drops = event["droppedRecords"].as_u64().unwrap_or(0);
        if drops != self.dropped {
            self.command = None;
            self.dropped = drops;
        }
        let now = event["unixMs"].as_u64().unwrap_or(0);
        if event["type"] == "renderMarker" {
            let marker = &event["marker"];
            let details = marker.get("details").unwrap_or(marker);
            let kind = format!(
                "{} {}",
                event["kind"].as_str().unwrap_or(""),
                details["type"].as_str().unwrap_or("")
            )
            .to_ascii_lowercase();
            if ["turn", "correction", "reset", "map", "focus"]
                .iter()
                .any(|s| kind.contains(s))
                || details["tsDisposition"] == "correction"
                || details["tsDisposition"] == "degraded"
            {
                self.command = None;
                return;
            }
            if details["type"] == "commandSent" {
                let coordinates = ["fromX", "fromY", "toX", "toY"]
                    .map(|key| details[key].as_f64().map(|v| v as f32));
                self.command = match coordinates {
                    [Some(x), Some(y), Some(tx), Some(ty)]
                        if [x, y, tx, ty].iter().all(|v| v.is_finite()) && (x != tx || y != ty) =>
                    {
                        Some(CorrelatedCommand {
                            from: Vec2::new(x * 48.0, y * 32.0),
                            to: Vec2::new(tx * 48.0, ty * 32.0),
                            unix_ms: now,
                            last_projection: None,
                        })
                    }
                    _ => None,
                };
            } else if details["type"] == "authoritative" && details["tsDisposition"] == "confirmed"
            {
                // A degraded run is a legitimate different destination.
                if let (Some(command), Some(x), Some(y)) = (
                    self.command.as_ref(),
                    details["x"].as_f64(),
                    details["y"].as_f64(),
                ) {
                    if command
                        .to
                        .distance(Vec2::new(x as f32 * 48.0, y as f32 * 32.0))
                        > 0.01
                    {
                        self.command = None;
                    }
                }
            }
            return;
        }
        if event["type"] != "renderFrame" {
            return;
        }
        let position = visual_world_center(event);
        event["visualWorldCenterStagePx"] = json!(position.map(|p| [p.x, p.y]));
        if event["boundarySuppressed"] == true {
            self.command = None;
            return;
        }
        let Some(command) = self.command.as_mut() else {
            return;
        };
        if now < command.unix_ms || now.saturating_sub(command.unix_ms) > PRE_MS {
            self.command = None;
            return;
        }
        let Some(position) = position else {
            self.command = None;
            return;
        };
        let segment = command.to - command.from;
        let length = segment.length();
        let direction = segment / length;
        let projection = (position - command.from).dot(direction);
        let closest = command.from + direction * projection.clamp(0.0, length);
        let deviation = position.distance(closest);
        let reverse = command
            .last_projection
            .map(|previous| previous - projection)
            .unwrap_or(0.0)
            .max(0.0);
        command.last_projection = Some(projection);
        event["movementCorrelation"] = json!({"commandUnixMs":command.unix_ms,
            "fromStagePx":[command.from.x,command.from.y],"toStagePx":[command.to.x,command.to.y],
            "projectionStagePx":projection,"segmentDeviationStagePx":deviation,"reverseStagePx":reverse,
            "classification":"presentation_candidate_not_server_verdict"});
        if let Some(flags) = event["anomalies"].as_array_mut() {
            if reverse > ANCHOR_THRESHOLD_PX {
                flags.push(json!("presentationReverseCandidate"));
            }
            if deviation > ANCHOR_THRESHOLD_PX {
                flags.push(json!("presentationOutsideCommandSegmentCandidate"));
            }
        }
    }
}

fn visual_world_center(event: &Value) -> Option<Vec2> {
    let map = &event["appliedMapCenter"];
    let camera = &event["cameraGlobalTransform"];
    let p = Vec2::new(
        map[0].as_f64()? as f32 * 48.0 + camera[0].as_f64()? as f32,
        map[1].as_f64()? as f32 * 32.0 - camera[1].as_f64()? as f32,
    );
    p.is_finite().then_some(p)
}

fn offer(mut event: Value) {
    let Some(sink) = sink() else {
        return;
    };
    event["droppedRecords"] = json!(sink.dropped.load(Ordering::Relaxed));
    if sink.sender.try_send(event).is_err() {
        sink.dropped.fetch_add(1, Ordering::Relaxed);
    }
}

/// Platform command/ACK/asset timings share the same nonblocking JSONL stream.
/// Explicit marker types containing turn/map/focus suppress anchor alarms for
/// 500ms. Markers are retained independently of anomaly-triggered frame capture.
pub fn native_render_diagnostics_enabled() -> bool {
    sink().is_some()
}

pub(crate) fn record_applied_entity_center(center: Option<[i32; 2]>) {
    if !native_render_diagnostics_enabled() {
        return;
    }
    if let Some([x, y]) = center {
        ENTITY_CENTER.store(
            ((x as u32 as u64) << 32) | (y as u32 as u64),
            Ordering::Relaxed,
        );
    }
    ENTITY_CENTER_VALID.store(center.is_some(), Ordering::Relaxed);
}

pub fn record_native_render_marker(kind: &str, value: Value) {
    if sink().is_none() {
        return;
    }
    let now = unix_ms();
    let event_type = format!(
        "{} {}",
        kind,
        value.get("type").and_then(Value::as_str).unwrap_or("")
    )
    .to_ascii_lowercase();
    if ["turn", "map", "focus"]
        .iter()
        .any(|word| event_type.contains(word))
    {
        BOUNDARY_UNTIL.store(now.saturating_add(500), Ordering::Relaxed);
    }
    offer(
        json!({"type":"renderMarker","unixMs":now,"processId":std::process::id(),"kind":kind,"marker":value}),
    );
}

pub(crate) struct SyncTimer {
    started: Option<Instant>,
    destination: &'static AtomicU64,
}
impl SyncTimer {
    pub(crate) fn map() -> Self {
        Self::new(&MAP_US)
    }
    pub(crate) fn entity() -> Self {
        Self::new(&ENTITY_US)
    }
    fn new(destination: &'static AtomicU64) -> Self {
        Self {
            started: sink().map(|_| Instant::now()),
            destination,
        }
    }
}
impl Drop for SyncTimer {
    fn drop(&mut self) {
        if let Some(start) = self.started {
            self.destination.store(
                start.elapsed().as_micros().min(u128::from(u64::MAX)) as u64,
                Ordering::Relaxed,
            );
        }
    }
}

#[derive(Default)]
pub(crate) struct FrameHistory {
    frame_id: u64,
    last: Option<Instant>,
    last_identity: Option<String>,
    last_focus: Option<bool>,
    last_anchor: Option<Vec2>,
    boundary_frames: u8,
}

pub(crate) fn capture_committed_frame(
    mut history: Local<FrameHistory>,
    registry: Res<SceneRegistry>,
    poses: Res<presentation_pose::PresentationPoseBuffer>,
    state: Res<RuntimeEntityRenderState>,
    camera: Query<&GlobalTransform, With<MainCamera>>,
    transforms: Query<&GlobalTransform>,
    windows: Query<&Window>,
) {
    if sink().is_none() {
        return;
    }
    let instant = Instant::now();
    history.frame_id = history.frame_id.saturating_add(1);
    let delta_ms = history
        .last
        .replace(instant)
        .map(|last| instant.duration_since(last).as_secs_f64() * 1000.0);
    let now = unix_ms();
    let focus = windows.iter().next().map(|w| w.focused);
    let self_id = state
        .snapshot
        .as_ref()
        .and_then(|s| s.entities.iter().find(|e| e.is_self))
        .map(|e| e.object_id.clone());
    if history.last_identity != self_id || history.last_focus != focus {
        history.boundary_frames = 3;
        history.last_anchor = None;
    }
    history.last_identity = self_id.clone();
    history.last_focus = focus;
    let camera_world = camera.iter().next().map(|v| v.translation());
    // The registry root is the last COMMITTED actor composite root, while the
    // body position below comes from actual propagated ECS GlobalTransform.
    // Neither value is taken from the desired pose or pending snapshot coords.
    let root = self_id
        .as_ref()
        .and_then(|id| registry.entity_render_actor_roots.get(id))
        .copied();
    let body = self_id
        .as_ref()
        .and_then(|id| registry.entity_render_layers.get(&format!("{id}:body")))
        .and_then(|handle| transforms.get(handle.entity).ok())
        .map(|v| v.translation());
    let anchor = root
        .zip(camera_world)
        .map(|(root, camera)| (root - camera).truncate());
    let map = poses.applied_map_center().map(|c| [c.x, c.y]);
    let packed = ENTITY_CENTER.load(Ordering::Relaxed);
    let entity = ENTITY_CENTER_VALID
        .load(Ordering::Relaxed)
        .then_some([(packed >> 32) as u32 as i32, packed as u32 as i32]);
    let boundary = history.boundary_frames > 0
        || now < BOUNDARY_UNTIL.load(Ordering::Relaxed)
        || focus != Some(true)
        || body.is_none()
        || map.is_none()
        || entity.is_none();
    let drift = if boundary {
        None
    } else {
        anchor.zip(history.last_anchor).map(|(a, b)| a.distance(b))
    };
    history.last_anchor = anchor;
    history.boundary_frames = history.boundary_frames.saturating_sub(1);
    let anomalies = anomalies(delta_ms, !boundary && map != entity, drift);
    let xyz = |v: Option<Vec3>| v.map(|v| [v.x, v.y, v.z]);
    offer(
        json!({"type":"renderFrame","unixMs":now,"processId":std::process::id(),"frameId":history.frame_id,
        "measurementStage":"main_world_last_after_transform_propagation","gpuPresentMeasured":false,"videoMeasured":false,
        "frameWallDeltaMs":delta_ms,"mapSyncCpuUs":MAP_US.swap(0,Ordering::Relaxed),"entitySyncCpuUs":ENTITY_US.swap(0,Ordering::Relaxed),
        "appliedMapCenter":map,"appliedEntityCenter":entity,"cameraGlobalTransform":xyz(camera_world),
        "selfBodyGlobalTransform":xyz(body),"selfCommittedActorRoot":xyz(root),"selfObjectId":self_id,
        "coordinateUnits":"stage_pixels_not_framebuffer_pixels",
        "selfScreenAnchorWorldUnits":anchor.map(|v|[v.x,v.y]),"selfAnchorDriftPx":drift,
        "anchorThresholdPx":ANCHOR_THRESHOLD_PX,"boundarySuppressed":boundary,"focused":focus,"anomalies":anomalies}),
    );
}

fn anomalies(delta_ms: Option<f64>, mismatch: bool, drift: Option<f32>) -> Vec<String> {
    let mut result = Vec::new();
    if let Some(ms) = delta_ms {
        for threshold in [33.0, 50.0, 100.0] {
            if ms >= threshold {
                result.push(format!("frameWallGe{}Ms", threshold as u32));
            }
        }
    }
    if mismatch {
        result.push("appliedCenterMismatch".into());
    }
    if drift.is_some_and(|d| d > ANCHOR_THRESHOLD_PX) {
        result.push("selfAnchorDrift".into());
    }
    result
}

#[derive(Default)]
struct HealthWindow {
    start: Option<u64>,
    frames: u64,
    buckets: [u64; 4],
    max_ms: f64,
    total_ms: f64,
}
impl HealthWindow {
    fn observe(&mut self, event: &Value) -> Option<Value> {
        if event["type"] != "renderFrame" {
            return None;
        }
        let now = event["unixMs"].as_u64()?;
        let start = *self.start.get_or_insert(now);
        self.frames += 1;
        if let Some(ms) = event["frameWallDeltaMs"].as_f64() {
            self.total_ms += ms;
            self.max_ms = self.max_ms.max(ms);
            self.buckets[if ms < 33.0 {
                0
            } else if ms < 50.0 {
                1
            } else if ms < 100.0 {
                2
            } else {
                3
            }] += 1;
        }
        if now.saturating_sub(start) < PRE_MS {
            return None;
        }
        let summary = json!({"type":"renderHealthSummary","unixMs":now,"processId":std::process::id(),"fromUnixMs":start,"frames":self.frames,
            "frameDeltaBucketsLt33Lt50Lt100Ge100":self.buckets,"maxFrameWallDeltaMs":self.max_ms,
            "meanFrameWallDeltaMs":self.total_ms/self.buckets.iter().sum::<u64>().max(1) as f64,
            "droppedRecords":event["droppedRecords"],"gpuPresentMeasured":false,"videoMeasured":false});
        *self = Self::default();
        Some(summary)
    }
}

#[derive(Default)]
struct CaptureRing {
    frames: VecDeque<Value>,
    until: Option<u64>,
    cooldown_until: u64,
}
impl CaptureRing {
    fn push(&mut self, event: Value) -> Vec<Value> {
        let now = event["unixMs"].as_u64().unwrap_or(0);
        let anomalous = event["anomalies"].as_array().is_some_and(|a| !a.is_empty());
        if self.until.is_some_and(|end| now <= end) {
            return vec![event];
        }
        if let Some(end) = self.until.take() {
            self.cooldown_until = end.saturating_add(PRE_MS);
        }
        while self
            .frames
            .front()
            .is_some_and(|v| now.saturating_sub(v["unixMs"].as_u64().unwrap_or(0)) > PRE_MS)
        {
            self.frames.pop_front();
        }
        self.frames.push_back(event);
        while self.frames.len() > MAX_FRAMES {
            self.frames.pop_front();
        }
        if anomalous && now >= self.cooldown_until {
            self.until = Some(now.saturating_add(POST_MS));
            let mut result = vec![
                json!({"type":"renderCaptureTriggered","processId":std::process::id(),"unixMs":now,"postUntilUnixMs":now.saturating_add(POST_MS),"preFrames":self.frames.len()}),
            ];
            result.extend(self.frames.drain(..));
            result
        } else {
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn real_file_writer_locks_and_flushes_session() {
        let path = std::env::temp_dir().join(format!("mir2-render-writer-{}-{}.jsonl", std::process::id(), unix_ms()));
        let (sender, receiver) = mpsc::sync_channel(1);
        drop(sender);
        run_writer(&path, receiver).expect("real platform file locking and write must succeed");
        let contents = std::fs::read_to_string(&path).unwrap();
        std::fs::remove_file(&path).unwrap();
        let session: Value = serde_json::from_str(contents.trim()).unwrap();
        assert_eq!(session["type"], "renderTraceSession");
    }

    #[test]
    fn file_budget_records_stop_reason_without_exceeding_limit() {
        let mut writer = BudgetWriter {
            writer: Vec::new(),
            written: 0,
            limit: 1024,
        };
        writer.record(&json!({"type":"small"})).unwrap();
        assert!(writer.record(&json!({"large":"x".repeat(600)})).is_err());
        assert!(writer.written <= 1024);
        let text = String::from_utf8(writer.writer).unwrap();
        let stopped: Value = serde_json::from_str(text.lines().last().unwrap()).unwrap();
        assert_eq!(stopped["reason"], "fileBudgetReached");
        assert_eq!(stopped["processId"], std::process::id());
    }
    #[test]
    fn existing_full_file_is_not_extended_and_io_errors_propagate() {
        let mut full = BudgetWriter {
            writer: Vec::new(),
            written: 1024,
            limit: 1024,
        };
        assert!(full.record(&json!({"type":"small"})).is_err());
        assert!(full.writer.is_empty());
        struct FailedDisk;
        impl Write for FailedDisk {
            fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
                Err(std::io::Error::other("test disk failure"))
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let mut failed = BudgetWriter {
            writer: FailedDisk,
            written: 0,
            limit: 1024,
        };
        assert!(failed
            .record(&json!({"type":"small"}))
            .unwrap_err()
            .to_string()
            .contains("test disk failure"));
    }
    fn command(time: u64, from: [i32; 2], to: [i32; 2]) -> Value {
        json!({"type":"renderMarker","kind":"movement","unixMs":time,"marker":{
            "type":"commandSent","fromX":from[0],"fromY":from[1],"toX":to[0],"toY":to[1]}})
    }
    fn committed(time: u64, map: [i32; 2], camera: [f32; 2]) -> Value {
        json!({"type":"renderFrame","unixMs":time,"appliedMapCenter":map,"cameraGlobalTransform":[camera[0],camera[1],0],
            "boundarySuppressed":false,"selfAnchorDriftPx":0,"anomalies":[]})
    }
    #[test]
    fn command_correlation_recenter_preserves_world_position_without_alarm() {
        let mut tracker = MovementCorrelation::default();
        tracker.observe(&mut command(100, [10, 10], [12, 12]));
        let mut source = committed(600, [10, 10], [96.0, -64.0]);
        tracker.observe(&mut source);
        let mut target = committed(700, [12, 12], [0.0, 0.0]);
        tracker.observe(&mut target);
        assert_eq!(
            source["visualWorldCenterStagePx"],
            target["visualWorldCenterStagePx"]
        );
        assert!(target["anomalies"].as_array().unwrap().is_empty());
        assert_eq!(target["movementCorrelation"]["reverseStagePx"], 0.0);
    }
    #[test]
    fn command_correlation_detects_world_pullback_even_when_self_anchor_stays_locked() {
        let mut tracker = MovementCorrelation::default();
        tracker.observe(&mut command(100, [10, 10], [12, 10]));
        tracker.observe(&mut committed(500, [10, 10], [80.0, 0.0]));
        let mut reversed = committed(520, [10, 10], [64.0, 0.0]);
        tracker.observe(&mut reversed);
        assert_eq!(reversed["selfAnchorDriftPx"], 0);
        assert_eq!(reversed["movementCorrelation"]["reverseStagePx"], 16.0);
        assert!(reversed["anomalies"]
            .as_array()
            .unwrap()
            .contains(&json!("presentationReverseCandidate")));
    }
    #[test]
    fn command_correlation_resets_on_turn_correction_focus_and_new_command() {
        for marker in [
            json!({"type":"turn"}),
            json!({"type":"authoritative","tsDisposition":"correction"}),
            json!({"type":"authoritative","tsDisposition":"degraded"}),
            json!({"type":"focusChanged"}),
            json!({"type":"reset"}),
            json!({"type":"mapChanged"}),
        ] {
            let mut tracker = MovementCorrelation::default();
            tracker.observe(&mut command(100, [10, 10], [12, 10]));
            tracker.observe(&mut committed(500, [10, 10], [80.0, 0.0]));
            tracker.observe(
                &mut json!({"type":"renderMarker","kind":"movement","unixMs":510,"marker":marker}),
            );
            let mut corrected = committed(520, [10, 10], [0.0, 0.0]);
            tracker.observe(&mut corrected);
            assert!(corrected.get("movementCorrelation").is_none());
            assert!(corrected["anomalies"].as_array().unwrap().is_empty());
        }
        let mut tracker = MovementCorrelation::default();
        tracker.observe(&mut command(100, [10, 10], [12, 10]));
        tracker.observe(&mut committed(500, [12, 10], [0.0, 0.0]));
        tracker.observe(&mut command(600, [12, 10], [10, 10]));
        let mut turn = committed(700, [12, 10], [-16.0, 0.0]);
        tracker.observe(&mut turn);
        assert!(turn["anomalies"].as_array().unwrap().is_empty());
    }
    fn frame(t: u64, bad: bool) -> Value {
        json!({"type":"renderFrame","unixMs":t,"anomalies":if bad {vec!["slow"]}else{vec![]}})
    }
    #[test]
    fn ring_bounds_preroll_and_fixed_postroll_without_extending_on_anomalies() {
        let mut ring = CaptureRing::default();
        for t in 0..2000 {
            assert!(ring.push(frame(t, false)).is_empty());
        }
        assert_eq!(ring.frames.len(), MAX_FRAMES);
        let capture = ring.push(frame(2000, true));
        assert_eq!(capture.len(), MAX_FRAMES + 1);
        assert_eq!(capture[1]["unixMs"], json!(801));
        assert_eq!(ring.push(frame(3999, true)).len(), 1);
        assert!(ring.push(frame(4001, true)).is_empty());
        assert!(ring.until.is_none());
    }
    #[test]
    fn time_bound_discards_old_frames_even_below_frame_cap() {
        let mut ring = CaptureRing::default();
        ring.push(frame(1, false));
        ring.push(frame(11000, false));
        let capture = ring.push(frame(11001, true));
        assert_eq!(capture.len(), 3);
        assert_eq!(capture[1]["unixMs"], json!(11000));
    }
    #[test]
    fn thresholds_are_inclusive_and_boundary_can_omit_anchor_alarm() {
        assert!(anomalies(Some(32.99), false, None).is_empty());
        assert_eq!(anomalies(Some(100.0), true, Some(5.0)).len(), 5);
        assert_eq!(anomalies(Some(50.0), false, None).len(), 2);
        assert!(anomalies(None, false, Some(4.0)).is_empty());
    }
    #[test]
    fn healthy_frames_still_emit_summary_and_markers_do_not_count_as_frames() {
        let mut health = HealthWindow::default();
        assert!(health
            .observe(&json!({"type":"renderMarker","unixMs":0}))
            .is_none());
        assert!(health
            .observe(&json!({"type":"renderFrame","unixMs":1,"frameWallDeltaMs":16}))
            .is_none());
        let report=health.observe(&json!({"type":"renderFrame","unixMs":10001,"frameWallDeltaMs":20,"droppedRecords":2})).unwrap();
        assert_eq!(report["frames"], 2);
        assert_eq!(report["meanFrameWallDeltaMs"], 18.0);
        assert_eq!(report["droppedRecords"], 2);
    }
}
