use mir2_protocol::frame::{decode_frame, encode_frame};
use mir2_protocol::ids::{ClientPacketId, ServerPacketId};
use mir2_protocol::io::{PacketReader, PacketWriter};
use mir2_protocol::packets::{
    decode_client_packet, decode_server_packet, encode_client_packet, encode_server_packet,
    ClientPacket, ServerPacket,
};
use mir2_protocol::trace::{
    trace_client_packets, trace_server_packets, PacketTraceDirection, PacketTraceEntry,
};
use mir2_protocol::types::{
    ChatType, ClientQuestInfo, ClientRecipeInfo, GroupMember, ItemInfo, MapInformation, MirClass,
    MirDirection, MirGender, MirGridType, MonsterInfo, NpcInfo, ObjectEffectInfo, ObjectMovement,
    ObjectPlayerInfo, ObjectSpellInfo, Point, QuestItemReward, Spell, UserInformation, UserItem,
    UserItemStat,
};

fn sample_user_item(unique_id: u64, count: u16) -> UserItem {
    UserItem {
        unique_id,
        item_index: 23,
        current_dura: 0,
        max_dura: 0,
        count,
        soul_bound_id: -1,
        identified: false,
        cursed: false,
        slots: Vec::new(),
        gem_count: 0,
        added_stats: vec![UserItemStat { stat: 5, value: 1 }],
        awake_type: 0,
        awake_values: Vec::new(),
        refined_value: 0,
        refine_added: 0,
        refine_success_chance: 0,
        wedding_ring: -1,
        expire_info: None,
        rental_information: None,
        is_shop_item: false,
        sealed_info: None,
        gm_made: false,
    }
}

fn sample_item_info() -> ItemInfo {
    ItemInfo {
        index: 658,
        name: "(HP)DrugSmall".to_string(),
        item_type: 13,
        grade: 0,
        required_type: 0,
        required_class: 0,
        required_gender: 0,
        item_set: 0,
        shape: 0,
        weight: 1,
        light: 0,
        required_amount: 0,
        image: 398,
        durability: 0,
        stack_size: 20,
        price: 40,
        start_item: false,
        effect: 0,
        need_identify: false,
        show_group_pickup: false,
        class_based: false,
        level_based: false,
        can_mine: false,
        global_drop_notify: false,
        bind: 0,
        unique: 0,
        random_stats_id: 0,
        can_fast_run: false,
        can_awakening: false,
        slots: 0,
        stats: vec![UserItemStat {
            stat: 12,
            value: 30,
        }],
        tooltip: Some(String::new()),
    }
}

#[test]
fn frame_roundtrip_preserves_packet_id_and_payload() {
    let bytes = encode_frame(17, &[0xAA, 0xBB, 0xCC]).expect("frame should encode");
    let frame = mir2_protocol::frame::decode_frame(&bytes).expect("frame should decode");

    assert_eq!(frame.packet_id, 17);
    assert_eq!(frame.payload, vec![0xAA, 0xBB, 0xCC]);
}

#[test]
fn packet_trace_entries_capture_stable_packet_ids_and_names() {
    let client_trace = trace_client_packets(&[
        ClientPacket::StartGame { character_index: 0 },
        ClientPacket::Attack {
            direction: MirDirection::Right,
            spell: Spell::None,
        },
    ]);
    assert_eq!(
        client_trace,
        vec![
            PacketTraceEntry {
                sequence: 0,
                direction: PacketTraceDirection::Client,
                packet_id: ClientPacketId::StartGame as i16,
                packet: "StartGame".to_string(),
            },
            PacketTraceEntry {
                sequence: 1,
                direction: PacketTraceDirection::Client,
                packet_id: ClientPacketId::Attack as i16,
                packet: "Attack".to_string(),
            },
        ]
    );

    let server_trace = trace_server_packets(&[
        ServerPacket::StartGame {
            result: 4,
            resolution: 0,
        },
        ServerPacket::MapInformation {
            info: MapInformation {
                map_index: 0,
                file_name: "0".to_string(),
                title: "BichonProvince".to_string(),
                mini_map: 101,
                big_map: 101,
                lights: 0,
                flags: 0,
                map_dark_light: 0,
                music: 0,
                weather_particles: 0,
            },
        },
    ]);
    assert_eq!(server_trace[0].sequence, 0);
    assert_eq!(server_trace[0].direction, PacketTraceDirection::Server);
    assert_eq!(server_trace[0].packet_id, ServerPacketId::StartGame as i16);
    assert_eq!(server_trace[0].packet, "StartGame");
    assert_eq!(server_trace[1].sequence, 1);
    assert_eq!(
        server_trace[1].packet_id,
        ServerPacketId::MapInformation as i16
    );
    assert_eq!(server_trace[1].packet, "MapInformation");
}

