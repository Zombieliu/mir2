//! Crystal frame-set metadata shared by the native entity presenter.
//!
//! The Web client already treats `original-ui/frame-sets.generated.json` as
//! authoritative for monster and NPC animation geometry. Loading the same
//! generated contract here prevents Windows from applying one generic table to
//! libraries whose action counts, skips, or intervals differ.

use std::collections::HashMap;
use std::fs;
use std::sync::OnceLock;

use mir2_bevy_runtime::entity_animation::{
    AnimationAction, AnimationCatalog, EntityKind, FrameDescriptor,
};
use serde_json::Value;

use crate::assets;

static FRAME_SET_CATALOG: OnceLock<NativeFrameSetCatalog> = OnceLock::new();
static MOUNTED_PLAYER_CATALOG: OnceLock<AnimationCatalog> = OnceLock::new();
static ARCHER_PLAYER_CATALOG: OnceLock<AnimationCatalog> = OnceLock::new();

#[derive(Debug, Default)]
struct NativeFrameSetCatalog {
    libraries: HashMap<String, AnimationCatalog>,
}

impl NativeFrameSetCatalog {
    fn parse(payload: &Value) -> Self {
        let mut libraries = HashMap::new();
        let Some(entries) = payload.get("libraries").and_then(Value::as_object) else {
            return Self { libraries };
        };

        for (library, entry) in entries {
            let Some(actions) = entry.get("actions").and_then(Value::as_array) else {
                continue;
            };
            let descriptors = actions
                .iter()
                .filter_map(parse_source_action)
                .collect::<HashMap<_, _>>();
            let Some(catalog) = build_animation_catalog(&descriptors) else {
                continue;
            };
            libraries.insert(normalize_library_key(library), catalog);
        }

        Self { libraries }
    }

    fn catalog_for(&self, kind: EntityKind, library: &str) -> AnimationCatalog {
        if kind == EntityKind::Player {
            return AnimationCatalog::crystal_player();
        }
        self.libraries
            .get(&normalize_library_key(library))
            .cloned()
            .unwrap_or_else(|| AnimationCatalog::crystal_default(kind))
    }
}

#[derive(Clone, Copy, Debug)]
struct SourceAction {
    descriptor: FrameDescriptor,
    reverse: bool,
}

pub fn animation_catalog_for(kind: EntityKind, library: &str, mounted: bool) -> AnimationCatalog {
    if kind == EntityKind::Player && mounted {
        return MOUNTED_PLAYER_CATALOG
            .get_or_init(build_mounted_player_catalog)
            .clone();
    }
    if kind == EntityKind::Player && normalize_library_key(library).starts_with("ARArmour/") {
        return ARCHER_PLAYER_CATALOG
            .get_or_init(build_archer_player_catalog)
            .clone();
    }
    FRAME_SET_CATALOG
        .get_or_init(load_frame_set_catalog)
        .catalog_for(kind, library)
}

fn build_archer_player_catalog() -> AnimationCatalog {
    let mut catalog = AnimationCatalog::crystal_player();
    catalog
        .insert(
            AnimationAction::Walking,
            FrameDescriptor::from_crystal(0, 6, 0, 100, false),
        )
        .expect("Crystal archer walking descriptor is valid");
    catalog
        .insert(
            AnimationAction::Running,
            FrameDescriptor::from_crystal(48, 6, 0, 100, false),
        )
        .expect("Crystal archer running descriptor is valid");
    catalog
}

fn build_mounted_player_catalog() -> AnimationCatalog {
    let mut catalog = AnimationCatalog::new();
    let entries = [
        (
            AnimationAction::Standing,
            FrameDescriptor::from_crystal(416, 4, 0, 500, false),
        ),
        (
            AnimationAction::Harvest,
            FrameDescriptor::from_crystal(416, 4, 0, 500, false),
        ),
        (
            AnimationAction::Walking,
            FrameDescriptor::from_crystal(448, 8, 0, 100, false),
        ),
        (
            AnimationAction::Running,
            FrameDescriptor::from_crystal(512, 6, 0, 100, false),
        ),
        (
            AnimationAction::Struck,
            FrameDescriptor::from_crystal(560, 3, 0, 100, false),
        ),
        (
            AnimationAction::Die,
            FrameDescriptor::from_crystal(416, 1, 3, 500, false),
        ),
        (
            AnimationAction::Dead,
            FrameDescriptor::from_crystal(416, 1, 3, 1000, false),
        ),
        (
            AnimationAction::Revive,
            FrameDescriptor::from_crystal(416, 1, 3, 100, false),
        ),
    ];
    for (action, descriptor) in entries {
        catalog
            .insert(action, descriptor)
            .expect("mounted Crystal descriptors are valid");
    }
    let mounted_attack = FrameDescriptor::from_crystal(584, 6, 0, 100, false);
    for action in [
        AnimationAction::Attack1,
        AnimationAction::Attack2,
        AnimationAction::Attack3,
        AnimationAction::Attack4,
        AnimationAction::AttackRange1,
        AnimationAction::Spell,
    ] {
        catalog
            .insert(action, mounted_attack)
            .expect("mounted Crystal attack descriptor is valid");
    }
    catalog
}

