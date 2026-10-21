//! Historical material outcomes and recovery of this operation's private fence.
//! Recovery publishes the latest canonical account, never the old item image.
use super::*;

fn same_fence(actual: &HuntingMaterialFence, expected: &HuntingMaterialFence) -> bool {
    actual.realm == expected.realm
        && actual.database_binding == expected.database_binding
        && actual.owner == expected.owner
        && actual.quote == expected.quote
        && actual.guilds == expected.guilds
        && actual.reason == expected.reason
        && actual.epoch == expected.epoch
}

fn publication_error(
    commit: Option<HuntingMaterialCommit>,
    detail: String,
) -> HuntingMaterialSourceError {
    match commit {
        Some(commit) => HuntingMaterialSourceError::KnownCommitted {
            commit: Box::new(commit),
            detail,
        },
        None => HuntingMaterialSourceError::OutcomeUnknown(detail),
    }
}

impl SimulationConfig {
    /// Historical queries still authorize the current account, ban state and
    /// character UUID. Current prices, quote TTL and admission policy do not
    /// invalidate a previously committed conversion.
    pub(crate) fn lookup_hunting_material_source(
        &self,
        account: &str,
        incarnation: &str,
        index: i32,
        quote_id: &str,
    ) -> Result<Option<HuntingMaterialCommit>, HuntingMaterialSourceError> {
        validate_actor(account, incarnation)?;
        if index < 0 || !valid_quote_id(quote_id) {
            return Err("invalid material lookup identity".to_string().into());
        }
        // This is only a probe. No fence guard crosses the persistence lock.
        let fence = self
            .hunting_material_fence
            .lock()
            .map_err(|_| "material lookup recovery fence unavailable".to_string())?
            .clone();
        let _persist = self
            .account_store_persist_lock
            .lock()
            .map_err(|_| "material lookup persistence lock unavailable".to_string())?;
        let (realm, url) = self.hunting_source_binding()?;
        let mut write_state = self
            .account_store_write_state
            .lock()
            .map_err(|_| "material lookup write state unavailable".to_string())?;
        let owns_fence = match &*write_state {
            AccountStoreWriteState::Writable => false,
            AccountStoreWriteState::Frozen { reason } => {
                let Some(fence) = fence.as_ref() else {
                    return Err("material lookup cannot release another publication fence"
                        .to_string()
                        .into());
                };
                if fence.realm != realm
                    || fence.database_binding != database_binding(url)
                    || fence.owner.account_id != account
                    || fence.owner.character_index != index
                    || fence.owner.incarnation != incarnation
                    || fence.quote.realm_id != realm
                    || fence.quote.owner != fence.owner
                    || fence.quote.quote_id != quote_id
                    || fence.reason != *reason
                    || fence.epoch != self.account_store_freeze_epoch.load(Ordering::Acquire)
                {
                    return Err("material lookup cannot release another publication fence"
                        .to_string()
                        .into());
                }
                true
            }
        };
        let guilds = if owns_fence {
            fence.as_ref().unwrap().guilds.clone()
        } else {
            Vec::new()
        };
        let locked = RefCell::new(None::<(LockedImage, Option<MaterialConversionQuoteReceipt>)>);
        // No writable/freeze helper may run while write_state is held. The
        // service owns the realm transaction and checks both immutable halves.
        let delivered = with_account_store_postgres_client(url, |client| {
            Ok(economy::query_material_conversion_authorized_with_timeouts(
                client,
                realm,
                quote_id,
                2000,
                10000,
                |tx, _| {
                    lock_guilds(tx, &guilds)?;
                    let image =
                        lock_and_load(tx, account, index, None, guilds.iter().cloned(), false)?
                            .ok_or(EconomyError::CharacterNotFound)?;
                    if image.owner.incarnation != incarnation {
                        return Err(EconomyError::CharacterNotFound);
                    }
                    verify_current(tx, &image)?;
                    let quote = read_quote(tx, realm, quote_id)?;
                    if let Some(quote) = quote.as_ref() {
                        if quote.owner != image.owner || quote.realm_id != realm {
                            return Err(EconomyError::CharacterNotFound);
                        }
                        if owns_fence && quote != &fence.as_ref().unwrap().quote {
                            return Err(EconomyError::RequestConflict);
                        }
                    } else if owns_fence {
                        return Err(EconomyError::CorruptHistory(
                            "material_fenced_quote_missing",
                        ));
                    }
                    let owner = image.owner.clone();
                    *locked.borrow_mut() = Some((image, quote));
                    Ok(owner)
                },
            ))
        });
        let receipt = match delivered {
            Ok(Ok(receipt)) => receipt,
            Ok(Err(error)) => {
                let unresolved = owns_fence
                    || matches!(
                        error,
                        EconomyError::OutcomeUnknown | EconomyError::CorruptHistory(_)
                    );
                if unresolved {
                    let verified_quote = locked
                        .borrow()
                        .as_ref()
                        .and_then(|(_, quote)| quote.clone());
                    drop(write_state);
                    let detail = if !owns_fence {
                        verified_quote.as_ref().map(|quote| {
                            self.fence_hunting_material(
                                realm,
                                url,
                                quote,
                                guilds,
                                "material original query evidence remains unresolved".into(),
                            )
                        })
                    } else {
                        None
                    };
                    return Err(HuntingMaterialSourceError::OutcomeUnknown(
                        detail
                            .unwrap_or_else(|| "material original query remains unresolved".into()),
                    ));
                }
                return Err(HuntingMaterialSourceError::Rejected(
                    "material query source unavailable".into(),
                ));
            }
            Err(_) => {
                let verified_quote = locked
                    .borrow()
                    .as_ref()
                    .and_then(|(_, quote)| quote.clone());
                drop(write_state);
                let detail = if !owns_fence {
                    verified_quote.as_ref().map(|quote| {
                        self.fence_hunting_material(
                            realm,
                            url,
                            quote,
                            guilds,
                            "material original query delivery remains unresolved".into(),
                        )
                    })
                } else {
                    None
                };
                return Err(HuntingMaterialSourceError::OutcomeUnknown(
                    detail.unwrap_or_else(|| "material original query remains unresolved".into()),
                ));
            }
        };
        let Some((image, quote)) = locked.into_inner() else {
            return Err(HuntingMaterialSourceError::OutcomeUnknown(
                "material query lacks its canonical owner image".into(),
            ));
        };
        let commit = if let Some(receipt) = receipt {
            let Some(quote) = quote.as_ref() else {
                return Err(HuntingMaterialSourceError::OutcomeUnknown(
                    "material outcome lacks its original quote".into(),
                ));
            };
            match self.material_commit(
                realm,
                url,
                quote,
                guilds.clone(),
                Idempotent {
                    receipt,
                    replayed: true,
                },
                image.account.saves.get(&index).map(|save| save.revision),
            ) {
                Ok(commit) => Some(commit),
                Err(_) => {
                    drop(write_state);
                    let detail = if owns_fence {
                        "material original outcome failed complete validation".into()
                    } else {
                        self.fence_hunting_material(
                            realm,
                            url,
                            quote,
                            guilds,
                            "material original outcome failed complete validation".into(),
                        )
                    };
                    return Err(HuntingMaterialSourceError::OutcomeUnknown(detail));
                }
            }
        } else {
            None
        };
        let mut live = match self.account_store.lock() {
            Ok(live) => live,
            Err(_) => {
                drop(write_state);
                let detail = if !owns_fence {
                    if let Some(quote) = quote.as_ref() {
                        self.fence_hunting_material(
                            realm,
                            url,
                            quote,
                            guilds,
                            "material query mirror publication unavailable".into(),
                        )
                    } else {
                        "material query mirror publication unavailable".into()
                    }
                } else {
                    "material query mirror publication unavailable".into()
                };
                return Err(publication_error(commit, detail));
            }
        };
        let mut staged = live.clone();
        staged.source_account_versions.remove(account);
        staged.source_save_versions.remove(account);
        staged.accounts.insert(account.into(), image.account);
        for (id, guild) in image.guilds {
            if let Some(guild) = guild {
                staged.shared_guilds.insert(id, guild);
            } else {
                staged.shared_guilds.remove(&id);
                staged.source_guild_versions.remove(&id);
            }
        }
        staged.merge_source_versions(image.versions);
        staged.normalize_next_character_index();
        if owns_fence {
            let fence = fence.as_ref().unwrap();
            let mut slot = match self.hunting_material_fence.lock() {
                Ok(slot) => slot,
                Err(_) => {
                    return Err(publication_error(
                        commit,
                        "material query recovery fence unavailable".into(),
                    ));
                }
            };
            if self.account_store_freeze_epoch.load(Ordering::Acquire) != fence.epoch
                || !matches!(&*write_state, AccountStoreWriteState::Frozen { reason } if reason == &fence.reason)
                || slot
                    .as_ref()
                    .is_none_or(|stored| !same_fence(stored, fence))
            {
                return Err(publication_error(
                    commit,
                    "material query was superseded by another publication fence".into(),
                ));
            }
            *live = staged;
            *write_state = AccountStoreWriteState::Writable;
            *slot = None;
        } else {
            *live = staged;
        }
        self.guild_clock_updates.fetch_add(1, Ordering::AcqRel);
        Ok(commit)
    }

