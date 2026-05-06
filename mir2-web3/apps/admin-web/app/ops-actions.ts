"use server";

import { revalidatePath } from "next/cache";
import { redirect } from "next/navigation";
import { adminPost, type SubmitCommandResponse } from "../lib/admin-api";

type AdminTarget = {
  targetType: string;
  targetId: string;
  accountId?: string;
  characterId?: string;
};

async function submitConsoleCommand(
  formData: FormData,
  redirectPath: string,
  target: AdminTarget,
  command: Record<string, unknown>
) {
  const response = await adminPost<SubmitCommandResponse>("/admin/commands/console", {
    commandId: optionalString(formData, "commandId"),
    traceId: optionalString(formData, "traceId"),
    approvalId: optionalString(formData, "approvalId"),
    target,
    command,
    reason: stringValue(formData, "reason") || "admin console operation"
  });
  revalidatePath(redirectPath);
  revalidatePath("/audit");
  revalidatePath("/timeline");
  if (!response.ok) {
    redirect(`${redirectPath}?error=${encodeURIComponent(response.error)}`);
  }
  redirect(`${redirectPath}?commandId=${encodeURIComponent(response.data.commandId)}`);
}

export async function createAccountAction(formData: FormData) {
  const accountId = stringValue(formData, "accountId");
  await submitConsoleCommand(
    formData,
    "/accounts",
    { targetType: "account", targetId: accountId, accountId },
    {
      type: "create_account",
      account_id: accountId,
      password: optionalString(formData, "password")
    }
  );
}

export async function updateAccountAction(formData: FormData) {
  const accountId = stringValue(formData, "accountId");
  await submitConsoleCommand(
    formData,
    "/accounts",
    { targetType: "account", targetId: accountId, accountId },
    {
      type: "update_account",
      account_id: accountId,
      password: optionalString(formData, "password"),
      storage_size: optionalNumber(formData, "storageSize"),
      clear_storage_password: booleanValue(formData, "clearStoragePassword"),
      unban: booleanValue(formData, "unban")
    }
  );
}

export async function deleteAccountAction(formData: FormData) {
  const accountId = stringValue(formData, "accountId");
  await submitConsoleCommand(
    formData,
    "/accounts",
    { targetType: "account", targetId: accountId, accountId },
    { type: "delete_account", account_id: accountId }
  );
}

export async function updateCharacterAction(formData: FormData) {
  const characterId = stringValue(formData, "characterId");
  const redirectPath = optionalString(formData, "redirectPath") ?? "/accounts";
  await submitConsoleCommand(
    formData,
    redirectPath,
    { targetType: "character", targetId: characterId, characterId },
    {
      type: "update_character",
      character_id: characterId,
      name: optionalString(formData, "name"),
      level: optionalNumber(formData, "level"),
      gold: optionalNumber(formData, "gold"),
      credit: optionalNumber(formData, "credit"),
      map_file_name: optionalString(formData, "mapFileName"),
      map_title: optionalString(formData, "mapTitle"),
      position_x: optionalNumber(formData, "positionX"),
      position_y: optionalNumber(formData, "positionY"),
      hp: optionalNumber(formData, "hp"),
      max_hp: optionalNumber(formData, "maxHp"),
      mp: optionalNumber(formData, "mp"),
      pk_points: optionalNumber(formData, "pkPoints")
    }
  );
}

export async function setCharacterFlagAction(formData: FormData) {
  const characterId = stringValue(formData, "characterId");
  await submitConsoleCommand(
    formData,
    `/players/${encodeURIComponent(characterId)}`,
    { targetType: "character", targetId: characterId, characterId },
    {
      type: "set_character_flag",
      character_id: characterId,
      flag_index: numberValue(formData, "flagIndex", 0),
      active: booleanValue(formData, "active")
    }
  );
}

export async function setChatBanAction(formData: FormData) {
  const characterId = stringValue(formData, "characterId");
  const redirectPath = optionalString(formData, "redirectPath") ?? `/players/${encodeURIComponent(characterId)}`;
  const durationMinutes = optionalNumber(formData, "durationMinutes");
  const explicitUntilMs = optionalNumber(formData, "untilMs");
  const banned = booleanValue(formData, "banned") || durationMinutes !== undefined || explicitUntilMs !== undefined;
  const untilMs = banned
    ? explicitUntilMs ?? Date.now() + Math.max(1, durationMinutes ?? 5) * 60_000
    : undefined;
  await submitConsoleCommand(
    formData,
    redirectPath,
    { targetType: "character", targetId: characterId, characterId },
    {
      type: "set_chat_ban",
      character_id: characterId,
      banned,
      until_ms: untilMs
    }
  );
}

export async function broadcastAction(formData: FormData) {
  await submitConsoleCommand(
    formData,
    "/console",
    { targetType: "world", targetId: "global" },
    { type: "broadcast", message: stringValue(formData, "message") }
  );
}

export async function messagePlayerAction(formData: FormData) {
  const characterId = stringValue(formData, "characterId");
  await submitConsoleCommand(
    formData,
    "/console",
    { targetType: "character", targetId: characterId, characterId },
    {
      type: "message_player",
      character_id: characterId,
      message: stringValue(formData, "message")
    }
  );
}

export async function returnSafeZoneAction(formData: FormData) {
  const characterId = stringValue(formData, "characterId");
  await submitConsoleCommand(
    formData,
    "/console",
    { targetType: "character", targetId: characterId, characterId },
    { type: "return_safe_zone", character_id: characterId }
  );
}

