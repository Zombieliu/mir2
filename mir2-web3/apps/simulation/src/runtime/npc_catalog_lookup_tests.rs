//! Actual imported and periodic NPC identity equivalence, plus a bounded
//! diagnostic of the original full-catalog lookup cost. Timing is evidence,
//! not a performance threshold or proof of an entire native map transition.
use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;

use mir2_game_data::CrystalNpcInfoTemplate;

use super::npc::{canonical_crystal_quest_npc_info, crystal_quest_ids_by_npc};

fn original_owned_lookup(index: u32) -> Option<CrystalNpcInfoTemplate> {
    let npcs = mir2_game_data::crystal_npc_info_manifest().npcs.into_iter()
        .chain(mir2_game_data::periodic_quests::npc_templates()).collect::<Vec<_>>();
    npcs.iter().find(|npc| npc.loaded_object_id == Some(index)).cloned().or_else(|| {
        let database_index = i32::try_from(index).ok()?;
        npcs.into_iter().find(|npc| npc.npc_index == database_index)
    })
}

fn actual_quest_references() -> Vec<u32> {
    mir2_game_data::crystal_quest_packet_manifest().quests.iter()
        .flat_map(|quest| [quest.npc_index, quest.finish_npc_index])
        .filter(|index| *index != 0).collect()
}

#[test]
fn all_imported_and_periodic_loaded_ids_database_ids_and_unknowns_keep_original_identity() {
    let mut indices = BTreeSet::from([0, u32::MAX, i32::MAX as u32]);
    let periodic = mir2_game_data::periodic_quests::npc_templates();
    for npc in mir2_game_data::crystal_npc_info_manifest_ref().npcs.iter().chain(periodic.iter()) {
        if let Some(index) = npc.loaded_object_id { indices.insert(index); }
        if let Ok(index) = u32::try_from(npc.npc_index) { indices.insert(index); }
    }
    indices.extend(actual_quest_references());
    for index in indices {
        assert_eq!(canonical_crystal_quest_npc_info(index), original_owned_lookup(index),
            "loaded-object priority and imported-before-periodic database fallback: {index}");
    }
}

#[test]
fn actual_quest_endpoint_index_keeps_every_original_reference_and_fallback() {
    let mut expected = BTreeMap::<u32, BTreeSet<i32>>::new();
    for quest in &mir2_game_data::crystal_quest_packet_manifest().quests {
        for index in [quest.npc_index, quest.finish_npc_index] {
            if index == 0 { continue; }
            let object = original_owned_lookup(index).and_then(|npc| npc.loaded_object_id).unwrap_or(index);
            expected.entry(object).or_default().insert(quest.index);
        }
    }
    assert_eq!(crystal_quest_ids_by_npc(), expected);
}

#[test]
fn actual_quest_lookup_reports_original_and_candidate_cost_with_identical_results() {
    let indices = actual_quest_references();
    assert!(!indices.is_empty());
    // Warm the actual immutable sources, then alternate the order to avoid
    // attributing first-load or one scheduling sample to the implementation.
    let expected: Vec<_> = indices.iter().map(|index| original_owned_lookup(*index)).collect();
    let mut original_us = Vec::new();
    let mut candidate_us = Vec::new();
    for reverse in [false, true, false, true] {
        for candidate in if reverse { [true, false] } else { [false, true] } {
            let started = Instant::now();
            let result: Vec<_> = indices.iter().map(|index| {
                if candidate { canonical_crystal_quest_npc_info(*index) }
                else { original_owned_lookup(*index) }
            }).collect();
            let elapsed = started.elapsed().as_micros();
            assert_eq!(result, expected);
            if candidate { candidate_us.push(elapsed); } else { original_us.push(elapsed); }
        }
    }
    eprintln!("actual canonical NPC catalog diagnostic: imported={}, references={}, original_us={original_us:?}, candidate_us={candidate_us:?}",
        mir2_game_data::crystal_npc_info_manifest_ref().npcs.len(), indices.len());
}
