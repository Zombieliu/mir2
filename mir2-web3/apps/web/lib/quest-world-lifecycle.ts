import { normalizeQuestMapFileName, questMapIndex, sameQuestWorldIdentity,
  type QuestWorldIdentity } from "./bevy-quest-world-context";

export type QuestMapAuthority = Readonly<{
  connectionGeneration: number; sessionGeneration: number; sceneRevision: number;
  mapFileName: string; mapIndex: number;
}>;

/** Map image numbers are not map identities. Only the ordinary map packet supplies this index. */
export function observeQuestMapPacket(
  previous: QuestMapAuthority | null, packet: "MapInformation" | "MapChanged",
  payload: Readonly<Record<string, unknown>>, connectionGeneration: number,
  sessionGeneration: number, sceneRevision: number, currentFileName: string | null,
): { authority: QuestMapAuthority | null; sceneRevision: number; retired: boolean } {
  const file = normalizeQuestMapFileName(typeof payload.fileName === "string" ? payload.fileName : null);
  const valid = file !== null && questMapIndex(payload.mapIndex)
    && Number.isSafeInteger(connectionGeneration) && connectionGeneration > 0
    && Number.isSafeInteger(sessionGeneration) && sessionGeneration > 0
    && Number.isSafeInteger(sceneRevision) && sceneRevision > 0;
  const retired = packet === "MapChanged" || !valid
    || file !== normalizeQuestMapFileName(currentFileName)
    || previous !== null && (previous.mapFileName !== file || previous.mapIndex !== payload.mapIndex
      || previous.connectionGeneration !== connectionGeneration || previous.sessionGeneration !== sessionGeneration);
  const nextRevision = retired ? sceneRevision < Number.MAX_SAFE_INTEGER ? sceneRevision + 1 : 0 : sceneRevision;
  return { sceneRevision: nextRevision, retired,
    authority: valid && nextRevision > 0 ? { connectionGeneration, sessionGeneration,
      sceneRevision: nextRevision, mapFileName: file, mapIndex: payload.mapIndex as number } : null };
}

export function currentQuestMapIndex(authority: QuestMapAuthority | null, identity: QuestWorldIdentity | null): number | null {
  return authority && identity && authority.connectionGeneration === identity.connectionGeneration
    && authority.sessionGeneration === identity.sessionGeneration && authority.sceneRevision === identity.sceneRevision
    && authority.mapFileName === identity.mapFileName ? authority.mapIndex : null;
}

export type QuestWorldLifetime = Readonly<{
  identity: QuestWorldIdentity; generation: number; runtime: object; runtimeGeneration: number;
  socket: object; questSource: string;
}>;

/** A captured local handoff does not survive a renderer, socket, scene, owner or quest change. */
export function sameQuestWorldLifetime(captured: QuestWorldLifetime, live: QuestWorldLifetime | null): boolean {
  return live !== null && sameQuestWorldIdentity(captured.identity, live.identity)
    && captured.generation === live.generation && captured.runtime === live.runtime
    && captured.runtimeGeneration === live.runtimeGeneration && captured.socket === live.socket
    && captured.questSource === live.questSource;
}

export type QuestAttackTarget = Readonly<{ objectId: string; kind: string; name: string;
  dead?: boolean | null; hp?: number | null; maxHp?: number | null }>;

/** Recheck raw target facts before handing the gesture to the common combat controller. */
export function questAttackTargetCurrent(target: QuestAttackTarget | undefined, objectId: number,
  role: "attackTarget" | "attackQuestTarget", selectedObjectId: string | null, name?: string): boolean {
  return Boolean(target && target.objectId === String(objectId) && target.kind === "monster"
    && target.dead === false && (name === undefined || target.name === name)
    && (role !== "attackTarget" || selectedObjectId === target.objectId
      && target.hp != null && target.maxHp != null && target.hp > 0 && target.maxHp > 0)
    && (target.hp == null || Number.isInteger(target.hp) && target.hp > 0)
    && (target.maxHp == null || Number.isInteger(target.maxHp) && target.maxHp > 0)
    && !(target.maxHp != null && target.maxHp > 0 && target.hp == null));
}
