"use client";

export type DistributionChannel = "direct" | "itch" | "crazyGames" | "steam";

export type ChannelIdentityCredential =
  | {
      kind: "authenticated";
      provider: "crazyGames" | "steam";
      credential: string;
    }
  | {
      kind: "guest";
      provider: "directGuest" | "itch" | "crazyGamesGuest";
    };

export type ChannelAdResult = {
  completed: boolean;
  error?: string;
};

export type ChannelContext = {
  channel: DistributionChannel;
  sdkAvailable: boolean;
  accountAvailable: boolean;
};

export interface ChannelAdapter {
  readonly channel: DistributionChannel;
  initialize(): Promise<ChannelContext>;
  identity(): Promise<ChannelIdentityCredential>;
  loadingStart(): void;
  loadingStop(): void;
  gameplayStart(): void;
  gameplayStop(): void;
  showRewardedAd(): Promise<ChannelAdResult>;
  subscribeIdentityChanges(listener: () => void): () => void;
}

type CrazyGamesSdk = {
  init(): Promise<void>;
  game: {
    loadingStart(): void;
    loadingStop(): void;
    gameplayStart(): void;
    gameplayStop(): void;
  };
  ad: {
    requestAd(adType: "midgame" | "rewarded", callbacks: {
      adStarted?: () => void;
      adFinished?: () => void;
      adError?: (error: unknown) => void;
    }): void;
  };
  user: {
    isUserAccountAvailable: boolean;
    getUserToken(): Promise<string>;
    addAuthListener(listener: () => void): void;
    removeAuthListener(listener: () => void): void;
  };
};

declare global {
  interface Window {
    CrazyGames?: { SDK: CrazyGamesSdk };
    __mir2ChannelBridge?: {
      context: ChannelContext | null;
      gameplayActive: boolean;
    };
  }
}

class DirectChannelAdapter implements ChannelAdapter {
  readonly channel: DistributionChannel;

  constructor(channel: "direct" | "itch") {
    this.channel = channel;
  }

  async initialize(): Promise<ChannelContext> {
    return {
      channel: this.channel,
      sdkAvailable: false,
      accountAvailable: false,
    };
  }

  async identity(): Promise<ChannelIdentityCredential> {
    return {
      kind: "guest",
      provider: this.channel === "itch" ? "itch" : "directGuest",
    };
  }

  loadingStart() {}
  loadingStop() {}
  gameplayStart() {}
  gameplayStop() {}

  async showRewardedAd(): Promise<ChannelAdResult> {
    return { completed: false, error: "rewarded ads are unavailable on this channel" };
  }

  subscribeIdentityChanges() {
    return () => {};
  }
}

/// Steam adapter for the Tauri desktop shell.
///
/// When the game runs inside the desktop launcher, the launcher exposes a
/// `steam_auth_ticket` Tauri command. This adapter detects the Tauri bridge,
/// fetches the ticket, and returns it as the authenticated credential the
/// Gateway exchanges via `/api/channels/session/exchange` (provider `"steam"`).
///
/// Outside the desktop shell (web / mobile / itch / crazyGames), Steam is not a
/// channel: the game falls back to direct/guest login.
class SteamDesktopChannelAdapter implements ChannelAdapter {
  readonly channel = "steam" as const;
  private ticket: string | null = null;

  async initialize(): Promise<ChannelContext> {
    this.ticket = await fetchSteamAuthTicket();
    return {
      channel: this.channel,
      sdkAvailable: this.ticket !== null,
      accountAvailable: this.ticket !== null,
    };
  }

  async identity(): Promise<ChannelIdentityCredential> {
    if (!this.ticket) {
      return { kind: "guest", provider: "directGuest" };
    }
    return { kind: "authenticated", provider: "steam", credential: this.ticket };
  }

  loadingStart() {}
  loadingStop() {}
  gameplayStart() {}
  gameplayStop() {}

  async showRewardedAd(): Promise<ChannelAdResult> {
    return { completed: false, error: "rewarded ads are unavailable on Steam" };
  }

  subscribeIdentityChanges() {
    return () => {};
  }
}

/// Whether the page runs inside the Tauri desktop shell.
function inTauriShell(): boolean {
  return (
    typeof window !== "undefined" &&
    typeof (window as unknown as { __TAURI__?: unknown }).__TAURI__ !== "undefined"
  );
}

/// Fetch the Steam auth ticket from the Tauri launcher, if present.
async function fetchSteamAuthTicket(): Promise<string | null> {
  if (!inTauriShell()) return null;
  try {
    // @ts-expect-error -- the Tauri global is injected by the desktop shell.
    const ticket = await window.__TAURI__.core.invoke<string | null>("steam_auth_ticket");
    return ticket && ticket.trim() ? ticket : null;
  } catch (error) {
    console.warn("[channel-bridge] steam_auth_ticket unavailable:", error);
    return null;
  }
}

class CrazyGamesChannelAdapter implements ChannelAdapter {
  readonly channel = "crazyGames" as const;
  private sdk: CrazyGamesSdk | null = null;

  async initialize(): Promise<ChannelContext> {
    await loadCrazyGamesSdk();
    const sdk = window.CrazyGames?.SDK;
    if (!sdk) throw new Error("CrazyGames SDK did not initialize");
    await sdk.init();
    this.sdk = sdk;
    sdk.game.loadingStart();
    return {
      channel: this.channel,
      sdkAvailable: true,
      accountAvailable: sdk.user.isUserAccountAvailable,
    };
  }

