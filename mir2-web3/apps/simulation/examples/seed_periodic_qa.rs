//! OFFLINE preparation only. Never run against an existing player account store.
//!
//! Private input: MIR2_PERIODIC_FIXTURE_PASSWORD (random, at least 20 chars).
//! Outputs: MIR2_PERIODIC_STORE, MIR2_PERIODIC_SCENARIO_DIR,
//! MIR2_PERIODIC_EVIDENCE. Defaults are the isolated 20261002 QA directories.
//! Provisioned levels/items are fixtures, not natural progression/timing proof.
use std::{
    collections::BTreeSet,
    env,
    fs::{self, OpenOptions},
    io::Write,
    path::{Component, Path, PathBuf},
};

use mir2_game_data::{crystal_item_by_name, crystal_magic_by_spell, CrystalItemTemplate};
use mir2_protocol::{
    ClientPacket, MirClass, MirDirection, MirGender, MirGridType, Point, ServerPacket, Spell,
};
use mir2_simulation::{
    AccountRecord, CharacterBindPoint, CharacterRecord, CharacterSaveRecord, SimulationConfig,
    SimulationSession, Stage5SystemsState,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const SOURCE: &str = "apps/simulation/examples/seed_periodic_qa.rs";
const LEVELS: [u16; 8] = [10, 14, 15, 24, 25, 34, 35, 50];

fn main() {
    if env::args().skip(1).any(|arg| arg == "--help") {
        println!("OFFLINE only; refuses existing output files. Set private MIR2_PERIODIC_FIXTURE_PASSWORD, and optional MIR2_PERIODIC_STORE / MIR2_PERIODIC_SCENARIO_DIR / MIR2_PERIODIC_EVIDENCE / MIR2_PERIODIC_GATEWAY / MIR2_PERIODIC_SERVER_REVISION / MIR2_PERIODIC_HEALTH_SOURCE. Creates 24 measurement and 3 independent smoke fixtures. No credentials are printed.");
        return;
    }
    if env::args().len() > 1 {
        fail("only --help is accepted; credentials must not be CLI arguments");
    }
    match run() {
        Ok((hash, count)) => println!("Offline owned fixtures prepared: {count}; store SHA256 {hash}. Natural progress/timing: not measured."),
        Err(error) => fail(&error),
    }
}

fn fail(message: &str) -> ! {
    eprintln!("seed_periodic_qa: {message}");
    std::process::exit(2)
}
fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
fn setting(key: &str, default: &str) -> String {
    env::var(key).unwrap_or_else(|_| default.into())
}

fn allowed_path(path: &Path, kind: &str) -> Result<(), String> {
    if !path.is_absolute()
        || path
            .components()
            .any(|part| matches!(part, Component::ParentDir | Component::CurDir))
    {
        return Err(format!(
            "{kind} output must be an absolute QA path without dot components"
        ));
    }
    let windows = Path::new("C:/mir2-periodic-qa-20261002");
    let linux_store = Path::new("/var/lib/mir2-periodic-qa-20261002");
    let linux_private = Path::new("/opt/mir2-periodic-qa-20261002/private");
    let linux_evidence = Path::new("/opt/mir2-periodic-qa-20261002/evidence");
    let allowed = match kind {
        "store" => path.starts_with(windows.join("private")) || path.starts_with(linux_store),
        "scenarios" => path.starts_with(windows.join("private")) || path.starts_with(linux_private),
        "evidence" => path.starts_with(windows) || path.starts_with(linux_evidence),
        _ => false,
    };
    if !allowed {
        return Err(format!(
            "{kind} output is outside the exclusively owned periodic QA roots"
        ));
    }
    // Existing ancestors must not redirect the protected lexical prefix.
    let mut ancestor = path;
    while !ancestor.exists() {
        ancestor = ancestor
            .parent()
            .ok_or("QA path has no existing ancestor")?;
    }
    let mut current = Some(ancestor);
    while let Some(candidate) = current {
        let metadata =
            fs::symlink_metadata(candidate).map_err(|_| "cannot inspect QA output ancestor")?;
        #[cfg(windows)]
        let redirected = {
            use std::os::windows::fs::MetadataExt;
            metadata.file_attributes() & 0x400 != 0 // FILE_ATTRIBUTE_REPARSE_POINT
        };
        #[cfg(not(windows))]
        let redirected = metadata.file_type().is_symlink();
        if redirected {
            return Err(format!("{kind} output has a symlink ancestor"));
        }
        current = candidate.parent();
    }
    Ok(())
}

fn create_private_dir(path: &Path) -> Result<(), String> {
    fs::create_dir_all(path).map_err(|_| "cannot create private QA output directory")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))
            .map_err(|_| "cannot protect private QA directory")?;
    }
    Ok(())
}

