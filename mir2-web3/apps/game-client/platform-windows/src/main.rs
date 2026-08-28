//! Native Windows desktop host for the mir2-web3 Bevy client.
//!
//! M2 slice: open a real OS window running the same shared Bevy app the Web
//! client uses (no DOM/canvas assumptions), and connect to the same Gateway via
//! the WebSocket protocol the browser speaks. Authority stays in Simulation and
//! the Gateway; this crate only owns window/lifecycle/input hosting and forwards
//! world snapshots into the shared runtime.

use bevy::ecs::schedule::SingleThreadedExecutor;
use bevy::prelude::IntoScheduleConfigs;
use mir2_bevy_runtime::{build_runtime_app, RuntimeWindowSpec};

mod assets;
mod atlas;
mod capture;
mod effects;
mod entity_overlays;
mod entity_presentation;
mod frame_sets;
mod gameplay_bridge;
mod gateway;
mod input;
mod map_parser;
mod native_protocol;
mod session_config;
mod shell_bridge;

/// Whether an effect frame PNG (a web path like /original-effects/Magic/0.png)
/// exists under the native asset root. Used by effects.rs so a missing asset
/// never produces a sprite or a fake stand-in.
fn frame_png_exists(web_path: &str) -> bool {
    assets::asset_path(web_path).is_some_and(|path| path.is_file())
}

fn main() {
    console_error_panic_hook::set_once();

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
        "[platform-windows] asset_root={} has_entity_manifest={} has_map_manifest={} has_effect_manifest={} complete={}",
        asset_root.display(),
        diag.has_entity_manifest,
        diag.has_map_manifest,
        diag.has_effect_manifest,
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
    app.edit_schedule(bevy::prelude::Update, |schedule| {
        schedule.set_executor(SingleThreadedExecutor::new());
    });

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
    // Apply network session transitions before UI or world input. A queued
    // disconnect/login transition must win over a same-frame D/click event.
    app.add_systems(
        bevy::app::Update,
        shell_bridge::drain_gateway_events
            .before(mir2_client_bevy::crystal_ui::NativePlayerUiSet::Mutate),
    );
    app.add_systems(
        bevy::app::Update,
        input::sanitize_native_hud_pointer_input
            .after(shell_bridge::drain_gateway_events)
            .before(mir2_client_bevy::crystal_ui::NativePlayerUiSet::Mutate),
    );
    app.add_systems(
        bevy::app::Update,
        input::mouse_world_interaction_system
            .after(entity_presentation::tick_native_entity_presentation)
            .before(gameplay_bridge::forward_quest_ui_intents)
            .after(mir2_client_bevy::crystal_ui::NativePlayerUiSet::Mutate),
    );
    app.add_systems(
        bevy::app::Update,
        (
            shell_bridge::forward_native_ui_intents,
            gameplay_bridge::forward_quest_ui_intents,
            input::keyboard_walk_system,
            input::keyboard_run_system,
            input::keyboard_turn_system,
            input::keyboard_town_revive_system,
            input::keyboard_skill_system,
        )
            .after(mir2_client_bevy::crystal_ui::NativePlayerUiSet::Mutate),
    );
    app.add_systems(
        bevy::app::Update,
        (
            gameplay_bridge::drain_gameplay_events
                .before(mir2_client_bevy::crystal_ui::NativePlayerUiSet::Mutate),
            entity_presentation::tick_native_entity_presentation,
            entity_overlays::sync_native_entity_overlays,
            effects::tick_native_effects,
            mir2_client_bevy::audio::sync_native_gameplay_audio
                .after(mir2_client_bevy::crystal_ui::NativePlayerUiSet::Mutate),
        )
            .chain(),
    );
    // Flush packet-authoritative map state first. A same-frame map reset must
    // invalidate stale NPC object ids before any queued Big Map request can
    // cross the Windows gateway boundary.
    app.add_systems(
        bevy::app::Update,
        gameplay_bridge::forward_big_map_intents.after(gameplay_bridge::drain_gameplay_events),
    );

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
