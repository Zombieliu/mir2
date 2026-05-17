"use client";

import {
  requestPasskeyLoginToken,
  requestWalletLoginToken,
  type SuiLoginToken,
} from "./passkey-auth";

export type { SuiLoginToken };

export type GatewaySend = (
  command: Record<string, unknown>,
  options?: { quiet?: boolean },
) => boolean;

export type SuiLoginKind = "passkey" | "wallet";

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
  send({ type: "clientVersion" }, { quiet: true });
  send({ type: "passkeyLogin", accountId, token });
}

export function requestSuiLoginToken(kind: SuiLoginKind): Promise<SuiLoginToken> {
  return kind === "passkey" ? requestPasskeyLoginToken() : requestWalletLoginToken();
}
