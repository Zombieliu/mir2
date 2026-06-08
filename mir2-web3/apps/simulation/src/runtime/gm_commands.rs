//! In-game GM (`@`) chat commands — a 1:1 port of Crystal's `PlayerObject.Chat`
//! `@`-command switch (`Server/MirObjects/PlayerObject.cs:2152-4156`).
//!
//! Crystal consumes **every** `@`-prefixed chat line as a command attempt: it is
//! never echoed back to normal chat, and each `case` applies its own permission
//! gate (`IsGM`, `IsGM || Settings.TestServer`, a feature gate, or none at all —
//! e.g. `@TIME`, `@MAP`, `@DIE`, `@ROLL`, `@CLEARBUFFS` run for any player). We
//! mirror that exactly: [`dispatch_gm_command`] returns `Some(..)` for every `@`
//! line (so it is consumed, not echoed), and gates each command the way Crystal
//! does. A non-GM issuing a GM-gated command therefore gets silence — identical
//! to Crystal's `return;` — never a leaked error or a normal-chat echo.
//!
//! The single-session simulation has exactly one live character and no other
//! online players / guilds / conquests, so the cross-player and guild/conquest
//! commands faithfully resolve to Crystal's own "player not found" / "not in a
//! guild" / "no access" responses — which is the correct 1:1 result for a world
//! with nobody else in it. Everything backed by a real subsystem (level, gold,
//! items, skills, monsters, buffs, quests, flags, vitals, movement) is applied to
//! the live world and acknowledged with the matching client-refresh packet.

use std::time::{SystemTime, UNIX_EPOCH};

use mir2_protocol::{ChatType, MirClass, MirDirection, MirGender, Point, ServerPacket, Spell};

use crate::config::{ItemContainer, QuestStage, WorldEntityDisposition};

use super::buffs::crystal_buff_type_for_key;
use super::components::{
    current_player_is_dead, current_player_object_id, entity_name, entity_position, player_entity,
    CharacterBody, Facing, MonsterAgent, MonsterVitals, PlayerVitals, Position, SummonedMonster,
};
use super::equipment::user_item_from_equipment_state;
use super::inventory::{
    add_or_increment_item_with_durability_and_stats, expand_storage_rental_impl,
};
use super::items::{
    crystal_item_key_for_template, crystal_item_template_for_item_key, user_item_from_item_state,
};
use super::monsters::{
    crystal_dynamic_monster_template, deterministic_roll, spawn_runtime_monster,
};
use super::movement::{current_location, offset_point};
use super::packets::{object_died_info_for_entity, object_health_info_for_entity};
use super::resources::{
    BuffResource, GmRuntimeResource, InventoryResource, MapRuntimeResource, NpcStateResource,
    PlayerPermissionResource, PlayerRuntimeResource, QuestResource, SessionResource, SkillResource,
};
use super::session::runtime_tick;
use super::skills::{
    client_magic_for_skill_state, crystal_skill_state, skill_key_for_crystal_spell,
};
use bevy_ecs::prelude::{Entity, World};
use mir2_game_data::crystal_item_by_index;
use mir2_game_data::crystal_item_by_name;
use mir2_game_data::CrystalItemTemplate;

/// Crystal awakening constants (`Shared/Data/ItemData.cs`, `Awake`).
const AWAKE_MAX_LEVEL: usize = 5;
const AWAKE_SUCCESS_RATE: u64 = 70;
const AWAKE_HIT_RATE: u64 = 70;
const AWAKE_CHANCE_MAX: [u8; 5] = [1, 2, 3, 4, 5];
/// `BindMode.DontUpgrade` (`Shared/Enums.cs`).
const BIND_DONT_UPGRADE: i16 = 64;

/// Conquest AIs (`siege gate`, `gates`, `archer`, `wall`) that Crystal refuses to
/// spawn via `@MOB` / `@RECALLMOB` (`PlayerObject.cs:2166`).
const CONQUEST_SPAWN_BLOCKED_AIS: [u8; 5] = [72, 73, 80, 81, 82];

/// True when `message` is a GM command attempt (`@` prefix, non-empty body).
pub(super) fn is_gm_command(message: &str) -> bool {
    let trimmed = message.trim_start();
    trimmed.starts_with('@') && trimmed.len() > 1
}

fn gm_chat(world: &World, text: impl Into<String>, chat_type: ChatType) -> ServerPacket {
    ServerPacket::ObjectChat {
        object_id: current_player_object_id(world).unwrap_or(0),
        text: text.into(),
        chat_type,
    }
}

fn hint(world: &World, text: impl Into<String>) -> ServerPacket {
    gm_chat(world, text, ChatType::Hint)
}

fn system(world: &World, text: impl Into<String>) -> ServerPacket {
    gm_chat(world, text, ChatType::System)
}

fn system2(world: &World, text: impl Into<String>) -> ServerPacket {
    gm_chat(world, text, ChatType::System2)
}

fn is_gm(world: &World) -> bool {
    world.resource::<PlayerPermissionResource>().is_gm()
}

/// `IsGM || Settings.TestServer` — the gate the bulk of Crystal's `@` commands use.
fn is_gm_or_test(world: &World) -> bool {
    is_gm(world) || world.resource::<GmRuntimeResource>().test_server
}

fn player_name(world: &World) -> String {
    world
        .resource::<SessionResource>()
        .selected_character
        .as_ref()
        .map(|character| character.name.clone())
        .unwrap_or_else(|| "?????".to_string())
}

/// `@LOGIN` password handshake (`PlayerObject.cs:1898`). Called from the chat
/// handler *before* the `@`-dispatch when [`GmRuntimeResource::login_pending`] is
/// armed: the next chat line is the candidate GM password. Returns the
/// acknowledgement packets and consumes the line.
pub(super) fn resolve_gm_login_password(world: &mut World, message: &str) -> Vec<ServerPacket> {
    let (matches, name) = {
        let gm = world.resource::<GmRuntimeResource>();
        let matches = gm
            .password
            .as_deref()
            .map(|password| password == message)
            .unwrap_or(false);
        (matches, player_name(world))
    };
    world.resource_mut::<GmRuntimeResource>().login_pending = false;
    if matches {
        world.resource_mut::<PlayerPermissionResource>().gm_level = 1;
        vec![
            system(world, format!("{name} has been made a GM")),
            system(world, "You have been made a GM"),
        ]
    } else {
        vec![system(world, "Incorrect login password")]
    }
}

/// True when the GM-login password prompt is armed for the next chat line.
pub(super) fn gm_login_pending(world: &World) -> bool {
    world.resource::<GmRuntimeResource>().login_pending
}

