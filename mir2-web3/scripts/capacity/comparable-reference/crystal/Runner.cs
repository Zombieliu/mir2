using System.Net;
using System.Security.Cryptography;
using System.Text.Json;
using C = ClientPackets;
using S = ServerPackets;

namespace ComparableCrystal;

internal sealed class ActionResult
{
    public int ActorOrdinal { get; init; }
    public long TicketSequence { get; init; }
    public string Kind { get; init; } = "";
    public double DueMs { get; init; }
    public double DeadlineMs { get; init; }
    public double PlannedAtMs { get; init; }
    public double DeadlineAtMs { get; init; }
    public double OfferedAtMs { get; init; }
    public double LatenessMs { get; set; }
    public double? SentAtMs { get; set; }
    public long? SentSequence { get; set; }
    public Cell? Origin { get; set; }
    public Cell? Target { get; init; }
    public string? Direction { get; init; }
    public string? NonceHash { get; init; }
    public uint ExpectedSharedId { get; set; }
    public int? TargetActorOrdinal { get; set; }
    public uint? ExpectedTargetSharedId { get; set; }
    public JsonElement? CombatEvidence { get; set; }
    public JsonElement? LateCombatEvidence { get; set; }
    public int[] ExpectedPeerOrdinals { get; init; } = Array.Empty<int>();
    public Dictionary<int, long> ReceiveSequenceFloors { get; } = new();
    public Dictionary<int, object> Received { get; } = new();
    public bool CompletedOnTime { get; set; }
    public bool Timeout { get; set; }
    public bool NotSent { get; set; }
    public bool SendOutcomeUnknown { get; set; }
    public bool Rejected { get; set; }
    public bool TerminalObserved { get; set; }
    public bool LateTerminal { get; set; }
    public bool CensoredAtObservationEnd { get; set; }
    public double? LatencyMs { get; set; }
    public string Reason { get; set; } = "";
    public int? WaitingForReceiverActorOrdinal { get; set; }
    internal JsonElement? FrozenOutcome { get; private set; }
    internal JsonElement FreezeOutcome()
    {
        if (FrozenOutcome is { } previous) return previous;
        var snapshot = JsonSerializer.SerializeToElement(this, Files.Compact); FrozenOutcome = snapshot; return snapshot;
    }
    internal void CompleteOnTime(IEnumerable<double> receivedAt, int expectedReceivers)
    {
        var times = receivedAt.ToArray();
        if (Timeout || NotSent || Rejected || times.Length != expectedReceivers || times.Any(x => !double.IsFinite(x) || x > DeadlineAtMs)) return;
        CompletedOnTime = true; LatencyMs = times.Max() - SentAtMs!.Value;
    }
}

internal interface IPendingPrimary
{
    Task? Task { get; }
    void Start();
}
internal sealed class ActorSlot
{
    public readonly object Gate = new();
    public IPendingPrimary? Pending;
    public ReactivePotion? Potion;
    public double LastPotionSentAt = double.NegativeInfinity;
}

