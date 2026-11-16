//! Prepared server profiles, real native Zone attack/death and issued proofs.
//! These checks do not claim socket, native-client or public-play acceptance.
use mir2_protocol::{MirClass, MirDirection, MirGender, Point, Spell};
use mir2_simulation::{
    SessionId, Stage5FriendIdentity, WorldEntityDisposition, ZoneCommand, ZoneExperiencePartner,
    ZoneExperienceProfile, ZoneExperienceRateBuff, ZoneExperienceRateSource,
    ZoneExperienceRateStat, ZoneGuildExperienceMembership, ZoneJoin, ZoneKey, ZoneManager,
    ZoneMentorBankAttribution, ZoneMonsterKillAward, ZoneMonsterSpawn, ZoneOutbound,
};
use std::collections::BTreeMap;

fn sid(name: &str) -> SessionId {
    SessionId::new(name)
}
fn identity(name: &str, index: i32) -> Stage5FriendIdentity {
    Stage5FriendIdentity {
        account_id: format!("{name}-account"),
        character_index: index,
    }
}
fn profile(name: &str, index: i32, level: u16) -> ZoneExperienceProfile {
    ZoneExperienceProfile {
        account_id: format!("{name}-account"),
        character_index: index,
        level,
        no_experience: false,
        can_gain_experience: true,
        guild: None,
        lover_partner: None,
        mentee_partner: None,
        reduce_monster_level_difference: false,
        world_experience_rate_bits: 1.0f32.to_bits(),
        monster_multiplier: 1,
        natural_kill_multiplier: 1,
        lover_eligible: false,
        lover_rate_percent: 5,
        mentee_eligible: false,
        mentee_rate_percent: 10,
        experience_rate_percent: 0,
        rate_source: None,
        guild_experience_rate_bits: 0.01f32.to_bits(),
    }
}
fn join(name: &str, index: i32, level: u16, map: &str, point: Point, roster: &[&str]) -> ZoneJoin {
    ZoneJoin {
        session_id: sid(name),
        account_id: format!("{name}-account"),
        character_index: index,
        object_id: index as u32 + 100,
        name: name.into(),
        class: MirClass::Warrior,
        gender: MirGender::Male,
        level,
        hp: 1000,
        max_hp: 1000,
        mp: 0,
        map_file_name: map.into(),
        position: point,
        direction: MirDirection::Right,
        chat_profile: mir2_simulation::ZoneChatProfile {
            group_members: roster.iter().map(|name| (*name).into()).collect(),
            ..Default::default()
        },
        combat_stats: Default::default(),
    }
}
fn admit(manager: &mut ZoneManager, join: ZoneJoin, profile: ZoneExperienceProfile) {
    let session = join.session_id.clone();
    manager.handle(ZoneCommand::Join(join));
    assert!(manager.update_experience_profile(&session, Some(profile)));
    manager.handle(ZoneCommand::sync_player_combat_state(
        session,
        MirClass::Warrior,
        false,
        false,
        false,
        false,
        false,
        false,
    ));
}
fn spawn(manager: &mut ZoneManager, actor: &str, experience: u32, hp: i32, now: u64) {
    spawn_at(
        manager,
        actor,
        experience,
        hp,
        now,
        Point { x: 331, y: 270 },
    );
}
fn spawn_at(
    manager: &mut ZoneManager,
    actor: &str,
    experience: u32,
    hp: i32,
    now: u64,
    position: Point,
) {
    manager.handle(ZoneCommand::SpawnMonster {
        session_id: sid(actor),
        now_ms: now,
        monster: ZoneMonsterSpawn {
            crystal_drop_seed: None,
            object_id: 700,
            name: "Scarecrow".into(),
            name_colour_argb: -1,
            image: 0,
            ai: 0,
            disposition: Some(WorldEntityDisposition::Hostile),
            level: 10,
            max_hp: hp,
            hp,
            experience,
            move_speed_ms: 60_000,
            attack_speed_ms: 60_000,
            friendly_guild: None,
            position,
            direction: MirDirection::Left,
            defense: Default::default(),
            respawn: None,
            drops: Vec::new(),
        },
    });
}
fn attack(manager: &mut ZoneManager, actor: &str, damage: i32, now: u64) -> Vec<ZoneOutbound> {
    let direction = if manager.player_transform(&sid(actor)).unwrap().0.x > 331 {
        MirDirection::Left
    } else {
        MirDirection::Right
    };
    manager.handle(ZoneCommand::PlayerAttackObject {
        session_id: sid(actor),
        object_id: 700,
        direction,
        spell: Spell::None as u8,
        level: 0,
        attack_type: 0,
        damage,
        now_ms: now,
    });
    manager.tick_all(now)
}
fn awards(out: &[ZoneOutbound]) -> BTreeMap<String, ZoneMonsterKillAward> {
    out.iter()
        .filter_map(|outbound| match outbound {
            ZoneOutbound::OwnedMonsterKillAward {
                session_id, award, ..
            } => Some((session_id.as_str().to_string(), award.clone())),
            _ => None,
        })
        .collect()
}

