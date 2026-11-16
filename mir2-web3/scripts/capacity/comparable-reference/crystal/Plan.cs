using System.Text.Json;

namespace ComparableCrystal;

internal sealed record Cell(int X, int Y)
{
    internal System.Drawing.Point Point => new(X, Y);
    public static Cell Of(System.Drawing.Point value) => new(value.X, value.Y);
    public int Distance(Cell other) => Math.Max(Math.Abs(X - other.X), Math.Abs(Y - other.Y));
}
internal sealed record PlanMap(int Index, string FileName, int Width, int Height, string RawSha256);
internal sealed record ActorTemplate(string Class, int Level, string Gender);
internal sealed record PlanActor(int Ordinal, int Group, int[] PeerOrdinals, Cell Start, Cell[] Lane, int? AttackTargetOrdinal);
internal sealed record PlanAction(long TicketSequence, int ActorOrdinal, string Kind, double DueMs, double DeadlineMs,
    string? Direction, Cell? Target, string? Nonce, string? NonceSha256, int? TargetActorOrdinal = null);
internal sealed record Limits(double MinimumCompletionRatio, double MaximumTimeoutRatioExclusive, double MovementP95Ms,
    double ChatP95Ms, double ScheduleP95Ms, int WalkDeadlineMs, int ChatDeadlineMs, int EquipmentAckDeadlineMs,
    int EquipmentStateDeadlineMs, int LogoutAckDeadlineMs, int TransportCloseDeadlineMs, double AttackP95Ms = 750, int AttackDeadlineMs = 2500);
