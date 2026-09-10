//! Fixed journal consumer, invoked only after the caller's original scope has
//! been validated. It derives its own guild write set from authorized sources.
use super::*;
use sha2::{Digest, Sha256};

impl AccountStoreMutationPlan {
    /// Even a no-Guild/legacy kill publishes its source receipt atomically.
    /// Losing that account-only COMMIT response must never invite a replay.
    pub(in crate::config) fn carries_guild_experience_source(&self) -> bool {
        self.accounts.values().any(|account| account.saves.values().any(|save|
            save.desired_save.as_ref().is_some_and(|save| !save.guild_experience_journal.is_empty())))
    }
    pub(in crate::config) fn fence_compensation(
        &mut self,
        receipt: &AccountStoreRepositorySave,
        committed: &AccountStore,
    ) {
        self.force_source_cas = true;
        for (id, mutation) in &mut self.guilds {
            mutation.expected_version = receipt.guild_versions.get(id).copied();
        }
        for (id, mutation) in &mut self.accounts {
            mutation.expected_version = receipt.account_versions.get(id).copied();
            for (index, save) in &mut mutation.saves {
                save.expected_version = receipt
                    .save_versions
                    .get(id)
                    .and_then(|versions| versions.get(index))
                    .copied();
            }
        }
        if let Some(global) = &mut self.global_stock {
            global.expected_version = receipt.game_shop_global_version;
            global.expected_purchases = committed.game_shop_global_purchases.clone();
        }
    }
}
pub(crate) fn event_hash(event: &GuildExperienceEvent) -> Result<String, String> {
    Ok(
        Sha256::digest(serde_json::to_vec(event).map_err(|e| e.to_string())?)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect(),
    )
}
fn valid_hash(hash: &str) -> bool {
    hash.len() == 64 && hash.bytes().all(|byte| byte.is_ascii_hexdigit())
}
fn merge_receipts(
    old: &BTreeMap<String, String>,
    new: &mut BTreeMap<String, String>,
    allow_new: bool,
) -> Result<(), String> {
    for (key, hash) in new.iter() {
        if key.is_empty() || key.len() > 4096 || !valid_hash(hash) {
            return Err("invalid guild XP receipt encoding".into());
        }
        match old.get(key) {
            Some(previous) if previous != hash => {
                return Err("guild XP receipt payload mismatch".into())
            }
            None if !allow_new => {
                return Err(
                    "guild XP applied receipt cannot be supplied without its source event".into(),
                )
            }
            _ => {}
        }
    }
    new.extend(old.clone());
    Ok(())
}
pub(in crate::config) fn settle_authorized_sources(
    original: &AccountStore,
    staged: &mut AccountStore,
    account_ids: &[String],
    permit: Option<&GuildExperienceCommitPermit>,
) -> Result<BTreeSet<String>, String> {
    let mut work = Vec::new();
    for account_id in account_ids {
        let Some(account) = staged.accounts.get(account_id) else {
            continue;
        };
        for (&index, save) in &account.saves {
            let journal = &save.guild_experience_journal;
            let previous = original
                .accounts
                .get(account_id)
                .and_then(|account| account.saves.get(&index));
            if journal.pending.is_empty() && previous.is_none() {
                if journal.next_sequence != 0
                    || !journal.event_payloads.is_empty()
                    || !journal.applied_kill_receipts.is_empty()
                    || !journal.kill_outcomes.is_empty()
                {
                    return Err("new character cannot import guild XP receipts".into());
                }
                continue;
            }
            if let Some(previous) = previous {
                if journal == &previous.guild_experience_journal {
                    continue;
                }
                if previous.guild_experience_journal.pending.len() > 0 {
                    return Err("durable source contains an unsettled guild XP journal".into());
                }
                if save.revision <= previous.revision {
                    return Err("guild XP source checkpoint revision did not advance".into());
                }
                if save.character.index != index {
                    return Err("guild XP source character identity mismatch".into());
                }
                work.push((
                    Stage5FriendIdentity {
                        account_id: account_id.clone(),
                        character_index: index,
                    },
                    previous.revision,
                    previous.guild_experience_journal.clone(),
                    journal.clone(),
                ));
            } else {
                return Err("guild XP requires an existing source checkpoint".into());
            }
        }
    }
    let mut changed = BTreeSet::new();
    for (identity, source_revision, previous, mut journal) in work {
        merge_receipts(&previous.event_payloads, &mut journal.event_payloads, false)?;
        for (key, outcome) in &journal.kill_outcomes {
            if previous.kill_outcomes.get(key) != Some(outcome) {
                return Err("guild XP outcomes are derived only by the trusted consumer".into());
            }
        }
        journal.kill_outcomes.extend(previous.kill_outcomes.clone());
        let added_kills: Vec<_> = journal
            .applied_kill_receipts
            .iter()
            .filter(|(key, _)| !previous.applied_kill_receipts.contains_key(*key))
            .map(|(key, hash)| (key.clone(), hash.clone()))
            .collect();
        for (key, hash) in &added_kills {
            if !permit.is_some_and(|permit| {
                permit.identity == identity
                    && permit.kill_key == *key
                    && permit.kill_payload_hash == *hash
            }) {
                return Err("shared kill receipt requires verified server authorization".into());
            }
        }
        merge_receipts(
            &previous.applied_kill_receipts,
            &mut journal.applied_kill_receipts,
            true,
        )?;
        let mut expected = previous.next_sequence;
        let mut seen = BTreeSet::new();
        let mut captured_outcome = None;
        for event in &journal.pending {
            if event.identity != identity || event.final_amount == 0 || !seen.insert(event.sequence)
            {
                return Err("invalid guild XP source identity or duplicate sequence".into());
            }
            let canonical = serde_json::to_string(&(
                "guild-xp-v1",
                &identity.account_id,
                identity.character_index,
                event.sequence,
            ))
            .map_err(|e| e.to_string())?;
            if event.event_id != canonical {
                return Err("guild XP event id is not derived from its source".into());
            }
            let hash = event_hash(event)?;
            if event.sequence <= previous.next_sequence {
                if previous.event_payloads.get(&event.event_id) != Some(&hash) {
                    return Err("guild XP replay receipt payload mismatch".into());
                }
                continue;
            }
            expected = expected
                .checked_add(1)
                .ok_or("guild XP sequence exhausted")?;
            if event.sequence != expected || event.source_revision != source_revision {
                return Err("guild XP source revision or sequence mismatch".into());
            }
            let captured = if let Some(capture_hash) = &event.capture_hash {
                let authorized = permit
                    .filter(|permit| permit.authorizes(event, capture_hash))
                    .ok_or("captured guild XP requires verified server authorization")?;
                if added_kills.len() != 1 || captured_outcome.is_some() {
                    return Err("captured guild XP requires exactly one new source receipt".into());
                }
                Some(authorized.selection.as_ref().unwrap())
            } else {
                None
            };
            let Some(guild) = staged.shared_guilds.get_mut(&event.guild_id) else {
                if captured.is_some() {
                    captured_outcome = Some(GuildExperienceKillOutcome::SkippedDisbanded);
                    journal.event_payloads.insert(event.event_id.clone(), hash);
                    continue;
                }
                return Err("guild XP source guild no longer exists".into());
            };
            if captured.is_none() {
                let member = guild
                    .members
                    .iter()
                    .find(|member| member.identity == identity)
                    .ok_or("guild XP source is no longer a guild member")?;
                if member.membership_epoch != event.membership_epoch {
                    return Err("guild XP membership epoch changed".into());
                }
            }
            if guild.experience_receipts.contains(&event.event_id) {
                // A guild receipt without the matching source checkpoint is not
                // a legitimate duplicate: atomic publication must save both.
                return Err("guild XP receipt exists without matching source checkpoint".into());
            }
            if let Some(selection) = captured {
                let delta = apply_captured_gain(
                    guild,
                    selection.guild_amount,
                    mir2_game_data::crystal_guild_settings(),
                )?;
                captured_outcome = Some(GuildExperienceKillOutcome::Credited {
                    guild_amount: delta.amount,
                });
            } else {
                apply_source_gain(
                    guild,
                    event.final_amount,
                    mir2_game_data::crystal_guild_settings(),
                )?;
            }
            guild.experience_receipts.insert(event.event_id.clone());
            guild
                .experience_receipt_payloads
                .insert(event.event_id.clone(), hash.clone());
            journal.event_payloads.insert(event.event_id.clone(), hash);
            changed.insert(event.guild_id.clone());
        }
        if journal.next_sequence != expected {
            return Err("guild XP checkpoint sequence does not match its events".into());
        }
        for (key, _) in added_kills {
            let permit = permit.unwrap();
            let outcome = match &permit.selection {
                None => GuildExperienceKillOutcome::LegacyNoCapture,
                Some(selection) if selection.final_amount == 0 => {
                    GuildExperienceKillOutcome::NoExperience
                }
                Some(selection) if selection.guild.is_none() => GuildExperienceKillOutcome::NoGuild,
                Some(_) => captured_outcome
                    .take()
                    .ok_or("shared kill source checkpoint omitted captured guild XP")?,
            };
            journal.kill_outcomes.insert(key, outcome);
        }
        journal.pending.clear();
        staged
            .accounts
            .get_mut(&identity.account_id)
            .unwrap()
            .saves
            .get_mut(&identity.character_index)
            .unwrap()
            .guild_experience_journal = journal;
    }
    for id in &changed {
        let guild = staged.shared_guilds.get_mut(id).unwrap();
        if original
            .shared_guilds
            .get(id)
            .is_some_and(|before| before.revision == guild.revision)
        {
            guild.revision = guild
                .revision
                .checked_add(1)
                .ok_or("guild revision exhausted")?;
        }
    }
    Ok(changed)
}
pub(in crate::config) fn validate_import(store: &AccountStore) -> Result<(), String> {
    for account in store.accounts.values() {
        for save in account.saves.values() {
            let journal = &save.guild_experience_journal;
            if !journal.pending.is_empty() {
                return Err("import cannot contain unsettled guild XP source events".into());
            }
            for (key, hash) in journal
                .event_payloads
                .iter()
                .chain(&journal.applied_kill_receipts)
            {
                if key.is_empty() || !valid_hash(hash) {
                    return Err("invalid imported guild XP receipt".into());
                }
            }
            for key in journal.kill_outcomes.keys() {
                if !journal.applied_kill_receipts.contains_key(key) {
                    return Err("imported guild XP outcome has no source receipt".into());
                }
            }
        }
    }
    Ok(())
}