#[test]
fn dotnet_string_roundtrip_uses_7bit_prefix() {
    let mut writer = PacketWriter::new();
    writer
        .write_string("hello-world")
        .expect("string should encode");

    let bytes = writer.into_inner();
    assert_eq!(bytes[0], 11);

    let mut reader = PacketReader::new(&bytes);
    let decoded = reader.read_string().expect("string should decode");

    assert_eq!(decoded, "hello-world");
    assert_eq!(reader.remaining(), 0);
}

#[test]
fn decode_login_success_packet() {
    let mut payload = PacketWriter::new();
    payload.write_i32(2);

    write_select_info(
        &mut payload,
        7,
        "HeroOne",
        38,
        MirClass::Warrior,
        MirGender::Male,
        638452512000000000,
    );
    write_select_info(
        &mut payload,
        9,
        "HeroTwo",
        22,
        MirClass::Wizard,
        MirGender::Female,
        638452598000000000,
    );

    let frame = encode_frame(ServerPacketId::LoginSuccess as i16, &payload.into_inner())
        .expect("frame should encode");
    let packet = decode_server_packet(&frame).expect("packet should decode");

    match packet {
        ServerPacket::LoginSuccess { characters } => {
            assert_eq!(characters.len(), 2);
            assert_eq!(characters[0].name, "HeroOne");
            assert_eq!(characters[0].level, 38);
            assert_eq!(characters[1].class, MirClass::Wizard);
        }
        other => panic!("unexpected packet: {other:?}"),
    }
}

#[test]
fn decode_map_information_packet() {
    let mut payload = PacketWriter::new();
    payload.write_i32(3);
    payload.write_string("0").expect("map file");
    payload.write_string("Bichon Province").expect("map title");
    payload.write_u16(12);
    payload.write_u16(45);
    payload.write_u8(1);
    payload.write_u8(0x03);
    payload.write_u8(2);
    payload.write_u16(9);
    payload.write_u16(6);

    let frame = encode_frame(ServerPacketId::MapInformation as i16, &payload.into_inner())
        .expect("frame should encode");
    let packet = decode_server_packet(&frame).expect("packet should decode");

    match packet {
        ServerPacket::MapInformation { info } => {
            assert_eq!(
                info,
                MapInformation {
                    map_index: 3,
                    file_name: "0".to_string(),
                    title: "Bichon Province".to_string(),
                    mini_map: 12,
                    big_map: 45,
                    lights: 1,
                    flags: 0x03,
                    map_dark_light: 2,
                    music: 9,
                    weather_particles: 6,
                }
            );
            assert!(info.has_lightning());
            assert!(info.has_fire());
        }
        other => panic!("unexpected packet: {other:?}"),
    }
}

#[test]
fn server_packet_encoder_roundtrip_for_chat() {
    for chat_type in [ChatType::Announcement, ChatType::Trainer] {
        let packet = ServerPacket::Chat {
            message: "Welcome to the shard".to_string(),
            chat_type,
        };

        let bytes = encode_server_packet(&packet).expect("packet should encode");
        let decoded = decode_server_packet(&bytes).expect("packet should decode");

        assert_eq!(decoded, packet);
    }
}

#[test]
fn group_member_info_packet_roundtrips_optional_fields() {
    let packet = ServerPacket::GroupMemberInfo {
        members: vec![
            GroupMember {
                name: "Leader".to_string(),
                level: Some(42),
                class: Some(MirClass::Warrior as u8),
                hp: Some(350),
                max_hp: Some(400),
                online: Some(true),
            },
            GroupMember {
                name: "Offline".to_string(),
                level: None,
                class: None,
                hp: None,
                max_hp: None,
                online: Some(false),
            },
        ],
        leader_name: "Leader".to_string(),
    };

    let bytes = encode_server_packet(&packet).expect("packet should encode");
    let decoded = decode_server_packet(&bytes).expect("packet should decode");

    assert_eq!(decoded, packet);
}

#[test]
fn item_info_packets_use_crystal_payloads() {
    let request = ClientPacket::RequestItemInfo { item_index: 658 };
    let request_bytes = encode_client_packet(&request).expect("request should encode");
    let request_frame = decode_frame(&request_bytes).expect("request frame should decode");
    let decoded_request = decode_client_packet(&request_bytes).expect("request should roundtrip");

    assert_eq!(
        request_frame.packet_id,
        ClientPacketId::RequestItemInfo as i16
    );
    assert_eq!(decoded_request, request);

    let response = ServerPacket::NewItemInfo {
        info: sample_item_info(),
    };
    let response_bytes = encode_server_packet(&response).expect("item info should encode");
    let response_frame = decode_frame(&response_bytes).expect("item info frame should decode");
    let decoded_response =
        decode_server_packet(&response_bytes).expect("item info should roundtrip");

    assert_eq!(response_frame.packet_id, ServerPacketId::NewItemInfo as i16);
    assert_eq!(decoded_response, response);
}

