//! Include keyboard_dialog as a child of crystal_ui::overlays so the shared
//! original-image button helpers remain the sole rendering implementation.
use super::super::*;
use super::*;

#[derive(Component)]
pub struct KeyboardPanel;
#[derive(Component)]
pub struct KeyboardThumb;

const CONTROLS: [(&str, u16, u16, u16, f32, f32, KeyboardAction); 5] = [
    ("Prguse2", 360, 361, 362, 489.0, 3.0, KeyboardAction::Close),
    (
        "Prguse2",
        197,
        198,
        199,
        491.0,
        88.0,
        KeyboardAction::Previous,
    ),
    ("Prguse2", 207, 208, 209, 491.0, 363.0, KeyboardAction::Next),
    ("Title", 120, 121, 122, 30.0, 400.0, KeyboardAction::Reset),
    (
        "Prguse",
        1346,
        1346,
        1346,
        105.0,
        406.0,
        KeyboardAction::Enforce,
    ),
];

pub fn hit_action(
    model: &KeyboardDialogUi,
    local: Vec2,
    dimensions: impl Fn(&str, u16) -> Option<Vec2>,
) -> Option<KeyboardAction> {
    if !model.open {
        return None;
    }
    for &(library, normal, _, _, x, y, action) in &CONTROLS {
        let size = dimensions(library, normal)?;
        if inside(local, x, y, size.x, size.y) {
            return Some(action);
        }
    }
    for row in model.rows() {
        if let KeyboardRow::Binding { index, y } = row {
            if inside(local, 360.0, y as f32, 120.0, 16.0) {
                return Some(KeyboardAction::Bind(index));
            }
        }
    }
    None
}

fn inside(point: Vec2, x: f32, y: f32, w: f32, h: f32) -> bool {
    point.x >= x && point.y >= y && point.x < x + w && point.y < y + h
}

