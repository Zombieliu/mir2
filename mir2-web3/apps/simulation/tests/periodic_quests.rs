//! Packet/persistence regressions for the additive daily/weekly catalog.
//!
//! Every account and File store is created by this test. Ready fixtures contain
//! real catalog task keys/counts but are NOT natural-kill or play-time evidence.
use std::{collections::BTreeSet, sync::Mutex};

use mir2_game_data::periodic_quests::{self as content, PeriodicCadence};
use mir2_protocol::{ClientPacket, MirClass, MirDirection, MirGender, Point, ServerPacket};
use mir2_simulation::{
    AccountRecord, CharacterRecord, CharacterSaveRecord, InProcessWorldRuntime, QuestStage,
    SimulationConfig, WorldCommand, WorldRuntime,
};
use serde_json::{json, Value};

static PROFILE_LOCK: Mutex<()> = Mutex::new(());
const BICHON: u32 = 2180;
const MONGCHON: u32 = 2181;
const DAILY: i32 = 92006;
const WEEKLY: i32 = 92009;

struct ProfileGuard(Option<std::ffi::OsString>);
impl ProfileGuard {
    fn newcomer_v2() -> Self {
        let previous = std::env::var_os("MIR2_QUEST_CADENCE");
        std::env::set_var("MIR2_QUEST_CADENCE", "newcomer-v2");
        Self(previous)
    }
}
impl Drop for ProfileGuard {
    fn drop(&mut self) {
        if let Some(previous) = self.0.take() {
            std::env::set_var("MIR2_QUEST_CADENCE", previous);
        } else {
            std::env::remove_var("MIR2_QUEST_CADENCE");
        }
    }
}

struct Fixture {
    config: SimulationConfig,
    account_id: String,
}

fn fixture(label: &str, level: u16, class: MirClass, npc_id: u32, rows: Vec<Value>) -> Fixture {
    let mut config = SimulationConfig::default()
        .with_crystal_map_runtime()
        .with_platinum_176_profile();
    let character = CharacterRecord {
        index: 0,
        name: "PeriodicProbe".into(),
        level,
        class,
        gender: MirGender::Male,
    };
    let mut save = CharacterSaveRecord::new(character.clone());
    put_near_npc(&mut save, npc_id);
    save.max_experience = config.experience_required_for_level(level);
    save.quest_states_json = rows.into_iter().map(|row| row.to_string()).collect();
    let mut account = AccountRecord::empty();
    account.characters.push(character.clone());
    account.saves.insert(0, save);
    config.default_character = character;
    let account_id = format!("periodic-packet-{label}");
    let mut store = config.account_store.lock().unwrap();
    store.accounts.clear();
    store.accounts.insert(account_id.clone(), account);
    drop(store);
    Fixture { config, account_id }
}

fn put_near_npc(save: &mut CharacterSaveRecord, npc_id: u32) {
    let npc = content::npc(npc_id).unwrap();
    save.map_file_name = npc.map.clone();
    save.map_title = if npc_id == BICHON {
        "BichonProvince"
    } else {
        "MongchonProvince"
    }
    .into();
    save.position = Point {
        x: npc.x - 1,
        y: npc.y,
    };
    save.direction = MirDirection::Right;
}

fn ready_row(id: i32, accepted_level: u16, locked_exp: u32) -> Value {
    let quest = content::quest(id).unwrap();
    let mut progress = serde_json::Map::new();
    let required: u32 = quest.kills.iter().map(|kill| kill.count).sum();
    for kill in &quest.kills {
        progress.insert(format!("kill:{}", kill.monster_index), json!(kill.count));
    }
    json!({
        "quest_id": id, "title": quest.title, "summary": quest.summary,
        "reward_preview": "Locked experience and gold", "required": required,
        "current": required, "stage": "readyToTurnIn", "task_progress": progress,
        "accepted_periodic_reward": {"level": accepted_level, "experience": locked_exp, "gold": quest.gold}
    })
}

fn login(fixture: &Fixture) -> (InProcessWorldRuntime, Vec<ServerPacket>) {
    let mut runtime = InProcessWorldRuntime::new(fixture.config.clone());
    let packets = packet(
        &mut runtime,
        ClientPacket::Login {
            account_id: fixture.account_id.clone(),
            password: "demo".into(),
        },
    );
    assert!(
        packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })),
        "{packets:?}"
    );
    let packets = packet(&mut runtime, ClientPacket::StartGame { character_index: 0 });
    assert!(
        packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::StartGame { result: 4, .. })),
        "{packets:?}"
    );
    (runtime, packets)
}

fn packet(runtime: &mut InProcessWorldRuntime, packet: ClientPacket) -> Vec<ServerPacket> {
    runtime
        .execute(WorldCommand::ClientPacket(packet))
        .expect("ordinary packet should execute")
}

fn open_offer(runtime: &mut InProcessWorldRuntime, npc_id: u32, quest_id: i32, finish: bool) {
    packet(
        runtime,
        ClientPacket::CallNpc {
            object_id: npc_id,
            key: "@Main".into(),
        },
    );
    let dialog = runtime
        .world_snapshot()
        .active_npc_dialog
        .expect("nearby Task Steward should display a page");
    assert_eq!(dialog.npc_object_id, npc_id);
    let target = format!(
        "@quest:{}:{quest_id}",
        if finish { "finish" } else { "accept" }
    );
    assert!(
        dialog.links.iter().any(|link| link.target == target),
        "expected {target}; {dialog:?}"
    );
}

