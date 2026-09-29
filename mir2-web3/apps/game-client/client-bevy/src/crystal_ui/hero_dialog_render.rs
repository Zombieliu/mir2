use super::super::*;
use super::*;
use crate::hero_model::HeroModel;
#[derive(Component)]
pub struct HeroRoot;
#[derive(Component)]
pub struct HeroButtonVisual {
    library: String,
    index: u16,
    action: HeroAction,
}
pub fn button_visuals(
    state: Res<NativePlayerUiState>,
    assets: Option<Res<AssetServer>>,
    mut buttons: Query<(&HeroButtonVisual, &mut ImageNode)>,
) {
    let Some(assets) = assets else { return };
    for (button, mut image) in &mut buttons {
        let hovered = state.hero.hovered == Some(button.action);
        let pressed = hovered && state.hero.armed == Some(button.action);
        let index = match button.action {
            HeroAction::Page(_) => button.index,
            HeroAction::Skill(_) => button.index + u16::from(pressed),
            HeroAction::Previous | HeroAction::Next => button.index + u16::from(pressed),
            HeroAction::AssignKey(_) if button.index == 1658 => 1658,
            _ => button.index + if pressed { 2 } else { u16::from(hovered) },
        };
        image.image = assets.load(format!("original-ui/{}/{index}.png", button.library));
    }
}
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub enum HeroAction {
    CloseInventory,
    CloseCharacter,
    Page(HeroPage),
    BeltClose,
    BeltRotate,
    InventoryCell(u8),
    EquipmentCell(u8),
    UseConfirm,
    UseCancel,
    AmountConfirm,
    AmountCancel,
    AutoPotItem(bool),
    AssignKey(u8),
    AssignSave,
    AutoHp,
    AutoMp,
    Skill(usize),
    Previous,
    Next,
}
fn image(
    parent: &mut ChildSpawnerCommands,
    assets: &AssetServer,
    library: &str,
    index: u16,
    rect: CrystalRect,
    opacity: f32,
) {
    parent.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(rect.left),
            top: Val::Px(rect.top),
            width: Val::Px(rect.width),
            height: Val::Px(rect.height),
            ..default()
        },
        ImageNode {
            image: assets.load(format!("original-ui/{library}/{index}.png")),
            color: Color::srgba(1., 1., 1., opacity),
            ..default()
        },
        bevy::ui::FocusPolicy::Pass,
    ));
}
fn button(
    parent: &mut ChildSpawnerCommands,
    assets: &AssetServer,
    library: &str,
    index: u16,
    rect: CrystalRect,
    action: HeroAction,
) {
    parent.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(rect.left),
            top: Val::Px(rect.top),
            width: Val::Px(rect.width),
            height: Val::Px(rect.height),
            ..default()
        },
        ImageNode::new(assets.load(format!("original-ui/{library}/{index}.png"))),
        Button,
        action,
        HeroButtonVisual {
            library: library.into(),
            index,
            action,
        },
    ));
}
fn item(
    parent: &mut ChildSpawnerCommands,
    assets: &AssetServer,
    state: &NativePlayerUiState,
    model: &HeroModel,
    container: u8,
    slot: u32,
    rect: CrystalRect,
    action: HeroAction,
) {
    let mut cell = parent.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(rect.left),
            top: Val::Px(rect.top),
            width: Val::Px(rect.width),
            height: Val::Px(rect.height),
            ..default()
        },
        Button,
        action,
    ));
    if let Some(item) = model
        .inventory_view
        .items
        .iter()
        .find(|item| item.container == container && item.slot == slot)
    {
        if let Some(info) = model.info.as_ref() {
            let mut actor = actor_ui(info);
            actor.player.crystal_stats = model.stats.clone();
            cell.insert(CrystalItemHint(crystal_item_tooltip_document(
                item,
                &actor.player,
            )));
        }
        let selected = state
            .hero
            .selected
            .is_some_and(|(gear, selected_slot, id)| {
                container == if gear { 2 } else { 0 }
                    && slot == u32::from(selected_slot)
                    && item.unique_id == Some(id)
            })
            || (container == 0
                && state.hero.cross.selected_cell() == Some(cross::Cell::Hero(slot as u8)));
        cell.with_children(|parent| {
            if let Some(index) = item.user_item_image_index() {
                let (marker, node, image) = original_item_image_bundle(
                    assets,
                    Some(index),
                    rect.width as i32,
                    rect.height as i32,
                );
                parent.spawn((marker, node, image));
            }
            if selected
                && matches!(
                    action,
                    HeroAction::InventoryCell(_) | HeroAction::EquipmentCell(_)
                )
            {
                parent.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(0.),
                        top: Val::Px(0.),
                        width: Val::Px(rect.width),
                        height: Val::Px(rect.height),
                        border: UiRect::all(Val::Px(1.)),
                        ..default()
                    },
                    BorderColor::all(Color::srgb(1., 0.85, 0.2)),
                    bevy::ui::FocusPolicy::Pass,
                ));
            }
            let count = inventory_cell_stack_label(item);
            if !count.is_empty() {
                overlay_text_at(
                    parent,
                    &count,
                    CrystalRect::new(0., 17., 36., 14.),
                    (32. / 3.),
                    Color::WHITE,
                );
            }
        });
    }
}
pub fn actor_ui(info: &mir2_protocol::HeroUserInformation) -> UiReadModel {
    UiReadModel {
        player: crate::read_model::PlayerStats {
            hp: info.hp,
            mp: info.mp,
            level: u32::from(info.level),
            name: Some(info.name.clone()),
            class_name: Some(format!("{:?}", info.class)),
            gender: Some(format!("{:?}", info.gender)),
            hair: Some(info.hair),
            experience: info.experience,
            max_experience: info.max_experience,
            ..default()
        },
        ..default()
    }
}
pub fn render(
    mut commands: Commands,
    roots: Query<Entity, With<OverlayRoot>>,
    old: Query<Entity, With<HeroRoot>>,
    state: Res<NativePlayerUiState>,
    model: Res<HeroModel>,
    shell: Res<NativeShellModel>,
    assets: Option<Res<AssetServer>>,
    wings: Option<Res<CrystalCharacterWingMaterials>>,
) {
    for entity in &old {
        commands.entity(entity).despawn();
    }
    let Some(assets) = assets else {
        return;
    };
    if shell.screen != NativeShellScreen::InGame {
        return;
    }
    let (Ok(root), Some(info)) = (roots.single(), model.info.as_ref()) else {
        return;
    };
    let ui = &state.hero;
    let mut displayed = info.clone();
    ui.assign.overlay(&mut displayed);
    let info = &displayed;
    if ui.use_confirmation.is_some() {
        commands.entity(root).with_children(|parent| {
            parent
                .spawn((
                    HeroRoot,
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(284.),
                        top: Val::Px(289.),
                        width: Val::Px(456.),
                        height: Val::Px(190.),
                        ..default()
                    },
                    GlobalZIndex(1101),
                    bevy::ui::FocusPolicy::Block,
                ))
                .with_children(|p| {
                    image(
                        p,
                        &assets,
                        "Prguse",
                        360,
                        CrystalRect::new(0., 0., 456., 190.),
                        1.,
                    );
                    friend_dialog::view::wrapped_text(
                        p,
                        "Are you use you want to use this Potion?",
                        CrystalRect::new(35., 35., 390., 110.),
                        Color::WHITE,
                    );
                    for (id, x, action) in [
                        (206, 260., HeroAction::UseConfirm),
                        (210, 360., HeroAction::UseCancel),
                    ] {
                        button(
                            p,
                            &assets,
                            "Title",
                            id,
                            CrystalRect::new(x, 157., 76., 25.),
                            action,
                        );
                    }
                });
        });
    }
    if let Some((_, amount)) = ui.amount.as_ref() {
        render_amount(&mut commands, root, &assets, amount);
    }
    if ui.assign.open {
        render_assign(&mut commands, root, &assets, ui, &state.keyboard, info);
    }
    if ui.inventory_open {
        let p = ui.inventory_position;
        commands.entity(root).with_children(|parent| {
            parent
                .spawn((
                    HeroRoot,
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(p[0] as f32),
                        top: Val::Px(p[1] as f32),
                        width: Val::Px(324.),
                        height: Val::Px(266.),
                        ..default()
                    },
                    ImageNode::new(assets.load("original-ui/Prguse/1422.png")),
                    bevy::ui::FocusPolicy::Block,
                    GlobalZIndex(geometry::window_z(ui, geometry::HeroWindow::Inventory)),
                ))
                .with_children(|parent| {
                    button(
                        parent,
                        &assets,
                        "Prguse2",
                        360,
                        CrystalRect::new(299., 2., 24., 21.),
                        HeroAction::CloseInventory,
                    );
                    for cell in 0..40usize {
                        if ui.inventory_cell_enabled(cell) {
                            item(
                                parent,
                                &assets,
                                &state,
                                &model,
                                0,
                                (cell + 2) as u32,
                                CrystalRect::new(
                                    14. + (cell % 8) as f32 * 37.,
                                    23. + (cell / 8) as f32 * 33.,
                                    36.,
                                    32.,
                                ),
                                HeroAction::InventoryCell((cell + 2) as u8),
                            );
                        }
                    }
                    for row in 0..4 {
                        if ui.locked_row(row) {
                            image(
                                parent,
                                &assets,
                                "Prguse",
                                1423,
                                CrystalRect::new(14., 56. + row as f32 * 33., 300., 33.),
                                1.,
                            );
                        }
                    }
                    if info.auto_pot {
                        let mut preview = model.clone();
                        preview.inventory_view = model.auto_pot_view.clone();
                        item(
                            parent,
                            &assets,
                            &state,
                            &preview,
                            0,
                            0,
                            CrystalRect::new(122., 211., 36., 32.),
                            HeroAction::AutoPotItem(true),
                        );
                        item(
                            parent,
                            &assets,
                            &state,
                            &preview,
                            0,
                            1,
                            CrystalRect::new(166., 211., 36., 32.),
                            HeroAction::AutoPotItem(false),
                        );
                        button(
                            parent,
                            &assets,
                            "Title",
                            560,
                            CrystalRect::new(58., 206., 60., 25.),
                            HeroAction::AutoHp,
                        );
                        button(
                            parent,
                            &assets,
                            "Title",
                            563,
                            CrystalRect::new(206., 206., 60., 25.),
                            HeroAction::AutoMp,
                        );
                        overlay_centered_text_at(
                            parent,
                            &format!("{}%", info.auto_hp_percent),
                            CrystalRect::new(58., 233., 60., 25.),
                            (32. / 3.),
                            Color::WHITE,
                        );
                        overlay_centered_text_at(
                            parent,
                            &format!("{}%", info.auto_mp_percent),
                            CrystalRect::new(206., 233., 60., 25.),
                            (32. / 3.),
                            Color::WHITE,
                        );
                    } else {
                        image(
                            parent,
                            &assets,
                            "Prguse",
                            1428,
                            CrystalRect::new(57., 196., 108., 62.),
                            1.,
                        );
                        image(
                            parent,
                            &assets,
                            "Prguse",
                            1429,
                            CrystalRect::new(162., 196., 108., 62.),
                            1.,
                        );
                    }
                });
        });
    }
    if ui.character_open {
        let p = ui.character_position.unwrap_or([760, 0]);
        commands.entity(root).with_children(|parent| {
            parent
                .spawn((
                    HeroRoot,
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(p[0] as f32),
                        top: Val::Px(p[1] as f32),
                        width: Val::Px(264.),
                        height: Val::Px(380.),
                        ..default()
                    },
                    ImageNode::new(assets.load("original-ui/Title/504.png")),
                    bevy::ui::FocusPolicy::Block,
                    GlobalZIndex(geometry::window_z(ui, geometry::HeroWindow::Character)),
                ))
                .with_children(|parent| {
                    button(
                        parent,
                        &assets,
                        "Prguse2",
                        360,
                        CrystalRect::new(241., 3., 24., 21.),
                        HeroAction::CloseCharacter,
                    );
                    overlay_centered_text_at(
                        parent,
                        &info.name,
                        CrystalRect::new(0., 12., 264., 18.),
                        (32. / 3.),
                        Color::WHITE,
                    );
                    image(
                        parent,
                        &assets,
                        "Prguse",
                        100 + info.class as u16,
                        CrystalRect::new(15., 33., 32., 32.),
                        1.,
                    );
                    for (i, page) in [
                        HeroPage::Equipment,
                        HeroPage::Status,
                        HeroPage::State,
                        HeroPage::Skills,
                    ]
                    .into_iter()
                    .enumerate()
                    {
                        let rect = CrystalRect::new(8. + i as f32 * 62., 70., 64., 20.);
                        if ui.page == page {
                            button(
                                parent,
                                &assets,
                                "Title",
                                500 + i as u16,
                                rect,
                                HeroAction::Page(page),
                            );
                        } else {
                            parent.spawn((
                                Node {
                                    position_type: PositionType::Absolute,
                                    left: Val::Px(rect.left),
                                    top: Val::Px(rect.top),
                                    width: Val::Px(rect.width),
                                    height: Val::Px(rect.height),
                                    ..default()
                                },
                                Button,
                                HeroAction::Page(page),
                            ));
                        }
                    }
                    let (library, index) = match ui.page {
                        HeroPage::Equipment => (
                            "Prguse",
                            if info.gender == mir2_protocol::MirGender::Male {
                                340
                            } else {
                                341
                            },
                        ),
                        HeroPage::Status => ("Title", 506),
                        HeroPage::State => ("Title", 507),
                        HeroPage::Skills => ("Title", 508),
                    };
                    image(
                        parent,
                        &assets,
                        library,
                        index,
                        CRYSTAL_CHARACTER_PAGE_RECT,
                        1.,
                    );
                    if ui.page == HeroPage::Equipment {
                        for (slot, rect) in CRYSTAL_CHARACTER_EQUIPMENT_SLOTS {
                            item(
                                parent,
                                &assets,
                                &state,
                                &model,
                                2,
                                slot,
                                rect,
                                HeroAction::EquipmentCell(slot as u8),
                            );
                        }
                        render_character_paper_doll(
                            parent,
                            &assets,
                            wings.as_deref(),
                            &model.inventory_view,
                            &actor_ui(info),
                        );
                    }
                    if matches!(ui.page, HeroPage::Status | HeroPage::State) {
                        let mut actor = actor_ui(info);
                        actor.player.crystal_stats = model.stats.clone();
                        let weights = model
                            .weights
                            .map(|weights| character_stats::Weights {
                                bag: Some(i64::from(weights.bag)),
                                wear: Some(i64::from(weights.wear)),
                                hand: Some(i64::from(weights.hand)),
                            })
                            .unwrap_or_default();
                        for (text, top) in character_stats::lines(
                            &actor.player,
                            ui.page == HeroPage::State,
                            weights,
                        ) {
                            overlay_text_at(
                                parent,
                                &text,
                                CrystalRect::new(134., top, 105., 16.),
                                32. / 3.,
                                Color::WHITE,
                            );
                        }
                    }
                    if ui.page == HeroPage::Skills {
                        for (row, magic) in
                            info.magics.iter().enumerate().skip(ui.skill_start).take(7)
                        {
                            let y = 98. + (row - ui.skill_start) as f32 * 33.;
                            button(
                                parent,
                                &assets,
                                "MagIcon2",
                                u16::from(magic.icon) * 2,
                                CrystalRect::new(52., y, 36., 34.),
                                HeroAction::Skill(row),
                            );
                            if let Some(frame) = model
                                .magic_clocks
                                .iter()
                                .find(|clock| clock.spell == magic.spell)
                                .and_then(|clock| {
                                    clock.dialog_frame(crate::hero_model::hero_clock_ms())
                                })
                            {
                                // Original Prguse2 1290..1324 are all 36x34 with zero offset.
                                image(
                                    parent,
                                    &assets,
                                    "Prguse2",
                                    frame,
                                    CrystalRect::new(52., y, 36., 34.),
                                    0.6,
                                );
                            }
                            image(
                                parent,
                                &assets,
                                "Title",
                                516,
                                CrystalRect::new(89., y + 7., 24., 9.),
                                1.,
                            );
                            image(
                                parent,
                                &assets,
                                "Title",
                                517,
                                CrystalRect::new(89., y + 19., 24., 11.),
                                1.,
                            );
                            overlay_text_at(
                                parent,
                                &magic.level.to_string(),
                                CrystalRect::new(104., y + 2., 21., 14.),
                                (32. / 3.),
                                Color::WHITE,
                            );
                            overlay_text_at(
                                parent,
                                &magic.name,
                                CrystalRect::new(125., y + 2., 125., 14.),
                                (32. / 3.),
                                Color::WHITE,
                            );
                            let exp = match magic.level {
                                0 => format!("{}/{}", magic.experience, magic.need1),
                                1 => format!("{}/{}", magic.experience, magic.need2),
                                2 => format!("{}/{}", magic.experience, magic.need3),
                                _ => "-".into(),
                            };
                            overlay_text_at(
                                parent,
                                &exp,
                                CrystalRect::new(125., y + 15., 125., 14.),
                                (32. / 3.),
                                Color::WHITE,
                            );
                            let key = if (17..=24).contains(&magic.key) {
                                format!("Shift\nF{}", magic.key - 16)
                            } else {
                                String::new()
                            };
                            overlay_text_at(
                                parent,
                                &key,
                                CrystalRect::new(18., y + 2., 34., 30.),
                                (32. / 3.),
                                Color::WHITE,
                            );
                        }
                        button(
                            parent,
                            &assets,
                            "Prguse",
                            398,
                            CrystalRect::new(98., 340., 16., 14.),
                            HeroAction::Previous,
                        );
                        button(
                            parent,
                            &assets,
                            "Prguse",
                            396,
                            CrystalRect::new(148., 340., 16., 14.),
                            HeroAction::Next,
                        );
                    }
                });
        });
    }
    if ui.belt_visible {
        let vertical = ui.belt_vertical;
        let p = ui
            .belt_position
            .unwrap_or(if vertical { [0, 446] } else { [475, 618] });
        let (w, h, frame, overlay) = if vertical {
            (40., 101., 1943, 1946)
        } else {
            (100., 38., 1921, 1934)
        };
        commands.entity(root).with_children(|parent| {
            parent
                .spawn((
                    HeroRoot,
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(p[0] as f32),
                        top: Val::Px(p[1] as f32),
                        width: Val::Px(w),
                        height: Val::Px(h),
                        ..default()
                    },
                    GlobalZIndex(geometry::window_z(ui, geometry::HeroWindow::Belt)),
                    bevy::ui::FocusPolicy::Block,
                ))
                .with_children(|parent| {
                    image(
                        parent,
                        &assets,
                        "Prguse",
                        overlay,
                        CrystalRect::new(
                            0.,
                            0.,
                            if vertical { 40. } else { 92. },
                            if vertical { 92. } else { 38. },
                        ),
                        0.5,
                    );
                    image(
                        parent,
                        &assets,
                        "Prguse",
                        frame,
                        CrystalRect::new(0., 0., w, h),
                        1.,
                    );
                    for cell in 0..2 {
                        let rect = if vertical {
                            CrystalRect::new(3., 12. + cell as f32 * 35., 32., 32.)
                        } else {
                            CrystalRect::new(12. + cell as f32 * 35., 3., 32., 32.)
                        };
                        item(
                            parent,
                            &assets,
                            &state,
                            &model,
                            0,
                            cell,
                            rect,
                            HeroAction::InventoryCell(cell as u8),
                        );
                        overlay_text_at(
                            parent,
                            &(cell + 7).to_string(),
                            CrystalRect::new(
                                if vertical {
                                    -1.
                                } else {
                                    8. + cell as f32 * 35.
                                },
                                if vertical {
                                    11. + cell as f32 * 35.
                                } else {
                                    2.
                                },
                                26.,
                                14.,
                            ),
                            (32. / 3.),
                            Color::WHITE,
                        );
                    }
                    button(
                        parent,
                        &assets,
                        "Prguse",
                        if vertical { 1935 } else { 1923 },
                        if vertical {
                            CrystalRect::new(3., 82., 16., 16.)
                        } else {
                            CrystalRect::new(82., 19., 16., 14.)
                        },
                        HeroAction::BeltClose,
                    );
                    button(
                        parent,
                        &assets,
                        "Prguse",
                        if vertical { 1938 } else { 1926 },
                        if vertical {
                            CrystalRect::new(19., 82., 16., 16.)
                        } else {
                            CrystalRect::new(82., 3., 16., 16.)
                        },
                        HeroAction::BeltRotate,
                    );
                });
        });
    }
}

