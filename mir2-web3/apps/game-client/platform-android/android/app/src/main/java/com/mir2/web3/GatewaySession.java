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
    private enum AccountOperation { NONE, CREATE, DELETE }
    private enum MailOperation { NONE, SEND, COLLECT, WAITING_REFRESH }

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
        final JSONObject accountEvent;
        View(Phase phase, String message, List<Character> characters, WorldPosition world,
                String worldSnapshot, JSONObject accountEvent) {
            this.phase = phase;
            this.message = message;
            this.characters = Collections.unmodifiableList(new ArrayList<>(characters));
            this.world = world;
            this.worldSnapshot = worldSnapshot;
            this.accountEvent = accountEvent;
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
    private final Consumer<String> gameplayObserver;
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
    private boolean hasOwnerSnapshot;
    private boolean hasSceneSnapshot;
    private long ownerObjectId;
    private String pendingSnapshot;
    private AccountOperation accountOperation = AccountOperation.NONE;
    private Integer pendingDeleteIndex;
    private JSONObject pendingAccountEvent;
    private MailOperation mailOperation = MailOperation.NONE;
    private String claimMailId;
    private long mailGeneration, mailOwnerId;
    private String mailOwnerName;
    private boolean closed;

    GatewaySession(OkHttpClient client, Consumer<View> observer) {
        this(client, observer, ignored -> {}, ignored -> {});
    }

    GatewaySession(OkHttpClient client, Consumer<View> observer, Consumer<String> receiptObserver) {
        this(client, observer, receiptObserver, ignored -> {});
    }

    GatewaySession(OkHttpClient client, Consumer<View> observer, Consumer<String> receiptObserver,
            Consumer<String> gameplayObserver) {
        this.client = client;
        this.observer = observer;
        this.receiptObserver = receiptObserver;
        this.gameplayObserver = gameplayObserver;
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
        if (phase != Phase.CHARACTERS || accountOperation != AccountOperation.NONE
                || characters.stream().noneMatch(c -> c.index == index)) return;
        resetWorld();
        phase = Phase.STARTING;
        armDeadline(generation);
        if (send(object("type", "startGame", "characterIndex", index))) publish("Entering world…");
    }

    synchronized void createCharacter(String name, String className, String genderName) {
        if (phase != Phase.CHARACTERS || accountOperation != AccountOperation.NONE) return;
        String trimmed = name.trim();
        int nameLength = trimmed.codePointCount(0, trimmed.length());
        if (nameLength < 1 || nameLength > 18
                || !(className.equals("Warrior") || className.equals("Wizard") || className.equals("Taoist"))
                || !(genderName.equals("Male") || genderName.equals("Female"))) {
            publish("Enter a valid character name, class and gender");
            return;
        }
        accountOperation = AccountOperation.CREATE;
        armDeadline(generation);
        if (send(object("type", "newCharacter", "name", trimmed,
                "class", className, "gender", genderName))) {
            publish("Creating character…");
        }
    }

    synchronized void deleteCharacter(int index) {
        if (phase != Phase.CHARACTERS || accountOperation != AccountOperation.NONE
                || characters.stream().noneMatch(c -> c.index == index)) return;
        accountOperation = AccountOperation.DELETE;
        pendingDeleteIndex = index;
        armDeadline(generation);
        if (send(object("type", "deleteCharacter", "characterIndex", index))) {
            publish("Deleting character…");
        }
    }

    /** Write one Rust-produced gameplay BrowserCommand on this authenticated session. */
    synchronized boolean sendAuthenticated(JSONObject command) {
        if (phase != Phase.IN_GAME || socket == null) return false;
        String type = command.optString("type", "");
        if (type.isEmpty() || type.length() > 64
                || type.equals("clientVersion") || type.equals("keepAlive")
                || type.equals("login") || type.equals("newAccount")
                || type.equals("newCharacter") || type.equals("deleteCharacter")
                || type.equals("startGame") || type.equals("passkeyLogin")) {
            return false;
        }
        boolean claim = type.equals("collectParcel"), sendMail = type.equals("sendMail");
        String claimId = null;
        if (claim || sendMail) {
            if (!startAccepted || !hasOwnerSnapshot || mailOperation != MailOperation.NONE) return false;
            if (claim) {
                // Android org.json rounds unsigned values beyond Long.MAX_VALUE
                // into doubles. Fail closed instead of correlating a rounded ID.
                Object raw = command.opt("mailId");
                if (!(raw instanceof Integer) && !(raw instanceof Long)) return false;
                if (((Number) raw).longValue() <= 0) return false;
                claimId = raw.toString();
            }
        }
        boolean written = socket.send(command.toString());
        if (written && (claim || sendMail)) {
            // Socket callbacks use this same monitor. Record only the actual
            // accepted write, not the Rust FIFO or a guessed payload ID.
            mailOperation = claim ? MailOperation.COLLECT : MailOperation.SEND;
            claimMailId = claimId;
            mailGeneration = generation;
            mailOwnerId = ownerObjectId;
            mailOwnerName = player;
        }
        return written;
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
            long owner = objectId(world, "playerObjectId");
            for (int i = 0; i < entities.length(); i++) {
                JSONObject entity = entities.getJSONObject(i);
                if ("selfPlayer".equals(entity.optString("kind"))
                        && owner == objectId(entity, "objectId")) {
                    player = bounded(entity.getString("name"));
                    map = bounded(world.getString("mapFileName"));
                    readPosition(entity);
                    // Keep the complete immutable server payload until StartGame is accepted.
                    // Host-only receipt epoch; server input cannot select it.
                    JSONObject hostWorld = new JSONObject(world.toString());
                    hostWorld.put("androidMailGeneration", String.valueOf(generation));
                    pendingSnapshot = hostWorld.toString();
                    hasOwnerSnapshot = true;
                    hasSceneSnapshot = true;
                    ownerObjectId = owner;
                    publishWorld();
                    if (hasMailRefresh(world) && mailOwnerMatches()) clearMailFeedbackWait();
                    return;
                }
            }
            return;
        }
        if (!type.equals("packet")) return;
        String packet = envelope.getString("packet");
        if (packet.equals("MailSent") || packet.equals("ParcelCollected")) {
            forwardOwnMailResult(packet, envelope.optJSONObject("payload"));
            return; // Never forward anonymous server ACKs or raw payload fields.
        }
        boolean forwardEntity = phase == Phase.IN_GAME && isEntityGameplayPacket(packet);
        boolean forwardPersonal = personalGameplayPhase()
                && (isPersonalSkillPacket(packet) || isInventoryOperationPacket(packet)
                        || isPersonalPlayerPacket(packet) || isReceivedChatPacket(packet));
        boolean forwardNpc = npcGameplayPhase() && isNpcServicePacket(packet);
        // Only public metadata for an authenticated, listed-character Start.
        // Rust stages it until the accepted owner snapshot; it is not bootstrap.
        boolean forwardQuestMetadata = worldPending() && isQuestMetadataPacket(packet);
        boolean forwardGameShopMetadata = worldPending() && isGameShopMetadataPacket(packet);
        boolean forwardStorageMetadata = worldPending() && isStorageMetadataPacket(packet);
        boolean forwardMailMetadata = worldPending() && isMailMetadataPacket(packet);
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
            case "NewCharacterSuccess":
                if (phase != Phase.CHARACTERS || accountOperation != AccountOperation.CREATE) return;
                Character created = parseCharacter(payload.getJSONObject("character"));
                if (characters.stream().anyMatch(c -> c.index == created.index)) {
                    throw new IllegalArgumentException("duplicate character index");
                }
                characters = new ArrayList<>(characters);
                characters.add(0, created);
                accountOperation = AccountOperation.NONE;
                cancelDeadline();
                pendingAccountEvent = object("type", "characterCreated",
                        "character", characterJson(created));
                publish("Character created. Select a character.");
                break;
            case "NewCharacter":
                if (phase == Phase.CHARACTERS && accountOperation == AccountOperation.CREATE) {
                    failAccountOperation("Character creation was rejected.");
                }
                break;
            case "DeleteCharacterSuccess":
                if (phase != Phase.CHARACTERS || accountOperation != AccountOperation.DELETE) return;
                int deletedIndex = integer(payload, "characterIndex");
                if (pendingDeleteIndex == null || pendingDeleteIndex != deletedIndex
                        || characters.stream().noneMatch(c -> c.index == deletedIndex)) {
                    throw new IllegalArgumentException("delete character index");
                }
                characters = new ArrayList<>(characters);
                characters.removeIf(c -> c.index == deletedIndex);
                accountOperation = AccountOperation.NONE;
                pendingDeleteIndex = null;
                cancelDeadline();
                pendingAccountEvent = object("type", "characterDeleted", "characterIndex", deletedIndex);
                publish("Character deleted.");
                break;
            case "DeleteCharacter":
                if (phase == Phase.CHARACTERS && accountOperation == AccountOperation.DELETE) {
                    failAccountOperation("Character deletion was rejected.");
                }
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
                // Reject other actors before changing Java's published coordinates;
                // a later Rust rejection cannot undo an already published View.
                if (!worldPending() || !isOwnerInformation(payload)) return;
                pendingSnapshot = null;
                player = bounded(payload.getString("name"));
                JSONObject location = payload.optJSONObject("location");
                if (location != null) readPosition(location);
                else if (payload.has("x") && payload.has("y")) readPosition(payload);
                publishWorld();
                break;
            case "MapInformation": case "MapChanged":
                if (!worldPending()) return;
                hasSceneSnapshot = false;
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
        if ((forwardEntity && phase == Phase.IN_GAME) || (forwardPersonal && personalGameplayPhase())
                || (forwardNpc && npcGameplayPhase())
                || (forwardQuestMetadata && worldPending())
                || (forwardGameShopMetadata && worldPending())
                || (forwardStorageMetadata && worldPending())
                || (forwardMailMetadata && worldPending())) {
            forwardBounded(envelope, gameplayObserver);
            if (forwardMailMetadata && packet.equals("ReceiveMail")
                    && payload.optJSONArray("mail") != null && mailOwnerMatches()
                    && mailPayloadOwnerMatches(payload)) {
                clearMailFeedbackWait();
            }
        }
    }

    private void forwardOwnMailResult(String packet, JSONObject payload) {
        if (!personalGameplayPhase() || payload == null || !mailOwnerMatches()) return;
        if (!(packet.equals("MailSent") && mailOperation == MailOperation.SEND)
                && !(packet.equals("ParcelCollected") && mailOperation == MailOperation.COLLECT)) return;
        JSONObject body = payload.optJSONObject("data");
        if (body == null) body = payload;
        if (!mailPayloadOwnerMatches(payload) || !mailPayloadOwnerMatches(body)) return;
        Object raw = body.opt("result");
        int result;
        if (raw instanceof Integer || raw instanceof Long) {
            long value = ((Number)raw).longValue();
            if (value != 1 && value != -1) return;
            result = (int)value;
        } else if (raw instanceof String) {
            // Match the frozen shared value_i32 numeric-string path as well.
            try { result = Integer.parseInt((String)raw); }
            catch (NumberFormatException invalid) { return; }
            if (result != 1 && result != -1) return;
        } else return;
        JSONObject own = object("type", "androidMailResult", "packet", packet,
                "result", result, "connectionGeneration", String.valueOf(mailGeneration),
                "ownerObjectId", mailOwnerId, "characterName", mailOwnerName,
                "claimMailId", claimMailId == null ? JSONObject.NULL : claimMailId);
        mailOperation = MailOperation.WAITING_REFRESH;
        forwardBounded(own, gameplayObserver);
    }

    private boolean mailOwnerMatches() {
        return mailOperation != MailOperation.NONE && mailGeneration == generation
                && mailOwnerId == ownerObjectId && player.equals(mailOwnerName);
    }

    private boolean mailPayloadOwnerMatches(JSONObject payload) {
        if (payload.has("hero") && !Boolean.FALSE.equals(payload.opt("hero"))) return false;
        if (payload.has("characterName") && !player.equals(payload.opt("characterName"))) return false;
        if (payload.has("ownerObjectId")) {
            Object raw = payload.opt("ownerObjectId");
            if (!(raw instanceof Integer) && !(raw instanceof Long)) return false;
            if (((Number)raw).longValue() != ownerObjectId) return false;
        }
        return true;
    }

    private void clearMailFeedbackWait() {
        if (mailOperation == MailOperation.WAITING_REFRESH) resetMailOperation();
    }

    private void resetMailOperation() {
        mailOperation = MailOperation.NONE; claimMailId = null;
        mailGeneration = mailOwnerId = 0; mailOwnerName = null;
    }

    private static boolean hasMailRefresh(JSONObject world) {
        // Same source precedence as the shared mail_source projection.
        for (String nested : new String[]{"stage5Systems", "stage5_systems"}) {
            JSONObject value = world.optJSONObject(nested);
            if (value != null && value.has("mail")) return value.optJSONArray("mail") != null;
        }
        for (String key : new String[]{"mails", "mail"}) {
            if (world.has(key)) return world.optJSONArray(key) != null;
        }
        JSONObject value = world.optJSONObject("stage5");
        return value != null && value.optJSONArray("mail") != null;
    }

    private void forwardReceipt(JSONObject envelope) {
        forwardBounded(envelope, receiptObserver);
    }

    private static void forwardBounded(JSONObject envelope, Consumer<String> target) {
        String raw = envelope.toString();
        // A full shared mailbox has up to 256 rows. Only its read-only packet
        // gets the larger bound; services and every other packet retain 16 KiB.
        int limit = "packet".equals(envelope.optString("type"))
                && "ReceiveMail".equals(envelope.optString("packet")) ? 512 * 1024 : 16 * 1024;
        if (raw.getBytes(StandardCharsets.UTF_8).length > limit) {
            throw new IllegalArgumentException("inbound packet size limit");
        }
        target.accept(raw);
    }

    private static boolean isPersonalSkillPacket(String packet) {
        // Public owner packets only. Rust shares the Windows identity/patch
        // adapter; forwarding is not acceptance of a cast or client authority.
        return packet.equals("Magic") || packet.equals("MagicCast")
                || packet.equals("MagicDelay") || packet.equals("SpellToggle")
                || packet.equals("NewMagic") || packet.equals("MagicLeveled")
                || packet.equals("RemoveMagic") || packet.equals("UserInformation");
    }

    private static boolean isInventoryOperationPacket(String packet) {
        // Exact frozen Windows route, not arbitrary transaction/admin packets.
        // DeleteItem's helper exists upstream but its production route does not.
        return packet.equals("DropItem") || packet.equals("MoveItem")
                || packet.equals("MergeItem") || packet.equals("SplitItem1")
                || packet.equals("SellItem") || packet.equals("EquipItem")
                || packet.equals("RemoveItem");
    }

    private static boolean isPersonalPlayerPacket(String packet) {
        // Public received deltas only. Owner/bootstrap authentication is unchanged;
        // Rust refuses a delta without this character's authoritative wallet base.
        return packet.equals("GainedGold") || packet.equals("LoseGold")
                || packet.equals("GainedCredit") || packet.equals("LoseCredit");
    }

    private static boolean isReceivedChatPacket(String packet) {
        // Public received chat, after this connection's accepted owner bootstrap.
        // ObjectChat is a peer/AOI message, not a personal-owner stat packet.
        return packet.equals("Chat") || packet.equals("ObjectChat");
    }

    private static boolean isNpcServicePacket(String packet) {
        // Public Windows NPC surface packets, not arbitrary service/admin JSON.
        return packet.equals("NPCResponse") || packet.equals("NPCGoods")
                || packet.equals("NPCPearlGoods") || packet.equals("NPCSell")
                || packet.equals("NPCRepair") || packet.equals("NPCSRepair");
    }

    private static boolean isQuestMetadataPacket(String packet) {
        return packet.equals("NewQuestInfo") || packet.equals("CompleteQuest");
    }

    private static boolean isGameShopMetadataPacket(String packet) {
        // Public catalogue/stock only, during this authenticated listed Start.
        // Rust stages before owner acceptance; no purchase/currency/receipt grant.
        return packet.equals("GameShopInfo") || packet.equals("GameShopStock");
    }

    private boolean npcGameplayPhase() {
        // Unlike personal item receipts, these replies belong to the current
        // scene. Position alone after MapChanged cannot authorize old services.
        return phase == Phase.IN_GAME && startAccepted && hasOwnerSnapshot && hasSceneSnapshot;
    }

    private static boolean isStorageMetadataPacket(String packet) {
        // Public read-only metadata, not a second transfer-receipt channel.
        return packet.equals("UserStorage") || packet.equals("StorageUnlockResult")
                || packet.equals("StoragePasswordResult") || packet.equals("ResizeStorage");
    }

    private static boolean isMailMetadataPacket(String packet) {
        // Read-only lists/server quotes/locks. Mail results have a separate
        // own-write/epoch path; anonymous ACKs never enter this whitelist.
        return packet.equals("ReceiveMail") || packet.equals("MailSendRequest")
                || packet.equals("MailCost") || packet.equals("MailLockedItem");
    }

    private static boolean isEntityGameplayPacket(String packet) {
        return packet.equals("UserLocation") || packet.equals("ObjectWalk")
                || packet.equals("ObjectRun") || packet.equals("UserBackStep")
                || packet.equals("ObjectBackStep") || packet.equals("Pushed")
                || packet.equals("ObjectPushed")
                || packet.equals("UserDash") || packet.equals("UserDashFail")
                || packet.equals("ObjectDash") || packet.equals("ObjectDashFail")
                || packet.equals("UserDashAttack") || packet.equals("UserAttackMove")
                || packet.equals("ObjectTurn") || packet.equals("ObjectHarvest")
                || packet.equals("ObjectHarvested") || packet.equals("ObjectAttack")
                || packet.equals("ObjectRangeAttack") || packet.equals("ObjectStruck")
                || packet.equals("ObjectDashAttack")
                || packet.equals("DamageIndicator")
                || packet.equals("ObjectMagic") || packet.equals("ObjectProjectile")
                || packet.equals("ObjectEffect") || packet.equals("MapEffect")
                || packet.equals("ObjectSpell")
                || packet.equals("ObjectHealth") || packet.equals("ObjectMana")
                || packet.equals("Death")
                || packet.equals("ObjectName") || packet.equals("ObjectColourChanged")
                || packet.equals("ObjectGuildNameChanged")
                || packet.equals("ObjectPoisoned")
                || packet.equals("ObjectLevelEffects")
                || packet.equals("ObjectHidden")
                || packet.equals("ObjectDied") || packet.equals("Revived")
                || packet.equals("ObjectRevived") || packet.equals("ObjectHide")
                || packet.equals("ObjectShow") || packet.equals("ObjectRemove")
                || packet.equals("ObjectTeleportOut") || packet.equals("ObjectTeleportIn")
                || packet.equals("ObjectItem") || packet.equals("ObjectGold")
                || packet.equals("ObjectPlayer") || packet.equals("ObjectHero")
                || packet.equals("ObjectMonster") || packet.equals("NewMonsterInfo")
                || packet.equals("ObjectNpc") || packet.equals("NewNpcInfo");
    }

    private boolean worldPending() { return phase == Phase.STARTING || phase == Phase.IN_GAME; }
    private boolean personalGameplayPhase() {
        // Owner bootstrap must be accepted within this connection/character.
        // Personal receipts survive map loading; entity packets do not.
        return worldPending() && startAccepted && hasOwnerSnapshot;
    }
    /** True while authentication has a position but the native render barrier still lacks a scene. */
    synchronized boolean awaitingRenderSnapshot() {
        return phase == Phase.IN_GAME && startAccepted && pendingSnapshot == null
                && deadline != null && !deadline.isDone();
    }
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
        // Position packets can precede the authoritative world snapshot. They
        // may publish the bootstrap coordinates, but cannot cancel the
        // StartGame/map-transition deadline: without a snapshot Rust has no
        // map/entity request and can never produce NativeRenderReady. Keep the
        // deadline armed until this publication actually carries that frame.
        if (pendingSnapshot != null) cancelDeadline();
        publish("Character: " + player + "\nMap: " + map + "\nServer position: (" + x + ", " + y + ")");
    }
    private void resetWorld() {
        resetMailOperation();
        player = map = ""; x = y = null; startAccepted = false; pendingSnapshot = null;
        hasOwnerSnapshot = false;
        hasSceneSnapshot = false;
        ownerObjectId = 0;
    }
    private boolean isOwnerInformation(JSONObject payload) {
        if (payload.has("hero") && !Boolean.FALSE.equals(payload.opt("hero"))) return false;
        // The legacy position bootstrap can precede the complete owner snapshot.
        // It cannot authorize personal gameplay packets: that still requires
        // hasOwnerSnapshot, which only a validated worldSnapshot can establish.
        if (!hasOwnerSnapshot) return true;
        Object rawId = payload.opt("objectId");
        // JSON integer tokens are Integer/Long on Android. Floating-point or
        // decimal tokens can round a fractional ID to an existing owner.
        if (!(rawId instanceof Integer) && !(rawId instanceof Long)) return false;
        return ((Number) rawId).longValue() == ownerObjectId
                && player.equals(payload.opt("name"));
    }
    private void resetAccountOperation() {
        accountOperation = AccountOperation.NONE;
        pendingDeleteIndex = null;
        pendingAccountEvent = null;
    }
    private void failAccountOperation(String message) {
        accountOperation = AccountOperation.NONE;
        pendingDeleteIndex = null;
        cancelDeadline();
        pendingAccountEvent = object("type", "operationFailure", "message", message);
        publish(message);
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
        resetAccountOperation();
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
        JSONObject accountEvent = pendingAccountEvent;
        pendingAccountEvent = null;
        observer.accept(new View(phase, text, characters, world, snapshot, accountEvent));
    }
    private static Character parseCharacter(JSONObject row) throws JSONException {
        int index = integer(row, "index");
        if (index < 0) throw new IllegalArgumentException("character index");
        return new Character(index, bounded(row.getString("name")), row.optInt("level", 0),
                row.optString("class", "Unknown"), row.optString("gender", "Unknown"));
    }
    private static JSONObject characterJson(Character character) {
        return object("index", character.index, "name", character.name, "level", character.level,
                "className", character.className, "genderName", character.genderName);
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
    private static long objectId(JSONObject value, String key) throws JSONException {
        Object raw = value.get(key);
        if (!(raw instanceof Integer) && !(raw instanceof Long)) {
            throw new IllegalArgumentException("integer object ID required");
        }
        Number number = (Number) raw;
        long id = number.longValue();
        if (id <= 0 || id > 0xFFFFFFFFL) {
            throw new IllegalArgumentException("owner ID");
        }
        return id;
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
