import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import ts from "typescript";

const compiled = ts.transpileModule(readFileSync(new URL("../lib/quest-reward-presentation.ts", import.meta.url), "utf8"), {
  compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
});
const module = { exports: {} };
new Function("exports", "module", compiled.outputText)(module.exports, module);
const { withQuestRewardTooltipSources: enrich } = module.exports;

const fixture = JSON.parse(readFileSync(new URL("../../game-client/client-bevy/tests/fixtures/web-bag-projection.json", import.meta.url), "utf8"));
const { item_index: fixtureIndex, ...fixtureInfo } = fixture.items[0].tooltipSource.info;
const wireInfo = { ...fixtureInfo, index: fixtureIndex };
const itemInfo = (index = 658, extra = {}) => ({ index, name: "Same item", image: 532, item_type: 13,
  durability: 1200, slots: 2, stats: [{ stat: 4, value: 7 }], ...extra });
const reward = (itemIndex = 658, extra = {}) => ({ name: "Localized reward", icon: 532, count: 11, itemIndex, ...extra });
const rawGroup = (items, key = "rewards_fixed_item") => ({ [key]: items.map((item) => ({ item, count: 11 })) });
const source = (parsed, raw) => enrich({ items: [parsed] }, rawGroup([raw])).items[0].tooltipSource;

test("real ItemInfo bytes survive preview enrichment without mutating parsed or wire data", () => {
  const raw = rawGroup([{ ...wireInfo, unknown_extension: { retained: [1, "future"] } }]);
  const parsed = { gold: 40, experience: 150, items: [reward(fixtureIndex)] };
  const beforeRaw = structuredClone(raw);
  const beforeParsed = structuredClone(parsed);
  const enriched = enrich(parsed, raw);
  assert.deepEqual(raw, beforeRaw);
  assert.deepEqual(parsed, beforeParsed);
  assert.notEqual(enriched, parsed);
  assert.notEqual(enriched.items[0], parsed.items[0]);
  assert.deepEqual(enriched.items[0].tooltipSource.info, {
    ...raw.rewards_fixed_item[0].item, item_index: fixtureIndex,
  });
  assert.equal(enriched.items[0].count, 11, "reward quantity stays on the cell");
  assert.equal(enriched.gold, 40);
  assert.equal(enriched.experience, 150);
});

test("QuestCell UserItem preview matches native constructor defaults instead of an owned item", () => {
  const result = source(reward(), itemInfo());
  assert.deepEqual(result.userItem, {
    unique_id: 0, item_index: 658, current_dura: 1200, max_dura: 1200, count: 0,
    soul_bound_id: -1, identified: false, cursed: false, slots: [null, null], gem_count: 0,
    added_stats: [], awake_type: 0, awake_values: [], refined_value: 0, refine_added: 0,
    refine_success_chance: 0, wedding_ring: -1, expire_info: null, rental_information: null,
    is_shop_item: false, sealed_info: null, gm_made: false,
  });
  assert.deepEqual(result.socketInfos, []);
  assert.deepEqual(result.realSocketInfos, []);
  assert.equal(result.realInfo, null);
  assert.equal(source(reward(), itemInfo(658, { durability: 0, slots: 0 })).userItem.max_dura, 0);
});

test("fixed and selectable sources match only their own group and original position", () => {
  const parsed = { items: [reward(658)], selectItems: [reward(659, { selectionIndex: 0, selectable: true })] };
  const result = enrich(parsed, { ...rawGroup([itemInfo(658)]), ...rawGroup([itemInfo(659)], "rewards_select_item") });
  assert.equal(result.items[0].tooltipSource.info.item_index, 658);
  assert.equal(result.selectItems[0].tooltipSource.info.item_index, 659);
  assert.equal(result.selectItems[0].selectionIndex, 0);
  assert.equal(result.selectItems[0].selectable, true);
  const swappedGroups = enrich(parsed, { ...rawGroup([itemInfo(659)]), ...rawGroup([itemInfo(658)], "rewards_select_item") });
  assert.equal(swappedGroups.items[0].tooltipSource, undefined);
  assert.equal(swappedGroups.selectItems[0].tooltipSource, undefined);
});