fn safe_potion_peer() -> (ZoneManager, ZoneExperienceProfile) {
    let source = mir2_game_data::crystal_map_respawns_ref("0").unwrap();
    assert!(source
        .safe_zones
        .iter()
        .any(|safe| { safe.location == Point { x: 328, y: 264 } && safe.size == 10 }));
    let mut manager = ZoneManager::new();
    let roster = ["Owner", "Near"];
    admit(
        &mut manager,
        join("Owner", 1, 20, "0", Point { x: 330, y: 275 }, &roster),
        profile("Owner", 1, 20),
    );
    let mut potion = profile("Near", 2, 20);
    // The buff acquisition and short clock are prepared; ordinary movement
    // uses the real imported safe-area geometry before real native death.
    potion.rate_source = Some(ZoneExperienceRateSource {
        captured_at_ms: 0,
        base_experience_rate_percent: 0,
        base_lover_rate_percent: 0,
        base_mentee_rate_percent: 0,
        buffs: vec![ZoneExperienceRateBuff {
            expires_at_ms: Some(1000),
            lover_buff: false,
            mentee_buff: false,
            newbie_buff: false,
            pause_in_safe_zone: true,
            paused_remaining_ms: None,
            source_buff_key: Some("exp".into()),
            source_buff_instance: 41,
            stats: vec![ZoneExperienceRateStat {
                stat: 100,
                value: 30,
            }],
        }],
        guild_stats: Vec::new(),
    });
    admit(
        &mut manager,
        join("Near", 2, 20, "0", Point { x: 339, y: 270 }, &roster),
        potion.clone(),
    );
    (manager, potion)
}

fn safe_move(
    manager: &mut ZoneManager,
    direction: MirDirection,
    now: u64,
    seq: u64,
) -> Vec<ZoneOutbound> {
    let mut output = manager.handle(ZoneCommand::Walk {
        session_id: sid("Near"),
        direction,
        seq,
        now_ms: now,
    });
    output.extend(manager.tick_all(now));
    output
}

#[test]
fn accepted_safe_entry_pauses_party_potion_before_peer_private_drain_and_stale_refresh() {
    let (mut manager, stale_running) = safe_potion_peer();
    let moved = safe_move(&mut manager, MirDirection::Left, 100, 1);
    assert_eq!(
        manager.player_transform(&sid("Near")).unwrap().0,
        Point { x: 338, y: 270 }
    );
    assert!(moved.iter().any(|out| matches!(out,
        ZoneOutbound::ToSession { session_id, packets } if session_id == &sid("Near")
            && packets.iter().any(|packet| matches!(packet, mir2_protocol::ServerPacket::PauseBuff { buff_type: 102, paused: true, .. }))
    )));
    // An owner read started before this accepted step cannot rewind the Zone.
    assert!(manager.update_experience_profile(&sid("Near"), Some(stale_running)));
    let paused = manager.player_experience_profile(&sid("Near")).unwrap();
    let row = &paused.rate_source.as_ref().unwrap().buffs[0];
    assert_eq!(row.paused_remaining_ms, Some(900));
    assert_eq!(row.expires_at_ms, None);
    spawn_at(
        &mut manager,
        "Owner",
        1000,
        1,
        200,
        Point { x: 331, y: 275 },
    );
    let awarded = awards(&attack(&mut manager, "Owner", 1, 200));
    let peer = awarded["Near"].experience_selection.as_ref().unwrap();
    assert_eq!(peer.killed_at_ms, 200);
    assert_eq!(peer.final_amount, 650);
}