fn accept(runtime: &mut InProcessWorldRuntime, npc_id: u32, quest_id: i32) -> Vec<ServerPacket> {
    packet(
        runtime,
        ClientPacket::AcceptQuest {
            npc_index: npc_id,
            quest_index: quest_id,
        },
    )
}

fn finish(runtime: &mut InProcessWorldRuntime, quest_id: i32) -> Vec<ServerPacket> {
    packet(
        runtime,
        ClientPacket::FinishQuest {
            quest_index: quest_id,
            selected_item_index: -1,
        },
    )
}

fn added(packets: &[ServerPacket], id: i32) -> bool {
    packets.iter().any(|packet| {
        matches!(packet, ServerPacket::ChangeQuest {
        quest_id, taken: true, quest_state: 0, ..
    } if *quest_id == id)
    })
}

fn removed(packets: &[ServerPacket], id: i32) -> bool {
    packets.iter().any(|packet| {
        matches!(packet, ServerPacket::ChangeQuest {
        quest_id, taken: false, quest_state: 2, ..
    } if *quest_id == id)
    })
}

fn saved(fixture: &Fixture) -> CharacterSaveRecord {
    fixture.config.account_store.lock().unwrap().accounts[&fixture.account_id].saves[&0].clone()
}

fn assert_same_save(left: &CharacterSaveRecord, right: &CharacterSaveRecord) {
    assert_eq!(
        serde_json::to_value(left).unwrap(),
        serde_json::to_value(right).unwrap()
    );
}

fn row(save: &CharacterSaveRecord, id: i32) -> Option<Value> {
    save.quest_states_json
        .iter()
        .map(|json| serde_json::from_str::<Value>(json).unwrap())
        .find(|value| value["quest_id"] == id)
}

fn locked_reward(save: &CharacterSaveRecord, id: i32) -> Value {
    row(save, id).unwrap()["accepted_periodic_reward"].clone()
}

// Explicitly seed owned fixture state between ordinary sessions. This is not a
// gameplay path: only catalog counters, location/level or buffs under test change.
fn rewrite_fixture(fixture: &Fixture, change: impl FnOnce(&mut CharacterSaveRecord)) {
    let mut store = fixture.config.account_store.lock().unwrap();
    let account = store.accounts.get_mut(&fixture.account_id).unwrap();
    let save = account.saves.get_mut(&0).unwrap();
    change(save);
    save.revision += 1;
    account.characters[0] = save.character.clone();
}

fn seed_accepted_ready(fixture: &Fixture, id: i32) {
    rewrite_fixture(fixture, |save| {
        let mut accepted = row(save, id).expect("ordinary acceptance must already be saved");
        let proof = ready_row(
            id,
            accepted["accepted_periodic_reward"]["level"]
                .as_u64()
                .unwrap() as u16,
            accepted["accepted_periodic_reward"]["experience"]
                .as_u64()
                .unwrap() as u32,
        );
        accepted["task_progress"] = proof["task_progress"].clone();
        accepted["current"] = proof["current"].clone();
        accepted["required"] = proof["required"].clone();
        accepted["stage"] = json!("readyToTurnIn");
        save.quest_states_json
            .retain(|json| serde_json::from_str::<Value>(json).unwrap()["quest_id"] != id);
        save.quest_states_json.push(accepted.to_string());
    });
}

fn xp_buff(percent: i32) -> String {
    json!({"key": "periodic-xp-fixture", "name": "Owned EXP fixture", "description": "Rate locking regression",
        "expires_at_tick": 3600, "real_time_duration": 60000, "attack_bonus": 0, "defence_bonus": 0,
        "stats": [{"stat": 100, "value": percent}]}).to_string()
}

#[test]
fn both_stewards_bootstrap_request_info_and_big_map_have_distinct_images() {
    let _lock = PROFILE_LOCK
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    let _profile = ProfileGuard::newcomer_v2();
    let all_ids = content::catalog()
        .quests
        .iter()
        .map(|quest| quest.id)
        .collect::<BTreeSet<_>>();
    let mut images = Vec::new();
    for npc in &content::catalog().npcs {
        let fixture = fixture(
            &format!("npc-{}", npc.object_id),
            20,
            MirClass::Wizard,
            npc.object_id,
            vec![],
        );
        let (mut runtime, bootstrap) = login(&fixture);
        let info = bootstrap
            .iter()
            .find_map(|packet| match packet {
                ServerPacket::ObjectNpc { info } if info.object_id == npc.object_id => Some(info),
                _ => None,
            })
            .expect("StartGame must advertise the nearby new NPC through ObjectNpc");
        assert_eq!(
            (info.location.x, info.location.y, info.image),
            (npc.x, npc.y, npc.image)
        );
        assert_eq!(
            info.quest_ids.iter().copied().collect::<BTreeSet<_>>(),
            all_ids
        );
        images.push(info.image);
        for requested in &content::catalog().npcs {
            let packets = packet(
                &mut runtime,
                ClientPacket::RequestNpcInfo {
                    npc_index: requested.object_id as i32,
                },
            );
            let info = packets
                .iter()
                .find_map(|packet| match packet {
                    ServerPacket::NewNpcInfo { info } => Some(info),
                    _ => None,
                })
                .unwrap();
            assert_eq!(info.object_id, requested.object_id);
            assert_eq!(info.image, requested.image);
            assert_eq!(
                info.quest_ids.iter().copied().collect::<BTreeSet<_>>(),
                all_ids
            );
        }
        let packets = packet(
            &mut runtime,
            ClientPacket::RequestMapInfo {
                map_index: npc.map_index,
            },
        );
        let info = packets
            .iter()
            .find_map(|packet| match packet {
                ServerPacket::NewMapInfo { map_index, info } if *map_index == npc.map_index => {
                    Some(info)
                }
                _ => None,
            })
            .unwrap();
        let matching = info
            .npcs
            .iter()
            .filter(|entry| entry.object_id == npc.object_id)
            .collect::<Vec<_>>();
        assert_eq!(
            matching.len(),
            1,
            "new NPC must not be lost or duplicated in big-map metadata"
        );
        assert!(matching[0].show_on_big_map);
        assert_eq!(matching[0].image, npc.image);
        assert_eq!(
            (matching[0].location.x, matching[0].location.y),
            (npc.x, npc.y)
        );
        let unique = info
            .npcs
            .iter()
            .map(|entry| entry.object_id)
            .collect::<BTreeSet<_>>();
        assert_eq!(
            unique.len(),
            info.npcs.len(),
            "new IDs must not overwrite/duplicate a loaded old NPC"
        );
    }
    assert_eq!(images.len(), 2);
    assert_ne!(images[0], images[1]);
}

