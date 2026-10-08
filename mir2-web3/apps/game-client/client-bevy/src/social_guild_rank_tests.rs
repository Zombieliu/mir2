//! Real typed server packets. These checks do not accept AOI, network or human UI.
use super::*;
use mir2_protocol::{GuildMember, GuildRank, ServerPacket};

pub(crate) fn rank(index: i32, name: &str, options: u8, members: &[(&str, i32)]) -> GuildRank {
    GuildRank {
        name: name.into(),
        options,
        index,
        members: members
            .iter()
            .map(|(name, id)| GuildMember {
                name: (*name).into(),
                id: *id,
                last_login_binary_datetime: 123,
                has_voted: false,
                online: true,
            })
            .collect(),
    }
}

pub(crate) fn full_ranks(name: &str) -> Vec<GuildRank> {
    vec![
        rank(0, "Leader", 255, &[("Leader", 1)]),
        rank(1, "Officer", 65, &[("Officer", 2)]),
        rank(2, name, 1, &[("Alice", 3), ("Bob", 4)]),
    ]
}

pub(crate) fn status(guild: &str, rank_name: &str, my_rank: i32) -> ServerPacket {
    ServerPacket::GuildStatus {
        guild_name: guild.into(),
        guild_rank_name: rank_name.into(),
        level: 3,
        experience: 41,
        max_experience: 1000,
        gold: 99123,
        spare_points: 2,
        member_count: 4,
        max_members: 50,
        voting: false,
        item_count: 0,
        buff_count: 0,
        my_options: if my_rank == 0 { 255 } else { 1 },
        my_rank_id: my_rank,
    }
}

pub(crate) fn terminal(actor: &str, index: i32, name: &str) -> ServerPacket {
    let ranks = full_ranks(name);
    ServerPacket::GuildMemberChange {
        name: actor.into(),
        rank_index: 0,
        status: 7,
        ranks: vec![ranks[index as usize].clone()],
    }
}

pub(crate) fn full(name: &str) -> ServerPacket {
    ServerPacket::GuildMemberChange {
        name: String::new(),
        rank_index: 0,
        status: 255,
        ranks: full_ranks(name),
    }
}

pub(crate) fn apply_typed(model: &mut SocialModel, packet: ServerPacket) -> bool {
    let serialized = serde_json::to_value(packet).unwrap();
    let (name, payload) = serialized.as_object().unwrap().iter().next().unwrap();
    model.apply_network_packet(name, payload)
}

pub(crate) fn seeded(my_rank: i32) -> SocialModel {
    let mut model = SocialModel::default();
    assert!(apply_typed(
        &mut model,
        status(
            "Rank Guild",
            if my_rank == 0 { "Leader" } else { "Scouts" },
            my_rank
        )
    ));
    assert!(apply_typed(&mut model, full("Scouts")));
    model.guild.notice = vec!["unchanged notice".into()];
    model.guild.storage_items = vec![None; 112];
    model
}

#[test]
fn guild_rank_status7_merges_one_nonzero_rank_retaining_three_ranks_four_members() {
    let mut model = seeded(0);
    let before = model.guild.clone();
    assert!(apply_typed(&mut model, terminal("Leader", 2, "Veterans")));
    assert_eq!(
        model.guild.ranks.len(),
        3,
        "one-rank delta is not a full roster"
    );
    assert_eq!(&model.guild.ranks[..2], &before.ranks[..2]);
    assert_eq!(model.guild.members.len(), 4);
    assert_eq!(model.guild.ranks[2].name, "Veterans");
    for member in model
        .guild
        .members
        .iter()
        .filter(|m| m.rank_index == Some(2))
    {
        assert_eq!(member.rank_name.as_deref(), Some("Veterans"));
    }
    assert_eq!(model.guild.gold, before.gold);
    assert_eq!(model.guild.storage_items, before.storage_items);
    assert_eq!(model.guild.notice, before.notice);
    assert_eq!(
        model.guild.rank_name, before.rank_name,
        "other rank is not my rank"
    );
    assert_eq!(model.guild.my_options, before.my_options);
}

