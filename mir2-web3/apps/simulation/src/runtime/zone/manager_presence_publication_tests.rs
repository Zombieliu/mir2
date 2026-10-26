use super::super::{ZoneChatProfile, ZoneExperienceProfile, ZoneGuildExperienceMembership};
use super::*;
use mir2_protocol::{MirClass, MirGender, ServerPacket};
use serde_json::json;

fn session() -> SessionId {
    SessionId::new("owner")
}

fn key() -> ZoneKey {
    ZoneKey::for_map("0")
}

fn fixture(mode: u8) -> ZoneManager {
    let mut manager = ZoneManager::new();
    manager
        .bind_logical_world(LogicalWorldId::legacy_execution_domain("presence-test").unwrap())
        .unwrap();
    for (name, object_id, map, x, y) in [
        ("owner", 101, "0", 332, 270),
        ("peer", 102, "0", 332, 271),
        ("remote", 103, "d024", 10, 20),
    ] {
        manager.join(ZoneJoin {
            session_id: SessionId::new(name),
            account_id: format!("account-{name}"),
            character_index: object_id as i32,
            object_id,
            name: name.into(),
            class: MirClass::Taoist,
            gender: MirGender::Male,
            level: 40,
            hp: 1000,
            max_hp: 1000,
            mp: 1000,
            map_file_name: map.into(),
            position: Point { x, y },
            direction: MirDirection::Right,
            chat_profile: ZoneChatProfile {
                pet_mode: mode,
                attack_mode: 5,
                ..Default::default()
            },
            combat_stats: Default::default(),
        });
    }
    manager
}

fn profile() -> ZoneExperienceProfile {
    ZoneExperienceProfile {
        account_id: "account-owner".into(),
        character_index: 101,
        level: 45,
        no_experience: false,
        can_gain_experience: true,
        guild: Some(ZoneGuildExperienceMembership {
            guild_id: "current-guild".into(),
            membership_epoch: 7,
        }),
        lover_partner: None,
        mentee_partner: None,
        reduce_monster_level_difference: true,
        world_experience_rate_bits: 1.0f32.to_bits(),
        monster_multiplier: 1,
        natural_kill_multiplier: 1,
        lover_eligible: false,
        lover_rate_percent: 0,
        mentee_eligible: false,
        mentee_rate_percent: 0,
        experience_rate_percent: 12,
        rate_source: None,
        guild_experience_rate_bits: 0.01f32.to_bits(),
    }
}

fn chat(manager: &ZoneManager) -> ZoneChatProfile {
    manager
        .zone(&key())
        .unwrap()
        .player_chat_profile(&session())
        .unwrap()
}

fn separately(
    manager: &mut ZoneManager,
    profile: Option<ZoneExperienceProfile>,
    chat_profile: ZoneChatProfile,
) -> (bool, Vec<ZoneOutbound>) {
    if !manager.update_experience_profile(&session(), profile) {
        return (false, Vec::new());
    }
    (
        true,
        manager.handle(ZoneCommand::UpdateChatProfile {
            session_id: session(),
            profile: chat_profile,
        }),
    )
}

fn assert_live_state_equal(reference: &ZoneManager, candidate: &ZoneManager) {
    assert_eq!(
        candidate.checkpoint_bytes().unwrap(),
        reference.checkpoint_bytes().unwrap()
    );
    assert_eq!(
        candidate.admitted_action_count(),
        reference.admitted_action_count()
    );
    assert_eq!(
        candidate.last_admitted_action(),
        reference.last_admitted_action()
    );
    assert_eq!(
        serde_json::to_value(candidate.online_snapshot()).unwrap(),
        serde_json::to_value(reference.online_snapshot()).unwrap()
    );
    for (key, zone) in &candidate.zones {
        assert_eq!(
            zone.retained_presence_publication_snapshot(),
            reference.zones[key].retained_presence_publication_snapshot()
        );
    }
}

fn compare_projection(
    initial: &ZoneManager,
    profile: Option<ZoneExperienceProfile>,
    chat_profile: ZoneChatProfile,
) -> (ZoneManager, Vec<ZoneOutbound>) {
    // A live fork retains the online namespace and pending runtime FIFOs; a
    // cold checkpoint restore intentionally cannot be the reference here.
    let mut reference = initial.fork_for_deterministic_reference();
    let mut candidate = initial.fork_for_deterministic_reference();
    let expected = separately(&mut reference, profile.clone(), chat_profile.clone());
    let actual = candidate.update_experience_and_chat_profile(&session(), profile, chat_profile);
    assert_eq!(actual, expected);
    assert_live_state_equal(&reference, &candidate);
    let followup = chat(&candidate);
    assert_eq!(
        candidate.handle(ZoneCommand::UpdateChatProfile {
            session_id: session(),
            profile: followup.clone(),
        }),
        reference.handle(ZoneCommand::UpdateChatProfile {
            session_id: session(),
            profile: followup,
        })
    );
    assert_live_state_equal(&reference, &candidate);
    (candidate, actual.1)
}

