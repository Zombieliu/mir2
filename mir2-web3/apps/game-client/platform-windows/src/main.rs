//! Native Windows desktop host for the mir2-web3 Bevy client.
//!
//! M2 slice: open a real OS window running the same shared Bevy app the Web
//! client uses (no DOM/canvas assumptions), and connect to the same Gateway via
//! the WebSocket protocol the browser speaks. Authority stays in Simulation and
//! the Gateway; this crate only owns window/lifecycle/input hosting and forwards
//! world snapshots into the shared runtime.

use mir2_bevy_runtime::{build_runtime_app, RuntimeWindowSpec};

mod assets;
mod atlas;
mod gateway;
mod input;
mod map_parser;
mod session_config;

fn main() {
    console_error_panic_hook::set_once();

    let session = session_config::NativeSessionConfig::from_env(gateway::LOCAL_GATEWAY_WS_URL)
        .unwrap_or_else(|error| {
            eprintln!("[platform-windows] configuration error: {error}");
            std::process::exit(2);
        });

    // The gateway command channel: the Bevy input systems push intents into the
    // sender; the gateway task drains the receiver on its tokio thread.
    let (command_tx, command_rx) = std::sync::mpsc::channel::<gateway::PlayerIntent>();

    // The shared Bevy app. Native window spec: opaque, real OS window. The
    // AssetServer resolves atlas image paths relative to the repo
    // `apps/web/public` (where `generated/map-atlas/...` and
    // `bevy-entity-atlases/...` live).
    let asset_root = assets::asset_root().unwrap_or_else(|| {
        eprintln!(
            "[platform-windows] no Mir2 asset bundle found; set {} or install mir2-assets beside the executable",
            assets::ASSET_ROOT_ENV
        );
        std::path::PathBuf::from(".")
    });
    let mut app = build_runtime_app(RuntimeWindowSpec {
        asset_root: asset_root.to_string_lossy().into_owned(),
        ..RuntimeWindowSpec::native("mir2-web3 (native)")
    });

    // Load the local starter entity atlas (if present) so entities render real
    // sprite pixels instead of the colored fallback. Must run after the app is
    // built so the runtime's native ingestion channel is registered; the runtime
    // drains it once `app.run()` starts.
    let _ = atlas::load_starter_entity_atlas();
    // The shared in-game HUD: renders player stats from the UiReadModel the
    // gateway task feeds via the runtime's native ingestion channel.
    app.add_plugins(mir2_client_bevy::hud::Mir2HudPlugin);
    // Map/entity fallback and atlas rendering are already owned by the shared
    // runtime. Registering the client-bevy fallback plugins here rendered a
    // second copy on top of that scene.
    // The shared inventory panel: draws the bag from the InventoryModel.
    app.add_plugins(mir2_client_bevy::inventory::Mir2InventoryPlugin);
    // The shared chat panel: draws recent chat lines from the ChatModel.
    app.add_plugins(mir2_client_bevy::chat::Mir2ChatPlugin);
    // The shared character panel: draws player stats from the UiReadModel.
    app.add_plugins(mir2_client_bevy::character::Mir2CharacterPlugin);
    // Native input: WASD / arrows → walk/run intents forwarded to the gateway.
    app.insert_resource(input::GatewayCommands::new(command_tx));
    app.add_systems(
        bevy::app::Update,
        (
            input::keyboard_walk_system,
            input::keyboard_run_system,
            input::keyboard_turn_system,
        ),
    );

    eprintln!("[platform-windows] native window opened; runtime running");

    let gateway_runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    let gateway_task = gateway_runtime.spawn(async move {
        match gateway::run_gateway_session(
            session.account_id,
            session.password,
            session.character_index,
            &session.gateway_url,
            command_rx,
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
    let _ = gateway_task.abort();
}
