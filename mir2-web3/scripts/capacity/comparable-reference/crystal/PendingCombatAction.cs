using System.Text.Json;
using C = ClientPackets;
using S = ServerPackets;

namespace ComparableCrystal;

internal sealed class PendingCombatAction(NativeActor actor, NativeActor target, Cell plannedOrigin, NativeActor[] peers, PlanAction action,
    Evidence evidence, ActorSlot slot, ActionResult result, CancellationToken observationEnd, double observationCapAtMs) : IPendingPrimary
{
    private readonly object gate = new();
    private readonly List<(Receipt Receipt, int HpBefore)> buffered = new();
    private readonly List<ReceiptTap> subscriptions = new();
    private readonly TaskCompletionSource<bool> signal = new(TaskCreationOptions.RunContinuationsAsynchronously);
    private NativeCombat? combat;
    public Task? Task { get; private set; }
    public void Start() => Task = Execute();
    private void Observe(Receipt receipt, int hpBefore)
    {
        if (receipt.Packet is not (S.UserLocation or S.ObjectAttack or S.ObjectRangeAttack or S.Struck or S.ObjectStruck or S.DamageIndicator or S.HealthChanged or S.Death or S.ObjectDied or S.ObjectRemove)) return;
        lock (gate)
        {
            if (combat is null)
            {
                if (buffered.Count >= 4096) { actor.StopGameplay("native_combat_presend_capture_bound_exceeded"); signal.TrySetResult(true); return; }
                buffered.Add((receipt, hpBefore)); return;
            }
            combat.Observe(receipt, hpBefore);
            if (combat.InvalidReason is not null) { actor.StopGameplay(combat.InvalidReason); signal.TrySetResult(true); }
            else if (combat.Complete) signal.TrySetResult(true);
        }
    }
    private void Outcome() => evidence.Add(new { kind = "actionOutcome", result = result.FreezeOutcome() });
    private async Task Execute()
    {
        try
        {
            Guard.Need(actor.InGame && target.InGame && actor.Hp > 0 && target.Hp > 0 && actor.Failure is null && target.Failure is null,
                "native_melee_participant_not_live");
            Guard.Need(action.TargetActorOrdinal == target.Ordinal && actor.Location == plannedOrigin && target.Location == result.Target &&
                actor.MapIndex == target.MapIndex && actor.Location.Distance(target.Location) == 1 &&
                peers.All(p => p.MapIndex == actor.MapIndex && p.InGame && p.Hp > 0 && actor.Sees(p.ObjectId) && p.Sees(actor.ObjectId) && p.Location.Distance(actor.Location) <= 12),
                "native_melee_planned_target_not_adjacent_and_visible");
            var origin = actor.Location; var targetPosition = target.Location; var direction = Functions.DirectionFromPoint(origin.Point, targetPosition.Point);
            result.Origin = origin; result.ExpectedSharedId = actor.ObjectId; result.ExpectedTargetSharedId = target.ObjectId;
            var receivers = new[] { actor }.Concat(peers).ToArray();
            foreach (var receiver in receivers)
            {
                result.ReceiveSequenceFloors[receiver.Ordinal] = receiver.Cursor;
                subscriptions.Add(receiver.Tap(Observe));
            }
            var sent = await actor.Send(new C.Attack { Direction = direction, Spell = Spell.None }, result.DeadlineAtMs, observationEnd,
                new { action = "attack", action.TicketSequence, targetActorOrdinal = target.Ordinal, expectedTargetSharedId = target.ObjectId,
                    targetPosition, origin, direction = (int)direction, spell = (int)Spell.None });
            result.SentAtMs = sent.At; result.SentSequence = sent.Sequence; result.LatenessMs = sent.At - result.PlannedAtMs;
            lock (gate)
            {
                combat = new NativeCombat(new(action.TicketSequence, actor.Ordinal, actor.ObjectId, target.Ordinal, target.ObjectId,
                    origin, targetPosition, direction, sent.At, result.DeadlineAtMs,
                    receivers.ToDictionary(x => x.Ordinal, x => new NativeCombatObserver(x.Connection, result.ReceiveSequenceFloors[x.Ordinal]))));
                foreach (var row in buffered) combat.Observe(row.Receipt, row.HpBefore);
                buffered.Clear();
                if (combat.InvalidReason is not null) { actor.StopGameplay(combat.InvalidReason); signal.TrySetResult(true); }
                else if (combat.Complete) signal.TrySetResult(true);
            }
            evidence.Add(new { kind = "sent", actorOrdinal = actor.Ordinal, ticketSequence = action.TicketSequence, action = "attack",
                sentSequence = sent.Sequence, sentAtMs = sent.At, plannedAtMs = result.PlannedAtMs, deadlineAtMs = result.DeadlineAtMs,
                origin, target = targetPosition, direction = direction.ToString(), expectedSharedId = actor.ObjectId,
                expectedTargetSharedId = target.ObjectId, targetActorOrdinal = target.Ordinal,
                expectedPeerOrdinals = result.ExpectedPeerOrdinals, receiveSequenceFloors = result.ReceiveSequenceFloors });
            await System.Threading.Tasks.Task.WhenAny(signal.Task, System.Threading.Tasks.Task.Delay(
                (int)Math.Ceiling(Math.Max(0, result.DeadlineAtMs - Time.Now)), observationEnd));
            observationEnd.ThrowIfCancellationRequested();
            lock (gate)
            {
                Guard.Need(combat.InvalidReason is null, combat.InvalidReason ?? "native_combat_causal_failure");
                if (combat.Complete && combat.CompletionTimes().All(x => x <= result.DeadlineAtMs))
                {
                    result.TerminalObserved = true; result.CombatEvidence = combat.FreezeOutcome();
                    result.CompleteOnTime(combat.CompletionTimes(), peers.Length + 2); Outcome(); return;
                }
                result.Timeout = true; result.Reason = "native_attack_evidence_incomplete_by_original_absolute_deadline";
                result.CombatEvidence = combat.FreezeOutcome(); Outcome();
                evidence.Add(new { kind = "timeout", actorOrdinal = actor.Ordinal, action = "attack", ticketSequence = action.TicketSequence,
                    deadlineAtMs = result.DeadlineAtMs, observedAtMs = Time.Now });
            }
            await signal.Task.WaitAsync(observationEnd);
            lock (gate)
            {
                Guard.Need(combat.InvalidReason is null, combat.InvalidReason ?? "native_combat_causal_failure");
                var late = combat.SnapshotEvidence(); result.LateCombatEvidence = late;
                if (combat.Complete && combat.CompletionTimes().All(x => x <= observationCapAtMs))
                {
                    result.TerminalObserved = true; result.LateTerminal = true;
                    evidence.Add(new { kind = "lateClosure", actorOrdinal = actor.Ordinal, action = "attack", ticketSequence = action.TicketSequence,
                        originalTimeoutRemainsFailure = true, combatEvidence = late, closedAtMs = Time.Now });
                }
                else result.CensoredAtObservationEnd = true;
            }
        }
        catch (OperationCanceledException) when (observationEnd.IsCancellationRequested)
        {
            result.CensoredAtObservationEnd = true; result.Reason = "native_attack_observation_ended";
            if (result.SentAtMs is null) result.NotSent = true;
            if (!result.Timeout) Outcome();
        }
        catch (Exception error)
        {
            result.Reason = error is InvalidDataException ? error.Message : "native_attack_transport_or_evidence_failure";
            if (error is NativeWriteUncertain unknown)
            { result.SendOutcomeUnknown = true; result.SentAtMs = unknown.Attempted.At; result.SentSequence = unknown.Attempted.Sequence; }
            else if (result.SentAtMs is null) result.NotSent = true;
            lock (gate) if (combat is not null) result.CombatEvidence = combat.SnapshotEvidence();
            if (result.SentAtMs is not null) actor.StopGameplay(result.Reason);
            if (!result.Timeout) Outcome();
        }
        finally
        {
            foreach (var subscription in subscriptions) subscription.Dispose();
            lock (slot.Gate) if (ReferenceEquals(slot.Pending, this)) slot.Pending = null;
        }
    }
}