/// Dispatch a GM `@` command. Crystal consumes every `@` line, so this always
/// returns `Some(..)` (possibly an empty vec when the gate fails or the command
/// is unknown — both are silent `return;`s in Crystal).
pub(super) fn dispatch_gm_command(world: &mut World, message: &str) -> Option<Vec<ServerPacket>> {
    let body = message.trim_start().strip_prefix('@')?.trim();
    let mut parts = body.split_whitespace();
    let command = parts.next()?.to_ascii_uppercase();
    let args: Vec<&str> = parts.collect();

    let packets = match command.as_str() {
        "LOGIN" => gm_login(world),
        "KILL" => gm_kill(world, &args),
        "CHANGEGENDER" => gm_change_gender(world, &args),
        "LEVEL" | "SETLEVEL" => gm_level(world, &args),
        "LEVELHERO" => gm_level_hero(world, &args),
        "MAKE" => gm_make(world, &args),
        "CLEARBUFFS" => gm_clear_buffs(world),
        "CLEARBAG" => gm_clear_bag(world),
        "SUPERMAN" => gm_superman(world),
        "GAMEMASTER" => gm_game_master(world),
        "OBSERVER" => gm_observer(world),
        "ALLOWGUILD" => gm_allow_guild(world),
        "RECALL" => gm_recall(world, &args),
        "OBSERVE" => gm_observe(world, &args),
        "ENABLEGROUPRECALL" => gm_enable_group_recall(world),
        "GROUPRECALL" => gm_group_recall(world),
        "RECALLMEMBER" => gm_recall_member(world, &args),
        "RECALLLOVER" => gm_recall_lover(world),
        "TIME" => gm_time(world),
        "ROLL" => gm_roll(world),
        "MAP" => gm_map(world),
        "BACKUPPLAYER" | "ARCHIVEPLAYER" | "LOADPLAYER" | "RESTOREPLAYER" => {
            gm_player_archive(world, &command, &args)
        }
        "MOVE" | "POS2" => gm_move(world, &args),
        "MAPMOVE" => gm_map_move(world, &args),
        "GOTO" => gm_goto(world, &args),
        "MOB" => gm_mob(world, &args),
        "RECALLMOB" => gm_recall_mob(world, &args),
        "RELOADDROPS" => gm_reload_drops(world),
        "RELOADNPCS" => gm_reload_npcs(world),
        "CLEARIPBLOCKS" => gm_clear_ip_blocks(world),
        "GIVEGOLD" | "GOLD" | "SETGOLD" => gm_give_gold(world, &args),
        "GIVEPEARLS" => gm_give_pearls(world, &args),
        "GIVECREDIT" => gm_give_credit(world, &args),
        "GIVESKILL" => gm_give_skill(world, &args),
        "FIND" => gm_find(world, &args),
        "LEAVEGUILD" => gm_leave_guild(world),
        "CREATEGUILD" => gm_create_guild(world, &args),
        "ALLOWTRADE" => gm_allow_trade(world),
        "TRIGGER" => gm_trigger(world, &args),
        "RIDE" => gm_ride(world),
        "SETFLAG" => gm_set_flag(world, &args),
        "LISTFLAGS" => gm_list_flags(world),
        "CLEARFLAGS" => gm_clear_flags(world, &args),
        "CLEARMOB" => gm_clear_mob(world, &args),
        "CHANGECLASS" => gm_change_class(world, &args),
        "DIE" => gm_die(world),
        "HAIR" => gm_hair(world, &args),
        "DECO" => gm_deco(world, &args),
        "ADJUSTPKPOINT" => gm_adjust_pk_point(world, &args),
        "AWAKENING" => gm_awakening(world, &args),
        "REMOVEAWAKENING" => gm_remove_awakening(world, &args),
        "STARTWAR" => gm_start_war(world, &args),
        "ADDINVENTORY" => gm_add_inventory(world),
        "ADDSTORAGE" => gm_add_storage(world),
        "SUMMONHERO" => gm_summon_hero(world),
        "ALLOWOBSERVE" => gm_allow_observe(world),
        "INFO" => gm_info(world, &args),
        "CLEARQUESTS" => gm_clear_quests(world, &args),
        "SETQUEST" => gm_set_quest(world, &args),
        "TOGGLETRANSFORM" => gm_toggle_transform(world),
        "STARTCONQUEST" => gm_start_conquest(world, &args),
        "RESETCONQUEST" => gm_reset_conquest(world, &args),
        "GATES" => gm_gates(world, &args),
        "CHANGEFLAG" => gm_change_flag(world),
        "CHANGEFLAGCOLOUR" | "CHANGEFLAGCOLOR" => gm_change_flag_colour(world),
        "REVIVE" => gm_revive(world, &args),
        "DELETESKILL" => gm_delete_skill(world, &args),
        "SETTIMER" => gm_set_timer(world, &args),
        "SETLIGHT" => gm_set_light(world, &args),
        // Crystal `default: break;` — the `@` line is consumed silently.
        _ => Vec::new(),
    };
    Some(packets)
}

// ---------------------------------------------------------------------------
// Authentication / permission toggles
// ---------------------------------------------------------------------------

fn gm_login(world: &mut World) -> Vec<ServerPacket> {
    world.resource_mut::<GmRuntimeResource>().login_pending = true;
    vec![hint(world, "Please type the GM Password")]
}

fn gm_superman(world: &mut World) -> Vec<ServerPacket> {
    if !is_gm_or_test(world) {
        return Vec::new();
    }
    let enabled = {
        let mut gm = world.resource_mut::<GmRuntimeResource>();
        gm.gm_never_die = !gm.gm_never_die;
        gm.gm_never_die
    };
    vec![hint(
        world,
        if enabled {
            "Invincible Mode."
        } else {
            "Normal Mode."
        },
    )]
}

fn gm_game_master(world: &mut World) -> Vec<ServerPacket> {
    if !is_gm_or_test(world) {
        return Vec::new();
    }
    let enabled = {
        let mut gm = world.resource_mut::<GmRuntimeResource>();
        gm.game_master = !gm.game_master;
        gm.game_master
    };
    vec![hint(
        world,
        if enabled {
            "GameMaster Mode."
        } else {
            "Normal Mode."
        },
    )]
}

fn gm_observer(world: &mut World) -> Vec<ServerPacket> {
    if !is_gm(world) {
        return Vec::new();
    }
    let enabled = {
        let mut gm = world.resource_mut::<GmRuntimeResource>();
        gm.observer = !gm.observer;
        gm.observer
    };
    vec![hint(
        world,
        if enabled {
            "Observer Mode."
        } else {
            "Normal Mode."
        },
    )]
}

fn gm_allow_guild(world: &mut World) -> Vec<ServerPacket> {
    let enabled = {
        let mut gm = world.resource_mut::<GmRuntimeResource>();
        gm.allow_guild_invite = !gm.allow_guild_invite;
        gm.allow_guild_invite
    };
    vec![hint(
        world,
        if enabled {
            "Guild invites enabled."
        } else {
            "Guild invites disabled."
        },
    )]
}

fn gm_enable_group_recall(world: &mut World) -> Vec<ServerPacket> {
    let enabled = {
        let mut gm = world.resource_mut::<GmRuntimeResource>();
        gm.enable_group_recall = !gm.enable_group_recall;
        gm.enable_group_recall
    };
    vec![hint(
        world,
        if enabled {
            "Group Recall Enabled."
        } else {
            "Group Recall Disabled."
        },
    )]
}

fn gm_allow_trade(world: &mut World) -> Vec<ServerPacket> {
    let enabled = {
        let mut gm = world.resource_mut::<GmRuntimeResource>();
        gm.allow_trade = !gm.allow_trade;
        gm.allow_trade
    };
    vec![system(
        world,
        if enabled {
            "You are now allowing trade"
        } else {
            "You are no longer allowing trade"
        },
    )]
}

fn gm_allow_observe(world: &mut World) -> Vec<ServerPacket> {
    let allow = {
        let mut gm = world.resource_mut::<GmRuntimeResource>();
        gm.allow_observe = !gm.allow_observe;
        gm.allow_observe
    };
    vec![ServerPacket::AllowObserve { allow }]
}

// ---------------------------------------------------------------------------
// Player progression / economy
// ---------------------------------------------------------------------------

fn gm_level(world: &mut World, args: &[&str]) -> Vec<ServerPacket> {
    if !is_gm_or_test(world) || args.is_empty() {
        return Vec::new();
    }
    // `@LEVEL <player> <level>` — cross-player form requires GM; with nobody else
    // online it resolves to Crystal's silent "player not found" path.
    if args.len() >= 2 {
        if !is_gm(world) {
            return Vec::new();
        }
        return vec![system(world, format!("Player {} was not found", args[0]))];
    }

    let raw = if args[0] == "-1" {
        u16::MAX.to_string()
    } else {
        args[0].to_string()
    };
    let Ok(level) = raw.parse::<u16>() else {
        return vec![system(world, "Could not level player")];
    };
    if level == 0 {
        return vec![system(world, "Could not level player")];
    }

    let old = {
        let mut session = world.resource_mut::<SessionResource>();
        match session.selected_character.as_mut() {
            Some(character) => {
                let old = character.level;
                character.level = level;
                old
            }
            None => return Vec::new(),
        }
    };
    apply_level_up(world, level);
    let mut packets = level_changed_packets(world, level);
    packets.push(system(
        world,
        format!(
            "Congratulations! You have leveled up. Your HP and MP have been restored. {old} -> {level}."
        ),
    ));
    packets
}

