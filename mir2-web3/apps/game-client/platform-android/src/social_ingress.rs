//! Owner-bound FIFO for the unchanged shared Group/Guild/Trade model.
//! Public packets are read models/receipts, never local transaction authority.
use mir2_client_bevy::{
    native_inventory_ingress::{native_tooltip_info, native_tooltip_source_for_user_item},
    native_player_ingress::NativeUiPlayerCursor,
    social::SocialModel,
};
use serde_json::{Value, json};
use std::collections::VecDeque;

pub(crate) const MAX_SOCIAL_PACKET_BYTES: usize = 512 * 1024;
const MAX_SMALL_PACKET_BYTES: usize = 16 * 1024;
const MAX_MODEL_BYTES: usize = 512 * 1024;
const MAX_PENDING: usize = 64;
const MAX_PENDING_BYTES: usize = 4 * 1024 * 1024;

pub(crate) fn is_social_packet_name(name: &str) -> bool {
    matches!(
        name,
        "SwitchGroup"
            | "DeleteGroup"
            | "DeleteMember"
            | "GroupInvite"
            | "GroupInviteResult"
            | "AddMember"
            | "GroupMembersMap"
            | "GroupMemberInfo"
            | "GuildStatus"
            | "GuildNoticeChange"
            | "GuildNoticeResult"
            | "GuildMemberChange"
            | "GuildStorageGoldChange"
            | "GuildStorageList"
            | "GuildStorageItemChange"
            | "GuildInvite"
            | "GuildInviteResult"
            | "TradeRequest"
            | "TradeAccept"
            | "TradeGold"
            | "TradeItem"
            | "TradeConfirm"
            | "TradeCancel"
            | "DepositTradeItem"
            | "RetrieveTradeItem"
    )
}

pub(crate) fn is_large_social_packet_name(name: &str) -> bool {
    matches!(
        name,
        "GroupMemberInfo"
            | "GuildMemberChange"
            | "GuildStorageList"
            | "TradeItem"
            | "GuildNoticeChange"
    )
}

/// Classification only selects a decoder. Phase, accepted owner, schema and
/// byte/queue checks still precede publication; it cannot bootstrap a player.
pub(crate) fn is_social_packet(raw: &str) -> bool {
    raw.len() <= MAX_SOCIAL_PACKET_BYTES
        && serde_json::from_str::<Value>(raw)
            .ok()
            .is_some_and(|event| {
                event["type"] == "packet"
                    && event["packet"].as_str().is_some_and(is_social_packet_name)
            })
}

#[derive(Clone, Default)]
pub(crate) struct AndroidSocialIngress {
    identity: Option<(u32, String)>,
    cursor: SocialModel,
    staged: VecDeque<String>,
    pending: VecDeque<String>,
}

impl AndroidSocialIngress {
    pub(crate) fn reset(&mut self) {
        *self = Self::default();
    }

    pub(crate) fn clear_scene(&mut self) {
        // Membership, invitations and trade belong to this connection/owner,
        // matching Windows and the shared SceneReset consumer. Keep FIFO too.
        self.cursor.clear_scene();
    }