#[test]
fn quest_and_recipe_info_packets_use_typed_crystal_payloads() {
    for (packet, packet_id) in [
        (
            ServerPacket::NewQuestInfo {
                info: ClientQuestInfo {
                    index: 1,
                    npc_index: 10,
                    name: "Quest".to_string(),
                    group: "Starter".to_string(),
                    description: vec!["Desc".to_string()],
                    task_description: vec!["Task".to_string()],
                    return_description: Vec::new(),
                    completion_description: vec!["Done".to_string()],
                    min_level_needed: 1,
                    max_level_needed: 0,
                    quest_needed: 0,
                    class_needed: 31,
                    quest_type: 0,
                    time_limit_in_seconds: 0,
                    reward_gold: 10,
                    reward_exp: 20,
                    reward_credit: 0,
                    rewards_fixed_item: vec![QuestItemReward {
                        item: sample_item_info(),
                        count: 1,
                    }],
                    rewards_select_item: Vec::new(),
                    finish_npc_index: 10,
                },
            },
            ServerPacketId::NewQuestInfo,
        ),
        (
            ServerPacket::NewRecipeInfo {
                info: ClientRecipeInfo {
                    gold: 50,
                    chance: 90,
                    item: sample_user_item(1, 1),
                    tools: vec![sample_user_item(2, 1)],
                    ingredients: vec![sample_user_item(3, 2)],
                },
            },
            ServerPacketId::NewRecipeInfo,
        ),
    ] {
        let bytes = encode_server_packet(&packet).expect("typed packet should encode");
        let frame = decode_frame(&bytes).expect("typed packet frame should decode");
        let decoded = decode_server_packet(&bytes).expect("typed packet should roundtrip");

        assert_eq!(frame.packet_id, packet_id as i16);
        assert_eq!(decoded, packet);
    }
}

#[test]
fn raw_server_packets_preserve_payload_when_encoding_frames() {
    let packet = ServerPacket::Raw {
        packet_id: ServerPacketId::Connected,
        payload: vec![7, 8, 9],
    };
    let bytes = encode_server_packet(&packet).expect("raw packet should encode");
    let frame = decode_frame(&bytes).expect("raw packet frame should decode");

    assert_eq!(frame.packet_id, ServerPacketId::Connected as i16);
    assert_eq!(frame.payload, vec![7, 8, 9]);
}

#[test]
fn mir_grid_type_values_match_crystal() {
    assert_eq!(MirGridType::Inventory as u8, 1);
    assert_eq!(MirGridType::Equipment as u8, 2);
    assert_eq!(MirGridType::Trade as u8, 3);
    assert_eq!(MirGridType::Storage as u8, 4);
    assert_eq!(MirGridType::QuestInventory as u8, 13);
    assert_eq!(MirGridType::Refine as u8, 16);
    assert_eq!(MirGridType::HeroEquipment as u8, 21);
    assert_eq!(MirGridType::HeroInventory as u8, 22);

    assert_eq!(MirGridType::try_from(4), Ok(MirGridType::Storage));
    assert_eq!(MirGridType::try_from(22), Ok(MirGridType::HeroInventory));
}

#[test]
fn item_action_ack_server_packets_use_crystal_ids() {
    let cases = [
        (
            ServerPacket::MoveItem {
                grid: MirGridType::Storage,
                from: 1,
                to: 2,
                success: true,
            },
            ServerPacketId::MoveItem,
        ),
        (
            ServerPacket::EquipItem {
                grid: MirGridType::Inventory,
                unique_id: 44,
                to: 0,
                success: true,
            },
            ServerPacketId::EquipItem,
        ),
        (
            ServerPacket::MergeItem {
                grid_from: MirGridType::Inventory,
                grid_to: MirGridType::Inventory,
                id_from: 3,
                id_to: 4,
                success: true,
            },
            ServerPacketId::MergeItem,
        ),
        (
            ServerPacket::RemoveItem {
                grid: MirGridType::Equipment,
                unique_id: 0,
                to: 5,
                success: true,
            },
            ServerPacketId::RemoveItem,
        ),
        (
            ServerPacket::RemoveSlotItem {
                grid: MirGridType::Equipment,
                grid_to: MirGridType::Inventory,
                unique_id: 0,
                to: 5,
                success: false,
            },
            ServerPacketId::RemoveSlotItem,
        ),
        (
            ServerPacket::TakeBackItem {
                from: 1,
                to: 6,
                success: true,
            },
            ServerPacketId::TakeBackItem,
        ),
        (
            ServerPacket::StoreItem {
                from: 6,
                to: 1,
                success: true,
            },
            ServerPacketId::StoreItem,
        ),
        (
            ServerPacket::CombineItem {
                grid: MirGridType::Inventory,
                id_from: 7,
                id_to: 8,
                success: true,
                destroy: false,
            },
            ServerPacketId::CombineItem,
        ),
        (
            ServerPacket::SplitItem1 {
                grid: MirGridType::Inventory,
                unique_id: 6,
                count: 2,
                success: true,
            },
            ServerPacketId::SplitItem1,
        ),
        (
            ServerPacket::UseItem {
                unique_id: 6,
                success: true,
                grid: MirGridType::Inventory,
            },
            ServerPacketId::UseItem,
        ),
        (
            ServerPacket::DropItem {
                unique_id: 6,
                count: 1,
                hero_inventory: false,
                success: true,
            },
            ServerPacketId::DropItem,
        ),
        (
            ServerPacket::DeleteItem {
                unique_id: 6,
                count: 1,
            },
            ServerPacketId::DeleteItem,
        ),
    ];

    for (packet, expected_id) in cases {
        let bytes = encode_server_packet(&packet).expect("ack packet should encode");
        let frame = decode_frame(&bytes).expect("ack frame should decode");
        let decoded = decode_server_packet(&bytes).expect("ack packet should roundtrip");

        assert_eq!(frame.packet_id, expected_id as i16);
        assert_eq!(decoded, packet);
    }
}