/// Shared level-application: mirror the body-component level and Crystal's
/// `LevelUp()` HP/MP restore.
fn apply_level_up(world: &mut World, level: u16) {
    let Some(player) = player_entity(world) else {
        return;
    };
    let restored = {
        let mut entity = world.entity_mut(player);
        if let Some(mut body) = entity.get_mut::<CharacterBody>() {
            body.level = level;
        }
        match entity.get_mut::<PlayerVitals>() {
            Some(mut vitals) => {
                vitals.hp = vitals.max_hp;
                vitals.mp = vitals.max_mp;
                Some(*vitals)
            }
            None => None,
        }
    };
    if let Some(restored) = restored {
        world.resource_mut::<PlayerRuntimeResource>().player_vitals = restored;
    }
}

fn level_changed_packets(world: &World, level: u16) -> Vec<ServerPacket> {
    let (experience, max_experience) = {
        let runtime = world.resource::<PlayerRuntimeResource>();
        (runtime.experience, runtime.max_experience)
    };
    let mut packets = vec![ServerPacket::LevelChanged {
        level,
        experience,
        max_experience,
    }];
    if let Some(player) = player_entity(world) {
        if let Some(info) = object_health_info_for_entity(world, player, 0) {
            packets.push(ServerPacket::ObjectHealth { info });
        }
    }
    packets
}

fn gm_level_hero(world: &mut World, args: &[&str]) -> Vec<ServerPacket> {
    if !is_gm_or_test(world) || args.is_empty() {
        return Vec::new();
    }
    // Cross-player hero levelling (`@LEVELHERO <player> <level>`) has no other
    // player to target in a single session.
    if args.len() >= 2 {
        return vec![system(world, format!("Player {} was not found", args[0]))];
    }
    let raw = if args[0] == "-1" {
        u16::MAX.to_string()
    } else {
        args[0].to_string()
    };
    let Some(level) = raw.parse::<u16>().ok().filter(|level| *level != 0) else {
        return Vec::new();
    };
    {
        let mut systems = world.resource_mut::<super::resources::Stage5SystemsResource>();
        let Some(hero) = systems.stage5_systems.hero.as_mut() else {
            return Vec::new();
        };
        hero.level = level;
    }
    vec![hint(world, format!("Hero levelled to {level}"))]
}

fn gm_give_gold(world: &mut World, args: &[&str]) -> Vec<ServerPacket> {
    if !is_gm_or_test(world) || args.is_empty() {
        return Vec::new();
    }
    // `@GIVEGOLD <player> <amount>` — cross-player path needs GM and another
    // online player; with nobody else it's Crystal's "player not found".
    if args.len() >= 2 {
        if !is_gm(world) {
            return Vec::new();
        }
        return vec![system(world, format!("Player {} was not found", args[0]))];
    }
    let Ok(amount) = args[0].parse::<u32>() else {
        return Vec::new();
    };
    let total = {
        let mut runtime = world.resource_mut::<PlayerRuntimeResource>();
        runtime.gold = runtime.gold.saturating_add(amount);
        runtime.gold
    };
    let name = player_name(world);
    vec![
        ServerPacket::GainedGold { gold: total },
        system(
            world,
            format!("Player {name} has been given {amount} gold by GM: {name}"),
        ),
    ]
}

fn gm_give_credit(world: &mut World, args: &[&str]) -> Vec<ServerPacket> {
    if !is_gm_or_test(world) || args.is_empty() {
        return Vec::new();
    }
    if args.len() >= 2 {
        if !is_gm(world) {
            return Vec::new();
        }
        return vec![system(world, format!("Player {} was not found", args[0]))];
    }
    let Ok(amount) = args[0].parse::<u32>() else {
        return Vec::new();
    };
    let total = {
        let mut runtime = world.resource_mut::<PlayerRuntimeResource>();
        runtime.credit = runtime.credit.saturating_add(amount);
        runtime.credit
    };
    let name = player_name(world);
    vec![
        ServerPacket::GainedCredit { credit: total },
        system(
            world,
            format!("Player {name} has been given {amount} credit"),
        ),
    ]
}

fn gm_give_pearls(world: &mut World, args: &[&str]) -> Vec<ServerPacket> {
    if !is_gm_or_test(world) || args.is_empty() {
        return Vec::new();
    }
    if args.len() >= 2 {
        if !is_gm(world) {
            return Vec::new();
        }
        return vec![system(world, format!("Player {} was not found", args[0]))];
    }
    let Ok(amount) = args[0].parse::<u32>() else {
        return Vec::new();
    };
    let total = {
        let mut gm = world.resource_mut::<GmRuntimeResource>();
        gm.pearls = gm.pearls.saturating_add(amount);
        gm.pearls
    };
    let name = player_name(world);
    vec![system(
        world,
        format!("Player {name} has been given {amount} pearl(s) by GM: {name} (total {total})"),
    )]
}

fn gm_adjust_pk_point(world: &mut World, args: &[&str]) -> Vec<ServerPacket> {
    if !is_gm_or_test(world) || args.is_empty() {
        return Vec::new();
    }
    // `@ADJUSTPKPOINT <player> <points>` — cross-player not addressable.
    if args.len() >= 2 {
        if !is_gm(world) {
            return Vec::new();
        }
        return vec![system(world, format!("Player {} was not found", args[0]))];
    }
    let Ok(points) = args[0].parse::<i32>() else {
        return Vec::new();
    };
    world.resource_mut::<PlayerRuntimeResource>().pk_points = points;
    vec![hint(world, format!("PK points set to {points}"))]
}

// ---------------------------------------------------------------------------
// Items / inventory / storage
// ---------------------------------------------------------------------------

fn gm_make(world: &mut World, args: &[&str]) -> Vec<ServerPacket> {
    if !is_gm_or_test(world) || args.is_empty() {
        return Vec::new();
    }
    let template = match args[0].parse::<i32>() {
        Ok(index) => crystal_item_by_index(index),
        Err(_) => crystal_item_by_name(args[0]),
    };
    let Some(template) = template else {
        return Vec::new();
    };
    let count: u32 = args
        .get(1)
        .and_then(|value| value.parse::<u32>().ok())
        .filter(|count| *count > 0)
        .unwrap_or(1);

    let key = crystal_item_key_for_template(&template);
    let durability = (template.durability > 0).then_some(template.durability);
    let gained = add_or_increment_item_with_durability_and_stats(
        world,
        ItemContainer::Bag1,
        &key,
        &template.name,
        template
            .tooltip
            .as_deref()
            .unwrap_or("Item created by a GM."),
        0,
        count,
        u16::from(template.weight.max(1)),
        durability,
        durability,
        0,
        0,
    );
    vec![
        ServerPacket::GainedItem {
            item: user_item_from_item_state(&gained),
        },
        system(
            world,
            format!("{} x{count} has been created.", template.name),
        ),
    ]
}

fn gm_clear_bag(world: &mut World) -> Vec<ServerPacket> {
    if !is_gm_or_test(world) {
        return Vec::new();
    }
    let cleared: Vec<(u64, u32)> = {
        let mut inventory = world.resource_mut::<InventoryResource>();
        let cleared = inventory
            .inventory_items
            .iter()
            .map(|item| (item.unique_id, item.quantity))
            .collect();
        inventory.inventory_items.clear();
        cleared
    };
    cleared
        .into_iter()
        .map(|(unique_id, count)| ServerPacket::DeleteItem {
            unique_id,
            count: u16::try_from(count).unwrap_or(u16::MAX),
        })
        .collect()
}

fn gm_add_inventory(world: &mut World) -> Vec<ServerPacket> {
    // Crystal `@ADDINVENTORY` opens the next four-slot block for a gold fee. The
    // simulation models a fixed inventory, so report the matching feedback.
    vec![system(world, "Inventory size increased.")]
}

fn gm_add_storage(world: &mut World) -> Vec<ServerPacket> {
    expand_storage_rental_impl(world)
}

// ---------------------------------------------------------------------------
// Skills / magic
// ---------------------------------------------------------------------------

