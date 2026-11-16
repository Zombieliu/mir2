//! Versioned internal identities. These are descriptions, never write capabilities.
//!
//! A legacy execution domain names a persistent logical world/line, not its host.
//! Placement and topology observations may change while the business effect stays
//! the same. Application receipts still need the owning transaction's durable
//! commit boundary; this module deliberately does not implement a second ledger.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{MapEffectOwnerRef, PartitionKey, TopologyRevision, ZoneKey};

const CONTRACT_VERSION: u32 = 1;
const LEGACY_WORLD_PREFIX: &str = "legacy-execution-domain:";

fn component(label: &str, value: &str) -> Result<(), String> {
    if value.is_empty()
        || value.trim() != value
        || value.len() > 512
        || value.chars().any(char::is_control)
    {
        return Err(format!(
            "{label} requires a canonical nonempty value of at most 512 bytes"
        ));
    }
    Ok(())
}

fn zone(key: &ZoneKey) -> Result<(), String> {
    component("shard", &key.shard_id)?;
    component("map", &key.map_file_name)?;
    component("instance", &key.instance_id)
}

fn version(value: u32) -> Result<(), String> {
    if value != CONTRACT_VERSION {
        return Err(format!("unsupported partition contract version {value}"));
    }
    Ok(())
}

// Legacy keys intentionally retain their old serde behavior elsewhere. New
// internal contracts reject unknown nested fields without changing old bytes.
fn strict_zone<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<ZoneKey, D::Error> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Wire {
        shard_id: String,
        map_file_name: String,
        channel_id: u16,
        instance_id: String,
    }
    let value = Wire::deserialize(deserializer)?;
    let value = ZoneKey::new(
        value.shard_id,
        value.map_file_name,
        value.channel_id,
        value.instance_id,
    );
    zone(&value).map_err(serde::de::Error::custom)?;
    Ok(value)
}

fn strict_partition<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<PartitionKey, D::Error> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Wire {
        #[serde(deserialize_with = "strict_zone")]
        zone: ZoneKey,
        partition_id: super::PartitionId,
    }
    let value = Wire::deserialize(deserializer)?;
    Ok(PartitionKey {
        zone: value.zone,
        partition_id: value.partition_id,
    })
}

fn strict_map_owner<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<MapEffectOwnerRef, D::Error> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Wire {
        version: u32,
        #[serde(deserialize_with = "strict_zone")]
        zone: ZoneKey,
    }
    let value = Wire::deserialize(deserializer)?;
    if value.version != MapEffectOwnerRef::VERSION {
        return Err(serde::de::Error::custom(
            "unsupported map effect owner version",
        ));
    }
    Ok(MapEffectOwnerRef::for_zone(value.zone))
}

/// Global namespace for an existing logical execution domain. Changing a host,
/// lease, worker count or endpoint must not change this value.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct LogicalWorldId(String);