#[test]
fn accepted_safe_exit_resumes_remaining_potion_time_and_new_stack_replaces_old_clock() {
    for (killed_at, expected) in [(5999, 845), (6000, 650)] {
        let (mut manager, _) = safe_potion_peer();
        safe_move(&mut manager, MirDirection::Left, 100, 1);
        let stale_paused = manager.player_experience_profile(&sid("Near")).unwrap();
        let moved = safe_move(&mut manager, MirDirection::Right, 5100, 2);
        assert_eq!(
            manager.player_transform(&sid("Near")).unwrap().0,
            Point { x: 339, y: 270 }
        );
        assert!(moved.iter().any(|out| matches!(out,
            ZoneOutbound::ToSession { session_id, packets } if session_id == &sid("Near")
                && packets.iter().any(|packet| matches!(packet, mir2_protocol::ServerPacket::PauseBuff { buff_type: 102, paused: false, .. }))
        )));
        assert!(manager.update_experience_profile(&sid("Near"), Some(stale_paused)));
        let resumed = manager.player_experience_profile(&sid("Near")).unwrap();
        let row = &resumed.rate_source.as_ref().unwrap().buffs[0];
        assert_eq!(row.paused_remaining_ms, None);
        assert_eq!(row.expires_at_ms, Some(6000));
        spawn_at(
            &mut manager,
            "Owner",
            1000,
            1,
            killed_at,
            Point { x: 331, y: 275 },
        );
        let awarded = awards(&attack(&mut manager, "Owner", 1, killed_at));
        assert_eq!(
            awarded["Near"]
                .experience_selection
                .as_ref()
                .unwrap()
                .final_amount,
            expected
        );
    }
    let (mut manager, mut replacement) = safe_potion_peer();
    safe_move(&mut manager, MirDirection::Left, 100, 1);
    safe_move(&mut manager, MirDirection::Right, 5100, 2);
    let source = replacement.rate_source.as_mut().unwrap();
    source.captured_at_ms = 5500;
    source.buffs[0].source_buff_instance = 42;
    source.buffs[0].expires_at_ms = Some(10_500);
    assert!(manager.update_experience_profile(&sid("Near"), Some(replacement)));
    assert_eq!(
        manager
            .player_experience_profile(&sid("Near"))
            .unwrap()
            .rate_source
            .unwrap()
            .buffs[0]
            .expires_at_ms,
        Some(10_500)
    );
}

#[test]
fn original_can_gain_exp_gate_does_not_suppress_other_party_recipients_or_redistribute_share() {
    for disabled in ["Owner", "Near"] {
        let mut manager = ZoneManager::new();
        let roster = ["Owner", "Near"];
        for (name, index, x) in [("Owner", 1, 330), ("Near", 2, 332)] {
            let mut source = profile(name, index, 20);
            source.can_gain_experience = name != disabled;
            admit(
                &mut manager,
                join(name, index, 20, "xp-test", Point { x, y: 270 }, &roster),
                source,
            );
        }
        spawn(&mut manager, "Owner", 1000, 1, 0);
        let awarded = awards(&attack(&mut manager, "Owner", 1, 100));
        assert_eq!(awarded.len(), 2);
        for name in ["Owner", "Near"] {
            assert_eq!(awarded[name].experience, 650);
            let selected = awarded[name].experience_selection.as_ref().unwrap();
            assert_eq!(
                selected.final_amount,
                if name == disabled { 0 } else { 650 }
            );
            assert_eq!(selected.guild_amount, 0);
        }
    }
}

