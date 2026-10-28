//! Mining crosses the shared Zone and exact personal inventory authority.
//! Only progression and position are prepared by the server fixture. Shop,
//! equipment, mining ingress, selling, repair and login use ordinary packets.
//! Pure prepared-swing cases vary trusted timestamps, never mining rates.
use super::*;
use mir2_simulation::{ItemContainer, ZoneMiningSwing, ZoneMiningTool};
use serde_json::{json, Value};
use std::{
    fs,
    io::Read,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

static ENVIRONMENT: Mutex<()> = Mutex::new(());
static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

struct MiningEnvironment(Option<std::ffi::OsString>);
impl MiningEnvironment {
    fn enter() -> Self {
        let previous = std::env::var_os("MIR2_CRYSTAL_MAP_PACK");
        let pack = previous.clone().map(PathBuf::from).unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("src/routing/tests/fixtures/mining-map-pack")
        });
        assert!(
            pack.join("0.map.gz").is_file() && pack.join("d401.map.gz").is_file(),
            "normal mining QA requires original Bichon and D401 collision: {pack:?}"
        );
        std::env::set_var("MIR2_CRYSTAL_MAP_PACK", pack);
        mir2_simulation::set_crystal_full_world_zone_collision(true);
        Self(previous)
    }
}
impl Drop for MiningEnvironment {
    fn drop(&mut self) {
        mir2_simulation::set_crystal_full_world_zone_collision(false);
        match &self.0 {
            Some(value) => std::env::set_var("MIR2_CRYSTAL_MAP_PACK", value),
            None => std::env::remove_var("MIR2_CRYSTAL_MAP_PACK"),
        }
    }
}

struct Fixture {
    runtime: SharedInProcessZoneSessionRuntime,
    config: GatewayConfig,
    path: PathBuf,
    wall: Point,
    next_time: u64,
    tool_uid: u64,
}

fn packet(
    runtime: &mut SharedInProcessZoneSessionRuntime,
    packet: ClientPacket,
) -> Vec<ServerPacket> {
    runtime.execute(WorldCommand::ClientPacket(packet)).unwrap()
}

fn source_wall() -> (Point, Point) {
    let pack = PathBuf::from(std::env::var_os("MIR2_CRYSTAL_MAP_PACK").unwrap());
    let compressed = fs::read(pack.join("d401.map.gz")).unwrap();
    let mut bytes = Vec::new();
    flate2::read::GzDecoder::new(compressed.as_slice())
        .read_to_end(&mut bytes)
        .unwrap();
    let v100 = bytes[0..4] == [1, 0, 0x43, 0x23];
    let read16 = |offset| i16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap());
    let (width, height, start, xor) = if v100 {
        (i32::from(read16(4)), i32::from(read16(6)), 8, 0)
    } else {
        assert_eq!(
            (bytes[0], bytes[2], bytes[7], bytes[14]),
            (0x10, 0x61, 0x31, 0x31),
            "source D401 must use known original v1 or canonical v100 cells"
        );
        let xor = read16(23);
        (
            i32::from(read16(21) ^ xor),
            i32::from(read16(25) ^ xor),
            54,
            xor,
        )
    };
    assert!(width > 0 && height > 0);
    let mut walkable = vec![false; (width * height) as usize];
    let mut offset = start;
    for x in 0..width {
        for y in 0..height {
            let (high, low, door) = if v100 {
                let high = i32::from_le_bytes(bytes[offset + 2..offset + 6].try_into().unwrap())
                    & 0x2000_0000
                    != 0;
                let low = read16(offset + 12) & i16::MIN != 0;
                let door = bytes[offset + 14] > 0;
                offset += 26;
                (high, low, door)
            } else {
                let high = (i32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
                    ^ 0xAA38_AA38_u32 as i32)
                    & 0x2000_0000
                    != 0;
                let low = (read16(offset + 6) ^ xor) & i16::MIN != 0;
                let door = bytes[offset + 8] > 0;
                offset += 15;
                (high, low, door)
            };
            walkable[(x * height + y) as usize] = !high && !low && !door;
        }
    }
    assert_eq!(offset, bytes.len());
    // Closest wall to the legal mine entrance limits unrelated monster AOI.
    let mut cells = Vec::new();
    for x in 1..width - 1 {
        for y in 1..height - 1 {
            if walkable[(x * height + y) as usize] && !walkable[(x * height + y - 1) as usize] {
                cells.push((Point { x, y }, Point { x, y: y - 1 }));
            }
        }
    }
    cells.sort_by_key(|(p, _)| (p.x - 24).abs() + (p.y - 181).abs());
    cells
        .into_iter()
        .next()
        .expect("source D401 has a walkable tile below an in-bounds wall")
}

fn prepare_transform(
    runtime: &mut SharedInProcessZoneSessionRuntime,
    map: &str,
    title: &str,
    position: Point,
    level: u16,
    gold: Option<u32>,
) {
    let mut save = runtime.inner.active_character_checkpoint().unwrap();
    save.character.level = level;
    save.map_file_name = map.into();
    save.map_title = title.into();
    save.position = position.clone();
    save.direction = MirDirection::Up;
    if let Some(gold) = gold {
        save.gold = gold;
    }
    runtime
        .inner
        .restore_active_character_checkpoint(&save)
        .unwrap();
    runtime.force_next_zone_transform_sync = true;
    runtime.sync_zone_snapshot();
    let sid = runtime.current_zone_session_id().unwrap();
    runtime.dispatch_zone_player_command(
        ZoneCommand::SyncPlayerTransform {
            session_id: sid.clone(),
            position,
            direction: MirDirection::Up,
        },
        false,
    );
    runtime.sync_authoritative_zone_combat_state(&sid).unwrap();
}