fn load_frame_set_catalog() -> NativeFrameSetCatalog {
    let Some(path) = assets::asset_path("original-ui/frame-sets.generated.json") else {
        eprintln!("[frame-sets] generated frame-set catalog is unavailable; using defaults");
        return NativeFrameSetCatalog::default();
    };
    let Ok(source) = fs::read_to_string(&path) else {
        eprintln!("[frame-sets] failed to read {path:?}; using defaults");
        return NativeFrameSetCatalog::default();
    };
    let Ok(payload) = serde_json::from_str::<Value>(&source) else {
        eprintln!("[frame-sets] failed to parse {path:?}; using defaults");
        return NativeFrameSetCatalog::default();
    };
    let catalog = NativeFrameSetCatalog::parse(&payload);
    eprintln!(
        "[frame-sets] loaded {} Crystal library catalogs from {path:?}",
        catalog.libraries.len()
    );
    catalog
}

fn normalize_library_key(library: &str) -> String {
    let trimmed = library.trim().trim_matches('/');
    trimmed
        .strip_prefix("original-ui/")
        .unwrap_or(trimmed)
        .replace('\\', "/")
}

fn parse_source_action(value: &Value) -> Option<(String, SourceAction)> {
    let action_name = value.get("actionName")?.as_str()?.to_owned();
    let start = i32::try_from(value.get("start")?.as_i64()?).ok()?;
    let count = u16::try_from(value.get("count")?.as_u64()?).ok()?;
    let skip = i16::try_from(value.get("skip")?.as_i64()?).ok()?;
    let interval = value.get("interval")?.as_u64()?;
    let reverse = value
        .get("reverse")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if count == 0 || interval == 0 {
        return None;
    }
    Some((
        action_name,
        SourceAction {
            descriptor: FrameDescriptor::from_crystal(start, count, skip, interval, reverse),
            reverse,
        },
    ))
}

fn build_animation_catalog(source: &HashMap<String, SourceAction>) -> Option<AnimationCatalog> {
    source.get("Standing")?;
    let mut catalog = AnimationCatalog::new();
    for action in [
        AnimationAction::Standing,
        AnimationAction::Harvest,
        AnimationAction::Walking,
        AnimationAction::Running,
        AnimationAction::Attack1,
        AnimationAction::Attack2,
        AnimationAction::Attack3,
        AnimationAction::Attack4,
        AnimationAction::AttackRange1,
        AnimationAction::Spell,
        AnimationAction::Struck,
        AnimationAction::Die,
        AnimationAction::Dead,
        AnimationAction::Revive,
    ] {
        let (source_name, source_action) = resolve_source_action(source, action)?;
        let mut descriptor = source_action.descriptor;
        // Web reverses a fallback Die/Standing cycle during revive, but keeps
        // an explicit Revive action's source flag.
        if action == AnimationAction::Revive && source_name != "Revive" {
            descriptor.reverse = true;
        } else {
            descriptor.reverse = source_action.reverse;
        }
        catalog.insert(action, descriptor).ok()?;
    }
    Some(catalog)
}

