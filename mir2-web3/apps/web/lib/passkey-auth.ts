"use client";

import {
  getWallets,
  isWalletWithRequiredFeatureSet,
  SuiSignPersonalMessage,
  type SuiSignPersonalMessageFeature,
} from "@mysten/wallet-standard";

const PASSKEY_STORAGE_KEY = "mir2.suiPasskey";
const SUI_LOGIN_PURPOSE = "mir2-sui-login";

type StoredPasskey = {
  publicKey: string;
  credentialId?: string;
};

export type SuiLoginToken = {
  accountId: string;
  token: string;
  expiresAt: number;
};

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

export async function requestWalletLoginToken(): Promise<SuiLoginToken> {
  const wallets = await discoverSuiWallets();
  const wallet = wallets[0];
  if (!wallet) {
    throw new Error("No Sui wallet found");
  }

  const connected = await wallet.features["standard:connect"].connect();
  const account =
    connected.accounts.find((entry) => entry.features.includes(SuiSignPersonalMessage)) ??
    connected.accounts.find((entry) => entry.chains?.some((chain) => chain.startsWith("sui:"))) ??
    connected.accounts[0];
  if (!account?.address) {
    throw new Error(`${wallet.name} did not return a Sui account`);
  }

  const message = buildSuiLoginMessage(account.address);
  const chain = account.chains?.find((entry) => entry.startsWith("sui:"));
  const { signature } = await wallet.features[SuiSignPersonalMessage].signPersonalMessage({
    account,
    message: new TextEncoder().encode(message),
    chain,
  });

  return requestSuiLoginToken(account.address, message, signature);
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

async function discoverSuiWallets() {
  return getWallets()
    .get()
    .filter((wallet) =>
      isWalletWithRequiredFeatureSet<SuiSignPersonalMessageFeature>(wallet, [
        SuiSignPersonalMessage,
      ]),
    );
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