fn call_smith(runtime: &mut SharedInProcessZoneSessionRuntime, key: &str) -> Vec<ServerPacket> {
    let smith =
        mir2_game_data::crystal_npc_info_by_script_key("BichonProvince/BorderVillage/Blacksmith")
            .unwrap();
    assert_eq!(
        (
            smith.map_file_name.as_deref(),
            smith.location.x,
            smith.location.y
        ),
        (Some("0"), 296, 613)
    );
    packet(
        runtime,
        ClientPacket::CallNpc {
            object_id: smith
                .loaded_object_id
                .expect("source Smith has a loaded NPC identity"),
            key: key.into(),
        },
    )
}

fn purchased_fixture(class: MirClass, label: &str) -> Fixture {
    let serial = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "mir2-mining-{}-{}-{serial}.json",
        std::process::id(),
        shared_gateway_now_ms()
    ));
    let config = GatewayConfig::default()
        .with_crystal_world_runtime()
        .with_platinum_176_profile()
        .with_account_store_path(path.clone())
        .with_save_recovery_dir(path.with_extension("recovery"))
        .with_save_recovery_mac_key(std::array::from_fn::<_, 32, _>(|index| {
            (index as u8).wrapping_mul(7)
        }))
        .unwrap();
    let shared = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
    let mut runtime = shared_session_runtime(shared);
    runtime.inner = InProcessWorldRuntime::new(config.clone());
    let account = format!("Mine{label}{serial}");
    start_new_runtime_with_class(&mut runtime, &account, &format!("Miner{serial}"), class);
    prepare_transform(
        &mut runtime,
        "0",
        "BichonProvince",
        Point { x: 296, y: 612 },
        12,
        Some(20_000),
    );
    call_smith(&mut runtime, "@main");
    let goods = call_smith(&mut runtime, "@BuySell");
    let pick = goods
        .iter()
        .find_map(|p| match p {
            ServerPacket::NPCGoods { list, .. } => {
                list.iter().find(|i| i.item_index == 836).cloned()
            }
            _ => None,
        })
        .expect("original Smith sells PickAxe");
    let before_gold = runtime.inner.world_snapshot().gold;
    let bought = packet(
        &mut runtime,
        ClientPacket::BuyItem {
            item_index: pick.unique_id,
            count: 1,
            panel_type: 0,
        },
    );
    let tool = bought
        .iter()
        .find_map(|p| match p {
            ServerPacket::GainedItem { item } if item.item_index == 836 => Some(item.clone()),
            _ => None,
        })
        .expect("ordinary purchase grants source PickAxe");
    assert_eq!(runtime.inner.world_snapshot().gold, before_gold - 2_500);
    assert_ne!(tool.unique_id, 0);
    assert_ne!(
        tool.unique_id, 1,
        "a purchased instance must not become the synthetic Weapon UID"
    );
    let equipped = packet(
        &mut runtime,
        ClientPacket::EquipItem {
            grid: MirGridType::Inventory,
            unique_id: tool.unique_id,
            to: 0,
        },
    );
    assert!(
        equipped.iter().any(
            |p| matches!(p,ServerPacket::EquipItem{unique_id,success:true,..}
        if *unique_id==tool.unique_id)
        ),
        "source PickAxe must equip for {class:?}: {equipped:?}"
    );
    assert_eq!(
        runtime.inner.shared_mining_tool().unwrap().unique_id,
        tool.unique_id
    );
    let (position, wall) = source_wall();
    prepare_transform(&mut runtime, "D401", "DeadMineEntrance", position, 12, None);
    let next_time = SharedInProcessZoneSessionRuntime::zone_now_ms().saturating_add(10_000);
    Fixture {
        runtime,
        config,
        path,
        wall,
        next_time,
        tool_uid: tool.unique_id,
    }
}

fn zone_state(f: &Fixture) -> Value {
    let sid = f.runtime.current_zone_session_id().unwrap();
    let state = f.runtime.zone_state.lock().unwrap();
    let key = state.zone_manager.zone_key_for_session(&sid).unwrap();
    serde_json::from_slice(
        &state
            .zone_manager
            .zone(&key)
            .unwrap()
            .checkpoint_bytes()
            .unwrap(),
    )
    .unwrap()
}
fn spot(f: &Fixture) -> Value {
    zone_state(f)["mining"]["spots"][format!("{},{}", f.wall.x, f.wall.y)].clone()
}

