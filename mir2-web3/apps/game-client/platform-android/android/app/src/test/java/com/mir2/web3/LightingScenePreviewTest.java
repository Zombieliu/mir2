package com.mir2.web3;
import static org.junit.Assert.*;
import java.nio.charset.StandardCharsets;
import java.util.List;
import java.util.Set;
import org.json.JSONObject;
import org.junit.Test;
/** Fixture only: the API31 run must separately prove actual JNI/native rendering. */
public class LightingScenePreviewTest {
    private static final String[] SCENES={"lighting-night-jni","lighting-dawn-jni","lighting-day-jni","lighting-map-jni"};
    private JSONObject packet(String raw) throws Exception {return WireJson.decode(WireJson.decode(raw).getString("envelope"));}
    @Test public void ordinaryBuildAndUnknownScenesCannotCreateLightingInputs() {
        for(String scene:SCENES) assertTrue(OfflinePersonalIngressPreview.events(false,scene).isEmpty());
        assertTrue(OfflinePersonalIngressPreview.events(true,"lighting-live").isEmpty());
    }
    @Test public void originalMetadataNamesPrecedeExactOwnerAndRenderRequestNotAuthorityReceipt() throws Exception {
        for(String scene:SCENES) {
            List<String> events=OfflinePersonalIngressPreview.events(true,scene);
            assertEquals(5,events.size());assertEquals("STARTING",WireJson.decode(events.get(0)).getString("phase"));
            assertEquals("TimeOfDay",packet(events.get(1)).getString("packet"));
            assertEquals("MapInformation",packet(events.get(2)).getString("packet"));
            JSONObject world=WireJson.decode(events.get(3));
            assertEquals("IN_GAME",world.getString("phase"));
            JSONObject snapshot=WireJson.decode(world.getString("worldSnapshot"));
            assertEquals(42,snapshot.getInt("playerObjectId"));
            assertEquals(2,snapshot.getJSONArray("entities").length());
            assertEquals(42,snapshot.getJSONArray("entities").getJSONObject(0).getInt("objectId"));
            assertEquals("OFFLINE JAVA JNI",snapshot.getJSONArray("entities").getJSONObject(0).getString("name"));
            assertEquals(3,snapshot.getJSONArray("entities").getJSONObject(0).getInt("light"));
            assertEquals("ObjectSpell",packet(events.get(4)).getString("packet"));
        }
    }
    @Test public void timeAndMapOverrideAreExplicitIndependentSourceInputs() throws Exception {
        int[] times={4,1,2,2},maps={4,1,2,4};
        for(int i=0;i<SCENES.length;i++) {
            List<String> events=OfflinePersonalIngressPreview.events(true,SCENES[i]);
            assertEquals(times[i],packet(events.get(1)).getJSONObject("payload").getInt("lights"));
            assertEquals(maps[i],packet(events.get(2)).getJSONObject("payload").getInt("lights"));
            assertEquals(i==3?2:0,packet(events.get(2)).getJSONObject("payload").getInt("mapDarkLight"));
            assertEquals(times[i],WireJson.decode(WireJson.decode(events.get(3)).getString("worldSnapshot")).getInt("lightSetting"));
        }
    }
    @Test public void boundedImmutableFixturesNeverSendAuthMovementCommandsOrGrantReceipts() throws Exception {
        Set<String> allowed=Set.of("TimeOfDay","MapInformation","ObjectSpell");
        for(String scene:SCENES) {
            List<String> events=OfflinePersonalIngressPreview.events(true,scene);
            for(String event:events) {
                JSONObject outer=WireJson.decode(event);
                assertFalse(outer.has("account_id"));assertFalse(outer.has("accountId"));
                assertFalse(outer.has("password"));assertFalse(outer.has("token"));
                if(outer.has("envelope")) {
                    assertTrue(outer.getString("envelope").getBytes(StandardCharsets.UTF_8).length<=16*1024);
                    assertTrue(allowed.contains(packet(event).getString("packet")));
                }
                if(outer.has("worldSnapshot")) assertTrue(outer.getString("worldSnapshot").getBytes(StandardCharsets.UTF_8).length<=1024*1024);
            }
            try {events.clear();fail("Mutable stream");} catch(UnsupportedOperationException expected) {}
        }
    }
}

