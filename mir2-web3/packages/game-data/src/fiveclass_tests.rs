//! Independent source-identity gates for the optional five-class profile.
use super::*;

const EXTRA_CLASSES: [MirClass; 2] = [MirClass::Assassin, MirClass::Archer];

fn source_class(flag: u8) -> Option<MirClass> {
    match flag {
        8 => Some(MirClass::Assassin),
        16 => Some(MirClass::Archer),
        _ => None,
    }
}

fn extra_source_books() -> Vec<CrystalItemTemplate> {
    crystal_item_manifest()
        .items
        .into_iter()
        .filter(|item| {
            item.item_type == 20
                && source_class(item.required_class).is_some()
                && item.required_type == 0
                && item.required_amount > 0
                && item.required_amount <= 50
        })
        .collect()
}

#[test]
fn fiveclass_profile_preserves_the_classic_world_and_exact_existing_items_skills() {
    let classic = platinum_176_profile();
    let five = platinum_fiveclass_profile();
    assert_eq!(classic.profile_id, "platinum_176");
    assert_eq!(classic.version, 28);
    assert_eq!(
        classic.allowed_classes,
        [MirClass::Warrior, MirClass::Wizard, MirClass::Taoist]
    );
    assert_eq!(classic.item_whitelist.len(), 204);
    assert_eq!(classic.skills.len(), 63);
    assert_eq!(five.profile_id, "platinum_fiveclass");
    assert_eq!(five.version, 2);
    assert_eq!(
        five.allowed_classes,
        [
            MirClass::Warrior,
            MirClass::Wizard,
            MirClass::Taoist,
            MirClass::Assassin,
            MirClass::Archer
        ]
    );
    assert_eq!(five.acceptance_level, 50);
    assert_eq!(five.item_whitelist.len(), 346);
    assert_eq!(five.skills.len(), 97);
    assert_eq!(
        &five.item_whitelist[..classic.item_whitelist.len()],
        classic.item_whitelist
    );
    assert_eq!(&five.skills[..classic.skills.len()], classic.skills);
    assert_eq!(
        &five.drop_overrides[..classic.drop_overrides.len()],
        classic.drop_overrides
    );
    assert_eq!(five.drop_overrides.len() - classic.drop_overrides.len(), 11);
    let mut classic_world = serde_json::to_value(&classic).unwrap();
    let mut five_world = serde_json::to_value(&five).unwrap();
    for field in [
        "profileId",
        "version",
        "source",
        "allowedClasses",
        "itemWhitelist",
        "skills",
        "dropOverrides",
    ] {
        classic_world.as_object_mut().unwrap().remove(field);
        five_world.as_object_mut().unwrap().remove(field);
    }
    assert_eq!(
        five_world, classic_world,
        "adding professions cannot mutate the classic world/rates"
    );
    assert_eq!(validate_content_profile(&classic), Ok(()));
    assert_eq!(validate_content_profile(&five), Ok(()));
}

