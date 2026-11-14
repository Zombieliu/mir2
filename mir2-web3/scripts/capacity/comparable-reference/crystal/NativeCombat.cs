using System.Text.Json;
using S = ServerPackets;

namespace ComparableCrystal;

internal sealed record NativeCombatObserver(int Connection, long ReceiveSequenceFloor);
internal sealed record NativeCombatSpec(long TicketSequence, int ActorOrdinal, uint AttackerSharedId,
    int TargetActorOrdinal, uint TargetSharedId, Cell Origin, Cell TargetPosition, MirDirection Direction,
    double SentAtMs, double DeadlineAtMs, IReadOnlyDictionary<int, NativeCombatObserver> Observers);
internal sealed record NativeCombatFact(int ReceiverActorOrdinal, int Connection, long ReceiveSequence,
    double ReceivedAtMs, string Packet, uint? SourceSharedId = null, uint? VictimSharedId = null,
    int? Damage = null, int? HpBefore = null, int? HpAfter = null);

// Pure, not yet connected to a live send/receive loop. The caller must capture every relevant
// fresh packet before Send starts, preserve target-stream order and supply actual prior HP.
// No request ID exists on native Attack: one unresolved ticket is a mandatory outer invariant.
internal sealed class NativeCombat
{
    private readonly NativeCombatSpec spec;
    private readonly Dictionary<int, NativeCombatFact> animations = new();
    private readonly List<NativeCombatFact> facts = new();
    private NativeCombatFact? owner, struck, hit, targetTerminal;
    private bool canonicalLaunch, foreignHazard;
    private JsonElement? frozen;
    public string? InvalidReason { get; private set; }
    public string? TerminalKind { get; private set; }
    public bool Complete => InvalidReason is null && owner is not null && targetTerminal is not null &&
        spec.Observers.Keys.Where(x => x != spec.ActorOrdinal).All(animations.ContainsKey);