impl LogicalWorldId {
    pub fn legacy_execution_domain(domain: &str) -> Result<Self, String> {
        component("legacy logical world", domain)?;
        Ok(Self(format!("{LEGACY_WORLD_PREFIX}{domain}")))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for LogicalWorldId {
    type Error = String;
    fn try_from(value: String) -> Result<Self, String> {
        let domain = value
            .strip_prefix(LEGACY_WORLD_PREFIX)
            .ok_or("unsupported logical world namespace")?;
        Self::legacy_execution_domain(domain)
    }
}

impl From<LogicalWorldId> for String {
    fn from(value: LogicalWorldId) -> String {
        value.0
    }
}

/// Wrap the unchanged legacy partition key instead of altering drop identities.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "WorldPartitionWire", into = "WorldPartitionWire")]
pub struct WorldPartitionKey {
    world: LogicalWorldId,
    partition: PartitionKey,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WorldPartitionWire {
    version: u32,
    world: LogicalWorldId,
    #[serde(deserialize_with = "strict_partition")]
    partition: PartitionKey,
}

impl TryFrom<WorldPartitionWire> for WorldPartitionKey {
    type Error = String;
    fn try_from(value: WorldPartitionWire) -> Result<Self, String> {
        version(value.version)?;
        zone(&value.partition.zone)?;
        Ok(Self::new(value.world, value.partition))
    }
}
impl From<WorldPartitionKey> for WorldPartitionWire {
    fn from(value: WorldPartitionKey) -> Self {
        Self {
            version: CONTRACT_VERSION,
            world: value.world,
            partition: value.partition,
        }
    }
}

impl WorldPartitionKey {
    pub fn new(world: LogicalWorldId, partition: PartitionKey) -> Self {
        Self { world, partition }
    }
    pub fn world(&self) -> &LogicalWorldId {
        &self.world
    }
    pub fn partition(&self) -> &PartitionKey {
        &self.partition
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "ActionWire", into = "ActionWire")]
pub struct StableActionId {
    world: LogicalWorldId,
    zone: ZoneKey,
    source_sequence: u64,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ActionWire {
    version: u32,
    world: LogicalWorldId,
    #[serde(deserialize_with = "strict_zone")]
    zone: ZoneKey,
    source_sequence: u64,
}

impl StableActionId {
    /// The sequence is allocated by server admission, never client-selected.
    /// Its durable stream must survive handoff; placement epochs cannot reset it.
    pub fn from_source_sequence(
        world: LogicalWorldId,
        zone_key: ZoneKey,
        source_sequence: u64,
    ) -> Result<Self, String> {
        zone(&zone_key)?;
        if source_sequence == 0 {
            return Err("action source sequence must be positive".into());
        }
        Ok(Self {
            world,
            zone: zone_key,
            source_sequence,
        })
    }
    pub fn world(&self) -> &LogicalWorldId {
        &self.world
    }
    pub fn zone(&self) -> &ZoneKey {
        &self.zone
    }
    pub fn source_sequence(&self) -> u64 {
        self.source_sequence
    }
    /// Does not wrap on exhaustion or mutate the original identity.
    pub fn checked_successor(&self) -> Result<Self, String> {
        let next = self
            .source_sequence
            .checked_add(1)
            .ok_or("action source sequence exhausted")?;
        Self::from_source_sequence(self.world.clone(), self.zone.clone(), next)
    }
}

impl TryFrom<ActionWire> for StableActionId {
    type Error = String;
    fn try_from(value: ActionWire) -> Result<Self, String> {
        version(value.version)?;
        Self::from_source_sequence(value.world, value.zone, value.source_sequence)
    }
}
impl From<StableActionId> for ActionWire {
    fn from(value: StableActionId) -> Self {
        Self {
            version: CONTRACT_VERSION,
            world: value.world,
            zone: value.zone,
            source_sequence: value.source_sequence,
        }
    }
}

/// Generation zero is a legitimate player's first life, but never an online
/// admission epoch or a newly allocated monster incarnation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum EntityLife {
    Player {
        online_namespace: String,
        online_epoch: u64,
        life_generation: u64,
    },
    Monster {
        incarnation: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "EntityWire", into = "EntityWire")]
pub struct EntityLifeRef {
    world: LogicalWorldId,
    zone: ZoneKey,
    object_id: u32,
    life: EntityLife,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct EntityWire {
    version: u32,
    world: LogicalWorldId,
    #[serde(deserialize_with = "strict_zone")]
    zone: ZoneKey,
    object_id: u32,
    life: EntityLife,
}

impl EntityLifeRef {
    pub fn player(
        world: LogicalWorldId,
        zone_key: ZoneKey,
        object_id: u32,
        online_namespace: String,
        online_epoch: u64,
        life_generation: u64,
    ) -> Result<Self, String> {
        Self::checked(
            world,
            zone_key,
            object_id,
            EntityLife::Player {
                online_namespace,
                online_epoch,
                life_generation,
            },
        )
    }
    pub fn monster(
        world: LogicalWorldId,
        zone_key: ZoneKey,
        object_id: u32,
        incarnation: u64,
    ) -> Result<Self, String> {
        Self::checked(
            world,
            zone_key,
            object_id,
            EntityLife::Monster { incarnation },
        )
    }
    fn checked(
        world: LogicalWorldId,
        zone_key: ZoneKey,
        object_id: u32,
        life: EntityLife,
    ) -> Result<Self, String> {
        zone(&zone_key)?;
        if object_id == 0 {
            return Err("entity object id must be positive".into());
        }
        match &life {
            EntityLife::Player {
                online_namespace,
                online_epoch,
                ..
            } => {
                component("online namespace", online_namespace)?;
                if *online_epoch == 0 {
                    return Err("online admission epoch must be positive".into());
                }
            }
            EntityLife::Monster { incarnation: 0 } => {
                return Err("monster incarnation must be positive".into())
            }
            _ => {}
        }
        Ok(Self {
            world,
            zone: zone_key,
            object_id,
            life,
        })
    }
    pub fn world(&self) -> &LogicalWorldId {
        &self.world
    }
    pub fn zone(&self) -> &ZoneKey {
        &self.zone
    }
    pub fn object_id(&self) -> u32 {
        self.object_id
    }
    /// The caller must obtain `current` from the real owner, not a Ghost or bytes.
    pub fn validate_current(&self, current: &Self) -> Result<(), String> {
        if self != current {
            return Err("entity world, admission or life is stale".into());
        }
        Ok(())
    }
}
impl TryFrom<EntityWire> for EntityLifeRef {
    type Error = String;
    fn try_from(value: EntityWire) -> Result<Self, String> {
        version(value.version)?;
        Self::checked(value.world, value.zone, value.object_id, value.life)
    }
}
impl From<EntityLifeRef> for EntityWire {
    fn from(value: EntityLifeRef) -> Self {
        Self {
            version: CONTRACT_VERSION,
            world: value.world,
            zone: value.zone,
            object_id: value.object_id,
            life: value.life,
        }
    }
}

/// Canonical SHA-256 bytes; callers cannot import malformed digest strings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct PayloadDigest(String);
impl PayloadDigest {
    pub fn of_bytes(bytes: &[u8]) -> Self {
        Self(
            Sha256::digest(bytes)
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect(),
        )
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl TryFrom<String> for PayloadDigest {
    type Error = String;
    fn try_from(value: String) -> Result<Self, String> {
        if value.len() != 64
            || !value
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err("payload digest must be 64 lowercase hexadecimal characters".into());
        }
        Ok(Self(value))
    }
}
impl From<PayloadDigest> for String {
    fn from(value: PayloadDigest) -> String {
        value.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "EffectOwnerWire", into = "EffectOwnerWire")]
pub struct WorldMapEffectOwner {
    world: LogicalWorldId,
    map_owner: MapEffectOwnerRef,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct EffectOwnerWire {
    version: u32,
    world: LogicalWorldId,
    #[serde(deserialize_with = "strict_map_owner")]
    map_owner: MapEffectOwnerRef,
}
impl WorldMapEffectOwner {
    pub fn for_map(world: LogicalWorldId, zone: ZoneKey) -> Self {
        Self {
            world,
            map_owner: MapEffectOwnerRef::for_zone(zone),
        }
    }
    pub fn world(&self) -> &LogicalWorldId {
        &self.world
    }
    pub fn map_owner(&self) -> &MapEffectOwnerRef {
        &self.map_owner
    }
}
impl TryFrom<EffectOwnerWire> for WorldMapEffectOwner {
    type Error = String;
    fn try_from(value: EffectOwnerWire) -> Result<Self, String> {
        version(value.version)?;
        zone(value.map_owner.zone())?;
        Ok(Self {
            world: value.world,
            map_owner: value.map_owner,
        })
    }
}
impl From<WorldMapEffectOwner> for EffectOwnerWire {
    fn from(value: WorldMapEffectOwner) -> Self {
        Self {
            version: CONTRACT_VERSION,
            world: value.world,
            map_owner: value.map_owner,
        }
    }
}

/// One accepted effect. Placement observations are excluded from the stable
/// effect identity and business digest, but are never ignored for authorization.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "EffectWire", into = "EffectWire")]
pub struct EffectEnvelope {
    action: StableActionId,
    source: Option<EntityLifeRef>,
    target: Option<EntityLifeRef>,
    accepted_at_ms: u64,
    due_at_ms: u64,
    effect_ordinal: u32,
    payload_digest: PayloadDigest,
    topology_revision: TopologyRevision,
    placement_epoch: u64,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct EffectWire {
    version: u32,
    action: StableActionId,
    source: Option<EntityLifeRef>,
    target: Option<EntityLifeRef>,
    accepted_at_ms: u64,
    due_at_ms: u64,
    effect_ordinal: u32,
    payload_digest: PayloadDigest,
    topology_revision: TopologyRevision,
    placement_epoch: u64,
}
impl EffectEnvelope {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        action: StableActionId,
        source: Option<EntityLifeRef>,
        target: Option<EntityLifeRef>,
        accepted_at_ms: u64,
        due_at_ms: u64,
        effect_ordinal: u32,
        payload_digest: PayloadDigest,
        topology_revision: TopologyRevision,
        placement_epoch: u64,
    ) -> Result<Self, String> {
        if due_at_ms < accepted_at_ms {
            return Err("effect due time precedes admission".into());
        }
        if placement_epoch == 0 {
            return Err("placement epoch must be positive".into());
        }
        for entity in source.iter().chain(target.iter()) {
            if entity.world() != action.world() || entity.zone() != action.zone() {
                return Err("effect entity belongs to another logical world or map".into());
            }
        }
        Ok(Self {
            action,
            source,
            target,
            accepted_at_ms,
            due_at_ms,
            effect_ordinal,
            payload_digest,
            topology_revision,
            placement_epoch,
        })
    }
    pub fn action(&self) -> &StableActionId {
        &self.action
    }
    pub fn source(&self) -> Option<&EntityLifeRef> {
        self.source.as_ref()
    }
    pub fn target(&self) -> Option<&EntityLifeRef> {
        self.target.as_ref()
    }
    pub fn due_at_ms(&self) -> u64 {
        self.due_at_ms
    }
    pub fn accepted_at_ms(&self) -> u64 {
        self.accepted_at_ms
    }
    pub fn effect_ordinal(&self) -> u32 {
        self.effect_ordinal
    }
    pub fn topology_revision(&self) -> TopologyRevision {
        self.topology_revision
    }
    pub fn placement_epoch(&self) -> u64 {
        self.placement_epoch
    }
    pub fn effect_owner(&self) -> WorldMapEffectOwner {
        WorldMapEffectOwner::for_map(self.action.world.clone(), self.action.zone.clone())
    }
    pub fn validate_payload(&self, payload: &[u8]) -> Result<(), String> {
        if self.payload_digest != PayloadDigest::of_bytes(payload) {
            return Err("effect payload digest mismatch".into());
        }
        Ok(())
    }
    /// Current references must be queried under the owning commit boundary.
    /// A successful result alone is not an owner lease or permission to mutate.
    pub fn validate_current(
        &self,
        source: Option<&EntityLifeRef>,
        target: Option<&EntityLifeRef>,
        payload: &[u8],
    ) -> Result<(), String> {
        if self.source.as_ref() != source || self.target.as_ref() != target {
            return Err("effect references a stale source or target life".into());
        }
        self.validate_payload(payload)
    }
    fn business_digest(&self) -> PayloadDigest {
        let bytes = serde_json::to_vec(&(
            CONTRACT_VERSION,
            &self.action,
            &self.source,
            &self.target,
            self.accepted_at_ms,
            self.due_at_ms,
            self.effect_ordinal,
            &self.payload_digest,
        ))
        .expect("effect contract contains only serializable values");
        let mut canonical = b"mir2.partition.effect.v1\0".to_vec();
        canonical.extend(bytes);
        PayloadDigest::of_bytes(&canonical)
    }
}
impl TryFrom<EffectWire> for EffectEnvelope {
    type Error = String;
    fn try_from(value: EffectWire) -> Result<Self, String> {
        version(value.version)?;
        Self::new(
            value.action,
            value.source,
            value.target,
            value.accepted_at_ms,
            value.due_at_ms,
            value.effect_ordinal,
            value.payload_digest,
            value.topology_revision,
            value.placement_epoch,
        )
    }
}
impl From<EffectEnvelope> for EffectWire {
    fn from(value: EffectEnvelope) -> Self {
        Self {
            version: CONTRACT_VERSION,
            action: value.action,
            source: value.source,
            target: value.target,
            accepted_at_ms: value.accepted_at_ms,
            due_at_ms: value.due_at_ms,
            effect_ordinal: value.effect_ordinal,
            payload_digest: value.payload_digest,
            topology_revision: value.topology_revision,
            placement_epoch: value.placement_epoch,
        }
    }
}

/// A replay-comparison record, not evidence that a transaction was committed.
/// The actual owner stores it atomically with its state mutation in P3.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "ReceiptWire", into = "ReceiptWire")]
pub struct EffectReplayReceipt {
    action: StableActionId,
    effect_ordinal: u32,
    business_digest: PayloadDigest,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReceiptWire {
    version: u32,
    action: StableActionId,
    effect_ordinal: u32,
    business_digest: PayloadDigest,
}
impl TryFrom<ReceiptWire> for EffectReplayReceipt {
    type Error = String;
    fn try_from(value: ReceiptWire) -> Result<Self, String> {
        version(value.version)?;
        Ok(Self {
            action: value.action,
            effect_ordinal: value.effect_ordinal,
            business_digest: value.business_digest,
        })
    }
}
impl From<EffectReplayReceipt> for ReceiptWire {
    fn from(value: EffectReplayReceipt) -> Self {
        Self {
            version: CONTRACT_VERSION,
            action: value.action,
            effect_ordinal: value.effect_ordinal,
            business_digest: value.business_digest,
        }
    }
}
impl EffectReplayReceipt {
    pub fn for_effect(effect: &EffectEnvelope) -> Self {
        Self {
            action: effect.action.clone(),
            effect_ordinal: effect.effect_ordinal,
            business_digest: effect.business_digest(),
        }
    }
    pub fn validate_replay(&self, effect: &EffectEnvelope) -> Result<(), String> {
        if self != &Self::for_effect(effect) {
            return Err("effect replay identity or business payload conflicts with receipt".into());
        }
        Ok(())
    }
}