#[test]
fn guild_rank_status7_updates_own_numeric_rank_and_source_options() {
    let mut model = seeded(2);
    let mut packet = terminal("Leader", 2, "Veterans");
    if let ServerPacket::GuildMemberChange { ranks, .. } = &mut packet {
        ranks[0].options = 65;
    }
    assert!(apply_typed(&mut model, packet));
    assert_eq!(model.guild.rank_name.as_deref(), Some("Veterans"));
    assert_eq!(model.guild.my_options, 65);
    assert_eq!(
        model.guild.permissions,
        vec!["CanChangeRank", "CanChangeNotice"]
    );
    assert_eq!(model.guild.ranks.len(), 3);
}

#[test]
fn guild_rank_source_reply_has_status7_nested_index_and_no_command_echo() {
    let value = serde_json::to_value(terminal("Leader", 2, "Veterans")).unwrap();
    let payload = &value["GuildMemberChange"];
    assert_eq!(payload["status"], 7);
    assert_eq!(payload["rankIndex"], 0);
    assert_eq!(payload["ranks"][0]["index"], 2);
    assert!(payload.get("changeType").is_none());
}

#[test]
fn guild_rank_raw_legal_spaces_and_twenty_utf16_survive_typed_status_and_delta() {
    for name in [" A ".to_owned(), "   ".to_owned(), "😀".repeat(10)] {
        assert!((3..=20).contains(&name.encode_utf16().count()));
        let mut model = seeded(2);
        assert!(apply_typed(&mut model, terminal("Leader", 2, &name)));
        assert_eq!(model.guild.ranks.len(), 3);
        assert_eq!(model.guild.ranks[2].name, name);
        assert!(apply_typed(&mut model, status("Rank Guild", &name, 2)));
        assert_eq!(model.guild.name.as_deref(), Some("Rank Guild"));
        assert_eq!(model.guild.rank_name.as_deref(), Some(name.as_str()));
    }
}

#[test]
fn guild_rank_status7_rejects_multi_rank_and_unknown_delta_without_model_mutation() {
    let model = seeded(0);
    let packets = [
        ServerPacket::GuildMemberChange {
            name: "Leader".into(),
            rank_index: 0,
            status: 7,
            ranks: full_ranks("Veterans")[..2].to_vec(),
        },
        ServerPacket::GuildMemberChange {
            name: "Leader".into(),
            rank_index: 0,
            status: 7,
            ranks: vec![rank(99, "Unknown", 1, &[])],
        },
    ];
    for packet in packets {
        let mut observed = model.clone();
        assert!(!apply_typed(&mut observed, packet));
        assert_eq!(
            observed, model,
            "rejected delta must not mutate or invent ACK"
        );
    }
}

#[test]
fn guild_rank_status255_remains_an_explicit_full_list_replacement() {
    let mut model = seeded(0);
    assert!(apply_typed(
        &mut model,
        ServerPacket::GuildMemberChange {
            name: String::new(),
            rank_index: 0,
            status: 255,
            ranks: vec![rank(0, "Leader", 255, &[("Leader", 1)])],
        }
    ));
    assert_eq!(model.guild.ranks.len(), 1);
    assert_eq!(model.guild.members.len(), 1);
    assert_eq!(model.guild.gold, 99123);
}

#[test]
fn guild_rank_status7_requires_actual_nested_index_options_and_member_list() {
    let model = seeded(0);
    for missing in ["index", "options", "members"] {
        let value = serde_json::to_value(terminal("Leader", 2, "Veterans")).unwrap();
        let mut payload = value["GuildMemberChange"].clone();
        payload["ranks"][0].as_object_mut().unwrap().remove(missing);
        let mut observed = model.clone();
        assert!(
            !observed.apply_network_packet("GuildMemberChange", &payload),
            "missing source field {missing} must not overwrite an unrelated rank"
        );
        assert_eq!(observed, model);
    }
}