#[test]
fn native_party_death_uses_each_recipient_timed_rates_at_the_exact_source_clock() {
    let mut issued = Vec::new();
    for (killed_at, expected_peer) in [(999, 851), (1000, 721)] {
        let mut manager = ZoneManager::new();
        let roster = ["Owner", "Near"];
        for (name, index, x) in [("Owner", 1, 330), ("Near", 2, 332)] {
            let mut source = profile(name, index, 20);
            if name == "Near" {
                source.rate_source = Some(ZoneExperienceRateSource {
                    captured_at_ms: 0,
                    base_experience_rate_percent: 7,
                    base_lover_rate_percent: 0,
                    base_mentee_rate_percent: 0,
                    buffs: vec![ZoneExperienceRateBuff {
                        expires_at_ms: Some(1000),
                        lover_buff: false,
                        mentee_buff: false,
                        newbie_buff: false,
                        pause_in_safe_zone: false,
                        paused_remaining_ms: None,
                        source_buff_key: None,
                        source_buff_instance: 0,
                        stats: vec![ZoneExperienceRateStat {
                            stat: 100,
                            value: 20,
                        }],
                    }],
                    guild_stats: vec![ZoneExperienceRateStat {
                        stat: 100,
                        value: 4,
                    }],
                });
            }
            admit(
                &mut manager,
                join(name, index, 20, "xp-test", Point { x, y: 270 }, &roster),
                source,
            );
        }
        spawn(&mut manager, "Owner", 1000, 10, 0);
        let out = attack(&mut manager, "Owner", 10, killed_at);
        let got = awards(&out);
        assert_eq!(got["Owner"].experience, 650);
        // The envelope retains the post-party WinExp share. The immutable
        // selection carries the recipient's final GainExp amount exactly once.
        assert_eq!(got["Near"].experience, 650);
        let selection = got["Near"].experience_selection.as_ref().unwrap();
        assert_eq!(selection.killed_at_ms, killed_at);
        assert_eq!(selection.final_amount, expected_peer);
        for outbound in &out {
            if let ZoneOutbound::OwnedMonsterKillAward {
                source,
                session_id,
                online_owner,
                award,
            } = outbound
            {
                assert!(manager.issued_monster_award_is_current(
                    source,
                    session_id,
                    online_owner,
                    award
                ));
            }
        }
        issued.push(selection.clone());
    }
    assert_eq!(issued[0].final_amount, 851);
    assert_eq!(issued[1].final_amount, 721);
}

#[test]
fn native_profile_rejects_invalid_expired_rate_inputs_before_award_admission() {
    let mut manager = ZoneManager::new();
    admit(
        &mut manager,
        join("Owner", 1, 20, "xp-test", Point { x: 330, y: 270 }, &[]),
        profile("Owner", 1, 20),
    );
    let mut bad = profile("Owner", 1, 20);
    bad.rate_source = Some(ZoneExperienceRateSource {
        captured_at_ms: 1000,
        base_experience_rate_percent: 0,
        base_lover_rate_percent: 0,
        base_mentee_rate_percent: 0,
        buffs: vec![ZoneExperienceRateBuff {
            expires_at_ms: Some(0),
            lover_buff: false,
            mentee_buff: false,
            newbie_buff: false,
            pause_in_safe_zone: false,
            paused_remaining_ms: None,
            source_buff_key: None,
            source_buff_instance: 0,
            stats: vec![ZoneExperienceRateStat {
                stat: 255,
                value: 20,
            }],
        }],
        guild_stats: Vec::new(),
    });
    assert!(!manager.update_experience_profile(&sid("Owner"), Some(bad)));
    assert!(manager
        .player_experience_profile(&sid("Owner"))
        .unwrap()
        .rate_source
        .is_none());
}

