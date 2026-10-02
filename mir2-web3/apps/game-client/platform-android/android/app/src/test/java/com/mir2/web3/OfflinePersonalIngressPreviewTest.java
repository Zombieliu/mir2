package com.mir2.web3;

import static org.junit.Assert.*;
import java.nio.charset.StandardCharsets;
import java.util.List;
import java.util.Set;
import org.json.JSONArray;
import org.json.JSONObject;
import org.junit.Test;

/** Offline inputs only; no native library, real Gateway or login is tested here. */
public class OfflinePersonalIngressPreviewTest {
    @Test public void normalBuildAndUnrecognizedSceneCannotGeneratePreviewInputs() throws Exception {
        for (String scene : new String[]{"gameshop-jni", "storage-jni", "storage-locked-jni", "hud", null}) {
            assertTrue(OfflinePersonalIngressPreview.events(false, scene).isEmpty());
        }
        for (String scene : new String[]{"hud", "gameshop", "storage", "qa.openStorage", null}) {
            assertTrue(OfflinePersonalIngressPreview.events(true, scene).isEmpty());
        }
    }

    @Test public void catalogueAndStockArriveBeforeExactOwnerWorld() throws Exception {
        List<String> events = OfflinePersonalIngressPreview.events(true, "gameshop-jni");
        assertEquals(111, events.size());
        assertEquals("STARTING", new JSONObject(events.get(0)).getString("phase"));
        for (int index = 0; index < 105; index++) {
            JSONObject packet = packet(events.get(index + 1));
            assertEquals("GameShopInfo", packet.getString("packet"));
            JSONObject item = packet.getJSONObject("payload").getJSONObject("item");
            assertEquals(2000 + index, item.getInt("gIndex"));
            assertTrue(item.getString("itemName").startsWith("JNI received "));
        }
        JSONObject stock = packet(events.get(106));
        assertEquals("GameShopStock", stock.getString("packet"));
        assertEquals(2104, stock.getJSONObject("payload").getInt("gIndex"));
        assertEquals(3, stock.getJSONObject("payload").getInt("stockLevel"));
        JSONObject world = new JSONObject(events.get(107));
        assertEquals("IN_GAME", world.getString("phase"));
        JSONObject snapshot = new JSONObject(world.getString("worldSnapshot"));
        assertEquals(42, snapshot.getInt("playerObjectId"));
        assertEquals("OFFLINE JAVA JNI", snapshot.getJSONArray("entities").getJSONObject(0).getString("name"));
        assertEquals("0", snapshot.getString("mapFileName"));
        assertTrue(snapshot.getBoolean("requireStoragePassword"));
    }

    @Test public void full160SlotsRemainLockedUnlessExplicitOfflineResultFollows() throws Exception {
        for (String scene : new String[]{"storage-jni", "storage-locked-jni"}) {
            List<String> events = OfflinePersonalIngressPreview.events(true, scene);
            JSONObject items = packet(events.get(108));
            assertEquals("UserStorage", items.getString("packet"));
            JSONArray slots = items.getJSONObject("payload").getJSONArray("storage");
            assertEquals(160, slots.length());
            assertEquals("ResizeStorage", packet(events.get(109)).getString("packet"));
            for (int slot = 0; slot < 160; slot++) {
                assertEquals(90000 + slot, slots.getJSONObject(slot).getLong("uniqueId"));
                assertEquals(slot % 5 + 1, slots.getJSONObject(slot).getInt("count"));
            }
            if ("storage-locked-jni".equals(scene)) assertEquals(110, events.size());
            else assertEquals("StorageUnlockResult", packet(events.get(110)).getString("packet"));
        }
    }

    @Test public void inputsAreBoundedDataNotAuthOrGameplayCommands() throws Exception {
        Set<String> allowed = Set.of("GameShopInfo", "GameShopStock", "ResizeStorage", "UserStorage", "StorageUnlockResult");
        for (String event : OfflinePersonalIngressPreview.events(true, "storage-jni")) {
            JSONObject outer = new JSONObject(event);
            assertFalse(outer.has("account_id"));
            assertFalse(outer.has("accountId"));
            assertFalse(outer.has("password"));
            if (outer.has("envelope")) {
                String inner = outer.getString("envelope");
                assertTrue(inner.getBytes(StandardCharsets.UTF_8).length <= 16 * 1024);
                assertTrue(allowed.contains(new JSONObject(inner).getString("packet")));
            } else if (outer.has("worldSnapshot")) {
                assertTrue(outer.getString("worldSnapshot").getBytes(StandardCharsets.UTF_8).length <= 1024 * 1024);
            }
            assertTrue(event.getBytes(StandardCharsets.UTF_8).length <= 65536 + 2 * outer.optString("worldSnapshot").length());
        }
        try {
            OfflinePersonalIngressPreview.events(true, "storage-jni").clear();
            fail("Plan must be immutable");
        } catch (UnsupportedOperationException expected) {}
    }

    private static JSONObject packet(String event) throws Exception {
        return new JSONObject(new JSONObject(event).getString("envelope"));
    }

    @Test public void independentResizeTestFollowsOwnerSnapshotAndFullItems() throws Exception {
        List<String> events = OfflinePersonalIngressPreview.events(true, "storage-jni");
        int world = -1, items = -1, resize = -1;
        for (int index = 0; index < events.size(); index++) {
            JSONObject event = new JSONObject(events.get(index));
            if (event.has("worldSnapshot")) world = index;
            else if (event.has("envelope")) {
                String name = packet(events.get(index)).getString("packet");
                if ("UserStorage".equals(name)) items = index;
                if ("ResizeStorage".equals(name)) resize = index;
            }
        }
        assertTrue("Resize must be an independent post-bootstrap event", world >= 0 && items > world && resize > items);
    }
}
