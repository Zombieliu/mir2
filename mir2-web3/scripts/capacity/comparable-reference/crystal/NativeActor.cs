using System.Collections.Concurrent;
using System.Net.Sockets;
using System.Security.Cryptography;
using System.Text.Json;
using C = ClientPackets;
using S = ServerPackets;

namespace ComparableCrystal;

internal sealed record Receipt(int ActorOrdinal, int Connection, long Sequence, double At, Packet Packet);
internal sealed record EquipmentAck(long SentSequence, long ReceiveSequence, double SentAtMs, double ReceivedAtMs, double DeadlineAtMs, double LatencyMs, bool Success, string AckDerivedExpectedFingerprint);
internal sealed record PotionReservation(ulong Uid, int Count, int Stock, int HpAtDecision, object Before);
internal sealed class ReceiptTap(Action<Receipt, int> observe, Action<ReceiptTap> remove) : IDisposable
{
    public void Observe(Receipt receipt, int hpBefore) => observe(receipt, hpBefore);
    public void Dispose() => remove(this);
}
internal sealed class NativeWriteUncertain(Receipt attempted) : IOException("native_write_outcome_unknown_no_retry")
{
    public Receipt Attempted { get; } = attempted;
}

internal sealed class PacketWatch : IDisposable
{
    private readonly Action<PacketWatch> remove;
    internal readonly Func<Receipt, bool> Match;
    internal readonly long Floor;
    internal readonly TaskCompletionSource<Receipt> Signal = new(TaskCreationOptions.RunContinuationsAsynchronously);
    public Task<Receipt> Task => Signal.Task;
    internal PacketWatch(long floor, Func<Receipt, bool> match, Action<PacketWatch> remove) { Floor = floor; Match = match; this.remove = remove; }
    internal bool Consider(Receipt receipt)
    {
        if (receipt.Sequence <= Floor || !Match(receipt)) return false;
        Signal.TrySetResult(receipt); return true;
    }
    public void Dispose() => remove(this);
}

