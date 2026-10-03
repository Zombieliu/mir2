//! Castle commerce uses ordinary NPC goods, stock and item allocation. The
//! resulting full character image and castle tax are published together.
//! No client packet carries a conquest, Guild, tax rate or price override.
use bevy_ecs::{prelude::Resource, world::World};
use mir2_game_data::{crystal_npc_info_by_script_key, CrystalItemTemplate};
use mir2_protocol::ServerPacket;
use std::collections::BTreeSet;

use super::npc::{self, ActiveNpcServiceState};
use super::resources::{
    is_in_world, InventoryResource, MapRuntimeResource, NpcStateResource, PlayerRuntimeResource,
    RuntimeConfigResource, SessionResource,
};
use super::save::{
    active_session_mutating_account_id, snapshot_active_character_save,
    stage_prepared_character_save,
};
use super::session::SimulationSession;
use crate::config::{AccountStore, CharacterSaveRecord, SimulationConfig, Stage5FriendIdentity};
use crate::conquest::{default_sabuk_defenses, valid_guild_id, SharedConquestRecord};

const SABUK_INDEX: i32 = 1;
const SABUK_SCRIPTS: &str = "MongchonProvince/SabukWall/";

/// Explicit repair of shipped Conquest=0 bindings. A map-wide tax would also
/// tax Mongchon merchants outside the castle, contrary to the Crystal source.
fn merchant_binding(script_key: &str, object_id: u32, map_file_name: &str) -> bool {
    let Some(suffix) = script_key.strip_prefix(SABUK_SCRIPTS) else {
        return false;
    };
    let expected = match suffix {
        "Butcher" => (927, "3"),
        "Peddlar" => (928, "3"),
        "Blacksmith" => (1147, "0151"),
        "Weaponsmith" => (1148, "0151"),
        "Blacksmith1" => (1149, "0151"),
        "Warehouse" => (1150, "0152"),
        "Potion1" => (1151, "0153"),
        "Potion" => (1152, "0153"),
        "Bracelet" => (1153, "0154"),
        "Necklace" => (1154, "0154"),
        "Ring" => (1155, "0154"),
        "Clothes" => (1156, "0155"),
        "Helmet" => (1157, "0155"),
        _ => return false,
    };
    object_id == expected.0
        && map_file_name.eq_ignore_ascii_case(expected.1)
        && crystal_npc_info_by_script_key(script_key).is_some_and(|info| {
            info.loaded_object_id == Some(object_id)
                && info
                    .map_file_name
                    .as_deref()
                    .is_some_and(|map| map.eq_ignore_ascii_case(expected.1))
        })
}

fn eligible_service(world: &World, buy: bool) -> Option<ActiveNpcServiceState> {
    let config = &world.resource::<RuntimeConfigResource>().config;
    if !config
        .conquest_policies
        .iter()
        .any(|p| p.index == SABUK_INDEX)
    {
        return None;
    }
    let service = npc::current_crystal_npc_service_in_range(world)?;
    if service.label_key == "PEARLBUY"
        || !(if buy {
            npc::active_crystal_buy_service(&service)
        } else {
            npc::active_crystal_sell_service(&service)
        })
        || !merchant_binding(
            &service.script_key,
            service.npc_object_id,
            &world.resource::<MapRuntimeResource>().current_map.file_name,
        )
    {
        return None;
    }
    Some(service)
}

fn validate_policy(config: &SimulationConfig) -> Result<(), String> {
    let policies: Vec<_> = config
        .conquest_policies
        .iter()
        .filter(|p| p.index == SABUK_INDEX)
        .collect();
    if policies.len() != 1 {
        return Err("castle merchant requires one available Sabuk policy".into());
    }
    let policy = policies[0];
    policy.validate()?;
    if policy.map_file_name != "3" || policy.palace_file_name != "0150" {
        return Err("castle merchant policy binding changed".into());
    }
    Ok(())
}

fn actor_guild(
    store: &AccountStore,
    identity: &Stage5FriendIdentity,
) -> Result<Option<String>, String> {
    let mut found = None;
    for (id, guild) in &store.shared_guilds {
        if id != &guild.id || !valid_guild_id(id) {
            return Err("invalid castle Guild authority".into());
        }
        let members = guild
            .members
            .iter()
            .filter(|member| &member.identity == identity)
            .count();
        if members > 1 || (members == 1 && found.is_some()) {
            return Err("ambiguous castle Guild membership".into());
        }
        if members == 1 {
            found = Some(id.clone());
        }
    }
    Ok(found)
}

fn validate_actor(
    store: &AccountStore,
    identity: &Stage5FriendIdentity,
    save: &CharacterSaveRecord,
) -> Result<(), String> {
    if identity.account_id.trim().is_empty()
        || identity.account_id != identity.account_id.trim()
        || identity.character_index < 0
        || save.character.index != identity.character_index
    {
        return Err("invalid castle trade identity".into());
    }
    let account = store
        .accounts
        .get(&identity.account_id)
        .ok_or("castle trade account missing")?;
    let character = account
        .characters
        .iter()
        .find(|c| c.index == identity.character_index)
        .ok_or("castle trade character missing")?;
    let saved = account
        .saves
        .get(&identity.character_index)
        .ok_or("castle trade save missing")?;
    if character.name != save.character.name
        || saved.character.name != save.character.name
        || saved.character.index != identity.character_index
        || saved.revision != save.revision
    {
        return Err("stale castle trade checkpoint".into());
    }
    Ok(())
}

