use super::*;
use crate::{SimulationConfig, VisibleNpcRecord};
use mir2_protocol::{ClientPacket, MirDirection};

const POSTMASTER: u32 = 4991;

fn post_office() -> SimulationSession {
    let mut config = SimulationConfig::default();
    config.visible_npcs.push(VisibleNpcRecord {
        object_id: POSTMASTER,
        name: "Warehouse".into(),
        image: 5,
        colour_argb: -1,
        position: Point { x: 331, y: 271 },
        direction: MirDirection::Left,
        quest_ids: vec![],
        script_key: Some("BichonProvince/BichonWall/Warehouse-0125".into()),
    });
    SimulationSession::new(config)
}

#[test]
fn parcel_request_requires_an_offered_in_range_authenticated_npc_link() {
    let mut session = post_office();
    assert!(session.select_npc_dialog_target("@SendParcel").is_empty());
    session.handle_packet(ClientPacket::Login {
        account_id: "demo".into(), password: "demo".into(),
    });
    session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    assert!(session.select_npc_dialog_target("@SendParcel").is_empty());
    session.interact(POSTMASTER);
    assert!(session.world_snapshot().active_npc_dialog.as_ref().is_some_and(|dialog| {
        dialog.links.iter().any(|link| link.target.eq_ignore_ascii_case("@SendParcel"))
    }));
    assert_eq!(session.select_npc_dialog_target("@SendParcel"), vec![ServerPacket::MailSendRequest]);

    // A different visible link does not authorize the built-in service.
    session.app.world_mut().resource_mut::<NpcStateResource>()
        .active_npc_dialog.as_mut().unwrap().links
        .retain(|link| !link.target.eq_ignore_ascii_case("@SendParcel"));
    assert!(session.select_npc_dialog_target("@SendParcel").is_empty());

    session.interact(POSTMASTER);
    let player = player_entity(session.app.world()).unwrap();
    session.app.world_mut().entity_mut(player).insert(Position(Point { x: 300, y: 300 }));
    let packets = session.select_npc_dialog_target("@SendParcel");
    assert!(!packets.contains(&ServerPacket::MailSendRequest), "out-of-range packets: {packets:?}");
    assert!(session.world_snapshot().active_npc_dialog.is_none());
}