    public NativeCombat(NativeCombatSpec spec)
    {
        this.spec = spec;
        Guard.Need(spec.ActorOrdinal != spec.TargetActorOrdinal && spec.AttackerSharedId != spec.TargetSharedId &&
            spec.Observers.ContainsKey(spec.ActorOrdinal) && spec.Observers.ContainsKey(spec.TargetActorOrdinal) &&
            spec.Observers.Count >= 2 && spec.DeadlineAtMs > spec.SentAtMs &&
            Cell.Of(Functions.PointMove(spec.Origin.Point, spec.Direction, 1)) == spec.TargetPosition,
            "native_combat_identity_or_target_geometry_invalid");
    }
    private void Invalid(string code) => InvalidReason ??= code;
    private NativeCombatFact Fact(Receipt receipt, uint? source = null, uint? victim = null, int? damage = null, int? hpBefore = null, int? hpAfter = null)
    {
        var fact = new NativeCombatFact(receipt.ActorOrdinal, receipt.Connection, receipt.Sequence, receipt.At,
            receipt.Packet.GetType().Name, source, victim, damage, hpBefore, hpAfter); facts.Add(fact); return fact;
    }
    private void Terminal(string kind, NativeCombatFact fact)
    {
        if (targetTerminal is not null) { Invalid("duplicate_native_combat_target_terminal"); return; }
        TerminalKind = kind; targetTerminal = fact; struck = hit = null;
    }
    public void Observe(Receipt receipt, int? hpBefore = null)
    {
        if (!spec.Observers.TryGetValue(receipt.ActorOrdinal, out var observer) || receipt.Connection != observer.Connection ||
            receipt.Sequence <= observer.ReceiveSequenceFloor) return;
        if (receipt.At < spec.SentAtMs) return; // Sequence floors alone are insufficient: never credit a pre-send packet.
        var canonicalTarget = receipt.ActorOrdinal == spec.TargetActorOrdinal;
        switch (receipt.Packet)
        {
            case S.UserLocation p when receipt.ActorOrdinal == spec.ActorOrdinal:
                if (owner is not null) { Invalid("duplicate_native_attack_owner_response"); return; }
                owner = Fact(receipt, spec.AttackerSharedId);
                if (Cell.Of(p.Location) != spec.Origin || p.Direction != spec.Direction) Invalid("native_attack_owner_transform_mismatch");
                break;
            case S.ObjectAttack p when p.ObjectID == spec.AttackerSharedId:
                var animation = Fact(receipt, p.ObjectID);
                if (receipt.ActorOrdinal == spec.ActorOrdinal || !animations.TryAdd(receipt.ActorOrdinal, animation))
                { Invalid("duplicate_or_unexpected_native_attack_animation"); return; }
                if (Cell.Of(p.Location) != spec.Origin || p.Direction != spec.Direction || p.Spell != Spell.None)
                { Invalid("native_attack_animation_geometry_or_spell_mismatch"); return; }
                if (canonicalTarget) canonicalLaunch = true;
                break;
            case S.ObjectAttack p when canonicalTarget && Cell.Of(Functions.PointMove(p.Location, p.Direction, 1)) == spec.TargetPosition:
                foreignHazard = true; Fact(receipt, p.ObjectID, spec.TargetSharedId); break;
            case S.ObjectRangeAttack when canonicalTarget:
                foreignHazard = true; Fact(receipt); break;
            case S.Struck p when canonicalTarget:
                var impact = Fact(receipt, p.AttackerID, spec.TargetSharedId);
                if (struck is not null || hit is not null) { Invalid("interleaved_native_target_struck"); return; }
                if (p.AttackerID != spec.AttackerSharedId) { foreignHazard = true; return; }
                if (!canonicalLaunch || targetTerminal is not null) { Invalid("native_target_struck_without_unique_launch"); return; }
                struck = impact; break;
            case S.ObjectStruck p when canonicalTarget && p.AttackerID == spec.AttackerSharedId:
                Fact(receipt, p.AttackerID, p.ObjectID); Invalid("native_attack_hit_wrong_target_or_unexpected_self_broadcast"); break;
            case S.DamageIndicator p when canonicalTarget && p.ObjectID == spec.TargetSharedId:
                var indicator = Fact(receipt, victim: p.ObjectID, damage: p.Damage);
                if (p.Type == DamageType.Miss)
                {
                    if (!canonicalLaunch || foreignHazard || struck is not null || hit is not null)
                    { Invalid("native_miss_has_no_unique_uncontested_launch"); return; }
                    Terminal("miss", indicator);
                }
                else if (p.Type == DamageType.Hit && p.Damage <= 0)
                {
                    if (!canonicalLaunch || struck is null || hit is not null)
                    { Invalid("native_hit_missing_unique_expected_struck"); return; }
                    if (p.Damage == 0) Terminal("explicitZeroDamage", indicator);
                    else hit = indicator;
                }
                else if (p.Type == DamageType.Hit && p.Damage > 0 && (struck is not null || hit is not null))
                    Invalid("native_heal_interleaved_with_expected_damage_chain");
                break;
            case S.HealthChanged p when canonicalTarget:
                var health = Fact(receipt, victim: spec.TargetSharedId, hpBefore: hpBefore, hpAfter: p.HP);
                if (p.HP <= 0) { Invalid("native_combat_target_died"); return; }
                if (hpBefore is null) { Invalid("native_combat_target_prior_hp_unavailable"); return; }
                if (hit is not null && struck is not null)
                {
                    if (p.HP >= hpBefore || p.HP - hpBefore != hit.Damage)
                    { Invalid("native_target_hp_not_exact_expected_hit_delta"); return; }
                    Terminal("positiveDamage", health);
                }
                else if (p.HP < hpBefore || struck is not null)
                    Invalid("native_hp_change_without_unique_expected_chain");
                break;
            case S.Death when canonicalTarget:
                Fact(receipt); Invalid("native_combat_identity_died_or_left_aoi"); break;
            case S.ObjectDied p when p.ObjectID == spec.TargetSharedId || p.ObjectID == spec.AttackerSharedId:
                Fact(receipt); Invalid("native_combat_identity_died_or_left_aoi"); break;
            case S.ObjectRemove p when p.ObjectID == spec.TargetSharedId || p.ObjectID == spec.AttackerSharedId:
                Fact(receipt); Invalid("native_combat_identity_died_or_left_aoi"); break;
        }
    }
    public double[] CompletionTimes() => animations.Values.Concat(new[] { owner, targetTerminal }.OfType<NativeCombatFact>()).Select(x => x.ReceivedAtMs).ToArray();
    public JsonElement SnapshotEvidence()
    {
        var required = animations.Values.Concat(new[] { owner, targetTerminal }.OfType<NativeCombatFact>()).ToArray();
        var completeAt = Complete ? required.Max(x => x.ReceivedAtMs) : (double?)null;
        var result = JsonSerializer.SerializeToElement(new { spec.TicketSequence, spec.ActorOrdinal, spec.TargetActorOrdinal,
            spec.AttackerSharedId, spec.TargetSharedId, spec.Origin, spec.TargetPosition, spec.Direction, spec.SentAtMs, spec.DeadlineAtMs, expectedObservers = spec.Observers,
            complete = Complete, completedOnTime = Complete && completeAt <= spec.DeadlineAtMs, completedAtMs = completeAt,
            terminalKind = TerminalKind, invalidReason = InvalidReason, facts = facts.ToArray(), lateReceiptCanRestoreSuccess = false }, Files.Compact);
        return result;
    }
    public JsonElement FreezeOutcome() => frozen ??= SnapshotEvidence();
}