fn create_new(path: &Path, bytes: &[u8], private: bool) -> Result<(), String> {
    let parent = path.parent().ok_or("output needs a parent")?;
    if private {
        create_private_dir(parent)?;
    } else {
        fs::create_dir_all(parent).map_err(|_| "cannot create evidence directory")?;
    }
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(if private { 0o600 } else { 0o644 });
    }
    let mut output = options.open(path).map_err(|_| {
        "output already exists or cannot be exclusively created; nothing may be replaced"
    })?;
    output
        .write_all(bytes)
        .and_then(|_| output.sync_all())
        .map_err(|_| "failed to write new fixture output".to_owned())
}

fn run() -> Result<(String, usize), String> {
    let password = env::var("MIR2_PERIODIC_FIXTURE_PASSWORD")
        .map_err(|_| "private random password environment input is required")?;
    if password.len() < 20
        || password.len() > 64
        || password.chars().any(char::is_whitespace)
        || password.chars().collect::<BTreeSet<_>>().len() < 8
    {
        return Err("private fixture password must have 20–64 nonwhitespace characters and at least eight distinct characters".into());
    }
    #[cfg(windows)]
    let defaults = (
        "C:/mir2-periodic-qa-20261002/private/accounts.json",
        "C:/mir2-periodic-qa-20261002/private/scenarios",
        "C:/mir2-periodic-qa-20261002/fixture-prep.json",
    );
    #[cfg(not(windows))]
    let defaults = (
        "/var/lib/mir2-periodic-qa-20261002/accounts.json",
        "/opt/mir2-periodic-qa-20261002/private/scenarios",
        "/opt/mir2-periodic-qa-20261002/evidence/fixture-prep.json",
    );
    let store_path = PathBuf::from(setting("MIR2_PERIODIC_STORE", defaults.0));
    let scenario_dir = PathBuf::from(setting("MIR2_PERIODIC_SCENARIO_DIR", defaults.1));
    let evidence_path = PathBuf::from(setting("MIR2_PERIODIC_EVIDENCE", defaults.2));
    allowed_path(&store_path, "store")?;
    allowed_path(&scenario_dir, "scenarios")?;
    allowed_path(&evidence_path, "evidence")?;
    if store_path.exists()
        || evidence_path.exists()
        || scenario_dir.exists()
            && fs::read_dir(&scenario_dir)
                .map_err(|_| "cannot inspect scenario directory")?
                .next()
                .is_some()
    {
        return Err("refusing existing store, receipt or nonempty scenario directory; use fresh owned QA outputs".into());
    }
    if store_path == evidence_path || store_path == scenario_dir || evidence_path == scenario_dir {
        return Err("private store and redacted evidence must have distinct paths".into());
    }
    let gateway = setting("MIR2_PERIODIC_GATEWAY", "ws://127.0.0.1:7290/ws");
    if gateway != "ws://127.0.0.1:7290/ws" {
        return Err(
            "offline seeder scenarios must target the isolated loopback 7290 gateway".into(),
        );
    }
    let health_source = setting(
        "MIR2_PERIODIC_HEALTH_SOURCE",
        "periodic-qa-20261002 isolated deployment /health receipt",
    );
    let expected_revision = env::var("MIR2_PERIODIC_SERVER_REVISION").ok();
    if expected_revision
        .as_ref()
        .is_some_and(|value| value.len() != 40 || !value.chars().all(|ch| ch.is_ascii_hexdigit()))
    {
        return Err("expected server revision must be a full 40-character source SHA".into());
    }
    env::set_var("MIR2_QUEST_CADENCE", "newcomer-v2");
    let config = SimulationConfig::default()
        .with_crystal_map_runtime()
        .with_platinum_176_profile();
    config
        .account_store
        .lock()
        .map_err(|_| "owned memory store unavailable")?
        .accounts
        .clear();
    let mut private_scenarios = Vec::new();
    let mut receipts = Vec::new();
    let mut index = 1000;
    for (class, class_name, short) in [
        (MirClass::Warrior, "Warrior", "War"),
        (MirClass::Wizard, "Wizard", "Wiz"),
        (MirClass::Taoist, "Taoist", "Tao"),
    ] {
        for (phase, level) in LEVELS
            .into_iter()
            .map(|level| ("measure", level))
            .chain(std::iter::once(("smoke", 10)))
        {
            index += 1;
            let ending = if phase == "smoke" { "S" } else { "M" };
            let scenario_id = format!("{phase}-{class_name}-{level}");
            let account_id = format!(
                "pqa-{}-{level}-{}",
                short.to_lowercase(),
                ending.to_lowercase()
            );
            let name = format!("Pq{short}{level}{ending}");
            let receipt = prepare(
                &config,
                &account_id,
                &password,
                CharacterRecord {
                    index,
                    name: name.clone(),
                    level,
                    class,
                    gender: MirGender::Male,
                },
                &scenario_id,
            )?;
            receipts.push(receipt);
            let mut scenario = json!({"scenarioId":scenario_id,"phase":phase,"className":class_name,"initialLevel":level,
                "accountId":account_id,"password":password,"name":name,"characterIndex":index,
                "fixture":{"kind":"offline-owned-level-equipment","source":SOURCE,"sha256":""},
                "gatewayUrl":gateway,"serverHealthSource":health_source});
            if let Some(revision) = &expected_revision {
                scenario["expectedServerRevision"] = json!(revision);
            }
            private_scenarios.push(scenario);
        }
    }
    let mut store = config
        .account_store
        .lock()
        .map_err(|_| "owned memory store unavailable")?
        .clone();
    store.next_character_index = index + 1;
    if store.accounts.len() != 27
        || !store.shared_guilds.is_empty()
        || !store.shared_heroes.is_empty()
        || store
            .accounts
            .iter()
            .any(|(id, account)| !id.starts_with("pqa-") || account.gm_level != 0)
    {
        return Err("unexpected unowned or privileged data in prepared fixture store".into());
    }
    let store_bytes =
        serde_json::to_vec_pretty(&store).map_err(|_| "cannot encode prepared private store")?;
    let hash = sha256(&store_bytes);
    for scenario in &mut private_scenarios {
        scenario["fixture"]["sha256"] = json!(hash);
    }
    let source_hash = sha256(include_bytes!("seed_periodic_qa.rs"));
    let evidence = json!({"schema":1,"kind":"offline-owned-level-equipment","source":SOURCE,"sourceSha256":source_hash,
        "storeSha256":hash,"fixtureCount":27,"measurementFixtureCount":24,"smokeFixtureCount":3,
        "credentialsRedacted":true,"onlineConnections":0,"ordinaryTimedCompletion":false,"naturalProgression":false,
        "profile":"platinum-176","questProfile":"newcomer-v2","timeZone":"UTC+8","initialMap":"0","initialPosition":{"x":335,"y":267},
        "preparation":"Canonical unenhanced catalog items; ordinary EquipItem and UseItem(book) packets validated offline; skills rank zero; zero quest task counts or claims; no summoned pets, guild, mentor, EXP buff or GM privileges.",
        "fixtureFrozenBeforeRun":true,"duringRunOfflineReplenishmentAllowed":false,"receipts":receipts});
    // All fixtures validate before any output is published. Exclusive create
    // keeps a race or rerun from replacing an already-started acceptance store.
    create_new(&store_path, &store_bytes, true)?;
    for scenario in &private_scenarios {
        let scenario_id = scenario["scenarioId"]
            .as_str()
            .ok_or("scenario identity missing")?;
        create_new(
            &scenario_dir.join(format!("{scenario_id}.json")),
            &serde_json::to_vec_pretty(scenario).map_err(|_| "cannot encode private scenario")?,
            true,
        )?;
    }
    create_new(
        &evidence_path,
        &serde_json::to_vec_pretty(&evidence).map_err(|_| "cannot encode redacted evidence")?,
        false,
    )?;
    Ok((hash, private_scenarios.len()))
}

