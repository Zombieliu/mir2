//! Atomic, explicitly guild-only clock transactions. No account, wallet or
//! game-shop rows are written here. The clock lock is always acquired first.
use super::*;

#[derive(Debug, Clone)]
pub(in crate::config) struct GuildClockCommitReceipt {
    pub before: GuildClockRecord,
    pub after: GuildClockRecord,
    pub clock_version: i64,
    pub before_guilds: BTreeMap<String, SharedGuildRecord>,
    pub after_guilds: BTreeMap<String, SharedGuildRecord>,
    pub guild_versions: BTreeMap<String, i64>,
    pub admitted_minutes: u64,
}
/// `None` means another live owner holds the lease, with no mutation. This is
/// expected standby behavior, not an error and never a reason to reset anchors.
pub(in crate::config) fn commit_postgres_tick(
    client: &mut Client,
    owner: &str,
    ttl_ms: u64,
) -> Result<Option<GuildClockCommitReceipt>, GuildClockTransactionError> {
    commit_postgres_tick_checked(client, owner, ttl_ms, None)
}
pub(in crate::config) fn commit_postgres_tick_checked(
    client: &mut Client,
    owner: &str,
    ttl_ms: u64,
    mirror: Option<&AccountStore>,
) -> Result<Option<GuildClockCommitReceipt>, GuildClockTransactionError> {
    commit_postgres_tick_checked_with_lease(client, owner, ttl_ms, mirror, None)
}
pub(in crate::config) fn commit_postgres_tick_checked_with_lease(
    client: &mut Client,
    owner: &str,
    ttl_ms: u64,
    mirror: Option<&AccountStore>,
    retained: Option<&GuildClockLease>,
) -> Result<Option<GuildClockCommitReceipt>, GuildClockTransactionError> {
    let mut transaction = client
        .transaction()
        .map_err(|e| format!("guild clock transaction begin failed: {e}"))?;
    let locked = LockedPostgresGuildClock::lock(&mut transaction)?;
    if let Some(mirror) = mirror {
        if mirror.guild_clock.clone().unwrap_or_default() != locked.record {
            return Err("guild clock File/PostgreSQL authority mismatch".into());
        }
        let mut observed = BTreeMap::new();
        for row in transaction
            .query(
                "SELECT guild_id,raw_json FROM shared_guilds ORDER BY guild_id",
                &[],
            )
            .map_err(|e| e.to_string())?
        {
            let guild: SharedGuildRecord =
                serde_json::from_value(row.get(1)).map_err(|e| e.to_string())?;
            observed.insert(row.get::<_, String>(0), guild);
        }
        if observed != mirror.shared_guilds {
            return Err("guild clock File/PostgreSQL guild authority mismatch".into());
        }
    }
    if locked
        .record
        .owner_token
        .as_deref()
        .is_some_and(|current| current != owner)
        && locked.record.lease_expires_ms > locked.now_ms
    {
        return Ok(None);
    }
    if let Some(expected) = retained {
        if expected.owner_token != owner
            || (locked.record.owner_token.as_deref() == Some(owner)
                && expected.generation != locked.record.generation)
        {
            return Err("stale retained guild clock generation".into());
        }
    }
    let retained = retained.filter(|lease| {
        lease.owner_token == owner && locked.record.lease().as_ref() == Some(*lease)
    });
    let (after, clock_version, admitted_minutes) = if let Some(lease) = retained {
        let (advance, version) = locked.advance_retained(&mut transaction, lease, ttl_ms)?;
        (advance.next, version, advance.admitted_minutes)
    } else if locked.record.owner_token.as_deref() == Some(owner)
        && locked.record.lease_expires_ms > locked.now_ms
    {
        let (advance, version) = locked.advance(
            &mut transaction,
            &locked.record.lease().ok_or("guild clock owner missing")?,
            ttl_ms,
        )?;
        (advance.next, version, advance.admitted_minutes)
    } else {
        let (record, version) = locked.acquire(&mut transaction, owner, ttl_ms)?;
        (record, version, 0)
    };
    let mut before_guilds = BTreeMap::new();
    let mut after_guilds = BTreeMap::new();
    let mut mutations = BTreeMap::new();
    if admitted_minutes > 0 {
        let mut view = AccountStore::new(CharacterRecord {
            index: 0,
            name: String::new(),
            level: 1,
            class: mir2_protocol::MirClass::Warrior,
            gender: mir2_protocol::MirGender::Male,
        });
        view.accounts.clear();
        for row in transaction
            .query(
                "SELECT guild_id,raw_json,store_version FROM shared_guilds ORDER BY guild_id",
                &[],
            )
            .map_err(|e| format!("guild clock guild read failed: {e}"))?
        {
            let id: String = row.get("guild_id");
            let guild: SharedGuildRecord = serde_json::from_value(row.get("raw_json"))
                .map_err(|e| format!("guild clock invalid guild {id}: {e}"))?;
            view.shared_guilds.insert(id.clone(), guild);
            view.source_guild_versions
                .insert(id, row.get("store_version"));
        }
        // PostgreSQL membership FKs enforce real identities; this complete guild
        // view additionally validates raw payloads and cross-guild uniqueness.
        shared_guild_store::validate_guild_state(&view)?;
        let mut indexed_members = BTreeMap::<String, BTreeSet<(String, i32)>>::new();
        for row in transaction.query("SELECT guild_id,account_id,character_index FROM shared_guild_members ORDER BY guild_id,account_id,character_index",&[]).map_err(|e|e.to_string())? {
            indexed_members.entry(row.get(0)).or_default().insert((row.get(1),row.get(2)));
        }
        for (id, guild) in &view.shared_guilds {
            let expected = guild
                .members
                .iter()
                .map(|member| {
                    (
                        member.identity.account_id.clone(),
                        member.identity.character_index,
                    )
                })
                .collect::<BTreeSet<_>>();
            if indexed_members.remove(id).unwrap_or_default() != expected {
                return Err(format!("guild clock membership authority mismatch for {id}").into());
            }
        }
        if !indexed_members.is_empty() {
            return Err("guild clock orphan membership index".into());
        }

        for (id, before) in &view.shared_guilds {
            let mut guild = before.clone();
            crate::runtime::advance_shared_guild_minutes(&mut guild, admitted_minutes);
            if guild == *before {
                continue;
            }
            guild.revision = guild
                .revision
                .checked_add(1)
                .ok_or("guild revision exhausted")?;
            before_guilds.insert(id.clone(), before.clone());
            after_guilds.insert(id.clone(), guild.clone());
            mutations.insert(
                id.clone(),
                shared_guild_store::GuildMutation {
                    expected_version: view.source_guild_versions.get(id).copied(),
                    desired: Some(guild),
                },
            );
        }
    }
    let guild_versions = shared_guild_store::write_guild_mutations(
        &mut transaction,
        &mutations,
        AccountStoreDatabaseMode::SourceOfTruth,
    )?;
    transaction.commit().map_err(classify_commit_error)?;
    Ok(Some(GuildClockCommitReceipt {
        before: locked.record,
        after,
        clock_version,
        before_guilds,
        after_guilds,
        guild_versions,
        admitted_minutes,
    }))
}
/// Compensate only the exact successful clock+guild receipt. A stale receipt
/// aborts the transaction, preserving a later owner's state. Its caller must
/// freeze further clock work if either durable mirror cannot be reconciled.
pub(in crate::config) fn compensate_postgres_tick(
    client: &mut Client,
    receipt: &GuildClockCommitReceipt,
) -> Result<GuildClockCommitReceipt, GuildClockTransactionError> {
    let mut transaction = client.transaction().map_err(|e| e.to_string())?;
    let locked = LockedPostgresGuildClock::lock(&mut transaction)?;
    let (after, clock_version) =
        locked.compensate(&mut transaction, receipt.clock_version, &receipt.before)?;
    let mutations = receipt
        .before_guilds
        .iter()
        .map(|(id, guild)| {
            let expected_version = receipt
                .guild_versions
                .get(id)
                .copied()
                .ok_or_else(|| format!("guild clock receipt missing guild {id}"))?;
            Ok((
                id.clone(),
                shared_guild_store::GuildMutation {
                    expected_version: Some(expected_version),
                    desired: Some(guild.clone()),
                },
            ))
        })
        .collect::<Result<BTreeMap<_, _>, String>>()?;
    let guild_versions = shared_guild_store::write_guild_mutations(
        &mut transaction,
        &mutations,
        AccountStoreDatabaseMode::SourceOfTruth,
    )?;
    transaction.commit().map_err(classify_commit_error)?;
    Ok(GuildClockCommitReceipt {
        before: receipt.after.clone(),
        after,
        clock_version,
        before_guilds: receipt.after_guilds.clone(),
        after_guilds: receipt.before_guilds.clone(),
        guild_versions,
        admitted_minutes: 0,
    })
}
