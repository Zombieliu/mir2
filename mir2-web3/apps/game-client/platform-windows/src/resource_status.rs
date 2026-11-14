//! Native presentation of authenticated resource availability. Network work
//! stays in the broker; the game keeps pumping events while a needed PNG waits.
use bevy::prelude::*;
use std::sync::atomic::{AtomicBool, Ordering};

static VISUAL_WAITING: AtomicBool = AtomicBool::new(false);

#[derive(Component)]
struct ResourceStatusRoot;
#[derive(Component)]
struct ResourceStatusText;
#[derive(Component)]
struct ResourceRetryButton;
#[derive(Component)]
struct ResourceRetryText;

pub(crate) fn visual_waiting() -> bool {
    VISUAL_WAITING.load(Ordering::Relaxed)
}

pub(crate) fn install(app: &mut App) {
    app.add_systems(PostUpdate, update_status);
}

/// A pack can become available before AssetServer has finished decoding its
/// image. Keep the shield until actual visible, catalog-backed handles resolve.
fn visible_images_waiting(world: &World) -> Result<bool, String> {
    let Some(server) = world.get_resource::<AssetServer>() else {
        return Ok(false);
    };
    let Some(store) = crate::packed_assets::store() else {
        return Ok(false);
    };
    let mut waiting = false;
    for entity in world.iter_entities() {
        if entity
            .get::<InheritedVisibility>()
            .is_some_and(|visible| !visible.get())
        {
            continue;
        }
        // Native dialogs can retain image handles while Display::None makes
        // their computed layout empty. Those unopened screens are background
        // work, not a reason to hold the current screen behind the shield.
        if entity.get::<ImageNode>().is_some()
            && entity
                .get::<ComputedNode>()
                .is_none_or(ComputedNode::is_empty)
        {
            continue;
        }
        let handle = entity
            .get::<ImageNode>()
            .map(|image| &image.image)
            .or_else(|| entity.get::<Sprite>().map(|sprite| &sprite.image));
        let Some(handle) = handle else {
            continue;
        };
        let Some(path) = server.get_path(handle.id()) else {
            continue;
        };
        let Some(key) = path.path().to_str() else {
            continue;
        };
        if !store.exists(key) {
            continue;
        }
        match server.get_load_state(handle.id()) {
            Some(bevy::asset::LoadState::Failed(_)) => {
                return Err(format!("verified image could not be decoded: {key}"));
            }
            _ if !server.is_loaded_with_dependencies(handle.id()) => waiting = true,
            _ => {}
        }
    }
    Ok(waiting)
}