fn stat(session: &SimulationSession, id: u8) -> i32 {
    session
        .world_snapshot()
        .player_crystal_stats
        .iter()
        .find(|value| value.stat == id)
        .map_or(0, |value| value.value)
}
fn canonical(name: &str) -> Result<CrystalItemTemplate, String> {
    crystal_item_by_name(name).ok_or_else(|| format!("canonical fixture item missing: {name}"))
}
fn item_stat(template: &CrystalItemTemplate, id: u8) -> i32 {
    template
        .stats
        .iter()
        .find(|value| value.stat == id)
        .map_or(0, |value| value.value)
}
fn slot_name(item_type: u8) -> Option<&'static str> {
    match item_type {
        1 => Some("weapon"),
        2 => Some("armour"),
        4 => Some("helmet"),
        5 => Some("necklace"),
        6 => Some("braceletLeft"),
        7 => Some("ringLeft"),
        8 => Some("amulet"),
        9 => Some("belt"),
        10 => Some("boots"),
        _ => None,
    }
}
fn item(template: &CrystalItemTemplate, quantity: u32, slot: u8, uid: u64) -> Value {
    let grade = match template.grade {
        1 => "common",
        2 => "rare",
        3 => "legendary",
        4 => "mythical",
        5 => "heroic",
        _ => "none",
    };
    let dura = (template.durability > 0).then_some(template.durability);
    json!({"key":format!("crystal-item-{}",template.item_index),"name":template.name,"icon":template.image,"slot":slot,
        "unique_id":uid,"container":"bag1","quantity":quantity,"description":template.tooltip.clone().unwrap_or_default(),
        "durability_current":dura,"durability_max":dura,"weight":template.weight,"equip_slot":slot_name(template.item_type),
        "grade":grade,"added_attack":0,"added_defence":0,"added_stats":[],"socketed":[],"socket_slots":template.slots,
        "user_item_metadata":{"item_index":template.item_index,"gm_made":false},"identified":!template.need_identify,
        "attack":item_stat(template,5),"defence":item_stat(template,1),"heal_hp":item_stat(template,12),"heal_mp":item_stat(template,13)})
}

