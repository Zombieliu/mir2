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

    @Test public void socialJniDiagnosticsMustHaveARealJavaEventStream() throws Exception {
        for (String scene : new String[]{"group-jni", "guild-jni", "trade-jni", "trade-closed-jni"}) {
            List<String> events = OfflinePersonalIngressPreview.events(true, scene);
            assertFalse("Social JNI stream is missing: " + scene, events.isEmpty());
            assertEquals("STARTING", WireJson.decode(events.get(0)).getString("phase"));
        }
        // JVM fixture contract only; no native library, TLS or real account.
    }

    @Test public void socialJniStreamsStayBoundedImmutableAndDisabledInNormalBuilds() throws Exception {
        Set<String> allowed = Set.of("SwitchGroup", "GroupMemberInfo", "GroupMembersMap", "GroupInvite",
                "GuildStatus", "GuildNoticeChange", "GuildMemberChange", "GuildStorageList", "GuildStorageGoldChange",
                "TradeAccept", "TradeGold", "TradeItem", "TradeCancel");
        for (String scene : new String[]{"group-jni", "guild-jni", "trade-jni", "trade-closed-jni"}) {
            assertTrue(OfflinePersonalIngressPreview.events(false, scene).isEmpty());
            List<String> events = OfflinePersonalIngressPreview.events(true, scene);
            assertTrue(events.size() <= 8);
            int owner = -1;
            for (int index = 0; index < events.size(); index++) {
                JSONObject outer = WireJson.decode(events.get(index));
                assertFalse(outer.has("accountId"));assertFalse(outer.has("account_id"));assertFalse(outer.has("password"));
                if (outer.has("worldSnapshot")) {
                    assertEquals(-1, owner);
                    owner = index;
                    JSONObject snapshot = WireJson.decode(outer.getString("worldSnapshot"));
                    assertEquals(42, snapshot.getInt("playerObjectId"));
                    assertEquals("OFFLINE JAVA JNI", snapshot.getJSONArray("entities").getJSONObject(0).getString("name"));
                    assertEquals(777, snapshot.getInt("gold"));
                    assertEquals(12, snapshot.getJSONArray("inventoryItems").length());
                    assertTrue(outer.getString("worldSnapshot").getBytes(StandardCharsets.UTF_8).length <= 1024 * 1024);
                } else if (outer.has("envelope")) {
                    JSONObject inner = WireJson.decode(outer.getString("envelope"));
                    assertEquals("packet", inner.getString("type"));
                    assertTrue(allowed.contains(inner.getString("packet")));
                    assertTrue(outer.getString("envelope").getBytes(StandardCharsets.UTF_8).length <= 512 * 1024);
                    if (owner >= 0) assertEquals("TradeCancel", inner.getString("packet"));
                }
            }
            assertTrue(owner > 0);
            assertThrows(UnsupportedOperationException.class, events::clear);
            assertTrue(OfflinePersonalIngressPreview.events(true, scene + "-bypass").isEmpty());
        }
    }

    @Test public void socialJniFullDomainDataKeepsOwnGuestAndWalletSeparate() throws Exception {
        List<String> group = OfflinePersonalIngressPreview.events(true, "group-jni");
        JSONObject members = packet(group.get(2)).getJSONObject("payload");
        assertEquals(15, members.getJSONArray("members").length());
        assertEquals("OFFLINE JAVA JNI", members.getString("leaderName"));
        assertEquals("JNI BORDER", packet(group.get(3)).getJSONObject("payload").getString("playerMap"));
        assertEquals("JNI Inviter", packet(group.get(4)).getJSONObject("payload").getString("name"));
        List<String> guild = OfflinePersonalIngressPreview.events(true, "guild-jni");
        assertEquals(200, packet(guild.get(2)).getJSONObject("payload").getJSONArray("notice").length());
        JSONObject rank = packet(guild.get(3)).getJSONObject("payload").getJSONArray("ranks").getJSONObject(0);
        assertEquals(200, rank.getJSONArray("members").length());
        assertEquals(136, rank.getInt("options"));
        JSONArray slots = WireJson.decode(WireJson.decode(guild.get(4)).getString("envelope"))
                .getJSONObject("payload").getJSONArray("items");
        assertEquals(112, slots.length());
        java.math.BigInteger max = new java.math.BigInteger("18446744073709551615");
        for (int slot = 0; slot < 112; slot++) {
            assertEquals(max.subtract(java.math.BigInteger.valueOf(slot)).toString(),
                    slots.getJSONObject(slot).getJSONObject("item").get("unique_id").toString());
            assertEquals(slot + 1, slots.getJSONObject(slot).getJSONObject("item").getInt("count"));
        }
        for (String scene : new String[]{"trade-jni", "trade-closed-jni"}) {
            List<String> trade = OfflinePersonalIngressPreview.events(true, scene);
            JSONObject world = WireJson.decode(WireJson.decode(trade.get(4)).getString("worldSnapshot"));
            assertEquals(777, world.getInt("gold"));
            assertEquals(max.toString(), world.getJSONArray("inventoryItems").getJSONObject(0).get("uniqueId").toString());
            JSONObject own = world.getJSONObject("stage5Systems").getJSONObject("trade");
            assertEquals(125, own.getInt("offeredGold"));assertTrue(own.getBoolean("locked"));assertFalse(own.getBoolean("completed"));
            assertEquals(max.toString(), own.getJSONObject("offeredUniqueIds").get("1").toString());
            assertEquals(17, packet(trade.get(2)).getJSONObject("payload").getInt("amount"));
            JSONArray guest = WireJson.decode(WireJson.decode(trade.get(3)).getString("envelope"))
                    .getJSONObject("payload").getJSONArray("tradeItems");
            assertEquals(10, guest.length());assertTrue(guest.isNull(1));assertEquals(9, guest.getJSONObject(2).getInt("count"));
            if (scene.equals("trade-closed-jni")) {
                assertEquals(6, trade.size());assertEquals("TradeCancel", packet(trade.get(5)).getString("packet"));
                assertFalse(packet(trade.get(5)).getJSONObject("payload").getBoolean("unlock"));
            } else assertEquals(5, trade.size());
        }
    }

    @Test public void socialJniOwnTradeItemsCarryOriginalRendererMetadata() throws Exception {
        for (String scene : new String[]{"trade-jni", "trade-closed-jni"}) {
            List<String> events = OfflinePersonalIngressPreview.events(true, scene);
            JSONObject world = WireJson.decode(WireJson.decode(events.get(4)).getString("worldSnapshot"));
            JSONArray bag = world.getJSONArray("inventoryItems");
            for (int slot : new int[]{0, 11}) {
                JSONObject item = bag.getJSONObject(slot);
                JSONObject source = item.getJSONObject("tooltipSource");
                JSONObject info = source.getJSONObject("info"), user = source.getJSONObject("userItem");
                assertEquals(1000, info.getInt("item_index"));
                assertEquals(100, info.getInt("image"));
                assertEquals(0, info.getInt("item_type"));
                assertEquals(500, info.getInt("stack_size"));
                assertEquals(info.getInt("item_index"), user.getInt("item_index"));
                assertEquals(item.get("uniqueId").toString(), user.get("unique_id").toString());
                assertEquals(slot == 0 ? 201 : 3, item.getInt("count"));
                assertEquals(item.getInt("count"), user.getInt("count"));
                assertTrue(user.getBoolean("identified"));
            }
            // No generic bag, other scene, account or operation is altered.
            assertFalse(bag.getJSONObject(1).has("tooltipSource"));
        }
        JSONObject group = WireJson.decode(WireJson.decode(
                OfflinePersonalIngressPreview.events(true, "group-jni").get(5)).getString("worldSnapshot"));
        assertFalse(group.getJSONArray("inventoryItems").getJSONObject(0).has("tooltipSource"));
        assertTrue(OfflinePersonalIngressPreview.events(false, "trade-jni").isEmpty());
    }

    @Test public void heroJniScenesAreIsolatedDataAndBindTheOriginalOwner() throws Exception {
        Set<String> allowed = Set.of("HeroInformation", "UpdateHeroSpawnState", "HeroHealthChanged",
                "SetAutoPotValue", "SetAutoPotItem", "MagicLeveled");
        for (String scene : new String[]{"hero-inventory-jni", "hero-equipment-jni", "hero-status-jni",
                "hero-state-jni", "hero-skills-jni", "hero-removed-jni"}) {
            assertTrue(OfflinePersonalIngressPreview.events(false, scene).isEmpty());
            List<String> events = OfflinePersonalIngressPreview.events(true, scene);
            assertFalse("Hero has no actual Java/JNI stream: " + scene, events.isEmpty());
            assertEquals("STARTING", WireJson.decode(events.get(0)).getString("phase"));
            assertEquals("HeroInformation", packet(events.get(1)).getString("packet"));
            JSONObject world = WireJson.decode(events.get(2));
            assertEquals("IN_GAME", world.getString("phase"));
            JSONObject owner = WireJson.decode(world.getString("worldSnapshot"));
            assertEquals(42, owner.getInt("playerObjectId"));
            assertEquals("OFFLINE JAVA JNI", owner.getJSONArray("entities").getJSONObject(0).getString("name"));
            assertEquals(777, owner.getInt("gold"));
            assertEquals(33, owner.getInt("credit"));
            assertEquals(12, owner.getJSONArray("inventoryItems").length());
            for (String raw : events) {
                JSONObject outer = WireJson.decode(raw);
                assertFalse(outer.has("account_id"));assertFalse(outer.has("password"));assertFalse(outer.has("command"));
                if (!outer.has("envelope")) continue;
                JSONObject envelope = WireJson.decode(outer.getString("envelope"));
                assertEquals("packet", envelope.getString("type"));
                assertTrue(allowed.contains(envelope.getString("packet")));
                assertTrue(outer.getString("envelope").getBytes(StandardCharsets.UTF_8).length <= 16 * 1024);
            }
            try { events.clear();fail("Hero stream must be immutable"); }
            catch (UnsupportedOperationException expected) {}
        }
        assertTrue(OfflinePersonalIngressPreview.events(true, "hero").isEmpty());
        assertTrue(OfflinePersonalIngressPreview.events(true, "qa.hero").isEmpty());
    }

    @Test public void heroJniKeepsUnsignedHeroItemsSeparateFromThePlayerBag() throws Exception {
        List<String> events = OfflinePersonalIngressPreview.events(true, "hero-inventory-jni");
        assertFalse("Missing Hero Java stream", events.isEmpty());
        JSONObject info = WireJson.decode(WireJson.decode(events.get(1)).getString("envelope"))
                .getJSONObject("payload").getJSONObject("info");
        assertEquals(12, info.getInt("object_id"));
        assertEquals("OFFLINE JNI Hero", info.getString("name"));
        assertEquals("Wizard", info.getString("class"));assertEquals("Female", info.getString("gender"));
        JSONArray bag = info.getJSONArray("inventory"), gear = info.getJSONArray("equipment");
        assertEquals(42, bag.length());assertEquals(14, gear.length());
        assertEquals("18446744073709551615", bag.getJSONObject(3).get("unique_id").toString());
        assertEquals(201, bag.getJSONObject(3).getInt("count"));
        assertEquals(27, bag.getJSONObject(3).getInt("item_index"));
        assertEquals(123, bag.getJSONObject(3).getInt("current_dura"));
        assertEquals(456, bag.getJSONObject(3).getInt("max_dura"));
        assertEquals(90003, bag.getJSONObject(3).getJSONArray("slots").getJSONObject(0).getLong("unique_id"));
        assertEquals(80006, bag.getJSONObject(6).getLong("unique_id"));
        assertEquals(70001, gear.getJSONObject(0).getLong("unique_id"));
        assertTrue(info.getBoolean("auto_pot"));
        assertEquals(17, info.getJSONArray("magics").getJSONObject(0).getInt("key"));
        assertEquals(18, info.getJSONArray("magics").getJSONObject(1).getInt("key"));
        JSONObject owner = WireJson.decode(WireJson.decode(events.get(2)).getString("worldSnapshot"));
        assertEquals(80003, owner.getJSONArray("inventoryItems").getJSONObject(3).getLong("uniqueId"));
        assertEquals(3, owner.getJSONArray("inventoryItems").getJSONObject(3).getInt("count"));
    }

    @Test public void heroJniHasRealPostBootstrapDeltasAndWrongOwnerSentinel() throws Exception {
        for (String scene : new String[]{"hero-inventory-jni", "hero-equipment-jni", "hero-status-jni",
                "hero-state-jni", "hero-skills-jni", "hero-removed-jni"}) {
            List<String> events = OfflinePersonalIngressPreview.events(true, scene);
            assertFalse("Missing Hero Java stream", events.isEmpty());
            assertEquals(10, events.size());
            assertEquals("UpdateHeroSpawnState", packet(events.get(3)).getString("packet"));
            assertEquals(scene.equals("hero-removed-jni") ? 1 : 2,
                    packet(events.get(3)).getJSONObject("payload").getInt("state"));
            JSONObject health = packet(events.get(4)).getJSONObject("payload");
            assertEquals(67, health.getInt("hp"));assertEquals(23, health.getInt("mp"));
            assertEquals(42, health.getInt("ownerObjectId"));
            assertEquals("MagicLeveled", packet(events.get(7)).getString("packet"));
            JSONObject wrong = packet(events.get(9)).getJSONObject("payload");
            assertEquals(43, wrong.getInt("ownerObjectId"));assertEquals(9999, wrong.getInt("hp"));
            assertEquals("OFFLINE JAVA JNI", wrong.getString("characterName"));
        }
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