fn resolve_source_action<'a>(
    source: &'a HashMap<String, SourceAction>,
    action: AnimationAction,
) -> Option<(&'a str, &'a SourceAction)> {
    let candidates: &[&str] = match action {
        AnimationAction::Standing => &["Standing"],
        AnimationAction::Harvest => &["Harvest", "Standing"],
        AnimationAction::Walking => &["Walking", "Standing"],
        AnimationAction::Running => &["Running", "Walking", "Standing"],
        AnimationAction::Attack1 => &["Attack1", "Standing"],
        AnimationAction::Attack2 => &["Attack2", "Attack1", "Standing"],
        AnimationAction::Attack3 => &["Attack3", "Attack1", "Standing"],
        AnimationAction::Attack4 => &["Attack4", "Attack1", "Standing"],
        AnimationAction::AttackRange1 => &["AttackRange1", "Attack1", "Standing"],
        AnimationAction::Spell => &["Spell", "AttackRange1", "Attack1", "Standing"],
        AnimationAction::Struck => &["Struck", "Standing"],
        AnimationAction::Die => &["Die", "Dead", "Standing"],
        AnimationAction::Dead => &["Dead", "Die", "Standing"],
        AnimationAction::Revive => &["Revive", "Die", "Standing"],
    };
    candidates
        .iter()
        .find_map(|name| source.get(*name).map(|action| (*name, action)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn library_catalog_uses_exact_counts_skips_intervals_and_fallbacks() {
        let parsed = NativeFrameSetCatalog::parse(&json!({
            "libraries": {
                "Monster/777": {
                    "actions": [
                        {
                            "actionName": "Standing", "start": 10, "count": 2,
                            "skip": -2, "interval": 400, "reverse": false
                        },
                        {
                            "actionName": "Walking", "start": 20, "count": 3,
                            "skip": 1, "interval": 125, "reverse": false
                        },
                        {
                            "actionName": "Die", "start": 40, "count": 5,
                            "skip": -1, "interval": 90, "reverse": false
                        }
                    ]
                }
            }
        }));
        let catalog = parsed.catalog_for(EntityKind::Monster, "/original-ui/Monster/777");

        assert_eq!(
            catalog.descriptor(AnimationAction::Standing),
            Some(&FrameDescriptor::from_crystal(10, 2, -2, 400, false))
        );
        assert_eq!(
            catalog.descriptor(AnimationAction::Walking),
            Some(&FrameDescriptor::from_crystal(20, 3, 1, 125, false))
        );
        assert_eq!(
            catalog.descriptor(AnimationAction::Running),
            catalog.descriptor(AnimationAction::Walking)
        );
        assert_eq!(
            catalog.descriptor(AnimationAction::Attack1),
            catalog.descriptor(AnimationAction::Standing)
        );
        assert_eq!(
            catalog.descriptor(AnimationAction::Dead),
            catalog.descriptor(AnimationAction::Die)
        );
        assert!(catalog
            .descriptor(AnimationAction::Revive)
            .is_some_and(|descriptor| descriptor.reverse));
    }

    #[test]
    fn explicit_revive_preserves_source_reverse_flag() {
        let parsed = NativeFrameSetCatalog::parse(&json!({
            "libraries": {
                "Dragon": {
                    "actions": [
                        {
                            "actionName": "Standing", "start": 0, "count": 10,
                            "skip": -10, "interval": 1000, "reverse": false
                        },
                        {
                            "actionName": "Revive", "start": 42, "count": 7,
                            "skip": -7, "interval": 120, "reverse": false
                        }
                    ]
                }
            }
        }));
        let catalog = parsed.catalog_for(EntityKind::Monster, "Dragon");
        assert!(
            !catalog
                .descriptor(AnimationAction::Revive)
                .expect("revive descriptor")
                .reverse
        );
        assert_eq!(
            catalog
                .descriptor(AnimationAction::Standing)
                .expect("standing")
                .direction_stride,
            0
        );
    }

    #[test]
    fn malformed_or_unknown_library_uses_kind_default() {
        let parsed = NativeFrameSetCatalog::parse(&json!({
            "libraries": {
                "Monster/invalid": {"actions": [{
                    "actionName": "Standing", "start": 0, "count": 0,
                    "skip": 0, "interval": 500
                }]}
            }
        }));
        assert_eq!(
            parsed.catalog_for(EntityKind::Monster, "Monster/invalid"),
            AnimationCatalog::crystal_monster()
        );
        assert_eq!(
            parsed.catalog_for(EntityKind::Player, "Monster/777"),
            AnimationCatalog::crystal_player()
        );
    }

    #[test]
    fn mounted_player_uses_crystal_mounted_body_offsets() {
        let catalog = animation_catalog_for(EntityKind::Player, "CArmour/00", true);
        assert_eq!(
            catalog.descriptor(AnimationAction::Walking),
            Some(&FrameDescriptor::from_crystal(448, 8, 0, 100, false))
        );
        assert_eq!(
            catalog.descriptor(AnimationAction::AttackRange1),
            Some(&FrameDescriptor::from_crystal(584, 6, 0, 100, false))
        );
    }

    #[test]
    fn archer_alt_library_uses_its_compact_walk_and_run_tables() {
        let catalog = animation_catalog_for(EntityKind::Player, "ARArmour/00", false);
        assert_eq!(
            catalog.descriptor(AnimationAction::Walking),
            Some(&FrameDescriptor::from_crystal(0, 6, 0, 100, false))
        );
        assert_eq!(
            catalog.descriptor(AnimationAction::Running),
            Some(&FrameDescriptor::from_crystal(48, 6, 0, 100, false))
        );
        assert_eq!(
            catalog.descriptor(AnimationAction::AttackRange1),
            AnimationCatalog::crystal_player().descriptor(AnimationAction::AttackRange1)
        );
    }
}