fn prepared(f: &mut Fixture, require_ore: bool, above_max: bool) -> ZoneMiningSwing {
    let sid = f.runtime.current_zone_session_id().unwrap();
    let tool = f.runtime.inner.shared_mining_tool().unwrap();
    for _ in 0..8192 {
        let state = f.runtime.zone_state.lock().unwrap();
        let swing = state
            .zone_manager
            .prepare_mining_swing(&sid, MirDirection::Up, &tool, f.next_time)
            .expect("prepared fixture must remain admitted at source wall");
        drop(state);
        if !require_ore
            || swing
                .ore()
                .is_some_and(|o| !above_max || o.current_dura > 10_000)
        {
            return swing;
        }
        f.next_time = f.next_time.saturating_add(tool.attack_delay_ms.max(550));
    }
    panic!("ordinary source rates did not produce requested deterministic branch");
}
fn commit(f: &mut Fixture, swing: &ZoneMiningSwing) -> Vec<ServerPacket> {
    let sid = f.runtime.current_zone_session_id().unwrap();
    let mut state = f.runtime.zone_state.lock().unwrap();
    let result = f.runtime.inner.apply_shared_mining_swing(swing).unwrap();
    assert!(state
        .zone_manager
        .commit_mining_swing(&sid, swing)
        .is_some());
    f.next_time = f.next_time.saturating_add(
        f.runtime
            .inner
            .shared_mining_tool()
            .unwrap()
            .attack_delay_ms
            .max(550),
    );
    result
}
fn refill(f: &mut Fixture) {
    for _ in 0..5 {
        let swing = prepared(f, false, false);
        assert_eq!(swing.tool_damage(), 0);
        assert!(swing.ore().is_none());
        assert!(commit(f, &swing).is_empty());
        if spot(f)["stones_left"].as_u64().unwrap() > 0 {
            return;
        }
        f.next_time = spot(f)["regen_at_ms"].as_u64().unwrap() + 1;
    }
    panic!("source refill returned zero five times");
}

