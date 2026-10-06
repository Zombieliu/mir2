package com.mir2.web3;

import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import org.json.JSONArray;
import org.json.JSONObject;

/**
 * Synthetic inputs for the separately packaged UI preview, never an account or
 * transport. MainActivity sends these through its actual nativeEvent JNI entry.
 * No auth, purchase, transfer, password text, save or QA commands are generated.
 */
final class OfflinePersonalIngressPreview {
    private OfflinePersonalIngressPreview() {}

    static List<String> events(boolean enabled, String scene) {
        if (!enabled) return Collections.emptyList();
        if (isLightingScene(scene)) return lightingEvents(scene);
        if (isHeroScene(scene)) return heroEvents(scene);
        if (isSocialScene(scene)) return socialEvents(scene);
        if (isMailScene(scene)) return mailEvents(scene);
        if (!("gameshop-jni".equals(scene) || "storage-jni".equals(scene)
                || "storage-locked-jni".equals(scene))) return Collections.emptyList();
        List<String> events = new ArrayList<>();
        events.add(GatewaySession.object("phase", "STARTING", "message",
                "OFFLINE JNI fixture; not a login or StartGame receipt.").toString());
        // Packet-first owner staging: all 105 rows must survive until snapshot.
        for (int index = 0; index < 105; index++) {
            events.add(packet("GameShopInfo", GatewaySession.object("item",
                    GatewaySession.object("gIndex", 2000 + index, "itemIndex", 1000 + index,
                    "itemName", String.format(java.util.Locale.ROOT, "JNI received %03d", index + 1),
                    "image", 100, "goldPrice", 100, "creditPrice", 25, "count", 1,
                    "category", "JNI OFFLINE", "stock", 20, "stockLevel", 20,
                    "canBuyGold", true, "canBuyCredit", true))));
        }
        events.add(packet("GameShopStock", GatewaySession.object("gIndex", 2104, "stockLevel", 3)));
        JSONObject snapshot = ownerSnapshot();
        events.add(GatewaySession.object("phase", "IN_GAME", "message",
                "OFFLINE Java/JNI world fixture, NOT authenticated gameplay.",
                "world", GatewaySession.object("playerName", "OFFLINE JAVA JNI",
                "mapFileName", "0", "x", 302, "y", 634),
                "worldSnapshot", snapshot.toString()).toString());
        JSONArray storage = new JSONArray();
        for (int slot = 0; slot < 160; slot++) {
            storage.put(GatewaySession.object("uniqueId", 90000 + slot,
                    "name", String.format(java.util.Locale.ROOT, "JNI storage %03d", slot),
                    "icon", 100, "count", slot % 5 + 1));
        }
        events.add(packet("UserStorage", GatewaySession.object("storage", storage)));
        // A separate newer result, after the complete owner/base and item list.
        // Do not mix the scalar-result gate with initial snapshot replacement.
        events.add(packet("ResizeStorage", GatewaySession.object("size", 160,
                "hasExpandedStorage", true, "expiryTimeBinaryDatetime", 635000000000000000L)));
        if (!"storage-locked-jni".equals(scene)) {
            events.add(packet("StorageUnlockResult", GatewaySession.object("result", 0, "hasPassword", true)));
        }
        return Collections.unmodifiableList(events);
    }

    private static boolean isLightingScene(String scene) {
        return "lighting-night-jni".equals(scene) || "lighting-dawn-jni".equals(scene)
                || "lighting-day-jni".equals(scene) || "lighting-map-jni".equals(scene);
    }

