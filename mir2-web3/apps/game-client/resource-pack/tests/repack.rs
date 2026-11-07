use mir2_resource_pack::{
    build,
    repack::{preflight, repack, repack_files, PLAN_SCHEMA},
    sha256, write_catalog_new, BuildOptions, BuildOutput, PackStore, ResourceInput, MAX_PACKS,
    MAX_PACK_BYTES,
};
use serde_json::{json, Value};
use std::{fs, path::PathBuf};

struct Fixture {
    temp: tempfile::TempDir,
    loose: PathBuf,
    objects: PathBuf,
    build: BuildOutput,
}
impl Fixture {
    fn new(files: &[(&str, &[u8])]) -> Self {
        let temp = tempfile::tempdir().unwrap();
        let loose = temp.path().join("loose");
        fs::create_dir(&loose).unwrap();
        let objects = temp.path().join("old-objects");
        let inputs = files
            .iter()
            .map(|(path, bytes)| {
                let p = loose.join(path);
                fs::create_dir_all(p.parent().unwrap()).unwrap();
                fs::write(p, bytes).unwrap();
                ResourceInput {
                    path: (*path).into(),
                    size: bytes.len() as u64,
                    sha256: sha256(bytes),
                }
            })
            .collect::<Vec<_>>();
        let build = build(&loose, &objects, &inputs, None, BuildOptions::default()).unwrap();
        Self {
            temp,
            loose,
            objects,
            build,
        }
    }
    fn store(&self) -> PackStore {
        PackStore::open(
            &self.objects,
            &self.build.catalog_bytes,
            &self.build.catalog_sha256,
        )
        .unwrap()
    }
    fn plan(&self) -> Value {
        json!({"schema":PLAN_SCHEMA,"sourceCatalogSha256":self.build.catalog_sha256,
            "targetPackBytes":1024,"groups":[]})
    }
    fn output(&self, name: &str) -> PathBuf {
        self.temp.path().join(name)
    }
}
fn bytes(value: &Value) -> Vec<u8> {
    serde_json::to_vec(value).unwrap()
}

#[test]
fn full_equivalence_group_isolation_alias_priority_and_no_loose_extraction() {
    let files: Vec<(&str, &[u8])> = vec![
        ("a/default-alias.png", b"shared"),
        ("x/later-alias.png", b"shared"),
        ("z/early-alias.png", b"shared"),
        ("ui/button.png", b"button"),
        ("map/0.map.gz", b"map"),
        ("empty.txt", b""),
    ];
    let f = Fixture::new(&files);
    let old = f
        .build
        .catalog
        .packs
        .iter()
        .map(|p| {
            (
                p.file_name(),
                fs::read(f.objects.join(p.file_name())).unwrap(),
            )
        })
        .collect::<Vec<_>>();
    let source =
        PackStore::open_profiled(&f.objects, &f.build.catalog_bytes, &f.build.catalog_sha256)
            .unwrap();
    fs::remove_dir_all(&f.loose).unwrap();
    let mut plan = f.plan();
    plan["groups"] = json!([
        {"name":"first-scene","paths":["z/early-alias.png","map/0.map.gz"]},
        {"name":"ui","paths":["x/later-alias.png","ui/button.png"]}
    ]);
    let pre = preflight(&source, &bytes(&plan)).unwrap();
    assert_eq!(pre.resource_count, 6);
    assert_eq!(pre.unique_payload_count, 4);
    assert_eq!(pre.projected_pack_count, 3);
    assert!(!f.output("fresh").exists());
    let receipt = repack(&source, &bytes(&plan), &f.output("fresh")).unwrap();
    assert!(receipt.full_logical_equivalence_verified && receipt.output_pack_store_verified);
    assert!(!receipt.bootstrap_authorized);
    let reads = source.read_profile_snapshot().unwrap();
    assert_eq!(reads.successful_read_operations, 4);
    assert_eq!(reads.failed_read_operations, 0);
    assert!(reads.resources.contains_key("z/early-alias.png"));
    assert!(!reads.resources.contains_key("a/default-alias.png"));
    let cb = fs::read(f.output("fresh/catalog.json")).unwrap();
    let new = PackStore::open(
        f.output("fresh/objects/packs"),
        &cb,
        &receipt.catalog_sha256,
    )
    .unwrap();
    assert_eq!(
        new.catalog().resources.len(),
        source.catalog().resources.len()
    );
    for (path, expected) in &files {
        let before = &source.catalog().resources[*path];
        let after = &new.catalog().resources[*path];
        assert_eq!((before.size, &before.sha256), (after.size, &after.sha256));
        assert_eq!(new.read(path).unwrap(), *expected);
    }
    let r = &new.catalog().resources;
    assert_eq!(r["a/default-alias.png"], r["z/early-alias.png"]);
    assert_eq!(r["x/later-alias.png"], r["z/early-alias.png"]);
    assert_ne!(
        r["z/early-alias.png"].pack_sha256,
        r["ui/button.png"].pack_sha256
    );
    assert_ne!(r["empty.txt"].pack_sha256, r["ui/button.png"].pack_sha256);
    assert_eq!(receipt.preflight.groups[0].logical_resource_count, 4);
    assert_eq!(
        receipt.groups[0].packs[0].sha256,
        r["z/early-alias.png"].pack_sha256
    );
    assert_eq!(fs::read_dir(f.output("fresh")).unwrap().count(), 3);
    for (name, data) in old {
        assert_eq!(fs::read(f.objects.join(name)).unwrap(), data);
    }
}