#[derive(Clone)]
struct CastleTradeQuote {
    record: Option<SharedConquestRecord>,
    actor_guild_id: Option<String>,
    guild_ids: Vec<String>,
    base_rate: f32,
    effective_rate: f32,
}

fn source_price_rate(rate: u16, tax: u8, exempt: bool) -> f32 {
    let base = f32::from(rate) / 100.0_f32;
    if exempt {
        base
    } else {
        ((base * f32::from(tax)) + f32::from(rate)) / 100.0_f32
    }
}

/// Exists only while an ordinary server-owned buy is being staged. A client
/// cannot populate this Resource, and matching includes the authenticated NPC.
#[derive(Resource)]
struct CastlePurchasePrice {
    script_key: String,
    object_id: u32,
    base_rate: f32,
    effective_rate: f32,
    costs: Option<(u32, u32)>,
    repeated_cost: bool,
}

pub(super) fn conquest_price_rate(
    world: &World,
    service: &ActiveNpcServiceState,
    base_rate: f32,
) -> f32 {
    world
        .get_resource::<CastlePurchasePrice>()
        .filter(|price| {
            price.script_key == service.script_key && price.object_id == service.npc_object_id
        })
        .map_or(base_rate, |price| price.effective_rate)
}

/// Hook called after ordinary stock selection/count clamping. Calculate each
/// total independently with Crystal single-precision/truncation, never by
/// rounding a percentage of the gold already charged.
pub(super) fn conquest_purchase_cost(
    world: &mut World,
    service: &ActiveNpcServiceState,
    template: &CrystalItemTemplate,
    count: u32,
    rate: f32,
) -> u32 {
    let charged = npc::crystal_npc_purchase_cost(template, count, rate);
    if let Some(mut price) = world.get_resource_mut::<CastlePurchasePrice>() {
        if price.script_key == service.script_key && price.object_id == service.npc_object_id {
            price.repeated_cost |=
                price.costs.is_some() || rate.to_bits() != price.effective_rate.to_bits();
            let base = npc::crystal_npc_purchase_cost(template, count, price.base_rate);
            price.costs = Some((base, charged));
        }
    }
    charged
}

/// Use on the server-generated goods packets after recording service context.
/// A quote failure leaves the displayed base price; actual trade then rejects
/// before a charge, rather than admitting an unobserved ownership decision.
pub(super) fn refresh_conquest_goods_price(world: &World, packets: &mut [ServerPacket]) {
    let Some(service) = eligible_service(world, true) else {
        return;
    };
    let Some(checkpoint) = snapshot_active_character_save(world) else {
        return;
    };
    let Some(account_id) = active_session_mutating_account_id(world.resource::<SessionResource>())
    else {
        return;
    };
    let identity = Stage5FriendIdentity {
        account_id,
        character_index: checkpoint.character.index,
    };
    let config = &world.resource::<RuntimeConfigResource>().config;
    let Ok(quote) = config.castle_trade_quote(&identity, &checkpoint, &service) else {
        return;
    };
    for packet in packets {
        if let ServerPacket::NPCGoods { rate, .. } = packet {
            *rate = quote.effective_rate;
        }
    }
}

impl SimulationConfig {
    fn castle_trade_quote(
        &self,
        identity: &Stage5FriendIdentity,
        checkpoint: &CharacterSaveRecord,
        service: &ActiveNpcServiceState,
    ) -> Result<CastleTradeQuote, String> {
        validate_policy(self)?;
        self.ensure_account_store_writable()?;
        self.refresh_shared_conquest_authority()?;
        self.refresh_account_store_account(&identity.account_id)?;
        let store = self
            .account_store
            .lock()
            .map_err(|_| "castle trade authority poisoned")?;
        validate_actor(&store, identity, checkpoint)?;
        let actor_guild_id = actor_guild(&store, identity)?;
        let record = store.shared_conquests.get(&SABUK_INDEX).cloned();
        if let Some(record) = &record {
            record.validate()?;
            if record.index != SABUK_INDEX {
                return Err("castle trade record index mismatch".into());
            }
            if record
                .owner_guild_id
                .as_ref()
                .is_some_and(|id| !store.shared_guilds.contains_key(id))
            {
                return Err("castle trade owner Guild missing".into());
            }
        }
        let info = crystal_npc_info_by_script_key(&service.script_key)
            .ok_or("castle merchant info missing")?;
        let owner = record.as_ref().and_then(|r| r.owner_guild_id.as_ref());
        let exempt = owner.is_none() || owner == actor_guild_id.as_ref();
        let tax = record.as_ref().map_or(0, |r| r.tax_rate_percent);
        let base_rate = source_price_rate(info.rate, 0, true);
        let effective_rate = source_price_rate(info.rate, tax, exempt);
        if !base_rate.is_finite()
            || base_rate < 0.0
            || info.price_rate.to_bits() != base_rate.to_bits()
            || !effective_rate.is_finite()
            || effective_rate < base_rate
        {
            return Err("invalid castle NPC price authority".into());
        }
        Ok(CastleTradeQuote {
            record,
            actor_guild_id,
            guild_ids: store.shared_guilds.keys().cloned().collect(),
            base_rate,
            effective_rate,
        })
    }