    private static List<String> lightingEvents(String scene) {
        int time = "lighting-dawn-jni".equals(scene) ? 1
                : ("lighting-day-jni".equals(scene) || "lighting-map-jni".equals(scene)) ? 2 : 4;
        int map = "lighting-map-jni".equals(scene) ? 4 : time;
        List<String> events = new ArrayList<>();
        events.add(GatewaySession.object("phase", "STARTING", "message",
                "OFFLINE lighting Java/JNI data, NOT login or a socket.").toString());
        events.add(packet("TimeOfDay", GatewaySession.object("lights", time)));
        events.add(packet("MapInformation", GatewaySession.object("fileName", "0", "lights", map,
                "mapDarkLight", "lighting-map-jni".equals(scene) ? 2 : 0)));
        JSONObject snapshot = ownerSnapshot();
        try {
            snapshot.put("lightSetting", time);
            snapshot.put("sceneView", GatewaySession.object("center", GatewaySession.object("x",302,"y",634),
                    "width",22,"height",18));
            JSONArray actors = snapshot.getJSONArray("entities");
            actors.getJSONObject(0).put("light",3);
            actors.put(GatewaySession.object("kind","monster","objectId",43,"name","JNI torch monster",
                    "x",304,"y",634,"direction","Right","light",2,
                    "sprite",GatewaySession.object("bodyLibrary","Monster/003","frameBaseOffset",0,"directionStride",4)));
        } catch (org.json.JSONException error) { throw new IllegalArgumentException(error); }
        events.add(GatewaySession.object("phase","IN_GAME","message",
                "OFFLINE lighting world, NOT authenticated gameplay.",
                "world",GatewaySession.object("playerName","OFFLINE JAVA JNI","mapFileName","0","x",302,"y",634),
                "worldSnapshot",snapshot.toString()).toString());
        events.add(packet("ObjectSpell",GatewaySession.object("objectId",9201,
                "location",GatewaySession.object("x",300,"y",633),"spell",39,"direction","Down","param",0)));
        return Collections.unmodifiableList(events);
    }

    private static boolean isHeroScene(String scene) {
        return "hero-inventory-jni".equals(scene) || "hero-equipment-jni".equals(scene)
                || "hero-status-jni".equals(scene) || "hero-state-jni".equals(scene)
                || "hero-skills-jni".equals(scene) || "hero-removed-jni".equals(scene);
    }

    private static List<String> heroEvents(String scene) {
        List<String> events = new ArrayList<>();
        events.add(GatewaySession.object("phase", "STARTING", "message",
                "OFFLINE Hero Java/JNI data, NOT login or a socket operation.").toString());
        events.add(packet("HeroInformation", GatewaySession.object("info", heroInformation())));
        JSONObject snapshot = ownerSnapshot();
        try {
            snapshot.put("stage5Systems", GatewaySession.object("heroLearnedMagics", new JSONArray()
                    .put(GatewaySession.object("spell", "FireBall", "key", 17))
                    .put(GatewaySession.object("spell", "Healing", "key", 18))));
            snapshot.put("heroStats", new JSONArray().put(GatewaySession.object("stat", 12, "value", 180))
                    .put(GatewaySession.object("stat", 13, "value", 90)));
            snapshot.put("heroWeights", GatewaySession.object("bag", 7, "wear", 3, "hand", 2));
        } catch (org.json.JSONException error) { throw new IllegalArgumentException(error); }
        events.add(GatewaySession.object("phase", "IN_GAME", "message",
                "OFFLINE Hero Java/JNI world, NOT authenticated gameplay.",
                "world", GatewaySession.object("playerName", "OFFLINE JAVA JNI",
                "mapFileName", "0", "x", 302, "y", 634), "worldSnapshot", snapshot.toString()).toString());
        events.add(packet("UpdateHeroSpawnState", GatewaySession.object("state",
                "hero-removed-jni".equals(scene) ? 1 : 2)));
        events.add(packet("HeroHealthChanged", GatewaySession.object("ownerObjectId", 42,
                "characterName", "OFFLINE JAVA JNI", "hp", 67, "mp", 23)));
        events.add(packet("SetAutoPotValue", GatewaySession.object("stat", 12, "value", 35)));
        events.add(packet("SetAutoPotItem", GatewaySession.object("grid", 24, "item_index", 17)));
        events.add(packet("MagicLeveled", GatewaySession.object("objectId", 12, "spell", "FireBall",
                "level", 2, "experience", 7)));
        events.add(packet("SetAutoPotValue", GatewaySession.object("stat", 13, "value", 45)));
        // A wrong-owner delta must not replace the accepted Hero health.
        events.add(packet("HeroHealthChanged", GatewaySession.object("ownerObjectId", 43,
                "characterName", "OFFLINE JAVA JNI", "hp", 9999, "mp", 9999)));
        return Collections.unmodifiableList(events);
    }

