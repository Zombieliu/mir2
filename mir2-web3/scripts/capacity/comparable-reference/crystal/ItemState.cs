using System.Text.Json;

namespace ComparableCrystal;

internal static class ItemState
{
    private static string ItemHash(UserItem item)
    {
        using var stream = new MemoryStream(); using var writer = new BinaryWriter(stream); item.Save(writer); writer.Flush();
        return Convert.ToHexString(System.Security.Cryptography.SHA256.HashData(stream.ToArray())).ToLowerInvariant();
    }
    public static object[] Slots(UserItem[] inventory, UserItem[] equipment, UserItem[]? quest = null) => new[] { ("inventory", inventory), ("equipment", equipment), ("questInventory", quest ?? Array.Empty<UserItem>()) }
        .SelectMany(grid => grid.Item2.Select((item, slot) => new { item, slot }).Where(x => x.item is not null)
            .Select(x => (object)new { grid = grid.Item1, slot = x.slot, uniqueId = x.item.UniqueID, itemIndex = x.item.ItemIndex, count = x.item.Count,
                currentDura = x.item.CurrentDura, maxDura = x.item.MaxDura, originalUserItemSaveSha256 = ItemHash(x.item) })).ToArray();
    private static object Owned(UserItem[] inventory, UserItem[] equipment, UserItem[]? quest, uint gold, uint credit) => new
    {
        slots = Slots(inventory, equipment, quest), gold, credit,
        capacity = new { inventoryIncludingBelt = inventory.Length, equipment = equipment.Length, questInventory = quest?.Length ?? 0, belt = Math.Min(6, inventory.Length), bag = Math.Max(0, inventory.Length - 6) },
        beltRule = "original Inventory indices0..5; alias of inventory, not duplicated item ownership"
    };
    public static object Describe(UserItem[] inventory, UserItem[] equipment, UserItem[]? quest = null, uint gold = 0, uint credit = 0)
    {
        var owned = Owned(inventory, equipment, quest, gold, credit);
        return new { owned, fingerprintSha256 = Files.HashText(JsonSerializer.Serialize(owned, Files.Compact)), scope = "all Inventory/Equipment/QuestInventory slots, complete original UserItem.Save fields, gold/credit/capacities; belt aliases inventory0..5" };
    }
    public static string Fingerprint(UserItem[] inventory, UserItem[] equipment, UserItem[]? quest = null, uint gold = 0, uint credit = 0) => Files.HashText(JsonSerializer.Serialize(Owned(inventory, equipment, quest, gold, credit), Files.Compact));
}
