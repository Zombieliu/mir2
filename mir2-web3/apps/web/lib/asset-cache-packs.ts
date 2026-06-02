import { ORIGINAL_UI } from "./original-ui";

export type AssetCacheScenePrewarm = {
  url: string;
  label: string;
  spriteFrameLimit: number;
};

export type AssetCachePack = {
  name: "login" | "character-select" | "hud-core" | "bichon-spawn";
  label: string;
  priority: number;
  phase?: "critical" | "background";
  cacheTier?: "critical" | "background";
  urls: string[];
  scenes?: AssetCacheScenePrewarm[];
};

export const ASSET_CACHE_PACKS: AssetCachePack[] = [
  {
    name: "login",
    label: "Login shell",
    priority: 10,
    cacheTier: "critical",
    urls: uniqueUrls([
      ...collectStaticAssetUrls(ORIGINAL_UI.login),
      "/original-ui/Sound/Login2.wav",
      "/original-ui/Sound/100.wav",
    ]),
  },
  {
    name: "character-select",
    label: "Character select",
    priority: 20,
    cacheTier: "critical",
    urls: uniqueUrls([
      ...collectStaticAssetUrls(ORIGINAL_UI.select),
      "/original-ui/Sound/Select2.wav",
      "/original-ui/Sound/NewChar.wav",
    ]),
  },
  {
    name: "hud-core",
    label: "Core HUD",
    priority: 30,
    cacheTier: "critical",
    urls: uniqueUrls([
      ...collectStaticAssetUrls(ORIGINAL_UI.hud),
      ...collectStaticAssetUrls({
        chatDialog: ORIGINAL_UI.game.chatDialog,
        chatControlBar: ORIGINAL_UI.game.chatControlBar,
        chatCountBar: ORIGINAL_UI.game.chatCountBar,
        chatFilterButtons: ORIGINAL_UI.game.chatFilterButtons,
        miniMap: ORIGINAL_UI.game.miniMap,
        miniMapSmall: ORIGINAL_UI.game.miniMapSmall,
        miniMapButtons: ORIGINAL_UI.game.miniMapButtons,
        miniMapIcons: ORIGINAL_UI.game.miniMapIcons,
        questIcons: ORIGINAL_UI.game.questIcons,
        belt: ORIGINAL_UI.game.belt,
      }),
      "/original-ui/MMap/0.png",
    ]),
  },
  {
    name: "bichon-spawn",
    label: "Bichon spawn scene",
    priority: 40,
    cacheTier: "critical",
    // Prewarm the packed entity atlas and the actual production Bichon entry footprint.
    // StartGame currently places Scout near 398,333, so warming the old login
    // showcase center still left the first playable scene to cold-load.
    urls: [
      "/original-ui/MMap/0.png",
      "/bevy-entity-atlases/manifest.json",
      "/bevy-entity-atlases/starter-bichon-base.png",
    ],
    scenes: [
      {
        label: "BichonProvince spawn",
        url: "/api/scene/crystal?map=0&x=398&y=333&width=56&height=72",
        spriteFrameLimit: 960,
      },
    ],
  },
];

function collectStaticAssetUrls(value: unknown, urls: string[] = []) {
  if (typeof value === "string") {
    if (value.startsWith("/original-ui/")) {
      urls.push(value);
    }
    return urls;
  }

  if (Array.isArray(value)) {
    for (const item of value) collectStaticAssetUrls(item, urls);
    return urls;
  }

  if (value && typeof value === "object") {
    for (const item of Object.values(value)) collectStaticAssetUrls(item, urls);
  }

  return urls;
}

function uniqueUrls(urls: string[]) {
  return Array.from(new Set(urls));
}
