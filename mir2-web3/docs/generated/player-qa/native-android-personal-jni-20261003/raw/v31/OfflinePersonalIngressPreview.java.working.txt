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
        if (!enabled || !("gameshop-jni".equals(scene) || "storage-jni".equals(scene)
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
        events.add(packet("ResizeStorage", GatewaySession.object("size", 160,
                "hasExpandedStorage", true, "expiryTimeBinaryDatetime", 635000000000000000L)));
        JSONObject snapshot = GatewaySession.object("playerObjectId", 42, "mapFileName", "0",
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
        if (!"storage-locked-jni".equals(scene)) {
            events.add(packet("StorageUnlockResult", GatewaySession.object("result", 0, "hasPassword", true)));
        }
        return Collections.unmodifiableList(events);
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
