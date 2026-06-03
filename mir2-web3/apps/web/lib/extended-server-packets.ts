// Shared helpers for the [fe-packets] extended server->client packet handlers.
//
// These are pure, framework-free utilities that normalise the JSON payload
// shapes emitted by the gateway (apps/gateway/src/web.rs `server_packet_to_event`)
// into the lightweight value objects consumed by the web client's WorldState.
//
// Field-name notes (derived from the gateway serializer + protocol serde attrs):
//   * Enum-level variant fields are camelCase (#[serde(rename_all_fields)]).
//   * `UserItem` has no rename_all, so its fields arrive snake_case
//     (`unique_id`, `item_index`, `current_dura`, `max_dura`, `count`).
//   * `ClientFriend` / `ClientMail` use rename_all = "camelCase".

export type PacketRecord = Record<string, unknown>;

export function packetNumber(value: unknown): number | undefined {
  return typeof value === "number" && Number.isFinite(value) ? value : undefined;
}

export function packetString(value: unknown): string | undefined {
  return typeof value === "string" ? value : undefined;
}

export function packetBool(value: unknown): boolean | undefined {
  return typeof value === "boolean" ? value : undefined;
}

/** Normalised view of a protocol `UserItem` payload. */
export type NormalizedUserItem = {
  uniqueId: number;
  itemIndex?: number;
  currentDura?: number;
  maxDura?: number;
  count: number;
};

/**
 * Reads a `UserItem` payload tolerating both snake_case (the wire shape) and
 * camelCase (in case a future serializer change normalises field names).
 */
export function normalizeUserItem(raw: unknown): NormalizedUserItem | null {
  if (!raw || typeof raw !== "object") {
    return null;
  }
  const record = raw as PacketRecord;
  const uniqueId =
    packetNumber(record.uniqueId) ?? packetNumber(record.unique_id);
  if (typeof uniqueId !== "number") {
    return null;
  }
  return {
    uniqueId,
    itemIndex: packetNumber(record.itemIndex) ?? packetNumber(record.item_index),
    currentDura:
      packetNumber(record.currentDura) ?? packetNumber(record.current_dura),
    maxDura: packetNumber(record.maxDura) ?? packetNumber(record.max_dura),
    count: packetNumber(record.count) ?? 1,
  };
}

/** Minimal item shape the client tracks inside its bag/belt/storage lists. */
export type ExtendedWorldItemPatch = {
  uniqueId: number;
  quantity?: number;
  durabilityCurrent?: number;
  durabilityMax?: number;
  name?: string;
  icon?: number;
};

/**
 * Applies a durability/quantity patch to every item in a list that matches the
 * given uniqueId. Returns the original list reference when nothing matched so
 * callers can avoid needless React state churn.
 */
export function patchItemsByUniqueId<
  T extends {
    uniqueId: number;
    quantity: number;
    durabilityCurrent?: number;
    durabilityMax?: number;
    name: string;
  },
>(items: T[], patch: ExtendedWorldItemPatch): T[] {
  let changed = false;
  const next = items.map((item) => {
    if (item.uniqueId !== patch.uniqueId) {
      return item;
    }
    changed = true;
    return {
      ...item,
      quantity: patch.quantity ?? item.quantity,
      durabilityCurrent: patch.durabilityCurrent ?? item.durabilityCurrent,
      durabilityMax: patch.durabilityMax ?? item.durabilityMax,
      name: patch.name ?? item.name,
    };
  });
  return changed ? next : items;
}

/** Removes (or decrements) an item across a list by uniqueId. */
export function removeItemByUniqueId<
  T extends { uniqueId: number; quantity: number },
>(items: T[], uniqueId: number, count: number): T[] {
  let changed = false;
  const next = items.flatMap((item) => {
    if (changed || item.uniqueId !== uniqueId) {
      return [item];
    }
    changed = true;
    if (count > 0 && item.quantity > count) {
      return [{ ...item, quantity: item.quantity - count }];
    }
    return [];
  });
  return changed ? next : items;
}

/** Normalised friend record for the social panel. */
export type NormalizedFriend = {
  index: number;
  name: string;
  memo: string;
  blocked: boolean;
  online: boolean;
};