fn gm_give_skill(world: &mut World, args: &[&str]) -> Vec<ServerPacket> {
    if !is_gm_or_test(world) || args.len() < 2 {
        return Vec::new();
    }
    // `@GIVESKILL <player> <spell> <level>` (cross-player) vs `<spell> <level>`.
    if args.len() > 2 {
        if !is_gm(world) {
            return Vec::new();
        }
        return vec![system(world, format!("Player {} was not found", args[0]))];
    }
    let Some(spell) = Spell::from_crystal_name(args[0]) else {
        return Vec::new();
    };
    if spell == Spell::None {
        return Vec::new();
    }
    let level = args[1].parse::<u8>().unwrap_or(0).min(3);
    let spell_name = format!("{spell:?}");
    let Some(skill_key) = skill_key_for_crystal_spell(spell) else {
        return Vec::new();
    };

    let known_index = world
        .resource::<SkillResource>()
        .skills
        .iter()
        .position(|skill| skill.key == skill_key);

    if let Some(index) = known_index {
        {
            let mut skills = world.resource_mut::<SkillResource>();
            skills.skills[index].level = level;
        }
        let object_id = current_player_object_id(world).unwrap_or(0);
        return vec![
            ServerPacket::MagicLeveled {
                object_id,
                spell,
                level,
                experience: 0,
            },
            hint(
                world,
                format!("Spell {spell_name} changed to level {level}"),
            ),
        ];
    }

    let Some(skill) = crystal_skill_state(&spell_name, level) else {
        return Vec::new();
    };
    world
        .resource_mut::<SkillResource>()
        .skills
        .push(skill.clone());
    let mut packets = Vec::new();
    if let Some(magic) = client_magic_for_skill_state(&skill, runtime_tick(world)) {
        packets.push(ServerPacket::NewMagic { magic, hero: false });
    }
    packets.push(hint(
        world,
        format!("You have learned {spell_name} at level {level}"),
    ));
    packets
}

fn gm_delete_skill(world: &mut World, args: &[&str]) -> Vec<ServerPacket> {
    if !is_gm(world) || args.is_empty() {
        return Vec::new();
    }
    // `@DELETESKILL <player> <spell>` vs `<spell>`.
    if args.len() > 1 {
        return vec![system(world, format!("Player {} was not found", args[0]))];
    }
    let Some(spell) = Spell::from_crystal_name(args[0]) else {
        return Vec::new();
    };
    if spell == Spell::None {
        return Vec::new();
    }
    let Some(skill_key) = skill_key_for_crystal_spell(spell) else {
        return Vec::new();
    };
    let name = player_name(world);
    let removed_index = {
        let mut skills = world.resource_mut::<SkillResource>();
        if let Some(index) = skills
            .skills
            .iter()
            .position(|skill| skill.key == skill_key)
        {
            skills.skills.remove(index);
            Some(index)
        } else {
            None
        }
    };
    match removed_index {
        Some(index) => vec![
            ServerPacket::RemoveMagic {
                place_id: index as i32,
            },
            hint(
                world,
                format!("You have deleted skill {spell:?} from player {name}"),
            ),
        ],
        None => vec![hint(world, "Unable to delete skill, it was not found")],
    }
}

// ---------------------------------------------------------------------------
// Buffs / transform
// ---------------------------------------------------------------------------

fn gm_clear_buffs(world: &mut World) -> Vec<ServerPacket> {
    let object_id = current_player_object_id(world).unwrap_or(0);
    let cleared: Vec<u8> = {
        let mut buffs = world.resource_mut::<BuffResource>();
        let cleared = buffs
            .buffs
            .iter()
            .filter_map(|buff| crystal_buff_type_for_key(&buff.key))
            .collect();
        buffs.buffs.clear();
        cleared
    };
    cleared
        .into_iter()
        .map(|buff_type| ServerPacket::RemoveBuff {
            buff_type,
            object_id,
        })
        .collect()
}

fn gm_toggle_transform(world: &mut World) -> Vec<ServerPacket> {
    // Crystal `@TOGGLETRANSFORM` pauses/unpauses the active Transform buff (and
    // RefreshStats). Without a Transform buff there is nothing to toggle (Crystal's
    // no-op `break`). The paused state is held in GmRuntimeResource and frozen
    // against expiry by tick_buffs while paused.
    let object_id = current_player_object_id(world).unwrap_or(0);
    let Some(buff_type) = world
        .resource::<BuffResource>()
        .buffs
        .iter()
        .find(|buff| buff.key.contains("transform"))
        .and_then(|buff| crystal_buff_type_for_key(&buff.key))
    else {
        return Vec::new();
    };
    let paused = {
        let mut gm = world.resource_mut::<GmRuntimeResource>();
        gm.transform_paused = !gm.transform_paused;
        gm.transform_paused
    };
    vec![
        ServerPacket::PauseBuff {
            buff_type,
            object_id,
            paused,
        },
        hint(
            world,
            if paused {
                "Transform Disabled."
            } else {
                "Transform Enabled."
            },
        ),
    ]
}

// ---------------------------------------------------------------------------
// Movement / maps
// ---------------------------------------------------------------------------

fn gm_move(world: &mut World, args: &[&str]) -> Vec<ServerPacket> {
    // Crystal: `IsGM || Teleport special item || TestServer`.
    if !is_gm_or_test(world) {
        return Vec::new();
    }
    let (Some(x), Some(y)) = (
        args.first().and_then(|value| value.parse::<i32>().ok()),
        args.get(1).and_then(|value| value.parse::<i32>().ok()),
    ) else {
        // Crystal falls back to `TeleportRandom`; we keep the player in place and
        // re-broadcast position rather than risk an off-map random jump.
        return vec![ServerPacket::UserLocation {
            location: current_location(world),
        }];
    };
    teleport_to(world, Point { x, y });
    vec![
        ServerPacket::UserLocation {
            location: current_location(world),
        },
        system(world, format!("Moved to ({x},{y})")),
    ]
}

fn gm_map_move(world: &mut World, args: &[&str]) -> Vec<ServerPacket> {
    if !is_gm_or_test(world) || args.is_empty() {
        return Vec::new();
    }
    let instance_id: i32 = args
        .get(1)
        .and_then(|v| v.parse::<i32>().ok())
        .unwrap_or(1)
        .max(1);
    let current_file = world
        .resource::<MapRuntimeResource>()
        .current_map
        .file_name
        .clone();
    // The single-session world only has the current map loaded; a request for any
    // other map is Crystal's "could not be found". Same-map requests teleport.
    if !args[0].eq_ignore_ascii_case(&current_file) {
        return vec![system(
            world,
            format!("Map {}:[{instance_id}] could not be found", args[0]),
        )];
    }
    if args.len() >= 4 {
        if let (Some(x), Some(y)) = (
            args[args.len() - 2].parse::<i32>().ok(),
            args[args.len() - 1].parse::<i32>().ok(),
        ) {
            teleport_to(world, Point { x, y });
        }
    }
    vec![
        ServerPacket::UserLocation {
            location: current_location(world),
        },
        system(world, format!("Moved to Map {current_file}")),
    ]
}

fn gm_goto(world: &mut World, args: &[&str]) -> Vec<ServerPacket> {
    if !is_gm(world) || args.is_empty() {
        return Vec::new();
    }
    // No other online player to teleport to.
    Vec::new()
}

fn teleport_to(world: &mut World, destination: Point) {
    if let Some(player) = player_entity(world) {
        world
            .entity_mut(player)
            .insert(Position(destination.clone()));
    }
    world
        .resource_mut::<PlayerRuntimeResource>()
        .player_position = destination;
}

// ---------------------------------------------------------------------------
// Monsters
// ---------------------------------------------------------------------------

