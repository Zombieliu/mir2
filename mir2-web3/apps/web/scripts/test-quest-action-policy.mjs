import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import { fileURLToPath } from "node:url";

import ts from "typescript";

const sourcePath = fileURLToPath(new URL("../lib/quest-action-policy.ts", import.meta.url));
const compiled = ts.transpileModule(readFileSync(sourcePath, "utf8"), {
  compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022, strict: true },
  fileName: sourcePath,
  reportDiagnostics: true,
});
assert.deepEqual(
  (compiled.diagnostics ?? []).filter((diagnostic) => diagnostic.category === ts.DiagnosticCategory.Error),
  [],
);
const loaded = { exports: {} };
new Function("exports", "module", compiled.outputText)(loaded.exports, loaded);
const {
  questGuidanceProfile,
  questEndpoint,
  questActionInput,
  hasPendingQuestAction,
  matchingQuestOperationAck,
  resolveQuestHostAction,
} = loaded.exports;

test("the deployment profile is explicit and defaults to Crystal", () => {
  assert.equal(questGuidanceProfile("newcomer-v1"), "newcomer-v1");
  assert.equal(questGuidanceProfile("newcomer-v2"), "newcomer-v2");
  assert.equal(questGuidanceProfile(undefined), "crystal");
  assert.equal(questGuidanceProfile("platinum_176"), "crystal");
});

test("wire endpoints preserve explicit zero and reject missing or malformed values", () => {
  assert.equal(questEndpoint(0), 0);
  assert.equal(questEndpoint(42), 42);
  for (const value of [undefined, null, "0", "42", -1, 1.5, NaN, Infinity, 0x1_0000_0000]) {
    assert.equal(questEndpoint(value), undefined, String(value));
  }
});

test("host projection passes endpoint, reward, stage, dialog and pending facts into shared Rust", () => {
  const quest = {
    stage: "readyToTurnIn",
    acceptNpcIndex: 0,
    finishNpcIndex: 0,
    rewards: { selectItems: [{ selectionIndex: 3 }, {}] },
  };
  const input = questActionInput({
    action: "finish",
    profile: "newcomer-v2",
    questId: 102,
    quest,
    dialog: { npcObjectId: "24", links: [{ target: "@FinishQuest:102" }] },
    selectedRewardIndex: 3,
    pending: true,
  });
  assert.deepEqual(input, {
    action: "finish",
    profile: "newcomer-v2",
    status: "readyToTurnIn",
    acceptNpcIndex: 0,
    finishNpcIndex: 0,
    selectableRewardIndices: [3, 1],
    selectedRewardIndex: 3,
    dialogNpcIndex: 24,
    dialogActionOffered: true,
    pending: true,
  });
  let received;
  const decision = { eligible: false, rejection: "pending" };
  assert.equal(resolveQuestHostAction({ resolveQuestAction(value) { received = value; return decision; } }, input), decision);
  assert.equal(received, input, "host delegates the qualification decision to the loaded Rust runtime");
});

test("absent metadata remains absent even with a matching NPC dialog link", () => {
  const input = questActionInput({
    action: "accept",
    profile: "crystal",
    questId: 9,
    quest: { stage: "available" },
    dialog: { npcObjectId: "17", links: [{ target: "@quest:accept:9" }] },
    pending: false,
  });
  assert.equal(input.acceptNpcIndex, undefined);
  assert.equal(input.finishNpcIndex, undefined);
  assert.equal(input.dialogActionOffered, true);
  assert.equal(input.dialogNpcIndex, 17);
  assert.equal(input.status, "notStarted");
});

test("NPC offer requires an exact active link and positive numeric object id", () => {
  const base = { action: "finish", profile: "crystal", questId: 8, quest: { stage: "readyToTurnIn" }, pending: false };
  const links = [{ target: "@quest:finish:8:2" }];
  assert.equal(questActionInput({ ...base, dialog: { npcObjectId: "0", links }, selectedRewardIndex: 2 }).dialogActionOffered, false);
  assert.equal(questActionInput({ ...base, dialog: { npcObjectId: "8garbage", links }, selectedRewardIndex: 2 }).dialogActionOffered, false);
  assert.equal(questActionInput({ ...base, dialog: { npcObjectId: "24", links }, selectedRewardIndex: 1 }).dialogActionOffered, false);
  assert.equal(questActionInput({ ...base, dialog: { npcObjectId: "24", links }, selectedRewardIndex: 2 }).dialogActionOffered, true);
  assert.equal(questActionInput({ ...base, dialog: { npcObjectId: "24", links: [{ ...links[0], enabled: false }] }, selectedRewardIndex: 2 }).dialogActionOffered, false);
});

