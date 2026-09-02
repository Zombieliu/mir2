//! Crystal `Shared/Data/ItemData.cs:641-681`, `UserItem.Image`.
//!
//! This is the image of a concrete stack, not a replacement for ItemInfo.Image.
//! QuestCell, GameShopCell and crafting shadow cells deliberately draw the
//! catalogue image instead. Do not apply this selector to those previews.

pub const fn crystal_user_item_image(
    item_type: u8,
    shape: i16,
    stack_size: u16,
    catalogue_image: u16,
    count: u32,
) -> u16 {
    // ItemType.Amulet includes both poisons. Neither names, rarity nor the
    // viewer's GetRealItem variant participate in Crystal's image property.
    if item_type != 8 || stack_size == 0 {
        return catalogue_image;
    }
    match (shape, count) {
        (0, 300..) => 3662,
        (0, 200..=299) => 3661,
        (0, _) => 3660,
        (1, 150..) => 3675,
        (1, 100..=149) => 2960,
        (1, 50..=99) => 3674,
        (1, _) => 3673,
        (2, 150..) => 3672,
        (2, 100..=149) => 2961,
        (2, 50..=99) => 3671,
        (2, _) => 3670,
        _ => catalogue_image,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crystal_item_manifest;

    #[test]
    fn every_wire_count_matches_the_source_image_bands() {
        let bands: &[(i16, &[(u32, u32, u16)])] = &[
            (0, &[(0, 199, 3660), (200, 299, 3661), (300, 65535, 3662)]),
            (
                1,
                &[
                    (0, 49, 3673),
                    (50, 99, 3674),
                    (100, 149, 2960),
                    (150, 65535, 3675),
                ],
            ),
            (
                2,
                &[
                    (0, 49, 3670),
                    (50, 99, 3671),
                    (100, 149, 2961),
                    (150, 65535, 3672),
                ],
            ),
        ];
        for &(shape, ranges) in bands {
            for &(first, last, image) in ranges {
                for count in first..=last {
                    assert_eq!(crystal_user_item_image(8, shape, 500, 999, count), image);
                }
            }
        }
    }

    #[test]
    fn guards_and_non_stack_previews_retain_the_catalogue_image() {
        for item_type in 0..=u8::MAX {
            for shape in [-1, 0, 1, 2, 3, i16::MAX] {
                for count in [
                    0, 1, 49, 50, 99, 100, 149, 150, 199, 200, 299, 300, 500, 65535,
                ] {
                    assert_eq!(
                        crystal_user_item_image(item_type, shape, 0, 277, count),
                        277
                    );
                    if item_type != 8 || !(0..=2).contains(&shape) {
                        assert_eq!(
                            crystal_user_item_image(item_type, shape, 500, 277, count),
                            277
                        );
                    }
                }
            }
        }
        // Crystal checks StackSize > 0, not > 1. Count is not clamped to it.
        assert_eq!(crystal_user_item_image(8, 0, 1, 270, 300), 3662);
        assert_eq!(crystal_user_item_image(8, 1, 1, 259, u32::MAX), 3675);
    }

    #[test]
    fn complete_catalogue_only_changes_the_three_source_stack_shapes() {
        let manifest = crystal_item_manifest();
        let mut affected = Vec::new();
        for info in manifest.items {
            let image = crystal_user_item_image(
                info.item_type,
                info.shape,
                info.stack_size,
                info.image,
                300,
            );
            if info.item_type == 8 && info.stack_size > 0 && (0..=2).contains(&info.shape) {
                affected.push(info.item_index);
                assert_ne!(image, info.image);
            } else {
                assert_eq!(image, info.image, "{} ({})", info.name, info.item_index);
            }
        }
        assert_eq!(affected, [710, 711, 712]);
    }
}
