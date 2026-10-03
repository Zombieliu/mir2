//! Account/save admission for a File primary's conquest-carrying mirror write.
//!
//! A reopened File store may legitimately have no PostgreSQL source metadata.
//! That does not authorize overwriting a different PostgreSQL business image.
//! Admit the observed versions only after the complete locked original account
//! and every separately stored save match. The admitted account operations then
//! use Source CAS and advance their versions, including on a Mirror database.
use super::*;

#[derive(Debug, Clone)]
struct ObservedAccount {
    version: i64,
    raw_json: Value,
}

#[derive(Debug, Clone)]
struct ObservedSave {
    version: i64,
    snapshot_json: Value,
}

/// Called after the common conquest/Guild lock order has been acquired, and
/// before executing any account operations. The returned clone is for the
/// account operations with `SourceOfTruth`; do not use its force flag to change
/// the mode of the preceding conquest admission on a first Mirror reopen.
pub(super) fn admit_mirror_conquest_accounts(
    transaction: &mut Transaction<'_>,
    plan: &AccountStoreMutationPlan,
) -> Result<AccountStoreMutationPlan, String> {
    if plan.conquests.is_empty() {
        return Err("conquest account admission requires an explicit conquest mutation".into());
    }
    let mut admitted = plan.clone();
    for (account_id, mutation) in &mut admitted.accounts {
        let account = transaction
            .query_opt(
                "SELECT store_version, raw_json FROM accounts \
                 WHERE account_id=$1 FOR UPDATE",
                &[&account_id],
            )
            .map_err(|error| {
                format!("postgres conquest account lock failed for {account_id}: {error}")
            })?
            .map(|row| ObservedAccount {
                version: row.get("store_version"),
                raw_json: row.get("raw_json"),
            });
        let saves = transaction
            .query(
                "SELECT character_index, save_version, snapshot_json FROM character_saves \
                 WHERE account_id=$1 ORDER BY character_index FOR UPDATE",
                &[&account_id],
            )
            .map_err(|error| {
                format!("postgres conquest save locks failed for {account_id}: {error}")
            })?
            .into_iter()
            .map(|row| {
                (
                    row.get("character_index"),
                    ObservedSave {
                        version: row.get("save_version"),
                        snapshot_json: row.get("snapshot_json"),
                    },
                )
            })
            .collect::<BTreeMap<i32, ObservedSave>>();
        admit_account_mutation(account_id, mutation, account.as_ref(), &saves)?;
    }
    admitted.force_source_cas = true;
    Ok(admitted)
}

