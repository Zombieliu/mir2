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

export type RegistrationDraft = {
  accountId: string; password: string; confirmPassword: string; userName: string;
  birthDate: string; secretQuestion: string; secretAnswer: string; emailAddress: string;
};
export type ChangePasswordDraft = {
  accountId: string; oldPassword: string; newPassword: string; confirmPassword: string;
};
export type LoginAuthSurface = "login" | "registration" | "changePassword" | "safeKey";
export type LoginAuthState = {
  surface: LoginAuthSurface; epoch: number; notice: string | null; focusField: string | null;
  registration: RegistrationDraft; changePassword: ChangePasswordDraft;
  safeKeys: string; safeFocus: "account" | "password";
};
/** Presentation callbacks are bound to the rendered form lease by the host. */
export type LoginAuthControls = {
  state: LoginAuthState; ready: boolean; pending: boolean;
  open(surface: Exclude<LoginAuthSurface, "login">): void; close(): void;
  registrationChange(field: keyof RegistrationDraft, value: string): void;
  passwordChange(field: keyof ChangePasswordDraft, value: string): void;
  submitRegistration(): void; submitChangePassword(): void;
  safeFocus(field: "account" | "password"): void; safePress(key: string): void;
  safeDelete(): void; safeRandom(): void; safeEnter(): void;
};

export type PreauthKind = "login" | "newAccount" | "changePassword";
export type PreauthProof = Readonly<{ socket: object; generation: number; epoch: number; kind: PreauthKind }>;

/** Legacy replies have no request identity. Once entered, this socket's auth
 * lane stays used even after a terminal reply; a new intent needs a new socket.
 * The durable gate stores only identity metadata, never command bodies. */
export class PreauthFlightGate {
  private lanes = new WeakMap<object, { proof: PreauthProof; entered: boolean; completed: boolean }>();
  reserve(socket: object, generation: number, epoch: number, kind: PreauthKind): PreauthProof | null {
    if (!Number.isSafeInteger(generation) || generation < 1 || !Number.isSafeInteger(epoch)
      || epoch < 1 || this.lanes.has(socket)) return null;
    const proof = Object.freeze({ socket, generation, epoch, kind });
    this.lanes.set(socket, { proof, entered: false, completed: false });
    return proof;
  }
  used(socket: object): boolean { return this.lanes.has(socket); }
  allows(proof: PreauthProof, socket: object, generation: number, kind: PreauthKind): boolean {
    const lane = this.lanes.get(socket);
    return lane?.proof === proof && !lane.entered && proof.socket === socket
      && proof.generation === generation && proof.kind === kind;
  }
  enter(proof: PreauthProof, socket: object, generation: number, kind: PreauthKind): boolean {
    if (!this.allows(proof, socket, generation, kind)) return false;
    this.lanes.get(socket)!.entered = true;
    return true;
  }
  cancelUnsent(proof: PreauthProof): boolean {
    const lane = this.lanes.get(proof.socket);
    if (lane?.proof !== proof || lane.entered) return false;
    this.lanes.delete(proof.socket);
    return true;
  }
  complete(socket: object, generation: number, kind: PreauthKind): PreauthProof | null {
    const lane = this.lanes.get(socket);
    if (!lane || !lane.entered || lane.completed || lane.proof.generation !== generation
      || lane.proof.kind !== kind) return null;
    lane.completed = true;
    return lane.proof;
  }
  pending(socket: object): boolean {
    const lane = this.lanes.get(socket);
    return !!lane?.entered && !lane.completed;
  }
}
const preauthGateKey = Symbol.for("mir2.preauth-flight-gate.v1");
/** Preserve an entered lane across UI remount/HMR in this document. */
export function persistentPreauthGate(owner: object): PreauthFlightGate {
  const holder = owner as { [preauthGateKey]?: PreauthFlightGate };
  return holder[preauthGateKey] ??= new PreauthFlightGate();
}
export function preauthCommandKind(command: Record<string, unknown>): PreauthKind | null {
  if (command.type === "login" || command.type === "passkeyLogin") return "login";
  return command.type === "newAccount" || command.type === "changePassword" ? command.type : null;
}
export function isSensitiveGatewayCommand(command: Record<string, unknown>): boolean {
  return preauthCommandKind(command) !== null;
}
export function newAccountCommand(fields: RegistrationDraft, birthDateBinary: string): Record<string, unknown> {
  // ISO midnight ticks are exactly representable, despite exceeding 2^53.
  // Prove the actual JSON integer token is unchanged; never widen the gateway
  // i64 schema or apply this exception to identities/arbitrary DateTime ticks.
  if (!/^(?:0|[1-9][0-9]{0,18})$/.test(birthDateBinary)) throw Error("Invalid birth date encoding");
  const exact = BigInt(birthDateBinary);
  const numeric = Number(birthDateBinary);
  if (exact > 3155378112000000000n || exact % 864000000000n !== 0n
    || !Number.isFinite(numeric) || BigInt(JSON.stringify(numeric)) !== exact) {
    throw Error("Birth date cannot be encoded exactly");
  }
  return Object.freeze({ type: "newAccount", accountId: fields.accountId, password: fields.password,
    birthDateBinary: numeric, userName: fields.userName, secretQuestion: fields.secretQuestion,
    secretAnswer: fields.secretAnswer, emailAddress: fields.emailAddress });
}

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
  send({ type: "login", accountId, password }, { quiet: true });
}

export function sendNewAccountCommand(
  send: GatewaySend,
  fields: RegistrationDraft,
  birthDateBinary: string,
) {
  send({ type: "clientVersion" }, { quiet: true });
  return send(newAccountCommand(fields, birthDateBinary), { quiet: true });
}
export function sendChangePasswordCommand(send: GatewaySend, fields: ChangePasswordDraft) {
  send({ type: "clientVersion" }, { quiet: true });
  return send({ type: "changePassword", accountId: fields.accountId,
    currentPassword: fields.oldPassword, newPassword: fields.newPassword }, { quiet: true });
}

export function sendSuiLoginCommand(
  send: GatewaySend,
  accountId: string,
  token: string,
) {
  // Wallet and WebAuthn proofs share the gateway's authenticated-token path;
  // `accountId` is an asserted identity, not a raw client-side login bypass.
  send({ type: "clientVersion" }, { quiet: true });
  send({ type: "passkeyLogin", accountId, token }, { quiet: true });
}

export function requestSuiLoginToken(kind: SuiLoginKind, walletId?: string): Promise<SuiLoginToken> {
  return kind === "passkey" ? requestPasskeyLoginToken() : requestWalletLoginToken(walletId);
}
