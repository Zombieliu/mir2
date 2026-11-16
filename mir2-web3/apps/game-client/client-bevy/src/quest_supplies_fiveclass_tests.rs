use super::*;
use crate::inventory::{
    CrystalItemTooltipSourceModel, CrystalUserItemExpireModel, CrystalUserItemModel,
    CrystalUserItemSealedModel,
};
use mir2_protocol::Spell;

const NOW: i64 = 638_000_000_000_000_000;

fn player(class: &str, level: u32) -> PlayerStats {
    PlayerStats {
        class_name: Some(class.into()),
        gender: Some("Male".into()),
        level,
        gold: 10_000,
        ..Default::default()
    }
}

fn source_item(index: i32, quantity: u32, container: u8, slot: u32, id: u64) -> ItemModel {
    let template = mir2_game_data::crystal_item_by_index(index).unwrap();
    ItemModel {
        name: template.name.clone(),
        quantity,
        container,
        slot,
        unique_id: Some(id),
        tooltip_source: Some(CrystalItemTooltipSourceModel {
            info: serde_json::from_value(serde_json::to_value(template).unwrap()).unwrap(),
            user_item: Some(CrystalUserItemModel {
                unique_id: id,
                item_index: index,
                count: quantity.try_into().unwrap(),
                ..Default::default()
            }),
            ..Default::default()
        }),
        ..Default::default()
    }
}

fn learned(spell: Spell, kind: &str) -> SkillModel {
    let canonical = format!("{spell:?}");
    let magic = mir2_game_data::crystal_magic_by_spell(&canonical).unwrap();
    serde_json::from_value(serde_json::json!({
        "skills": [{
            "id": spell as u32,
            "name": "已本地化的技能名称",
            "level": 0,
            "spell": canonical,
            "castKind": kind,
            "mpCost": magic.base_cost,
            "canUse": false,
            "cooldownRemainingMs": 1399
        }]
    }))
    .unwrap()
}

fn supplies(player: &PlayerStats, inventory: &InventoryModel, skills: &SkillModel) -> SupplyPlan {
    plan_at(player, inventory, Some(skills), None, NOW)
}

fn row<'a>(plan: &'a SupplyPlan, name: &str) -> Option<&'a SupplyRow> {
    plan.rows.iter().find(|row| row.label == name)
}

#[test]
fn fiveclass_active_learned_skills_show_real_mp_stock_and_potion_tier() {
    let inventory = InventoryModel {
        items: vec![
            source_item(659, 2, 0, 1, 1),
            source_item(661, 1, 1, 0, 2),
            source_item(661, 1, 1, 0, 2),
            source_item(663, 100, 4, 0, 3),
        ],
        ..Default::default()
    };
    for (class, level, spell, kind, expected_index, minimum, target) in [
        ("Assassin", 15, Spell::DoubleSlash, "toggle", 659, 4, 12),
        ("Archer", 9, Spell::StraightShot, "target", 659, 4, 12),
        // The real DoubleShot book admits level 11; MagicInfo level1=14 is its
        // skill-upgrade gate and must not replace the learned-book gate here.
        ("Archer", 11, Spell::DoubleShot, "target", 659, 4, 12),
        ("Assassin", 20, Spell::Haste, "self", 661, 8, 20),
        ("Assassin", 34, Spell::PoisonSword, "direction", 663, 12, 24),
        ("Archer", 50, Spell::PoisonShot, "target", 663, 12, 24),
    ] {
        let plan = supplies(&player(class, level), &inventory, &learned(spell, kind));
        let mp = row(&plan, "蓝药").expect("a learned MP-active skill needs mana supplies");
        let template = mir2_game_data::crystal_item_by_index(expected_index).unwrap();
        assert_eq!((mp.held, mp.minimum, mp.target), (3, minimum, target));
        assert_eq!(
            (&mp.item_name, mp.unit_price),
            (&template.name, template.price)
        );
        assert_eq!(mp.vendor, SupplyVendor::Potions);
        assert!(mp.is_low());
    }
}

#[test]
fn fiveclass_level_or_passive_learning_does_not_invent_mp_active_skills() {
    let inventory = InventoryModel::default();
    for class in ["Assassin", "Archer"] {
        for level in [7, 9, 15, 34, 50] {
            let plan = plan_at(&player(class, level), &inventory, None, None, NOW);
            assert!(row(&plan, "蓝药").is_none());
        }
    }
    for (class, level, spell) in [
        ("Assassin", 7, Spell::FatalSword),
        ("Assassin", 38, Spell::MPEater),
        ("Assassin", 47, Spell::Hemorrhage),
        ("Archer", 7, Spell::Focus),
        ("Archer", 11, Spell::MentalState),
        ("Archer", 19, Spell::Meditation),
    ] {
        let plan = supplies(
            &player(class, level),
            &inventory,
            &learned(spell, "passive"),
        );
        assert!(row(&plan, "蓝药").is_none(), "{class} {spell:?}");
    }
}