#[test]
fn fiveclass_all_thirty_four_source_books_use_shape_class_and_book_level() {
    let profile = platinum_fiveclass_profile();
    let books = extra_source_books();
    assert_eq!(books.len(), 34);
    assert_eq!(
        books.iter().filter(|book| book.required_class == 8).count(),
        16
    );
    assert_eq!(
        books
            .iter()
            .filter(|book| book.required_class == 16)
            .count(),
        18
    );
    let mut identities = BTreeSet::new();
    for book in &books {
        let magic = crystal_magic_for_book(book)
            .unwrap_or_else(|| panic!("unresolved {} Shape {}", book.name, book.shape));
        let source_spell = Spell::try_from(u8::try_from(book.shape).unwrap()).unwrap();
        assert_eq!(Spell::from_crystal_name(&magic.spell), Some(source_spell));
        assert!(
            profile.item_whitelist.contains(&book.name),
            "book {} omitted",
            book.name
        );
        let rule = profile
            .skills
            .iter()
            .find(|rule| rule.spell == magic.spell)
            .unwrap();
        assert_eq!(
            rule.class,
            source_class(book.required_class).unwrap(),
            "{}",
            book.name
        );
        assert_eq!(
            rule.required_level,
            u16::from(book.required_amount),
            "{}",
            book.name
        );
        assert!(identities.insert(magic.spell));
    }
    let extra_rules = profile
        .skills
        .iter()
        .filter(|rule| EXTRA_CLASSES.contains(&rule.class))
        .collect::<Vec<_>>();
    assert_eq!(extra_rules.len(), books.len());
    assert!(extra_rules
        .iter()
        .all(|rule| identities.contains(&rule.spell)));
    assert!(profile
        .skills
        .iter()
        .all(|rule| (1..=50).contains(&rule.required_level)));
    // MagicInfo.Level1 controls later skill growth; it must not replace the
    // source book's independent learn gate (DoubleShot's book is level 11).
    assert_eq!(
        profile
            .skills
            .iter()
            .find(|rule| rule.spell == "DoubleShot")
            .unwrap()
            .required_level,
        11
    );
    assert_eq!(
        profile
            .skills
            .iter()
            .find(|rule| rule.spell == "FatalSword")
            .unwrap()
            .required_level,
        7
    );
}

#[test]
fn fiveclass_book_shape_resolves_source_typos_and_rejects_invalid_types_ids() {
    for (name, spell) in [
        ("CresentSlash", Spell::CrescentSlash),
        ("CatTognue", Spell::CatTongue),
        ("StoneTrap", Spell::Stonetrap),
    ] {
        let book = crystal_item_by_name(name).unwrap();
        let magic = crystal_magic_for_book(&book).unwrap();
        assert_eq!(Spell::from_crystal_name(&magic.spell), Some(spell));
        assert!(
            crystal_magic_by_spell(name).is_none(),
            "display aliases are not canonical spell IDs"
        );
        let mut renamed = book.clone();
        renamed.name = "FireBall".into();
        assert_eq!(
            crystal_magic_for_book(&renamed).unwrap().spell,
            magic.spell,
            "a display name cannot inject another spell"
        );
    }
    let mut book = crystal_item_by_name("FatalSword").unwrap();
    for shape in [-1, 256, i16::MAX] {
        book.shape = shape;
        assert!(
            crystal_magic_for_book(&book).is_none(),
            "invalid Shape {shape}"
        );
    }
    book.shape = Spell::FatalSword as i16;
    book.item_type = 0;
    assert!(
        crystal_magic_for_book(&book).is_none(),
        "equipment cannot be consumed as a book"
    );
}

#[test]
fn fiveclass_item_extension_equals_source_books_and_usable_equipment_through_fifty() {
    let classic = platinum_176_profile();
    let five = platinum_fiveclass_profile();
    let mut expected = classic
        .item_whitelist
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    expected.extend(extra_source_books().into_iter().map(|book| book.name));
    for item in crystal_item_manifest().items {
        if source_class(item.required_class).is_some()
            && item.item_type != 20
            && item.required_type == 0
            && item.required_amount <= 50
            && item.price > 0
            && item.name.bytes().all(|byte| (0x20..=0x7e).contains(&byte))
        {
            expected.insert(item.name);
        }
    }
    assert_eq!(five.item_whitelist.len(), expected.len());
    assert_eq!(
        five.item_whitelist.iter().cloned().collect::<BTreeSet<_>>(),
        expected
    );
    assert!(five
        .item_whitelist
        .iter()
        .filter(|name| !classic.item_whitelist.contains(name))
        .all(|name| crystal_item_by_name(name).is_some()));
}