#[test]
fn newcomer_v2_each_class_and_band_offers_three_daily_and_two_weekly_slots() {
    let _lock = PROFILE_LOCK
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    let _profile = ProfileGuard::newcomer_v2();
    for (class_index, class) in [MirClass::Warrior, MirClass::Wizard, MirClass::Taoist]
        .into_iter()
        .enumerate()
    {
        for level in [10, 14, 15, 24, 25, 34, 35, 50] {
            let fixture = fixture(
                &format!("gate-{class_index}-{level}"),
                level,
                class,
                BICHON,
                vec![],
            );
            let (mut runtime, _) = login(&fixture);
            packet(
                &mut runtime,
                ClientPacket::CallNpc {
                    object_id: BICHON,
                    key: "@Main".into(),
                },
            );
            let dialog = runtime.world_snapshot().active_npc_dialog.unwrap();
            let offered = dialog
                .links
                .iter()
                .filter_map(|link| {
                    link.target
                        .strip_prefix("@quest:accept:")?
                        .parse::<i32>()
                        .ok()
                })
                .filter(|id| content::is_periodic(*id))
                .collect::<BTreeSet<_>>();
            let expected = content::catalog()
                .quests
                .iter()
                .filter(|quest| (quest.min_level..=quest.max_level).contains(&(level as i32)))
                .map(|quest| quest.id)
                .collect::<BTreeSet<_>>();
            assert_eq!(offered, expected, "class={class:?}, level={level}");
            assert_eq!(
                offered
                    .iter()
                    .filter(|id| content::quest(**id).unwrap().cadence == PeriodicCadence::Daily)
                    .count(),
                3
            );
            assert_eq!(
                offered
                    .iter()
                    .filter(|id| content::quest(**id).unwrap().cadence == PeriodicCadence::Weekly)
                    .count(),
                2
            );
            let diary_ids = runtime
                .world_snapshot()
                .quest_log
                .iter()
                .filter(|quest| content::is_periodic(quest.quest_id))
                .map(|quest| quest.quest_id)
                .collect::<BTreeSet<_>>();
            assert_eq!(
                diary_ids, expected,
                "out-of-band periodic tasks must not clutter the diary"
            );
        }
    }
    for level in [9, 51] {
        let fixture = fixture(
            &format!("gate-outside-{level}"),
            level,
            MirClass::Warrior,
            BICHON,
            vec![],
        );
        let (mut runtime, _) = login(&fixture);
        packet(
            &mut runtime,
            ClientPacket::CallNpc {
                object_id: BICHON,
                key: "@Main".into(),
            },
        );
        let snapshot = runtime.world_snapshot();
        assert!(!snapshot
            .quest_log
            .iter()
            .any(|quest| content::is_periodic(quest.quest_id)));
        assert!(snapshot
            .active_npc_dialog
            .is_none_or(|dialog| !dialog.links.iter().any(|link| link.target.contains("920"))));
    }
}

#[test]
fn ordinary_daily_and_weekly_acceptance_immediately_commits_locked_rewards() {
    let _lock = PROFILE_LOCK
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    let _profile = ProfileGuard::newcomer_v2();
    let fixture = fixture("accept-memory", 20, MirClass::Taoist, BICHON, vec![]);
    let (mut runtime, _) = login(&fixture);
    let starting = saved(&fixture);
    for (id, expected_exp) in [(DAILY, 14000), (WEEKLY, 70000)] {
        open_offer(&mut runtime, BICHON, id, false);
        let packets = accept(&mut runtime, BICHON, id);
        assert!(added(&packets, id), "{packets:?}");
        let durable = saved(&fixture);
        assert!(durable.revision > starting.revision);
        let quest = row(&durable, id).unwrap();
        assert_eq!(quest["stage"], "inProgress");
        assert_eq!(quest["current"], 0);
        assert_eq!(
            locked_reward(&durable, id),
            json!({"level": 20, "experience": expected_exp, "gold": content::quest(id).unwrap().gold})
        );
        let info = packets
            .iter()
            .find_map(|packet| match packet {
                ServerPacket::NewQuestInfo { info } if info.index == id => Some(info),
                _ => None,
            })
            .unwrap();
        assert_eq!(info.reward_exp, expected_exp);
        assert_eq!(info.reward_gold, content::quest(id).unwrap().gold);
        assert_eq!(
            (durable.gold, durable.experience),
            (starting.gold, starting.experience)
        );
    }
    drop(runtime);
    let (runtime, _) = login(&fixture);
    for id in [DAILY, WEEKLY] {
        assert_eq!(
            runtime
                .world_snapshot()
                .quest_log
                .iter()
                .find(|quest| quest.quest_id == id)
                .unwrap()
                .stage,
            QuestStage::InProgress
        );
        assert!(locked_reward(&runtime.active_character_checkpoint().unwrap(), id).is_object());
    }
}

