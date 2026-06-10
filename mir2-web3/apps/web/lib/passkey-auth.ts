"use client";

import {
  getWallets,
  isWalletWithRequiredFeatureSet,
  SuiSignPersonalMessage,
  type Wallet,
  type WalletAccount,
  type SuiSignPersonalMessageFeature,
} from "@mysten/wallet-standard";

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
  token: string;
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

  const address = keypair.getPublicKey().toSuiAddress();
  const message = buildSuiLoginMessage(address);
  const { signature } = await keypair.signPersonalMessage(new TextEncoder().encode(message));

  return requestSuiLoginToken(address, message, signature);
}

export async function requestWalletLoginToken(walletId?: string): Promise<SuiLoginToken> {
  const wallets = discoverSuiWallets();
  const wallet = selectSuiWallet(wallets, walletId);
  if (!wallet) {
    throw new Error("No Sui wallet found");
  }

  const connected = await wallet.features["standard:connect"].connect();
  const account = pickSuiAccount(connected.accounts, wallet.name);
  // Retain the connected wallet so on-chain mining can sign with it later (M4, WF-6).
  retainWalletSession(wallet, account);

  const message = buildSuiLoginMessage(account.address);
  const chain = account.chains?.find((entry) => entry.startsWith("sui:"));
  const { signature } = await wallet.features[SuiSignPersonalMessage].signPersonalMessage({
    account,
    message: new TextEncoder().encode(message),
    chain,
  });

  return requestSuiLoginToken(account.address, message, signature);
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

function buildSuiLoginMessage(address: string) {
  const now = Date.now();
  return JSON.stringify({
    purpose: SUI_LOGIN_PURPOSE,
    accountId: `sui:${address}`,
    address,
    origin: window.location.origin,
    issuedAt: now,
    expiresAt: now + 60_000,
    nonce: crypto.randomUUID(),
  });
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

function discoverSuiWallets() {
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