#[test]
fn split_item_server_packet_uses_crystal_user_item_payload() {
    let packet = ServerPacket::SplitItem {
        item: Some(sample_user_item(9, 2)),
        grid: MirGridType::Inventory,
    };
    let bytes = encode_server_packet(&packet).expect("split item packet should encode");
    let frame = decode_frame(&bytes).expect("split item frame should decode");
    let decoded = decode_server_packet(&bytes).expect("split item packet should roundtrip");

    assert_eq!(frame.packet_id, ServerPacketId::SplitItem as i16);
    assert_eq!(decoded, packet);

    let empty_packet = ServerPacket::SplitItem {
        item: None,
        grid: MirGridType::Storage,
    };
    let empty_bytes =
        encode_server_packet(&empty_packet).expect("empty split item packet should encode");
    let empty_decoded =
        decode_server_packet(&empty_bytes).expect("empty split item packet should roundtrip");

    assert_eq!(empty_decoded, empty_packet);
}

#[test]
fn gained_item_server_packet_uses_crystal_user_item_payload() {
    let packet = ServerPacket::GainedItem {
        item: sample_user_item(12, 5),
    };
    let bytes = encode_server_packet(&packet).expect("gained item packet should encode");
    let frame = decode_frame(&bytes).expect("gained item frame should decode");
    let decoded = decode_server_packet(&bytes).expect("gained item packet should roundtrip");

    assert_eq!(frame.packet_id, ServerPacketId::GainedItem as i16);
    assert_eq!(decoded, packet);
}

#[test]
fn refresh_item_server_packet_uses_crystal_user_item_payload() {
    let packet = ServerPacket::RefreshItem {
        item: sample_user_item(13, 1),
    };
    let bytes = encode_server_packet(&packet).expect("refresh item packet should encode");
    let frame = decode_frame(&bytes).expect("refresh item frame should decode");
    let decoded = decode_server_packet(&bytes).expect("refresh item packet should roundtrip");

    assert_eq!(frame.packet_id, ServerPacketId::RefreshItem as i16);
    assert_eq!(decoded, packet);
}

#[test]
fn currency_delta_server_packets_use_crystal_ids() {
    let cases = [
        (
            ServerPacket::GainedGold { gold: 1234 },
            ServerPacketId::GainedGold,
        ),
        (
            ServerPacket::LoseGold { gold: 5678 },
            ServerPacketId::LoseGold,
        ),
        (
            ServerPacket::GainedCredit { credit: 12 },
            ServerPacketId::GainedCredit,
        ),
        (
            ServerPacket::LoseCredit { credit: 7 },
            ServerPacketId::LoseCredit,
        ),
    ];

    for (packet, expected_id) in cases {
        let bytes = encode_server_packet(&packet).expect("currency packet should encode");
        let frame = decode_frame(&bytes).expect("currency frame should decode");
        let decoded = decode_server_packet(&bytes).expect("currency packet should roundtrip");

        assert_eq!(frame.packet_id, expected_id as i16);
        assert_eq!(decoded, packet);
    }
}

#[test]
fn sell_and_repair_server_packets_use_crystal_ids() {
    let cases = [
        (
            ServerPacket::SellItem {
                unique_id: 4,
                count: 2,
                success: true,
            },
            ServerPacketId::SellItem,
        ),
        (
            ServerPacket::RepairItem { unique_id: 4 },
            ServerPacketId::RepairItem,
        ),
    ];

    for (packet, expected_id) in cases {
        let bytes = encode_server_packet(&packet).expect("packet should encode");
        let frame = decode_frame(&bytes).expect("frame should decode");
        let decoded = decode_server_packet(&bytes).expect("packet should roundtrip");

        assert_eq!(frame.packet_id, expected_id as i16);
        assert_eq!(decoded, packet);
    }
}

