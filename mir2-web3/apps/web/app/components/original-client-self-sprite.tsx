"use client";

import { useEffect, useLayoutEffect, useMemo, useState, useSyncExternalStore, type ReactNode } from "react";
import { originalAssetPath } from "../../lib/asset-url";
import type { R16EntrySpriteImages } from "../../lib/r16-entry-scene";
import { loadSelfSpriteImage, type DecodedSelfSpriteImage } from "../../lib/self-sprite-image";
import {
  SelfSpriteResidency, type SelfSpriteLayer, type SelfSpritePose, type ResidentSelfSpritePose,
} from "../../lib/self-sprite-residency";
import { sceneAssetCandidateUrls, type ViewportEntitySprite } from "./original-client-scene-rendering";
import { r16StandingGroup, readR16StandingSprites, type R16StandingImages } from "../../lib/r16-standing-sprites";

export type ResidentActorImage = DecodedSelfSpriteImage & { key: string };
export type ResidentActorImages = ReadonlyMap<string, ResidentActorImage>;
const emptySnapshot = () => null;

export function selfSpritePose(sprite: ViewportEntitySprite | null, packed: R16EntrySpriteImages | null,
  initial: R16StandingImages | null = null): SelfSpritePose | null {
  if (!sprite?.body) return null;
  const layers: SelfSpriteLayer[] = [];
  const add = (role: string, frame: NonNullable<ViewportEntitySprite["body"]> | null) => {
    if (frame) {
      const image = initial?.[frame.path];
      const png = image?.width === frame.width && image.height === frame.height ? image.png : undefined;
      layers.push({ ...frame, role, source: packed?.[frame.path] ?? png ?? originalAssetPath(frame.path) });
    }
  };
  add("mount", sprite.mount); sprite.rearWeapons.forEach((frame, index) => add("rear:" + index, frame));
  add("body", sprite.body); add("hair", sprite.hair);
  sprite.frontWeapons.forEach((frame, index) => add("front:" + index, frame));
  return { key: JSON.stringify([sprite.nameplateTop, layers.map(frame =>
    [frame.role, frame.source, frame.x, frame.y, frame.width, frame.height, frame.shadowX, frame.shadowY, frame.maskPath])]),
    layers, nameplateTop: sprite.nameplateTop };
}

export function selfStandingWarmFrames(sprite: ViewportEntitySprite | null, packed: R16EntrySpriteImages | null,
  initial: R16StandingImages | null = null): SelfSpriteLayer[] {
  const body = sprite?.body;
  if (!body || packed?.[body.path]) return [];
  const match = body.path.match(/^(\/original-ui\/CArmour\/[^/]+\/)(\d+)\.png$/);
  if (!match) return [];
  const index = Number(match[2]), base = index >= 808 ? 808 : 0;
  if (index - base >= 32) return [];
  const first = base + Math.floor((index - base) / 4) * 4;
  return (sprite?.preloadFrames ?? []).filter(frame => {
    if (!frame.path.startsWith(match[1])) return false;
    const index = Number(frame.path.slice(match[1].length, -4));
    return Number.isInteger(index) && index >= first && index < first + 4;
  }).slice(0, 4).map(frame => {
    const image = initial?.[frame.path];
    const png = image?.width === frame.width && image.height === frame.height ? image.png : undefined;
    return { ...frame, role: "body", source: png ?? originalAssetPath(frame.path) };
  });
}

function displayedSprite(target: ViewportEntitySprite | null, pose: ResidentSelfSpritePose | null): ViewportEntitySprite | null {
  if (!target || !pose) return target ? { ...target, body: null, hair: null, mount: null, rearWeapons: [], frontWeapons: [] } : null;
  const layer = (role: string) => pose.layers.find(frame => frame.role === role) ?? null;
  return { ...target, body: layer("body"), hair: layer("hair"), mount: layer("mount"), nameplateTop: pose.nameplateTop,
    rearWeapons: pose.layers.filter(frame => frame.role.startsWith("rear:")),
    frontWeapons: pose.layers.filter(frame => frame.role.startsWith("front:")) };
}

/** Only the self actor uses this path; other actors retain their exact picking geometry. */
export function DecodedSelfSprite({ sprite, packedActors, assetReleaseId, children }: {
  sprite: ViewportEntitySprite | null;
  packedActors: R16EntrySpriteImages | null;
  assetReleaseId: string | null;
  children: (sprite: ViewportEntitySprite | null, images: ResidentActorImages | undefined,
    failed: (key: string, url: string) => void) => ReactNode;
}) {
  const residency = useMemo(() => new SelfSpriteResidency((frame, signal) =>
    loadSelfSpriteImage(frame, signal, frame.source.startsWith("data:") ? [frame.source] : sceneAssetCandidateUrls(frame.source))), [assetReleaseId]);
  const snapshot = useSyncExternalStore(residency.subscribe, residency.getSnapshot, emptySnapshot);
  const group = r16StandingGroup(assetReleaseId, sprite?.body?.path);
  const completeBody = Boolean(sprite?.body && packedActors?.[sprite.body.path]);
  const [initial, setInitial] = useState<{ key: string; images: R16StandingImages } | null>(null);
  useEffect(() => {
    if (!group || completeBody) return;
    const owner = new AbortController();
    let retry: ReturnType<typeof setTimeout> | undefined;
    let observations = 0;
    const observe = async () => {
      observations++;
      const images = await readR16StandingSprites(group, owner.signal);
      if (owner.signal.aborted) return;
      if (images) setInitial({ key: group.key, images });
      else if (observations < 2) retry = setTimeout(() => { void observe(); }, 5_100);
    };
    void observe();
    return () => { owner.abort(); clearTimeout(retry); };
  }, [group?.key, completeBody]); // eslint-disable-line react-hooks/exhaustive-deps
  const images = group?.key === initial?.key ? initial?.images ?? null : null;
  const target = selfSpritePose(sprite, packedActors, images);
  const warm = selfStandingWarmFrames(sprite, packedActors, images);
  const warmKey = warm.map(frame => frame.source).join("\n");
  useLayoutEffect(() => { residency.start(); return () => residency.stop(); }, [residency]);
  // Geometry/source identity changes drive requests; the 60Hz position clock does not.
  useLayoutEffect(() => { residency.setTarget(target, warm); }, [residency, target?.key, warmKey]); // eslint-disable-line react-hooks/exhaustive-deps
  useLayoutEffect(() => { residency.presentedPose(snapshot); }, [residency, snapshot]);
  return children(displayedSprite(sprite, snapshot), snapshot?.images, residency.invalidate);
}
