//! Native Windows desktop host for the mir2-web3 Bevy client.
//!
//! M2 slice: open a real OS window running the same shared Bevy app the Web
//! client uses (no DOM/canvas assumptions), and connect to the same Gateway via
//! the WebSocket protocol the browser speaks. Authority stays in Simulation and
//! the Gateway; this crate only owns window/lifecycle/input hosting and forwards
//! world snapshots into the shared runtime.

use bevy::prelude::IntoScheduleConfigs;
use mir2_bevy_runtime::{build_runtime_app, RuntimeWindowSpec};

mod assets;
mod atlas;
mod capture;
mod clipboard;
mod cursor;
mod effects;
mod entity_overlays;
mod entity_presentation;
mod frame_sets;
mod gameplay_bridge;
mod gateway;
mod input;
mod map_parser;
mod movement_trace;
mod native_protocol;
mod session_config;
mod shell_bridge;

/// Whether an effect frame PNG (a web path like /original-effects/Magic/0.png)
/// exists under the native asset root. Used by effects.rs so a missing asset
/// never produces a sprite or a fake stand-in.
fn frame_png_exists(web_path: &str) -> bool {
    assets::asset_path(web_path).is_some_and(|path| path.is_file())
}

#[derive(bevy::prelude::Resource, Default)]
struct R2ChatReporter {
    timer: Option<bevy::prelude::Timer>,
    last_local: usize,
    last_r2: usize,
}

fn report_r2_progress_via_chat(
    time: bevy::prelude::Res<bevy::prelude::Time>,
    mut reporter: bevy::prelude::ResMut<R2ChatReporter>,
    mut chat: bevy::prelude::ResMut<mir2_client_bevy::chat::ChatModel>,
) {
    // Bichon town batch: fire-and-forget every tick so the 30-page lattice
    // goes from per-page on-demand (首帧黑) to once批补 (首进城镇一次 30 并发).
    crate::assets::batch_prefetch_bichon_town();
    let timer = reporter.timer.get_or_insert_with(|| {
        // First report after 5s so the player sees it quickly, then every 30s.
        bevy::prelude::Timer::from_seconds(5.0, bevy::prelude::TimerMode::Repeating)
    });
    let finished = timer.tick(time.delta()).just_finished();
    // After the first 5s, switch to 30s cadence.
    if timer.duration().as_secs_f32() < 29.0 && finished {
        *timer = bevy::prelude::Timer::from_seconds(30.0, bevy::prelude::TimerMode::Repeating);
    } else if !finished {
        return;
    }
    let (local, r2) = assets::asset_hit_stats();
    let cached_files = std::fs::read_dir(assets::r2_cache_dir())
        .map(|rd| rd.count())
        .unwrap_or(0);
    let has_full = assets::has_local_full();
    let is_remote_active = r2 > 0 || cached_files > 0;
    let msg = if has_full {
        format!("[Assets] Local {local} / Remote {r2} -- Full ready (Bichon town local, cache {cached_files} files)")
    } else if !is_remote_active {
        format!("[Assets] Local {local} / Remote 0 -- Starter only, Bichon town needs full/R2 (cache {cached_files} files)")
    } else if r2 == 0 && cached_files > 0 {
        format!(
            "[Assets] Local {local} / Remote 0 (cache {cached_files} files ready, no new R2 fetch)"
        )
    } else if r2 > 0 && local > 0 {
        format!("[Assets] Local {local} / Remote {r2} (cache {cached_files} files) -- town tiles streaming from R2, first flash then local")
    } else {
        format!(
            "[Assets] Remote {r2} / Local {local}, cache {cached_files} files -- streaming town"
        )
    };
    chat.push(mir2_client_bevy::chat::ChatLine {
        text: msg,
        channel: "system".to_owned(),
    });
    reporter.last_local = local;
    reporter.last_r2 = r2;
}