internal sealed class NativeActor : IAsyncDisposable
{
    private readonly object gate = new();
    private readonly SemaphoreSlim sendGate = new(1, 1);
    private readonly CancellationTokenSource life = new();
    private readonly List<PacketWatch> watches = new();
    private readonly List<ReceiptTap> taps = new();
    private readonly Dictionary<uint, (Cell Position, bool Dead)> visible = new();
    private readonly HashSet<(uint ObjectId, BuffType Type)> buffs = new();
    private bool requireNoBuffs;
    private readonly ConcurrentDictionary<long, double> pings = new();
    private readonly Evidence evidence;
    private readonly TaskCompletionSource<bool> failed = new(TaskCreationOptions.RunContinuationsAsynchronously);
    private TcpClient? tcp;
    private NetworkStream? stream;
    private Task? receiveTask, heartbeatTask;
    private bool closing;
    private long sequence, ping;
    private (bool Equip, ulong Uid, int To, MirGridType Grid)? equipmentCommand;
    private (ulong Uid, int Count, int Stock)? potionCommand;
    private UserItem[] inventory = Array.Empty<UserItem>(), equipment = Array.Empty<UserItem>(), questInventory = Array.Empty<UserItem>();
    private uint gold, credit;
    public Receipt? LastUserInformation { get; private set; }
    public JsonElement? LastSuccessfulLogoutState { get; private set; }
    public int Ordinal { get; }
    public int Connection { get; }
    public PrivateActor Identity { get; }
    private volatile uint objectId;
    private volatile Cell location = new(-1, -1);
    private volatile int mapIndex = -1, hp, mp;
    private volatile bool connected, inGame;
    private volatile string? failure;
    private volatile string setupPhase = "not-started";
    public uint ObjectId { get => objectId; private set => objectId = value; }
    public Cell Location { get => location; private set => location = value; }
    public int MapIndex { get => mapIndex; private set => mapIndex = value; }
    public int Hp { get => hp; private set => hp = value; }
    public bool Connected { get => connected; private set => connected = value; }
    public bool InGame { get => inGame; private set => inGame = value; }
    public string? Failure { get => failure; private set => failure = value; }
    public long BytesSent, BytesReceived;
    public ApplicationCounterSnapshot ApplicationCounters()
    {
        var before = Time.Now; var sent = Interlocked.Read(ref BytesSent); var received = Interlocked.Read(ref BytesReceived);
        return new(Ordinal, Connection, before, Time.Now, sent, received);
    }
    public long Cursor { get { lock (gate) return sequence; } }
    public int PotionStock { get { lock (gate) return inventory.Where(x => x?.ItemIndex == 662).Sum(x => x.Count); } }
    public ReceiptTap Tap(Action<Receipt, int> observe)
    {
        var tap = new ReceiptTap(observe, value => { lock (gate) taps.Remove(value); });
        lock (gate) taps.Add(tap); return tap;
    }
    public NativeActor(PrivateActor identity, int connection, Evidence evidence) { Identity = identity; Ordinal = identity.Ordinal; Connection = connection; this.evidence = evidence; }
    public object State()
    {
        lock (gate) return new { actorOrdinal = Ordinal, connection = Connection, objectId = ObjectId, mapIndex = MapIndex, position = Location, hp = Hp, mp, connected = Connected, inGame = InGame, setupPhase, failure = Failure,
            itemState = ItemState.Describe(inventory, equipment, questInventory, gold, credit), potionStock = inventory.Where(x => x?.ItemIndex == 662).Sum(x => x.Count) };
    }
    public string ItemFingerprint { get { lock (gate) return ItemState.Fingerprint(inventory, equipment, questInventory, gold, credit); } }
    public bool Sees(uint id) { lock (gate) return visible.TryGetValue(id, out var entity) && !entity.Dead; }
    public uint[] VisiblePlayerIds() { lock (gate) return visible.Where(x => !x.Value.Dead).Select(x => x.Key).Order().ToArray(); }
    public int OwnedBuffCount { get { lock (gate) return buffs.Count(x => x.ObjectId == ObjectId); } }
    public void RequireNoCombatBuffs() { lock (gate) { Guard.Need(OwnedBuffCount == 0, "native_melee_initial_buff_present"); requireNoBuffs = true; } }
    public async Task WaitForPeer(uint id, double deadline)
    {
        if (Sees(id)) return; var floor = Cursor; using var watch = Watch<S.ObjectPlayer>(floor, x => x.ObjectID == id && !x.Dead);
        if (Sees(id)) return; await Wait(watch, deadline); Guard.Need(Sees(id), "declared_peer_not_visible");
    }
    public object PeerView(uint id) { lock (gate) return visible.TryGetValue(id, out var p) ? new { objectId = id, present = true, p.Position, p.Dead } : new { objectId = id, present = false, Position = new Cell(-1, -1), Dead = false }; }
    private void Fail(string code)
    {
        lock (gate) Failure ??= code;
        failed.TrySetResult(true);
    }
    public void StopGameplay(string code) => Fail(code);
    public PacketWatch Watch<T>(long floor, Func<T, bool>? predicate = null) where T : Packet
    {
        PacketWatch? watch = null;
        watch = new PacketWatch(floor, e => e.ActorOrdinal == Ordinal && e.Connection == Connection && e.Packet is T p && (predicate is null || predicate(p)), w => { lock (gate) watches.Remove(w); });
        lock (gate) watches.Add(watch); return watch;
    }
    public async Task<Receipt> Wait(PacketWatch watch, double absoluteDeadline, CancellationToken token = default, bool allowFailure = false)
    {
        var remaining = absoluteDeadline - Time.Now;
        if (watch.Task.IsCompletedSuccessfully)
        { var captured = await watch.Task; if (captured.At <= absoluteDeadline) return captured; throw new TimeoutException("matching_packet_after_absolute_deadline"); }
        if (remaining <= 0) throw new TimeoutException("absolute_deadline_expired");
        var timer = Task.Delay(TimeSpan.FromMilliseconds(remaining), token);
        var winner = allowFailure ? await Task.WhenAny(watch.Task, timer) : await Task.WhenAny(watch.Task, timer, failed.Task);
        // Recheck receipt time even if the timeout continuation was scheduled later.
        if (watch.Task.IsCompletedSuccessfully)
        { var receipt = await watch.Task; if (receipt.At <= absoluteDeadline) return receipt; throw new TimeoutException("matching_packet_after_absolute_deadline"); }
        token.ThrowIfCancellationRequested();
        if (!allowFailure && winner == failed.Task) throw new IOException(Failure ?? "native_actor_failed");
        throw new TimeoutException("absolute_deadline_expired");
    }
    public async Task<Receipt> Send(Packet packet, double? absoluteDeadline = null, CancellationToken token = default, object? correlation = null)
    {
        await sendGate.WaitAsync(token);
        try
        {
            Guard.Need(stream is not null && Connected && !closing, "native_transport_not_connected");
            Guard.Need(evidence.Complete || packet is C.LogOut or C.KeepAlive, "fatal_evidence_failure_no_further_gameplay_send");
            var bytes = packet.GetPacketBytes().ToArray();
            if (absoluteDeadline is not null && Time.Now > absoluteDeadline) throw new TimeoutException("not_sent_absolute_deadline_expired");
            var at = Time.Now; long seq; lock (gate) seq = ++sequence;
            using var writeLimit = CancellationTokenSource.CreateLinkedTokenSource(token, life.Token);
            writeLimit.CancelAfter(TimeSpan.FromMilliseconds(Math.Max(1, (absoluteDeadline ?? at + 5000) - Time.Now)));
            try
            {
                var write = stream!.WriteAsync(bytes, writeLimit.Token);
                evidence.Add(new { kind = "packet", direction = "sent", actorOrdinal = Ordinal, connection = Connection, sequence = seq, monotonicMs = at, atUtc = Time.Utc(at), packet = packet.GetType().Name, correlation,
                    timestampScope = "immediately before native NetworkStream.WriteAsync invocation; not delivery acknowledgement" });
                await write; Interlocked.Add(ref BytesSent, bytes.Length);
            }
            catch { Fail("native_write_outcome_unknown_no_retry"); throw new NativeWriteUncertain(new Receipt(Ordinal, Connection, seq, at, packet)); }
            return new Receipt(Ordinal, Connection, seq, at, packet);
        }
        finally { sendGate.Release(); }
    }
    public async Task Connect(Endpoint endpoint, byte[] md5, int setupTimeout = 15000, double? outerDeadline = null)
    {
        double Deadline() => Math.Min(Time.Now + setupTimeout, outerDeadline ?? double.MaxValue);
        TimeSpan Remaining() { var remaining = Deadline() - Time.Now; if (remaining <= 0) throw new TimeoutException("native_outer_state_deadline_expired"); return TimeSpan.FromMilliseconds(remaining); }
        void Phase(string value)
        {
            setupPhase = value;
            evidence.Add(new { kind = "bootstrapPhase", actorOrdinal = Ordinal, connection = Connection, phase = value, monotonicMs = Time.Now });
        }
        try
        {
        Phase("connect");
        Packet.IsServer = false; tcp = new TcpClient { NoDelay = true };
        await tcp.ConnectAsync(endpoint.Host, endpoint.Port).WaitAsync(Remaining());
        stream = tcp.GetStream(); Connected = true;
        Phase("greeting");
        using var connected = Watch<S.Connected>(0); receiveTask = Receive(); await Wait(connected, Deadline());
        Phase("version");
        var floor = Cursor; using var version = Watch<S.ClientVersion>(floor);
        var versionDeadline = Deadline(); await Send(new C.ClientVersion { VersionHash = md5 }, versionDeadline); var versionReceipt = await Wait(version, versionDeadline);
        Guard.Need(((S.ClientVersion)versionReceipt.Packet).Result == 1, "native_version_rejected");
        Phase("login");
        floor = Cursor; using var login = Watch<S.LoginSuccess>(floor);
        var loginDeadline = Deadline(); await Send(new C.Login { AccountID = Identity.AccountId, Password = Identity.Password }, loginDeadline); var loginReceipt = await Wait(login, loginDeadline);
        Guard.Need(((S.LoginSuccess)loginReceipt.Packet).Characters.Any(x => x.Index == Identity.CharacterIndex && x.Name == Identity.Name), "native_login_character_mismatch");
        Phase("start");
        floor = Cursor; using var start = Watch<S.StartGame>(floor); using var user = Watch<S.UserInformation>(floor, x => x.Name == Identity.Name); using var map = Watch<S.MapInformation>(floor, x => x.MapIndex == 440);
        var deadline = Deadline(); await Send(new C.StartGame { CharacterIndex = Identity.CharacterIndex }, deadline);
        var startReceipt = await Wait(start, deadline); Guard.Need(((S.StartGame)startReceipt.Packet).Result == 4, "native_StartGame_rejected");
        var userReceipt = await Wait(user, deadline); await Wait(map, deadline);
        var info = (S.UserInformation)userReceipt.Packet;
        Guard.Need(info.HP > 0 && info.Level == 40 && info.Class == MirClass.Warrior && info.Gender == MirGender.Male && info.RealId == Identity.CharacterIndex && MapIndex == 440, "native_live_fixture_mismatch");
        LastUserInformation = userReceipt; InGame = true; heartbeatTask = Heartbeat();
        Phase("ready");
        evidence.Add(new { kind = "admitted", actorOrdinal = Ordinal, connection = Connection, objectId = ObjectId, mapIndex = MapIndex, position = Location, hp = Hp, monotonicMs = Time.Now, atUtc = Time.Utc(Time.Now), setupReceiveSequence = userReceipt.Sequence });
        }
        catch (Exception error)
        {
            evidence.Add(new { kind = "bootstrapFailure", actorOrdinal = Ordinal, connection = Connection, phase = setupPhase,
                exceptionType = error.GetType().Name, actorFailure = Failure, payloadRecorded = false, monotonicMs = Time.Now });
            throw;
        }
    }
    private async Task Receive()
    {
        var phase = "header"; int frameLength = 0, frameIndex = -1;
        try
        {
            var header = new byte[2];
            while (!life.IsCancellationRequested)
            {
                phase = "header"; await stream!.ReadExactlyAsync(header, life.Token); var length = BitConverter.ToUInt16(header); frameLength = length; frameIndex = -1;
                Guard.Need(length >= 4, "native_frame_length_invalid"); var frame = new byte[length]; header.CopyTo(frame, 0);
                phase = "body"; await stream.ReadExactlyAsync(frame.AsMemory(2), life.Token); Interlocked.Add(ref BytesReceived, length); frameIndex = BitConverter.ToInt16(frame, 2);
                phase = "decode";
                var packet = Packet.ReceivePacket(frame, out var extra); Guard.Need(packet is not null && extra.Length == 0, "native_frame_decode_failed");
                phase = "observe"; Observe(packet!);
            }
        }
        catch (OperationCanceledException) when (life.IsCancellationRequested) { }
        catch (Exception error)
        {
            if (!closing)
            {
                evidence.Add(new { kind = "transportFailure", actorOrdinal = Ordinal, connection = Connection, phase, exceptionType = error.GetType().Name,
                    frameLength, frameIndex, payloadRecorded = false, monotonicMs = Time.Now });
                Fail("native_receive_failure_" + phase + "_" + error.GetType().Name);
            }
        }
        finally { Connected = false; if (!closing) Fail("native_transport_closed"); }
    }
    internal void Observe(Packet packet)
    {
        var at = Time.Now; Receipt receipt; int previousHp; ReceiptTap[] observers;
        lock (gate)
        {
            previousHp = Hp;
            receipt = new Receipt(Ordinal, Connection, ++sequence, at, packet);
            switch (packet)
            {
                case S.UserInformation p: ObjectId = p.ObjectID; Location = Cell.Of(p.Location); Hp = p.HP; mp = p.MP; inventory = p.Inventory; equipment = p.Equipment; questInventory = p.QuestInventory; gold = p.Gold; credit = p.Credit; break;
                case S.MapInformation p: MapIndex = p.MapIndex; break;
                case S.UserLocation p: Location = Cell.Of(p.Location); break;
                case S.ObjectPlayer p: visible[p.ObjectID] = (Cell.Of(p.Location), p.Dead); break;
                case S.ObjectWalk p when visible.ContainsKey(p.ObjectID): visible[p.ObjectID] = (Cell.Of(p.Location), false); break;
                case S.ObjectRun p when visible.ContainsKey(p.ObjectID): visible[p.ObjectID] = (Cell.Of(p.Location), false); break;
                case S.ObjectRemove p: visible.Remove(p.ObjectID); break;
                case S.ObjectDied p when visible.TryGetValue(p.ObjectID, out var existing): visible[p.ObjectID] = (existing.Position, true); break;
                case S.HealthChanged p: Hp = p.HP; mp = p.MP; if (Hp <= 0) Fail("native_actor_dead"); break;
                case S.Death: Hp = 0; Fail("native_actor_dead"); break;
                case S.Login p: Fail("native_login_rejected_result_" + p.Result); break;
                case S.LoginBanned: Fail("native_login_banned"); break;
                case S.StartGameDelay: Fail("native_StartGame_delay"); break;
                case S.StartGameBanned: Fail("native_StartGame_banned"); break;
                case S.LogOutSuccess:
                    InGame = false; LastSuccessfulLogoutState = JsonSerializer.SerializeToElement(State(), Files.Compact); break;
                case S.KeepAlive p when pings.TryRemove(p.Time, out var sent):
                    evidence.Add(new { kind = "keepAlive", actorOrdinal = Ordinal, connection = Connection, receiveSequence = receipt.Sequence, sentAtMs = sent, receivedAtMs = at, latencyMs = at - sent }); break;
                case S.EquipItem p when p.Success: ApplyEquipment(true, p.UniqueID, p.To, p.Grid); break;
                case S.RemoveItem p when p.Success: ApplyEquipment(false, p.UniqueID, p.To, p.Grid); break;
                case S.RefreshItem p: ReplaceItem(p.Item); break;
                case S.AddBuff p:
                    buffs.Add((p.Buff.ObjectID, p.Buff.Type));
                    if (requireNoBuffs && p.Buff.ObjectID == ObjectId) Fail("native_melee_unexpected_buff");
                    break;
                case S.RemoveBuff p: buffs.Remove((p.ObjectID, p.Type)); break;
                case S.DuraChanged p: ChangeDurability(p.UniqueID, p.CurrentDura); break;
                case S.DeleteItem p: RemoveOwnedCount(p.UniqueID, p.Count); break;
                case S.UseItem p when p.Success:
                    if (p.Grid != MirGridType.Inventory || potionCommand is not { } expected || p.UniqueID != expected.Uid)
                    { Fail("unmatched_native_successful_potion_ack"); break; }
                    var stockItem = inventory.SingleOrDefault(x => x?.UniqueID == p.UniqueID);
                    if (stockItem is null || stockItem.ItemIndex != 662 || stockItem.Count != expected.Count || PotionStock != expected.Stock)
                    { Fail("native_potion_prior_stock_drift"); break; }
                    RemoveOwnedCount(p.UniqueID, 1); break;
            }
            foreach (var watch in watches.ToArray())
                if (watch.Consider(receipt)) watches.Remove(watch);
            observers = taps.ToArray();
        }
        object? detail = packet switch
        {
            S.UserInformation p => new { objectId = p.ObjectID, realCharacterIndex = p.RealId, nameHash = Files.HashText(p.Name), identityMatchesFixture = p.Name == Identity.Name && p.RealId == Identity.CharacterIndex,
                position = Cell.Of(p.Location), hp = p.HP, mp = p.MP, level = p.Level, magicCount = p.Magics.Count, p.HasHero, p.CreatureSummoned,
                intelligentCreatureCount = p.IntelligentCreatures.Count, authoritativeItemState = ItemState.Describe(p.Inventory, p.Equipment, p.QuestInventory, p.Gold, p.Credit),
                stateSource = "this received native UserInformation payload, not client reducer" },
            S.UserLocation p => new { position = Cell.Of(p.Location), direction = (int)p.Direction },
            S.ObjectWalk p => new { objectId = p.ObjectID, position = Cell.Of(p.Location) },
            S.ObjectPlayer p => new { objectId = p.ObjectID, position = Cell.Of(p.Location), dead = p.Dead },
            S.ObjectRemove p => new { objectId = p.ObjectID },
            S.ObjectChat p => ChatDetail(p),
            S.EquipItem p => new { p.UniqueID, p.To, p.Grid, p.Success },
            S.RemoveItem p => new { p.UniqueID, p.To, p.Grid, p.Success },
            S.ChangeAMode p => new { mode = (int)p.Mode },
            S.ObjectAttack p => new { objectId = p.ObjectID, position = Cell.Of(p.Location), direction = (int)p.Direction, spell = (int)p.Spell },
            S.Struck p => new { attackerId = p.AttackerID },
            S.ObjectStruck p => new { objectId = p.ObjectID, attackerId = p.AttackerID, position = Cell.Of(p.Location), direction = (int)p.Direction },
            S.DamageIndicator p => new { objectId = p.ObjectID, damage = p.Damage, type = (int)p.Type },
            S.HealthChanged p => new { hpBefore = previousHp, hp = p.HP, mp = p.MP },
            S.UseItem p => new { p.UniqueID, p.Grid, p.Success, remainingCountPresentInNativePacket = false },
            S.DuraChanged p => new { p.UniqueID, p.CurrentDura },
            S.DeleteItem p => new { p.UniqueID, p.Count },
            S.ObjectDied p => new { objectId = p.ObjectID },
            S.LogOutSuccess => new { lastAuthoritativeOwnerState = LastSuccessfulLogoutState,
                stateAnchor = "this ACK after all prior packets on the same native TCP connection were applied" },
            S.AddBuff p => new { objectId = p.Buff.ObjectID, type = (int)p.Buff.Type, p.Buff.Paused },
            S.RemoveBuff p => new { objectId = p.ObjectID, type = (int)p.Type },
            _ => null
        };
        evidence.Add(new { kind = "packet", direction = "received", actorOrdinal = Ordinal, connection = Connection, receipt.Sequence, monotonicMs = at, atUtc = Time.Utc(at), packet = packet.GetType().Name, detail });
        foreach (var observer in observers)
            try { observer.Observe(receipt, previousHp); }
            catch { Fail("native_receipt_observer_failed"); }
        if (packet is S.HealthChanged changed && changed.HP > previousHp)
            evidence.Add(new { kind = "hpIncreaseObservation", actorOrdinal = Ordinal, connection = Connection, receiveSequence = receipt.Sequence,
                receivedAtMs = at, hpBefore = previousHp, hp = changed.HP, potionCausationClaimed = false });
    }
    private static object ChatDetail(S.ObjectChat chat)
    {
        var at = chat.Text.LastIndexOf(' '); var suffix = at >= 0 ? chat.Text[(at + 1)..] : chat.Text;
        return new { objectId = chat.ObjectID, nonceHash = suffix.StartsWith("m2cmp:", StringComparison.Ordinal) ? Files.HashText(suffix) : null, textHash = Files.HashText(chat.Text) };
    }
    private void ApplyEquipment(bool equip, ulong uid, int to, MirGridType grid)
    {
        if (equipmentCommand is not { } command || command != (equip, uid, to, grid)) { Fail("unmatched_native_equipment_ack"); return; }
        var from = equip ? inventory : equipment; var into = equip ? equipment : inventory;
        var index = Array.FindIndex(from, x => x?.UniqueID == uid);
        if (grid != MirGridType.Inventory || index < 0 || to < 0 || to >= into.Length || into[to] is not null) { Fail("native_equipment_state_mismatch"); return; }
        into[to] = from[index]; from[index] = null!;
    }
    private void ReplaceItem(UserItem item)
    {
        foreach (var grid in new[] { inventory, equipment })
        { var index = Array.FindIndex(grid, x => x?.UniqueID == item.UniqueID); if (index >= 0) { grid[index] = item; return; } }
    }
    private void ChangeDurability(ulong uid, ushort current)
    {
        var item = inventory.Concat(equipment).Concat(questInventory).SingleOrDefault(x => x?.UniqueID == uid);
        if (item is null) { Fail("native_durability_unknown_owned_item"); return; }
        item.CurrentDura = current;
    }
    private void RemoveOwnedCount(ulong uid, int count)
    {
        foreach (var grid in new[] { inventory, equipment, questInventory })
        {
            var at = Array.FindIndex(grid, x => x?.UniqueID == uid); if (at < 0) continue;
            if (count <= 0 || count > grid[at].Count) { Fail("native_owned_item_count_invalid"); return; }
            if (count == grid[at].Count) grid[at] = null!; else grid[at].Count -= checked((ushort)count);
            return;
        }
        Fail("native_owned_item_count_unknown_uid");
    }
    public async Task<object> SetAttackAll(int budget)
    {
        var floor = Cursor; using var watch = Watch<S.ChangeAMode>(floor, p => p.Mode == AttackMode.All);
        var deadline = Time.Now + budget; var sent = await Send(new C.ChangeAMode { Mode = AttackMode.All }, deadline);
        var ack = await Wait(watch, deadline);
        var result = new { actorOrdinal = Ordinal, connection = Connection, mode = (int)AttackMode.All, sentSequence = sent.Sequence,
            receiveSequence = ack.Sequence, sentAtMs = sent.At, receivedAtMs = ack.At, deadlineAtMs = deadline };
        evidence.Add(new { kind = "attackModeAll", result }); return result;
    }
    internal PotionReservation ReservePotion(int maximumHp)
    {
        lock (gate)
        {
            var issueHp = Hp; Guard.Need(maximumHp > 0 && issueHp > 0 && issueHp <= maximumHp * .7 && potionCommand is null,
                "native_reactive_potion_trigger_or_pending_invalid");
            var item = inventory.FirstOrDefault(x => x?.ItemIndex == 662 && x.Count > 0) ?? throw new InvalidDataException("native_ordinary_potion_exhausted");
            var reservation = new PotionReservation(item.UniqueID, item.Count, PotionStock, issueHp, ItemState.Describe(inventory, equipment, questInventory, gold, credit));
            potionCommand = (reservation.Uid, reservation.Count, reservation.Stock); return reservation;
        }
    }
    public async Task<object> UsePotion(int maximumHp, Action<double> issued, CancellationToken token)
    {
        var decision = ReservePotion(maximumHp); var uid = decision.Uid; var count = decision.Count; var stock = decision.Stock;
        var floor = Cursor; using var watch = Watch<S.UseItem>(floor, p => p.UniqueID == uid && p.Grid == MirGridType.Inventory);
        var deadline = Time.Now + 2500;
        try
        {
            var sent = await Send(new C.UseItem { UniqueID = uid, Grid = MirGridType.Inventory }, deadline, token); issued(sent.At);
            var ack = await Wait(watch, deadline, token); Guard.Need(((S.UseItem)ack.Packet).Success && Failure is null, "native_ordinary_potion_ack_rejected");
            object after; int remaining, remainingStock;
            lock (gate)
            {
                remaining = inventory.SingleOrDefault(x => x?.UniqueID == uid)?.Count ?? 0; remainingStock = PotionStock;
                Guard.Need(remaining == count - 1 && remainingStock == stock - 1, "native_potion_ack_derived_stock_not_exactly_one");
                after = ItemState.Describe(inventory, equipment, questInventory, gold, credit); potionCommand = null;
            }
            return new { actorOrdinal = Ordinal, connection = Connection, uniqueId = uid, grid = (int)MirGridType.Inventory, itemIndex = 662,
                hpAtDecision = decision.HpAtDecision, maximumHp, triggerRatio = .7, sentSequence = sent.Sequence, receiveSequence = ack.Sequence,
                sentAtMs = sent.At, receivedAtMs = ack.At, deadlineAtMs = deadline, success = true, countBefore = count, countAfter = remaining,
                stockBefore = stock, stockAfter = remainingStock, before = decision.Before, after,
                immediateStockEvidence = "initial/native-refresh owned state plus successful native UseItem ACK semantics; server packet contains no remaining count",
                freshStockSnapshotClaimed = false, finalFreshAndAllDiskReadbackRequired = true, laterHpIncreaseIsUniquePotionProof = false };
        }
        catch { Fail("native_issued_potion_evidence_failed_no_retry"); throw; }
    }
    private async Task Heartbeat()
    {
        try
        {
            while (!life.IsCancellationRequested)
            {
                var id = Interlocked.Increment(ref ping); pings[id] = Time.Now; await Send(new C.KeepAlive { Time = id }, token: life.Token);
                foreach (var old in pings.Where(x => Time.Now - x.Value > 5000).ToArray())
                    if (pings.TryRemove(old.Key, out _)) evidence.Add(new { kind = "keepAliveUnresolvedDiagnostic", actorOrdinal = Ordinal, connection = Connection, elapsedMs = Time.Now - old.Value, notAnAdditionalWorkloadVeto = true });
                await Task.Delay(2000, life.Token);
            }
        }
        catch (OperationCanceledException) when (life.IsCancellationRequested) { }
        catch { if (!closing) Fail("native_keepalive_failure"); }
    }
    public (ulong UniqueId, int BagSlot, string BeforeFingerprint) EquipmentChoice()
    {
        lock (gate)
        {
            Guard.Need(equipment.Length > (int)EquipmentSlot.Weapon && equipment[(int)EquipmentSlot.Weapon] is not null, "ordinary_weapon_not_equipped");
            var uid = equipment[(int)EquipmentSlot.Weapon].UniqueID; const int bagSlot = 15; Guard.Need(inventory.Length > bagSlot && inventory[bagSlot] is null, "common_wire_bag_slot15_must_be_empty");
            return (uid, bagSlot, ItemState.Fingerprint(inventory, equipment, questInventory, gold, credit));
        }
    }
    public async Task<EquipmentAck> EquipmentCommand(bool equip, ulong uid, int to, int budget)
    {
        var floor = Cursor; using var watch = Watch<Packet>(floor, packet => equip ? packet is S.EquipItem p && p.UniqueID == uid && p.To == to && p.Grid == MirGridType.Inventory : packet is S.RemoveItem r && r.UniqueID == uid && r.To == to && r.Grid == MirGridType.Inventory);
        lock (gate) equipmentCommand = (equip, uid, to, MirGridType.Inventory);
        var at = Time.Now; var deadline = at + budget;
        var sent = await Send(equip ? new C.EquipItem { Grid = MirGridType.Inventory, UniqueID = uid, To = to } : new C.RemoveItem { Grid = MirGridType.Inventory, UniqueID = uid, To = to }, deadline);
        var receipt = await Wait(watch, deadline); var ok = receipt.Packet is S.EquipItem e ? e.Success : ((S.RemoveItem)receipt.Packet).Success;
        Guard.Need(ok && Failure is null, "native_equipment_rejected"); lock (gate) equipmentCommand = null;
        return new EquipmentAck(sent.Sequence, receipt.Sequence, sent.At, receipt.At, deadline, receipt.At - sent.At, ok, ItemFingerprint);
    }
    public async Task<object> Logout(int budget, double? outerDeadline = null)
    {
        Guard.Need(InGame, "logout_actor_not_in_world"); var floor = Cursor;
        using var watch = Watch<Packet>(floor, p => p is S.LogOutSuccess or S.LogOutFailed); var at = Time.Now; var deadline = Math.Min(at + budget, outerDeadline ?? double.MaxValue);
        var sent = await Send(new C.LogOut(), deadline); var response = await Wait(watch, deadline, allowFailure: true);
        var ok = response.Packet is S.LogOutSuccess;
        var result = new { actorOrdinal = Ordinal, connection = Connection, normalLogout = ok, result = response.Packet.GetType().Name, sentSequence = sent.Sequence, receiveSequence = response.Sequence, sentAtMs = sent.At, receivedAtMs = response.At, deadlineAtMs = deadline, elapsedMs = response.At - sent.At,
            finalState = ok ? LastSuccessfulLogoutState : JsonSerializer.SerializeToElement(State(), Files.Compact),
            stateAnchor = "owner state frozen while processing this normal LogOut ACK, including prior HP and owned-item packets in TCP order" };
        evidence.Add(new { kind = "logout", result }); return result;
    }
    public async Task<object> Close(int budget, double? outerDeadline = null)
    {
        Guard.Need(!closing, "native_connection_already_closed"); var at = Time.Now; var deadline = Math.Min(at + budget, outerDeadline ?? double.MaxValue);
        closing = true; life.Cancel(); tcp?.Close();
        try
        {
            var left = deadline - Time.Now; if (left <= 0) throw new TimeoutException("native_tcp_close_deadline_expired");
            await Task.WhenAll(new[] { receiveTask, heartbeatTask }.Where(x => x is not null).Select(x => x!)).WaitAsync(TimeSpan.FromMilliseconds(left));
            Guard.Need(Time.Now <= deadline, "native_tcp_close_deadline_expired");
            var result = new { actorOrdinal = Ordinal, connection = Connection, localTcpCloseCompleted = true, receiveLoopCompleted = receiveTask?.IsCompleted ?? true,
                elapsedMs = Time.Now - at, startedAtMs = at, closedAtMs = Time.Now, deadlineAtMs = deadline, remoteWebSocketCloseAckApplicable = false };
            evidence.Add(new { kind = "transportClosed", result }); return result;
        }
        finally { Connected = false; stream?.Dispose(); tcp?.Dispose(); life.Dispose(); sendGate.Dispose(); }
    }
    public async ValueTask DisposeAsync()
    {
        if (closing) return; try { await Close(5000); } catch { }
    }
}
