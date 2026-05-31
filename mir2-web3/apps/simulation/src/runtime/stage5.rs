use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

use bevy_ecs::prelude::World;
use mir2_game_data::{
    crystal_game_shop_packet_manifest, crystal_item_by_index, format_localized_text,
    localized_text_or_fallback,
};
use mir2_protocol::{ChatType, MirDirection, Point, ServerPacket};

use crate::config::{
    EquipmentSlot, ItemContainer, ItemGrade, Stage5AuctionListing, Stage5GuildState,
    Stage5HeroState, Stage5MailMessage, Stage5TradeState, WorldEntityDisposition,
};

use super::components::{
    entity_by_object_id, entity_position, player_entity, DisplayName, Npc, NpcAgent, ObjectId,
    PlayerVitals, Position, WorldObject,
};
use super::crystal_compat::CRYSTAL_ITEM_SEAL_DELAY_MINUTES;
use super::equipment::{
    damage_equipment_item, equipment_slot_from_stage5_arg, equipment_slot_unique_id,
    equipment_uses_durability,
};
use super::inventory::{
    add_minutes_to_binary_datetime, add_or_increment_item, allocate_item_unique_id,
    binary_datetime_ticks, can_gain_item_quantity, crystal_duration_label_from_seconds,
    crystal_npc_storage_open_packets, current_binary_datetime, find_empty_inventory_item_slot,
    free_bag_slots, future_binary_datetime_minutes, item_heal_values_for_key,
};
use super::items::{
    crystal_equipment_slot_for_item_key, crystal_item_key_for_template,
    crystal_seal_minutes_for_source_item, crystal_socket_slot_limit_for_item_key,
    crystal_socket_source_valid_for_item, item_icon_for_key, ItemState,
};
use super::map::spawn_stage5_hero;
use super::monsters::{
    crystal_dynamic_monster_template, crystal_spawn_candidates_on_map, spawn_runtime_monster,
};
use super::npc::ActiveNpcServiceState;
use super::packets::{object_health_info_for_entity, stage5_guild_request_war_packet};
use super::resources::{
    InventoryResource, MapRuntimeResource, NpcStateResource, PlayerRuntimeResource,
    RuntimeConfigResource, SessionResource, Stage5SystemsResource,
};
use super::session::{current_language, system_message, SimulationSession};
use super::social_economy::{
    stage5_mail_exact_item_slots, stage5_social_add_friend_entry, stage5_trade_item_can_enter,
    Stage5SocialAddResult,
};

pub(super) fn stage5_player_name(world: &World) -> String {
    world
        .resource::<SessionResource>()
        .selected_character
        .as_ref()
        .map(|character| character.name.clone())
        .unwrap_or_else(|| "Scout".to_string())
}

pub(super) fn push_unique(values: &mut Vec<String>, value: String) {
    if !values
        .iter()
        .any(|existing| existing.eq_ignore_ascii_case(&value))
    {
        values.push(value);
    }
}

pub(super) fn unique_strings(values: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut result = Vec::new();
    for value in values {
        push_unique(&mut result, value);
    }
    result
}

fn stage5_guild_rank_is_leader(rank: &str) -> bool {
    let rank = rank.trim();
    rank.is_empty()
        || rank.eq_ignore_ascii_case("Guild Chief")
        || rank.eq_ignore_ascii_case("Leader")
}

