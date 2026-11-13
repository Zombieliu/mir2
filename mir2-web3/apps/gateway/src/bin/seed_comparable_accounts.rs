//! Offline, fresh-only File fixture for game-core-local. Never a Gateway route.
//! Ordinary creation/password hashing, complete checkpoint validation, UID history
//! floors, File writer fencing and normal logout/save CAS remain in use.
use base64::Engine as _;
use mir2_protocol::{ClientPacket, MirClass, MirDirection, MirGender, Point, ServerPacket};
use mir2_simulation::{
    validate_commercial_identity_credentials, AccountStore, CharacterBindPoint,
    CharacterSaveRecord, ItemContainer, SimulationConfig, SimulationSession, Stage5SystemsState,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    env,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

const MAP_SHA: &str = "d34167876db45bcac3d879e3ca1e8909efc38fffedbfc1511cef4c4215c2f608";
const COMBAT_CONTRACT_SHA: &str =
    "8b373b5d03a71c468934d3591c6d06ad65ce6fe2eeb3208acca39ee0ae25a25b";
type Result<T> = std::result::Result<T, &'static str>;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Manifest {
    schema: String,
    qualification_layer: String,
    plan_sha256: String,
    navigation_sha256: String,
    map_sha256: String,
    actors: Vec<Actor>,
}
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Actor {
    ordinal: usize,
    account_id: String,
    password: String,
    name: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Offline {
    schema: String,
    gateway_state: String,
    gateway_pid: Option<u32>,
    actual_exit_code: Option<i32>,
    live_marker: PathBuf,
}

fn main() {
    std::panic::set_hook(Box::new(|_| {
        eprintln!("comparable fixture failed; private details suppressed")
    }));
    if let Err(error) = run() {
        eprintln!("{}", json!({"passed":false,"error":error}));
        std::process::exit(1);
    }
}
fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
fn read(path: &Path, maximum: u64) -> Result<Vec<u8>> {
    let meta = fs::symlink_metadata(path).map_err(|_| "required input unavailable")?;
    if !meta.is_file() || meta.file_type().is_symlink() || meta.len() > maximum {
        return Err("input must be bounded regular file");
    }
    fs::read(path).map_err(|_| "input read failed")
}
fn fresh_private_path(path: &Path) -> Result<PathBuf> {
    if !path.is_absolute() || path.exists() {
        return Err("fresh absolute output required");
    }
    let parent = path
        .parent()
        .ok_or("output parent required")?
        .canonicalize()
        .map_err(|_| "private parent missing")?;
    if parent.file_name().and_then(|x| x.to_str()) != Some("private") {
        return Err("dedicated private parent required");
    }
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()
        .map_err(|_| "repository boundary unavailable")?;
    if parent.starts_with(repo) {
        return Err("output must be outside repository");
    }
    #[cfg(windows)]
    {
        use std::path::{Component, Prefix};
        let on_c = matches!(parent.components().next(), Some(Component::Prefix(prefix))
            if matches!(prefix.kind(), Prefix::Disk(drive) | Prefix::VerbatimDisk(drive) if drive.to_ascii_uppercase() == b'C'));
        if !on_c {
            return Err("comparison artifacts must be on C drive");
        }
    }
    let mut cursor = Some(parent.as_path());
    while let Some(p) = cursor {
        let m = fs::symlink_metadata(p).map_err(|_| "parent metadata unavailable")?;
        if m.file_type().is_symlink() {
            return Err("linked parent rejected");
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            if m.file_attributes() & 0x400 != 0 {
                return Err("reparse parent rejected");
            }
        }
        cursor = p.parent();
    }
    Ok(parent.join(path.file_name().ok_or("output filename missing")?))
}
fn write_new(path: &Path, value: &Value) -> Result<()> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .map_err(|_| "exclusive output creation failed")?;
    file.write_all(
        serde_json::to_string_pretty(value)
            .map_err(|_| "serialization failed")?
            .as_bytes(),
    )
    .map_err(|_| "output write failed")?;
    file.sync_all().map_err(|_| "output sync failed")
}
fn run() -> Result<()> {
    let args: Vec<_> = env::args().skip(1).collect();
    if args == ["--help"] {
        println!("seed_comparable_accounts --manifest PRIVATE_JSON --plan PLAN_JSON --navigation NAV_JSON --account-store FRESH_PRIVATE_ACCOUNTS_JSON --client-root ISOLATED_CLIENT_ROOT --offline-receipt OFFLINE_JSON [--combat-contract PINNED_JSON (required for melee)]\nOnly fresh C-only game-core-local File fixtures. No socket, Gateway startup, public auth or existing store modification.");
        return Ok(());
    }
    if ![12, 14].contains(&args.len()) {
        return Err("six paths plus explicit combat contract for melee; credentials belong only in private JSON");
    }
    let mut options = std::collections::BTreeMap::new();
    for pair in args.chunks_exact(2) {
        if ![
            "--manifest",
            "--plan",
            "--navigation",
            "--account-store",
            "--client-root",
            "--offline-receipt",
            "--combat-contract",
        ]
        .contains(&pair[0].as_str())
            || options
                .insert(pair[0].as_str(), PathBuf::from(&pair[1]))
                .is_some()
        {
            return Err("unknown/duplicate option");
        }
    }
    let get = |name| options.get(name).ok_or("required option missing");
    let output = fresh_private_path(get("--account-store")?)?;
    let ready = fresh_private_path(&output.parent().unwrap().join("actors.ready.json"))?;
    let receipt = fresh_private_path(&output.parent().unwrap().join("seed-receipt.json"))?;
    let private = output.parent().unwrap();
    if get("--manifest")?
        .parent()
        .and_then(|p| p.canonicalize().ok())
        .as_deref()
        != Some(private)
        || get("--manifest")?.file_name().and_then(|x| x.to_str()) != Some("actors.json")
    {
        return Err("manifest must belong to this new private fixture");
    }
    let offline: Offline = serde_json::from_slice(&read(get("--offline-receipt")?, 16384)?)
        .map_err(|_| "invalid offline receipt")?;
    if offline.schema != "mir2.comparable.modern-offline.v1"
        || offline.live_marker.file_name().and_then(|p| p.to_str()) != Some("modern-live.json")
        || offline
            .live_marker
            .parent()
            .and_then(|p| p.canonicalize().ok())
            .as_deref()
            != private.parent()
        || offline.live_marker.exists()
    {
        return Err("owned Gateway must be offline before fixture creation");
    }
    match offline.gateway_state.as_str() {
        "never-started" if offline.gateway_pid.is_none() && offline.actual_exit_code.is_none() => {}
        "stopped" if offline.gateway_pid.is_some() && offline.actual_exit_code == Some(0) => {
            let system = sysinfo::System::new_all();
            if system
                .process(sysinfo::Pid::from_u32(offline.gateway_pid.unwrap()))
                .is_some()
            {
                return Err("recorded owned Gateway PID still exists");
            }
        }
        _ => return Err("unproven Gateway stop state"),
    }
    let manifest: Manifest = serde_json::from_slice(&read(get("--manifest")?, 128 * 1024)?)
        .map_err(|_| "invalid private manifest")?;
    let plan_bytes = read(get("--plan")?, 8 * 1024 * 1024)?;
    let plan: Value = serde_json::from_slice(&plan_bytes).map_err(|_| "invalid plan")?;
    let melee = match plan["scenario"].as_str() {
        Some("dense-movement-chat") if !options.contains_key("--combat-contract") => false,
        Some("dispersed-melee") => {
            let path = get("--combat-contract")?;
            if !path.is_absolute() {
                return Err("absolute external combat contract required");
            }
            let bytes = read(path, 16 * 1024)?;
            let contract: Value =
                serde_json::from_slice(&bytes).map_err(|_| "invalid combat contract")?;
            if digest(&bytes) != COMBAT_CONTRACT_SHA
                || plan["combatContractSha256"] != COMBAT_CONTRACT_SHA
                || plan["combatContract"] != contract
            {
                return Err("external/embedded combat contract mismatch");
            }
            true
        }
        _ => return Err("unsupported scenario or unexpected combat contract"),
    };
    let nav_bytes = read(get("--navigation")?, 2 * 1024 * 1024)?;
    let nav: Value = serde_json::from_slice(&nav_bytes).map_err(|_| "invalid navigation")?;
    let map_path = get("--client-root")?.join("Map/DogYoHyun.map");
    if manifest.schema != "mir2.comparable.modern-actors.v1"
        || manifest.qualification_layer != "game-core-local"
        || manifest.plan_sha256 != digest(&plan_bytes)
        || manifest.navigation_sha256 != digest(&nav_bytes)
        || manifest.map_sha256 != MAP_SHA
        || digest(&read(&map_path, 16 * 1024 * 1024)?) != MAP_SHA
        || plan["schema"] != "mir2.comparable.plan.v1"
        || plan["qualificationLayer"] != "game-core-local"
        || plan["navigationSha256"] != manifest.navigation_sha256
        || plan["map"]["rawSha256"] != MAP_SHA
        || plan["map"]["index"] != 440
        || plan["actorTemplate"]["class"] != "Warrior"
        || plan["actorTemplate"]["gender"] != "Male"
        || plan["actorTemplate"]["level"] != 40
        || nav["schema"] != "mir2.comparable.navigation.v1"
        || nav["map"]["rawSha256"] != MAP_SHA
        || nav["map"]["index"] != 440
        || nav["source"] != "original-library-cell-valid-and-movement-exclusion"
        || nav["gameCodeUnmodified"] != true
        || nav["map"] != plan["map"]
        || nav["encoding"] != "row-major-lsb0-bitset"
        || nav["exclusions"]["movementRadius"] != 3
        || nav["exclusions"]["npcRadius"] != 3
        || nav["exclusions"]["allDoorCells"] != true
        || nav["exclusions"]["safeZones"] != true
    {
        return Err("fixture/source/map/navigation pin mismatch");
    }
    let encoded = nav["eligibleBitsBase64"]
        .as_str()
        .ok_or("navigation bitset absent")?;
    let bits = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|_| "invalid navigation base64")?;
    if bits.len() != 80_000
        || base64::engine::general_purpose::STANDARD.encode(&bits) != encoded
        || bits.iter().map(|b| u64::from(b.count_ones())).sum::<u64>() != 404_451
        || nav["eligibleCount"] != 404_451
    {
        return Err("navigation bitset size/count mismatch");
    }
    let positions = plan["actors"].as_array().ok_or("plan actors absent")?;
    if ![2, 25, 50, 100].contains(&manifest.actors.len())
        || manifest.actors.len() != positions.len()
    {
        return Err("cohort count mismatch");
    }
    let mut ids = BTreeSet::new();
    let mut names = BTreeSet::new();
    let mut cells = BTreeSet::new();
    for (i, actor) in manifest.actors.iter().enumerate() {
        if actor.ordinal != i
            || positions[i]["ordinal"] != i
            || !ids.insert(actor.account_id.to_ascii_lowercase())
            || !names.insert(actor.name.to_ascii_lowercase())
            || actor.account_id.starts_with("sui:")
            || !actor.name.bytes().all(|b| b.is_ascii_alphanumeric())
            || !(3..=16).contains(&actor.name.len())
            || validate_commercial_identity_credentials(&actor.account_id, &actor.password).is_err()
        {
            return Err("invalid dedicated ordinary identity");
        }
        validate_lane(&positions[i], melee, &mut cells, &bits)?;
    }
    // This is a new process. Never inherit another realm's storage, GM, RPC,
    // wallet, procurement, or unsafe-development settings into fixture work.
    for (key, _) in env::vars_os()
        .filter(|(k, _)| {
            let k = k.to_string_lossy();
            k.starts_with("MIR2_") || k.starts_with("CRYSTAL_")
        })
        .collect::<Vec<_>>()
    {
        env::remove_var(key);
    }
    env::set_var("MIR2_RUNTIME_ENV", "development");
    env::set_var("MIR2_IDENTITY_POLICY", "commercial");
    env::set_var("MIR2_ACCOUNT_STORE_BACKEND", "file");
    env::set_var("CRYSTAL_CLIENT_ROOT", get("--client-root")?);
    let config = base_config();
    config
        .account_store
        .lock()
        .map_err(|_| "store lock failed")?
        .accounts
        .clear();
    let mut prepared = Vec::new();
    for (i, actor) in manifest.actors.iter().enumerate() {
        let point = Point {
            x: positions[i]["start"]["x"]
                .as_i64()
                .ok_or("start x missing")? as i32,
            y: positions[i]["start"]["y"]
                .as_i64()
                .ok_or("start y missing")? as i32,
        };
        let index = prepare_actor(&config, actor, point, Some(private), melee)?;
        prepared.push(json!({"ordinal":i,"accountId":actor.account_id,"password":actor.password,"name":actor.name,"characterIndex":index}));
    }
    let store: AccountStore = config
        .account_store
        .lock()
        .map_err(|_| "store lock failed")?
        .clone();
    if store.accounts.len() != manifest.actors.len()
        || store.accounts.contains_key("demo")
        || store.accounts.values().any(|a| a.gm_level != 0)
    {
        return Err("prepared cohort mismatch");
    }
    // Acquire the original File authority before publication. Its normal lock,
    // writer binding, publication and UID-census behavior are not bypassed.
    {
        let durable = base_config().with_account_store_path(&output);
        *durable
            .account_store
            .lock()
            .map_err(|_| "File authority lock failed")? = store;
        durable
            .save_account_store()
            .map_err(|_| "File authority publication failed")?;
    }
    let durable = base_config().with_account_store_path(&output);
    let mut proofs = Vec::new();
    for (i, actor) in manifest.actors.iter().enumerate() {
        let index = prepared[i]["characterIndex"].as_i64().unwrap() as i32;
        let mut session = SimulationSession::new(durable.clone());
        packet(
            &mut session,
            ClientPacket::Login {
                account_id: actor.account_id.clone(),
                password: actor.password.clone(),
            },
            |p| matches!(p, ServerPacket::LoginSuccess { .. }),
        )?;
        packet(
            &mut session,
            ClientPacket::StartGame {
                character_index: index,
            },
            |p| matches!(p, ServerPacket::StartGame { result: 4, .. }),
        )?;
        let snapshot = session.world_snapshot();
        let checkpoint = session
            .active_character_checkpoint()
            .ok_or("fresh checkpoint absent")?;
        if melee {
            validate_melee_checkpoint(&checkpoint)?;
            if !snapshot.known_skills.is_empty()
                || !snapshot.active_buffs.is_empty()
                || snapshot.hero_vitals.is_some()
                || snapshot.stage5_systems.hero.is_some()
                || !snapshot.stage5_systems.intelligent_creatures.is_empty()
            {
                return Err("fresh combat baseline differs; no runtime clearing permitted");
            }
        }
        if checkpoint.character.level != 40
            || checkpoint.map_file_name != "DogYoHyun"
            || checkpoint.position.x != positions[i]["start"]["x"].as_i64().unwrap() as i32
            || checkpoint.position.y != positions[i]["start"]["y"].as_i64().unwrap() as i32
            || checkpoint.equipment_items_json.len() != 4
            || snapshot.equipment_items.len() != 4
            || checkpoint.gold != 0
            || checkpoint.inventory_capacity != 46
        {
            return Err("fresh File ordinary bootstrap mismatch");
        }
        proofs.push(json!({"ordinal":i,"characterIndex":index,"level":40,"hp":checkpoint.hp,"mp":checkpoint.mp,"gold":checkpoint.gold,
            "inventoryCapacity":checkpoint.inventory_capacity,"equipmentCount":snapshot.equipment_items.len(),"checkpointSha256":digest(&serde_json::to_vec(&checkpoint).map_err(|_|"checkpoint encode failed")?)}));
        packet(&mut session, ClientPacket::LogOut, |p| {
            matches!(p, ServerPacket::LogOutSuccess { .. })
        })?;
    }
    drop(durable);
    let ready_value = json!({"schema":"mir2.comparable.modern-cohort.v1","qualificationLayer":"game-core-local","planSha256":manifest.plan_sha256,
        "navigationSha256":manifest.navigation_sha256,"mapSha256":MAP_SHA,"accountStore":get("--account-store")?,"actors":prepared});
    write_new(&ready, &ready_value)?;
    let public = json!({"schema":"mir2.comparable.modern-seed-receipt.v1","passed":true,"qualificationLayer":"game-core-local",
        "backend":"file","sessionCache":"in-memory","clientAuthentication":"ordinary-password","networkRequests":0,
        "preseeded":true,"progressionEvidence":false,"count":manifest.actors.len(),"planSha256":manifest.plan_sha256,
        "scenario":plan["scenario"],"combatContractSha256":if melee {Some(COMBAT_CONTRACT_SHA)}else{None},
        "mapSha256":MAP_SHA,"navigationSha256":manifest.navigation_sha256,"accountStoreSha256":digest(&read(&output,64*1024*1024)?),
        "actorsReadySha256":digest(&read(&ready,128*1024)?),"completeCheckpointValidatorUsed":true,"normalLogoutCasUsed":true,"freshFileLoginStartGameVerified":true,"proofs":proofs});
    write_new(&receipt, &public)?;
    println!("{}", public);
    Ok(())
}
fn validate_lane(
    actor: &Value,
    melee: bool,
    cells: &mut BTreeSet<(usize, usize)>,
    bits: &[u8],
) -> Result<()> {
    let lane = actor["lane"]
        .as_array()
        .filter(|a| a.len() == 2)
        .ok_or("two-cell lane declaration required")?;
    if actor["start"] != lane[0]
        || if melee {
            lane[1] != lane[0]
        } else {
            lane[1]["x"].as_i64() != lane[0]["x"].as_i64().map(|x| x + 1)
                || lane[1]["y"] != lane[0]["y"]
        }
    {
        return Err("invalid fixed lane for scenario");
    }
    for cell in lane.iter().take(if melee { 1 } else { 2 }) {
        let x = cell["x"]
            .as_u64()
            .filter(|x| *x < 800)
            .ok_or("invalid lane x")? as usize;
        let y = cell["y"]
            .as_u64()
            .filter(|y| *y < 800)
            .ok_or("invalid lane y")? as usize;
        let index = y * 800 + x;
        if !cells.insert((x, y))
            || !bits
                .get(index >> 3)
                .is_some_and(|byte| byte & (1 << (index & 7)) != 0)
        {
            return Err("overlapping or nonwalkable lane");
        }
    }
    Ok(())
}
fn validate_melee_checkpoint(save: &CharacterSaveRecord) -> Result<()> {
    let systems: Stage5SystemsState = serde_json::from_str(
        save.stage5_systems_json
            .as_deref()
            .ok_or("complete combat systems absent")?,
    )
    .map_err(|_| "malformed combat systems")?;
    if !save.skill_states_json.is_empty()
        || !save.buff_states_json.is_empty()
        || !save.saved_pets.is_empty()
        || save.hero_vitals.is_some()
        || save.hero_registry_attachment.is_some()
        || !save.hero_inventory_items_json.is_empty()
        || !save.hero_equipment_items_json.is_empty()
        || systems.hero.is_some()
        || !systems.hero_learned_magics.is_empty()
        || !systems.intelligent_creatures.is_empty()
        || systems.summoned_intelligent_creature_type != Some(99)
        || save.hp != 674
        || save.max_hp != 674
        || save.mp != 151
    {
        return Err("combat checkpoint must be fresh full-HP with no skills, buffs, hero or pets");
    }
    // These are the declared common items; never replace gear to make a run pass.
    for index in [221, 317, 482] {
        let template = mir2_game_data::crystal_item_by_index(index)
            .ok_or("common combat item template absent")?;
        if template.item_set != 0
            || template.unique != 0
            || template.stats.iter().any(|s| {
                (matches!(s.stat, 12 | 19 | 32 | 46 | 48 | 125 | 126) && s.value != 0)
                    || (s.stat == 14 && s.value < 0)
            })
        {
            return Err("common combat gear has unsupported vitality or combat effects");
        }
    }
    Ok(())
}
fn base_config() -> SimulationConfig {
    let mut c = SimulationConfig::default().with_crystal_world_runtime();
    c.map.map_index = 440;
    c.map.file_name = "DogYoHyun".into();
    c.map.title = "DogYoHyun".into();
    c.map.mini_map = 451;
    c.map.big_map = 451;
    c.spawn = Point { x: 100, y: 100 };
    c
}
fn packet<F: Fn(&ServerPacket) -> bool>(
    s: &mut SimulationSession,
    p: ClientPacket,
    accept: F,
) -> Result<()> {
    let replies = s
        .try_handle_packet(p)
        .map_err(|_| "ordinary fixture command rejected")?;
    if !replies.iter().any(accept) {
        return Err("ordinary fixture command missing successful ACK");
    }
    Ok(())
}
fn prepare_actor(
    config: &SimulationConfig,
    a: &Actor,
    start: Point,
    diagnostic_directory: Option<&Path>,
    melee: bool,
) -> Result<i32> {
    let mut personal = config.clone();
    personal.spawn = start.clone();
    let mut s = SimulationSession::new(personal);
    packet(
        &mut s,
        ClientPacket::NewAccount {
            account_id: a.account_id.clone(),
            password: a.password.clone(),
            birth_date_binary: 0,
            user_name: String::new(),
            secret_question: String::new(),
            secret_answer: String::new(),
            email_address: String::new(),
        },
        |p| matches!(p, ServerPacket::NewAccount { result: 8 }),
    )?;
    packet(
        &mut s,
        ClientPacket::Login {
            account_id: a.account_id.clone(),
            password: a.password.clone(),
        },
        |p| matches!(p, ServerPacket::LoginSuccess { .. }),
    )?;
    let replies = s
        .try_handle_packet(ClientPacket::NewCharacter {
            name: a.name.clone(),
            class: MirClass::Warrior,
            gender: MirGender::Male,
        })
        .map_err(|_| "ordinary character creation rejected")?;
    let index = replies
        .iter()
        .find_map(|p| {
            if let ServerPacket::NewCharacterSuccess { char_info } = p {
                Some(char_info.index)
            } else {
                None
            }
        })
        .ok_or("new character ACK missing")?;
    packet(
        &mut s,
        ClientPacket::StartGame {
            character_index: index,
        },
        |p| matches!(p, ServerPacket::StartGame { result: 4, .. }),
    )?;
    let mut save = s
        .active_character_checkpoint()
        .ok_or("base complete checkpoint unavailable")?;
    save.character.level = 40;
    save.map_file_name = "DogYoHyun".into();
    save.map_title = "DogYoHyun".into();
    save.position = start.clone();
    save.direction = MirDirection::Down;
    save.bind_point = Some(CharacterBindPoint {
        map_file_name: "DogYoHyun".into(),
        position: start,
    });
    save.hp = 674;
    save.max_hp = 674;
    save.mp = 151;
    save.max_mp = 151;
    save.experience = 0;
    save.max_experience = 12_000_000;
    save.gold = 0;
    save.inventory_capacity = 46;
    save.inventory_items_json.clear();
    save.belt_items_json.clear();
    save.storage_items_json.clear();
    save.equipment_items_json.clear();
    let first = 1_000_000_u64 + (a.ordinal as u64) * 1024;
    for (offset, index, slot) in [
        (1, 221, "weapon"),
        (2, 317, "armour"),
        (3, 482, "braceletLeft"),
        (4, 482, "braceletRight"),
    ] {
        save.equipment_items_json
            .push(equipment(index, slot, first + offset)?);
    }
    for slot in 0..9 {
        save.inventory_items_json
            .push(potion(slot, first + 5 + u64::from(slot))?);
    }
    if melee {
        validate_melee_checkpoint(&save)?;
    }
    // Public server-only restore invokes the original complete validator and
    // raises the issuer history floor; ordinary LogOut uses the original CAS.
    if let Err(error) = s.restore_active_character_checkpoint(&save) {
        if let Some(directory) = diagnostic_directory {
            let stamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|_| "diagnostic clock failed")?
                .as_nanos();
            let name = format!("checkpoint-rejection-{}-{stamp}.json", a.ordinal);
            write_new(
                &directory.join(&name),
                &json!({"schema":"mir2.comparable.private-seed-diagnostic.v1","actorOrdinal":a.ordinal,
                "stage":"completeCheckpointRestore","reason":error.chars().take(2048).collect::<String>(),
                "inventoryCount":save.inventory_items_json.len(),"equipmentCount":save.equipment_items_json.len(),"beltCount":save.belt_items_json.len()}),
            )?;
            eprintln!(
                "{}",
                json!({"event":"privateSeedDiagnostic","actorOrdinal":a.ordinal,"fileName":name})
            );
        }
        return Err("complete fixture checkpoint rejected");
    }
    packet(&mut s, ClientPacket::LogOut, |p| {
        matches!(p, ServerPacket::LogOutSuccess { .. })
    })?;
    Ok(index)
}
fn equipment(index: i32, slot: &str, uid: u64) -> Result<String> {
    let t = mir2_game_data::crystal_item_by_index(index)
        .ok_or("ordinary equipment template missing")?;
    let max_stat = |id| {
        t.stats
            .iter()
            .find(|s| s.stat == id)
            .map(|s| s.value)
            .unwrap_or(0)
    };
    serde_json::to_string(&json!({"key":format!("crystal-item-{index}"),"slot":slot,"quantity":1,"name":t.name,"icon":t.image,"shape":t.shape,
        "description":t.tooltip.unwrap_or_default(),"durability_current":t.durability,"durability_max":t.durability,
        "grade":if t.grade==1{"common"}else{"rare"},"attack":max_stat(5),"defence":max_stat(1),"identified":true,
        "user_item_unique_id":uid,"user_item_metadata":{"item_index":index,"wedding_ring":-1,"socket_layout_hydrated":true,"captured_socket_positions":[]}})).map_err(|_|"equipment encoding failed")
}
fn potion(slot: u8, uid: u64) -> Result<String> {
    let t = mir2_game_data::crystal_item_by_index(662).ok_or("ordinary potion template missing")?;
    serde_json::to_string(&json!({"key":"crystal-item-662","name":t.name,"icon":t.image,"slot":slot,"unique_id":uid,"container":ItemContainer::Bag1,"quantity":20,
        "description":t.tooltip.unwrap_or_default(),"durability_current":null,"durability_max":null,"weight":t.weight,"equip_slot":null,"grade":"none",
        "attack":0,"defence":0,"heal_hp":110,"heal_mp":0,"identified":true,"user_item_metadata":{"item_index":662,"wedding_ring":-1,"socket_layout_hydrated":true,"captured_socket_positions":[]}})).map_err(|_|"potion encoding failed")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ordinary_template_carriers_are_complete_and_unique() {
        let a: Value = serde_json::from_str(&equipment(221, "weapon", 1_000_001).unwrap()).unwrap();
        let b: Value =
            serde_json::from_str(&equipment(482, "braceletLeft", 1_000_002).unwrap()).unwrap();
        assert_eq!(a["user_item_metadata"]["item_index"], 221);
        assert_eq!(a["durability_current"], 4000);
        assert_ne!(a["user_item_unique_id"], b["user_item_unique_id"]);
        assert_eq!(b["slot"], "braceletLeft");
        let p: Value = serde_json::from_str(&potion(0, 1_000_003).unwrap()).unwrap();
        assert_eq!(p["quantity"], 20);
        assert_eq!(p["user_item_metadata"]["item_index"], 662);
    }
    #[test]
    fn complete_restore_rejects_malformed_required_item_before_save() {
        let c = SimulationConfig::default();
        c.account_store.lock().unwrap().accounts.clear();
        let a = Actor {
            ordinal: 0,
            account_id: "cmp_test_actor".into(),
            password: "local-unique-password-123".into(),
            name: "CmpTestActor".into(),
        };
        let index = prepare_actor(&c, &a, Point { x: 100, y: 100 }, None, false).unwrap();
        let mut s = SimulationSession::new(c.clone());
        packet(
            &mut s,
            ClientPacket::Login {
                account_id: a.account_id.clone(),
                password: a.password,
            },
            |p| matches!(p, ServerPacket::LoginSuccess { .. }),
        )
        .unwrap();
        packet(
            &mut s,
            ClientPacket::StartGame {
                character_index: index,
            },
            |p| matches!(p, ServerPacket::StartGame { result: 4, .. }),
        )
        .unwrap();
        let before = s.active_character_checkpoint().unwrap();
        let mut invalid = before.clone();
        invalid.equipment_items_json[0] = "{".into();
        assert!(s.restore_active_character_checkpoint(&invalid).is_err());
        assert_eq!(s.active_character_checkpoint().unwrap(), before);
    }
    #[test]
    fn scenario_lane_guards_keep_dense_two_cells_and_melee_one_unique_occupancy() {
        let dense = json!({"start":{"x":10,"y":10},"lane":[{"x":10,"y":10},{"x":11,"y":10}]});
        let melee = json!({"start":{"x":10,"y":10},"lane":[{"x":10,"y":10},{"x":10,"y":10}]});
        let bits = vec![255; 80_000];
        let mut cells = BTreeSet::new();
        validate_lane(&dense, false, &mut cells, &bits).unwrap();
        assert_eq!(cells.len(), 2);
        assert!(validate_lane(&dense, true, &mut BTreeSet::new(), &bits).is_err());
        assert!(validate_lane(&melee, false, &mut BTreeSet::new(), &bits).is_err());
        let mut cells = BTreeSet::new();
        validate_lane(&melee, true, &mut cells, &bits).unwrap();
        assert_eq!(cells.len(), 1);
        assert!(validate_lane(&melee, true, &mut cells, &bits).is_err());
        assert!(validate_lane(&melee, true, &mut BTreeSet::new(), &[0; 80_000]).is_err());
    }
    #[test]
    fn melee_fresh_checkpoint_rejects_skills_without_clearing_them() {
        let config = base_config();
        config.account_store.lock().unwrap().accounts.clear();
        let actor = Actor {
            ordinal: 0,
            account_id: "cmp_melee_actor".into(),
            password: "local-unique-password-123".into(),
            name: "CmpMeleeActor".into(),
        };
        let index = prepare_actor(&config, &actor, Point { x: 100, y: 100 }, None, true).unwrap();
        let save =
            config.account_store.lock().unwrap().accounts[&actor.account_id].saves[&index].clone();
        validate_melee_checkpoint(&save).unwrap();
        let mut invalid = save;
        invalid.skill_states_json.push("{}".into());
        assert!(validate_melee_checkpoint(&invalid).is_err());
        assert_eq!(invalid.skill_states_json.len(), 1);
    }
}