#[test]
fn same_plan_is_deterministic_and_target_is_soft_only_for_indivisible_resources() {
    let big = [9u8; 500];
    let f = Fixture::new(&[("big.png", &big), ("small.png", b"tiny")]);
    let source = f.store();
    let mut plan = f.plan();
    plan["targetPackBytes"] = json!(96);
    let first = repack(&source, &bytes(&plan), &f.output("one")).unwrap();
    let second = repack(&source, &bytes(&plan), &f.output("two")).unwrap();
    assert_eq!(first.catalog_sha256, second.catalog_sha256);
    assert_eq!(first.preflight.projected_pack_count, 2);
    assert!(first.groups[0].packs.iter().any(|p| p.size == 596));
    for name in ["catalog.json", "REPACK-RECEIPT.json"] {
        assert_eq!(
            fs::read(f.output(&format!("one/{name}"))).unwrap(),
            fs::read(f.output(&format!("two/{name}"))).unwrap()
        );
    }
}

#[test]
fn invalid_plans_fail_before_any_output_is_created() {
    let f = Fixture::new(&[("a.png", b"a"), ("b.png", b"b")]);
    let source = f.store();
    let base = f.plan();
    let mut cases = Vec::new();
    let mut p = base.clone();
    p["schema"] = json!("wrong");
    cases.push(p);
    let mut p = base.clone();
    p["sourceCatalogSha256"] = json!("00".repeat(32));
    cases.push(p);
    let mut p = base.clone();
    p["sourceCatalogSha256"] = json!("../unsafe");
    cases.push(p);
    for n in [0, 95, MAX_PACK_BYTES + 1] {
        let mut p = base.clone();
        p["targetPackBytes"] = json!(n);
        cases.push(p);
    }
    for paths in [
        json!(["a.png", "a.png"]),
        json!(["a.png", "A.png"]),
        json!(["missing.png"]),
        json!(["../a.png"]),
        json!(["a.png/child"]),
    ] {
        let mut p = base.clone();
        p["groups"] = json!([{"name":"first","paths":paths}]);
        cases.push(p);
    }
    for groups in [
        json!([{"name":"one","paths":["a.png"]},{"name":"two","paths":["a.png"]}]),
        json!([{"name":"First","paths":["a.png"]},{"name":"first","paths":["b.png"]}]),
        json!([{"name":"DEFAULT","paths":["a.png"]}]),
        json!([{"name":"../unsafe","paths":["a.png"]}]),
        json!([{"name":"empty","paths":[]}]),
    ] {
        let mut p = base.clone();
        p["groups"] = groups;
        cases.push(p);
    }
    let mut p = base.clone();
    p["unknownAuthority"] = json!(true);
    cases.push(p);
    let mut p = base.clone();
    p["groups"] = json!([{"name":"x","paths":["a.png"],"prefix":"all"}]);
    cases.push(p);
    let mut p = base.clone();
    p["groups"] = json!(vec![json!({"name":"x","paths":["a.png"]}); MAX_PACKS]);
    cases.push(p);
    for (index, p) in cases.iter().enumerate() {
        let out = f.output(&format!("invalid-{index}"));
        assert!(repack(&source, &bytes(p), &out).is_err(), "case {index}");
        assert!(!out.exists(), "preflight wrote output for case {index}");
    }
    let duplicate = format!("{{\"schema\":\"{PLAN_SCHEMA}\",\"schema\":\"{PLAN_SCHEMA}\",\"sourceCatalogSha256\":\"{}\",\"targetPackBytes\":1024,\"groups\":[]}}", f.build.catalog_sha256);
    assert!(repack(&source, duplicate.as_bytes(), &f.output("duplicate-field")).is_err());
    assert!(!f.output("duplicate-field").exists());
}

#[test]
fn pack_count_is_preflighted_before_writing_any_object() {
    let names = (0..257).map(|i| format!("r{i}.png")).collect::<Vec<_>>();
    let data = (0..257u16).map(|i| i.to_le_bytes()).collect::<Vec<_>>();
    let files = names
        .iter()
        .zip(&data)
        .map(|(p, b)| (p.as_str(), b.as_slice()))
        .collect::<Vec<_>>();
    let f = Fixture::new(&files);
    let source = f.store();
    let mut p = f.plan();
    p["targetPackBytes"] = json!(96);
    assert!(preflight(&source, &bytes(&p)).is_err());
    assert!(repack(&source, &bytes(&p), &f.output("too-many")).is_err());
    assert!(!f.output("too-many").exists());
}