fn gm_mob(world: &mut World, args: &[&str]) -> Vec<ServerPacket> {
    if !is_gm_or_test(world) {
        return Vec::new();
    }
    if args.is_empty() {
        return vec![system(world, "Not enough parameters to spawn monster")];
    }
    let Some(template) = crystal_dynamic_monster_template(args[0]) else {
        return vec![system(world, format!("Monster {} does not exist", args[0]))];
    };
    if CONQUEST_SPAWN_BLOCKED_AIS.contains(&template.monster_ai) {
        return vec![system(
            world,
            format!("Cannot spawn the conquest item {}", args[0]),
        )];
    }
    let count: u32 = if is_gm(world) {
        args.get(1).and_then(|v| v.parse::<u32>().ok()).unwrap_or(1)
    } else {
        1
    };

    let Some(player) = player_entity(world) else {
        return Vec::new();
    };
    let Some(origin) = entity_position(world, player) else {
        return Vec::new();
    };
    let Some(object_id) = current_player_object_id(world) else {
        return Vec::new();
    };
    let direction = world.resource::<PlayerRuntimeResource>().player_direction;

    for _ in 0..count {
        let _ = spawn_runtime_monster(
            world,
            &template,
            origin.clone(),
            direction,
            None,
            // The runtime only spawns on a static spawn-point or an already-occupied
            // cell; spawning on the GM's own (occupied) cell passes validation. The
            // hostile overrides make it a wild aggressor rather than a pet.
            Some(SummonedMonster {
                summoner_object_id: object_id,
                visible_extra: false,
                expire_tick: None,
                require_summoner_within: None,
                despawn_tick_after_death: None,
                totem_master_object_id: None,
                max_minions: None,
            }),
            Some(true),
            Some(WorldEntityDisposition::Hostile),
            0,
        );
    }
    vec![system(
        world,
        format!(
            "Monster {} x{count} has been spawned.",
            template.monster_name
        ),
    )]
}

fn gm_recall_mob(world: &mut World, args: &[&str]) -> Vec<ServerPacket> {
    if !is_gm_or_test(world) || args.is_empty() {
        return Vec::new();
    }
    let Some(template) = crystal_dynamic_monster_template(args[0]) else {
        return Vec::new();
    };
    if CONQUEST_SPAWN_BLOCKED_AIS.contains(&template.monster_ai) {
        return vec![system(
            world,
            format!("Cannot spawn the conquest item {}", args[0]),
        )];
    }
    let count: u32 = args
        .get(1)
        .and_then(|v| v.parse::<u32>().ok())
        .filter(|count| *count <= 50)
        .unwrap_or(1);

    let Some(player) = player_entity(world) else {
        return Vec::new();
    };
    let Some(origin) = entity_position(world, player) else {
        return Vec::new();
    };
    let Some(object_id) = current_player_object_id(world) else {
        return Vec::new();
    };
    let direction = world.resource::<PlayerRuntimeResource>().player_direction;

    for _ in 0..count {
        let _ = spawn_runtime_monster(
            world,
            &template,
            origin.clone(),
            direction,
            None,
            Some(SummonedMonster {
                summoner_object_id: object_id,
                visible_extra: false,
                expire_tick: None,
                require_summoner_within: None,
                despawn_tick_after_death: None,
                totem_master_object_id: None,
                max_minions: Some(50),
            }),
            Some(false),
            Some(WorldEntityDisposition::Friendly),
            0,
        );
    }
    vec![system(
        world,
        format!("Pet {} x{count} has been recalled.", template.monster_name),
    )]
}

fn gm_kill(world: &mut World, args: &[&str]) -> Vec<ServerPacket> {
    if !is_gm(world) {
        return Vec::new();
    }
    // `@KILL <player>` has no other player to target. The no-arg form kills the
    // monsters in the cell in front of the GM.
    if !args.is_empty() {
        return vec![system(world, format!("Could not find {}.", args[0]))];
    }
    let Some(player) = player_entity(world) else {
        return Vec::new();
    };
    let Some(position) = entity_position(world, player) else {
        return Vec::new();
    };
    let direction = world.resource::<PlayerRuntimeResource>().player_direction;
    let front = offset_point(&position, direction, 1);
    let targets = monster_entities_at(world, &front);
    let mut packets = Vec::new();
    for entity in targets {
        if let Some(packet) = kill_monster_entity(world, entity) {
            packets.push(packet);
        }
    }
    packets
}

fn gm_clear_mob(world: &mut World, args: &[&str]) -> Vec<ServerPacket> {
    if !is_gm(world) {
        return Vec::new();
    }
    // `@CLEARMOB <map>` only matches the single loaded map; an unknown map name
    // is Crystal's silent `return` after a failed map lookup.
    if let Some(map) = args.first() {
        let current_file = world
            .resource::<MapRuntimeResource>()
            .current_map
            .file_name
            .clone();
        if !map.eq_ignore_ascii_case(&current_file) {
            return Vec::new();
        }
    }
    let entities: Vec<Entity> = world
        .query_filtered::<Entity, bevy_ecs::query::With<super::components::Monster>>()
        .iter(world)
        .collect();
    let mut packets = Vec::new();
    for entity in entities {
        if let Some(packet) = kill_monster_entity(world, entity) {
            packets.push(packet);
        }
    }
    packets
}

fn monster_entities_at(world: &mut World, point: &Point) -> Vec<Entity> {
    world
        .query_filtered::<(Entity, &Position), bevy_ecs::query::With<super::components::Monster>>()
        .iter(world)
        .filter(|(_, position)| position.0 == *point)
        .map(|(entity, _)| entity)
        .collect()
}

/// Mirror Crystal's `MonsterObject.Die()` minimally: drop the monster to a corpse
/// (HP 0, `dead`) and broadcast its death. The runtime's existing death/cleanup
/// handles removal; GM kills, like Crystal's `@CLEARMOB`, intentionally carry no
/// EXP owner.
fn kill_monster_entity(world: &mut World, entity: Entity) -> Option<ServerPacket> {
    let agent = world.entity(entity).get::<MonsterAgent>()?.clone();
    if agent.dead {
        return None;
    }
    let max_hp = world
        .entity(entity)
        .get::<MonsterVitals>()
        .map(|vitals| vitals.max_hp)
        .unwrap_or(1);
    let facing = world
        .entity(entity)
        .get::<Facing>()
        .map(|facing| facing.0)
        .unwrap_or(MirDirection::Up);
    {
        let mut entry = world.entity_mut(entity);
        let mut agent = agent;
        agent.dead = true;
        entry.insert((MonsterVitals { hp: 0, max_hp }, agent, Facing(facing)));
    }
    object_died_info_for_entity(world, entity, 0).map(|info| ServerPacket::ObjectDied { info })
}

// ---------------------------------------------------------------------------
// Death / revive / vitals
// ---------------------------------------------------------------------------

fn gm_die(world: &mut World) -> Vec<ServerPacket> {
    // `@DIE` is ungated in Crystal and calls `Die()` directly — it does not honour
    // `GMNeverDie` (only the Revival special item short-circuits death).
    let Some(player) = player_entity(world) else {
        return Vec::new();
    };
    if current_player_is_dead(world) {
        return Vec::new();
    }
    let dead = {
        let mut entity = world.entity_mut(player);
        match entity.get_mut::<PlayerVitals>() {
            Some(mut vitals) => {
                vitals.hp = 0;
                Some(*vitals)
            }
            None => None,
        }
    };
    if let Some(dead) = dead {
        world.resource_mut::<PlayerRuntimeResource>().player_vitals = dead;
    }
    let mut packets = Vec::new();
    if let Some(info) = object_died_info_for_entity(world, player, 0) {
        packets.push(ServerPacket::ObjectDied { info });
    }
    packets
}

fn gm_revive(world: &mut World, args: &[&str]) -> Vec<ServerPacket> {
    if !is_gm(world) {
        return Vec::new();
    }
    if !args.is_empty() {
        return vec![system(world, format!("Player {} was not found", args[0]))];
    }
    let Some(player) = player_entity(world) else {
        return Vec::new();
    };
    let revived = {
        let mut entity = world.entity_mut(player);
        let Some(mut vitals) = entity.get_mut::<PlayerVitals>() else {
            return Vec::new();
        };
        vitals.hp = vitals.max_hp.max(1);
        vitals.mp = vitals.max_mp;
        *vitals
    };
    world.resource_mut::<PlayerRuntimeResource>().player_vitals = revived;
    let mut packets = Vec::new();
    if let Some(info) = super::packets::object_revived_info_for_entity(world, player, true) {
        packets.push(ServerPacket::ObjectRevived { info });
    }
    if let Some(info) = object_health_info_for_entity(world, player, 0) {
        packets.push(ServerPacket::ObjectHealth { info });
    }
    packets
}

// ---------------------------------------------------------------------------
// Appearance / identity
// ---------------------------------------------------------------------------

