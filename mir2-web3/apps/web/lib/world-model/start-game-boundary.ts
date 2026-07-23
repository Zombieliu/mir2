import type { WorldState } from "./types";

/** Establish a fresh packet population before StartGame bootstrap packets. */
export function resetWorldPopulationForStartGame(current: WorldState): WorldState {
  return {
    ...current,
    selectedObjectId: null,
    activeNpcDialog: null,
    entities: current.playerObjectId
      ? current.entities.filter((entity) => entity.objectId === current.playerObjectId)
      : [],
    groundDrops: [],
  };
}
