export function itemCount(snapshot, itemIndex) {
  return ['inventoryItems', 'beltItems', 'equipmentItems'].flatMap(k => snapshot[k] ?? [])
    .filter(i => Number(i.tooltipSource?.info?.item_index ?? String(i.key).replace('crystal-item-', '')) === itemIndex)
    .reduce((sum, i) => sum + Number(i.quantity ?? 1), 0);
}

export function expectedItemRewards(quest, selectedItemIndex) {
  return [...quest.rewards.fixedItems, ...quest.rewards.selectableItems.filter(i => i.selectionIndex === selectedItemIndex)]
    .filter(i => i.count > 0);
}

export function verifyItemRewards(before, after, expected) {
  const grouped = new Map();
  for (const item of expected) grouped.set(item.itemIndex, { ...item, count: item.count + (grouped.get(item.itemIndex)?.count ?? 0) });
  return [...grouped.values()].map(item => {
    const previous = itemCount(before, item.itemIndex), current = itemCount(after, item.itemIndex);
    if (current - previous < item.count) throw new Error(`Reward missing: ${item.itemName}, expected +${item.count}, observed ${current - previous}`);
    return { itemIndex: item.itemIndex, name: item.itemName, expected: item.count, before: previous, after: current };
  });
}