export function normalizeFriendList(raw: unknown): NormalizedFriend[] {
  if (!Array.isArray(raw)) {
    return [];
  }
  return raw.flatMap((entry) => {
    if (!entry || typeof entry !== "object") {
      return [];
    }
    const record = entry as PacketRecord;
    const name = packetString(record.name);
    if (!name) {
      return [];
    }
    return [
      {
        index: packetNumber(record.index) ?? 0,
        name,
        memo: packetString(record.memo) ?? "",
        blocked: packetBool(record.blocked) ?? false,
        online: packetBool(record.online) ?? false,
      },
    ];
  });
}

/** Normalised mail record for the mailbox panel (stored in stage5Systems.mail). */
export function normalizeMailList(raw: unknown): Array<Record<string, unknown>> {
  if (!Array.isArray(raw)) {
    return [];
  }
  return raw.flatMap((entry) => {
    if (!entry || typeof entry !== "object") {
      return [];
    }
    const record = entry as PacketRecord;
    const mailId = packetNumber(record.mailId) ?? packetNumber(record.mail_id);
    if (typeof mailId !== "number") {
      return [];
    }
    return [
      {
        mailId,
        senderName:
          packetString(record.senderName) ?? packetString(record.sender_name) ?? "",
        message: packetString(record.message) ?? "",
        opened: packetBool(record.opened) ?? false,
        locked: packetBool(record.locked) ?? false,
        canReply: packetBool(record.canReply) ?? packetBool(record.can_reply) ?? false,
        collected: packetBool(record.collected) ?? false,
        gold: packetNumber(record.gold) ?? 0,
        itemCount: Array.isArray(record.items) ? record.items.length : 0,
      },
    ];
  });
}

/** Maps the numeric attack mode (ChangeAMode) to a human-readable label. */
export function attackModeLabel(mode: number): string {
  switch (mode) {
    case 0:
      return "Peaceful";
    case 1:
      return "Group";
    case 2:
      return "Guild";
    case 3:
      return "RedOnly";
    case 4:
      return "All";
    case 5:
      return "PeaceAttack";
    default:
      return `Mode ${mode}`;
  }
}

/** Maps the numeric pet mode (ChangePMode) to a human-readable label. */
export function petModeLabel(mode: number): string {
  switch (mode) {
    case 0:
      return "Both";
    case 1:
      return "Group";
    case 2:
      return "Guild";
    case 3:
      return "None";
    case 4:
      return "AttackOnly";
    case 5:
      return "MoveOnly";
    default:
      return `Pet Mode ${mode}`;
  }
}

/** Result-code text for MailSent / ParcelCollected. */
export function mailResultMessage(result: number, parcel: boolean): string {
  if (result === 1) {
    return parcel ? "Parcel collected." : "Mail sent.";
  }
  switch (result) {
    case 0:
      return parcel ? "Parcel could not be collected." : "Mail could not be sent.";
    case -1:
      return "Recipient not found.";
    case -2:
      return "Not enough gold.";
    case -3:
      return "Mailbox is full.";
    default:
      return parcel
        ? `Parcel collection returned ${result}.`
        : `Mail send returned ${result}.`;
  }
}

/** NewHero / NewCharacter style result -> message. */
export function heroCreateResultMessage(result: number): string {
  switch (result) {
    case 1:
      return "Hero name is too long.";
    case 2:
      return "Hero name contains banned words.";
    case 3:
      return "A hero with that name already exists.";
    case 4:
      return "Maximum number of heroes reached.";
    case 8:
      return "Hero created successfully.";
    default:
      return `Hero creation returned ${result}.`;
  }
}

/** Builds the immutable stage5 group slice update for membership changes. */
export function groupMembersAfterChange(
  current: string[] | undefined,
  change: { add?: string; remove?: string; replace?: string[] },
): string[] {
  if (change.replace) {
    return [...change.replace];
  }
  const base = current ? [...current] : [];
  if (change.remove) {
    return base.filter((member) => member !== change.remove);
  }
  if (change.add && !base.includes(change.add)) {
    return [...base, change.add];
  }
  return base;
}
