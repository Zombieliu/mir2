using System.Collections.Concurrent;

namespace ComparableCrystal;

internal sealed class ReactivePotion(NativeActor actor, ActorSlot slot, int maximumHp, Evidence evidence,
    ConcurrentQueue<object> results, CancellationToken observationEnd)
{
    public Task? Task { get; private set; }
    public void Start() => Task = Execute();
    private async Task Execute()
    {
        var issued = false;
        try
        {
            // A heal arriving between the scheduler decision and this call cancels this unissued decision.
            if (actor.Hp > maximumHp * .7) return;
            var result = await actor.UsePotion(maximumHp, at => { issued = true; lock (slot.Gate) slot.LastPotionSentAt = at; }, observationEnd);
            results.Enqueue(result); evidence.Add(new { kind = "potion", result });
        }
        catch (Exception error)
        {
            if (!issued && actor.Hp > maximumHp * .7 && error is InvalidDataException { Message: "native_reactive_potion_trigger_or_pending_invalid" }) return;
            actor.StopGameplay("native_reactive_potion_evidence_failed_no_retry");
            var result = new { actorOrdinal = actor.Ordinal, connection = actor.Connection, success = false,
                issued = issued || error is NativeWriteUncertain, sendOutcomeUnknown = error is NativeWriteUncertain,
                attemptedAtMs = error is NativeWriteUncertain uncertain ? uncertain.Attempted.At : (double?)null,
                reason = "native_reactive_potion_evidence_failed_no_retry", exceptionType = error.GetType().Name, atMs = Time.Now };
            results.Enqueue(result); evidence.Add(new { kind = "potionFailure", result });
        }
        finally { lock (slot.Gate) if (ReferenceEquals(slot.Potion, this)) slot.Potion = null; }
    }
}

internal sealed class ReactivePotions(List<NativeActor> actors, Dictionary<int, ActorSlot> slots, int maximumHp,
    Evidence evidence, double endsAtMs, CancellationToken observationEnd) : IDisposable
{
    private readonly CancellationTokenSource issuing = new();
    private readonly ConcurrentQueue<ReactivePotion> operations = new();
    public ConcurrentQueue<object> Results { get; } = new();
    public Task? IssuingTask { get; private set; }
    public Task[] PendingTasks => operations.Where(x => x.Task is not null).Select(x => x.Task!).ToArray();
    public void Start() => IssuingTask = Run();
    public void StopIssuing() => issuing.Cancel();
    private async Task Run()
    {
        try
        {
            while (Time.Now < endsAtMs && !issuing.IsCancellationRequested && !observationEnd.IsCancellationRequested)
            {
                foreach (var actor in actors)
                {
                    if (Time.Now >= endsAtMs || issuing.IsCancellationRequested) break;
                    var slot = slots[actor.Ordinal];
                    lock (slot.Gate)
                    {
                        if (slot.Pending is not null || slot.Potion is not null || actor.Failure is not null || !actor.InGame || actor.Hp <= 0 ||
                            actor.Hp > maximumHp * .7 || Time.Now - slot.LastPotionSentAt < 2000) continue;
                        var operation = new ReactivePotion(actor, slot, maximumHp, evidence, Results, observationEnd);
                        slot.Potion = operation; operations.Enqueue(operation); operation.Start();
                    }
                }
                await Task.Delay(25, issuing.Token);
            }
        }
        catch (OperationCanceledException) when (issuing.IsCancellationRequested) { }
    }
    public void Dispose() { issuing.Cancel(); issuing.Dispose(); }
}
