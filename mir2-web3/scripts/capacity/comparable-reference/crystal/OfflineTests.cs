namespace ComparableCrystal;

internal static class OfflineTests
{
    public static int Run()
    {
        var passed = new List<string>();
        Check("server_shared_walk_wire_compatibility", () =>
        {
            Packet.IsServer = true; var packet = new ClientPackets.Walk { Direction = MirDirection.Right };
            var decoded = Packet.ReceivePacket(packet.GetPacketBytes().ToArray(), out var extra);
            Guard.Need(decoded is ClientPackets.Walk p && p.Direction == MirDirection.Right && extra.Length == 0, "client_walk_roundtrip_failed");
        });
        Check("server_shared_chat_wire_compatibility", () =>
        {
            Packet.IsServer = false; var packet = new ServerPackets.ObjectChat { ObjectID = 50001, Text = "fixture: m2cmp:synthetic:0:1", Type = ChatType.Normal };
            var decoded = Packet.ReceivePacket(packet.GetPacketBytes().ToArray(), out var extra);
            Guard.Need(decoded is ServerPackets.ObjectChat p && p.ObjectID == 50001 && p.Text.EndsWith("m2cmp:synthetic:0:1", StringComparison.Ordinal) && extra.Length == 0, "server_chat_roundtrip_failed");
        });
        Check("server_shared_equipment_wire_compatibility", () =>
        {
            Packet.IsServer = false; var packet = new ServerPackets.EquipItem { Grid = MirGridType.Inventory, UniqueID = 17, To = 0, Success = true };
            var decoded = Packet.ReceivePacket(packet.GetPacketBytes().ToArray(), out var extra);
            Guard.Need(decoded is ServerPackets.EquipItem p && p.UniqueID == 17 && p.Success && extra.Length == 0, "equipment_roundtrip_failed");
        });
        Check("state_path_cannot_escape_assigned_root", () =>
        {
            var rejected = false; try { Files.StatePath(Path.GetFullPath(Path.Combine(Files.StateBase, "..", "outside.json"))); } catch (InvalidDataException) { rejected = true; }
            Guard.Need(rejected, "state_path_escape_accepted");
        });
        Check("chat_matches_exact_nonce_and_shared_id", () =>
        {
            var nonce = "m2cmp:synthetic:0:1";
            Guard.Need(PendingAction.ExactNonce(new ServerPackets.ObjectChat { ObjectID = 51, Text = "fixture: " + nonce }, 51, nonce), "valid_nonce_not_matched");
            Guard.Need(!PendingAction.ExactNonce(new ServerPackets.ObjectChat { ObjectID = 52, Text = "fixture: " + nonce }, 51, nonce), "wrong_shared_id_matched");
            Guard.Need(!PendingAction.ExactNonce(new ServerPackets.ObjectChat { ObjectID = 51, Text = "fixture: " + nonce + "extra" }, 51, nonce), "nonce_prefix_matched");
            Guard.Need(!PendingAction.ExactNonce(new ServerPackets.ObjectChat { ObjectID = 51, Text = "prefix" + nonce }, 51, nonce), "nonce_embedded_without_boundary_matched");
        });
        Check("late_completion_cannot_restore_timeout", () =>
        {
            var row = Row(); row.Timeout = true; var frozen = row.FreezeOutcome(); row.CompleteOnTime(new[] { 1001d, 1002d }, 2); row.LateTerminal = true;
            Guard.Need(!row.CompletedOnTime && frozen.GetProperty("timeout").GetBoolean() && !row.FreezeOutcome().GetProperty("lateTerminal").GetBoolean(), "timeout_was_rewritten");
        });
        Check("missing_or_late_observer_cannot_complete", () =>
        {
            var row = Row(); row.CompleteOnTime(new[] { 1001d }, 2); Guard.Need(!row.CompletedOnTime, "missing_observer_accepted");
            row.CompleteOnTime(new[] { 1001d, 3001d }, 2); Guard.Need(!row.CompletedOnTime, "late_observer_accepted");
            row.CompleteOnTime(new[] { 1001d, 1002d }, 2); Guard.Need(row.CompletedOnTime && row.LatencyMs == 2, "valid_observer_completion_failed");
        });
        Check("not_sent_and_rejected_are_not_success", () =>
        {
            var unsent = Row(); unsent.NotSent = true; unsent.CompleteOnTime(new[] { 1001d, 1002d }, 2);
            var rejected = Row(); rejected.Rejected = true; rejected.CompleteOnTime(new[] { 1001d, 1002d }, 2);
            Guard.Need(!unsent.CompletedOnTime && !unsent.Timeout && !rejected.CompletedOnTime, "failed_action_completed");
        });
        Check("fresh_connection_and_sequence_floor_are_required", () =>
        {
            using var watch = new PacketWatch(9, r => r.ActorOrdinal == 0 && r.Connection == 2 && r.Packet is ServerPackets.UserLocation, _ => { });
            Guard.Need(!watch.Consider(new Receipt(0, 1, 10, 1001, new ServerPackets.UserLocation())), "old_connection_receipt_accepted");
            Guard.Need(!watch.Consider(new Receipt(0, 2, 9, 1001, new ServerPackets.UserLocation())), "old_sequence_receipt_accepted");
            Guard.Need(watch.Consider(new Receipt(0, 2, 10, 1001, new ServerPackets.UserLocation())), "fresh_receipt_rejected");
        });
        Check("wait_uses_packet_timestamp_not_late_poll_time", () =>
        {
            var root = Path.Combine(Files.StateBase, "offline-wait-" + Guid.NewGuid().ToString("N")); Directory.CreateDirectory(root);
            using var evidence = new Evidence(Path.Combine(root, "events.jsonl"), 10000);
            var actor = new NativeActor(new PrivateActor(0, "synthetic", "synthetic", "Synthetic", 1, 440, new Cell(1, 1), 0, "Other"), 1, evidence);
            var deadline = Time.Now - 5;
            using var onTime = actor.Watch<ServerPackets.UserLocation>(0); onTime.Signal.TrySetResult(new Receipt(0, 1, 1, deadline - 1, new ServerPackets.UserLocation()));
            Guard.Need(actor.Wait(onTime, deadline).GetAwaiter().GetResult().At == deadline - 1, "captured_in_budget_receipt_lost_to_late_poll");
            using var late = actor.Watch<ServerPackets.UserLocation>(0); late.Signal.TrySetResult(new Receipt(0, 1, 2, deadline + 1, new ServerPackets.UserLocation()));
            var refused = false; try { actor.Wait(late, deadline).GetAwaiter().GetResult(); } catch (TimeoutException) { refused = true; }
            Guard.Need(refused, "late_captured_receipt_accepted"); actor.DisposeAsync().GetAwaiter().GetResult();
        });
        Check("fingerprint_covers_original_item_fields_and_capacity", () =>
        {
            var item = new UserItem(new ItemInfo { Index = 221 }) { UniqueID = 17, Count = 1 };
            var inventory = new UserItem[46]; inventory[6] = item; var equipment = new UserItem[14]; var quest = new UserItem[40];
            var before = ItemState.Fingerprint(inventory, equipment, quest, 3, 4); item.Identified = !item.Identified;
            Guard.Need(before != ItemState.Fingerprint(inventory, equipment, quest, 3, 4), "nonpresentation_item_field_ignored"); item.Identified = !item.Identified;
            Guard.Need(before != ItemState.Fingerprint(inventory, equipment, quest, 4, 4), "gold_ignored");
            Guard.Need(before != ItemState.Fingerprint(new UserItem[47], equipment, quest, 3, 4), "capacity_ignored");
        });
        Check("sealed_live_evidence_can_hash_and_dispose_twice_on_Windows", () =>
        {
            var root = Path.Combine(Files.StateBase, "offline-evidence-" + Guid.NewGuid().ToString("N")); Directory.CreateDirectory(root);
            var path = Path.Combine(root, "events.jsonl"); using var writer = new Evidence(path, 10000);
            writer.Add(new { synthetic = true, kind = "fixture" }); writer.Flush(); writer.Seal();
            var actual = Files.Hash(path); writer.Dispose();
            Guard.Need(writer.Rows == 1 && writer.Complete && actual == Files.Hash(path) && actual.Length == 64, "sealed_evidence_hash_or_idempotent_dispose_failed");
        });
        Check("report_population_metadata_collision_reproduced_and_distinct_samples_serialize", () =>
        {
            var failed = false;
            try { System.Text.Json.JsonSerializer.Serialize(new { Population = 2, population = Array.Empty<object>() }, Files.Json); }
            catch (InvalidOperationException) { failed = true; }
            Guard.Need(failed, "old_population_property_collision_not_reproduced");
            var state = System.Text.Json.JsonSerializer.SerializeToElement(new { Population = 2, populationSamples = Array.Empty<object>() }, Files.Json);
            Guard.Need(state.GetProperty("population").GetInt32() == 2 && state.GetProperty("populationSamples").GetArrayLength() == 0, "population_report_and_samples_not_distinct");
        });
        Check("serialization_failure_does_not_create_empty_report", () =>
        {
            var path = Path.Combine(Files.StateBase, "offline-report-" + Guid.NewGuid().ToString("N"), "report.json"); var failed = false;
            try { Files.Write(path, new { Population = 2, population = Array.Empty<object>() }); }
            catch (InvalidOperationException) { failed = true; }
            Guard.Need(failed && !File.Exists(path), "failed_serialization_left_report_artifact");
        });
        Check("traffic_window_excludes_bootstrap_and_rejects_cross_connection_evidence", () =>
        {
            var start = new ApplicationCounterSnapshot(0, 5, 999, 1000, 2000, 7000);
            var end = new ApplicationCounterSnapshot(0, 5, 61000, 61001, 2134, 7456);
            var window = ApplicationTraffic.Window(start, end);
            Guard.Need(window.BytesSent == 134 && window.BytesReceived == 456 && window.Start.CounterAtMs == 1000 && window.End.CounterAtMs == 61001,
                "steady_bytes_include_bootstrap_or_lose_sample_time");
            var refused = false; try { ApplicationTraffic.Window(start, end with { Connection = 6 }); } catch (InvalidDataException) { refused = true; }
            Guard.Need(refused, "traffic_window_reused_other_connection");
        });
        NativeCombatTests.Run(Check);
        Console.WriteLine(System.Text.Json.JsonSerializer.Serialize(new { schema = "mir2.comparable.crystal-offline-tests.v1", passed, failed = 0, gameInstancesStarted = 0 })); return 0;
        void Check(string name, Action body) { body(); passed.Add(name); }
    }
    private static ActionResult Row() => new() { ActorOrdinal = 0, TicketSequence = 1, Kind = "walk", PlannedAtMs = 1000, DeadlineAtMs = 3000, SentAtMs = 1000, ExpectedPeerOrdinals = new[] { 1 } };
    public static int Examples(Options options)
    {
        var root = Files.StatePath(options.Required("output-directory")); Guard.Need(!Directory.Exists(root), "fresh_example_directory_required"); Directory.CreateDirectory(root);
        var sample = Row(); sample.Timeout = true; sample.Reason = "matching_receipts_not_complete_by_absolute_deadline";
        var frozen = sample.FreezeOutcome(); sample.LateTerminal = true;
        Files.Write(Path.Combine(root, "synthetic-report.example.json"), new { schema = "mir2.comparable.crystal-report.v1", processId = Environment.ProcessId, synthetic = true, notARealRun = true,
            planSha256 = (string?)null, measurementCompleted = false, measurementPassed = false, lifecyclePassed = false, ok = false, capacityQualifiedByAdapterAlone = false,
            offered = 1, completed = 0, timeouts = 1, notSent = 0, rows = new[] { frozen }, finalLateState = new[] { new { sample.ActorOrdinal, sample.TicketSequence, sample.Timeout, sample.LateTerminal } },
            windows = Array.Empty<object>(), note = "partial schema illustration only; never capacity evidence" });
        using (var events = new Evidence(Path.Combine(root, "synthetic-events.example.jsonl"), 10000))
        {
            events.Add(new { synthetic = true, kind = "offered", actorOrdinal = 0, action = "walk", ticketSequence = 1, plannedAtMs = 1000, deadlineAtMs = 3000, target = new Cell(56, 51), direction = "Right", expectedSharedId = 50001, expectedPeerOrdinals = new[] { 1 } });
            events.Add(new { synthetic = true, kind = "sent", actorOrdinal = 0, ticketSequence = 1, action = "walk", sentSequence = 7, sentAtMs = 1001, receiveSequenceFloors = new Dictionary<int, long> { [0] = 6, [1] = 4 } });
            events.Add(new { synthetic = true, kind = "ownerReceipt", actorOrdinal = 0, ticketSequence = 1, detail = new { receiverActorOrdinal = 0, connection = 1, receiveSequence = 8, receivedAtMs = 1100, onTime = true } });
            events.Add(new { synthetic = true, kind = "timeout", actorOrdinal = 0, ticketSequence = 1, deadlineAtMs = 3000, observedAtMs = 3001 });
            events.Add(new { synthetic = true, kind = "actionOutcome", result = frozen });
            events.Add(new { synthetic = true, kind = "peerReceipt", actorOrdinal = 0, ticketSequence = 1, detail = new { receiverActorOrdinal = 1, connection = 1, receiveSequence = 5, receivedAtMs = 3300, onTime = false } });
            events.Add(new { synthetic = true, kind = "lateClosure", actorOrdinal = 0, ticketSequence = 1, originalTimeoutRemainsFailure = true, closedAtMs = 3300 });
        }
        Console.WriteLine(System.Text.Json.JsonSerializer.Serialize(new { outputDirectory = root, synthetic = true, gameInstancesStarted = 0 })); return 0;
    }
}