fn update_status(world: &mut World) {
    let Some(progress) = crate::packed_assets::progress() else {
        VISUAL_WAITING.store(false, Ordering::Relaxed);
        return;
    };
    let progress = match progress {
        Ok(progress) => progress,
        Err(error) => {
            crate::packed_assets::record_failure(format!("resource status failed: {error}"));
            return;
        }
    };
    let visuals = match visible_images_waiting(world) {
        Ok(waiting) => waiting,
        Err(error) => {
            crate::packed_assets::record_failure(error);
            true
        }
    };
    VISUAL_WAITING.store(visuals, Ordering::Relaxed);
    let blocking = progress.foreground_pending > 0 || visuals;
    if blocking
        && world
            .get_resource::<ButtonInput<KeyCode>>()
            .is_some_and(|keys| keys.just_pressed(KeyCode::F5))
    {
        crate::packed_assets::retry();
    }
    // F5 remains a gameplay skill key while the game is usable. Background
    // retries have their own pointer button; foreground waiting owns F5.
    let retry_clicked = !blocking
        && world
            .get_resource::<ButtonInput<MouseButton>>()
            .is_some_and(|mouse| mouse.just_pressed(MouseButton::Left))
        && world
            .query_filtered::<&Interaction, With<ResourceRetryButton>>()
            .iter(world)
            .any(|interaction| *interaction == Interaction::Pressed);
    if retry_clicked {
        crate::packed_assets::retry();
    }
    let chinese = matches!(
        mir2_client_bevy::native_i18n::locale(),
        mir2_client_bevy::native_i18n::Locale::SimplifiedChinese
            | mir2_client_bevy::native_i18n::Locale::TraditionalChinese
    );
    let mib = |bytes: u64| bytes as f64 / (1024.0 * 1024.0);
    let message = if blocking {
        if chinese {
            format!(
                "正在准备当前画面所需素材，请稍候\n已校验 {:.1} / {:.1} MiB\n{}",
                mib(progress.available_bytes),
                mib(progress.total_bytes),
                if progress.last_error.is_some() {
                    "下载暂时失败，按 F5 重试；可以正常关闭游戏"
                } else {
                    "游戏保持响应，素材就绪后恢复操作"
                }
            )
        } else {
            format!(
                "Preparing resources for this screen. Please wait.\nVerified {:.1} / {:.1} MiB\n{}",
                mib(progress.available_bytes),
                mib(progress.total_bytes),
                if progress.last_error.is_some() {
                    "Download paused. Press F5 to retry; closing remains available."
                } else {
                    "The game remains responsive; controls resume when resources are ready."
                }
            )
        }
    } else if progress.complete {
        if chinese {
            "全部素材已校验完成".to_owned()
        } else {
            "All game resources verified".to_owned()
        }
    } else if chinese {
        format!(
            "当前素材已就绪 · 其余素材后台补齐 {:.1} / {:.1} MiB{}",
            mib(progress.available_bytes),
            mib(progress.total_bytes),
            if progress.last_error.is_some() {
                " · 已暂停"
            } else {
                ""
            }
        )
    } else {
        format!(
            "Current resources ready · Background completion {:.1} / {:.1} MiB{}",
            mib(progress.available_bytes),
            mib(progress.total_bytes),
            if progress.last_error.is_some() {
                " · Paused"
            } else {
                ""
            }
        )
    };
    let root = world
        .query_filtered::<Entity, With<ResourceStatusRoot>>()
        .iter(world)
        .next();
    let root = root.unwrap_or_else(|| {
        world
            .spawn((
                ResourceStatusRoot,
                Node::default(),
                GlobalZIndex(1_000_000),
                BackgroundColor(Color::BLACK),
                bevy::ui::FocusPolicy::Block,
                Interaction::None,
            ))
            .with_children(|parent| {
                parent.spawn((
                    ResourceStatusText,
                    Text::default(),
                    TextFont {
                        font_size: FontSize::Px(16.0),
                        ..default()
                    },
                    TextColor(Color::WHITE),
                    Node {
                        margin: UiRect::all(px(16.0)),
                        ..default()
                    },
                ));
                parent
                    .spawn((
                        ResourceRetryButton,
                        Button,
                        Node {
                            display: Display::None,
                            margin: UiRect::all(px(8.0)),
                            padding: UiRect::axes(px(12.0), px(6.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.22, 0.17, 0.07)),
                    ))
                    .with_children(|button| {
                        button.spawn((
                            ResourceRetryText,
                            Text::new("Retry"),
                            TextFont {
                                font_size: FontSize::Px(16.0),
                                ..default()
                            },
                            TextColor(Color::WHITE),
                        ));
                    });
            })
            .id()
    });
    let node = if blocking {
        Node {
            position_type: PositionType::Absolute,
            left: px(0.0),
            top: px(0.0),
            width: percent(100.0),
            height: percent(100.0),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            ..default()
        }
    } else {
        Node {
            position_type: PositionType::Absolute,
            left: px(8.0),
            bottom: px(8.0),
            max_width: percent(95.0),
            ..default()
        }
    };
    world.entity_mut(root).insert((
        node,
        BackgroundColor(if blocking {
            Color::BLACK
        } else {
            Color::srgba(0.02, 0.02, 0.02, 0.9)
        }),
    ));
    for mut text in world
        .query_filtered::<&mut Text, With<ResourceStatusText>>()
        .iter_mut(world)
    {
        if text.0 != message {
            text.0.clone_from(&message);
        }
    }
    for mut node in world
        .query_filtered::<&mut Node, With<ResourceRetryButton>>()
        .iter_mut(world)
    {
        node.display = if !blocking && progress.last_error.is_some() {
            Display::Flex
        } else {
            Display::None
        };
    }
    for mut text in world
        .query_filtered::<&mut Text, With<ResourceRetryText>>()
        .iter_mut(world)
    {
        let label = if chinese { "重试" } else { "Retry" };
        if text.0 != label {
            text.0 = label.into();
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::gateway::GatewayCommand;
    use crate::native_protocol::NativeOutboundCommand;
    #[test]
    fn resource_wait_blocks_world_actions_without_blocking_session_exit_or_connection() {
        let walk = GatewayCommand::Wire(NativeOutboundCommand::Walk {
            direction: "down".into(),
        });
        assert!(crate::input::resource_gate_allows(false, &walk));
        assert!(!crate::input::resource_gate_allows(true, &walk));
        for command in [
            GatewayCommand::Connect,
            GatewayCommand::Shutdown,
            GatewayCommand::Wire(NativeOutboundCommand::LogOut),
            GatewayCommand::Wire(NativeOutboundCommand::Disconnect),
        ] {
            assert!(crate::input::resource_gate_allows(true, &command));
        }
    }
}