    private static JSONObject heroInformation() {
        JSONArray inventory = new JSONArray(), equipment = new JSONArray();
        java.math.BigInteger max = new java.math.BigInteger("18446744073709551615");
        for (int slot = 0; slot < 42; slot++) {
            if (slot == 0) inventory.put(heroItem(java.math.BigInteger.valueOf(60000), 27, 5, false));
            else if (slot == 3) inventory.put(heroItem(max, 27, 201, true));
            else if (slot == 6) inventory.put(heroItem(java.math.BigInteger.valueOf(80006), 17, 3, false));
            else inventory.put(JSONObject.NULL);
        }
        for (int slot = 0; slot < 14; slot++) equipment.put(slot == 0
                ? heroItem(java.math.BigInteger.valueOf(70001), 17, 1, false) : JSONObject.NULL);
        return GatewaySession.object("object_id", 12, "name", "OFFLINE JNI Hero", "class", "Wizard",
                "gender", "Female", "level", 20, "hair", 0, "hp", 100, "mp", 30,
                "experience", 10, "max_experience", 100, "inventory", inventory, "equipment", equipment,
                "magics", new JSONArray().put(heroMagic("FireBall", 17)).put(heroMagic("Healing", 18)),
                "auto_pot", true, "auto_hp_percent", 30, "auto_mp_percent", 40,
                "hp_item_index", 27, "mp_item_index", 17);
    }

    private static JSONObject heroItem(java.math.BigInteger id, int index, int count, boolean socket) {
        JSONArray slots = new JSONArray();
        if (socket) slots.put(heroItem(java.math.BigInteger.valueOf(90003), 17, 1, false));
        return GatewaySession.object("unique_id", id, "item_index", index, "count", count,
                "current_dura", 123, "max_dura", 456, "soul_bound_id", -1, "identified", true,
                "cursed", false, "slots", slots, "gem_count", 0, "added_stats", new JSONArray(),
                "awake_type", 0, "awake_values", new JSONArray(), "refined_value", 0, "refine_added", 0,
                "refine_success_chance", 0, "wedding_ring", -1, "expire_info", JSONObject.NULL,
                "rental_information", JSONObject.NULL, "is_shop_item", false,
                "sealed_info", JSONObject.NULL, "gm_made", false);
    }

    private static JSONObject heroMagic(String spell, int key) {
        return GatewaySession.object("name", "JNI " + spell, "spell", spell, "base_cost", 1, "level_cost", 0,
                "icon", 1, "level1", 1, "level2", 2, "level3", 3, "need1", 1, "need2", 2, "need3", 3,
                "level", 1, "key", key, "experience", 0, "delay", 3400, "range", 8, "cast_time", -1000);
    }

    private static boolean isSocialScene(String scene) {
        return "group-jni".equals(scene) || "guild-jni".equals(scene)
                || "trade-jni".equals(scene) || "trade-closed-jni".equals(scene);
    }