    /// This is a trusted login preflight, not receipt authorization by a former
    /// actor. Only the private fence supplies the historical owner. A removed,
    /// banned or recreated character is refreshed from current canonical state.
    pub(crate) fn reconcile_hunting_material_before_account_refresh(
        &self,
        account: &str,
    ) -> Result<(), String> {
        let fence = self
            .hunting_material_fence
            .lock()
            .map_err(|_| "material refresh recovery fence unavailable")?
            .clone();
        let Some(fence) = fence else {
            return Ok(());
        };
        if fence.owner.account_id != account
            || self.hunting_economy_realm.as_deref() != Some(fence.realm.as_str())
            || self
                .account_store_database_url
                .as_deref()
                .is_none_or(|url| database_binding(url) != fence.database_binding)
        {
            return Ok(());
        }
        let _persist = self
            .account_store_persist_lock
            .lock()
            .map_err(|_| "material refresh recovery persistence lock unavailable")?;
        let (realm, url) = self.hunting_source_binding()?;
        if realm != fence.realm || database_binding(url) != fence.database_binding {
            return Ok(());
        }
        let mut write_state = self
            .account_store_write_state
            .lock()
            .map_err(|_| "material refresh recovery write state unavailable")?;
        if self.account_store_freeze_epoch.load(Ordering::Acquire) != fence.epoch
            || !matches!(&*write_state, AccountStoreWriteState::Frozen { reason } if reason == &fence.reason)
        {
            return Ok(());
        }
        let current = RefCell::new(None);
        let delivered = with_account_store_postgres_client(url, |client| {
            let receipt = economy::query_material_conversion_authorized_with_timeouts(
                client,
                realm,
                &fence.quote.quote_id,
                2000,
                10000,
                |tx, _| {
                    lock_guilds(tx, &fence.guilds)?;
                    let image = lock_and_load(
                        tx,
                        account,
                        fence.owner.character_index,
                        None,
                        fence.guilds.iter().cloned(),
                        true,
                    )?;
                    if let Some(image) = image.as_ref() {
                        if !image.owner.incarnation.is_empty() {
                            verify_current(tx, image)?;
                        }
                    }
                    let quote = read_quote(tx, realm, &fence.quote.quote_id)?
                        .ok_or(EconomyError::CorruptHistory("material_fenced_quote_missing"))?;
                    if quote != fence.quote
                        || quote.owner != fence.owner
                        || quote.realm_id != realm
                    {
                        return Err(EconomyError::RequestConflict);
                    }
                    let (account_image, mut versions, mut guild_images) = match image {
                        Some(image) => (Some(image.account), image.versions, image.guilds),
                        None => (None, AccountStoreSourceVersions::default(), BTreeMap::new()),
                    };
                    // lock_and_load returns early for a deleted account. Its
                    // former Guild mirrors still need a complete version refresh.
                    if account_image.is_none() {
                        for id in &fence.guilds {
                            let row = tx
                                .query_opt(
                                    "SELECT raw_json,store_version FROM shared_guilds WHERE guild_id=$1",
                                    &[id],
                                )
                                .map_err(source_db)?;
                            let guild = if let Some(row) = row {
                                versions.guilds.insert(id.clone(), row.get("store_version"));
                                Some(serde_json::from_value(row.get("raw_json")).map_err(|_| {
                                    EconomyError::CorruptHistory("material_current_guild_checkpoint")
                                })?)
                            } else {
                                None
                            };
                            guild_images.insert(id.clone(), guild);
                        }
                    }
                    *current.borrow_mut() = Some((account_image, versions, guild_images));
                    // This private, fence-bound historical probe authorizes no
                    // gameplay operation or publication to the former actor.
                    Ok(fence.owner.clone())
                },
            )
            .map_err(|_| "material original refresh recovery remains unresolved")?;
            if let Some(receipt) = receipt.as_ref() {
                validate_receipt(realm, &fence.quote, receipt)
                    .map_err(|_| "material original refresh outcome failed validation")?;
            }
            Ok(())
        });
        delivered
            .map_err(|_| "material original refresh recovery remains unresolved".to_string())?;
        let (current, versions, guild_images) = current
            .into_inner()
            .ok_or("material refresh recovery lacks its current canonical image")?;
        let mut live = self
            .account_store
            .lock()
            .map_err(|_| "material refresh recovery mirror unavailable")?;
        let mut slot = self
            .hunting_material_fence
            .lock()
            .map_err(|_| "material refresh recovery fence unavailable")?;
        if self.account_store_freeze_epoch.load(Ordering::Acquire) != fence.epoch
            || !matches!(&*write_state, AccountStoreWriteState::Frozen { reason } if reason == &fence.reason)
            || slot
                .as_ref()
                .is_none_or(|stored| !same_fence(stored, &fence))
        {
            return Err(
                "material refresh recovery was superseded by another publication fence".into(),
            );
        }
        let mut staged = live.clone();
        staged.source_account_versions.remove(account);
        staged.source_save_versions.remove(account);
        if let Some(current) = current {
            staged.accounts.insert(account.into(), current);
        } else {
            staged.accounts.remove(account);
        }
        for (id, guild) in guild_images {
            if let Some(guild) = guild {
                staged.shared_guilds.insert(id, guild);
            } else {
                staged.shared_guilds.remove(&id);
                staged.source_guild_versions.remove(&id);
            }
        }
        staged.merge_source_versions(versions);
        staged.normalize_next_character_index();
        *live = staged;
        *write_state = AccountStoreWriteState::Writable;
        *slot = None;
        self.guild_clock_updates.fetch_add(1, Ordering::AcqRel);
        Ok(())
    }
}