export async function killPlayerAction(formData: FormData) {
  const characterId = stringValue(formData, "characterId");
  await submitConsoleCommand(
    formData,
    "/console",
    { targetType: "character", targetId: characterId, characterId },
    { type: "kill_player", character_id: characterId }
  );
}

export async function killPetsAction(formData: FormData) {
  const characterId = stringValue(formData, "characterId");
  await submitConsoleCommand(
    formData,
    "/console",
    { targetType: "character", targetId: characterId, characterId },
    { type: "kill_pets", character_id: characterId }
  );
}

export async function serverControlAction(formData: FormData) {
  const action = stringValue(formData, "action");
  await submitConsoleCommand(
    formData,
    "/console",
    { targetType: "server", targetId: action },
    {
      type: "server_control",
      action,
      target: optionalString(formData, "target")
    }
  );
}

export async function cancelMarketListingAction(formData: FormData) {
  const characterId = stringValue(formData, "characterId");
  const listingId = numberValue(formData, "listingId", 0);
  await submitConsoleCommand(
    formData,
    "/market",
    { targetType: "market", targetId: `${characterId}:${listingId}`, characterId },
    {
      type: "market_cancel_listing",
      character_id: characterId,
      listing_id: listingId,
      notify_seller: booleanValue(formData, "notifySeller")
    }
  );
}

export async function expireMarketListingAction(formData: FormData) {
  const characterId = stringValue(formData, "characterId");
  const listingId = numberValue(formData, "listingId", 0);
  await submitConsoleCommand(
    formData,
    "/market",
    { targetType: "market", targetId: `${characterId}:${listingId}`, characterId },
    {
      type: "market_expire_listing",
      character_id: characterId,
      listing_id: listingId,
      notify_seller: booleanValue(formData, "notifySeller")
    }
  );
}

export async function deleteMarketListingAction(formData: FormData) {
  const characterId = stringValue(formData, "characterId");
  const listingId = numberValue(formData, "listingId", 0);
  await submitConsoleCommand(
    formData,
    "/market",
    { targetType: "market", targetId: `${characterId}:${listingId}`, characterId },
    {
      type: "market_delete_listing",
      character_id: characterId,
      listing_id: listingId,
      reason_message: optionalString(formData, "reasonMessage")
    }
  );
}

export async function removeGuildMemberAction(formData: FormData) {
  const guildName = stringValue(formData, "guildName");
  await submitConsoleCommand(
    formData,
    "/guilds",
    { targetType: "guild", targetId: guildName },
    {
      type: "guild_remove_member",
      guild_name: guildName,
      member_name: stringValue(formData, "memberName")
    }
  );
}

export async function sendGuildMessageAction(formData: FormData) {
  const guildName = stringValue(formData, "guildName");
  await submitConsoleCommand(
    formData,
    "/guilds",
    { targetType: "guild", targetId: guildName },
    {
      type: "guild_send_message",
      guild_name: guildName,
      message: stringValue(formData, "message")
    }
  );
}

export async function addNameListAction(formData: FormData) {
  const listName = stringValue(formData, "listName");
  await submitConsoleCommand(
    formData,
    "/namelists",
    { targetType: "name_list", targetId: listName },
    {
      type: "name_list_add",
      list_name: listName,
      player_name: stringValue(formData, "playerName")
    }
  );
}

export async function removeNameListAction(formData: FormData) {
  const listName = stringValue(formData, "listName");
  await submitConsoleCommand(
    formData,
    "/namelists",
    { targetType: "name_list", targetId: listName },
    {
      type: "name_list_remove",
      list_name: listName,
      player_name: stringValue(formData, "playerName")
    }
  );
}

export async function createNameListAction(formData: FormData) {
  const listName = stringValue(formData, "listName");
  await submitConsoleCommand(
    formData,
    "/namelists",
    { targetType: "name_list", targetId: listName },
    {
      type: "name_list_create",
      list_name: listName
    }
  );
}

export async function deleteNameListAction(formData: FormData) {
  const listName = stringValue(formData, "listName");
  await submitConsoleCommand(
    formData,
    "/namelists",
    { targetType: "name_list", targetId: listName },
    {
      type: "name_list_delete",
      list_name: listName
    }
  );
}

export async function publishContentBundleAction(formData: FormData) {
  const bundleId = stringValue(formData, "bundleId");
  const category = stringValue(formData, "category");
  let payload: unknown = {};
  const rawPayload = stringValue(formData, "payloadJson");
  if (rawPayload) {
    try {
      payload = JSON.parse(rawPayload);
    } catch {
      redirect("/content?error=payloadJson%20must%20be%20valid%20JSON");
    }
  }
  await submitConsoleCommand(
    formData,
    "/content",
    { targetType: "content_bundle", targetId: bundleId },
    {
      type: "publish_content_bundle",
      bundle_id: bundleId,
      category,
      record_key: stringValue(formData, "recordKey"),
      payload_json: payload
    }
  );
}

function stringValue(formData: FormData, key: string) {
  const value = formData.get(key);
  return typeof value === "string" ? value.trim() : "";
}

function optionalString(formData: FormData, key: string) {
  return stringValue(formData, key) || undefined;
}

function numberValue(formData: FormData, key: string, fallback: number) {
  const value = Number(stringValue(formData, key));
  return Number.isFinite(value) ? Math.floor(value) : fallback;
}

function optionalNumber(formData: FormData, key: string) {
  const value = stringValue(formData, key);
  if (!value) {
    return undefined;
  }
  const number = Number(value);
  return Number.isFinite(number) ? Math.floor(number) : undefined;
}

function booleanValue(formData: FormData, key: string) {
  return formData.get(key) === "on" || formData.get(key) === "true";
}