#[test]
fn item_slot_seal_and_upgrade_server_packets_use_crystal_ids() {
    let cases = [
        (
            ServerPacket::ItemSlotSizeChanged {
                unique_id: 42,
                slot_size: 3,
            },
            ServerPacketId::ItemSlotSizeChanged,
        ),
        (
            ServerPacket::ItemSealChanged {
                unique_id: 43,
                expiry_date_binary_datetime: 638000000000000000,
            },
            ServerPacketId::ItemSealChanged,
        ),
        (
            ServerPacket::ItemUpgraded {
                item: sample_user_item(44, 1),
            },
            ServerPacketId::ItemUpgraded,
        ),
    ];

    for (packet, expected_id) in cases {
        let bytes = encode_server_packet(&packet).expect("packet should encode");
        let frame = decode_frame(&bytes).expect("frame should decode");
        let decoded = decode_server_packet(&bytes).expect("packet should roundtrip");

        assert_eq!(frame.packet_id, expected_id as i16);
        assert_eq!(decoded, packet);
    }
}

#[test]
fn npc_service_server_packets_use_crystal_ids() {
    let cases = [
        (ServerPacket::TeleportIn, ServerPacketId::TeleportIn),
        (
            ServerPacket::NPCGoods {
                list: vec![sample_user_item(61, 2)],
                rate: 1.25,
                panel_type: 3,
                hide_added_stats: true,
            },
            ServerPacketId::NPCGoods,
        ),
        (ServerPacket::NPCSell, ServerPacketId::NPCSell),
        (
            ServerPacket::NPCRepair { rate: 1.5 },
            ServerPacketId::NPCRepair,
        ),
        (
            ServerPacket::NPCSRepair { rate: 2.5 },
            ServerPacketId::NPCSRepair,
        ),
        (
            ServerPacket::NPCRefine {
                rate: 3.5,
                refining: true,
            },
            ServerPacketId::NPCRefine,
        ),
        (ServerPacket::NPCCheckRefine, ServerPacketId::NPCCheckRefine),
        (
            ServerPacket::NPCCollectRefine { success: true },
            ServerPacketId::NPCCollectRefine,
        ),
        (
            ServerPacket::NPCReplaceWedRing { rate: 4.5 },
            ServerPacketId::NPCReplaceWedRing,
        ),
        (ServerPacket::NPCStorage, ServerPacketId::NPCStorage),
        (
            ServerPacket::UserStorage {
                storage: Some(vec![Some(sample_user_item(62, 3)), None]),
            },
            ServerPacketId::UserStorage,
        ),
        (
            ServerPacket::CraftItem { success: false },
            ServerPacketId::CraftItem,
        ),
    ];

    for (packet, expected_id) in cases {
        let bytes = encode_server_packet(&packet).expect("packet should encode");
        let frame = decode_frame(&bytes).expect("frame should decode");
        let decoded = decode_server_packet(&bytes).expect("packet should roundtrip");

        assert_eq!(frame.packet_id, expected_id as i16);
        assert_eq!(decoded, packet);
    }
}

#[test]
fn delete_item_client_packet_uses_crystal_payload() {
    let packet = ClientPacket::DeleteItem {
        unique_id: 9,
        count: 3,
        hero_inventory: false,
    };
    let bytes = encode_client_packet(&packet).expect("delete item packet should encode");
    let frame = decode_frame(&bytes).expect("delete item frame should decode");
    let decoded = decode_client_packet(&bytes).expect("delete item packet should roundtrip");

    assert_eq!(frame.packet_id, ClientPacketId::DeleteItem as i16);
    assert_eq!(decoded, packet);
}

#[test]
fn durability_repair_server_packets_use_crystal_ids() {
    let dura_packet = ServerPacket::DuraChanged {
        unique_id: 12,
        current_dura: 45,
    };
    let dura_bytes = encode_server_packet(&dura_packet).expect("dura packet should encode");
    let dura_frame = decode_frame(&dura_bytes).expect("dura frame should decode");
    let decoded_dura = decode_server_packet(&dura_bytes).expect("dura packet should roundtrip");

    assert_eq!(dura_frame.packet_id, ServerPacketId::DuraChanged as i16);
    assert_eq!(decoded_dura, dura_packet);

    let repaired_packet = ServerPacket::ItemRepaired {
        unique_id: 12,
        max_dura: 60,
        current_dura: 60,
    };
    let repaired_bytes =
        encode_server_packet(&repaired_packet).expect("repair packet should encode");
    let repaired_frame = decode_frame(&repaired_bytes).expect("repair frame should decode");
    let decoded_repaired =
        decode_server_packet(&repaired_bytes).expect("repair packet should roundtrip");

    assert_eq!(
        repaired_frame.packet_id,
        ServerPacketId::ItemRepaired as i16
    );
    assert_eq!(decoded_repaired, repaired_packet);
}