fn packets(outbounds: &[ZoneOutbound]) -> impl Iterator<Item = &ServerPacket> {
    outbounds.iter().flat_map(|outbound| match outbound {
        ZoneOutbound::ToSession { packets, .. } | ZoneOutbound::ToMany { packets, .. } => {
            packets.as_slice()
        }
        _ => &[],
    })
}

fn pet_target_count(manager: &ZoneManager) -> usize {
    let checkpoint: serde_json::Value =
        serde_json::from_slice(&manager.zones[&key()].checkpoint_bytes().unwrap()).unwrap();
    checkpoint["entity_combat"]["owned_pet"]["targets"]
        .as_object()
        .map_or(0, |targets| targets.len())
}

#[test]
fn combined_matches_separate_same_and_changed_chat_all_pet_modes() {
    for mode in [0, 2, 1, 3] {
        for changed in [false, true] {
            let mut initial = fixture(mode);
            install_clock(&mut initial);
            initial
                .zones
                .get_mut(&key())
                .unwrap()
                .prepare_presence_publication_health_and_poison(&session());
            initial
                .zones
                .get_mut(&key())
                .unwrap()
                .prepare_presence_publication_pet_target(&session());
            assert_eq!(pet_target_count(&initial), 1);
            assert!(initial
                .zones
                .values()
                .all(|zone| zone.online_projection_owners_match(&initial.online_identities)));
            let mut next_chat = chat(&initial);
            if changed {
                next_chat.guild_name = Some("Current Guild".into());
                next_chat.active_guild_wars = vec!["Enemy Guild".into()];
            }
            let (candidate, out) = compare_projection(&initial, Some(profile()), next_chat.clone());
            assert_eq!(chat(&candidate), next_chat);
            assert_eq!(
                candidate.player_experience_profile(&session()),
                Some(profile())
            );
            let mp_fifo = out
                .iter()
                .filter_map(|outbound| match outbound {
                    ZoneOutbound::OwnerHealthChanged { change } => Some(change.mp),
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert_eq!(mp_fifo, vec![999, 998]);
            assert_eq!(
                pet_target_count(&candidate),
                usize::from(!matches!(mode, 1 | 3))
            );
            assert!(packets(&out).any(|packet| matches!(
                packet,
                ServerPacket::ObjectPoisoned {
                    object_id: 9001,
                    poison: 0
                }
            )));
        }
    }
}

#[test]
fn combined_keeps_pending_vampire_burst_and_pet_death_flushes() {
    let mut vampire = fixture(0);
    vampire
        .zones
        .get_mut(&key())
        .unwrap()
        .prepare_presence_publication_vampire_death(&session());
    let (candidate, out) = compare_projection(&vampire, Some(profile()), chat(&vampire));
    assert_eq!(candidate.player_vitals(&session()).unwrap().0, 980);
    assert!(out.iter().any(|outbound| matches!(outbound,
        ZoneOutbound::PlayerDamaged { session_id, damage: 20, .. } if session_id == &session())));

    let mut pet = fixture(3);
    pet.zones
        .get_mut(&key())
        .unwrap()
        .prepare_presence_publication_pet_death(&session());
    let (_, out) = compare_projection(&pet, Some(profile()), chat(&pet));
    assert!(packets(&out).any(|packet| matches!(packet,
        ServerPacket::ObjectDied { info } if info.object_id == 701)));
}

fn install_clock(manager: &mut ZoneManager) {
    let online = manager.online_identities.owner(&session()).unwrap();
    let clock = serde_json::from_value(json!({
        "online": online,
        "last_hit_until_ms": 0,
        "brown_until_ms": 10_000
    }))
    .unwrap();
    manager
        .zones
        .get_mut(&key())
        .unwrap()
        .restore_player_owned_pet_clock(&session(), clock);
    manager.refresh_online_presence();
}

#[test]
fn stale_owner_certificate_falls_back_when_pet_clock_projection_needs_two_refreshes() {
    for missing in [true, false] {
        let mut initial = fixture(0);
        install_clock(&mut initial);
        let mut stale = initial.online_snapshot();
        if missing {
            stale.remove(&session());
        } else {
            stale.get_mut(&session()).unwrap().owner.epoch += 1;
        }
        initial
            .zones
            .get_mut(&key())
            .unwrap()
            .ingest_online_presence(stale, true);
        assert!(!initial.zones[&key()].online_projection_owners_match(&initial.online_identities));
        let first = initial.online_snapshot();
        assert!(first[&session()].owned_pet_clock.is_none());
        assert_eq!(first[&session()].brown_until_ms, 0);
        let mut refreshed = initial.fork_for_deterministic_reference();
        refreshed.refresh_online_presence();
        let second = refreshed.online_snapshot();
        assert!(second[&session()].owned_pet_clock.is_some());
        assert_eq!(second[&session()].brown_until_ms, 10_000);
        let (candidate, _) = compare_projection(&initial, Some(profile()), chat(&initial));
        let projected = candidate.online_snapshot();
        assert!(projected[&session()].owned_pet_clock.is_some());
        assert_eq!(projected[&session()].brown_until_ms, 10_000);
    }
}

#[test]
fn invalid_profile_and_exhausted_chat_admission_keep_separate_call_results() {
    let initial = fixture(0);
    let mut invalid = profile();
    invalid.account_id = "different-account".into();
    let before = initial.checkpoint_bytes().unwrap();
    let (candidate, out) = compare_projection(&initial, Some(invalid), chat(&initial));
    assert!(out.is_empty());
    assert_eq!(candidate.player_experience_profile(&session()), None);
    let mut rejected = initial.fork_for_deterministic_reference();
    let mut wrong = profile();
    wrong.character_index = -1;
    assert_eq!(
        rejected.update_experience_and_chat_profile(&session(), Some(wrong), chat(&initial)),
        (false, Vec::new())
    );
    assert_eq!(rejected.checkpoint_bytes().unwrap(), before);

    let mut exhausted = fixture(0);
    exhausted.admission_sequence = u64::MAX;
    let mut receipt = serde_json::to_value(exhausted.last_admission.as_ref().unwrap()).unwrap();
    receipt["id"] = serde_json::to_value(
        super::super::StableActionId::from_source_sequence(
            exhausted.logical_world.clone().unwrap(),
            key(),
            u64::MAX,
        )
        .unwrap(),
    )
    .unwrap();
    exhausted.last_admission = Some(serde_json::from_value(receipt).unwrap());
    let mut next_chat = chat(&exhausted);
    next_chat.guild_name = Some("must not be applied".into());
    let (candidate, out) = compare_projection(&exhausted, Some(profile()), next_chat);
    assert!(out.is_empty());
    assert_eq!(
        candidate.player_experience_profile(&session()),
        Some(profile())
    );
    assert_eq!(chat(&candidate).guild_name, None);
    assert_eq!(candidate.admitted_action_count(), u64::MAX);
}

#[test]
fn absent_unadmitted_owners_and_last_zone_move_keep_original_projection() {
    let mut initial = fixture(0);
    initial.online_identities = OnlineIdentityBook::default();
    for zone in initial.zones.values_mut() {
        zone.ingest_online_presence(BTreeMap::new(), true);
    }
    assert!(initial
        .zones
        .values()
        .all(|zone| zone.online_projection_owners_match(&initial.online_identities)));
    compare_projection(&initial, None, chat(&initial));

    let mut moved = fixture(0);
    install_clock(&mut moved);
    moved
        .zones
        .get_mut(&key())
        .unwrap()
        .prepare_presence_publication_health_and_poison(&session());
    let mut cloned = moved.fork_for_deterministic_reference();
    let snapshot = cloned.online_snapshot();
    for zone in cloned.zones.values_mut() {
        zone.ingest_online_presence(snapshot.clone(), true);
    }
    moved.refresh_online_presence();
    assert_live_state_equal(&cloned, &moved);
    let next_chat = chat(&moved);
    assert_eq!(
        moved.handle(ZoneCommand::UpdateChatProfile {
            session_id: session(),
            profile: next_chat.clone(),
        }),
        cloned.handle(ZoneCommand::UpdateChatProfile {
            session_id: session(),
            profile: next_chat,
        })
    );
    assert_live_state_equal(&cloned, &moved);

    for maps in [0, 1, 3] {
        let mut moved = fixture(0);
        moved.zones.clear();
        moved.session_zones.clear();
        for index in 0..maps {
            moved.install_empty_zone(ZoneRuntime::new(ZoneKey::for_map(format!("empty-{index}"))));
        }
        let mut cloned = moved.fork_for_deterministic_reference();
        let snapshot = cloned.online_snapshot();
        for zone in cloned.zones.values_mut() {
            zone.ingest_online_presence(snapshot.clone(), true);
        }
        moved.refresh_online_presence();
        assert_live_state_equal(&cloned, &moved);
    }
}