fn render_assign(
    commands: &mut Commands,
    root: Entity,
    assets: &AssetServer,
    ui: &HeroDialogModel,
    _keyboard: &keyboard_dialog::KeyboardDialogUi,
    info: &mir2_protocol::HeroUserInformation,
) {
    let assign = &ui.assign;
    let Some(magic) = info.magics.iter().find(|m| Some(m.spell) == assign.spell) else {
        return;
    };
    commands.entity(root).with_children(|parent| {
        parent
            .spawn((
                HeroRoot,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(322.),
                    top: Val::Px(312.),
                    width: Val::Px(380.),
                    height: Val::Px(144.),
                    ..default()
                },
                GlobalZIndex(1100),
                FocusPolicy::Block,
            ))
            .with_children(|p| {
                spawn_overlay_frame(p, assets, "original-ui/Prguse/710.png", 380., 144.);
                image(
                    p,
                    assets,
                    "MagIcon2",
                    u16::from(magic.icon) * 2,
                    CrystalRect::new(16., 16., 36., 34.),
                    1.,
                );
                overlay_centered_text_at(
                    p,
                    &format!("Select the Key for: {}", magic.name),
                    CrystalRect::new(49., 17., 230., 32.),
                    32. / 3.,
                    Color::WHITE,
                );
                for i in 0..8u8 {
                    let rect = CrystalRect::new(
                        17. + 32. * f32::from(i) + 5. * f32::from(i / 4),
                        58.,
                        32.,
                        32.,
                    );
                    button(
                        p,
                        assets,
                        "Prguse",
                        if assign.key == 17 + i { 1658 } else { 1656 },
                        rect,
                        HeroAction::AssignKey(17 + i),
                    );
                    let label = format!("Shift\nF{}", i + 1);
                    overlay_text_at(
                        p,
                        &label,
                        CrystalRect::new(rect.left + 1., rect.top, rect.width - 1., rect.height),
                        32. / 3.,
                        Color::WHITE,
                    );
                }
                button(
                    p,
                    assets,
                    "Title",
                    287,
                    CrystalRect::new(284., 64., 76., 25.),
                    HeroAction::AssignKey(0),
                );
                button(
                    p,
                    assets,
                    "Title",
                    156,
                    CrystalRect::new(284., 101., 60., 25.),
                    HeroAction::AssignSave,
                );
                if let Some(notice) = assign.notice.as_deref() {
                    overlay_text_at(
                        p,
                        notice,
                        CrystalRect::new(16., 130., 260., 14.),
                        8.,
                        Color::WHITE,
                    );
                }
            });
    });
}