fn eligible(
    config: &SimulationConfig,
    template: &CrystalItemTemplate,
    character: &CharacterRecord,
) -> bool {
    let flag = match character.class {
        MirClass::Warrior => 1,
        MirClass::Wizard => 2,
        MirClass::Taoist => 4,
        _ => 0,
    };
    config.item_is_allowed(&template.name)
        && template.required_class & flag != 0
        && template.required_gender & 1 != 0
        && template.required_type == 0
        && u16::from(template.required_amount) <= character.level
}

fn preferred_weapon(class: MirClass, level: u16) -> &'static str {
    match class {
        MirClass::Warrior => match level {
            40.. => "DragonSlayer",
            30.. => "JudgementMace",
            25.. => "PurifierSword",
            22.. => "PowerAxe",
            20.. => "MartialSword",
            13.. => "BronzeAxe",
            _ => "IronSword",
        },
        MirClass::Wizard => match level {
            40.. => "DragonStaff",
            36.. => "MagicScythe",
            30.. => "WarMageStaff",
            26.. => "MageStaff",
            15.. => "Trident",
            _ => "EbonySword",
        },
        _ => match level {
            40.. => "SoulSabre",
            36.. => "StoneBambooFan",
            30.. => "SoulSpringWand",
            26.. => "SerpentSword",
            15.. => "Scimitar",
            _ => "EbonySword",
        },
    }
}
fn preferred_armour(class: MirClass, level: u16) -> &'static str {
    match (class, level) {
        (MirClass::Warrior, 40..) => "SteelArmour(M)",
        (MirClass::Warrior, 33..) => "IronArmour(M)",
        (MirClass::Warrior, 22..) => "HeavyArmour(M)",
        (MirClass::Wizard, 40..) => "DragonRobe(M)",
        (MirClass::Wizard, 33..) => "WizardRobe(M)",
        (MirClass::Wizard, 22..) => "MagicRobe(M)",
        (MirClass::Taoist, 40..) => "TitanArmour(M)",
        (MirClass::Taoist, 33..) => "PearlArmour(M)",
        (MirClass::Taoist, 22..) => "SoulArmour(M)",
        (_, 16..) => "MediumArmour(M)",
        (_, 11..) => "LightArmour(M)",
        _ => "BaseDress(M)",
    }
}
fn equipment_plan(character: &CharacterRecord) -> Vec<(&'static str, i32)> {
    let level = character.level;
    let class = character.class;
    let necklace = if level < 13 {
        "GoldNecklace"
    } else {
        match class {
            MirClass::Warrior => "BlackNecklace",
            MirClass::Wizard => "EbonyNecklace",
            _ => "YellowNecklace",
        }
    };
    let ring = match class {
        MirClass::Warrior if level >= 25 => "CoralRing",
        MirClass::Warrior if level >= 16 => "BlueRing",
        MirClass::Warrior => "HornRing",
        MirClass::Wizard if level >= 20 => "SerpentEyeRing",
        MirClass::Wizard => "HexagonalRing",
        MirClass::Taoist if level >= 20 => "PearlRing",
        _ => "GlassRing",
    };
    let bracelet = if level >= 18 {
        if class == MirClass::Warrior {
            "HardGlove"
        } else {
            "MagicBracelet"
        }
    } else {
        "SteelBracelet"
    };
    vec![
        (preferred_weapon(class, level), 0),
        (preferred_armour(class, level), 1),
        (necklace, 4),
        (ring, 7),
        (ring, 8),
        (bracelet, 5),
        (bracelet, 6),
        (
            if level >= 14 {
                "MagicHelmet"
            } else {
                "BronzeHelmet"
            },
            2,
        ),
        ("LeatherBelt", 10),
        ("LowShoes", 11),
    ]
}
fn books(class: MirClass) -> &'static [&'static str] {
    match class {
        MirClass::Warrior => &[
            "Fencing",
            "Slaying",
            "Thrusting",
            "HalfMoon",
            "ShoulderDash",
            "FlamingSword",
        ],
        MirClass::Wizard => &[
            "FireBall",
            "Repulsion",
            "GreatFireBall",
            "ThunderBolt",
            "FireBang",
            "FireWall",
            "Lightning",
            "MagicShield",
            "IceStorm",
        ],
        _ => &[
            "Healing",
            "SpiritSword",
            "Poisoning",
            "SoulFireBall",
            "SummonSkeleton",
            "Hiding",
            "SoulShield",
            "BlessedArmour",
            "SummonShinsu",
        ],
    }
}
fn supplies(class: MirClass, level: u16) -> Vec<(&'static str, u32)> {
    let (hp, mp, hp_count, mp_count) = match (class, level) {
        (MirClass::Warrior, 10..=14) => ("(HP)DrugSmall", "(MP)DrugSmall", 40, 10),
        (MirClass::Wizard, 10..=14) => ("(HP)DrugSmall", "(MP)DrugSmall", 20, 30),
        (_, 10..=14) => ("(HP)DrugSmall", "(MP)DrugSmall", 30, 20),
        (MirClass::Warrior, 15..=24) => ("(HP)DrugMedium", "(MP)DrugMedium", 30, 10),
        (MirClass::Wizard, 15..=24) => ("(HP)DrugMedium", "(MP)DrugMedium", 12, 25),
        (_, 15..=24) => ("(HP)DrugMedium", "(MP)DrugMedium", 20, 20),
        (MirClass::Warrior, 25..=34) => ("(HP)DrugLarge", "(MP)DrugLarge", 40, 10),
        (MirClass::Wizard, 25..=34) => ("(HP)DrugLarge", "(MP)DrugLarge", 15, 30),
        (_, 25..=34) => ("(HP)DrugLarge", "(MP)DrugLarge", 30, 30),
        // Platinum176 deliberately exposes no XL medicines. Higher bands
        // retain lawful Large stock and ordinary shop gold, not forbidden items.
        (MirClass::Warrior, _) => ("(HP)DrugLarge", "(MP)DrugLarge", 80, 20),
        (MirClass::Wizard, _) => ("(HP)DrugLarge", "(MP)DrugLarge", 30, 60),
        _ => ("(HP)DrugLarge", "(MP)DrugLarge", 40, 70),
    };
    let mut result = vec![
        (hp, hp_count),
        (mp, mp_count),
        ("TownTeleport", if level < 25 { 1 } else { 2 }),
        ("RandomTeleport", if level < 25 { 5 } else { 10 }),
    ];
    if class == MirClass::Taoist && level >= 14 {
        result.extend([("GreenPoison", 40), ("RedPoison", 40)]);
    }
    if class == MirClass::Taoist && level >= 18 {
        result.push(("Amulet", 200));
    }
    result
}

