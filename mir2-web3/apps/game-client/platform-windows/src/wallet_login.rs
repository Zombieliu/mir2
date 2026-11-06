//! Native Dubhe Wallet worker and main-thread login lifecycle.
//!
//! Only the worker owns the private HTTP intent. The UI receives a public QR
//! image, a countdown and fixed messages; signed credentials go to the gateway.

use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use bevy::{
    asset::RenderAssetUsages,
    image::ImageSampler,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use mir2_client_bevy::{
    native_shell::{NativeShellModel, NativeShellScreen},
    native_wallet_login::{
        NativeWalletLogin, NativeWalletLoginAction, NativeWalletLoginActionQueue,
        NativeWalletLoginMessage, NativeWalletLoginStage,
    },
};
use qrcode::{Color, QrCode};

use crate::{
    gateway::NativeCommandStamp,
    input::GatewayCommands,
    shell_bridge::send_verified_identity_login,
    wallet_login_http::{NativeWalletAuthConfig, VerifiedWalletLogin, WalletLoginPoll},
};

const POLL_INTERVAL: Duration = Duration::from_millis(1_500);
const MAX_ATTEMPT_LIFETIME: Duration = Duration::from_secs(300);
const QR_QUIET_ZONE: usize = 4;

pub struct NativeWalletLoginPlugin {
    pub config: Option<NativeWalletAuthConfig>,
}

impl Plugin for NativeWalletLoginPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<NativeWalletLogin>()
            .init_resource::<NativeWalletLoginActionQueue>()
            .init_resource::<Assets<Image>>()
            .insert_resource(WalletLoginRuntime::new(self.config.clone()))
            .add_systems(PostUpdate, drive_wallet_login);
    }
}

// No Debug implementation: worker messages can contain a gateway credential.
struct WalletJob {
    id: u64,
    cancel: Arc<AtomicBool>,
}

struct WorkerEvent {
    id: u64,
    outcome: WorkerOutcome,
}

enum WorkerOutcome {
    Created {
        approval_url: String,
        expires_at_ms: u64,
    },
    Approved(VerifiedWalletLogin),
    Rejected,
    Expired,
    Error,
}

struct WalletAttempt {
    id: u64,
    stamp: Option<NativeCommandStamp>,
    cancel: Arc<AtomicBool>,
    started_at: Instant,
    expires_at_ms: Option<u64>,
    expires_at: Option<Instant>,
}

impl WalletAttempt {
    fn remaining(&self) -> Duration {
        let lifetime = MAX_ATTEMPT_LIFETIME.saturating_sub(self.started_at.elapsed());
        let monotonic = self
            .expires_at
            .map(|deadline| deadline.saturating_duration_since(Instant::now()))
            .unwrap_or(lifetime);
        let wall = self
            .expires_at_ms
            .map(|expiry| Duration::from_millis(expiry.saturating_sub(unix_ms())))
            .unwrap_or(monotonic);
        lifetime.min(monotonic).min(wall)
    }
}

impl Drop for WalletAttempt {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Release);
    }
}

#[derive(Resource)]
struct WalletLoginRuntime {
    jobs: Option<SyncSender<WalletJob>>,
    events: Mutex<Receiver<WorkerEvent>>,
    active: Option<WalletAttempt>,
    // Retain ownership independently of the view; UI Refresh replaces its state.
    qr_image: Option<Handle<Image>>,
    next_id: Option<u64>,
    qa_start_once: bool,
}

impl WalletLoginRuntime {
    fn new(config: Option<NativeWalletAuthConfig>) -> Self {
        let (event_tx, event_rx) = mpsc::sync_channel(8);
        let jobs = config.and_then(|config| {
            let (job_tx, job_rx) = mpsc::sync_channel::<WalletJob>(1);
            thread::Builder::new()
                .name("mir2-wallet-login".into())
                .spawn(move || wallet_worker(config, job_rx, event_tx))
                .ok()
                .map(|_| job_tx)
        });
        Self {
            jobs,
            events: Mutex::new(event_rx),
            active: None,
            qr_image: None,
            next_id: Some(1),
            qa_start_once: qa_auto_start_enabled(
                std::env::var("MIR2_NATIVE_WALLET_QA_START").ok().as_deref(),
                std::env::var("MIR2_NATIVE_CAPTURE_DIR").ok().as_deref(),
            ),
        }
    }