#[test]
fn fiveclass_validation_keeps_profile_class_admission_separate_and_book_backed() {
    let mut classic = platinum_176_profile();
    classic.allowed_classes.extend(EXTRA_CLASSES);
    assert!(validate_content_profile(&classic).is_err());
    let mut five = platinum_fiveclass_profile();
    five.allowed_classes.swap(3, 4);
    assert!(
        validate_content_profile(&five).is_err(),
        "canonical class order is part of the profile"
    );
    let mut five = platinum_fiveclass_profile();
    five.item_whitelist.retain(|name| name != "CresentSlash");
    assert!(validate_content_profile(&five)
        .unwrap_err()
        .iter()
        .any(|error| error.contains("CrescentSlash") && error.contains("book")));
    let mut five = platinum_fiveclass_profile();
    five.skills
        .iter_mut()
        .find(|rule| rule.spell == "FatalSword")
        .unwrap()
        .required_level = 51;
    assert!(validate_content_profile(&five).is_err());
    let mut five = platinum_fiveclass_profile();
    five.skills
        .iter_mut()
        .find(|rule| rule.spell == "FatalSword")
        .unwrap()
        .class = MirClass::Warrior;
    assert!(
        validate_content_profile(&five).is_err(),
        "a source Assassin book cannot authorize a Warrior skill"
    );
    let mut five = platinum_fiveclass_profile();
    five.skills
        .iter_mut()
        .find(|rule| rule.spell == "FatalSword")
        .unwrap()
        .required_level = 1;
    assert!(
        validate_content_profile(&five).is_err(),
        "the source book's learn gate cannot be lowered by profile metadata"
    );
}

#[test]
fn fiveclass_bundle_binds_new_profile_and_original_classic_profile_identity() {
    let profile = platinum_fiveclass_profile();
    let bundle = platinum_fiveclass_profile_bundle();
    let classic = platinum_176_profile_bundle();
    assert_eq!(bundle.schema, "mir2-content-profile-bundle/1");
    assert_eq!(bundle.profile_id, profile.profile_id);
    assert_eq!(bundle.profile_version, profile.version);
    assert_eq!(bundle.acceptance_level, 50);
    assert_eq!(bundle.source, profile.source);
    assert_eq!(bundle.hash_algorithm, "sha256");
    assert_eq!(bundle.content_hash.len(), 64);
    assert!(bundle
        .content_hash
        .bytes()
        .all(|byte| byte.is_ascii_hexdigit()));
    assert_eq!(
        bundle.files.len(),
        15,
        "fourteen dependencies plus the retained base profile"
    );
    assert_eq!(
        bundle
            .files
            .iter()
            .map(|file| &file.path)
            .collect::<BTreeSet<_>>()
            .len(),
        15
    );
    let base_path = "packages/game-data/data/content_profiles/platinum_176.json";
    let base = bundle
        .files
        .iter()
        .find(|file| file.path == base_path)
        .unwrap();
    assert_eq!(
        base,
        classic
            .files
            .iter()
            .find(|file| file.path == base_path)
            .unwrap()
    );
    let own = bundle
        .files
        .iter()
        .find(|file| {
            file.path == "packages/game-data/data/content_profiles/platinum_fiveclass.json"
        })
        .unwrap();
    assert_eq!(
        own.bytes,
        include_bytes!("../data/content_profiles/platinum_fiveclass.json").len() as u64
    );
    assert_eq!(bundle.source_data, classic.source_data);
    assert_eq!(bundle.summary.maps, profile.map_whitelist.len());
    assert_eq!(bundle.summary.monsters, profile.monster_whitelist.len());
    assert_eq!(
        bundle.summary.native_maps,
        profile.map_whitelist.len() + profile.native_resource_only_maps.len()
    );
    assert_eq!(bundle.summary.items, 346);
    assert_eq!(bundle.summary.skills, 97);
    assert_eq!(bundle.summary.drop_overrides, profile.drop_overrides.len());
    assert_eq!(
        bundle.summary.npc_scripts,
        profile.npc_script_whitelist.len()
    );
    assert_eq!(
        bundle.summary.respawn_overrides,
        profile.respawn_overrides.len()
    );
    assert_eq!(
        bundle.summary.quest_prerequisite_overrides,
        profile.quest_prerequisite_overrides.len()
    );
    assert_eq!(
        bundle.summary.quest_reward_overrides,
        profile.quest_reward_overrides.len()
    );
}
