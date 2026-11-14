import type { QuestRoutePlan, QuestRoutePoint } from "./bevy-quest-route-plan";

export type QuestRouteStep = { type: "arrived" } | { type: "replan" } | {
  type: "move"; point: QuestRoutePoint; direction: string; mode: "walk" | "run";
};
const samePoint = (a: QuestRoutePoint, b: QuestRoutePoint) => a.x === b.x && a.y === b.y;
// Use the same canonical directions as the movement controller. Its run-to-walk
// confirmation computes the intermediate tile from this value before transport encoding.
const directions: Record<string, string> = { "0,-1": "Up", "1,-1": "UpRight", "1,0": "Right",
  "1,1": "DownRight", "0,1": "Down", "-1,1": "DownLeft", "-1,0": "Left", "-1,-1": "UpLeft" };

/** Follow only the complete common route. A deviation never resumes the old short-range planner. */
export function nextQuestRouteStep(plan: QuestRoutePlan, current: QuestRoutePoint,
  runDistance: 1 | 2 | 3, blocked: (from: QuestRoutePoint, to: QuestRoutePoint) => boolean): QuestRouteStep {
  const at = samePoint(current, plan.origin) ? -1 : plan.steps.findIndex(point => samePoint(point, current));
  if (at === -1 && !samePoint(current, plan.origin)) return { type: "replan" };
  const next = plan.steps[at + 1];
  if (!next) return samePoint(current, plan.destination) ? { type: "arrived" } : { type: "replan" };
  // Match Windows big_map_input::advance: inspect the next three route edges,
  // including bends, before committing even a one-tile walk or a two-tile run.
  let previous = current;
  for (const point of plan.steps.slice(at + 1, at + 4)) {
    if (!directions[`${point.x - previous.x},${point.y - previous.y}`] || blocked(previous, point)) {
      return { type: "replan" };
    }
    previous = point;
  }
  const dx = next.x - current.x, dy = next.y - current.y, direction = directions[`${dx},${dy}`];
  // A run requires every intermediate tile to be on the same straight route and currently free.
  if (runDistance > 1) {
    let previous = next, complete = true;
    for (let distance = 2; distance <= runDistance; distance++) {
      const point = plan.steps[at + distance];
      if (!point || point.x !== current.x + dx * distance || point.y !== current.y + dy * distance) {
        complete = false; break;
      }
      previous = point;
    }
    if (complete) return { type: "move", point: previous, direction, mode: "run" };
  }
  return { type: "move", point: next, direction, mode: "walk" };
}
