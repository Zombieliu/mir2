"use client";

import {
  getWallets,
  isWalletWithRequiredFeatureSet,
  SuiSignPersonalMessage,
  type Wallet,
  type WalletAccount,
  type SuiSignPersonalMessageFeature,
} from "@mysten/wallet-standard";
import { normalizeSuiAddress } from "@mysten/sui/utils";

const PASSKEY_STORAGE_KEY = "mir2.suiPasskey";
const SUI_LOGIN_PURPOSE = "mir2-sui-login";
export const DUBHE_WALLET_URL = "https://dubhe.obelisk.build/en/wallet";

export type SuiWalletSummary = {
  id: string;
  name: string;
  icon?: string;
  isDubhe: boolean;
};

type StoredPasskey = {
  publicKey: string;
  credentialId?: string;
};

export type SuiLoginToken = {
  accountId: string;
  playerId?: string;
  provider?:
    | "suiPasskey"
    | "suiWallet"
    | "crazyGames"
    | "crazyGamesGuest"
    | "directGuest"
    | "itch";
  token: string;
  expiresAt: number;
};

export type SuiIdentityCredential = {
  provider: "suiPasskey" | "suiWallet";
  credential: string;
};

type SuiLoginChallenge = {
  challenge: string;
  challengeId: string;
  issuedAt: number;
  expiresAt: number;
};

/**
 * The wallet + account a successful wallet login connected, retained so the on-chain
 * mining flow (lib/onchain-mine-session.ts) can sign transactions with the SAME wallet
 * without a second connect prompt (M4, WF-6). Module-local, never persisted.
 */
export type ActiveSuiWalletSession = {
  wallet: Wallet;
  account: WalletAccount;
  /** e.g. "sui:testnet" — the account's first sui chain. */
  chain: `sui:${string}`;
};

let activeWalletSession: ActiveSuiWalletSession | null = null;

/** The wallet session retained by the last wallet login/connect, if any (passkey logins have none). */
export function getActiveSuiWalletSession(): ActiveSuiWalletSession | null {
  return activeWalletSession;
}

/**
 * Connect (or re-connect) a Sui wallet WITHOUT logging in, retaining it for transaction
 * signing — used when the game session was authenticated some other way (passkey,
 * reconnect token) and the player enables on-chain mining afterwards.
 */
export async function connectSuiWalletForSigning(walletId?: string): Promise<ActiveSuiWalletSession> {
  const wallets = discoverSuiWallets();
  const wallet = selectSuiWallet(wallets, walletId);
  if (!wallet) {
    throw new Error("No Sui wallet found");
  }
  const connected = await wallet.features["standard:connect"].connect();
  const account = pickSuiAccount(connected.accounts, wallet.name);
  return retainWalletSession(wallet, account);
}

function pickSuiAccount(accounts: readonly WalletAccount[], walletName: string): WalletAccount {
  const account =
    accounts.find((entry) => entry.features.includes(SuiSignPersonalMessage)) ??
    accounts.find((entry) => entry.chains?.some((chain) => chain.startsWith("sui:"))) ??
    accounts[0];
  if (!account?.address) {
    throw new Error(`${walletName} did not return a Sui account`);
  }
  return account;
}

function retainWalletSession(wallet: Wallet, account: WalletAccount): ActiveSuiWalletSession {
  const chain = (account.chains?.find((entry) => entry.startsWith("sui:")) ??
    "sui:testnet") as `sui:${string}`;
  activeWalletSession = { wallet, account, chain };
  return activeWalletSession;
}

export async function requestPasskeyLoginToken(): Promise<SuiLoginToken> {
  const identity = await requestPasskeyIdentityCredential();
  return exchangeChannelSession(identity.provider, identity.credential);
}

export async function requestPasskeyIdentityCredential(): Promise<SuiIdentityCredential> {
  if (!("credentials" in navigator) || !window.PublicKeyCredential) {
    throw new Error("Passkey is not supported by this browser");
  }

  const { BrowserPasskeyProvider, PasskeyKeypair } = await import("@mysten/sui/keypairs/passkey");
  const provider = new BrowserPasskeyProvider("Mir2 Web3", {
    rp: {
      id: window.location.hostname,
    },
    authenticatorSelection: {
      authenticatorAttachment: "platform",
      residentKey: "required",
      requireResidentKey: true,
      userVerification: "required",
    },
  });

  const stored = readStoredPasskey();
  const keypair = stored
    ? new PasskeyKeypair(bytesFromBase64Url(stored.publicKey), provider, stored.credentialId ? bytesFromBase64Url(stored.credentialId) : undefined)
    : await PasskeyKeypair.getPasskeyInstance(provider);

  if (!stored) {
    writeStoredPasskey({
      publicKey: bytesToBase64Url(keypair.getPublicKey().toRawBytes()),
      credentialId: keypair.getCredentialId() ? bytesToBase64Url(keypair.getCredentialId()!) : undefined,
    });
  }

  const address = normalizeSuiAddress(keypair.getPublicKey().toSuiAddress());
  const challenge = await requestSuiLoginChallenge(address, "passkey");
  const message = buildSuiLoginMessage(address, challenge, "passkey");
  const { signature } = await keypair.signPersonalMessage(new TextEncoder().encode(message));

  const proof = await requestSuiLoginToken(address, message, signature);
  return { provider: "suiPasskey", credential: proof.token };
}

