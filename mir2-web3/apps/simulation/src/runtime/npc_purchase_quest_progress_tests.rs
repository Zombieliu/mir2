//! Actual File-source regressions. Root remains the sole runner; reuse only
//! the parent's fresh guarded fixture directories, retaining all evidence.
use super::*;
use crate::config::QuestStage;
use crate::runtime::{
    items::crystal_belt_slot_range_for_item_key, quests, resources::QuestResource,
};

const Q6: i32 = 2_110_006;
const ACCEPTED_AT: &str = "v2:accepted_at:";

fn potion_fixture(label: &str, service: &str, accepted: bool) -> Fixture {
    let mut fixture = Fixture::new(label, service, NpcPurchaseSource::Trade);
    {
        let mut resource = fixture
            .session
            .app
            .world_mut()
            .resource_mut::<QuestResource>();
        resource.quests.clear();
        resource.newcomer_v1_cadence = false;
        resource.newcomer_v2_cadence = true;
    }
    if accepted {
        accept_q6(&mut fixture);
    } else {
        fixture.session.save_active_character().unwrap();
    }
    fixture
}

fn accept_q6(fixture: &mut Fixture) {
    // Establish the real server acceptance stamp in the baseline checkpoint;
    // the purchase itself must not create or migrate an acceptance record.
    quests::begin_quest(fixture.session.app.world_mut(), Q6);
    assert_eq!(
        live_q6(&fixture.session).unwrap().stage,
        QuestStage::InProgress
    );
    assert!(live_q6(&fixture.session)
        .unwrap()
        .task_progress
        .keys()
        .any(|key| key.starts_with(ACCEPTED_AT)));
    fixture.session.save_active_character().unwrap();
    assert_quest_checkpoint(fixture);
}

fn saved_q6(save: &CharacterSaveRecord) -> Option<quests::QuestState> {
    save.quest_states_json
        .iter()
        .map(|encoded| serde_json::from_str::<quests::QuestState>(encoded).unwrap())
        .find(|quest| quest.quest_id == Q6)
}

fn live_q6(session: &SimulationSession) -> Option<quests::QuestState> {
    session
        .app
        .world()
        .resource::<QuestResource>()
        .quests
        .iter()
        .find(|quest| quest.quest_id == Q6)
        .cloned()
}

fn stored_small_hp(save: &CharacterSaveRecord) -> Vec<(u64, ItemContainer, u8, u32)> {
    let mut items = Vec::new();
    for (rows, belt) in [
        (&save.inventory_items_json, false),
        (&save.belt_items_json, true),
    ] {
        for encoded in rows {
            let item: ItemState = serde_json::from_str(encoded).unwrap();
            if item.name != "(HP)DrugSmall" {
                continue;
            }
            if belt {
                assert_eq!(item.container, ItemContainer::Belt);
                let (start, end) = crystal_belt_slot_range_for_item_key(&item.key)
                    .expect("an ordinary small HP potion is belt-eligible");
                assert!(
                    (start..end).contains(&item.slot),
                    "potion was stored in a legal belt slot"
                );
            } else {
                assert!(matches!(
                    item.container,
                    ItemContainer::Bag1 | ItemContainer::Bag2
                ));
            }
            items.push((item.unique_id, item.container, item.slot, item.quantity));
        }
    }
    items.sort_unstable_by_key(|item| item.0);
    items
}

fn assert_small_hp_delivery(
    before: &CharacterSaveRecord,
    after: &CharacterSaveRecord,
    entry: &NpcPurchaseJournalEntry,
) {
    let NpcPurchaseProcessingOutcome::Committed {
        admitted_count,
        incoming_unique_id,
        ..
    } = &entry.outcome
    else {
        panic!("expected a committed small HP purchase");
    };
    assert_eq!(*admitted_count, 2);
    let owned_before = stored_small_hp(before);
    let owned_after = stored_small_hp(after);
    assert!(
        owned_before.is_empty(),
        "this fixture starts without small HP potions"
    );
    assert_eq!(
        owned_after.iter().map(|item| item.3).sum::<u32>(),
        u32::from(*admitted_count)
    );
    assert!(
        owned_after.iter().any(|item| item.0 == *incoming_unique_id),
        "the committed incoming UID must be owned in the saved bag or belt"
    );
}

fn assert_ready(quest: &quests::QuestState) {
    assert_eq!(quest.stage, QuestStage::ReadyToTurnIn);
    assert_eq!((quest.current, quest.required), (1, 1));
    assert_eq!(quest.task_progress.get("flag:2210061"), Some(&1));
    assert_eq!(
        quest
            .task_progress
            .get("v2:condition:2210061:ordinaryBasicPotionBought"),
        Some(&1)
    );
}

