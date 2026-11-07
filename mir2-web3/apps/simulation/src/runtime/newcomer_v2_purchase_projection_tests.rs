//! In-memory projection tests: no File source, environment mutation or runner data.
use super::*;
use crate::{SimulationConfig, SimulationSession};
use mir2_protocol::{ClientPacket, UserItem};

const Q6: i32 = 2_110_006;

fn fixture(accepted: bool) -> SimulationSession {
    let mut config = SimulationConfig::default();
    config.account_store_path = None;
    config.account_store_database_url = None;
    let mut session = SimulationSession::new(config);
    session.handle_packet(ClientPacket::Login {
        account_id: "demo".into(),
        password: "demo".into(),
    });
    session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    {
        let mut quests = session.app.world_mut().resource_mut::<QuestResource>();
        quests.quests.clear();
        quests.newcomer_v1_cadence = false;
        quests.newcomer_v2_cadence = true;
    }
    session
        .app
        .world_mut()
        .resource_mut::<MapRuntimeResource>()
        .current_map
        .file_name = "0".into();
    if accepted {
        super::super::begin_quest(session.app.world_mut(), Q6);
    }
    session
}

fn acceptance(world: &World) -> u64 {
    world.resource::<QuestResource>().quests[0]
        .task_progress
        .keys()
        .filter_map(|key| key.strip_prefix(ACCEPTED_AT)?.parse().ok())
        .max()
        .unwrap()
}

fn delivered(name: &str, count: u16) -> ServerPacket {
    let template = crystal_item_by_name(name).unwrap();
    ServerPacket::GainedItem {
        item: UserItem {
            unique_id: 900_001,
            item_index: template.item_index,
            current_dura: template.durability,
            max_dura: template.durability,
            count,
            soul_bound_id: -1,
            identified: false,
            cursed: false,
            slots: vec![None; usize::from(template.slots)],
            gem_count: 0,
            added_stats: vec![],
            awake_type: 0,
            awake_values: vec![],
            refined_value: 0,
            refine_added: 0,
            refine_success_chance: 0,
            wedding_ring: -1,
            expire_info: None,
            rental_information: None,
            is_shop_item: false,
            sealed_info: None,
            gm_made: false,
        },
    }
}

fn payment(name: &str) -> Vec<ServerPacket> {
    vec![ServerPacket::LoseGold { gold: 40 }, delivered(name, 1)]
}

fn encoded(world: &World) -> serde_json::Value {
    serde_json::to_value(&world.resource::<QuestResource>().quests).unwrap()
}

#[test]
fn newcomer_v2_potion_projection_is_owned_and_post_acceptance() {
    let mut session = fixture(true);
    let world = session.app.world();
    let stamp = acceptance(world);
    let before = encoded(world);
    let projection = prepare_committed_potion_purchase_at(world, &payment("(HP)DrugSmall"), stamp)
        .expect("actual accepted basic potion purchase projects progress");
    assert_eq!(
        encoded(world),
        before,
        "preparation cannot change live Quest state"
    );
    assert_eq!(projection.changed_quest_ids, vec![Q6]);
    let quest = &projection.quests.quests[0];
    assert_eq!(quest.stage, QuestStage::ReadyToTurnIn);
    assert_eq!((quest.current, quest.required), (1, 1));
    assert_eq!(quest.task_progress.get("flag:2210061"), Some(&1));
    assert_eq!(
        quest
            .task_progress
            .get("v2:condition:2210061:ordinaryBasicPotionBought"),
        Some(&1)
    );
    *session.app.world_mut().resource_mut::<QuestResource>() = projection.quests;
    assert!(
        prepare_committed_potion_purchase_at(
            session.app.world(),
            &payment("(HP)DrugSmall"),
            stamp + 1
        )
        .is_none(),
        "a ready task does not receive repeated evidence"
    );
}

#[test]
fn newcomer_v2_potion_projection_rejects_past_missing_malformed_or_future_acceptance() {
    let mut session = fixture(true);
    let stamp = acceptance(session.app.world());
    let packets = payment("(HP)DrugSmall");
    assert!(prepare_committed_potion_purchase_at(
        session.app.world(),
        &packets,
        stamp.saturating_sub(1)
    )
    .is_none());
    for key in [
        None,
        Some("v2:accepted_at:not-a-time"),
        Some("v2:accepted_at:18446744073709551615"),
    ] {
        {
            let mut quests = session.app.world_mut().resource_mut::<QuestResource>();
            let progress = &mut quests.quests[0].task_progress;
            progress.retain(|key, _| !key.starts_with(ACCEPTED_AT));
            if let Some(key) = key {
                progress.insert(key.into(), 1);
            }
        }
        let before = encoded(session.app.world());
        assert!(
            prepare_committed_potion_purchase_at(session.app.world(), &packets, stamp).is_none()
        );
        assert_eq!(encoded(session.app.world()), before);
    }
}

