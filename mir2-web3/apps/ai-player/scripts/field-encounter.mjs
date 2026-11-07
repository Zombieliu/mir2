import { nextWalkDirection } from "../src/skills/navigation.mjs";
import { findCollisionGridPath } from "../../web/scripts/quest-agent/policy.mjs";

/** Size the test's finite travel window from the public path, not straight-line distance. */
export function homeReturnBudget({ player, home, collision }) {
  const path = findCollisionGridPath({ start: player, target: home, desiredDistance: 0,
    bounds: { minX: 0, minY: 0, maxX: collision.width - 1, maxY: collision.height - 1 },
    blocked: [...collision.blocked], occupied: [] });
  if (!path || path.length - 1 > 96) return null;
  const pathSteps = path.length - 1;
  // Current AOI occupants can move while the controller replans. This is a
  // finite allowance, never permission to bypass them or alter the clock.
  return { pathSteps, occupancyAllowance: 16, maxTicks: pathSteps + 16 };
}

/** Bounded test setup using public collision and the current ordinary AOI only. */
export function fieldApproachStep({ player, monster, collision, occupied = [], home = { x: 288, y: 616 }, safeRadius = 10 }) {
  const distance = (a,b) => Math.max(Math.abs(a.x-b.x), Math.abs(a.y-b.y));
  const candidates = [[-1,-1], [0,-1], [1,-1], [-1,0], [1,0], [-1,1], [0,1], [1,1]]
    .map(([x,y]) => ({ x: monster.x+x, y: monster.y+y }))
    .filter(tile => distance(tile, home) > safeRadius && !collision.blocked.has(`${tile.x},${tile.y}`)
      && !occupied.some(value => distance(value,tile) === 0))
    .sort((a,b) => distance(player,a)-distance(player,b));
  for (const tile of candidates) {
    const direction = nextWalkDirection({ player, target: tile, desiredDistance: 0, collision, occupied });
    if (direction) return { tile, direction };
  }
  return null;
}