fn main() {
    console_error_panic_hook::set_once();
    movement_trace::initialize();

    let session = session_config::NativeSessionConfig::load(gateway::LOCAL_GATEWAY_WS_URL)
        .unwrap_or_else(|error| {
            eprintln!("[platform-windows] configuration error: {error}");
            std::process::exit(2);
        });

    // Cross-thread channels: Bevy owns visible UI state on the main thread; one
    // async task exclusively owns the WebSocket.
    let (command_tx, command_rx) = gateway::command_channel(256);
    let (event_tx, event_rx) =
        std::sync::mpsc::channel::<mir2_client_bevy::native_shell::NativeGatewayEvent>();
    let (gameplay_tx, gameplay_rx) =
        std::sync::mpsc::channel::<gameplay_bridge::NativeGameplaySnapshot>();

    // The shared Bevy app. Native window spec: opaque, real OS window. The
    // AssetServer resolves atlas image paths relative to the repo
    // `apps/web/public` (where `generated/map-atlas/...` and
    // `bevy-entity-atlases/...` live).
    let asset_root = assets::require_asset_root().unwrap_or_else(|error| {
        eprintln!("[platform-windows] FATAL: {error}");
        std::process::exit(1);
    });
    let diag = assets::diagnose_asset_root(&asset_root);
    eprintln!(
        "[platform-windows] asset_root={} has_entity_manifest={} has_map_manifest={} has_native_map_keyed_manifest={} has_effect_manifest={} has_crystal_cursors={} complete={}",
        asset_root.display(),
        diag.has_entity_manifest,
        diag.has_map_manifest,
        diag.has_native_map_keyed_manifest,
        diag.has_effect_manifest,
        diag.has_crystal_cursors,
        diag.is_complete
    );
    eprintln!(
        "[platform-windows] gateway_url={} window={}x{}",
        session.gateway_url, session.window_width, session.window_height
    );
    let mut app = build_runtime_app(RuntimeWindowSpec {
        asset_root: asset_root.to_string_lossy().into_owned(),
        width: session.window_width,
        height: session.window_height,
        ..RuntimeWindowSpec::native("mir2-web3 (native)")
    });
    app.world_mut()
        .resource_mut::<mir2_bevy_runtime::PresentationPoseBuffer>()
        .set_native_consumer_enabled(true);
    // Windows emits the command before the authoritative ACK, so let the
    // phase-latched local presentation own the self camera for the complete
    // 6 x 100 ms Crystal movement. The wall-clock fallback window remains in
    // place for corrections and path mismatches, but cannot collapse normal
    // moves into whole-cell camera jumps when the map center commits early.
    mir2_bevy_runtime::set_mir2_local_motion_presentation_enabled(true);
    // Load the local starter entity atlas (if present) so entities render real
    // sprite pixels instead of the colored fallback. Must run after the app is
    // built so the runtime's native ingestion channel is registered; the runtime
    // drains it once `app.run()` starts.
    let _ = atlas::load_starter_entity_atlas();
    if std::env::var_os("MIR2_NATIVE_TRACE_RENDER").is_some() {
        app.init_resource::<atlas::NativeRenderTrace>();
        app.add_systems(bevy::app::Update, atlas::trace_rendered_entity_sprites);
    }
    // Map/entity fallback and atlas rendering are already owned by the shared
    // runtime. Registering the client-bevy fallback plugins here rendered a
    // second copy on top of that scene.
    // A real Bevy UI shell (not DOM/WebView) owns login, character selection,
    // character creation, connection errors, and the transition into the game.
    app.add_plugins(mir2_client_bevy::native_shell_ui::Mir2NativeShellUiPlugin);
    // Native-only Crystal presentation consumes the existing authoritative
    // read models. It is registered only by this Windows host; Web/WASM keeps
    // its own renderer and shared runtime behavior remains unchanged.
    app.add_plugins(mir2_client_bevy::crystal_ui::minimap::Mir2CrystalMiniMapPlugin);
    app.add_plugins(mir2_client_bevy::crystal_ui::hud::Mir2CrystalHudPlugin);
    app.add_plugins(mir2_client_bevy::crystal_ui::chat::Mir2CrystalChatPlugin);
    app.add_plugins(mir2_client_bevy::crystal_ui::notice::Mir2CrystalNoticePlugin);
    // Quest/NPC/target/drop intent handling remains in the established plugin;
    // its former player/control/bag placeholder panels are suppressed once the
    // Crystal MainDialog owns those regions.
    app.add_plugins(mir2_client_bevy::quest_ui::Mir2QuestUiPlugin);
    // F12 and optional state-triggered PNG capture are disabled unless an
    // explicit capture directory is configured for native acceptance.
    app.add_plugins(capture::Mir2NativeScreenshotPlugin);
    // Native input: WASD / arrows → walk/run intents forwarded to the gateway.
    app.insert_resource(shell_bridge::initial_shell_model(
        session.auto_login.as_ref(),
    ));
    app.insert_resource(mir2_client_bevy::native_shell::NativeUiIntentQueue::default());
    app.insert_resource(shell_bridge::GatewayEventInbox::new(event_rx));
    app.insert_resource(gameplay_bridge::GameplayEventInbox::new(gameplay_rx));
    // The Big Map model and its bounded request queue are renderer-neutral.
    // Crystal UI wiring can consume these resources later without owning
    // gateway state or fabricating transport results.
    app.init_resource::<mir2_client_bevy::big_map::BigMapModel>();
    app.init_resource::<mir2_client_bevy::big_map::BigMapGatewayIntentQueue>();
    app.init_resource::<entity_presentation::NativeEntityPresentation>();
    app.init_resource::<entity_overlays::NativeEntityOverlays>();
    app.init_resource::<effects::NativeEffects>();
    app.init_resource::<input::WorldPointerMovementState>();
    app.insert_resource(shell_bridge::NativeAutoLoginFlow::from_config(
        session.auto_login.as_ref(),
    ));
    app.insert_resource(input::GatewayCommands::new(command_tx.clone()));
    app.add_systems(bevy::app::Startup, cursor::load_native_crystal_cursors);
    // Native packet production and held-pointer movement must precede the
    // shared Runtime's Update schedule. Previously these systems were appended
    // after build_runtime_app(), so a movement boundary was rendered once with
    // the expired camera window before the next command-time window arrived.
    // At Crystal's 6 x 100 ms cadence that one-frame producer lag repeated on
    // every step and made the terrain visibly chase sustained running.
    app.add_systems(
        bevy::app::PreUpdate,
        (
            shell_bridge::drain_gateway_events,
            gameplay_bridge::drain_gameplay_events,
            entity_presentation::tick_native_entity_presentation,
            input::sanitize_native_hud_pointer_input,
            input::mouse_world_interaction_system,
            input::keyboard_movement_system,
        )
            .chain()
            .after(bevy::input::InputSystems),
    );
    // Ctrl+V is owned by the focused native text field. The host reads the
    // Windows Unicode clipboard only for that explicit shortcut; ordinary
    // key/text input remains owned by the shared shell/overlay systems.
    app.add_systems(
        bevy::app::Update,
        clipboard::paste_system.before(mir2_client_bevy::crystal_ui::NativePlayerUiSet::Mutate),
    );
    app.add_systems(
        bevy::app::Update,
        cursor::sync_native_crystal_cursor
            .after(mir2_client_bevy::crystal_ui::NativePlayerUiSet::Mutate),
    );
    app.add_systems(
        bevy::app::Update,
        (
            shell_bridge::forward_native_ui_intents,
            gameplay_bridge::forward_quest_ui_intents,
            input::keyboard_turn_system,
            input::keyboard_town_revive_system,
            input::keyboard_skill_system,
        )
            .after(mir2_client_bevy::crystal_ui::NativePlayerUiSet::Mutate),
    );
    app.add_systems(
        bevy::app::Update,
        (
            entity_overlays::sync_native_entity_overlays
                .after(mir2_bevy_runtime::RuntimePresentationSet),
            effects::tick_native_effects,
            mir2_client_bevy::audio::sync_native_gameplay_audio
                .after(mir2_client_bevy::crystal_ui::NativePlayerUiSet::Mutate),
        )
            .chain(),
    );
    // Flush packet-authoritative map state first. A same-frame map reset must
    // invalidate stale NPC object ids before any queued Big Map request can
    // cross the Windows gateway boundary.
    app.add_systems(bevy::app::Update, gameplay_bridge::forward_big_map_intents);
    // R2 on-demand progress: local-first, remote fallback. Report every 30s
    // in the in-game chat so the player knows whether Bichon town tiles are
    // still streaming from R2 (e.g. 284,621) or already local.
    app.insert_resource(R2ChatReporter::default());
    app.add_systems(bevy::app::Update, report_r2_progress_via_chat);

    eprintln!("[platform-windows] native window opened; runtime running");

    let gateway_runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    let gateway_url = session.gateway_url;
    let gateway_task = gateway_runtime.spawn(async move {
        match gateway::run_gateway_client(
            &gateway_url,
            command_rx,
            event_tx,
            gameplay_tx,
            session.reconnect,
        )
        .await
        {
            Ok(()) => eprintln!("[platform-windows] gateway session ended"),
            Err(error) => eprintln!("[platform-windows] gateway session error: {error}"),
        }
    });

    // Run the Bevy loop on the main thread; the gateway task runs on its own
    // tokio runtime in the background and pushes snapshots into the runtime
    // channel. The runtime handle stays alive until the window closes.
    app.run();

    // Best-effort: drop the gateway task after the window closes.
    let _ = command_tx.send(gateway::GatewayCommand::Shutdown);
    let _ = gateway_task.abort();
}
