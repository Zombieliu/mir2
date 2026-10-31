import "server-only";

import { passkeyGatewaySecret } from "./passkey-auth";
import type { DubheAuthContext } from "./dubhe-connect-auth";

/** Hosting production code does not turn this promotion campaign into mainnet. */
export function promotionDedicatedConfigurationRequired(): boolean {
  return process.env.VERCEL === "1" || !!process.env.VERCEL_ENV?.trim();
}

function exactOrigin(value: string): string {
  const url = new URL(value);
  if (value !== url.origin || url.username || url.password ||
      (url.protocol !== "https:" && !(url.protocol === "http:" &&
        ["127.0.0.1", "localhost", "[::1]"].includes(url.hostname)))) {
    throw new Error("promotion_wallet_configuration_invalid");
  }
  return url.origin;
}

export function promotionWalletContext(): DubheAuthContext {
  const hosted = promotionDedicatedConfigurationRequired();
  const secret = process.env.MIR2_PROMOTION_WALLET_AUTH_SECRET ?? (hosted ? undefined : passkeyGatewaySecret());
  const originList = process.env.MIR2_PROMOTION_ALLOWED_ORIGINS ?? (hosted ? undefined :
    process.env.MIR2_PASSKEY_ALLOWED_ORIGINS ?? process.env.MIR2_ALLOWED_WEB_ORIGINS);
  const network = process.env.MIR2_PROMOTION_DUBHE_CONNECT_NETWORK?.trim() ||
    (hosted ? undefined : process.env.MIR2_DUBHE_CONNECT_NETWORK?.trim() || "testnet");
  if (!secret || secret.length < 32 || !originList || (network !== "testnet" && network !== "mainnet")) {
    throw new Error("promotion_wallet_configuration_required");
  }
  const allowedOrigins = originList.split(",").map(value => value.trim());
  if (!allowedOrigins.length || allowedOrigins.some(value => !value) || new Set(allowedOrigins).size !== allowedOrigins.length) {
    throw new Error("promotion_wallet_configuration_invalid");
  }
  allowedOrigins.forEach(exactOrigin);
  const publicOrigin = process.env.MIR2_PROMOTION_PUBLIC_ORIGIN?.trim();
  if (hosted && (!publicOrigin || !publicOrigin.startsWith("https://"))) throw new Error("promotion_public_origin_required");
  if (publicOrigin && !allowedOrigins.includes(exactOrigin(publicOrigin))) throw new Error("promotion_public_origin_not_allowed");
  const relayOrigin = exactOrigin(process.env.MIR2_PROMOTION_DUBHE_CONNECT_RELAY_URL?.trim() ||
    (!hosted && process.env.MIR2_DUBHE_CONNECT_RELAY_URL?.trim()) || "https://dubhe-connect-relay.zombiesliu.workers.dev");
  const walletOrigin = exactOrigin(process.env.MIR2_PROMOTION_DUBHE_WALLET_ORIGIN?.trim() ||
    (!hosted && process.env.MIR2_DUBHE_WALLET_ORIGIN?.trim()) || "https://wallet.dubhe.obelisk.build");
  return Object.freeze({ secret, allowedOrigins: Object.freeze(allowedOrigins), network, relayOrigin, walletOrigin });
}

/** For the promotion bootstrap's public network label; never returns a secret. */
export function promotionWalletNetwork(): "testnet" | "mainnet" {
  return promotionWalletContext().network;
}
