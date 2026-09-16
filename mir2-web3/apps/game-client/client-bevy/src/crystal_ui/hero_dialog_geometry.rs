//! Source-sized current-frame hit regions; independent of recreated Bevy interaction nodes.
use super::super::{CrystalRect, CRYSTAL_CHARACTER_EQUIPMENT_SLOTS};
use super::{render::HeroAction, HeroDialogModel, HeroPage};
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum HeroWindow {
    Inventory,
    #[default]
    Character,
    Belt,
}
pub fn window_z(ui: &HeroDialogModel, window: HeroWindow) -> i32 {
    if ui.front == window {
        983
    } else {
        match window {
            HeroWindow::Character => 982,
            HeroWindow::Inventory => 981,
            HeroWindow::Belt => 980,
        }
    }
}
pub fn window_rect(ui: &HeroDialogModel, window: HeroWindow) -> Option<CrystalRect> {
    let (p, w, h) = match window {
        HeroWindow::Inventory if ui.inventory_open => (ui.inventory_position, 324., 266.),
        HeroWindow::Character if ui.character_open => {
            (ui.character_position.unwrap_or([760, 0]), 264., 380.)
        }
        HeroWindow::Belt if ui.belt_visible => (
            ui.belt_position.unwrap_or(if ui.belt_vertical {
                [0, 446]
            } else {
                [475, 618]
            }),
            if ui.belt_vertical { 40. } else { 100. },
            if ui.belt_vertical { 101. } else { 38. },
        ),
        _ => return None,
    };
    Some(CrystalRect::new(p[0] as f32, p[1] as f32, w, h))
}
pub fn hit(
    ui: &HeroDialogModel,
    point: [f32; 2],
    front: HeroWindow,
) -> (Option<HeroWindow>, Option<HeroAction>) {
    let contains = |r: CrystalRect| r.contains(point[0], point[1]);
    if ui.use_confirmation.is_some() {
        for (action, x) in [
            (HeroAction::UseConfirm, 544.),
            (HeroAction::UseCancel, 644.),
        ] {
            if contains(CrystalRect::new(x, 446., 76., 25.)) {
                return (None, Some(action));
            }
        }
        return (None, None);
    }
    if let Some((_, input)) = ui.amount.as_ref() {
        for (action, r) in [
            (
                HeroAction::AmountCancel,
                CrystalRect::new(590., 332., 24., 21.),
            ),
            (
                HeroAction::AmountCancel,
                CrystalRect::new(520., 405., 76., 25.),
            ),
            (
                HeroAction::AmountConfirm,
                CrystalRect::new(433., 405., 76., 25.),
            ),
        ] {
            if contains(r) && (action != HeroAction::AmountConfirm || input.amount().is_some()) {
                return (None, Some(action));
            }
        }
        return (None, None);
    }
    if ui.assign.open {
        if ui.assign.pending.is_some() {
            return (None, None);
        }
        for i in 0..8u8 {
            if contains(CrystalRect::new(
                349. + 32. * f32::from(i) + 5. * f32::from(i / 4),
                369.,
                28.,
                30.,
            )) {
                return (None, Some(HeroAction::AssignKey(i + 17)));
            }
        }
        if contains(CrystalRect::new(616., 375., 64., 28.)) {
            return (None, Some(HeroAction::AssignKey(0)));
        }
        if contains(CrystalRect::new(616., 412., 64., 28.)) {
            return (None, Some(HeroAction::AssignSave));
        }
        return (None, None);
    }
    let order = [
        front,
        HeroWindow::Character,
        HeroWindow::Inventory,
        HeroWindow::Belt,
    ];
    for window in order {
        let Some(rect) = window_rect(ui, window).filter(|r| contains(*r)) else {
            continue;
        };
        let x = point[0] - rect.left;
        let y = point[1] - rect.top;
        let hit = |r: CrystalRect| r.contains(x, y);
        let action = match window {
            HeroWindow::Inventory => {
                if hit(CrystalRect::new(299., 2., 24., 21.)) {
                    Some(HeroAction::CloseInventory)
                } else if let Some(cell) = (0..40usize).find(|cell| {
                    ui.inventory_cell_enabled(*cell)
                        && hit(CrystalRect::new(
                            14. + (*cell % 8) as f32 * 37.,
                            23. + (*cell / 8) as f32 * 33.,
                            36.,
                            32.,
                        ))
                }) {
                    Some(HeroAction::InventoryCell((cell + 2) as u8))
                } else if ui.info.as_ref().is_some_and(|i| i.auto_pot) {
                    [
                        (HeroAction::AutoHp, CrystalRect::new(58., 206., 60., 25.)),
                        (HeroAction::AutoMp, CrystalRect::new(206., 206., 60., 25.)),
                        (
                            HeroAction::AutoPotItem(true),
                            CrystalRect::new(122., 211., 36., 32.),
                        ),
                        (
                            HeroAction::AutoPotItem(false),
                            CrystalRect::new(166., 211., 36., 32.),
                        ),
                    ]
                    .into_iter()
                    .find(|(_, r)| hit(*r))
                    .map(|(a, _)| a)
                } else {
                    None
                }
            }
            HeroWindow::Character => {
                if hit(CrystalRect::new(241., 3., 24., 21.)) {
                    Some(HeroAction::CloseCharacter)
                } else if let Some(i) =
                    (0..4).find(|i| hit(CrystalRect::new(8. + *i as f32 * 62., 70., 64., 20.)))
                {
                    Some(HeroAction::Page(
                        [
                            HeroPage::Equipment,
                            HeroPage::Status,
                            HeroPage::State,
                            HeroPage::Skills,
                        ][i],
                    ))
                } else if ui.page == HeroPage::Equipment {
                    CRYSTAL_CHARACTER_EQUIPMENT_SLOTS
                        .into_iter()
                        .find(|(_, r)| hit(*r))
                        .map(|(slot, _)| HeroAction::EquipmentCell(slot as u8))
                } else if ui.page == HeroPage::Skills {
                    if hit(CrystalRect::new(98., 340., 16., 14.)) {
                        Some(HeroAction::Previous)
                    } else if hit(CrystalRect::new(148., 340., 16., 14.)) {
                        Some(HeroAction::Next)
                    } else {
                        (0..7usize)
                            .find(|row| {
                                ui.info
                                    .as_ref()
                                    .is_some_and(|i| ui.skill_start + *row < i.magics.len())
                                    && hit(CrystalRect::new(52., 98. + *row as f32 * 33., 36., 34.))
                            })
                            .map(|row| HeroAction::Skill(ui.skill_start + row))
                    }
                } else {
                    None
                }
            }
            HeroWindow::Belt => {
                let (close, rotate) = if ui.belt_vertical {
                    (
                        CrystalRect::new(3., 82., 16., 16.),
                        CrystalRect::new(19., 82., 16., 16.),
                    )
                } else {
                    (
                        CrystalRect::new(82., 19., 16., 14.),
                        CrystalRect::new(82., 3., 16., 16.),
                    )
                };
                if hit(close) {
                    Some(HeroAction::BeltClose)
                } else if hit(rotate) {
                    Some(HeroAction::BeltRotate)
                } else {
                    (0..2u8)
                        .find(|cell| {
                            hit(if ui.belt_vertical {
                                CrystalRect::new(3., 12. + f32::from(*cell) * 35., 32., 32.)
                            } else {
                                CrystalRect::new(12. + f32::from(*cell) * 35., 3., 32., 32.)
                            })
                        })
                        .map(HeroAction::InventoryCell)
                }
            }
        };
        return (Some(window), action);
    }
    (None, None)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn own_modal_regions_never_click_a_covered_belt_or_character_tab() {
        let mut ui = HeroDialogModel::default();
        ui.belt_visible = true;
        ui.character_open = true;
        ui.assign.open = true;
        assert_eq!(hit(&ui, [490., 625.], HeroWindow::Belt), (None, None));
        assert_eq!(
            hit(&ui, [350., 370.], HeroWindow::Belt),
            (None, Some(HeroAction::AssignKey(17)))
        );
        ui.assign.pending = Some(crate::hero_model::HeroKeyPending {
            hero_generation: 0,
            request_id: 1,
            session_epoch: 1,
            object_id: 1,
            snapshot_serial: 0,
            spell: mir2_protocol::Spell::FireBall,
            key: 17,
            old_key: 0,
        });
        assert_eq!(hit(&ui, [350., 370.], HeroWindow::Belt), (None, None));
    }
}