#[test]
fn mining_original_d401_strict_checkpoint_restores_stone_clock_and_pending_effect() {
    let _lock = ENVIRONMENT
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let _environment = MiningEnvironment::enter();
    let (position, wall) = source_wall();
    let key = ZoneKey::for_map("D401");
    let collision = serde_json::to_value(ZoneCollision::for_map("D401")).unwrap();
    assert!(
        collision["bounds"].is_object(),
        "use the bounded original mine"
    );
    assert!(collision["blocked_cells"].as_array().unwrap().len() > 1_000);
    assert!(collision["blocked_cells"]
        .as_array()
        .unwrap()
        .contains(&json!([wall.x, wall.y])));

    let mut uninterrupted = ZoneRuntime::new(key);
    let sid = SessionId::new("d401-checkpoint-miner");
    uninterrupted.handle(ZoneCommand::Join(ZoneJoin {
        session_id: sid.clone(),
        account_id: "D401Checkpoint".into(),
        character_index: 1,
        object_id: 900_001,
        name: "MineCheckpoint".into(),
        class: MirClass::Warrior,
        gender: MirGender::Male,
        level: 30,
        hp: 200,
        max_hp: 200,
        mp: 100,
        map_file_name: "D401".into(),
        position: position.clone(),
        direction: MirDirection::Up,
        chat_profile: Default::default(),
        combat_stats: Default::default(),
    }));
    uninterrupted.handle(ZoneCommand::sync_player_combat_state(
        sid.clone(),
        MirClass::Warrior,
        false,
        false,
        false,
        false,
        false,
        false,
    ));
    let template = mir2_game_data::crystal_item_by_name("PickAxe").unwrap();
    assert_eq!((template.item_index, template.can_mine), (836, true));
    let stat = |id| {
        template
            .stats
            .iter()
            .filter(|s| s.stat == id)
            .map(|s| s.value)
            .sum::<i32>()
    };
    // Trusted level-30 fixture, ordinary source PickAxe and no added stats.
    // Only timestamps choose a hit branch; production hit/drop rates remain.
    let mut tool = ZoneMiningTool {
        unique_id: 900_002,
        durability: template.durability,
        accuracy: stat(10),
        strong: stat(20),
        mine_rate_percent: stat(103),
        attack_delay_ms: 1_030,
    };
    assert_eq!(
        (tool.accuracy, tool.strong, tool.mine_rate_percent),
        (0, 0, 0)
    );
    let checkpoint = |zone: &ZoneRuntime| -> Value {
        serde_json::from_slice(&zone.checkpoint_bytes().unwrap()).unwrap()
    };
    let wall_key = format!("{},{}", wall.x, wall.y);
    let mut refill_at = 1_000_000;
    let mut available = 0;
    for _ in 0..32 {
        let swing = uninterrupted
            .prepare_mining_swing(&sid, MirDirection::Up, &tool, refill_at)
            .unwrap();
        assert_eq!(swing.tool_damage(), 0);
        assert!(swing.ore().is_none());
        uninterrupted.commit_mining_swing(&swing).unwrap();
        available = checkpoint(&uninterrupted)["mining"]["spots"][&wall_key]["stones_left"]
            .as_u64()
            .unwrap();
        if available >= 2 {
            break;
        }
        if available == 1 {
            let drain_at = refill_at + tool.attack_delay_ms;
            let drain = uninterrupted
                .prepare_mining_swing(&sid, MirDirection::Up, &tool, drain_at)
                .unwrap();
            uninterrupted.commit_mining_swing(&drain).unwrap();
            tool.durability = tool.durability.saturating_sub(drain.tool_damage());
            uninterrupted.tick(drain_at + 400);
        }
        refill_at = checkpoint(&uninterrupted)["mining"]["spots"][&wall_key]["regen_at_ms"]
            .as_u64()
            .unwrap()
            + 1;
    }
    assert!(
        available >= 2,
        "bounded source refill search must leave two stones"
    );
    let sequence = checkpoint(&uninterrupted)["mining"]["sequence"]
        .as_u64()
        .unwrap();
    let mut hit = None;
    for offset in 0..4_096 {
        let at = refill_at + tool.attack_delay_ms + offset;
        let swing = uninterrupted
            .prepare_mining_swing(&sid, MirDirection::Up, &tool, at)
            .unwrap();
        if swing.tool_damage() > 0 {
            hit = Some((at, swing));
            break;
        }
    }
    let (hit_at, swing) = hit.expect("ordinary source PickAxe eventually hits the real wall");
    uninterrupted.commit_mining_swing(&swing).unwrap();
    tool.durability = tool.durability.saturating_sub(swing.tool_damage());
    let bytes = uninterrupted.checkpoint_bytes().unwrap();
    let before: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(before["version"], 7);
    assert_eq!(
        before["state_root"],
        uninterrupted.canonical_state_root().unwrap()
    );
    assert_eq!(before["mining"]["sequence"], sequence + 1);
    assert_eq!(
        before["mining"]["spots"][&wall_key]["stones_left"],
        available - 1
    );
    assert_eq!(
        before["mining"]["spots"][&wall_key]["regen_at_ms"],
        refill_at + 300_000
    );
    let effects = before["mining"]["effects"].as_array().unwrap();
    assert_eq!(effects.len(), 1);
    assert_eq!(effects[0]["ready_at_ms"], hit_at + 400);
    let ready_at = hit_at + tool.attack_delay_ms;
    assert_eq!(
        before["players"][sid.as_str()]["next_attack_ready_at_ms"],
        ready_at
    );

    let mut recovered = ZoneRuntime::restore_checkpoint(&bytes)
        .expect("strict root must accept the original D401 collision identity");
    let recovered_checkpoint = checkpoint(&recovered);
    assert_eq!(recovered_checkpoint["mining"], before["mining"]);
    assert_eq!(recovered_checkpoint["players"], before["players"]);
    assert!(!before["online_presence"].as_object().unwrap().is_empty());
    assert!(recovered_checkpoint["online_presence"]
        .as_object()
        .unwrap()
        .is_empty());
    assert_ne!(recovered_checkpoint["state_root"], before["state_root"]);
    assert_eq!(
        recovered_checkpoint["state_root"],
        recovered.canonical_state_root().unwrap()
    );
    // Cold recovery authenticates the original root, then revokes online
    // qualifications. Compare every other checkpoint field without deletion.
    let normalize_online = |mut value: Value| {
        value["online_presence"] = json!({});
        value["state_root"] = json!("online-authority-normalized");
        value
    };
    assert_eq!(
        normalize_online(recovered_checkpoint),
        normalize_online(before.clone())
    );
    for zone in [&uninterrupted, &recovered] {
        assert!(zone
            .prepare_mining_swing(&sid, MirDirection::Up, &tool, ready_at - 1)
            .is_none());
        assert!(zone
            .prepare_mining_swing(&sid, MirDirection::Up, &tool, ready_at)
            .is_some());
    }
    let map_effects = |out: &[ZoneOutbound]| {
        out.iter()
            .flat_map(|o| match o {
                ZoneOutbound::ToSession { packets, .. }
                | ZoneOutbound::ToMany { packets, .. }
                | ZoneOutbound::ToAll { packets } => packets.as_slice(),
                _ => &[],
            })
            .filter(|p| matches!(p, ServerPacket::MapEffect { effect: 12, .. }))
            .cloned()
            .collect::<Vec<_>>()
    };
    let before_effect = uninterrupted.tick(hit_at + 399);
    let restored_before_effect = recovered.tick(hit_at + 399);
    assert_eq!(restored_before_effect, before_effect);
    assert!(map_effects(&before_effect).is_empty());
    let effect = uninterrupted.tick(hit_at + 400);
    let restored_effect = recovered.tick(hit_at + 400);
    assert_eq!(restored_effect, effect);
    assert!(map_effects(&effect).iter().any(|p| matches!(p,
        ServerPacket::MapEffect { location, effect: 12, value: 0 } if *location == position)));
    assert_eq!(
        checkpoint(&recovered)["mining"],
        checkpoint(&uninterrupted)["mining"]
    );
    assert!(checkpoint(&recovered)["mining"]["effects"]
        .as_array()
        .unwrap()
        .is_empty());

    let original_swing = uninterrupted
        .prepare_mining_swing(&sid, MirDirection::Up, &tool, ready_at)
        .unwrap();
    let restored_swing = recovered
        .prepare_mining_swing(&sid, MirDirection::Up, &tool, ready_at)
        .unwrap();
    assert_eq!(restored_swing.ore(), original_swing.ore());
    assert_eq!(restored_swing.tool_damage(), original_swing.tool_damage());
    let expected = uninterrupted.commit_mining_swing(&original_swing).unwrap();
    let actual = recovered.commit_mining_swing(&restored_swing).unwrap();
    assert_eq!(actual, expected);
    assert_eq!(
        normalize_online(checkpoint(&recovered)),
        normalize_online(checkpoint(&uninterrupted))
    );
    assert_eq!(
        checkpoint(&recovered)["mining"]["spots"][&wall_key]["stones_left"],
        available - 2
    );

    for field in ["stones_left", "sequence", "effect", "cooldown"] {
        let mut altered = before.clone();
        match field {
            "stones_left" => altered["mining"]["spots"][&wall_key][field] = json!(available),
            "sequence" => altered["mining"][field] = json!(sequence + 2),
            "effect" => altered["mining"]["effects"][0]["ready_at_ms"] = json!(hit_at + 401),
            "cooldown" => {
                altered["players"][sid.as_str()]["next_attack_ready_at_ms"] = json!(ready_at + 1)
            }
            _ => unreachable!(),
        }
        let error = ZoneRuntime::restore_checkpoint(&serde_json::to_vec(&altered).unwrap())
            .err()
            .unwrap_or_else(|| panic!("strict mining restore accepted altered {field}"));
        assert!(error.contains("state root mismatch"), "{field}: {error}");
    }
}

