//! Normalized Postgres read-side projections.
//!
//! The authoritative account store is the JSON blob pair
//! (`accounts.raw_json` + `character_saves.snapshot_json`). To make operational
//! reads (admin, economy, anti-fraud) queryable without deserializing every
//! account blob in Rust, each authoritative character save also projects into a
//! set of normalized tables **inside the same Postgres transaction**. Because the
//! projection rides the save transaction, the query models never drift from the
//! authoritative snapshot.
//!
//! This module owns:
//! - the ordered, idempotent migration runner ([`apply_migrations`]) shared by the
//!   simulation account store and the admin API, and
//! - the pure projection derivation ([`derive_character_projection`]) plus its
//!   transactional writer ([`write_character_projection`]).
//!
//! The derivation is intentionally tolerant: it parses the per-item / per-mail
//! JSON generically so a single malformed blob can never abort an authoritative
//! save — it simply yields a best-effort projection row.

use postgres::{Client, Transaction};
use serde_json::Value;

use crate::config::{CharacterSaveRecord, Stage5SystemsState};

/// Ordered list of `(version, sql)` migrations. Each SQL file is itself
/// idempotent (`IF NOT EXISTS`), and the runner additionally records applied
/// versions in `schema_migrations` so already-applied files are skipped.
pub const MIGRATIONS: &[(&str, &str)] = &[
    (
        "0001_core",
        include_str!("../../../infra/postgres/migrations/0001_core.sql"),
    ),
    (
        "0002_normalized_projections",
        include_str!("../../../infra/postgres/migrations/0002_normalized_projections.sql"),
    ),
    (
        "0003_zone_owner_leases",
        include_str!("../../../infra/postgres/migrations/0003_zone_owner_leases.sql"),
    ),
    (
        "0004_city_currencies",
        include_str!("../../../infra/postgres/migrations/0004_city_currencies.sql"),
    ),
    (
        "0005_game_economy_outbox",
        include_str!("../../../infra/postgres/migrations/0005_game_economy_outbox.sql"),
    ),
    (
        "0006_game_economy_bootstrap",
        include_str!("../../../infra/postgres/migrations/0006_game_economy_bootstrap.sql"),
    ),
    (
        "0007_admin_read_audits",
        include_str!("../../../infra/postgres/migrations/0007_admin_read_audits.sql"),
    ),
    (
        "0008_ai_daily_reports",
        include_str!("../../../infra/postgres/migrations/0008_ai_daily_reports.sql"),
    ),
    (
        "0009_world_director_approval",
        include_str!("../../../infra/postgres/migrations/0009_world_director_approval.sql"),
    ),
    (
        "0010_commercial_identity",
        include_str!("../../../infra/postgres/migrations/0010_commercial_identity.sql"),
    ),
];

/// Apply every pending migration in order.
///
/// PostgreSQL `CREATE ... IF NOT EXISTS` is not sufficient concurrency control:
/// two fresh processes can still race while PostgreSQL creates the table's
/// implicit row type. Hold one transaction-scoped advisory lock from the schema
/// bootstrap through the last migration record so all Gateway, Zone Host and
/// worker processes serialize only during migration.
pub fn apply_migrations(client: &mut Client) -> Result<(), String> {
    const MIGRATION_ADVISORY_LOCK_ID: i64 = 0x4D49_5232_4D49_4752;

    let mut transaction = client
        .transaction()
        .map_err(|error| format!("schema migration transaction begin failed: {error}"))?;
    transaction
        .query_one(
            "SELECT pg_advisory_xact_lock($1)",
            &[&MIGRATION_ADVISORY_LOCK_ID],
        )
        .map_err(|error| format!("schema migration advisory lock failed: {error}"))?;
    transaction
        .batch_execute(
            "CREATE TABLE IF NOT EXISTS schema_migrations (\
                version TEXT PRIMARY KEY, \
                applied_at TIMESTAMPTZ NOT NULL DEFAULT now())",
        )
        .map_err(|error| format!("schema_migrations bootstrap failed: {error}"))?;

    for (version, sql) in MIGRATIONS {
        let already_applied = transaction
            .query_opt(
                "SELECT 1 FROM schema_migrations WHERE version = $1",
                &[version],
            )
            .map_err(|error| format!("schema_migrations lookup failed for {version}: {error}"))?
            .is_some();
        if already_applied {
            continue;
        }
        transaction
            .batch_execute(sql)
            .map_err(|error| format!("migration {version} failed: {error}"))?;
        transaction
            .execute(
                "INSERT INTO schema_migrations (version) VALUES ($1) ON CONFLICT DO NOTHING",
                &[version],
            )
            .map_err(|error| format!("schema_migrations record failed for {version}: {error}"))?;
    }
    transaction
        .commit()
        .map_err(|error| format!("schema migration transaction commit failed: {error}"))
}

