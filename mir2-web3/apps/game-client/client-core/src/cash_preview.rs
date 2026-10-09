//! Pure Crystal cash-shop preview layer policy. No world or item mutation.
//! Extracted from the native game_shop_dialog::preview_layers composition.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CashPreviewInput {
    pub item_type: u8,
    pub shape: i32,
    pub required_gender: u8,
    pub armour_shape: i32,
    pub female: bool,
    pub direction: u8,
    pub elapsed_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CashPreviewLayer {
    pub library: String,
    pub frame: u16,
}

fn layer(library: String, frame: u16) -> CashPreviewLayer {
    CashPreviewLayer { library, frame }
}

/// Preserve Native direction clamping, armour fallback/cast and layer ordering.
/// Metadata availability is deliberately the renderer's responsibility.
pub fn preview_layers(input: CashPreviewInput) -> Vec<CashPreviewLayer> {
    let Ok(raw_shape) = i16::try_from(input.shape) else {
        return vec![];
    };
    let Ok(shape) = u16::try_from(raw_shape) else {
        return vec![];
    };
    let d = u16::from(input.direction.clamp(1, 8) - 1);
    let anim = ((input.elapsed_ms / 150) % 6) as u16;
    if i16::try_from(input.armour_shape).is_err() { return vec![]; }
    let body_shape = input.armour_shape.max(0) as u16;
    let body = |frame| layer(format!("CArmour/{body_shape:02}"), frame);
    match input.item_type {
        1 => {
            let armour = body(if input.female { 840 } else { 32 } + d * 6 + anim);
            let frame = 32 + d * 6 + anim;
            if (100..200).contains(&shape) {
                let right = layer(format!("AWeaponR/{:02}", shape - 100), frame);
                let left = layer(format!("AWeaponL/{:02}", shape - 100), frame);
                if matches!(input.direction, 2 | 3) {
                    vec![left, armour, right]
                } else if matches!(input.direction, 7 | 8) {
                    vec![right, armour, left]
                } else {
                    vec![left, right, armour]
                }
            } else {
                let weapon = if shape >= 200 {
                    layer(format!("ARWeapon/{:02}", shape - 200), frame)
                } else {
                    layer(format!("CWeapon/{shape:02}"), frame)
                };
                if if shape >= 200 {
                    matches!(input.direction, 6..=8)
                } else {
                    matches!(input.direction, 2..=4)
                } {
                    vec![armour, weapon]
                } else {
                    vec![weapon, armour]
                }
            }
        }
        2 => vec![layer(
            format!("CArmour/{shape:02}"),
            if input.required_gender == 1 { 32 } else { 840 } + d * 6 + anim,
        )],
        19 => {
            let anim = ((input.elapsed_ms / 150) % 8) as u16;
            vec![
                layer(format!("Mount/{shape:02}"), 32 + d * 8 + anim),
                body(if input.female { 1256 } else { 448 } + d * 8 + anim),
            ]
        }
        37 => vec![layer(format!("Transform/{shape:02}"), 32 + d * 6 + anim)],
        _ => vec![],
    }
}

/// Local presentation only. The Web facade rejects directions outside 1..=8.
pub fn preview_turn(direction: u8, right: bool) -> u8 {
    let direction = direction.clamp(1, 8);
    if right { direction % 8 + 1 } else { (direction + 6) % 8 + 1 }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(item_type: u8, shape: i32, direction: u8, elapsed_ms: u64) -> CashPreviewInput {
        CashPreviewInput { item_type, shape, direction, elapsed_ms,
            required_gender: 1, armour_shape: 0, female: false }
    }

    #[test]
    fn native_weapon_gender_direction_and_clock_are_preserved() {
        assert_eq!(preview_layers(input(1, 9, 2, 150)), vec![
            layer("CArmour/00".into(), 39), layer("CWeapon/09".into(), 39)]);
        let mut female = input(1, 9, 8, 900);
        female.female = true;
        assert_eq!(preview_layers(female), vec![
            layer("CWeapon/09".into(), 74), layer("CArmour/00".into(), 882)]);
    }

    #[test]
    fn assassin_two_weapons_keep_native_depth_in_all_directions() {
        for direction in 1..=8 {
            let mut i = input(1, 109, direction, 0);
            i.armour_shape = 12;
            let layers = preview_layers(i);
            let libraries: Vec<_> = layers.iter().map(|l| l.library.as_str()).collect();
            assert_eq!(libraries, if matches!(direction, 2 | 3) {
                vec!["AWeaponL/09", "CArmour/12", "AWeaponR/09"]
            } else if matches!(direction, 7 | 8) {
                vec!["AWeaponR/09", "CArmour/12", "AWeaponL/09"]
            } else { vec!["AWeaponL/09", "AWeaponR/09", "CArmour/12"] });
        }
    }

    #[test]
    fn archer_armour_transform_and_mount_use_native_frames() {
        assert_eq!(preview_layers(input(1, 209, 6, 150))[0].library, "CArmour/00");
        assert_eq!(preview_layers(input(2, 7, 1, 150)), vec![layer("CArmour/07".into(), 33)]);
        let mut armour = input(2, 7, 1, 150);
        armour.required_gender = 0;
        assert_eq!(preview_layers(armour)[0].frame, 841);
        assert_eq!(preview_layers(input(37, 4, 8, 900))[0], layer("Transform/04".into(), 74));
        assert_eq!(preview_layers(input(19, 3, 1, 150)), vec![
            layer("Mount/03".into(), 33), layer("CArmour/00".into(), 449)]);
        let mut mount = input(19, 3, 8, 1200);
        mount.female = true;
        assert_eq!(preview_layers(mount)[1].frame, 1312);
    }

    #[test]
    fn unsupported_shape_and_local_eight_direction_turn_are_bounded() {
        assert!(preview_layers(input(0, 1, 1, 0)).is_empty());
        assert!(preview_layers(input(1, -1, 1, 0)).is_empty());
        assert!(preview_layers(input(1, 32768, 1, 0)).is_empty());
        assert!(preview_layers(input(1, 65536, 1, 0)).is_empty());
        let mut invalid_armour = input(1, 9, 1, 0);
        invalid_armour.armour_shape = 32768;
        assert!(preview_layers(invalid_armour).is_empty());
        assert_eq!(preview_turn(8, true), 1);
        assert_eq!(preview_turn(1, false), 8);
        for d in 1..=8 { assert_eq!(preview_turn(preview_turn(d, true), false), d); }
    }
}
