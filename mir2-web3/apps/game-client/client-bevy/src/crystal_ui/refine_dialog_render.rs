use super::super::*;
use super::*;

pub const REQUIRED_ASSETS: &[(&str, &[u16])] = &[
    ("Prguse", &[1002]),
    ("Prguse2", &[351, 360, 361, 362]),
    ("Title", &[18, 290, 291, 292]),
];

#[derive(Component)]
pub struct RefinePanel;
#[derive(Component)]
struct RefineDecoration;
#[derive(Component)]
struct RefineConfirm;
#[derive(Component)]
struct RefineClose;

const MATERIAL_ORIGIN: Vec2 = Vec2::new(0.0, 225.0);
const TARGET_ORIGIN: Vec2 = Vec2::new(264.0, 224.0);

pub fn material_rect(slot: u8) -> CrystalRect {
    CrystalRect::new(
        MATERIAL_ORIGIN.x + 12.0 + f32::from(slot % 4) * 35.0,
        MATERIAL_ORIGIN.y + 37.0 + f32::from(slot / 4) * 33.0,
        34.0,
        32.0,
    )
}
pub fn material_cell_at(cursor: Vec2) -> Option<u8> {
    (0..16).find(|slot| material_rect(*slot).contains(cursor.x, cursor.y))
}
pub fn target_click_rect() -> CrystalRect {
    CrystalRect::new(TARGET_ORIGIN.x + 20.0, TARGET_ORIGIN.y + 55.0, 75.0, 75.0)
}
pub fn confirm_rect() -> CrystalRect {
    CrystalRect::new(TARGET_ORIGIN.x + 114.0, TARGET_ORIGIN.y + 62.0, 48.0, 25.0)
}
pub fn close_rect() -> CrystalRect {
    CrystalRect::new(TARGET_ORIGIN.x + 150.0, TARGET_ORIGIN.y + 3.0, 24.0, 24.0)
}
pub fn covers(model: &RefineDialogUi, cursor: Vec2) -> bool {
    model.open
        && (CrystalRect::new(TARGET_ORIGIN.x, TARGET_ORIGIN.y, 176.0, 147.0)
            .contains(cursor.x, cursor.y)
            || (model.mode == Some(RefineMode::Refine)
                && CrystalRect::new(MATERIAL_ORIGIN.x, MATERIAL_ORIGIN.y, 176.0, 182.0)
                    .contains(cursor.x, cursor.y)))
}

fn item(
    parent: &mut ChildSpawnerCommands,
    assets: &AssetServer,
    item: &ItemModel,
    rect: CrystalRect,
) {
    parent
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(rect.left),
                top: Val::Px(rect.top),
                width: Val::Px(rect.width),
                height: Val::Px(rect.height),
                ..default()
            },
            FocusPolicy::Pass,
            crate::crystal_ui::widget::CrystalHint::new(crate::player_text::name(&item.name)),
        ))
        .with_children(|cell| {
            cell.spawn(original_item_image_bundle(
                assets,
                item.user_item_image_index(),
                rect.width as i32,
                rect.height as i32,
            ));
            let count = item.crystal_stack_label();
            if !count.is_empty() {
                cell.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        right: Val::Px(1.0),
                        bottom: Val::Px(1.0),
                        ..default()
                    },
                    Text::new(count),
                    crate::crystal_ui::typography::crystal_text_font(
                        crate::crystal_ui::typography::CRYSTAL_DEFAULT_FONT_SIZE_PX,
                    ),
                    TextColor(Color::srgb(1.0, 1.0, 0.0)),
                    FocusPolicy::Pass,
                ));
            }
        });
}

