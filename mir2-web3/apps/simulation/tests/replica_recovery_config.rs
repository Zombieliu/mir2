use mir2_simulation::SimulationConfig;

#[test]
fn replica_apply_cannot_write_authoritative_recovery_journal_until_promotion() {
    let recovery_dir = std::env::temp_dir().join("mir2-authoritative-recovery-config");
    let recovery_key = [
        0x10, 0x21, 0x32, 0x43, 0x54, 0x65, 0x76, 0x87, 0x98, 0xa9, 0xba, 0xcb, 0xdc, 0xed, 0xfe,
        0x0f, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee,
        0xf1, 0x02,
    ];
    let authoritative = SimulationConfig::default()
        .with_save_recovery_dir(recovery_dir.clone())
        .with_save_recovery_mac_key(recovery_key)
        .unwrap();

    let mut replica = authoritative
        .fork_for_replica_apply()
        .expect("replica configuration should fork");
    assert!(replica.account_store_path.is_none());
    assert!(replica.account_store_database_url.is_none());
    assert!(replica.save_recovery_dir.is_none());
    assert!(replica.save_recovery_mac_key().is_none());

    replica.rebind_account_store_from(&authoritative);
    assert_eq!(replica.save_recovery_dir, Some(recovery_dir));
    assert_eq!(replica.save_recovery_mac_key(), Some(&recovery_key));
}