    fn remove_image(&mut self, view: &mut NativeWalletLogin, images: &mut Assets<Image>) {
        if let Some(image) = self.qr_image.take() {
            images.remove(image.id());
        }
        view.qr_image = None;
    }

    fn retire(&mut self, view: &mut NativeWalletLogin, images: &mut Assets<Image>) {
        self.active = None;
        self.remove_image(view, images);
        *view = NativeWalletLogin::default();
    }

    fn terminal(
        &mut self,
        view: &mut NativeWalletLogin,
        images: &mut Assets<Image>,
        stage: NativeWalletLoginStage,
        message: NativeWalletLoginMessage,
    ) {
        self.active = None;
        self.remove_image(view, images);
        view.visible = true;
        view.stage = stage;
        view.remaining_seconds = 0;
        view.message = message;
    }

    fn begin(
        &mut self,
        view: &mut NativeWalletLogin,
        images: &mut Assets<Image>,
        commands: &GatewayCommands,
    ) {
        self.retire(view, images);
        *view = NativeWalletLogin {
            visible: true,
            message: NativeWalletLoginMessage::Creating,
            ..default()
        };
        let Some(id) = self.next_id else {
            self.terminal(
                view,
                images,
                NativeWalletLoginStage::Error,
                NativeWalletLoginMessage::RestartRequired,
            );
            return;
        };
        self.next_id = id.checked_add(1);
        let Some(jobs) = &self.jobs else {
            self.terminal(
                view,
                images,
                NativeWalletLoginStage::Error,
                NativeWalletLoginMessage::ServiceUnavailable,
            );
            return;
        };
        let stamp = current_stamp(commands);
        if !stamp_is_current(commands, stamp) {
            self.terminal(
                view,
                images,
                NativeWalletLoginStage::Error,
                NativeWalletLoginMessage::ConnectionChanged,
            );
            return;
        }
        let cancel = Arc::new(AtomicBool::new(false));
        let attempt = WalletAttempt {
            id,
            stamp,
            cancel: cancel.clone(),
            started_at: Instant::now(),
            expires_at_ms: None,
            expires_at: None,
        };
        if jobs.try_send(WalletJob { id, cancel }).is_err() {
            self.terminal(
                view,
                images,
                NativeWalletLoginStage::Error,
                NativeWalletLoginMessage::Cancelling,
            );
            return;
        }
        self.active = Some(attempt);
    }

    fn active_is_current(
        &self,
        shell: &NativeShellModel,
        view: &NativeWalletLogin,
        commands: &GatewayCommands,
    ) -> bool {
        self.active.as_ref().is_some_and(|attempt| {
            view.visible
                && shell.screen == NativeShellScreen::Login
                && !shell.login_request_in_flight
                && !attempt.cancel.load(Ordering::Acquire)
                && stamp_is_current(commands, attempt.stamp)
        })
    }
}

fn qa_auto_start_enabled(flag: Option<&str>, capture_dir: Option<&str>) -> bool {
    flag == Some("1") && capture_dir.is_some_and(|value| !value.trim().is_empty())
}

fn current_stamp(commands: &GatewayCommands) -> Option<NativeCommandStamp> {
    commands.ownership_fence().and_then(|fence| fence.stamp())
}

fn stamp_is_current(commands: &GatewayCommands, captured: Option<NativeCommandStamp>) -> bool {
    commands
        .ownership_fence()
        .map_or(captured.is_none(), |fence| {
            captured.is_some() && fence.stamp() == captured
        })
}

fn unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or(0)
}

fn cancel_aware_wait(cancel: &AtomicBool, duration: Duration) -> bool {
    let deadline = Instant::now() + duration;
    while !cancel.load(Ordering::Acquire) {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return true;
        }
        thread::sleep(remaining.min(Duration::from_millis(50)));
    }
    false
}

