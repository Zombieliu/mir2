export type ContentTranslateFn = (
  key: string,
  params?: Array<string | number>,
  fallback?: string,
) => string;

const MAP_KEY_BY_TITLE: Readonly<Record<string, string>> = {
  BichonProvince: "content.map.bichonProvince.name",
};

const ENTITY_KEY_BY_NAME: Readonly<Record<string, string>> = {
  Deer: "content.entity.deer.name",
  Scarecrow: "content.entity.scarecrow.name",
  Hen: "content.entity.hen.name",
  Royal_Guard: "content.entity.royalGuard.name",
  Royal_Archer: "content.entity.royalArcher.name",
  HookingCat: "content.entity.hookingCat.name",
  Oma: "content.entity.oma.name",
  RakingCat: "content.entity.rakingCat.name",
  ForestYeti: "content.entity.forestYeti.name",
  ForestYeti0: "content.entity.forestYeti.name",
  Teleport_Gilbert: "content.entity.teleportGilbert.name",
  BorderVillage_Board: "content.entity.borderVillageBoard.name",
  Assistant_Jane: "content.entity.assistantJane.name",
  CraftsLady_Jude: "content.entity.craftsLadyJude.name",
  Blacksmith_Smith: "content.entity.blacksmithSmith.name",
  Merchant_John: "content.entity.merchantJohn.name",
  Merchant_Whitney: "content.entity.merchantWhitney.name",
  Merchant_Ruben: "content.entity.merchantRuben.name",
  Merchant_Scott: "content.entity.merchantScott.name",
  MaterialDealer_Reece: "content.entity.materialDealerReece.name",
  Master_Wa: "content.entity.masterWa.name",
  MirGuide_Peter: "content.entity.mirGuidePeter.name",
};

const ITEM_KEY_BY_NAME: Readonly<Record<string, string>> = {
  "(HP)DrugSmall": "content.item.smallHpDrug.name",
  GoldenPendant: "content.item.goldenPendant.name",
  CopperRing: "content.item.copperRing.name",
  SharpDagger: "content.item.sharpDagger.name",
  ToughHoaSword: "content.item.toughHoaSword.name",
  StiffWoodenBow: "content.item.stiffWoodenBow.name",
  OldCopperRing: "content.item.oldCopperRing.name",
  WornIronBracelet: "content.item.wornIronBracelet.name",
  BronzeWarriorSword: "content.item.bronzeWarriorSword.name",
  StrongHoaSword: "content.item.strongHoaSword.name",
  StrongWoodenBow: "content.item.strongWoodenBow.name",
  OldLoafer: "content.item.oldLoafer.name",
  Fencing: "content.item.fencing.name",
  PrecisionPendant: "content.item.precisionPendant.name",
};

/** Localize canonical Crystal map titles without mutating protocol/world state. */
export function localizeCrystalMapTitle(
  mapTitle: string | null | undefined,
  t: ContentTranslateFn,
): string | null {
  if (!mapTitle) return null;
  const key = MAP_KEY_BY_TITLE[mapTitle];
  return key ? t(key, [], mapTitle) : mapTitle;
}

/** Localize canonical NPC/monster labels at the presentation edge only. */
export function localizeCrystalEntityName(name: string, t: ContentTranslateFn): string {
  const key = ENTITY_KEY_BY_NAME[name];
  return key ? t(key, [], name) : name;
}

/** Localize canonical Crystal item names used by structured quest rewards. */
export function localizeCrystalItemName(name: string, t: ContentTranslateFn): string {
  const key = ITEM_KEY_BY_NAME[name];
  return key ? t(key, [], name) : name;
}
