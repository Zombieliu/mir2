import assert from "node:assert/strict";
import test from "node:test";
import { createServer } from "node:net";
import { acquireResidentGuard, ResidentActor } from "../src/resident-actor.mjs";

async function port() {
  const server = createServer();
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  const result = server.address().port;
  await new Promise((resolve) => server.close(resolve));
  return result;
}

function fakeClient({ characterClass = "Warrior", hp = 18 } = {}) {
  const calls = [];
  const state = {
    ready: true, mapFileName: "0", playerHp: hp, playerMaxHp: 18,
    player: { x: 288, y: 608, direction: "Down", dead: false },
    entities: [], inventoryItems: [], questLog: [], gold: 0,
  };
  return {
    calls, state,
    connect: async () => { calls.push("connect"); },
    login: async () => { calls.push("login"); return [{ name: "AchuanAI", index: 1, class: characterClass }]; },
    createAccount: async () => { calls.push("createAccount"); },
    createCharacter: async () => { calls.push("createCharacter"); },
    startGame: async () => { calls.push("startGame"); return structuredClone(state); },
    snapshot: () => structuredClone(state),
    walk: async () => { calls.push("walk"); return { moved: false, after: state.player }; },
    logout: async () => { calls.push("logout"); return true; },
    close: async () => { calls.push("close"); },
  };
}

test("the fixed resident guard rejects a second instance and can be released", async () => {
  const guardPort = await port();
  const release = await acquireResidentGuard(guardPort);
  try { await assert.rejects(acquireResidentGuard(guardPort), { code: "EADDRINUSE" }); }
  finally { await release(); }
  await (await acquireResidentGuard(guardPort))();
});

test("existing character binding does not silently create another account or character", async () => {
  const client = fakeClient();
  const actor = new ResidentActor({ characterName: "AchuanAI", guardPort: await port() }, { client });
  await actor.start({ accountId: "test-account", password: "private-password" });
  await actor.stop();
  assert.deepEqual(client.calls, ["connect", "login", "startGame", "logout", "close"]);
});

test("a mismatched class closes and releases the guard without starting the wrong character", async () => {
  const client = fakeClient({ characterClass: "Wizard" });
  const guardPort = await port();
  const actor = new ResidentActor({ characterName: "AchuanAI", guardPort }, { client });
  await assert.rejects(actor.start({ accountId: "test-account", password: "private-password" }), /Warrior/);
  assert.equal(client.calls.includes("startGame"), false);
  assert.equal(client.calls.at(-1), "close");
  await (await acquireResidentGuard(guardPort))();
});

test("diagnostic exploration never moves with unknown or depleted health", async () => {
  for (const hp of [null, 0, 4]) {
    const client = fakeClient({ hp });
    const abort = new AbortController();
    const actor = new ResidentActor({}, {
      client,
      report: (event) => { if (event.type === "residentStatus") abort.abort(); },
    });
    await actor.run({ durationMs: 1_000, explore: true, signal: abort.signal });
    assert.equal(client.calls.includes("walk"), false);
  }
});

test("saved status is emitted only after the actual logout operation resolves", async () => {
  const client = fakeClient();
  const events = [];
  let confirm;
  client.logout = () => new Promise((resolve) => { confirm = resolve; });
  const actor = new ResidentActor({ characterName: "AchuanAI", guardPort: await port() }, { client, report: (event) => events.push(event) });
  await actor.start({ accountId: "test-account", password: "private-password" });
  const stopping = actor.stop();
  assert.equal(events.some((event) => event.saved === true), false);
  confirm(true);
  assert.equal(await stopping, true);
  assert.equal(events.at(-1).saved, true);
  assert.equal(await actor.stop(), false);
});