#[test]
fn delete_character_uses_crystal_packet_ids() {
    let client_packet = ClientPacket::DeleteCharacter { character_index: 2 };
    let client_bytes = encode_client_packet(&client_packet).expect("client packet should encode");
    let client_frame = decode_frame(&client_bytes).expect("client frame should decode");
    let decoded_client = decode_client_packet(&client_bytes).expect("client packet should decode");

    assert_eq!(
        client_frame.packet_id,
        ClientPacketId::DeleteCharacter as i16
    );
    assert_eq!(decoded_client, client_packet);

    let failed_packet = ServerPacket::DeleteCharacter { result: 1 };
    let failed_bytes = encode_server_packet(&failed_packet).expect("failed packet should encode");
    let failed_frame = decode_frame(&failed_bytes).expect("failed frame should decode");
    let decoded_failed = decode_server_packet(&failed_bytes).expect("failed packet should decode");

    assert_eq!(
        failed_frame.packet_id,
        ServerPacketId::DeleteCharacter as i16
    );
    assert_eq!(decoded_failed, failed_packet);

    let success_packet = ServerPacket::DeleteCharacterSuccess { character_index: 2 };
    let success_bytes =
        encode_server_packet(&success_packet).expect("success packet should encode");
    let success_frame = decode_frame(&success_bytes).expect("success frame should decode");
    let decoded_success =
        decode_server_packet(&success_bytes).expect("success packet should decode");

    assert_eq!(
        success_frame.packet_id,
        ServerPacketId::DeleteCharacterSuccess as i16
    );
    assert_eq!(decoded_success, success_packet);
}

#[test]
fn change_password_uses_crystal_packet_ids() {
    let client_packet = ClientPacket::ChangePassword {
        account_id: "demo".to_string(),
        current_password: "old".to_string(),
        new_password: "new".to_string(),
    };
    let client_bytes = encode_client_packet(&client_packet).expect("client packet should encode");
    let client_frame = decode_frame(&client_bytes).expect("client frame should decode");
    let decoded_client = decode_client_packet(&client_bytes).expect("client packet should decode");

    assert_eq!(
        client_frame.packet_id,
        ClientPacketId::ChangePassword as i16
    );
    assert_eq!(decoded_client, client_packet);

    let server_packet = ServerPacket::ChangePassword { result: 6 };
    let server_bytes = encode_server_packet(&server_packet).expect("server packet should encode");
    let server_frame = decode_frame(&server_bytes).expect("server frame should decode");
    let decoded_server = decode_server_packet(&server_bytes).expect("server packet should decode");

    assert_eq!(
        server_frame.packet_id,
        ServerPacketId::ChangePassword as i16
    );
    assert_eq!(decoded_server, server_packet);
}

#[test]
fn item_and_combat_client_packets_use_crystal_payloads() {
    let cases = [
        (
            ClientPacket::UseItem {
                unique_id: 3,
                grid: MirGridType::Inventory,
            },
            ClientPacketId::UseItem as i16,
        ),
        (
            ClientPacket::DropItem {
                unique_id: 4,
                count: 2,
                hero_inventory: false,
            },
            ClientPacketId::DropItem as i16,
        ),
        (
            ClientPacket::DeleteItem {
                unique_id: 4,
                count: 2,
                hero_inventory: false,
            },
            ClientPacketId::DeleteItem as i16,
        ),
        (ClientPacket::PickUp, ClientPacketId::PickUp as i16),
        (
            ClientPacket::RequestItemInfo { item_index: 658 },
            ClientPacketId::RequestItemInfo as i16,
        ),
        (
            ClientPacket::Attack {
                direction: MirDirection::Right,
                spell: Spell::None,
            },
            ClientPacketId::Attack as i16,
        ),
        (
            ClientPacket::RangeAttack {
                direction: MirDirection::UpRight,
                location: Point { x: 330, y: 271 },
                target_id: 3001,
                target_location: Point { x: 331, y: 270 },
            },
            ClientPacketId::RangeAttack as i16,
        ),
        (
            ClientPacket::Harvest {
                direction: MirDirection::Right,
            },
            ClientPacketId::Harvest as i16,
        ),
        (
            ClientPacket::BuyItem {
                item_index: 43_122_688,
                count: 5,
                panel_type: 0,
            },
            ClientPacketId::BuyItem as i16,
        ),
        (
            ClientPacket::CombineItem {
                grid: MirGridType::Inventory,
                id_from: 31,
                id_to: 32,
            },
            ClientPacketId::CombineItem as i16,
        ),
    ];

    for (packet, packet_id) in cases {
        let bytes = encode_client_packet(&packet).expect("client packet should encode");
        let frame = decode_frame(&bytes).expect("client frame should decode");
        let decoded = decode_client_packet(&bytes).expect("client packet should decode");

        assert_eq!(frame.packet_id, packet_id);
        assert_eq!(decoded, packet);
    }
}

