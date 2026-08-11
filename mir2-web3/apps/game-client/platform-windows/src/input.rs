//! Native keyboard input → gateway player intents.
//!
//! Maps WASD / arrow keys to Mir2 directions and forwards them to the gateway
//! WebSocket task via the cross-thread [`gateway::GatewayCommandSender`]. This
//! is a thin presentation→intent edge (ADR-0001): the server validates
//! movement; the client only expresses the request.

use bevy::input::ButtonInput;
use bevy::prelude::{KeyCode, Res};
use mir2_client_bevy::entities::{EntityKind, EntityModelSet};

use crate::gateway::{GatewayCommandSender, PlayerIntent};

/// Mir2 direction strings the gateway `Walk`/`Run`/`Turn` commands accept.
const UP: &str = "up";
const DOWN: &str = "down";
const LEFT: &str = "left";
const RIGHT: &str = "right";

/// Bevy resource holding the gateway command sender, injected by the host.
#[derive(bevy::prelude::Resource)]
pub struct GatewayCommands {
    sender: GatewayCommandSender,
}

impl GatewayCommands {
    pub fn new(sender: GatewayCommandSender) -> Self {
        Self { sender }
    }

    fn send(&self, intent: PlayerIntent) {
        let _ = self.sender.send(intent);
    }
}

/// Forward walk intents on WASD / arrow key presses.
pub fn keyboard_walk_system(keys: Res<ButtonInput<KeyCode>>, commands: Res<GatewayCommands>) {
    if keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight) {
        return;
    }
    for (code, direction) in walk_key_map() {
        if keys.just_pressed(code) {
            commands.send(PlayerIntent::Walk {
                direction: direction.to_owned(),
            });
        }
    }
}

/// Forward run intents on WASD / arrows while Shift is held.
pub fn keyboard_run_system(keys: Res<ButtonInput<KeyCode>>, commands: Res<GatewayCommands>) {
    if !keys.pressed(KeyCode::ShiftLeft) && !keys.pressed(KeyCode::ShiftRight) {
        return;
    }
    for (code, direction) in walk_key_map() {
        if keys.just_pressed(code) {
            commands.send(PlayerIntent::Run {
                direction: direction.to_owned(),
            });
        }
    }
}

/// Forward absolute turn intents derived from the latest authoritative self
/// facing. The gateway protocol accepts an absolute direction, not a relative
/// "left"/"right" turn sense.
pub fn keyboard_turn_system(
    keys: Res<ButtonInput<KeyCode>>,
    commands: Res<GatewayCommands>,
    entities: Res<EntityModelSet>,
) {
    let Some(current) = entities
        .entities
        .iter()
        .find(|entity| entity.kind == EntityKind::SelfPlayer)
        .and_then(|entity| entity.direction.as_deref())
    else {
        return;
    };

    let turn_delta = if keys.just_pressed(KeyCode::KeyQ) {
        Some(-1)
    } else if keys.just_pressed(KeyCode::KeyE) {
        Some(1)
    } else {
        None
    };
    if let Some(delta) = turn_delta.and_then(|delta| rotate_direction(current, delta)) {
        commands.send(PlayerIntent::Turn {
            direction: delta.to_owned(),
        });
    }
}

fn rotate_direction(current: &str, delta: i32) -> Option<&'static str> {
    const DIRECTIONS: [&str; 8] = [
        "up",
        "upright",
        "right",
        "downright",
        "down",
        "downleft",
        "left",
        "upleft",
    ];
    let current_index = DIRECTIONS
        .iter()
        .position(|direction| direction.eq_ignore_ascii_case(current))?
        as i32;
    let next_index = (current_index + delta).rem_euclid(DIRECTIONS.len() as i32) as usize;
    Some(DIRECTIONS[next_index])
}

/// The WASD / arrow → direction mapping shared by walk and run.
fn walk_key_map() -> [(KeyCode, &'static str); 8] {
    [
        (KeyCode::KeyW, UP),
        (KeyCode::KeyS, DOWN),
        (KeyCode::KeyA, LEFT),
        (KeyCode::KeyD, RIGHT),
        (KeyCode::ArrowUp, UP),
        (KeyCode::ArrowDown, DOWN),
        (KeyCode::ArrowLeft, LEFT),
        (KeyCode::ArrowRight, RIGHT),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use mir2_client_bevy::entities::{EntityKind, EntityModel, EntityModelSet};

    fn input_app() -> (bevy::prelude::App, std::sync::mpsc::Receiver<PlayerIntent>) {
        let (sender, receiver) = std::sync::mpsc::channel();
        let mut app = bevy::prelude::App::new();
        app.insert_resource(ButtonInput::<KeyCode>::default());
        app.insert_resource(GatewayCommands::new(sender));
        app.insert_resource(EntityModelSet {
            entities: vec![EntityModel {
                object_id: "self".to_owned(),
                kind: EntityKind::SelfPlayer,
                name: "Self".to_owned(),
                x: 0,
                y: 0,
                level: Some(1),
                direction: Some("up".to_owned()),
            }],
        });
        (app, receiver)
    }

    #[test]
    fn shift_direction_emits_only_run() {
        let (mut app, receiver) = input_app();
        app.add_systems(
            bevy::prelude::Update,
            (keyboard_walk_system, keyboard_run_system),
        );
        {
            let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            keys.press(KeyCode::ShiftLeft);
            keys.press(KeyCode::KeyW);
        }
        app.update();

        let intents = receiver.try_iter().collect::<Vec<_>>();
        assert_eq!(intents.len(), 1);
        assert!(matches!(
            &intents[0],
            PlayerIntent::Run { direction } if direction == "up"
        ));
    }

    #[test]
    fn q_rotates_current_facing_left_by_one_crystal_direction() {
        let (mut app, receiver) = input_app();
        app.add_systems(bevy::prelude::Update, keyboard_turn_system);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyQ);
        app.update();

        let intent = receiver.try_recv().expect("turn intent");
        assert!(matches!(
            intent,
            PlayerIntent::Turn { direction } if direction == "upleft"
        ));
    }
}
