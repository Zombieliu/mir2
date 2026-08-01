import { ORIGINAL_UI } from "./original-ui";

export type AssetCacheScenePrewarm = {
  url: string;
  label: string;
  spriteFrameLimit: number;
};

export type AssetPrewarmStage = "login" | "character-select" | "game";

export type AssetCachePack = {
  name:
    "login" | "bichon-spawn" | "character-select" | "hud-core" | "login-audio";
  label: string;
  priority: number;
  stage: AssetPrewarmStage;
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
    stage: "login",
    cacheTier: "critical",
    // ChrSel/0..18 is roughly 40 MiB and is rendered lazily by the login scene.
    // Keep the blocking prewarm limited to the small interactive login shell.
    urls: uniqueUrls([
      ...collectStaticAssetUrls({
        dialog: ORIGINAL_UI.login.dialog,
        title: ORIGINAL_UI.login.title,
        accountLabel: ORIGINAL_UI.login.accountLabel,
        passwordLabel: ORIGINAL_UI.login.passwordLabel,
        buttons: ORIGINAL_UI.login.buttons,
      }),
    ]),
  },
  {
    name: "bichon-spawn",
    label: "Bichon spawn scene",
    priority: 15,
    stage: "game",
    phase: "background",
    cacheTier: "background",
    // Prewarm the packed entity atlas and the actual production Bichon entry footprint.
    // StartGame currently places Scout near 398,333, so warming the old login
    // showcase center still left the first playable scene to cold-load.
    urls: [
      "/original-ui/MMap/0.png",
      "/bevy-entity-atlases/manifest.json",
      "/bevy-entity-atlases/starter-bichon-base.png",
      // Map-atlas index (opt-in GPU map rendering). Atlas pages are SW cache-first on
      // first use; the index is tiny and lets the renderer resolve tiles immediately.
      "/generated/map-atlas/manifest.json",
    ],
    scenes: [
      {
        label: "BichonProvince spawn",
        url: "/api/scene/crystal?map=0&x=398&y=333&width=56&height=72",
        spriteFrameLimit: 960,
      },
    ],
  },
  {
    name: "character-select",
    label: "Character select",
    priority: 20,
    stage: "character-select",
    phase: "background",
    cacheTier: "background",
    urls: uniqueUrls([...collectStaticAssetUrls(ORIGINAL_UI.select)]),
  },
  {
    name: "hud-core",
    label: "Core HUD",
    priority: 30,
    stage: "game",
    phase: "background",
    cacheTier: "background",
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
    name: "login-audio",
    label: "Login and select audio",
    priority: 60,
    stage: "character-select",
    phase: "background",
    cacheTier: "background",
    urls: uniqueUrls([
      "/original-ui/Sound/Login2.wav",
      "/original-ui/Sound/100.wav",
      "/original-ui/Sound/Select2.wav",
      "/original-ui/Sound/NewChar.wav",
    ]),
  },
];

export function selectAssetCachePacksForStage(
  stage: AssetPrewarmStage,
  packs: readonly AssetCachePack[] = ASSET_CACHE_PACKS,
): AssetCachePack[] {
  return packs
    .filter((pack) => pack.stage === stage)
    .sort((left, right) => left.priority - right.priority);
}

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
