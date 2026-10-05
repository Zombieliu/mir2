package com.mir2.web3;

import static org.junit.Assert.*;
import java.util.concurrent.BlockingQueue;
import java.util.concurrent.LinkedBlockingQueue;
import java.util.concurrent.TimeUnit;
import okhttp3.OkHttpClient;
import okhttp3.Response;
import okhttp3.WebSocket;
import okhttp3.WebSocketListener;
import okhttp3.mockwebserver.MockResponse;
import okhttp3.mockwebserver.MockWebServer;
import okhttp3.tls.HandshakeCertificates;
import okhttp3.tls.HeldCertificate;
import org.json.JSONObject;
import org.junit.After;
import org.junit.Before;
import org.junit.Test;

/** Local TLS metadata admission fixtures, not a real account or lighting GPU test. */
public class LightingMetadataIngressTest {
    private MockWebServer server;
    private GatewaySession session;
    private OkHttpClient client;
    private volatile WebSocket peer;
    private final BlockingQueue<GatewaySession.View> views = new LinkedBlockingQueue<>();
    private final BlockingQueue<JSONObject> commands = new LinkedBlockingQueue<>();
    private final BlockingQueue<String> metadata = new LinkedBlockingQueue<>();
    private final BlockingQueue<String> gameplay = new LinkedBlockingQueue<>();

    @Before public void setUp() throws Exception {
        HeldCertificate cert = new HeldCertificate.Builder().commonName("localhost")
                .addSubjectAlternativeName("localhost").build();
        HandshakeCertificates serverTls = new HandshakeCertificates.Builder().heldCertificate(cert).build();
        HandshakeCertificates clientTls = new HandshakeCertificates.Builder().addTrustedCertificate(cert.certificate()).build();
        server = new MockWebServer();
        server.useHttps(serverTls.sslSocketFactory(), false);
        client = new OkHttpClient.Builder().sslSocketFactory(clientTls.sslSocketFactory(), clientTls.trustManager())
                .connectTimeout(2, TimeUnit.SECONDS).build();
        session = new GatewaySession(client, views::add, ignored -> {}, gameplay::add, metadata::add);
        server.enqueue(new MockResponse().withWebSocketUpgrade(new WebSocketListener() {
            @Override public void onOpen(WebSocket socket, Response response) { peer = socket; }
            @Override public void onMessage(WebSocket socket, String text) {
                try {
                    JSONObject value = new JSONObject(text);
                    commands.add(value);
                    if ("clientVersion".equals(value.getString("type"))) {
                        packet("Connected", new JSONObject());
                    }
                } catch (Exception error) { throw new AssertionError(error); }
            }
        }));
        server.start();
    }

    @After public void tearDown() throws Exception {
        session.close();
        client.dispatcher().executorService().shutdownNow();
        client.connectionPool().evictAll();
        server.shutdown();
    }

    private GatewaySession.View phase(GatewaySession.Phase expected) throws Exception {
        long until = System.nanoTime() + TimeUnit.SECONDS.toNanos(5);
        while (System.nanoTime() < until) {
            GatewaySession.View view = views.poll(100, TimeUnit.MILLISECONDS);
            if (view != null && view.phase == expected) return view;
        }
        throw new AssertionError("Missing phase " + expected);
    }

    private void packet(String name, JSONObject payload) throws Exception {
        peer.send(new JSONObject().put("type", "packet").put("packet", name)
                .put("payload", payload).toString());
    }

    private void connect() throws Exception {
        session.connect(server.url("/ws").toString().replace("https://", "wss://"));
        phase(GatewaySession.Phase.READY);
        assertEquals("clientVersion", commands.poll(3, TimeUnit.SECONDS).getString("type"));
    }

    private void roster() throws Exception {
        session.login("lighting-fixture", "local-fixture-only");
        assertEquals("login", commands.poll(3, TimeUnit.SECONDS).getString("type"));
        packet("LoginSuccess", new JSONObject("{\"characters\":[{\"index\":7,\"name\":\"Fixture\"}]}"));
        assertEquals(7, phase(GatewaySession.Phase.CHARACTERS).characters.get(0).index);
    }

    private void start() throws Exception {
        connect();
        roster();
        session.start(7);
        assertEquals("startGame", commands.poll(3, TimeUnit.SECONDS).getString("type"));
        phase(GatewaySession.Phase.STARTING);
    }

    private JSONObject next(String name) throws Exception {
        String raw = metadata.poll(3, TimeUnit.SECONDS);
        assertNotNull("Missing ordinary lighting metadata " + name, raw);
        JSONObject event = new JSONObject(raw);
        assertEquals("packet", event.getString("type"));
        assertEquals(name, event.getString("packet"));
        return event.getJSONObject("payload");
    }