#[test]
fn mining_three_classes_buy_equip_mine_exact_uid_purity_and_save_reload() {
    let _lock = ENVIRONMENT
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let _environment = MiningEnvironment::enter();
    for (class, label) in [
        (MirClass::Warrior, "Warrior"),
        (MirClass::Wizard, "Wizard"),
        (MirClass::Taoist, "Taoist"),
    ] {
        let mut f = purchased_fixture(class, label);
        refill(&mut f);
        let before_stones = spot(&f)["stones_left"].as_u64().unwrap();
        let before_dura = f.runtime.inner.shared_mining_tool().unwrap().durability;
        let swing = prepared(&mut f, true, true);
        let expected = swing.ore().unwrap().clone();
        let damage = swing.tool_damage();
        let out = commit(&mut f, &swing);
        let ore = out
            .iter()
            .find_map(|p| match p {
                ServerPacket::GainedItem { item } => Some(item.clone()),
                _ => None,
            })
            .unwrap();
        let template = mir2_game_data::crystal_item_by_name(&expected.item_name).unwrap();
        assert_eq!(ore.item_index, template.item_index);
        assert_eq!(ore.count, 1);
        assert_eq!(ore.current_dura, expected.current_dura);
        assert_eq!(ore.max_dura, 10_000);
        assert!(
            ore.current_dura > ore.max_dura,
            "source purity must not clamp to MaxDura"
        );
        assert_ne!(ore.unique_id, f.tool_uid);
        assert!(out.iter().any(
            |p| matches!(p,ServerPacket::DuraChanged{unique_id,current_dura}
            if *unique_id==f.tool_uid && *current_dura==before_dura-damage)
        ));
        assert_eq!(spot(&f)["stones_left"], before_stones - 1);
        let identity = f.runtime.inner.active_identity().unwrap();
        let saved = f.runtime.inner.active_character_checkpoint().unwrap();
        let shared = f.runtime.zone_state.clone();
        packet(&mut f.runtime, ClientPacket::LogOut);
        let persisted = fs::read(&f.path).unwrap();
        assert!(!persisted.is_empty());
        let mut resumed = shared_session_runtime(shared);
        resumed.inner = InProcessWorldRuntime::new(f.config.clone());
        packet(
            &mut resumed,
            ClientPacket::Login {
                account_id: identity.account_id.clone(),
                password: identity.account_id,
            },
        );
        let start = packet(
            &mut resumed,
            ClientPacket::StartGame {
                character_index: identity.character_index,
            },
        );
        assert!(start
            .iter()
            .any(|p| matches!(p, ServerPacket::StartGame { result: 4, .. })));
        assert_eq!(
            resumed.inner.shared_mining_tool().unwrap().unique_id,
            f.tool_uid
        );
        assert_eq!(
            resumed.inner.shared_mining_tool().unwrap().durability,
            before_dura - damage
        );
        let restored = resumed.inner.world_snapshot();
        let actual = restored
            .inventory_items
            .iter()
            .chain(&restored.belt_items)
            .find(|i| i.unique_id == ore.unique_id)
            .unwrap();
        assert_eq!(actual.durability_current, Some(expected.current_dura));
        assert_eq!(actual.durability_max, Some(10_000));
        assert_eq!(actual.name, expected.item_name);
        assert_eq!(
            resumed
                .inner
                .active_character_checkpoint()
                .unwrap()
                .position,
            saved.position
        );
        f.runtime = resumed;
        assert_eq!(
            spot(&f)["stones_left"],
            before_stones - 1,
            "normal logout and rejoin cannot refill shared wall"
        );
    }
}