internal sealed class PendingAction : IPendingPrimary
{
    private readonly NativeActor actor;
    private readonly NativeActor[] peers;
    private readonly PlanAction action;
    private readonly Evidence evidence;
    private readonly ActorSlot slot;
    private readonly CancellationToken observationEnd;
    private readonly double observationCapAtMs;
    private readonly Dictionary<int, Receipt> captured = new();
    private readonly List<(NativeActor Actor, PacketWatch Watch)> watches = new();
    public ActionResult Result { get; }
    public Task? Task { get; private set; }
    public PendingAction(NativeActor actor, NativeActor[] peers, PlanAction action, Evidence evidence, ActorSlot slot, ActionResult result, CancellationToken observationEnd, double observationCapAtMs)
    { this.actor = actor; this.peers = peers; this.action = action; this.evidence = evidence; this.slot = slot; Result = result; this.observationEnd = observationEnd; this.observationCapAtMs = observationCapAtMs; }
    public void Start() => Task = Execute();
    internal static bool ExactNonce(S.ObjectChat packet, uint id, string nonce)
    {
        if (packet.ObjectID != id || !packet.Text.EndsWith(nonce, StringComparison.Ordinal)) return false;
        var prefix = packet.Text.Length - nonce.Length;
        return prefix == 0 || char.IsWhiteSpace(packet.Text[prefix - 1]) || packet.Text[prefix - 1] == ':';
    }
    private void AddReceipt(Receipt receipt)
    {
        if (!captured.TryAdd(receipt.ActorOrdinal, receipt)) return;
        var detail = new { receiverActorOrdinal = receipt.ActorOrdinal, connection = receipt.Connection, receiveSequence = receipt.Sequence, receivedAtMs = receipt.At,
            atUtc = Time.Utc(receipt.At), onTime = receipt.At <= Result.DeadlineAtMs, packet = receipt.Packet.GetType().Name };
        Result.Received[receipt.ActorOrdinal] = detail;
        evidence.Add(new { kind = receipt.ActorOrdinal == actor.Ordinal ? "ownerReceipt" : "peerReceipt", actorOrdinal = actor.Ordinal, action = action.Kind, ticketSequence = action.TicketSequence,
            expectedSharedId = Result.ExpectedSharedId, nonceHash = Result.NonceHash, target = action.Target, detail });
    }
    private void Outcome() => evidence.Add(new { kind = "actionOutcome", result = Result.FreezeOutcome() });
    private async Task Execute()
    {
        try
        {
            Guard.Need(actor.InGame && actor.Hp > 0 && actor.Failure is null, "actor_not_live");
            Guard.Need(peers.All(p => p.InGame && p.MapIndex == actor.MapIndex && p.Hp > 0 && actor.Sees(p.ObjectId) && p.Sees(actor.ObjectId) && p.Location.Distance(actor.Location) <= 12), "declared_peer_not_mutually_visible");
            Result.Origin = actor.Location; Result.ExpectedSharedId = actor.ObjectId;
            if (action.Kind == "walk")
            {
                Guard.Need(Enum.TryParse<MirDirection>(action.Direction, out var direction), "walk_direction_invalid");
                Guard.Need(Cell.Of(Functions.PointMove(actor.Location.Point, direction, 1)) == action.Target, "plan_and_authoritative_position_disagree");
            }
            foreach (var receiver in new[] { actor }.Concat(peers))
            {
                var floor = receiver.Cursor; Result.ReceiveSequenceFloors[receiver.Ordinal] = floor;
                PacketWatch watch;
                if (action.Kind == "chat") watch = receiver.Watch<S.ObjectChat>(floor, p => ExactNonce(p, actor.ObjectId, action.Nonce!));
                else if (receiver == actor) watch = receiver.Watch<S.UserLocation>(floor);
                else watch = receiver.Watch<S.ObjectWalk>(floor, p => p.ObjectID == actor.ObjectId && Cell.Of(p.Location) == action.Target);
                watches.Add((receiver, watch));
            }
            var packet = action.Kind == "walk" ? (Packet)new C.Walk { Direction = Enum.Parse<MirDirection>(action.Direction!) } : new C.Chat { Message = action.Nonce! };
            var sent = await actor.Send(packet, Result.DeadlineAtMs, observationEnd, new { action = action.Kind, ticketSequence = action.TicketSequence, target = action.Target, direction = action.Direction, nonceHash = action.NonceSha256 });
            Result.SentAtMs = sent.At; Result.SentSequence = sent.Sequence; Result.LatenessMs = sent.At - Result.PlannedAtMs;
            evidence.Add(new { kind = "sent", actorOrdinal = actor.Ordinal, ticketSequence = action.TicketSequence, action = action.Kind, sentSequence = sent.Sequence, sentAtMs = sent.At,
                plannedAtMs = Result.PlannedAtMs, deadlineAtMs = Result.DeadlineAtMs, origin = Result.Origin, target = Result.Target, direction = Result.Direction,
                expectedSharedId = Result.ExpectedSharedId, expectedPeerOrdinals = Result.ExpectedPeerOrdinals, receiveSequenceFloors = Result.ReceiveSequenceFloors, nonceHash = Result.NonceHash,
                peerViews = peers.Select(p => new { receiverActorOrdinal = p.Ordinal, peerPosition = p.Location, speakerInPeer = p.PeerView(actor.ObjectId) }).ToArray() });
            try
            {
                await Collect(false);
                Result.TerminalObserved = true;
                Result.CompleteOnTime(captured.Values.Select(x => x.At), peers.Length + 1);
                Outcome();
            }
            catch (TimeoutException)
            {
                Result.Timeout = true; Result.Reason = "matching_receipts_not_complete_by_absolute_deadline";
                evidence.Add(new { kind = "timeout", actorOrdinal = actor.Ordinal, action = action.Kind, ticketSequence = action.TicketSequence, deadlineAtMs = Result.DeadlineAtMs, observedAtMs = Time.Now });
                Outcome(); // Sticky original failure: never rewritten after the late observer completes.
                try
                {
                    await Collect(true); Result.TerminalObserved = true; Result.LateTerminal = true;
                    evidence.Add(new { kind = "lateClosure", actorOrdinal = actor.Ordinal, action = action.Kind, ticketSequence = action.TicketSequence,
                        originalTimeoutRemainsFailure = true, rejected = Result.Rejected, captured = Result.Received, closedAtMs = Time.Now });
                }
                catch (OperationCanceledException) when (observationEnd.IsCancellationRequested) { Result.CensoredAtObservationEnd = true; }
            }
        }
        catch (OperationCanceledException) when (observationEnd.IsCancellationRequested)
        {
            Result.CensoredAtObservationEnd = true; Result.Reason = "observation_ended_with_unresolved_action";
            if (Result.SentAtMs is null) Result.NotSent = true;
            if (!Result.Timeout) Outcome();
        }
        catch (Exception error)
        {
            Result.Reason = error is InvalidDataException ? error.Message : error is TimeoutException ? "planned_deadline_expired_before_send" : "native_action_or_transport_failure";
            if (error is NativeWriteUncertain unknown)
            {
                Result.SendOutcomeUnknown = true; Result.SentAtMs = unknown.Attempted.At; Result.SentSequence = unknown.Attempted.Sequence;
                evidence.Add(new { kind = "sendOutcomeUnknown", actorOrdinal = actor.Ordinal, action = action.Kind, ticketSequence = action.TicketSequence, noRetryAllowed = true });
            }
            else if (Result.SentAtMs is null) Result.NotSent = true;
            if (Result.NotSent) evidence.Add(new { kind = "notSent", result = Result });
            if (!Result.Timeout) Outcome();
        }
        finally
        {
            foreach (var watch in watches) watch.Watch.Dispose();
            lock (slot.Gate) if (ReferenceEquals(slot.Pending, this)) slot.Pending = null;
        }
    }
    private async Task Collect(bool late)
    {
        foreach (var receiver in watches)
        {
            if (captured.ContainsKey(receiver.Actor.Ordinal)) continue;
            Result.WaitingForReceiverActorOrdinal = receiver.Actor.Ordinal;
            var receipt = late ? await receiver.Watch.Task.WaitAsync(observationEnd) : await receiver.Actor.Wait(receiver.Watch, Result.DeadlineAtMs, observationEnd);
            if (late && receipt.At > observationCapAtMs) { Result.CensoredAtObservationEnd = true; throw new InvalidDataException("receipt_after_fixed_observation_cap"); }
            AddReceipt(receipt);
            if (action.Kind == "walk" && receiver.Actor == actor && Cell.Of(((S.UserLocation)receipt.Packet).Location) != action.Target)
            {
                Result.Rejected = true; Result.Reason = Cell.Of(((S.UserLocation)receipt.Packet).Location) == Result.Origin ? "negative_correction_at_origin" : "unexpected_authoritative_transform";
                if (Result.Reason == "unexpected_authoritative_transform") actor.StopGameplay("unexpected_authoritative_transform");
                return; // A native negative response is terminal; no ObjectWalk witness should be invented.
            }
        }
        Result.WaitingForReceiverActorOrdinal = null;
    }
}

