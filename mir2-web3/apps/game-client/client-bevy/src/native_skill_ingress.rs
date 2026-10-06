//! Shared native-host adaptation of authoritative skill snapshots and owner packets.
//!
//! Extracted from the frozen Windows host. This cursor patches presentation data
//! only; it does not decide hits, damage, MP expenditure, cooldown eligibility,
//! learned skills or successful key changes. Hosts own connection/character
//! boundaries and must validate their authenticated snapshots before ingestion.

use crate::skill_model::{SkillModelAuthority, MAX_LEARNED_SKILLS};
use serde_json::{json, Value};

const MAX_SKILL_PACKET_PATCHES: usize = MAX_LEARNED_SKILLS;

#[derive(Debug, Clone, Default)]
struct SkillPacketPatch {
    identity: String,
    base_snapshot_tick: u64,
    /// Tick-less deltas may affect only one bounded snapshot serial. This
    /// prevents an event without an ordering tick from living forever.
    zero_tick_expires_at_snapshot_serial: Option<u64>,
    delay_ms: Option<u32>,
    level: Option<u8>,
    experience: Option<u16>,
    can_use: Option<bool>,
    mp_cost: Option<u32>,
}

#[derive(Debug, Clone, Copy)]
struct PlayerVitalsPatch {
    base_snapshot_tick: u64,
    zero_tick_expires_at_snapshot_serial: Option<u64>,
    mp: Option<i32>,
    max_mp: Option<i32>,
}

#[derive(Debug, Clone, Copy)]
struct SkillRemovalPatch {
    hotkey: u8,
    base_snapshot_tick: u64,
    zero_tick_expires_at_snapshot_serial: Option<u64>,
}

/// One connection/character's bounded authoritative skill metadata and patches.
#[derive(Debug)]
pub struct NativeSkillPacketCursor {
    session_epoch: u64,
    magic_icons: std::collections::HashMap<String, u8>,
    magic_needs: std::collections::HashMap<String, [Option<u16>; 3]>,
    magic_names: std::collections::HashMap<String, String>,
    magic_casts: std::collections::HashMap<String, u64>,
    next_magic_cast: u64,
    snapshot_serial: u64,
    patches: Vec<SkillPacketPatch>,
    removals: Vec<SkillRemovalPatch>,
    vitals: Option<PlayerVitalsPatch>,
    player_object_id: Option<u32>,
}

impl Default for NativeSkillPacketCursor {
    fn default() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        Self {
            session_epoch: NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            snapshot_serial: 0,
            magic_icons: Default::default(),
            magic_needs: Default::default(),
            magic_names: Default::default(),
            magic_casts: Default::default(),
            next_magic_cast: 0,
            patches: vec![],
            removals: vec![],
            vitals: None,
            player_object_id: None,
        }
    }
}

/// Read-only diagnostics for host regressions; no mutable authority is exposed.
#[derive(Debug, Clone, Copy)]
pub struct NativeSkillIngressDiagnostics {
    pub magic_need_count: usize,
    pub patch_count: usize,
    pub removal_count: usize,
    pub has_vitals: bool,
    pub first_patch_expiry_serial: Option<u64>,
    pub first_removal_expiry_serial: Option<u64>,
    pub vitals_expiry_serial: Option<u64>,
}