#[test]
fn mining_ordinary_attack_ingress_rejects_immediate_spam_and_non_mining_spell() {
    let _lock = ENVIRONMENT
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let _environment = MiningEnvironment::enter();
    let mut f = purchased_fixture(MirClass::Warrior, "Ingress");
    let before = f.runtime.inner.active_character_checkpoint().unwrap();
    let out = packet(
        &mut f.runtime,
        ClientPacket::Attack {
            direction: MirDirection::Up,
            spell: Spell::None,
        },
    );
    assert!(
        out.iter()
            .any(|p| matches!(p,ServerPacket::ObjectAttack{info} if info.spell==Spell::None as u8)),
        "{out:?}"
    );
    assert!(!out.iter().any(|p| matches!(
        p,
        ServerPacket::GainedItem { .. } | ServerPacket::DuraChanged { .. }
    )));
    let spot_before = spot(&f);
    let sequence = zone_state(&f)["mining"]["sequence"].clone();
    let spam = packet(
        &mut f.runtime,
        ClientPacket::Attack {
            direction: MirDirection::Up,
            spell: Spell::None,
        },
    );
    assert!(!spam.iter().any(|p| matches!(
        p,
        ServerPacket::ObjectAttack { .. }
            | ServerPacket::GainedItem { .. }
            | ServerPacket::DuraChanged { .. }
    )));
    let rejected = packet(
        &mut f.runtime,
        ClientPacket::Attack {
            direction: MirDirection::Up,
            spell: Spell::Thrusting,
        },
    );
    assert!(!rejected.iter().any(|p| matches!(
        p,
        ServerPacket::GainedItem { .. } | ServerPacket::DuraChanged { .. }
    )));
    assert_eq!(spot(&f), spot_before);
    assert_eq!(zone_state(&f)["mining"]["sequence"], sequence);
    let after = f.runtime.inner.active_character_checkpoint().unwrap();
    assert_eq!(after.inventory_items_json, before.inventory_items_json);
    assert_eq!(after.equipment_items_json, before.equipment_items_json);
}

#[test]
fn mining_breaking_pick_refreshes_shared_ac_hp_before_next_attack() {
    let _lock = ENVIRONMENT
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let _environment = MiningEnvironment::enter();
    let mut f = purchased_fixture(MirClass::Warrior, "ToolBreak");
    let mut save = f.runtime.inner.active_character_checkpoint().unwrap();
    let weapon_index = save
        .equipment_items_json
        .iter()
        .position(|entry| serde_json::from_str::<Value>(entry).unwrap()["slot"] == "weapon")
        .unwrap();
    let mut weapon: Value = serde_json::from_str(&save.equipment_items_json[weapon_index]).unwrap();
    assert_eq!(weapon["slot"], "weapon");
    weapon["durability_current"] = json!(1);
    // Trusted branch fixture: high Accuracy guarantees an admitted hit; AC
    // and HP expose the required shared-stat refresh when the tool breaks.
    // The source hit/drop/cooldown formula itself is unchanged.
    weapon["added_stats"] = json!([
        {"stat":10,"value":10},{"stat":1,"value":100},{"stat":12,"value":100}
    ]);
    save.equipment_items_json[weapon_index] = weapon.to_string();
    f.runtime
        .inner
        .restore_active_character_checkpoint(&save)
        .unwrap();
    let sid = f.runtime.current_zone_session_id().unwrap();
    f.runtime
        .sync_authoritative_zone_combat_state(&sid)
        .unwrap();
    let before_stats = f.runtime.inner.zone_player_combat_stats();
    let before_max = f.runtime.inner.world_snapshot().player_max_hp.unwrap();
    assert!(before_stats.max_ac >= 100);
    f.next_time = SharedInProcessZoneSessionRuntime::zone_now_ms().saturating_sub(601_000);
    refill(&mut f);
    let out = packet(
        &mut f.runtime,
        ClientPacket::Attack {
            direction: MirDirection::Up,
            spell: Spell::None,
        },
    );
    assert!(
        out.iter().any(
            |p| matches!(p,ServerPacket::DuraChanged{unique_id,current_dura:0}
        if *unique_id==f.tool_uid)
        ),
        "{out:?}"
    );
    assert!(f.runtime.inner.shared_mining_tool().is_none());
    let after_stats = f.runtime.inner.zone_player_combat_stats();
    let snapshot = f.runtime.inner.world_snapshot();
    assert!(after_stats.max_ac < before_stats.max_ac);
    assert!(snapshot.player_max_hp.unwrap() < before_max);
    let state = zone_state(&f);
    let player = &state["players"][sid.as_str()];
    assert_eq!(
        player["combat_stats"],
        serde_json::to_value(after_stats).unwrap()
    );
    let shared = f.runtime.zone_state.lock().unwrap();
    let (hp, max_hp, mp) = shared.zone_manager.player_vitals(&sid).unwrap();
    assert_eq!(
        (hp, max_hp, mp),
        (
            snapshot.player_hp.unwrap(),
            snapshot.player_max_hp.unwrap(),
            snapshot.player_mp.unwrap()
        )
    );
}

fn fill_inventory(f: &mut Fixture, bag_slots: u16, belt_slots: u8, capacity: u16, weight: u16) {
    let info = mir2_game_data::crystal_item_by_name("BlackIronOre").unwrap();
    let make = |index: u16, container: &str, slot: u8| {
        json!({
            "key":format!("crystal-item-{}",info.item_index),"name":info.name,"icon":info.image,
            "slot":slot,"unique_id":400_000+u64::from(index),"container":container,"quantity":1,
            "description":"trusted occupied inventory fixture","durability_current":10_000,
            "durability_max":10_000,"weight":weight,"equip_slot":null,"attack":0,"defence":0,
            "heal_hp":0,"heal_mp":0,"user_item_metadata":{"item_index":info.item_index}
        })
        .to_string()
    };
    let mut save = f.runtime.inner.active_character_checkpoint().unwrap();
    save.inventory_capacity = capacity;
    save.inventory_items_json = (0..bag_slots)
        .map(|i| make(i, if i < 40 { "bag1" } else { "bag2" }, (i % 40) as u8))
        .collect();
    save.belt_items_json = (0..belt_slots)
        .map(|i| make(80 + u16::from(i), "belt", i))
        .collect();
    f.runtime
        .inner
        .restore_active_character_checkpoint(&save)
        .unwrap();
    let sid = f.runtime.current_zone_session_id().unwrap();
    f.runtime
        .sync_authoritative_zone_combat_state(&sid)
        .unwrap();
}

