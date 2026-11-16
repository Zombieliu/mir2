export type R16SocialRect = Readonly<{ left: number; top: number; width: number; height: number }>;
const rect = (left: number, top: number, width: number, height: number): R16SocialRect => ({ left, top, width, height });

/** Logical 1024x768 coordinates from the R16 native Group/Trade dialogs. */
export const R16_SOCIAL_LAYOUT = {
  stage: rect(0, 0, 1024, 768),
  group: {
    panel: rect(396, 259, 232, 249),
    title: rect(18, 8, 55, 15),
    close: rect(206, 3, 24, 21),
    allow: rect(25, 219, 28, 25),
    add: rect(70, 219, 60, 25),
    remove: rect(140, 219, 60, 25),
    input: rect(368, 306, 288, 156),
    inputField: rect(23, 86, 240, 19),
    inputConfirm: rect(60, 123, 76, 25),
    inputCancel: rect(160, 123, 76, 25),
  },
  trade: {
    own: rect(298, 418, 204, 152),
    guest: rect(522, 418, 204, 152),
    ownName: rect(20, 10, 150, 14),
    guestName: rect(0, 10, 204, 14),
    gold: rect(35, 123, 90, 15),
    confirm: rect(135, 120, 48, 25),
    close: rect(181, 3, 24, 21),
    amount: rect(410, 329, 204, 109),
    amountField: rect(58, 43, 132, 19),
    amountConfirm: rect(23, 76, 76, 25),
    amountCancel: rect(110, 76, 76, 25),
    amountClose: rect(180, 3, 24, 21),
    bagPicker: rect(708, 250, 316, 156),
  },
  invitation: {
    panel: rect(284, 289, 456, 190),
    accept: rect(260, 157, 76, 25),
    decline: rect(360, 157, 76, 25),
  },
} as const;

/** Crystal trade arrays are column-major: slot = 2*x + y. */
export function r16TradeCell(slot: number): R16SocialRect | null {
  return Number.isInteger(slot) && slot >= 0 && slot < 10
    ? rect(10 + Math.floor(slot / 2) * 37, 39 + (slot % 2) * 33, 36, 32)
    : null;
}

export function r16GroupMemberRect(index: number): R16SocialRect | null {
  if (!Number.isInteger(index) || index < 0 || index >= 15) return null;
  return index === 0 ? rect(16, 33, 96, 16)
    : rect(((index + 1) % 2) * 100 + 16, 55 + Math.floor((index - 1) / 2) * 20, 96, 16);
}

export function r16ClampSocialWindow(
  position: Readonly<{ left: number; top: number }>,
  size: Readonly<{ width: number; height: number }>,
  stage: Readonly<{ width: number; height: number }> = R16_SOCIAL_LAYOUT.stage,
) {
  return {
    left: Math.max(0, Math.min(Math.max(0, stage.width - size.width), position.left)),
    top: Math.max(0, Math.min(Math.max(0, stage.height - size.height), position.top)),
  };
}

/** Amount entry never changes a balance. The existing host revalidates the send. */
export function r16TradeGoldAmount(draft: string, availableGold?: number): number | null {
  if (!/^[0-9]+$/.test(draft)) return null;
  const value = Number(draft);
  if (!Number.isSafeInteger(value) || value < 0 || value > 0xffff_ffff) return null;
  return typeof availableGold === "number" && Number.isSafeInteger(availableGold) && availableGold >= 0
    ? Math.min(value, availableGold) : value;
}
