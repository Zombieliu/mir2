using S = ServerPackets;

namespace ComparableCrystal;

internal static class NativeCombatTests
{
    public static void Run(Action<string, Action> check)
    {
        check("native_melee_positive_requires_full_owner_peer_and_exact_target_chain", () =>
        {
            var combat = Started();
            Guard.Need(!combat.Complete, "animations_alone_credited_as_hit");
            Hit(combat, 100, 90);
            var row = combat.FreezeOutcome();
            Guard.Need(row.GetProperty("completedOnTime").GetBoolean() && row.GetProperty("terminalKind").GetString() == "positiveDamage", "full_native_hit_not_proven");
        });
        check("native_melee_unpaired_or_inexact_hp_never_proves_hit", () =>
        {
            var unpaired = Started(); unpaired.Observe(Target(22, 1300, new S.HealthChanged { HP = 90 }), 100);
            Guard.Need(unpaired.InvalidReason is not null && !unpaired.Complete, "unpaired_hp_accepted");
            var wrong = Started(); Hit(wrong, 100, 95);
            Guard.Need(wrong.InvalidReason is not null && !wrong.Complete, "inexact_hp_accepted");
        });
        check("native_melee_miss_requires_uncontested_launch", () =>
        {
            var good = Started(); good.Observe(Target(22, 1300, new S.DamageIndicator { ObjectID = 52, Type = DamageType.Miss }));
            Guard.Need(good.Complete && good.TerminalKind == "miss", "explicit_native_miss_not_terminal");
            var contested = Started(); contested.Observe(Target(22, 1200, new S.ObjectAttack { ObjectID = 99, Location = new(1, 1), Direction = MirDirection.Right, Spell = Spell.None }));
            contested.Observe(Target(23, 1300, new S.DamageIndicator { ObjectID = 52, Type = DamageType.Miss }));
            Guard.Need(contested.InvalidReason is not null && !contested.Complete, "contested_miss_accepted");
        });
        check("native_melee_every_declared_peer_animation_is_required", () =>
        {
            var spec = Spec() with { Observers = new Dictionary<int, NativeCombatObserver> { [0] = new(5, 10), [1] = new(3, 20), [2] = new(4, 30) } };
            var combat = new NativeCombat(spec);
            combat.Observe(Owner(11, 1010, new S.UserLocation { Location = new(1, 1), Direction = MirDirection.Right }));
            combat.Observe(Target(21, 1020, new S.ObjectAttack { ObjectID = 51, Location = new(1, 1), Direction = MirDirection.Right, Spell = Spell.None }));
            Hit(combat, 100, 90); Guard.Need(!combat.Complete, "missing_third_observer_was_credited");
            combat.Observe(new(2, 4, 31, 1310, new S.ObjectAttack { ObjectID = 51, Location = new(1, 1), Direction = MirDirection.Right, Spell = Spell.None }));
            Guard.Need(combat.Complete, "full_three_observer_proof_not_complete");
        });
        check("native_melee_explicit_zero_hit_requires_expected_struck", () =>
        {
            var combat = Started(); combat.Observe(Target(22, 1200, new S.Struck { AttackerID = 51 }));
            combat.Observe(Target(23, 1201, new S.DamageIndicator { ObjectID = 52, Type = DamageType.Hit, Damage = 0 }));
            Guard.Need(combat.Complete && combat.TerminalKind == "explicitZeroDamage", "explicit_zero_hit_not_distinguished_from_positive_hp_damage");
        });
        check("native_melee_old_connection_and_late_completion_do_not_restore_outcome", () =>
        {
            var combat = new NativeCombat(Spec());
            combat.Observe(new Receipt(0, 4, 11, 1010, new S.UserLocation { Location = new(1, 1), Direction = MirDirection.Right }));
            Guard.Need(!combat.Complete, "old_connection_credited");
            var frozen = combat.FreezeOutcome();
            combat.Observe(Owner(11, 3600, new S.UserLocation { Location = new(1, 1), Direction = MirDirection.Right }));
            combat.Observe(Target(21, 3601, new S.ObjectAttack { ObjectID = 51, Location = new(1, 1), Direction = MirDirection.Right, Spell = Spell.None }));
            combat.Observe(Target(22, 3602, new S.DamageIndicator { ObjectID = 52, Type = DamageType.Miss }));
            Guard.Need(!frozen.GetProperty("completedOnTime").GetBoolean() && !combat.FreezeOutcome().GetProperty("completedOnTime").GetBoolean(), "frozen_failure_rewritten_by_late_packets");
        });
        check("native_melee_separate_heal_hp_packets_preserve_exact_damage_proof", () =>
        {
            var combat = Started();
            combat.Observe(Target(22, 1100, new S.HealthChanged { HP = 110 }), 100);
            combat.Observe(Target(23, 1101, new S.DamageIndicator { ObjectID = 52, Type = DamageType.Hit, Damage = 10 }));
            combat.Observe(Target(24, 1300, new S.Struck { AttackerID = 51 }));
            combat.Observe(Target(25, 1301, new S.DamageIndicator { ObjectID = 52, Type = DamageType.Hit, Damage = -10 }));
            combat.Observe(Target(26, 1302, new S.HealthChanged { HP = 100 }), 110);
            Guard.Need(combat.Complete && combat.TerminalKind == "positiveDamage", "separate_heal_wrongly_merged_into_damage_hp");
        });
        check("native_melee_regen_inside_damage_chain_is_not_guessed", () =>
        {
            var combat = Started(); combat.Observe(Target(22, 1200, new S.Struck { AttackerID = 51 }));
            combat.Observe(Target(23, 1201, new S.DamageIndicator { ObjectID = 52, Type = DamageType.Hit, Damage = 9 }));
            Guard.Need(combat.InvalidReason is not null && !combat.Complete, "interleaved_hp_effect_silently_reconciled");
        });
        check("native_use_item_ack_consumes_one_actual_uid_and_durability_is_preserved", () =>
        {
            var root = Path.Combine(Files.StateBase, "offline-potion-" + Guid.NewGuid().ToString("N")); Directory.CreateDirectory(root);
            using var evidence = new Evidence(Path.Combine(root, "events.jsonl"), 10000); var actor = PotionActor(evidence);
            var decision = actor.ReservePotion(100); var before = actor.ItemFingerprint;
            actor.Observe(new S.UseItem { UniqueID = decision.Uid, Grid = MirGridType.Inventory, Success = true });
            Guard.Need(actor.PotionStock == 19 && actor.Failure is null && actor.ItemFingerprint != before, "successful_native_use_did_not_consume_exactly_one");
            var afterPotion = actor.ItemFingerprint; actor.Observe(new S.DuraChanged { UniqueID = 900, CurrentDura = 99 });
            Guard.Need(actor.ItemFingerprint != afterPotion && actor.PotionStock == 19 && actor.Failure is null, "ordinary_dura_change_lost_or_consumed_extra_stock");
            actor.DisposeAsync().GetAwaiter().GetResult();
        });
        check("native_use_item_wrong_uid_ack_is_hard_failure_without_stock_credit", () =>
        {
            var root = Path.Combine(Files.StateBase, "offline-potion-" + Guid.NewGuid().ToString("N")); Directory.CreateDirectory(root);
            using var evidence = new Evidence(Path.Combine(root, "events.jsonl"), 10000); var actor = PotionActor(evidence); actor.ReservePotion(100);
            actor.Observe(new S.UseItem { UniqueID = 999, Grid = MirGridType.Inventory, Success = true });
            Guard.Need(actor.Failure is not null && actor.PotionStock == 20, "unmatched_use_ack_credited_or_ignored"); actor.DisposeAsync().GetAwaiter().GetResult();
        });
    }
    private static NativeCombatSpec Spec() => new(1, 0, 51, 1, 52, new(1, 1), new(2, 1), MirDirection.Right, 1000, 3500,
        new Dictionary<int, NativeCombatObserver> { [0] = new(5, 10), [1] = new(3, 20) });
    private static NativeActor PotionActor(Evidence evidence)
    {
        var actor = new NativeActor(new(0, "synthetic", "synthetic", "Synthetic", 1, 440, new(1, 1), 0, "Other"), 1, evidence);
        var inventory = new UserItem[46]; inventory[6] = new UserItem(new ItemInfo { Index = 662 }) { UniqueID = 901, Count = 20 };
        var equipment = new UserItem[14]; equipment[0] = new UserItem(new ItemInfo { Index = 221 }) { UniqueID = 900, Count = 1, CurrentDura = 100, MaxDura = 100 };
        actor.Observe(new S.UserInformation { ObjectID = 51, RealId = 1, Name = "Synthetic", HP = 70, Level = 40,
            Inventory = inventory, Equipment = equipment, QuestInventory = new UserItem[40], Location = new(1, 1) }); return actor;
    }
    private static Receipt Owner(long sequence, double at, Packet packet) => new(0, 5, sequence, at, packet);
    private static Receipt Target(long sequence, double at, Packet packet) => new(1, 3, sequence, at, packet);
    private static NativeCombat Started()
    {
        var combat = new NativeCombat(Spec());
        combat.Observe(Owner(11, 1010, new S.UserLocation { Location = new(1, 1), Direction = MirDirection.Right }));
        combat.Observe(Target(21, 1020, new S.ObjectAttack { ObjectID = 51, Location = new(1, 1), Direction = MirDirection.Right, Spell = Spell.None })); return combat;
    }
    private static void Hit(NativeCombat combat, int before, int after)
    {
        combat.Observe(Target(22, 1300, new S.Struck { AttackerID = 51 }));
        combat.Observe(Target(23, 1301, new S.DamageIndicator { ObjectID = 52, Type = DamageType.Hit, Damage = -10 }));
        combat.Observe(Target(24, 1302, new S.HealthChanged { HP = after }), before);
    }
}