#[test]
fn original_party_counts_nearby_dead_and_cross_map_levels_then_truncates_alive_shares() {
    let mut manager = ZoneManager::new();
    let roster = ["Owner", "Near", "Dead", "Cross", "Far"];
    for (name, index, level, map, x) in [
        ("Owner", 1, 10, "xp-test", 330),
        ("Near", 2, 30, "xp-test", 332),
        ("Dead", 3, 60, "xp-test", 334),
        ("Cross", 4, 20, "xp-cross", 330),
        ("Far", 5, 90, "xp-test", 347),
    ] {
        let mut source = profile(name, index, level);
        // Only the EXPOwner's pre-party multiplier is used.
        source.monster_multiplier = if name == "Owner" { 2 } else { 9 };
        admit(
            &mut manager,
            join(name, index, level, map, Point { x, y: 270 }, &roster),
            source,
        );
    }
    manager.handle(ZoneCommand::SyncPlayerVitalsAndLife {
        session_id: sid("Dead"),
        hp: 0,
        max_hp: 1000,
        mp: 0,
        dead: true,
    });
    spawn(&mut manager, "Owner", 101, 10, 0);
    let got = awards(&attack(&mut manager, "Owner", 10, 10));
    // Four nearby references: level sum120, source rate1.5, owner raw202.
    assert_eq!(got.len(), 2);
    assert_eq!(got["Owner"].experience, 25);
    assert_eq!(got["Near"].experience, 75);
    assert_eq!(
        got["Owner"]
            .experience_selection
            .as_ref()
            .unwrap()
            .final_amount,
        25
    );
    assert_eq!(
        got["Near"]
            .experience_selection
            .as_ref()
            .unwrap()
            .final_amount,
        75
    );
    assert!(!got.contains_key("Dead") && !got.contains_key("Cross") && !got.contains_key("Far"));
    assert_eq!(got.values().map(|award| award.experience).sum::<u32>(), 100);
}

#[test]
fn original_party_square_range_includes_sixteen_and_excludes_seventeen() {
    let mut manager = ZoneManager::new();
    let roster = ["Owner", "Edge", "Outside"];
    for (name, index, x, y) in [
        ("Owner", 1, 330, 270),
        ("Edge", 2, 346, 286),
        ("Outside", 3, 347, 270),
    ] {
        admit(
            &mut manager,
            join(name, index, 10, "xp-test", Point { x, y }, &roster),
            profile(name, index, 10),
        );
    }
    spawn(&mut manager, "Owner", 101, 10, 0);
    let got = awards(&attack(&mut manager, "Owner", 10, 10));
    assert_eq!(got.len(), 2);
    assert_eq!(got["Owner"].experience, 65);
    assert_eq!(got["Edge"].experience, 65);
    assert_eq!(got.values().map(|award| award.experience).sum::<u32>(), 130);
}

#[test]
fn original_party_multiplier_caps_at_eleven_without_capping_level_sum() {
    let mut manager = ZoneManager::new();
    let names: Vec<_> = (0..12).map(|index| format!("Member{index}")).collect();
    let refs: Vec<_> = names.iter().map(String::as_str).collect();
    for (index, name) in names.iter().enumerate() {
        let point = Point {
            x: 330 + index as i32,
            y: if index == 0 { 270 } else { 271 },
        };
        admit(
            &mut manager,
            join(name, index as i32, 10, "xp-test", point, &refs),
            profile(name, index as i32, 10),
        );
    }
    spawn(&mut manager, "Member0", 1200, 10, 0);
    let got = awards(&attack(&mut manager, "Member0", 10, 10));
    assert_eq!(got.len(), 12);
    assert!(got.values().all(|award| award.experience == 220));
}