test("malformed reward selection fails at the wire boundary instead of becoming no-choice", () => {
  const base = { action: "finish", profile: "newcomer-v1", questId: 8, dialog: null, pending: false };
  assert.equal(questActionInput({ ...base, quest: { stage: "readyToTurnIn", rewards: { selectItems: [{ selectionIndex: 2.5 }] } } }), null);
  assert.equal(questActionInput({ ...base, quest: { stage: "readyToTurnIn", rewards: { selectItems: [{ selectionIndex: 0x8000_0000 }] } } }), null);
  assert.equal(questActionInput({ ...base, quest: { stage: "readyToTurnIn", rewards: { selectItems: [{}] } }, selectedRewardIndex: -1 }), null);
  assert.deepEqual(
    questActionInput({ ...base, quest: { stage: "readyToTurnIn", rewards: { selectItems: [{}] } }, selectedRewardIndex: 0 }).selectableRewardIndices,
    [0],
  );
});

test("only the exact request and operation receipt releases pending transport state", () => {
  const pending = { requestId: "qs-0000000000000009", operation: "finishQuest", questIndex: 102, selectedItemIndex: 3 };
  const ack = { ...pending, success: false };
  assert.deepEqual(matchingQuestOperationAck(ack, pending), { success: false });
  assert.equal(matchingQuestOperationAck({ ...ack, requestId: "qs-0000000000000008" }, pending), null);
  assert.equal(matchingQuestOperationAck({ ...ack, questIndex: 103 }, pending), null);
  assert.equal(matchingQuestOperationAck({ ...ack, selectedItemIndex: 2 }, pending), null);
  assert.equal(matchingQuestOperationAck({ ...ack, success: "false" }, pending), null);
  assert.equal(matchingQuestOperationAck(ack, undefined), null);
  const diaryAccept = { requestId: "qs-0000000000000010", operation: "acceptQuest", questIndex: 9, npcIndex: 0 };
  assert.deepEqual(matchingQuestOperationAck({ ...diaryAccept, success: true }, diaryAccept), { success: true });
  assert.equal(matchingQuestOperationAck({ ...diaryAccept, npcIndex: 24, success: true }, diaryAccept), null);
});

test("changing the selected reward cannot submit another finish while the quest is pending", () => {
  const firstSelection = {
    requestId: "qs-0000000000000011",
    operation: "finishQuest",
    questIndex: 102,
    selectedItemIndex: 1,
  };
  const pending = new Map([[firstSelection.requestId, firstSelection]]);
  assert.equal(hasPendingQuestAction(pending.values(), 102, "finishQuest"), true);
  assert.equal(hasPendingQuestAction(pending.values(), 102, "acceptQuest"), false);
  assert.equal(hasPendingQuestAction(pending.values(), 103, "finishQuest"), false);
  assert.equal(matchingQuestOperationAck({ ...firstSelection, selectedItemIndex: 2, success: false }, pending.get(firstSelection.requestId)), null);
  assert.deepEqual(matchingQuestOperationAck({ ...firstSelection, success: false }, pending.get(firstSelection.requestId)), { success: false });
  pending.delete(firstSelection.requestId);
  assert.equal(hasPendingQuestAction(pending.values(), 102, "finishQuest"), false);
});

test("a synchronous shared runtime failure cannot escape the host adapter", () => {
  const input = questActionInput({ action: "accept", profile: "crystal", questId: 9, quest: null, dialog: null, pending: false });
  assert.equal(resolveQuestHostAction({ resolveQuestAction() { throw new Error("WASM exception"); } }, input), null);
  assert.equal(resolveQuestHostAction({ resolveQuestAction() { return {}; } }, input), null);
});
