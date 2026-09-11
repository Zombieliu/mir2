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
    private final BlockingQueue<String> receipts = new LinkedBlockingQueue<>();
    private final BlockingQueue<String> gameplayPackets = new LinkedBlockingQueue<>();
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
        session = new GatewaySession(client, views::add, receipts::add, gameplayPackets::add);
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
        session = new GatewaySession(client, views::add, receipts::add, gameplayPackets::add);
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
        String refreshed = world.replace("\"x\":300", "\"x\":301");
        peer.send(refreshed);
        GatewaySession.View refreshedView = phase(GatewaySession.Phase.IN_GAME);
        assertNotNull(refreshedView.worldSnapshot);
        assertEquals(301, refreshedView.world.x);
        peer.send("{\"type\":\"packet\",\"packet\":\"MapChanged\",\"payload\":{\"fileName\":\"1\"}}");
        GatewaySession.View changed = phase(GatewaySession.Phase.STARTING);
        assertNull(changed.world);
        assertNull(changed.worldSnapshot);
        String destination = world.replace("\"mapFileName\":\"0\"", "\"mapFileName\":\"1\"")
                .replace("\"x\":300", "\"x\":50").replace("\"y\":630", "\"y\":60");
        peer.send(destination);
        GatewaySession.View destinationView = phase(GatewaySession.Phase.IN_GAME);
        assertEquals("1", destinationView.world.mapFileName);
        assertEquals(50, destinationView.world.x);
        assertNotNull(destinationView.worldSnapshot);
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

    @Test public void gameplayWritesRequireAnAuthenticatedInGameSession() throws Exception {
        connect();
        assertFalse(session.sendAuthenticated(GatewaySession.object("type", "attack", "objectId", 99)));
        assertNull(commands.poll(200, TimeUnit.MILLISECONDS));
        roster(); session.start(7);
        commands.poll(3, TimeUnit.SECONDS);
        peer.send("{\"type\":\"packet\",\"packet\":\"StartGame\",\"payload\":{\"result\":4}}");
        peer.send("{\"type\":\"packet\",\"packet\":\"MapInformation\",\"payload\":{\"fileName\":\"0\"}}");
        peer.send("{\"type\":\"packet\",\"packet\":\"UserInformation\",\"payload\":{\"name\":\"Fixture\"}}");
        peer.send("{\"type\":\"packet\",\"packet\":\"UserLocation\",\"payload\":{\"x\":302,\"y\":634}}");
        phase(GatewaySession.Phase.IN_GAME);
        assertFalse(session.sendAuthenticated(GatewaySession.object("type", "login", "accountId", "forged")));
        assertTrue(session.sendAuthenticated(GatewaySession.object("type", "attack", "objectId", 99)));
        JSONObject command = commands.poll(3, TimeUnit.SECONDS);
        assertEquals("attack", command.getString("type"));
        assertEquals(99, command.getInt("objectId"));
    }

    @Test public void forwardsOnlyBoundedAuthoritativeTransactionReceipts() throws Exception {
        connect();
        peer.send("{\"type\":\"packet\",\"packet\":\"StoreItemV2\",\"payload\":{\"requestId\":\"st-1\",\"from\":3,\"to\":9,\"success\":true}}");
        assertEquals("StoreItemV2", new JSONObject(receipts.poll(3, TimeUnit.SECONDS)).getString("packet"));
        peer.send("{\"type\":\"gameShopReceipt\",\"protocol\":\"nativeGameShopReceiptV1\",\"requestId\":\"gs-1\",\"success\":true,\"gIndex\":31,\"quantity\":1,\"priceType\":1}");
        assertEquals("gameShopReceipt", new JSONObject(receipts.poll(3, TimeUnit.SECONDS)).getString("type"));
        peer.send("{\"type\":\"packet\",\"packet\":\"ObjectChat\",\"payload\":{\"text\":\"not a receipt\"}}");
        assertNull(receipts.poll(200, TimeUnit.MILLISECONDS));
    }

    @Test public void entityPacketsForwardOnlyAfterAuthoritativeWorldEntry() throws Exception {
        connect();
        peer.send("{\"type\":\"packet\",\"packet\":\"ObjectWalk\",\"payload\":{\"objectId\":43,\"x\":302,\"y\":631,\"direction\":\"DownRight\"}}");
        assertNull(gameplayPackets.poll(200, TimeUnit.MILLISECONDS));
        roster(); session.start(7); commands.poll(3, TimeUnit.SECONDS);
        peer.send("{\"type\":\"packet\",\"packet\":\"StartGame\",\"payload\":{\"result\":4}}");
        peer.send("{\"type\":\"packet\",\"packet\":\"MapInformation\",\"payload\":{\"fileName\":\"0\"}}");
        peer.send("{\"type\":\"packet\",\"packet\":\"UserInformation\",\"payload\":{\"name\":\"Fixture\"}}");
        peer.send("{\"type\":\"packet\",\"packet\":\"UserLocation\",\"payload\":{\"x\":302,\"y\":634}}");
        phase(GatewaySession.Phase.IN_GAME);
        peer.send("{\"type\":\"packet\",\"packet\":\"ObjectWalk\",\"payload\":{\"objectId\":43,\"x\":302,\"y\":631,\"direction\":\"DownRight\"}}");
        assertEquals("ObjectWalk", new JSONObject(gameplayPackets.poll(3, TimeUnit.SECONDS)).getString("packet"));
        peer.send("{\"type\":\"packet\",\"packet\":\"NewMonsterInfo\",\"payload\":{\"objectId\":77,\"name\":\"Hen\",\"location\":{\"x\":299,\"y\":629}}}");
        assertEquals("NewMonsterInfo", new JSONObject(gameplayPackets.poll(3, TimeUnit.SECONDS)).getString("packet"));
        String[] lifecycle = new String[] {
                "{\"type\":\"packet\",\"packet\":\"ObjectHealth\",\"payload\":{\"objectId\":77,\"percent\":0,\"expire\":0}}",
                "{\"type\":\"packet\",\"packet\":\"ObjectRangeAttack\",\"payload\":{\"objectId\":77,\"location\":{\"x\":299,\"y\":629},\"direction\":\"Left\"}}",
                "{\"type\":\"packet\",\"packet\":\"ObjectDied\",\"payload\":{\"objectId\":77,\"location\":{\"x\":299,\"y\":629},\"direction\":\"Left\"}}",
                "{\"type\":\"packet\",\"packet\":\"ObjectRevived\",\"payload\":{\"objectId\":77,\"effect\":true}}",
                "{\"type\":\"packet\",\"packet\":\"Death\",\"payload\":{\"location\":{\"x\":302,\"y\":634},\"direction\":\"Down\"}}",
                "{\"type\":\"packet\",\"packet\":\"Revived\",\"payload\":{}}",
                "{\"type\":\"packet\",\"packet\":\"ObjectHide\",\"payload\":{\"objectId\":77}}",
                "{\"type\":\"packet\",\"packet\":\"ObjectShow\",\"payload\":{\"objectId\":77}}",
                "{\"type\":\"packet\",\"packet\":\"ObjectTeleportOut\",\"payload\":{\"objectId\":77,\"effectType\":1}}",
                "{\"type\":\"packet\",\"packet\":\"ObjectTeleportIn\",\"payload\":{\"objectId\":77,\"effectType\":1}}"
        };
        for (String packet : lifecycle) {
            peer.send(packet);
            JSONObject expected = new JSONObject(packet);
            JSONObject forwarded = new JSONObject(gameplayPackets.poll(3, TimeUnit.SECONDS));
            assertEquals(expected.getString("packet"), forwarded.getString("packet"));
        }
        peer.send("{\"type\":\"packet\",\"packet\":\"ObjectChat\",\"payload\":{\"text\":\"not an entity transform\"}}");
        assertNull(gameplayPackets.poll(200, TimeUnit.MILLISECONDS));
    }
}