#[test]
fn generic_boss_retains_exp_owner_even_when_party_peer_has_more_damage() {
    let mut manager = ZoneManager::new();
    let roster = ["Owner", "HeavyPeer"];
    for (name, index, x, multiplier) in [("Owner", 1, 330, 2), ("HeavyPeer", 2, 332, 9)] {
        let mut source = profile(name, index, 10);
        source.monster_multiplier = multiplier;
        admit(
            &mut manager,
            join(name, index, 10, "xp-test", Point { x, y: 270 }, &roster),
            source,
        );
    }
    let boss = mir2_game_data::crystal_monster_by_name("WoomaTaurus").unwrap();
    assert!(
        boss.is_boss || mir2_game_data::platinum_176_monster_is_boss(&boss.name),
        "the runtime's actual imported/template boss classification"
    );
    manager.handle(ZoneCommand::SpawnMonster {
        session_id: sid("Owner"),
        now_ms: 0,
        monster: ZoneMonsterSpawn {
            crystal_drop_seed: None,
            object_id: 700,
            name: boss.name.clone(),
            name_colour_argb: -1,
            image: boss.image,
            // Prepared ordinary AI isolates ownership from encounter mechanics.
            ai: 0,
            disposition: Some(WorldEntityDisposition::Hostile),
            level: 10,
            max_hp: 10,
            hp: 10,
            experience: 100,
            move_speed_ms: 60_000,
            attack_speed_ms: 60_000,
            friendly_guild: None,
            position: Point { x: 331, y: 270 },
            direction: MirDirection::Left,
            defense: Default::default(),
            respawn: None,
            drops: Vec::new(),
        },
    });
    assert!(awards(&attack(&mut manager, "Owner", 1, 10)).is_empty());
    let issued = attack(&mut manager, "HeavyPeer", 9, 620);
    let got = awards(&issued);
    assert_eq!(got.len(), 2);
    assert_eq!(got["Owner"].experience, 130);
    assert_eq!(got["HeavyPeer"].experience, 130);
    let audit = got["Owner"].boss_audit.as_ref().unwrap();
    assert_eq!(audit.reward_owner_session_id, sid("Owner"));
    assert_eq!(audit.last_hit_session_id, sid("HeavyPeer"));
    assert_eq!(audit.damage_contributions[&sid("Owner")], 1);
    assert_eq!(audit.damage_contributions[&sid("HeavyPeer")], 9);
    assert!(got["HeavyPeer"].boss_audit.is_none());
    for event in issued {
        if let ZoneOutbound::OwnedMonsterKillAward {
            source,
            session_id,
            online_owner,
            award,
        } = event
        {
            assert!(manager.issued_monster_award_is_current(
                &source,
                &session_id,
                &online_owner,
                &award
            ));
        }
    }
}

#[test]
fn original_party_selection_is_authenticated_by_the_source_issued_payload() {
    let mut manager = ZoneManager::new();
    let mut source = profile("Owner", 1, 10);
    source.guild = Some(ZoneGuildExperienceMembership {
        guild_id: "0123456789abcdef0123456789abcdef".into(),
        membership_epoch: 7,
    });
    admit(
        &mut manager,
        join("Owner", 1, 10, "xp-test", Point { x: 330, y: 270 }, &[]),
        source,
    );
    spawn(&mut manager, "Owner", 1000, 10, 0);
    let out = attack(&mut manager, "Owner", 10, 10);
    let (session, zone, proof, award) = out
        .iter()
        .find_map(|outbound| match outbound {
            ZoneOutbound::OwnedMonsterKillAward {
                session_id,
                source,
                online_owner,
                award,
            } => Some((session_id, source, online_owner, award)),
            _ => None,
        })
        .expect("issued death award");
    assert!(manager.issued_monster_award_is_current(zone, session, proof, award));
    assert_eq!(
        award.experience_selection.as_ref().unwrap().guild_amount,
        10
    );
    for field in ["amount", "guild-amount", "epoch", "guild", "missing"] {
        let mut forged = award.clone();
        if field == "missing" {
            forged.experience_selection = None;
        } else {
            let selection = forged.experience_selection.as_mut().unwrap();
            match field {
                "amount" => selection.final_amount += 1,
                "guild-amount" => selection.guild_amount += 1,
                "epoch" => selection.guild.as_mut().unwrap().membership_epoch += 1,
                "guild" => {
                    selection.guild.as_mut().unwrap().guild_id =
                        "1123456789abcdef0123456789abcdef".into()
                }
                _ => unreachable!(),
            }
        }
        assert!(
            !manager.issued_monster_award_is_current(zone, session, proof, &forged),
            "tamper {field}"
        );
    }
    let mut prepared = award.clone();
    prepared.source_receipt_key = Some("server-receipt".into());
    assert!(manager.issued_monster_award_is_current(zone, session, proof, &prepared));
}

