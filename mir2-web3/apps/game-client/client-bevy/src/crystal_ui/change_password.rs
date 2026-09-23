//! Crystal LoginScene.ChangePasswordDialog geometry and original artwork.
use super::{
    assets::CrystalButtonAssetSet,
    spec::{CrystalButtonSpec, CrystalRect},
    typography::crystal_text_font,
    widget::spawn_crystal_image_button,
};
use crate::native_shell::{validate_change_password_fields, ChangePasswordFocus, NativeShellModel};
use crate::native_shell_ui::{NativeShellButton, NativeShellField};
use bevy::prelude::*;

const LEFT: f32 = (1024.0 - 348.0) / 2.0;
const TOP: f32 = (768.0 - 268.0) / 2.0;

fn node(x: f32, y: f32, width: f32, height: f32) -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: Val::Px(LEFT + x),
        top: Val::Px(TOP + y),
        width: Val::Px(width),
        height: Val::Px(height),
        ..default()
    }
}

pub(crate) fn spawn_change_password(
    parent: &mut ChildSpawnerCommands,
    assets: &AssetServer,
    model: &NativeShellModel,
) {
    parent.spawn((
        node(0.0, 0.0, 348.0, 268.0),
        ImageNode::new(assets.load("original-ui/Prguse/50.png")),
    ));
    let form = &model.change_password;
    let fields = [
        (
            ChangePasswordFocus::AccountId,
            75.0,
            form.account_id.clone(),
        ),
        (
            ChangePasswordFocus::OldPassword,
            113.0,
            "*".repeat(form.old_password.chars().count()),
        ),
        (
            ChangePasswordFocus::NewPassword,
            151.0,
            "*".repeat(form.new_password.chars().count()),
        ),
        (
            ChangePasswordFocus::ConfirmPassword,
            188.0,
            "*".repeat(form.confirm_password.chars().count()),
        ),
    ];
    for (focus, y, value) in fields {
        let mut field_node = node(178.0, y, 136.0, 18.0);
        field_node.border = UiRect::all(Val::Px(1.0));
        field_node.padding = UiRect::horizontal(Val::Px(2.0));
        field_node.overflow = Overflow::clip();
        field_node.align_items = AlignItems::Center;
        let mut field = parent.spawn((
            field_node,
            BackgroundColor(Color::BLACK),
            BorderColor::all(Color::srgb_u8(128, 128, 128)),
            NativeShellField::ChangePassword(focus),
        ));
        if !model.change_password_request_in_flight {
            field.insert(Button);
        }
        field.with_children(|p| {
            p.spawn((
                Text::new(value),
                crystal_text_font(11.0),
                TextColor(Color::WHITE),
            ));
        });
    }
    let valid = validate_change_password_fields(
        &form.account_id,
        &form.old_password,
        &form.new_password,
        &form.confirm_password,
    )
    .is_ok();
    for (index, x, width, action, focus, enabled) in [
        (
            107,
            80.0,
            90.0,
            NativeShellButton::SubmitChangePassword,
            ChangePasswordFocus::SubmitButton,
            valid && !model.change_password_request_in_flight,
        ),
        (
            110,
            222.0,
            68.0,
            NativeShellButton::CancelChangePassword,
            ChangePasswordFocus::CancelButton,
            !model.change_password_request_in_flight,
        ),
    ] {
        let spec = CrystalButtonSpec::new(
            "Title",
            index,
            index + 1,
            index + 2,
            CrystalRect::new(LEFT + x, TOP + 236.0, width, 25.0),
            width,
            25.0,
        );
        spawn_crystal_image_button(
            parent,
            assets,
            spec,
            CrystalButtonAssetSet::from_spec(spec),
            action,
            form.focus == focus,
            enabled,
        );
    }
    if let Some(notice) = &model.notice {
        parent
            .spawn((
                node(0.0, 274.0, 348.0, 40.0),
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.85)),
            ))
            .with_children(|p| {
                p.spawn((
                    Text::new(&notice.message),
                    crystal_text_font(11.0),
                    TextColor(Color::WHITE),
                ));
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crystal_ui::widget::CrystalImageButton;
    use bevy::asset::AssetApp;

    fn spawn(mut commands: Commands, assets: Res<AssetServer>, model: Res<NativeShellModel>) {
        commands
            .spawn(Node::default())
            .with_children(|p| spawn_change_password(p, &assets, &model));
    }

    #[test]
    fn change_password_original_frame_masks_secrets_and_disables_invalid_submit() {
        for valid in [false, true] {
            let mut model = NativeShellModel::default();
            model.change_password.account_id = "testuser".into();
            model.change_password.old_password = "secret1".into();
            model.change_password.new_password = "secret2".into();
            model.change_password.confirm_password = if valid { "secret2" } else { "wrong" }.into();
            let mut app = App::new();
            app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()))
                .init_asset::<Image>()
                .insert_resource(model)
                .add_systems(Startup, spawn);
            app.update();
            let world = app.world_mut();
            let mut fields = world.query::<(&NativeShellField, &Node, &BackgroundColor)>();
            let fields: Vec<_> = fields.iter(world).collect();
            assert_eq!(fields.len(), 4);
            for (_, n, bg) in fields {
                assert_eq!(n.width, Val::Px(136.0));
                assert_eq!(n.height, Val::Px(18.0));
                assert_eq!(n.left, Val::Px(LEFT + 178.0));
                assert_eq!(bg.0, Color::BLACK);
            }
            let mut buttons = world.query::<(&NativeShellButton, &CrystalImageButton, &Node)>();
            let (_, button, n) = buttons
                .iter(world)
                .find(|(action, _, _)| matches!(action, NativeShellButton::SubmitChangePassword))
                .unwrap();
            assert_eq!(button.enabled, valid);
            assert!(button.assets.normal.ends_with("Title/107.png"));
            assert_eq!(n.width, Val::Px(90.0));
            let mut texts = world.query::<&Text>();
            assert!(texts.iter(world).any(|t| t.0 == "*******"));
            assert!(!texts.iter(world).any(|t| t.0.contains("secret")));
        }
    }
}