export async function requestWalletLoginToken(walletId?: string): Promise<SuiLoginToken> {
  const identity = await requestWalletIdentityCredential(walletId);
  return exchangeChannelSession(identity.provider, identity.credential);
}

export async function requestWalletIdentityCredential(
  walletId?: string,
): Promise<SuiIdentityCredential> {
  const wallets = discoverSuiWallets();
  const wallet = selectSuiWallet(wallets, walletId);
  if (!wallet) {
    throw new Error("No Sui wallet found");
  }

  const connected = await wallet.features["standard:connect"].connect();
  const account = pickSuiAccount(connected.accounts, wallet.name);
  // Retain the connected wallet so on-chain mining can sign with it later (M4, WF-6).
  retainWalletSession(wallet, account);

  const address = normalizeSuiAddress(account.address);
  const challenge = await requestSuiLoginChallenge(address, "wallet");
  const message = buildSuiLoginMessage(address, challenge, "wallet");
  const chain = account.chains?.find((entry) => entry.startsWith("sui:"));
  const { signature } = await wallet.features[SuiSignPersonalMessage].signPersonalMessage({
    account,
    message: new TextEncoder().encode(message),
    chain,
  });

  const proof = await requestSuiLoginToken(address, message, signature);
  return { provider: "suiWallet", credential: proof.token };
}

export function getSuiWalletSummaries(): SuiWalletSummary[] {
  return discoverSuiWallets()
    .map((wallet) => ({
      id: walletIdentifier(wallet),
      name: wallet.name,
      icon: wallet.icon,
      isDubhe: isDubheWallet(wallet),
    }))
    .sort((left, right) => {
      if (left.isDubhe !== right.isDubhe) return left.isDubhe ? -1 : 1;
      return left.name.localeCompare(right.name);
    });
}

export function subscribeToSuiWalletChanges(listener: () => void) {
  const wallets = getWallets();
  const offRegister = wallets.on("register", listener);
  const offUnregister = wallets.on("unregister", listener);
  return () => {
    offRegister();
    offUnregister();
  };
}

function buildSuiLoginMessage(
  address: string,
  challenge: SuiLoginChallenge,
  authMethod: "passkey" | "wallet",
) {
  return JSON.stringify({
    purpose: SUI_LOGIN_PURPOSE,
    accountId: `sui:${address}`,
    address,
    origin: window.location.origin,
    issuedAt: challenge.issuedAt,
    expiresAt: challenge.expiresAt,
    nonce: challenge.challengeId,
    challenge: challenge.challenge,
    authMethod,
  });
}

async function requestSuiLoginChallenge(
  address: string,
  authMethod: "passkey" | "wallet",
): Promise<SuiLoginChallenge> {
  const response = await fetch("/api/passkey/challenge", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ address, authMethod }),
  });
  const body = (await response.json().catch(() => null)) as
    | SuiLoginChallenge
    | { error?: string }
    | null;
  if (!response.ok || !body || !("challenge" in body)) {
    throw new Error(body && "error" in body && body.error ? body.error : "Sui login challenge failed");
  }
  return body;
}

async function requestSuiLoginToken(address: string, message: string, signature: string) {
  const response = await fetch("/api/passkey/login", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ address, message, signature }),
  });
  const body = (await response.json().catch(() => null)) as SuiLoginToken | { error?: string } | null;
  if (!response.ok || !body || !("token" in body)) {
    throw new Error(body && "error" in body && body.error ? body.error : "Sui login failed");
  }
  return body;
}

async function exchangeChannelSession(
  provider:
    | "suiPasskey"
    | "suiWallet"
    | "crazyGames"
    | "crazyGamesGuest"
    | "directGuest"
    | "itch"
    | "steam",
  credential: string,
): Promise<SuiLoginToken> {
  const response = await fetch("/api/channels/session/exchange", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ provider, credential }),
  });
  const body = (await response.json().catch(() => null)) as
    | SuiLoginToken
    | { error?: string }
    | null;
  if (!response.ok || !body || !("token" in body) || !("accountId" in body)) {
    throw new Error(
      body && "error" in body && body.error ? body.error : "channel session exchange failed",
    );
  }
  return body;
}