fn gm_change_gender(world: &mut World, args: &[&str]) -> Vec<ServerPacket> {
    if !is_gm_or_test(world) {
        return Vec::new();
    }
    // Cross-player form addresses another character; single session targets self.
    if !args.is_empty() && is_gm(world) && !args[0].eq_ignore_ascii_case(&player_name(world)) {
        return vec![system(world, format!("Player {} was not found", args[0]))];
    }
    let new_gender = {
        let mut session = world.resource_mut::<SessionResource>();
        let Some(character) = session.selected_character.as_mut() else {
            return Vec::new();
        };
        character.gender = match character.gender {
            MirGender::Male => MirGender::Female,
            MirGender::Female => MirGender::Male,
        };
        character.gender
    };
    let name = player_name(world);
    vec![system(
        world,
        format!("Player {name} has been changed to {new_gender:?}"),
    )]
}

fn gm_change_class(world: &mut World, args: &[&str]) -> Vec<ServerPacket> {
    if !is_gm_or_test(world) || args.is_empty() {
        return Vec::new();
    }
    let Some(class) = parse_class(args[args.len() - 1]) else {
        return Vec::new();
    };
    let changed = {
        let mut session = world.resource_mut::<SessionResource>();
        let Some(character) = session.selected_character.as_mut() else {
            return Vec::new();
        };
        if character.class == class {
            return Vec::new();
        }
        character.class = class;
        true
    };
    if changed {
        if let Some(player) = player_entity(world) {
            if let Some(mut body) = world
                .entity_mut(player)
                .get_mut::<super::components::CharacterBody>()
            {
                body.class = class;
            }
        }
    }
    let name = player_name(world);
    vec![system(
        world,
        format!("Player {name} has been changed to {class:?}"),
    )]
}

fn parse_class(value: &str) -> Option<MirClass> {
    match value.to_ascii_uppercase().as_str() {
        "WARRIOR" => Some(MirClass::Warrior),
        "WIZARD" => Some(MirClass::Wizard),
        "TAOIST" => Some(MirClass::Taoist),
        "ASSASSIN" => Some(MirClass::Assassin),
        "ARCHER" => Some(MirClass::Archer),
        _ => None,
    }
}

fn gm_hair(world: &mut World, args: &[&str]) -> Vec<ServerPacket> {
    if !is_gm_or_test(world) {
        return Vec::new();
    }
    // Crystal `Info.Hair = value` (or `Random.Next(0,9)` with no arg) — set on the
    // character with no immediate packet; it shows on the next appearance refresh.
    let hair = match args.first() {
        Some(value) => value.parse::<u8>().unwrap_or(0),
        None => (runtime_tick(world) % 9) as u8,
    };
    world
        .resource_mut::<super::resources::Stage5SystemsResource>()
        .stage5_systems
        .appearance
        .hair = hair;
    Vec::new()
}

fn gm_deco(world: &mut World, args: &[&str]) -> Vec<ServerPacket> {
    if !is_gm_or_test(world) || args.is_empty() {
        return Vec::new();
    }
    let Ok(image) = args[0].parse::<i32>() else {
        return Vec::new();
    };
    let location = current_location(world).position;
    let object_id = super::monsters::allocate_runtime_monster_object_id(world);
    vec![ServerPacket::ObjectDeco {
        object_id,
        location,
        image,
    }]
}

fn gm_set_light(world: &mut World, args: &[&str]) -> Vec<ServerPacket> {
    if !is_gm(world) || args.is_empty() {
        return Vec::new();
    }
    let Ok(light) = args[0].parse::<u8>() else {
        return Vec::new();
    };
    world.resource_mut::<GmRuntimeResource>().light = light;
    // Crystal `Light = light; Enqueue/Broadcast(GetUpdateInfo())` — a PlayerUpdate
    // carrying the new personal light radius.
    super::packets::self_player_update_packet(world, light)
        .into_iter()
        .collect()
}

// ---------------------------------------------------------------------------
// Story flags
// ---------------------------------------------------------------------------

fn gm_set_flag(world: &mut World, args: &[&str]) -> Vec<ServerPacket> {
    if !is_gm_or_test(world) || args.is_empty() {
        return Vec::new();
    }
    let Ok(index) = args[0].parse::<u32>() else {
        return Vec::new();
    };
    let mut npc_state = world.resource_mut::<NpcStateResource>();
    if let Some(flag) = npc_state
        .npc_flags
        .iter_mut()
        .find(|flag| flag.index == index)
    {
        flag.value = !flag.value;
    } else {
        npc_state
            .npc_flags
            .push(super::npc::NpcFlagState { index, value: true });
    }
    Vec::new()
}

fn gm_list_flags(world: &World) -> Vec<ServerPacket> {
    if !is_gm_or_test(world) {
        return Vec::new();
    }
    let flags: Vec<u32> = world
        .resource::<NpcStateResource>()
        .npc_flags
        .iter()
        .filter(|flag| flag.value)
        .map(|flag| flag.index)
        .collect();
    flags
        .into_iter()
        .map(|index| hint(world, format!("Flag {index}")))
        .collect()
}

fn gm_clear_flags(world: &mut World, args: &[&str]) -> Vec<ServerPacket> {
    if !is_gm_or_test(world) {
        return Vec::new();
    }
    // `@CLEARFLAGS <player>` targets another character (none online here).
    if !args.is_empty() && is_gm(world) && !args[0].eq_ignore_ascii_case(&player_name(world)) {
        return vec![system(world, format!("{} is not online.", args[0]))];
    }
    for flag in world
        .resource_mut::<NpcStateResource>()
        .npc_flags
        .iter_mut()
    {
        flag.value = false;
    }
    Vec::new()
}

// ---------------------------------------------------------------------------
// Quests
// ---------------------------------------------------------------------------

fn gm_clear_quests(world: &mut World, args: &[&str]) -> Vec<ServerPacket> {
    if !is_gm_or_test(world) {
        return Vec::new();
    }
    if !args.is_empty() && is_gm(world) && !args[0].eq_ignore_ascii_case(&player_name(world)) {
        return vec![system(world, format!("{} is not online.", args[0]))];
    }
    world.resource_mut::<QuestResource>().quests.clear();
    Vec::new()
}

fn gm_set_quest(world: &mut World, args: &[&str]) -> Vec<ServerPacket> {
    if !is_gm_or_test(world) || args.len() < 2 {
        return Vec::new();
    }
    // `@SETQUEST <id> <state> [player]` — the optional 4th arg names another player.
    if args.len() > 2 && is_gm(world) && !args[2].eq_ignore_ascii_case(&player_name(world)) {
        return vec![system(world, format!("{} is not online.", args[2]))];
    }
    let Ok(quest_id) = args[0].parse::<i32>() else {
        return Vec::new();
    };
    let Ok(state) = args[1].parse::<i32>() else {
        return Vec::new();
    };
    if quest_id < 1 {
        return Vec::new();
    }
    let mut quests = world.resource_mut::<QuestResource>();
    quests.quests.retain(|quest| quest.quest_id != quest_id);
    match state {
        1 => {
            // Mark complete.
            quests.quests.push(QuestStateMarker::completed(quest_id));
        }
        _ => {
            // 0 = cancel: removal above already cleared it.
        }
    }
    Vec::new()
}

/// Helper to build a minimal completed [`super::quests::QuestState`] for
/// `@SETQUEST <id> 1`.
struct QuestStateMarker;

impl QuestStateMarker {
    fn completed(quest_id: i32) -> super::quests::QuestState {
        super::quests::QuestState {
            quest_id,
            title: format!("Quest {quest_id}"),
            summary: String::new(),
            reward_preview: String::new(),
            required: 0,
            current: 0,
            stage: QuestStage::Completed,
            task_progress: Default::default(),
        }
    }
}

// ---------------------------------------------------------------------------
// Hero / mount
// ---------------------------------------------------------------------------

fn gm_summon_hero(world: &mut World) -> Vec<ServerPacket> {
    // Crystal requires `HasHero`; toggles spawn/despawn. With no hero owned this
    // is a no-op, and an already-spawned hero is left in place.
    let spawned = match world
        .resource::<super::resources::Stage5SystemsResource>()
        .stage5_systems
        .hero
        .as_ref()
    {
        Some(hero) => hero.spawned,
        None => return Vec::new(),
    };
    if spawned {
        return Vec::new();
    }
    let _ = super::map::spawn_stage5_hero(world);
    Vec::new()
}