pub fn render(
    parent: &mut ChildSpawnerCommands,
    assets: &AssetServer,
    model: &RefineDialogUi,
    dimensions: impl Fn(&'static str, u16) -> Option<Vec2>,
) -> bool {
    if !model.open {
        return false;
    }
    for &(library, indexes) in REQUIRED_ASSETS {
        if indexes
            .iter()
            .any(|index| dimensions(library, *index).is_none())
        {
            return false;
        }
    }
    if model.mode == Some(RefineMode::Refine) {
        let size = dimensions("Prguse", 1002).unwrap();
        parent
            .spawn((
                RefinePanel,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(MATERIAL_ORIGIN.x),
                    top: Val::Px(MATERIAL_ORIGIN.y),
                    width: Val::Px(size.x),
                    height: Val::Px(size.y),
                    ..default()
                },
                FocusPolicy::Block,
                GlobalZIndex(OVERLAY_HELP_SORTED_Z - 2),
            ))
            .with_children(|parent| {
                spawn_overlay_frame(
                    parent,
                    assets,
                    "original-ui/Prguse/1002.png",
                    size.x,
                    size.y,
                );
                let title_size = dimensions("Title", 18).unwrap();
                let title = CrystalButtonSpec::new(
                    "Title",
                    18,
                    18,
                    18,
                    CrystalRect::new(28.0, 8.0, title_size.x, title_size.y),
                    title_size.x,
                    title_size.y,
                );
                spawn_crystal_image_button(
                    parent,
                    assets,
                    title,
                    CrystalButtonAssetSet::from_spec(title),
                    RefineDecoration,
                    false,
                    false,
                );
                for (slot, held) in model.materials.iter().enumerate() {
                    if let Some(held) = held {
                        let world_rect = material_rect(slot as u8);
                        item(
                            parent,
                            assets,
                            held,
                            CrystalRect::new(
                                world_rect.left - MATERIAL_ORIGIN.x,
                                world_rect.top - MATERIAL_ORIGIN.y,
                                world_rect.width,
                                world_rect.height,
                            ),
                        );
                    }
                }
            });
    }
    parent
        .spawn((
            RefinePanel,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(TARGET_ORIGIN.x),
                top: Val::Px(TARGET_ORIGIN.y),
                width: Val::Px(176.0),
                height: Val::Px(147.0),
                ..default()
            },
            FocusPolicy::Block,
            GlobalZIndex(OVERLAY_HELP_SORTED_Z - 2),
        ))
        .with_children(|parent| {
            // NPCDropPanel_BeforeDraw uses Prguse2/351, including refinement.
            spawn_overlay_frame(parent, assets, "original-ui/Prguse2/351.png", 176.0, 147.0);
            let confirm = CrystalButtonSpec::new(
                "Title",
                290,
                291,
                292,
                CrystalRect::new(114.0, 62.0, 48.0, 25.0),
                48.0,
                25.0,
            );
            spawn_crystal_image_button(
                parent,
                assets,
                confirm,
                CrystalButtonAssetSet::from_spec(confirm),
                RefineConfirm,
                false,
                model.target.is_some() && !model.pending(),
            );
            // Closing the source NPC closes both child refinement frames. This
            // standard close control also works when the parent text page is absent.
            let close = CrystalButtonSpec::new(
                "Prguse2",
                360,
                361,
                362,
                CrystalRect::new(150.0, 3.0, 24.0, 24.0),
                24.0,
                24.0,
            );
            spawn_crystal_image_button(
                parent,
                assets,
                close,
                CrystalButtonAssetSet::from_spec(close),
                RefineClose,
                false,
                true,
            );
            let label = if model.pending() {
                crate::native_i18n::tr("Waiting for server")
            } else if model.mode == Some(RefineMode::Check) {
                crate::native_i18n::key("client.CheckRefine", "Check Refine")
            } else if let Some(cost) = model.quote() {
                crate::native_i18n::format_key(
                    "game.npc.service_quote",
                    "{service}: {amount} gold",
                    &[
                        (
                            "service",
                            &crate::native_i18n::key("client.Refine", "Refine"),
                        ),
                        ("amount", &cost.to_string()),
                    ],
                )
            } else {
                crate::native_i18n::key("client.Refine", "Refine")
            };
            overlay_text_at(
                parent,
                &label,
                CrystalRect::new(30.0, 10.0, 115.0, 36.0),
                9.0,
                TEXT,
            );
            if let Some(target) = model.target.as_ref() {
                item(
                    parent,
                    assets,
                    &target.item,
                    CrystalRect::new(38.0, 72.0, 36.0, 32.0),
                );
            }
        });
    true
}
