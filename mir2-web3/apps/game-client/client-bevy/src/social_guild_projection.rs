//! Ordinary Crystal GuildMemberChange projection. A delta is never a full roster.

use std::collections::BTreeSet;

use serde_json::Value;

use super::{
    GuildMemberModel, GuildModel, GuildRankChange, GuildRankModel, SocialViewer,
    GUILD_PERMISSION_KEYS, MAX_GUILD_MEMBERS, MAX_GUILD_RANKS,
};

/// Validate and project into a clone; malformed or uncorrelated input commits nothing.
pub(super) fn apply(
    guild: &mut GuildModel,
    payload: &Value,
    viewer: Option<&SocialViewer>,
) -> bool {
    let Some(next) = project(guild, payload, viewer) else {
        return false;
    };
    *guild = next;
    true
}

fn project(
    guild: &GuildModel,
    payload: &Value,
    viewer: Option<&SocialViewer>,
) -> Option<GuildModel> {
    payload.as_object()?;
    if viewer.is_some_and(|viewer| !identity_name(&viewer.name) || viewer.character_index < 0) {
        return None;
    }
    let status = byte(payload.get("status")?)?;
    if status > 8 && status != 255 {
        return None;
    }
    let actor = payload.get("name")?.as_str()?;
    if !(status == 255 && actor.is_empty()) && !identity_name(actor) {
        return None;
    }
    let target_index = byte(field(payload, "rank_index", "rankIndex")?)?;
    let mut delta = parse_ranks(payload.get("ranks")?)?;
    if (status <= 5 && !delta.is_empty()) || ((6..=8).contains(&status) && delta.len() != 1) {
        return None;
    }
    let mut next = guild.clone();
    // Existing nested and flattened views must agree before a delta can operate on them.
    // A full roster may replace an incomplete/invalid old projection instead.
    if status != 255 && flatten(&next.ranks)? != next.members {
        return None;
    }
    match status {
        255 => {
            next.ranks = delta;
            next.members = flatten(&next.ranks)?;
            next.member_count = u16::try_from(next.members.len()).ok()?;
            next.roster_complete = true;
            next.roster_needs_refresh = false;
            confirm_own_rank(&mut next, viewer);
        }
        0 | 1 => {
            let member = next
                .ranks
                .iter_mut()
                .flat_map(|rank| &mut rank.members)
                .find(|member| member.name == actor);
            if let Some(member) = member {
                member.online = status == 1;
            } else {
                require_roster(&mut next);
            }
        }
        2 => {
            // Name-only admission has no character id. Do not fabricate a member.
            next.member_count = next.member_count.saturating_add(1);
            require_roster(&mut next);
        }
        3 | 4 => {
            if viewer.is_some_and(|viewer| viewer.name == actor) {
                // The authenticated viewer, not a guessed rank/member name, left this guild.
                let scope_epoch = if next.name.is_some() {
                    next.scope_epoch.checked_add(1)?
                } else {
                    next.scope_epoch
                };
                next = GuildModel {
                    scope_epoch,
                    ..Default::default()
                };
                revoke_own_rank(&mut next);
            } else {
                let found = next.members.iter().any(|member| member.name == actor);
                for rank in &mut next.ranks {
                    rank.members.retain(|member| member.name != actor);
                }
                next.member_count = next.member_count.saturating_sub(1);
                if !found {
                    require_roster(&mut next);
                }
            }
        }
        5 => {
            let target = next
                .ranks
                .iter()
                .position(|rank| rank.index == i32::from(target_index));
            let source = next.ranks.iter().enumerate().find_map(|(index, rank)| {
                rank.members
                    .iter()
                    .position(|member| member.name == actor)
                    .map(|member| (index, member))
            });
            let may_be_self = viewer.map_or_else(
                || source.is_some_and(|(index, _)| next.ranks[index].index == next.my_rank_id),
                |viewer| viewer.name == actor,
            );
            if let (Some(target), Some((source, member))) = (target, source) {
                let mut member = next.ranks[source].members.remove(member);
                member.rank_index = Some(target_index);
                member.rank_name = Some(next.ranks[target].name.clone());
                next.ranks[target].members.push(member);
            } else {
                require_roster(&mut next);
            }
            // The Gateway confirms the viewer's new role with status 8, not this peer delta.
            if may_be_self {
                revoke_own_rank(&mut next);
                if viewer.is_none() {
                    require_roster(&mut next);
                }
            }
        }
        6 => {
            let added = delta.pop()?;
            if added.options != 0 || !added.members.is_empty() {
                return None;
            }
            let mut ordered = next.ranks.clone();
            ordered.sort_by_key(|rank| rank.index);
            let count = ordered.len();
            if count == 0
                || ordered
                    .iter()
                    .enumerate()
                    .any(|(index, rank)| rank.index != index as i32)
            {
                require_roster(&mut next);
                revoke_own_rank(&mut next);
            } else {
                let insertion = if count > 1 { count - 1 } else { 1 };
                if count >= MAX_GUILD_RANKS || added.index != insertion as i32 {
                    return None;
                }
                for rank in &mut ordered {
                    if rank.index >= insertion as i32 {
                        rank.index = rank.index.checked_add(1)?;
                        let index = u8::try_from(rank.index).ok()?;
                        for member in &mut rank.members {
                            member.rank_index = Some(index);
                        }
                    }
                }
                if viewer.is_none() && next.my_rank_id >= insertion as i32 {
                    next.my_rank_id = next.my_rank_id.checked_add(1)?;
                }
                ordered.insert(insertion, added);
                next.ranks = ordered;
                confirm_own_rank(&mut next, viewer);
            }
        }
        7 => {
            if next.name.is_none() {
                return None;
            }
            let updated = delta.pop()?;
            let index = u8::try_from(updated.index).ok()?;
            if next
                .ranks
                .iter()
                .filter(|rank| rank.index == updated.index)
                .count()
                != 1
            {
                return None;
            }
            let affects_self = viewer.map_or_else(
                || next.my_rank_id == updated.index,
                |viewer| {
                    own_rank(&next.ranks, viewer).is_some_and(|rank| rank.index == updated.index)
                        || own_rank(std::slice::from_ref(&updated), viewer).is_some()
                },
            );
            let revision = next
                .rank_change
                .as_ref()
                .map_or(0, |change| change.revision)
                .checked_add(1)?;
            let change = GuildRankChange {
                revision,
                scope_epoch: next.scope_epoch,
                actor: actor.to_owned(),
                rank_index: index,
                rank_name: updated.name.clone(),
                options: Some(updated.options),
            };
            replace_rank(&mut next.ranks, updated)?;
            if affects_self {
                confirm_own_rank(&mut next, viewer);
            }
            next.rank_change = Some(change);
        }
        8 => {
            let viewer = viewer?;
            if next.name.is_none() {
                return None;
            }
            let updated = delta.pop()?;
            // The packet name is the actor. Only the nested stable viewer identity proves self.
            own_rank(std::slice::from_ref(&updated), viewer)?;
            let had_target = next.ranks.iter().any(|rank| rank.index == updated.index);
            replace_rank(&mut next.ranks, updated)?;
            if !had_target {
                require_roster(&mut next);
            }
            confirm_own_rank(&mut next, Some(viewer));
        }
        _ => return None,
    }
    next.members = flatten(&next.ranks)?;
    if guild.members.is_empty() && !next.members.is_empty() {
        next.pending_invite_from = None;
    }
    next.rank_packet_status = Some(status);
    Some(next)
}

