use std::collections::BTreeSet;

use mir2_protocol::{ClientPacket, ServerPacket};
use mir2_simulation::{SimulationConfig, SimulationSession};

fn login_demo(session: &mut SimulationSession) {
    let packets = session.handle_packet(ClientPacket::Login {
        account_id: "demo".to_string(),
        password: "demo".to_string(),
    });
    assert!(packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
}

#[test]
fn native_startup_sends_item_info_before_every_equipped_user_item() {
    let mut session = SimulationSession::new(SimulationConfig::default());
    login_demo(&mut session);
    let packets = session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    let user_information_index = packets
        .iter()
        .position(|packet| matches!(packet, ServerPacket::UserInformation { .. }))
        .expect("start game must send UserInformation");
    let item_info_indices = packets[..user_information_index]
        .iter()
        .filter_map(|packet| match packet {
            ServerPacket::NewItemInfo { info } => Some(info.index),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    let equipped_indices = packets[user_information_index..]
        .iter()
        .find_map(|packet| match packet {
            ServerPacket::UserInformation { info } => info.equipment.as_ref(),
            _ => None,
        })
        .expect("UserInformation must include the equipment section")
        .iter()
        .flatten()
        .map(|item| item.item_index)
        .collect::<BTreeSet<_>>();

    assert!(!equipped_indices.is_empty());
    assert!(equipped_indices.is_subset(&item_info_indices));
}