fn admit_account_mutation(
    account_id: &str,
    mutation: &mut AccountStoreAccountMutation,
    observed_account: Option<&ObservedAccount>,
    observed_saves: &BTreeMap<i32, ObservedSave>,
) -> Result<(), String> {
    validate_mutation_images(account_id, mutation)?;
    if mutation
        .expected_version
        .is_some_and(|version| version <= 0)
        || mutation
            .saves
            .values()
            .any(|save| save.expected_version.is_some_and(|version| version <= 0))
        || observed_account.is_some_and(|account| account.version <= 0)
        || observed_saves.values().any(|save| save.version <= 0)
    {
        return Err(format!(
            "invalid conquest account source version for {account_id}"
        ));
    }
    if observed_account.is_none() && !observed_saves.is_empty() {
        return Err(format!(
            "orphaned conquest character saves for {account_id}"
        ));
    }

    let observed_versions = observed_saves
        .iter()
        .map(|(&index, save)| (index, save.version))
        .collect::<BTreeMap<_, _>>();
    let has_source_metadata = mutation.expected_version.is_some()
        || mutation
            .saves
            .values()
            .any(|save| save.expected_version.is_some());
    if has_source_metadata {
        // Never turn stale or incomplete source metadata into a fresh receipt.
        // Even matching business data cannot repair an observed stale version.
        validate_account_store_account_mutation(
            account_id,
            mutation,
            observed_account.map(|account| account.version),
            &observed_versions,
            AccountStoreDatabaseMode::SourceOfTruth,
        )?;
    }

    if let Some(observed) = observed_account {
        let original = mutation.original_account.as_ref().ok_or_else(|| {
            format!("stale postgres conquest account {account_id}: unobserved existing account")
        })?;
        if to_json(original)? != observed.raw_json {
            return Err(format!(
                "stale postgres conquest account {account_id}: original account image differs"
            ));
        }
        let original_indices = original.saves.keys().copied().collect::<BTreeSet<_>>();
        let observed_indices = observed_saves.keys().copied().collect::<BTreeSet<_>>();
        if original_indices != observed_indices {
            return Err(format!(
                "stale postgres conquest account {account_id}: original save set differs"
            ));
        }
        for (&index, save) in &original.saves {
            if to_json(save)? != observed_saves[&index].snapshot_json {
                return Err(format!(
                    "stale postgres conquest character save {account_id}/{index}: original save image differs"
                ));
            }
        }
    }

    if !has_source_metadata {
        // All business-image checks above succeeded before changing any source
        // metadata. A genuinely absent database row permits initial mirroring;
        // its negative-presence expectation remains None for Source insertion.
        mutation.expected_version = observed_account.map(|account| account.version);
        for (&index, save) in &mut mutation.saves {
            save.expected_version = observed_versions.get(&index).copied();
        }
    }
    validate_account_store_account_mutation(
        account_id,
        mutation,
        observed_account.map(|account| account.version),
        &observed_versions,
        AccountStoreDatabaseMode::SourceOfTruth,
    )
}