/// Return false while any original image is unavailable. Dimensions always
/// come from decoded assets, including the Title119 background.
pub fn render(
    parent: &mut ChildSpawnerCommands,
    assets: &AssetServer,
    model: &mut KeyboardDialogUi,
    viewport: Vec2,
    dimensions: impl Fn(&str, u16) -> Option<Vec2>,
) -> bool {
    if !model.open {
        return false;
    }
    let Some(size) = dimensions("Title", 119) else {
        return false;
    };
    for &(library, normal, hover, pressed, _, _, _) in &CONTROLS {
        for index in [normal, hover, pressed] {
            if dimensions(library, index).is_none() {
                return false;
            }
        }
    }
    for (library, index) in [
        ("Prguse", 1347),
        ("Prguse2", 190),
        ("Prguse2", 191),
        ("Prguse2", 192),
        ("Prguse2", 201),
        ("Prguse2", 202),
        ("Prguse2", 205),
        ("Prguse2", 206),
    ] {
        if dimensions(library, index).is_none() {
            return false;
        }
    }
    let position = *model
        .position
        .get_or_insert([(viewport.x - size.x) / 2.0, (viewport.y - size.y) / 2.0]);
    let defaults = crystal_default_keybinds();
    parent
        .spawn((
            KeyboardPanel,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(position[0]),
                top: Val::Px(position[1]),
                width: Val::Px(size.x),
                height: Val::Px(size.y),
                ..default()
            },
            FocusPolicy::Block,
            GlobalZIndex(OVERLAY_HELP_SORTED_Z),
        ))
        .with_children(|panel| {
            spawn_overlay_frame(panel, assets, "original-ui/Title/119.png", size.x, size.y);
            overlay_centered_text_at(
                panel,
                "Keyboard Settings",
                CrystalRect::new(135.0, 34.0, 242.0, 30.0),
                40.0 / 3.0,
                Color::WHITE,
            );
            for &(library, normal, hover, pressed, x, y, action) in &CONTROLS {
                let d = dimensions(library, normal).unwrap();
                let normal = if action == KeyboardAction::Enforce && model.enforce {
                    1347
                } else {
                    normal
                };
                let (hover, pressed) = if action == KeyboardAction::Enforce {
                    (normal, normal)
                } else {
                    (hover, pressed)
                };
                let spec = CrystalButtonSpec::new(
                    library,
                    normal,
                    hover,
                    pressed,
                    CrystalRect::new(x, y, d.x, d.y),
                    d.x,
                    d.y,
                );
                spawn_crystal_image_button(
                    panel,
                    assets,
                    spec,
                    CrystalButtonAssetSet::from_spec(spec),
                    action,
                    false,
                    true,
                );
            }
            overlay_text_at(
                panel,
                if model.enforce {
                    "Assign Rule: Strict"
                } else {
                    "Assign Rule: Relaxed"
                },
                CrystalRect::new(120.0, 404.0, 300.0, 20.0),
                32.0 / 3.0,
                Color::WHITE,
            );
            if model.bindings.len() > 16 {
                let d = dimensions("Prguse2", 205).unwrap();
                let spec = CrystalButtonSpec::new(
                    "Prguse2",
                    205,
                    206,
                    206,
                    CrystalRect::new(491.0, model.thumb_y() as f32, d.x, d.y),
                    d.x,
                    d.y,
                );
                spawn_crystal_image_button(
                    panel,
                    assets,
                    spec,
                    CrystalButtonAssetSet::from_spec(spec),
                    KeyboardThumb,
                    false,
                    true,
                );
            }
            for row in model.rows() {
                match row {
                    KeyboardRow::Heading { label, y } => {
                        for (index, x, dy) in [(201, 25.0, 10.0), (202, 15.0, 25.0)] {
                            let d = dimensions("Prguse2", index).unwrap();
                            let spec = CrystalButtonSpec::new(
                                "Prguse2",
                                index,
                                index,
                                index,
                                CrystalRect::new(x, y as f32 + dy, d.x, d.y),
                                d.x,
                                d.y,
                            );
                            spawn_crystal_image_button(
                                panel,
                                assets,
                                spec,
                                CrystalButtonAssetSet::from_spec(spec),
                                KeyboardDecoration,
                                false,
                                false,
                            );
                        }
                        overlay_text_at(
                            panel,
                            &label,
                            CrystalRect::new(35.0, y as f32 + 5.0, 200.0, 20.0),
                            12.0,
                            Color::WHITE,
                        );
                    }
                    KeyboardRow::Binding { index, y } => {
                        let bind = &model.bindings[index];
                        overlay_text_at(
                            panel,
                            &bind.description,
                            CrystalRect::new(20.0, y as f32, 200.0, 15.0),
                            32.0 / 3.0,
                            Color::WHITE,
                        );
                        overlay_text_at(
                            panel,
                            &defaults[index].display(),
                            CrystalRect::new(220.0, y as f32, 100.0, 15.0),
                            32.0 / 3.0,
                            Color::WHITE,
                        );
                        let waiting = model.waiting == Some(index);
                        let spec = CrystalButtonSpec::new(
                            "Prguse2",
                            if waiting { 192 } else { 190 },
                            if waiting { 192 } else { 191 },
                            192,
                            CrystalRect::new(360.0, y as f32, 120.0, 16.0),
                            120.0,
                            16.0,
                        );
                        spawn_crystal_image_button(
                            panel,
                            assets,
                            spec,
                            CrystalButtonAssetSet::from_spec(spec),
                            KeyboardAction::Bind(index),
                            false,
                            true,
                        );
                        overlay_text_at(
                            panel,
                            &format!(
                                "  {}",
                                if waiting {
                                    "????".into()
                                } else {
                                    bind.display()
                                }
                            ),
                            CrystalRect::new(360.0, y as f32, 120.0, 16.0),
                            32.0 / 3.0,
                            Color::WHITE,
                        );
                    }
                }
            }
        });
    true
}

#[derive(Component)]
struct KeyboardDecoration;