test("same name/icon, reordered rows and filtered group lengths never bind another source", () => {
  assert.equal(source(reward(), itemInfo(659)), undefined);
  const reordered = enrich({ items: [reward(658), reward(659)] }, rawGroup([itemInfo(659), itemInfo(658)]));
  assert.ok(reordered.items.every((item) => item.tooltipSource === undefined));
  for (const rawItems of [[], [itemInfo(658), itemInfo(658)]]) {
    assert.equal(enrich({ items: [reward()] }, rawGroup(rawItems)).items[0].tooltipSource, undefined);
  }
  const shorter = enrich({ items: [reward(), reward(659)] }, rawGroup([itemInfo()]));
  assert.ok(shorter.items.every((item) => item.tooltipSource === undefined));
  for (const selectionIndex of [1, -1, 0.5, NaN]) {
    const result = enrich({ selectItems: [reward(658, { selectionIndex })] }, rawGroup([itemInfo()], "rewards_select_item"));
    assert.equal(result.selectItems[0].tooltipSource, undefined);
    assert.ok(Object.is(result.selectItems[0].selectionIndex, selectionIndex));
  }
});

test("index aliases must be valid and agree, while item index zero remains valid", () => {
  assert.equal(source(reward(0), itemInfo(0)).info.item_index, 0);
  const { index: _wireIndex, ...withoutIndex } = itemInfo();
  assert.equal(source(reward(), { ...withoutIndex, item_index: 658 }).info.item_index, 658);
  assert.equal(source(reward(), itemInfo(658, { item_index: 658 })).info.item_index, 658);
  assert.equal(source(reward(), itemInfo(658, { item_index: 659 })), undefined);
  assert.equal(source(reward(), withoutIndex), undefined);
  for (const invalid of [undefined, null, "658", -1, 1.5, NaN, Infinity, 0x80000000]) {
    assert.equal(source(reward(658, { itemIndex: invalid }), itemInfo()), undefined);
    assert.equal(source(reward(), { ...withoutIndex, index: invalid }), undefined);
    assert.equal(source(reward(), itemInfo(658, { item_index: invalid === undefined ? null : invalid })), undefined);
  }
});

test("invalid durability or socket count cannot create a malformed preview", () => {
  for (const durability of [undefined, null, "1200", -1, 0.5, NaN, Infinity, 65536]) {
    assert.equal(source(reward(), itemInfo(658, { durability })), undefined);
  }
  for (const slots of [undefined, null, "2", -1, 0.5, NaN, Infinity, 256]) {
    assert.equal(source(reward(), itemInfo(658, { slots })), undefined);
  }
  const maximum = source(reward(), itemInfo(658, { durability: 65535, slots: 255 }));
  assert.equal(maximum.userItem.current_dura, 65535);
  assert.equal(maximum.userItem.slots.length, 255);
});

test("missing/malformed raw data drops an old source instead of borrowing across definitions", () => {
  const old = enrich({ items: [reward()] }, rawGroup([itemInfo()]));
  for (const raw of [undefined, null, [], "bad", {}, { rewards_fixed_item: {} }, { rewards_fixed_item: [null] }]) {
    assert.equal(enrich(old, raw).items[0].tooltipSource, undefined);
  }
  for (const rawItem of [undefined, null, [], "bad"]) assert.equal(source(reward(), rawItem), undefined);
  assert.equal(enrich(undefined, rawGroup([itemInfo()])), undefined);
  assert.deepEqual(enrich({ gold: 30 }, undefined), { gold: 30 });
});

test("class/level based sources remain partial without inventing viewer-derived realInfo", () => {
  for (const flags of [{ class_based: true }, { level_based: true }, { class_based: true, level_based: true }]) {
    const raw = itemInfo(658, { ...flags, required_class: 31, required_amount: 20 });
    const result = source(reward(), raw);
    assert.deepEqual(result.info, { ...raw, item_index: 658 });
    assert.equal(result.realInfo, null);
    assert.equal(result.userItem.identified, false);
  }
});