    fn commit_castle_trade(
        &self,
        world: &World,
        identity: &Stage5FriendIdentity,
        quote: &CastleTradeQuote,
        before: &CharacterSaveRecord,
        after: CharacterSaveRecord,
        tax: u32,
    ) -> Result<CharacterSaveRecord, String> {
        validate_policy(self)?;
        if before.revision != after.revision
            || before.character.index != after.character.index
            || before.character.name != after.character.name
        {
            return Err("castle preview identity changed".into());
        }
        self.commit_account_store_transaction_with_conquests(
            std::slice::from_ref(&identity.account_id),
            &quote.guild_ids,
            &[SABUK_INDEX],
            |store| {
                validate_actor(store, identity, before)?;
                let guild_ids: BTreeSet<_> = store.shared_guilds.keys().cloned().collect();
                if guild_ids != quote.guild_ids.iter().cloned().collect()
                    || actor_guild(store, identity)? != quote.actor_guild_id
                    || store.shared_conquests.get(&SABUK_INDEX) != quote.record.as_ref()
                {
                    return Err("castle ownership or Guild membership changed during trade".into());
                }
                let mut castle = match &quote.record {
                    Some(record) => {
                        let mut record = record.clone();
                        // Even a zero-tax trade must fence its ownership quote.
                        record.changed()?;
                        record
                    }
                    None => {
                        let mut record = SharedConquestRecord::new(SABUK_INDEX);
                        record.defenses = default_sabuk_defenses();
                        record
                    }
                };
                let owner = castle.owner_guild_id.as_ref();
                if tax != 0 && (owner.is_none() || owner == quote.actor_guild_id.as_ref()) {
                    return Err("castle tax attempted on an exempt actor".into());
                }
                castle.gold = castle
                    .gold
                    .checked_add(tax)
                    .ok_or("castle tax treasury exhausted")?;
                castle.validate()?;
                stage_prepared_character_save(world, store, after.clone())?;
                let committed = store
                    .accounts
                    .get(&identity.account_id)
                    .and_then(|a| a.saves.get(&identity.character_index))
                    .cloned()
                    .ok_or("castle trade save receipt missing")?;
                // External mentor/ring settlement cannot be silently projected
                // as an ordinary purchase. Reject before COMMIT if it would
                // change the staged wallet, items, character or experience.
                if committed.gold != after.gold
                    || committed.credit != after.credit
                    || committed.city_currencies != after.city_currencies
                    || committed.character != after.character
                    || committed.experience != after.experience
                    || committed.max_experience != after.max_experience
                    || committed.inventory_items_json != after.inventory_items_json
                    || committed.belt_items_json != after.belt_items_json
                    || committed.equipment_items_json != after.equipment_items_json
                {
                    return Err("castle trade requires refreshed personal economy authority".into());
                }
                store.shared_conquests.insert(SABUK_INDEX, castle);
                Ok(committed)
            },
        )
    }
}

#[derive(Clone)]
struct TradeRollback {
    inventory: InventoryResource,
    player: PlayerRuntimeResource,
    npc: NpcStateResource,
}
impl TradeRollback {
    fn take(world: &World) -> Self {
        Self {
            inventory: world.resource::<InventoryResource>().clone(),
            player: world.resource::<PlayerRuntimeResource>().clone(),
            npc: world.resource::<NpcStateResource>().clone(),
        }
    }
    fn restore(self, world: &mut World) {
        *world.resource_mut::<InventoryResource>() = self.inventory;
        *world.resource_mut::<PlayerRuntimeResource>() = self.player;
        *world.resource_mut::<NpcStateResource>() = self.npc;
        world.remove_resource::<CastlePurchasePrice>();
    }
}

fn trade_failure(world: &World, sale: Option<(u64, u16)>) -> Vec<ServerPacket> {
    let mut packets = vec![super::session::system_message_key(
        world,
        "server.InvalidPacketReceived",
    )];
    if let Some((unique_id, count)) = sale {
        packets.push(ServerPacket::SellItem {
            unique_id,
            count,
            success: false,
        });
    }
    packets
}

enum StagedTrade {
    Rejected(Vec<ServerPacket>),
    Committed(Vec<ServerPacket>),
}

impl SimulationSession {
    pub(super) fn try_shared_conquest_buy(
        &mut self,
        item_index: u64,
        count: u16,
        panel_type: u8,
    ) -> Option<Vec<ServerPacket>> {
        let service = eligible_service(self.app.world(), true)?;
        Some(self.stage_castle_trade(service, None, |world| {
            npc::buy_item_impl(world, item_index, count, panel_type)
        }))
    }

    pub(super) fn try_shared_conquest_sell(
        &mut self,
        unique_id: u64,
        count: u16,
    ) -> Option<Vec<ServerPacket>> {
        let service = eligible_service(self.app.world(), false)?;
        Some(
            self.stage_castle_trade(service, Some((unique_id, count)), |world| {
                npc::sell_item_impl(world, unique_id, count)
            }),
        )
    }

    fn stage_castle_trade<F>(
        &mut self,
        service: ActiveNpcServiceState,
        sale: Option<(u64, u16)>,
        preview: F,
    ) -> Vec<ServerPacket>
    where
        F: FnOnce(&mut World) -> Vec<ServerPacket>,
    {
        let baseline = TradeRollback::take(self.app.world());
        let result = self.stage_castle_trade_inner(&service, sale, preview);
        match result {
            Ok(StagedTrade::Committed(packets)) => packets,
            Ok(StagedTrade::Rejected(packets)) => {
                baseline.restore(self.app.world_mut());
                packets
            }
            Err(error) => {
                baseline.restore(self.app.world_mut());
                eprintln!("castle NPC trade rejected: {error}");
                trade_failure(self.app.world(), sale)
            }
        }
    }