#[test]
fn fiveclass_mp_respects_source_class_book_gate_and_level_50_boundary() {
    for (class, level, spell) in [
        ("Assassin", 14, Spell::DoubleSlash),
        ("Archer", 8, Spell::StraightShot),
        ("Archer", 10, Spell::DoubleShot),
        ("Assassin", 34, Spell::StraightShot),
        ("Archer", 34, Spell::DoubleSlash),
        ("Assassin", 0, Spell::DoubleSlash),
        ("Assassin", 51, Spell::DoubleSlash),
        ("Archer", 51, Spell::StraightShot),
    ] {
        let plan = supplies(
            &player(class, level),
            &InventoryModel::default(),
            &learned(spell, "target"),
        );
        assert!(row(&plan, "蓝药").is_none(), "{class} {level} {spell:?}");
    }
}

#[test]
fn fiveclass_mp_requires_current_learned_entry_and_known_canonical_binding() {
    let player = player("Archer", 30);
    let inventory = InventoryModel::default();
    let learned = learned(Spell::StraightShot, "target");
    let mut malformed = Vec::new();
    let mut orphan = learned.clone();
    orphan.skills.clear();
    malformed.push(orphan);
    let mut unrelated = learned.clone();
    unrelated.bindings[0].skill_id += 1;
    malformed.push(unrelated);
    let mut display_only = learned.clone();
    display_only.bindings[0].spell = None;
    malformed.push(display_only);
    let mut missing = learned.clone();
    missing.bindings[0].cast_kind = None;
    malformed.push(missing);
    let mut future = learned.clone();
    future.bindings[0].cast_kind = Some("future".into());
    malformed.push(future);
    let mut passive = learned.clone();
    passive.bindings[0].cast_kind = Some("passive".into());
    malformed.push(passive);
    let mut excessive_upgrade = learned.clone();
    excessive_upgrade.skills[0].level = 4;
    malformed.push(excessive_upgrade);
    for skills in malformed {
        assert!(row(&supplies(&player, &inventory, &skills), "蓝药").is_none());
    }
    assert!(row(&supplies(&player, &inventory, &learned), "蓝药").is_some());
}

#[test]
fn fiveclass_mp_preserves_off_toggle_cooldown_explicit_zero_and_source_cost() {
    let player = player("Assassin", 15);
    let inventory = InventoryModel::default();
    let mut skill = learned(Spell::DoubleSlash, "toggle");
    assert_eq!(skill.bindings[0].can_use, Some(false));
    assert_eq!(skill.bindings[0].cooldown_remaining_ms, Some(1399));
    assert!(row(&supplies(&player, &inventory, &skill), "蓝药").is_some());
    skill.bindings[0].mp_cost = Some(0);
    skill.skills[0].mp_cost = 9;
    assert!(row(&supplies(&player, &inventory, &skill), "蓝药").is_none());
    skill.bindings[0].mp_cost = None;
    skill.skills[0].mp_cost = 0;
    assert_eq!(
        mir2_game_data::crystal_magic_by_spell("DoubleSlash")
            .unwrap()
            .base_cost,
        2
    );
    assert!(row(&supplies(&player, &inventory, &skill), "蓝药").is_some());
}

#[test]
fn assassin_poison_guidance_requires_actual_poisonsword_learning_34_to_50() {
    let inventory = InventoryModel::default();
    let learned = learned(Spell::PoisonSword, "direction");
    for level in [34, 50] {
        let plan = supplies(&player("Assassin", level), &inventory, &learned);
        assert!(row(&plan, "毒粉").is_some());
        assert_eq!(
            plan.assassin_poison_readiness,
            Some(AssassinPoisonReadiness::NeedsEquipment)
        );
    }
    for (class, level) in [
        ("Assassin", 33),
        ("Assassin", 51),
        ("Archer", 34),
        ("Warrior", 34),
        ("Taoist", 34),
    ] {
        let plan = supplies(&player(class, level), &inventory, &learned);
        assert!(row(&plan, "毒粉").is_none());
        assert_eq!(plan.assassin_poison_readiness, None);
    }
    let mut orphan = learned.clone();
    orphan.skills.clear();
    let plan = supplies(&player("Assassin", 34), &inventory, &orphan);
    assert!(row(&plan, "毒粉").is_none());
    assert_eq!(plan.assassin_poison_readiness, None);
    let plan = plan_at(
        &player("Assassin", 34),
        &inventory,
        None,
        Some(2110021),
        NOW,
    );
    assert!(row(&plan, "毒粉").is_none());
    assert_eq!(plan.assassin_poison_readiness, None);
}