    pub(crate) fn bind(
        &mut self,
        owner: u32,
        name: &str,
        player: &NativeUiPlayerCursor,
    ) -> Result<(), &'static str> {
        if owner == 0 || name.trim().is_empty() || name.chars().count() > 128 {
            return Err("Invalid social owner");
        }
        if self
            .identity
            .as_ref()
            .is_some_and(|(id, old)| *id != owner || old != name)
        {
            return Err("Social character changed without session boundary");
        }
        // Replay packet-first metadata only against the validated personal
        // owner/cursor. All-or-nothing binding prevents a partial old roster.
        let mut next = self.clone();
        next.identity = Some((owner, name.to_owned()));
        while let Some(raw) = next.staged.pop_front() {
            next.packet(&raw, player)?;
        }
        *self = next;
        Ok(())
    }

    pub(crate) fn packet(
        &mut self,
        raw: &str,
        player: &NativeUiPlayerCursor,
    ) -> Result<bool, &'static str> {
        if raw.len() > MAX_SOCIAL_PACKET_BYTES {
            return Err("Social packet too large");
        }
        let event: Value = serde_json::from_str(raw).map_err(|_| "Invalid social packet")?;
        if event["type"] != "packet" {
            return Ok(false);
        }
        let name = event["packet"].as_str().unwrap_or("");
        if !is_social_packet_name(name) {
            return Ok(false);
        }
        if !is_large_social_packet_name(name) && raw.len() > MAX_SMALL_PACKET_BYTES {
            return Err("Social receipt too large");
        }
        let mut payload = event["payload"].clone();
        if !payload.is_object() {
            return Err("Invalid social payload");
        }
        if payload
            .get("hero")
            .is_some_and(|value| value.as_bool() != Some(false))
        {
            return Ok(false);
        }
        if let Some((owner, character)) = &self.identity {
            if payload
                .get("ownerObjectId")
                .is_some_and(|id| value_u64(Some(id)) != Some(u64::from(*owner)))
                || payload
                    .get("characterName")
                    .is_some_and(|value| value.as_str() != Some(character.as_str()))
            {
                return Ok(false);
            }
        }
        add_social_item_tooltip_sources(name, &mut payload, player);
        // Some shared validators can mutate before rejecting. Never commit
        // such a candidate, or one whose serialized projection cannot queue.
        let mut candidate = self.cursor.clone();
        if !candidate.apply_network_packet(name, &payload) {
            return Err("Invalid shared social model");
        }
        let model = serde_json::to_string(&candidate).map_err(|_| "Invalid social projection")?;
        if model.len() > MAX_MODEL_BYTES {
            return Err("Social model too large");
        }
        serde_json::from_str::<SocialModel>(&model).map_err(|_| "Invalid typed social model")?;
        if self.identity.is_none() {
            self.check_append(raw.len())?;
            self.staged.push_back(raw.to_owned());
        } else {
            self.check_append(model.len())?;
            self.pending.push_back(model);
            self.cursor = candidate;
        }
        Ok(true)
    }

    fn check_append(&self, bytes: usize) -> Result<(), &'static str> {
        if self.staged.len() + self.pending.len() >= MAX_PENDING
            || self
                .staged
                .iter()
                .chain(&self.pending)
                .map(String::len)
                .sum::<usize>()
                .saturating_add(bytes)
                > MAX_PENDING_BYTES
        {
            return Err("Social ingress queue full");
        }
        Ok(())
    }

    pub(crate) fn flush(&mut self, mut push: impl FnMut(String) -> bool) -> bool {
        if self.identity.is_none() {
            return false;
        }
        while let Some(front) = self.pending.front() {
            if !push(front.clone()) {
                return false;
            }
            self.pending.pop_front();
        }
        true
    }

    #[cfg(test)]
    pub(crate) fn pending_count(&self) -> usize {
        self.staged.len() + self.pending.len()
    }

    #[cfg(test)]
    pub(crate) fn model(&self) -> &SocialModel {
        &self.cursor
    }
}

// Read-only wire enrichment mirrors frozen Windows gateway.rs. Template,
// viewer, socket and real-info rules all stay in native_inventory_ingress.
fn enrich_guild_storage_item(value: &mut Value, cursor: &NativeUiPlayerCursor) {
    let Some(item) = value.get("item").cloned() else {
        return;
    };
    let Some(source) = native_tooltip_source_for_user_item(&item, cursor) else {
        return;
    };
    if let Some(object) = value.as_object_mut() {
        object.insert("tooltipSource".to_owned(), json!(source));
    }
}

