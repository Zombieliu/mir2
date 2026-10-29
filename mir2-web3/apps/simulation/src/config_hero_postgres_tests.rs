use super::super::hero_registry::{SharedHeroCustody, SharedHeroState};
use super::super::CharacterRecord;
use super::*;
use mir2_protocol::{MirClass, MirGender};
use postgres::{Client, NoTls};

struct Database {
    client: Client,
    schema: String,
}
impl Database {
    fn new() -> Self {
        let url = std::env::var("MIR2_HERO_TEST_DATABASE_URL")
            .or_else(|_| std::env::var("MIR2_GUILD_TEST_DATABASE_URL"))
            .expect("an explicitly configured isolated test database is required");
        let mut client = Client::connect(&url, NoTls).unwrap();
        let schema = format!(
            "hero_pg_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        client
            .batch_execute(&format!(
                "CREATE SCHEMA {schema}; SET search_path TO {schema}"
            ))
            .unwrap();
        crate::db_projection::apply_migrations(&mut client).unwrap();
        Self { client, schema }
    }
    fn allocator(&mut self) -> (i32, i64) {
        let row = self
            .client
            .query_one(
                "SELECT high_watermark,store_version FROM shared_hero_allocator",
                &[],
            )
            .unwrap();
        (row.get(0), row.get(1))
    }
    fn owner(&mut self, account: &str) {
        self.client
            .execute(
                "INSERT INTO accounts(account_id,raw_json) VALUES($1,'{}')",
                &[&account],
            )
            .unwrap();
        self.client.execute("INSERT INTO characters(account_id,character_index,character_name,class,gender,level,raw_json) VALUES($1,0,$1,'Warrior','Male',22,'{}')", &[&account]).unwrap();
    }
    fn other_client(&self) -> Client {
        let url = std::env::var("MIR2_HERO_TEST_DATABASE_URL")
            .or_else(|_| std::env::var("MIR2_GUILD_TEST_DATABASE_URL"))
            .unwrap();
        let mut client = Client::connect(&url, NoTls).unwrap();
        client
            .batch_execute(&format!("SET search_path TO {}", self.schema))
            .unwrap();
        client
    }
}
impl Drop for Database {
    fn drop(&mut self) {
        if self.schema.starts_with("hero_pg_")
            && self
                .schema
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'_')
        {
            let _ = self
                .client
                .batch_execute(&format!("DROP SCHEMA {} CASCADE", self.schema));
        }
    }
}
fn record(id: i32, custody: SharedHeroCustody) -> SharedHeroRecord {
    SharedHeroRecord {
        id,
        revision: 1,
        custody,
        state: SharedHeroState {
            name: format!("Hero{id}"),
            class: MirClass::Warrior,
            gender: MirGender::Male,
            hair: 0,
            grade: 0,
            level: 22,
            experience: 9_007_199_254_740_993,
            inventory_items_json: vec![],
            equipment_items_json: vec![],
            inventory_capacity: 10,
            inventory_legacy_40: false,
            vitals: None,
            magics: vec![],
            seal_count: 0,
            auto_pot: false,
            auto_hp_percent: 50,
            auto_mp_percent: 50,
            hp_item_index: 0,
            mp_item_index: 0,
        },
    }
}
fn attached(account: &str) -> SharedHeroCustody {
    SharedHeroCustody::Attached {
        account_id: account.into(),
        character_index: 0,
        slot: 0,
        attachment_revision: 1,
    }
}
fn store() -> AccountStore {
    AccountStore::new(CharacterRecord {
        index: 0,
        name: "Owner".into(),
        level: 22,
        class: MirClass::Warrior,
        gender: MirGender::Male,
    })
}
fn create(db: &mut Database, records: Vec<SharedHeroRecord>) -> HeroSourceVersions {
    let (high, version) = db.allocator();
    let desired_high = records.iter().map(|r| r.id).max().unwrap();
    let mutations = records
        .into_iter()
        .map(|desired| {
            (
                desired.id,
                HeroMutation {
                    expected_version: None,
                    desired,
                },
            )
        })
        .collect();
    let mut tx = db.client.transaction().unwrap();
    let result = write_hero_mutations(
        &mut tx,
        &mutations,
        Some(&HeroAllocatorMutation {
            expected_version: Some(version),
            expected_high_watermark: high,
            desired_high_watermark: desired_high,
        }),
        AccountStoreDatabaseMode::SourceOfTruth,
    )
    .unwrap();
    tx.commit().unwrap();
    result
}