    @Test public void listedStartForwardsOriginalEnvironmentNamesInArrivalOrder() throws Exception {
        start();
        packet("TimeOfDay", new JSONObject().put("lights", 4));
        packet("MapInformation", new JSONObject().put("fileName", "0").put("lights", 3).put("mapDarkLight", 2));
        packet("NewMapInfo", new JSONObject().put("mapFileName", "0").put("lights", 2));
        packet("MapChanged", new JSONObject().put("fileName", "1").put("lights", 1));
        assertEquals(4, next("TimeOfDay").getInt("lights"));
        assertEquals(2, next("MapInformation").getInt("mapDarkLight"));
        assertEquals("0", next("NewMapInfo").getString("mapFileName"));
        assertEquals("1", next("MapChanged").getString("fileName"));
        assertNull(metadata.poll(200, TimeUnit.MILLISECONDS));
        assertNull("Lighting metadata changed the existing gameplay callback", gameplay.poll(200, TimeUnit.MILLISECONDS));
        // Metadata is not accepted StartGame/owner bootstrap or an outgoing command.
        assertNull(commands.poll(200, TimeUnit.MILLISECONDS));
    }

    @Test public void acceptedOwnerKeepsMetadataSeparateWithoutApprovingDestinationScene() throws Exception {
        start();
        packet("StartGame", new JSONObject().put("result", 4));
        peer.send("{\"type\":\"worldSnapshot\",\"payload\":{\"playerObjectId\":42,\"mapFileName\":\"0\",\"entities\":["
                + "{\"kind\":\"selfPlayer\",\"objectId\":42,\"name\":\"Fixture\",\"x\":10,\"y\":20}]}}");
        assertNotNull(phase(GatewaySession.Phase.IN_GAME).worldSnapshot);
        packet("TimeOfDay", new JSONObject().put("lights", 4));
        assertEquals(4, next("TimeOfDay").getInt("lights"));
        packet("MapChanged", new JSONObject().put("fileName", "1"));
        assertNull(phase(GatewaySession.Phase.STARTING).worldSnapshot);
        assertEquals("1", next("MapChanged").getString("fileName"));
        packet("NewMapInfo", new JSONObject().put("mapFileName", "1").put("lights", 3));
        assertEquals(3, next("NewMapInfo").getInt("lights"));
        assertNull(gameplay.poll(200, TimeUnit.MILLISECONDS));
        assertNull(commands.poll(200, TimeUnit.MILLISECONDS));
    }

    @Test public void metadataCannotBypassLoginOrAnUnlistedStart() throws Exception {
        connect();
        session.start(7);
        packet("TimeOfDay", new JSONObject().put("lights", 4));
        packet("MapInformation", new JSONObject().put("fileName", "0").put("lights", 3));
        assertNull(metadata.poll(300, TimeUnit.MILLISECONDS));
        roster();
        session.start(999);
        packet("TimeOfDay", new JSONObject().put("lights", 1));
        packet("NewMapInfo", new JSONObject().put("mapFileName", "0").put("lights", 2));
        assertNull(metadata.poll(300, TimeUnit.MILLISECONDS));
        assertNull(commands.poll(200, TimeUnit.MILLISECONDS));
    }

    @Test public void environmentKeepsExistingSixteenKiBForwardLimit() throws Exception {
        start();
        packet("TimeOfDay", new JSONObject().put("lights", 4).put("padding", "x".repeat(17000)));
        phase(GatewaySession.Phase.DISCONNECTED);
        assertNull(metadata.poll(200, TimeUnit.MILLISECONDS));
    }

    @Test public void disconnectDoesNotAdmitLateConnectionMetadata() throws Exception {
        start();
        packet("TimeOfDay", new JSONObject().put("lights", 4));
        assertEquals(4, next("TimeOfDay").getInt("lights"));
        session.disconnect("Fixture disconnect");
        phase(GatewaySession.Phase.DISCONNECTED);
        peer.send("{\"type\":\"packet\",\"packet\":\"TimeOfDay\",\"payload\":{\"lights\":1}}");
        assertNull(metadata.poll(300, TimeUnit.MILLISECONDS));
    }

    @Test public void unrelatedLightNamesAreNotASecondProtocol() throws Exception {
        start();
        packet("LightSetting", new JSONObject().put("lights", 4));
        packet("androidLightingEnvironment", new JSONObject().put("mapLightSetting", 4));
        assertNull(metadata.poll(300, TimeUnit.MILLISECONDS));
        assertNull(commands.poll(200, TimeUnit.MILLISECONDS));
    }
}
