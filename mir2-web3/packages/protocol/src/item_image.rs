//! Crystal `Shared/Data/ItemData.cs:641-681`, `UserItem.Image`.
//!
//! This is the image of a concrete stack, not a replacement for ItemInfo.Image.
//! QuestCell, GameShopCell and crafting shadow cells deliberately draw the
//! catalogue image instead. Do not apply this selector to those previews.
//!
//! Keep this source-data rule independent of the generated game catalogue so
//! both renderer-neutral clients and the server can use the same property.

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
