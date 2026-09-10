use super::*;

fn guild_fixture() -> (
    crate::SimulationSession,
    SimulationConfig,
    Stage5FriendIdentity,
) {
    let mut session = super::tests::session();
    session.grant_shared_guild_creation_from_npc();
    session.submit_shared_guild_name("BankGuild").unwrap();
    let config = session
        .app
        .world()
        .resource::<RuntimeConfigResource>()
        .config
        .clone();
    let identity = world_identity(session.app.world()).unwrap();
    (session, config, identity)
}

#[test]
fn guild_bank_gold_atomic_deposit_withdraw_stale_and_failure() {
    let (session, config, identity) = guild_fixture();
    let before = session.active_character_checkpoint().unwrap();
    let deposit = config
        .commit_shared_guild_gold(&identity, &before, 0, 400)
        .unwrap();
    assert_eq!(deposit.gold, before.gold - 400);
    assert_eq!(
        config
            .shared_guild_for_identity(&identity)
            .unwrap()
            .unwrap()
            .gold,
        400
    );
    assert!(config
        .commit_shared_guild_gold(&identity, &before, 0, 400)
        .is_err());
    config
        .inject_account_store_transaction_fault(crate::AccountStoreTransactionFault::BeforePersist);
    assert!(config
        .commit_shared_guild_gold(&identity, &deposit, 1, 100)
        .is_err());
    assert_eq!(
        config
            .shared_guild_for_identity(&identity)
            .unwrap()
            .unwrap()
            .gold,
        400
    );
    assert_eq!(
        config.account_store.lock().unwrap().accounts[&identity.account_id].saves
            [&identity.character_index]
            .gold,
        deposit.gold
    );
    let withdrawn = config
        .commit_shared_guild_gold(&identity, &deposit, 1, 100)
        .unwrap();
    assert_eq!(withdrawn.gold, before.gold - 300);
    assert_eq!(
        config
            .shared_guild_for_identity(&identity)
            .unwrap()
            .unwrap()
            .gold,
        300
    );
    assert!(config
        .commit_shared_guild_gold(&identity, &withdrawn, 1, 301)
        .is_err());
    assert!(config
        .commit_shared_guild_gold(&identity, &withdrawn, 0, u32::MAX)
        .is_err());
    assert!(config
        .commit_shared_guild_gold(&identity, &withdrawn, 2, 1)
        .is_err());
}

#[test]
fn guild_bank_gold_checks_current_rank_and_live_wallet_limits() {
    let (session, config, identity) = guild_fixture();
    let checkpoint = session.active_character_checkpoint().unwrap();
    let deposit = config
        .commit_shared_guild_gold(&identity, &checkpoint, 0, 20)
        .unwrap();
    let mut capped = deposit.clone();
    capped.gold = u32::MAX;
    assert!(config
        .commit_shared_guild_gold(&identity, &capped, 1, 1)
        .is_err());
    let mut poor = deposit.clone();
    poor.gold = 0;
    assert!(config
        .commit_shared_guild_gold(&identity, &poor, 0, 1)
        .is_err());
    let guild_id = config
        .shared_guild_for_identity(&identity)
        .unwrap()
        .unwrap()
        .id;
    // Add a real second member so the authority retains a valid leader.
    config
        .commit_account_store_transaction_with_guilds(
            &[identity.account_id.clone()],
            &[guild_id.clone()],
            |store| {
                let account = store.accounts.get_mut(&identity.account_id).unwrap();
                let mut character = account.characters[0].clone();
                character.index = 17;
                character.name = "SecondLeader".into();
                account.characters.push(character);
                let guild = store.shared_guilds.get_mut(&guild_id).unwrap();
                guild.ranks.push(SharedGuildRank {
                    index: 1,
                    name: "Member".into(),
                    options: 255,
                });
                guild.members[0].rank_index = 1;
                guild.members.push(SharedGuildMember { membership_epoch: 0,
                    identity: Stage5FriendIdentity {
                        account_id: identity.account_id.clone(),
                        character_index: 17,
                    },
                    name: "SecondLeader".into(),
                    rank_index: 0,
                });
                guild.revision += 1;
                Ok(())
            },
        )
        .unwrap();
    assert!(
        config
            .commit_shared_guild_gold(&identity, &deposit, 1, 1)
            .is_err(),
        "all permission bits do not replace rank zero"
    );
    assert!(
        config
            .commit_shared_guild_gold(&identity, &deposit, 0, 1)
            .is_ok(),
        "all real members may donate"
    );
}