/// A single inventory/equipment/storage row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemRow {
    pub container: &'static str,
    pub ordinal: i32,
    pub slot: i32,
    pub item_key: String,
    pub item_name: String,
    pub unique_id: i64,
    pub quantity: i64,
    pub grade: String,
    pub durability_current: Option<i32>,
    pub durability_max: Option<i32>,
    pub cursed: bool,
    pub sealed: bool,
    pub rental: bool,
}

/// A single mail row owned by the projected character.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MailRow {
    pub mail_id: i64,
    pub sender: String,
    pub recipient: String,
    pub subject: String,
    pub gold: i64,
    pub item_count: i32,
    pub opened: bool,
    pub locked: bool,
    pub claimed: bool,
    pub deleted: bool,
}

/// A single auction listing posted by the projected character.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuctionRow {
    pub listing_id: i64,
    pub seller_name: String,
    pub item_key: String,
    pub price: i64,
    pub sold: bool,
    pub cancelled: bool,
    pub expired: bool,
}

/// A single NPC script flag/value row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NpcStateRow {
    pub kind: &'static str,
    pub state_key: String,
    pub state_value: String,
}

/// Flattened per-character header row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CharacterStateRow {
    pub character_name: String,
    pub class: String,
    pub gender: String,
    pub level: i32,
    pub experience: i64,
    pub max_experience: i64,
    pub gold: i64,
    pub credit: i64,
    pub pk_points: i32,
    pub hp: i32,
    pub max_hp: i32,
    pub mp: i32,
    pub map_file_name: String,
    pub position_x: i32,
    pub position_y: i32,
    pub chat_banned: bool,
    pub guild_name: String,
    pub inventory_count: i32,
    pub belt_count: i32,
    pub storage_count: i32,
    pub equipment_count: i32,
    pub hero_count: i32,
    pub mail_count: i32,
    pub unread_mail_count: i32,
    pub unclaimed_mail_count: i32,
    pub active_auction_count: i32,
    pub has_hero: bool,
}

/// The full normalized projection of one character save.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CharacterProjection {
    pub account_id: String,
    pub character_index: i32,
    pub save_version: i64,
    pub updated_at_ms: i64,
    pub state: CharacterStateRow,
    pub items: Vec<ItemRow>,
    pub mail: Vec<MailRow>,
    pub auctions: Vec<AuctionRow>,
    pub npc_state: Vec<NpcStateRow>,
}

fn enum_label<T: serde::Serialize>(value: &T) -> String {
    match serde_json::to_value(value) {
        Ok(Value::String(label)) => label,
        Ok(other) => other.to_string(),
        Err(_) => String::new(),
    }
}