    fn stage_castle_trade_inner<F>(
        &mut self,
        service: &ActiveNpcServiceState,
        sale: Option<(u64, u16)>,
        preview: F,
    ) -> Result<StagedTrade, String>
    where
        F: FnOnce(&mut World) -> Vec<ServerPacket>,
    {
        if !is_in_world(self.app.world()) {
            return Err("castle trade requires an active character".into());
        }
        let before = snapshot_active_character_save(self.app.world())
            .ok_or("castle trade checkpoint missing")?;
        let account_id =
            active_session_mutating_account_id(self.app.world().resource::<SessionResource>())
                .ok_or("castle trade requires authenticated account")?;
        let identity = Stage5FriendIdentity {
            account_id,
            character_index: before.character.index,
        };
        let config = self
            .app
            .world()
            .resource::<RuntimeConfigResource>()
            .config
            .clone();
        let quote = config.castle_trade_quote(&identity, &before, service)?;
        if sale.is_none() {
            self.app.world_mut().insert_resource(CastlePurchasePrice {
                script_key: service.script_key.clone(),
                object_id: service.npc_object_id,
                base_rate: quote.base_rate,
                effective_rate: quote.effective_rate,
                costs: None,
                repeated_cost: false,
            });
        }
        let packets = preview(self.app.world_mut());
        let price = self
            .app
            .world_mut()
            .remove_resource::<CastlePurchasePrice>();
        let success = if let Some((unique_id, count)) = sale {
            packets.iter().any(|p| matches!(p, ServerPacket::SellItem { unique_id: id, count: n, success: true } if *id == unique_id && *n == count))
        } else {
            packets
                .iter()
                .any(|p| matches!(p, ServerPacket::GainedItem { .. }))
        };
        if !success {
            return Ok(StagedTrade::Rejected(packets));
        }
        let after = snapshot_active_character_save(self.app.world())
            .ok_or("castle trade preview missing")?;
        // Crystal's inventory includes its first six belt cells. Ordinary NPC
        // buys fill those cells (or their existing stacks) before bag slots;
        // snapshots serialize the two parts separately. Both are inventory.
        if after.inventory_items_json == before.inventory_items_json
            && after.belt_items_json == before.belt_items_json
        {
            return Err("castle trade changed no inventory".into());
        }
        let tax = if sale.is_some() {
            if after.gold < before.gold {
                return Err("castle sale reduced the wallet".into());
            }
            // Crystal PlayerObject.Sell pays item.Price()/2 and does not tax it.
            0
        } else {
            let price = price.ok_or("castle purchase price receipt missing")?;
            if price.repeated_cost {
                return Err("castle purchase price receipt repeated".into());
            }
            let (base, charged) = price.costs.ok_or("castle purchase cost hook missing")?;
            let debit = before
                .gold
                .checked_sub(after.gold)
                .ok_or("castle buy increased the wallet")?;
            let wire_debit: Option<u32> = packets
                .iter()
                .filter_map(|p| match p {
                    ServerPacket::LoseGold { gold } => Some(*gold),
                    _ => None,
                })
                .try_fold(0u32, |sum, value| sum.checked_add(value));
            if debit != charged || wire_debit != Some(charged) {
                return Err("castle buy debit differs from its price receipt".into());
            }
            charged
                .checked_sub(base)
                .ok_or("castle buy price is below its base cost")?
        };
        let committed =
            config.commit_castle_trade(self.app.world(), &identity, &quote, &before, after, tax)?;
        if !self
            .app
            .world()
            .resource::<SessionResource>()
            .advance_active_save_revision(before.revision, committed.revision)
        {
            return Err(config.freeze_prepared_kill_source(
                "castle trade committed but active save cursor changed; reconciliation required"
                    .into(),
            ));
        }
        // Keep the ordinary preview resources: they contain the exact committed
        // allocator IDs, stack merges and buyback changes. No success escaped
        // before the account + conquest commit and its save-revision receipt.
        Ok(StagedTrade::Committed(packets))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{
        AccountStoreTransactionFault, ItemContainer, SharedGuildMember, SharedGuildRank,
        SharedGuildRecord,
    };
    use crate::conquest::sabuk_policy;
    use crate::VisibleNpcRecord;
    use mir2_game_data::{crystal_item_by_index, crystal_npc_script_by_key};
    use mir2_protocol::{ClientPacket, MirDirection, MirGridType, Point};
    use std::collections::BTreeMap;

    const SCRIPT: &str = "MongchonProvince/SabukWall/Potion";
    const GUILD: &str = "0123456789abcdef0123456789abcdef";

    // A mechanical authoritative NPC/service fixture. This does not establish
    // a human castle-interior or remote gateway acceptance claim.
    fn fixture(
        owner_member: bool,
    ) -> (
        SimulationSession,
        SimulationConfig,
        u64,
        CrystalItemTemplate,
    ) {
        let mut config = SimulationConfig::default();
        config.conquest_policies = vec![sabuk_policy()];
        config.visible_npcs.push(VisibleNpcRecord {
            object_id: 1152,
            name: "Merchant Harry".into(),
            image: 5,
            colour_argb: -1,
            position: Point {
                x: config.spawn.x,
                y: config.spawn.y + 1,
            },
            direction: MirDirection::Left,
            quest_ids: vec![],
            script_key: Some(SCRIPT.into()),
        });
        {
            let mut store = config.account_store.lock().unwrap();
            let (owner_identity, name) = if owner_member {
                let character = &store.accounts["demo"].characters[0];
                (
                    Stage5FriendIdentity {
                        account_id: "demo".into(),
                        character_index: character.index,
                    },
                    character.name.clone(),
                )
            } else {
                let mut account = store.accounts["demo"].clone();
                let original = account.characters[0].clone();
                let mut character = original.clone();
                character.index = 100;
                character.name = "CastleLord".into();
                let mut save = account.saves.remove(&original.index).unwrap();
                save.character = character.clone();
                account.characters = vec![character.clone()];
                account.saves.clear();
                account.saves.insert(character.index, save);
                store.accounts.insert("castle-owner".into(), account);
                (
                    Stage5FriendIdentity {
                        account_id: "castle-owner".into(),
                        character_index: character.index,
                    },
                    character.name,
                )
            };
            store.shared_guilds.insert(
                GUILD.into(),
                SharedGuildRecord {
                    id: GUILD.into(),
                    name: "Knights".into(),
                    revision: 1,
                    level: 0,
                    experience: 0,
                    spare_points: 0,
                    gold: 1000,
                    ranks: vec![SharedGuildRank {
                        index: 0,
                        name: "Leader".into(),
                        options: 255,
                    }],
                    members: vec![SharedGuildMember {
                        membership_epoch: 0,
                        identity: owner_identity,
                        name,
                        rank_index: 0,
                    }],
                    notice: vec![],
                    storage: BTreeMap::new(),
                    buffs: BTreeMap::new(),
                    last_buff_tick_ms: 0,
                    experience_receipts: BTreeSet::new(),
                    experience_receipt_payloads: BTreeMap::new(),
                },
            );
            let mut castle = SharedConquestRecord::new(SABUK_INDEX);
            castle.owner_guild_id = Some(GUILD.into());
            castle.tax_rate_percent = 25;
            castle.defenses = default_sabuk_defenses();
            store.shared_conquests.insert(SABUK_INDEX, castle);
        }
        let mut session = SimulationSession::new(config.clone());
        session.handle_packet(ClientPacket::Login {
            account_id: "demo".into(),
            password: "demo".into(),
        });
        session.handle_packet(ClientPacket::StartGame { character_index: 0 });
        session
            .app
            .world_mut()
            .resource_mut::<MapRuntimeResource>()
            .current_map
            .file_name = "0153".into();
        session
            .app
            .world_mut()
            .resource_mut::<PlayerRuntimeResource>()
            .gold = 50_000;
        session
            .app
            .world_mut()
            .resource_mut::<NpcStateResource>()
            .active_npc_service = Some(ActiveNpcServiceState {
            script_key: SCRIPT.into(),
            label_key: "BUYSELL".into(),
            npc_object_id: 1152,
        });
        let script = crystal_npc_script_by_key(SCRIPT).unwrap();
        let goods = npc::crystal_npc_trade_goods_for_script(&script);
        let (item, template) = goods
            .into_iter()
            .find_map(|item| {
                let template = crystal_item_by_index(item.item_index)?;
                (template.stack_size >= 2 && template.price > 0).then_some((item, template))
            })
            .expect("canonical potion catalog offers stackable goods");
        assert!(eligible_service(session.app.world(), true).is_some());
        (session, config, item.unique_id, template)
    }

    fn gained(packets: &[ServerPacket]) -> bool {
        packets
            .iter()
            .any(|p| matches!(p, ServerPacket::GainedItem { .. }))
    }
    fn durable(config: &SimulationConfig) -> CharacterSaveRecord {
        config.account_store.lock().unwrap().accounts["demo"].saves[&0].clone()
    }
    fn treasury(config: &SimulationConfig) -> u32 {
        config.account_store.lock().unwrap().shared_conquests[&SABUK_INDEX].gold
    }
    fn buy(session: &mut SimulationSession, item: u64) -> Vec<ServerPacket> {
        session
            .try_shared_conquest_buy(item, 2, 0)
            .expect("bound castle merchant")
    }
    fn inventory_image(session: &SimulationSession) -> serde_json::Value {
        let inventory = session.app.world().resource::<InventoryResource>();
        serde_json::json!({
            "bag": inventory.inventory_items,
            "belt": inventory.belt_items,
        })
    }

    fn move_purchased_item_to_bag(session: &mut SimulationSession, unique_id: u64) {
        let (from, to, quantity) = {
            let inventory = session.app.world().resource::<InventoryResource>();
            if inventory
                .inventory_items
                .iter()
                .any(|item| item.unique_id == unique_id)
            {
                return;
            }
            let item = inventory
                .belt_items
                .iter()
                .find(|item| item.unique_id == unique_id)
                .expect("ordinary potion purchase must exist in the belt or bag");
            let (container, slot) = super::super::inventory::find_empty_inventory_item_slot(
                &inventory.inventory_items,
                ItemContainer::Bag1,
                inventory.inventory_capacity,
            )
            .expect("sale fixture has a free bag cell");
            let bag_index = match container {
                ItemContainer::Bag1 => i32::from(slot),
                ItemContainer::Bag2 => 40 + i32::from(slot),
                _ => unreachable!("bag-slot lookup returned a non-bag container"),
            };
            (i32::from(item.slot), 6 + bag_index, item.quantity)
        };
        let packets = session.handle_packet(ClientPacket::MoveItem {
            grid: MirGridType::Belt,
            from,
            to,
        });
        assert!(packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::MoveItem {
                grid: MirGridType::Belt,
                success: true,
                ..
            }
        )));
        let inventory = session.app.world().resource::<InventoryResource>();
        assert!(inventory
            .inventory_items
            .iter()
            .any(|item| item.unique_id == unique_id && item.quantity == quantity));
        assert!(!inventory
            .belt_items
            .iter()
            .any(|item| item.unique_id == unique_id));
    }
    fn stock_image(session: &SimulationSession) -> (serde_json::Value, serde_json::Value) {
        let npc = session.app.world().resource::<NpcStateResource>();
        (
            serde_json::to_value(&npc.npc_buy_back_items).unwrap(),
            serde_json::to_value(&npc.npc_used_goods_items).unwrap(),
        )
    }

    #[test]
    fn castle_buy_commits_inventory_wallet_and_tax_together_and_reloads_exact_uids() {
        let (mut session, config, item, template) = fixture(false);
        let before = snapshot_active_character_save(session.app.world()).unwrap();
        let before_revision = durable(&config).revision;
        let castle_revision = config
            .shared_conquest_snapshot(SABUK_INDEX)
            .unwrap()
            .revision;
        let cost = npc::crystal_npc_purchase_cost(&template, 2, 1.25);
        let base = npc::crystal_npc_purchase_cost(&template, 2, 1.0);
        let packets = buy(&mut session, item);
        assert!(gained(&packets));
        let saved = durable(&config);
        assert_eq!(saved.revision, before_revision + 1);
        assert_eq!(saved.gold, 50_000 - cost);
        assert_eq!(saved.inventory_items_json, before.inventory_items_json);
        assert_ne!(saved.belt_items_json, before.belt_items_json);
        assert_eq!(
            saved.inventory_items_json,
            snapshot_active_character_save(session.app.world())
                .unwrap()
                .inventory_items_json
        );
        assert_eq!(treasury(&config), cost - base);
        assert_eq!(
            saved.belt_items_json,
            snapshot_active_character_save(session.app.world())
                .unwrap()
                .belt_items_json
        );
        assert_eq!(
            config
                .shared_conquest_snapshot(SABUK_INDEX)
                .unwrap()
                .revision,
            castle_revision + 1
        );
        assert_eq!(
            session
                .app
                .world()
                .resource::<SessionResource>()
                .active_save_revision(),
            Some(saved.revision)
        );
        let expected = saved.inventory_items_json.clone();
        let expected_belt = saved.belt_items_json.clone();
        session.handle_packet(ClientPacket::LogOut);
        session.handle_packet(ClientPacket::StartGame { character_index: 0 });
        assert_eq!(
            snapshot_active_character_save(session.app.world())
                .unwrap()
                .inventory_items_json,
            expected
        );
        assert_eq!(
            snapshot_active_character_save(session.app.world())
                .unwrap()
                .belt_items_json,
            expected_belt
        );
        assert_eq!(
            session.app.world().resource::<PlayerRuntimeResource>().gold,
            saved.gold
        );
        assert_eq!(treasury(&config), cost - base);
    }

    #[test]
    fn owner_membership_exempts_buy_but_fences_the_ownership_quote() {
        let (mut session, config, item, template) = fixture(true);
        let before = config
            .shared_conquest_snapshot(SABUK_INDEX)
            .unwrap()
            .revision;
        assert!(gained(&buy(&mut session, item)));
        let base = npc::crystal_npc_purchase_cost(&template, 2, 1.0);
        assert_eq!(durable(&config).gold, 50_000 - base);
        assert_eq!(treasury(&config), 0);
        assert_eq!(
            config
                .shared_conquest_snapshot(SABUK_INDEX)
                .unwrap()
                .revision,
            before + 1
        );
    }

    #[test]
    fn castle_buy_persist_failure_restores_gold_inventory_allocator_and_stock_then_retry_once() {
        let (mut session, config, item, template) = fixture(false);
        let before = inventory_image(&session);
        let stock = stock_image(&session);
        let allocated = session
            .app
            .world()
            .resource::<InventoryResource>()
            .reserved_item_unique_ids
            .clone();
        let saved = serde_json::to_value(durable(&config)).unwrap();
        config.inject_account_store_transaction_fault(AccountStoreTransactionFault::BeforePersist);
        assert!(!gained(&buy(&mut session, item)));
        assert_eq!(inventory_image(&session), before);
        assert_eq!(stock_image(&session), stock);
        assert_eq!(
            session
                .app
                .world()
                .resource::<InventoryResource>()
                .reserved_item_unique_ids,
            allocated
        );
        assert_eq!(
            session.app.world().resource::<PlayerRuntimeResource>().gold,
            50_000
        );
        assert_eq!(serde_json::to_value(durable(&config)).unwrap(), saved);
        assert_eq!(treasury(&config), 0);
        assert!(gained(&buy(&mut session, item)));
        assert_eq!(
            treasury(&config),
            npc::crystal_npc_purchase_cost(&template, 2, 1.25)
                - npc::crystal_npc_purchase_cost(&template, 2, 1.0)
        );
    }

    #[test]
    fn castle_tax_overflow_rolls_back_the_ordinary_preview_without_an_item_ack() {
        let (mut session, config, item, _) = fixture(false);
        config
            .account_store
            .lock()
            .unwrap()
            .shared_conquests
            .get_mut(&SABUK_INDEX)
            .unwrap()
            .gold = u32::MAX;
        let before = inventory_image(&session);
        let revision = durable(&config).revision;
        assert!(!gained(&buy(&mut session, item)));
        assert_eq!(inventory_image(&session), before);
        assert_eq!(
            session.app.world().resource::<PlayerRuntimeResource>().gold,
            50_000
        );
        assert_eq!(durable(&config).revision, revision);
        assert_eq!(treasury(&config), u32::MAX);
    }

    #[test]
    fn castle_sell_is_untaxed_and_stock_rollback_preserves_the_exact_item() {
        let (mut session, config, item, _) = fixture(false);
        let packets = buy(&mut session, item);
        let unique_id = packets
            .iter()
            .find_map(|p| match p {
                ServerPacket::GainedItem { item } => Some(item.unique_id),
                _ => None,
            })
            .unwrap();
        // The ordinary SellItem path sells a bag item. A potion bought into a
        // belt cell must go through the same MoveItem operation the UI uses.
        move_purchased_item_to_bag(&mut session, unique_id);
        let taxed = treasury(&config);
        let before = inventory_image(&session);
        let stock = stock_image(&session);
        let gold = session.app.world().resource::<PlayerRuntimeResource>().gold;
        let revision = durable(&config).revision;
        config.inject_account_store_transaction_fault(AccountStoreTransactionFault::BeforePersist);
        let failed = session.try_shared_conquest_sell(unique_id, 1).unwrap();
        assert!(failed
            .iter()
            .any(|p| matches!(p, ServerPacket::SellItem { success: false, .. })));
        assert!(!failed
            .iter()
            .any(|p| matches!(p, ServerPacket::GainedGold { .. })));
        assert_eq!(inventory_image(&session), before);
        assert_eq!(stock_image(&session), stock);
        assert_eq!(durable(&config).revision, revision);
        assert_eq!(
            session.app.world().resource::<PlayerRuntimeResource>().gold,
            gold
        );
        let sold = session.try_shared_conquest_sell(unique_id, 1).unwrap();
        assert!(sold
            .iter()
            .any(|p| matches!(p, ServerPacket::SellItem { success: true, .. })));
        assert_eq!(treasury(&config), taxed);
        assert!(durable(&config).gold > gold);
        assert_ne!(stock_image(&session), stock);
        assert_eq!(
            durable(&config).npc_buy_back_items_json,
            snapshot_active_character_save(session.app.world())
                .unwrap()
                .npc_buy_back_items_json
        );
    }

    #[test]
    fn castle_buy_packet_fills_belt_then_bag_and_persists_each_quantity() {
        let (mut session, config, item, template) = fixture(false);
        let key = super::super::items::crystal_item_key_for_template(&template);
        let quantity = |session: &SimulationSession| {
            let inventory = session.app.world().resource::<InventoryResource>();
            inventory
                .inventory_items
                .iter()
                .chain(inventory.belt_items.iter())
                .filter(|item| item.key == key)
                .map(|item| item.quantity)
                .sum::<u32>()
        };
        let mut bought_into_bag = false;
        // Buying a full stack fills an existing belt stack or one belt slot;
        // the potion's belt range contains at most six slots. The next full
        // stack must reach a free bag cell without changing its purchase cost.
        for _ in 0..7 {
            let before = snapshot_active_character_save(session.app.world()).unwrap();
            let before_quantity = quantity(&session);
            let packets = session.handle_packet(ClientPacket::BuyItem {
                item_index: item,
                count: template.stack_size,
                panel_type: 0,
            });
            assert!(gained(&packets));
            assert_eq!(
                quantity(&session),
                before_quantity + u32::from(template.stack_size)
            );
            let after = snapshot_active_character_save(session.app.world()).unwrap();
            let saved = durable(&config);
            assert_eq!(saved.inventory_items_json, after.inventory_items_json);
            assert_eq!(saved.belt_items_json, after.belt_items_json);
            assert_eq!(saved.gold, after.gold);
            assert_eq!(saved.revision, before.revision + 1);
            if after.inventory_items_json != before.inventory_items_json {
                bought_into_bag = true;
                break;
            }
        }
        assert!(
            bought_into_bag,
            "full belt must send further purchased stock into the bag"
        );
    }

    #[test]
    fn stale_character_revision_rejects_before_mutating_gold_or_items() {
        let (mut session, config, item, _) = fixture(false);
        config
            .account_store
            .lock()
            .unwrap()
            .accounts
            .get_mut("demo")
            .unwrap()
            .saves
            .get_mut(&0)
            .unwrap()
            .revision += 1;
        let before = inventory_image(&session);
        assert!(!gained(&buy(&mut session, item)));
        assert_eq!(inventory_image(&session), before);
        assert_eq!(
            session.app.world().resource::<PlayerRuntimeResource>().gold,
            50_000
        );
        assert_eq!(treasury(&config), 0);
    }

    #[test]
    fn changed_membership_or_capture_invalidates_a_prepared_quote_before_commit() {
        for change_membership in [false, true] {
            let (session, config, _, _) = fixture(false);
            let before = snapshot_active_character_save(session.app.world()).unwrap();
            let service = eligible_service(session.app.world(), true).unwrap();
            let identity = Stage5FriendIdentity {
                account_id: "demo".into(),
                character_index: 0,
            };
            let quote = config
                .castle_trade_quote(&identity, &before, &service)
                .unwrap();
            {
                let mut store = config.account_store.lock().unwrap();
                if change_membership {
                    store
                        .shared_guilds
                        .get_mut(GUILD)
                        .unwrap()
                        .members
                        .push(SharedGuildMember {
                            membership_epoch: 0,
                            identity: identity.clone(),
                            name: before.character.name.clone(),
                            rank_index: 0,
                        });
                } else {
                    store
                        .shared_conquests
                        .get_mut(&SABUK_INDEX)
                        .unwrap()
                        .changed()
                        .unwrap();
                }
            }
            let revision = durable(&config).revision;
            assert!(config
                .commit_castle_trade(
                    session.app.world(),
                    &identity,
                    &quote,
                    &before,
                    before.clone(),
                    0
                )
                .is_err());
            assert_eq!(durable(&config).revision, revision);
            assert_eq!(treasury(&config), 0);
        }
    }

    #[test]
    fn unowned_first_trade_initializes_defenses_once_and_charges_no_tax() {
        let (mut session, config, item, template) = fixture(false);
        config
            .account_store
            .lock()
            .unwrap()
            .shared_conquests
            .clear();
        assert!(gained(&buy(&mut session, item)));
        let castle = config.shared_conquest_snapshot(SABUK_INDEX).unwrap();
        assert_eq!(castle.revision, 1);
        assert_eq!(castle.defenses, default_sabuk_defenses());
        assert_eq!(castle.gold, 0);
        assert_eq!(
            durable(&config).gold,
            50_000 - npc::crystal_npc_purchase_cost(&template, 2, 1.0)
        );
        {
            let mut store = config.account_store.lock().unwrap();
            let castle = store.shared_conquests.get_mut(&SABUK_INDEX).unwrap();
            castle.defenses.remove("archer:1");
            castle.changed().unwrap();
        }
        assert!(gained(&buy(&mut session, item)));
        assert!(!config
            .shared_conquest_snapshot(SABUK_INDEX)
            .unwrap()
            .defenses
            .contains_key("archer:1"));
    }

    #[test]
    fn canonical_bindings_exclude_other_mongchon_shops_officer_forged_ids_and_wrong_maps() {
        assert!(merchant_binding(SCRIPT, 1152, "0153"));
        assert!(merchant_binding(
            "MongchonProvince/SabukWall/Butcher",
            927,
            "3"
        ));
        assert!(!merchant_binding(SCRIPT, 172, "0153"));
        assert!(!merchant_binding(SCRIPT, 1152, "3"));
        assert!(!merchant_binding(
            "MongchonProvince/SabukWall/Conquest",
            1146,
            "0150"
        ));
        assert!(!merchant_binding(
            "MongchonProvince/SabukWall/SwampNPC",
            929,
            "3"
        ));
        assert!(!merchant_binding(
            "MongchonProvince/MongchonWall/Potion",
            1152,
            "3"
        ));
    }

    #[test]
    fn pearl_and_disabled_policy_use_the_ordinary_unmodified_path() {
        let (mut session, _, item, _) = fixture(false);
        session
            .app
            .world_mut()
            .resource_mut::<NpcStateResource>()
            .active_npc_service
            .as_mut()
            .unwrap()
            .label_key = "PEARLBUY".into();
        assert!(session.try_shared_conquest_buy(item, 1, 0).is_none());
        session
            .app
            .world_mut()
            .resource_mut::<NpcStateResource>()
            .active_npc_service
            .as_mut()
            .unwrap()
            .label_key = "BUYSELL".into();
        session
            .app
            .world_mut()
            .resource_mut::<RuntimeConfigResource>()
            .config
            .conquest_policies
            .clear();
        assert!(session.try_shared_conquest_buy(item, 1, 0).is_none());
    }

    #[test]
    fn low_gold_or_invalid_quantity_preserves_ordinary_admission_without_persisting_tax() {
        let (mut session, config, item, _) = fixture(false);
        session
            .app
            .world_mut()
            .resource_mut::<PlayerRuntimeResource>()
            .gold = 0;
        let revision = durable(&config).revision;
        let before = inventory_image(&session);
        assert!(buy(&mut session, item).is_empty());
        assert!(session
            .try_shared_conquest_buy(item, 0, 0)
            .unwrap()
            .is_empty());
        assert_eq!(inventory_image(&session), before);
        assert_eq!(durable(&config).revision, revision);
        assert_eq!(treasury(&config), 0);
    }

    #[test]
    fn tax_math_uses_source_f32_rate_and_independently_truncated_totals() {
        assert_eq!(source_price_rate(100, 25, false), 1.25);
        assert_eq!(source_price_rate(200, 25, false), 2.5);
        assert_eq!(source_price_rate(100, 25, true), 1.0);
        let mut template = crystal_item_by_index(1).expect("source item");
        template.price = 11;
        let charged =
            npc::crystal_npc_purchase_cost(&template, 1, source_price_rate(100, 25, false));
        let base = npc::crystal_npc_purchase_cost(&template, 1, source_price_rate(100, 0, true));
        assert_eq!((base, charged, charged - base), (11, 13, 2));
    }
}