fn require_roster(guild: &mut GuildModel) {
    guild.roster_complete = false;
    guild.roster_needs_refresh = true;
}

fn revoke_own_rank(guild: &mut GuildModel) {
    guild.my_rank_id = -1;
    guild.rank_name = None;
    guild.my_options = 0;
    guild.permissions.clear();
}

fn confirm_own_rank(guild: &mut GuildModel, viewer: Option<&SocialViewer>) {
    let rank = match viewer {
        Some(viewer) => own_rank(&guild.ranks, viewer),
        None if guild.name.is_some() => guild
            .ranks
            .iter()
            .find(|rank| rank.index == guild.my_rank_id),
        None => None,
    };
    let role = rank.map(|rank| (rank.index, rank.name.clone(), rank.options));
    if let Some((index, name, options)) = role {
        guild.my_rank_id = index;
        guild.rank_name = Some(name);
        guild.my_options = options;
        guild.permissions = GUILD_PERMISSION_KEYS
            .iter()
            .enumerate()
            .filter(|(bit, _)| options & (1_u8 << *bit) != 0)
            .map(|(_, key)| (*key).to_owned())
            .collect();
    } else {
        revoke_own_rank(guild);
    }
}

fn own_rank<'a>(ranks: &'a [GuildRankModel], viewer: &SocialViewer) -> Option<&'a GuildRankModel> {
    ranks.iter().find(|rank| {
        rank.members
            .iter()
            .any(|member| member.id == viewer.character_index && member.name == viewer.name)
    })
}

