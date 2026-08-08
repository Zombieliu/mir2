"use client";

import {
  connectSuiWalletForSigning,
  DUBHE_WALLET_URL,
  getActiveSuiWalletSession,
  getSuiWalletSummaries,
  linkChannelIdentity,
  linkSuiIdentity,
  requestPasskeyLoginToken,
  requestPasskeyIdentityCredential,
  requestChannelSessionToken,
  requestGuestChannelSessionToken,
  requestWalletLoginToken,
  requestWalletIdentityCredential,
  subscribeToSuiWalletChanges,
  type ActiveSuiWalletSession,
  type SuiLoginToken,
  type SuiWalletSummary,
} from "./passkey-auth";

export {
  connectSuiWalletForSigning,
  DUBHE_WALLET_URL,
  getActiveSuiWalletSession,
  getSuiWalletSummaries,
  linkChannelIdentity,
  linkSuiIdentity,
  requestPasskeyIdentityCredential,
  subscribeToSuiWalletChanges,
  requestChannelSessionToken,
  requestGuestChannelSessionToken,
  requestWalletIdentityCredential,
};
export type { ActiveSuiWalletSession, SuiLoginToken, SuiWalletSummary };

export type GatewaySend = (
  command: Record<string, unknown>,
  options?: { quiet?: boolean },
) => boolean;

export type SuiLoginKind = "passkey" | "wallet";

/**
 * Development/demo shortcut that enters the first character immediately.
 * The normal login screen deliberately uses `sendPasswordLoginCommand` so the
 * character-list response remains the authority for the next transition.
 */
export function sendBootstrapSequence(
  send: GatewaySend,
  accountId: string,
  password: string,
) {
  send({ type: "clientVersion" });
  send({ type: "login", accountId, password });
  send({ type: "startGame", characterIndex: 0 });
}

export function sendPasswordLoginCommand(
  send: GatewaySend,
  accountId: string,
  password: string,
  options?: { quietClientVersion?: boolean },
) {
  send({ type: "clientVersion" }, { quiet: options?.quietClientVersion });
  send({ type: "login", accountId, password });
}

export function sendNewAccountCommand(
  send: GatewaySend,
  accountId: string,
  password: string,
) {
  // Crystal's account packet has required profile fields even though this UI
  // only asks for credentials. Preserve the wire shape with neutral values.
  send({ type: "clientVersion" }, { quiet: true });
  send({
    type: "newAccount",
    accountId,
    password,
    birthDateBinary: 0,
    userName: accountId,
    secretQuestion: "",
    secretAnswer: "",
    emailAddress: "",
  });
}

export function sendSuiLoginCommand(
  send: GatewaySend,
  accountId: string,
  token: string,
) {
  // Wallet and WebAuthn proofs share the gateway's authenticated-token path;
  // `accountId` is an asserted identity, not a raw client-side login bypass.
  send({ type: "clientVersion" }, { quiet: true });
  send({ type: "passkeyLogin", accountId, token });
}

export function requestSuiLoginToken(kind: SuiLoginKind, walletId?: string): Promise<SuiLoginToken> {
  return kind === "passkey" ? requestPasskeyLoginToken() : requestWalletLoginToken(walletId);
}
