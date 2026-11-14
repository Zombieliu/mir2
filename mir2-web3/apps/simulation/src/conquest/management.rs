use super::{ConquestDefenseKind, ConquestDefenseState, SharedConquestRecord};
use std::collections::BTreeMap;

const SABUK_FULL_REPAIR_COST: u32 = 1_000;

/// The selected Crystal Server.MirDB Sabuk record. These are conquest-owned
/// defenses, not ordinary respawns. Archer slots 10 and 12 share a source tile;
/// placement/collision reconciliation must not turn that into a second world.
pub fn default_sabuk_defenses() -> BTreeMap<String, ConquestDefenseState> {
    let mut defenses = BTreeMap::new();
    let mut add = |kind, slot, template_name: &str, x, y, max_hp| {
        let prefix = match kind {
            ConquestDefenseKind::Gate => "gate",
            ConquestDefenseKind::Wall => "wall",
            ConquestDefenseKind::Archer => "archer",
        };
        defenses.insert(
            format!("{prefix}:{slot}"),
            ConquestDefenseState {
                slot,
                kind,
                template_name: template_name.into(),
                x,
                y,
                hp: max_hp,
                max_hp,
                open: false,
            },
        );
    };
    add(ConquestDefenseKind::Gate, 1, "SabukGate", 672, 330, 5_000);
    for (slot, name, x, y) in [
        (1, "PalaceWallLeft", 624, 278),
        (2, "PalaceWall1", 627, 278),
        (3, "PalaceWall2", 634, 271),
    ] {
        add(ConquestDefenseKind::Wall, slot, name, x, y, 5_000);
    }
    for (slot, x, y) in [
        (1, 662, 333),
        (2, 664, 331),
        (3, 666, 329),
        (4, 676, 319),
        (5, 678, 317),
        (6, 681, 314),
        (7, 628, 271),
        (8, 632, 267),
        (9, 670, 335),
        (10, 671, 334),
        (11, 675, 330),
        (12, 671, 334),
    ] {
        add(
            ConquestDefenseKind::Archer,
            slot,
            "ArcherGuard3",
            x,
            y,
            9_999,
        );
    }
    defenses
}

impl SharedConquestRecord {
    /// Trusted owner-management operation; the normal NPC admission layer must
    /// recheck current guild ownership and permission before invoking it.
    pub fn set_tax(&mut self, rate: u8) -> Result<bool, String> {
        self.validate()?;
        if ![10, 15, 20, 25].contains(&rate) {
            return Err("unsupported Sabuk tax rate".into());
        }
        if self.tax_rate_percent == rate {
            return Ok(false);
        }
        let mut next = self.clone();
        next.tax_rate_percent = rate;
        self.commit_management_change(next)?;
        Ok(true)
    }

    /// A dead gate cannot be opened or closed by management. Its HP already
    /// makes its footprint passable; a paid repair restores a closed gate.
    pub fn set_gate_open(&mut self, key: &str, open: bool) -> Result<bool, String> {
        self.validate()?;
        let defense = self.defenses.get(key).ok_or("unknown conquest defense")?;
        if defense.kind != ConquestDefenseKind::Gate {
            return Err("conquest defense is not a gate".into());
        }
        if defense.hp == 0 {
            return Err("destroyed conquest gate requires repair".into());
        }
        if defense.open == open {
            return Ok(false);
        }
        let mut next = self.clone();
        next.defenses.get_mut(key).expect("validated gate").open = open;
        self.commit_management_change(next)?;
        Ok(true)
    }

    /// Crystal's exact nested integer division, not a linear missing-HP quote.
    /// Archers can only be rehired after death, rather than healed for a fee.
    pub fn repair_cost(&self, key: &str) -> Result<u32, String> {
        self.validate()?;
        let defense = self.defenses.get(key).ok_or("unknown conquest defense")?;
        if defense.kind == ConquestDefenseKind::Archer {
            return Ok(if defense.hp == 0 {
                SABUK_FULL_REPAIR_COST
            } else {
                0
            });
        }
        if defense.hp == defense.max_hp {
            return Ok(0);
        }
        let max_hp = u32::try_from(defense.max_hp).map_err(|_| "invalid defense maximum HP")?;
        let missing =
            u32::try_from(defense.max_hp - defense.hp).map_err(|_| "invalid defense repair HP")?;
        Ok(SABUK_FULL_REPAIR_COST / (max_hp / missing))
    }

    /// The persistence caller must atomically commit both this next record and
    /// the current owner's guild bank. A rejected domain mutation changes neither.
    pub fn repair(&mut self, key: &str, guild_bank: &mut u32) -> Result<u32, String> {
        self.validate()?;
        let defense = self.defenses.get(key).ok_or("unknown conquest defense")?;
        if defense.hp == defense.max_hp
            || (defense.kind == ConquestDefenseKind::Archer && defense.hp != 0)
        {
            return Ok(0);
        }
        let cost = self.repair_cost(key)?;
        let next_bank = guild_bank
            .checked_sub(cost)
            .ok_or("insufficient guild bank gold")?;
        let mut next = self.clone();
        let target = next.defenses.get_mut(key).expect("validated defense");
        if target.kind == ConquestDefenseKind::Gate && target.hp == 0 {
            target.open = false;
        }
        target.hp = target.max_hp;
        self.commit_management_change(next)?;
        *guild_bank = next_bank;
        Ok(cost)
    }

    /// All collected tax goes to the guild bank, never to a character's wallet.
    /// Repeating an already committed withdrawal is a zero-change retry.
    pub fn withdraw_tax(&mut self, guild_bank: &mut u32) -> Result<u32, String> {
        self.validate()?;
        let amount = self.gold;
        if amount == 0 {
            return Ok(0);
        }
        let next_bank = guild_bank
            .checked_add(amount)
            .ok_or("guild bank gold capacity exceeded")?;
        let mut next = self.clone();
        next.gold = 0;
        self.commit_management_change(next)?;
        *guild_bank = next_bank;
        Ok(amount)
    }

    /// Apply only the HP produced by committed, authoritative Zone combat.
    /// The caller validates attack ownership/friendly immunity; client HP values
    /// are never an input. HP cannot increase, regenerate, or change after cutoff.
    pub fn update_authoritative_defense_damage(
        &mut self,
        key: &str,
        hp: i32,
        now_ms: u64,
    ) -> Result<bool, String> {
        self.validate()?;
        if !self.war_active || now_ms < self.last_observed_ms || now_ms >= self.war_ends_ms {
            return Err("conquest defense damage outside active war".into());
        }
        let defense = self.defenses.get(key).ok_or("unknown conquest defense")?;
        if hp < 0 || hp > defense.hp {
            return Err("invalid authoritative conquest defense damage".into());
        }
        if hp == defense.hp {
            return Ok(false);
        }
        let mut next = self.clone();
        next.defenses.get_mut(key).expect("validated defense").hp = hp;
        self.commit_management_change(next)?;
        Ok(true)
    }

    fn commit_management_change(&mut self, mut next: Self) -> Result<(), String> {
        next.changed()?;
        next.validate()?;
        *self = next;
        Ok(())
    }
}