/// Replace a real rank while moving only explicitly identified existing members between ranks.
fn replace_rank(ranks: &mut Vec<GuildRankModel>, updated: GuildRankModel) -> Option<()> {
    let old = flatten(ranks)?;
    for member in &updated.members {
        if old.iter().any(|existing| {
            existing.id == member.id && existing.name != member.name
                || existing.name.eq_ignore_ascii_case(&member.name) && existing.id != member.id
        }) {
            return None;
        }
    }
    let target = ranks.iter().position(|rank| rank.index == updated.index);
    for rank in ranks.iter_mut().filter(|rank| rank.index != updated.index) {
        rank.members
            .retain(|member| !updated.members.iter().any(|next| next.id == member.id));
    }
    if let Some(target) = target {
        ranks[target] = updated;
    } else {
        ranks.push(updated);
    }
    flatten(ranks)?;
    Some(())
}

fn identity_name(text: &str) -> bool {
    !text.trim().is_empty() && text.chars().count() <= 32 && !text.contains('\0')
}

fn rank_name(text: &str) -> bool {
    !text.is_empty() && text.encode_utf16().count() <= 20 && !text.contains('\\')
}

fn byte(value: &Value) -> Option<u8> {
    value.as_u64().and_then(|value| u8::try_from(value).ok())
}

fn field<'a>(value: &'a Value, snake: &str, camel: &str) -> Option<&'a Value> {
    match (value.get(snake), value.get(camel)) {
        (Some(left), Some(right)) if left != right => None,
        (Some(value), _) | (_, Some(value)) => Some(value),
        _ => None,
    }
}

fn parse_ranks(value: &Value) -> Option<Vec<GuildRankModel>> {
    let rows = value.as_array()?;
    if rows.len() > MAX_GUILD_RANKS {
        return None;
    }
    let mut total = 0_usize;
    let mut ranks = Vec::with_capacity(rows.len());
    for value in rows {
        value.as_object()?;
        let name = value.get("name")?.as_str()?;
        if !rank_name(name) {
            return None;
        }
        let index = byte(value.get("index")?)?;
        let options = byte(value.get("options")?)?;
        let members = value.get("members")?.as_array()?;
        total = total.checked_add(members.len())?;
        if total > MAX_GUILD_MEMBERS {
            return None;
        }
        let mut parsed = Vec::with_capacity(members.len());
        for value in members {
            value.as_object()?;
            let member_name = value.get("name")?.as_str()?;
            if !identity_name(member_name) {
                return None;
            }
            let id = i32::try_from(value.get("id")?.as_i64()?).ok()?;
            if id < 0 {
                return None;
            }
            let date_snake = value.get("last_login_binary_datetime");
            let date_camel = value.get("lastLoginBinaryDatetime");
            if date_snake
                .zip(date_camel)
                .is_some_and(|(left, right)| left != right)
            {
                return None;
            }
            let last_login_binary_datetime = match date_snake.or(date_camel) {
                Some(value) => value.as_i64()?,
                None => 0,
            };
            parsed.push(GuildMemberModel {
                name: member_name.to_owned(),
                id,
                online: value.get("online")?.as_bool()?,
                last_login_binary_datetime,
                rank_name: Some(name.to_owned()),
                rank_index: Some(index),
            });
        }
        ranks.push(GuildRankModel {
            name: name.to_owned(),
            index: i32::from(index),
            options,
            members: parsed,
        });
    }
    flatten(&ranks)?;
    Some(ranks)
}

