//! Exact Crystal images/coordinates; host supplies decoded frame geometry including offsets.
use super::*;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OriginalFrame {
    pub width: f32,
    pub height: f32,
    pub x: f32,
    pub y: f32,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
pub enum EquipmentWindow {
    Mount,
    Fishing,
    Status,
    Notice,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
pub struct AttachmentCell {
    pub kind: EquipmentDialog,
    pub slot: usize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HitAction {
    Button(EquipmentAction),
    Cell(AttachmentCell),
    Drag(EquipmentWindow),
    Block,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DialogHit {
    pub rect: CrystalRect,
    pub action: HitAction,
}

pub fn attachment_rect(kind: EquipmentDialog, slot: usize, count: usize) -> Option<CrystalRect> {
    if slot >= count.min(5) {
        return None;
    }
    Some(match kind {
        EquipmentDialog::Mount => {
            let off = if count == 4 { 1.0 } else { 0.0 };
            CrystalRect::new(36.0 + slot as f32 * 54.0 + off, 323.0 + off, 34.0, 30.0)
        }
        EquipmentDialog::Fishing => {
            let (x, y) = if slot == 0 {
                (17.0, 203.0)
            } else {
                (17.0 + (slot - 1) as f32 * 40.0, 241.0)
            };
            CrystalRect::new(x, y, 34.0, 30.0)
        }
    })
}
pub fn required_frames(model: &MountFishingUi, now: u64) -> Vec<(&'static str, u16)> {
    let mut frames = vec![
        ("Prguse", 160),
        ("Prguse", 167),
        ("Prguse", 155),
        ("Prguse", 156),
        ("Prguse", 157),
        ("Prguse", 164),
        ("Prguse", 165),
        ("Prguse", 166),
        ("Prguse2", 360),
        ("Prguse2", 361),
        ("Prguse2", 362),
        ("Prguse2", 257),
        ("Prguse2", 258),
        ("Prguse2", 259),
        ("Prguse", 1340),
        ("Prguse", 1341),
        ("Prguse", 1342),
        ("Prguse", 1343),
        ("Prguse", 1344),
        ("Prguse", 1346),
        ("Prguse", 1347),
        ("Prguse", 1349),
        ("Title", 45),
        ("Title", 149),
        ("Title", 180),
        ("Title", 181),
        ("Title", 182),
        ("Prguse", 360),
        ("Title", 200),
        ("Title", 201),
        ("Title", 202),
    ];
    if let Some(index) = model.mount_animation_index(now) {
        let base = index - ((now / 100) % 16) as u16;
        for frame in
            (u32::from(base)..u32::from(base) + 16).filter_map(|index| u16::try_from(index).ok())
        {
            frames.push(("Prguse", frame));
        }
    }
    if let Some(rod) = &model.rod {
        frames.push((
            "StateItem",
            if rod.shape == 49 { 1333 } else { 1335 } + u16::from(rod.has_slot(0)),
        ));
    }
    frames
}

fn image(
    parent: &mut ChildSpawnerCommands,
    assets: &AssetServer,
    lib: &str,
    index: u16,
    x: f32,
    y: f32,
    frame: OriginalFrame,
    use_offset: bool,
) {
    let rect = CrystalRect::new(
        x + if use_offset { frame.x } else { 0.0 },
        y + if use_offset { frame.y } else { 0.0 },
        frame.width,
        frame.height,
    );
    spawn_static_overlay_sprite(
        parent,
        assets,
        format!("original-ui/{lib}/{index}.png"),
        rect,
    );
}

fn button(
    parent: &mut ChildSpawnerCommands,
    assets: &AssetServer,
    lib: &'static str,
    ids: [u16; 3],
    position: Vec2,
    frame: OriginalFrame,
    action: EquipmentAction,
    origin: Vec2,
    hits: &mut Vec<DialogHit>,
) {
    let rect = CrystalRect::new(position.x, position.y, frame.width, frame.height);
    let spec = CrystalButtonSpec::new(lib, ids[0], ids[1], ids[2], rect, frame.width, frame.height);
    spawn_crystal_image_button(
        parent,
        assets,
        spec,
        CrystalButtonAssetSet::from_spec(spec),
        action,
        false,
        true,
    );
    hits.push(DialogHit {
        rect: CrystalRect::new(
            origin.x + rect.left,
            origin.y + rect.top,
            rect.width,
            rect.height,
        ),
        action: HitAction::Button(action),
    });
}

fn cells(
    parent: &mut ChildSpawnerCommands,
    assets: &AssetServer,
    kind: EquipmentDialog,
    host: &AttachmentHost,
    origin: Vec2,
    viewer: &crate::read_model::PlayerStats,
    hits: &mut Vec<DialogHit>,
) {
    for slot in 0..host.slot_count() {
        let rect = attachment_rect(kind, slot, host.slot_count()).unwrap();
        let cell = AttachmentCell { kind, slot };
        hits.push(DialogHit {
            rect: CrystalRect::new(
                origin.x + rect.left,
                origin.y + rect.top,
                rect.width,
                rect.height,
            ),
            action: HitAction::Cell(cell),
        });
        let item = host.slots.get(slot).and_then(Option::as_ref);
        let mut entity = parent.spawn((
            cell,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(rect.left),
                top: Val::Px(rect.top),
                width: Val::Px(rect.width),
                height: Val::Px(rect.height),
                ..default()
            },
            Interaction::None,
            FocusPolicy::Block,
        ));
        if let Some(item) = item {
            entity.insert(CrystalItemHint(crystal_item_tooltip_document(item, viewer)));
            entity.with_children(|cell| {
                if let Some(index) = item.user_item_image_index() {
                    let (marker, node, image) =
                        original_item_image_bundle(assets, Some(index), 34, 30);
                    cell.spawn((marker, node, image));
                }
                if item.quantity > 1 {
                    overlay_inventory_count(cell, &item.quantity.to_string(), 34.0, 30.0);
                }
            });
        }
    }
}

fn bar(
    parent: &mut ChildSpawnerCommands,
    assets: &AssetServer,
    index: u16,
    x: f32,
    y: f32,
    height: f32,
    percent: i32,
) {
    let width = (2.16 * percent.clamp(0, 100) as f32).floor();
    parent
        .spawn(Node {
            position_type: PositionType::Absolute,
            left: Val::Px(x),
            top: Val::Px(y),
            width: Val::Px(width),
            height: Val::Px(height),
            overflow: Overflow::clip(),
            ..default()
        })
        .with_children(|clip| {
            spawn_static_overlay_sprite(
                clip,
                assets,
                format!("original-ui/Prguse/{index}.png"),
                CrystalRect::new(0.0, 0.0, 216.0, height),
            );
        });
}

/// Rebuild under the fixed 1024x768 OverlayRoot. Returned hit rectangles use
/// that logical stage. Process the last matching rectangle first; Notice blocks all input.
pub fn render(
    parent: &mut ChildSpawnerCommands,
    assets: &AssetServer,
    model: &mut MountFishingUi,
    now: u64,
    viewer: &crate::read_model::PlayerStats,
    frame: impl Fn(&str, u16) -> Option<OriginalFrame>,
) -> Vec<DialogHit> {
    let mut hits = Vec::new();
    if required_frames(model, now)
        .into_iter()
        .any(|(lib, index)| frame(lib, index).is_none())
    {
        return hits;
    }
    if model.mount_open {
        if let Some(host) = &model.mount {
            let four = host.slot_count() == 4;
            let origin = model.mount_position;
            let index = if four { 160 } else { 167 };
            let bg = frame("Prguse", index).unwrap();
            hits.push(DialogHit {
                rect: CrystalRect::new(origin.x, origin.y, bg.width, bg.height),
                action: HitAction::Drag(EquipmentWindow::Mount),
            });
            parent
                .spawn((
                    EquipmentWindow::Mount,
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(origin.x),
                        top: Val::Px(origin.y),
                        width: Val::Px(bg.width),
                        height: Val::Px(bg.height),
                        ..default()
                    },
                    GlobalZIndex(OVERLAY_HELP_SORTED_Z),
                    FocusPolicy::Block,
                ))
                .with_children(|p| {
                    image(p, assets, "Prguse", index, 0.0, 0.0, bg, false);
                    let text_width = if four { 208.0 } else { 260.0 };
                    overlay_centered_text_at(
                        p,
                        &host.item.name,
                        CrystalRect::new(30.0, 10.0, text_width, 15.0),
                        32.0 / 3.0,
                        TEXT,
                    );
                    let raw = host
                        .item
                        .tooltip_source
                        .as_ref()
                        .and_then(|s| s.user_item.as_ref())
                        .unwrap();
                    overlay_centered_text_at(
                        p,
                        &format!("{} / {} Loyalty", raw.current_dura, raw.max_dura),
                        CrystalRect::new(30.0, 30.0, text_width, 15.0),
                        32.0 / 3.0,
                        TEXT,
                    );
                    let ride = if four { 164 } else { 155 };
                    let x = if four { 210.0 } else { 262.0 };
                    button(
                        p,
                        assets,
                        "Prguse",
                        [ride, ride + 1, ride + 2],
                        Vec2::new(x, 70.0),
                        frame("Prguse", ride).unwrap(),
                        EquipmentAction::Ride,
                        origin,
                        &mut hits,
                    );
                    button(
                        p,
                        assets,
                        "Prguse2",
                        [360, 361, 362],
                        Vec2::new(if four { 245.0 } else { 297.0 }, 3.0),
                        frame("Prguse2", 360).unwrap(),
                        EquipmentAction::CloseMount,
                        origin,
                        &mut hits,
                    );
                    button(
                        p,
                        assets,
                        "Prguse2",
                        [257, 258, 259],
                        Vec2::new(if four { 221.0 } else { 274.0 }, 3.0),
                        frame("Prguse2", 257).unwrap(),
                        EquipmentAction::MountHelp,
                        origin,
                        &mut hits,
                    );
                    if let Some(anim) = model.mount_animation_index(now) {
                        image(
                            p,
                            assets,
                            "Prguse",
                            anim,
                            if four { 110.0 } else { 0.0 },
                            if four { 250.0 } else { 70.0 },
                            frame("Prguse", anim).unwrap(),
                            true,
                        );
                    }
                    cells(
                        p,
                        assets,
                        EquipmentDialog::Mount,
                        host,
                        origin,
                        viewer,
                        &mut hits,
                    );
                });
        }
    }
    if model.fishing_open {
        if let Some(host) = &model.rod {
            let bg = frame("Prguse", 1340).unwrap();
            let origin = *model.fishing_position.get_or_insert(Vec2::new(
                (1024.0 - bg.width) / 2.0,
                (768.0 - bg.height) / 2.0,
            ));
            hits.push(DialogHit {
                rect: CrystalRect::new(origin.x, origin.y, bg.width, bg.height),
                action: HitAction::Drag(EquipmentWindow::Fishing),
            });
            parent
                .spawn((
                    EquipmentWindow::Fishing,
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(origin.x),
                        top: Val::Px(origin.y),
                        width: Val::Px(bg.width),
                        height: Val::Px(bg.height),
                        ..default()
                    },
                    GlobalZIndex(OVERLAY_HELP_SORTED_Z),
                    FocusPolicy::Block,
                ))
                .with_children(|p| {
                    image(p, assets, "Prguse", 1340, 0.0, 0.0, bg, false);
                    overlay_centered_text_at(
                        p,
                        &host.item.name,
                        CrystalRect::new(10.0, 4.0, 180.0, 20.0),
                        32.0 / 3.0,
                        TEXT,
                    );
                    button(
                        p,
                        assets,
                        "Prguse2",
                        [360, 361, 362],
                        Vec2::new(175.0, 3.0),
                        frame("Prguse2", 360).unwrap(),
                        EquipmentAction::CloseFishing,
                        origin,
                        &mut hits,
                    );
                    let rod =
                        if host.shape == 49 { 1333 } else { 1335 } + u16::from(host.has_slot(0));
                    // FishingRod_BeforeDraw uses the owning dialog's Location, not its child origin.
                    image(
                        p,
                        assets,
                        "StateItem",
                        rod,
                        10.0,
                        40.0,
                        frame("StateItem", rod).unwrap(),
                        false,
                    );
                    cells(
                        p,
                        assets,
                        EquipmentDialog::Fishing,
                        host,
                        origin,
                        viewer,
                        &mut hits,
                    );
                });
        }
    }
    if model.status_open {
        let origin = model.status_position;
        hits.push(DialogHit {
            rect: CrystalRect::new(origin.x, origin.y, 244.0, 128.0),
            action: HitAction::Drag(EquipmentWindow::Status),
        });
        parent
            .spawn((
                EquipmentWindow::Status,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(origin.x),
                    top: Val::Px(origin.y),
                    width: Val::Px(244.0),
                    height: Val::Px(128.0),
                    ..default()
                },
                GlobalZIndex(OVERLAY_HELP_SORTED_Z + 1),
                FocusPolicy::Block,
            ))
            .with_children(|p| {
                image(
                    p,
                    assets,
                    "Prguse",
                    1341,
                    0.0,
                    0.0,
                    frame("Prguse", 1341).unwrap(),
                    false,
                );
                bar(p, assets, 1342, 14.0, 64.0, 12.0, model.chance);
                bar(p, assets, 1349, 14.0, 79.0, 8.0, model.progress);
                overlay_centered_text_at(
                    p,
                    &format!("{}%", model.chance),
                    CrystalRect::new(14.0, 62.0, 216.0, 12.0),
                    32.0 / 3.0,
                    TEXT,
                );
                button(
                    p,
                    assets,
                    "Prguse2",
                    [360, 361, 362],
                    Vec2::new(216.0, 4.0),
                    frame("Prguse2", 360).unwrap(),
                    EquipmentAction::CloseStatus,
                    origin,
                    &mut hits,
                );
                // Checked-out Crystal never makes its FishButton visible; retain disabled image149.
                image(
                    p,
                    assets,
                    "Title",
                    149,
                    47.0,
                    95.0,
                    frame("Title", 149).unwrap(),
                    false,
                );
                if model.can_auto_cast() {
                    button(
                        p,
                        assets,
                        "Title",
                        [180, 181, 182],
                        Vec2::new(110.0, 95.0),
                        frame("Title", 180).unwrap(),
                        EquipmentAction::AutoCast,
                        origin,
                        &mut hits,
                    );
                    let index = if model.auto_cast { 1344 } else { 1343 };
                    image(
                        p,
                        assets,
                        "Prguse",
                        index,
                        172.0,
                        95.0,
                        frame("Prguse", index).unwrap(),
                        false,
                    );
                }
                button(
                    p,
                    assets,
                    "Prguse",
                    [1346, 1346, 1346],
                    Vec2::new(135.0, 41.0),
                    frame("Prguse", 1346).unwrap(),
                    EquipmentAction::EscapeToggle,
                    origin,
                    &mut hits,
                );
                if model.escape_cancels {
                    image(
                        p,
                        assets,
                        "Prguse",
                        1347,
                        135.0,
                        41.0,
                        frame("Prguse", 1347).unwrap(),
                        false,
                    );
                }
                image(
                    p,
                    assets,
                    "Title",
                    45,
                    150.0,
                    40.0,
                    frame("Title", 45).unwrap(),
                    false,
                );
            });
    }
    if let Some(notice) = model.notice {
        let bg = frame("Prguse", 360).unwrap();
        let origin = Vec2::new((1024.0 - bg.width) / 2.0, (768.0 - bg.height) / 2.0);
        hits.push(DialogHit {
            rect: CrystalRect::new(0.0, 0.0, 1024.0, 768.0),
            action: HitAction::Block,
        });
        parent
            .spawn((
                EquipmentWindow::Notice,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(origin.x),
                    top: Val::Px(origin.y),
                    width: Val::Px(bg.width),
                    height: Val::Px(bg.height),
                    ..default()
                },
                GlobalZIndex(OVERLAY_INVENTORY_DELETE_MODAL_Z),
                FocusPolicy::Block,
            ))
            .with_children(|p| {
                image(p, assets, "Prguse", 360, 0.0, 0.0, bg, false);
                overlay_text_at(
                    p,
                    notice.text(),
                    CrystalRect::new(35.0, 35.0, 390.0, 110.0),
                    32.0 / 3.0,
                    TEXT,
                );
                button(
                    p,
                    assets,
                    "Title",
                    [200, 201, 202],
                    Vec2::new(360.0, 157.0),
                    frame("Title", 200).unwrap(),
                    EquipmentAction::DismissNotice,
                    origin,
                    &mut hits,
                );
            });
    }
    hits
}