#[test]
fn assassin_carried_red_green_powder_counts_for_purchases_but_needs_equipment() {
    let inventory = InventoryModel {
        items: vec![
            source_item(710, 20, 0, 1, 1),
            source_item(711, 5, 1, 1, 2),
            source_item(711, 5, 1, 1, 2),
        ],
        ..Default::default()
    };
    let plan = supplies(
        &player("Assassin", 34),
        &inventory,
        &learned(Spell::PoisonSword, "direction"),
    );
    let poison = row(&plan, "毒粉").unwrap();
    assert_eq!(
        (
            poison.held,
            poison.minimum,
            poison.target,
            poison.shortage()
        ),
        (25, 4, 20, 0)
    );
    assert!(!poison.is_low());
    assert_eq!(
        plan.assassin_poison_readiness,
        Some(AssassinPoisonReadiness::NeedsEquipment)
    );
    assert!(!plan.needs_taoist_material_guidance);
    assert_eq!(poison.vendor, SupplyVendor::Poison);
    assert_eq!(poison.item_name, "GreenPoison");
    let destination = poison
        .vendor
        .destination()
        .expect("the real powder merchant is loaded");
    let merchant = mir2_game_data::crystal_npc_info_manifest()
        .npcs
        .into_iter()
        .find(|npc| {
            npc.name == poison.vendor.npc_name()
                && npc.loaded_object_id == Some(destination.npc_object_id)
        })
        .unwrap();
    assert_eq!(merchant.map_file_name.as_deref(), Some("0109"));
    let local = poison.vendor.route(destination.map_index).unwrap();
    assert!(!local.is_entrance);
    assert_eq!((local.x, local.y), (destination.x, destination.y));
    assert!(poison.vendor.route(1).unwrap().is_entrance);
}

#[test]
fn assassin_one_equipped_red_or_green_is_cast_material_ready_but_stock_is_low() {
    for index in [710, 711] {
        for slot in [6, 9] {
            let inventory = InventoryModel {
                items: vec![source_item(index, 1, 2, slot, 1)],
                ..Default::default()
            };
            let plan = supplies(
                &player("Assassin", 34),
                &inventory,
                &learned(Spell::PoisonSword, "direction"),
            );
            let poison = row(&plan, "毒粉").unwrap();
            assert_eq!(
                plan.assassin_poison_readiness,
                Some(AssassinPoisonReadiness::Equipped)
            );
            assert_eq!(
                (poison.held, poison.shortage(), poison.unit_price),
                (1, 19, 40)
            );
            assert!(poison.is_low());
        }
    }
}

#[test]
fn assassin_wrong_powder_slot_container_shape_type_or_zero_count_is_not_ready() {
    for (container, slot, quantity, held) in [
        (0, 1, 20, 20),
        (1, 1, 20, 20),
        (2, 0, 20, 0),
        (2, 4, 20, 0),
        (2, 8, 20, 0),
        (3, 9, 20, 0),
        (4, 9, 20, 0),
        (2, 9, 0, 0),
    ] {
        let inventory = InventoryModel {
            items: vec![source_item(710, quantity, container, slot, 1)],
            ..Default::default()
        };
        let plan = supplies(
            &player("Assassin", 34),
            &inventory,
            &learned(Spell::PoisonSword, "direction"),
        );
        assert_eq!(row(&plan, "毒粉").unwrap().held, held);
        assert_eq!(
            plan.assassin_poison_readiness,
            Some(AssassinPoisonReadiness::NeedsEquipment)
        );
    }
    for invalid_type in [false, true] {
        let mut powder = source_item(710, 20, 2, 9, 1);
        let info = &mut powder.tooltip_source.as_mut().unwrap().info;
        if invalid_type {
            info.item_type = 0;
        } else {
            info.shape = 0;
        }
        let inventory = InventoryModel {
            items: vec![powder],
            ..Default::default()
        };
        let plan = supplies(
            &player("Assassin", 34),
            &inventory,
            &learned(Spell::PoisonSword, "direction"),
        );
        assert_eq!(row(&plan, "毒粉").unwrap().held, 0);
        assert_eq!(
            plan.assassin_poison_readiness,
            Some(AssassinPoisonReadiness::NeedsEquipment)
        );
    }
}

