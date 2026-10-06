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
    private volatile boolean exactShopReceiptsNegotiated;

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
                    } else if (value.getString("type").equals("clientCapabilities")) {
                        exactShopReceiptsNegotiated = value.getJSONArray("capabilities").length() == 1
                                && "nativeGameShopReceiptV1".equals(value.getJSONArray("capabilities").getString(0));
                    } else if (value.getString("type").equals("gameShopBuy") && exactShopReceiptsNegotiated) {
                        // Local TLS fixture models the real Gateway's opt-in boundary;
                        // this rejection is not a purchase or real-account acceptance.
                        socket.send(GatewaySession.object("type", "gameShopReceipt",
                                "protocol", "nativeGameShopReceiptV1", "requestId", value.getString("requestId"),
                                "gIndex", value.getInt("gIndex"), "quantity", value.getInt("quantity"),
                                "priceType", value.getInt("priceType"), "success", false,
                                "code", "insufficientCurrency").toString());
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

    private GatewaySession.View accountEvent(String expected) throws Exception {
        long until = System.nanoTime() + TimeUnit.SECONDS.toNanos(5);
        while (System.nanoTime() < until) {
            GatewaySession.View view = views.poll(100, TimeUnit.MILLISECONDS);
            if (view != null && view.accountEvent != null
                    && expected.equals(view.accountEvent.optString("type"))) return view;
        }
        throw new AssertionError("Missing account event " + expected);
    }

    private void connect() throws Exception {
        session.connect(server.url("/ws").toString().replace("https://", "wss://"));
        phase(GatewaySession.Phase.READY);
        assertEquals("clientVersion", commands.poll(3, TimeUnit.SECONDS).getString("type"));
        assertImplementedCapabilities(commands.poll(3, TimeUnit.SECONDS));
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

    private void assertImplementedCapabilities(JSONObject command) throws Exception {
        assertNotNull("The real Java socket must negotiate its implemented native receipt contract", command);
        assertEquals("clientCapabilities", command.getString("type"));
        assertEquals(2, command.length());
        assertEquals(1, command.getJSONArray("capabilities").length());
        assertEquals("nativeGameShopReceiptV1", command.getJSONArray("capabilities").getString(0));
        assertFalse(command.toString().contains("nativeResumeV1"));
    }

    @Test public void nativeCapabilitySocketNegotiatesAfterVersionWithoutUnimplementedResume() throws Exception {
        session.connect(server.url("/ws").toString().replace("https://", "wss://"));
        phase(GatewaySession.Phase.READY);
        JSONObject version = commands.poll(3, TimeUnit.SECONDS);
        assertNotNull(version);
        assertEquals("clientVersion", version.getString("type"));
        assertEquals(1, version.length());
        assertImplementedCapabilities(commands.poll(2, TimeUnit.SECONDS));
        assertNull(commands.poll(200, TimeUnit.MILLISECONDS));
        assertFalse(session.sendAuthenticated(GatewaySession.object("type", "attack", "objectId", 99)));
    }

    @Test public void nativeCapabilitySocketNegotiatesOncePerConnectionNotPerConnectedPacket() throws Exception {
        session.connect(server.url("/ws").toString().replace("https://", "wss://"));
        phase(GatewaySession.Phase.READY);
        assertEquals("clientVersion", commands.poll(3, TimeUnit.SECONDS).getString("type"));
        assertImplementedCapabilities(commands.poll(2, TimeUnit.SECONDS));
        peer.send("{\"type\":\"packet\",\"packet\":\"Connected\",\"payload\":{}}");
        assertNull(commands.poll(200, TimeUnit.MILLISECONDS));
        session.disconnect("Local reconnect fixture");
        phase(GatewaySession.Phase.DISCONNECTED);
        server.enqueue(new MockResponse().withWebSocketUpgrade(new WebSocketListener() {
            @Override public void onOpen(WebSocket socket, Response response) { peer = socket; }
            @Override public void onMessage(WebSocket socket, String text) {
                try {
                    JSONObject value = new JSONObject(text);
                    commands.add(value);
                    if ("clientVersion".equals(value.getString("type"))) {
                        socket.send("{\"type\":\"packet\",\"packet\":\"Connected\",\"payload\":{}}");
                    }
                } catch (Exception error) { throw new AssertionError(error); }
            }
        }));
        session.connect(server.url("/ws").toString().replace("https://", "wss://"));
        phase(GatewaySession.Phase.READY);
        assertEquals("clientVersion", commands.poll(3, TimeUnit.SECONDS).getString("type"));
        assertImplementedCapabilities(commands.poll(2, TimeUnit.SECONDS));
        assertNull(commands.poll(200, TimeUnit.MILLISECONDS));
    }

    @Test public void nativeCapabilityGameplayCannotRenegotiateHostCapabilities() throws Exception {
        enterMailResultWorld();
        assertFalse(session.sendAuthenticated(GatewaySession.object("type", "clientCapabilities",
                "capabilities", new org.json.JSONArray().put("nativeResumeV1"))));
        assertNull(commands.poll(200, TimeUnit.MILLISECONDS));
    }

    @Test public void nativeCapabilityGameplayCannotInjectAResumeCredential() throws Exception {
        enterMailResultWorld();
        assertFalse(session.sendAuthenticated(GatewaySession.object("type", "resumeSession",
                "credential", "A".repeat(43))));
        assertNull(commands.poll(200, TimeUnit.MILLISECONDS));
    }

    @Test public void nativeCapabilityNegotiatedSocketReceivesExactRejectedShopRequest() throws Exception {
        enterMailResultWorld();
        JSONObject request = GatewaySession.object("type", "gameShopBuy", "requestId", "gs-capability-fixture",
                "gIndex", 31, "quantity", 2, "priceType", 1);
        assertTrue(session.sendAuthenticated(request));
        assertEquals(request.toString(), commands.poll(3, TimeUnit.SECONDS).toString());
        String raw = receipts.poll(2, TimeUnit.SECONDS);
        assertNotNull("The opt-in fixture must deliver the exact authoritative rejection", raw);
        JSONObject result = new JSONObject(raw);
        assertEquals("gameShopReceipt", result.getString("type"));
        assertEquals("nativeGameShopReceiptV1", result.getString("protocol"));
        assertEquals("gs-capability-fixture", result.getString("requestId"));
        assertEquals(31, result.getInt("gIndex"));
        assertEquals(2, result.getInt("quantity"));
        assertEquals(1, result.getInt("priceType"));
        assertFalse(result.getBoolean("success"));
        assertEquals("insufficientCurrency", result.getString("code"));
        assertFalse(result.has("accountId"));
        assertNull(receipts.poll(200, TimeUnit.MILLISECONDS));
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

    @Test public void characterCreateAndDeleteUseDedicatedRosterPhaseCommands() throws Exception {
        connect(); roster();
        session.createCharacter(" NewHero ", "Wizard", "Female");
        JSONObject create = commands.poll(3, TimeUnit.SECONDS);
        assertEquals("newCharacter", create.getString("type"));
        assertEquals("NewHero", create.getString("name"));
        assertEquals("Wizard", create.getString("class"));
        assertEquals("Female", create.getString("gender"));
        assertEquals(4, create.length());
        session.start(7);
        assertNull(commands.poll(200, TimeUnit.MILLISECONDS));

        peer.send("{\"type\":\"packet\",\"packet\":\"NewCharacterSuccess\",\"payload\":{\"character\":{\"index\":9,\"name\":\"NewHero\",\"level\":1,\"class\":\"Wizard\",\"gender\":\"Female\"}}}");
        GatewaySession.View created = accountEvent("characterCreated");
        assertEquals(GatewaySession.Phase.CHARACTERS, created.phase);
        assertEquals(2, created.characters.size());
        assertEquals(9, created.characters.get(0).index);
        assertEquals("NewHero", created.accountEvent.getJSONObject("character").getString("name"));

        session.deleteCharacter(9);
        JSONObject delete = commands.poll(3, TimeUnit.SECONDS);
        assertEquals("deleteCharacter", delete.getString("type"));
        assertEquals(9, delete.getInt("characterIndex"));
        assertEquals(2, delete.length());
        peer.send("{\"type\":\"packet\",\"packet\":\"DeleteCharacterSuccess\",\"payload\":{\"characterIndex\":9}}");
        GatewaySession.View deleted = accountEvent("characterDeleted");
        assertEquals(9, deleted.accountEvent.getInt("characterIndex"));
        assertEquals(1, deleted.characters.size());
        assertEquals(7, deleted.characters.get(0).index);
    }

    @Test public void characterOperationFailuresAreBoundedAndRetryable() throws Exception {
        connect(); roster();
        session.createCharacter("", "Wizard", "Female");
        assertNull(commands.poll(200, TimeUnit.MILLISECONDS));
        GatewaySession.View invalid = phase(GatewaySession.Phase.CHARACTERS);
        assertNull(invalid.accountEvent);

        session.createCharacter("NewHero", "Wizard", "Female");
        assertEquals("newCharacter", commands.poll(3, TimeUnit.SECONDS).getString("type"));
        peer.send("{\"type\":\"packet\",\"packet\":\"NewCharacter\",\"payload\":{\"result\":1}}");
        GatewaySession.View rejectedCreate = accountEvent("operationFailure");
        assertTrue(rejectedCreate.accountEvent.getString("message").contains("creation"));
        assertEquals(1, rejectedCreate.characters.size());

        session.deleteCharacter(7);
        assertEquals("deleteCharacter", commands.poll(3, TimeUnit.SECONDS).getString("type"));
        peer.send("{\"type\":\"packet\",\"packet\":\"DeleteCharacter\",\"payload\":{\"result\":1}}");
        GatewaySession.View rejectedDelete = accountEvent("operationFailure");
        assertTrue(rejectedDelete.accountEvent.getString("message").contains("deletion"));
        assertEquals(1, rejectedDelete.characters.size());

        session.start(7);
        assertEquals("startGame", commands.poll(3, TimeUnit.SECONDS).getString("type"));
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

    @Test public void renderDeadlineRemainsArmedUntilAWorldSnapshotIsDelivered() throws Exception {
        connect(); roster(); session.start(7);
        commands.poll(3, TimeUnit.SECONDS);
        peer.send("{\"type\":\"packet\",\"packet\":\"StartGame\",\"payload\":{\"result\":4}}");
        peer.send("{\"type\":\"packet\",\"packet\":\"MapInformation\",\"payload\":{\"fileName\":\"0\"}}");
        peer.send("{\"type\":\"packet\",\"packet\":\"UserInformation\",\"payload\":{\"name\":\"Fixture\"}}");
        peer.send("{\"type\":\"packet\",\"packet\":\"UserLocation\",\"payload\":{\"x\":302,\"y\":634}}");
        GatewaySession.View coordinates = phase(GatewaySession.Phase.IN_GAME);
        assertNotNull(coordinates.world);
        assertNull(coordinates.worldSnapshot);
        assertTrue(session.awaitingRenderSnapshot());

        peer.send("{\"type\":\"worldSnapshot\",\"payload\":{\"playerObjectId\":42,\"mapFileName\":\"0\",\"entities\":["
                + "{\"objectId\":42,\"kind\":\"selfPlayer\",\"name\":\"Fixture\",\"x\":302,\"y\":634}]}}");
        GatewaySession.View rendered = phase(GatewaySession.Phase.IN_GAME);
        assertNotNull(rendered.worldSnapshot);
        assertFalse(session.awaitingRenderSnapshot());

        peer.send("{\"type\":\"packet\",\"packet\":\"MapChanged\",\"payload\":{\"fileName\":\"1\"}}");
        phase(GatewaySession.Phase.STARTING);
        peer.send("{\"type\":\"packet\",\"packet\":\"UserLocation\",\"payload\":{\"x\":50,\"y\":60}}");
        GatewaySession.View destinationCoordinates = phase(GatewaySession.Phase.IN_GAME);
        assertEquals("1", destinationCoordinates.world.mapFileName);
        assertNull(destinationCoordinates.worldSnapshot);
        assertTrue(session.awaitingRenderSnapshot());

        peer.send("{\"type\":\"worldSnapshot\",\"payload\":{\"playerObjectId\":42,\"mapFileName\":\"1\",\"entities\":["
                + "{\"objectId\":42,\"kind\":\"selfPlayer\",\"name\":\"Fixture\",\"x\":50,\"y\":60}]}}");
        GatewaySession.View destinationRendered = phase(GatewaySession.Phase.IN_GAME);
        assertNotNull(destinationRendered.worldSnapshot);
        assertFalse(session.awaitingRenderSnapshot());
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
        assertFalse(session.sendAuthenticated(GatewaySession.object("type", "newCharacter",
                "name", "Forged", "class", "Warrior", "gender", "Male")));
        assertFalse(session.sendAuthenticated(GatewaySession.object("type", "deleteCharacter",
                "characterIndex", 7)));
        assertTrue(session.sendAuthenticated(GatewaySession.object("type", "attack", "objectId", 99)));
        JSONObject command = commands.poll(3, TimeUnit.SECONDS);
        assertEquals("attack", command.getString("type"));
        assertEquals(99, command.getInt("objectId"));
    }

    @Test public void nativeMailWritesUseTheAuthenticatedSocketAndKeepAllSevenPublicShapes() throws Exception {
        long uid = 9007199254740993L;
        org.json.JSONArray indices = new org.json.JSONArray().put(uid).put(2).put(0).put(0).put(0);
        JSONObject[] mail = new JSONObject[]{
                GatewaySession.object("type","readMail","mailId",uid),
                GatewaySession.object("type","lockMail","mailId",7,"lock",true),
                GatewaySession.object("type","collectParcel","mailId",8),
                GatewaySession.object("type","deleteMail","mailId",9),
                GatewaySession.object("type","mailCost","gold",4294967295L,"itemsIdx",indices,"stamped",true),
                GatewaySession.object("type","mailLockedItem","uniqueId",uid,"locked",false),
                GatewaySession.object("type","sendMail","name","Friend","message","邮件\nhello 👋",
                        "gold",23,"itemsIdx",indices,"stamped",true)
        };
        for (JSONObject command : mail) assertFalse(session.sendAuthenticated(command));
        connect();
        for (JSONObject command : mail) assertFalse(session.sendAuthenticated(command));
        assertNull(commands.poll(200, TimeUnit.MILLISECONDS));
        roster();
        for (JSONObject command : mail) assertFalse(session.sendAuthenticated(command));
        assertNull(commands.poll(200, TimeUnit.MILLISECONDS));
        session.start(7);
        assertEquals("startGame", commands.poll(3, TimeUnit.SECONDS).getString("type"));
        for (JSONObject command : mail) assertFalse(session.sendAuthenticated(command));
        peer.send("{\"type\":\"packet\",\"packet\":\"StartGame\",\"payload\":{\"result\":4}}");
        peer.send("{\"type\":\"worldSnapshot\",\"payload\":{\"playerObjectId\":42,\"mapFileName\":\"0\",\"entities\":["
                + "{\"objectId\":42,\"kind\":\"selfPlayer\",\"name\":\"Fixture\",\"x\":302,\"y\":634}]}}");
        assertNotNull(phase(GatewaySession.Phase.IN_GAME).worldSnapshot);
        for (JSONObject expected : mail) {
            assertTrue(session.sendAuthenticated(expected));
            JSONObject actual = commands.poll(3, TimeUnit.SECONDS);
            assertNotNull(actual);
            assertEquals(expected.toString(), actual.toString());
            assertFalse(actual.has("account_id"));
            assertFalse(actual.has("accountId"));
            // Preserve the wire-shape assertions, now with the real protocol
            // boundary between two otherwise ambiguous mail mutations.
            if ("collectParcel".equals(expected.getString("type"))) finishOwnMail("ParcelCollected");
            if ("sendMail".equals(expected.getString("type"))) finishOwnMail("MailSent");
        }
        peer.send("{\"type\":\"packet\",\"packet\":\"MapChanged\",\"payload\":{\"fileName\":\"1\"}}");
        phase(GatewaySession.Phase.STARTING);
        for (JSONObject command : mail) assertFalse(session.sendAuthenticated(command));
        assertNull(commands.poll(200, TimeUnit.MILLISECONDS));
        session.disconnect("Mail egress fixture end");
        phase(GatewaySession.Phase.DISCONNECTED);
        for (JSONObject command : mail) assertFalse(session.sendAuthenticated(command));
        assertNull(commands.poll(200, TimeUnit.MILLISECONDS));
    }

    private String enterMailResultWorld() throws Exception {
        connect(); roster(); session.start(7); commands.poll(3,TimeUnit.SECONDS);
        peer.send("{\"type\":\"packet\",\"packet\":\"StartGame\",\"payload\":{\"result\":4}}");
        peer.send("{\"type\":\"worldSnapshot\",\"payload\":{\"androidMailGeneration\":\"999\",\"playerObjectId\":42,\"mapFileName\":\"0\",\"entities\":["
                + "{\"objectId\":42,\"kind\":\"selfPlayer\",\"name\":\"Fixture\",\"x\":302,\"y\":634}]}}");
        return new JSONObject(phase(GatewaySession.Phase.IN_GAME).worldSnapshot).getString("androidMailGeneration");
    }

    private void finishOwnMail(String packet) throws Exception {
        peer.send(GatewaySession.object("type","packet","packet",packet,"payload",GatewaySession.object("result",1)).toString());
        String raw=gameplayPackets.poll(3,TimeUnit.SECONDS);
        assertNotNull("Missing owned result",raw);
        assertEquals("androidMailResult",new JSONObject(raw).getString("type"));
        peer.send("{\"type\":\"packet\",\"packet\":\"ReceiveMail\",\"payload\":{\"mail\":[]}}");
        assertEquals("ReceiveMail",new JSONObject(gameplayPackets.poll(3,TimeUnit.SECONDS)).getString("packet"));
    }

    @Test public void mailResultClaimBindsOnlyTheActualWriteAndNotThePayloadId() throws Exception {
        String epoch=enterMailResultWorld();
        assertNotEquals("Server input cannot choose the host epoch","999",epoch);
        assertTrue(Long.parseLong(epoch)>0);
        peer.send("{\"type\":\"packet\",\"packet\":\"ParcelCollected\",\"payload\":{\"result\":1}}");
        assertNull(gameplayPackets.poll(200,TimeUnit.MILLISECONDS));
        assertTrue(session.sendAuthenticated(GatewaySession.object("type","collectParcel","mailId",9007199254740993L)));
        assertEquals(9007199254740993L,commands.poll(3,TimeUnit.SECONDS).getLong("mailId"));
        assertFalse(session.sendAuthenticated(GatewaySession.object("type","collectParcel","mailId",2)));
        assertFalse(session.sendAuthenticated(GatewaySession.object("type","sendMail","name","Friend","message","body")));
        peer.send("{\"type\":\"packet\",\"packet\":\"MailSent\",\"payload\":{\"result\":1}}");
        peer.send("{\"type\":\"packet\",\"packet\":\"ParcelCollected\",\"payload\":{\"result\":0}}");
        assertNull(gameplayPackets.poll(200,TimeUnit.MILLISECONDS));
        peer.send("{\"type\":\"packet\",\"packet\":\"ParcelCollected\",\"payload\":{\"result\":1,\"mailId\":2,\"accountId\":\"spoofed\"}}");
        JSONObject result=new JSONObject(gameplayPackets.poll(3,TimeUnit.SECONDS));
        assertEquals("androidMailResult",result.getString("type"));
        assertEquals("9007199254740993",result.getString("claimMailId"));
        assertEquals(epoch,result.getString("connectionGeneration"));
        assertEquals(42,result.getLong("ownerObjectId"));
        assertEquals("Fixture",result.getString("characterName"));
        assertEquals(1,result.getInt("result"));assertFalse(result.has("payload"));
        assertFalse(session.sendAuthenticated(GatewaySession.object("type","collectParcel","mailId",2)));
        peer.send("{\"type\":\"packet\",\"packet\":\"ParcelCollected\",\"payload\":{\"result\":1}}");
        assertNull(gameplayPackets.poll(200,TimeUnit.MILLISECONDS));
        peer.send("{\"type\":\"packet\",\"packet\":\"ReceiveMail\",\"payload\":{\"mail\":[]}}");
        assertEquals("ReceiveMail",new JSONObject(gameplayPackets.poll(3,TimeUnit.SECONDS)).getString("packet"));
        assertTrue(session.sendAuthenticated(GatewaySession.object("type","collectParcel","mailId",2)));
    }

    @Test public void mailResultSendRejectsForgedInternalEventsAndSurvivesSameOwnerScene() throws Exception {
        enterMailResultWorld();
        peer.send("{\"type\":\"androidMailResult\",\"packet\":\"MailSent\",\"result\":1,\"connectionGeneration\":\"1\",\"ownerObjectId\":42,\"characterName\":\"Fixture\"}");
        assertNull(gameplayPackets.poll(200,TimeUnit.MILLISECONDS));
        assertTrue(session.sendAuthenticated(GatewaySession.object("type","sendMail","name","Friend","message","private body")));
        commands.poll(3,TimeUnit.SECONDS);
        peer.send("{\"type\":\"packet\",\"packet\":\"MapChanged\",\"payload\":{\"fileName\":\"1\"}}");phase(GatewaySession.Phase.STARTING);
        peer.send("{\"type\":\"packet\",\"packet\":\"MailSent\",\"payload\":{\"data\":{\"result\":-1}}}");
        JSONObject result=new JSONObject(gameplayPackets.poll(3,TimeUnit.SECONDS));
        assertEquals("androidMailResult",result.getString("type"));assertEquals(-1,result.getInt("result"));
        assertFalse(result.toString().contains("private body"));assertTrue(result.isNull("claimMailId"));
        session.disconnect("Fixture ended");phase(GatewaySession.Phase.DISCONNECTED);
        assertFalse(session.sendAuthenticated(GatewaySession.object("type","sendMail","name","Friend","message","body")));
        peer.send("{\"type\":\"packet\",\"packet\":\"MailSent\",\"payload\":{\"result\":1}}");
        assertNull(gameplayPackets.poll(200,TimeUnit.MILLISECONDS));
    }

    @Test public void mailResultFailedSocketWriteNeverRecordsAnOperation() throws Exception {
        enterMailResultWorld();
        java.lang.reflect.Field socketField=GatewaySession.class.getDeclaredField("socket");
        socketField.setAccessible(true);
        java.lang.reflect.Field operationField=GatewaySession.class.getDeclaredField("mailOperation");
        operationField.setAccessible(true);
        synchronized(session) {
            WebSocket socket=(WebSocket)socketField.get(session);
            assertTrue(socket.close(1000,"fixture closing"));
            assertFalse(session.sendAuthenticated(GatewaySession.object("type","collectParcel","mailId",17)));
            assertEquals("NONE",operationField.get(session).toString());
        }
        assertNull(commands.poll(200,TimeUnit.MILLISECONDS));
        assertNull(gameplayPackets.poll(200,TimeUnit.MILLISECONDS));
        // Complete the fixture's close handshake; the default peer listener
        // does not echo a close and MockWebServer otherwise cannot shut down.
        peer.close(1000,"fixture closing");
        phase(GatewaySession.Phase.DISCONNECTED);
    }

    @Test public void mailResultReconnectRetiresPreviousOperationAndEpoch() throws Exception {
        String oldEpoch=enterMailResultWorld();
        assertTrue(session.sendAuthenticated(GatewaySession.object("type","sendMail","name","Friend","message","old body")));
        commands.poll(3,TimeUnit.SECONDS);
        session.disconnect("Fixture reconnect");phase(GatewaySession.Phase.DISCONNECTED);
        server.enqueue(new MockResponse().withWebSocketUpgrade(new WebSocketListener() {
            @Override public void onOpen(WebSocket ws,Response response) { peer=ws; }
            @Override public void onMessage(WebSocket ws,String text) {
                try {
                    JSONObject value=new JSONObject(text);commands.add(value);
                    if ("clientVersion".equals(value.getString("type"))) {
                        ws.send("{\"type\":\"packet\",\"packet\":\"Connected\",\"payload\":{}}");
                    }
                } catch(Exception error) { throw new AssertionError(error); }
            }
        }));
        String newEpoch=enterMailResultWorld();assertNotEquals(oldEpoch,newEpoch);
        peer.send("{\"type\":\"packet\",\"packet\":\"MailSent\",\"payload\":{\"result\":1}}");
        assertNull(gameplayPackets.poll(200,TimeUnit.MILLISECONDS));
        assertTrue(session.sendAuthenticated(GatewaySession.object("type","sendMail","name","Friend","message","new body")));
        commands.poll(3,TimeUnit.SECONDS);
        peer.send("{\"type\":\"packet\",\"packet\":\"MailSent\",\"payload\":{\"result\":1}}");
        JSONObject own=new JSONObject(gameplayPackets.poll(3,TimeUnit.SECONDS));
        assertEquals(newEpoch,own.getString("connectionGeneration"));
        assertFalse(own.toString().contains("new body"));
    }

    @Test public void mailResultRefusesRoundedClaimAndWrongOwnerWithoutLosingOwnPending() throws Exception {
        enterMailResultWorld();
        assertFalse(session.sendAuthenticated(GatewaySession.object("type","collectParcel","mailId",9007199254740992.0)));
        assertFalse(session.sendAuthenticated(GatewaySession.object("type","collectParcel","mailId",0)));
        assertTrue(session.sendAuthenticated(GatewaySession.object("type","collectParcel","mailId",17)));
        commands.poll(3,TimeUnit.SECONDS);
        peer.send("{\"type\":\"packet\",\"packet\":\"ParcelCollected\",\"payload\":{\"result\":1,\"ownerObjectId\":43}}");
        assertNull(gameplayPackets.poll(200,TimeUnit.MILLISECONDS));
        assertFalse(session.sendAuthenticated(GatewaySession.object("type","collectParcel","mailId",18)));
        peer.send("{\"type\":\"packet\",\"packet\":\"ParcelCollected\",\"payload\":{\"result\":-1,\"mailId\":18}}");
        JSONObject own=new JSONObject(gameplayPackets.poll(3,TimeUnit.SECONDS));
        assertEquals("17",own.getString("claimMailId"));assertEquals(-1,own.getInt("result"));
    }

    @Test public void mailResultClaimPreservesTheFullUnsignedIdOnTheActualWrite() throws Exception {
        String epoch = enterMailResultWorld();
        java.math.BigInteger id = new java.math.BigInteger("18446744073709551615");
        assertTrue("The shared u64 mail ID must not be truncated or rejected",
                session.sendAuthenticated(GatewaySession.object("type", "collectParcel", "mailId", id)));
        JSONObject written = commands.poll(3, TimeUnit.SECONDS);
        assertNotNull(written);
        assertEquals(id.toString(), written.get("mailId").toString());
        peer.send("{\"type\":\"packet\",\"packet\":\"ParcelCollected\",\"payload\":{\"result\":1,\"mailId\":2}}");
        JSONObject result = new JSONObject(gameplayPackets.poll(3, TimeUnit.SECONDS));
        assertEquals(id.toString(), result.getString("claimMailId"));
        assertEquals(epoch, result.getString("connectionGeneration"));
    }

    @Test public void wireDecoderPreservesUnsignedIntegersThroughNestedCloneAndEncoding() throws Exception {
        for (String id : new String[]{"1", "9007199254740993", "9223372036854775807",
                "9223372036854775808", "18446744073709551614", "18446744073709551615"}) {
            JSONObject value = WireJson.decode("{\"a\":[{\"id\":" + id + "}],\"text\":\"" + id + "\"}");
            assertEquals(id, WireJson.unsignedIdentity(value.getJSONArray("a").getJSONObject(0).get("id")));
            assertTrue(value.toString().contains("\"id\":" + id));
            assertEquals(id, WireJson.unsignedIdentity(WireJson.decode(value.toString())
                    .getJSONArray("a").getJSONObject(0).get("id")));
            assertTrue(value.get("text") instanceof String);
        }
        assertEquals("-9223372036854775808", WireJson.decode("{\"id\":-9223372036854775808}").get("id").toString());
    }

    @Test public void nativeWireEnvelopeSendsTheExactUnsignedIdAndCorrelatesItsOwnAck() throws Exception {
        enterMailResultWorld();
        JSONObject envelope = WireJson.decode("{\"sequence\":7,\"command\":{\"type\":\"collectParcel\",\"mailId\":18446744073709551615}}");
        assertEquals(7, envelope.getLong("sequence"));
        assertTrue(session.sendAuthenticated(envelope.getJSONObject("command")));
        assertEquals("18446744073709551615", commands.poll(3, TimeUnit.SECONDS).get("mailId").toString());
        peer.send("{\"type\":\"packet\",\"packet\":\"ParcelCollected\",\"payload\":{\"result\":1}}");
        JSONObject result = WireJson.decode(gameplayPackets.poll(3, TimeUnit.SECONDS));
        assertEquals("18446744073709551615", result.getString("claimMailId"));
        assertFalse(session.sendAuthenticated(envelope.getJSONObject("command")));
    }

    @Test public void wireIdentityNeverAcceptsStringsFloatsOrOutOfRangeValues() throws Exception {
        for (Object value : new Object[]{0, -1L, 9007199254740992.0, "18446744073709551615",
                new java.math.BigInteger("18446744073709551616"), new java.math.BigDecimal("1.0")}) {
            assertNull(WireJson.unsignedIdentity(value));
        }
        JSONObject decimal = WireJson.decode("{\"a\":1.0,\"b\":1e3}");
        assertNull(WireJson.unsignedIdentity(decimal.get("a")));
        assertNull(WireJson.unsignedIdentity(decimal.get("b")));
    }

    @Test public void wireDecoderRejectsOutOfRangeAndMalformedNumericLiterals() throws Exception {
        for (String raw : new String[]{"{\"id\":18446744073709551616}", "{\"id\":-9223372036854775809}",
                "{\"id\":01}", "{\"id\":1abc}", "{} trailing", "[]", "", "null"}) {
            assertThrows(org.json.JSONException.class, () -> WireJson.decode(raw));
        }
    }

    @Test public void wireDecoderBoundsDepthBytesAndIntegerAllocation() throws Exception {
        assertThrows(org.json.JSONException.class,
                () -> WireJson.decode("{\"a\":" + "[".repeat(65) + "0" + "]".repeat(65) + "}"));
        assertThrows(org.json.JSONException.class,
                () -> WireJson.decode("{\"text\":\"" + "文".repeat(350000) + "\"}"));
        assertThrows(org.json.JSONException.class,
                () -> WireJson.decode("{\"id\":" + "9".repeat(10000) + "}"));
        assertThrows(org.json.JSONException.class, () -> WireJson.decode(null));
    }

    @Test public void mailResultRejectsUnrepresentableIdentityWithoutRecordingAnOperation() throws Exception {
        enterMailResultWorld();
        for (Object value : new Object[]{new java.math.BigInteger("18446744073709551616"),
                "18446744073709551615", new java.math.BigDecimal("1.0"), -1L}) {
            assertFalse(session.sendAuthenticated(GatewaySession.object("type", "collectParcel", "mailId", value)));
        }
        assertNull(commands.poll(200, TimeUnit.MILLISECONDS));
        assertTrue(session.sendAuthenticated(GatewaySession.object("type", "collectParcel", "mailId", 17)));
        commands.poll(3, TimeUnit.SECONDS);
        finishOwnMail("ParcelCollected");
    }

    @Test public void wireReadOnlyMailAndWorldSnapshotRetainTheFullUnsignedId() throws Exception {
        enterMailResultWorld();
        String id = "18446744073709551615";
        peer.send("{\"type\":\"packet\",\"packet\":\"ReceiveMail\",\"payload\":{\"mail\":[{\"mailId\":" + id + "}]}}");
        String packet = gameplayPackets.poll(3, TimeUnit.SECONDS);
        assertNotNull(packet);
        assertTrue(packet.contains("\"mailId\":" + id));
        peer.send("{\"type\":\"worldSnapshot\",\"payload\":{\"playerObjectId\":42,\"mapFileName\":\"0\","
                + "\"mail\":[{\"mailId\":" + id + "}],\"entities\":["
                + "{\"objectId\":42,\"kind\":\"selfPlayer\",\"name\":\"Fixture\",\"x\":302,\"y\":634}]}}");
        String snapshot = phase(GatewaySession.Phase.IN_GAME).worldSnapshot;
        assertNotNull(snapshot);
        assertTrue(snapshot.contains("\"mailId\":" + id));
        assertEquals("Fixture", WireJson.decode(snapshot).getJSONArray("entities").getJSONObject(0).getString("name"));
    }

    @Test public void wireOutOfRangeNetworkIntegerDisconnectsWithoutForwarding() throws Exception {
        enterMailResultWorld();
        peer.send("{\"type\":\"packet\",\"packet\":\"ReceiveMail\",\"payload\":{\"mail\":[{\"mailId\":18446744073709551616}]}}");
        phase(GatewaySession.Phase.DISCONNECTED);
        assertNull(gameplayPackets.poll(200, TimeUnit.MILLISECONDS));
        assertFalse(session.sendAuthenticated(GatewaySession.object("type", "collectParcel", "mailId", 17)));
    }

    @Test public void mailResultNumericStringUsesTheFrozenWindowsScalarPath() throws Exception {
        enterMailResultWorld();
        assertTrue(session.sendAuthenticated(GatewaySession.object("type","sendMail","name","Friend","message","body")));
        commands.poll(3,TimeUnit.SECONDS);
        peer.send("{\"type\":\"packet\",\"packet\":\"MailSent\",\"payload\":{\"data\":{\"result\":\"+1\"}}}");
        assertEquals(1,new JSONObject(gameplayPackets.poll(3,TimeUnit.SECONDS)).getInt("result"));
    }

    @Test public void nativeParcelWriterPreservesBothStampChoicesAndAllFiveAttachments() throws Exception {
        connect(); roster(); session.start(7); commands.poll(3, TimeUnit.SECONDS);
        peer.send("{\"type\":\"packet\",\"packet\":\"StartGame\",\"payload\":{\"result\":4}}");
        peer.send("{\"type\":\"worldSnapshot\",\"payload\":{\"playerObjectId\":42,\"mapFileName\":\"0\",\"entities\":["
                + "{\"objectId\":42,\"kind\":\"selfPlayer\",\"name\":\"Fixture\",\"x\":302,\"y\":634}]}}");
        assertNotNull(phase(GatewaySession.Phase.IN_GAME).worldSnapshot);
        for (boolean stamped : new boolean[]{false,true}) {
            JSONObject parcel = GatewaySession.object("type","sendMail","name","Recipient","message","body",
                    "gold",0,"itemsIdx",new org.json.JSONArray().put(5).put(4).put(3).put(2).put(1),"stamped",stamped);
            assertTrue(session.sendAuthenticated(parcel));
            JSONObject actual = commands.poll(3, TimeUnit.SECONDS);
            assertNotNull(actual);
            assertEquals(stamped, actual.getBoolean("stamped"));
            assertEquals("[5,4,3,2,1]", actual.getJSONArray("itemsIdx").toString());
            assertEquals("Recipient", actual.getString("name"));
            assertEquals(0, actual.getInt("gold"));
            assertEquals(parcel.toString(), actual.toString());
            finishOwnMail("MailSent");
        }
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

    @Test public void inventoryReceiptsRequireOwnerBootstrapAndSurviveMapLoading() throws Exception {
        connect();
        JSONObject payload = GatewaySession.object("objectId", 42, "grid", "Inventory",
                "gridFrom", "Inventory", "gridTo", "Belt", "uniqueId", "9007199254740993",
                "idFrom", "9007199254740993", "idTo", 99, "count", 2, "from", 6, "to", 7,
                "heroInventory", false, "success", false);
        peer.send(GatewaySession.object("type", "packet", "packet", "DropItem", "payload", payload).toString());
        assertNull(gameplayPackets.poll(200, TimeUnit.MILLISECONDS));
        roster(); session.start(7); commands.poll(3, TimeUnit.SECONDS);
        peer.send("{\"type\":\"packet\",\"packet\":\"StartGame\",\"payload\":{\"result\":4}}");
        phase(GatewaySession.Phase.STARTING);
        peer.send(GatewaySession.object("type", "packet", "packet", "DropItem", "payload", payload).toString());
        assertNull(gameplayPackets.poll(200, TimeUnit.MILLISECONDS));
        peer.send("{\"type\":\"worldSnapshot\",\"payload\":{\"playerObjectId\":42,\"mapFileName\":\"0\",\"entities\":["
                + "{\"objectId\":42,\"kind\":\"selfPlayer\",\"name\":\"Fixture\",\"x\":302,\"y\":634}]}}");
        assertNotNull(phase(GatewaySession.Phase.IN_GAME).worldSnapshot);
        for (String packet : new String[]{"DropItem", "MoveItem", "MergeItem", "SplitItem1",
                "SellItem", "EquipItem", "RemoveItem"}) {
            peer.send(GatewaySession.object("type", "packet", "packet", packet, "payload", payload).toString());
            String raw = gameplayPackets.poll(3, TimeUnit.SECONDS);
            assertNotNull("Missing inventory receipt " + packet, raw);
            JSONObject forwarded = new JSONObject(raw);
            assertEquals(packet, forwarded.getString("packet"));
            assertEquals("9007199254740993", forwarded.getJSONObject("payload").getString("uniqueId"));
            assertFalse(forwarded.getJSONObject("payload").getBoolean("success"));
        }
        peer.send("{\"type\":\"packet\",\"packet\":\"MapChanged\",\"payload\":{\"fileName\":\"1\"}}");
        assertNull(phase(GatewaySession.Phase.STARTING).worldSnapshot);
        peer.send(GatewaySession.object("type", "packet", "packet", "EquipItem", "payload", payload).toString());
        assertEquals("EquipItem", new JSONObject(gameplayPackets.poll(3, TimeUnit.SECONDS)).getString("packet"));
        for (String unsupported : new String[]{"SplitItem", "DeleteItem", "Stage5Command"}) {
            peer.send(GatewaySession.object("type", "packet", "packet", unsupported, "payload", payload).toString());
            assertNull(gameplayPackets.poll(200, TimeUnit.MILLISECONDS));
        }
        session.disconnect("Inventory fixture end");
        phase(GatewaySession.Phase.DISCONNECTED);
        assertNull(gameplayPackets.poll(200, TimeUnit.MILLISECONDS));
    }

    @Test public void userInformationCannotMutatePublishedWorldBeforeOwnerValidation() throws Exception {
        connect(); roster(); session.start(7); commands.poll(3, TimeUnit.SECONDS);
        peer.send("{\"type\":\"packet\",\"packet\":\"StartGame\",\"payload\":{\"result\":4}}");
        peer.send("{\"type\":\"worldSnapshot\",\"payload\":{\"playerObjectId\":42,\"mapFileName\":\"0\",\"entities\":["
                + "{\"objectId\":42,\"kind\":\"selfPlayer\",\"name\":\"Fixture\",\"x\":302,\"y\":634}]}}");
        GatewaySession.View original = phase(GatewaySession.Phase.IN_GAME);
        views.clear(); gameplayPackets.clear();
        for (JSONObject payload : new JSONObject[]{
                GatewaySession.object("objectId",43,"name","Other","x",1,"y",2),
                GatewaySession.object("objectId",42,"hero",true,"name","Fixture","x",1,"y",2),
                GatewaySession.object("objectId",42,"hero",JSONObject.NULL,"name","Fixture","x",1,"y",2),
                GatewaySession.object("objectId","42","name","Fixture","x",1,"y",2),
                GatewaySession.object("objectId",JSONObject.NULL,"name","Fixture","x",1,"y",2),
                GatewaySession.object("name","Fixture","x",1,"y",2),
                GatewaySession.object("objectId",42.5,"name","Fixture","x",1,"y",2),
                GatewaySession.object("objectId",new java.math.BigDecimal("42.000000000000000001"),"name","Fixture","x",1,"y",2),
                GatewaySession.object("objectId",42,"name","Next character","x",1,"y",2)}) {
            peer.send(GatewaySession.object("type","packet","packet","UserInformation","payload",payload).toString());
            assertNull("Invalid owner must not publish a new world: " + payload, views.poll(200,TimeUnit.MILLISECONDS));
            assertNull("Invalid owner must not reach JNI", gameplayPackets.poll(100,TimeUnit.MILLISECONDS));
        }
        assertEquals("Fixture", original.world.playerName);
        assertEquals(302, original.world.x);
        peer.send(GatewaySession.object("type","packet","packet","UserInformation","payload",
                GatewaySession.object("objectId",42,"name","Fixture","x",303,"y",634,"hp",80)).toString());
        assertEquals(303, phase(GatewaySession.Phase.IN_GAME).world.x);
        assertEquals(80, new JSONObject(gameplayPackets.poll(3,TimeUnit.SECONDS)).getJSONObject("payload").getInt("hp"));
    }

    @Test public void playerWalletPacketsRequireOwnerBootstrapAndSurviveMapLoading() throws Exception {
        connect(); roster(); session.start(7); commands.poll(3, TimeUnit.SECONDS);
        JSONObject delta = GatewaySession.object("gold", 23, "objectId", 42);
        peer.send(GatewaySession.object("type", "packet", "packet", "GainedGold", "payload", delta).toString());
        assertNull(gameplayPackets.poll(200, TimeUnit.MILLISECONDS));
        peer.send("{\"type\":\"packet\",\"packet\":\"StartGame\",\"payload\":{\"result\":4}}");
        phase(GatewaySession.Phase.STARTING);
        peer.send(GatewaySession.object("type", "packet", "packet", "GainedGold", "payload", delta).toString());
        assertNull(gameplayPackets.poll(200, TimeUnit.MILLISECONDS));
        peer.send("{\"type\":\"worldSnapshot\",\"payload\":{\"playerObjectId\":42,\"mapFileName\":\"0\",\"entities\":["
                + "{\"objectId\":42,\"kind\":\"selfPlayer\",\"name\":\"Fixture\",\"x\":302,\"y\":634}]}}");
        assertNotNull(phase(GatewaySession.Phase.IN_GAME).worldSnapshot);
        for (String packet : new String[]{"GainedGold", "LoseGold", "GainedCredit", "LoseCredit"}) {
            JSONObject payload = GatewaySession.object("objectId", 42, "amount", 23);
            peer.send(GatewaySession.object("type", "packet", "packet", packet, "payload", payload).toString());
            String raw = gameplayPackets.poll(3, TimeUnit.SECONDS);
            assertNotNull("Missing wallet packet " + packet, raw);
            JSONObject forwarded = new JSONObject(raw);
            assertEquals(packet, forwarded.getString("packet"));
            assertEquals(23, forwarded.getJSONObject("payload").getInt("amount"));
        }
        peer.send("{\"type\":\"packet\",\"packet\":\"MapChanged\",\"payload\":{\"fileName\":\"1\"}}");
        assertNull(phase(GatewaySession.Phase.STARTING).worldSnapshot);
        peer.send(GatewaySession.object("type", "packet", "packet", "LoseGold", "payload", delta).toString());
        assertEquals("LoseGold", new JSONObject(gameplayPackets.poll(3, TimeUnit.SECONDS)).getString("packet"));
        peer.send("{\"type\":\"packet\",\"packet\":\"qa.giveItem\",\"payload\":{}}");
        assertNull(gameplayPackets.poll(200, TimeUnit.MILLISECONDS));
        session.disconnect("Wallet fixture end"); phase(GatewaySession.Phase.DISCONNECTED);
        assertNull(gameplayPackets.poll(200, TimeUnit.MILLISECONDS));
    }

    @Test public void integerOwnerIdSupportsFullUnsignedRangeAcrossMapLoading() throws Exception {
        connect(); roster(); session.start(7); commands.poll(3, TimeUnit.SECONDS);
        peer.send("{\"type\":\"packet\",\"packet\":\"StartGame\",\"payload\":{\"result\":4}}");
        peer.send("{\"type\":\"worldSnapshot\",\"payload\":{\"playerObjectId\":4294967295,\"mapFileName\":\"0\",\"entities\":["
                + "{\"objectId\":4294967295,\"kind\":\"selfPlayer\",\"name\":\"Fixture\",\"x\":302,\"y\":634}]}}");
        assertNotNull(phase(GatewaySession.Phase.IN_GAME).worldSnapshot);
        peer.send("{\"type\":\"packet\",\"packet\":\"MapChanged\",\"payload\":{\"fileName\":\"1\"}}");
        phase(GatewaySession.Phase.STARTING);
        peer.send(GatewaySession.object("type","packet","packet","UserInformation","payload",
                GatewaySession.object("objectId",4294967295L,"name","Fixture","x",303,"y",634,"hp",80)).toString());
        GatewaySession.View view = phase(GatewaySession.Phase.IN_GAME);
        assertEquals("1",view.world.mapFileName);
        assertEquals(303,view.world.x);
        assertEquals(4294967295L,new JSONObject(gameplayPackets.poll(3,TimeUnit.SECONDS))
                .getJSONObject("payload").getLong("objectId"));
    }

    @Test public void receivedChatRequiresOwnerBootstrapAndPreservesOrderedPeerAndSystemPayloads() throws Exception {
        JSONObject object = GatewaySession.object("type", "packet", "packet", "ObjectChat",
                "payload", GatewaySession.object("objectId", 99, "text", "邻居: hello 👋",
                        "chatType", "Normal", "message", "wrong object field"));
        JSONObject direct = GatewaySession.object("type", "packet", "packet", "Chat",
                "payload", GatewaySession.object("message", "server.CannotPickupNotOwner",
                        "chatType", "System", "text", "wrong direct field"));
        connect();
        peer.send(object.toString());
        assertNull(gameplayPackets.poll(200, TimeUnit.MILLISECONDS));
        roster();
        peer.send(direct.toString());
        assertNull(gameplayPackets.poll(200, TimeUnit.MILLISECONDS));
        session.start(7); commands.poll(3, TimeUnit.SECONDS);
        peer.send(object.toString());
        assertNull(gameplayPackets.poll(200, TimeUnit.MILLISECONDS));
        peer.send("{\"type\":\"packet\",\"packet\":\"StartGame\",\"payload\":{\"result\":4}}");
        phase(GatewaySession.Phase.STARTING);
        peer.send(direct.toString());
        assertNull(gameplayPackets.poll(200, TimeUnit.MILLISECONDS));
        peer.send("{\"type\":\"worldSnapshot\",\"payload\":{\"playerObjectId\":42,\"mapFileName\":\"0\",\"entities\":["
                + "{\"objectId\":42,\"kind\":\"selfPlayer\",\"name\":\"Fixture\",\"x\":302,\"y\":634}]}}");
        assertNotNull(phase(GatewaySession.Phase.IN_GAME).worldSnapshot);
        peer.send(object.toString());
        peer.send(direct.toString());
        JSONObject first = new JSONObject(gameplayPackets.poll(3, TimeUnit.SECONDS));
        JSONObject second = new JSONObject(gameplayPackets.poll(3, TimeUnit.SECONDS));
        assertEquals("ObjectChat", first.getString("packet"));
        assertEquals(99, first.getJSONObject("payload").getInt("objectId"));
        assertEquals("邻居: hello 👋", first.getJSONObject("payload").getString("text"));
        assertEquals("Normal", first.getJSONObject("payload").getString("chatType"));
        assertEquals("Chat", second.getString("packet"));
        assertEquals("server.CannotPickupNotOwner", second.getJSONObject("payload").getString("message"));
        assertEquals("System", second.getJSONObject("payload").getString("chatType"));
        assertNull(receipts.poll(200, TimeUnit.MILLISECONDS));
        peer.send("{\"type\":\"packet\",\"packet\":\"MapChanged\",\"payload\":{\"fileName\":\"1\"}}");
        phase(GatewaySession.Phase.STARTING);
        peer.send(direct.toString());
        assertEquals("Chat", new JSONObject(gameplayPackets.poll(3, TimeUnit.SECONDS)).getString("packet"));
        peer.send(GatewaySession.object("type", "packet", "packet", "AdminChat",
                "payload", GatewaySession.object("message", "not public")).toString());
        assertNull(gameplayPackets.poll(200, TimeUnit.MILLISECONDS));
        session.disconnect("Chat test end");
        phase(GatewaySession.Phase.DISCONNECTED);
        assertNull(gameplayPackets.poll(200, TimeUnit.MILLISECONDS));
    }

    @Test public void personalSkillPacketsForwardOnlyInsideAuthenticatedGameAndPreservePayload() throws Exception {
        connect();
        JSONObject cast = GatewaySession.object("type", "packet", "packet", "Magic",
                "payload", GatewaySession.object("spell", "FireBall", "cast", true));
        peer.send(cast.toString());
        assertNull(gameplayPackets.poll(200, TimeUnit.MILLISECONDS));
        roster(); session.start(7); commands.poll(3, TimeUnit.SECONDS);
        peer.send("{\"type\":\"packet\",\"packet\":\"StartGame\",\"payload\":{\"result\":4}}");
        peer.send("{\"type\":\"worldSnapshot\",\"payload\":{\"playerObjectId\":42,\"mapFileName\":\"0\",\"entities\":["
                + "{\"objectId\":42,\"kind\":\"selfPlayer\",\"name\":\"Fixture\",\"x\":302,\"y\":634}]}}");
        assertNotNull(phase(GatewaySession.Phase.IN_GAME).worldSnapshot);
        for (String packet : new String[]{"Magic", "MagicCast", "MagicDelay", "SpellToggle",
                "NewMagic", "MagicLeveled", "RemoveMagic", "UserInformation"}) {
            JSONObject payload = GatewaySession.object("objectId", 42, "spell", "FireBall",
                    "cast", false, "delay", 2200, "canUse", false, "hero", false,
                    "name", "Fixture", "mp", 10, "maxMp", 20);
            peer.send(GatewaySession.object("type", "packet", "packet", packet,
                    "payload", payload).toString());
            String raw = gameplayPackets.poll(3, TimeUnit.SECONDS);
            assertNotNull("Missing owner packet " + packet, raw);
            JSONObject forwarded = new JSONObject(raw);
            assertEquals(packet, forwarded.getString("packet"));
            assertEquals(42, forwarded.getJSONObject("payload").getInt("objectId"));
            assertFalse(forwarded.getJSONObject("payload").getBoolean("cast"));
            assertEquals(2200, forwarded.getJSONObject("payload").getInt("delay"));
        }
        peer.send(GatewaySession.object("type", "packet", "packet", "Stage5Command",
                "payload", GatewaySession.object("success", true)).toString());
        assertNull(gameplayPackets.poll(200, TimeUnit.MILLISECONDS));
        session.disconnect("Skill test end");
        phase(GatewaySession.Phase.DISCONNECTED);
        assertNull(gameplayPackets.poll(200, TimeUnit.MILLISECONDS));
    }

    @Test public void personalSkillPacketsSurviveMapTransitionButNotInitialBootstrap() throws Exception {
        connect(); roster(); session.start(7); commands.poll(3, TimeUnit.SECONDS);
        JSONObject cast = GatewaySession.object("type", "packet", "packet", "MagicCast",
                "payload", GatewaySession.object("spell", "FireBall"));
        peer.send(cast.toString());
        assertNull(gameplayPackets.poll(200, TimeUnit.MILLISECONDS));
        peer.send("{\"type\":\"packet\",\"packet\":\"StartGame\",\"payload\":{\"result\":4}}");
        phase(GatewaySession.Phase.STARTING);
        peer.send(cast.toString());
        assertNull(gameplayPackets.poll(200, TimeUnit.MILLISECONDS));
        peer.send("{\"type\":\"worldSnapshot\",\"payload\":{\"playerObjectId\":42,\"mapFileName\":\"0\",\"entities\":["
                + "{\"objectId\":42,\"kind\":\"selfPlayer\",\"name\":\"Fixture\",\"x\":302,\"y\":634}]}}");
        assertNotNull(phase(GatewaySession.Phase.IN_GAME).worldSnapshot);
        peer.send("{\"type\":\"packet\",\"packet\":\"MapChanged\",\"payload\":{\"fileName\":\"1\"}}");
        GatewaySession.View transition = phase(GatewaySession.Phase.STARTING);
        assertNull(transition.world);
        assertNull(transition.worldSnapshot);
        for (String packet : new String[]{"MagicCast", "MagicDelay", "NewMagic"}) {
            peer.send(GatewaySession.object("type", "packet", "packet", packet,
                    "payload", GatewaySession.object("objectId", 42, "spell", "FireBall")).toString());
            String raw = gameplayPackets.poll(3, TimeUnit.SECONDS);
            assertNotNull("Missing transition packet " + packet, raw);
            assertEquals(packet, new JSONObject(raw).getString("packet"));
        }
        peer.send("{\"type\":\"packet\",\"packet\":\"ObjectWalk\",\"payload\":{\"objectId\":43,\"x\":4,\"y\":5}}");
        assertNull(gameplayPackets.poll(200, TimeUnit.MILLISECONDS));
        session.disconnect("Transition test end");
        phase(GatewaySession.Phase.DISCONNECTED);
        assertNull(gameplayPackets.poll(200, TimeUnit.MILLISECONDS));
    }

    @Test public void npcPacketsRequireAnAuthenticatedCurrentSceneAndKeepTheirPayload() throws Exception {
        connect();
        JSONObject page = GatewaySession.object("page", new org.json.JSONArray().put("Welcome").put("<Buy/@BuySell>"));
        peer.send(GatewaySession.object("type", "packet", "packet", "NPCResponse", "payload", page).toString());
        assertNull(gameplayPackets.poll(200, TimeUnit.MILLISECONDS));
        roster(); session.start(7); commands.poll(3, TimeUnit.SECONDS);
        peer.send("{\"type\":\"packet\",\"packet\":\"StartGame\",\"payload\":{\"result\":4}}");
        phase(GatewaySession.Phase.STARTING);
        peer.send(GatewaySession.object("type", "packet", "packet", "NPCResponse", "payload", page).toString());
        assertNull(gameplayPackets.poll(200, TimeUnit.MILLISECONDS));
        String world = "{\"type\":\"worldSnapshot\",\"payload\":{\"playerObjectId\":42,\"mapFileName\":\"0\",\"entities\":["
                + "{\"objectId\":42,\"kind\":\"selfPlayer\",\"name\":\"Fixture\",\"x\":302,\"y\":634}]}}";
        peer.send(world);
        assertNotNull(phase(GatewaySession.Phase.IN_GAME).worldSnapshot);
        String[] names = {"NPCResponse", "NPCGoods", "NPCPearlGoods", "NPCSell", "NPCRepair", "NPCSRepair"};
        Object[] payloads = {page, GatewaySession.object("list", new org.json.JSONArray()),
                GatewaySession.object("list", new org.json.JSONArray()), JSONObject.NULL,
                GatewaySession.object("rate", 1.5), GatewaySession.object("rate", 2.0)};
        for (int i = 0; i < names.length; i++) {
            JSONObject envelope = GatewaySession.object("type", "packet", "packet", names[i], "payload", payloads[i]);
            peer.send(envelope.toString());
            String raw = gameplayPackets.poll(3, TimeUnit.SECONDS);
            assertNotNull("Missing NPC packet " + names[i], raw);
            assertEquals(envelope.toString(), new JSONObject(raw).toString());
        }
        peer.send("{\"type\":\"packet\",\"packet\":\"MapChanged\",\"payload\":{\"fileName\":\"1\"}}");
        phase(GatewaySession.Phase.STARTING);
        peer.send(GatewaySession.object("type", "packet", "packet", "NPCSell", "payload", JSONObject.NULL).toString());
        assertNull(gameplayPackets.poll(200, TimeUnit.MILLISECONDS));
        peer.send("{\"type\":\"packet\",\"packet\":\"UserLocation\",\"payload\":{\"x\":50,\"y\":60}}");
        phase(GatewaySession.Phase.IN_GAME);
        peer.send(GatewaySession.object("type", "packet", "packet", "NPCSell", "payload", JSONObject.NULL).toString());
        assertNull("Coordinates alone cannot authorize the new scene", gameplayPackets.poll(200, TimeUnit.MILLISECONDS));
        peer.send(world.replace("\"mapFileName\":\"0\"", "\"mapFileName\":\"1\"")
                .replace("\"x\":302", "\"x\":50").replace("\"y\":634", "\"y\":60"));
        assertNotNull(phase(GatewaySession.Phase.IN_GAME).worldSnapshot);
        peer.send(GatewaySession.object("type", "packet", "packet", "NPCSell", "payload", JSONObject.NULL).toString());
        assertEquals("NPCSell", new JSONObject(gameplayPackets.poll(3, TimeUnit.SECONDS)).getString("packet"));
        for (String unsupported : new String[]{"qa.openStorage", "Stage5Command", "NPCStorage"}) {
            peer.send(GatewaySession.object("type", "packet", "packet", unsupported, "payload", new JSONObject()).toString());
            assertNull(gameplayPackets.poll(100, TimeUnit.MILLISECONDS));
        }
    }

    @Test public void heroMetadataRequiresListedStartAndPreservesPublicPackets() throws Exception {
        String[] packets = {"HeroInformation", "HeroBaseStatsInfo", "HeroHealthChanged",
                "UpdateHeroSpawnState", "TransferHeroItem", "TakeBackHeroItem",
                "SetAutoPotValue", "SetAutoPotItem", "UseItem", "DeleteItem",
                "MoveItem", "EquipItem", "RemoveItem", "MergeItem", "ObjectHero",
                "ObjectRemove", "ObjectDied", "MountUpdate", "FishingUpdate", "ObjectPoisoned",
                "NewMagic", "ObjectMagic", "MagicDelay", "MagicLeveled"};
        connect();
        for (String packet : packets) peer.send(GatewaySession.object("type", "packet", "packet", packet,
                "payload", new JSONObject()).toString());
        assertNull(gameplayPackets.poll(200, TimeUnit.MILLISECONDS));
        roster();
        for (String packet : packets) peer.send(GatewaySession.object("type", "packet", "packet", packet,
                "payload", new JSONObject()).toString());
        assertNull(gameplayPackets.poll(200, TimeUnit.MILLISECONDS));
        session.start(7); commands.poll(3, TimeUnit.SECONDS);
        assertNull("Hero metadata is not owner/bootstrap authority", phase(GatewaySession.Phase.STARTING).world);
        for (String packet : packets) {
            JSONObject envelope = GatewaySession.object("type", "packet", "packet", packet,
                    "payload", GatewaySession.object("marker", packet, "grid", "HeroInventory",
                            "gridFrom", "HeroInventory", "gridTo", "Inventory", "hero", true));
            peer.send(envelope.toString());
            String raw = gameplayPackets.poll(3, TimeUnit.SECONDS);
            assertNotNull("Public Hero packet missing from Android host: " + packet, raw);
            assertEquals(envelope.toString(), WireJson.decode(raw).toString());
        }
        assertNull("Hero information is not a storage transfer receipt", receipts.poll(100, TimeUnit.MILLISECONDS));
        for (String unsupported : new String[]{"HeroModel", "heroInventory", "stage5Command", "qa.giveItem"}) {
            peer.send(GatewaySession.object("type", "packet", "packet", unsupported,
                    "payload", new JSONObject()).toString());
            assertNull(gameplayPackets.poll(100, TimeUnit.MILLISECONDS));
        }
        peer.send("{\"type\":\"packet\",\"packet\":\"StartGame\",\"payload\":{\"result\":1}}");
        phase(GatewaySession.Phase.CHARACTERS);
        peer.send(GatewaySession.object("type", "packet", "packet", "HeroInformation",
                "payload", new JSONObject()).toString());
        assertNull("Rejected Start retires Hero forwarding", gameplayPackets.poll(200, TimeUnit.MILLISECONDS));
    }


    private static JSONObject heroWireItem(Object id) {
        return GatewaySession.object("unique_id",id,"item_index",27,"current_dura",123,"max_dura",456,
                "count",201,"soul_bound_id",-1,"identified",true,"cursed",false,
                "slots",new org.json.JSONArray(),"gem_count",0,"added_stats",new org.json.JSONArray(),
                "awake_type",0,"awake_values",new org.json.JSONArray(),"refined_value",0,"refine_added",0,
                "refine_success_chance",0,"wedding_ring",-1,"expire_info",JSONObject.NULL,
                "rental_information",JSONObject.NULL,"is_shop_item",false,"sealed_info",JSONObject.NULL,"gm_made",false);
    }

    @Test public void fullHeroInformationPreserves42Cells14EquipmentAndUnsignedNestedItems() throws Exception {
        connect(); roster(); session.start(7); commands.poll(3,TimeUnit.SECONDS);
        phase(GatewaySession.Phase.STARTING);
        org.json.JSONArray inventory=new org.json.JSONArray(), equipment=new org.json.JSONArray();
        for (int i=0;i<42;i++) {
            JSONObject item=heroWireItem(i==3 ? new java.math.BigInteger("18446744073709551615") : Long.valueOf(19000+i));
            org.json.JSONArray sockets=item.getJSONArray("slots");
            for (int j=0;j<3;j++) sockets.put(heroWireItem(21000+i*3+j));
            inventory.put(item);
        }
        for (int i=0;i<14;i++) equipment.put(i==0 ? heroWireItem(70001) : JSONObject.NULL);
        JSONObject info=GatewaySession.object("object_id",12,"name","OFFLINE TLS Hero","class","Wizard",
                "gender","Female","level",20,"hair",0,"hp",100,"mp",30,"experience",10,"max_experience",100,
                "inventory",inventory,"equipment",equipment,"magics",new org.json.JSONArray(),
                "auto_pot",false,"auto_hp_percent",30,"auto_mp_percent",40,"hp_item_index",27,"mp_item_index",17);
        JSONObject envelope=GatewaySession.object("type","packet","packet","HeroInformation",
                "payload",GatewaySession.object("info",info));
        int bytes=envelope.toString().getBytes(java.nio.charset.StandardCharsets.UTF_8).length;
        assertTrue(bytes > 65536 && bytes < 512*1024);
        peer.send(envelope.toString());
        String raw=gameplayPackets.poll(3,TimeUnit.SECONDS);
        assertNotNull("Full Hero information must reach the typed Rust decoder",raw);
        assertEquals(envelope.toString(),WireJson.decode(raw).toString());
        JSONObject actual=WireJson.decode(raw).getJSONObject("payload").getJSONObject("info");
        assertEquals(42,actual.getJSONArray("inventory").length());
        assertEquals(14,actual.getJSONArray("equipment").length());
        JSONObject carried=actual.getJSONArray("inventory").getJSONObject(3);
        assertEquals("18446744073709551615",carried.opt("unique_id").toString());
        assertEquals(201,carried.getInt("count"));
        assertEquals(3,carried.getJSONArray("slots").length());
        assertNull("Hero info is not a private transfer ACK",receipts.poll(100,TimeUnit.MILLISECONDS));
        peer.send("{\"type\":\"packet\",\"packet\":\"StartGame\",\"payload\":{\"result\":1}}");
        phase(GatewaySession.Phase.CHARACTERS);
    }

    @Test public void oversizedHeroInformationFailsClosedBeforeOwnerBootstrap() throws Exception {
        connect(); roster(); session.start(7); commands.poll(3,TimeUnit.SECONDS);
        phase(GatewaySession.Phase.STARTING);
        peer.send(GatewaySession.object("type","packet","packet","HeroInformation",
                "payload",GatewaySession.object("info",new JSONObject(),"probe","文".repeat(180000))).toString());
        phase(GatewaySession.Phase.DISCONNECTED);
        assertNull(gameplayPackets.poll(200,TimeUnit.MILLISECONDS));
        assertNull(receipts.poll(100,TimeUnit.MILLISECONDS));
    }

    @Test public void smallHeroReceiptDoesNotAcquireLargeInformationByteBudget() throws Exception {
        connect(); roster(); session.start(7); commands.poll(3,TimeUnit.SECONDS);
        phase(GatewaySession.Phase.STARTING);
        peer.send(GatewaySession.object("type","packet","packet","HeroHealthChanged",
                "payload",GatewaySession.object("hp",80,"mp",20,"probe","文".repeat(7000))).toString());
        phase(GatewaySession.Phase.DISCONNECTED);
        assertNull(gameplayPackets.poll(200,TimeUnit.MILLISECONDS));
        assertNull(receipts.poll(100,TimeUnit.MILLISECONDS));
    }

    @Test public void heroMetadataSurvivesSameOwnerMapLoadWithoutOpeningPlayerReceiptRoutes() throws Exception {
        connect(); roster(); session.start(7); commands.poll(3,TimeUnit.SECONDS);
        peer.send("{\"type\":\"packet\",\"packet\":\"StartGame\",\"payload\":{\"result\":4}}");
        peer.send("{\"type\":\"worldSnapshot\",\"payload\":{\"playerObjectId\":42,\"mapFileName\":\"0\",\"entities\":["
                + "{\"objectId\":42,\"kind\":\"selfPlayer\",\"name\":\"Fixture\",\"x\":302,\"y\":634}]}}");
        phase(GatewaySession.Phase.IN_GAME);
        peer.send("{\"type\":\"packet\",\"packet\":\"MapChanged\",\"payload\":{\"fileName\":\"1\"}}");
        assertNull(phase(GatewaySession.Phase.STARTING).worldSnapshot);
        for (String name:new String[]{"HeroHealthChanged","SetAutoPotValue","TakeBackHeroItem"}) {
            JSONObject envelope=GatewaySession.object("type","packet","packet",name,
                    "payload",GatewaySession.object("hp",80,"mp",20,"success",false,"stat",12,"value",40));
            peer.send(envelope.toString());
            String raw=gameplayPackets.poll(3,TimeUnit.SECONDS);
            assertNotNull(raw);
            assertEquals(envelope.toString(),WireJson.decode(raw).toString());
        }
        JSONObject playerDeletion=GatewaySession.object("type","packet","packet","DeleteItem","payload",
                GatewaySession.object("grid","Inventory","heroInventory",false,"uniqueId",99,"count",1));
        peer.send(playerDeletion.toString());
        assertNull("Hero route must not enable player DeleteItem",gameplayPackets.poll(200,TimeUnit.MILLISECONDS));
        peer.send(GatewaySession.object("type","packet","packet","DeleteItem","payload",
                GatewaySession.object("uniqueId",new java.math.BigInteger("18446744073709551615"),"count",1)).toString());
        assertEquals("18446744073709551615",WireJson.decode(gameplayPackets.poll(3,TimeUnit.SECONDS))
                .getJSONObject("payload").opt("uniqueId").toString());
        assertNull(receipts.poll(100,TimeUnit.MILLISECONDS));
        session.disconnect("OFFLINE Hero fixture end");phase(GatewaySession.Phase.DISCONNECTED);
        peer.send(GatewaySession.object("type","packet","packet","HeroHealthChanged",
                "payload",new JSONObject()).toString());
        assertNull(gameplayPackets.poll(200,TimeUnit.MILLISECONDS));
    }

    @Test public void questMetadataStagesOnlyDuringAuthenticatedSelectedStart() throws Exception {
        JSONObject definition = GatewaySession.object("type", "packet", "packet", "NewQuestInfo",
                "payload", GatewaySession.object("id", 2100004, "name", "OFFLINE TLS quest"));
        JSONObject completed = GatewaySession.object("type", "packet", "packet", "CompleteQuest",
                "payload", GatewaySession.object("completedQuests", new org.json.JSONArray().put(1)));
        connect();
        peer.send(definition.toString()); peer.send(completed.toString());
        assertNull(gameplayPackets.poll(200, TimeUnit.MILLISECONDS));
        roster();
        peer.send(definition.toString());
        assertNull(gameplayPackets.poll(200, TimeUnit.MILLISECONDS));
        session.start(7);
        assertEquals("startGame", commands.poll(3, TimeUnit.SECONDS).getString("type"));
        assertNull("Metadata is not world bootstrap", phase(GatewaySession.Phase.STARTING).world);
        for (JSONObject envelope : new JSONObject[]{definition, completed}) {
            peer.send(envelope.toString());
            String raw = gameplayPackets.poll(3, TimeUnit.SECONDS);
            assertNotNull("Selected Start must retain packet-first quest metadata", raw);
            assertEquals(envelope.toString(), new JSONObject(raw).toString());
        }
        peer.send("{\"type\":\"packet\",\"packet\":\"StartGame\",\"payload\":{\"result\":1}}");
        phase(GatewaySession.Phase.CHARACTERS);
        peer.send(definition.toString()); peer.send(completed.toString());
        assertNull("Rejected Start must not keep accepting metadata", gameplayPackets.poll(200, TimeUnit.MILLISECONDS));
        session.disconnect("TLS fixture ended");
        assertTrue(phase(GatewaySession.Phase.DISCONNECTED).characters.isEmpty());
    }

    @Test public void mailMetadataStagesOnlyDuringAuthenticatedSelectedStart() throws Exception {
        connect();
        JSONObject payload = GatewaySession.object("mail", new org.json.JSONArray(),
                "cost", 125, "uniqueId", "77", "locked", true);
        JSONObject incoming = GatewaySession.object("type", "packet", "packet", "ReceiveMail", "payload", payload);
        peer.send(incoming.toString());
        assertNull(gameplayPackets.poll(200, TimeUnit.MILLISECONDS));
        roster();
        peer.send(incoming.toString());
        assertNull(gameplayPackets.poll(200, TimeUnit.MILLISECONDS));
        session.start(7);
        assertEquals("startGame", commands.poll(3, TimeUnit.SECONDS).getString("type"));
        GatewaySession.View startingMail = phase(GatewaySession.Phase.STARTING);
        assertNull(startingMail.worldSnapshot);
        for (String packet : new String[]{"ReceiveMail", "MailSendRequest", "MailCost", "MailLockedItem"}) {
            peer.send(GatewaySession.object("type", "packet", "packet", packet, "payload", payload).toString());
            String raw = gameplayPackets.poll(3, TimeUnit.SECONDS);
            assertNotNull("Missing mail metadata " + packet, raw);
            JSONObject forwarded = new JSONObject(raw);
            assertEquals(packet, forwarded.getString("packet"));
            assertEquals(125, forwarded.getJSONObject("payload").getInt("cost"));
            assertEquals("77", forwarded.getJSONObject("payload").getString("uniqueId"));
        }
        assertNull(receipts.poll(200, TimeUnit.MILLISECONDS));
        for (String unsupported : new String[]{"MailSent", "ParcelCollected", "AdminMail", "Stage5Command"}) {
            peer.send(GatewaySession.object("type", "packet", "packet", unsupported,
                    "payload", GatewaySession.object("result", 1, "success", true)).toString());
            assertNull("Unsupported mail receipt " + unsupported, gameplayPackets.poll(200, TimeUnit.MILLISECONDS));
        }
        assertNull(receipts.poll(200, TimeUnit.MILLISECONDS));
    }

    @Test public void mailMetadataSurvivesSameOwnerMapTransitionWithoutBootstrap() throws Exception {
        connect(); roster(); session.start(7); commands.poll(3, TimeUnit.SECONDS);
        peer.send("{\"type\":\"packet\",\"packet\":\"StartGame\",\"payload\":{\"result\":4}}");
        peer.send("{\"type\":\"worldSnapshot\",\"payload\":{\"playerObjectId\":42,\"mapFileName\":\"0\",\"entities\":["
                + "{\"objectId\":42,\"kind\":\"selfPlayer\",\"name\":\"Fixture\",\"x\":302,\"y\":634}]}}");
        assertNotNull(phase(GatewaySession.Phase.IN_GAME).worldSnapshot);
        peer.send("{\"type\":\"packet\",\"packet\":\"MapChanged\",\"payload\":{\"fileName\":\"1\"}}");
        GatewaySession.View transition = phase(GatewaySession.Phase.STARTING);
        assertNull(transition.worldSnapshot);
        for (String packet : new String[]{"ReceiveMail", "MailCost", "MailLockedItem"}) {
            peer.send(GatewaySession.object("type", "packet", "packet", packet,
                    "payload", GatewaySession.object("mail", new org.json.JSONArray(),
                            "data", GatewaySession.object("cost", 321, "uniqueId", "88", "locked", false))).toString());
            String raw = gameplayPackets.poll(3, TimeUnit.SECONDS);
            assertNotNull("Missing transition mail " + packet, raw);
            assertEquals(packet, new JSONObject(raw).getString("packet"));
        }
        assertNull(receipts.poll(200, TimeUnit.MILLISECONDS));
        session.disconnect("Mail fixture end");
        phase(GatewaySession.Phase.DISCONNECTED);
        assertNull(gameplayPackets.poll(200, TimeUnit.MILLISECONDS));
    }

    @Test public void fullMailMetadataRetains256RowsAndAllFiveAttachments() throws Exception {
        connect(); roster(); session.start(7); commands.poll(3, TimeUnit.SECONDS);
        org.json.JSONArray rows = new org.json.JSONArray();
        for (int id = 1; id <= 256; id++) {
            org.json.JSONArray items = new org.json.JSONArray();
            for (int slot = 1; slot <= 5; slot++) {
                items.put(GatewaySession.object("uniqueId", String.valueOf(id * 10 + slot),
                        "name", "RedPotion", "count", slot));
            }
            rows.put(GatewaySession.object("mailId", String.valueOf(id), "senderName", "NPC",
                    "message", "邮件正文".repeat(20), "items", items,
                    "canReply", true, "dateSentBinaryDatetime", "638970336000000000"));
        }
        String incoming = GatewaySession.object("type", "packet", "packet", "ReceiveMail",
                "payload", GatewaySession.object("mail", rows)).toString();
        int bytes = incoming.getBytes(java.nio.charset.StandardCharsets.UTF_8).length;
        assertTrue(bytes > 16 * 1024 && bytes <= 512 * 1024);
        peer.send(incoming);
        String raw = gameplayPackets.poll(3, TimeUnit.SECONDS);
        assertNotNull("A complete bounded mailbox must not be dropped", raw);
        org.json.JSONArray forwarded = new JSONObject(raw).getJSONObject("payload").getJSONArray("mail");
        assertEquals(256, forwarded.length());
        assertEquals("256", forwarded.getJSONObject(255).getString("mailId"));
        for (int i = 0; i < forwarded.length(); i++) {
            assertEquals(5, forwarded.getJSONObject(i).getJSONArray("items").length());
        }
        assertEquals(incoming, raw);
        assertNull(receipts.poll(200, TimeUnit.MILLISECONDS));
    }

    @Test public void oversizedMailServiceKeepsTheExistingSmallPacketLimit() throws Exception {
        connect(); roster(); session.start(7); commands.poll(3, TimeUnit.SECONDS);
        peer.send(GatewaySession.object("type", "packet", "packet", "MailCost",
                "payload", GatewaySession.object("cost", 125, "unused", "x".repeat(16 * 1024 + 1))).toString());
        phase(GatewaySession.Phase.DISCONNECTED);
        assertNull(gameplayPackets.poll(200, TimeUnit.MILLISECONDS));
        assertNull(receipts.poll(200, TimeUnit.MILLISECONDS));
    }

    @Test public void oversizedMailMetadataTerminatesInsteadOfSilentlyDroppingInbox() throws Exception {
        connect(); roster(); session.start(7); commands.poll(3, TimeUnit.SECONDS);
        peer.send(GatewaySession.object("type", "packet", "packet", "ReceiveMail",
                "payload", GatewaySession.object("mail", new org.json.JSONArray(), "body", "x".repeat(512 * 1024 + 1))).toString());
        phase(GatewaySession.Phase.DISCONNECTED);
        assertNull(gameplayPackets.poll(200, TimeUnit.MILLISECONDS));
        assertNull(receipts.poll(200, TimeUnit.MILLISECONDS));
    }

    @Test public void storageMetadataStagesOnlyDuringAuthenticatedSelectedStart() throws Exception {
        JSONObject[] metadata = new JSONObject[]{
            GatewaySession.object("type", "packet", "packet", "UserStorage", "payload",
                    GatewaySession.object("storage", new org.json.JSONArray().put(JSONObject.NULL)
                            .put(GatewaySession.object("uniqueId", "17", "name", "TLS storage fixture")))),
            GatewaySession.object("type", "packet", "packet", "StorageUnlockResult", "payload",
                    GatewaySession.object("result", 2, "hasPassword", true)),
            GatewaySession.object("type", "packet", "packet", "StoragePasswordResult", "payload",
                    GatewaySession.object("result", 4, "hasPassword", true, "removing", false)),
            GatewaySession.object("type", "packet", "packet", "ResizeStorage", "payload",
                    GatewaySession.object("size", 160, "hasExpandedStorage", true,
                            "expiryTimeBinaryDatetime", "635000000000000000"))
        };
        connect();
        for (JSONObject envelope : metadata) peer.send(envelope.toString());
        assertNull(gameplayPackets.poll(200, TimeUnit.MILLISECONDS));
        roster();
        for (JSONObject envelope : metadata) peer.send(envelope.toString());
        assertNull(gameplayPackets.poll(200, TimeUnit.MILLISECONDS));
        session.start(7); commands.poll(3, TimeUnit.SECONDS);
        assertNull("Storage metadata cannot bootstrap a character", phase(GatewaySession.Phase.STARTING).world);
        for (JSONObject envelope : metadata) {
            peer.send(envelope.toString());
            String raw = gameplayPackets.poll(3, TimeUnit.SECONDS);
            assertNotNull("Selected Start must retain public storage metadata", raw);
            assertEquals(envelope.toString(), new JSONObject(raw).toString());
        }
        assertNull("Storage metadata is not an exact transfer receipt", receipts.poll(100, TimeUnit.MILLISECONDS));
        for (String unsupported : new String[]{"NPCStorage", "qa.openStorage", "GuildStorageContents"}) {
            peer.send(GatewaySession.object("type", "packet", "packet", unsupported,
                    "payload", new JSONObject()).toString());
            assertNull(gameplayPackets.poll(100, TimeUnit.MILLISECONDS));
        }
        peer.send("{\"type\":\"packet\",\"packet\":\"StartGame\",\"payload\":{\"result\":1}}");
        phase(GatewaySession.Phase.CHARACTERS);
        for (JSONObject envelope : metadata) peer.send(envelope.toString());
        assertNull("Rejected Start closes storage forwarding", gameplayPackets.poll(200, TimeUnit.MILLISECONDS));
    }

    @Test public void oversizedStorageMetadataFailsClosedBeforeOwnerBootstrap() throws Exception {
        connect(); roster(); session.start(7); commands.poll(3, TimeUnit.SECONDS);
        phase(GatewaySession.Phase.STARTING);
        peer.send(GatewaySession.object("type", "packet", "packet", "UserStorage", "payload",
                GatewaySession.object("storage", new org.json.JSONArray().put(
                        GatewaySession.object("uniqueId", "17", "name", "文".repeat(6000))))).toString());
        phase(GatewaySession.Phase.DISCONNECTED);
        assertNull(gameplayPackets.poll(200, TimeUnit.MILLISECONDS));
        assertNull(receipts.poll(100, TimeUnit.MILLISECONDS));
    }

    @Test public void socialMetadataRequiresListedStartAndPreservesEveryPublicPacket() throws Exception {
        String[] packets = {"SwitchGroup", "DeleteGroup", "DeleteMember", "GroupInvite",
                "GroupInviteResult", "AddMember", "GroupMembersMap", "GroupMemberInfo",
                "GuildStatus", "GuildNoticeChange", "GuildNoticeResult", "GuildMemberChange",
                "GuildStorageGoldChange", "GuildStorageList", "GuildStorageItemChange",
                "GuildInvite", "GuildInviteResult", "TradeRequest", "TradeAccept", "TradeGold",
                "TradeItem", "TradeConfirm", "TradeCancel", "DepositTradeItem", "RetrieveTradeItem"};
        connect();
        for (String packet : packets) peer.send(GatewaySession.object("type", "packet", "packet", packet,
                "payload", new JSONObject()).toString());
        assertNull(gameplayPackets.poll(200, TimeUnit.MILLISECONDS));
        roster();
        for (String packet : packets) peer.send(GatewaySession.object("type", "packet", "packet", packet,
                "payload", new JSONObject()).toString());
        assertNull(gameplayPackets.poll(200, TimeUnit.MILLISECONDS));
        session.start(7); commands.poll(3, TimeUnit.SECONDS);
        assertNull("Social metadata is not owner/bootstrap authority", phase(GatewaySession.Phase.STARTING).world);
        for (String packet : packets) {
            JSONObject envelope = GatewaySession.object("type", "packet", "packet", packet,
                    "payload", GatewaySession.object("marker", packet, "unique_id", "18446744073709551615"));
            peer.send(envelope.toString());
            String raw = gameplayPackets.poll(3, TimeUnit.SECONDS);
            assertNotNull("Public social packet missing from Android host: " + packet, raw);
            assertEquals(envelope.toString(), WireJson.decode(raw).toString());
        }
        assertNull("Social data is not a private transfer receipt", receipts.poll(100, TimeUnit.MILLISECONDS));
        for (String unsupported : new String[]{"GuildStorageContents", "GroupTeleport", "stage5Command", "qa.giveItem"}) {
            peer.send(GatewaySession.object("type", "packet", "packet", unsupported,
                    "payload", new JSONObject()).toString());
            assertNull(gameplayPackets.poll(100, TimeUnit.MILLISECONDS));
        }
        peer.send("{\"type\":\"packet\",\"packet\":\"StartGame\",\"payload\":{\"result\":1}}");
        phase(GatewaySession.Phase.CHARACTERS);
        peer.send(GatewaySession.object("type", "packet", "packet", "GuildStatus",
                "payload", new JSONObject()).toString());
        assertNull("Rejected Start retires social metadata", gameplayPackets.poll(200, TimeUnit.MILLISECONDS));
    }

    @Test public void socialLargeReadModelsHaveOnlyTheirOwnBoundedRoute() throws Exception {
        connect(); roster(); session.start(7); commands.poll(3, TimeUnit.SECONDS);
        phase(GatewaySession.Phase.STARTING);
        for (String packet : new String[]{"GroupMemberInfo", "GuildMemberChange", "GuildStorageList",
                "TradeItem", "GuildNoticeChange"}) {
            JSONObject envelope = GatewaySession.object("type", "packet", "packet", packet,
                    "payload", GatewaySession.object("probe", "文".repeat(7000)));
            assertTrue(envelope.toString().getBytes(java.nio.charset.StandardCharsets.UTF_8).length > 16 * 1024);
            peer.send(envelope.toString());
            String raw = gameplayPackets.poll(3, TimeUnit.SECONDS);
            assertNotNull("Bounded large social read model must reach Rust validation: " + packet, raw);
            assertEquals(envelope.toString(), WireJson.decode(raw).toString());
        }
        peer.send(GatewaySession.object("type", "packet", "packet", "TradeGold", "payload",
                GatewaySession.object("amount", 1, "probe", "文".repeat(7000))).toString());
        phase(GatewaySession.Phase.DISCONNECTED);
        assertNull("A small social receipt does not acquire a large cap", gameplayPackets.poll(200, TimeUnit.MILLISECONDS));
    }

    @Test public void socialReadModelAboveHardCapFailsClosed() throws Exception {
        connect(); roster(); session.start(7); commands.poll(3, TimeUnit.SECONDS);
        phase(GatewaySession.Phase.STARTING);
        peer.send(GatewaySession.object("type", "packet", "packet", "GuildNoticeChange", "payload",
                GatewaySession.object("notice", new org.json.JSONArray().put("a"),
                        "probe", "文".repeat(180000))).toString());
        phase(GatewaySession.Phase.DISCONNECTED);
        assertNull(gameplayPackets.poll(200, TimeUnit.MILLISECONDS));
    }

    @Test public void socialMetadataSurvivesAcceptedOwnerMapTransitionWithoutAuthorizingScene() throws Exception {
        connect(); roster(); session.start(7); commands.poll(3, TimeUnit.SECONDS);
        peer.send("{\"type\":\"packet\",\"packet\":\"StartGame\",\"payload\":{\"result\":4}}");
        peer.send("{\"type\":\"worldSnapshot\",\"payload\":{\"playerObjectId\":42,\"mapFileName\":\"0\",\"entities\":["
                + "{\"objectId\":42,\"kind\":\"selfPlayer\",\"name\":\"Fixture\",\"x\":302,\"y\":634}]}}");
        phase(GatewaySession.Phase.IN_GAME);
        JSONObject social = GatewaySession.object("type", "packet", "packet", "GroupInvite", "payload",
                GatewaySession.object("name", "Alice"));
        peer.send(social.toString());
        assertEquals(social.toString(), WireJson.decode(gameplayPackets.poll(3, TimeUnit.SECONDS)).toString());
        peer.send("{\"type\":\"packet\",\"packet\":\"MapChanged\",\"payload\":{\"fileName\":\"1\"}}");
        assertNull(phase(GatewaySession.Phase.STARTING).world);
        peer.send(social.toString());
        assertEquals(social.toString(), WireJson.decode(gameplayPackets.poll(3, TimeUnit.SECONDS)).toString());
        assertNull(receipts.poll(100, TimeUnit.MILLISECONDS));
        session.disconnect("Fixture end"); phase(GatewaySession.Phase.DISCONNECTED);
        assertNull(gameplayPackets.poll(200, TimeUnit.MILLISECONDS));
    }

    @Test public void storageMetadataSurvivesOwnerMapTransitionWithoutDuplicatingTransferReceipts() throws Exception {
        connect(); roster(); session.start(7); commands.poll(3, TimeUnit.SECONDS);
        peer.send("{\"type\":\"packet\",\"packet\":\"StartGame\",\"payload\":{\"result\":4}}");
        peer.send("{\"type\":\"worldSnapshot\",\"payload\":{\"playerObjectId\":42,\"mapFileName\":\"0\",\"entities\":["
                + "{\"objectId\":42,\"kind\":\"selfPlayer\",\"name\":\"Fixture\",\"x\":302,\"y\":634}]}}");
        assertEquals("0", phase(GatewaySession.Phase.IN_GAME).world.mapFileName);
        JSONObject resize=GatewaySession.object("type","packet","packet","ResizeStorage","payload",
                GatewaySession.object("size",160,"hasExpandedStorage",true,"expiryTimeBinaryDatetime",9));
        peer.send(resize.toString());
        assertEquals(resize.toString(),new JSONObject(gameplayPackets.poll(3,TimeUnit.SECONDS)).toString());
        assertNull(receipts.poll(100,TimeUnit.MILLISECONDS));

        JSONObject transfer=GatewaySession.object("type","packet","packet","StoreItemV2","payload",
                GatewaySession.object("requestId","st-fixture","from",0,"to",159,"success",true));
        peer.send(transfer.toString());
        assertEquals(transfer.toString(),new JSONObject(receipts.poll(3,TimeUnit.SECONDS)).toString());
        assertNull("Do not duplicate the exact transfer channel",gameplayPackets.poll(200,TimeUnit.MILLISECONDS));

        peer.send("{\"type\":\"packet\",\"packet\":\"MapChanged\",\"payload\":{\"fileName\":\"1\"}}");
        assertNull(phase(GatewaySession.Phase.STARTING).world);
        peer.send(resize.toString());
        assertEquals(resize.toString(),new JSONObject(gameplayPackets.poll(3,TimeUnit.SECONDS)).toString());
        peer.send("{\"type\":\"worldSnapshot\",\"payload\":{\"playerObjectId\":42,\"mapFileName\":\"1\",\"entities\":["
                + "{\"objectId\":42,\"kind\":\"selfPlayer\",\"name\":\"Fixture\",\"x\":50,\"y\":60}]}}");
        assertEquals("1",phase(GatewaySession.Phase.IN_GAME).world.mapFileName);
    }

    @Test public void gameShopMetadataStagesOnlyDuringAuthenticatedSelectedStart() throws Exception {
        JSONObject catalog = GatewaySession.object("type", "packet", "packet", "GameShopInfo",
                "payload", GatewaySession.object("item", GatewaySession.object("gIndex", 31,
                        "itemName", "TLS fixture product", "stock", 10), "stockLevel", 8));
        JSONObject stock = GatewaySession.object("type", "packet", "packet", "GameShopStock",
                "payload", GatewaySession.object("gIndex", 31, "stockLevel", 3));
        connect();
        peer.send(catalog.toString()); peer.send(stock.toString());
        assertNull(gameplayPackets.poll(200, TimeUnit.MILLISECONDS));
        roster();
        peer.send(catalog.toString()); peer.send(stock.toString());
        assertNull(gameplayPackets.poll(200, TimeUnit.MILLISECONDS));
        session.start(7);
        assertEquals("startGame", commands.poll(3, TimeUnit.SECONDS).getString("type"));
        assertNull("Catalog metadata must not bootstrap a player",
                phase(GatewaySession.Phase.STARTING).world);
        for (JSONObject envelope : new JSONObject[]{catalog, stock}) {
            peer.send(envelope.toString());
            String raw = gameplayPackets.poll(3, TimeUnit.SECONDS);
            assertNotNull("Selected Start must retain packet-first GameShop metadata", raw);
            assertEquals(envelope.toString(), new JSONObject(raw).toString());
        }
        for (String unsupported : new String[]{"NPCStorage", "qa.giveItem", "GameShopPurchase"}) {
            peer.send(GatewaySession.object("type", "packet", "packet", unsupported,
                    "payload", new JSONObject()).toString());
            assertNull(gameplayPackets.poll(100, TimeUnit.MILLISECONDS));
        }
        peer.send("{\"type\":\"packet\",\"packet\":\"StartGame\",\"payload\":{\"result\":1}}");
        phase(GatewaySession.Phase.CHARACTERS);
        peer.send(catalog.toString()); peer.send(stock.toString());
        assertNull("Rejected Start must close catalog forwarding",
                gameplayPackets.poll(200, TimeUnit.MILLISECONDS));
        session.disconnect("TLS fixture ended");
        assertTrue(phase(GatewaySession.Phase.DISCONNECTED).characters.isEmpty());
    }

    @Test public void gameShopMetadataSurvivesAcceptedOwnerMapTransitionWithoutBootstrap() throws Exception {
        connect(); roster(); session.start(7); commands.poll(3, TimeUnit.SECONDS);
        peer.send("{\"type\":\"packet\",\"packet\":\"StartGame\",\"payload\":{\"result\":4}}");
        peer.send("{\"type\":\"worldSnapshot\",\"payload\":{\"playerObjectId\":42,\"mapFileName\":\"0\",\"entities\":["
                + "{\"objectId\":42,\"kind\":\"selfPlayer\",\"name\":\"Fixture\",\"x\":302,\"y\":634}]}}");
        assertEquals("0", phase(GatewaySession.Phase.IN_GAME).world.mapFileName);
        JSONObject catalog = GatewaySession.object("type", "packet", "packet", "GameShopInfo",
                "payload", GatewaySession.object("gIndex", 31, "itemName", "TLS fixture product"));
        peer.send(catalog.toString());
        assertEquals(catalog.toString(), new JSONObject(gameplayPackets.poll(3, TimeUnit.SECONDS)).toString());

        peer.send("{\"type\":\"packet\",\"packet\":\"MapChanged\",\"payload\":{\"fileName\":\"1\"}}");
        assertNull("Destination position still absent", phase(GatewaySession.Phase.STARTING).world);
        JSONObject stock = GatewaySession.object("type", "packet", "packet", "GameShopStock",
                "payload", GatewaySession.object("gIndex", 31, "stockLevel", 3));
        peer.send(stock.toString());
        assertEquals(stock.toString(), new JSONObject(gameplayPackets.poll(3, TimeUnit.SECONDS)).toString());
        peer.send("{\"type\":\"worldSnapshot\",\"payload\":{\"playerObjectId\":42,\"mapFileName\":\"1\",\"entities\":["
                + "{\"objectId\":42,\"kind\":\"selfPlayer\",\"name\":\"Fixture\",\"x\":50,\"y\":60}]}}");
        GatewaySession.View entered = phase(GatewaySession.Phase.IN_GAME);
        assertEquals("1", entered.world.mapFileName);
        assertEquals(50, entered.world.x);
        peer.send(catalog.toString());
        assertEquals(catalog.toString(), new JSONObject(gameplayPackets.poll(3, TimeUnit.SECONDS)).toString());
    }

    @Test public void oversizedGameShopMetadataFailsClosedBeforeOwnerBootstrap() throws Exception {
        connect(); roster(); session.start(7); commands.poll(3, TimeUnit.SECONDS);
        phase(GatewaySession.Phase.STARTING);
        peer.send(GatewaySession.object("type", "packet", "packet", "GameShopInfo", "payload",
                GatewaySession.object("item", GatewaySession.object("gIndex", 31,
                        "itemName", "文".repeat(6000)))).toString());
        phase(GatewaySession.Phase.DISCONNECTED);
        assertNull(gameplayPackets.poll(200, TimeUnit.MILLISECONDS));
    }

    @Test public void oversizedQuestMetadataFailsClosedBeforeOwnerBootstrap() throws Exception {
        connect(); roster(); session.start(7); commands.poll(3, TimeUnit.SECONDS);
        phase(GatewaySession.Phase.STARTING);
        peer.send(GatewaySession.object("type", "packet", "packet", "NewQuestInfo", "payload",
                GatewaySession.object("id", 1, "name", "文".repeat(6000))).toString());
        phase(GatewaySession.Phase.DISCONNECTED);
        assertNull(gameplayPackets.poll(200, TimeUnit.MILLISECONDS));
    }

    @Test public void oversizedNpcPacketFailsClosedAtTheJniBoundary() throws Exception {
        connect(); roster(); session.start(7); commands.poll(3, TimeUnit.SECONDS);
        peer.send("{\"type\":\"packet\",\"packet\":\"StartGame\",\"payload\":{\"result\":4}}");
        peer.send("{\"type\":\"worldSnapshot\",\"payload\":{\"playerObjectId\":42,\"mapFileName\":\"0\",\"entities\":["
                + "{\"objectId\":42,\"kind\":\"selfPlayer\",\"name\":\"Fixture\",\"x\":302,\"y\":634}]}}");
        phase(GatewaySession.Phase.IN_GAME);
        peer.send(GatewaySession.object("type", "packet", "packet", "NPCResponse", "payload",
                GatewaySession.object("page", new org.json.JSONArray().put("文".repeat(6000)))).toString());
        phase(GatewaySession.Phase.DISCONNECTED);
        assertNull(gameplayPackets.poll(200, TimeUnit.MILLISECONDS));
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
                "{\"type\":\"packet\",\"packet\":\"ObjectMana\",\"payload\":{\"objectId\":77,\"percent\":64}}",
                "{\"type\":\"packet\",\"packet\":\"ObjectName\",\"payload\":{\"objectId\":77,\"name\":\"Hen renamed\"}}",
                "{\"type\":\"packet\",\"packet\":\"ObjectColourChanged\",\"payload\":{\"objectId\":77,\"nameColourArgb\":-65281}}",
                "{\"type\":\"packet\",\"packet\":\"ObjectGuildNameChanged\",\"payload\":{\"objectId\":77,\"guildName\":\"AUTHORITATIVE\"}}",
                "{\"type\":\"packet\",\"packet\":\"ObjectPoisoned\",\"payload\":{\"objectId\":77,\"poison\":8}}",
                "{\"type\":\"packet\",\"packet\":\"ObjectLevelEffects\",\"payload\":{\"objectId\":77,\"levelEffects\":4}}",
                "{\"type\":\"packet\",\"packet\":\"ObjectHidden\",\"payload\":{\"objectId\":77,\"hidden\":true}}",
                "{\"type\":\"packet\",\"packet\":\"Pushed\",\"payload\":{\"location\":{\"x\":300,\"y\":629},\"direction\":\"Right\"}}",
                "{\"type\":\"packet\",\"packet\":\"ObjectPushed\",\"payload\":{\"objectId\":77,\"location\":{\"x\":300,\"y\":629},\"direction\":\"Right\"}}",
                "{\"type\":\"packet\",\"packet\":\"UserBackStep\",\"payload\":{\"location\":{\"x\":299,\"y\":629},\"direction\":\"Right\"}}",
                "{\"type\":\"packet\",\"packet\":\"UserDash\",\"payload\":{\"location\":{\"x\":301,\"y\":629},\"direction\":\"Right\"}}",
                "{\"type\":\"packet\",\"packet\":\"UserDashFail\",\"payload\":{\"location\":{\"x\":301,\"y\":629},\"direction\":\"Right\"}}",
                "{\"type\":\"packet\",\"packet\":\"ObjectDash\",\"payload\":{\"objectId\":77,\"location\":{\"x\":301,\"y\":629},\"direction\":\"Right\"}}",
                "{\"type\":\"packet\",\"packet\":\"ObjectDashFail\",\"payload\":{\"objectId\":77,\"location\":{\"x\":301,\"y\":629},\"direction\":\"Right\"}}",
                "{\"type\":\"packet\",\"packet\":\"UserDashAttack\",\"payload\":{\"location\":{\"x\":302,\"y\":629},\"direction\":\"Right\"}}",
                "{\"type\":\"packet\",\"packet\":\"UserAttackMove\",\"payload\":{\"location\":{\"x\":303,\"y\":629},\"direction\":\"Right\"}}",
                "{\"type\":\"packet\",\"packet\":\"ObjectRangeAttack\",\"payload\":{\"objectId\":77,\"location\":{\"x\":299,\"y\":629},\"direction\":\"Left\"}}",
                "{\"type\":\"packet\",\"packet\":\"ObjectDied\",\"payload\":{\"objectId\":77,\"location\":{\"x\":299,\"y\":629},\"direction\":\"Left\"}}",
                "{\"type\":\"packet\",\"packet\":\"ObjectRevived\",\"payload\":{\"objectId\":77,\"effect\":true}}",
                "{\"type\":\"packet\",\"packet\":\"Death\",\"payload\":{\"location\":{\"x\":302,\"y\":634},\"direction\":\"Down\"}}",
                "{\"type\":\"packet\",\"packet\":\"Revived\",\"payload\":{}}",
                "{\"type\":\"packet\",\"packet\":\"ObjectHide\",\"payload\":{\"objectId\":77}}",
                "{\"type\":\"packet\",\"packet\":\"ObjectShow\",\"payload\":{\"objectId\":77}}",
                "{\"type\":\"packet\",\"packet\":\"ObjectTeleportOut\",\"payload\":{\"objectId\":77,\"effectType\":1}}",
                "{\"type\":\"packet\",\"packet\":\"ObjectTeleportIn\",\"payload\":{\"objectId\":77,\"effectType\":1}}",
                "{\"type\":\"packet\",\"packet\":\"ObjectItem\",\"payload\":{\"objectId\":90,\"name\":\"Potion\",\"nameColourArgb\":-1,\"location\":{\"x\":302,\"y\":634},\"image\":0,\"grade\":0}}",
                "{\"type\":\"packet\",\"packet\":\"ObjectGold\",\"payload\":{\"objectId\":91,\"gold\":250,\"location\":{\"x\":302,\"y\":634}}}",
                "{\"type\":\"packet\",\"packet\":\"DamageIndicator\",\"payload\":{\"objectId\":77,\"damage\":12,\"damageType\":2}}",
                "{\"type\":\"packet\",\"packet\":\"ObjectMagic\",\"payload\":{\"objectId\":42,\"location\":{\"x\":302,\"y\":634},\"direction\":\"Down\",\"spell\":\"FireBall\",\"targetId\":77,\"target\":{\"x\":299,\"y\":629},\"cast\":true,\"level\":1,\"selfBroadcast\":false,\"secondaryTargetIds\":[]}}",
                "{\"type\":\"packet\",\"packet\":\"ObjectProjectile\",\"payload\":{\"spell\":\"FireBall\",\"sourceId\":42,\"destinationId\":77}}",
                "{\"type\":\"packet\",\"packet\":\"ObjectEffect\",\"payload\":{\"objectId\":77,\"effect\":3,\"effectType\":0,\"delayTime\":0,\"time\":0}}",
                "{\"type\":\"packet\",\"packet\":\"MapEffect\",\"payload\":{\"location\":{\"x\":302,\"y\":634},\"effect\":12,\"value\":0}}",
                "{\"type\":\"packet\",\"packet\":\"ObjectSpell\",\"payload\":{\"objectId\":501,\"location\":{\"x\":302,\"y\":634},\"spell\":39,\"direction\":\"Down\",\"param\":0}}"
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