fn gm_ride(world: &mut World) -> Vec<ServerPacket> {
    // Crystal `ToggleRide()` mounts/dismounts the equipped mount and broadcasts the
    // new state; a no-op when no mount is equipped.
    super::equipment::gm_toggle_ride(world)
}

// ---------------------------------------------------------------------------
// Informational
// ---------------------------------------------------------------------------

fn gm_time(world: &World) -> Vec<ServerPacket> {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    let hour = (secs / 3600) % 24;
    let minute = (secs / 60) % 60;
    vec![system(
        world,
        format!("The time is : {hour:02}:{minute:02}"),
    )]
}

fn gm_map(world: &World) -> Vec<ServerPacket> {
    let map = world.resource::<MapRuntimeResource>();
    let file_name = map.current_map.file_name.clone();
    let title = map.current_map.title.clone();
    vec![system(
        world,
        format!("You are currently in {title}. Map ID: {file_name}"),
    )]
}

fn gm_roll(world: &World) -> Vec<ServerPacket> {
    // `Random.Next(5) + 1` → 1..=5.
    let dice = (runtime_tick(world) % 5 + 1) as u8;
    let name = player_name(world);
    vec![gm_chat(
        world,
        format!("{name} has rolled a {dice}"),
        ChatType::Group,
    )]
}

fn gm_info(world: &mut World, args: &[&str]) -> Vec<ServerPacket> {
    if !is_gm_or_test(world) {
        return Vec::new();
    }
    // `@INFO <player>` resolves another player by name; with nobody else online
    // we fall through to inspecting the object in front of the GM.
    if !args.is_empty() {
        if args[0].eq_ignore_ascii_case(&player_name(world)) {
            return player_info_packets(world);
        }
        return Vec::new();
    }
    let Some(player) = player_entity(world) else {
        return Vec::new();
    };
    let Some(position) = entity_position(world, player) else {
        return Vec::new();
    };
    let direction = world.resource::<PlayerRuntimeResource>().player_direction;
    let front = offset_point(&position, direction, 1);
    let target = monster_entities_at(world, &front).into_iter().next();
    match target {
        Some(entity) => monster_info_packets(world, entity),
        None => Vec::new(),
    }
}

fn player_info_packets(world: &World) -> Vec<ServerPacket> {
    let name = player_name(world);
    let level = world
        .resource::<SessionResource>()
        .selected_character
        .as_ref()
        .map(|character| character.level)
        .unwrap_or(0);
    let location = current_location(world).position;
    vec![
        system2(world, "--Player Info--"),
        system2(
            world,
            format!(
                "Name : {name}, Level : {level}, X : {}, Y : {}",
                location.x, location.y
            ),
        ),
    ]
}

fn monster_info_packets(world: &mut World, entity: Entity) -> Vec<ServerPacket> {
    let name = entity_name(world, entity).unwrap_or_else(|| "Monster".to_string());
    let position = entity_position(world, entity).unwrap_or(Point { x: 0, y: 0 });
    let (hp, max_hp) = world
        .entity(entity)
        .get::<MonsterVitals>()
        .map(|vitals| (vitals.hp, vitals.max_hp))
        .unwrap_or((0, 0));
    vec![
        system2(world, "--Monster Info--"),
        system2(world, format!("Name : {name}")),
        system2(
            world,
            format!("X : {}, Y : {}, HP : {hp}/{max_hp}", position.x, position.y),
        ),
    ]
}

// ---------------------------------------------------------------------------
// Cross-player / probe (no other players in a single session)
// ---------------------------------------------------------------------------

fn gm_recall(world: &World, args: &[&str]) -> Vec<ServerPacket> {
    if !is_gm(world) || args.is_empty() {
        return Vec::new();
    }
    Vec::new()
}

fn gm_observe(_world: &World, args: &[&str]) -> Vec<ServerPacket> {
    if args.is_empty() {
        return Vec::new();
    }
    // No other online player to observe.
    Vec::new()
}

fn gm_find(world: &World, args: &[&str]) -> Vec<ServerPacket> {
    if !is_gm(world) || args.is_empty() {
        return Vec::new();
    }
    vec![system(world, format!("{} is not online.", args[0]))]
}

fn gm_recall_member(world: &World, args: &[&str]) -> Vec<ServerPacket> {
    // Crystal requires the caller to be the group leader; with no group the
    // leadership guard fails first.
    let _ = args;
    vec![system(world, "You are not the group leader.")]
}

fn gm_group_recall(world: &World) -> Vec<ServerPacket> {
    // Requires the caller to be the leader of a non-empty group; ungrouped → no-op.
    let _ = world;
    Vec::new()
}

fn gm_recall_lover(world: &World) -> Vec<ServerPacket> {
    vec![system(world, "You're not married.")]
}

fn gm_player_archive(world: &World, command: &str, args: &[&str]) -> Vec<ServerPacket> {
    if !is_gm(world) || args.is_empty() {
        return Vec::new();
    }
    let _ = command;
    vec![system(world, format!("Player {} was not found", args[0]))]
}

// ---------------------------------------------------------------------------
// Guild / conquest (no guild membership in a single session)
// ---------------------------------------------------------------------------

fn gm_leave_guild(world: &World) -> Vec<ServerPacket> {
    // Crystal `return`s silently when `MyGuild == null`.
    let _ = world;
    Vec::new()
}

fn gm_create_guild(world: &World, args: &[&str]) -> Vec<ServerPacket> {
    if !is_gm_or_test(world) || args.is_empty() {
        return Vec::new();
    }
    let guild_name = if args.len() < 2 { args[0] } else { args[1] };
    if guild_name.len() < 3 || guild_name.len() > 20 {
        return vec![system(
            world,
            "Guild name must be between 3 and 20 characters.",
        )];
    }
    vec![system(
        world,
        format!("Successfully created guild {guild_name}"),
    )]
}

fn gm_start_war(world: &World, args: &[&str]) -> Vec<ServerPacket> {
    if !is_gm(world) || args.is_empty() {
        return Vec::new();
    }
    vec![system(world, "You are not in a guild.")]
}

fn gm_start_conquest(world: &World, args: &[&str]) -> Vec<ServerPacket> {
    if !is_gm_or_test(world) || args.is_empty() {
        return Vec::new();
    }
    vec![system(world, "You need to be in a guild to start a War")]
}

fn gm_reset_conquest(world: &World, args: &[&str]) -> Vec<ServerPacket> {
    if !is_gm_or_test(world) || args.is_empty() {
        return Vec::new();
    }
    vec![system(world, "You need to be in a guild to start a War")]
}

fn gm_gates(world: &World, args: &[&str]) -> Vec<ServerPacket> {
    let _ = args;
    vec![system(
        world,
        "You don't have access to control any gates at the moment.",
    )]
}

fn gm_change_flag(world: &World) -> Vec<ServerPacket> {
    vec![system(
        world,
        "You don't have access to change any flags at the moment.",
    )]
}

fn gm_change_flag_colour(world: &World) -> Vec<ServerPacket> {
    vec![system(
        world,
        "You don't have access to change any flags at the moment.",
    )]
}

// ---------------------------------------------------------------------------
// Awakening (equipment upgrade) — no equipped target in scope
// ---------------------------------------------------------------------------

fn parse_item_type(value: &str) -> Option<u8> {
    Some(match value.to_ascii_uppercase().as_str() {
        "WEAPON" => 1,
        "ARMOUR" | "ARMOR" => 2,
        "HELMET" => 4,
        "NECKLACE" => 5,
        "BRACELET" => 6,
        "RING" => 7,
        "AMULET" => 8,
        "BELT" => 9,
        "BOOTS" => 10,
        "STONE" => 11,
        "TORCH" => 12,
        "MOUNT" => 19,
        _ => return None,
    })
}

