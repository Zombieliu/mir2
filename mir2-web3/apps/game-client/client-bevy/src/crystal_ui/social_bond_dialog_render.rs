use super::super::*;
use super::*;

#[derive(Component)]
pub struct BondPanel(pub BondPage);
#[derive(Component)]
pub struct BondControl(pub BondPage, pub BondAction);
#[derive(Component)]
pub struct BondPromptPanel;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
pub struct BondAnswer {
    pub revision: u64,
    pub accept: bool,
}
#[derive(Component)]
struct BondDecoration;

pub const REQUIRED_ASSETS: &[(&str, &[u16])] = &[
    (
        "Title",
        &[
            51, 52, 213, 214, 215, 216, 217, 218, 200, 201, 202, 203, 204, 205, 206, 207, 208, 210,
            211, 212,
        ],
    ),
    (
        "Prguse",
        &[
            170, 583, 114, 115, 116, 117, 118, 119, 610, 611, 612, 600, 601, 602, 616, 617, 618,
            437, 438, 439, 566, 567, 568, 360, 660,
        ],
    ),
    ("Prguse2", &[360, 361, 362]),
];

fn buttons(page: BondPage, allow: u16) -> Vec<(&'static str, u16, f32, f32, BondAction)> {
    match page {
        BondPage::Mentor => vec![
            ("Prguse2", 360, 219., 3., BondAction::Close),
            ("Prguse", allow, 30., 178., BondAction::Allow),
            ("Title", 213, 60., 178., BondAction::AddMentor),
            ("Title", 216, 135., 178., BondAction::CancelMentor),
        ],
        BondPage::Relationship => vec![
            ("Prguse2", 360, 260., 3., BondAction::Close),
            ("Prguse", 610, 50., 164., BondAction::Allow),
            ("Prguse", 600, 85., 164., BondAction::Marriage),
            ("Prguse", 616, 120., 164., BondAction::Divorce),
            ("Prguse", 437, 155., 164., BondAction::Mail),
            ("Prguse", 566, 190., 164., BondAction::Whisper),
        ],
    }
}

pub fn hit_action(
    model: &SocialBondDialogs,
    page: BondPage,
    local: Vec2,
    dimensions: impl Fn(&str, u16) -> Option<Vec2>,
) -> Option<BondAction> {
    if model.prompt.is_some()
        || !match page {
            BondPage::Mentor => model.mentor.open,
            BondPage::Relationship => model.relationship.open,
        }
    {
        return None;
    }
    for (lib, index, x, y, action) in buttons(page, model.mentor.allow_frame) {
        let size = dimensions(lib, index)?;
        if inside(local, x, y, size.x, size.y) {
            return Some(action);
        }
    }
    None
}

pub fn render(
    parent: &mut ChildSpawnerCommands,
    assets: &AssetServer,
    model: &mut SocialBondDialogs,
    page: BondPage,
    viewport: Vec2,
    owner_name: &str,
    owner_level: u16,
    short_date: impl Fn(i64) -> String,
    dimensions: impl Fn(&str, u16) -> Option<Vec2>,
) -> bool {
    let (open, frame, title) = match page {
        BondPage::Mentor => (model.mentor.open, 170, 51),
        BondPage::Relationship => (model.relationship.open, 583, 52),
    };
    if !open {
        return false;
    }
    let Some(size) = dimensions("Prguse", frame) else {
        return false;
    };
    let Some(title_size) = dimensions("Title", title) else {
        return false;
    };
    let controls = buttons(page, model.mentor.allow_frame);
    for &(lib, index, _, _, _) in &controls {
        for i in index..=index + 2 {
            if dimensions(lib, i).is_none() {
                return false;
            }
        }
    }
    let labels = match page {
        BondPage::Mentor => model.mentor.labels(owner_name, owner_level),
        BondPage::Relationship => model.relationship.labels(short_date),
    };
    let position = match page {
        BondPage::Mentor => &mut model.mentor.position,
        BondPage::Relationship => &mut model.relationship.position,
    };
    let position =
        *position.get_or_insert([(viewport.x - size.x) / 2., (viewport.y - size.y) / 2.]);
    parent
        .spawn((
            BondPanel(page),
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
        .with_children(|p| {
            spawn_overlay_frame(
                p,
                assets,
                match frame {
                    170 => "original-ui/Prguse/170.png",
                    583 => "original-ui/Prguse/583.png",
                    660 => "original-ui/Prguse/660.png",
                    _ => "original-ui/Prguse/360.png",
                },
                size.x,
                size.y,
            );
            let spec = CrystalButtonSpec::new(
                "Title",
                title,
                title,
                title,
                CrystalRect::new(18., 8., title_size.x, title_size.y),
                title_size.x,
                title_size.y,
            );
            spawn_crystal_image_button(
                p,
                assets,
                spec,
                CrystalButtonAssetSet::from_spec(spec),
                BondDecoration,
                false,
                false,
            );
            for (lib, index, x, y, action) in controls {
                let d = dimensions(lib, index).unwrap();
                let spec = CrystalButtonSpec::new(
                    lib,
                    index,
                    index + 1,
                    index + 2,
                    CrystalRect::new(x, y, d.x, d.y),
                    d.x,
                    d.y,
                );
                spawn_crystal_image_button(
                    p,
                    assets,
                    spec,
                    CrystalButtonAssetSet::from_spec(spec),
                    (
                        BondControl(page, action),
                        crate::crystal_ui::widget::CrystalHint::new(hint(model, page, action)),
                    ),
                    false,
                    true,
                );
            }
            for label in labels {
                p.spawn(
                    (Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(label.x as f32),
                        top: Val::Px(label.y as f32),
                        width: Val::Px(label.width as f32),
                        height: Val::Px(30.),
                        align_items: AlignItems::Center,
                        overflow: Overflow::clip(),
                        ..default()
                    }),
                )
                .with_children(|p| {
                    p.spawn((
                        Text::new(label.text),
                        crate::crystal_ui::typography::crystal_text_font(label.font_points as f32 * 4. / 3.),
                        TextColor(match label.tone {
                            BondTone::DimGray => Color::srgb_u8(105, 105, 105),
                            BondTone::LightGray => Color::srgb_u8(211, 211, 211),
                            BondTone::Green => Color::srgb_u8(0, 128, 0),
                        }),
                        TextLayout::new(Justify::Left, LineBreak::NoWrap),
                    ));
                });
            }
        });
    true
}

pub fn hint(model: &SocialBondDialogs, page: BondPage, action: BondAction) -> &'static str {
    match (page, action) {
        (_, BondAction::Close) => "",
        (BondPage::Mentor, BondAction::Allow) => "Allow/Disallow Mentor Requests",
        (BondPage::Mentor, BondAction::AddMentor) => "Add Mentor",
        (BondPage::Mentor, BondAction::CancelMentor) => "Remove Mentor/Mentee",
        (BondPage::Relationship, BondAction::Allow) => model.relationship.allow_hint(),
        (BondPage::Relationship, BondAction::Marriage) => "Request Marriage",
        (BondPage::Relationship, BondAction::Divorce) => "Request Divorce",
        (BondPage::Relationship, BondAction::Mail) => "Mail Lover",
        (BondPage::Relationship, BondAction::Whisper) => "Whisper Lover",
        _ => "",
    }
}

