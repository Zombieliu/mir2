import { createInterface } from "node:readline";

/** Local operator input only. This is never connected to game chat or a model. */
export function attachCompanionControl({ actor, input, report = () => {}, stop = () => {} }) {
  if (!actor.companion) throw new Error("Interactive controls require a companion profile.");
  const lines = createInterface({ input, terminal: false });
  lines.on("line", line => {
    try {
      switch (line.trim()) {
        case "teach":
        case "组队先支援，战斗后拾取":
          actor.companion.teachSupportBeforeLoot(); break;
        case "recall": actor.companion.recallRequested = true; report({ type: "companionRecall", enabled: true }); break;
        case "resume": actor.companion.recallRequested = false; report({ type: "companionRecall", enabled: false }); break;
        case "quit": stop(); break;
        case "": break;
        default: report({ type: "companionControlHelp", commands: ["teach", "recall", "resume", "quit"] });
      }
    } catch { report({ type: "companionControlError", message: "Local control was not applied; check the companion and its memory store." }); }
  });
  report({ type: "companionControlReady", commands: ["teach", "recall", "resume", "quit"] });
  return () => lines.close();
}
