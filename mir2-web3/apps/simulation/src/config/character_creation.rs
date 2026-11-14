use super::{
    AccountStoreDatabaseMode, AccountStoreMutationScope, CharacterRecord, CharacterSaveRecord,
    SimulationConfig,
};
use postgres::Transaction;

const MAX_CHARACTERS: usize = 4;
const SLOTS_FULL: &str = "character creation slots full";
const NAME_TAKEN: &str = "character creation name taken";
const INDEX_FLOOR: &str = "character creation requires index at least ";

pub(crate) fn rejection_code(error: &str) -> u8 {
    match error {
        SLOTS_FULL => 4,
        NAME_TAKEN => 5,
        _ => 1,
    }
}

impl SimulationConfig {
    pub(crate) fn commit_character_creation(
        &self,
        account_id: &str,
        starter_save: &CharacterSaveRecord,
    ) -> Result<CharacterRecord, String> {
        self.refresh_account_store_account(account_id)?;
        let account_ids = [account_id.to_owned()];
        let mut minimum_index = 0;

        for _ in 0..4 {
            let result = self.commit_account_store_transaction_authorized_post_inner(
                AccountStoreMutationScope::Accounts(&account_ids),
                None,
                |store| {
                    let account = store
                        .accounts
                        .get(account_id)
                        .ok_or("character creation requires an existing account")?;
                    if account.characters.len() >= MAX_CHARACTERS {
                        return Err(SLOTS_FULL.into());
                    }
                    // A PG source cache can still have another account's deleted
                    // character. Its durable roster decides name availability.
                    if self.account_store_database_mode != AccountStoreDatabaseMode::SourceOfTruth
                        && store.accounts.values().any(|account| {
                            account.characters.iter().any(|existing| {
                                existing
                                    .name
                                    .eq_ignore_ascii_case(&starter_save.character.name)
                            })
                        })
                    {
                        return Err(NAME_TAKEN.into());
                    }

                    store.next_character_index = store.next_character_index.max(minimum_index);
                    let index = store.allocate_character_index();
                    if index == i32::MAX {
                        return Err("character index space is exhausted".into());
                    }
                    let mut save = starter_save.clone();
                    save.character.index = index;
                    crate::runtime::validate_character_save_record(&save)?;
                    let character = save.character.clone();
                    let account = store.accounts.get_mut(account_id).expect("account checked");
                    account.saves.insert(index, save);
                    account.characters.push(character.clone());
                    Ok(character)
                },
                |_| Ok(()),
                Some(account_id),
            );
            // Retry only the SQL check made before any writes to this account.
            // Reuse the prepared starter items, even if another gateway won the
            // provisional index. A failed or lost COMMIT never enters this path.
            let next_floor = result.as_ref().err().and_then(|error| {
                error
                    .strip_prefix(INDEX_FLOOR)
                    .and_then(|value| value.parse::<i32>().ok())
            });
            let Some(next_floor) = next_floor else {
                return result;
            };
            minimum_index = next_floor;
        }
        Err("character index allocation remained busy".into())
    }
}

pub(super) fn lock_postgres(transaction: &mut Transaction<'_>) -> Result<(), String> {
    // One short creation lane per realm DB. Regular saves never acquire it.
    transaction
        .query_one("SELECT pg_advisory_xact_lock(1296650802, 4)", &[])
        .map_err(|error| format!("postgres character creation lock failed: {error}"))?;
    Ok(())
}

pub(super) fn validate_postgres(
    transaction: &mut Transaction<'_>,
    account_id: &str,
    character: &CharacterRecord,
) -> Result<(), String> {
    // The caller has locked the existing account row. Count the visible roster,
    // which may differ from the projection if an old store has orphaned saves.
    let count = transaction
        .query_opt(
            "SELECT COALESCE(jsonb_array_length(raw_json->'characters'), 0) \
             FROM accounts WHERE account_id = $1",
            &[&account_id],
        )
        .map_err(|error| format!("postgres character roster check failed: {error}"))?
        .map_or(0, |row| row.get::<_, i32>(0));
    if count >= MAX_CHARACTERS as i32 {
        return Err(SLOTS_FULL.into());
    }

    // The C collation folds the same ASCII characters as the File store.
    let name_key = character.name.to_ascii_lowercase();
    let realm = transaction
        .query_one(
            r#"SELECT COALESCE(bool_or(lower(character_name COLLATE "C") = $1), false),
                      COALESCE(max(character_index), -1) FROM characters"#,
            &[&name_key],
        )
        .map_err(|error| format!("postgres character name and index check failed: {error}"))?;
    if realm.get::<_, bool>(0) {
        return Err(NAME_TAKEN.into());
    }
    let max_index: i32 = realm.get(1);
    if max_index >= character.index {
        let minimum_index = max_index
            .checked_add(1)
            .ok_or("character index space is exhausted")?;
        return Err(format!("{INDEX_FLOOR}{minimum_index}"));
    }
    Ok(())
}