#[test]
fn mining_original_free_space_rule_prefers_bag2_then_belt_and_full_inventory_still_wears() {
    let _lock = ENVIRONMENT
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let _environment = MiningEnvironment::enter();
    for (label, bags, belt, capacity, expected) in [
        ("BagTwo", 40, 0, 86, Some(ItemContainer::Bag2)),
        ("Belt", 40, 0, 46, Some(ItemContainer::Belt)),
        ("Full", 40, 6, 46, None),
        ("Heavy", 35, 0, 46, Some(ItemContainer::Bag1)),
    ] {
        let mut f = purchased_fixture(MirClass::Taoist, label);
        refill(&mut f);
        fill_inventory(&mut f, bags, belt, capacity, 4);
        let before = f.runtime.inner.world_snapshot();
        let stones = spot(&f)["stones_left"].as_u64().unwrap();
        if label == "Heavy" {
            assert!(
                before.current_weight > before.max_weight,
                "fixture must actually exceed bag weight"
            );
        }
        let dura = f.runtime.inner.shared_mining_tool().unwrap().durability;
        let swing = prepared(&mut f, true, false);
        let damage = swing.tool_damage();
        let out = commit(&mut f, &swing);
        let gained = out.iter().find_map(|p| match p {
            ServerPacket::GainedItem { item } => Some(item),
            _ => None,
        });
        if let Some(expected) = expected {
            let gained = gained.expect("ordinary free space admits source ore");
            let snapshot = f.runtime.inner.world_snapshot();
            let actual = snapshot
                .inventory_items
                .iter()
                .chain(&snapshot.belt_items)
                .find(|i| i.unique_id == gained.unique_id)
                .unwrap();
            assert_eq!(actual.container, expected, "{label}");
            if label == "BagTwo" {
                assert_eq!(actual.slot, 0);
            }
            if label == "Belt" {
                assert_eq!(actual.slot, 0);
            }
        } else {
            assert!(gained.is_none());
            let after = f.runtime.inner.world_snapshot();
            assert_eq!(after.inventory_items, before.inventory_items);
            assert_eq!(after.belt_items, before.belt_items);
        }
        assert_eq!(
            f.runtime.inner.shared_mining_tool().unwrap().durability,
            dura - damage
        );
        assert_eq!(spot(&f)["stones_left"], stones - 1);
    }
}

#[test]
fn mining_save_failure_rolls_back_exact_item_uid_tool_and_shared_stone() {
    let _lock = ENVIRONMENT
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let _environment = MiningEnvironment::enter();
    let mut f = purchased_fixture(MirClass::Wizard, "FailedSave");
    refill(&mut f);
    let swing = prepared(&mut f, true, false);
    let before = f.runtime.inner.active_character_checkpoint().unwrap();
    let disk = fs::read(&f.path).unwrap();
    let shared = zone_state(&f);
    f.config.inject_account_store_transaction_fault(
        mir2_simulation::AccountStoreTransactionFault::BeforePersist,
    );
    assert!(f.runtime.inner.apply_shared_mining_swing(&swing).is_err());
    let after = f.runtime.inner.active_character_checkpoint().unwrap();
    assert_eq!(after.inventory_items_json, before.inventory_items_json);
    assert_eq!(after.belt_items_json, before.belt_items_json);
    assert_eq!(after.equipment_items_json, before.equipment_items_json);
    assert_eq!(fs::read(&f.path).unwrap(), disk);
    assert_eq!(zone_state(&f), shared);
    let out = commit(&mut f, &swing);
    assert_eq!(
        out.iter()
            .filter(|p| matches!(p, ServerPacket::GainedItem { .. }))
            .count(),
        1,
        "retry must create one carrier after definitive rollback"
    );
}