#[test]
#[ignore = "requires explicit isolated PostgreSQL test URL"]
fn hero_postgres_preserves_int64_and_unsigned_seal_and_rejects_stale_mirror() {
    let mut db = Database::new();
    let hero = record(
        1,
        SharedHeroCustody::Sealed {
            carrier_uid: u64::MAX,
        },
    );
    create(&mut db, vec![hero.clone()]);
    let mut loaded = store();
    load_heroes(&mut db.client, &mut loaded).unwrap();
    assert_eq!(loaded.shared_heroes[&1], hero);
    let stale = loaded.source_hero_versions[&1];
    let mut changed = hero.clone();
    changed.revision += 1;
    changed.state.name = "Changed".into();
    let mutations = BTreeMap::from([(
        1,
        HeroMutation {
            expected_version: Some(stale),
            desired: changed.clone(),
        },
    )]);
    let mut other = db.other_client();
    let mut tx = other.transaction().unwrap();
    write_hero_mutations(&mut tx, &mutations, None, AccountStoreDatabaseMode::Mirror).unwrap();
    tx.commit().unwrap();
    let mut tx = db.client.transaction().unwrap();
    assert!(
        write_hero_mutations(&mut tx, &mutations, None, AccountStoreDatabaseMode::Mirror)
            .unwrap_err()
            .contains("stale postgres Hero")
    );
    tx.rollback().unwrap();
    load_heroes(&mut db.client, &mut loaded).unwrap();
    assert_eq!(loaded.shared_heroes[&1], changed);
    let mut tx = db.client.transaction().unwrap();
    assert!(write_hero_mutations(
        &mut tx,
        &BTreeMap::new(),
        Some(&HeroAllocatorMutation {
            expected_version: loaded.source_hero_allocator_version,
            expected_high_watermark: 1,
            desired_high_watermark: 0,
        }),
        AccountStoreDatabaseMode::SourceOfTruth
    )
    .is_err());
    tx.rollback().unwrap();
    assert_eq!(db.allocator().0, 1);
}

#[test]
#[ignore = "requires explicit isolated PostgreSQL test URL"]
fn hero_postgres_source_and_allocator_roll_back_together() {
    let mut db = Database::new();
    db.owner("owner");
    let initial = db.allocator();
    let mutations = BTreeMap::from([(
        1,
        HeroMutation {
            expected_version: None,
            desired: record(1, attached("owner")),
        },
    )]);
    {
        let mut tx = db.client.transaction().unwrap();
        write_hero_mutations(
            &mut tx,
            &mutations,
            Some(&HeroAllocatorMutation {
                expected_version: Some(initial.1),
                expected_high_watermark: 0,
                desired_high_watermark: 1,
            }),
            AccountStoreDatabaseMode::Mirror,
        )
        .unwrap();
        tx.execute("UPDATE accounts SET raw_json=jsonb_set(raw_json,'{marker}','1') WHERE account_id='owner'", &[]).unwrap();
        tx.query_one("SELECT 1 / 0", &[])
            .expect_err("injected SQL failure aborts transaction");
        // Drop is rollback; neither allocator nor Hero can be published alone.
    }
    assert_eq!(db.allocator(), initial);
    assert!(db
        .client
        .query_one(
            "SELECT raw_json ->> 'marker' FROM accounts WHERE account_id='owner'",
            &[]
        )
        .unwrap()
        .get::<_, Option<String>>(0)
        .is_none());
    assert_eq!(
        db.client
            .query_one("SELECT count(*) FROM shared_heroes", &[])
            .unwrap()
            .get::<_, i64>(0),
        0
    );
    create(&mut db, vec![record(1, attached("owner"))]);
    assert_eq!(db.allocator().0, 1);
}

#[test]
#[ignore = "requires explicit isolated PostgreSQL test URL"]
fn hero_postgres_custody_uniques_and_character_fk_are_deferred_and_atomic() {
    let mut db = Database::new();
    db.owner("one");
    db.owner("two");
    create(
        &mut db,
        vec![record(1, attached("one")), record(2, attached("two"))],
    );
    let mut first = record(1, attached("two"));
    first.revision = 2;
    let mut second = record(2, attached("one"));
    second.revision = 2;
    let mut tx = db.client.transaction().unwrap();
    write_hero_mutations(
        &mut tx,
        &BTreeMap::from([
            (
                1,
                HeroMutation {
                    expected_version: Some(1),
                    desired: first,
                },
            ),
            (
                2,
                HeroMutation {
                    expected_version: Some(1),
                    desired: second,
                },
            ),
        ]),
        None,
        AccountStoreDatabaseMode::SourceOfTruth,
    )
    .unwrap();
    tx.commit().unwrap();
    for account in ["one", "missing-character"] {
        let before = db.allocator();
        let mut tx = db.client.transaction().unwrap();
        write_hero_mutations(
            &mut tx,
            &BTreeMap::from([(
                3,
                HeroMutation {
                    expected_version: None,
                    desired: record(3, attached(account)),
                },
            )]),
            Some(&HeroAllocatorMutation {
                expected_version: Some(before.1),
                expected_high_watermark: before.0,
                desired_high_watermark: 3,
            }),
            AccountStoreDatabaseMode::SourceOfTruth,
        )
        .unwrap();
        assert!(
            tx.commit().is_err(),
            "duplicate custody or absent real character must reject commit"
        );
        assert_eq!(db.allocator(), before);
    }
}

#[test]
#[ignore = "requires explicit isolated PostgreSQL test URL"]
fn hero_postgres_bad_source_does_not_replace_live_registry() {
    let mut db = Database::new();
    create(&mut db, vec![record(1, SharedHeroCustody::Released)]);
    let mut live = store();
    load_heroes(&mut db.client, &mut live).unwrap();
    let before = live.shared_heroes.clone();
    let versions = live.source_hero_versions.clone();
    db.client.execute("UPDATE shared_heroes SET raw_json=jsonb_set(raw_json,'{state,name}','\"\"') WHERE hero_id=1",&[]).unwrap();
    assert!(load_heroes(&mut db.client, &mut live).is_err());
    assert_eq!(live.shared_heroes, before);
    assert_eq!(live.source_hero_versions, versions);
}
