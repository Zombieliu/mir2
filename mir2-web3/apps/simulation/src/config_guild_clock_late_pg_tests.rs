use super::*;

#[test]
#[ignore = "requires dedicated MIR2_GUILD_TEST_DATABASE_URL; reused owner generation fence"]
fn guild_clock_postgres_reused_owner_generation_rejects_retained_token() {
    let db = Database::new();
    let mut client = db.connect();
    let (first, _) = acquire(&mut client, A).unwrap();
    let retained = first.lease().unwrap();
    client.execute("UPDATE shared_guild_clock SET minute_anchor_ms=(extract(epoch from clock_timestamp())*1000)::bigint-60001,lease_expires_ms=(extract(epoch from clock_timestamp())*1000)::bigint-1 WHERE singleton",&[]).unwrap();
    let (replacement, version) = acquire(&mut client, A).unwrap();
    assert!(replacement.generation > retained.generation);
    assert!(transactions::commit_postgres_tick_checked_with_lease(
        &mut client,
        A,
        TTL,
        None,
        Some(&retained)
    )
    .unwrap_err()
    .to_string()
    .contains("stale retained"));
    assert_eq!(load_postgres(&mut client).unwrap().version, version);
    let accepted = transactions::commit_postgres_tick_checked_with_lease(
        &mut client,
        A,
        TTL,
        None,
        replacement.lease().as_ref(),
    )
    .unwrap()
    .unwrap();
    assert_eq!(accepted.admitted_minutes, 0);
}

#[test]
#[ignore = "requires dedicated MIR2_GUILD_TEST_DATABASE_URL; two independent Config coordinators"]
fn guild_clock_postgres_late_owner_and_standby_observation_preserve_fencing() {
    let db = Database::new();
    let mut client = db.connect();
    let url = format!(
        "{}{}options=-csearch_path%3D{}",
        db.url,
        if db.url.contains('?') { "&" } else { "?" },
        db.schema
    );
    let directory = std::env::temp_dir().join(format!(
        "mir2-clock-late-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&directory).unwrap();
    let backup_path = directory.join("backup.json");
    let first = SimulationConfig::default()
        .with_postgres_account_store(url.clone())
        .unwrap();
    let mut backup = AccountStore::new(first.default_character.clone());
    let mut guild = super::schema_tests::fixture_guild(&backup);
    let buff = mir2_game_data::crystal_guild_buff_definitions()
        .iter()
        .find(|buff| buff.time_limit > 0)
        .unwrap();
    guild.buffs.insert(
        buff.id,
        SharedGuildBuff {
            id: buff.id,
            active: true,
            remaining_minutes: 2,
        },
    );
    let id = guild.id.clone();
    backup.shared_guilds.insert(id.clone(), guild);
    backup.save_to_path(&backup_path).unwrap();
    first
        .restore_account_store_from_backup(&backup_path)
        .unwrap();
    let second = SimulationConfig::default()
        .with_postgres_account_store(url)
        .unwrap();
    assert!(first.tick_shared_guild_clock().unwrap().is_empty());
    assert!(second.tick_shared_guild_clock().unwrap().is_empty());
    let first_clock = load_postgres(&mut client).unwrap();
    client.execute("UPDATE shared_guild_clock SET minute_anchor_ms=(extract(epoch from clock_timestamp())*1000)::bigint-60001,lease_expires_ms=(extract(epoch from clock_timestamp())*1000)::bigint-1 WHERE singleton",&[]).unwrap();
    assert_eq!(first.tick_shared_guild_clock().unwrap(), vec![id.clone()]);
    let after = load_postgres(&mut client).unwrap();
    assert_eq!(
        after.record.generation, first_clock.record.generation,
        "a delayed live owner did not restart"
    );
    second.refresh_shared_guild_authority().unwrap();
    let observed_generation = second.shared_guild_clock_generation();
    assert_eq!(second.tick_shared_guild_clock().unwrap(), vec![id.clone()]);
    assert_eq!(
        second.shared_guild_clock_generation(),
        observed_generation + 1
    );
    assert_eq!(
        second.account_store.lock().unwrap().shared_guilds[&id].buffs[&buff.id].remaining_minutes,
        1
    );
    assert_eq!(
        load_postgres(&mut client).unwrap().version,
        after.version,
        "standby observation must not renew/write the lease"
    );
    assert!(second.tick_shared_guild_clock().unwrap().is_empty());
    assert_eq!(
        second.shared_guild_clock_generation(),
        observed_generation + 1
    );

    // A different owner really takes over an expired lease: no offline charge,
    // a higher generation, and the old coordinator cannot advance it.
    client.execute("UPDATE shared_guild_clock SET minute_anchor_ms=(extract(epoch from clock_timestamp())*1000)::bigint-60001,lease_expires_ms=(extract(epoch from clock_timestamp())*1000)::bigint-1 WHERE singleton",&[]).unwrap();
    assert!(second.tick_shared_guild_clock().unwrap().is_empty());
    let takeover = load_postgres(&mut client).unwrap();
    assert!(takeover.record.generation > after.record.generation);
    assert!(first.tick_shared_guild_clock().unwrap().is_empty());
    assert_eq!(
        load_postgres(&mut client).unwrap().version,
        takeover.version
    );
    assert_eq!(
        first.account_store.lock().unwrap().shared_guilds[&id].buffs[&buff.id].remaining_minutes,
        1
    );
    drop(first);
    drop(second);
    fs::remove_dir_all(directory).unwrap();
}
