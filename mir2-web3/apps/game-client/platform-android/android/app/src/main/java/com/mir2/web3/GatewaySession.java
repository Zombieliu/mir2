package com.mir2.web3;

import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.concurrent.Executors;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.ScheduledFuture;
import java.util.concurrent.TimeUnit;
import java.util.function.Consumer;
import okhttp3.HttpUrl;
import okhttp3.OkHttpClient;
import okhttp3.Request;
import okhttp3.Response;
import okhttp3.WebSocket;
import okhttp3.WebSocketListener;
import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

/** Android transport for the existing BrowserCommand protocol. No gameplay rules. */
final class GatewaySession implements AutoCloseable {
    enum Phase { DISCONNECTED, CONNECTING, READY, LOGIN, CHARACTERS, STARTING, IN_GAME }

    static final class Character {
        final int index;
        final String name;
        final int level;
        final String className, genderName;
        Character(int index, String name, int level, String className, String genderName) {
            this.index = index; this.name = name; this.level = level;
            this.className = className; this.genderName = genderName;
        }
        @Override public String toString() { return name + " (#" + index + ")"; }
    }

    static final class View {
        final Phase phase;
        final String message;
        final List<Character> characters;
        final WorldPosition world;
        final String worldSnapshot;
        View(Phase phase, String message, List<Character> characters, WorldPosition world, String worldSnapshot) {
            this.phase = phase;
            this.message = message;
            this.characters = Collections.unmodifiableList(new ArrayList<>(characters));
            this.world = world;
            this.worldSnapshot = worldSnapshot;
        }
    }

    /** Immutable authenticated server projection, not a client-authored transform. */
    static final class WorldPosition {
        final String playerName, mapFileName;
        final int x, y;
        WorldPosition(String playerName, String mapFileName, int x, int y) {
            this.playerName = playerName; this.mapFileName = mapFileName;
            this.x = x; this.y = y;
        }
        JSONObject toJson() {
            return object("playerName", playerName, "mapFileName", mapFileName, "x", x, "y", y);
        }
    }

    private final OkHttpClient client;
    private final Consumer<View> observer;
    private final Consumer<String> receiptObserver;
    private final ScheduledExecutorService timer = Executors.newSingleThreadScheduledExecutor();
    private ScheduledFuture<?> deadline;
    private ScheduledFuture<?> heartbeat;
    private WebSocket socket;
    private long generation;
    private long deadlineEpoch;
    private Phase phase = Phase.DISCONNECTED;
    private List<Character> characters = new ArrayList<>();
    private String player = "", map = "";
    private Integer x, y;
    private boolean startAccepted;
    private String pendingSnapshot;
    private boolean closed;

    GatewaySession(OkHttpClient client, Consumer<View> observer) {
        this(client, observer, ignored -> {});
    }

    GatewaySession(OkHttpClient client, Consumer<View> observer, Consumer<String> receiptObserver) {
        this.client = client;
        this.observer = observer;
        this.receiptObserver = receiptObserver;
    }

    static HttpUrl endpoint(String input) {
        if (!input.startsWith("wss://")) throw new IllegalArgumentException("Use a WSS test endpoint");
        HttpUrl url = HttpUrl.parse("https://" + input.substring(6));
        if (url == null || !url.username().isEmpty() || !url.password().isEmpty()
                || url.query() != null || url.fragment() != null) {
            throw new IllegalArgumentException("Endpoint must not contain credentials, query or fragment");
        }
        return url;
    }

    synchronized void connect(String address) {
        if (closed) return;
        HttpUrl url = endpoint(address.trim());
        disconnect("Connecting");
        phase = Phase.CONNECTING;
        final long attempt = generation;
        publish("Connecting securely…");
        armDeadline(attempt);
        socket = client.newWebSocket(new Request.Builder().url(url).build(), new WebSocketListener() {
            @Override public void onOpen(WebSocket ws, Response response) {
                synchronized (GatewaySession.this) {
                    if (attempt != generation) { ws.cancel(); return; }
                    // The gateway validates the normal version packet before account operations.
                    if (!send(object("type", "clientVersion"))) return;
                    heartbeat = timer.scheduleAtFixedRate(() -> {
                        synchronized (GatewaySession.this) {
                            if (attempt == generation && phase != Phase.DISCONNECTED) {
                                send(object("type", "keepAlive", "time", System.currentTimeMillis()));
                            }
                        }
                    }, 5, 5, TimeUnit.SECONDS);
                }
            }
            @Override public void onMessage(WebSocket ws, String text) {
                synchronized (GatewaySession.this) {
                    if (attempt != generation) return;
                    if (text.length() > 1024 * 1024
                            || text.getBytes(StandardCharsets.UTF_8).length > 1024 * 1024) {
                        disconnect("Gateway message too large"); return;
                    }
                    try { receive(new JSONObject(text)); }
                    catch (JSONException | IllegalArgumentException error) {
                        disconnect("Invalid gateway response; reconnect and log in again");
                    }
                }
            }
            @Override public void onClosing(WebSocket ws, int code, String reason) {
                synchronized (GatewaySession.this) {
                    if (attempt == generation) disconnect("Connection closed; log in again");
                }
            }
            @Override public void onFailure(WebSocket ws, Throwable error, Response response) {
                synchronized (GatewaySession.this) {
                    // Never expose URLs, credentials or raw server errors in UI/logs.
                    if (attempt == generation) disconnect("Connection failed; check endpoint/network and retry");
                }
            }
        });
    }