fn parse_items(blobs: &[String], container: &'static str, rows: &mut Vec<ItemRow>) {
    for (ordinal, blob) in blobs.iter().enumerate() {
        let value: Value = match serde_json::from_str(blob) {
            Ok(value) => value,
            Err(_) => continue,
        };
        let get_str = |key: &str| {
            value
                .get(key)
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string()
        };
        let durability_current = value
            .get("durability_current")
            .and_then(Value::as_u64)
            .map(|v| v as i32);
        let durability_max = value
            .get("durability_max")
            .and_then(Value::as_u64)
            .map(|v| v as i32);
        let grade = value
            .get("grade")
            .map(|grade| match grade {
                Value::String(label) => label.clone(),
                other => other.to_string(),
            })
            .unwrap_or_default();
        let sealed = value
            .get("sealed_expiry_time_binary_datetime")
            .and_then(Value::as_i64)
            .unwrap_or(0)
            != 0;
        let rental = value
            .get("rental_locked")
            .and_then(Value::as_bool)
            .unwrap_or(false)
            || value
                .get("rental_binding_flags")
                .and_then(Value::as_i64)
                .unwrap_or(0)
                != 0;
        rows.push(ItemRow {
            container,
            ordinal: ordinal as i32,
            slot: value.get("slot").and_then(Value::as_u64).unwrap_or(0) as i32,
            item_key: get_str("key"),
            item_name: get_str("name"),
            unique_id: value.get("unique_id").and_then(Value::as_u64).unwrap_or(0) as i64,
            quantity: value.get("quantity").and_then(Value::as_u64).unwrap_or(0) as i64,
            grade,
            durability_current,
            durability_max,
            cursed: value
                .get("cursed")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            sealed,
            rental,
        });
    }
}

/// Derive the full normalized projection from an authoritative save. Pure: no IO,
/// so it is unit-tested without a database.
pub fn derive_character_projection(
    account_id: &str,
    character_index: i32,
    save: &CharacterSaveRecord,
    save_version: i64,
    now_ms: i64,
) -> CharacterProjection {
    let mut items = Vec::new();
    parse_items(&save.inventory_items_json, "inventory", &mut items);
    parse_items(&save.belt_items_json, "belt", &mut items);
    parse_items(&save.storage_items_json, "storage", &mut items);
    parse_items(&save.equipment_items_json, "equipment", &mut items);
    parse_items(&save.hero_inventory_items_json, "hero", &mut items);

    let systems: Option<Stage5SystemsState> = save
        .stage5_systems_json
        .as_deref()
        .and_then(|raw| serde_json::from_str(raw).ok());

    let owner_name = save.character.name.clone();
    let mut mail = Vec::new();
    let mut auctions = Vec::new();
    let mut guild_name = String::new();
    let mut has_hero = false;
    let mut unread_mail_count = 0;
    let mut unclaimed_mail_count = 0;
    let mut active_auction_count = 0;
    if let Some(systems) = systems.as_ref() {
        guild_name = systems.guild.name.clone();
        has_hero = systems.hero.is_some();
        for message in &systems.mail {
            if message.deleted {
                // Still project deleted mail so audits can see the tombstone, but
                // it does not count toward live unread/unclaimed metrics.
            } else {
                if !message.opened {
                    unread_mail_count += 1;
                }
                if !message.claimed {
                    unclaimed_mail_count += 1;
                }
            }
            mail.push(MailRow {
                mail_id: i64::from(message.id),
                sender: message.from.clone(),
                recipient: message.to.clone(),
                subject: message.subject.clone(),
                gold: i64::from(message.gold),
                item_count: message.items.len() as i32,
                opened: message.opened,
                locked: message.locked,
                claimed: message.claimed,
                deleted: message.deleted,
            });
        }
        for listing in &systems.auction {
            let active = !listing.sold && !listing.cancelled && !listing.expired;
            if active {
                active_auction_count += 1;
            }
            auctions.push(AuctionRow {
                listing_id: i64::from(listing.id),
                seller_name: listing.seller.clone(),
                item_key: listing.item_key.clone(),
                price: i64::from(listing.price),
                sold: listing.sold,
                cancelled: listing.cancelled,
                expired: listing.expired,
            });
        }
    }

    let mut npc_state = Vec::new();
    for flag in decode_objects(&save.npc_flag_states_json) {
        let index = flag.get("index").and_then(Value::as_u64).unwrap_or(0);
        let value = flag.get("value").and_then(Value::as_bool).unwrap_or(false);
        npc_state.push(NpcStateRow {
            kind: "flag",
            state_key: index.to_string(),
            state_value: value.to_string(),
        });
    }
    for saved in decode_objects(&save.npc_saved_values_json) {
        let file_name = saved.get("file_name").and_then(Value::as_str).unwrap_or("");
        let group = saved.get("group").and_then(Value::as_str).unwrap_or("");
        let key = saved.get("key").and_then(Value::as_str).unwrap_or("");
        let value = saved
            .get("value")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        npc_state.push(NpcStateRow {
            kind: "value",
            state_key: format!("{file_name}|{group}|{key}"),
            state_value: value,
        });
    }

    let state = CharacterStateRow {
        character_name: owner_name,
        class: enum_label(&save.character.class),
        gender: enum_label(&save.character.gender),
        level: i32::from(save.character.level),
        experience: save.experience,
        max_experience: save.max_experience,
        gold: i64::from(save.gold),
        credit: i64::from(save.credit),
        pk_points: save.pk_points,
        hp: save.hp,
        max_hp: save.max_hp,
        mp: save.mp,
        map_file_name: save.map_file_name.clone(),
        position_x: save.position.x as i32,
        position_y: save.position.y as i32,
        chat_banned: save.chat_banned,
        guild_name,
        inventory_count: save.inventory_items_json.len() as i32,
        belt_count: save.belt_items_json.len() as i32,
        storage_count: save.storage_items_json.len() as i32,
        equipment_count: save.equipment_items_json.len() as i32,
        hero_count: save.hero_inventory_items_json.len() as i32,
        mail_count: mail.len() as i32,
        unread_mail_count,
        unclaimed_mail_count,
        active_auction_count,
        has_hero,
    };

    CharacterProjection {
        account_id: account_id.to_string(),
        character_index,
        save_version,
        updated_at_ms: now_ms,
        state,
        items,
        mail,
        auctions,
        npc_state,
    }
}

