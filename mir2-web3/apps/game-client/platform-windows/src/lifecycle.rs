//! Preserve the game's quit confirmation for OS close requests and record exits.
use bevy::app::AppExit;
use bevy::prelude::*;
use bevy::window::{WindowCloseRequested, WindowClosed};
use mir2_client_bevy::crystal_ui::overlays::{
    leave_game_dialog::LeaveKind, NativePlayerUiState,
};
use mir2_client_bevy::native_shell::{NativeShellModel, NativeShellScreen};

pub fn handle_close_requests(
    mut requests: MessageReader<WindowCloseRequested>,
    shell: Option<Res<NativeShellModel>>,
    mut ui: Option<ResMut<NativePlayerUiState>>,
    mut exits: MessageWriter<AppExit>,
) {
    for request in requests.read() {
        let in_game = shell.as_deref().is_some_and(|s| s.screen == NativeShellScreen::InGame);
        eprintln!("[native-lifecycle] os_close_requested window={:?} in_game={in_game}", request.window);
        if in_game {
            if let Some(ui) = ui.as_deref_mut() {
                ui.leave_game.request(LeaveKind::Exit, std::time::Instant::now());
            }
        } else {
            eprintln!("[native-lifecycle] exit_source=os_close_outside_game");
            exits.write(AppExit::Success);
        }
    }
}

pub fn record_exit_events(
    mut closed: MessageReader<WindowClosed>,
    mut exits: MessageReader<AppExit>,
) {
    for event in closed.read() {
        eprintln!("[native-lifecycle] window_closed window={:?}", event.window);
    }
    for event in exits.read() {
        eprintln!("[native-lifecycle] app_exit event={event:?}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn os_close_requires_confirmation_in_game_but_allows_login_close() {
        for in_game in [true, false] {
            let mut app = App::new();
            app.add_plugins(MinimalPlugins)
                .add_plugins(bevy::window::WindowPlugin { close_when_requested: false, ..default() })
                .init_resource::<NativePlayerUiState>()
                .add_systems(PostUpdate, handle_close_requests);
            let mut shell = NativeShellModel::default();
            shell.screen = if in_game { NativeShellScreen::InGame } else { NativeShellScreen::Login };
            app.insert_resource(shell);
            let window = app.world_mut().query_filtered::<Entity, With<Window>>().single(app.world()).unwrap();
            app.world_mut().write_message(WindowCloseRequested { window });
            app.update();
            assert_eq!(app.world().resource::<NativePlayerUiState>().leave_game.prompt, in_game.then_some(LeaveKind::Exit));
            assert_eq!(app.should_exit().is_some(), !in_game);
            assert!(app.world().get::<Window>(window).is_some());
        }
    }
}
