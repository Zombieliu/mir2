#![cfg(windows)]
use super::*;
fn entry(bytes: &[u8]) -> crate::model::FileEntry {
    crate::model::FileEntry {
        path: "control.bin".into(),
        size: bytes.len() as u64,
        sha256: crate::model::hash(bytes),
    }
}
#[test]
fn readonly_actual_file_lease_checks_hash_and_denies_mutation_and_ancestor_rename() {
    let root = tempfile::tempdir().unwrap();
    let parent = root.path().join("ancestor/child");
    let path = parent.join("control.bin");
    write_new(&path, b"signed bytes").unwrap();
    let mut bad = entry(b"signed bytes");
    bad.size += 1;
    assert!(FileLease::verified(&path, &bad).is_err());
    bad = entry(b"signed bytes");
    bad.sha256 = "A".repeat(64);
    assert!(FileLease::verified(&path, &bad).is_err());
    let lease = FileLease::verified(&path, &entry(b"signed bytes")).unwrap();
    assert_eq!(fs::read(&path).unwrap(), b"signed bytes");
    assert!(OpenOptions::new().write(true).open(&path).is_err());
    assert!(fs::remove_file(&path).is_err());
    assert!(fs::rename(&parent, root.path().join("renamed-child")).is_err());
    assert!(fs::rename(
        root.path().join("ancestor"),
        root.path().join("renamed-ancestor")
    )
    .is_err());
    // Leases do not prohibit unrelated child creation/private logs.
    write_new(&parent.join("player.log"), b"personal").unwrap();
    drop(lease);
    fs::rename(
        root.path().join("ancestor"),
        root.path().join("renamed-ancestor"),
    )
    .unwrap();
}
#[test]
fn leased_unicode_and_extended_local_paths_keep_the_existing_read_contract() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("数据-مرحبا-😀/control.bin");
    write_new(&path, b"unicode").unwrap();
    let normal = FileLease::verified(&path, &entry(b"unicode")).unwrap();
    let extended = fs::canonicalize(&path).unwrap();
    assert!(extended.to_string_lossy().starts_with(r"\\?\"));
    let long = FileLease::verified(&extended, &entry(b"unicode")).unwrap();
    assert_eq!(fs::read(&extended).unwrap(), b"unicode");
    drop((normal, long));
}
#[test]
fn leased_hardlink_alias_is_rejected_without_touching_personal_bytes() {
    let root = tempfile::tempdir().unwrap();
    let personal = root.path().join("personal.log");
    write_new(&personal, b"personal").unwrap();
    let alias = root.path().join("control.bin");
    fs::hard_link(&personal, &alias).unwrap();
    assert!(FileLease::verified(&alias, &entry(b"personal")).is_err());
    assert_eq!(fs::read(personal).unwrap(), b"personal");
}
#[test]
#[ignore = "requires Windows symbolic-link privilege; run explicitly for native release"]
fn actual_reparse_file_and_ancestor_are_rejected_without_touching_the_target() {
    let root = tempfile::tempdir().unwrap();
    let outside = root.path().join("personal");
    let personal = outside.join("player.log");
    write_new(&personal, b"personal").unwrap();
    let link = root.path().join("linked.bin");
    std::os::windows::fs::symlink_file(&personal, &link)
        .expect("explicit native symlink verification requires privilege");
    assert!(FileLease::verified(&link, &entry(b"personal")).is_err());
    let linked_parent = root.path().join("linked-parent");
    std::os::windows::fs::symlink_dir(&outside, &linked_parent)
        .expect("explicit native directory symlink verification requires privilege");
    assert!(DirectoryLease::acquire(&linked_parent).is_err());
    assert!(FileLease::verified(&linked_parent.join("player.log"), &entry(b"personal")).is_err());
    assert_eq!(fs::read(personal).unwrap(), b"personal");
    fs::remove_file(link).unwrap();
    fs::remove_dir(linked_parent).unwrap();
}
#[test]
fn verified_executable_can_be_mapped_while_the_parent_retains_its_readonly_lease() {
    let root = tempfile::tempdir().unwrap();
    let system = PathBuf::from(std::env::var_os("SystemRoot").expect("Windows system root"))
        .join("System32/cmd.exe");
    let bytes = fs::read(system).unwrap();
    let exe = root.path().join("fixture.exe");
    write_new(&exe, &bytes).unwrap();
    let lease = FileLease::verified(&exe, &entry(&bytes)).unwrap();
    let mut child = std::process::Command::new(&exe)
        .args(["/d", "/c", "exit", "0"])
        .spawn()
        .unwrap();
    assert!(fs::remove_file(&exe).is_err());
    assert!(child.wait().unwrap().success());
    drop(lease);
    fs::remove_file(exe).unwrap();
}
#[test]
fn namespace_normalization_distinguishes_unc_and_local_aliases_without_lossy_text() {
    let wide = |s: &str| s.encode_utf16().collect::<Vec<_>>();
    assert_eq!(
        windows_namespace(&wide(r"\\?\UNC\server\share\路径")),
        wide(r"\\server\share\路径")
    );
    assert_eq!(
        windows_namespace(&wide(r"\\?\unc\server\share\路径")),
        wide(r"\\server\share\路径")
    );
    assert_eq!(windows_namespace(&wide(r"\\?\C:\路径")), wide(r"C:\路径"));
    assert_ne!(
        windows_namespace(&wide(r"\\?\UNC\server\share\路径")),
        wide(r"UNC\server\share\路径")
    );
}