fn decode_objects(blobs: &[String]) -> Vec<Value> {
    blobs
        .iter()
        .filter_map(|blob| serde_json::from_str::<Value>(blob).ok())
        .collect()
}

/// Replace the normalized projection rows for a single character inside an
/// existing save transaction. Snapshot semantics: existing rows for the character
/// are deleted and re-inserted so the projection always equals the latest save.
pub fn write_character_projection(
    tx: &mut Transaction<'_>,
    projection: &CharacterProjection,
) -> Result<(), String> {
    let account_id = &projection.account_id;
    let character_index = projection.character_index;

    for table in ["character_items", "character_mail", "character_npc_state"] {
        tx.execute(
            &format!("DELETE FROM {table} WHERE account_id = $1 AND character_index = $2"),
            &[account_id, &character_index],
        )
        .map_err(|error| format!("projection delete {table} failed: {error}"))?;
    }
    tx.execute(
        "DELETE FROM auction_listings WHERE seller_account_id = $1 AND seller_character_index = $2",
        &[account_id, &character_index],
    )
    .map_err(|error| format!("projection delete auction_listings failed: {error}"))?;

    let state = &projection.state;
    tx.execute(
        "INSERT INTO character_state (
            account_id, character_index, character_name, class, gender, level,
            experience, max_experience, gold, credit, pk_points, hp, max_hp, mp,
            map_file_name, position_x, position_y, chat_banned, guild_name,
            inventory_count, belt_count, storage_count, equipment_count, hero_count,
            mail_count, unread_mail_count, unclaimed_mail_count, active_auction_count,
            has_hero, save_version, updated_at_ms, updated_at
        ) VALUES (
            $1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19,
            $20,$21,$22,$23,$24,$25,$26,$27,$28,$29,$30,$31, now()
        )
        ON CONFLICT (account_id, character_index) DO UPDATE SET
            character_name = EXCLUDED.character_name,
            class = EXCLUDED.class,
            gender = EXCLUDED.gender,
            level = EXCLUDED.level,
            experience = EXCLUDED.experience,
            max_experience = EXCLUDED.max_experience,
            gold = EXCLUDED.gold,
            credit = EXCLUDED.credit,
            pk_points = EXCLUDED.pk_points,
            hp = EXCLUDED.hp,
            max_hp = EXCLUDED.max_hp,
            mp = EXCLUDED.mp,
            map_file_name = EXCLUDED.map_file_name,
            position_x = EXCLUDED.position_x,
            position_y = EXCLUDED.position_y,
            chat_banned = EXCLUDED.chat_banned,
            guild_name = EXCLUDED.guild_name,
            inventory_count = EXCLUDED.inventory_count,
            belt_count = EXCLUDED.belt_count,
            storage_count = EXCLUDED.storage_count,
            equipment_count = EXCLUDED.equipment_count,
            hero_count = EXCLUDED.hero_count,
            mail_count = EXCLUDED.mail_count,
            unread_mail_count = EXCLUDED.unread_mail_count,
            unclaimed_mail_count = EXCLUDED.unclaimed_mail_count,
            active_auction_count = EXCLUDED.active_auction_count,
            has_hero = EXCLUDED.has_hero,
            save_version = EXCLUDED.save_version,
            updated_at_ms = EXCLUDED.updated_at_ms,
            updated_at = now()",
        &[
            account_id,
            &character_index,
            &state.character_name,
            &state.class,
            &state.gender,
            &state.level,
            &state.experience,
            &state.max_experience,
            &state.gold,
            &state.credit,
            &state.pk_points,
            &state.hp,
            &state.max_hp,
            &state.mp,
            &state.map_file_name,
            &state.position_x,
            &state.position_y,
            &state.chat_banned,
            &state.guild_name,
            &state.inventory_count,
            &state.belt_count,
            &state.storage_count,
            &state.equipment_count,
            &state.hero_count,
            &state.mail_count,
            &state.unread_mail_count,
            &state.unclaimed_mail_count,
            &state.active_auction_count,
            &state.has_hero,
            &projection.save_version,
            &projection.updated_at_ms,
        ],
    )
    .map_err(|error| format!("projection upsert character_state failed: {error}"))?;

    for item in &projection.items {
        tx.execute(
            "INSERT INTO character_items (
                account_id, character_index, container, ordinal, slot, item_key,
                item_name, unique_id, quantity, grade, durability_current,
                durability_max, cursed, sealed, rental, updated_at_ms
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16)",
            &[
                account_id,
                &character_index,
                &item.container,
                &item.ordinal,
                &item.slot,
                &item.item_key,
                &item.item_name,
                &item.unique_id,
                &item.quantity,
                &item.grade,
                &item.durability_current,
                &item.durability_max,
                &item.cursed,
                &item.sealed,
                &item.rental,
                &projection.updated_at_ms,
            ],
        )
        .map_err(|error| format!("projection insert character_items failed: {error}"))?;
    }

    for message in &projection.mail {
        tx.execute(
            "INSERT INTO character_mail (
                account_id, character_index, mail_id, owner_name, sender, recipient,
                subject, gold, item_count, opened, locked, claimed, deleted, updated_at_ms
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14)
            ON CONFLICT (account_id, character_index, mail_id) DO UPDATE SET
                owner_name = EXCLUDED.owner_name,
                sender = EXCLUDED.sender,
                recipient = EXCLUDED.recipient,
                subject = EXCLUDED.subject,
                gold = EXCLUDED.gold,
                item_count = EXCLUDED.item_count,
                opened = EXCLUDED.opened,
                locked = EXCLUDED.locked,
                claimed = EXCLUDED.claimed,
                deleted = EXCLUDED.deleted,
                updated_at_ms = EXCLUDED.updated_at_ms",
            &[
                account_id,
                &character_index,
                &message.mail_id,
                &projection.state.character_name,
                &message.sender,
                &message.recipient,
                &message.subject,
                &message.gold,
                &message.item_count,
                &message.opened,
                &message.locked,
                &message.claimed,
                &message.deleted,
                &projection.updated_at_ms,
            ],
        )
        .map_err(|error| format!("projection insert character_mail failed: {error}"))?;
    }

    for listing in &projection.auctions {
        let active = !listing.sold && !listing.cancelled && !listing.expired;
        tx.execute(
            "INSERT INTO auction_listings (
                seller_account_id, seller_character_index, listing_id, seller_name,
                item_key, price, sold, cancelled, expired, active, updated_at_ms
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)
            ON CONFLICT (seller_account_id, seller_character_index, listing_id) DO UPDATE SET
                seller_name = EXCLUDED.seller_name,
                item_key = EXCLUDED.item_key,
                price = EXCLUDED.price,
                sold = EXCLUDED.sold,
                cancelled = EXCLUDED.cancelled,
                expired = EXCLUDED.expired,
                active = EXCLUDED.active,
                updated_at_ms = EXCLUDED.updated_at_ms",
            &[
                account_id,
                &character_index,
                &listing.listing_id,
                &listing.seller_name,
                &listing.item_key,
                &listing.price,
                &listing.sold,
                &listing.cancelled,
                &listing.expired,
                &active,
                &projection.updated_at_ms,
            ],
        )
        .map_err(|error| format!("projection insert auction_listings failed: {error}"))?;
    }

    for entry in &projection.npc_state {
        tx.execute(
            "INSERT INTO character_npc_state (
                account_id, character_index, kind, state_key, state_value, updated_at_ms
            ) VALUES ($1,$2,$3,$4,$5,$6)
            ON CONFLICT (account_id, character_index, kind, state_key) DO UPDATE SET
                state_value = EXCLUDED.state_value,
                updated_at_ms = EXCLUDED.updated_at_ms",
            &[
                account_id,
                &character_index,
                &entry.kind,
                &entry.state_key,
                &entry.state_value,
                &projection.updated_at_ms,
            ],
        )
        .map_err(|error| format!("projection insert character_npc_state failed: {error}"))?;
    }

    Ok(())
}