fn publish(events: &SyncSender<WorkerEvent>, job: &WalletJob, outcome: WorkerOutcome) -> bool {
    !job.cancel.load(Ordering::Acquire)
        && events
            .try_send(WorkerEvent {
                id: job.id,
                outcome,
            })
            .is_ok()
}

fn wallet_worker(
    config: NativeWalletAuthConfig,
    jobs: Receiver<WalletJob>,
    events: SyncSender<WorkerEvent>,
) {
    while let Ok(job) = jobs.recv() {
        if job.cancel.load(Ordering::Acquire) {
            continue;
        }
        let created = match config.create(&job.cancel) {
            Ok(created) => created,
            Err(_) => {
                publish(&events, &job, WorkerOutcome::Error);
                continue;
            }
        };
        if !publish(
            &events,
            &job,
            WorkerOutcome::Created {
                approval_url: created.approval_url.clone(),
                expires_at_ms: created.expires_at_ms,
            },
        ) {
            continue;
        }
        let started_at = Instant::now();
        while cancel_aware_wait(&job.cancel, POLL_INTERVAL) {
            if unix_ms() >= created.expires_at_ms || started_at.elapsed() >= MAX_ATTEMPT_LIFETIME {
                publish(&events, &job, WorkerOutcome::Expired);
                break;
            }
            let terminal = match config.poll(&created, &job.cancel) {
                Ok(WalletLoginPoll::Pending) => continue,
                Ok(WalletLoginPoll::Rejected) => WorkerOutcome::Rejected,
                Ok(WalletLoginPoll::Expired) => WorkerOutcome::Expired,
                Ok(WalletLoginPoll::Approved(login)) => WorkerOutcome::Approved(login),
                Err(_) => WorkerOutcome::Error,
            };
            publish(&events, &job, terminal);
            break;
        }
    }
}

