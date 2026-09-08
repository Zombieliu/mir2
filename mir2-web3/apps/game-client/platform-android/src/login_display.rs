//! Latest-value handoff from the Activity's validated session to Bevy.
//! There is no pending packet queue and no credentials cross this interface.

use bevy::prelude::*;
use std::sync::Mutex;

static STATUS: Mutex<String> = Mutex::new(String::new());

#[derive(Component)]
pub(crate) struct LoginStatus;

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_mir2_web3_MainActivity_nativeStatus<'a>(
    mut env: jni::EnvUnowned<'a>,
    _class: jni::objects::JClass<'a>,
    text: jni::objects::JString<'a>,
) {
    env.with_env(|_env| -> Result<(), jni::errors::Error> {
        let value = text.to_string();
        if value.len() <= 2048 {
            *STATUS.lock().unwrap_or_else(|e| e.into_inner()) = value;
        }
        Ok(())
    })
    .resolve::<jni::errors::ThrowRuntimeExAndDefault>();
}

pub(crate) fn spawn(mut commands: Commands) {
    commands.spawn((
        LoginStatus,
        Text::new("Connect to your approved test Gateway"),
        TextFont {
            font_size: FontSize::Px(26.0),
            ..default()
        },
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            left: px(440),
            top: px(48),
            max_width: px(640),
            ..default()
        },
    ));
}

pub(crate) fn update(mut labels: Query<&mut Text, With<LoginStatus>>) {
    let value = STATUS.lock().unwrap_or_else(|e| e.into_inner());
    if value.is_empty() {
        return;
    }
    for mut label in &mut labels {
        if label.0 != *value {
            label.0.clone_from(&value);
        }
    }
}