#[test]
fn existing_output_even_empty_and_source_descendants_are_rejected_without_changes() {
    let f = Fixture::new(&[("a.png", b"original")]);
    let source = f.store();
    let plan = bytes(&f.plan());
    let root = f.output("existing");
    fs::create_dir(&root).unwrap();
    assert!(repack(&source, &plan, &root).is_err());
    assert!(fs::read_dir(&root).unwrap().next().is_none());
    let file = f.output("existing-file");
    fs::write(&file, b"retain me").unwrap();
    assert!(repack(&source, &plan, &file).is_err());
    assert_eq!(fs::read(file).unwrap(), b"retain me");
    let nested = f.objects.join("forbidden-output");
    assert!(repack(&source, &plan, &nested).is_err());
    assert!(!nested.exists());
    assert!(repack(&source, &plan, &f.objects).is_err());
    let receipt = repack(&source, &plan, &f.output("complete")).unwrap();
    let before = fs::read(f.output("complete/REPACK-RECEIPT.json")).unwrap();
    assert!(repack(&source, &plan, &f.output("complete")).is_err());
    assert_eq!(
        before,
        fs::read(f.output("complete/REPACK-RECEIPT.json")).unwrap()
    );
    assert!(!receipt.bootstrap_authorized);
}

#[test]
fn a_late_source_content_failure_keeps_created_objects_and_never_claims_success() {
    let f = Fixture::new(&[("good.png", b"good payload"), ("bad.png", b"bad payload")]);
    let original = &f.build.catalog.packs[0];
    let original_bytes = fs::read(f.objects.join(original.file_name())).unwrap();
    let mut corrupt = original_bytes.clone();
    corrupt[f.build.catalog.resources["bad.png"].offset as usize] ^= 1;
    // An adversarial test catalog authenticates the changed whole object but
    // retains its lying per-resource digest/index. open succeeds, read must not.
    let changed = sha256(&corrupt);
    fs::write(
        f.objects
            .join(format!("{}.m2p", changed.to_ascii_lowercase())),
        corrupt,
    )
    .unwrap();
    let mut catalog = serde_json::to_value(&f.build.catalog).unwrap();
    catalog["packs"][0]["sha256"] = json!(changed);
    for r in catalog["resources"].as_object_mut().unwrap().values_mut() {
        r["packSha256"] = json!(changed);
    }
    let cb = bytes(&catalog);
    let trusted = sha256(&cb);
    let source = PackStore::open(&f.objects, &cb, &trusted).unwrap();
    let mut p = f.plan();
    p["sourceCatalogSha256"] = json!(trusted);
    p["groups"] = json!([{"name":"first-good","paths":["good.png"]}]);
    let out = f.output("retained-failure");
    assert!(repack(&source, &bytes(&p), &out).is_err());
    assert!(!out.join("catalog.json").exists() && !out.join("REPACK-RECEIPT.json").exists());
    let paths = fs::read_dir(out.join("objects/packs"))
        .unwrap()
        .map(|p| p.unwrap().path())
        .collect::<Vec<_>>();
    assert_eq!(paths.len(), 1);
    let retained = fs::read(&paths[0]).unwrap();
    assert!(repack(&source, &bytes(&p), &out).is_err());
    assert_eq!(retained, fs::read(&paths[0]).unwrap());
    assert_eq!(
        original_bytes,
        fs::read(f.objects.join(original.file_name())).unwrap()
    );
}

#[test]
fn safe_file_adapter_binds_old_catalog_and_rejects_hardlinked_json_inputs() {
    let f = Fixture::new(&[("a.png", b"one")]);
    let cp = f.output("old-catalog.json");
    write_catalog_new(&cp, &f.build.catalog_bytes, &f.build.catalog_sha256).unwrap();
    let pp = f.output("plan.json");
    fs::write(&pp, bytes(&f.plan())).unwrap();
    let out = f.output("file-adapter");
    assert!(repack_files(&cp, &"00".repeat(32), &f.objects, &pp, &out).is_err());
    assert!(!out.exists());
    let receipt = repack_files(&cp, &f.build.catalog_sha256, &f.objects, &pp, &out).unwrap();
    assert_eq!(
        receipt.preflight.source_catalog_sha256,
        f.build.catalog_sha256
    );
    fs::hard_link(&pp, f.output("linked-plan.json")).unwrap();
    let blocked = f.output("hardlink-blocked");
    assert!(repack_files(&cp, &f.build.catalog_sha256, &f.objects, &pp, &blocked).is_err());
    assert!(!blocked.exists());
}

#[cfg(unix)]
#[test]
fn output_symlink_ancestor_cannot_redirect_new_objects() {
    use std::os::unix::fs::symlink;
    let f = Fixture::new(&[("a.png", b"one")]);
    let source = f.store();
    let real = f.output("real");
    fs::create_dir(&real).unwrap();
    let link = f.output("link");
    symlink(&real, &link).unwrap();
    assert!(repack(&source, &bytes(&f.plan()), &link.join("fresh")).is_err());
    assert!(fs::read_dir(real).unwrap().next().is_none());
}
