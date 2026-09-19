//! Crystal Title468 and options/grade/input panels. Positions are logical 1024x768.
pub use super::super::mount_fishing_dialog::view::OriginalFrame;
use super::*;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
pub enum CreatureWindow {
    Main,
    Options,
    Grade,
    Input,
    Notice,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
pub struct CreatureTextInput;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HitAction {
    Button(CreatureAction),
    Drag,
    Input,
    Block,
}
#[derive(Debug, Clone, PartialEq)]
pub struct Hit {
    pub rect: CrystalRect,
    pub action: HitAction,
    pub hint: Option<String>,
}
pub fn required_frames(model: &CreatureUi) -> Vec<(&'static str, u16)> {
    let mut frames = vec![
        ("Title", 468),
        ("Title", 469),
        ("Prguse", 237),
        ("Prguse", 2086),
        ("Prguse", 2087),
        ("Prguse", 396),
        ("Prguse", 397),
        ("Prguse", 398),
        ("Prguse", 399),
        ("Prguse", 660),
        ("Prguse", 360),
    ];
    for id in [
        420, 427, 428, 530, 531, 532, 533, 535, 360, 361, 362, 257, 258, 259,
    ] {
        frames.push(("Prguse2", id));
    }
    for id in (570..=588)
        .chain(590..=595)
        .chain(610..=615)
        .chain(200..=205)
    {
        frames.push(("Title", id));
    }
    for pet in &model.creatures {
        if let Ok(icon) = u16::try_from(pet.icon) {
            frames.push(("Prguse2", icon));
        }
        if let Some((a, n, _, b, m, _)) = animation(pet.pet_type) {
            for id in (a..a + n).chain(b..b + m) {
                frames.push(("Prguse2", id));
            }
        }
    }
    frames
}
fn img(
    p: &mut ChildSpawnerCommands,
    a: &AssetServer,
    lib: &str,
    id: u16,
    x: f32,
    y: f32,
    f: OriginalFrame,
    offset: bool,
) {
    spawn_static_overlay_sprite(
        p,
        a,
        format!("original-ui/{lib}/{id}.png"),
        CrystalRect::new(
            x + if offset { f.x } else { 0.0 },
            y + if offset { f.y } else { 0.0 },
            f.width,
            f.height,
        ),
    );
}
fn btn(
    p: &mut ChildSpawnerCommands,
    a: &AssetServer,
    lib: &'static str,
    ids: [u16; 3],
    x: f32,
    y: f32,
    f: OriginalFrame,
    action: CreatureAction,
    enabled: bool,
    origin: Vec2,
    hits: &mut Vec<Hit>,
) {
    let rect = CrystalRect::new(x, y, f.width, f.height);
    let spec = CrystalButtonSpec::new(lib, ids[0], ids[1], ids[2], rect, f.width, f.height);
    spawn_crystal_image_button(
        p,
        a,
        spec,
        CrystalButtonAssetSet::from_spec(spec),
        action,
        false,
        enabled,
    );
    if enabled {
        hits.push(Hit {
            rect: CrystalRect::new(origin.x + x, origin.y + y, f.width, f.height),
            action: HitAction::Button(action),
            hint: None,
        });
    }
}
fn label(
    p: &mut ChildSpawnerCommands,
    text: &str,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    center: bool,
    color: Color,
) {
    let rect = CrystalRect::new(x, y, w, h);
    if center {
        overlay_centered_text_at(p, text, rect, 32.0 / 3.0, color);
    } else {
        overlay_text_at(p, text, rect, 32.0 / 3.0, color);
    }
}
fn clip(
    p: &mut ChildSpawnerCommands,
    a: &AssetServer,
    id: u16,
    x: f32,
    y: f32,
    f: OriginalFrame,
    percent: f32,
) {
    let w = (f.width * percent.clamp(0.0, 1.0)).floor();
    p.spawn(Node {
        position_type: PositionType::Absolute,
        left: Val::Px(x),
        top: Val::Px(y),
        width: Val::Px(w),
        height: Val::Px(f.height),
        overflow: Overflow::clip(),
        ..default()
    })
    .with_children(|p| img(p, a, "Prguse2", id, 0.0, 0.0, f, false));
}
fn panel<'a>(
    p: &'a mut ChildSpawnerCommands<'_>,
    kind: CreatureWindow,
    origin: Vec2,
    f: OriginalFrame,
    z: i32,
) -> EntityCommands<'a> {
    p.spawn((
        kind,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(origin.x),
            top: Val::Px(origin.y),
            width: Val::Px(f.width),
            height: Val::Px(f.height),
            ..default()
        },
        GlobalZIndex(z),
        FocusPolicy::Block,
    ))
}
fn time_text(seconds: i64) -> String {
    let s = seconds.max(0);
    if s >= 86400 {
        format!(
            "{}d {:02}h {:02}m {:02}s",
            s / 86400,
            (s % 86400) / 3600,
            (s % 3600) / 60,
            s % 60
        )
    } else if s >= 3600 {
        format!("{}h {:02}m {:02}s", s / 3600, (s % 3600) / 60, s % 60)
    } else if s >= 60 {
        format!("{}m {:02}s", s / 60, s % 60)
    } else {
        format!("{s}s")
    }
}
/// Returns ordered hit targets (last target wins). Host owns input events and transport.
pub fn render(
    parent: &mut ChildSpawnerCommands,
    assets: &AssetServer,
    model: &mut CreatureUi,
    now_ms: u64,
    now_dotnet_ticks: i64,
    frame: impl Fn(&str, u16) -> Option<OriginalFrame>,
) -> Vec<Hit> {
    let mut hits = vec![];
    if required_frames(model)
        .into_iter()
        .any(|(lib, id)| frame(lib, id).is_none())
    {
        return hits;
    }
    if model.open {
        let bg = frame("Title", 468).unwrap();
        let origin = *model.position.get_or_insert(Vec2::new(
            (1024.0 - bg.width) / 2.0,
            (768.0 - bg.height) / 2.0,
        ));
        hits.push(Hit {
            rect: CrystalRect::new(origin.x, origin.y, bg.width, bg.height),
            action: HitAction::Drag,
            hint: None,
        });
        panel(
            parent,
            CreatureWindow::Main,
            origin,
            bg,
            OVERLAY_HELP_SORTED_Z,
        )
        .with_children(|p| {
            img(p, assets, "Title", 468, 0.0, 0.0, bg, false);
            for (ids, x, action) in [
                ([360, 361, 362], bg.width - 25.0, CreatureAction::Close),
                ([257, 258, 259], bg.width - 48.0, CreatureAction::Help),
            ] {
                btn(
                    p,
                    assets,
                    "Prguse2",
                    ids,
                    x,
                    3.0,
                    frame("Prguse2", ids[0]).unwrap(),
                    action,
                    true,
                    origin,
                    &mut hits,
                );
            }
            for (id, x, y) in [(530, 185.0, 129.0), (427, 29.0, 348.0), (428, 215.0, 348.0)] {
                img(
                    p,
                    assets,
                    "Prguse2",
                    id,
                    x,
                    y,
                    frame("Prguse2", id).unwrap(),
                    false,
                );
            }
            label(
                p,
                &model.pearls.to_string(),
                53.0,
                348.0,
                140.0,
                21.0,
                false,
                TEXT,
            );
            let selected = model.selected();
            let ready = selected.is_some() && !model.pending();
            let summoned_selected = selected.is_some_and(|c| model.summoned == Some(c.pet_type));
            let summon_ids = if summoned_selected {
                [580, 581, 582]
            } else if model.summoned.is_some() {
                [593, 594, 595]
            } else {
                [576, 577, 578]
            };
            btn(
                p,
                assets,
                "Title",
                summon_ids,
                113.0,
                217.0,
                frame("Title", summon_ids[0]).unwrap(),
                if summoned_selected {
                    CreatureAction::Dismiss
                } else {
                    CreatureAction::Summon
                },
                ready && (summoned_selected || model.summoned.is_none()),
                origin,
                &mut hits,
            );
            btn(
                p,
                assets,
                "Title",
                [583, 584, 585],
                255.0,
                217.0,
                frame("Title", 583).unwrap(),
                CreatureAction::Release,
                ready && !summoned_selected,
                origin,
                &mut hits,
            );
            btn(
                p,
                assets,
                "Title",
                [573, 574, 575],
                375.0,
                160.0,
                frame("Title", 573).unwrap(),
                CreatureAction::Options,
                ready,
                origin,
                &mut hits,
            );
            let auto = selected.is_some_and(|c| c.pet_mode == 0);
            let mode = if auto { 610 } else { 613 };
            btn(
                p,
                assets,
                "Title",
                [mode, mode + 1, mode + 2],
                375.0,
                187.0,
                frame("Title", mode).unwrap(),
                CreatureAction::ToggleMode,
                ready,
                origin,
                &mut hits,
            );
            if model.rename_enabled && model.input.is_none() {
                btn(
                    p,
                    assets,
                    "Title",
                    [570, 571, 572],
                    344.0,
                    50.0,
                    frame("Title", 570).unwrap(),
                    CreatureAction::Rename,
                    ready,
                    origin,
                    &mut hits,
                );
            }
            for pet in model.creatures.iter().take(10) {
                if !(0..10).contains(&pet.slot_index) {
                    continue;
                }
                let x = 44.0 + (pet.slot_index % 5) as f32 * 81.0;
                let y = 259.0 + (pet.slot_index / 5) as f32 * 40.0;
                if let Ok(icon) = u16::try_from(pet.icon) {
                    let f = frame("Prguse2", icon).unwrap();
                    btn(
                        p,
                        assets,
                        "Prguse2",
                        [icon; 3],
                        x,
                        y,
                        f,
                        CreatureAction::Select(pet.slot_index),
                        true,
                        origin,
                        &mut hits,
                    );
                    if let Some(hit) = hits.last_mut() {
                        hit.hint = Some(pet.custom_name.clone());
                    }
                }
                if model.selected_slot == Some(pet.slot_index) {
                    img(
                        p,
                        assets,
                        "Prguse2",
                        535,
                        x - 2.0,
                        y - 2.0,
                        frame("Prguse2", 535).unwrap(),
                        false,
                    );
                }
            }
            if let Some(pet) = selected {
                label(p, &pet.custom_name, 170.0, 50.0, 166.0, 21.0, true, TEXT);
                let ticks = (pet.expire_binary_datetime as u64 & 0x3fff_ffff_ffff_ffff) as i64;
                let ticks = if ticks > 3_155_378_975_999_999_999 {
                    ticks - 0x4000_0000_0000_0000
                } else {
                    ticks
                };
                let expire = if ticks == 0 {
                    "Expire: Never".into()
                } else {
                    format!(
                        "Expire: {}",
                        time_text(ticks.saturating_sub(now_dotnet_ticks) / 10_000_000)
                    )
                };
                label(p, &expire, 140.0, 85.0, 350.0, 21.0, false, TEXT);
                let rules = &pet.creature_rules;
                let semi = if rules.semi_auto_pickup_enabled {
                    format!(
                        "{0}x{0} {1}semi-auto{2}",
                        rules.auto_pickup_range,
                        if rules.auto_pickup_enabled {
                            "auto/"
                        } else {
                            ""
                        },
                        if rules.mouse_pickup_enabled { ", " } else { "" }
                    )
                } else {
                    String::new()
                };
                let mouse = if rules.semi_auto_pickup_enabled {
                    format!("{0}x{0} mouse", rules.mouse_pickup_range)
                } else {
                    String::new()
                };
                label(
                    p,
                    &format!("Can pickup items ({semi}{mouse})."),
                    19.0,
                    161.0,
                    350.0,
                    15.0,
                    false,
                    TEXT,
                );
                if rules.can_produce_blackstone {
                    label(
                        p,
                        "Can produce BlackStones.",
                        19.0,
                        176.0,
                        350.0,
                        15.0,
                        false,
                        TEXT,
                    );
                    label(
                        p,
                        "Can produce Pearls, used to buy Creature items.",
                        19.0,
                        191.0,
                        350.0,
                        15.0,
                        false,
                        TEXT,
                    );
                }
                if let Some(id) = model.animation_frame(now_ms) {
                    img(
                        p,
                        assets,
                        "Prguse2",
                        id,
                        50.0,
                        110.0,
                        frame("Prguse2", id).unwrap(),
                        true,
                    );
                }
                let fullness = frame("Prguse2", 531).unwrap();
                clip(
                    p,
                    assets,
                    531,
                    185.0,
                    129.0,
                    fullness,
                    pet.fullness as f32 / 10000.0,
                );
                let min_x = 185.0
                    + (fullness.width * rules.minimal_fullness.clamp(0, 10000) as f32 / 10000.0)
                        .floor()
                    - 8.0;
                let now_x = if pet.fullness <= 0 {
                    179.0
                } else {
                    185.0 + (fullness.width * pet.fullness.min(10000) as f32 / 10000.0).floor()
                        - 8.0
                };
                for (id, x, y) in [(532, min_x, 118.0), (533, now_x, 143.0)] {
                    img(
                        p,
                        assets,
                        "Prguse2",
                        id,
                        x,
                        y,
                        frame("Prguse2", id).unwrap(),
                        false,
                    );
                }
                for (rect, hint) in [
                    (
                        CrystalRect::new(185.0, 129.0, fullness.width, fullness.height),
                        format!("{} / 10000", pet.fullness),
                    ),
                    (
                        CrystalRect::new(min_x, 118.0, 16.0, 11.0),
                        format!("Needed {}", rules.minimal_fullness),
                    ),
                    (
                        CrystalRect::new(215.0, 348.0, 150.0, 21.0),
                        if rules.can_produce_blackstone {
                            time_text(10800 - pet.blackstone_time)
                        } else {
                            "No Production.".into()
                        },
                    ),
                ] {
                    hits.push(Hit {
                        rect: CrystalRect::new(
                            origin.x + rect.left,
                            origin.y + rect.top,
                            rect.width,
                            rect.height,
                        ),
                        action: HitAction::Block,
                        hint: Some(hint),
                    });
                }
                clip(
                    p,
                    assets,
                    420,
                    242.0,
                    353.0,
                    frame("Prguse2", 420).unwrap(),
                    pet.blackstone_time as f32 / 10800.0,
                );
            }
        });
        if let Some(pet) = &model.options {
            let origin = origin + Vec2::new(450.0, 63.0);
            let bg = frame("Title", 469).unwrap();
            hits.push(Hit {
                rect: CrystalRect::new(origin.x, origin.y, bg.width, bg.height),
                action: HitAction::Block,
                hint: None,
            });
            panel(
                parent,
                CreatureWindow::Options,
                origin,
                bg,
                OVERLAY_HELP_SORTED_Z + 1,
            )
            .with_children(|p| {
                img(p, assets, "Title", 469, 0.0, 0.0, bg, false);
                let f = &pet.filter;
                let flags = [
                    f.pet_pickup_all,
                    f.pet_pickup_gold,
                    f.pet_pickup_weapons,
                    f.pet_pickup_armours,
                    f.pet_pickup_helmets,
                    f.pet_pickup_boots,
                    f.pet_pickup_belts,
                    f.pet_pickup_accessories,
                    f.pet_pickup_others,
                ];
                for (i, name) in [
                    "All Items",
                    "Gold",
                    "Weapons",
                    "Armours",
                    "Helmets",
                    "Boots",
                    "Belts",
                    "Jewelry",
                    "Others",
                ]
                .iter()
                .enumerate()
                {
                    let id = if flags[i] { 2087 } else { 2086 };
                    btn(
                        p,
                        assets,
                        "Prguse",
                        [id; 3],
                        16.0,
                        16.0 + i as f32 * 30.0,
                        frame("Prguse", id).unwrap(),
                        CreatureAction::Filter(i),
                        true,
                        origin,
                        &mut hits,
                    );
                    label(
                        p,
                        name,
                        36.0,
                        16.0 + i as f32 * 30.0,
                        100.0,
                        20.0,
                        false,
                        TEXT,
                    );
                }
                for (ids, x, action) in [
                    ([586, 587, 588], 10.0, CreatureAction::SaveOptions),
                    ([590, 591, 592], 60.0, CreatureAction::CancelOptions),
                ] {
                    btn(
                        p,
                        assets,
                        "Title",
                        ids,
                        x,
                        280.0,
                        frame("Title", ids[0]).unwrap(),
                        action,
                        true,
                        origin,
                        &mut hits,
                    );
                }
            });
            let origin = origin + Vec2::new(-1.0, -24.0);
            let bg = frame("Prguse", 237).unwrap();
            hits.push(Hit {
                rect: CrystalRect::new(origin.x, origin.y, bg.width, bg.height),
                action: HitAction::Block,
                hint: None,
            });
            panel(
                parent,
                CreatureWindow::Grade,
                origin,
                bg,
                OVERLAY_HELP_SORTED_Z + 1,
            )
            .with_children(|p| {
                img(p, assets, "Prguse", 237, 0.0, 0.0, bg, false);
                let grade = pet.pickup_grade.min(5) as usize;
                let color = [
                    TEXT,
                    Color::srgb(1.0, 1.0, 0.0),
                    Color::srgb(0.0, 0.75, 1.0),
                    Color::srgb(0.87, 0.63, 0.87),
                    Color::srgb(1.0, 0.55, 0.0),
                    Color::srgb(1.0, 0.0, 0.0),
                ][grade];
                label(
                    p,
                    ["All", "Common", "Rare", "Mythical", "Legendary", "Heroic"][grade],
                    8.0,
                    0.0,
                    70.0,
                    21.0,
                    false,
                    color,
                );
                for (ids, x, delta) in [([396, 396, 397], 96.0, 1), ([398, 398, 399], 76.0, -1)] {
                    btn(
                        p,
                        assets,
                        "Prguse",
                        ids,
                        x,
                        5.0,
                        frame("Prguse", ids[0]).unwrap(),
                        CreatureAction::Grade(delta),
                        true,
                        origin,
                        &mut hits,
                    );
                }
            });
        }
    }
    if let Some(input) = &model.input {
        let bg = frame("Prguse", 660).unwrap();
        let origin = Vec2::new((1024.0 - bg.width) / 2.0, (768.0 - bg.height) / 2.0);
        hits.push(Hit {
            rect: CrystalRect::new(0.0, 0.0, 1024.0, 768.0),
            action: HitAction::Block,
            hint: None,
        });
        panel(
            parent,
            CreatureWindow::Input,
            origin,
            bg,
            OVERLAY_INVENTORY_DELETE_MODAL_Z,
        )
        .with_children(|p| {
            img(p, assets, "Prguse", 660, 0.0, 0.0, bg, false);
            label(
                p,
                if input.purpose == InputPurpose::Rename {
                    "Please enter a new name for the creature."
                } else {
                    "Please enter the creature's name for verification."
                },
                25.0,
                25.0,
                235.0,
                40.0,
                false,
                TEXT,
            );
            super::super::friend_dialog::view::render_editor(
                p,
                &model.text_input,
                CrystalRect::new(23., 86., 240., 19.),
                false,
            );
            if let Some(notice) = &model.text_input.edit_notice {
                super::super::friend_dialog::view::wrapped_text(
                    p,
                    notice,
                    CrystalRect::new(15., bg.height + 2., bg.width - 30., 35.),
                    Color::srgb_u8(255, 90, 70),
                );
            }
            hits.push(Hit {
                rect: CrystalRect::new(origin.x + 23.0, origin.y + 86.0, 240.0, 19.0),
                action: HitAction::Input,
                hint: None,
            });
            for (ids, x, action) in [
                ([200, 201, 202], 60.0, CreatureAction::SubmitInput),
                ([203, 204, 205], 160.0, CreatureAction::CancelInput),
            ] {
                btn(
                    p,
                    assets,
                    "Title",
                    ids,
                    x,
                    123.0,
                    frame("Title", ids[0]).unwrap(),
                    action,
                    true,
                    origin,
                    &mut hits,
                );
            }
        });
    }
    if let Some(notice) = &model.notice {
        let bg = frame("Prguse", 360).unwrap();
        let origin = Vec2::new((1024.0 - bg.width) / 2.0, (768.0 - bg.height) / 2.0);
        hits.push(Hit {
            rect: CrystalRect::new(0.0, 0.0, 1024.0, 768.0),
            action: HitAction::Block,
            hint: None,
        });
        panel(
            parent,
            CreatureWindow::Notice,
            origin,
            bg,
            OVERLAY_INVENTORY_DELETE_MODAL_Z + 1,
        )
        .with_children(|p| {
            img(p, assets, "Prguse", 360, 0.0, 0.0, bg, false);
            label(p, notice, 35.0, 35.0, 390.0, 110.0, false, TEXT);
            btn(
                p,
                assets,
                "Title",
                [200, 201, 202],
                360.0,
                157.0,
                frame("Title", 200).unwrap(),
                CreatureAction::DismissNotice,
                true,
                origin,
                &mut hits,
            );
        });
    }
    hits
}
