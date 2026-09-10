//! Child of overlays::friend_dialog; uses the common original-image renderer.
use super::super::*;
use super::*;
#[derive(Component)]
pub struct FriendPanel;
#[derive(Component)]
pub struct FriendControl(pub FriendAction);
#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
pub enum MemoAction {
    Save,
    Cancel,
    Edit,
}
#[derive(Component)]
pub struct MemoPanel;

const BUTTONS: [(&str, u16, f32, f32, FriendAction); 8] = [
    ("Prguse2", 360, 237., 3., FriendAction::Close),
    ("Prguse2", 240, 70., 218., FriendAction::Previous),
    ("Prguse2", 243, 171., 218., FriendAction::Next),
    ("Prguse", 554, 60., 241., FriendAction::Add),
    ("Prguse", 557, 88., 241., FriendAction::Remove),
    ("Prguse", 560, 116., 241., FriendAction::Memo),
    ("Prguse", 563, 144., 241., FriendAction::Mail),
    ("Prguse", 566, 172., 241., FriendAction::Whisper),
];
pub fn hit_action(
    model: &FriendDialogUi,
    local: Vec2,
    dimensions: impl Fn(&str, u16) -> Option<Vec2>,
) -> Option<FriendAction> {
    if !model.open || model.modal.is_some() {
        return None;
    }
    for (lib, index, x, y, action) in BUTTONS {
        let size = dimensions(lib, index)?;
        if contains(local, x, y, size.x, size.y) {
            return Some(action);
        }
    }
    for (index, x, action) in [
        (163, 10., FriendAction::Friends),
        (166, 128., FriendAction::Blocked),
    ] {
        let size = dimensions("Title", index)?;
        if contains(local, x, 34., size.x, size.y) {
            return Some(action);
        }
    }
    for (row, _) in model.rows().enumerate() {
        if contains(
            local,
            16. + (row % 2) as f32 * 115.,
            55. + (row / 2) as f32 * 22.,
            115.,
            17.,
        ) {
            return Some(FriendAction::Select(row));
        }
    }
    None
}
fn contains(p: Vec2, x: f32, y: f32, w: f32, h: f32) -> bool {
    p.x >= x && p.y >= y && p.x < x + w && p.y < y + h
}
pub fn memo_hit_action(
    local: Vec2,
    dimensions: impl Fn(&str, u16) -> Option<Vec2>,
) -> Option<MemoAction> {
    for (lib, index, x, y, action) in [
        ("Title", 382, 30., 133., MemoAction::Save),
        ("Title", 385, 115., 133., MemoAction::Cancel),
        ("Prguse2", 360, 168., 3., MemoAction::Cancel),
    ] {
        let d = dimensions(lib, index)?;
        if contains(local, x, y, d.x, d.y) {
            return Some(action);
        }
    }
    contains(local, 15., 30., 165., 100.).then_some(MemoAction::Edit)
}
pub fn memo_text_at(model: &FriendDialogUi, local: Vec2) -> Option<&str> {
    for (row, f) in model.rows().enumerate() {
        if contains(
            local,
            16. + (row % 2) as f32 * 115.,
            55. + (row / 2) as f32 * 22.,
            115.,
            17.,
        ) && !f.memo.is_empty()
        {
            return Some(&f.memo);
        }
    }
    None
}
/// The host supplies text editing/focus and dispatches modal_packet only on Save.
pub fn render_memo(
    parent: &mut ChildSpawnerCommands,
    assets: &AssetServer,
    model: &mut FriendDialogUi,
    viewport: Vec2,
    dimensions: impl Fn(&str, u16) -> Option<Vec2>,
) -> bool {
    let Some(FriendModal::Memo { .. }) = &model.modal else {
        return false;
    };
    let Some(size) = dimensions("Title", 209) else {
        return false;
    };
    let controls = [
        ("Title", 382, 30., 133., MemoAction::Save),
        ("Title", 385, 115., 133., MemoAction::Cancel),
        ("Prguse2", 360, 168., 3., MemoAction::Cancel),
    ];
    for (lib, index, ..) in controls {
        if (index..=index + 2).any(|i| dimensions(lib, i).is_none()) {
            return false;
        }
    }
    let position = *model
        .memo_position
        .get_or_insert([(viewport.x - size.x) / 2., (viewport.y - size.y) / 2.]);
    parent
        .spawn((
            MemoPanel,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(position[0]),
                top: Val::Px(position[1]),
                width: Val::Px(size.x),
                height: Val::Px(size.y),
                ..default()
            },
            FocusPolicy::Block,
            GlobalZIndex(OVERLAY_HELP_SORTED_Z + 1),
        ))
        .with_children(|panel| {
            spawn_overlay_frame(panel, assets, "original-ui/Title/209.png", size.x, size.y);
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
                    panel,
                    assets,
                    spec,
                    CrystalButtonAssetSet::from_spec(spec),
                    action,
                    false,
                    true,
                );
            }
            render_editor(panel, model, CrystalRect::new(15., 30., 165., 100.), true);
            if let Some(notice) = &model.edit_notice {
                wrapped_text(
                    panel,
                    notice,
                    CrystalRect::new(15., 158., 170., 40.),
                    Color::srgb_u8(255, 90, 70),
                );
            }
        });
    true
}
pub fn render(
    parent: &mut ChildSpawnerCommands,
    assets: &AssetServer,
    model: &mut FriendDialogUi,
    viewport: Vec2,
    dimensions: impl Fn(&str, u16) -> Option<Vec2>,
) -> bool {
    if !model.open {
        return false;
    }
    let Some(size) = dimensions("Title", 199) else {
        return false;
    };
    for (lib, index, ..) in BUTTONS {
        if (index..=index + 2).any(|i| dimensions(lib, i).is_none()) {
            return false;
        }
    }
    for i in [6, 163, 164, 166, 167] {
        if dimensions("Title", i).is_none() {
            return false;
        }
    }
    let position = *model
        .position
        .get_or_insert([(viewport.x - size.x) / 2., (viewport.y - size.y) / 2.]);
    parent
        .spawn((
            FriendPanel,
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
            spawn_overlay_frame(panel, assets, "original-ui/Title/199.png", size.x, size.y);
            for (lib, index, x, y, action) in BUTTONS {
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
                    panel,
                    assets,
                    spec,
                    CrystalButtonAssetSet::from_spec(spec),
                    FriendControl(action),
                    false,
                    true,
                );
            }
            for (index, x, y, action) in [
                (6, 18., 8., None),
                (
                    if model.blocked { 164 } else { 163 },
                    10.,
                    34.,
                    Some(FriendAction::Friends),
                ),
                (
                    if model.blocked { 166 } else { 167 },
                    128.,
                    34.,
                    Some(FriendAction::Blocked),
                ),
            ] {
                let d = dimensions("Title", index).unwrap();
                let spec = CrystalButtonSpec::new(
                    "Title",
                    index,
                    index,
                    index,
                    CrystalRect::new(x, y, d.x, d.y),
                    d.x,
                    d.y,
                );
                spawn_crystal_image_button(
                    panel,
                    assets,
                    spec,
                    CrystalButtonAssetSet::from_spec(spec),
                    FriendControl(action.unwrap_or(FriendAction::Friends)),
                    false,
                    action.is_some(),
                );
            }
            overlay_centered_text_at(
                panel,
                &format!("{} / {}", model.page + 1, model.page_count()),
                CrystalRect::new(87., 216., 83., 17.),
                32. / 3.,
                Color::WHITE,
            );
            for (row, f) in model.rows().enumerate() {
                let x = 16. + (row % 2) as f32 * 115.;
                let y = 55. + (row / 2) as f32 * 22.;
                panel
                    .spawn((
                        FriendControl(FriendAction::Select(row)),
                        Interaction::None,
                        FocusPolicy::Block,
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Px(x),
                            top: Val::Px(y),
                            width: Val::Px(115.),
                            height: Val::Px(17.),
                            ..default()
                        },
                        BackgroundColor(if model.selected == Some(f.index) {
                            Color::srgb_u8(128, 128, 128)
                        } else {
                            Color::NONE
                        }),
                    ))
                    .with_children(|r| {
                        overlay_text_at(
                            r,
                            &f.name,
                            CrystalRect::new(0., 0., 115., 17.),
                            32. / 3.,
                            if f.online {
                                Color::srgb_u8(0, 128, 0)
                            } else {
                                Color::WHITE
                            },
                        );
                    });
            }
        });
    true
}

