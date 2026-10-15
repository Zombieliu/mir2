use super::monster_body_library;

#[test]
fn platinum_gate_images_use_crystal_gate_libraries() {
    // Crystal Shared/Enums.cs and MonsterObject.Load: the four profile gates
    // are SabukGate, PalaceWallLeft, PalaceWall1, and PalaceWall2.
    for (image, library) in [
        (950, "Gate/00"),
        (951, "Gate/01"),
        (952, "Gate/02"),
        (953, "Gate/03"),
    ] {
        assert_eq!(monster_body_library(image), library);
    }
}

#[test]
fn gate_mapping_does_not_reclassify_other_images() {
    for image in [0, 5, 139, 247, 900, 902, 904, 949, 954, 10000, 10014] {
        assert_eq!(monster_body_library(image), format!("Monster/{image:03}"));
    }
}
