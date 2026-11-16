//! Local admission of installed signed packs. The locator grants no authority;
//! CMS, engine source, catalog, required availability and release floors are
//! verified first. v1 still requires every pack; v2 requires its signed scope.
use super::*;

const LOCATOR: &str = "updater/packed-install.json";

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Locator {
    schema: String,
    manifest_sha256: String,
}

fn locator(root: &Path) -> Result<Option<String>> {
    let path = safe::target(root, LOCATOR)?;
    if !path.exists() {
        return Ok(None);
    }
    let value: Locator = serde_json::from_slice(&safe::read_bounded(&path, 4096)?)?;
    ensure!(
        value.schema == "mir2.windows.packed-install.v1"
            && model::valid_hash(&value.manifest_sha256.to_ascii_uppercase())
            && value.manifest_sha256 == value.manifest_sha256.to_ascii_lowercase(),
        "invalid packed installation locator"
    );
    Ok(Some(value.manifest_sha256.to_ascii_uppercase()))
}

struct VerifiedSeed {
    selection: Selection,
    feed_bytes: Vec<u8>,
    feed_signature: Vec<u8>,
}

fn verified(root: &Path, digest: &str) -> Result<VerifiedSeed> {
    let (manifest, generation, catalog, scope, feed_identity) =
        read_generation_metadata(root, digest)?;
    let (engine, _) = crate::update::active_engine(root)?;
    ensure!(
        engine.source_revision == manifest.engine_source_revision,
        "offline installation requires its authenticated engine"
    );
    let bytes = safe::read_bounded(&generation.join("PACK-FEED.json"), 32768)?;
    let signature = safe::read_bounded(&generation.join("PACK-FEED.p7s"), 32768)?;
    ensure!(
        model::hash(&bytes) == feed_identity,
        "offline installation feed changed during verification"
    );
    let feed = authenticated_feed(&bytes, &signature, false)?;
    ensure!(
        feed.sequence > crate::update::legacy_sequence_floor(root)?
            && feed.sequence > crate::update::engine_maintenance::receipt_sequence_floor(root)?,
        "offline installation below signed release floor"
    );
    if let Some((highest, identity)) = highest(root)? {
        ensure!(
            feed.sequence > highest.sequence
                || (feed.sequence == highest.sequence && model::hash(&bytes) == identity),
            "offline installation downgrade or sequence reuse"
        );
    }
    ensure!(
        !safe::target(root, &format!("state/quarantine/{digest}.json"))?.exists(),
        "offline installation quarantined after failed launch"
    );
    let _packs = progressive_generation::open_packs(root, &manifest, &catalog, scope.as_deref())?;
    Ok(VerifiedSeed {
        selection: Selection::Generation {
            manifest_sha256: digest.into(),
            sequence: manifest.sequence,
        },
        feed_bytes: bytes,
        feed_signature: signature,
    })
}

/// Verify content without network, selection writes or a health assertion.
pub fn verify_installed_seed(root: &Path) -> Result<Option<Selection>> {
    let Some(digest) = locator(root)? else {
        return Ok(None);
    };
    Ok(Some(verified(root, &digest)?.selection))
}

/// Caller holds InstallLock and GameLease. Existing selections and recovery
/// obligations remain authoritative; an installer locator never replaces them.
pub fn adopt_installed_seed(root: &Path) -> Result<bool> {
    if active_record(root)?.is_some() {
        return Ok(false);
    }
    let Some(digest) = locator(root)? else {
        return Ok(false);
    };
    ensure!(
        !crate::transaction::pending_activation(root)?,
        "legacy recovery must finish before admitting an offline installation"
    );
    let started = Instant::now();
    let seed = verified(root, &digest)?;
    let previous = if safe::target(root, "game/mir2-platform-windows.exe")?.exists() {
        Some(legacy_selection(root)?)
    } else {
        None
    };
    let health = if previous.is_some() {
        Health::Pending
    } else {
        Health::FirstLaunchPending
    };
    // Persist the signed floor first. Power-loss recovery admits only the same
    // signed bytes or a newer release if the selector was not yet committed.
    remember_highest(root, &seed.feed_bytes, &seed.feed_signature)?;
    save_record(
        root,
        &ActiveRecord {
            schema: "mir2.windows.active-generation.v1".into(),
            active: seed.selection,
            previous,
            health,
            attempt: nonce()?,
            launch_started: false,
        },
    )?;
    crate::update::log(
        root,
        "packed-offline-install-adopted",
        serde_json::json!({"manifestSha256":digest,"elapsedMs":started.elapsed().as_millis(),"responseBodyBytes":0,"firstLaunchPending":true}),
    );
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_locator_does_not_create_selection_or_floor() {
        let root = tempfile::tempdir().unwrap();
        assert!(verify_installed_seed(root.path()).unwrap().is_none());
        assert!(!adopt_installed_seed(root.path()).unwrap());
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 0);
    }

    #[test]
    fn literal_lowercase_locator_normalizes_to_authenticated_uppercase_hashes() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir(root.path().join("updater")).unwrap();
        fs::write(root.path().join(LOCATOR),
            br#"{"schema":"mir2.windows.packed-install.v1","manifestSha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}"#).unwrap();
        assert_eq!(locator(root.path()).unwrap(), Some("A".repeat(64)));
    }

    #[test]
    fn malformed_locator_and_unsigned_content_cannot_activate() {
        let examples: &[&[u8]] = &[
            br#"{"schema":"other","manifestSha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}"#,
            br#"{"schema":"mir2.windows.packed-install.v1","manifestSha256":"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"}"#,
            br#"{"schema":"mir2.windows.packed-install.v1","manifestSha256":"../payload"}"#,
            br#"{"schema":"mir2.windows.packed-install.v1","manifestSha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","healthy":true}"#,
            br#"{"schema":"mir2.windows.packed-install.v1","manifestSha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}"#,
        ];
        for bytes in examples {
            let root = tempfile::tempdir().unwrap();
            fs::create_dir(root.path().join("updater")).unwrap();
            fs::write(root.path().join(LOCATOR), bytes).unwrap();
            assert!(verify_installed_seed(root.path()).is_err());
            assert!(adopt_installed_seed(root.path()).is_err());
            assert!(!root.path().join(SELECTOR).exists());
            assert!(!root.path().join(HIGHEST).exists());
        }
    }

    #[test]
    fn failed_initial_launch_cannot_fabricate_healthy_rollback() {
        let root = tempfile::tempdir().unwrap();
        let digest = "A".repeat(64);
        let record = ActiveRecord {
            schema: "mir2.windows.active-generation.v1".into(),
            active: Selection::Generation {
                manifest_sha256: digest.clone(),
                sequence: 106,
            },
            previous: None,
            health: Health::FirstLaunchPending,
            attempt: "B".repeat(64),
            launch_started: true,
        };
        save_record(root.path(), &record).unwrap();
        assert!(active_record(root.path()).unwrap().is_some());
        assert!(recover(root.path()).is_err());
        assert!(root
            .path()
            .join(format!("state/quarantine/{digest}.json"))
            .exists());
        let after = active_record(root.path()).unwrap().unwrap();
        assert_eq!(after.health, Health::FirstLaunchPending);
        assert!(after.previous.is_none());
        assert!(recover(root.path()).is_err());
        assert!(!root.path().join(HIGHEST).exists());
    }
}