#[test]
fn native_and_displayed_dialog_finish_paths_commit_once_for_daily_and_weekly() {
    let _lock = PROFILE_LOCK
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    let _profile = ProfileGuard::newcomer_v2();
    for (path_index, path) in ["native", "call-npc", "select-dialog"]
        .into_iter()
        .enumerate()
    {
        for (id, exp) in [(DAILY, 14000), (WEEKLY, 70000)] {
            let fixture = fixture(
                &format!("finish-{path_index}-{id}"),
                20,
                MirClass::Warrior,
                BICHON,
                vec![ready_row(id, 20, exp)],
            );
            let (mut runtime, _) = login(&fixture);
            open_offer(&mut runtime, BICHON, id, true);
            let before = saved(&fixture);
            let target = format!("@quest:finish:{id}");
            let packets = match path {
                "native" => finish(&mut runtime, id),
                "call-npc" => packet(
                    &mut runtime,
                    ClientPacket::CallNpc {
                        object_id: BICHON,
                        key: target.clone(),
                    },
                ),
                _ => runtime
                    .execute(WorldCommand::SelectNpcDialog {
                        target: target.clone(),
                    })
                    .unwrap(),
            };
            assert!(removed(&packets, id), "{path}: {packets:?}");
            let durable = saved(&fixture);
            assert!(durable.revision > before.revision);
            assert_eq!(durable.gold, before.gold + content::quest(id).unwrap().gold);
            assert_eq!(durable.experience, before.experience + exp as i64);
            let quest = row(&durable, id).unwrap();
            assert_eq!(quest["stage"], "completed");
            assert!(quest["cadence_last_claimed_period"].is_u64());
            assert_eq!(
                quest["cadence_last_claimed_period"],
                quest["cadence_high_watermark_period"]
            );
            for repeated in [
                WorldCommand::ClientPacket(ClientPacket::FinishQuest {
                    quest_index: id,
                    selected_item_index: -1,
                }),
                WorldCommand::ClientPacket(ClientPacket::CallNpc {
                    object_id: BICHON,
                    key: target.clone(),
                }),
                WorldCommand::SelectNpcDialog { target },
            ] {
                assert!(!removed(&runtime.execute(repeated).unwrap(), id));
            }
            assert_eq!(
                (
                    saved(&fixture).gold,
                    saved(&fixture).experience,
                    saved(&fixture).revision
                ),
                (durable.gold, durable.experience, durable.revision)
            );
            drop(runtime);
            let (mut runtime, _) = login(&fixture);
            packet(
                &mut runtime,
                ClientPacket::CallNpc {
                    object_id: BICHON,
                    key: "@Main".into(),
                },
            );
            assert!(!removed(&finish(&mut runtime, id), id));
            assert_eq!(
                (saved(&fixture).gold, saved(&fixture).experience),
                (durable.gold, durable.experience)
            );
        }
    }
}

#[test]
fn forged_unshown_finish_alias_is_rejected_without_consuming_the_valid_offer() {
    let _lock = PROFILE_LOCK
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    let _profile = ProfileGuard::newcomer_v2();
    let fixture = fixture(
        "forged-alias",
        20,
        MirClass::Wizard,
        BICHON,
        vec![ready_row(DAILY, 20, 14000)],
    );
    let (mut runtime, _) = login(&fixture);
    open_offer(&mut runtime, BICHON, DAILY, true);
    let before = saved(&fixture);
    let alias = format!("@FinishQuest:{DAILY}");
    let packets = packet(
        &mut runtime,
        ClientPacket::CallNpc {
            object_id: BICHON,
            key: alias.clone(),
        },
    );
    assert!(!removed(&packets, DAILY));
    assert!(!removed(
        &runtime
            .execute(WorldCommand::SelectNpcDialog { target: alias })
            .unwrap(),
        DAILY
    ));
    assert_same_save(&saved(&fixture), &before);
    assert!(
        removed(&finish(&mut runtime, DAILY), DAILY),
        "the real displayed offer remains usable"
    );
}

#[test]
fn cross_town_claim_and_upgrading_band_cannot_repeat_a_cadence_slot() {
    let _lock = PROFILE_LOCK
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    let _profile = ProfileGuard::newcomer_v2();
    let fixture = fixture("cross-town", 20, MirClass::Taoist, BICHON, vec![]);
    let (mut runtime, _) = login(&fixture);
    for id in [DAILY, WEEKLY] {
        open_offer(&mut runtime, BICHON, id, false);
        assert!(added(&accept(&mut runtime, BICHON, id), id));
    }
    drop(runtime);
    for id in [DAILY, WEEKLY] {
        seed_accepted_ready(&fixture, id);
    }
    rewrite_fixture(&fixture, |save| put_near_npc(save, MONGCHON));
    let (mut runtime, _) = login(&fixture);
    for id in [DAILY, WEEKLY] {
        open_offer(&mut runtime, MONGCHON, id, true);
        assert!(removed(&finish(&mut runtime, id), id));
    }
    let claimed = saved(&fixture);
    drop(runtime);
    rewrite_fixture(&fixture, |save| {
        put_near_npc(save, BICHON);
        save.character.level = 25;
        save.experience = 0;
    });
    let (mut runtime, _) = login(&fixture);
    packet(
        &mut runtime,
        ClientPacket::CallNpc {
            object_id: BICHON,
            key: "@Main".into(),
        },
    );
    for id in [DAILY, WEEKLY, 92011, 92014] {
        assert!(!added(&accept(&mut runtime, BICHON, id), id));
        assert!(!removed(&finish(&mut runtime, id), id));
    }
    assert_eq!(saved(&fixture).gold, claimed.gold);
    drop(runtime);
    // A second band's Ready fixture must also be denied at settlement even if
    // its own counters are legal: the first band's slot was already claimed.
    rewrite_fixture(&fixture, |save| {
        save.quest_states_json
            .push(ready_row(92011, 25, 50000).to_string());
        save.quest_states_json
            .push(ready_row(92014, 25, 250000).to_string());
    });
    let (mut runtime, _) = login(&fixture);
    let before = saved(&fixture);
    for id in [92011, 92014] {
        packet(
            &mut runtime,
            ClientPacket::CallNpc {
                object_id: BICHON,
                key: "@Main".into(),
            },
        );
        assert!(!removed(&finish(&mut runtime, id), id));
    }
    assert_eq!(
        (saved(&fixture).gold, saved(&fixture).experience),
        (before.gold, before.experience)
    );
}