fn assert_unready(quest: &quests::QuestState) {
    assert_eq!(quest.stage, QuestStage::InProgress);
    assert_eq!(quest.current, 0);
    assert!(!quest.task_progress.contains_key("flag:2210061"));
    assert!(!quest
        .task_progress
        .contains_key("v2:condition:2210061:ordinaryBasicPotionBought"));
}

fn assert_quest_checkpoint(fixture: &Fixture) {
    fixture.assert_checkpoint();
    let active = snapshot_active_character_save(fixture.session.app.world()).unwrap();
    assert_eq!(
        active.quest_states_json,
        fixture.save_from_file().quest_states_json
    );
}

fn reopen_v2(config: SimulationConfig) -> SimulationSession {
    assert!(config.account_store_path.is_some());
    assert!(config.account_store_database_url.is_none());
    let mut session = SimulationSession::new(config);
    assert!(session
        .handle_packet(ClientPacket::Login {
            account_id: "demo".into(),
            password: "demo".into()
        })
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
    // Match the configured server profile without changing process environment.
    {
        let mut resource = session.app.world_mut().resource_mut::<QuestResource>();
        resource.newcomer_v1_cadence = false;
        resource.newcomer_v2_cadence = true;
    }
    session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    assert_eq!(session.active_identity().unwrap().character_index, 0);
    session
}

fn has_q6_update(packets: &[ServerPacket]) -> bool {
    packets.iter().any(|packet| {
        matches!(
            packet,
            ServerPacket::ChangeQuest {
                quest_id: Q6,
                completed: true,
                ..
            }
        )
    })
}

#[test]
fn durable_potion_quest_progress_commits_with_wallet_inventory_and_reopens() {
    let mut fixture = potion_fixture("q6-atomic", "BUYSELL", true);
    let operation = fixture.operation(1);
    let before = fixture.save_from_file();
    let execution = fixture.session.try_durable_npc_purchase(operation).unwrap();
    assert_commit(
        &execution,
        operation,
        NpcPurchaseCurrency::Gold,
        NpcPurchaseSource::Trade,
    );
    let after = fixture.save_from_file();
    assert_eq!(
        after.revision,
        before.revision + 1,
        "Quest evidence is part of the purchase CAS, not a second save"
    );
    assert_eq!(after.gold, before.gold - 160);
    assert_small_hp_delivery(&before, &after, &execution.receipt.entry);
    assert_ready(&saved_q6(&after).unwrap());
    assert_ready(&live_q6(&fixture.session).unwrap());
    assert_quest_checkpoint(&fixture);
    assert_eq!(
        execution
            .packets
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
    assert!(
        !execution
            .packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::GainedGold { .. })),
        "buying medicine does not claim the task reward"
    );
    let file = fs::read(&fixture.path).unwrap();
    let live = fixture.live();
    assert_eq!(
        fixture.session.query_npc_purchase(operation).unwrap(),
        Some(execution.receipt.clone())
    );
    let replay = fixture.session.try_durable_npc_purchase(operation).unwrap();
    assert!(replay.replayed);
    assert!(replay.packets.is_empty());
    assert_eq!(replay.receipt, execution.receipt);
    assert_eq!(fixture.live(), live);
    assert_eq!(fs::read(&fixture.path).unwrap(), file);
    assert_quest_checkpoint(&fixture);
    let path = fixture.path.clone();
    let visible_npcs = fixture.config.visible_npcs.clone();
    drop(fixture);
    let mut independent = SimulationConfig::default().with_account_store_path(&path);
    independent.visible_npcs = visible_npcs;
    independent.ensure_account_store_writable().unwrap();
    let reopened = reopen_v2(independent);
    assert_ready(&live_q6(&reopened).unwrap());
    let reopened_save = snapshot_active_character_save(reopened.app.world()).unwrap();
    assert_eq!(reopened_save.gold, after.gold);
    assert_eq!(stored_small_hp(&reopened_save), stored_small_hp(&after));
}

#[test]
fn durable_potion_quest_progress_does_not_retroactively_count_a_preacceptance_order() {
    let mut fixture = potion_fixture("q6-preacceptance", "BUYSELL", false);
    let operation = fixture.operation(1);
    let execution = fixture.session.try_durable_npc_purchase(operation).unwrap();
    assert!(!has_q6_update(&execution.packets));
    assert!(saved_q6(&fixture.save_from_file()).is_none());
    accept_q6(&mut fixture);
    let before = fixture.save_from_file();
    let bytes = fs::read(&fixture.path).unwrap();
    assert_eq!(
        fixture.session.query_npc_purchase(operation).unwrap(),
        Some(execution.receipt)
    );
    let replay = fixture.session.try_durable_npc_purchase(operation).unwrap();
    assert!(replay.replayed && replay.packets.is_empty());
    assert_unready(&saved_q6(&fixture.save_from_file()).unwrap());
    assert_unready(&live_q6(&fixture.session).unwrap());
    assert_eq!(
        fixture.save_from_file().quest_states_json,
        before.quest_states_json
    );
    assert_eq!(fixture.save_from_file().gold, before.gold);
    assert_eq!(fs::read(&fixture.path).unwrap(), bytes);
    assert_quest_checkpoint(&fixture);
}

