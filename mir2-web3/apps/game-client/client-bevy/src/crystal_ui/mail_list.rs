//! Crystal MailItemRow: original columns, full-bitmap icon centering and badges.
use super::*;

#[derive(Component)]
pub(super) struct RowIcon;

pub(super) fn layout_icons(images: Option<Res<Assets<Image>>>, mut icons: Query<(&ImageNode, &mut Node), With<RowIcon>>) {
    let Some(images) = images else { return };
    for (image, mut node) in &mut icons {
        let Some(bitmap) = images.get(&image.image) else {
            node.display = Display::None;
            continue;
        };
        let size = bitmap.texture_descriptor.size;
        node.left = Val::Px(((34 - size.width as i32) / 2) as f32);
        node.top = Val::Px(((32 - size.height as i32) / 2) as f32);
        node.width = Val::Px(size.width as f32);
        node.height = Val::Px(size.height as f32);
        node.display = Display::Flex;
    }
}

fn preview(message: &MailMessage) -> String {
    format!("{}{}", if message.locked { "[*] " } else { "" }, message.body.replace("\r\n", " "))
}

pub(super) fn row(parent: &mut ChildSpawnerCommands, assets: Option<&AssetServer>, message: &MailMessage, index: usize, selected: bool) {
    parent.spawn((Button, OverlayButton::SelectMail(message.id), FocusPolicy::Block, Node {
        position_type: PositionType::Absolute,
        left: Val::Px(10.0), top: Val::Px(55.0 + index as f32 * 33.0),
        width: Val::Px(290.0), height: Val::Px(33.0),
        ..default()
    }, BackgroundColor(Color::NONE))).with_children(|row| {
        if let Some(assets) = assets {
            let path = if let Some(item) = message.items.first() {
                item.image.map(|image| format!("original-ui/Items/{image}.png"))
            } else {
                Some(format!("original-ui/Prguse/{}.png", if message.gold > 0 { 541 } else { 540 }))
            };
            if let Some(path) = path {
                row.spawn((RowIcon, Node { position_type: PositionType::Absolute, display: Display::None, ..default() }, ImageNode::new(assets.load(path))));
            }
            if !message.read {
                spawn_static_overlay_sprite(row, assets, "original-ui/Prguse/550.png".into(), CrystalRect::new(if !message.claimed || message.locked {20.0} else {5.0}, 17.0, 12.0, 9.0));
            }
            if message.locked {
                spawn_static_overlay_sprite(row, assets, "original-ui/Prguse/551.png".into(), CrystalRect::new(5.0, 17.0, 12.0, 10.0));
            }
            if !message.claimed {
                spawn_static_overlay_sprite(row, assets, "original-ui/Prguse/552.png".into(), CrystalRect::new(5.0, 17.0, 12.0, 11.0));
            }
            if selected {
                spawn_static_overlay_sprite(row, assets, "original-ui/Prguse/545.png".into(), CrystalRect::new(-5.0, -3.0, 296.0, 38.0));
            }
        }
        for (text, left, width) in [(message.sender.clone(), 35.0, 130.0), (preview(message), 170.0, 115.0)] {
            row.spawn(Node {
                position_type: PositionType::Absolute, left: Val::Px(left), top: Val::Px(0.0),
                width: Val::Px(width), height: Val::Px(31.0), align_items: AlignItems::Center,
                overflow: Overflow::clip(), ..default()
            }).with_children(|label| {
                label.spawn((Text::new(text), crate::crystal_ui::typography::crystal_text_font(10.0),
                    TextColor(TEXT), TextLayout::new(Justify::Left, LineBreak::NoWrap)));
            });
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn row_preview_uses_message_not_subject_and_updates_lock_marker() {
        let mut message = MailMessage { subject: "not the preview".into(), body: "First\r\nSecond".into(), ..default() };
        assert_eq!(preview(&message), "First Second");
        message.locked = true;
        assert_eq!(preview(&message), "[*] First Second");
    }

    #[test]
    fn list_uses_original_rows_and_hides_unavailable_reply_and_list_claim() {
        let mut app = super::super::tests::overlay_render_test_app();
        app.world_mut().resource_mut::<NativePlayerUiState>().core.panel = mir2_ui_core::state::UiPanel::Mail;
        {
            let mut mail = app.world_mut().resource_mut::<MailModel>();
            mail.mails.push(MailMessage { id: 5, sender: "Sender".into(), body: "Preview".into(), gold: 100, locked: true, ..default() });
            mail.selected_id = Some(5);
        }
        app.update();
        let world = app.world_mut();
        let paths: Vec<_> = world.query::<&ImageNode>().iter(world)
            .filter_map(|image| image.image.path().map(|path| path.to_string())).collect();
        for path in ["Title/7", "Prguse/541", "Prguse/545", "Prguse/550", "Prguse/551", "Prguse/552", "Prguse/520", "Prguse/523"] {
            assert!(paths.contains(&format!("original-ui/{path}.png")), "missing {path}");
        }
        let buttons: Vec<_> = world.query::<&OverlayButton>().iter(world).collect();
        assert!(!buttons.iter().any(|button| matches!(button, OverlayButton::MailReply(_) | OverlayButton::ClaimMail(_))));
        let (node, _) = world.query::<(&Node, &OverlayButton)>().iter(world)
            .find(|(_, button)| matches!(button, OverlayButton::SelectMail(5))).unwrap();
        assert_eq!((node.left, node.top, node.width, node.height), (Val::Px(10.0), Val::Px(55.0), Val::Px(290.0), Val::Px(33.0)));
    }
}