fn flatten(ranks: &[GuildRankModel]) -> Option<Vec<GuildMemberModel>> {
    if ranks.len() > MAX_GUILD_RANKS {
        return None;
    }
    let mut indices = BTreeSet::new();
    let mut ids = BTreeSet::new();
    let mut names = BTreeSet::new();
    let mut members = Vec::new();
    for rank in ranks {
        let index = u8::try_from(rank.index).ok()?;
        if !rank_name(&rank.name) || !indices.insert(index) {
            return None;
        }
        for member in &rank.members {
            if !identity_name(&member.name)
                || member.id < 0
                || member.rank_index != Some(index)
                || member.rank_name.as_deref() != Some(rank.name.as_str())
                || !ids.insert(member.id)
                || !names.insert(member.name.to_ascii_lowercase())
            {
                return None;
            }
            if members.len() >= MAX_GUILD_MEMBERS {
                return None;
            }
            members.push(member.clone());
        }
    }
    Some(members)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mir2_protocol::{GuildMember, GuildRank, ServerPacket};
    use serde_json::json;

    fn viewer() -> SocialViewer {
        SocialViewer {
            name: "Alice".into(),
            character_index: 3,
        }
    }

    fn rank(index: i32, name: &str, options: u8, members: &[(&str, i32)]) -> GuildRank {
        GuildRank {
            index,
            name: name.into(),
            options,
            members: members
                .iter()
                .map(|(name, id)| GuildMember {
                    name: (*name).into(),
                    id: *id,
                    online: true,
                    last_login_binary_datetime: 123,
                    has_voted: false,
                })
                .collect(),
        }
    }

    fn ranks() -> Vec<GuildRank> {
        vec![
            rank(0, "Leader", 255, &[("Leader", 1)]),
            rank(1, "Officer", 65, &[("Officer", 2)]),
            rank(2, "Scouts", 9, &[("Alice", 3), ("Bob", 4)]),
        ]
    }

    fn packet(status: u8, name: &str, rank_index: u8, ranks: Vec<GuildRank>) -> Value {
        let serialized = serde_json::to_value(ServerPacket::GuildMemberChange {
            status,
            name: name.into(),
            rank_index,
            ranks,
        })
        .expect("actual ServerPacket serializes");
        serialized
            .as_object()
            .unwrap()
            .values()
            .next()
            .unwrap()
            .clone()
    }

    fn full(ranks: Vec<GuildRank>) -> Value {
        packet(255, "", 0, ranks)
    }
    fn delta(status: u8, target: &str, rank: u8) -> Value {
        packet(status, target, rank, vec![])
    }

    fn seeded() -> GuildModel {
        let mut guild = GuildModel {
            name: Some("Rank Guild".into()),
            my_rank_id: 2,
            scope_epoch: 7,
            notice: vec!["preserved notice".into()],
            gold: 99123,
            storage_items: vec![None; 112],
            ..Default::default()
        };
        assert!(apply(&mut guild, &full(ranks()), Some(&viewer())));
        guild
    }

    fn member<'a>(guild: &'a GuildModel, name: &str) -> &'a GuildMemberModel {
        guild
            .members
            .iter()
            .find(|member| member.name == name)
            .unwrap()
    }

    fn assert_coherent(guild: &GuildModel) {
        assert_eq!(
            flatten(&guild.ranks).as_deref(),
            Some(guild.members.as_slice())
        );
    }

    #[test]
    fn native_guild_projection_full255_establishes_complete_and_stable_own_role() {
        let guild = seeded();
        assert!(guild.roster_complete);
        assert!(!guild.roster_needs_refresh);
        assert_eq!(guild.rank_packet_status, Some(255));
        assert_eq!(guild.ranks.len(), 3);
        assert_eq!(guild.members.len(), 4);
        assert_eq!(guild.member_count, 4);
        assert_eq!(guild.my_rank_id, 2);
        assert_eq!(guild.rank_name.as_deref(), Some("Scouts"));
        assert_eq!(guild.permissions, ["CanChangeRank", "CanStoreItem"]);
        assert_eq!(guild.gold, 99123);
        assert_eq!(guild.notice, ["preserved notice"]);
        assert_coherent(&guild);
    }

    #[test]
    fn native_guild_projection_online_empty_deltas_do_not_clear_the_roster() {
        let mut guild = seeded();
        let before = guild.clone();
        assert!(apply(&mut guild, &delta(0, "Bob", 0), Some(&viewer())));
        assert!(!member(&guild, "Bob").online);
        assert_eq!(guild.ranks.len(), 3);
        assert_eq!(guild.members.len(), 4);
        assert_eq!(guild.my_options, before.my_options);
        assert!(guild.roster_complete);
        assert!(apply(&mut guild, &delta(1, "Bob", 0), Some(&viewer())));
        assert_eq!(guild.ranks, before.ranks);
        assert_eq!(guild.members, before.members);
        assert_coherent(&guild);
        assert!(apply(&mut guild, &delta(0, "Unseen", 0), Some(&viewer())));
        assert_eq!(guild.members, before.members);
        assert!(!guild.roster_complete);
        assert!(guild.roster_needs_refresh);
    }

    #[test]
    fn native_guild_projection_join_requests_full_without_fabricating_character_id() {
        let mut guild = seeded();
        let before = guild.clone();
        assert!(apply(&mut guild, &delta(2, "Newcomer", 2), Some(&viewer())));
        assert_eq!(guild.ranks, before.ranks);
        assert_eq!(guild.members, before.members);
        assert_eq!(guild.member_count, 5);
        assert!(!guild.roster_complete);
        assert!(guild.roster_needs_refresh);
        assert_eq!(guild.rank_packet_status, Some(2));
        let mut joined = ranks();
        joined[2].members.push(GuildMember {
            name: "Newcomer".into(),
            id: 5,
            online: true,
            last_login_binary_datetime: 0,
            has_voted: false,
        });
        assert!(apply(&mut guild, &full(joined), Some(&viewer())));
        assert!(guild.roster_complete);
        assert!(!guild.roster_needs_refresh);
        assert_eq!(member(&guild, "Newcomer").id, 5);
    }

    #[test]
    fn native_guild_projection_peer_kick_and_leave_remove_only_the_target() {
        for status in [3, 4] {
            let mut guild = seeded();
            assert!(apply(&mut guild, &delta(status, "Bob", 0), Some(&viewer())));
            assert_eq!(guild.members.len(), 3);
            assert!(guild.members.iter().all(|member| member.name != "Bob"));
            assert_eq!(member(&guild, "Alice").id, 3);
            assert_eq!(guild.my_rank_id, 2);
            assert_eq!(guild.member_count, 3);
            assert_eq!(guild.name.as_deref(), Some("Rank Guild"));
            assert_coherent(&guild);
        }
    }

    #[test]
    fn native_guild_projection_viewer_kick_or_leave_retires_guild_and_permissions() {
        for status in [3, 4] {
            let mut guild = seeded();
            let epoch = guild.scope_epoch;
            assert!(apply(
                &mut guild,
                &delta(status, "Alice", 0),
                Some(&viewer())
            ));
            assert!(guild.name.is_none());
            assert!(guild.ranks.is_empty() && guild.members.is_empty());
            assert!(guild.permissions.is_empty() && guild.my_options == 0);
            assert_eq!(guild.my_rank_id, -1);
            assert!(guild.rank_name.is_none());
            assert_eq!(guild.scope_epoch, epoch + 1);
            assert_eq!(guild.rank_packet_status, Some(status));
            assert!(!guild.roster_complete && !guild.roster_needs_refresh);
        }
    }

    #[test]
    fn native_guild_projection_peer_rank_move_preserves_real_id_and_other_members() {
        let mut guild = seeded();
        assert!(apply(&mut guild, &delta(5, "Bob", 1), Some(&viewer())));
        assert_eq!(member(&guild, "Bob").id, 4);
        assert_eq!(member(&guild, "Bob").rank_index, Some(1));
        assert_eq!(member(&guild, "Bob").rank_name.as_deref(), Some("Officer"));
        assert_eq!(guild.my_rank_id, 2);
        assert_eq!(guild.members.len(), 4);
        assert_coherent(&guild);
        assert!(apply(&mut guild, &delta(5, "Missing", 1), Some(&viewer())));
        assert!(guild.roster_needs_refresh && !guild.roster_complete);
        assert_eq!(guild.members.len(), 4);
    }

    #[test]
    fn native_guild_projection_own_peer_delta_waits_for_identity_verified_status8() {
        let mut guild = seeded();
        assert!(apply(&mut guild, &delta(5, "Alice", 1), Some(&viewer())));
        assert_eq!(member(&guild, "Alice").rank_index, Some(1));
        assert!(guild.permissions.is_empty());
        assert_eq!(guild.my_rank_id, -1);
        let own = packet(
            8,
            "Leader",
            0,
            vec![rank(1, "Officer", 5, &[("Officer", 2), ("Alice", 3)])],
        );
        assert!(apply(&mut guild, &own, Some(&viewer())));
        assert_eq!(guild.my_rank_id, 1);
        assert_eq!(guild.my_options, 5);
        assert_eq!(guild.permissions, ["CanChangeRank", "CanKick"]);
        assert_eq!(guild.members.len(), 4);
        assert_coherent(&guild);
    }

    #[test]
    fn native_guild_projection_status6_inserts_and_renumbers_nested_flat_and_own_rank() {
        let mut guild = seeded();
        assert!(apply(
            &mut guild,
            &packet(6, "Leader", 0, vec![rank(2, "Veterans", 0, &[])]),
            Some(&viewer())
        ));
        assert_eq!(
            guild
                .ranks
                .iter()
                .map(|rank| rank.index)
                .collect::<Vec<_>>(),
            [0, 1, 2, 3]
        );
        assert_eq!(guild.ranks[2].name, "Veterans");
        assert_eq!(guild.ranks[3].name, "Scouts");
        assert_eq!(member(&guild, "Alice").rank_index, Some(3));
        assert_eq!(member(&guild, "Bob").rank_index, Some(3));
        assert_eq!(guild.my_rank_id, 3);
        assert_eq!(guild.members.len(), 4);
        assert_coherent(&guild);
    }

    #[test]
    fn native_guild_projection_status6_partial_base_needs_roster_and_cannot_guess_positions() {
        let mut guild = seeded();
        let mut sparse = ranks();
        sparse[2].index = 9;
        assert!(apply(&mut guild, &full(sparse), Some(&viewer())));
        let before = guild.ranks.clone();
        assert!(apply(
            &mut guild,
            &packet(6, "Leader", 0, vec![rank(2, "New", 0, &[])]),
            Some(&viewer())
        ));
        assert_eq!(guild.ranks, before);
        assert!(guild.roster_needs_refresh && !guild.roster_complete);
        assert!(guild.permissions.is_empty());
        let mut guild = seeded();
        let before = guild.clone();
        for bad in [
            rank(1, "Wrong insertion", 0, &[]),
            rank(2, "Has permissions", 1, &[]),
            rank(2, "Has member", 0, &[("New", 9)]),
        ] {
            assert!(!apply(
                &mut guild,
                &packet(6, "Leader", 0, vec![bad]),
                Some(&viewer())
            ));
            assert_eq!(guild, before);
        }
    }

    #[test]
    fn native_guild_projection_status7_existing_index_updates_options_and_persists_cursor() {
        let mut guild = seeded();
        let before = guild.clone();
        let update = packet(
            7,
            "Leader",
            0,
            vec![rank(2, "Veterans", 129, &[("Alice", 3), ("Bob", 4)])],
        );
        assert!(apply(&mut guild, &update, Some(&viewer())));
        assert_eq!(&guild.ranks[..2], &before.ranks[..2]);
        assert_eq!(guild.members.len(), 4);
        assert_eq!(guild.rank_name.as_deref(), Some("Veterans"));
        assert_eq!(guild.my_options, 129);
        let cursor = guild.rank_change.clone().unwrap();
        assert_eq!(cursor.revision, 1);
        assert_eq!(cursor.scope_epoch, 7);
        assert_eq!(cursor.actor, "Leader");
        assert_eq!(cursor.rank_index, 2);
        assert_eq!(cursor.rank_name, "Veterans");
        assert_eq!(cursor.options, Some(129));
        assert_eq!(guild.gold, before.gold);
        assert_eq!(guild.notice, before.notice);
        assert_eq!(guild.storage_items, before.storage_items);
        assert!(apply(&mut guild, &delta(0, "Bob", 0), Some(&viewer())));
        assert_eq!(guild.rank_change.as_ref(), Some(&cursor));
        let mut refreshed = ranks();
        refreshed[2].name = "Veterans".into();
        refreshed[2].options = 129;
        assert!(apply(&mut guild, &full(refreshed), Some(&viewer())));
        assert_eq!(guild.rank_change.as_ref(), Some(&cursor));
        assert_eq!(guild.rank_packet_status, Some(255));
        assert_coherent(&guild);
    }

    #[test]
    fn native_guild_projection_status7_unknown_index_never_creates_a_rank_or_cursor() {
        let mut guild = seeded();
        let before = guild.clone();
        assert!(!apply(
            &mut guild,
            &packet(7, "Leader", 0, vec![rank(9, "Unknown", 255, &[])]),
            Some(&viewer())
        ));
        assert_eq!(guild, before);
    }

    #[test]
    fn native_guild_projection_status8_requires_both_viewer_name_and_character_id() {
        let mut guild = seeded();
        let before = guild.clone();
        for members in [vec![("Alice", 99)], vec![("Foreign", 3)], vec![("Bob", 4)]] {
            assert!(!apply(
                &mut guild,
                &packet(8, "Alice", 0, vec![rank(1, "Officer", 255, &members)]),
                Some(&viewer())
            ));
            assert_eq!(guild, before);
        }
        let valid = packet(
            8,
            "Leader",
            0,
            vec![rank(1, "Officer", 255, &[("Alice", 3)])],
        );
        assert!(!apply(&mut guild, &valid, None));
        assert_eq!(guild, before);
        assert!(apply(&mut guild, &valid, Some(&viewer())));
        assert_eq!(guild.my_rank_id, 1);
        assert_eq!(guild.my_options, 255);
        assert_eq!(member(&guild, "Alice").id, 3);
        assert_coherent(&guild);
    }

    #[test]
    fn native_guild_projection_status8_new_verified_rank_is_partial_until255() {
        let mut guild = seeded();
        let update = packet(
            8,
            "Leader",
            0,
            vec![rank(4, "New role", 65, &[("Alice", 3)])],
        );
        assert!(apply(&mut guild, &update, Some(&viewer())));
        assert_eq!(guild.my_rank_id, 4);
        assert_eq!(guild.my_options, 65);
        assert!(!guild.roster_complete && guild.roster_needs_refresh);
        assert_eq!(guild.members.len(), 4);
        assert_eq!(guild.ranks[2].members.len(), 1);
        assert_coherent(&guild);
    }

    #[test]
    fn native_guild_projection_full_missing_viewer_or_known_rank_revokes_old_permissions() {
        let mut guild = seeded();
        let mut missing = ranks();
        missing[2].members.remove(0);
        assert!(apply(&mut guild, &full(missing), Some(&viewer())));
        assert!(guild.roster_complete);
        assert!(guild.name.is_some());
        assert!(guild.permissions.is_empty() && guild.my_options == 0);
        assert_eq!(guild.my_rank_id, -1);
        let mut guild = seeded();
        let mut updated = ranks();
        updated[2].options = 5;
        assert!(apply(&mut guild, &full(updated), None));
        assert_eq!(guild.my_rank_id, 2);
        assert_eq!(guild.my_options, 5);
        assert!(apply(&mut guild, &full(ranks()[..2].to_vec()), None));
        assert_eq!(guild.my_rank_id, -1);
        assert!(guild.permissions.is_empty());
    }

    #[test]
    fn native_guild_projection_rejects_unknown_or_wrong_shape_status_without_partial_commit() {
        let mut guild = seeded();
        let before = guild.clone();
        let malformed = [
            json!(null),
            json!({}),
            json!({"status": 9,"name":"Alice","rankIndex":0,"ranks":[]}),
            json!({"status": -1,"name":"Alice","rankIndex":0,"ranks":[]}),
            json!({"status": 256,"name":"Alice","rankIndex":0,"ranks":[]}),
            json!({"status": "255","name":"","rankIndex":0,"ranks":[]}),
            json!({"status": 255,"name":"","ranks":[]}),
            json!({"status": 255,"name":"","rankIndex":256,"ranks":[]}),
            packet(0, "Alice", 0, ranks()),
            packet(5, "Alice", 0, ranks()),
            packet(6, "Leader", 0, vec![]),
            packet(7, "Leader", 0, ranks()),
            packet(8, "Leader", 0, vec![]),
        ];
        for payload in malformed {
            assert!(!apply(&mut guild, &payload, Some(&viewer())), "{payload}");
            assert_eq!(
                guild, before,
                "invalid packet may not update cursor, flags, authority or roster"
            );
        }
    }

    #[test]
    fn native_guild_projection_rejects_missing_identity_duplicate_and_malformed_late_rows_atomically(
    ) {
        let mut guild = seeded();
        let before = guild.clone();
        let good = json!({"index":2,"name":"Scouts","options":9,"members":[{"name":"Alice","id":3,"online":true}]});
        let mut invalid_rows = Vec::new();
        for key in ["index", "name", "options", "members"] {
            let mut row = good.clone();
            row.as_object_mut().unwrap().remove(key);
            invalid_rows.push(row);
        }
        for (key, value) in [
            ("index", json!(-1)),
            ("index", json!(256)),
            ("options", json!(256)),
            ("options", json!("9")),
            ("members", json!(null)),
            ("name", json!("")),
            ("name", json!("A\\B")),
        ] {
            let mut row = good.clone();
            row[key] = value;
            invalid_rows.push(row);
        }
        for key in ["id", "name", "online"] {
            let mut row = good.clone();
            row["members"][0].as_object_mut().unwrap().remove(key);
            invalid_rows.push(row);
        }
        for (key, value) in [
            ("id", json!(-1)),
            ("id", json!(2147483648_i64)),
            ("id", json!("3")),
            ("online", json!(1)),
            ("name", json!("")),
            ("lastLoginBinaryDatetime", json!("123")),
        ] {
            let mut row = good.clone();
            row["members"][0][key] = value;
            invalid_rows.push(row);
        }
        let mut aliases = good.clone();
        aliases["members"][0]["lastLoginBinaryDatetime"] = json!(1);
        aliases["members"][0]["last_login_binary_datetime"] = json!(2);
        invalid_rows.push(aliases);
        let mut duplicate_id = good.clone();
        duplicate_id["members"]
            .as_array_mut()
            .unwrap()
            .push(json!({"name":"Bob","id":3,"online":true}));
        invalid_rows.push(duplicate_id);
        let mut duplicate_name = good.clone();
        duplicate_name["members"]
            .as_array_mut()
            .unwrap()
            .push(json!({"name":"ALICE","id":4,"online":true}));
        invalid_rows.push(duplicate_name);
        for bad in invalid_rows {
            let payload = json!({"status":255,"name":"","rankIndex":0,"ranks":[
                {"index":0,"name":"Leader","options":255,"members":[{"name":"Leader","id":1,"online":true}]},bad]});
            assert!(!apply(&mut guild, &payload, Some(&viewer())), "{payload}");
            assert_eq!(guild, before);
        }
        let duplicate_rank = packet(255, "", 0, vec![rank(0, "A", 0, &[]), rank(0, "B", 0, &[])]);
        assert!(!apply(&mut guild, &duplicate_rank, Some(&viewer())));
        assert_eq!(guild, before);
    }

    #[test]
    fn native_guild_projection_rank_delta_cannot_rebind_an_existing_name_or_character_id() {
        let mut guild = seeded();
        let before = guild.clone();
        for changed in [
            rank(2, "Scouts", 9, &[("Leader", 3)]),
            rank(2, "Scouts", 9, &[("Impostor", 1)]),
        ] {
            assert!(!apply(
                &mut guild,
                &packet(7, "Leader", 0, vec![changed]),
                Some(&viewer())
            ));
            assert_eq!(guild, before);
        }
    }

    #[test]
    fn native_guild_projection_keeps_existing_200_member_limit_and_accepts_real_zero_id() {
        let make = |count: usize| GuildRank {
            index: 0,
            name: "Members".into(),
            options: 0,
            members: (0..count)
                .map(|id| GuildMember {
                    id: id as i32,
                    name: format!("Player{id}"),
                    online: true,
                    last_login_binary_datetime: 0,
                    has_voted: false,
                })
                .collect(),
        };
        let mut guild = seeded();
        assert!(apply(
            &mut guild,
            &full(vec![make(MAX_GUILD_MEMBERS)]),
            None
        ));
        assert_eq!(guild.members.len(), 200);
        assert_eq!(guild.members[0].id, 0);
        let before = guild.clone();
        assert!(!apply(
            &mut guild,
            &full(vec![make(MAX_GUILD_MEMBERS + 1)]),
            None
        ));
        assert_eq!(guild, before);
        let too_many_ranks = (0..=MAX_GUILD_RANKS)
            .map(|index| rank(index as i32, "Rank", 0, &[]))
            .collect();
        assert!(!apply(&mut guild, &full(too_many_ranks), None));
        assert_eq!(guild, before);
    }

    #[test]
    fn native_guild_projection_cursor_and_scope_overflow_fail_without_any_mutation() {
        let mut guild = seeded();
        guild.rank_change = Some(GuildRankChange {
            revision: u64::MAX,
            scope_epoch: 7,
            actor: "Leader".into(),
            rank_index: 2,
            rank_name: "Scouts".into(),
            options: Some(9),
        });
        let before = guild.clone();
        assert!(!apply(
            &mut guild,
            &packet(
                7,
                "Leader",
                0,
                vec![rank(2, "New", 9, &[("Alice", 3), ("Bob", 4)])]
            ),
            Some(&viewer())
        ));
        assert_eq!(guild, before);
        guild.scope_epoch = u64::MAX;
        let before = guild.clone();
        assert!(!apply(&mut guild, &delta(4, "Alice", 0), Some(&viewer())));
        assert_eq!(guild, before);
    }
}