#[test]
fn accepted_reward_survives_level_and_exp_rate_changes_without_a_second_multiplier() {
    let _lock = PROFILE_LOCK
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    let _profile = ProfileGuard::newcomer_v2();
    let fixture = fixture("reward-lock", 20, MirClass::Wizard, BICHON, vec![]);
    rewrite_fixture(&fixture, |save| save.buff_states_json = vec![xp_buff(50)]);
    let (mut runtime, _) = login(&fixture);
    assert!(runtime
        .world_snapshot()
        .player_crystal_stats
        .iter()
        .any(|stat| stat.stat == 100 && stat.value == 50));
    open_offer(&mut runtime, BICHON, DAILY, false);
    assert!(added(&accept(&mut runtime, BICHON, DAILY), DAILY));
    assert_eq!(locked_reward(&saved(&fixture), DAILY)["experience"], 21000);
    drop(runtime);
    seed_accepted_ready(&fixture, DAILY);
    rewrite_fixture(&fixture, |save| {
        save.character.level = 25;
        save.experience = 0;
        save.buff_states_json = vec![xp_buff(100)];
    });
    let (mut runtime, bootstrap) = login(&fixture);
    assert!(runtime
        .world_snapshot()
        .player_crystal_stats
        .iter()
        .any(|stat| stat.stat == 100 && stat.value == 100));
    if let Some(info) = bootstrap.iter().find_map(|packet| match packet {
        ServerPacket::NewQuestInfo { info } if info.index == DAILY => Some(info),
        _ => None,
    }) {
        assert_eq!(
            info.reward_exp, 21000,
            "locked absolute reward must reach the client"
        );
    }
    open_offer(&mut runtime, BICHON, DAILY, true);
    let before = saved(&fixture);
    assert!(removed(&finish(&mut runtime, DAILY), DAILY));
    let durable = saved(&fixture);
    assert_eq!(durable.character.level, 25);
    assert_eq!(
        durable.experience, 21000,
        "neither level-25 recalculation nor 100% finish-time bonus applies"
    );
    assert_eq!(durable.gold, before.gold + 8000);
    assert_eq!(
        locked_reward(&durable, DAILY),
        json!({"level": 20, "experience": 21000, "gold": 8000})
    );
}

#[test]
fn missing_or_invalid_locked_reward_cannot_award_a_ready_fixture() {
    let _lock = PROFILE_LOCK
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    let _profile = ProfileGuard::newcomer_v2();
    for (index, invalid) in [
        Value::Null,
        json!({"level": 20, "experience": 0, "gold": 8000}),
        json!({"level": 20, "experience": 14000, "gold": 0}),
        json!({"level": 0, "experience": 14000, "gold": 8000}),
    ]
    .into_iter()
    .enumerate()
    {
        let mut quest = ready_row(DAILY, 20, 14000);
        quest["accepted_periodic_reward"] = invalid;
        let fixture = fixture(
            &format!("invalid-lock-{index}"),
            20,
            MirClass::Warrior,
            BICHON,
            vec![quest],
        );
        let (mut runtime, _) = login(&fixture);
        packet(
            &mut runtime,
            ClientPacket::CallNpc {
                object_id: BICHON,
                key: "@Main".into(),
            },
        );
        let before = saved(&fixture);
        assert!(!removed(&finish(&mut runtime, DAILY), DAILY));
        assert_eq!(
            (saved(&fixture).gold, saved(&fixture).experience),
            (before.gold, before.experience)
        );
        assert_ne!(
            row(&runtime.active_character_checkpoint().unwrap(), DAILY).unwrap()["stage"],
            "completed"
        );
    }
}

#[test]
fn saved_ready_stage_without_catalog_kill_proof_is_downgraded_and_cannot_finish() {
    let _lock = PROFILE_LOCK
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    let _profile = ProfileGuard::newcomer_v2();
    for (index, progress) in [json!({}), json!({"kill:48": 69}), json!({"kill:85": 70})]
        .into_iter()
        .enumerate()
    {
        let mut quest = ready_row(DAILY, 20, 14000);
        quest["task_progress"] = progress;
        let fixture = fixture(
            &format!("invalid-ready-proof-{index}"),
            20,
            MirClass::Warrior,
            BICHON,
            vec![quest],
        );
        let (mut runtime, _) = login(&fixture);
        assert_eq!(
            runtime
                .world_snapshot()
                .quest_log
                .iter()
                .find(|quest| quest.quest_id == DAILY)
                .unwrap()
                .stage,
            QuestStage::InProgress
        );
        packet(
            &mut runtime,
            ClientPacket::CallNpc {
                object_id: BICHON,
                key: "@Main".into(),
            },
        );
        let before = saved(&fixture);
        assert!(!removed(&finish(&mut runtime, DAILY), DAILY));
        assert_eq!(
            (saved(&fixture).gold, saved(&fixture).experience),
            (before.gold, before.experience)
        );
    }
}

