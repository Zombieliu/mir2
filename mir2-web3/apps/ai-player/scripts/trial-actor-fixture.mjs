// Isolated local acceptance runner. Gameplay and owner controls use real /ws.
import { ResidentActor } from "../src/resident-actor.mjs";
import { facingDirection } from "../src/companion-policy.mjs";

let actor, abortRun, runPromise;
let queue = Promise.resolve();
const reply = value => { if (process.connected) process.send(value); };
const publicPackets = new Set(["ObjectStruck", "ObjectAttack", "ObjectDied", "ObjectWalk", "UserLocation", "UseItem", "DeleteItem", "GainedItem", "ObjectItem", "HealthChanged", "GroupMemberInfo", "SendMemberLocation"]);

async function stop() {
  abortRun?.abort();
  await runPromise?.catch(() => {});
  runPromise = null;
  return actor?.stop();
}

process.on("message", message => {
  if (message?.command === "stop") abortRun?.abort();
  queue = queue.then(async () => {
    try {
      let result;
      switch (message.command) {
        case "start": {
          if (actor) throw new Error("Already started");
          actor = new ResidentActor(message.profile, { report: event => reply({ type: "report", event }) });
          actor.client.on("command", ({ type }) => reply({ type: "command", command: type }));
          actor.client.on("frame", ({ event }) => {
            if (publicPackets.has(event.packet)) reply({ type: "proof", at: Date.now(), event });
            if (event.type === "worldSnapshot") {
              const own = actor.client.snapshot();
              reply({ type: "proof", at: Date.now(), event: { type: "snapshotPosition", payload: {
                tick: own.tick, player: own.player && { x: own.player.x, y: own.player.y }, selfZoneObjectId: own.selfZoneObjectId,
              } } });
            }
          });
          result = await actor.start({ accountId: process.env.MIR2_AI_PLAYER_ACCOUNT, password: process.env.MIR2_AI_PLAYER_PASSWORD });
          if (message.run === true) {
            abortRun = new AbortController();
            runPromise = actor.run({ durationMs: 86_400_000, signal: abortRun.signal }).catch(() => {
              reply({ type: "report", event: { type: "fixtureRunStopped" } });
            });
          }
          break;
        }
        case "snapshot": result = actor.client.snapshot(); break;
        case "modelStatus": result = actor.companion?.modelDriver?.view() ?? { provider: "rules", modelEnabled: false }; break;
        case "tick":
          if (runPromise) throw new Error("Automatic runner active");
          result = await actor.companion.tick(); break;
        case "walk":
          if (runPromise) throw new Error("Automatic runner active");
          result = await actor.client.walk(message.direction); break;
        case "acceptInvite":
          if (runPromise) throw new Error("Automatic runner active");
          result = await actor.client.acceptGroupInvite(); break;
        case "dropCandle": {
          if (runPromise) throw new Error("Automatic runner active");
          const item = actor.client.snapshot().inventoryItems.find(entry => entry.name === "Candle");
          if (!item) throw new Error("No observed owned candle");
          result = await actor.client.dropOwnedItem(item.uniqueId); break;
        }
        case "attackObservedMonster": {
          if (runPromise) throw new Error("Automatic runner active");
          const state = actor.client.snapshot();
          const target = state.entities.find(entity => entity.kind === "monster" && entity.objectId === message.objectId && !entity.dead && entity.disposition !== "friendly");
          if (!target || Math.max(Math.abs(state.player.x - target.x), Math.abs(state.player.y - target.y)) !== 1) throw new Error("No adjacent observed monster");
          result = await actor.client.attackDirection(facingDirection(state.player, target)); break;
        }
        case "stop": result = await stop(); break;
        default: throw new Error("Unsupported fixture operation");
      }
      reply({ type: "result", id: message.id, result });
      if (message.command === "stop" && process.connected) process.disconnect();
    } catch (error) {
      // Never serialize transport errors, credentials, keys or private frames.
      const safeHints = ["Invalid base appearance", "Runtime enrollment was not accepted.", "The registered world epoch does not match this archive.",
        "The login character does not match the enrolled archive.", "Configured character was not found; character creation must be explicitly enabled.",
        "This archive is already running or its local archive lock is unavailable.", "Timed out waiting for the expected gateway result.",
        "Gateway rejected the player command.", "The exact owned item must be observed before use."];
      reply({ type: "result", id: message.id, error: "The bounded fixture operation failed.",
        errorHint: safeHints.includes(error.message) ? error.message : "A local startup/action invariant was not satisfied." });
    }
  });
});
process.on("disconnect", () => { abortRun?.abort(); stop().catch(() => {}); });