#[test]
fn retained_cross_map_exp_owner_uses_its_current_map_for_party_and_lover_eligibility() {
    let mut manager = ZoneManager::new();
    let roster = ["Owner", "DestinationPeer", "SourceKiller"];
    let mut owner_profile = profile("Owner", 1, 10);
    owner_profile.lover_partner = Some(ZoneExperiencePartner {
        identity: identity("DestinationPeer", 2),
        relationship_epoch: 3,
    });
    owner_profile.lover_eligible = true;
    let mut peer_profile = profile("DestinationPeer", 2, 10);
    peer_profile.lover_partner = Some(ZoneExperiencePartner {
        identity: identity("Owner", 1),
        relationship_epoch: 3,
    });
    admit(
        &mut manager,
        join(
            "Owner",
            1,
            10,
            "xp-source",
            Point { x: 330, y: 270 },
            &roster,
        ),
        owner_profile.clone(),
    );
    admit(
        &mut manager,
        join(
            "SourceKiller",
            3,
            10,
            "xp-source",
            Point { x: 332, y: 270 },
            &roster,
        ),
        profile("SourceKiller", 3, 10),
    );
    admit(
        &mut manager,
        join(
            "DestinationPeer",
            2,
            10,
            "xp-destination",
            Point { x: 334, y: 270 },
            &roster,
        ),
        peer_profile,
    );
    spawn(&mut manager, "Owner", 1000, 10, 0);
    assert!(awards(&attack(&mut manager, "Owner", 1, 10)).is_empty());
    let mut transferred = join(
        "Owner",
        1,
        10,
        "xp-destination",
        Point { x: 330, y: 270 },
        &roster,
    );
    transferred.direction = MirDirection::Right;
    manager.handle(ZoneCommand::Join(transferred));
    assert!(manager.update_experience_profile(&sid("Owner"), Some(owner_profile)));
    let got = awards(&attack(&mut manager, "SourceKiller", 9, 620));
    // Three nearby levels counted even across maps; only destination receives.
    assert_eq!(got.len(), 2);
    assert_eq!(got["Owner"].experience, 466);
    assert_eq!(
        got["Owner"]
            .experience_selection
            .as_ref()
            .unwrap()
            .final_amount,
        489
    );
    assert_eq!(got["DestinationPeer"].experience, 466);
    assert!(!got.contains_key("SourceKiller"));
    assert_eq!(
        got["Owner"]
            .experience_selection
            .as_ref()
            .unwrap()
            .source_zone,
        serde_json::to_string(&ZoneKey::for_map("xp-source")).unwrap()
    );
}

#[test]
fn social_capture_requires_actual_partner_range_life_group_and_relationship_epoch() {
    for (case, spouse_x, teacher_dead, bank_epoch, teacher_group, expected_owner) in [
        ("eligible", 346, false, 3, true, 900),
        ("lover-outside", 347, false, 3, true, 858),
        ("dead-teacher", 346, true, 3, true, 818),
        ("wrong-epoch", 346, false, 4, true, 818),
        ("different-group", 346, false, 3, false, 818),
    ] {
        let mut manager = ZoneManager::new();
        let roster = ["Owner", "Teacher"];
        let mut source = profile("Owner", 1, 10);
        source.lover_partner = Some(ZoneExperiencePartner {
            identity: identity("Spouse", 3),
            relationship_epoch: 2,
        });
        source.mentee_partner = Some(ZoneExperiencePartner {
            identity: identity("Teacher", 2),
            relationship_epoch: 3,
        });
        source.lover_eligible = true;
        source.mentee_eligible = true;
        source.experience_rate_percent = 20;
        let mut spouse = profile("Spouse", 3, 10);
        spouse.lover_partner = Some(ZoneExperiencePartner {
            identity: identity("Owner", 1),
            relationship_epoch: 2,
        });
        admit(
            &mut manager,
            join("Owner", 1, 10, "xp-test", Point { x: 330, y: 270 }, &roster),
            source,
        );
        admit(
            &mut manager,
            join(
                "Teacher",
                2,
                10,
                "xp-test",
                Point { x: 332, y: 270 },
                if teacher_group { &roster } else { &[] },
            ),
            profile("Teacher", 2, 10),
        );
        admit(
            &mut manager,
            join(
                "Spouse",
                3,
                10,
                "xp-test",
                Point {
                    x: spouse_x,
                    y: 286,
                },
                &[],
            ),
            spouse,
        );
        manager.handle(ZoneCommand::UpdateMentorBank {
            session_id: sid("Owner"),
            attribution: Some(ZoneMentorBankAttribution {
                pupil: identity("Owner", 1),
                teacher: identity("Teacher", 2),
                relationship_epoch: bank_epoch,
            }),
        });
        if teacher_dead {
            manager.handle(ZoneCommand::SyncPlayerVitalsAndLife {
                session_id: sid("Teacher"),
                hp: 0,
                max_hp: 1000,
                mp: 0,
                dead: true,
            });
        }
        spawn(&mut manager, "Owner", 1000, 10, 0);
        let got = awards(&attack(&mut manager, "Owner", 10, 10));
        assert_eq!(got["Owner"].experience, 650, "case {case}");
        assert_eq!(
            got["Owner"]
                .experience_selection
                .as_ref()
                .unwrap()
                .final_amount,
            expected_owner,
            "case {case}"
        );
    }
}