#[test]
fn durable_potion_quest_progress_rejects_failed_nonbasic_pearl_and_missing_acceptance_evidence() {
    for kind in ["rejected", "nonbasic", "pearls", "missing-acceptance"] {
        let service = if kind == "pearls" {
            "PEARLBUY"
        } else {
            "BUYSELL"
        };
        let mut fixture = potion_fixture("q6-negative", service, true);
        if kind == "nonbasic" {
            fixture.session.interact(4990);
            let packets = fixture.session.select_npc_dialog_target("@BuySell");
            let item = packets
                .iter()
                .find_map(|packet| match packet {
                    ServerPacket::NPCGoods { list, .. } => list.iter().find(|item| {
                        item.count == 1
                            && mir2_game_data::crystal_item_by_index(item.item_index).is_some_and(
                                |info| {
                                    !matches!(info.name.as_str(), "(HP)DrugSmall" | "(MP)DrugSmall")
                                },
                            )
                    }),
                    _ => None,
                })
                .expect("actual ordinary merchant has a nonbasic stock item");
            fixture.request.item_index = item.unique_id;
            fixture.request.count = 1;
        }
        if kind == "missing-acceptance" {
            fixture
                .session
                .app
                .world_mut()
                .resource_mut::<QuestResource>()
                .quests[0]
                .task_progress
                .retain(|key, _| !key.starts_with(ACCEPTED_AT));
            fixture.session.save_active_character().unwrap();
        }
        let mut operation = fixture.operation(1);
        if kind == "rejected" {
            operation.intent.service_catalog_proof = [8; 32];
        }
        let execution = fixture.session.try_durable_npc_purchase(operation).unwrap();
        assert_eq!(
            matches!(
                execution.receipt.entry.outcome,
                NpcPurchaseProcessingOutcome::Rejected { .. }
            ),
            kind == "rejected"
        );
        assert!(!has_q6_update(&execution.packets));
        assert_unready(&live_q6(&fixture.session).unwrap());
        assert_unready(&saved_q6(&fixture.save_from_file()).unwrap());
        if kind == "missing-acceptance" {
            assert!(!saved_q6(&fixture.save_from_file())
                .unwrap()
                .task_progress
                .keys()
                .any(|key| key.starts_with(ACCEPTED_AT)));
        }
        assert_quest_checkpoint(&fixture);
    }
}

#[test]
fn durable_potion_quest_progress_prepersist_failures_publish_neither_quest_nor_purchase() {
    for fault in [
        AccountStoreTransactionFault::BeforePersist,
        AccountStoreTransactionFault::BeforeFileRename,
    ] {
        let mut fixture = potion_fixture("q6-prepersist", "BUYSELL", true);
        let operation = fixture.operation(1);
        let before = fixture.live();
        let bytes = fs::read(&fixture.path).unwrap();
        let quests = snapshot_active_character_save(fixture.session.app.world())
            .unwrap()
            .quest_states_json;
        fixture.config.inject_account_store_transaction_fault(fault);
        assert!(matches!(
            fixture.session.try_durable_npc_purchase(operation),
            Err(NpcPurchaseDurableError::Unknown { .. })
        ));
        assert_eq!(fixture.live(), before);
        assert_eq!(fs::read(&fixture.path).unwrap(), bytes);
        assert_eq!(
            snapshot_active_character_save(fixture.session.app.world())
                .unwrap()
                .quest_states_json,
            quests
        );
        assert_eq!(fixture.session.query_npc_purchase(operation).unwrap(), None);
        assert_unready(&live_q6(&fixture.session).unwrap());
        assert_quest_checkpoint(&fixture);
    }
}

