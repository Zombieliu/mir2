//! The administrator supplies an explicit complete-history census. Absence is
//! unavailable authority, never permission to reconstruct a floor from zero.
use super::*;

pub(super) fn read_reservation(
    tx: &mut Transaction<'_>,
    binding: &PlayerDropOperationBinding,
    count: usize,
) -> Result<Option<Vec<u64>>, String> {
    let Some(batch)=tx.query_opt("SELECT binding,item_count FROM public.zone_player_drop_item_uid_batches WHERE source_namespace=current_schema() AND operation_key=$1",&[&binding.operation_key()]).map_err(|e|e.to_string())? else {return Ok(None)};
    if batch.get::<_, serde_json::Value>(0)
        != serde_json::to_value(binding).map_err(|e| e.to_string())?
        || batch.get::<_, i32>(1) as usize != count
    {
        return Err("player drop item UID reservation binding conflict".into());
    }
    let rows=tx.query("SELECT ordinal,item_uid::text FROM public.zone_player_drop_item_uid_reservations WHERE source_namespace=current_schema() AND operation_key=$1 ORDER BY ordinal",&[&binding.operation_key()]).map_err(|e|e.to_string())?;
    if rows.len() != count {
        return Err("player drop item UID reservation is incomplete".into());
    }
    rows.into_iter()
        .enumerate()
        .map(|(ordinal, row)| {
            if row.get::<_, i32>(0) as usize != ordinal {
                return Err("player drop item UID reservation ordinal conflict".into());
            }
            row.get::<_, String>(1)
                .parse::<u64>()
                .map_err(|_| "invalid player drop item UID reservation".into())
        })
        .collect::<Result<Vec<_>, _>>()
        .map(Some)
}

impl PostgresEconomyStore {
    /// Explicit startup/admin boundary. `census_digest` must identify the complete
    /// inspected canonical/offline/world/custody/receipt history. The normal
    /// drop path cannot call this automatically or lower an existing floor.
    pub fn initialize_player_drop_item_uid_authority(
        &self,
        censused_floor: u64,
        census_digest: &str,
        administrator_id: &str,
    ) -> Result<(), String> {
        if census_digest.len() != 64
            || !census_digest
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            || administrator_id.trim().is_empty()
            || administrator_id.len() > 1024
            || administrator_id.chars().any(char::is_control)
        {
            return Err("invalid explicit player drop UID census audit".into());
        }
        let mut client = self.player_drop_connect()?;
        let mut tx = client.transaction().map_err(|e| e.to_string())?;
        zone_fence::bound_transaction(&mut tx)?;
        tx.query_one(
            "SELECT pg_advisory_xact_lock(hashtextextended('mir2.player-drop-uid-census',0))",
            &[],
        )
        .map_err(|e| e.to_string())?;
        let old=tx.query_opt("SELECT issued_through::text FROM public.zone_player_drop_item_uid_authority WHERE source_namespace=current_schema() AND singleton=TRUE FOR UPDATE",&[]).map_err(|e|e.to_string())?.map(|r|r.get::<_,String>(0));
        if old
            .as_ref()
            .map(|v| {
                v.parse::<u64>()
                    .map_err(|_| "invalid durable player drop UID authority")
            })
            .transpose()?
            .is_some_and(|floor| floor > censused_floor)
        {
            return Err("player drop UID census cannot lower the issued floor".into());
        }
        let floor = censused_floor.to_string();
        let now = now_ms() as i64;
        tx.execute("INSERT INTO public.zone_player_drop_item_uid_authority(singleton,issued_through,census_digest,initialized_by,initialized_at_ms) VALUES(TRUE,$1::text::numeric,$2,$3,$4) ON CONFLICT(source_namespace) DO UPDATE SET issued_through=EXCLUDED.issued_through,census_digest=EXCLUDED.census_digest,initialized_by=EXCLUDED.initialized_by,initialized_at_ms=EXCLUDED.initialized_at_ms",
            &[&floor,&census_digest,&administrator_id,&now]).map_err(|e|e.to_string())?;
        tx.execute("INSERT INTO public.zone_player_drop_item_uid_census_audit(previous_floor,censused_floor,census_digest,administrator_id,recorded_at_ms) VALUES($1::text::numeric,$2::text::numeric,$3,$4,$5)",
            &[&old,&floor,&census_digest,&administrator_id,&now]).map_err(|e|e.to_string())?;
        tx.commit().map_err(|_|"PLAYER_DROP_UID_CENSUS_COMMIT_OUTCOME_UNKNOWN: query authority/audit; do not repeat from a lower floor".into())
    }