  async identity(): Promise<ChannelIdentityCredential> {
    if (!this.sdk?.user.isUserAccountAvailable) {
      return { kind: "guest", provider: "crazyGamesGuest" };
    }
    try {
      return {
        kind: "authenticated",
        provider: "crazyGames",
        credential: await this.sdk.user.getUserToken(),
      };
    } catch (error) {
      if (crazyGamesErrorCode(error) === "userNotAuthenticated") {
        return { kind: "guest", provider: "crazyGamesGuest" };
      }
      throw error;
    }
  }

  loadingStart() {
    this.sdk?.game.loadingStart();
  }

  loadingStop() {
    this.sdk?.game.loadingStop();
  }

  gameplayStart() {
    this.sdk?.game.gameplayStart();
  }

  gameplayStop() {
    this.sdk?.game.gameplayStop();
  }

  showRewardedAd(): Promise<ChannelAdResult> {
    if (!this.sdk) {
      return Promise.resolve({ completed: false, error: "CrazyGames SDK is unavailable" });
    }
    return new Promise((resolve) => {
      this.sdk!.ad.requestAd("rewarded", {
        adFinished: () => resolve({ completed: true }),
        adError: (error) =>
          resolve({
            completed: false,
            error: error instanceof Error ? error.message : String(error),
          }),
      });
    });
  }

  subscribeIdentityChanges(listener: () => void) {
    if (!this.sdk) return () => {};
    this.sdk.user.addAuthListener(listener);
    return () => this.sdk?.user.removeAuthListener(listener);
  }
}

let adapterPromise: Promise<ChannelAdapter> | null = null;
let channelContext: ChannelContext | null = null;
let gameplayActive = false;

export function detectedDistributionChannel(location = window.location): DistributionChannel {
  const requested = new URLSearchParams(location.search).get("channel")?.trim().toLowerCase();
  if (requested === "steam" || requested === "steamworks") return "steam";
  if (requested === "crazygames" || requested === "crazy-games") return "crazyGames";
  if (requested === "itch" || requested === "itch.io") return "itch";
  if (new URLSearchParams(location.search).get("isCrazyGames") === "true") return "crazyGames";
  if (/itch\.(?:io|zone)$/iu.test(location.hostname)) return "itch";
  if (inTauriShell()) return "steam";
  return "direct";
}

export async function channelAdapter(): Promise<ChannelAdapter> {
  if (!adapterPromise) {
    const channel = detectedDistributionChannel();
    adapterPromise = Promise.resolve(
      channel === "crazyGames"
        ? new CrazyGamesChannelAdapter()
        : channel === "steam"
          ? new SteamDesktopChannelAdapter()
          : new DirectChannelAdapter(channel),
    );
  }
  return adapterPromise;
}

export async function initializeChannelBridge(): Promise<ChannelContext> {
  const adapter = await channelAdapter();
  adapter.loadingStart();
  channelContext = await adapter.initialize();
  publishDebugState();
  return channelContext;
}

export async function channelIdentity(): Promise<ChannelIdentityCredential> {
  return (await channelAdapter()).identity();
}

export async function channelLoadingFinished() {
  (await channelAdapter()).loadingStop();
}

export async function channelGameplayStart() {
  if (gameplayActive) return;
  gameplayActive = true;
  (await channelAdapter()).gameplayStart();
  publishDebugState();
}

export async function channelGameplayStop() {
  if (!gameplayActive) return;
  gameplayActive = false;
  (await channelAdapter()).gameplayStop();
  publishDebugState();
}

export async function subscribeChannelIdentityChanges(listener: () => void) {
  return (await channelAdapter()).subscribeIdentityChanges(listener);
}

function publishDebugState() {
  window.__mir2ChannelBridge = {
    context: channelContext,
    gameplayActive,
  };
}

function crazyGamesErrorCode(error: unknown) {
  if (typeof error === "object" && error && "code" in error) {
    return String((error as { code?: unknown }).code ?? "");
  }
  return "";
}

async function loadCrazyGamesSdk() {
  if (window.CrazyGames?.SDK) return;
  const existing = document.querySelector<HTMLScriptElement>(
    'script[data-mir2-channel-sdk="crazygames"]',
  );
  if (existing) {
    await scriptReady(existing);
    return;
  }
  const script = document.createElement("script");
  script.src = "https://sdk.crazygames.com/crazygames-sdk-v3.js";
  script.async = true;
  script.dataset.mir2ChannelSdk = "crazygames";
  document.head.appendChild(script);
  await scriptReady(script);
}

function scriptReady(script: HTMLScriptElement) {
  return new Promise<void>((resolve, reject) => {
    if (window.CrazyGames?.SDK) {
      resolve();
      return;
    }
    if (script.dataset.mir2Loaded === "true") {
      reject(new Error("CrazyGames SDK loaded without exposing its API"));
      return;
    }
    script.addEventListener(
      "load",
      () => {
        script.dataset.mir2Loaded = "true";
        resolve();
      },
      { once: true },
    );
    script.addEventListener("error", () => reject(new Error("CrazyGames SDK load failed")), {
      once: true,
    });
  });
}
