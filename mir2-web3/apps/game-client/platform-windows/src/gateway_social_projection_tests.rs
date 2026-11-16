//! Actual Native protocol classification and the same social ingress used by gateway.rs.
use super::*;
use mir2_client_bevy::social::SocialViewer;

fn incoming(packet: &str, payload: Value) -> InboundEvent {
    parse_inbound_event(&json!({"type":"packet","packet":packet,"payload":payload}).to_string())
        .unwrap()
}

#[test]
fn native_guild_gateway_typed_user_information_binds_real_character_not_object_or_selection() {
    let mut social = SocialModel::default();
    let event = incoming(
        "UserInformation",
        json!({"name":"Alice","realId":41,"objectId":9001}),
    );
    assert!(matches!(
        event,
        InboundEvent::Packet(PacketEvent::UserInformation(_))
    ));
    assert!(apply_social_event(&mut social, &event));
    assert_eq!(
        social.viewer,
        Some(SocialViewer {
            name: "Alice".into(),
            character_index: 41
        })
    );
    social.clear_scene();
    assert_eq!(social.viewer.as_ref().unwrap().character_index, 41);
    social.clear_session();
    assert!(social.viewer.is_none());
}

#[test]
fn native_guild_gateway_other_players_and_invalid_real_identity_cannot_replace_self() {
    let mut social = SocialModel::default();
    assert!(apply_social_event(
        &mut social,
        &incoming(
            "UserInformation",
            json!({"name":"Alice","realId":41,"objectId":9001})
        )
    ));
    let original = social.clone();
    for payload in [
        json!({"name":"Other","objectId":123}),
        json!({"name":"Other","realId":2147483648_u64}),
        json!({"name":"","realId":42}),
    ] {
        assert!(!apply_social_event(
            &mut social,
            &incoming("UserInformation", payload)
        ));
        assert_eq!(social, original);
    }
    assert!(!apply_social_event(
        &mut social,
        &incoming(
            "ObjectPlayer",
            json!({"name":"Other","realId":42,"objectId":123})
        )
    ));
    assert_eq!(social, original);
}

#[test]
fn native_guild_gateway_typed_identity_allows_only_real_member_id_in_status8() {
    let mut social = SocialModel::default();
    assert!(apply_social_event(
        &mut social,
        &incoming(
            "UserInformation",
            json!({"name":"Alice","realId":41,"objectId":9001})
        )
    ));
    assert!(apply_social_event(
        &mut social,
        &incoming(
            "GuildStatus",
            json!({"guildName":"Guild","guildRankName":"Member","myRankId":2,"myOptions":0})
        )
    ));
    let rank = |id| json!({"name":"Officer","index":1,"options":65,"members":[{"name":"Alice","id":id,"online":true,"lastLoginBinaryDatetime":0}]});
    let packet = |id| {
        incoming(
            "GuildMemberChange",
            json!({"name":"Leader","rankIndex":0,"status":8,"ranks":[rank(id)]}),
        )
    };
    let before = social.clone();
    assert!(!apply_social_event(&mut social, &packet(9001)));
    assert_eq!(social, before, "AOI object ID does not authorize own rank");
    assert!(apply_social_event(&mut social, &packet(41)));
    assert_eq!(social.guild.my_rank_id, 1);
    assert_eq!(social.guild.my_options, 65);
    assert!(social.guild.roster_needs_refresh);
}