#[test]
fn trusted_profile_rejects_another_character_and_refreshes_current_level() {
    let mut manager = ZoneManager::new();
    admit(
        &mut manager,
        join("Owner", 1, 10, "xp-test", Point { x: 330, y: 270 }, &[]),
        profile("Owner", 1, 10),
    );
    assert!(!manager.update_experience_profile(&sid("Owner"), Some(profile("Other", 2, 20))));
    let mut current = profile("Owner", 1, 22);
    current.reduce_monster_level_difference = true;
    assert!(manager.update_experience_profile(&sid("Owner"), Some(current)));
    spawn(&mut manager, "Owner", 101, 10, 0);
    let got = awards(&attack(&mut manager, "Owner", 10, 10));
    assert_eq!(
        got["Owner"].experience, 89,
        "level22 vs monster10 removes 2*(101/15)"
    );
}

#[test]
fn personal_source_copy_uses_current_peer_life_without_mutating_admitted_owner() {
    let mut manager = ZoneManager::new();
    let stored = profile("Owner", 1, 10);
    admit(
        &mut manager,
        join("Owner", 1, 10, "xp-test", Point { x: 330, y: 270 }, &[]),
        stored.clone(),
    );
    let mut spouse = profile("Spouse", 2, 10);
    spouse.lover_partner = Some(ZoneExperiencePartner {
        identity: identity("Owner", 1),
        relationship_epoch: 4,
    });
    admit(
        &mut manager,
        join("Spouse", 2, 10, "xp-test", Point { x: 346, y: 286 }, &[]),
        spouse,
    );
    let mut admitted_copy = stored.clone();
    admitted_copy.lover_partner = Some(ZoneExperiencePartner {
        identity: identity("Spouse", 2),
        relationship_epoch: 4,
    });
    admitted_copy.lover_eligible = true;
    let read_copy = |manager: &ZoneManager, profile| {
        manager
            .zone(&ZoneKey::for_map("xp-test"))
            .unwrap()
            .current_social_experience_profile_with(&sid("Owner"), profile)
    };
    assert!(
        read_copy(&manager, Some(admitted_copy.clone()))
            .unwrap()
            .unwrap()
            .lover_eligible
    );
    assert_eq!(
        manager.player_experience_profile(&sid("Owner")),
        Some(stored.clone())
    );
    assert!(read_copy(&manager, Some(profile("Other", 3, 10))).is_err());
    assert_eq!(read_copy(&manager, None).unwrap(), None);

    // GainExp allows a spawned dead recipient, while the bonus peer must live.
    manager.handle(ZoneCommand::SyncPlayerVitalsAndLife {
        session_id: sid("Owner"),
        hp: 0,
        max_hp: 1000,
        mp: 0,
        dead: true,
    });
    assert!(
        read_copy(&manager, Some(admitted_copy.clone()))
            .unwrap()
            .unwrap()
            .lover_eligible
    );
    manager.handle(ZoneCommand::SyncPlayerVitalsAndLife {
        session_id: sid("Spouse"),
        hp: 0,
        max_hp: 1000,
        mp: 0,
        dead: true,
    });
    assert!(
        !read_copy(&manager, Some(admitted_copy))
            .unwrap()
            .unwrap()
            .lover_eligible
    );
    assert_eq!(
        manager.player_experience_profile(&sid("Owner")),
        Some(stored)
    );
}