fn add_social_item_tooltip_sources(
    packet: &str,
    payload: &mut Value,
    cursor: &NativeUiPlayerCursor,
) {
    match packet {
        "GuildStorageList" => {
            if let Some(items) = payload.get_mut("items").and_then(Value::as_array_mut) {
                for item in items.iter_mut().filter(|item| !item.is_null()) {
                    enrich_guild_storage_item(item, cursor);
                }
            }
        }
        "GuildStorageItemChange" => {
            if let Some(item) = payload.get_mut("item").filter(|item| !item.is_null()) {
                enrich_guild_storage_item(item, cursor);
            }
        }
        "TradeItem" => {
            let source_items = payload.get("tradeItems").and_then(Value::as_array).cloned();
            let Some(source_items) = source_items else {
                return;
            };
            let partner_items = source_items
                .into_iter()
                .map(|item| {
                    if item.is_null() {
                        return Value::Null;
                    }
                    let Some(mut entry) = item.as_object().cloned() else {
                        return item;
                    };
                    let item_index = value_i32(item.get("item_index"));
                    if let Some(item_index) = item_index {
                        entry.insert("itemIndex".to_owned(), json!(item_index));
                        let name = native_tooltip_info(item_index)
                            .map(|info| info.name)
                            .unwrap_or_else(|| format!("Item #{item_index}"));
                        entry.insert("name".to_owned(), json!(name));
                    }
                    if let Some(unique_id) = value_u64(item.get("unique_id")) {
                        entry.insert("uniqueId".to_owned(), json!(unique_id));
                    }
                    if let Some(source) = native_tooltip_source_for_user_item(&item, cursor) {
                        entry.insert("tooltipSource".to_owned(), json!(source));
                    }
                    Value::Object(entry)
                })
                .collect();
            if let Some(object) = payload.as_object_mut() {
                object.insert("partnerItems".to_owned(), Value::Array(partner_items));
            }
        }
        _ => {}
    }
}

fn value_i32(value: Option<&Value>) -> Option<i32> {
    value.and_then(|value| {
        value
            .as_i64()
            .and_then(|number| i32::try_from(number).ok())
            .or_else(|| value.as_str()?.parse::<i32>().ok())
    })
}

fn value_u64(value: Option<&Value>) -> Option<u64> {
    value.and_then(value_u64_ref)
}