internal static class Runner
{
    private static async Task<object> FreshEquipmentState(List<NativeActor> actors, List<NativeActor> connections, int index, EquipmentAck ack, Plan plan, PrivateFixture fixture, byte[] md5, Evidence events)
    {
        var old = actors[index]; var outer = ack.ReceivedAtMs + plan.Thresholds.EquipmentStateDeadlineMs; var expectedPosition = old.Location; var expectedFingerprint = ack.AckDerivedExpectedFingerprint;
        var logout = await old.Logout(plan.Thresholds.LogoutAckDeadlineMs, outer);
        Guard.Need(JsonSerializer.SerializeToElement(logout, Files.Compact).GetProperty("normalLogout").GetBoolean(), "equipment_state_normal_logout_required");
        var closed = await old.Close(plan.Thresholds.TransportCloseDeadlineMs, outer);
        var fresh = new NativeActor(old.Identity, old.Connection + 1, events); actors[index] = fresh; connections.Add(fresh);
        await fresh.Connect(fixture.Endpoint, md5, outerDeadline: outer);
        var info = fresh.LastUserInformation ?? throw new InvalidDataException("fresh_native_UserInformation_missing");
        Guard.Need(info.At <= outer && fresh.Location == expectedPosition && fresh.ItemFingerprint == expectedFingerprint, "fresh_equipment_state_not_preserved_by_outer_deadline");
        var result = new { method = "normal_logout_TCPclose_fresh_Login_StartGame_UserInformation", actorOrdinal = fresh.Ordinal,
            priorConnection = old.Connection, connection = fresh.Connection, priorSharedId = old.ObjectId, newSharedId = fresh.ObjectId,
            ackReceivedAtMs = ack.ReceivedAtMs, outerDeadlineAtMs = outer, userInformationReceiveSequence = info.Sequence, userInformationReceivedAtMs = info.At,
            elapsedAfterAckMs = info.At - ack.ReceivedAtMs, expectedFingerprint, actualFingerprint = fresh.ItemFingerprint, independentFullInventoryEquipmentBeltGoldCapacity = true,
            logout, closed, state = fresh.State(), passed = true };
        events.Add(new { kind = "equipmentFreshState", result }); return result;
    }
    private static double? P95(IEnumerable<double> values) { var sorted = values.Order().ToArray(); return sorted.Length == 0 ? null : sorted[(int)Math.Ceiling(sorted.Length * .95) - 1]; }
    internal static object[] Windows(Plan plan, IReadOnlyList<ActionResult> rows)
    {
        var output = new List<object>();
        foreach (var actor in plan.Actors) for (var minute = 0; minute < plan.DurationMs / 60000; minute++)
        {
            var slice = rows.Where(x => x.ActorOrdinal == actor.Ordinal && x.DueMs >= minute * 60000 && x.DueMs < (minute + 1) * 60000).ToArray();
            var done = slice.Count(x => x.CompletedOnTime); var timeout = slice.Count(x => x.Timeout); var walks = slice.Count(x => x.Kind == "walk" && x.CompletedOnTime); var chats = slice.Count(x => x.Kind == "chat" && x.CompletedOnTime);
            var melee = plan.Scenario == "dispersed-melee"; var expectedOffers = melee ? 52 : 32;
            var attacks = slice.Count(x => x.Kind == "attack" && x.CompletedOnTime);
            var attackP95 = P95(slice.Where(x => x.Kind == "attack" && x.CompletedOnTime).Select(x => x.LatencyMs!.Value));
            var moveP95 = P95(slice.Where(x => x.Kind == "walk" && x.CompletedOnTime).Select(x => x.LatencyMs!.Value));
            var chatP95 = P95(slice.Where(x => x.Kind == "chat" && x.CompletedOnTime).Select(x => x.LatencyMs!.Value)); var schedule = P95(slice.Where(x => x.SentAtMs is not null).Select(x => Math.Max(0, x.SentAtMs!.Value - x.PlannedAtMs)));
            var primaryP95Passed = melee ? attackP95 is not null && attackP95 <= plan.Thresholds.AttackP95Ms : moveP95 is not null && moveP95 <= plan.Thresholds.MovementP95Ms;
            var passed = slice.Length == expectedOffers && done >= slice.Length * plan.Thresholds.MinimumCompletionRatio && timeout / (double)slice.Length < plan.Thresholds.MaximumTimeoutRatioExclusive
                && primaryP95Passed && chatP95 is not null && chatP95 <= plan.Thresholds.ChatP95Ms && schedule is not null && schedule <= plan.Thresholds.ScheduleP95Ms;
            output.Add(new { actorOrdinal = actor.Ordinal, minute = minute + 1, offered = slice.Length, completed = done, walks, attacks, chats, timeouts = timeout, notSent = slice.Count(x => x.NotSent), movementP95Ms = moveP95, attackP95Ms = attackP95, chatP95Ms = chatP95, scheduleP95Ms = schedule, passed });
        }
        return output.ToArray();
    }
    public static async Task<int> Run(Options options)
    {
        var planPath = Path.GetFullPath(options.Required("plan")); var plan = Files.Read<Plan>(planPath); plan.Validate(false); var combatContractSha = CombatPolicy.Verify(plan, options);
        var melee = plan.Scenario == "dispersed-melee";
        WorldTools.ValidateNavigation(plan, Path.GetFullPath(options.Required("navigation")));
        var fixture = Files.Read<PrivateFixture>(Files.StatePath(options.Required("actors")));
        Guard.Need(fixture.Schema == "mir2.crystal-reference-actors.v1" && fixture.PlanSha256 == Files.Hash(planPath) && fixture.NavigationSha256 == plan.NavigationSha256 && fixture.Actors.Length == plan.Population, "fixture_plan_identity_mismatch");
        Guard.Need(IPAddress.TryParse(fixture.Endpoint.Host, out var address) && IPAddress.IsLoopback(address), "only_loopback_game_endpoint_supported");
        var actualAssembly = typeof(Runner).Assembly.Location;
        Guard.Need(Files.Hash(fixture.VersionFile) == fixture.VersionFileSha256 && Files.Hash(actualAssembly) == fixture.VersionFileSha256, "fixture_version_must_pin_actual_running_adapter");
        var ready = Files.Read(Files.StatePath(options.Required("server-ready"))); var map = ready.GetProperty("fixturePlayerMap"); var bootstrap = ready.GetProperty("originalGuiBootstrap");
        Guard.Need(map.GetProperty("mapIndex").GetInt32() == 440 && map.GetProperty("monsterCount").GetInt32() == 0 && map.GetProperty("npcCount").GetInt32() == 0 && bootstrap.GetProperty("editLoadedBeforeMainStart").GetBoolean() && bootstrap.GetProperty("editGameShopCount").GetInt32() == 105 && bootstrap.GetProperty("mainGameShopCount").GetInt32() == 105, "native_host_ready_baseline_mismatch");
        var reportFile = Files.StatePath(options.Required("output")); Guard.Need(!File.Exists(reportFile), "fresh_report_required");
        var eventsFile = Files.StatePath(options.Required("events")); var stopFile = Files.StatePath(options.Required("stop-file")); Guard.Need(!File.Exists(stopFile), "fresh_stop_marker_required");
        using var events = new Evidence(eventsFile, options.Integer("max-event-rows", 8000000));
        var actors = new List<NativeActor>(); var rows = new List<ActionResult>(); var equipment = new List<object>(); var cleanup = new List<object>(); var reentries = new List<object>(); var transportCloses = new List<object>();
        var connections = new List<NativeActor>(); ApplicationCounterSnapshot[] trafficStart = Array.Empty<ApplicationCounterSnapshot>(), trafficEnd = Array.Empty<ApplicationCounterSnapshot>();
        var baseline = new Dictionary<int, string>(); var initialStates = new Dictionary<int, object>(); var finalStates = new Dictionary<int, object>(); var population = new List<object>(); var pending = new List<IPendingPrimary>(); object[] actorRoster = Array.Empty<object>();
        ReactivePotions? potions = null; var attackModes = new List<object>(); var combatBaselines = new List<object>(); var maximumHp = 0;
        var finalReentryOrdinals = new[] { 0, plan.Population - 1 };
        using var measurement = new CancellationTokenSource(); string? failure = null; double? start = null, end = null; object[] windows = Array.Empty<object>(); var measurementPassed = false;
        try
        {
            var md5 = MD5.HashData(await File.ReadAllBytesAsync(fixture.VersionFile));
            foreach (var definition in fixture.Actors.OrderBy(x => x.Ordinal))
            {
                var actor = new NativeActor(definition, 1, events); actors.Add(actor); connections.Add(actor); await actor.Connect(fixture.Endpoint, md5);
                Guard.Need(actor.Location == plan.Actors.Single(x => x.Ordinal == actor.Ordinal).Start, "native_authoritative_start_differs_from_plan"); baseline[actor.Ordinal] = actor.ItemFingerprint; initialStates[actor.Ordinal] = actor.State();
            }
            foreach (var ordinal in plan.EquipmentActorOrdinals!)
                for (var round = 1; round <= plan.EquipmentRoundsPerActor; round++)
                {
                    var index = actors.FindIndex(x => x.Ordinal == ordinal); var actor = actors[index]; var choice = actor.EquipmentChoice();
                    var remove = await actor.EquipmentCommand(false, choice.UniqueId, choice.BagSlot, plan.Thresholds.EquipmentAckDeadlineMs);
                    var removedState = await FreshEquipmentState(actors, connections, index, remove, plan, fixture, md5, events);
                    actor = actors[index]; var equip = await actor.EquipmentCommand(true, choice.UniqueId, (int)EquipmentSlot.Weapon, plan.Thresholds.EquipmentAckDeadlineMs);
                    var equippedState = await FreshEquipmentState(actors, connections, index, equip, plan, fixture, md5, events);
                    Guard.Need(actors[index].ItemFingerprint == choice.BeforeFingerprint, "full_equipment_roundtrip_state_mismatch");
                    var result = new { actorOrdinal = ordinal, round, removeTargetWireInventorySlot = choice.BagSlot, equipTargetWeaponSlot = (int)EquipmentSlot.Weapon,
                        itemIdentityHash = Files.HashText(choice.UniqueId.ToString()), beforeFingerprint = choice.BeforeFingerprint,
                        afterFingerprint = actors[index].ItemFingerprint, remove, removedState, equip, equippedState, passed = true };
                    equipment.Add(result); events.Add(new { kind = "equipmentRoundtrip", result });
                }
            var aoiDeadline = Time.Now + 15000;
            await Task.WhenAll(actors.SelectMany(actor => plan.Actors.Single(x => x.Ordinal == actor.Ordinal).PeerOrdinals.Select(peer => actor.WaitForPeer(actors.Single(x => x.Ordinal == peer).ObjectId, aoiDeadline))));
            if (melee)
            {
                var publicFixture = Files.Read(Path.Combine(Path.GetDirectoryName(options.Required("actors"))!, "comparable-fixture.json"));
                Guard.Need(publicFixture.GetProperty("planSha256").GetString() == Files.Hash(planPath) && publicFixture.GetProperty("combatContractSha256").GetString() == combatContractSha &&
                    publicFixture.GetProperty("combatSeedProof").ValueKind == JsonValueKind.Object, "native_melee_public_seed_proof_mismatch");
                maximumHp = publicFixture.GetProperty("maximumHp").GetInt32();
                Guard.Need(actors.All(x => x.Hp == maximumHp && x.LastUserInformation?.Packet is S.UserInformation user && user.Magics.Count == 0 && !user.HasHero && !user.CreatureSummoned && user.IntelligentCreatures.Count == 0),
                    "native_melee_initial_full_hp_or_no_skills_pets_proof_failed");
                foreach (var actor in actors) { attackModes.Add(await actor.SetAttackAll(2500)); actor.RequireNoCombatBuffs(); }
                foreach (var actor in actors)
                {
                    Guard.Need(actor.VisiblePlayerIds().SequenceEqual(plan.Actors.Single(x => x.Ordinal == actor.Ordinal).PeerOrdinals.Select(i => actors.Single(x => x.Ordinal == i).ObjectId).Order()),
                        "native_melee_visible_player_set_differs_from_planned_group");
                    var full = (S.UserInformation)actor.LastUserInformation!.Packet;
                    combatBaselines.Add(new { actorOrdinal = actor.Ordinal, connection = actor.Connection, objectId = actor.ObjectId, position = actor.Location,
                        maximumHp, actualHp = actor.Hp, initialUserInformationReceiveSequence = actor.LastUserInformation.Sequence,
                        initialUserInformationAtMs = actor.LastUserInformation.At, magicCount = full.Magics.Count, full.HasHero, full.CreatureSummoned,
                        intelligentCreatureCount = full.IntelligentCreatures.Count, activeOwnedBuffCount = actor.OwnedBuffCount,
                        visiblePlayerIds = actor.VisiblePlayerIds(), publicNativePreparedSeed = publicFixture.GetProperty("combatSeedProof").Clone() });
                }
            }
            actorRoster = actors.Select(a => (object)new { actorOrdinal = a.Ordinal, connection = a.Connection, sharedObjectId = a.ObjectId, mapIndex = a.MapIndex, position = a.Location,
                expectedPeerOrdinals = plan.Actors.Single(x => x.Ordinal == a.Ordinal).PeerOrdinals }).ToArray();
            var slots = actors.ToDictionary(x => x.Ordinal, _ => new ActorSlot()); var byOrdinal = actors.ToDictionary(x => x.Ordinal);
            start = Time.Now + 500; await Time.Until(start.Value); var nextPopulation = start.Value;
            trafficStart = actors.Select(x => x.ApplicationCounters()).ToArray();
            events.Add(new { kind = "steadyTrafficStart", measurementAnchorMs = start, counters = trafficStart });
            if (melee) { potions = new ReactivePotions(actors, slots, maximumHp, events, start.Value + plan.DurationMs, measurement.Token); potions.Start(); }
            foreach (var action in plan.Actions.OrderBy(x => x.DueMs).ThenBy(x => x.ActorOrdinal).ThenBy(x => x.TicketSequence))
            {
                await Time.Until(start.Value + action.DueMs);
                if (File.Exists(stopFile)) throw new InvalidDataException("external_owned_stop_marker");
                Guard.Need(events.Complete, "fatal_evidence_budget_or_write_failure");
                Guard.Need(actors.All(x => x.Connected && x.InGame && x.Hp > 0 && x.Failure is null), "native_population_or_transport_failure");
                var now = Time.Now; var actor = byOrdinal[action.ActorOrdinal]; var actorPlan = plan.Actors.Single(x => x.Ordinal == action.ActorOrdinal);
                var result = new ActionResult { ActorOrdinal = action.ActorOrdinal, TicketSequence = action.TicketSequence, Kind = action.Kind, DueMs = action.DueMs, DeadlineMs = action.DeadlineMs,
                    PlannedAtMs = start.Value + action.DueMs, DeadlineAtMs = start.Value + action.DeadlineMs, OfferedAtMs = now, LatenessMs = now - start.Value - action.DueMs,
                    Target = action.Kind == "attack" ? plan.Actors.Single(x => x.Ordinal == action.TargetActorOrdinal!.Value).Start : action.Target,
                    Direction = action.Kind == "attack" ? Functions.DirectionFromPoint(actorPlan.Start.Point, plan.Actors.Single(x => x.Ordinal == action.TargetActorOrdinal!.Value).Start.Point).ToString() : action.Direction,
                    TargetActorOrdinal = action.TargetActorOrdinal, ExpectedTargetSharedId = action.Kind == "attack" ? byOrdinal[action.TargetActorOrdinal!.Value].ObjectId : null,
                    NonceHash = action.NonceSha256, ExpectedPeerOrdinals = actorPlan.PeerOrdinals, ExpectedSharedId = actor.ObjectId };
                rows.Add(result); events.Add(new { kind = "offered", actorOrdinal = actor.Ordinal, action = action.Kind, ticketSequence = action.TicketSequence, plannedAtMs = result.PlannedAtMs,
                    deadlineAtMs = result.DeadlineAtMs, offeredAtMs = now, atUtc = Time.Utc(now), target = result.Target, direction = result.Direction, nonceHash = result.NonceHash, expectedSharedId = actor.ObjectId, expectedPeerOrdinals = actorPlan.PeerOrdinals,
                    targetActorOrdinal = action.TargetActorOrdinal, expectedTargetSharedId = result.ExpectedTargetSharedId });
                var slot = slots[actor.Ordinal];
                lock (slot.Gate)
                {
                    if (slot.Pending is not null || slot.Potion is not null || now > result.DeadlineAtMs)
                    {
                        result.NotSent = true; result.Reason = slot.Pending is not null ? "previous_action_unresolved" : slot.Potion is not null ? "ordinary_potion_evidence_pending" : "planned_deadline_already_expired";
                        events.Add(new { kind = "notSent", result }); events.Add(new { kind = "actionOutcome", result = result.FreezeOutcome() });
                    }
                    else
                    {
                        IPendingPrimary operation = action.Kind == "attack"
                            ? new PendingCombatAction(actor, byOrdinal[action.TargetActorOrdinal!.Value], actorPlan.Start, actorPlan.PeerOrdinals.Select(x => byOrdinal[x]).ToArray(), action, events, slot, result, measurement.Token, start.Value + plan.DurationMs + 2500)
                            : new PendingAction(actor, actorPlan.PeerOrdinals.Select(x => byOrdinal[x]).ToArray(), action, events, slot, result, measurement.Token, start.Value + plan.Actions.Max(x => x.DeadlineMs));
                        slot.Pending = operation; pending.Add(operation); operation.Start();
                    }
                }
                if (now >= nextPopulation)
                { var sample = new { monotonicMs = now, controlledConnected = actors.Count(x => x.Connected), controlledInWorld = actors.Count(x => x.InGame), alive = actors.Count(x => x.Hp > 0) }; population.Add(sample); events.Add(new { kind = "population", sample }); nextPopulation = now + 1000; }
            }
            await Time.Until(start.Value + plan.DurationMs); end = Time.Now;
            trafficEnd = actors.Select(x => x.ApplicationCounters()).ToArray();
            events.Add(new { kind = "steadyTrafficEnd", plannedMeasurementEndMs = start + plan.DurationMs, counters = trafficEnd });
            if (potions is not null) { potions.StopIssuing(); if (potions.IssuingTask is not null) await potions.IssuingTask; }
            // Evidence-only drain. Late results can close unique pending slots but never change the original action's success.
            var cap = melee ? start.Value + plan.DurationMs + 2500 : start.Value + plan.Actions.Max(x => x.DeadlineMs);
            Guard.Need(cap <= start.Value + plan.DurationMs + 2500, "original_deadline_tail_exceeds_common_limit");
            var all = Task.WhenAll(pending.Select(x => x.Task!).Concat(potions?.PendingTasks ?? Array.Empty<Task>()));
            await Task.WhenAny(all, Time.Until(cap)); measurement.Cancel(); await all;
            var frozenRows = rows.Select(x => JsonSerializer.Deserialize<ActionResult>(x.FreezeOutcome(), Files.Json)!).ToArray();
            windows = Windows(plan, frozenRows);
            foreach (var result in windows) events.Add(new { kind = "minuteAssessment", assessedAtMs = Time.Now, method = "once from immutable original outcomes after all original deadlines", result });
            measurementPassed = rows.Count == plan.Actions.Length && windows.All(x => JsonSerializer.SerializeToElement(x, Files.Compact).GetProperty("passed").GetBoolean()) && events.Complete
                && !rows.Any(x => x.CensoredAtObservationEnd || x.SendOutcomeUnknown || x.SentAtMs is not null && !x.TerminalObserved) && actors.All(x => x.Failure is null && x.Hp > 0);
        }
        catch (Exception error)
        {
            failure = (error is InvalidDataException or IOException) && System.Text.RegularExpressions.Regex.IsMatch(error.Message, "^[A-Za-z0-9_.-]{1,160}$")
                ? error.Message : "native_runner_failure_" + error.GetType().Name;
        }
        finally
        {
            if (potions is not null) { potions.StopIssuing(); if (potions.IssuingTask is not null) await potions.IssuingTask; }
            if (trafficStart.Length != 0 && trafficEnd.Length == 0)
            {
                trafficEnd = actors.Select(x => x.ApplicationCounters()).ToArray();
                events.Add(new { kind = "steadyTrafficAbortedEnd", counters = trafficEnd });
            }
            measurement.Cancel(); await Task.WhenAll(pending.Where(x => x.Task is not null).Select(x => x.Task!).Concat(potions?.PendingTasks ?? Array.Empty<Task>()));
            foreach (var actor in actors) finalStates[actor.Ordinal] = actor.State();
            if (actors.Any(x => x.InGame)) await Task.Delay(plan.SettleBeforeLogoutMs);
            foreach (var actor in actors)
            {
                var normal = false;
                try
                {
                    var logout = await actor.Logout(plan.Thresholds.LogoutAckDeadlineMs);
                    normal = JsonSerializer.SerializeToElement(logout, Files.Compact).GetProperty("normalLogout").GetBoolean(); cleanup.Add(logout);
                    finalStates[actor.Ordinal] = actor.LastSuccessfulLogoutState is { } saved ? saved : actor.State();
                }
                catch { cleanup.Add(new { actorOrdinal = actor.Ordinal, connection = actor.Connection, normalLogout = false, reason = "native_logout_failed_or_deadline" }); }
                try { transportCloses.Add(await actor.Close(plan.Thresholds.TransportCloseDeadlineMs)); }
                catch { transportCloses.Add(new { actorOrdinal = actor.Ordinal, connection = actor.Connection, localTcpCloseCompleted = false }); normal = false; failure ??= "native_transport_close_deadline_or_failure"; }
                if (!normal || !finalReentryOrdinals.Contains(actor.Ordinal)) continue;
                NativeActor? fresh = null;
                try
                {
                    fresh = new NativeActor(actor.Identity, actor.Connection + 1, events); connections.Add(fresh); await fresh.Connect(fixture.Endpoint, MD5.HashData(await File.ReadAllBytesAsync(fixture.VersionFile)));
                    var saved = actor.LastSuccessfulLogoutState ?? throw new InvalidDataException("normal_logout_frozen_native_state_missing");
                    var expectedLastFingerprint = melee ? saved.GetProperty("itemState").GetProperty("fingerprintSha256").GetString() : baseline[actor.Ordinal];
                    var position = fresh.Location == actor.Location; var items = fresh.ItemFingerprint == expectedLastFingerprint;
                    var fullUser = (S.UserInformation)fresh.LastUserInformation!.Packet;
                    var vitals = fullUser.HP == saved.GetProperty("hp").GetInt32() && fullUser.MP == saved.GetProperty("mp").GetInt32();
                    var secondLogout = await fresh.Logout(plan.Thresholds.LogoutAckDeadlineMs);
                    var normalAgain = JsonSerializer.SerializeToElement(secondLogout, Files.Compact).GetProperty("normalLogout").GetBoolean();
                    var freshClosed = await fresh.Close(plan.Thresholds.TransportCloseDeadlineMs);
                    reentries.Add(new { actorOrdinal = actor.Ordinal, freshTcpConnection = true, connection = fresh.Connection, nativeUserInformationObserved = true, positionPreserved = position, inventoryEquipmentFieldsPreserved = items,
                        hpPositive = fresh.Hp > 0, initialUserInformationVitalsPreserved = vitals, expectedLastHp = saved.GetProperty("hp").GetInt32(), expectedLastMp = saved.GetProperty("mp").GetInt32(),
                        userInformationReceiveSequence = fresh.LastUserInformation.Sequence, userInformationReceivedAtMs = fresh.LastUserInformation.At,
                        normalLogout = normalAgain, logout = secondLogout, transportClosed = freshClosed,
                        expectedFingerprintSource = melee ? "last native acknowledged owned state before TCP close; includes ordinary durability and stock changes" : "initial native full owned state", expectedLastFingerprint,
                        passed = position && items && vitals && fresh.Hp > 0 && normalAgain, state = fresh.State() });
                }
                catch { reentries.Add(new { actorOrdinal = actor.Ordinal, freshTcpConnection = true, passed = false, reason = "native_fresh_tcp_reentry_or_state_failed" }); }
                finally { if (fresh is not null) await fresh.DisposeAsync(); }
            }
            potions?.Dispose();
        }
        events.Seal(); // Windows File.OpenRead must not race a live write handle with incompatible sharing flags.
        var totalTraffic = connections.Select(x => x.ApplicationCounters()).ToArray();
        var lifecyclePassed = cleanup.Count == plan.Population && cleanup.All(x => JsonSerializer.SerializeToElement(x, Files.Compact).GetProperty("normalLogout").GetBoolean()) && reentries.Count == finalReentryOrdinals.Length
            && reentries.All(x => JsonSerializer.SerializeToElement(x, Files.Compact).GetProperty("passed").GetBoolean()) && transportCloses.Count == plan.Population
            && transportCloses.All(x => JsonSerializer.SerializeToElement(x, Files.Compact).GetProperty("localTcpCloseCompleted").GetBoolean());
        var ok = failure is null && measurementPassed && lifecyclePassed && events.Complete;
        Files.Write(reportFile, new { schema = "mir2.comparable.crystal-report.v1", processId = Environment.ProcessId, planSha256 = Files.Hash(planPath), plan.PlanId, plan.Scenario, plan.Purpose, plan.QualificationLayer, plan.Population, plan.DurationMs,
            sourceLockSha256 = Files.VerifySourceLock(options.Required("source-lock")), adapterSha256 = Files.Hash(actualAssembly), serverReadySha256 = Files.Hash(options.Required("server-ready")), frozenSharedSha256 = Files.SharedSha,
            measurementStartedAtMs = start, measurementStartedAtUtc = start is null ? (DateTimeOffset?)null : Time.Utc(start.Value), measurementEndedAtMs = end,
            measurementCompleted = end is not null, measurementPassed, lifecyclePassed, ok, failure, capacityQualifiedByAdapterAlone = false,
            offered = rows.Count, completed = rows.Count(x => x.CompletedOnTime), timeouts = rows.Count(x => x.Timeout), notSent = rows.Count(x => x.NotSent), rows = rows.Select(x => x.FreezeOutcome()).ToArray(),
            finalLateState = rows.Select(x => new { x.ActorOrdinal, x.TicketSequence, x.Timeout, x.LateTerminal, x.TerminalObserved, x.CensoredAtObservationEnd, received = x.Received, x.LateCombatEvidence }).ToArray(), windows, actorRoster, initialStates, populationSamples = population, equipment, cleanup, transportCloses, finalReentryOrdinals,
            finalReentryRequiredCount = finalReentryOrdinals.Length, allActorNativeDiskReadbackRequired = true, reentries, finalStates,
            steadyApplicationTraffic = ApplicationTraffic.Steady(trafficStart, trafficEnd, plan.Population, start, plan.DurationMs, end is not null),
            totalApplicationTraffic = new { scope = ApplicationTraffic.Scope, counterSemantics = ApplicationTraffic.Semantics, includesBootstrapEquipmentSteadyCleanupAndAllFreshConnections = true,
                connections = totalTraffic, bytesSent = totalTraffic.Sum(x => x.BytesSent), bytesReceived = totalTraffic.Sum(x => x.BytesReceived) },
            eventRows = events.Rows, evidenceComplete = events.Complete, evidenceBudgetExceeded = events.Exceeded, evidenceWriteFailed = events.WriteFailed,
            recordsRejectedAfterFatalEvidenceFailure = events.RejectedAfterFatalFailure, eventsSha256 = Files.Hash(eventsFile), originalNativeLoadAccountsReadbackStillRequired = true,
            combatContractSha256 = combatContractSha, combatBaselines, attackModes, potions = potions?.Results.ToArray() ?? Array.Empty<object>(),
            potionPollingMs = melee ? 25 : (int?)null, potionHpIncreaseIsUniqueCausationClaimed = false,
            lateObservationCapExtraMs = 0, originalDeadlineTailMaximumMs = 2500, lateReceiptCanRestoreSuccess = false, combatProfileImplemented = true });
        Console.WriteLine(JsonSerializer.Serialize(new { report = reportFile, sha256 = Files.Hash(reportFile), ok, measurementPassed, lifecyclePassed, capacityQualifiedByAdapterAlone = false }, Files.Compact));
        return ok ? 0 : 1;
    }
}
