use super::*;

#[test]
#[ignore = "requires dedicated MIR2_GUILD_TEST_DATABASE_URL; independent PostgreSQL connections"]
fn guild_xp_postgres_mirror_compensation_cannot_overwrite_later_guild_or_source() {
    for change_guild in [false, true] {
        let db = Database::new();
        let mut client = db.connect();
        let config = SimulationConfig::default();
        let mut original = config.account_store.lock().unwrap().clone();
        let guild = super::schema_tests::fixture_guild(&original);
        let id = guild.id.clone();
        let identity = guild.members[0].identity.clone();
        original
            .accounts
            .get_mut(&identity.account_id)
            .unwrap()
            .saves
            .insert(
                identity.character_index,
                CharacterSaveRecord::new(config.default_character.clone()),
            );
        original.shared_guilds.insert(id.clone(), guild);
        let mut empty = original.clone();
        empty.accounts.clear();
        empty.shared_guilds.clear();
        let accounts = vec![identity.account_id.clone()];
        let guilds = vec![id.clone()];
        let scope = AccountStoreMutationScope::AccountsWithGuilds {
            account_ids: &accounts,
            guild_ids: &guilds,
        };
        let seed = build_account_store_mutation_plan(&empty, &original, scope, false);
        let versions = write_account_store_mutation_plan_to_postgres(
            &mut client,
            &seed,
            AccountStoreDatabaseMode::Mirror,
        )
        .unwrap();
        apply_account_store_mutation_source_versions(&mut original, &seed, versions);

        let mut committed = original.clone();
        let save = committed
            .accounts
            .get_mut(&identity.account_id)
            .unwrap()
            .saves
            .get_mut(&identity.character_index)
            .unwrap();
        save.gold -= 100;
        save.experience += 100;
        save.revision += 1;
        let guild = committed.shared_guilds.get_mut(&id).unwrap();
        guild.experience += 1;
        guild.revision += 1;
        let plan = build_account_store_mutation_plan(&original, &committed, scope, false);
        let receipt = write_account_store_mutation_plan_to_postgres(
            &mut client,
            &plan,
            AccountStoreDatabaseMode::Mirror,
        )
        .unwrap();
        let mut rollback = build_account_store_mutation_plan(&committed, &original, scope, false);
        rollback.fence_compensation(&receipt.clone().into(), &committed);
        apply_account_store_mutation_source_versions(&mut committed, &plan, receipt);

        // An independent writer wins after the forward COMMIT but before the
        // File primary can publish. The old compensation must fail as a whole.
        let mut latest = committed.clone();
        if change_guild {
            let guild = latest.shared_guilds.get_mut(&id).unwrap();
            guild.gold += 7;
            guild.revision += 1;
        } else {
            let save = latest
                .accounts
                .get_mut(&identity.account_id)
                .unwrap()
                .saves
                .get_mut(&identity.character_index)
                .unwrap();
            save.gold += 17;
            save.revision += 1;
        }
        let later_scope = if change_guild {
            AccountStoreMutationScope::AccountsWithGuilds {
                account_ids: &[],
                guild_ids: &guilds,
            }
        } else {
            AccountStoreMutationScope::Accounts(&accounts)
        };
        let later_plan = build_account_store_mutation_plan(&committed, &latest, later_scope, false);
        write_account_store_mutation_plan_to_postgres(
            &mut db.connect(),
            &later_plan,
            AccountStoreDatabaseMode::SourceOfTruth,
        )
        .unwrap();
        let error = write_account_store_mutation_plan_to_postgres(
            &mut client,
            &rollback,
            AccountStoreDatabaseMode::Mirror,
        )
        .unwrap_err();
        assert!(error.contains("stale"), "{error}");
        let row = client
            .query_one(
                "SELECT gold FROM character_saves WHERE account_id=$1 AND character_index=$2",
                &[&identity.account_id, &identity.character_index],
            )
            .unwrap();
        assert_eq!(
            row.get::<_, i64>(0),
            i64::from(latest.accounts[&identity.account_id].saves[&identity.character_index].gold)
        );
        let row = client
            .query_one(
                "SELECT raw_json FROM shared_guilds WHERE guild_id=$1",
                &[&id],
            )
            .unwrap();
        let observed: SharedGuildRecord = serde_json::from_value(row.get(0)).unwrap();
        assert_eq!(observed, latest.shared_guilds[&id]);
    }
}
