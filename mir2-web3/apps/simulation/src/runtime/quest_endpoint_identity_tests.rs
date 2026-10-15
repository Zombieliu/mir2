use super::*;
use crate::runtime::npc::crystal_quest_ids_by_npc;

#[test]
fn canni_tea_endpoints_prefer_loaded_object_identity_over_database_index() {
    assert_eq!(canonical_crystal_quest_npc_object_id(55), 33);

    let q51 = crystal_quest_info_by_id(51).expect("q51 should exist");
    assert_eq!(q51.finish_npc_index, 33);
    assert_eq!(crystal_quest_finish_npc_label(&q51), "CraftsLady_Alice");
    assert!(crystal_quest_finish_npc_matches(&q51, 33));
    assert!(!crystal_quest_finish_npc_matches(&q51, 22));

    let q52 = crystal_quest_info_by_id(52).expect("q52 should exist");
    assert_eq!(q52.npc_index, 33);
    assert_eq!(q52.finish_npc_index, 33);
    assert!(crystal_quest_start_npc_matches(&q52, 33));
    assert!(crystal_quest_finish_npc_matches(&q52, 33));
    assert!(!crystal_quest_start_npc_matches(&q52, 22));
    assert!(!crystal_quest_finish_npc_matches(&q52, 22));
}

#[test]
fn canni_tea_quest_icons_only_attach_to_the_canonical_npc() {
    let quest_ids_by_npc = crystal_quest_ids_by_npc();
    let crafts_lady_quests = quest_ids_by_npc
        .get(&33)
        .expect("CraftsLady_Alice should have quest bindings");
    assert!(crafts_lady_quests.contains(&51));
    assert!(crafts_lady_quests.contains(&52));

    assert!(!quest_ids_by_npc
        .get(&22)
        .is_some_and(|quests| quests.contains(&51)));
    assert!(!quest_ids_by_npc
        .get(&22)
        .is_some_and(|quests| quests.contains(&52)));
}