fn render_amount(
    commands: &mut Commands,
    root: Entity,
    assets: &AssetServer,
    input: &CrystalAmountInput,
) {
    commands.entity(root).with_children(|parent| {
        parent
            .spawn((
                HeroRoot,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(410.),
                    top: Val::Px(329.),
                    width: Val::Px(204.),
                    height: Val::Px(109.),
                    ..default()
                },
                GlobalZIndex(1100),
                FocusPolicy::Block,
            ))
            .with_children(|p| {
                spawn_overlay_frame(p, assets, "original-ui/Prguse/238.png", 204., 109.);
                overlay_text_at(
                    p,
                    "Enter Value",
                    CrystalRect::new(19., 8., 158., 14.),
                    32. / 3.,
                    Color::WHITE,
                );
                button(
                    p,
                    assets,
                    "Prguse2",
                    360,
                    CrystalRect::new(180., 3., 24., 21.),
                    HeroAction::AmountCancel,
                );
                p.spawn(Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(15.),
                    top: Val::Px(34.),
                    width: Val::Px(38.),
                    height: Val::Px(34.),
                    ..default()
                })
                .with_children(|p| spawn_original_item_image(p, assets, 116, 38, 34));
                if input.amount().is_some() {
                    button(
                        p,
                        assets,
                        "Title",
                        200,
                        CrystalRect::new(23., 76., 76., 25.),
                        HeroAction::AmountConfirm,
                    );
                }
                button(
                    p,
                    assets,
                    "Title",
                    203,
                    CrystalRect::new(110., 76., 76., 25.),
                    HeroAction::AmountCancel,
                );
                let border = match input.amount() {
                    None => Color::srgb(1., 0., 0.),
                    Some(v) if v == 99 => Color::srgb(1., 0.647, 0.),
                    Some(_) => Color::srgb(0., 1., 0.),
                };
                p.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(58.),
                        top: Val::Px(43.),
                        width: Val::Px(132.),
                        height: Val::Px(19.),
                        border: UiRect::all(Val::Px(1.)),
                        overflow: Overflow::clip(),
                        ..default()
                    },
                    BackgroundColor(Color::BLACK),
                    BorderColor::all(border),
                ))
                .with_children(|p| {
                    overlay_text_at(
                        p,
                        &input.draft,
                        CrystalRect::new(2., 1., 128., 17.),
                        32. / 3.,
                        Color::WHITE,
                    );
                });
            });
    });
}