pub fn prompt_hit(
    prompt: &BondPrompt,
    local: Vec2,
    dimensions: impl Fn(&str, u16) -> Option<Vec2>,
) -> Option<BondAnswer> {
    let input = matches!(prompt.kind, BondPromptKind::MentorName { .. });
    for (index, x, y, accept) in if input {
        [(200, 60., 123., true), (203, 160., 123., false)]
    } else {
        [(206, 260., 157., true), (210, 360., 157., false)]
    } {
        let size = dimensions("Title", index)?;
        if inside(local, x, y, size.x, size.y) {
            return Some(BondAnswer {
                revision: prompt.revision,
                accept,
            });
        }
    }
    None
}

pub fn render_prompt(
    parent: &mut ChildSpawnerCommands,
    assets: &AssetServer,
    prompt: &BondPrompt,
    viewport: Vec2,
    owner_class: &str,
    input_ui: &super::super::friend_dialog::FriendDialogUi,
    dimensions: impl Fn(&str, u16) -> Option<Vec2>,
) -> bool {
    let input = matches!(prompt.kind, BondPromptKind::MentorName { .. });
    let frame = if input { 660 } else { 360 };
    let Some(size) = dimensions("Prguse", frame) else {
        return false;
    };
    let controls = if input {
        [(200, 60., 123., true), (203, 160., 123., false)]
    } else {
        [(206, 260., 157., true), (210, 360., 157., false)]
    };
    for (index, _, _, _) in controls {
        for i in index..=index + 2 {
            if dimensions("Title", i).is_none() {
                return false;
            }
        }
    }
    parent
        .spawn((
            BondPromptPanel,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(((viewport.x - size.x) / 2.).floor()),
                top: Val::Px(((viewport.y - size.y) / 2.).floor()),
                width: Val::Px(size.x),
                height: Val::Px(size.y),
                ..default()
            },
            FocusPolicy::Block,
            GlobalZIndex(1100),
        ))
        .with_children(|p| {
            spawn_overlay_frame(
                p,
                assets,
                match frame {
                    170 => "original-ui/Prguse/170.png",
                    583 => "original-ui/Prguse/583.png",
                    660 => "original-ui/Prguse/660.png",
                    _ => "original-ui/Prguse/360.png",
                },
                size.x,
                size.y,
            );
            for (index, x, y, accept) in controls {
                let d = dimensions("Title", index).unwrap();
                let spec = CrystalButtonSpec::new(
                    "Title",
                    index,
                    index + 1,
                    index + 2,
                    CrystalRect::new(x, y, d.x, d.y),
                    d.x,
                    d.y,
                );
                spawn_crystal_image_button(
                    p,
                    assets,
                    spec,
                    CrystalButtonAssetSet::from_spec(spec),
                    BondAnswer {
                        revision: prompt.revision,
                        accept,
                    },
                    false,
                    true,
                );
            }
            p.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(if input { 25. } else { 35. }),
                    top: Val::Px(if input { 25. } else { 35. }),
                    width: Val::Px(if input { 235. } else { 390. }),
                    height: Val::Px(if input { 40. } else { 110. }),
                    ..default()
                },
                Text::new(prompt.text(owner_class)),
                crate::crystal_ui::typography::crystal_text_font(32. / 3.),
                TextColor(Color::WHITE),
            ));
            if input {
                super::super::friend_dialog::view::render_editor(
                    p,
                    input_ui,
                    CrystalRect::new(23., 86., 240., 19.),
                    false,
                );
            }
            if let Some(notice) = &input_ui.edit_notice {
                super::super::friend_dialog::view::wrapped_text(
                    p,
                    notice,
                    CrystalRect::new(15., size.y + 2., size.x - 30., 35.),
                    Color::srgb_u8(255, 90, 70),
                );
            }
        });
    true
}

fn inside(p: Vec2, x: f32, y: f32, w: f32, h: f32) -> bool {
    p.x >= x && p.x < x + w && p.y >= y && p.y < y + h
}