fn qr_image(approval_url: &str) -> Result<Image, ()> {
    let code = QrCode::new(approval_url.as_bytes()).map_err(|_| ())?;
    let modules = code.width();
    let side = modules + QR_QUIET_ZONE * 2;
    let mut pixels = vec![255; side * side * 4];
    for y in 0..modules {
        for x in 0..modules {
            if code[(x, y)] == Color::Dark {
                let offset = ((y + QR_QUIET_ZONE) * side + x + QR_QUIET_ZONE) * 4;
                pixels[offset..offset + 3].fill(0);
            }
        }
    }
    let mut image = Image::new(
        Extent3d {
            width: side as u32,
            height: side as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        pixels,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    image.sampler = ImageSampler::nearest();
    Ok(image)
}

// PostUpdate runs after the shell's Update input processing, so explicit Cancel
// and password login invalidate a QR operation before a worker result is admitted.
fn drive_wallet_login(
    mut runtime: ResMut<WalletLoginRuntime>,
    mut view: ResMut<NativeWalletLogin>,
    mut queue: ResMut<NativeWalletLoginActionQueue>,
    mut images: ResMut<Assets<Image>>,
    mut shell: ResMut<NativeShellModel>,
    commands: Res<GatewayCommands>,
) {
    if shell.screen != NativeShellScreen::Login || shell.login_request_in_flight {
        runtime.retire(&mut view, &mut images);
        queue.drain().for_each(drop);
    } else {
        if runtime.active.is_some() && !runtime.active_is_current(&shell, &view, &commands) {
            runtime.retire(&mut view, &mut images);
        }
        let mut actions = queue.drain().collect::<Vec<_>>();
        if runtime.qa_start_once && !view.visible {
            runtime.qa_start_once = false;
            actions.push(NativeWalletLoginAction::Begin);
        }
        for action in actions {
            match action {
                NativeWalletLoginAction::Cancel => runtime.retire(&mut view, &mut images),
                NativeWalletLoginAction::Begin if runtime.active.is_none() => {
                    runtime.begin(&mut view, &mut images, &commands)
                }
                NativeWalletLoginAction::Refresh => {
                    runtime.begin(&mut view, &mut images, &commands)
                }
                NativeWalletLoginAction::Begin => {}
            }
        }
    }

    if let Some(attempt) = runtime.active.as_ref() {
        let remaining = attempt.remaining();
        if remaining.is_zero() {
            runtime.terminal(
                &mut view,
                &mut images,
                NativeWalletLoginStage::Expired,
                NativeWalletLoginMessage::Expired,
            );
        } else if attempt.expires_at_ms.is_some() {
            view.remaining_seconds =
                u32::try_from(remaining.as_millis().div_ceil(1_000)).unwrap_or(u32::MAX);
        }
    }

    let events = runtime
        .events
        .lock()
        .map(|events| events.try_iter().collect::<Vec<_>>())
        .unwrap_or_default();
    for event in events {
        if !runtime
            .active
            .as_ref()
            .is_some_and(|attempt| attempt.id == event.id)
            || !runtime.active_is_current(&shell, &view, &commands)
        {
            continue;
        }
        match event.outcome {
            WorkerOutcome::Created {
                approval_url,
                expires_at_ms,
            } => {
                let remaining = Duration::from_millis(expires_at_ms.saturating_sub(unix_ms()));
                if remaining.is_zero() || remaining > MAX_ATTEMPT_LIFETIME {
                    runtime.terminal(
                        &mut view,
                        &mut images,
                        NativeWalletLoginStage::Expired,
                        NativeWalletLoginMessage::Expired,
                    );
                    continue;
                }
                let Ok(image) = qr_image(&approval_url) else {
                    runtime.terminal(
                        &mut view,
                        &mut images,
                        NativeWalletLoginStage::Error,
                        NativeWalletLoginMessage::QrGenerationFailed,
                    );
                    continue;
                };
                runtime.remove_image(&mut view, &mut images);
                let handle = images.add(image);
                runtime.qr_image = Some(handle.clone());
                view.qr_image = Some(handle);
                view.stage = NativeWalletLoginStage::Pending;
                view.remaining_seconds =
                    u32::try_from(remaining.as_millis().div_ceil(1_000)).unwrap_or(u32::MAX);
                view.message = NativeWalletLoginMessage::ConfirmOnPhone;
                if let Some(attempt) = runtime.active.as_mut() {
                    attempt.expires_at_ms = Some(expires_at_ms);
                    attempt.expires_at = Some(Instant::now() + remaining);
                }
            }
            WorkerOutcome::Approved(login) => {
                // Recheck on this frame immediately before the irreversible queue
                // admission. A token is never replayed after cancel or retry.
                if login.expires_at_ms <= unix_ms()
                    || runtime
                        .active
                        .as_ref()
                        .is_none_or(|attempt| attempt.remaining().is_zero())
                {
                    runtime.terminal(
                        &mut view,
                        &mut images,
                        NativeWalletLoginStage::Expired,
                        NativeWalletLoginMessage::CredentialExpired,
                    );
                    continue;
                }
                if runtime.active_is_current(&shell, &view, &commands) {
                    send_verified_identity_login(
                        &commands,
                        &mut shell,
                        login.account_id,
                        login.token,
                    );
                    runtime.retire(&mut view, &mut images);
                }
            }
            WorkerOutcome::Rejected => runtime.terminal(
                &mut view,
                &mut images,
                NativeWalletLoginStage::Rejected,
                NativeWalletLoginMessage::Rejected,
            ),
            WorkerOutcome::Expired => runtime.terminal(
                &mut view,
                &mut images,
                NativeWalletLoginStage::Expired,
                NativeWalletLoginMessage::Expired,
            ),
            WorkerOutcome::Error => runtime.terminal(
                &mut view,
                &mut images,
                NativeWalletLoginStage::Error,
                NativeWalletLoginMessage::Retry,
            ),
        }
    }
}

#[cfg(test)]
#[path = "wallet_login_tests.rs"]
mod tests;