fn parse_awake_type(value: &str) -> Option<u8> {
    Some(match value.to_ascii_uppercase().as_str() {
        "DC" => 1,
        "MC" => 2,
        "SC" => 3,
        "AC" => 4,
        "MAC" => 5,
        "HPMP" => 6,
        _ => return None,
    })
}

fn awake_key_seed(key: &str) -> usize {
    key.bytes().fold(0usize, |acc, byte| {
        acc.wrapping_mul(31).wrapping_add(usize::from(byte))
    })
}

/// Crystal `Awake.CheckAwakening` — whether `requested` may be applied to a worn
/// item of `template` currently at `current_type`/`level`.
fn awake_check(
    template: &CrystalItemTemplate,
    current_type: u8,
    level: usize,
    requested: u8,
) -> bool {
    if template.bind & BIND_DONT_UPGRADE != 0 {
        return false;
    }
    if !template.can_awakening {
        return false;
    }
    if template.grade == 0 {
        return false;
    }
    if level >= AWAKE_MAX_LEVEL {
        return false;
    }
    if current_type == 0 {
        match template.item_type {
            1 => matches!(requested, 1 | 2 | 3), // Weapon: DC/MC/SC
            4 => matches!(requested, 4 | 5),     // Helmet: AC/MAC
            2 => requested == 6,                 // Armour: HPMP
            _ => false,
        }
    } else {
        current_type == requested
    }
}

/// Crystal `Awake.Awakening` value roll: `MakeHit` summed over five hits scaled by
/// the item-type rate (Weapon 1, Armour 5, Helmet 1). Deterministic so GM output
/// is reproducible.
fn awake_make_value(tick: u64, template: &CrystalItemTemplate, seed: usize) -> u8 {
    let grade = template.grade.clamp(1, 5);
    let max_value = f32::from(AWAKE_CHANCE_MAX[usize::from(grade - 1)].max(1));
    let step = max_value / 5.0;
    let mut total = 0.0_f32;
    for hit in 0..5 {
        if deterministic_roll(tick, seed, hit, 100) < AWAKE_HIT_RATE {
            total += step;
        }
    }
    let make = if total <= 1.0 { 1 } else { total as i32 };
    let rate = match template.item_type {
        1 => 1,
        2 => 5,
        4 => 1,
        _ => 0,
    };
    u8::try_from((make * rate).max(0)).unwrap_or(u8::MAX)
}

fn gm_awakening(world: &mut World, args: &[&str]) -> Vec<ServerPacket> {
    if !is_gm_or_test(world) || args.len() < 2 {
        return Vec::new();
    }
    let Some(item_type) = parse_item_type(args[0]) else {
        return Vec::new();
    };
    let Some(awake_type) = parse_awake_type(args[1]) else {
        return Vec::new();
    };
    let tick = runtime_tick(world);
    let mut packets = Vec::new();
    let count = world.resource::<InventoryResource>().equipment_items.len();
    for idx in 0..count {
        let (key, current_type, level, name) = {
            let inventory = world.resource::<InventoryResource>();
            let item = &inventory.equipment_items[idx];
            (
                item.key.clone(),
                item.awake_type,
                item.awake_values.len(),
                item.name.clone(),
            )
        };
        let Some(template) = crystal_item_template_for_item_key(&key) else {
            continue;
        };
        if template.item_type != item_type {
            continue;
        }
        if !awake_check(&template, current_type, level, awake_type) {
            packets.push(system(world, format!("{name} : Condition Error.")));
            continue;
        }
        let success =
            deterministic_roll(tick, awake_key_seed(&key), level, 100) <= AWAKE_SUCCESS_RATE;
        if success {
            let value = awake_make_value(tick, &template, awake_key_seed(&key).wrapping_add(level));
            {
                let mut inventory = world.resource_mut::<InventoryResource>();
                let item = &mut inventory.equipment_items[idx];
                item.awake_type = awake_type;
                item.awake_values.push(value);
            }
            let (new_level, new_value) = {
                let inventory = world.resource::<InventoryResource>();
                let item = &inventory.equipment_items[idx];
                (
                    item.awake_values.len(),
                    item.awake_values
                        .iter()
                        .map(|value| u32::from(*value))
                        .sum::<u32>(),
                )
            };
            packets.push(system(
                world,
                format!("{name} : AWAKE Level {new_level}, value {new_value}~{new_value}."),
            ));
            if let Some(user_item) = {
                let inventory = world.resource::<InventoryResource>();
                user_item_from_equipment_state(&inventory.equipment_items[idx])
            } {
                packets.push(ServerPacket::RefreshItem { item: user_item });
            }
        } else {
            // CheckAwakening already fixed the awake type even on a failed roll.
            world.resource_mut::<InventoryResource>().equipment_items[idx].awake_type = awake_type;
            packets.push(system(world, format!("{name} : Upgrade Failed.")));
        }
    }
    packets
}

fn gm_remove_awakening(world: &mut World, args: &[&str]) -> Vec<ServerPacket> {
    if !is_gm_or_test(world) || args.is_empty() {
        return Vec::new();
    }
    let Some(item_type) = parse_item_type(args[0]) else {
        return Vec::new();
    };
    let mut packets = Vec::new();
    let count = world.resource::<InventoryResource>().equipment_items.len();
    for idx in 0..count {
        let (key, name) = {
            let inventory = world.resource::<InventoryResource>();
            let item = &inventory.equipment_items[idx];
            (item.key.clone(), item.name.clone())
        };
        let Some(template) = crystal_item_template_for_item_key(&key) else {
            continue;
        };
        if template.item_type != item_type {
            continue;
        }
        // Crystal `Awake.RemoveAwake`: pop the last level; 0 = nothing to remove.
        let (result, new_level) = {
            let mut inventory = world.resource_mut::<InventoryResource>();
            let item = &mut inventory.equipment_items[idx];
            if item.awake_values.is_empty() {
                item.awake_type = 0;
                (0, 0)
            } else {
                item.awake_values.pop();
                if item.awake_values.is_empty() {
                    item.awake_type = 0;
                }
                (1, item.awake_values.len())
            }
        };
        if result == 0 {
            packets.push(system(world, format!("{name} : Remove failed Level 0")));
        } else {
            packets.push(system(
                world,
                format!("{name} : Remove success. Level {new_level}"),
            ));
            if let Some(user_item) = {
                let inventory = world.resource::<InventoryResource>();
                user_item_from_equipment_state(&inventory.equipment_items[idx])
            } {
                packets.push(ServerPacket::RefreshItem { item: user_item });
            }
        }
    }
    packets
}

// ---------------------------------------------------------------------------
// Server administration (no-op surfaces in the single-session sim)
// ---------------------------------------------------------------------------

fn gm_reload_drops(world: &World) -> Vec<ServerPacket> {
    if !is_gm(world) {
        return Vec::new();
    }
    vec![hint(world, "Drops Reloaded.")]
}

fn gm_reload_npcs(world: &World) -> Vec<ServerPacket> {
    if !is_gm(world) {
        return Vec::new();
    }
    vec![hint(world, "NPC Scripts Reloaded.")]
}

fn gm_clear_ip_blocks(world: &World) -> Vec<ServerPacket> {
    // Crystal clears `Envir.IPBlocks` with no acknowledgement.
    let _ = world;
    Vec::new()
}

fn gm_trigger(world: &World, args: &[&str]) -> Vec<ServerPacket> {
    if !is_gm(world) || args.is_empty() {
        return Vec::new();
    }
    // `@TRIGGER <key> [player]` fires NPC trigger scripts; without a default-NPC
    // trigger bound this is Crystal's no-op fan-out.
    Vec::new()
}

fn gm_set_timer(_world: &World, args: &[&str]) -> Vec<ServerPacket> {
    // `@SETTIMER <key> <seconds> <type>` — ungated in Crystal (`SetTimer`). Sends
    // the client a named countdown via `S.SetTimer`.
    if args.len() < 3 {
        return Vec::new();
    }
    let key = args[0].to_string();
    let Ok(seconds) = args[1].parse::<i32>() else {
        return Vec::new();
    };
    let Ok(timer_type) = args[2].parse::<u8>() else {
        return Vec::new();
    };
    vec![ServerPacket::SetTimer {
        key,
        timer_type,
        seconds,
    }]
}
