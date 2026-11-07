import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import { ResidentActor } from "./resident-actor.mjs";
import { stringifyGatewayJson } from "./lossless-json.mjs";
import { attachCompanionControl } from "./companion-control.mjs";

function options(args) {
  const result = { profile: null, duration: 60, createAccount: false, createCharacter: false, explore: false, say: null, teach: false, recall: false, interactive: false };
  for (let index = 0; index < args.length; index += 1) {
    const arg = args[index];
    if (arg === "--profile") result.profile = args[++index];
    else if (arg === "--duration") result.duration = Number(args[++index]);
    else if (arg === "--say") {
      result.say = args[++index];
      if (typeof result.say !== "string" || result.say.startsWith("--")) throw new Error("Supply one nearby-chat line after --say.");
    }
    else if (arg === "--create-account") result.createAccount = true;
    else if (arg === "--create-character") result.createCharacter = true;
    else if (arg === "--teach-support-before-loot") result.teach = true;
    else if (arg === "--recall") result.recall = true;
    else if (arg === "--interactive") result.interactive = true;
    else if (arg === "--explore") result.explore = true;
    else if (arg === "--help") return null;
    else throw new Error(`Unknown argument: ${arg}`);
  }
  if (!result.profile || !Number.isFinite(result.duration) || result.duration < 1 || result.duration > 86_400) {
    throw new Error("Use --profile <file> and --duration <1..86400 seconds>.");
  }
  return result;
}

async function main() {
  const args = options(process.argv.slice(2));
  if (!args) {
    console.log("node src/main.mjs --profile profiles/bichon-warrior.local.json [--duration 60] [--explore] [--say <text>] [--create-account] [--create-character]");
    console.log("Companion profiles support --teach-support-before-loot, --recall, --interactive (teach/recall/resume/quit). OpenAI planning is opt-in through the private owner interface and local model settings; keep credentials outside profiles.");
    return;
  }
  const profile = JSON.parse(await readFile(resolve(args.profile), "utf8"));
  if (typeof profile.characterName !== "string" || !profile.characterName.trim()) throw new Error("Configure characterName in the profile.");
  if (profile.companion && args.explore) throw new Error("Companion control and diagnostic exploration cannot run together.");
  if (!profile.companion && (args.teach || args.recall || args.interactive)) throw new Error("Teaching and recall require a companion profile.");
  const actor = new ResidentActor(profile, { report: (event) => console.log(stringifyGatewayJson(event)) });
  const abort = new AbortController();
  const stop = () => abort.abort();
  process.once("SIGINT", stop);
  process.once("SIGTERM", stop);
  let closeControl;
  try {
    await actor.start({
      accountId: process.env.MIR2_AI_PLAYER_ACCOUNT,
      password: process.env.MIR2_AI_PLAYER_PASSWORD,
      createAccount: args.createAccount,
      createCharacter: args.createCharacter,
    });
    if (args.teach) {
      if (!actor.companion) throw new Error("Teaching requires a companion profile.");
      actor.companion.teachSupportBeforeLoot();
    }
    if (args.recall) {
      if (!actor.companion) throw new Error("Recall requires a companion profile.");
      actor.companion.recallRequested = true;
    }
    if (args.say !== null) console.log(JSON.stringify({ type: "nearbyChat", ...await actor.client.sayNearby(args.say) }));
    if (args.interactive) closeControl = attachCompanionControl({ actor, input: process.stdin, report: event => console.log(stringifyGatewayJson(event)), stop });
    await actor.run({ durationMs: args.duration * 1_000, explore: args.explore, signal: abort.signal });
  } finally {
    closeControl?.();
    await actor.stop();
    process.removeListener("SIGINT", stop);
    process.removeListener("SIGTERM", stop);
  }
}

main().catch(() => {
  // Server error strings and incoming identity messages can contain private data.
  console.error("AI player stopped: check the configured world, account, character, and gateway availability. No credentials were logged.");
  process.exitCode = 1;
});