#[test]
fn harvest_server_packets_use_crystal_payloads() {
    let movement = ObjectMovement {
        object_id: 1000,
        position: Point { x: 331, y: 270 },
        direction: MirDirection::Right,
    };
    let packets = [
        (
            ServerPacket::ObjectHarvest {
                movement: movement.clone(),
            },
            ServerPacketId::ObjectHarvest as i16,
        ),
        (
            ServerPacket::ObjectHarvested { movement },
            ServerPacketId::ObjectHarvested as i16,
        ),
    ];

    for (packet, packet_id) in packets {
        let bytes = encode_server_packet(&packet).expect("server packet should encode");
        let frame = decode_frame(&bytes).expect("server frame should decode");
        let decoded = decode_server_packet(&bytes).expect("server packet should decode");

        assert_eq!(frame.packet_id, packet_id);
        assert_eq!(decoded, packet);
    }
}

#[test]
fn object_back_step_packet_uses_crystal_payload() {
    let packet = ServerPacket::ObjectBackStep {
        movement: ObjectMovement {
            object_id: 1000,
            position: Point { x: 331, y: 270 },
            direction: MirDirection::Left,
        },
        distance: 2,
    };

    let bytes = encode_server_packet(&packet).expect("server packet should encode");
    let frame = decode_frame(&bytes).expect("server frame should decode");
    let decoded = decode_server_packet(&bytes).expect("server packet should decode");

    assert_eq!(frame.packet_id, ServerPacketId::ObjectBackStep as i16);
    assert_eq!(decoded, packet);
}

#[test]
fn object_sit_down_packet_uses_crystal_payload() {
    let packet = ServerPacket::ObjectSitDown {
        movement: ObjectMovement {
            object_id: 1000,
            position: Point { x: 331, y: 270 },
            direction: MirDirection::Down,
        },
        sitting: true,
    };

    let bytes = encode_server_packet(&packet).expect("server packet should encode");
    let frame = decode_frame(&bytes).expect("server frame should decode");
    let decoded = decode_server_packet(&bytes).expect("server packet should decode");

    assert_eq!(frame.packet_id, ServerPacketId::ObjectSitDown as i16);
    assert_eq!(decoded, packet);
}

#[test]
fn object_teleport_packets_use_crystal_payloads() {
    let packets = [
        (
            ServerPacket::ObjectTeleportOut {
                object_id: 1000,
                effect_type: 0,
            },
            ServerPacketId::ObjectTeleportOut as i16,
        ),
        (
            ServerPacket::ObjectTeleportIn {
                object_id: 1000,
                effect_type: 0,
            },
            ServerPacketId::ObjectTeleportIn as i16,
        ),
    ];

    for (packet, packet_id) in packets {
        let bytes = encode_server_packet(&packet).expect("server packet should encode");
        let frame = decode_frame(&bytes).expect("server frame should decode");
        let decoded = decode_server_packet(&bytes).expect("server packet should decode");

        assert_eq!(frame.packet_id, packet_id);
        assert_eq!(decoded, packet);
    }
}

#[test]
fn object_effect_packet_uses_crystal_payload() {
    let packet = ServerPacket::ObjectEffect {
        info: ObjectEffectInfo {
            object_id: 1000,
            effect: 8,
            effect_type: 0,
            delay_time: 250,
            time: 5000,
        },
    };

    let bytes = encode_server_packet(&packet).expect("server packet should encode");
    let frame = decode_frame(&bytes).expect("server frame should decode");
    let decoded = decode_server_packet(&bytes).expect("server packet should decode");

    assert_eq!(frame.packet_id, ServerPacketId::ObjectEffect as i16);
    assert_eq!(decoded, packet);
}

#[test]
fn object_spell_packet_uses_crystal_payload() {
    for (spell, param) in [
        (Spell::DigOutArmadillo, true),
        (Spell::GeneralMeowMeowThunder, false),
        (Spell::TucsonGeneralRock, false),
    ] {
        let packet = ServerPacket::ObjectSpell {
            info: ObjectSpellInfo {
                object_id: 1000,
                location: Point { x: 330, y: 270 },
                spell,
                direction: MirDirection::Up,
                param,
            },
        };

        let bytes = encode_server_packet(&packet).expect("server packet should encode");
        let frame = decode_frame(&bytes).expect("server frame should decode");
        let decoded = decode_server_packet(&bytes).expect("server packet should decode");

        assert_eq!(frame.packet_id, ServerPacketId::ObjectSpell as i16);
        assert_eq!(decoded, packet);
    }
}