    synchronized void login(String account, String password) {
        if (phase != Phase.READY) return;
        if (account.isBlank() || account.length() > 32 || password.isEmpty() || password.length() > 128) {
            publish("Enter a valid account and password");
            return;
        }
        phase = Phase.LOGIN;
        armDeadline(generation);
        if (send(object("type", "login", "accountId", account, "password", password))) {
            publish("Authenticating…");
        }
        // Password is never stored in a field, preferences, savedInstanceState or logs.
    }

    synchronized void start(int index) {
        if (phase != Phase.CHARACTERS || characters.stream().noneMatch(c -> c.index == index)) return;
        resetWorld();
        phase = Phase.STARTING;
        armDeadline(generation);
        if (send(object("type", "startGame", "characterIndex", index))) publish("Entering world…");
    }

    /** Write one Rust-produced gameplay BrowserCommand on this authenticated session. */
    synchronized boolean sendAuthenticated(JSONObject command) {
        if (phase != Phase.IN_GAME || socket == null) return false;
        String type = command.optString("type", "");
        if (type.isEmpty() || type.length() > 64
                || type.equals("clientVersion") || type.equals("keepAlive")
                || type.equals("login") || type.equals("newAccount")
                || type.equals("startGame") || type.equals("passkeyLogin")) {
            return false;
        }
        return socket.send(command.toString());
    }

    private void receive(JSONObject envelope) throws JSONException {
        String type = envelope.getString("type");
        if (type.equals("error")) { disconnect("Gateway rejected request; reconnect and log in again"); return; }
        if (type.equals("gameShopReceipt")) {
            forwardReceipt(envelope);
            return;
        }
        if (type.equals("worldSnapshot")) {
            if (!worldPending()) return;
            JSONObject world = envelope.getJSONObject("payload");
            JSONArray entities = world.getJSONArray("entities");
            if (entities.length() > 8192) throw new IllegalArgumentException("entity limit");
            // Match the server's self entity and object ID, never the first visible actor.
            long owner = world.getLong("playerObjectId");
            if (owner <= 0 || owner > 0xFFFFFFFFL) throw new IllegalArgumentException("owner ID");
            for (int i = 0; i < entities.length(); i++) {
                JSONObject entity = entities.getJSONObject(i);
                if ("selfPlayer".equals(entity.optString("kind"))
                        && owner == entity.getLong("objectId")) {
                    player = bounded(entity.getString("name"));
                    map = bounded(world.getString("mapFileName"));
                    readPosition(entity);
                    // Keep the complete immutable server payload until StartGame is accepted.
                    pendingSnapshot = world.toString();
                    publishWorld();
                    return;
                }
            }
            return;
        }
        if (!type.equals("packet")) return;
        String packet = envelope.getString("packet");
        if (packet.equals("StoreItemV2") || packet.equals("TakeBackItemV2")
                || packet.equals("ChangePassword") || packet.equals("ChangePasswordBanned")) {
            forwardReceipt(envelope);
        }
        JSONObject payload = envelope.optJSONObject("payload");
        if (payload == null) payload = new JSONObject();
        switch (packet) {
            case "Connected":
                if (phase == Phase.CONNECTING) {
                    phase = Phase.READY;
                    cancelDeadline();
                    publish("Connected. Enter your test account.");
                }
                break;
            case "LoginSuccess":
                if (phase != Phase.LOGIN) return;
                JSONArray roster = payload.getJSONArray("characters");
                if (roster.length() > 64) throw new IllegalArgumentException("roster limit");
                List<Character> next = new ArrayList<>();
                for (int i = 0; i < roster.length(); i++) {
                    JSONObject row = roster.getJSONObject(i);
                    int index = integer(row, "index");
                    if (index < 0 || next.stream().anyMatch(c -> c.index == index)) {
                        throw new IllegalArgumentException("character index");
                    }
                    next.add(new Character(index, bounded(row.getString("name")), row.optInt("level", 0),
                            row.optString("class", "Unknown"), row.optString("gender", "Unknown")));
                }
                characters = next;
                phase = Phase.CHARACTERS;
                cancelDeadline();
                publish(next.isEmpty() ? "Login accepted. No characters on this account."
                        : "Login accepted. Select a character.");
                break;
            case "Login": case "LoginBanned":
                if (phase == Phase.LOGIN) {
                    phase = Phase.READY;
                    cancelDeadline();
                    publish("Login rejected. Check credentials/account status.");
                }
                break;
            case "StartGame":
                if (phase != Phase.STARTING) return;
                if (integer(payload, "result") != 4) {
                    resetWorld();
                    phase = Phase.CHARACTERS;
                    cancelDeadline();
                    publish("StartGame rejected. Select a character or reconnect.");
                    return;
                }
                startAccepted = true;
                publishWorld();
                break;
            case "UserInformation":
                if (!worldPending()) return;
                pendingSnapshot = null;
                player = bounded(payload.getString("name"));
                JSONObject location = payload.optJSONObject("location");
                if (location != null) readPosition(location);
                else if (payload.has("x") && payload.has("y")) readPosition(payload);
                publishWorld();
                break;
            case "MapInformation": case "MapChanged":
                if (!worldPending()) return;
                pendingSnapshot = null;
                if (phase == Phase.IN_GAME) {
                    phase = Phase.STARTING;
                    armDeadline(generation);
                }
                map = payload.has("fileName") ? bounded(payload.getString("fileName"))
                        : payload.has("mapFileName") ? bounded(payload.getString("mapFileName")) : "";
                x = y = null; // Never combine destination map with old-map position.
                publishWorld();
                break;
            case "UserLocation":
                if (!worldPending()) return;
                pendingSnapshot = null;
                readPosition(payload);
                publishWorld();
                break;
            case "Disconnect": case "ReturnToLogin": case "LogOutSuccess":
                disconnect("Session ended. Log in again.");
                break;
            default: break;
        }
    }

