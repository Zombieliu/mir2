use super::*;

#[test]
fn original_baby_pig_zero_is_summoned_and_none_ninety_nine_is_not() {
    let mut ui = CreatureUi::default();
    ui.observe(&ServerPacket::UpdateIntelligentCreatureList {
        creature_list: vec![pet(0, 0)],
        creature_summoned: true,
        summoned_creature_type: 0,
        pearl_count: 0,
    });
    assert_eq!(ui.summoned, Some(0));
    assert_eq!(ui.selected().unwrap().pet_type, 0);
    assert!(animation(0).is_some());
    assert!(animation(14).is_some());
    assert!(animation(99).is_none());
    ui.observe(&ServerPacket::UpdateIntelligentCreatureList {
        creature_list: vec![pet(0, 0)],
        creature_summoned: true,
        summoned_creature_type: 99,
        pearl_count: 0,
    });
    assert_eq!(ui.summoned, None);
}
fn pet(ty: u8, slot: i32) -> ClientIntelligentCreature {
    ClientIntelligentCreature {
        pet_type: ty,
        slot_index: slot,
        icon: 540,
        custom_name: format!("Pet{ty}"),
        fullness: 10000,
        expire_binary_datetime: 0,
        blackstone_time: 0,
        pet_mode: 0,
        creature_rules: mir2_protocol::IntelligentCreatureRules {
            minimal_fullness: 1000,
            mouse_pickup_enabled: true,
            mouse_pickup_range: 3,
            auto_pickup_enabled: true,
            auto_pickup_range: 3,
            semi_auto_pickup_enabled: true,
            semi_auto_pickup_range: 3,
            can_produce_blackstone: true,
        },
        filter: mir2_protocol::IntelligentCreatureItemFilter {
            pet_pickup_all: true,
            pet_pickup_gold: false,
            pet_pickup_weapons: false,
            pet_pickup_armours: false,
            pet_pickup_helmets: false,
            pet_pickup_boots: false,
            pet_pickup_belts: false,
            pet_pickup_accessories: false,
            pet_pickup_others: false,
        },
        pickup_grade: 0,
        maintain_food_time: 0,
    }
}
fn list(summoned: bool) -> ServerPacket {
    ServerPacket::UpdateIntelligentCreatureList {
        creature_list: vec![pet(0, 3), pet(1, 8)],
        creature_summoned: summoned,
        summoned_creature_type: 1,
        pearl_count: 25,
    }
}
fn ui() -> CreatureUi {
    let mut ui = CreatureUi::default();
    ui.observe(&list(false));
    ui.show(0);
    ui
}
#[test]
fn open_subscribes_and_selects_authoritative_summoned_slot() {
    let mut ui = CreatureUi::default();
    ui.data_received = true;
    assert!(ui.show(0).is_none());
    assert_eq!(ui.notice.as_deref(), Some("You do not own any creatures."));
    ui.observe(&list(true));
    assert_eq!(
        ui.show(1),
        Some(ClientPacket::RequestIntelligentCreatureUpdates { update: true })
    );
    assert_eq!(ui.selected_slot, Some(8));
    assert!(ui.show(2).is_none());
    assert_eq!(
        ui.action(CreatureAction::Close, 3),
        Some(ClientPacket::RequestIntelligentCreatureUpdates { update: false })
    );
}
#[test]
fn summon_dismiss_release_are_gated_by_authoritative_state() {
    let mut ui = ui();
    let request = ui.action(CreatureAction::Summon, 0).unwrap();
    assert!(matches!(
        request,
        ClientPacket::UpdateIntelligentCreature {
            summon_me: true,
            ..
        }
    ));
    assert!(ui.action(CreatureAction::Summon, 0).is_none());
    assert!(ui.summoned.is_none());
    ui.release_unsent(&request);
    ui.observe(&list(true));
    assert!(ui.action(CreatureAction::Summon, 0).is_none());
    ui.action(CreatureAction::Select(8), 0);
    assert!(ui.action(CreatureAction::Release, 0).is_none());
    assert!(ui.input.is_none());
    assert!(matches!(
        ui.action(CreatureAction::Dismiss, 0),
        Some(ClientPacket::UpdateIntelligentCreature {
            unsummon_me: true,
            ..
        })
    ));
}
#[test]
fn rename_requires_server_token_and_original_ascii_name_rules() {
    let mut ui = ui();
    ui.action(CreatureAction::Rename, 0);
    assert!(ui.input.is_none());
    ui.observe(&ServerPacket::IntelligentCreatureEnableRename);
    ui.action(CreatureAction::Rename, 0);
    ui.input.as_mut().unwrap().text = "中文名".into();
    assert!(ui.action(CreatureAction::SubmitInput, 0).is_none());
    assert!(ui.notice.is_some());
    ui.input.as_mut().unwrap().text = "Valid123".into();
    let request = ui.action(CreatureAction::SubmitInput, 0).unwrap();
    assert!(!ui.rename_enabled);
    assert_eq!(ui.selected().unwrap().custom_name, "Pet0");
    ui.release_unsent(&request);
    assert!(ui.rename_enabled);
}
#[test]
fn release_verification_is_case_insensitive_and_bound_to_opened_pet() {
    let mut ui = ui();
    ui.action(CreatureAction::Release, 0);
    ui.action(CreatureAction::Select(8), 0);
    ui.input.as_mut().unwrap().text = "wrong".into();
    assert!(ui.action(CreatureAction::SubmitInput, 0).is_none());
    assert!(ui.system_message.is_some());
    ui.action(CreatureAction::Select(3), 0);
    ui.action(CreatureAction::Release, 0);
    ui.action(CreatureAction::Select(8), 0);
    ui.input.as_mut().unwrap().text = "pET0".into();
    assert!(
        matches!(ui.action(CreatureAction::SubmitInput,0),Some(ClientPacket::UpdateIntelligentCreature{creature,release_me:true,..}) if creature.pet_type==0)
    );
}
#[test]
fn filters_are_draft_until_save_and_mode_respects_rules() {
    let mut ui = ui();
    ui.action(CreatureAction::Options, 0);
    ui.action(CreatureAction::Filter(1), 0);
    assert!(!ui.options.as_ref().unwrap().filter.pet_pickup_all);
    assert!(ui.options.as_ref().unwrap().filter.pet_pickup_gold);
    assert!(ui.selected().unwrap().filter.pet_pickup_all);
    ui.action(CreatureAction::Filter(1), 0);
    assert!(ui.options.as_ref().unwrap().filter.pet_pickup_all);
    ui.action(CreatureAction::Filter(1), 0);
    ui.action(CreatureAction::Grade(100), 0);
    assert_eq!(ui.options.as_ref().unwrap().pickup_grade, 5);
    let packet = ui.action(CreatureAction::SaveOptions, 0).unwrap();
    assert!(
        matches!(&packet,ClientPacket::UpdateIntelligentCreature{creature,..} if creature.pickup_grade==5&&creature.filter.pet_pickup_gold)
    );
    ui.release_unsent(&packet);
    ui.creatures[0].creature_rules.semi_auto_pickup_enabled = false;
    assert!(ui.action(CreatureAction::ToggleMode, 0).is_none());
}
#[test]
fn animation_uses_source_strips_and_switches_after_completed_idle_loop() {
    let ui = ui();
    assert_eq!(ui.animation_frame(0), Some(540));
    assert_eq!(ui.animation_frame(1000), Some(545));
    assert_eq!(ui.animation_frame(8400), Some(550));
    assert_eq!(ui.animation_frame(9900), Some(540));
    for kind in 0..15 {
        let (a, n, _, b, m, _) = animation(kind).unwrap();
        assert!(n > 0 && m > 0 && a != b);
    }
    assert!(animation(99).is_none());
}