#[test]
fn newcomer_v2_potion_projection_requires_loss_packet_and_actual_small_potion_delivery() {
    let session = fixture(true);
    let world = session.app.world();
    let stamp = acceptance(world);
    let before = encoded(world);
    for packets in [
        vec![],
        vec![ServerPacket::LoseGold { gold: 40 }],
        vec![delivered("(HP)DrugSmall", 1)],
        vec![
            ServerPacket::LoseGold { gold: 40 },
            delivered("(HP)DrugSmall", 0),
        ],
        vec![ServerPacket::LoseGold { gold: 40 }, delivered("Candle", 1)],
    ] {
        assert!(prepare_committed_potion_purchase_at(world, &packets, stamp).is_none());
    }
    for name in ["(HP)DrugSmall", "(MP)DrugSmall"] {
        assert!(prepare_committed_potion_purchase_at(world, &payment(name), stamp).is_some());
        assert!(
            prepare_committed_potion_purchase_at(
                world,
                &[ServerPacket::LoseGold { gold: 0 }, delivered(name, 1)],
                stamp
            )
            .is_some(),
            "retain the existing legal zero-price LoseGold packet contract"
        );
    }
    assert_eq!(encoded(world), before);
}

#[test]
fn newcomer_v2_potion_projection_respects_profile_stage_map_and_no_preacceptance_purchase() {
    let mut session = fixture(false);
    let packets = payment("(HP)DrugSmall");
    assert!(prepare_committed_potion_purchase_at(session.app.world(), &packets, 0).is_none());
    super::super::begin_quest(session.app.world_mut(), Q6);
    assert_eq!(
        session.app.world().resource::<QuestResource>().quests[0].current,
        0,
        "accepting later must not synthesize evidence for the earlier purchase"
    );
    let stamp = acceptance(session.app.world());
    session
        .app
        .world_mut()
        .resource_mut::<QuestResource>()
        .newcomer_v2_cadence = false;
    assert!(prepare_committed_potion_purchase_at(session.app.world(), &packets, stamp).is_none());
    session
        .app
        .world_mut()
        .resource_mut::<QuestResource>()
        .newcomer_v2_cadence = true;
    session
        .app
        .world_mut()
        .resource_mut::<MapRuntimeResource>()
        .current_map
        .file_name = "D001".into();
    assert!(prepare_committed_potion_purchase_at(session.app.world(), &packets, stamp).is_none());
    session
        .app
        .world_mut()
        .resource_mut::<MapRuntimeResource>()
        .current_map
        .file_name = "0".into();
    session
        .app
        .world_mut()
        .resource_mut::<QuestResource>()
        .quests[0]
        .stage = QuestStage::Available;
    assert!(prepare_committed_potion_purchase_at(session.app.world(), &packets, stamp).is_none());
}

#[test]
fn newcomer_v2_potion_projection_matches_legacy_buyitem_observer_once() {
    let mut session = fixture(true);
    let packets = payment("(HP)DrugSmall");
    let projection = prepare_committed_potion_purchase(session.app.world(), &packets).unwrap();
    let expected = serde_json::to_value(&projection.quests.quests).unwrap();
    let context = command_context(&ClientPacket::BuyItem {
        item_index: 658,
        count: 1,
        panel_type: 0,
    });
    let updates = observe_committed_command(session.app.world_mut(), context, &packets);
    assert_eq!(encoded(session.app.world()), expected);
    assert_eq!(
        updates
            .iter()
            .filter(|packet| matches!(
                packet,
                ServerPacket::ChangeQuest {
                    quest_id: Q6,
                    completed: true,
                    ..
                }
            ))
            .count(),
        1
    );
    assert!(observe_committed_command(
        session.app.world_mut(),
        CommandContext::PotionPurchase,
        &packets
    )
    .is_empty());

    // Old legacy saves may have an active task without the delayed-event
    // stamp. The synchronous BuyItem path still counts that actual purchase,
    // while the new durable projection does not invent an acceptance record.
    let mut legacy = fixture(true);
    legacy
        .app
        .world_mut()
        .resource_mut::<QuestResource>()
        .quests[0]
        .task_progress
        .retain(|key, _| !key.starts_with(ACCEPTED_AT));
    assert!(prepare_committed_potion_purchase(legacy.app.world(), &packets).is_none());
    let updates = observe_committed_command(
        legacy.app.world_mut(),
        CommandContext::PotionPurchase,
        &packets,
    );
    assert_eq!(
        updates
            .iter()
            .filter(|packet| matches!(
                packet,
                ServerPacket::ChangeQuest {
                    quest_id: Q6,
                    completed: true,
                    ..
                }
            ))
            .count(),
        1
    );
    let quest = &legacy.app.world().resource::<QuestResource>().quests[0];
    assert_eq!(quest.stage, QuestStage::ReadyToTurnIn);
    assert!(!quest
        .task_progress
        .keys()
        .any(|key| key.starts_with(ACCEPTED_AT)));
}