    private void forwardReceipt(JSONObject envelope) {
        String raw = envelope.toString();
        if (raw.getBytes(StandardCharsets.UTF_8).length > 16 * 1024) {
            throw new IllegalArgumentException("receipt size limit");
        }
        receiptObserver.accept(raw);
    }

    private boolean worldPending() { return phase == Phase.STARTING || phase == Phase.IN_GAME; }
    private void readPosition(JSONObject value) throws JSONException {
        x = integer(value, "x"); y = integer(value, "y");
        if (x < 0 || y < 0) throw new IllegalArgumentException("negative position");
    }
    private void publishWorld() {
        if (!startAccepted || player.isEmpty() || map.isEmpty() || x == null || y == null) {
            publish("Waiting for authoritative character, map and position…");
            return;
        }
        phase = Phase.IN_GAME;
        cancelDeadline();
        publish("Character: " + player + "\nMap: " + map + "\nServer position: (" + x + ", " + y + ")");
    }
    private void resetWorld() {
        player = map = ""; x = y = null; startAccepted = false; pendingSnapshot = null;
    }
    synchronized void disconnect(String reason) {
        generation++;
        cancelDeadline();
        if (heartbeat != null) { heartbeat.cancel(false); heartbeat = null; }
        WebSocket old = socket;
        socket = null;
        phase = Phase.DISCONNECTED;
        characters.clear();
        resetWorld();
        if (old != null) old.cancel();
        publish(reason);
    }
    private boolean send(JSONObject command) {
        if (socket != null && socket.send(command.toString())) return true;
        disconnect("Send failed; reconnect and log in again");
        return false;
    }
    private void armDeadline(long attempt) {
        cancelDeadline();
        final long operation = deadlineEpoch;
        deadline = timer.schedule(() -> {
            synchronized (GatewaySession.this) {
                if (attempt == generation && operation == deadlineEpoch) {
                    disconnect("Request timed out; reconnect and log in again");
                }
            }
        }, 20, TimeUnit.SECONDS);
    }
    private void cancelDeadline() {
        deadlineEpoch++;
        if (deadline != null) { deadline.cancel(false); deadline = null; }
    }
    private void publish(String text) {
        WorldPosition world = phase == Phase.IN_GAME && startAccepted
                && !player.isEmpty() && !map.isEmpty() && x != null && y != null
                ? new WorldPosition(player, map, x, y) : null;
        String snapshot = world == null ? null : pendingSnapshot;
        if (snapshot != null) pendingSnapshot = null; // Deliver once, never replay on a later packet.
        observer.accept(new View(phase, text, characters, world, snapshot));
    }
    private static String bounded(String value) {
        if (value.isBlank() || value.length() > 128) throw new IllegalArgumentException("text limit");
        return value;
    }
    private static int integer(JSONObject value, String key) throws JSONException {
        Object raw = value.get(key);
        if (!(raw instanceof Number) || ((Number) raw).doubleValue() != ((Number) raw).intValue()) {
            throw new IllegalArgumentException("integer required");
        }
        return ((Number) raw).intValue();
    }
    static JSONObject object(Object... pairs) {
        JSONObject value = new JSONObject();
        try { for (int i = 0; i < pairs.length; i += 2) value.put((String) pairs[i], pairs[i + 1]); }
        catch (JSONException error) { throw new IllegalArgumentException("Invalid command", error); }
        return value;
    }
    @Override public synchronized void close() {
        closed = true;
        disconnect("Closed");
        timer.shutdownNow();
    }
}