fn validate_mutation_images(
    account_id: &str,
    mutation: &AccountStoreAccountMutation,
) -> Result<(), String> {
    // The ordinary writer writes desired_account.saves, while CAS iterates the
    // mutation's save map. Keep those two views complete and identical so a
    // malformed internal plan cannot insert a save outside its fenced set.
    for account in [
        mutation.original_account.as_ref(),
        mutation.desired_account.as_ref(),
    ]
    .into_iter()
    .flatten()
    {
        for (&index, save) in &account.saves {
            if index != save.character.index || !mutation.saves.contains_key(&index) {
                return Err(format!(
                    "incomplete conquest character-save mutation for {account_id}/{index}"
                ));
            }
        }
    }
    for (&index, save) in &mutation.saves {
        let desired = mutation
            .desired_account
            .as_ref()
            .and_then(|account| account.saves.get(&index));
        let desired_json = desired.map(to_json).transpose()?;
        let mutation_json = save.desired_save.as_ref().map(to_json).transpose()?;
        if desired_json != mutation_json {
            return Err(format!(
                "inconsistent conquest character-save mutation for {account_id}/{index}"
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn account() -> AccountRecord {
        let mut account = AccountRecord::new(CharacterRecord {
            index: 1,
            name: "Owner".into(),
            level: 22,
            class: MirClass::Warrior,
            gender: MirGender::Male,
        });
        let second = CharacterRecord {
            index: 2,
            name: "Alt".into(),
            level: 15,
            class: MirClass::Wizard,
            gender: MirGender::Female,
        };
        account.characters.push(second.clone());
        account.saves.insert(2, CharacterSaveRecord::new(second));
        account.saves.get_mut(&1).unwrap().gold = 50000;
        account
    }

    fn mutation(
        original: Option<AccountRecord>,
        desired: Option<AccountRecord>,
    ) -> AccountStoreAccountMutation {
        let mut indices = BTreeSet::new();
        for account in [original.as_ref(), desired.as_ref()].into_iter().flatten() {
            indices.extend(account.saves.keys().copied());
        }
        let saves = indices
            .into_iter()
            .map(|index| {
                (
                    index,
                    AccountStoreSaveMutation {
                        expected_version: None,
                        desired_save: desired
                            .as_ref()
                            .and_then(|account| account.saves.get(&index))
                            .cloned(),
                    },
                )
            })
            .collect();
        AccountStoreAccountMutation {
            expected_version: None,
            original_account: original,
            desired_account: desired,
            saves,
        }
    }

    fn observed(account: &AccountRecord) -> (ObservedAccount, BTreeMap<i32, ObservedSave>) {
        (
            ObservedAccount {
                version: 7,
                raw_json: to_json(account).unwrap(),
            },
            account
                .saves
                .iter()
                .map(|(&index, save)| {
                    (
                        index,
                        ObservedSave {
                            version: i64::from(index) + 9,
                            snapshot_json: to_json(save).unwrap(),
                        },
                    )
                })
                .collect(),
        )
    }

    fn set_known_versions(mutation: &mut AccountStoreAccountMutation) {
        mutation.expected_version = Some(7);
        for (&index, save) in &mut mutation.saves {
            save.expected_version = Some(i64::from(index) + 9);
        }
    }

    #[test]
    fn conquest_reopened_mirror_admits_only_complete_original_images() {
        let original = account();
        let mut desired = original.clone();
        desired.saves.get_mut(&1).unwrap().gold -= 100;
        desired.saves.get_mut(&1).unwrap().revision += 1;
        let (row, saves) = observed(&original);
        let mut write = mutation(Some(original), Some(desired));
        admit_account_mutation("owner", &mut write, Some(&row), &saves).unwrap();
        assert_eq!(write.expected_version, Some(7));
        assert_eq!(write.saves[&1].expected_version, Some(10));
        assert_eq!(write.saves[&2].expected_version, Some(11));
        assert_eq!(write.desired_account.unwrap().saves[&1].gold, 49900);
    }

    #[test]
    fn conquest_initial_mirror_keeps_absent_row_expectations() {
        let original = account();
        for before in [None, Some(original.clone())] {
            let mut write = mutation(before, Some(original.clone()));
            admit_account_mutation("owner", &mut write, None, &BTreeMap::new()).unwrap();
            assert!(write.expected_version.is_none());
            assert!(write
                .saves
                .values()
                .all(|save| save.expected_version.is_none()));
        }
    }

    #[test]
    fn conquest_mirror_cannot_claim_unobserved_existing_account() {
        let account = account();
        let (row, saves) = observed(&account);
        let mut write = mutation(None, Some(account));
        let error = admit_account_mutation("owner", &mut write, Some(&row), &saves).unwrap_err();
        assert!(error.contains("unobserved existing account"));
        assert!(write.expected_version.is_none());
    }

    #[test]
    fn conquest_mirror_rejects_changed_account_even_with_same_version() {
        for known in [false, true] {
            let original = account();
            let (mut row, saves) = observed(&original);
            let mut write = mutation(Some(original.clone()), Some(original));
            if known {
                set_known_versions(&mut write);
            }
            row.raw_json["gm_level"] = serde_json::json!(1);
            let error =
                admit_account_mutation("owner", &mut write, Some(&row), &saves).unwrap_err();
            assert!(error.contains("account image differs"));
            assert_eq!(write.expected_version, known.then_some(7));
        }
    }

    #[test]
    fn conquest_mirror_checks_separate_save_images_even_if_account_json_matches() {
        for known in [false, true] {
            let original = account();
            let (row, mut saves) = observed(&original);
            let mut write = mutation(Some(original.clone()), Some(original));
            if known {
                set_known_versions(&mut write);
            }
            saves.get_mut(&2).unwrap().snapshot_json["gold"] = serde_json::json!(90000);
            let error =
                admit_account_mutation("owner", &mut write, Some(&row), &saves).unwrap_err();
            assert!(error.contains("save image differs"));
            assert_eq!(write.saves[&1].expected_version, known.then_some(10));
        }
    }

    #[test]
    fn conquest_mirror_rejects_missing_or_extra_separate_save() {
        let original = account();
        let (row, saves) = observed(&original);
        let mut missing = saves.clone();
        missing.remove(&2);
        let mut extra = saves.clone();
        extra.insert(3, saves[&1].clone());
        for rows in [missing, extra] {
            let mut write = mutation(Some(original.clone()), Some(original.clone()));
            let error = admit_account_mutation("owner", &mut write, Some(&row), &rows).unwrap_err();
            assert!(error.contains("save set differs"));
            assert!(write.expected_version.is_none());
        }
    }

    #[test]
    fn conquest_known_matching_versions_are_preserved_and_stale_versions_rejected() {
        let original = account();
        let (row, saves) = observed(&original);
        let mut write = mutation(Some(original.clone()), Some(original));
        set_known_versions(&mut write);
        admit_account_mutation("owner", &mut write, Some(&row), &saves).unwrap();
        assert_eq!(write.expected_version, Some(7));
        let mut changed = row.clone();
        changed.version += 1;
        assert!(admit_account_mutation("owner", &mut write, Some(&changed), &saves).is_err());
        assert_eq!(write.expected_version, Some(7));
        let mut changed_saves = saves.clone();
        changed_saves.get_mut(&2).unwrap().version += 1;
        assert!(admit_account_mutation("owner", &mut write, Some(&row), &changed_saves).is_err());
        assert_eq!(write.saves[&2].expected_version, Some(11));
    }

    #[test]
    fn conquest_partial_metadata_is_never_readmitted() {
        let original = account();
        let (row, saves) = observed(&original);
        for account_metadata in [false, true] {
            let mut write = mutation(Some(original.clone()), Some(original.clone()));
            if account_metadata {
                write.expected_version = Some(7);
            } else {
                write.saves.get_mut(&1).unwrap().expected_version = Some(10);
            }
            assert!(admit_account_mutation("owner", &mut write, Some(&row), &saves).is_err());
            assert_eq!(write.expected_version, account_metadata.then_some(7));
            assert!(write.saves[&2].expected_version.is_none());
        }
    }

    #[test]
    fn conquest_mirror_can_fence_deletion_and_new_save_together() {
        let original = account();
        let (row, saves) = observed(&original);
        let mut desired = original.clone();
        desired.saves.remove(&2);
        desired.characters.retain(|character| character.index != 2);
        let mut character = desired.characters[0].clone();
        character.index = 3;
        character.name = "New".into();
        desired.characters.push(character.clone());
        desired.saves.insert(3, CharacterSaveRecord::new(character));
        let mut write = mutation(Some(original), Some(desired));
        admit_account_mutation("owner", &mut write, Some(&row), &saves).unwrap();
        assert!(write.saves[&2].desired_save.is_none());
        assert_eq!(write.saves[&2].expected_version, Some(11));
        assert!(write.saves[&3].expected_version.is_none());
    }

    #[test]
    fn conquest_mirror_account_tombstone_still_fences_every_original_save() {
        let original = account();
        let (row, saves) = observed(&original);
        let mut write = mutation(Some(original), None);
        admit_account_mutation("owner", &mut write, Some(&row), &saves).unwrap();
        assert_eq!(write.expected_version, Some(7));
        assert_eq!(write.saves[&2].expected_version, Some(11));
        assert!(write.saves.values().all(|save| save.desired_save.is_none()));
    }

    #[test]
    fn conquest_mutation_cannot_omit_or_invent_a_desired_save_image() {
        let original = account();
        let (row, saves) = observed(&original);
        let mut missing = mutation(Some(original.clone()), Some(original.clone()));
        missing.saves.remove(&2);
        assert!(admit_account_mutation("owner", &mut missing, Some(&row), &saves).is_err());
        let mut wrong = mutation(Some(original.clone()), Some(original));
        wrong
            .saves
            .get_mut(&2)
            .unwrap()
            .desired_save
            .as_mut()
            .unwrap()
            .gold += 1;
        assert!(admit_account_mutation("owner", &mut wrong, Some(&row), &saves).is_err());
    }

    #[test]
    fn conquest_account_and_save_versions_must_be_positive() {
        let original = account();
        let (row, saves) = observed(&original);
        for bad in [0, -1] {
            let mut write = mutation(Some(original.clone()), Some(original.clone()));
            let mut bad_row = row.clone();
            bad_row.version = bad;
            assert!(admit_account_mutation("owner", &mut write, Some(&bad_row), &saves).is_err());
            let mut bad_saves = saves.clone();
            bad_saves.get_mut(&1).unwrap().version = bad;
            assert!(admit_account_mutation("owner", &mut write, Some(&row), &bad_saves).is_err());
            write.expected_version = Some(bad);
            assert!(admit_account_mutation("owner", &mut write, Some(&row), &saves).is_err());
            write.expected_version = None;
            write.saves.get_mut(&2).unwrap().expected_version = Some(bad);
            assert!(admit_account_mutation("owner", &mut write, Some(&row), &saves).is_err());
        }
    }

    #[test]
    fn conquest_save_rows_without_parent_account_are_rejected() {
        let original = account();
        let (_, saves) = observed(&original);
        let mut write = mutation(Some(original.clone()), Some(original));
        let error = admit_account_mutation("owner", &mut write, None, &saves).unwrap_err();
        assert!(error.contains("orphaned"));
    }

    struct Database {
        url: String,
        schema: String,
    }

    impl Database {
        fn new() -> Self {
            let url = std::env::var("MIR2_CONQUEST_TEST_DATABASE_URL")
                .or_else(|_| std::env::var("MIR2_GUILD_TEST_DATABASE_URL"))
                .expect(
                    "an explicitly configured dedicated conquest/Guild test database is required",
                );
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let schema = format!("conquest_accounts_{}_{nonce:x}", std::process::id());
            let db = Self { url, schema };
            let mut client = Client::connect(&db.url, NoTls).unwrap();
            client
                .batch_execute(&format!("CREATE SCHEMA {}", db.schema))
                .unwrap();
            crate::db_projection::apply_migrations(&mut db.connect()).unwrap();
            db
        }

        fn connect(&self) -> Client {
            let mut client = Client::connect(&self.url, NoTls).unwrap();
            client
                .batch_execute(&format!(
                    "SET search_path TO {}; SET lock_timeout='3s'; SET statement_timeout='10s'",
                    self.schema
                ))
                .unwrap();
            client
        }
    }

    impl Drop for Database {
        fn drop(&mut self) {
            assert!(self.schema.starts_with("conquest_accounts_"));
            assert!(self
                .schema
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_'));
            if let Ok(mut client) = Client::connect(&self.url, NoTls) {
                let _ = client.batch_execute(&format!("DROP SCHEMA {} CASCADE", self.schema));
            }
        }
    }

    #[test]
    #[ignore = "requires dedicated MIR2_CONQUEST_TEST_DATABASE_URL or MIR2_GUILD_TEST_DATABASE_URL; isolated schema and independent writers"]
    fn conquest_mirror_postgres_accounts_use_source_cas_and_reject_later_save_images() {
        for later_mode in [
            AccountStoreDatabaseMode::SourceOfTruth,
            AccountStoreDatabaseMode::Mirror,
        ] {
            let db = Database::new();
            let mut client = db.connect();
            let config = SimulationConfig::default();
            let mut original = config.account_store.lock().unwrap().clone();
            original.accounts.insert("owner".into(), account());
            original.accounts.remove("demo");
            original
                .shared_conquests
                .insert(1, SharedConquestRecord::new(1));
            let mut empty = original.clone();
            empty.accounts.clear();
            empty.shared_conquests.clear();
            let account_ids = ["owner".to_string()];
            let scope = AccountStoreMutationScope::AccountsWithGuildsAndConquests {
                account_ids: &account_ids,
                guild_ids: &[],
                conquest_ids: &[1],
            };
            let seed = build_account_store_mutation_plan(&empty, &original, scope, false);
            write_account_store_mutation_plan_to_postgres(
                &mut client,
                &seed,
                AccountStoreDatabaseMode::SourceOfTruth,
            )
            .unwrap();

            // Reopened File metadata is absent, but its complete images match.
            let mut committed = original.clone();
            committed
                .accounts
                .get_mut("owner")
                .unwrap()
                .saves
                .get_mut(&1)
                .unwrap()
                .gold -= 100;
            committed
                .accounts
                .get_mut("owner")
                .unwrap()
                .saves
                .get_mut(&1)
                .unwrap()
                .revision += 1;
            committed.shared_conquests.get_mut(&1).unwrap().revision += 1;
            let forward = build_account_store_mutation_plan(&original, &committed, scope, false);
            let receipt = write_account_store_mutation_plan_to_postgres(
                &mut client,
                &forward,
                AccountStoreDatabaseMode::Mirror,
            )
            .unwrap();
            assert_eq!(receipt.accounts["owner"], 2);
            assert_eq!(receipt.saves["owner"][&1], 2);
            assert_eq!(receipt.saves["owner"][&2], 2);
            assert_eq!(receipt.conquests[&1], 2);
            apply_account_store_mutation_source_versions(&mut committed, &forward, receipt);

            // A second connection commits a newer wallet. In legacy Mirror
            // mode its versions remain equal: the separate business guard is
            // still necessary, beyond known-version or reopened-version CAS.
            let mut latest = committed.clone();
            let save = latest
                .accounts
                .get_mut("owner")
                .unwrap()
                .saves
                .get_mut(&1)
                .unwrap();
            save.gold += 17;
            save.revision += 1;
            let later = build_account_store_mutation_plan(
                &committed,
                &latest,
                AccountStoreMutationScope::Accounts(&account_ids),
                false,
            );
            write_account_store_mutation_plan_to_postgres(&mut db.connect(), &later, later_mode)
                .unwrap();

            for reopened in [false, true] {
                let mut stale = committed.clone();
                if reopened {
                    stale.source_account_versions.clear();
                    stale.source_save_versions.clear();
                }
                let mut stale_desired = stale.clone();
                stale_desired.shared_conquests.get_mut(&1).unwrap().revision += 1;
                let save = stale_desired
                    .accounts
                    .get_mut("owner")
                    .unwrap()
                    .saves
                    .get_mut(&1)
                    .unwrap();
                save.gold -= 100;
                save.revision += 1;
                let stale_plan =
                    build_account_store_mutation_plan(&stale, &stale_desired, scope, false);
                let error = write_account_store_mutation_plan_to_postgres(
                    &mut client,
                    &stale_plan,
                    AccountStoreDatabaseMode::Mirror,
                )
                .unwrap_err();
                assert!(
                    error.contains("stale"),
                    "a stale account must reject the whole castle transaction"
                );
                let row = client.query_one("SELECT gold, save_version FROM character_saves WHERE account_id='owner' AND character_index=1", &[]).unwrap();
                assert_eq!(row.get::<_, i64>("gold"), 49917);
                assert_eq!(
                    row.get::<_, i64>("save_version"),
                    if later_mode == AccountStoreDatabaseMode::SourceOfTruth {
                        3
                    } else {
                        2
                    }
                );
                let row = client.query_one("SELECT raw_json, store_version FROM shared_conquests WHERE conquest_index=1", &[]).unwrap();
                assert_eq!(row.get::<_, i64>("store_version"), 2);
                assert_eq!(
                    row.get::<_, Value>("raw_json"),
                    to_json(&committed.shared_conquests[&1]).unwrap(),
                    "rejected account admission must roll back the preceding conquest write"
                );
            }
        }
    }
}