    private static List<String> socialEvents(String scene) {
        List<String> events = new ArrayList<>();
        events.add(GatewaySession.object("phase", "STARTING", "message",
                "OFFLINE social Java/JNI data, NOT login or a socket operation.").toString());
        JSONObject snapshot = ownerSnapshot();
        java.math.BigInteger max = new java.math.BigInteger("18446744073709551615");
        if ("group-jni".equals(scene)) {
            JSONArray members = new JSONArray();
            for (int index = 0; index < 15; index++) {
                members.put(GatewaySession.object("name", index == 0 ? "OFFLINE JAVA JNI" : "JNI Group " + index,
                        "leader", index == 0, "online", true, "level", 22 + index,
                        "class", index % 3, "hp", 80 + index, "maxHp", 200, "map", "0"));
            }
            events.add(packet("SwitchGroup", GatewaySession.object("allowGroup", true)));
            events.add(packet("GroupMemberInfo", GatewaySession.object("leaderName", "OFFLINE JAVA JNI", "members", members)));
            events.add(packet("GroupMembersMap", GatewaySession.object("playerName", "JNI Group 14", "playerMap", "JNI BORDER")));
            events.add(packet("GroupInvite", GatewaySession.object("name", "JNI Inviter")));
        } else if ("guild-jni".equals(scene)) {
            events.add(packet("GuildStatus", GatewaySession.object("guildName", "JNI GUILD", "guildRankName", "JNI Rank",
                    "level", 7, "gold", 4096, "memberCount", 200, "maxMembers", 200, "myOptions", 136, "myRankId", 4)));
            JSONArray notice = new JSONArray(), members = new JSONArray(), slots = new JSONArray();
            for (int index = 0; index < 200; index++) {
                notice.put(String.format(java.util.Locale.ROOT, "JNI notice %03d", index));
                members.put(GatewaySession.object("name", index == 0 ? "OFFLINE JAVA JNI" : "JNI Member " + index,
                        "id", index + 1, "online", index % 2 == 0, "lastLoginBinaryDatetime", 635000000000000000L));
            }
            for (int slot = 0; slot < 112; slot++) {
                slots.put(GatewaySession.object("userId", 77000L + slot, "item",
                        GatewaySession.object("unique_id", max.subtract(java.math.BigInteger.valueOf(slot)),
                                "item_index", 1000, "count", slot + 1, "identified", true)));
            }
            events.add(packet("GuildNoticeChange", GatewaySession.object("notice", notice, "update", 0)));
            events.add(packet("GuildMemberChange", GatewaySession.object("ranks", new JSONArray().put(
                    GatewaySession.object("name", "JNI Rank", "index", 4, "options", 136, "members", members)))));
            events.add(packet("GuildStorageList", GatewaySession.object("items", slots)));
            events.add(packet("GuildStorageGoldChange", GatewaySession.object("changeType", 0, "amount", 9)));
        } else {
            events.add(packet("TradeAccept", GatewaySession.object("name", "JNI Guest")));
            events.add(packet("TradeGold", GatewaySession.object("amount", 17)));
            JSONArray guest = new JSONArray();
            for (int slot = 0; slot < 10; slot++) {
                guest.put(slot == 2 || slot == 9 ? GatewaySession.object("unique_id",
                        max.subtract(java.math.BigInteger.valueOf(slot)), "item_index", 1000,
                        "count", slot == 2 ? 9 : 5, "identified", true) : JSONObject.NULL);
            }
            events.add(packet("TradeItem", GatewaySession.object("tradeItems", guest)));
            try {
                JSONObject first = snapshot.getJSONArray("inventoryItems").getJSONObject(0);
                first.put("uniqueId", max);
                first.put("count", 201);
                // Only these offline own-offer rows need the shared trade
                // renderer's explicit original item metadata. A bag icon by
                // itself is not a tooltip source, item grant or operation ACK.
                first.put("tooltipSource", socialTradeItemSource(max, 201, first.getString("name")));
                JSONObject last = snapshot.getJSONArray("inventoryItems").getJSONObject(11);
                last.put("tooltipSource", socialTradeItemSource(java.math.BigInteger.valueOf(80011), 3,
                        last.getString("name")));
                snapshot.put("stage5Systems", GatewaySession.object("trade",
                        GatewaySession.object("settlementNonce", "jni-offer-1", "partner", "JNI Guest",
                                "offeredSlots", GatewaySession.object("1", 0, "8", 11),
                                "offeredUniqueIds", GatewaySession.object("1", max, "8", 80011),
                                "offeredGold", 125, "offeredCurrency", "gold", "locked", true, "completed", false)));
            } catch (org.json.JSONException error) { throw new IllegalArgumentException(error); }
        }
        events.add(GatewaySession.object("phase", "IN_GAME", "message",
                "OFFLINE social Java/JNI owner, NOT authenticated gameplay.",
                "world", GatewaySession.object("playerName", "OFFLINE JAVA JNI",
                "mapFileName", "0", "x", 302, "y", 634), "worldSnapshot", snapshot.toString()).toString());
        if ("trade-closed-jni".equals(scene)) {
            // Public authoritative close, NOT a settlement or inventory grant.
            events.add(packet("TradeCancel", GatewaySession.object("unlock", false)));
        }
        return Collections.unmodifiableList(events);
    }

    private static JSONObject socialTradeItemSource(java.math.BigInteger uniqueId, int count, String name) {
        return GatewaySession.object("info", GatewaySession.object("item_index", 1000, "name", name,
                "image", 100, "item_type", 0, "shape", 0, "stack_size", 500),
                "userItem", GatewaySession.object("unique_id", uniqueId, "item_index", 1000,
                "count", count, "identified", true));
    }

    private static boolean isMailScene(String scene) {
        return "mail-claim-jni".equals(scene) || "mail-claim-failure-jni".equals(scene)
                || "mail-send-jni".equals(scene) || "mail-send-failure-jni".equals(scene);
    }