fn stage5_guild_permission_key(permission: &str) -> String {
    permission
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn stage5_guild_can_alter_alliance(guild: &Stage5GuildState) -> bool {
    stage5_guild_rank_is_leader(&guild.rank)
        || guild.permissions.iter().any(|permission| {
            matches!(
                stage5_guild_permission_key(permission).as_str(),
                "alteralliance" | "alliance" | "conquest"
            )
        })
}

fn stage5_guild_canonical_alliance_target(
    guild: &Stage5GuildState,
    territory_owner: &str,
    name: &str,
) -> Option<String> {
    let target = name.trim();
    if target.is_empty() {
        return None;
    }
    if guild.name.eq_ignore_ascii_case(target) {
        return Some(guild.name.clone());
    }
    guild
        .known_guilds
        .iter()
        .find(|known| known.eq_ignore_ascii_case(target))
        .cloned()
        .or_else(|| {
            guild
                .allied_guilds
                .iter()
                .find(|ally| ally.eq_ignore_ascii_case(target))
                .cloned()
        })
        .or_else(|| {
            let territory_owner = territory_owner.trim();
            (!territory_owner.is_empty() && territory_owner.eq_ignore_ascii_case(target))
                .then(|| territory_owner.to_string())
        })
}

fn stage5_guild_is_allied(guild: &Stage5GuildState, target: &str) -> bool {
    guild
        .allied_guilds
        .iter()
        .any(|ally| ally.eq_ignore_ascii_case(target))
}

pub(super) fn parse_u32_arg(args: &[String], index: usize) -> Option<u32> {
    args.get(index).and_then(|value| value.parse::<u32>().ok())
}

pub(super) fn game_shop_purchase_details(
    args: Vec<String>,
    credit: bool,
) -> Option<(String, String, u32)> {
    let game_shop_index = args.first()?.parse::<i32>().ok()?;
    let quantity = args
        .get(1)
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(1)
        .clamp(1, 99);
    let product = crystal_game_shop_packet_manifest()
        .items
        .into_iter()
        .find(|item| item.game_shop_index == game_shop_index)?;
    let unit_price = if credit {
        product.credit_price
    } else {
        product.gold_price
    };
    if unit_price == 0 {
        return None;
    }
    let template = crystal_item_by_index(product.item_index)?;
    let item_key = crystal_item_key_for_template(&template);
    let total_price = unit_price.checked_mul(quantity)?;
    Some((item_key, template.name, total_price))
}

pub(super) fn stage5_item_name(key: &str) -> String {
    key.split(['-', '_'])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().chain(chars).collect::<String>(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub(super) fn crystal_npc_asset_id(parts: &[&str]) -> Option<u8> {
    parts
        .get(1)
        .or_else(|| parts.first())
        .and_then(|value| value.parse::<u8>().ok())
}

pub(super) fn normalize_stage5_key(value: &str) -> String {
    value
        .trim()
        .trim_matches('"')
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

pub(super) fn push_unique_u8(values: &mut Vec<u8>, value: u8) {
    if !values.contains(&value) {
        values.push(value);
    }
}

pub(super) fn now_unix_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}

impl SimulationSession {
    pub fn stage5_command(&mut self, action: &str, args: Vec<String>) -> Vec<ServerPacket> {
        let packets = self.stage5_command_impl(action, args);
        self.finalize_packets(packets)
    }

    fn stage5_command_impl(&mut self, action: &str, args: Vec<String>) -> Vec<ServerPacket> {
        match action {
            "group.create" => self.stage5_group_create(args),
            "group.loot" => self.stage5_group_loot(args),
            "group.leave" => self.stage5_group_leave(),
            "guild.create" => self.stage5_guild_create(args),
            "guild.rank" => self.stage5_guild_rank(args),
            "guild.requestWar" => stage5_guild_request_war_packet(self.app.world()),
            "guild.ally" | "guild.alliance" => self.stage5_guild_ally(args),
            "guild.unally" | "guild.endAlliance" => self.stage5_guild_unally(args),
            "guild.chat" => self.stage5_guild_chat(args),
            "social.friend" => self.stage5_social_friend(args),
            "social.unfriend" => self.stage5_social_unfriend(args),
            "social.block" => self.stage5_social_block(args),
            "social.unblock" => self.stage5_social_unblock(args),
            "mail.send" => self.stage5_mail_send(args),
            "mail.claim" => self.stage5_mail_claim(args),
            "mail.delete" => self.stage5_mail_delete(args),
            "trade.start" => self.stage5_trade_start(args),
            "trade.offerGold" => self.stage5_trade_offer_gold(args),
            "trade.offerItem" => self.stage5_trade_offer_item(args),
            "trade.accept" => self.stage5_trade_accept(),
            "trade.cancel" => self.stage5_trade_cancel(),
            "shop.buy" => self.stage5_shop_buy(args),
            "shop.buyCredit" => self.stage5_shop_buy_credit(args),
            "gameShop.buyCredit" => self.game_shop_buy_credit(args),
            "gameShop.buyGold" => self.game_shop_buy_gold(args),
            "auction.list" => self.stage5_auction_list(args),
            "auction.buy" => self.stage5_auction_buy(args),
            "auction.cancel" => self.stage5_auction_cancel(args),
            "conquest.start" => self.stage5_conquest_start(args),
            "conquest.owner" => self.stage5_conquest_owner(args),
            "conquest.end" => self.stage5_conquest_end(args),
            "event.spawn" => self.stage5_event_spawn(args),
            "hero.recruit" => self.stage5_hero_recruit(args),
            "hero.behaviour" => self.stage5_hero_behaviour(args),
            "mine" => self.stage5_mine(args),
            "craft" => self.stage5_craft(args),
            "item.addSocket" => self.stage5_item_add_socket(args),
            "item.seal" => self.stage5_item_seal(args),
            "qa.damageEquipment" => self.stage5_qa_damage_equipment(args),
            "qa.damagePlayer" => self.stage5_qa_damage_player(args),
            "qa.giveItem" => self.stage5_qa_give_item(args),
            "qa.openNpcDialog" => self.stage5_qa_open_npc_dialog(),
            "qa.openStorage" => self.stage5_qa_open_storage(),
            other => {
                let language = current_language(self.app.world());
                vec![system_message(&format_localized_text(
                    language,
                    "server.InvalidPacketReceived",
                    [other.to_string()],
                ))]
            }
        }
    }

    fn stage5_group_create(&mut self, args: Vec<String>) -> Vec<ServerPacket> {
        let member = args
            .first()
            .cloned()
            .unwrap_or_else(|| "Companion".to_string());
        let player_name = stage5_player_name(self.app.world());
        let mut stage5 = self.app.world_mut().resource_mut::<Stage5SystemsResource>();
        stage5.stage5_systems.group.members = unique_strings([player_name, member]);
        Vec::new()
    }

    fn stage5_group_loot(&mut self, args: Vec<String>) -> Vec<ServerPacket> {
        let mode = args.first().cloned().unwrap_or_else(|| "free".to_string());
        self.app
            .world_mut()
            .resource_mut::<Stage5SystemsResource>()
            .stage5_systems
            .group
            .loot_mode = mode.clone();
        Vec::new()
    }

    fn stage5_group_leave(&mut self) -> Vec<ServerPacket> {
        self.app
            .world_mut()
            .resource_mut::<Stage5SystemsResource>()
            .stage5_systems
            .group = Default::default();
        Vec::new()
    }

    fn stage5_guild_create(&mut self, args: Vec<String>) -> Vec<ServerPacket> {
        let name = args
            .first()
            .cloned()
            .unwrap_or_else(|| "Bichon".to_string());
        let language = current_language(self.app.world());
        let player_name = stage5_player_name(self.app.world());
        let mut stage5 = self.app.world_mut().resource_mut::<Stage5SystemsResource>();
        let guild = &mut stage5.stage5_systems.guild;
        guild.name = name.clone();
        guild.members = unique_strings([player_name]);
        guild.rank = "Guild Chief".to_string();
        push_unique(&mut guild.known_guilds, name.clone());
        guild.active_wars.clear();
        guild.active_war_ticks_remaining.clear();
        guild.allied_guilds.clear();
        guild.ally_count = 0;
        guild.alliance_broadcasts.clear();
        guild.war_broadcasts.clear();
        guild.permissions = vec![
            "changeRank".to_string(),
            "recruit".to_string(),
            "kick".to_string(),
            "storeItem".to_string(),
            "retrieveItem".to_string(),
            "alterAlliance".to_string(),
            "changeNotice".to_string(),
            "activateBuff".to_string(),
        ];
        vec![system_message(&format_localized_text(
            language,
            "server.SuccessfullyCreatedGuild",
            [name],
        ))]
    }

    fn stage5_guild_rank(&mut self, args: Vec<String>) -> Vec<ServerPacket> {
        let rank = args
            .first()
            .cloned()
            .unwrap_or_else(|| "Member".to_string());
        self.app
            .world_mut()
            .resource_mut::<Stage5SystemsResource>()
            .stage5_systems
            .guild
            .rank = rank.clone();
        Vec::new()
    }

    fn stage5_guild_ally(&mut self, args: Vec<String>) -> Vec<ServerPacket> {
        self.stage5_guild_alliance_change(args, true)
    }

    fn stage5_guild_unally(&mut self, args: Vec<String>) -> Vec<ServerPacket> {
        self.stage5_guild_alliance_change(args, false)
    }

    fn stage5_guild_alliance_change(
        &mut self,
        args: Vec<String>,
        create: bool,
    ) -> Vec<ServerPacket> {
        let language = current_language(self.app.world());
        let target_name = args.first().map(|name| name.trim()).unwrap_or_default();
        let (guild_name, can_alter, canonical_target, already_allied, at_war) = {
            let resources = self.app.world().resource::<Stage5SystemsResource>();
            let guild = &resources.stage5_systems.guild;
            let guild_name = guild.name.trim().to_string();
            let canonical_target = stage5_guild_canonical_alliance_target(
                guild,
                &resources.stage5_systems.guild_territory.owner,
                target_name,
            );
            let already_allied = canonical_target
                .as_deref()
                .is_some_and(|target| stage5_guild_is_allied(guild, target));
            let at_war = canonical_target.as_deref().is_some_and(|target| {
                guild
                    .active_wars
                    .iter()
                    .any(|war| war.eq_ignore_ascii_case(target))
            });
            (
                guild_name,
                stage5_guild_can_alter_alliance(guild),
                canonical_target,
                already_allied,
                at_war,
            )
        };

        if guild_name.is_empty() {
            return vec![system_message(&localized_text_or_fallback(
                language,
                "server.NotInGuild",
                "server.NotInGuild",
            ))];
        }
        if !can_alter {
            return vec![system_message(&localized_text_or_fallback(
                language,
                "server.NoCorrectGuildRank",
                "server.NoCorrectGuildRank",
            ))];
        }
        let Some(target_name) = canonical_target else {
            return vec![system_message(&format_localized_text(
                language,
                "server.GuildNotFound",
                [target_name.to_string()],
            ))];
        };
        if guild_name.eq_ignore_ascii_case(&target_name) {
            return vec![system_message(&localized_text_or_fallback(
                language,
                "server.CannotWarOwnGuild",
                "server.CannotWarOwnGuild",
            ))];
        }
        if create && at_war {
            return vec![system_message(&localized_text_or_fallback(
                language,
                "server.AlreadyAtWarWithGuild",
                "server.AlreadyAtWarWithGuild",
            ))];
        }
        if create && already_allied {
            return Vec::new();
        }
        if !create && !already_allied {
            return Vec::new();
        }

        let message = if create {
            format!("Alliance formed with {target_name}.")
        } else {
            format!("Alliance ended with {target_name}.")
        };
        {
            let mut resources = self.app.world_mut().resource_mut::<Stage5SystemsResource>();
            let guild = &mut resources.stage5_systems.guild;
            if create {
                push_unique(&mut guild.allied_guilds, target_name);
            } else {
                guild
                    .allied_guilds
                    .retain(|ally| !ally.eq_ignore_ascii_case(&target_name));
            }
            guild.ally_count = u32::try_from(guild.allied_guilds.len()).unwrap_or(u32::MAX);
            guild.alliance_broadcasts.push(message.clone());
        }

        vec![ServerPacket::Chat {
            message,
            chat_type: ChatType::Guild,
        }]
    }

    fn stage5_guild_chat(&mut self, args: Vec<String>) -> Vec<ServerPacket> {
        let message = args.join(" ");
        let text = if message.is_empty() {
            "Guild message".to_string()
        } else {
            message
        };
        let sender = stage5_player_name(self.app.world());
        self.app
            .world_mut()
            .resource_mut::<Stage5SystemsResource>()
            .stage5_systems
            .guild
            .chat_log
            .push(format!("{sender}: {text}"));
        vec![ServerPacket::Chat {
            message: text,
            chat_type: ChatType::Guild,
        }]
    }

    fn stage5_social_friend(&mut self, args: Vec<String>) -> Vec<ServerPacket> {
        let name = args
            .first()
            .cloned()
            .unwrap_or_else(|| "Friend".to_string());
        let player_name = stage5_player_name(self.app.world());
        let result = {
            let mut stage5 = self.app.world_mut().resource_mut::<Stage5SystemsResource>();
            stage5_social_add_friend_entry(
                &mut stage5.stage5_systems.social,
                &player_name,
                &name,
                false,
            )
        };
        self.stage5_social_add_result(result)
    }

    fn stage5_social_add_result(&self, result: Stage5SocialAddResult) -> Vec<ServerPacket> {
        let language = current_language(self.app.world());
        match result {
            Stage5SocialAddResult::Added => Vec::new(),
            Stage5SocialAddResult::EmptyTarget => {
                vec![system_message(&localized_text_or_fallback(
                    language,
                    "server.PlayerDoesNotExist",
                    "server.PlayerDoesNotExist",
                ))]
            }
            Stage5SocialAddResult::SelfTarget => {
                vec![system_message(&localized_text_or_fallback(
                    language,
                    "server.CannotAddYourself",
                    "server.CannotAddYourself",
                ))]
            }
            Stage5SocialAddResult::AlreadyAdded => {
                vec![system_message(&localized_text_or_fallback(
                    language,
                    "server.PlayerAlreadyAdded",
                    "server.PlayerAlreadyAdded",
                ))]
            }
        }
    }

    fn stage5_social_unfriend(&mut self, args: Vec<String>) -> Vec<ServerPacket> {
        let name = args
            .first()
            .cloned()
            .unwrap_or_else(|| "Friend".to_string());
        self.app
            .world_mut()
            .resource_mut::<Stage5SystemsResource>()
            .stage5_systems
            .social
            .friends
            .retain(|friend| !friend.eq_ignore_ascii_case(&name));
        Vec::new()
    }

    fn stage5_social_block(&mut self, args: Vec<String>) -> Vec<ServerPacket> {
        let name = args
            .first()
            .cloned()
            .unwrap_or_else(|| "Blocked".to_string());
        let player_name = stage5_player_name(self.app.world());
        let result = {
            let mut stage5 = self.app.world_mut().resource_mut::<Stage5SystemsResource>();
            stage5_social_add_friend_entry(
                &mut stage5.stage5_systems.social,
                &player_name,
                &name,
                true,
            )
        };
        self.stage5_social_add_result(result)
    }

    fn stage5_social_unblock(&mut self, args: Vec<String>) -> Vec<ServerPacket> {
        let name = args
            .first()
            .cloned()
            .unwrap_or_else(|| "Blocked".to_string());
        self.app
            .world_mut()
            .resource_mut::<Stage5SystemsResource>()
            .stage5_systems
            .social
            .blocked
            .retain(|blocked| !blocked.eq_ignore_ascii_case(&name));
        Vec::new()
    }

    fn stage5_mail_send(&mut self, args: Vec<String>) -> Vec<ServerPacket> {
        let to = args
            .first()
            .cloned()
            .unwrap_or_else(|| stage5_player_name(self.app.world()));
        let subject = args
            .get(1)
            .cloned()
            .unwrap_or_else(|| "Crystal Mail".to_string());
        let body = args
            .get(2)
            .cloned()
            .unwrap_or_else(|| "Message".to_string());
        let gold = args
            .get(3)
            .and_then(|value| value.parse::<u32>().ok())
            .unwrap_or(0);
        let from = stage5_player_name(self.app.world());
        let mut stage5 = self.app.world_mut().resource_mut::<Stage5SystemsResource>();
        let id = stage5
            .stage5_systems
            .mail
            .iter()
            .map(|mail| mail.id)
            .max()
            .unwrap_or(0)
            + 1;
        stage5.stage5_systems.mail.push(Stage5MailMessage {
            id,
            from,
            to,
            subject,
            body,
            gold,
            items: Vec::new(),
            item_states_json: Vec::new(),
            opened: false,
            locked: false,
            claimed: false,
            deleted: false,
        });
        Vec::new()
    }

    fn stage5_mail_claim(&mut self, args: Vec<String>) -> Vec<ServerPacket> {
        let Some(id) = parse_u32_arg(&args, 0) else {
            let language = current_language(self.app.world());
            return vec![system_message(&format_localized_text(
                language,
                "server.InvalidPacketReceived",
                ["mail.claim".to_string()],
            ))];
        };
        let language = current_language(self.app.world());
        let (index, gold, items, item_states_json) = {
            let stage5 = self.app.world().resource::<Stage5SystemsResource>();
            let Some(index) = stage5
                .stage5_systems
                .mail
                .iter()
                .position(|mail| mail.id == id && !mail.deleted)
            else {
                return vec![system_message(&localized_text_or_fallback(
                    language,
                    "server.NotFound",
                    "server.NotFound",
                ))];
            };
            if stage5.stage5_systems.mail[index].claimed {
                return Vec::new();
            }
            (
                index,
                stage5.stage5_systems.mail[index].gold,
                stage5.stage5_systems.mail[index].items.clone(),
                stage5.stage5_systems.mail[index].item_states_json.clone(),
            )
        };
        let item_states = item_states_json
            .iter()
            .filter_map(|state| serde_json::from_str::<ItemState>(state).ok())
            .collect::<Vec<_>>();
        let has_exact_item_states = !item_states.is_empty();
        let keyed_items = if has_exact_item_states {
            Vec::new()
        } else {
            items
        };
        let exact_item_slots;
        {
            let resources = self.app.world().resource::<InventoryResource>();
            exact_item_slots =
                match stage5_mail_exact_item_slots(&resources.inventory_items, &item_states) {
                    Some(slots) => slots,
                    None => {
                        return vec![system_message(&localized_text_or_fallback(
                            language,
                            "server.YouCannotCarryAnymore",
                            "server.YouCannotCarryAnymore",
                        ))];
                    }
                };
            for key in &keyed_items {
                if !can_gain_item_quantity(&resources, ItemContainer::Bag1, key, 1) {
                    return vec![system_message(&localized_text_or_fallback(
                        language,
                        "server.YouCannotCarryAnymore",
                        "server.YouCannotCarryAnymore",
                    ))];
                }
            }
        }
        {
            let mut player = self.app.world_mut().resource_mut::<PlayerRuntimeResource>();
            player.gold = player.gold.saturating_add(gold);
        }
        self.app
            .world_mut()
            .resource_mut::<Stage5SystemsResource>()
            .stage5_systems
            .mail[index]
            .claimed = true;
        for (mut item, (container, slot)) in item_states.into_iter().zip(exact_item_slots) {
            item.container = container;
            item.slot = slot;
            item.unique_id = allocate_item_unique_id(
                self.app.world().resource::<InventoryResource>(),
                item.container,
                item.slot,
            );
            self.app
                .world_mut()
                .resource_mut::<InventoryResource>()
                .inventory_items
                .push(item);
        }
        for key in keyed_items {
            add_or_increment_item(
                self.app.world_mut(),
                ItemContainer::Bag1,
                &key,
                &stage5_item_name(&key),
                "Stage 5 mail attachment.",
                20,
                1,
                1,
            );
        }
        if has_exact_item_states {
            let mut stage5 = self.app.world_mut().resource_mut::<Stage5SystemsResource>();
            let mail = &mut stage5.stage5_systems.mail[index];
            mail.gold = 0;
            mail.items.clear();
            mail.item_states_json.clear();
        }
        Vec::new()
    }

    fn stage5_mail_delete(&mut self, args: Vec<String>) -> Vec<ServerPacket> {
        let Some(id) = parse_u32_arg(&args, 0) else {
            let language = current_language(self.app.world());
            return vec![system_message(&format_localized_text(
                language,
                "server.InvalidPacketReceived",
                ["mail.delete".to_string()],
            ))];
        };
        let mut stage5 = self.app.world_mut().resource_mut::<Stage5SystemsResource>();
        if let Some(mail) = stage5
            .stage5_systems
            .mail
            .iter_mut()
            .find(|mail| mail.id == id)
        {
            mail.deleted = true;
        }
        Vec::new()
    }

    fn stage5_trade_start(&mut self, args: Vec<String>) -> Vec<ServerPacket> {
        let partner = args
            .first()
            .cloned()
            .unwrap_or_else(|| "Trader".to_string());
        self.app
            .world_mut()
            .resource_mut::<Stage5SystemsResource>()
            .stage5_systems
            .trade = Some(Stage5TradeState {
            partner: partner.clone(),
            offered_items: Vec::new(),
            offered_slots: BTreeMap::new(),
            offered_gold: 0,
            accepted: false,
            locked: false,
            completed: false,
        });
        Vec::new()
    }

    fn stage5_trade_offer_gold(&mut self, args: Vec<String>) -> Vec<ServerPacket> {
        let language = current_language(self.app.world());
        let Some(amount) = parse_u32_arg(&args, 0) else {
            return vec![system_message(&format_localized_text(
                language,
                "server.InvalidPacketReceived",
                ["trade.offerGold"],
            ))];
        };
        let language = current_language(self.app.world());
        if self.app.world().resource::<PlayerRuntimeResource>().gold < amount {
            return vec![system_message(&localized_text_or_fallback(
                language,
                "server.LowGold",
                "server.LowGold",
            ))];
        }
        let mut stage5 = self.app.world_mut().resource_mut::<Stage5SystemsResource>();
        let Some(trade) = stage5.stage5_systems.trade.as_mut() else {
            return vec![system_message(&localized_text_or_fallback(
                language,
                "server.NotFound",
                "server.NotFound",
            ))];
        };
        if trade.completed || trade.locked {
            return vec![system_message(&localized_text_or_fallback(
                language,
                "server.NotFound",
                "server.NotFound",
            ))];
        }
        trade.offered_gold = amount;
        trade.accepted = false;
        trade.locked = false;
        Vec::new()
    }

    fn stage5_trade_offer_item(&mut self, args: Vec<String>) -> Vec<ServerPacket> {
        let key = args
            .first()
            .cloned()
            .unwrap_or_else(|| "red-potion".to_string());
        let language = current_language(self.app.world());
        let resources = self.app.world_mut().resource_mut::<InventoryResource>();
        let Some(item) = resources
            .inventory_items
            .iter()
            .find(|item| item.key == key)
        else {
            return vec![system_message(&localized_text_or_fallback(
                language,
                "server.NotFound",
                "server.NotFound",
            ))];
        };
        if !stage5_trade_item_can_enter(item) {
            return vec![system_message(&localized_text_or_fallback(
                language,
                "client.CantTrade",
                "client.CantTrade",
            ))];
        }
        drop(resources);
        let mut stage5 = self.app.world_mut().resource_mut::<Stage5SystemsResource>();
        let Some(trade) = stage5.stage5_systems.trade.as_mut() else {
            return vec![system_message(&localized_text_or_fallback(
                language,
                "server.NotFound",
                "server.NotFound",
            ))];
        };
        if trade.completed || trade.locked {
            return vec![system_message(&localized_text_or_fallback(
                language,
                "server.NotFound",
                "server.NotFound",
            ))];
        }
        push_unique(&mut trade.offered_items, key.clone());
        trade.accepted = false;
        trade.locked = false;
        Vec::new()
    }

    fn stage5_trade_accept(&mut self) -> Vec<ServerPacket> {
        let language = current_language(self.app.world());
        let Some((offered_gold, offered_items)) = self
            .app
            .world()
            .resource::<Stage5SystemsResource>()
            .stage5_systems
            .trade
            .as_ref()
            .map(|trade| (trade.offered_gold, trade.offered_items.clone()))
        else {
            return vec![system_message(&localized_text_or_fallback(
                language,
                "server.NotFound",
                "server.NotFound",
            ))];
        };
        {
            let inventory = self.app.world().resource::<InventoryResource>();
            for offered_item in offered_items {
                let Some(item) = inventory
                    .inventory_items
                    .iter()
                    .find(|item| item.key == offered_item)
                else {
                    return vec![system_message(&localized_text_or_fallback(
                        language,
                        "server.NotFound",
                        "server.NotFound",
                    ))];
                };
                if !stage5_trade_item_can_enter(item) {
                    return vec![system_message(&localized_text_or_fallback(
                        language,
                        "client.CantTrade",
                        "client.CantTrade",
                    ))];
                }
            }
        }
        let mut player = self.app.world_mut().resource_mut::<PlayerRuntimeResource>();
        if player.gold < offered_gold {
            return vec![system_message(&localized_text_or_fallback(
                language,
                "server.LowGold",
                "server.LowGold",
            ))];
        }
        player.gold -= offered_gold;
        drop(player);
        if let Some(trade) = self
            .app
            .world_mut()
            .resource_mut::<Stage5SystemsResource>()
            .stage5_systems
            .trade
            .as_mut()
        {
            trade.accepted = true;
            trade.completed = true;
        }
        vec![system_message(&localized_text_or_fallback(
            language,
            "server.TradeSuccessful",
            "server.TradeSuccessful",
        ))]
    }

    fn stage5_trade_cancel(&mut self) -> Vec<ServerPacket> {
        self.app
            .world_mut()
            .resource_mut::<Stage5SystemsResource>()
            .stage5_systems
            .trade = None;
        Vec::new()
    }

    fn stage5_shop_buy(&mut self, args: Vec<String>) -> Vec<ServerPacket> {
        let key = args
            .first()
            .cloned()
            .unwrap_or_else(|| "red-potion".to_string());
        let price = args
            .get(1)
            .and_then(|value| value.parse::<u32>().ok())
            .unwrap_or(25);
        let language = current_language(self.app.world());
        {
            if self.app.world().resource::<PlayerRuntimeResource>().gold < price {
                return vec![system_message(&localized_text_or_fallback(
                    language,
                    "server.LowGold",
                    "server.LowGold",
                ))];
            }
            let resources = self.app.world().resource::<InventoryResource>();
            if !can_gain_item_quantity(&resources, ItemContainer::Bag1, &key, 1) {
                return vec![system_message(&localized_text_or_fallback(
                    language,
                    "server.YouCannotCarryAnymore",
                    "server.YouCannotCarryAnymore",
                ))];
            }
        }
        self.app
            .world_mut()
            .resource_mut::<PlayerRuntimeResource>()
            .gold -= price;
        add_or_increment_item(
            self.app.world_mut(),
            ItemContainer::Bag1,
            &key,
            &stage5_item_name(&key),
            "Stage 5 shop purchase.",
            20,
            1,
            1,
        );
        vec![system_message(&format_localized_text(
            language,
            "server.BoughtItemForGold",
            [key, price.to_string()],
        ))]
    }

    fn stage5_shop_buy_credit(&mut self, args: Vec<String>) -> Vec<ServerPacket> {
        let key = args
            .first()
            .cloned()
            .unwrap_or_else(|| "red-potion".to_string());
        let price = args
            .get(1)
            .and_then(|value| value.parse::<u32>().ok())
            .unwrap_or(1);
        let language = current_language(self.app.world());
        let player_name = stage5_player_name(self.app.world());
        let mail_id;
        {
            let mut player = self.app.world_mut().resource_mut::<PlayerRuntimeResource>();
            if player.credit < price {
                return vec![system_message(&localized_text_or_fallback(
                    language,
                    "server.YouDontHaveEnoughCurrency",
                    "server.YouDontHaveEnoughCurrency",
                ))];
            }
            player.credit -= price;
            drop(player);
            let mut stage5 = self.app.world_mut().resource_mut::<Stage5SystemsResource>();
            mail_id = stage5
                .stage5_systems
                .mail
                .iter()
                .map(|mail| mail.id)
                .max()
                .unwrap_or(0)
                + 1;
            stage5.stage5_systems.mail.push(Stage5MailMessage {
                id: mail_id,
                from: "Gameshop".to_string(),
                to: player_name,
                subject: "Game shop purchase".to_string(),
                body: format!("{key} was sent from the game shop."),
                gold: 0,
                items: vec![key.clone()],
                item_states_json: Vec::new(),
                opened: false,
                locked: false,
                claimed: false,
                deleted: false,
            });
        }
        vec![
            ServerPacket::LoseCredit { credit: price },
            system_message(&format_localized_text(
                language,
                "server.BoughtItemForCredit",
                [key, price.to_string()],
            )),
        ]
    }

    fn game_shop_buy_credit(&mut self, args: Vec<String>) -> Vec<ServerPacket> {
        let Some((item_key, item_name, price)) = game_shop_purchase_details(args, true) else {
            return vec![system_message(&format_localized_text(
                current_language(self.app.world()),
                "server.InvalidPacketReceived",
                ["gameShop.buyCredit".to_string()],
            ))];
        };
        self.deliver_game_shop_credit_purchase(item_key, item_name, price)
    }

    fn game_shop_buy_gold(&mut self, args: Vec<String>) -> Vec<ServerPacket> {
        let Some((item_key, item_name, price)) = game_shop_purchase_details(args, false) else {
            return vec![system_message(&format_localized_text(
                current_language(self.app.world()),
                "server.InvalidPacketReceived",
                ["gameShop.buyGold".to_string()],
            ))];
        };
        let language = current_language(self.app.world());
        {
            if self.app.world().resource::<PlayerRuntimeResource>().gold < price {
                return vec![system_message(&localized_text_or_fallback(
                    language,
                    "server.LowGold",
                    "server.LowGold",
                ))];
            }
            let resources = self.app.world().resource::<InventoryResource>();
            if !can_gain_item_quantity(&resources, ItemContainer::Bag1, &item_key, 1) {
                return vec![system_message(&localized_text_or_fallback(
                    language,
                    "server.YouCannotCarryAnymore",
                    "server.YouCannotCarryAnymore",
                ))];
            }
        }
        self.app
            .world_mut()
            .resource_mut::<PlayerRuntimeResource>()
            .gold -= price;
        add_or_increment_item(
            self.app.world_mut(),
            ItemContainer::Bag1,
            &item_key,
            &item_name,
            "Crystal game shop purchase.",
            8,
            1,
            1,
        );
        vec![system_message(&format_localized_text(
            language,
            "server.BoughtItemForGold",
            [item_name, price.to_string()],
        ))]
    }

    fn deliver_game_shop_credit_purchase(
        &mut self,
        item_key: String,
        item_name: String,
        price: u32,
    ) -> Vec<ServerPacket> {
        let language = current_language(self.app.world());
        let player_name = stage5_player_name(self.app.world());
        {
            let mut player = self.app.world_mut().resource_mut::<PlayerRuntimeResource>();
            if player.credit < price {
                return vec![system_message(&localized_text_or_fallback(
                    language,
                    "server.YouDontHaveEnoughCurrency",
                    "server.YouDontHaveEnoughCurrency",
                ))];
            }
            player.credit -= price;
            drop(player);
            let mut stage5 = self.app.world_mut().resource_mut::<Stage5SystemsResource>();
            let mail_id = stage5
                .stage5_systems
                .mail
                .iter()
                .map(|mail| mail.id)
                .max()
                .unwrap_or(0)
                + 1;
            stage5.stage5_systems.mail.push(Stage5MailMessage {
                id: mail_id,
                from: "Gameshop".to_string(),
                to: player_name,
                subject: "Game shop purchase".to_string(),
                body: format!("{item_name} was sent from the game shop."),
                gold: 0,
                items: vec![item_key],
                item_states_json: Vec::new(),
                opened: false,
                locked: false,
                claimed: false,
                deleted: false,
            });
        }
        vec![
            ServerPacket::LoseCredit { credit: price },
            system_message(&format_localized_text(
                language,
                "server.BoughtItemForCredit",
                [item_name, price.to_string()],
            )),
        ]
    }

    fn stage5_auction_list(&mut self, args: Vec<String>) -> Vec<ServerPacket> {
        let item_key = args
            .first()
            .cloned()
            .unwrap_or_else(|| "red-potion".to_string());
        let price = args
            .get(1)
            .and_then(|value| value.parse::<u32>().ok())
            .unwrap_or(50);
        let seller = stage5_player_name(self.app.world());
        let mut stage5 = self.app.world_mut().resource_mut::<Stage5SystemsResource>();
        let id = stage5
            .stage5_systems
            .auction
            .iter()
            .map(|listing| listing.id)
            .max()
            .unwrap_or(0)
            + 1;
        stage5.stage5_systems.auction.push(Stage5AuctionListing {
            id,
            seller,
            item_key,
            price,
            sold: false,
            cancelled: false,
            expired: false,
        });
        Vec::new()
    }

    fn stage5_auction_buy(&mut self, args: Vec<String>) -> Vec<ServerPacket> {
        let Some(id) = parse_u32_arg(&args, 0) else {
            let language = current_language(self.app.world());
            return vec![system_message(&format_localized_text(
                language,
                "server.InvalidPacketReceived",
                ["auction.buy".to_string()],
            ))];
        };
        let language = current_language(self.app.world());
        let (index, price, item_key) = {
            let stage5 = self.app.world().resource::<Stage5SystemsResource>();
            let Some(index) = stage5.stage5_systems.auction.iter().position(|listing| {
                listing.id == id && !listing.sold && !listing.cancelled && !listing.expired
            }) else {
                return vec![system_message(&localized_text_or_fallback(
                    language,
                    "server.NotFound",
                    "server.NotFound",
                ))];
            };
            let price = stage5.stage5_systems.auction[index].price;
            let item_key = stage5.stage5_systems.auction[index].item_key.clone();
            (index, price, item_key)
        };
        {
            if self.app.world().resource::<PlayerRuntimeResource>().gold < price {
                return vec![system_message(&localized_text_or_fallback(
                    language,
                    "server.LowGold",
                    "server.LowGold",
                ))];
            }
            let resources = self.app.world().resource::<InventoryResource>();
            if !can_gain_item_quantity(&resources, ItemContainer::Bag1, &item_key, 1) {
                return vec![system_message(&localized_text_or_fallback(
                    language,
                    "server.YouCannotCarryAnymore",
                    "server.YouCannotCarryAnymore",
                ))];
            }
        }
        self.app
            .world_mut()
            .resource_mut::<PlayerRuntimeResource>()
            .gold -= price;
        self.app
            .world_mut()
            .resource_mut::<Stage5SystemsResource>()
            .stage5_systems
            .auction[index]
            .sold = true;
        add_or_increment_item(
            self.app.world_mut(),
            ItemContainer::Bag1,
            &item_key,
            &stage5_item_name(&item_key),
            "Stage 5 auction purchase.",
            21,
            1,
            1,
        );
        Vec::new()
    }

    fn stage5_auction_cancel(&mut self, args: Vec<String>) -> Vec<ServerPacket> {
        let Some(id) = parse_u32_arg(&args, 0) else {
            let language = current_language(self.app.world());
            return vec![system_message(&format_localized_text(
                language,
                "server.InvalidPacketReceived",
                ["auction.cancel".to_string()],
            ))];
        };
        let mut stage5 = self.app.world_mut().resource_mut::<Stage5SystemsResource>();
        if let Some(listing) = stage5
            .stage5_systems
            .auction
            .iter_mut()
            .find(|listing| listing.id == id && !listing.sold && !listing.expired)
        {
            listing.cancelled = true;
        }
        Vec::new()
    }

    fn stage5_conquest_start(&mut self, args: Vec<String>) -> Vec<ServerPacket> {
        let castle = args.first().cloned().unwrap_or_else(|| "Sabuk".to_string());
        let mut resources = self.app.world_mut().resource_mut::<Stage5SystemsResource>();
        push_unique(
            &mut resources.stage5_systems.conquest.active_wars,
            castle.clone(),
        );
        resources
            .stage5_systems
            .conquest
            .event_log
            .push(format!("War started: {castle}"));
        Vec::new()
    }

    fn stage5_conquest_owner(&mut self, args: Vec<String>) -> Vec<ServerPacket> {
        let owner = args
            .first()
            .cloned()
            .or_else(|| {
                let resources = self.app.world().resource::<Stage5SystemsResource>();
                (!resources.stage5_systems.guild.name.is_empty())
                    .then(|| resources.stage5_systems.guild.name.clone())
            })
            .unwrap_or_else(|| "Independent".to_string());
        // Optional second arg: the conquest index this owner now controls, used
        // to gate conquest movements (Crystal `MyGuild.Conquest.Info.Index`).
        let conquest_index = args.get(1).and_then(|value| value.parse::<i32>().ok());
        {
            let mut resources = self.app.world_mut().resource_mut::<Stage5SystemsResource>();
            resources.stage5_systems.conquest.castle_owner = owner.clone();
            resources
                .stage5_systems
                .conquest
                .event_log
                .push(format!("Castle owner: {owner}"));
        }
        if let Some(index) = conquest_index {
            self.app
                .world_mut()
                .resource_mut::<MapRuntimeResource>()
                .conquest_owners
                .insert(index, owner.clone());
        }
        Vec::new()
    }

    fn stage5_conquest_end(&mut self, args: Vec<String>) -> Vec<ServerPacket> {
        let castle = args.first().cloned().unwrap_or_else(|| "Sabuk".to_string());
        let mut resources = self.app.world_mut().resource_mut::<Stage5SystemsResource>();
        resources
            .stage5_systems
            .conquest
            .active_wars
            .retain(|war| !war.eq_ignore_ascii_case(&castle));
        resources
            .stage5_systems
            .conquest
            .event_log
            .push(format!("War ended: {castle}"));
        Vec::new()
    }

    fn stage5_event_spawn(&mut self, args: Vec<String>) -> Vec<ServerPacket> {
        let monster_name = args
            .first()
            .cloned()
            .unwrap_or_else(|| "BugBat".to_string());
        let count = args
            .get(1)
            .and_then(|value| value.parse::<u8>().ok())
            .unwrap_or(1);
        let language = current_language(self.app.world());
        let Some(template) = crystal_dynamic_monster_template(&monster_name) else {
            return vec![system_message(&localized_text_or_fallback(
                language,
                "server.NotFound",
                "server.NotFound",
            ))];
        };
        let Some(player) = player_entity(self.app.world()) else {
            return vec![system_message(&localized_text_or_fallback(
                language,
                "server.NotFound",
                "server.NotFound",
            ))];
        };
        let Some(origin) = entity_position(self.app.world(), player) else {
            return vec![system_message(&localized_text_or_fallback(
                language,
                "server.NotFound",
                "server.NotFound",
            ))];
        };
        let (config, map_file_name) = {
            let world = self.app.world();
            (
                world.resource::<RuntimeConfigResource>().config.clone(),
                world
                    .resource::<MapRuntimeResource>()
                    .current_map
                    .file_name
                    .clone(),
            )
        };
        let mut spawn_points = crystal_spawn_candidates_on_map(&config, &map_file_name, &origin, 8)
            .into_iter()
            .filter(|point| point != &origin)
            .collect::<Vec<_>>();
        spawn_points.sort_by_key(|point| {
            let dx = (point.x - origin.x).abs();
            let dy = (point.y - origin.y).abs();
            (dx.max(dy), dx + dy, point.y, point.x)
        });

        let mut spawned = 0_u8;
        for index in 0..count {
            let position = spawn_points
                .get(usize::from(index))
                .cloned()
                .unwrap_or_else(|| Point {
                    x: origin.x + 1 + i32::from(index),
                    y: origin.y,
                });
            if spawn_runtime_monster(
                self.app.world_mut(),
                &template,
                position,
                MirDirection::Left,
                Some(player),
                None,
                Some(true),
                Some(WorldEntityDisposition::Hostile),
                0,
            )
            .is_some()
            {
                spawned += 1;
            }
        }
        self.app
            .world_mut()
            .resource_mut::<Stage5SystemsResource>()
            .stage5_systems
            .conquest
            .event_log
            .push(format!("Event spawned {spawned}x {monster_name}"));
        Vec::new()
    }

    fn stage5_hero_recruit(&mut self, args: Vec<String>) -> Vec<ServerPacket> {
        let name = args.first().cloned().unwrap_or_else(|| "Hero".to_string());
        self.app
            .world_mut()
            .resource_mut::<Stage5SystemsResource>()
            .stage5_systems
            .hero = Some(Stage5HeroState {
            name: name.clone(),
            level: 1,
            class: mir2_protocol::MirClass::Warrior,
            gender: mir2_protocol::MirGender::Male,
            behaviour: 0,
            experience: 0,
            spawned: true,
            auto_pot: true,
            auto_hp_percent: 0,
            auto_mp_percent: 0,
            hp_item_index: 0,
            mp_item_index: 0,
        });
        let _ = spawn_stage5_hero(self.app.world_mut());
        Vec::new()
    }

    fn stage5_hero_behaviour(&mut self, args: Vec<String>) -> Vec<ServerPacket> {
        let behaviour = args
            .first()
            .and_then(|value| value.parse::<u8>().ok())
            .unwrap_or(0);
        let language = current_language(self.app.world());
        let mut resources = self.app.world_mut().resource_mut::<Stage5SystemsResource>();
        let Some(hero) = resources.stage5_systems.hero.as_mut() else {
            return vec![system_message(&localized_text_or_fallback(
                language,
                "server.NotFound",
                "server.NotFound",
            ))];
        };
        hero.behaviour = behaviour;
        Vec::new()
    }

    fn stage5_mine(&mut self, args: Vec<String>) -> Vec<ServerPacket> {
        let ore = args
            .get(0)
            .and_then(|value| value.parse::<u32>().ok())
            .unwrap_or(1);
        let mut resources = self.app.world_mut().resource_mut::<Stage5SystemsResource>();
        resources.stage5_systems.profession.ore =
            resources.stage5_systems.profession.ore.saturating_add(ore);
        resources.stage5_systems.profession.mining_level =
            resources.stage5_systems.profession.mining_level.max(1);
        Vec::new()
    }

    fn stage5_craft(&mut self, args: Vec<String>) -> Vec<ServerPacket> {
        let item_key = args
            .first()
            .cloned()
            .unwrap_or_else(|| "crafted-blade".to_string());
        let language = current_language(self.app.world());
        {
            let stage5 = self.app.world().resource::<Stage5SystemsResource>();
            if stage5.stage5_systems.profession.ore == 0 {
                return vec![system_message(&localized_text_or_fallback(
                    language,
                    "server.CraftingAttemptFailed",
                    "server.CraftingAttemptFailed",
                ))];
            }
            let resources = self.app.world().resource::<InventoryResource>();
            if free_bag_slots(&resources) == 0 {
                return vec![system_message(&localized_text_or_fallback(
                    language,
                    "server.YouCannotCarryAnymore",
                    "server.YouCannotCarryAnymore",
                ))];
            }
        }
        {
            let mut resources = self.app.world_mut().resource_mut::<Stage5SystemsResource>();
            resources.stage5_systems.profession.ore -= 1;
            push_unique(
                &mut resources.stage5_systems.profession.crafted_items,
                item_key.clone(),
            );
        }
        add_or_increment_item(
            self.app.world_mut(),
            ItemContainer::Bag1,
            &item_key,
            &stage5_item_name(&item_key),
            "Stage 5 crafted item.",
            22,
            1,
            1,
        );
        Vec::new()
    }

    fn stage5_item_add_socket(&mut self, args: Vec<String>) -> Vec<ServerPacket> {
        let slot = args
            .first()
            .and_then(|value| equipment_slot_from_stage5_arg(value))
            .unwrap_or(EquipmentSlot::Weapon);
        let source_key = args.get(1).cloned();
        let language = current_language(self.app.world());
        let result = {
            let mut resources = self.app.world_mut().resource_mut::<InventoryResource>();
            let Some(item_index) = resources
                .equipment_items
                .iter()
                .position(|item| item.slot == slot)
            else {
                return vec![system_message(&localized_text_or_fallback(
                    language,
                    "server.NotFound",
                    "server.NotFound",
                ))];
            };
            let item = &resources.equipment_items[item_index];
            let Some(unique_id) = equipment_slot_unique_id(item.slot) else {
                return vec![system_message(&localized_text_or_fallback(
                    language,
                    "server.NotFound",
                    "server.NotFound",
                ))];
            };
            let Some(max_slots) = crystal_socket_slot_limit_for_item_key(&item.key) else {
                return vec![system_message(&localized_text_or_fallback(
                    language,
                    "server.NotFound",
                    "server.NotFound",
                ))];
            };
            if max_slots == 0 || item.socket_slots >= max_slots {
                return vec![system_message(&localized_text_or_fallback(
                    language,
                    "server.ItemMaxSockets",
                    "server.ItemMaxSockets",
                ))];
            }
            let source_index = if let Some(source_key) = source_key.as_deref() {
                let Some(source_index) = resources
                    .inventory_items
                    .iter()
                    .position(|item| item.key == source_key || item.name == source_key)
                else {
                    return vec![system_message(&localized_text_or_fallback(
                        language,
                        "server.NotFound",
                        "server.NotFound",
                    ))];
                };
                let source_item = &resources.inventory_items[source_index];
                if !crystal_socket_source_valid_for_item(source_item, &item.key) {
                    return vec![system_message(&localized_text_or_fallback(
                        language,
                        "server.InvalidCombination",
                        "server.InvalidCombination",
                    ))];
                }
                Some(source_index)
            } else {
                None
            };
            resources.equipment_items[item_index].socket_slots = resources.equipment_items
                [item_index]
                .socket_slots
                .saturating_add(1);
            if let Some(source_index) = source_index {
                if resources.inventory_items[source_index].quantity > 1 {
                    resources.inventory_items[source_index].quantity -= 1;
                } else {
                    resources.inventory_items.remove(source_index);
                }
            }
            (
                unique_id,
                i32::from(resources.equipment_items[item_index].socket_slots),
            )
        };
        let (unique_id, slot_size) = result;

        vec![
            ServerPacket::ItemSlotSizeChanged {
                unique_id,
                slot_size,
            },
            system_message(&localized_text_or_fallback(
                language,
                "server.ItemSocketsIncreased",
                "server.ItemSocketsIncreased",
            )),
        ]
    }

    fn stage5_item_seal(&mut self, args: Vec<String>) -> Vec<ServerPacket> {
        let slot = args
            .first()
            .and_then(|value| equipment_slot_from_stage5_arg(value))
            .unwrap_or(EquipmentSlot::Weapon);
        let fallback_minutes = args
            .get(1)
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(60)
            .max(1);
        let source_key = args.get(2).cloned();
        let now_binary_datetime = current_binary_datetime();
        let language = current_language(self.app.world());
        let result = {
            let mut resources = self.app.world_mut().resource_mut::<InventoryResource>();
            let Some(item_index) = resources
                .equipment_items
                .iter()
                .position(|item| item.slot == slot)
            else {
                return vec![system_message(&localized_text_or_fallback(
                    language,
                    "server.NotFound",
                    "server.NotFound",
                ))];
            };
            let item = &resources.equipment_items[item_index];
            if item.sealed_expiry_time_binary_datetime != 0
                && binary_datetime_ticks(item.sealed_expiry_time_binary_datetime)
                    > binary_datetime_ticks(now_binary_datetime)
            {
                return vec![system_message(&localized_text_or_fallback(
                    language,
                    "server.ItemAlreadySealed",
                    "server.ItemAlreadySealed",
                ))];
            }
            if item.sealed_next_time_binary_datetime != 0
                && binary_datetime_ticks(item.sealed_next_time_binary_datetime)
                    > binary_datetime_ticks(now_binary_datetime)
            {
                let remaining_ticks = binary_datetime_ticks(item.sealed_next_time_binary_datetime)
                    - binary_datetime_ticks(now_binary_datetime);
                let remaining_seconds =
                    u64::try_from((remaining_ticks + 9_999_999) / 10_000_000).unwrap_or(1);
                return vec![system_message(&format_localized_text(
                    language,
                    "server.ItemCannotBeResealedFor",
                    [crystal_duration_label_from_seconds(
                        remaining_seconds.max(1),
                    )],
                ))];
            }
            let Some(unique_id) = equipment_slot_unique_id(item.slot) else {
                return vec![system_message(&localized_text_or_fallback(
                    language,
                    "server.NotFound",
                    "server.NotFound",
                ))];
            };
            let source_index_and_minutes = if let Some(source_key) = source_key.as_deref() {
                let Some(source_index) = resources
                    .inventory_items
                    .iter()
                    .position(|item| item.key == source_key || item.name == source_key)
                else {
                    return vec![system_message(&localized_text_or_fallback(
                        language,
                        "server.NotFound",
                        "server.NotFound",
                    ))];
                };
                let source_item = &resources.inventory_items[source_index];
                let Some(minutes) =
                    crystal_seal_minutes_for_source_item(source_item, fallback_minutes)
                else {
                    return vec![system_message(&localized_text_or_fallback(
                        language,
                        "server.InvalidCombination",
                        "server.InvalidCombination",
                    ))];
                };

                Some((source_index, minutes))
            } else {
                None
            };
            let minutes = source_index_and_minutes
                .map(|(_, minutes)| minutes)
                .unwrap_or(fallback_minutes);
            let expiry_date_binary_datetime = future_binary_datetime_minutes(minutes);
            let next_seal_binary_datetime = add_minutes_to_binary_datetime(
                expiry_date_binary_datetime,
                CRYSTAL_ITEM_SEAL_DELAY_MINUTES,
            );

            resources.equipment_items[item_index].sealed_expiry_time_binary_datetime =
                expiry_date_binary_datetime;
            resources.equipment_items[item_index].sealed_next_time_binary_datetime =
                next_seal_binary_datetime;
            if let Some((source_index, _)) = source_index_and_minutes {
                if resources.inventory_items[source_index].quantity > 1 {
                    resources.inventory_items[source_index].quantity -= 1;
                } else {
                    resources.inventory_items.remove(source_index);
                }
            }
            (unique_id, expiry_date_binary_datetime, minutes)
        };
        let (unique_id, expiry_date_binary_datetime, minutes) = result;

        vec![
            ServerPacket::ItemSealChanged {
                unique_id,
                expiry_date_binary_datetime,
            },
            system_message(&format_localized_text(
                language,
                "server.ItemSealedFor",
                [crystal_duration_label_from_seconds(
                    minutes.saturating_mul(60),
                )],
            )),
        ]
    }

    fn stage5_qa_damage_equipment(&mut self, args: Vec<String>) -> Vec<ServerPacket> {
        let slot = args
            .first()
            .and_then(|value| equipment_slot_from_stage5_arg(value))
            .unwrap_or(EquipmentSlot::Weapon);
        let amount = args
            .get(1)
            .and_then(|value| value.parse::<u16>().ok())
            .unwrap_or(1)
            .max(1);
        let language = current_language(self.app.world());
        let mut resources = self.app.world_mut().resource_mut::<InventoryResource>();
        let Some(item) = resources
            .equipment_items
            .iter_mut()
            .find(|item| item.slot == slot)
        else {
            return vec![system_message(&localized_text_or_fallback(
                language,
                "server.NotFound",
                "server.NotFound",
            ))];
        };
        if !equipment_uses_durability(item) {
            return Vec::new();
        }
        let _ = damage_equipment_item(item, amount);
        let Some(unique_id) = equipment_slot_unique_id(item.slot) else {
            return Vec::new();
        };

        vec![ServerPacket::DuraChanged {
            unique_id,
            current_dura: item.durability_current,
        }]
    }

    fn stage5_qa_damage_player(&mut self, args: Vec<String>) -> Vec<ServerPacket> {
        let amount = parse_u32_arg(&args, 0).unwrap_or(25).max(1) as i32;
        let Some(player) = player_entity(self.app.world()) else {
            return Vec::new();
        };
        let next_vitals = {
            let mut entity = self.app.world_mut().entity_mut(player);
            let mut vitals = entity
                .get_mut::<PlayerVitals>()
                .expect("current player should have vitals");
            vitals.hp = (vitals.hp - amount).max(1);
            *vitals
        };
        self.app
            .world_mut()
            .resource_mut::<PlayerRuntimeResource>()
            .player_vitals = next_vitals;
        object_health_info_for_entity(self.app.world(), player, 0)
            .map(|info| vec![ServerPacket::ObjectHealth { info }])
            .unwrap_or_default()
    }

    fn stage5_qa_give_item(&mut self, args: Vec<String>) -> Vec<ServerPacket> {
        let item_key = args
            .first()
            .cloned()
            .unwrap_or_else(|| "blue-potion".to_string());
        let quantity = parse_u32_arg(&args, 1).unwrap_or(1).max(1);
        let language = current_language(self.app.world());
        let mut resources = self.app.world_mut().resource_mut::<InventoryResource>();
        let Some((container, slot)) =
            find_empty_inventory_item_slot(&resources.inventory_items, ItemContainer::Bag1)
        else {
            return vec![system_message(&localized_text_or_fallback(
                language,
                "server.YouCannotCarryAnymore",
                "server.YouCannotCarryAnymore",
            ))];
        };
        let unique_id = allocate_item_unique_id(&resources, container, slot);
        let (heal_hp, heal_mp) = item_heal_values_for_key(&item_key);
        resources.inventory_items.push(ItemState {
            key: item_key.clone(),
            name: stage5_item_name(&item_key),
            icon: item_icon_for_key(&item_key),
            slot,
            unique_id,
            container,
            quantity,
            description: "Stage 5 QA seeded item.".to_string(),
            durability_current: None,
            durability_max: None,
            weight: 1,
            equip_slot: crystal_equipment_slot_for_item_key(&item_key),
            grade: ItemGrade::None,
            added_attack: 0,
            added_defence: 0,
            added_stats: Vec::new(),
            socketed: Vec::new(),
            cursed: false,
            socket_slots: 0,
            gem_count: 0,
            identified: None,
            soul_bound_id: None,
            sealed_expiry_time_binary_datetime: 0,
            sealed_next_time_binary_datetime: 0,
            rental_binding_flags: 0,
            rental_owner_name: String::new(),
            rental_expiry_binary_datetime: 0,
            rental_locked: false,
            attack: 0,
            defence: 0,
            heal_hp,
            heal_mp,
        });
        Vec::new()
    }

    fn stage5_qa_open_npc_dialog(&mut self) -> Vec<ServerPacket> {
        let Some(player) = player_entity(self.app.world()) else {
            return Vec::new();
        };
        let Some(position) = entity_position(self.app.world(), player) else {
            return Vec::new();
        };
        let npc_position = Point {
            x: position.x,
            y: position.y.saturating_sub(1),
        };
        let world = self.app.world_mut();
        if let Some(npc) = entity_by_object_id(world, 21) {
            world.entity_mut(npc).insert((
                Npc,
                Position(npc_position),
                DisplayName::literal("InnKeeper_Brittney"),
                NpcAgent {
                    image: 6,
                    colour_argb: 0,
                    quest_ids: Vec::new(),
                    script_key: Some("BichonProvince/BichonWall/Warehouse1".to_string()),
                },
            ));
        } else {
            world.spawn((
                WorldObject,
                Npc,
                ObjectId(21),
                Position(npc_position),
                DisplayName::literal("InnKeeper_Brittney"),
                NpcAgent {
                    image: 6,
                    colour_argb: 0,
                    quest_ids: Vec::new(),
                    script_key: Some("BichonProvince/BichonWall/Warehouse1".to_string()),
                },
            ));
        }
        self.interact_impl(21)
    }

    fn stage5_qa_open_storage(&mut self) -> Vec<ServerPacket> {
        let Some(player) = player_entity(self.app.world()) else {
            return Vec::new();
        };
        let Some(position) = entity_position(self.app.world(), player) else {
            return Vec::new();
        };
        let world = self.app.world_mut();
        if let Some(npc) = entity_by_object_id(world, 21) {
            world.entity_mut(npc).insert((
                Npc,
                Position(position),
                DisplayName::literal("InnKeeper_Brittney"),
                NpcAgent {
                    image: 6,
                    colour_argb: 0,
                    quest_ids: Vec::new(),
                    script_key: Some("BichonProvince/BichonWall/Warehouse1".to_string()),
                },
            ));
        } else {
            world.spawn((
                WorldObject,
                Npc,
                ObjectId(21),
                Position(position),
                DisplayName::literal("InnKeeper_Brittney"),
                NpcAgent {
                    image: 6,
                    colour_argb: 0,
                    quest_ids: Vec::new(),
                    script_key: Some("BichonProvince/BichonWall/Warehouse1".to_string()),
                },
            ));
        }
        world.resource_mut::<NpcStateResource>().active_npc_service = Some(ActiveNpcServiceState {
            script_key: "BichonProvince/BichonWall/Warehouse1".to_string(),
            label_key: "STORAGE".to_string(),
            npc_object_id: 21,
        });
        world.resource_mut::<InventoryResource>().storage_unlocked = false;
        crystal_npc_storage_open_packets(world)
    }
}