export function requestChannelSessionToken(
  provider:
    | "suiPasskey"
    | "suiWallet"
    | "crazyGames"
    | "crazyGamesGuest"
    | "directGuest"
    | "itch"
    | "steam",
  credential: string,
) {
  return exchangeChannelSession(provider, credential);
}

export async function requestGuestChannelSessionToken(
  provider: "crazyGamesGuest" | "directGuest" | "itch",
) {
  const proofResponse = await fetch("/api/channels/guest", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ provider }),
  });
  const proof = (await proofResponse.json().catch(() => null)) as
    | { credential?: string; error?: string }
    | null;
  if (!proofResponse.ok || !proof?.credential) {
    throw new Error(proof?.error ?? "guest channel proof failed");
  }
  return exchangeChannelSession(provider, proof.credential);
}

export async function linkSuiIdentity(
  accountId: string,
  sessionToken: string,
  identity: SuiIdentityCredential,
) {
  return linkChannelIdentity(
    accountId,
    sessionToken,
    identity.provider,
    identity.credential,
  );
}

export async function linkChannelIdentity(
  accountId: string,
  sessionToken: string,
  provider: "suiPasskey" | "suiWallet" | "crazyGames" | "steam",
  credential: string,
) {
  const response = await fetch("/api/channels/identity/link", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({
      accountId,
      sessionToken,
      provider,
      credential,
    }),
  });
  const body = (await response.json().catch(() => null)) as
    | { accountId?: string; linkedProvider?: string; identityCount?: number; error?: string }
    | null;
  if (!response.ok || !body?.linkedProvider) {
    throw new Error(body?.error ?? "identity link failed");
  }
  return body;
}

/**
 * Re-announce `wallet-standard:app-ready` and forward any responses into the
 * `getWallets()` registry. Some extensions (Slush, Suiet) only RESPOND to app-ready
 * and never self-announce via `wallet-standard:register-wallet`; if they inject after
 * the registry's single app-ready dispatch at module init, they stay invisible
 * forever. Rescanning on every discovery closes that race (found in the M4 live e2e).
 * Already-known wallets are skipped so repeat scans cannot duplicate entries.
 */
function rescanWalletStandard() {
  if (typeof window === "undefined") return;
  const registry = getWallets();
  const known = new Set(registry.get().map((wallet) => `${wallet.name}:${wallet.version}`));
  try {
    window.dispatchEvent(
      new CustomEvent("wallet-standard:app-ready", {
        detail: {
          register: (...wallets: Wallet[]) => {
            const fresh = wallets.filter(
              (wallet) => !known.has(`${wallet.name}:${wallet.version}`),
            );
            fresh.forEach((wallet) => known.add(`${wallet.name}:${wallet.version}`));
            return fresh.length > 0 ? registry.register(...fresh) : () => {};
          },
        },
      }),
    );
  } catch {
    // A misbehaving wallet throwing during registration must not break login.
  }
}

function discoverSuiWallets() {
  rescanWalletStandard();
  return getWallets()
    .get()
    .filter((wallet) =>
      isWalletWithRequiredFeatureSet<SuiSignPersonalMessageFeature>(wallet, [
        SuiSignPersonalMessage,
      ]),
    );
}

function selectSuiWallet(wallets: ReturnType<typeof discoverSuiWallets>, walletId?: string) {
  if (!walletId) return wallets[0];
  return wallets.find((wallet) => walletIdentifier(wallet) === walletId || wallet.name === walletId) ?? null;
}

function walletIdentifier(wallet: Wallet) {
  return wallet.id ?? wallet.name;
}

function isDubheWallet(wallet: Wallet) {
  return /dubhe/iu.test(`${wallet.id ?? ""} ${wallet.name}`);
}

function readStoredPasskey(): StoredPasskey | null {
  try {
    const stored = localStorage.getItem(PASSKEY_STORAGE_KEY);
    return stored ? (JSON.parse(stored) as StoredPasskey) : null;
  } catch {
    return null;
  }
}

function writeStoredPasskey(passkey: StoredPasskey) {
  localStorage.setItem(PASSKEY_STORAGE_KEY, JSON.stringify(passkey));
}

function bytesToBase64Url(bytes: Uint8Array) {
  const binary = Array.from(bytes, (byte) => String.fromCharCode(byte)).join("");
  return btoa(binary).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/u, "");
}

function bytesFromBase64Url(value: string) {
  const padded = value.replace(/-/g, "+").replace(/_/g, "/").padEnd(Math.ceil(value.length / 4) * 4, "=");
  const binary = atob(padded);
  return Uint8Array.from(binary, (char) => char.charCodeAt(0));
}