#[test]
fn malformed_locked_reward_fails_start_game_before_a_reward_path_exists() {
    let _lock = PROFILE_LOCK
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    let _profile = ProfileGuard::newcomer_v2();
    let mut quest = ready_row(DAILY, 20, 14000);
    quest["accepted_periodic_reward"] = json!({"level": 20, "gold": 8000});
    let fixture = fixture("malformed-lock", 20, MirClass::Warrior, BICHON, vec![quest]);
    let before = saved(&fixture);
    let mut runtime = InProcessWorldRuntime::new(fixture.config.clone());
    let packets = packet(
        &mut runtime,
        ClientPacket::Login {
            account_id: fixture.account_id.clone(),
            password: "demo".into(),
        },
    );
    assert!(packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
    let started = packet(&mut runtime, ClientPacket::StartGame { character_index: 0 });
    assert!(!started
        .iter()
        .any(|packet| matches!(packet, ServerPacket::StartGame { result: 4, .. })));
    assert!(!removed(&finish(&mut runtime, DAILY), DAILY));
    assert_same_save(&saved(&fixture), &before);
}

#[test]
fn abandon_commits_cleared_lock_and_allows_ordinary_reacceptance() {
    let _lock = PROFILE_LOCK
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    let _profile = ProfileGuard::newcomer_v2();
    let fixture = fixture("abandon", 20, MirClass::Taoist, BICHON, vec![]);
    let (mut runtime, _) = login(&fixture);
    open_offer(&mut runtime, BICHON, DAILY, false);
    assert!(added(&accept(&mut runtime, BICHON, DAILY), DAILY));
    let accepted = saved(&fixture);
    assert!(removed(
        &packet(
            &mut runtime,
            ClientPacket::AbandonQuest { quest_index: DAILY }
        ),
        DAILY
    ));
    let abandoned = saved(&fixture);
    assert!(abandoned.revision > accepted.revision);
    let quest = row(&abandoned, DAILY).unwrap();
    assert_eq!(quest["stage"], "available");
    assert_eq!(quest["current"], 0);
    assert!(quest["task_progress"].as_object().unwrap().is_empty());
    assert!(quest
        .get("accepted_periodic_reward")
        .is_none_or(Value::is_null));
    assert!(!quest["cadence_last_claimed_period"].is_u64());
    drop(runtime);
    let (mut runtime, _) = login(&fixture);
    open_offer(&mut runtime, BICHON, DAILY, false);
    assert!(added(&accept(&mut runtime, BICHON, DAILY), DAILY));
    assert_eq!(locked_reward(&saved(&fixture), DAILY)["experience"], 14000);
}

#[test]
fn unopened_offer_unknown_ids_wrong_town_and_stale_far_offer_cannot_claim() {
    let _lock = PROFILE_LOCK
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    let _profile = ProfileGuard::newcomer_v2();
    let fixture = fixture(
        "offer-guards",
        20,
        MirClass::Warrior,
        BICHON,
        vec![ready_row(DAILY, 20, 14000)],
    );
    let (mut runtime, _) = login(&fixture);
    let before = saved(&fixture);
    assert!(!removed(&finish(&mut runtime, DAILY), DAILY));
    assert!(!removed(
        &runtime
            .execute(WorldCommand::SelectNpcDialog {
                target: format!("@quest:finish:{DAILY}")
            })
            .unwrap(),
        DAILY
    ));
    assert!(!added(&accept(&mut runtime, BICHON, WEEKLY), WEEKLY));
    packet(
        &mut runtime,
        ClientPacket::CallNpc {
            object_id: BICHON,
            key: "@Main".into(),
        },
    );
    assert!(!added(&accept(&mut runtime, MONGCHON, WEEKLY), WEEKLY));
    assert!(!added(&accept(&mut runtime, BICHON, 92999), 92999));
    assert!(!removed(&finish(&mut runtime, 92999), 92999));
    assert!(!removed(
        &packet(
            &mut runtime,
            ClientPacket::CallNpc {
                object_id: BICHON,
                key: "@quest:finish:92999".into()
            }
        ),
        92999
    ));
    assert_same_save(&saved(&fixture), &before);
    open_offer(&mut runtime, BICHON, DAILY, true);
    // Model a legitimate shared-Zone transform after the page was displayed.
    // The client still cannot use the now-out-of-range old offer.
    runtime.force_authoritative_player_transform(Point { x: 400, y: 400 }, MirDirection::Down);
    assert!(!removed(&finish(&mut runtime, DAILY), DAILY));
    assert!(!removed(
        &packet(
            &mut runtime,
            ClientPacket::CallNpc {
                object_id: BICHON,
                key: format!("@quest:finish:{DAILY}")
            }
        ),
        DAILY
    ));
    assert_eq!(
        (saved(&fixture).gold, saved(&fixture).experience),
        (before.gold, before.experience)
    );
    assert_eq!(
        row(&runtime.active_character_checkpoint().unwrap(), DAILY).unwrap()["stage"],
        "readyToTurnIn"
    );
}

#[test]
fn stale_full_save_claim_restores_the_durable_winner_without_a_success_ack() {
    let _lock = PROFILE_LOCK
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    let _profile = ProfileGuard::newcomer_v2();
    let fixture = fixture(
        "stale-cas",
        20,
        MirClass::Warrior,
        BICHON,
        vec![ready_row(DAILY, 20, 14000)],
    );
    let (mut winner, _) = login(&fixture);
    let (mut stale, _) = login(&fixture);
    open_offer(&mut winner, BICHON, DAILY, true);
    open_offer(&mut stale, BICHON, DAILY, true);
    assert!(removed(&finish(&mut winner, DAILY), DAILY));
    let committed = saved(&fixture);
    let error = stale
        .execute(WorldCommand::ClientPacket(ClientPacket::FinishQuest {
            quest_index: DAILY,
            selected_item_index: -1,
        }))
        .expect_err("a stale claim must not return the successful packet vector");
    assert!(
        error.contains("stale full character save rejected"),
        "{error}"
    );
    assert_same_save(&saved(&fixture), &committed);
    let restored = stale.active_character_checkpoint().unwrap();
    assert_eq!(
        (restored.revision, restored.gold, restored.experience),
        (committed.revision, committed.gold, committed.experience)
    );
    assert_eq!(row(&restored, DAILY).unwrap()["stage"], "completed");
    packet(
        &mut stale,
        ClientPacket::CallNpc {
            object_id: BICHON,
            key: "@Main".into(),
        },
    );
    assert!(!removed(&finish(&mut stale, DAILY), DAILY));
    assert_same_save(&saved(&fixture), &committed);
}

#[cfg(feature = "test-support")]
#[test]
fn accept_before_persist_failure_does_not_ack_or_leave_an_unsaved_lock() {
    let _lock = PROFILE_LOCK
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    let _profile = ProfileGuard::newcomer_v2();
    let fixture = fixture("accept-fault", 20, MirClass::Taoist, BICHON, vec![]);
    let (mut runtime, _) = login(&fixture);
    open_offer(&mut runtime, BICHON, WEEKLY, false);
    let before = saved(&fixture);
    fixture.config.inject_account_store_transaction_fault(
        mir2_simulation::AccountStoreTransactionFault::BeforePersist,
    );
    runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::AcceptQuest {
            npc_index: BICHON,
            quest_index: WEEKLY,
        }))
        .expect_err("uncommitted acceptance must not expose ADD success");
    assert_same_save(&saved(&fixture), &before);
    let restored = runtime.active_character_checkpoint().unwrap();
    assert!(row(&restored, WEEKLY).is_none_or(
        |quest| quest["stage"] == "available" && quest["accepted_periodic_reward"].is_null()
    ));
    open_offer(&mut runtime, BICHON, WEEKLY, false);
    assert!(added(&accept(&mut runtime, BICHON, WEEKLY), WEEKLY));
    assert_eq!(locked_reward(&saved(&fixture), WEEKLY)["experience"], 70000);
}