impl NativeSkillPacketCursor {
    pub fn authority(&self) -> SkillModelAuthority {
        SkillModelAuthority {
            session_epoch: self.session_epoch,
            snapshot_serial: self.snapshot_serial,
            player_object_id: self.player_object_id.unwrap_or(0),
        }
    }
    pub fn player_object_id(&self) -> Option<u32> {
        self.player_object_id
    }
    pub fn magic_name(&self, spell: &str) -> Option<&str> {
        self.magic_names
            .get(&spell.to_ascii_lowercase())
            .map(String::as_str)
    }
    pub fn diagnostics(&self) -> NativeSkillIngressDiagnostics {
        NativeSkillIngressDiagnostics {
            magic_need_count: self.magic_needs.len(),
            patch_count: self.patches.len(),
            removal_count: self.removals.len(),
            has_vitals: self.vitals.is_some(),
            first_patch_expiry_serial: self
                .patches
                .first()
                .and_then(|p| p.zero_tick_expires_at_snapshot_serial),
            first_removal_expiry_serial: self
                .removals
                .first()
                .and_then(|p| p.zero_tick_expires_at_snapshot_serial),
            vitals_expiry_serial: self
                .vitals
                .and_then(|p| p.zero_tick_expires_at_snapshot_serial),
        }
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn observe_snapshot(&mut self, payload: &mut Value) {
        self.snapshot_serial = self.snapshot_serial.saturating_add(1);
        self.player_object_id = value_u32(payload.get("playerObjectId"));
        let snapshot_tick = world_payload_tick_from(Some(payload));
        let snapshot_serial = self.snapshot_serial;
        self.patches
            .retain(|patch| Self::patch_is_active(patch, snapshot_tick, snapshot_serial));
        self.removals
            .retain(|patch| Self::removal_is_active(patch, snapshot_tick, snapshot_serial));
        if self
            .vitals
            .is_some_and(|patch| !Self::vitals_is_active(&patch, snapshot_tick, snapshot_serial))
        {
            self.vitals = None;
        }
        self.apply_active_patches(payload, snapshot_tick);
        // A tick-less patch is valid for the snapshot that is being observed,
        // then retires even if future snapshots keep reporting tick=0.
        self.patches
            .retain(|patch| patch.zero_tick_expires_at_snapshot_serial != Some(snapshot_serial));
        self.removals
            .retain(|patch| patch.zero_tick_expires_at_snapshot_serial != Some(snapshot_serial));
        if self.vitals.is_some_and(|patch| {
            patch.zero_tick_expires_at_snapshot_serial == Some(snapshot_serial)
        }) {
            self.vitals = None;
        }
    }

    pub fn apply_active_patches(&self, payload: &mut Value, snapshot_tick: u64) {
        payload["_nativeSkillAuthority"] = json!({"sessionEpoch":self.session_epoch,"snapshotSerial":self.snapshot_serial,"playerObjectId":self.player_object_id.unwrap_or(0)});
        if let Some(skills) = skill_array_mut(payload) {
            skills.truncate(MAX_LEARNED_SKILLS);

            for skill in skills.iter_mut() {
                if let Some(name) = skill
                    .get("spell")
                    .and_then(Value::as_str)
                    .and_then(|spell| self.magic_names.get(&spell.to_ascii_lowercase()))
                {
                    skill["magicName"] = json!(name);
                }

                if let Some(sequence) = skill
                    .get("spell")
                    .and_then(Value::as_str)
                    .and_then(|spell| self.magic_casts.get(&spell.to_ascii_lowercase()))
                {
                    skill["castSequence"] = json!(sequence);
                }

                if let Some(icon) = skill
                    .get("spell")
                    .and_then(Value::as_str)
                    .and_then(|spell| self.magic_icons.get(&spell.to_ascii_lowercase()))
                    .copied()
                {
                    skill["icon"] = json!(icon);
                }
                if let Some(needs) = skill
                    .get("spell")
                    .and_then(Value::as_str)
                    .and_then(|spell| self.magic_needs.get(&spell.to_ascii_lowercase()))
                {
                    for (field, value) in ["need1", "need2", "need3"].into_iter().zip(needs) {
                        if let Some(value) = value {
                            skill[field] = json!(value);
                        }
                    }
                }
                if self.removals.iter().any(|removal| {
                    skill_hotkey(skill) == Some(removal.hotkey)
                        && Self::removal_is_active(removal, snapshot_tick, self.snapshot_serial)
                }) {
                    continue;
                }
                for patch in self.patches.iter().filter(|patch| {
                    Self::patch_is_active(patch, snapshot_tick, self.snapshot_serial)
                }) {
                    if skill_matches_identity(skill, &patch.identity) {
                        apply_skill_patch(skill, patch);
                    }
                }
            }

            skills.retain(|skill| {
                !self.removals.iter().any(|removal| {
                    skill_hotkey(skill) == Some(removal.hotkey)
                        && Self::removal_is_active(removal, snapshot_tick, self.snapshot_serial)
                })
            });
        }

        if let Some(vitals) = self.vitals {
            if Self::vitals_is_active(&vitals, snapshot_tick, self.snapshot_serial) {
                if let Some(mp) = vitals.mp {
                    payload["playerMp"] = json!(mp);
                }
                if let Some(max_mp) = vitals.max_mp {
                    payload["playerMaxMp"] = json!(max_mp);
                }
            }
        }
    }

    fn patch_is_active(patch: &SkillPacketPatch, snapshot_tick: u64, snapshot_serial: u64) -> bool {
        patch
            .zero_tick_expires_at_snapshot_serial
            .map(|expires| snapshot_serial <= expires)
            .unwrap_or(snapshot_tick == 0 || snapshot_tick <= patch.base_snapshot_tick)
    }

    fn removal_is_active(
        patch: &SkillRemovalPatch,
        snapshot_tick: u64,
        snapshot_serial: u64,
    ) -> bool {
        patch
            .zero_tick_expires_at_snapshot_serial
            .map(|expires| snapshot_serial <= expires)
            .unwrap_or(snapshot_tick == 0 || snapshot_tick <= patch.base_snapshot_tick)
    }

    fn vitals_is_active(
        patch: &PlayerVitalsPatch,
        snapshot_tick: u64,
        snapshot_serial: u64,
    ) -> bool {
        patch
            .zero_tick_expires_at_snapshot_serial
            .map(|expires| snapshot_serial <= expires)
            .unwrap_or(snapshot_tick == 0 || snapshot_tick <= patch.base_snapshot_tick)
    }

    pub fn apply_packet(&mut self, packet: &str, payload: &Value, base_snapshot_tick: u64) -> bool {
        match packet {
            "NewMagic" => {
                if payload.get("hero").and_then(Value::as_bool) != Some(false) {
                    return false;
                }
                let Some(magic) = payload.get("magic") else {
                    return false;
                };
                let Some(spell) = magic
                    .get("spell")
                    .and_then(Value::as_str)
                    .filter(|v| !v.is_empty())
                else {
                    return false;
                };
                let known = self
                    .magic_icons
                    .keys()
                    .chain(self.magic_names.keys())
                    .chain(self.magic_needs.keys())
                    .any(|key| key.eq_ignore_ascii_case(spell));
                let distinct = self
                    .magic_icons
                    .keys()
                    .chain(self.magic_names.keys())
                    .chain(self.magic_needs.keys())
                    .map(|key| key.to_ascii_lowercase())
                    .collect::<std::collections::HashSet<_>>()
                    .len();
                if !known && distinct >= MAX_LEARNED_SKILLS {
                    return false;
                }
                if let Some(name) = magic
                    .get("name")
                    .and_then(Value::as_str)
                    .filter(|v| !v.is_empty())
                {
                    self.magic_names
                        .insert(spell.to_ascii_lowercase(), name.to_owned());
                }
                if let Some(icon) = value_u32(magic.get("icon")).and_then(|v| u8::try_from(v).ok())
                {
                    self.magic_icons.insert(spell.to_ascii_lowercase(), icon);
                }
                self.magic_needs.insert(
                    spell.to_ascii_lowercase(),
                    ["need1", "need2", "need3"].map(|field| {
                        value_u32(magic.get(field)).and_then(|v| u16::try_from(v).ok())
                    }),
                );
                true
            }

            "UserInformation" => {
                let Some(object_id) = value_u32(payload.get("objectId")) else {
                    return false;
                };
                self.player_object_id = Some(object_id);
                self.vitals = Some(PlayerVitalsPatch {
                    base_snapshot_tick,
                    zero_tick_expires_at_snapshot_serial: zero_tick_expiry_serial(
                        self.snapshot_serial,
                        base_snapshot_tick,
                    ),
                    mp: value_i32(payload.get("mp").or_else(|| payload.get("playerMp"))),
                    max_mp: value_i32(payload.get("maxMp").or_else(|| payload.get("playerMaxMp"))),
                });
                if self
                    .vitals
                    .is_some_and(|patch| patch.mp.is_none() && patch.max_mp.is_none())
                {
                    self.vitals = None;
                    return false;
                }
                true
            }
            "Magic" | "MagicCast" => self.observe_owner_cast(packet, payload),
            "MagicDelay" => {
                if !self.packet_targets_player(payload) {
                    return false;
                }
                let Some(identity) = packet_spell_identity(payload) else {
                    return false;
                };
                let Some(delay) = value_u32(payload.get("delay")) else {
                    return false;
                };
                self.upsert_patch(identity, base_snapshot_tick, |patch| {
                    patch.delay_ms = Some(delay);
                    if let Some(mp_cost) =
                        value_u32(payload.get("mpCost").or_else(|| payload.get("mp_cost")))
                    {
                        patch.mp_cost = Some(mp_cost);
                    }
                });
                true
            }
            "MagicLeveled" => {
                if !self.packet_targets_player(payload) {
                    return false;
                }
                let Some(identity) = packet_spell_identity(payload) else {
                    return false;
                };
                let Some(level) =
                    value_u32(payload.get("level")).and_then(|value| u8::try_from(value).ok())
                else {
                    return false;
                };
                self.upsert_patch(identity, base_snapshot_tick, |patch| {
                    patch.level = Some(level);
                    patch.experience = value_u32(payload.get("experience"))
                        .and_then(|value| u16::try_from(value).ok());
                });
                true
            }
            "SpellToggle" => {
                if !self.packet_targets_player(payload) {
                    return false;
                }
                let Some(identity) = packet_spell_identity(payload) else {
                    return false;
                };
                let Some(can_use) = payload.get("canUse").and_then(Value::as_bool) else {
                    return false;
                };
                self.upsert_patch(identity, base_snapshot_tick, |patch| {
                    patch.can_use = Some(can_use);
                });
                true
            }
            "RemoveMagic" => {
                let Some(hotkey) = value_u32(payload.get("placeId"))
                    .and_then(|value| (1..=8).contains(&value).then_some(value as u8))
                else {
                    return false;
                };
                if self.removals.len() >= MAX_SKILL_PACKET_PATCHES {
                    self.removals.remove(0);
                }
                self.removals.push(SkillRemovalPatch {
                    hotkey,
                    base_snapshot_tick,
                    zero_tick_expires_at_snapshot_serial: zero_tick_expiry_serial(
                        self.snapshot_serial,
                        base_snapshot_tick,
                    ),
                });
                true
            }
            _ => false,
        }
    }

    fn observe_owner_cast(&mut self, packet: &str, payload: &Value) -> bool {
        match packet {
            "Magic" if payload.get("cast").and_then(Value::as_bool) == Some(true) => {}
            "MagicCast"
                if payload
                    .get("cast")
                    .is_none_or(|cast| cast.as_bool() == Some(true)) => {}
            _ => return false,
        }
        let Some(player_id) = self.player_object_id.filter(|id| *id != 0) else {
            return false;
        };
        // These are owner-only Crystal packets and normally omit objectId.
        // Never let an explicit Hero/other actor identity override that owner.
        if payload
            .get("hero")
            .is_some_and(|hero| hero.as_bool() != Some(false))
            || payload
                .get("objectId")
                .is_some_and(|actor| value_u32(Some(actor)) != Some(player_id))
        {
            return false;
        }
        let Some(identity) = packet_spell_identity(payload) else {
            return false;
        };
        if !self.magic_casts.contains_key(&identity) && self.magic_casts.len() >= MAX_LEARNED_SKILLS
        {
            return false;
        }
        self.next_magic_cast = self.next_magic_cast.saturating_add(1);
        self.magic_casts.insert(identity, self.next_magic_cast);
        true
    }

    fn packet_targets_player(&self, payload: &Value) -> bool {
        // The authoritative SpellToggle packet always carries an object id.
        // Treat a missing/malformed id as an invalid packet instead of letting
        // it mutate the local player's skill state by default.
        let Some(object_id) = value_u32(payload.get("objectId")) else {
            return false;
        };
        object_id != 0
            && self
                .player_object_id
                .map(|player_id| player_id != 0 && player_id == object_id)
                .unwrap_or(false)
    }

    fn upsert_patch(
        &mut self,
        identity: String,
        base_snapshot_tick: u64,
        update: impl FnOnce(&mut SkillPacketPatch),
    ) {
        let zero_tick_expires_at_snapshot_serial =
            zero_tick_expiry_serial(self.snapshot_serial, base_snapshot_tick);
        if let Some(patch) = self
            .patches
            .iter_mut()
            .find(|patch| patch.identity == identity)
        {
            patch.base_snapshot_tick = base_snapshot_tick;
            patch.zero_tick_expires_at_snapshot_serial = zero_tick_expires_at_snapshot_serial;
            update(patch);
            return;
        }
        if self.patches.len() >= MAX_SKILL_PACKET_PATCHES {
            self.patches.remove(0);
        }
        let mut patch = SkillPacketPatch {
            identity,
            base_snapshot_tick,
            zero_tick_expires_at_snapshot_serial,
            ..Default::default()
        };
        update(&mut patch);
        self.patches.push(patch);
    }
}

fn zero_tick_expiry_serial(snapshot_serial: u64, base_snapshot_tick: u64) -> Option<u64> {
    (base_snapshot_tick == 0).then(|| snapshot_serial.saturating_add(1))
}

pub fn world_payload_tick_from(payload: Option<&Value>) -> u64 {
    payload
        .and_then(|payload| {
            value_u64(payload.get("tick"))
                .or_else(|| value_u64(payload.get("snapshotTick")))
                .or_else(|| value_u64(payload.get("snapshot_tick")))
        })
        .unwrap_or(0)
}

fn skill_array_mut(payload: &mut Value) -> Option<&mut Vec<Value>> {
    if payload.get("knownSkills").is_some() {
        return payload.get_mut("knownSkills").and_then(Value::as_array_mut);
    }
    if payload.get("known_skills").is_some() {
        return payload
            .get_mut("known_skills")
            .and_then(Value::as_array_mut);
    }
    payload.get_mut("skills").and_then(Value::as_array_mut)
}

fn packet_spell_identity(payload: &Value) -> Option<String> {
    let spell = payload.get("spell").and_then(Value::as_str)?.trim();
    (!spell.is_empty()).then(|| spell.to_ascii_lowercase())
}

fn skill_hotkey(skill: &Value) -> Option<u8> {
    value_u32(skill.get("hotkey").or_else(|| skill.get("key")))
        .and_then(|value| u8::try_from(value).ok())
}

fn skill_matches_identity(skill: &Value, identity: &str) -> bool {
    // Packet spell ids may only patch a snapshot's authoritative spell field.
    // Display names and local keys are not protocol identifiers.
    skill
        .get("spell")
        .and_then(Value::as_str)
        .map(|value| value.trim().eq_ignore_ascii_case(identity))
        .unwrap_or(false)
}

fn apply_skill_patch(skill: &mut Value, patch: &SkillPacketPatch) {
    if let Some(delay) = patch.delay_ms {
        skill["delayMs"] = json!(delay);
    }
    if let Some(level) = patch.level {
        skill["level"] = json!(level);
    }
    if let Some(experience) = patch.experience {
        skill["experience"] = json!(experience);
    }
    if let Some(can_use) = patch.can_use {
        skill["canUse"] = json!(can_use);
    }
    if let Some(mp_cost) = patch.mp_cost {
        skill["mpCost"] = json!(mp_cost);
    }
}

pub fn project_native_skill_model(payload: &Value) -> Value {
    let skills = payload
        .get("knownSkills")
        .or_else(|| payload.get("known_skills"))
        .or_else(|| payload.get("skills"))
        .and_then(Value::as_array)
        .map(|skills| {
            skills
                .iter()
                .take(MAX_LEARNED_SKILLS)
                .enumerate()
                .map(|(idx, skill)| {
                    let id = value_u32(skill.get("id")).unwrap_or(idx as u32);
                    let key = skill.get("key").and_then(Value::as_str).map(str::to_owned);
                    let name = skill
                        .get("magicName")
                        .and_then(Value::as_str)
                        .filter(|v| !v.is_empty())
                        .or_else(|| skill.get("name").and_then(Value::as_str))
                        .unwrap_or_default()
                        .to_owned();
                    let level = value_u32(skill.get("level"))
                        .and_then(|value| u8::try_from(value).ok())
                        .unwrap_or(0);
                    let delay_ms = value_i64(
                        skill
                            .get("delayMs")
                            .or_else(|| skill.get("delay_ms"))
                            .or_else(|| skill.get("cooldownMs")),
                    )
                    .unwrap_or(0);
                    let mut transformed = serde_json::Map::new();
                    transformed.insert("id".to_owned(), json!(id));
                    transformed.insert(
                        "castSequence".to_owned(),
                        skill.get("castSequence").cloned().unwrap_or(json!(0)),
                    );
                    transformed.insert(
                        "icon".to_owned(),
                        value_u32(skill.get("icon"))
                            .and_then(|v| u8::try_from(v).ok())
                            .map(|v| json!(v))
                            .unwrap_or(Value::Null),
                    );
                    let definition = skill
                        .get("spell")
                        .and_then(Value::as_str)
                        .and_then(mir2_game_data::crystal_magic_by_spell);
                    for (field, fallback) in [
                        ("experience", None),
                        ("need1", definition.as_ref().map(|v| v.need1)),
                        ("need2", definition.as_ref().map(|v| v.need2)),
                        ("need3", definition.as_ref().map(|v| v.need3)),
                    ] {
                        let value = skill
                            .get(field)
                            .map(|v| value_u32(Some(v)).and_then(|n| u16::try_from(n).ok()))
                            .unwrap_or(fallback);
                        transformed.insert(
                            field.to_owned(),
                            value.map(|v| json!(v)).unwrap_or(Value::Null),
                        );
                    }
                    transformed.insert("name".to_owned(), json!(name));
                    transformed.insert("level".to_owned(), json!(level));
                    transformed.insert("key".to_owned(), json!(key));
                    transformed.insert("cooldown_ms".to_owned(), json!(delay_ms.max(0)));
                    transformed.insert(
                        "spell".to_owned(),
                        optional_non_empty_string_value(skill.get("spell")),
                    );
                    transformed.insert(
                        "castKind".to_owned(),
                        optional_non_empty_string_value(skill.get("castKind")),
                    );
                    transformed.insert(
                        "canUse".to_owned(),
                        skill.get("canUse").cloned().unwrap_or(Value::Null),
                    );
                    transformed.insert(
                        "offensive".to_owned(),
                        skill.get("offensive").cloned().unwrap_or(Value::Null),
                    );
                    transformed.insert(
                        "hotkey".to_owned(),
                        skill.get("hotkey").cloned().unwrap_or(Value::Null),
                    );
                    transformed.insert(
                        "cooldownRemainingTicks".to_owned(),
                        value_u32(
                            skill
                                .get("cooldownRemainingTicks")
                                .or_else(|| skill.get("cooldown_remaining_ticks")),
                        )
                        .map(|value| json!(value))
                        .unwrap_or_else(|| json!(0)),
                    );
                    transformed.insert(
                        "cooldownRemainingMs".to_owned(),
                        value_u32(
                            skill
                                .get("cooldownRemainingMs")
                                .or_else(|| skill.get("cooldown_remaining_ms")),
                        )
                        .map(|value| json!(value))
                        .unwrap_or(Value::Null),
                    );
                    transformed.insert(
                        "mpCost".to_owned(),
                        value_u32(skill.get("mpCost").or_else(|| skill.get("mp_cost")))
                            .map(|value| json!(value))
                            .unwrap_or(Value::Null),
                    );
                    transformed.insert(
                        "castTimeMs".to_owned(),
                        value_i64(
                            skill
                                .get("castTimeMs")
                                .or_else(|| skill.get("cast_time_ms")),
                        )
                        .map(|value| json!(value))
                        .unwrap_or(Value::Null),
                    );
                    transformed.insert(
                        "experience".to_owned(),
                        value_u32(skill.get("experience"))
                            .and_then(|value| u16::try_from(value).ok())
                            .map(|value| json!(value))
                            .unwrap_or(Value::Null),
                    );
                    Value::Object(transformed)
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    json!({ "skills": skills,"skillKeyAck":payload.get("skillKeyAck").cloned().unwrap_or(Value::Null), "authority":payload.get("_nativeSkillAuthority").cloned().unwrap_or(json!({"sessionEpoch":0,"snapshotSerial":0,"playerObjectId":0})) })
}

fn value_u32(value: Option<&Value>) -> Option<u32> {
    value.and_then(|value| {
        value
            .as_u64()
            .and_then(|number| u32::try_from(number).ok())
            .or_else(|| value.as_str()?.parse::<u32>().ok())
    })
}

fn value_i32(value: Option<&Value>) -> Option<i32> {
    value.and_then(|value| {
        value
            .as_i64()
            .and_then(|number| i32::try_from(number).ok())
            .or_else(|| value.as_str()?.parse::<i32>().ok())
    })
}

fn value_i64(value: Option<&Value>) -> Option<i64> {
    value.and_then(|value| {
        value
            .as_i64()
            .or_else(|| value.as_str()?.parse::<i64>().ok())
    })
}

fn value_u64(value: Option<&Value>) -> Option<u64> {
    value.and_then(value_u64_ref)
}

fn value_u64_ref(value: &Value) -> Option<u64> {
    value
        .as_u64()
        .or_else(|| value.as_str()?.parse::<u64>().ok())
}

fn optional_non_empty_string_value(value: Option<&Value>) -> Value {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| json!(value))
        .unwrap_or(Value::Null)
}