fn prepare(
    config: &SimulationConfig,
    account_id: &str,
    password: &str,
    character: CharacterRecord,
    scenario_id: &str,
) -> Result<Value, String> {
    let mut draft = CharacterSaveRecord::new(character.clone());
    draft.map_file_name = "0".into();
    draft.map_title = "BichonProvince".into();
    draft.position = Point { x: 335, y: 267 };
    draft.direction = MirDirection::Up;
    draft.bind_point = Some(CharacterBindPoint {
        map_file_name: "0".into(),
        position: Point { x: 330, y: 268 },
    });
    draft.max_experience = config.experience_required_for_level(character.level);
    draft.gold = match character.level {
        10..=14 => 20000,
        15..=24 => 50000,
        25..=34 => 120000,
        _ => 200000,
    };
    draft.equipment_items_explicit_empty = true;
    draft.inventory_items_json = vec![item(
        &canonical("(HP)DrugSmall")?,
        1,
        0,
        (character.index as u64) * 1000 + 1,
    )
    .to_string()];
    let mut account = AccountRecord::empty();
    account.password = password.into();
    account.characters.push(character.clone());
    account.saves.insert(character.index, draft.clone());
    config
        .account_store
        .lock()
        .map_err(|_| "owned memory store unavailable")?
        .accounts
        .insert(account_id.into(), account);
    let mut session = SimulationSession::new(config.clone());
    let login = session
        .try_handle_packet(ClientPacket::Login {
            account_id: account_id.into(),
            password: password.into(),
        })
        .map_err(|_| format!("{scenario_id}: offline login failed"))?;
    if !login
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. }))
    {
        return Err(format!("{scenario_id}: offline login rejected"));
    }
    let start = session
        .try_handle_packet(ClientPacket::StartGame {
            character_index: character.index,
        })
        .map_err(|_| format!("{scenario_id}: offline StartGame failed"))?;
    if !start
        .iter()
        .any(|packet| matches!(packet, ServerPacket::StartGame { result: 4, .. }))
    {
        return Err(format!("{scenario_id}: offline StartGame rejected"));
    }
    let hand_capacity = stat(&session, 17).max(0) as u32;
    let wear_capacity = stat(&session, 18).max(0) as u32;
    let mut wear = if character.class == MirClass::Taoist && character.level >= 14 {
        1
    } else {
        0
    };
    let mut gear = Vec::new();
    let mut omitted = Vec::new();
    let mut receipt = Vec::new();
    let mut inventory = Vec::new();
    let mut uid = (character.index as u64) * 1000;
    let mut add = |template: &CrystalItemTemplate, count: u32| -> Result<u64, String> {
        let first = uid + 1;
        let mut remaining = count;
        while remaining > 0 {
            let stack = remaining.min(u32::from(template.stack_size.max(1)));
            uid += 1;
            if inventory.len() >= 40 {
                return Err(format!(
                    "{scenario_id}: canonical preparation exceeds the ordinary bag capacity"
                ));
            }
            inventory.push(item(template, stack, inventory.len() as u8, uid).to_string());
            remaining -= stack;
        }
        Ok(first)
    };
    for (name, to) in equipment_plan(&character) {
        let template = canonical(name)?;
        if !eligible(config, &template, &character) {
            return Err(format!(
                "{scenario_id}: canonical gear is not eligible: {name}"
            ));
        }
        if to == 0 && u32::from(template.weight) > hand_capacity
            || to != 0 && wear + u32::from(template.weight) > wear_capacity
        {
            omitted.push(json!({"name":name,"reason":"canonical hand/wear capacity"}));
            continue;
        }
        if to != 0 {
            wear += u32::from(template.weight);
        }
        let item_uid = add(&template, 1)?;
        gear.push((item_uid, to, template.clone()));
        receipt.push(json!({"kind":"equipment","name":template.name,"itemIndex":template.item_index,"quantity":1,"unitCatalogPrice":template.price,"catalogValue":template.price,"slotIndex":to,"unenhanced":true}));
    }
    let mut skill_books = Vec::new();
    for name in books(character.class) {
        let template = canonical(name)?;
        let Some(magic) = crystal_magic_by_spell(name) else {
            return Err(format!("canonical spell missing: {name}"));
        };
        if !eligible(config, &template, &character)
            || !config.skill_is_allowed(&magic.spell, character.class, character.level)
        {
            continue;
        }
        skill_books.push((add(&template, 1)?, magic.spell.clone()));
        receipt.push(json!({"kind":"skillBook","name":template.name,"itemIndex":template.item_index,"quantity":1,"unitCatalogPrice":template.price,"catalogValue":template.price,"learnedRank":0}));
    }
    let mut amulet = None;
    for (name, count) in supplies(character.class, character.level) {
        let template = canonical(name)?;
        if !eligible(config, &template, &character) {
            return Err(format!(
                "{scenario_id}: canonical supply is not eligible: {name}"
            ));
        }
        let item_uid = add(&template, count)?;
        if character.class == MirClass::Taoist
            && (name == "Amulet" || name == "GreenPoison" && character.level < 18)
        {
            amulet = Some((item_uid, template.clone()));
        }
        receipt.push(json!({"kind":"supply","name":template.name,"itemIndex":template.item_index,"quantity":count,"unitCatalogPrice":template.price,"catalogValue":u64::from(template.price)*u64::from(count),"offlineGranted":true}));
    }
    drop(add);
    draft.inventory_items_json = inventory;
    session
        .restore_active_character_checkpoint(&draft)
        .map_err(|_| format!("{scenario_id}: canonical fixture decode/restore rejected"))?;
    for (item_uid, to, template) in &gear {
        session
            .try_handle_packet(ClientPacket::EquipItem {
                grid: MirGridType::Inventory,
                unique_id: *item_uid,
                to: *to,
            })
            .map_err(|_| format!("{scenario_id}: ordinary EquipItem failed"))?;
        if !session.world_snapshot().equipment_items.iter().any(|item| {
            item.key == format!("crystal-item-{}", template.item_index)
                && item.unique_id == Some(*item_uid)
        }) {
            return Err(format!(
                "{scenario_id}: ordinary equipment requirements rejected {}",
                template.name
            ));
        }
    }
    if let Some((item_uid, template)) = &amulet {
        session
            .try_handle_packet(ClientPacket::EquipItem {
                grid: MirGridType::Inventory,
                unique_id: *item_uid,
                to: 9,
            })
            .map_err(|_| format!("{scenario_id}: ordinary Amulet equip failed"))?;
        if !session
            .world_snapshot()
            .equipment_items
            .iter()
            .any(|item| item.key == format!("crystal-item-{}", template.item_index))
        {
            return Err(format!("{scenario_id}: ordinary consumable equip rejected"));
        }
    }
    for (item_uid, spell) in &skill_books {
        session
            .try_handle_packet(ClientPacket::UseItem {
                unique_id: *item_uid,
                grid: MirGridType::Inventory,
            })
            .map_err(|_| format!("{scenario_id}: ordinary skill-book use failed"))?;
        if !session
            .world_snapshot()
            .known_skills
            .iter()
            .any(|skill| skill.spell.as_deref() == Some(spell.as_str()) && skill.level == 0)
        {
            return Err(format!(
                "{scenario_id}: ordinary learned-skill validation failed for {spell}"
            ));
        }
    }
    for (key, name) in match character.class {
        MirClass::Warrior => vec![
            (1, "Slaying"),
            (2, "Thrusting"),
            (3, "HalfMoon"),
            (4, "FlamingSword"),
        ],
        MirClass::Wizard => vec![
            (1, "FireBall"),
            (2, "GreatFireBall"),
            (3, "Lightning"),
            (4, "MagicShield"),
            (5, "FireBang"),
            (6, "FireWall"),
        ],
        _ => vec![
            (1, "Healing"),
            (2, "SoulFireBall"),
            (3, "SummonSkeleton"),
            (4, "Poisoning"),
            (5, "SummonShinsu"),
        ],
    } {
        if let Some(spell) = Spell::from_crystal_name(name).filter(|_| {
            session
                .world_snapshot()
                .known_skills
                .iter()
                .any(|skill| skill.spell.as_deref() == Some(name))
        }) {
            session
                .try_handle_packet(ClientPacket::MagicKey {
                    spell,
                    key,
                    old_key: 0,
                })
                .map_err(|_| format!("{scenario_id}: ordinary MagicKey failed"))?;
        }
    }
    let snapshot = session.world_snapshot();
    let weights = snapshot
        .player_weights
        .clone()
        .ok_or_else(|| format!("{scenario_id}: canonical carried weights unavailable"))?;
    if weights.bag > stat(&session, 16).max(0) as u32
        || weights.hand > hand_capacity
        || weights.wear > wear_capacity
    {
        return Err(format!(
            "{scenario_id}: final canonical loadout exceeds carrying capacities"
        ));
    }
    session
        .save_active_character()
        .map_err(|_| format!("{scenario_id}: offline fixture save failed"))?;
    let final_save = config
        .account_store
        .lock()
        .map_err(|_| "owned memory store unavailable")?
        .accounts[account_id]
        .saves[&character.index]
        .clone();
    if final_save.character != character
        || final_save.map_file_name != "0"
        || final_save.position != (Point { x: 335, y: 267 })
        || final_save.hp <= 0
        || final_save.hp != final_save.max_hp
        || final_save.mp != final_save.max_mp
        || !snapshot
            .known_skills
            .iter()
            .all(|skill| skill.level == 0 && skill.experience == 0)
    {
        return Err(format!("{scenario_id}: fixture identity, position, vitals or skill practice changed unexpectedly"));
    }
    for encoded in &final_save.quest_states_json {
        let quest: Value =
            serde_json::from_str(encoded).map_err(|_| "prepared quest state is invalid")?;
        if quest["current"].as_u64().unwrap_or(0) != 0
            || quest["stage"] == "completed"
            || quest["stage"] == "readyToTurnIn"
            || quest["cadence_last_claimed_period"].is_u64()
            || quest["task_progress"].as_object().is_some_and(|progress| {
                progress
                    .values()
                    .any(|count| count.as_u64().unwrap_or(0) != 0)
            })
        {
            return Err(format!(
                "{scenario_id}: offline preparation unexpectedly advanced a quest counter or claim"
            ));
        }
    }
    if !final_save.buff_states_json.is_empty()
        || final_save.hero_vitals.is_some()
        || final_save.credit != 0
        || final_save.experience != 0
    {
        return Err(format!(
            "{scenario_id}: unexpected buff/hero/credit/EXP in offline fixture"
        ));
    }
    let systems: Stage5SystemsState = final_save
        .stage5_systems_json
        .as_deref()
        .map(serde_json::from_str)
        .transpose()
        .map_err(|_| "prepared personal systems invalid")?
        .unwrap_or_default();
    if !systems.guild.name.is_empty()
        || !systems.group.members.is_empty()
        || !systems.mentor.name.is_empty()
        || systems.mentor.partner_identity.is_some()
        || systems.hero.is_some()
        || !systems.intelligent_creatures.is_empty()
        || systems.active_intelligent_creature().is_some()
        || snapshot
            .entities
            .iter()
            .any(|entity| entity.owner_name.is_some())
    {
        return Err(format!(
            "{scenario_id}: unexpected guild, mentor or hero fixture"
        ));
    }
    let total_value: u64 = receipt
        .iter()
        .map(|entry| entry["catalogValue"].as_u64().unwrap_or(0))
        .sum();
    Ok(
        json!({"scenarioId":scenario_id,"className":format!("{:?}",character.class),"initialLevel":character.level,"characterIndex":character.index,
        "initialGold":final_save.gold,"inventoryCatalogValue":total_value,"purchaseGoldDebited":0,"offlineProvisioning":true,"saveSha256":sha256(&serde_json::to_vec(&final_save).map_err(|_|"cannot hash prepared save")?),
        "hp":final_save.hp,"maxHp":final_save.max_hp,"mp":final_save.mp,"maxMp":final_save.max_mp,
        "carriedWeights":weights,"weightCapacities":{"bag":stat(&session,16),"hand":hand_capacity,"wear":wear_capacity},
        "initialBagSlots":snapshot.inventory_items.len(),"skills":snapshot.known_skills.iter().map(|skill|json!({"spell":skill.spell,"rank":skill.level,"hotkey":skill.hotkey,"mpCost":skill.mp_cost,"delayMs":skill.delay_ms})).collect::<Vec<_>>(),
        "items":receipt,"omittedOptionalEquipment":omitted,"taskCounters":0,"completedClaims":0,"gmLevel":0,"preSummonedPets":0}),
    )
}