#[cfg(feature = "test-support")]
#[test]
fn definite_persistence_faults_roll_back_rewards_and_leave_a_retryable_offer() {
    let _lock = PROFILE_LOCK
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    let _profile = ProfileGuard::newcomer_v2();
    for (index, fault) in [
        mir2_simulation::AccountStoreTransactionFault::BeforePersist,
        mir2_simulation::AccountStoreTransactionFault::Persist,
    ]
    .into_iter()
    .enumerate()
    {
        let fixture = fixture(
            &format!("finish-fault-{index}"),
            20,
            MirClass::Warrior,
            BICHON,
            vec![ready_row(WEEKLY, 20, 70000)],
        );
        let (mut runtime, _) = login(&fixture);
        open_offer(&mut runtime, BICHON, WEEKLY, true);
        let before = saved(&fixture);
        fixture.config.inject_account_store_transaction_fault(fault);
        runtime
            .execute(WorldCommand::ClientPacket(ClientPacket::FinishQuest {
                quest_index: WEEKLY,
                selected_item_index: -1,
            }))
            .expect_err("uncommitted reward must not expose REMOVE success");
        assert_same_save(&saved(&fixture), &before);
        let restored = runtime.active_character_checkpoint().unwrap();
        assert_eq!(
            (restored.gold, restored.experience, restored.revision),
            (before.gold, before.experience, before.revision)
        );
        assert_eq!(row(&restored, WEEKLY).unwrap()["stage"], "readyToTurnIn");
        open_offer(&mut runtime, BICHON, WEEKLY, true);
        assert!(removed(&finish(&mut runtime, WEEKLY), WEEKLY));
        assert_eq!(saved(&fixture).gold, before.gold + 60000);
    }
}