#[test]
fn assassin_unusable_powder_instances_cannot_hide_a_real_stock_shortage() {
    let mut expired = source_item(710, 100, 2, 9, 1);
    expired
        .tooltip_source
        .as_mut()
        .unwrap()
        .user_item
        .as_mut()
        .unwrap()
        .expire_info = Some(CrystalUserItemExpireModel {
        expiry_binary_datetime: NOW,
    });
    let mut sealed = source_item(710, 100, 2, 6, 2);
    sealed
        .tooltip_source
        .as_mut()
        .unwrap()
        .user_item
        .as_mut()
        .unwrap()
        .sealed_info = Some(CrystalUserItemSealedModel {
        expiry_binary_datetime: NOW + 10_000_000,
        ..Default::default()
    });
    let mut shop = source_item(711, 100, 0, 1, 3);
    shop.tooltip_source
        .as_mut()
        .unwrap()
        .user_item
        .as_mut()
        .unwrap()
        .is_shop_item = true;
    let mut broken = source_item(710, 100, 2, 9, 4);
    broken.durability_max = Some(10);
    broken.durability_current = Some(0);
    let mut wrong_class = source_item(710, 100, 2, 9, 5);
    wrong_class
        .tooltip_source
        .as_mut()
        .unwrap()
        .info
        .required_class = 4;
    let mut wrong_gender = source_item(710, 100, 2, 9, 6);
    wrong_gender
        .tooltip_source
        .as_mut()
        .unwrap()
        .info
        .required_gender = 2;
    let inventory = InventoryModel {
        items: vec![
            expired,
            sealed,
            shop,
            broken,
            wrong_class,
            wrong_gender,
            source_item(711, 2, 1, 1, 7),
        ],
        ..Default::default()
    };
    let plan = supplies(
        &player("Assassin", 34),
        &inventory,
        &learned(Spell::PoisonSword, "direction"),
    );
    let poison = row(&plan, "毒粉").unwrap();
    assert_eq!((poison.held, poison.shortage()), (2, 18));
    assert!(poison.is_low());
    assert_eq!(
        plan.assassin_poison_readiness,
        Some(AssassinPoisonReadiness::NeedsEquipment)
    );
}

#[test]
fn fiveclass_supply_item_class_masks_follow_all_five_source_bits() {
    for (class, bit) in [
        ("Warrior", 1),
        ("Wizard", 2),
        ("Taoist", 4),
        ("Assassin", 8),
        ("Archer", 16),
    ] {
        for mask in [1, 2, 4, 8, 16, 31] {
            for (index, kind, container, slot) in [(663, 1, 0, 0), (710, 3, 2, 9)] {
                let mut item = source_item(index, 1, container, slot, 1);
                item.tooltip_source.as_mut().unwrap().info.required_class = mask;
                assert_eq!(
                    usable_supply_kind(&item, &player(class, 34), NOW),
                    (mask & bit != 0).then_some(kind),
                    "{class} mask={mask} index={index}"
                );
            }
        }
    }
}

#[test]
fn classic_three_supply_rows_and_taoist_carried_poisoning_remain_unchanged() {
    let inventory = InventoryModel {
        items: vec![
            source_item(663, 5, 1, 0, 1),
            source_item(710, 2, 1, 1, 2),
            source_item(711, 2, 0, 1, 3),
            source_item(712, 30, 0, 2, 4),
            source_item(712, 20, 2, 9, 5),
        ],
        ..Default::default()
    };
    let taoist = plan_at(
        &player("Taoist", 28),
        &inventory,
        Some(&learned(Spell::Poisoning, "target")),
        Some(2110021),
        NOW,
    );
    assert_eq!(row(&taoist, "毒粉").unwrap().held, 4);
    assert_eq!(row(&taoist, "护身符").unwrap().held, 50);
    assert!(taoist.needs_taoist_material_guidance);
    assert_eq!(taoist.assassin_poison_readiness, None);
    let warrior = supplies(
        &player("Warrior", 28),
        &inventory,
        &learned(Spell::DoubleSlash, "toggle"),
    );
    assert_eq!(
        warrior.rows.iter().map(|row| row.label).collect::<Vec<_>>(),
        ["红药", "回城卷", "随机卷"]
    );
    let wizard = plan_at(&player("Wizard", 28), &inventory, None, Some(2110021), NOW);
    assert_eq!(
        (
            row(&wizard, "蓝药").unwrap().held,
            row(&wizard, "回城卷").unwrap().minimum
        ),
        (5, 2)
    );
    assert!(row(&wizard, "毒粉").is_none());
    assert_eq!(wizard.assassin_poison_readiness, None);
    let early_wizard = supplies(
        &player("Wizard", 6),
        &inventory,
        &learned(Spell::FireBall, "target"),
    );
    assert!(row(&early_wizard, "蓝药").is_some());
}