    private static List<String> mailEvents(String scene) {
        List<String> events = new ArrayList<>();
        events.add(GatewaySession.object("phase", "STARTING", "message",
                "OFFLINE mail JNI fixture, NOT login or a socket write.").toString());
        JSONObject snapshot = ownerSnapshot();
        try {
            snapshot.put("androidMailGeneration", "9");
            snapshot.put("mail", mailRows(false));
        } catch (org.json.JSONException error) { throw new IllegalArgumentException(error); }
        events.add(GatewaySession.object("phase", "IN_GAME", "message",
                "OFFLINE mail Java/JNI world, NOT authenticated gameplay.",
                "world", GatewaySession.object("playerName", "OFFLINE JAVA JNI",
                "mapFileName", "0", "x", 302, "y", 634), "worldSnapshot", snapshot.toString()).toString());
        boolean collect = scene.startsWith("mail-claim"), success = !scene.contains("failure");
        // Synthetic host-shaped receipt only. This does not run GatewaySession,
        // create a pending operation, authenticate, write a socket or settle mail.
        events.add(GatewaySession.object("type", "gatewayGameplayPacket", "envelope",
                GatewaySession.object("type", "androidMailResult", "packet", collect ? "ParcelCollected" : "MailSent",
                "result", success ? 1 : -1, "connectionGeneration", "9", "ownerObjectId", 42,
                "characterName", "OFFLINE JAVA JNI", "claimMailId",
                collect ? "18446744073709551615" : JSONObject.NULL).toString()).toString());
        events.add(packet("ReceiveMail", GatewaySession.object("mail", mailRows(false))));
        // A following newer mailbox must not cover up the transient feedback.
        // Its unchanged gold/claimed/items deliberately do not imply settlement.
        events.add(packet("ReceiveMail", GatewaySession.object("mail", mailRows(true))));
        return Collections.unmodifiableList(events);
    }

    private static JSONArray mailRows(boolean newer) {
        JSONArray rows = new JSONArray();
        java.math.BigInteger max = new java.math.BigInteger("18446744073709551615");
        for (int index = 0; index < 256; index++) {
            JSONArray attachments = new JSONArray();
            for (int item = 0; item < 5; item++) {
                attachments.put(GatewaySession.object("uniqueId", max.subtract(java.math.BigInteger.valueOf(index * 5L + item)),
                        "name", "JNI attachment " + item, "itemIndex", 1000 + item, "count", item + 1,
                        "currentDura", 17, "maxDura", 23, "identified", true));
            }
            rows.put(GatewaySession.object("mailId", index == 255 ? max : java.math.BigInteger.valueOf(index + 1L),
                    "senderName", String.format(java.util.Locale.ROOT, "JNI mail %03d", index + 1),
                    "message", newer ? "JNI newer refresh; NOT settlement" : "JNI authoritative inbox; NOT live mail",
                    "gold", 77, "items", attachments, "collected", false, "canReply", true,
                    "dateSentBinaryDatetime", 635000000000000000L, "locked", false, "opened", false));
        }
        return rows;
    }

    private static JSONObject ownerSnapshot() {
        return GatewaySession.object("playerObjectId", 42, "mapFileName", "0",
                "mapTitle", "OFFLINE JNI Bichon", "entities", new JSONArray().put(
                GatewaySession.object("kind", "selfPlayer", "objectId", 42, "name", "OFFLINE JAVA JNI",
                "x", 302, "y", 634, "direction", "Down", "class", "Warrior", "gender", "Male",
                "level", 22, "sprite", GatewaySession.object("bodyLibrary", "CArmour/00",
                "frameBaseOffset", 0, "directionStride", 4))),
                "playerHp", 80, "playerMaxHp", 200, "playerMp", 20, "playerMaxMp", 100,
                "gold", 777, "credit", 33, "inventoryCapacity", 46, "inventoryItems", bag(),
                "storageItems", new JSONArray(), "storageSize", 80,
                "hasStoragePassword", true, "requireStoragePassword", true,
                "questLog", new JSONArray(), "completedQuests", new JSONArray());
    }

    private static JSONArray bag() {
        JSONArray bag = new JSONArray();
        for (int slot = 0; slot < 12; slot++) {
            bag.put(GatewaySession.object("uniqueId", 80000 + slot, "name", "JNI bag " + slot,
                    "icon", 100, "count", 3, "slot", slot));
        }
        return bag;
    }

    private static String packet(String name, JSONObject payload) {
        return GatewaySession.object("type", "gatewayGameplayPacket", "envelope",
                GatewaySession.object("type", "packet", "packet", name, "payload", payload).toString()).toString();
    }
}