#[derive(Component)]
pub struct FriendEditText {
    pub revision: u64,
    pub text: String,
    pub viewport: [f32; 2],
}

pub fn wrapped_text(
    parent: &mut ChildSpawnerCommands,
    text: &str,
    rect: CrystalRect,
    color: Color,
) {
    parent.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(rect.left),
            top: Val::Px(rect.top),
            width: Val::Px(rect.width),
            height: Val::Px(rect.height),
            overflow: Overflow::clip(),
            ..default()
        },
        Text::new(text),
        crate::crystal_ui::typography::crystal_text_font(32. / 3.),
        TextColor(color),
        TextLayout::new(Justify::Left, LineBreak::WordBoundary),
    ));
}

pub fn render_editor(
    parent: &mut ChildSpawnerCommands,
    model: &FriendDialogUi,
    rect: CrystalRect,
    multiline: bool,
) {
    render_editor_styled(parent, model, rect, multiline, Color::srgb_u8(0, 255, 0));
}
pub fn render_editor_styled(
    parent: &mut ChildSpawnerCommands,
    model: &FriendDialogUi,
    rect: CrystalRect,
    multiline: bool,
    border: Color,
) {
    let Some(editor) = model.display_editor() else {
        return;
    };
    parent
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(rect.left),
                top: Val::Px(rect.top),
                width: Val::Px(rect.width),
                height: Val::Px(rect.height),
                overflow: Overflow::clip(),
                border: UiRect::all(Val::Px(if multiline { 0. } else { 1. })),
                ..default()
            },
            BorderColor::all(border),
            FocusPolicy::Block,
        ))
        .with_children(|input| {
            let scroll = model.text_scroll;
            if model.layout_text == editor.text() {
                for r in model.text_layout.selection_rects(editor.selection()) {
                    input.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Px(r.x - scroll[0]),
                            top: Val::Px(r.y - scroll[1]),
                            width: Val::Px(r.width),
                            height: Val::Px(r.height),
                            ..default()
                        },
                        BackgroundColor(Color::srgb_u8(35, 85, 145)),
                    ));
                }
            }
            if let (Some(composition), Some(original)) = (&model.composition, &model.editor) {
                let start = original.selection().start;
                if model.layout_text == editor.text() {
                    for r in model
                        .text_layout
                        .selection_rects(start..start + composition.value.len())
                    {
                        input.spawn((
                            Node {
                                position_type: PositionType::Absolute,
                                left: Val::Px(r.x - scroll[0]),
                                top: Val::Px(r.y + r.height - 1. - scroll[1]),
                                width: Val::Px(r.width),
                                height: Val::Px(1.),
                                ..default()
                            },
                            BackgroundColor(Color::WHITE),
                        ));
                    }
                }
            }
            input.spawn((
                FriendEditText {
                    viewport: [rect.width, rect.height],
                    revision: model.editor_revision,
                    text: editor.text().to_owned(),
                },
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(-scroll[0]),
                    top: Val::Px(-scroll[1]),
                    width: if multiline {
                        Val::Px(rect.width)
                    } else {
                        Val::Auto
                    },
                    min_width: Val::Px(rect.width),
                    ..default()
                },
                Text::new(editor.text()),
                crate::crystal_ui::typography::crystal_text_font(32. / 3.),
                TextColor(Color::WHITE),
                TextLayout::new(
                    Justify::Left,
                    if multiline {
                        LineBreak::WordBoundary
                    } else {
                        LineBreak::NoWrap
                    },
                ),
            ));
            if model.editor_focused
                && model
                    .composition
                    .as_ref()
                    .is_none_or(|c| c.cursor.is_some())
            {
                let caret = model
                    .text_layout
                    .lines
                    .get(model.visual_line)
                    .and_then(|l| {
                        l.stops
                            .iter()
                            .find(|s| s.byte == editor.caret())
                            .map(|s| (s.x, l.y, l.height))
                    });
                let caret = caret.or_else(|| editor.text().is_empty().then_some((0., 0., 14.)));
                if let Some((x, y, h)) = caret {
                    input.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Px(x - scroll[0]),
                            top: Val::Px(y - scroll[1]),
                            width: Val::Px(1.),
                            height: Val::Px(h),
                            ..default()
                        },
                        BackgroundColor(Color::WHITE),
                    ));
                }
            }
        });
}

#[derive(Component)]
pub struct MemoTooltip;
/// Crystal Functions.StringOverLines(memo, 5, 20), including trailing spaces.
pub fn memo_hint_text(memo: &str) -> String {
    let mut out = String::new();
    let mut line_len = 0;
    for (i, word) in memo.split(' ').enumerate() {
        line_len += word.encode_utf16().count() + 1;
        out.push_str(word);
        out.push(' ');
        if i > 0 && i % 5 == 0 && line_len > 20 {
            line_len = 0;
            out.push('\n');
        }
    }
    out
}