#[test]
fn user_information_packet_roundtrip() {
    let packet = ServerPacket::UserInformation {
        info: UserInformation {
            object_id: 1000,
            real_id: 2000,
            name: "Scout".to_string(),
            guild_name: String::new(),
            guild_rank: String::new(),
            name_colour_argb: -1,
            class: MirClass::Warrior,
            gender: MirGender::Male,
            level: 7,
            location: Point { x: 330, y: 270 },
            direction: MirDirection::Down,
            hair: 0,
            hp: 120,
            mp: 45,
            experience: 0,
            max_experience: 100,
            level_effects: 0,
            has_hero: false,
            hero_behaviour: 0,
            inventory_section_present: false,
            inventory: None,
            equipment_section_present: false,
            equipment: None,
            quest_inventory_section_present: false,
            quest_inventory: None,
            gold: 1000,
            credit: 0,
            has_expanded_storage: false,
            has_storage_password: false,
            require_storage_password: false,
            storage_password_last_set_binary_datetime: 0,
            expanded_storage_expiry_time_binary_datetime: 0,
            magic_count: 0,
            intelligent_creature_count: 0,
            summoned_creature_type: 0,
            creature_summoned: false,
            allow_observe: false,
            observer: false,
        },
    };

    let bytes = encode_server_packet(&packet).expect("packet should encode");
    let decoded = decode_server_packet(&bytes).expect("packet should decode");

    assert_eq!(decoded, packet);
}

#[test]
fn object_player_packet_roundtrip() {
    let packet = ServerPacket::ObjectPlayer {
        info: ObjectPlayerInfo {
            object_id: 2001,
            name: "Mentor".to_string(),
            guild_name: String::new(),
            guild_rank_name: String::new(),
            name_colour_argb: -1,
            class: MirClass::Wizard,
            gender: MirGender::Female,
            level: 18,
            location: Point { x: 334, y: 268 },
            direction: MirDirection::Left,
            hair: 0,
            light: 0,
            weapon: 0,
            weapon_effect: 0,
            armour: 0,
            poison: 0,
            dead: false,
            hidden: false,
            effect: 0,
            wing_effect: 0,
            extra: false,
            mount_type: 0,
            riding_mount: false,
            fishing: false,
            transform_type: 0,
            element_orb_effect: 0,
            element_orb_level: 0,
            element_orb_max: 0,
            buffs: vec![],
            level_effects: 0,
        },
    };

    let bytes = encode_server_packet(&packet).expect("packet should encode");
    let decoded = decode_server_packet(&bytes).expect("packet should decode");

    assert_eq!(decoded, packet);
}

#[test]
fn monster_info_packet_roundtrip() {
    let packet = ServerPacket::NewMonsterInfo {
        info: MonsterInfo {
            object_id: 3001,
            name: "Training Dummy".to_string(),
            name_colour_argb: -1,
            location: Point { x: 338, y: 273 },
            image: 10,
            direction: MirDirection::Left,
            effect: 0,
            ai: 0,
            light: 0,
            dead: false,
            skeleton: false,
            poison: 0,
            hidden: false,
            shock_time: 0,
            binding_shot_center: false,
            extra: false,
            extra_byte: 0,
            master_object_id: 0,
            rarity: 0,
            buffs: vec![],
        },
    };

    let bytes = encode_server_packet(&packet).expect("packet should encode");
    let decoded = decode_server_packet(&bytes).expect("packet should decode");

    assert_eq!(decoded, packet);
}

#[test]
fn npc_info_packet_roundtrip() {
    let packet = ServerPacket::NewNpcInfo {
        info: NpcInfo {
            object_id: 4001,
            name: "Village Guide".to_string(),
            name_colour_argb: -1,
            image: 5,
            colour_argb: -1,
            location: Point { x: 326, y: 271 },
            direction: MirDirection::Right,
            quest_ids: vec![1001],
        },
    };

    let bytes = encode_server_packet(&packet).expect("packet should encode");
    let decoded = decode_server_packet(&bytes).expect("packet should decode");

    assert_eq!(decoded, packet);
}

fn write_select_info(
    writer: &mut PacketWriter,
    index: i32,
    name: &str,
    level: u16,
    class: MirClass,
    gender: MirGender,
    last_access_binary_datetime: i64,
) {
    writer.write_i32(index);
    writer.write_string(name).expect("select info name");
    writer.write_u16(level);
    writer.write_u8(class as u8);
    writer.write_u8(gender as u8);
    writer.write_i64(last_access_binary_datetime);
}
