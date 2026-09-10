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

/** TLS transport fixtures, not a real Gateway/account acceptance test. */
public class GatewaySessionTest {
    private MockWebServer server;
    private GatewaySession session;
    private OkHttpClient client;
    private final BlockingQueue<GatewaySession.View> views = new LinkedBlockingQueue<>();
    private final BlockingQueue<JSONObject> commands = new LinkedBlockingQueue<>();
    private volatile WebSocket peer;

    @Before public void setUp() throws Exception {
        HeldCertificate cert = new HeldCertificate.Builder().commonName("localhost")
                .addSubjectAlternativeName("localhost").build();
        HandshakeCertificates serverTls = new HandshakeCertificates.Builder().heldCertificate(cert).build();
        HandshakeCertificates clientTls = new HandshakeCertificates.Builder().addTrustedCertificate(cert.certificate()).build();
        server = new MockWebServer();
        server.useHttps(serverTls.sslSocketFactory(), false);
        client = new OkHttpClient.Builder().sslSocketFactory(clientTls.sslSocketFactory(), clientTls.trustManager())
                .connectTimeout(2, TimeUnit.SECONDS).build();
        session = new GatewaySession(client, views::add);
        server.enqueue(new MockResponse().withWebSocketUpgrade(new WebSocketListener() {
            @Override public void onOpen(WebSocket socket, Response response) { peer = socket; }
            @Override public void onMessage(WebSocket socket, String text) {
                try {
                    JSONObject value = new JSONObject(text);
                    commands.add(value);
                    if (value.getString("type").equals("clientVersion")) {
                        socket.send("{\"type\":\"packet\",\"packet\":\"Connected\",\"payload\":{}}");
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

    private void connect() throws Exception {
        session.connect(server.url("/ws").toString().replace("https://", "wss://"));
        phase(GatewaySession.Phase.READY);
        assertEquals("clientVersion", commands.poll(3, TimeUnit.SECONDS).getString("type"));
    }

    private void roster() throws Exception {
        session.login("fixture", "fixture-secret");
        JSONObject login = commands.poll(3, TimeUnit.SECONDS);
        assertEquals("login", login.getString("type"));
        assertEquals("fixture", login.getString("accountId"));
        assertEquals("fixture-secret", login.getString("password"));
        assertEquals(3, login.length());
        peer.send("{\"type\":\"packet\",\"packet\":\"LoginSuccess\",\"payload\":{\"characters\":[{\"index\":7,\"name\":\"Fixture\"}]}}");
        assertEquals(7, phase(GatewaySession.Phase.CHARACTERS).characters.get(0).index);
    }

    @Test public void tlsLoginStartAndAuthoritativePosition() throws Exception {
        connect(); roster();
        session.start(7);
        assertEquals("startGame", commands.poll(3, TimeUnit.SECONDS).getString("type"));
        peer.send("{\"type\":\"packet\",\"packet\":\"StartGame\",\"payload\":{\"result\":4}}");
        peer.send("{\"type\":\"packet\",\"packet\":\"MapInformation\",\"payload\":{\"fileName\":\"0\"}}");
        peer.send("{\"type\":\"packet\",\"packet\":\"UserInformation\",\"payload\":{\"name\":\"Fixture\"}}");
        peer.send("{\"type\":\"packet\",\"packet\":\"UserLocation\",\"payload\":{\"x\":302,\"y\":634}}");
        GatewaySession.View state = phase(GatewaySession.Phase.IN_GAME);
        assertNotNull(state.world);
        assertEquals("Fixture", state.world.playerName);
        assertEquals("0", state.world.mapFileName);
        assertEquals(302, state.world.x);
        assertEquals(634, state.world.y);
        assertEquals(302, state.world.toJson().getInt("x"));
        assertTrue(state.message.contains("(302, 634)"));
        assertTrue(state.message.contains("Map: 0"));
        assertFalse(state.message.contains("fixture-secret"));
        session.disconnect("Backgrounded");
        GatewaySession.View stopped = phase(GatewaySession.Phase.DISCONNECTED);
        assertTrue(stopped.characters.isEmpty());
        assertNull(stopped.world);
        assertEquals(302, state.world.x); // Previously published view remains immutable.
        assertFalse(stopped.message.contains("302"));
    }

    @Test public void startCannotBypassLoginOrSelectUnlistedCharacter() throws Exception {
        connect();
        session.start(7);
        assertNull(commands.poll(200, TimeUnit.MILLISECONDS));
        roster();
        session.start(999);
        assertNull(commands.poll(200, TimeUnit.MILLISECONDS));
    }

    @Test public void rejectedLoginNeverEntersCharacterSelection() throws Exception {
        connect();
        session.login("fixture", "bad-password");
        commands.poll(3, TimeUnit.SECONDS);
        peer.send("{\"type\":\"packet\",\"packet\":\"Login\",\"payload\":{\"result\":4}}");
        assertTrue(phase(GatewaySession.Phase.READY).characters.isEmpty());
        session.start(7);
        assertNull(commands.poll(200, TimeUnit.MILLISECONDS));
    }

    @Test public void unsolicitedLocationDoesNotProduceWorld() throws Exception {
        connect();
        peer.send("{\"type\":\"packet\",\"packet\":\"UserLocation\",\"payload\":{\"x\":10,\"y\":20}}");
        assertNull(views.poll(200, TimeUnit.MILLISECONDS));
    }

    @Test public void rejectsCleartextAndCredentialBearingEndpoints() {
        for (String url : new String[]{"ws://localhost/ws", "https://localhost/ws",
                "wss://user:secret@localhost/ws", "wss://localhost/ws?token=secret", "wss://localhost/ws#secret"}) {
            try { GatewaySession.endpoint(url); fail(url); }
            catch (IllegalArgumentException expected) { /* expected */ }
        }
        assertEquals("https", GatewaySession.endpoint("wss://localhost/ws").scheme());
    }

    @Test public void untrustedTlsCertificateFailsClosed() throws Exception {
        session.close();
        client.dispatcher().executorService().shutdownNow();
        client.connectionPool().evictAll();
        client = new OkHttpClient.Builder().connectTimeout(2, TimeUnit.SECONDS).build();
        session = new GatewaySession(client, views::add);
        views.clear();
        session.connect(server.url("/ws").toString().replace("https://", "wss://"));
        phase(GatewaySession.Phase.CONNECTING);
        GatewaySession.View failure = phase(GatewaySession.Phase.DISCONNECTED);
        assertTrue(failure.message.contains("failed"));
        assertTrue(commands.isEmpty());
    }

    @Test public void snapshotMustWaitForStartAckAndMatchSelfObject() throws Exception {
        connect(); roster(); session.start(7);
        commands.poll(3, TimeUnit.SECONDS);
        phase(GatewaySession.Phase.STARTING);
        String world = "{\"type\":\"worldSnapshot\",\"payload\":{\"playerObjectId\":42,\"mapFileName\":\"0\",\"entities\":["
                + "{\"objectId\":999,\"kind\":\"monster\",\"name\":\"Deer\",\"x\":1,\"y\":2},"
                + "{\"objectId\":42,\"kind\":\"selfPlayer\",\"name\":\"Fixture\",\"x\":300,\"y\":630}]}}";
        peer.send(world);
        GatewaySession.View waiting = views.poll(3, TimeUnit.SECONDS);
        assertEquals(GatewaySession.Phase.STARTING, waiting.phase);
        assertNull(waiting.worldSnapshot);
        peer.send("{\"type\":\"packet\",\"packet\":\"StartGame\",\"payload\":{\"result\":4}}");
        GatewaySession.View accepted = phase(GatewaySession.Phase.IN_GAME);
        assertTrue(accepted.message.contains("(300, 630)"));
        JSONObject retained = new JSONObject(accepted.worldSnapshot);
        assertEquals(2, retained.getJSONArray("entities").length());
        assertEquals(42, retained.getLong("playerObjectId"));
        peer.send("{\"type\":\"packet\",\"packet\":\"UserLocation\",\"payload\":{\"x\":301,\"y\":630}}");
        assertNull(phase(GatewaySession.Phase.IN_GAME).worldSnapshot);
        peer.send("{\"type\":\"packet\",\"packet\":\"MapChanged\",\"payload\":{\"fileName\":\"1\"}}");
        GatewaySession.View changed = phase(GatewaySession.Phase.STARTING);
        assertNull(changed.world);
        assertNull(changed.worldSnapshot);
        session.disconnect("Test end");
        assertNull(phase(GatewaySession.Phase.DISCONNECTED).worldSnapshot);
        assertEquals(2, new JSONObject(accepted.worldSnapshot).getJSONArray("entities").length());
    }

    @Test public void malformedPositionInvalidatesSession() throws Exception {
        connect(); roster(); session.start(7);
        commands.poll(3, TimeUnit.SECONDS);
        peer.send("{\"type\":\"packet\",\"packet\":\"UserLocation\",\"payload\":{\"x\":1.5,\"y\":2}}");
        assertTrue(phase(GatewaySession.Phase.DISCONNECTED).characters.isEmpty());
    }
}