#[cfg(feature = "test-support")]
fn file_fixture(label: &str, rows: Vec<Value>) -> (Fixture, std::path::PathBuf) {
    let fixture = fixture(label, 20, MirClass::Wizard, BICHON, rows);
    let seed = fixture.config.account_store.lock().unwrap().clone();
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let directory = std::env::temp_dir().join(format!(
        "mir2-periodic-packet-{label}-{}-{unique}",
        std::process::id()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("owned-accounts.json");
    let config = fixture.config.with_account_store_path(&path);
    *config.account_store.lock().unwrap() = seed;
    config.save_account_store().unwrap();
    (
        Fixture {
            config,
            account_id: fixture.account_id,
        },
        path,
    )
}

#[cfg(feature = "test-support")]
#[test]
fn file_acceptance_and_reward_are_durable_before_ack_and_survive_reopen() {
    let _lock = PROFILE_LOCK
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    let _profile = ProfileGuard::newcomer_v2();
    let (fixture, path) = file_fixture("file-roundtrip", vec![]);
    let (mut runtime, _) = login(&fixture);
    for id in [DAILY, WEEKLY] {
        open_offer(&mut runtime, BICHON, id, false);
        assert!(added(&accept(&mut runtime, BICHON, id), id));
        let disk: mir2_simulation::AccountStore =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        let durable = &disk.accounts[&fixture.account_id].saves[&0];
        assert_eq!(row(durable, id).unwrap()["stage"], "inProgress");
        assert!(locked_reward(durable, id).is_object());
        assert_same_save(durable, &saved(&fixture));
    }
    drop(runtime);
    for id in [DAILY, WEEKLY] {
        seed_accepted_ready(&fixture, id);
    }
    fixture.config.save_account_store().unwrap();
    let (mut runtime, _) = login(&fixture);
    for id in [DAILY, WEEKLY] {
        open_offer(&mut runtime, BICHON, id, true);
        assert!(removed(&finish(&mut runtime, id), id));
        let disk: mir2_simulation::AccountStore =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_same_save(
            &disk.accounts[&fixture.account_id].saves[&0],
            &saved(&fixture),
        );
        assert_eq!(row(&saved(&fixture), id).unwrap()["stage"], "completed");
    }
    let committed = saved(&fixture);
    let account_id = fixture.account_id.clone();
    drop(runtime);
    drop(fixture);
    let reopened = Fixture {
        config: SimulationConfig::default()
            .with_crystal_map_runtime()
            .with_platinum_176_profile()
            .with_account_store_path(&path),
        account_id,
    };
    let (mut runtime, _) = login(&reopened);
    packet(
        &mut runtime,
        ClientPacket::CallNpc {
            object_id: BICHON,
            key: "@Main".into(),
        },
    );
    for id in [DAILY, WEEKLY] {
        assert!(!removed(&finish(&mut runtime, id), id));
    }
    assert_same_save(&saved(&reopened), &committed);
    // Keep this small owned fixture for failed-run diagnosis; never remove any
    // user account directory or read a deployment account-store path.
}

#[cfg(feature = "test-support")]
#[test]
fn file_before_rename_failure_keeps_original_bytes_and_withholds_reward_success() {
    let _lock = PROFILE_LOCK
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    let _profile = ProfileGuard::newcomer_v2();
    let (fixture, path) = file_fixture("file-before-rename", vec![ready_row(DAILY, 20, 14000)]);
    let (mut runtime, _) = login(&fixture);
    open_offer(&mut runtime, BICHON, DAILY, true);
    let bytes = std::fs::read(&path).unwrap();
    let before = saved(&fixture);
    fixture.config.inject_account_store_transaction_fault(
        mir2_simulation::AccountStoreTransactionFault::BeforeFileRename,
    );
    runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::FinishQuest {
            quest_index: DAILY,
            selected_item_index: -1,
        }))
        .expect_err("pre-publication failure must not return the success ACK");
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    assert_same_save(&saved(&fixture), &before);
    assert_eq!(
        row(&runtime.active_character_checkpoint().unwrap(), DAILY).unwrap()["stage"],
        "readyToTurnIn"
    );
    open_offer(&mut runtime, BICHON, DAILY, true);
    assert!(removed(&finish(&mut runtime, DAILY), DAILY));
    assert_eq!(saved(&fixture).gold, before.gold + 8000);
}

#[cfg(feature = "test-support")]
#[test]
fn unknown_file_publication_withholds_success_and_fences_further_claims_and_saves() {
    let _lock = PROFILE_LOCK
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    let _profile = ProfileGuard::newcomer_v2();
    let (fixture, path) = file_fixture(
        "file-unknown-publication",
        vec![ready_row(DAILY, 20, 14000)],
    );
    let (mut runtime, _) = login(&fixture);
    open_offer(&mut runtime, BICHON, DAILY, true);
    let before = saved(&fixture);
    fixture.config.inject_account_store_transaction_fault(
        mir2_simulation::AccountStoreTransactionFault::AfterFileRenameBeforeDirectorySync,
    );
    let error = runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::FinishQuest {
            quest_index: DAILY,
            selected_item_index: -1,
        }))
        .expect_err("uncertain publication must not return the successful packet vector");
    assert!(
        error.contains("ACCOUNT_STORE_COMMIT_OUTCOME_UNKNOWN"),
        "{error}"
    );
    let published_bytes = std::fs::read(&path).unwrap();
    let disk: mir2_simulation::AccountStore = serde_json::from_slice(&published_bytes).unwrap();
    let disk_save = &disk.accounts[&fixture.account_id].saves[&0];
    assert_eq!(row(disk_save, DAILY).unwrap()["stage"], "completed");
    assert_eq!(disk_save.gold, before.gold + 8000);
    assert_eq!(disk_save.experience, before.experience + 14000);
    for command in [
        WorldCommand::ClientPacket(ClientPacket::FinishQuest {
            quest_index: DAILY,
            selected_item_index: -1,
        }),
        WorldCommand::ClientPacket(ClientPacket::CallNpc {
            object_id: BICHON,
            key: "@Main".into(),
        }),
    ] {
        assert!(
            runtime.execute(command).is_err(),
            "frozen authority must not accept a second mutation"
        );
    }
    assert!(runtime.save_active_character().is_err());
    assert!(fixture.config.save_account_store().is_err());
    assert_eq!(std::fs::read(&path).unwrap(), published_bytes);
    let rebound = fixture.config.clone().with_account_store_path(&path);
    assert!(
        rebound.save_account_store().is_err(),
        "rebind must not clear the publication fence"
    );
}
