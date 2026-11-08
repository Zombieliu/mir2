// Local acceptance child. All gameplay still travels through the ordinary client.
import { ResidentActor } from "../src/resident-actor.mjs";
import { facingDirection } from "../src/companion-policy.mjs";

let actor;
let queue = Promise.resolve();
const reply = value => { if (process.connected) process.send(value); };
process.on("message", message => {
  queue = queue.then(async () => {
    try {
      let result;
      switch (message.command) {
        case "start": {
          actor = new ResidentActor(message.profile, { report: event => reply({ type: "report", event }) });
          actor.client.on("command", ({ type }) => reply({ type: "command", command: type }));
          actor.client.on("frame", ({ event }) => {
            if (["ObjectStruck", "ObjectAttack", "ObjectDied", "ObjectWalk", "UserLocation", "UseItem", "DeleteItem", "GainedItem", "ObjectItem", "HealthChanged", "GroupMemberInfo", "SendMemberLocation"].includes(event.packet)) reply({ type: "proof", at: Date.now(), event });
            if (event.type === "worldSnapshot") {
              const own = actor.client.snapshot();
              reply({ type: "proof", at: Date.now(), event: { type: "snapshotPosition", payload: { tick: own.tick, player: own.player && { x: own.player.x, y: own.player.y }, selfZoneObjectId: own.selfZoneObjectId } } });
            }
          });
          result = await actor.start({ accountId: process.env.MIR2_AI_PLAYER_ACCOUNT, password: process.env.MIR2_AI_PLAYER_PASSWORD,
            createAccount: message.create, createCharacter: message.create });
          break;
        }
        case "snapshot": result = actor.client.snapshot(); break;
        case "tick": result = await actor.companion.tick(); break;
        case "teach": result = actor.companion.teachSupportBeforeLoot(message.feedbackId); break;
        case "recall": actor.companion.recallRequested = message.value === true; result = true; break;
        case "walk": result = await actor.client.walk(message.direction); break;
        case "removeArmour": result = await actor.client.removeOwnedEquipment("armour"); break;
        case "attackObservedMonster": {
          const state = actor.client.snapshot();
          const monster = state.entities.find(entity => entity.kind === "monster" && entity.objectId === message.objectId && !entity.dead && entity.disposition !== "friendly");
          if (!monster || Math.max(Math.abs(state.player.x - monster.x), Math.abs(state.player.y - monster.y)) !== 1) throw new Error("Fixture may only start an adjacent observed monster encounter.");
          result = await actor.client.attackDirection(facingDirection(state.player, monster)); break;
        }
        case "dropCandle": {
          const item = actor.client.snapshot().inventoryItems.find(item => item.name === "Candle");
          if (!item) throw new Error("No owned candle for ordinary drop fixture.");
          result = await actor.client.dropOwnedItem(item.uniqueId); break;
        }
        case "stop": result = await actor.stop(); break;
        default: throw new Error("Unknown bounded fixture operation.");
      }
      reply({ type: "result", id: message.id, result });
      if (message.command === "stop") process.disconnect();
    } catch (error) { reply({ type: "result", id: message.id, error: error.message }); }
  });
});
process.on("disconnect", () => { if (actor?.releaseGuard) actor.stop().catch(() => {}); });
