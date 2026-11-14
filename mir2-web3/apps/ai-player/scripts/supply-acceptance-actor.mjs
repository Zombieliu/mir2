// Separate OS process for supply acceptance. Only ordinary player methods run.
import { ResidentActor } from "../src/resident-actor.mjs";

let actor, abortRun, runPromise, dropNext = false, dropInjected = false, unsafeObserved = false;
let queue = Promise.resolve();
const calls = { begin: 0, quote: 0, purchase: 0, query: 0 };
const operations = [], queries = [], purchases = [];
const send = value => { if (process.connected) process.send(value); };
const countHp = state => [...state.inventoryItems ?? [], ...state.beltItems ?? []]
  .filter(item => item.name === "(HP)DrugSmall").reduce((n, item) => n + Number(item.quantity ?? 0), 0);
const economic = state => ({ gold: state.gold, hpPotions: countHp(state), player: state.player && { x: state.player.x, y: state.player.y },
  mapFileName: state.mapFileName, playerHp: state.playerHp, playerMaxHp: state.playerMaxHp });
const snapshot = state => ({ ...economic(state), ready: state.ready, connectionEpoch: state.connectionEpoch,
  entities: state.entities.map(e => ({ objectId: e.objectId, kind: e.kind, name: e.name, x: e.x, y: e.y, dead: e.dead })),
  selfZoneObjectId: state.selfZoneObjectId, playerObjectId: state.playerObjectId,
  observedPotionGood: state.nativeNpcShop?.packetType === "NPCGoods" && state.nativeNpcShop?.service?.toUpperCase() === "BUYSELL"
    ? structuredClone(state.nativeNpcShop.list.find(item => item.itemIndex === 658 && item.name === "(HP)DrugSmall" && item.count === 1) ?? null) : null });

function installObservingHooks(client) {
  for (const [name, key] of [["beginNpcPurchase", "begin"], ["quoteNpcPurchase", "quote"], ["queryNpcPurchase", "query"]]) {
    const original = client[name].bind(client);
    client[name] = async value => {
      calls[key]++;
      if (key === "query") queries.push(structuredClone(value));
      return await original(value);
    };
  }
  const purchase = client.purchaseNpcItem.bind(client);
  client.purchaseNpcItem = async operation => {
    calls.purchase++;
    operations.push(structuredClone(operation));
    const before = economic(client.snapshot());
    const frame = await purchase(operation);
    const receipt = frame.reply?.receipt ?? null;
    const proof = { operation: structuredClone(operation), before, after: economic(client.snapshot()),
      receipt: receipt && structuredClone(receipt), authority: frame.authority && structuredClone(frame.authority),
      hasCompleteSnapshot: frame.snapshot !== null && Array.isArray(frame.snapshot?.inventoryItems)
        && Array.isArray(frame.snapshot?.beltItems) && Array.isArray(frame.snapshot?.entities) };
    purchases.push(proof);
    send({ type: "supplyPurchaseBoundary", proof });
    if (dropNext && receipt?.entry?.outcome?.committed) {
      dropNext = false; dropInjected = true;
      // The real server response and debit already happened. Discard only the
      // method result seen by the runner, then stop its loop before any query.
      abortRun?.abort();
      send({ type: "supplyFaultInjected", kind: "drop-committed-method-result", originalOperation: structuredClone(operation) });
      const error = new Error("The acceptance boundary deliberately discarded the committed result.");
      error.code = "NPC_PURCHASE_UNKNOWN";
      throw error;
    }
    return frame;
  };
}

async function stop() {
  abortRun?.abort();
  await runPromise?.catch(() => {});
  runPromise = null;
  const saved = await actor?.stop();
  return { saved: saved === true, processId: process.pid };
}

process.on("message", message => {
  if (message?.command === "stop") abortRun?.abort();
  queue = queue.then(async () => {
    try {
      let result;
      switch (message.command) {
        case "start": {
          if (actor || message.profile?.runtimeService?.provider !== "rules") throw new Error("Supply acceptance requires a new rules-only actor.");
          actor = new ResidentActor(message.profile, { report: event => {
            if (["supplyAcquired", "supplyReturned", "supplyMemoryWriteUnavailable"].includes(event.type)) send({ type: "report", event });
          } });
          installObservingHooks(actor.client);
          actor.client.on("command", ({ type }) => send({ type: "supplyActorCommand", command: type }));
          actor.client.on("frame", () => {
            if (!runPromise || unsafeObserved) return;
            const state = actor.client.snapshot();
            if (state.ready && (state.player?.dead === true || Number.isSafeInteger(state.playerHp)
                && Number.isSafeInteger(state.playerMaxHp) && state.playerHp < state.playerMaxHp)) {
              unsafeObserved = true; abortRun?.abort();
              send({ type: "supplyActorSafetyStopped", reason: "ordinary-character-injured" });
            }
          });
          await actor.start({ accountId: process.env.MIR2_AI_PLAYER_ACCOUNT, password: process.env.MIR2_AI_PLAYER_PASSWORD });
          abortRun = new AbortController();
          runPromise = actor.run({ durationMs: 900_000, signal: abortRun.signal }).catch(() => {
            send({ type: "supplyActorRunStopped" });
          });
          result = { processId: process.pid, state: snapshot(actor.client.snapshot()) };
          break;
        }
        case "snapshot": result = snapshot(actor.client.snapshot()); break;
        case "audit": result = { processId: process.pid, calls: { ...calls }, operations: structuredClone(operations), queries: structuredClone(queries), purchases: structuredClone(purchases), dropInjected, unsafeObserved }; break;
        case "armDropCommittedResult":
          if (dropNext || dropInjected) throw new Error("Only one deliberate result-loss fault is permitted per process.");
          dropNext = true; result = { armed: true }; break;
        case "stop": result = await stop(); break;
        default: throw new Error("Unsupported supply acceptance actor command.");
      }
      send({ type: "result", id: message.id, result });
      if (message.command === "stop" && process.connected) process.disconnect();
    } catch {
      send({ type: "result", id: message.id, error: "The bounded supply acceptance actor operation failed." });
    }
  });
});
process.on("disconnect", () => { abortRun?.abort(); stop().catch(() => {}); });
