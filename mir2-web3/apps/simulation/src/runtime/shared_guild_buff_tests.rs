use super::*;
fn fixture() -> (
    crate::SimulationSession,
    SimulationConfig,
    Stage5FriendIdentity,
) {
    let mut session = super::super::tests::session();
    session.grant_shared_guild_creation_from_npc();
    session.submit_shared_guild_name("BuffGuild").unwrap();
    let config = session
        .app
        .world()
        .resource::<RuntimeConfigResource>()
        .config
        .clone();
    let identity = world_identity(session.app.world()).unwrap();
    (session, config, identity)
}
fn provision(config: &SimulationConfig, identity: &Stage5FriendIdentity, level: u8) {
    let id = config
        .shared_guild_for_identity(identity)
        .unwrap()
        .unwrap()
        .id;
    config
        .commit_account_store_transaction_with_guilds(&[], std::slice::from_ref(&id), |store| {
            let guild = store.shared_guilds.get_mut(&id).unwrap();
            guild.level = level;
            guild.spare_points = 255;
            guild.gold = u32::MAX;
            guild.revision += 1;
            Ok(())
        })
        .unwrap();
}
#[test]
fn guild_buff_buy_activate_costs_and_failures_are_authoritative() {
    let (_session, config, identity) = fixture();
    // Current shipped source buffs are all free-to-activate and timed; exercise
    // the supported paid config branch with a server-owned definition fixture.
    let mut definition = mir2_game_data::crystal_guild_buff_definitions()[0].clone();
    definition.activation_cost = 100;
    definition.time_limit = 2;
    definition.points_requirement = 1;
    assert!(config
        .commit_shared_guild_buff_definition(&identity, 1, &definition)
        .is_err());
    provision(&config, &identity, definition.level_requirement);
    config
        .inject_account_store_transaction_fault(crate::AccountStoreTransactionFault::BeforePersist);
    assert!(config
        .commit_shared_guild_buff_definition(&identity, 1, &definition)
        .is_err());
    let before = config
        .shared_guild_for_identity(&identity)
        .unwrap()
        .unwrap();
    assert_eq!(before.spare_points, 255);
    assert_eq!(before.gold, u32::MAX);
    assert!(before.buffs.is_empty());
    let packets = config
        .commit_shared_guild_buff_definition(&identity, 1, &definition)
        .unwrap();
    assert!(packets.iter().any(|packet|matches!(packet,ServerPacket::GuildBuffList{active_buffs,..} if active_buffs.iter().any(|buff|buff.id==definition.id&&buff.active))));
    let bought = config
        .shared_guild_for_identity(&identity)
        .unwrap()
        .unwrap();
    assert_eq!(bought.gold, u32::MAX - definition.activation_cost as u32);
    assert_eq!(bought.spare_points, 255 - definition.points_requirement);
    assert!(config
        .commit_shared_guild_buff_definition(&identity, 1, &definition)
        .is_err());
    assert!(config
        .commit_shared_guild_buff_definition(&identity, 2, &definition)
        .is_err());
    let guild_id = bought.id.clone();
    config
        .commit_account_store_transaction_with_guilds(
            &[],
            std::slice::from_ref(&guild_id),
            |store| {
                let guild = store.shared_guilds.get_mut(&guild_id).unwrap();
                assert_eq!(
                    advance_minutes(guild, definition.time_limit as u64 + 1).len(),
                    1
                );
                guild.revision += 1;
                Ok(())
            },
        )
        .unwrap();
    config
        .commit_shared_guild_buff_definition(&identity, 2, &definition)
        .unwrap();
    let reactivated = config
        .shared_guild_for_identity(&identity)
        .unwrap()
        .unwrap();
    assert_eq!(reactivated.spare_points, bought.spare_points);
    assert_eq!(
        reactivated.gold,
        bought.gold - definition.activation_cost as u32
    );
    assert_eq!(
        reactivated.buffs[&definition.id].remaining_minutes,
        definition.time_limit
    );
    assert!(config
        .commit_shared_guild_buff(&identity, 1, i32::MAX)
        .is_err());
}
#[test]
fn guild_buff_original_minute_boundary_and_permanent_stats() {
    let (session, config, identity) = fixture();
    let definitions = mir2_game_data::crystal_guild_buff_definitions();
    let timed = definitions
        .iter()
        .find(|definition| definition.time_limit > 0)
        .unwrap();
    let mut permanent = definitions
        .iter()
        .find(|definition| !definition.stats.is_empty() && definition.id != timed.id)
        .unwrap()
        .clone();
    permanent.time_limit = 0;
    permanent.activation_cost = 123;
    provision(
        &config,
        &identity,
        timed.level_requirement.max(permanent.level_requirement),
    );
    let before = super::super::super::stats::compute_player_stats(session.app.world());
    config
        .commit_shared_guild_buff_definition(&identity, 1, &permanent)
        .unwrap();
    let permanent_guild = config
        .shared_guild_for_identity(&identity)
        .unwrap()
        .unwrap();
    assert_eq!(
        permanent_guild.gold,
        u32::MAX,
        "permanent purchase does not charge activation gold"
    );
    assert_eq!(active_stats(session.app.world()), permanent.stats);
    let after = super::super::super::stats::compute_player_stats(session.app.world());
    assert_ne!(
        before, after,
        "active source stats affect real derived stats"
    );
    config
        .commit_shared_guild_buff(&identity, 1, timed.id)
        .unwrap();
    let mut guild = config
        .shared_guild_for_identity(&identity)
        .unwrap()
        .unwrap();
    assert!(advance_minutes_with_definitions(
        &mut guild,
        timed.time_limit as u64,
        &[timed.clone(), permanent.clone()]
    )
    .is_empty());
    assert_eq!(guild.buffs[&timed.id].remaining_minutes, 0);
    assert!(guild.buffs[&timed.id].active);
    assert_eq!(
        advance_minutes_with_definitions(&mut guild, 1, &[timed.clone(), permanent.clone()]).len(),
        1
    );
    assert!(!guild.buffs[&timed.id].active);
    assert!(advance_minutes_with_definitions(
        &mut guild,
        u64::MAX,
        &[timed.clone(), permanent.clone()]
    )
    .is_empty());
    assert!(guild.buffs[&permanent.id].active);
    assert_eq!(guild.buffs[&permanent.id].remaining_minutes, 0);
}
