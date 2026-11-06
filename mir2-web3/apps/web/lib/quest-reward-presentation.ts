/** A catalogue preview, never an authoritative inventory instance. */
export type QuestRewardTooltipSource = {
  info: Record<string, unknown> & { item_index: number; durability: number; slots: number };
  realInfo: null;
  userItem: {
    unique_id: 0;
    item_index: number;
    current_dura: number;
    max_dura: number;
    count: 0;
    soul_bound_id: -1;
    identified: false;
    cursed: false;
    slots: null[];
    gem_count: 0;
    added_stats: [];
    awake_type: 0;
    awake_values: [];
    refined_value: 0;
    refine_added: 0;
    refine_success_chance: 0;
    wedding_ring: -1;
    expire_info: null;
    rental_information: null;
    is_shop_item: false;
    sealed_info: null;
    gm_made: false;
  };
  socketInfos: [];
  realSocketInfos: [];
};

export type QuestRewardPresentationItem = {
  name: string;
  icon?: number;
  count?: number;
  itemIndex?: number;
  selectable?: boolean;
  selectionIndex?: number;
  tooltipSource?: QuestRewardTooltipSource;
};

export type QuestRewardsPresentation = {
  gold?: number;
  experience?: number;
  credit?: number;
  items?: QuestRewardPresentationItem[];
  selectItems?: QuestRewardPresentationItem[];
};

function record(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function unsignedInteger(value: unknown, maximum: number): value is number {
  return typeof value === "number" && Number.isSafeInteger(value) && value >= 0 && value <= maximum;
}

function previewSource(rawItem: unknown, itemIndex: number | undefined): QuestRewardTooltipSource | undefined {
  if (!record(rawItem) || !unsignedInteger(itemIndex, 0x7fff_ffff)) return undefined;
  // The wire ItemInfo uses `index`; the shared tooltip model uses `item_index`.
  // If both are present, neither is allowed to contradict the selected reward.
  const indices = [rawItem.index, rawItem.item_index].filter((value) => value !== undefined);
  if (indices.length === 0 || indices.some((value) => !unsignedInteger(value, 0x7fff_ffff) || value !== itemIndex)) {
    return undefined;
  }
  if (!unsignedInteger(rawItem.durability, 0xffff) || !unsignedInteger(rawItem.slots, 0xff)) return undefined;

  return {
    // Keep every original ItemInfo field, including unknown additive metadata.
    info: { ...rawItem, item_index: itemIndex, durability: rawItem.durability, slots: rawItem.slots },
    // Class/level variants require the native catalogue resolver and viewer.
    // Keeping this absent lets the shared label report a partial source.
    realInfo: null,
    // Native QuestCell constructs new UserItem(info), leaves count at zero,
    // and paints the actual reward count separately. These are preview defaults,
    // including UID 0, and must never be used to address an owned item.
    userItem: {
      unique_id: 0,
      item_index: itemIndex,
      current_dura: rawItem.durability,
      max_dura: rawItem.durability,
      count: 0,
      soul_bound_id: -1,
      identified: false,
      cursed: false,
      slots: Array.from({ length: rawItem.slots }, () => null),
      gem_count: 0,
      added_stats: [],
      awake_type: 0,
      awake_values: [],
      refined_value: 0,
      refine_added: 0,
      refine_success_chance: 0,
      wedding_ring: -1,
      expire_info: null,
      rental_information: null,
      is_shop_item: false,
      sealed_info: null,
      gm_made: false,
    },
    socketInfos: [],
    realSocketInfos: [],
  };
}

function attachGroup(
  items: QuestRewardPresentationItem[] | undefined,
  rawGroup: unknown,
  selectable: boolean,
): QuestRewardPresentationItem[] | undefined {
  if (!items) return undefined;
  // parseQuestRewards may have filtered malformed rows. A length change means
  // positions no longer prove correspondence, even when template IDs repeat.
  const aligned = Array.isArray(rawGroup) && rawGroup.length === items.length;
  return items.map((item, index) => {
    // Do not inherit a source from a previous definition when raw data disappears.
    const { tooltipSource: _previous, ...presentation } = item;
    const rawReward: unknown = aligned ? rawGroup[index] : undefined;
    const positionMatches = !selectable || item.selectionIndex === undefined || item.selectionIndex === index;
    const tooltipSource = positionMatches && record(rawReward)
      ? previewSource(rawReward.item, item.itemIndex) : undefined;
    return { ...presentation, ...(tooltipSource ? { tooltipSource } : {}) };
  });
}

/** Enrich parsed rewards only from their matching NewQuestInfo ItemInfo rows. */
export function withQuestRewardTooltipSources(
  rewards: QuestRewardsPresentation | undefined,
  rawInfo: unknown,
): QuestRewardsPresentation | undefined {
  if (!rewards) return undefined;
  const info = record(rawInfo) ? rawInfo : {};
  return {
    ...rewards,
    ...(rewards.items ? { items: attachGroup(rewards.items, info.rewards_fixed_item, false) } : {}),
    ...(rewards.selectItems ? { selectItems: attachGroup(rewards.selectItems, info.rewards_select_item, true) } : {}),
  };
}
