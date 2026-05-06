use std::time::{SystemTime, UNIX_EPOCH};

use bevy_ecs::prelude::World;
use mir2_game_data::{
    crystal_game_shop_packet_manifest, crystal_item_by_index, format_localized_text,
    localized_text_or_fallback,
};
use mir2_protocol::{ChatType, MirDirection, Point, ServerPacket};

use crate::config::{
    EquipmentSlot, ItemContainer, Stage5AuctionListing, Stage5HeroState, Stage5MailMessage,
    Stage5TradeState, WorldEntityDisposition,
};

use super::components::{entity_position, player_entity};
use super::crystal_compat::CRYSTAL_ITEM_SEAL_DELAY_MINUTES;
use super::equipment::{equipment_slot_from_stage5_arg, equipment_slot_unique_id};
use super::inventory::{
    add_minutes_to_binary_datetime, add_or_increment_item, binary_datetime_ticks,
    can_gain_item_quantity, crystal_duration_label_from_seconds, current_binary_datetime,
    free_bag_slots, future_binary_datetime_minutes,
};
use super::items::{
    crystal_item_key_for_template, crystal_seal_minutes_for_source_item,
    crystal_socket_slot_limit_for_item_key, crystal_socket_source_valid_for_item,
};
use super::monsters::{crystal_dynamic_monster_template, spawn_runtime_monster};
use super::resources::{
    InventoryResource, PlayerRuntimeResource, SessionResource, Stage5SystemsResource,
};
use super::session::{current_language, system_message, SimulationSession};

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
        stage5.stage5_systems.guild.name = name.clone();
        stage5.stage5_systems.guild.members = unique_strings([player_name]);
        stage5.stage5_systems.guild.rank = "Guild Chief".to_string();
        stage5.stage5_systems.guild.permissions = vec![
            "invite".to_string(),
            "rank".to_string(),
            "storage".to_string(),
            "conquest".to_string(),
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
        let mut stage5 = self.app.world_mut().resource_mut::<Stage5SystemsResource>();
        push_unique(&mut stage5.stage5_systems.social.friends, name.clone());
        Vec::new()
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
        let mut stage5 = self.app.world_mut().resource_mut::<Stage5SystemsResource>();
        push_unique(&mut stage5.stage5_systems.social.blocked, name.clone());
        stage5
            .stage5_systems
            .social
            .friends
            .retain(|friend| !friend.eq_ignore_ascii_case(&name));
        Vec::new()
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
        let (index, gold, items) = {
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
            )
        };
        {
            let resources = self.app.world().resource::<InventoryResource>();
            for key in &items {
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
        for key in items {
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
            offered_gold: 0,
            accepted: false,
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
        trade.offered_gold = amount;
        Vec::new()
    }

    fn stage5_trade_offer_item(&mut self, args: Vec<String>) -> Vec<ServerPacket> {
        let key = args
            .first()
            .cloned()
            .unwrap_or_else(|| "red-potion".to_string());
        let language = current_language(self.app.world());
        let resources = self.app.world_mut().resource_mut::<InventoryResource>();
        if !resources.inventory_items.iter().any(|item| item.key == key) {
            return vec![system_message(&localized_text_or_fallback(
                language,
                "server.NotFound",
                "server.NotFound",
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
        push_unique(&mut trade.offered_items, key.clone());
        Vec::new()
    }

    fn stage5_trade_accept(&mut self) -> Vec<ServerPacket> {
        let language = current_language(self.app.world());
        let Some(offered_gold) = self
            .app
            .world()
            .resource::<Stage5SystemsResource>()
            .stage5_systems
            .trade
            .as_ref()
            .map(|trade| trade.offered_gold)
        else {
            return vec![system_message(&localized_text_or_fallback(
                language,
                "server.NotFound",
                "server.NotFound",
            ))];
        };
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
        let mut resources = self.app.world_mut().resource_mut::<Stage5SystemsResource>();
        resources.stage5_systems.conquest.castle_owner = owner.clone();
        resources
            .stage5_systems
            .conquest
            .event_log
            .push(format!("Castle owner: {owner}"));
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
            .unwrap_or_else(|| "Field Wasp".to_string());
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
        let mut spawned = 0_u8;
        for index in 0..count {
            let position = Point {
                x: origin.x + 1 + i32::from(index),
                y: origin.y,
            };
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
            behaviour: 0,
        });
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
}