fn value_u64_ref(value: &Value) -> Option<u64> {
    value
        .as_u64()
        .or_else(|| value.as_str()?.parse::<u64>().ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    use mir2_client_bevy::social::{MAX_GUILD_STORAGE_ITEMS, SocialPendingOperation};

    fn wire(name: &str, payload: Value) -> String {
        json!({"type":"packet","packet":name,"payload":payload}).to_string()
    }

    fn bind(ingress: &mut AndroidSocialIngress) {
        ingress
            .bind(42, "Fixture", &NativeUiPlayerCursor::default())
            .unwrap();
    }

    fn take(ingress: &mut AndroidSocialIngress) -> Vec<SocialModel> {
        let mut rows = Vec::new();
        assert!(ingress.flush(|raw| {
            rows.push(serde_json::from_str(&raw).unwrap());
            true
        }));
        rows
    }

    #[test]
    fn all_25_public_packets_use_the_unchanged_shared_cursor() {
        let packets = vec![
            ("SwitchGroup", json!({"allowGroup":true})),
            ("DeleteGroup", json!({})),
            ("DeleteMember", json!({"name":"Alice"})),
            ("GroupInvite", json!({"name":"Alice"})),
            ("GroupInviteResult", json!({"name":"Alice","success":false})),
            ("AddMember", json!({"name":"Alice"})),
            (
                "GroupMembersMap",
                json!({"playerName":"Alice","playerMap":"0"}),
            ),
            (
                "GroupMemberInfo",
                json!({"members":[{"name":"Alice","leader":true}],"leaderName":"Alice"}),
            ),
            (
                "GuildStatus",
                json!({"guildName":"WireGuild","guildRankName":"Leader","myOptions":255,"myRankId":2}),
            ),
            (
                "GuildNoticeChange",
                json!({"notice":["Authoritative notice"],"update":0}),
            ),
            ("GuildNoticeResult", json!({"success":false})),
            (
                "GuildMemberChange",
                json!({"ranks":[{"name":"Leader","index":2,"options":3,
                "members":[{"name":"Fixture","id":7,"online":true}]}]}),
            ),
            ("GuildStorageGoldChange", json!({"changeType":0,"amount":1})),
            ("GuildStorageList", json!({"items":[]})),
            (
                "GuildStorageItemChange",
                json!({"changeType":1,"from":0,"to":0}),
            ),
            ("GuildInvite", json!({"name":"Alice"})),
            ("GuildInviteResult", json!({"name":"Alice","success":false})),
            ("TradeRequest", json!({"name":"Alice"})),
            ("TradeAccept", json!({"name":"Alice"})),
            ("TradeGold", json!({"amount":10})),
            (
                "TradeItem",
                json!({"tradeItems":[null,{"unique_id":u64::MAX,"item_index":i32::MAX,"count":2}]}),
            ),
            ("TradeConfirm", json!({})),
            ("TradeCancel", json!({"unlock":false})),
            ("DepositTradeItem", json!({"from":0,"to":0,"success":false})),
            ("RetrieveTradeItem", json!({"from":0,"to":0,"success":true})),
        ];
        let player = NativeUiPlayerCursor::default();
        let mut ingress = AndroidSocialIngress::default();
        bind(&mut ingress);
        let mut expected = SocialModel::default();
        for (name, mut payload) in packets {
            assert!(is_social_packet_name(name));
            assert!(
                ingress
                    .packet(&wire(name, payload.clone()), &player)
                    .unwrap(),
                "{name}"
            );
            add_social_item_tooltip_sources(name, &mut payload, &player);
            assert!(expected.apply_network_packet(name, &payload));
            assert_eq!(
                ingress.cursor, expected,
                "Host changed shared {name} semantics"
            );
            assert_eq!(take(&mut ingress), vec![expected.clone()]);
        }
    }

    #[test]
    fn packet_first_data_is_invisible_until_owner_and_replays_in_order() {
        let mut ingress = AndroidSocialIngress::default();
        let player = NativeUiPlayerCursor::default();
        for payload in [
            json!({"name":"Old","ownerObjectId":99}),
            json!({"name":"Alice","ownerObjectId":42,"characterName":"Fixture"}),
            json!({"playerName":"Alice","playerMap":"Bichon"}),
        ] {
            let name = if payload.get("playerMap").is_some() {
                "GroupMembersMap"
            } else {
                "AddMember"
            };
            assert!(ingress.packet(&wire(name, payload), &player).unwrap());
        }
        assert_eq!(ingress.cursor, SocialModel::default());
        assert!(!ingress.flush(|_| panic!("Unbound data published")));
        bind(&mut ingress);
        let rows = take(&mut ingress);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].group.members[0].name, "Alice");
        assert_eq!(rows[1].group.members[0].map.as_deref(), Some("Bichon"));
        assert!(rows.iter().all(|row| row.group.members.len() == 1));
        assert!(ingress.bind(99, "Other", &player).is_err());
        assert_eq!(ingress.cursor, rows[1]);
    }

    #[test]
    fn foreign_owner_and_hero_never_change_or_queue_social_state() {
        let mut ingress = AndroidSocialIngress::default();
        bind(&mut ingress);
        let player = NativeUiPlayerCursor::default();
        for payload in [
            json!({"name":"Alice","hero":true}),
            json!({"name":"Alice","hero":"false"}),
            json!({"name":"Alice","ownerObjectId":99}),
            json!({"name":"Alice","characterName":"Other"}),
        ] {
            assert!(
                !ingress
                    .packet(&wire("AddMember", payload), &player)
                    .unwrap()
            );
        }
        assert_eq!(ingress.cursor, SocialModel::default());
        assert_eq!(ingress.pending_count(), 0);
    }

    #[test]
    fn shared_rejection_and_backpressure_never_commit_partial_candidates() {
        let mut ingress = AndroidSocialIngress::default();
        bind(&mut ingress);
        let player = NativeUiPlayerCursor::default();
        // The shared decoder resizes storage before rejecting an invalid slot.
        let old = ingress.cursor.clone();
        assert!(
            ingress
                .packet(
                    &wire(
                        "GuildStorageItemChange",
                        json!({
            "changeType":1,"from":112,"to":0})
                    ),
                    &player
                )
                .is_err()
        );
        assert_eq!(ingress.cursor, old);
        assert_eq!(ingress.pending_count(), 0);
        let invite = wire("GroupInvite", json!({"name":"Alice"}));
        for _ in 0..MAX_PENDING {
            assert!(ingress.packet(&invite, &player).unwrap());
        }
        let full = ingress.cursor.clone();
        assert!(ingress.packet(&invite, &player).is_err());
        assert_eq!(ingress.cursor, full);
        assert!(!ingress.flush(|_| false));
        assert_eq!(ingress.pending_count(), MAX_PENDING);
        let rows = take(&mut ingress);
        assert_eq!(rows.len(), MAX_PENDING);
        assert_eq!(rows[0].group.pending_invite_epoch, 1);
        assert_eq!(
            rows[MAX_PENDING - 1].group.pending_invite_epoch,
            MAX_PENDING as u64
        );
        assert_eq!(ingress.pending_count(), 0);
    }

    #[test]
    fn session_reset_retires_fifo_but_scene_reset_keeps_personal_models() {
        let mut ingress = AndroidSocialIngress::default();
        bind(&mut ingress);
        ingress
            .packet(
                &wire("AddMember", json!({"name":"Alice"})),
                &NativeUiPlayerCursor::default(),
            )
            .unwrap();
        ingress.clear_scene();
        assert_eq!(ingress.cursor.group.members[0].name, "Alice");
        assert!(ingress.cursor.last_event.is_none());
        assert_eq!(
            take(&mut ingress)[0].last_event.as_ref().unwrap().packet,
            "AddMember"
        );
        ingress.reset();
        assert_eq!(ingress.cursor, SocialModel::default());
        assert!(!ingress.flush(|_| panic!("Old owner FIFO leaked")));
        ingress
            .bind(99, "Other", &NativeUiPlayerCursor::default())
            .unwrap();
        assert!(take(&mut ingress).is_empty());
    }

    #[test]
    fn raw_byte_caps_and_aggregate_staging_are_bounded() {
        let mut ingress = AndroidSocialIngress::default();
        let player = NativeUiPlayerCursor::default();
        let large = wire(
            "GuildNoticeChange",
            json!({"notice":["a"],"probe":"x".repeat(300_000)}),
        );
        assert!(is_social_packet(&large));
        let mut accepted = 0;
        while ingress.packet(&large, &player).is_ok() {
            accepted += 1;
        }
        assert!(accepted > 0 && accepted < MAX_PENDING);
        assert_eq!(ingress.pending_count(), accepted);
        let count = ingress.pending_count();
        assert!(
            ingress
                .packet(
                    &wire("TradeGold", json!({"amount":1,"probe":"x".repeat(20_000)})),
                    &player
                )
                .is_err()
        );
        assert!(
            ingress
                .packet(
                    &wire(
                        "GuildNoticeChange",
                        json!({"notice":["a"],"probe":"x".repeat(MAX_SOCIAL_PACKET_BYTES)})
                    ),
                    &player
                )
                .is_err()
        );
        assert_eq!(ingress.pending_count(), count);
    }

    #[test]
    fn shared_domain_limits_remain_fail_closed() {
        let mut ingress = AndroidSocialIngress::default();
        bind(&mut ingress);
        let player = NativeUiPlayerCursor::default();
        let invalid = [
            (
                "GroupMemberInfo",
                json!({"members":vec![json!({"name":"A"});16]}),
            ),
            (
                "GuildMemberChange",
                json!({"ranks":vec![json!({"name":"R","members":[]});256]}),
            ),
            (
                "GuildMemberChange",
                json!({"ranks":[{"name":"R","members":vec![json!({"name":"A"});201]}]}),
            ),
            ("GuildStorageList", json!({"items":vec![Value::Null;113]})),
            ("GuildNoticeChange", json!({"notice":vec![json!("A");201]})),
            ("TradeItem", json!({"tradeItems":vec![Value::Null;11]})),
            ("TradeItem", json!({"tradeItems":[{}]})),
            ("GuildStatus", json!({"guildName":"G"})),
        ];
        for (name, payload) in invalid {
            assert!(
                ingress.packet(&wire(name, payload), &player).is_err(),
                "{name}"
            );
            assert_eq!(ingress.cursor, SocialModel::default());
            assert_eq!(ingress.pending_count(), 0);
        }
        for name in [
            "stage5Command",
            "qa.giveItem",
            "GuildStorageContents",
            "ObjectGuildNameChanged",
        ] {
            assert!(!is_social_packet_name(name));
            assert!(!ingress.packet(&wire(name, json!({})), &player).unwrap());
        }
    }

    #[test]
    fn item_enrichment_preserves_shared_tooltips_null_slots_and_unsigned_identity() {
        let (index, info) = (0..128)
            .find_map(|i| native_tooltip_info(i).map(|info| (i, info)))
            .expect("An actual unique shared template must be present");
        let player = NativeUiPlayerCursor {
            level: Some(20),
            class_name: Some("Warrior".into()),
            gold: Some(777),
            credit: Some(33),
            ..Default::default()
        };
        let user = json!({"unique_id":u64::MAX,"item_index":index,"count":2,
            "current_dura":12,"max_dura":30,"slots":[null],"added_stats":[{"stat":4,"value":3}]});
        let expected =
            native_tooltip_source_for_user_item(&user, &player).expect("Typed user item tooltip");
        let mut ingress = AndroidSocialIngress::default();
        ingress.bind(42, "Fixture", &player).unwrap();
        ingress
            .packet(
                &wire(
                    "GuildStorageList",
                    json!({"items":[null,{"item":user.clone(),"user_id":-17}]}),
                ),
                &player,
            )
            .unwrap();
        let row = take(&mut ingress).pop().unwrap();
        assert_eq!(row.guild.storage_items.len(), MAX_GUILD_STORAGE_ITEMS);
        assert!(row.guild.storage_items[0].is_none());
        let item = row.guild.storage_items[1].as_ref().unwrap();
        assert_eq!(
            (item.unique_id, item.item_index, item.count, item.user_id),
            (u64::MAX, index, 2, -17)
        );
        assert_eq!(item.tooltip_source.as_ref(), Some(&expected));
        ingress
            .packet(&wire("TradeAccept", json!({"name":"Alice"})), &player)
            .unwrap();
        ingress
            .packet(
                &wire("TradeItem", json!({"tradeItems":[null,user]})),
                &player,
            )
            .unwrap();
        ingress
            .packet(&wire("TradeGold", json!({"amount":50})), &player)
            .unwrap();
        let trade = take(&mut ingress).pop().unwrap().trade;
        assert!(trade.partner_items[0].is_none());
        let item = trade.partner_items[1].as_ref().unwrap();
        assert_eq!(item.unique_id, Some(u64::MAX));
        assert_eq!(item.name.as_deref(), Some(info.name.as_str()));
        assert_eq!(item.tooltip_source.as_ref(), Some(&expected));
        assert_eq!(trade.partner_gold, 50);
        assert_eq!(trade.my_gold, 0);
        assert!(trade.my_items.is_empty());
        assert_eq!((player.gold, player.credit), (Some(777), Some(33)));
    }

    #[test]
    fn matching_receipts_reconcile_shared_pending_without_optimistic_success() {
        let mut ingress = AndroidSocialIngress::default();
        bind(&mut ingress);
        let mut ui = SocialModel::default();
        assert!(ui.begin_pending(SocialPendingOperation::GuildStorageGold {
            change_type: 0,
            amount: 10
        }));
        for (amount, pending) in [(11, 1), (10, 0)] {
            ingress
                .packet(
                    &wire(
                        "GuildStorageGoldChange",
                        json!({"changeType":0,"amount":amount}),
                    ),
                    &NativeUiPlayerCursor::default(),
                )
                .unwrap();
            for row in take(&mut ingress) {
                ui.apply_authoritative(row);
            }
            assert_eq!(ui.pending.len(), pending);
        }
        assert_eq!(ui.guild.gold, 21);
    }

    #[test]
    fn maximum_valid_roster_and_notice_are_not_desktop_small_packet_limited() {
        let mut ingress = AndroidSocialIngress::default();
        bind(&mut ingress);
        let player = NativeUiPlayerCursor::default();
        let notice = wire(
            "GuildNoticeChange",
            json!({"notice":vec!["文".repeat(32);200],"update":0}),
        );
        assert!(notice.len() > 16 * 1024);
        assert!(ingress.packet(&notice, &player).unwrap());
        let roster = wire(
            "GuildMemberChange",
            json!({"ranks":[{"name":"R","index":0,"options":255,
            "members":(0..200).map(|id|json!({"id":id,"name":format!("M{id}{}","文".repeat(24)),"online":true})).collect::<Vec<_>>() }]}),
        );
        assert!(roster.len() > 16 * 1024);
        assert!(ingress.packet(&roster, &player).unwrap());
        let rows = take(&mut ingress);
        assert_eq!(rows[1].guild.notice.len(), 200);
        assert_eq!(rows[1].guild.members.len(), 200);
    }
}