#[test]
fn durable_potion_quest_progress_ambiguous_rename_retains_one_complete_file_for_recovery() {
    let mut fixture = potion_fixture("q6-ambiguous", "BUYSELL", true);
    let operation = fixture.operation(1);
    let before = fixture.save_from_file();
    fixture.config.inject_account_store_transaction_fault(
        AccountStoreTransactionFault::AfterFileRenameBeforeDirectorySync,
    );
    assert!(matches!(
        fixture.session.try_durable_npc_purchase(operation),
        Err(NpcPurchaseDurableError::Unknown { .. })
    ));
    assert_unready(&live_q6(&fixture.session).unwrap());
    let saved = fixture.save_from_file();
    assert_ready(&saved_q6(&saved).unwrap());
    assert_eq!(saved.gold, before.gold - 160);
    assert_eq!(saved.revision, before.revision + 1);
    let entry = saved
        .npc_purchase_journal
        .as_ref()
        .unwrap()
        .lookup(&operation)
        .unwrap()
        .unwrap()
        .clone();
    assert_small_hp_delivery(&before, &saved, &entry);
    assert!(
        fixture.session.query_npc_purchase(operation).is_err(),
        "the old cache remains frozen"
    );
    // A second config while the fixture owns this canonical path only shares
    // the frozen cache. Release that test authority, then inspect the freshly
    // loaded File image. The durable publication marker still forbids normal
    // StartGame/producer writes until explicit operator reconciliation.
    let path = fixture.path.clone();
    let bytes = fs::read(&path).unwrap();
    let mut marker_path = path.as_os_str().to_os_string();
    marker_path.push(".writer.lock");
    let marker_path = PathBuf::from(marker_path);
    drop(fixture);
    let marker = fs::read(&marker_path).unwrap();
    assert!(
        !marker.is_empty(),
        "ambiguous File publication leaves a durable fence"
    );
    let reopened = SimulationConfig::default().with_account_store_path(&path);
    assert!(reopened
        .ensure_account_store_writable()
        .unwrap_err()
        .contains("durable publication marker requires explicit reconciliation"));
    let recovered = reopened.account_store.lock().unwrap().accounts["demo"].saves
        [&saved.character.index]
        .clone();
    assert_ready(&saved_q6(&recovered).unwrap());
    assert_eq!(stored_small_hp(&recovered), stored_small_hp(&saved));
    assert_eq!(
        recovered
            .npc_purchase_journal
            .as_ref()
            .unwrap()
            .lookup(&operation)
            .unwrap(),
        Some(&entry)
    );
    assert_eq!(
        serde_json::to_value(&recovered).unwrap(),
        serde_json::to_value(&saved).unwrap(),
        "read-only recovery loads the entire committed checkpoint, not the stale cache"
    );
    drop(reopened);
    assert_eq!(fs::read(&path).unwrap(), bytes);
    assert_eq!(fs::read(&marker_path).unwrap(), marker);
}

#[test]
fn durable_potion_quest_progress_postcommit_error_or_panic_preserves_saved_progress_once() {
    for panic in [false, true] {
        let mut fixture = potion_fixture("q6-postcommit", "BUYSELL", true);
        let operation = fixture.operation(1);
        let before = fixture.save_from_file();
        let error = if panic {
            fixture.session.durable_npc_purchase_post_panic(operation)
        } else {
            fixture.session.durable_npc_purchase_post_failure(operation)
        }
        .unwrap_err();
        let NpcPurchaseDurableError::PostCommit { receipt, .. } = error else {
            panic!("one committed receipt must survive postprocessing");
        };
        let saved = fixture.save_from_file();
        assert_ready(&saved_q6(&saved).unwrap());
        assert_ready(&live_q6(&fixture.session).unwrap());
        assert_eq!(saved.revision, before.revision + 1);
        assert_eq!(saved.gold, before.gold - 160);
        assert_quest_checkpoint(&fixture);
        let bytes = fs::read(&fixture.path).unwrap();
        assert_eq!(
            fixture.session.query_npc_purchase(operation).unwrap(),
            Some(receipt.clone())
        );
        let replay = fixture.session.try_durable_npc_purchase(operation).unwrap();
        assert!(replay.replayed && replay.packets.is_empty());
        assert_eq!(replay.receipt, receipt);
        assert_eq!(fs::read(&fixture.path).unwrap(), bytes);
        let reopened = reopen_v2(fixture.config.clone());
        assert_ready(&live_q6(&reopened).unwrap());
        assert_eq!(
            snapshot_active_character_save(reopened.app.world())
                .unwrap()
                .gold,
            saved.gold
        );
    }
}

#[test]
fn durable_potion_quest_progress_default_checkpoint_merge_neither_decodes_nor_overwrites_quests() {
    let mut fixture = potion_fixture("q6-default-merge", "BUYSELL", true);
    let before = snapshot_active_character_save(fixture.session.app.world())
        .unwrap()
        .quest_states_json;
    let mut unrelated = fixture.save_from_file();
    unrelated.quest_states_json =
        vec!["not JSON: unrelated default merge must not read this field".into()];
    let publication =
        PreparedCheckpointMerge::new(fixture.session.app.world(), &unrelated).unwrap();
    publication.apply(fixture.session.app.world_mut());
    assert_eq!(
        snapshot_active_character_save(fixture.session.app.world())
            .unwrap()
            .quest_states_json,
        before
    );
}
