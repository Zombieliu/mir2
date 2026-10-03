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

    @Test public void mailJniVariantsProvideOwnerBoundResultFollowedByAuthoritativeMailbox() throws Exception {
        for (String scene : new String[]{"mail-claim-jni", "mail-claim-failure-jni", "mail-send-jni", "mail-send-failure-jni"}) {
            List<String> events = OfflinePersonalIngressPreview.events(true, scene);
            assertFalse("Mail has no real Java/JNI diagnostic stream yet", events.isEmpty());
            assertEquals("STARTING", WireJson.decode(events.get(0)).getString("phase"));
            JSONObject owner = WireJson.decode(WireJson.decode(events.get(1)).getString("worldSnapshot"));
            assertEquals("9", owner.getString("androidMailGeneration"));
            assertEquals("OFFLINE JAVA JNI", owner.getJSONArray("entities").getJSONObject(0).getString("name"));
            JSONObject result = packet(events.get(2));
            assertEquals("androidMailResult", result.getString("type"));
            assertEquals(scene.startsWith("mail-claim") ? "ParcelCollected" : "MailSent", result.getString("packet"));
            assertEquals(scene.contains("failure") ? -1 : 1, result.getInt("result"));
            assertEquals("9", result.getString("connectionGeneration"));
            assertEquals(42, result.getInt("ownerObjectId"));
            assertEquals("OFFLINE JAVA JNI", result.getString("characterName"));
            if (scene.startsWith("mail-claim")) assertEquals("18446744073709551615", result.getString("claimMailId"));
            else assertTrue(result.isNull("claimMailId"));
            assertEquals("ReceiveMail", packet(events.get(3)).getString("packet"));
            assertEquals("ReceiveMail", packet(events.get(4)).getString("packet"));
            assertEquals(5, events.size());
        }
    }

    @Test public void mailJniFullMailboxKeepsAllFiveUnsignedAttachmentsWithoutInventingSettlement() throws Exception {
        for (String scene : new String[]{"mail-claim-jni", "mail-claim-failure-jni", "mail-send-jni", "mail-send-failure-jni"}) {
            List<String> events = OfflinePersonalIngressPreview.events(true, scene);
            JSONObject owner = WireJson.decode(WireJson.decode(events.get(1)).getString("worldSnapshot"));
            assertEquals(777, owner.getInt("gold"));
            assertEquals(12, owner.getJSONArray("inventoryItems").length());
            for (JSONArray rows : new JSONArray[]{owner.getJSONArray("mail"),
                    packet(events.get(3)).getJSONObject("payload").getJSONArray("mail"),
                    packet(events.get(4)).getJSONObject("payload").getJSONArray("mail")}) {
                assertEquals(256, rows.length());
                for (int index = 0; index < 256; index++) {
                    JSONObject row = rows.getJSONObject(index);
                    assertEquals(index == 255 ? "18446744073709551615" : String.valueOf(index + 1), row.get("mailId").toString());
                    assertEquals(77, row.getInt("gold"));
                    assertFalse(row.getBoolean("collected"));
                    JSONArray attachments = row.getJSONArray("items");
                    assertEquals(5, attachments.length());
                    for (int item = 0; item < 5; item++) {
                        String expected = new java.math.BigInteger("18446744073709551615")
                                .subtract(java.math.BigInteger.valueOf(index * 5L + item)).toString();
                        assertEquals(expected, attachments.getJSONObject(item).get("uniqueId").toString());
                    }
                }
            }
        }
    }

    @Test public void mailJniInputsStayBoundedImmutableAndAbsentInNormalOrUnlistedScenes() throws Exception {
        for (String scene : new String[]{"mail-claim-jni", "mail-claim-failure-jni", "mail-send-jni", "mail-send-failure-jni"}) {
            assertTrue(OfflinePersonalIngressPreview.events(false, scene).isEmpty());
            List<String> events = OfflinePersonalIngressPreview.events(true, scene);
            for (String event : events) {
                JSONObject outer = WireJson.decode(event);
                assertFalse(outer.has("account_id"));assertFalse(outer.has("accountId"));assertFalse(outer.has("password"));
                int worldBytes = outer.optString("worldSnapshot").getBytes(StandardCharsets.UTF_8).length;
                int packetBytes = outer.optString("envelope").getBytes(StandardCharsets.UTF_8).length;
                assertTrue(worldBytes <= 1024 * 1024);
                assertTrue(packetBytes <= 512 * 1024);
                assertTrue(event.getBytes(StandardCharsets.UTF_8).length <= 65536 + 2 * worldBytes + 2 * packetBytes);
                if (outer.has("envelope")) {
                    JSONObject inner = WireJson.decode(outer.getString("envelope"));
                    assertTrue(inner.getString("type").equals("androidMailResult")
                            || (inner.getString("type").equals("packet") && inner.getString("packet").equals("ReceiveMail")));
                }
            }
            assertThrows(UnsupportedOperationException.class, events::clear);
        }
        assertTrue(OfflinePersonalIngressPreview.events(true, "mail-claim-jni-bypass").isEmpty());
        assertTrue(OfflinePersonalIngressPreview.events(true, "mail-result-jni").isEmpty());
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