/// Delete projection rows for characters of an account that no longer exist
/// (e.g. after a character delete). `present_indices` are the character indices
/// still owned by the account; rows for any other index are removed so derived
/// aggregates (economy totals, item-holder counts) never include ghosts.
///
/// An empty `present_indices` removes all projection rows for the account, which
/// is correct for an account with no characters.
pub fn retain_character_projections(
    tx: &mut Transaction<'_>,
    account_id: &str,
    present_indices: &[i32],
) -> Result<(), String> {
    for table in [
        "character_items",
        "character_mail",
        "character_npc_state",
        "character_state",
    ] {
        tx.execute(
            &format!("DELETE FROM {table} WHERE account_id = $1 AND character_index <> ALL($2)"),
            &[&account_id, &present_indices],
        )
        .map_err(|error| format!("projection retain {table} failed: {error}"))?;
    }
    tx.execute(
        "DELETE FROM auction_listings \
         WHERE seller_account_id = $1 AND seller_character_index <> ALL($2)",
        &[&account_id, &present_indices],
    )
    .map_err(|error| format!("projection retain auction_listings failed: {error}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::CharacterRecord;
    use mir2_protocol::{MirClass, MirGender};

    fn sample_save() -> CharacterSaveRecord {
        let mut save = CharacterSaveRecord::new(CharacterRecord {
            index: 0,
            name: "Hero".to_string(),
            level: 12,
            class: MirClass::Wizard,
            gender: MirGender::Female,
        });
        save.gold = 5000;
        save.credit = 42;
        save.map_file_name = "0".to_string();
        save.position = mir2_protocol::Point { x: 330, y: 280 };
        save.inventory_items_json = vec![
            r#"{"key":"red-potion","name":"Red Potion","icon":1,"slot":0,"unique_id":7,"container":"Bag1","quantity":15,"description":"","durability_current":null,"durability_max":null,"weight":1,"equip_slot":null,"grade":"Common","attack":0,"defence":0,"heal_hp":50,"heal_mp":0}"#.to_string(),
            r#"{"key":"iron-sword","name":"Iron Sword","icon":2,"slot":1,"unique_id":8,"container":"Bag1","quantity":1,"description":"","durability_current":30,"durability_max":40,"weight":10,"equip_slot":null,"grade":"Rare","cursed":true,"sealed_expiry_time_binary_datetime":99,"rental_locked":true,"attack":5,"defence":0,"heal_hp":0,"heal_mp":0}"#.to_string(),
        ];
        save.equipment_items_json = vec![
            r#"{"key":"bronze-helmet","name":"Bronze Helmet","icon":3,"slot":0,"unique_id":9,"container":"Equipment","quantity":1,"description":"","durability_current":20,"durability_max":25,"weight":5,"equip_slot":"Helmet","grade":"Common","attack":0,"defence":3,"heal_hp":0,"heal_mp":0}"#.to_string(),
        ];
        save.npc_flag_states_json = vec![r#"{"index":42,"value":true}"#.to_string()];
        save.npc_saved_values_json = vec![
            r#"{"file_name":"start","group":"main","key":"progress","value":"3"}"#.to_string(),
        ];

        let mut systems = Stage5SystemsState::default();
        systems.guild.name = "Heroes".to_string();
        systems.mail.push(crate::config::Stage5MailMessage {
            id: 1,
            delivery_nonce: "projection-fixture-mail-1".to_string(),
            from: "GM".to_string(),
            to: "Hero".to_string(),
            subject: "Welcome".to_string(),
            body: "Hi".to_string(),
            gold: 100,
            items: vec![],
            item_states_json: vec![],
            opened: false,
            locked: false,
            claimed: false,
            deleted: false,
        });
        systems.auction.push(crate::config::Stage5AuctionListing {
            id: 5,
            seller: "Hero".to_string(),
            item_key: "iron-sword".to_string(),
            price: 1200,
            currency: crate::config::CurrencyKind::Gold,
            sold: false,
            cancelled: false,
            expired: false,
        });
        save.stage5_systems_json = Some(serde_json::to_string(&systems).unwrap());
        save
    }

    #[test]
    fn derives_items_across_containers() {
        let projection = derive_character_projection("acc", 0, &sample_save(), 3, 1000);
        assert_eq!(projection.items.len(), 3);
        let sword = projection
            .items
            .iter()
            .find(|item| item.item_key == "iron-sword")
            .expect("sword projected");
        assert_eq!(sword.container, "inventory");
        assert_eq!(sword.quantity, 1);
        assert_eq!(sword.unique_id, 8);
        assert_eq!(sword.durability_current, Some(30));
        assert_eq!(sword.durability_max, Some(40));
        assert!(sword.cursed);
        assert!(sword.sealed);
        assert!(sword.rental);
        let helmet = projection
            .items
            .iter()
            .find(|item| item.item_key == "bronze-helmet")
            .expect("helmet projected");
        assert_eq!(helmet.container, "equipment");
    }

    #[test]
    fn derives_state_header_counts() {
        let projection = derive_character_projection("acc", 0, &sample_save(), 3, 1000);
        let state = &projection.state;
        assert_eq!(state.gold, 5000);
        assert_eq!(state.credit, 42);
        assert_eq!(state.level, 12);
        assert_eq!(state.class, "Wizard");
        assert_eq!(state.guild_name, "Heroes");
        assert_eq!(state.inventory_count, 2);
        assert_eq!(state.equipment_count, 1);
        assert_eq!(state.mail_count, 1);
        assert_eq!(state.unread_mail_count, 1);
        assert_eq!(state.unclaimed_mail_count, 1);
        assert_eq!(state.active_auction_count, 1);
    }

    #[test]
    fn derives_mail_auction_and_npc_state() {
        let projection = derive_character_projection("acc", 0, &sample_save(), 3, 1000);
        assert_eq!(projection.mail.len(), 1);
        assert_eq!(projection.mail[0].sender, "GM");
        assert_eq!(projection.mail[0].recipient, "Hero");
        assert_eq!(projection.mail[0].gold, 100);
        assert_eq!(projection.auctions.len(), 1);
        assert_eq!(projection.auctions[0].item_key, "iron-sword");
        assert_eq!(projection.auctions[0].price, 1200);
        assert!(projection
            .npc_state
            .iter()
            .any(|row| row.kind == "flag" && row.state_key == "42" && row.state_value == "true"));
        assert!(projection
            .npc_state
            .iter()
            .any(|row| row.kind == "value" && row.state_key == "start|main|progress"));
    }

    #[test]
    fn malformed_item_blob_is_skipped_not_fatal() {
        let mut save = sample_save();
        save.inventory_items_json
            .push("{ this is not json".to_string());
        let projection = derive_character_projection("acc", 0, &save, 1, 0);
        // The two valid inventory items + one equipment item still project.
        assert_eq!(projection.items.len(), 3);
    }
}