#[test]
fn mining_uncertain_file_publication_retains_one_durable_award_and_freezes_retries() {
    let _lock = ENVIRONMENT
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let _environment = MiningEnvironment::enter();
    let mut f = purchased_fixture(MirClass::Wizard, "UnknownSave");
    refill(&mut f);
    let swing = prepared(&mut f, true, false);
    let before = f.runtime.inner.active_character_checkpoint().unwrap();
    let shared = zone_state(&f);
    let expected = swing.ore().unwrap().clone();
    f.config.inject_account_store_transaction_fault(
        mir2_simulation::AccountStoreTransactionFault::AfterFileRenameBeforeDirectorySync,
    );
    assert!(f.runtime.inner.apply_shared_mining_swing(&swing).is_err());
    let after = f.runtime.inner.active_character_checkpoint().unwrap();
    assert_eq!(after.inventory_items_json, before.inventory_items_json);
    assert_eq!(after.equipment_items_json, before.equipment_items_json);
    assert_eq!(zone_state(&f), shared);
    let disk = fs::read(&f.path).unwrap();
    let store: mir2_simulation::AccountStore = serde_json::from_slice(&disk).unwrap();
    let identity = f.runtime.inner.active_identity().unwrap();
    let durable = &store.accounts[&identity.account_id].saves[&identity.character_index];
    let ores = durable
        .inventory_items_json
        .iter()
        .chain(&durable.belt_items_json)
        .map(|entry| serde_json::from_str::<Value>(entry).unwrap())
        .filter(|item| item["name"] == expected.item_name)
        .collect::<Vec<_>>();
    assert_eq!(
        ores.len(),
        1,
        "unknown publication must retain its exact first durable award"
    );
    assert_eq!(ores[0]["durability_current"], expected.current_dura);
    let weapon = durable
        .equipment_items_json
        .iter()
        .map(|entry| serde_json::from_str::<Value>(entry).unwrap())
        .find(|item| item["slot"] == "weapon")
        .unwrap();
    assert_eq!(weapon["durability_current"], 10_000 - swing.tool_damage());
    assert!(
        f.config.save_account_store().is_err(),
        "publication uncertainty fences all repository writes"
    );
    assert!(
        f.runtime.inner.apply_shared_mining_swing(&swing).is_err(),
        "same capability cannot publish a duplicate"
    );
    assert_eq!(fs::read(&f.path).unwrap(), disk);
    assert_eq!(zone_state(&f), shared);
}

#[test]
fn mining_source_ore_sells_at_purity_price_and_unloaded_pick_repairs_normally() {
    let _lock = ENVIRONMENT
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let _environment = MiningEnvironment::enter();
    let mut f = purchased_fixture(MirClass::Warrior, "SellRepair");
    refill(&mut f);
    let swing = prepared(&mut f, true, false);
    let out = commit(&mut f, &swing);
    let ore = out
        .iter()
        .find_map(|p| match p {
            ServerPacket::GainedItem { item } => Some(item.clone()),
            _ => None,
        })
        .unwrap();
    let template = mir2_game_data::crystal_item_by_index(ore.item_index).unwrap();
    // This is the original Price()/2 floating-point/truncation formula.
    let max_value = (f32::from(ore.max_dura)
        * ((template.price as f32 / 2.0) / f32::from(template.durability)))
    .trunc();
    let expected = (max_value / 2.0
        + (max_value / 2.0) * (f32::from(ore.current_dura) / f32::from(ore.max_dura))
        + template.price as f32 / 2.0)
        .floor() as u32
        / 2;
    let before_dura = f.runtime.inner.shared_mining_tool().unwrap().durability;
    prepare_transform(
        &mut f.runtime,
        "0",
        "BichonProvince",
        Point { x: 296, y: 612 },
        12,
        None,
    );
    call_smith(&mut f.runtime, "@main");
    call_smith(&mut f.runtime, "@BuySell");
    let gold = f.runtime.inner.world_snapshot().gold;
    let sell = packet(
        &mut f.runtime,
        ClientPacket::SellItem {
            unique_id: ore.unique_id,
            count: 1,
        },
    );
    assert!(
        sell.iter().any(
            |p| matches!(p,ServerPacket::SellItem{unique_id,count:1,success:true}
        if *unique_id==ore.unique_id)
        ),
        "{sell:?}"
    );
    assert_eq!(f.runtime.inner.world_snapshot().gold, gold + expected);
    let free = f
        .runtime
        .inner
        .world_snapshot()
        .inventory_items
        .iter()
        .map(|i| i.slot)
        .collect::<BTreeSet<_>>();
    let dest = (0..40).find(|slot| !free.contains(slot)).unwrap();
    let removed = packet(
        &mut f.runtime,
        ClientPacket::RemoveItem {
            grid: MirGridType::Inventory,
            unique_id: f.tool_uid,
            to: i32::from(dest),
        },
    );
    assert!(
        removed
            .iter()
            .any(|p| matches!(p, ServerPacket::RemoveItem { success: true, .. })),
        "{removed:?}"
    );
    assert!(f.runtime.inner.shared_mining_tool().is_none());
    call_smith(&mut f.runtime, "@main");
    call_smith(&mut f.runtime, "@Repair");
    let repaired = packet(
        &mut f.runtime,
        ClientPacket::RepairItem {
            unique_id: f.tool_uid,
        },
    );
    let expected_max = 10_000 - (10_000 - before_dura) / 30;
    assert!(
        repaired.iter().any(
            |p| matches!(p,ServerPacket::ItemRepaired{unique_id,max_dura,current_dura}
        if *unique_id==f.tool_uid && *max_dura==expected_max && *current_dura==expected_max)
        ),
        "{repaired:?}"
    );
    assert!(repaired
        .iter()
        .any(|p| matches!(p, ServerPacket::LoseGold { .. })));
    let tool = f
        .runtime
        .inner
        .world_snapshot()
        .inventory_items
        .into_iter()
        .find(|i| i.unique_id == f.tool_uid)
        .unwrap();
    assert_eq!(tool.durability_current, Some(expected_max));
    assert_eq!(tool.durability_max, Some(expected_max));
}