internal sealed record Plan(string Schema, string PlanId, string Scenario, string Purpose, string QualificationLayer, int Population,
    int DurationMs, PlanMap Map, string NavigationSha256, PlanActor[] Actors, PlanAction[] Actions, Limits Thresholds, ActorTemplate ActorTemplate,
    JsonElement ResourceContract, int SettleBeforeLogoutMs, int[]? EquipmentActorOrdinals, int EquipmentRoundsPerActor = 2, JsonElement? CombatContract = null,
    string? CombatContractSha256 = null, string? CombatContractCanonicalSha256 = null)
{
    public void Validate(bool movementOnly = true)
    {
        Guard.Need(Schema == "mir2.comparable.plan.v1" && Map.Index == 440 && Map.FileName == "DogYoHyun" && Map.Width == 800 && Map.Height == 800 && Map.RawSha256 == Files.MapSha, "plan_schema_or_original_map_mismatch");
        Guard.Need((Purpose == "functional" && Population == 2 && DurationMs == 60000) || (Purpose == "capacity" && new[] { 25, 50, 100 }.Contains(Population) && DurationMs == 300000), "plan_population_duration_mismatch");
        Guard.Need(Scenario is "dense-movement-chat" or "dispersed-melee", "unknown_scenario");
        if (Scenario == "dispersed-melee") CombatPolicy.RequireEmbedded(this);
        if (movementOnly) Guard.Need(Scenario == "dense-movement-chat" && Actions.All(x => x.Kind is "walk" or "chat"), "melee_execution_not_implemented_no_capacity_claim");
        Guard.Need(Actors.Length == Population && Actors.Select(x => x.Ordinal).SequenceEqual(Enumerable.Range(0, Population)), "actor_ordinals_mismatch");
        Guard.Need(ActorTemplate is { Class: "Warrior", Level: 40, Gender: "Male" }, "actor_template_mismatch");
        Guard.Need(Actors.Select(x => x.Start).Distinct().Count() == Population && NavigationSha256.Length == 64 && SettleBeforeLogoutMs == 11000, "layout_or_settle_mismatch");
        Guard.Need(EquipmentRoundsPerActor == 2 && EquipmentActorOrdinals is not null && EquipmentActorOrdinals.SequenceEqual(Enumerable.Range(0, Purpose == "functional" ? 2 : 3)), "fixed_equipment_calibration_set_required");
        Guard.Need(Actions.All(x => x.ActorOrdinal >= 0 && x.ActorOrdinal < Population), "action_actor_outside_population");
        Guard.Need(Actions.Where(x => x.Kind == "chat").Select(x => x.Nonce).Distinct().Count() == Actions.Count(x => x.Kind == "chat"), "chat_nonce_reuse_forbidden");
        if (Scenario == "dense-movement-chat") Guard.Need(Actors.SelectMany(x => x.Lane).Distinct().Count() == Population * 2, "ordinary_lanes_must_not_overlap");
        foreach (var actor in Actors)
        {
            Guard.Need(actor.PeerOrdinals.Length > 0 && actor.PeerOrdinals.Distinct().Count() == actor.PeerOrdinals.Length && actor.PeerOrdinals.All(i => i >= 0 && i < Population && i != actor.Ordinal && Actors.Single(x => x.Ordinal == i).Group == actor.Group), "declared_peers_invalid");
            Guard.Need(actor.Lane.Length == 2 && actor.Lane[0] == actor.Start && actor.Lane.All(p => p.X >= 0 && p.X < 800 && p.Y >= 0 && p.Y < 800), "lane_mismatch");
            if (Scenario == "dense-movement-chat")
            {
                Guard.Need(actor.Lane[0].Distance(actor.Lane[1]) == 1, "ordinary_one_step_lane_required");
                foreach (var peer in actor.PeerOrdinals.Select(i => Actors.Single(x => x.Ordinal == i)))
                    Guard.Need(actor.Lane.All(p => peer.Lane.All(q => p.Distance(q) <= 12)), "planned_witness_outside_margin");
            }
            else
            {
                Guard.Need(actor.Lane[0] == actor.Lane[1] && actor.AttackTargetOrdinal is { } target && actor.PeerOrdinals.Contains(target) &&
                    actor.Start.Distance(Actors.Single(x => x.Ordinal == target).Start) == 1 && Actors.Count(x => x.AttackTargetOrdinal == actor.Ordinal) == 1,
                    "melee_requires_stationary_adjacent_unique_incoming_target");
            }
            var rows = Actions.Where(x => x.ActorOrdinal == actor.Ordinal).OrderBy(x => x.DueMs).ThenBy(x => x.Kind, StringComparer.Ordinal).ToArray();
            Guard.Need(rows.Select(x => x.TicketSequence).SequenceEqual(Enumerable.Range(1, rows.Length).Select(i => (long)i)), "actor_ticket_sequence_invalid");
            Guard.Need(rows.All(x => x.DueMs >= 0 && x.DueMs < DurationMs && x.DeadlineMs > x.DueMs && x.DeadlineMs <= DurationMs + 15000), "absolute_action_deadline_invalid");
            foreach (var action in rows)
            {
                if (action.Kind == "walk")
                    Guard.Need(action.Target is not null && actor.Lane.Contains(action.Target) && Enum.TryParse<MirDirection>(action.Direction, out _) && action.DeadlineMs - action.DueMs == Thresholds.WalkDeadlineMs, "walk_shape_mismatch");
                else if (action.Kind == "chat")
                    Guard.Need(action.Nonce is not null && action.Nonce.Length is > 0 and <= 80 && !action.Nonce.Any(char.IsWhiteSpace) && action.NonceSha256 == Files.HashText(action.Nonce) && action.DeadlineMs - action.DueMs == Thresholds.ChatDeadlineMs, "chat_shape_mismatch");
                else if (action.Kind == "attack")
                    Guard.Need(Scenario == "dispersed-melee" && action.TargetActorOrdinal == actor.AttackTargetOrdinal && action.DeadlineMs - action.DueMs == Thresholds.AttackDeadlineMs,
                        "ordinary_attack_target_or_deadline_mismatch");
                else throw new InvalidDataException("unsupported_action");
            }
            if (Scenario == "dense-movement-chat")
                for (var minute = 0; minute < DurationMs / 60000; minute++)
                {
                    var slice = rows.Where(x => x.DueMs >= minute * 60000 && x.DueMs < (minute + 1) * 60000).ToArray();
                    Guard.Need(slice.Count(x => x.Kind == "walk") == 30 && slice.Count(x => x.Kind == "chat") == 2, "fixed_minute_workload_required");
                }
            else
                for (var minute = 0; minute < DurationMs / 60000; minute++)
                {
                    var slice = rows.Where(x => x.DueMs >= minute * 60000 && x.DueMs < (minute + 1) * 60000).ToArray();
                    Guard.Need(slice.Count(x => x.Kind == "attack") == 50 && slice.Count(x => x.Kind == "chat") == 2 &&
                        slice.Where(x => x.Kind == "attack").Zip(slice.Where(x => x.Kind == "attack").Skip(1)).All(pair => pair.Second.DueMs - pair.First.DueMs == 1200),
                        "fixed_melee_minute_workload_required");
                }
        }
        Guard.Need(Actions.Select(x => (x.ActorOrdinal, x.TicketSequence)).Distinct().Count() == Actions.Length, "duplicate_action_ticket");
        Guard.Need(Thresholds.MinimumCompletionRatio == .95 && Thresholds.MaximumTimeoutRatioExclusive == .01 && Thresholds.WalkDeadlineMs == 2000 && Thresholds.ChatDeadlineMs == 2500 && Thresholds.LogoutAckDeadlineMs == 5000 && Thresholds.TransportCloseDeadlineMs == 5000 &&
            Thresholds.EquipmentAckDeadlineMs == 15000 && Thresholds.EquipmentStateDeadlineMs == 15000 && Thresholds.MovementP95Ms == 750 && Thresholds.ChatP95Ms == 1000 && Thresholds.ScheduleP95Ms == 250 &&
            Thresholds.AttackP95Ms == 750 && Thresholds.AttackDeadlineMs == 2500, "unreviewed_threshold_change");
    }
}