    /// Retires exactly one immutable, pre-order UID list for this operation.
    /// Rejected gameplay may leave gaps; no later operation can receive them.
    pub fn reserve_player_drop_item_uids(
        &self,
        permit: &ZoneEconomyWritePermit,
        binding: &PlayerDropOperationBinding,
        count: usize,
    ) -> Result<Vec<u64>, String> {
        binding.validate_integrity()?;
        if !(1..=1024).contains(&count) || permit.fence().owner_id() != binding.owner_id() {
            return Err("invalid player drop item UID reservation".into());
        }
        let mut client = self.player_drop_connect()?;
        let mut tx = client.transaction().map_err(|e| e.to_string())?;
        permit.lock_and_validate(
            &mut tx,
            &self.database_url,
            binding.zone_id(),
            binding.owner_epoch(),
        )?;
        let namespace: String = tx
            .query_one("SELECT current_schema()::text", &[])
            .map_err(|_| "player drop UID namespace unavailable")?
            .try_get(0)
            .map_err(|_| "player drop UID namespace unavailable")?;
        mir2_simulation::user_item_uid_postgres::lock_existing_ordinary_item_uid_authority_in_transaction(
            &mut tx,
            &self.database_url,
            &namespace,
            permit,
        )
        .map_err(|error| error.to_string())?;
        if let Some(ids) = read_reservation(&mut tx, binding, count)? {
            return Ok(ids);
        }
        let ids =
            mir2_simulation::user_item_uid_postgres::reserve_ordinary_item_uids_in_transaction(
                &mut tx,
                &self.database_url,
                &namespace,
                permit,
                count,
            )
            .map_err(|error| error.to_string())?;
        tx.execute("INSERT INTO public.zone_player_drop_item_uid_batches(operation_key,binding,item_count,reserved_at_ms) VALUES($1,$2,$3,$4)",
            &[&binding.operation_key(),&serde_json::to_value(binding).map_err(|e|e.to_string())?,&(count as i32),&(now_ms() as i64)]).map_err(|e|e.to_string())?;
        for (ordinal, uid) in ids.iter().enumerate() {
            tx.execute("INSERT INTO public.zone_player_drop_item_uid_reservations(operation_key,ordinal,item_uid) VALUES($1,$2,$3::text::numeric)",
                &[&binding.operation_key(),&(ordinal as i32),&uid.to_string()]).map_err(|e|e.to_string())?;
        }
        tx.commit().map_err(|_| permit.commit_outcome_unknown())?;
        Ok(ids)
    }

    /// Read-only Unknown recovery. None remains unresolved; it never authorizes
    /// a second allocation or any account debit.
    pub fn lookup_player_drop_item_uids(
        &self,
        binding: &PlayerDropOperationBinding,
        count: usize,
    ) -> Result<Option<Vec<u64>>, String> {
        binding.validate_integrity()?;
        if !(1..=1024).contains(&count) {
            return Err("invalid player drop UID reservation size".into());
        }
        let mut client = self.player_drop_connect()?;
        let mut tx = client.transaction().map_err(|e| e.to_string())?;
        zone_fence::bound_transaction(&mut tx)?;
        read_reservation(&mut tx, binding, count)
    }
}
