use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const QUALITY_DENOMINATOR: u128 = 10_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VerifiedWorkReceipt {
    pub receipt_id: String,
    pub game_id: String,
    pub epoch: u64,
    pub zone_id: String,
    pub control_height: u64,
    pub placement_generation: u64,
    pub work_units: u64,
    pub availability_bps: u16,
    pub quorum_node_ids: Vec<String>,
    pub execution_commitment: String,
    pub observed_at_ms: u64,
}

impl VerifiedWorkReceipt {
    pub fn validate(&self) -> Result<(), String> {
        validate_id("receipt id", &self.receipt_id)?;
        validate_id("game id", &self.game_id)?;
        validate_id("zone id", &self.zone_id)?;
        if self.work_units == 0 {
            return Err("verified work receipt must contain work".to_string());
        }
        if self.availability_bps > 10_000 {
            return Err("verified work availability exceeds 10000 bps".to_string());
        }
        if self.quorum_node_ids.is_empty() {
            return Err("verified work receipt has no quorum attestors".to_string());
        }
        let mut unique = BTreeSet::new();
        for node_id in &self.quorum_node_ids {
            validate_id("node id", node_id)?;
            if !unique.insert(node_id) {
                return Err(format!("duplicate quorum node {node_id}"));
            }
        }
        if self.execution_commitment.len() != 64
            || !self
                .execution_commitment
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
        {
            return Err("verified work execution commitment must be a sha256 hex digest".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameRewardPolicy {
    pub game_id: String,
    pub epoch: u64,
    pub reward_budget: u64,
    pub reward_per_work_unit: u64,
    pub max_reward_per_node: u64,
    pub minimum_availability_bps: u16,
    pub minimum_quorum: u16,
    pub settlement_coin_type: String,
}

impl GameRewardPolicy {
    pub fn validate(&self) -> Result<(), String> {
        validate_id("game id", &self.game_id)?;
        if self.reward_budget == 0 || self.reward_per_work_unit == 0 {
            return Err("reward budget and unit price must be positive".to_string());
        }
        if self.max_reward_per_node == 0 {
            return Err("per-node reward cap must be positive".to_string());
        }
        if self.minimum_availability_bps > 10_000 {
            return Err("minimum availability exceeds 10000 bps".to_string());
        }
        if self.minimum_quorum == 0 {
            return Err("minimum reward quorum must be positive".to_string());
        }
        validate_id("settlement coin type", &self.settlement_coin_type)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RewardAllocation {
    pub game_id: String,
    pub epoch: u64,
    pub node_id: String,
    pub amount: u64,
    pub work_score: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RewardSettlementBatch {
    pub batch_id: String,
    pub game_id: String,
    pub epoch: u64,
    pub merkle_root: String,
    pub total_reward: u64,
    pub allocation_count: u32,
    pub finalized_control_height: u64,
    pub settlement_coin_type: String,
    pub allocations: Vec<RewardAllocation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RewardClaimProof {
    pub allocation: RewardAllocation,
    pub leaf_index: u32,
    pub siblings: Vec<String>,
}

impl RewardClaimProof {
    pub fn verify(&self, expected_root: &str) -> bool {
        let Ok(mut hash) = allocation_leaf(&self.allocation) else {
            return false;
        };
        let mut index = self.leaf_index as usize;
        for sibling in &self.siblings {
            let Some(sibling) = decode_digest(sibling) else {
                return false;
            };
            hash = if index % 2 == 0 {
                merkle_parent(&hash, &sibling)
            } else {
                merkle_parent(&sibling, &hash)
            };
            index /= 2;
        }
        hex_digest(&hash) == expected_root
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "status")]
pub enum SettlementStatus {
    Pending,
    Submitted {
        transaction_digest: String,
    },
    Finalized {
        transaction_digest: String,
        checkpoint: u64,
    },
}

#[derive(Debug, Default)]
pub struct MultiGameRewardLedger {
    policies: BTreeMap<(String, u64), GameRewardPolicy>,
    receipts: BTreeMap<(String, u64), Vec<VerifiedWorkReceipt>>,
    receipt_ids: BTreeSet<(String, String)>,
    batches: BTreeMap<(String, u64), RewardSettlementBatch>,
    settlement: BTreeMap<String, SettlementStatus>,
}

impl MultiGameRewardLedger {
    pub fn register_policy(&mut self, policy: GameRewardPolicy) -> Result<(), String> {
        policy.validate()?;
        let key = (policy.game_id.clone(), policy.epoch);
        if self.batches.contains_key(&key) {
            return Err("cannot change a finalized reward epoch".to_string());
        }
        if self.policies.insert(key, policy).is_some() {
            return Err("reward policy already exists".to_string());
        }
        Ok(())
    }

    pub fn ingest_verified(&mut self, receipt: VerifiedWorkReceipt) -> Result<bool, String> {
        receipt.validate()?;
        let epoch_key = (receipt.game_id.clone(), receipt.epoch);
        let policy = self
            .policies
            .get(&epoch_key)
            .ok_or_else(|| "no reward policy for verified work receipt".to_string())?;
        if self.batches.contains_key(&epoch_key) {
            return Err("reward epoch is already finalized".to_string());
        }
        if receipt.availability_bps < policy.minimum_availability_bps {
            return Err("verified work availability is below policy minimum".to_string());
        }
        if receipt.quorum_node_ids.len() < usize::from(policy.minimum_quorum) {
            return Err("verified work quorum is below policy minimum".to_string());
        }
        let receipt_key = (receipt.game_id.clone(), receipt.receipt_id.clone());
        if !self.receipt_ids.insert(receipt_key) {
            return Ok(false);
        }
        self.receipts.entry(epoch_key).or_default().push(receipt);
        Ok(true)
    }

    pub fn finalize_epoch(
        &mut self,
        game_id: &str,
        epoch: u64,
        finalized_control_height: u64,
    ) -> Result<RewardSettlementBatch, String> {
        let key = (game_id.to_string(), epoch);
        if let Some(batch) = self.batches.get(&key) {
            return Ok(batch.clone());
        }
        let policy = self
            .policies
            .get(&key)
            .ok_or_else(|| "reward policy is not registered".to_string())?
            .clone();
        let receipts = self
            .receipts
            .get(&key)
            .ok_or_else(|| "reward epoch contains no verified work".to_string())?;
        let mut scores = BTreeMap::<String, u128>::new();
        for receipt in receipts {
            if receipt.control_height > finalized_control_height {
                return Err(format!(
                    "receipt {} depends on unfinalized control height {}",
                    receipt.receipt_id, receipt.control_height
                ));
            }
            let score = u128::from(receipt.work_units)
                .saturating_mul(u128::from(receipt.availability_bps))
                / QUALITY_DENOMINATOR;
            for node_id in &receipt.quorum_node_ids {
                scores
                    .entry(node_id.clone())
                    .and_modify(|current| *current = current.saturating_add(score))
                    .or_insert(score);
            }
        }
        scores.retain(|_, score| *score > 0);
        if scores.is_empty() {
            return Err("verified work produced no payable score".to_string());
        }

        let mut desired = Vec::with_capacity(scores.len());
        for (node_id, score) in scores {
            let amount = score
                .saturating_mul(u128::from(policy.reward_per_work_unit))
                .min(u128::from(policy.max_reward_per_node));
            desired.push((node_id, score, amount));
        }
        let desired_total = desired.iter().fold(0_u128, |total, (_, _, amount)| {
            total.saturating_add(*amount)
        });
        let budget = u128::from(policy.reward_budget);
        let paid_total = desired_total.min(budget);
        let mut amounts = BTreeMap::<String, u128>::new();
        if desired_total <= budget {
            for (node_id, _, amount) in &desired {
                amounts.insert(node_id.clone(), *amount);
            }
        } else {
            let mut allocated = 0_u128;
            let mut remainders = Vec::with_capacity(desired.len());
            for (node_id, _, amount) in &desired {
                let numerator = amount.saturating_mul(budget);
                let share = numerator / desired_total;
                allocated = allocated.saturating_add(share);
                amounts.insert(node_id.clone(), share);
                remainders.push((numerator % desired_total, node_id.clone()));
            }
            remainders.sort_by(|left, right| right.0.cmp(&left.0).then(left.1.cmp(&right.1)));
            for (_, node_id) in remainders.into_iter().take((budget - allocated) as usize) {
                amounts.entry(node_id).and_modify(|amount| *amount += 1);
            }
        }

        let allocations = desired
            .into_iter()
            .map(|(node_id, score, _)| RewardAllocation {
                game_id: game_id.to_string(),
                epoch,
                amount: amounts[&node_id] as u64,
                work_score: score.min(u128::from(u64::MAX)) as u64,
                node_id,
            })
            .filter(|allocation| allocation.amount > 0)
            .collect::<Vec<_>>();
        let tree = merkle_levels(&allocations)?;
        let root = tree
            .last()
            .and_then(|level| level.first())
            .ok_or_else(|| "reward allocation tree is empty".to_string())?;
        let merkle_root = hex_digest(root);
        let total_reward = paid_total as u64;
        let batch_id = reward_batch_id(game_id, epoch, &merkle_root, total_reward);
        let batch = RewardSettlementBatch {
            batch_id: batch_id.clone(),
            game_id: game_id.to_string(),
            epoch,
            merkle_root,
            total_reward,
            allocation_count: allocations.len() as u32,
            finalized_control_height,
            settlement_coin_type: policy.settlement_coin_type,
            allocations,
        };
        self.settlement.insert(batch_id, SettlementStatus::Pending);
        self.batches.insert(key, batch.clone());
        Ok(batch)
    }

    pub fn claim_proof(
        &self,
        game_id: &str,
        epoch: u64,
        node_id: &str,
    ) -> Option<RewardClaimProof> {
        let batch = self.batches.get(&(game_id.to_string(), epoch))?;
        let leaf_index = batch
            .allocations
            .iter()
            .position(|allocation| allocation.node_id == node_id)?;
        let levels = merkle_levels(&batch.allocations).ok()?;
        let mut index = leaf_index;
        let mut siblings = Vec::new();
        for level in levels.iter().take(levels.len().saturating_sub(1)) {
            let sibling = if index % 2 == 0 {
                level.get(index + 1).unwrap_or(&level[index])
            } else {
                &level[index - 1]
            };
            siblings.push(hex_digest(sibling));
            index /= 2;
        }
        Some(RewardClaimProof {
            allocation: batch.allocations[leaf_index].clone(),
            leaf_index: leaf_index as u32,
            siblings,
        })
    }

    pub fn mark_submitted(&mut self, batch_id: &str, tx_digest: &str) -> Result<(), String> {
        validate_id("transaction digest", tx_digest)?;
        let status = self
            .settlement
            .get_mut(batch_id)
            .ok_or_else(|| "unknown reward settlement batch".to_string())?;
        match status {
            SettlementStatus::Pending => {
                *status = SettlementStatus::Submitted {
                    transaction_digest: tx_digest.to_string(),
                };
                Ok(())
            }
            SettlementStatus::Submitted { transaction_digest }
            | SettlementStatus::Finalized {
                transaction_digest, ..
            } if transaction_digest == tx_digest => Ok(()),
            _ => Err("reward batch was submitted with a different transaction".to_string()),
        }
    }

    pub fn mark_finalized(
        &mut self,
        batch_id: &str,
        tx_digest: &str,
        checkpoint: u64,
    ) -> Result<(), String> {
        self.mark_submitted(batch_id, tx_digest)?;
        self.settlement.insert(
            batch_id.to_string(),
            SettlementStatus::Finalized {
                transaction_digest: tx_digest.to_string(),
                checkpoint,
            },
        );
        Ok(())
    }

    pub fn settlement_status(&self, batch_id: &str) -> Option<&SettlementStatus> {
        self.settlement.get(batch_id)
    }
}

fn reward_batch_id(game_id: &str, epoch: u64, root: &str, total: u64) -> String {
    let mut hash = Sha256::new();
    hash.update(b"obelisk.shared-compute.reward-batch.v1\0");
    hash.update((game_id.len() as u64).to_be_bytes());
    hash.update(game_id.as_bytes());
    hash.update(epoch.to_be_bytes());
    hash.update(root.as_bytes());
    hash.update(total.to_be_bytes());
    hex_digest(&hash.finalize())
}

fn allocation_leaf(allocation: &RewardAllocation) -> Result<[u8; 32], String> {
    validate_id("game id", &allocation.game_id)?;
    validate_id("node id", &allocation.node_id)?;
    let mut hash = Sha256::new();
    hash.update(b"obelisk.shared-compute.reward-leaf.v1\0");
    hash.update((allocation.game_id.len() as u64).to_be_bytes());
    hash.update(allocation.game_id.as_bytes());
    hash.update(allocation.epoch.to_be_bytes());
    hash.update((allocation.node_id.len() as u64).to_be_bytes());
    hash.update(allocation.node_id.as_bytes());
    hash.update(allocation.amount.to_be_bytes());
    hash.update(allocation.work_score.to_be_bytes());
    Ok(hash.finalize().into())
}

fn merkle_levels(allocations: &[RewardAllocation]) -> Result<Vec<Vec<[u8; 32]>>, String> {
    if allocations.is_empty() {
        return Err("reward allocation tree needs at least one leaf".to_string());
    }
    let mut levels = vec![allocations
        .iter()
        .map(allocation_leaf)
        .collect::<Result<Vec<_>, _>>()?];
    while levels.last().is_some_and(|level| level.len() > 1) {
        let current = levels.last().expect("merkle level exists");
        let mut next = Vec::with_capacity(current.len().div_ceil(2));
        for pair in current.chunks(2) {
            next.push(merkle_parent(&pair[0], pair.get(1).unwrap_or(&pair[0])));
        }
        levels.push(next);
    }
    Ok(levels)
}

fn merkle_parent(left: &[u8; 32], right: &[u8; 32]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"obelisk.shared-compute.reward-node.v1\0");
    hash.update(left);
    hash.update(right);
    hash.finalize().into()
}

fn decode_digest(value: &str) -> Option<[u8; 32]> {
    if value.len() != 64 {
        return None;
    }
    let mut result = [0_u8; 32];
    for (index, byte) in result.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16).ok()?;
    }
    Some(result)
}

fn hex_digest(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn validate_id(label: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty() || value.len() > 255 || value.chars().any(char::is_control) {
        return Err(format!("invalid {label}"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy(game_id: &str, budget: u64) -> GameRewardPolicy {
        GameRewardPolicy {
            game_id: game_id.to_string(),
            epoch: 7,
            reward_budget: budget,
            reward_per_work_unit: 10,
            max_reward_per_node: 10_000,
            minimum_availability_bps: 9_000,
            minimum_quorum: 2,
            settlement_coin_type: "0x2::sui::SUI".to_string(),
        }
    }

    fn receipt(id: &str, game_id: &str, nodes: &[&str], work: u64) -> VerifiedWorkReceipt {
        VerifiedWorkReceipt {
            receipt_id: id.to_string(),
            game_id: game_id.to_string(),
            epoch: 7,
            zone_id: "mir2/map/0".to_string(),
            control_height: 11,
            placement_generation: 3,
            work_units: work,
            availability_bps: 10_000,
            quorum_node_ids: nodes.iter().map(|node| (*node).to_string()).collect(),
            execution_commitment: "ab".repeat(32),
            observed_at_ms: 1,
        }
    }

    #[test]
    fn verified_receipts_are_deduplicated_and_capped_by_game_budget() {
        let mut ledger = MultiGameRewardLedger::default();
        ledger.register_policy(policy("mir2", 1_000)).unwrap();
        assert!(ledger
            .ingest_verified(receipt("r1", "mir2", &["node-a", "node-b"], 100))
            .unwrap());
        assert!(!ledger
            .ingest_verified(receipt("r1", "mir2", &["node-a", "node-b"], 100))
            .unwrap());
        assert!(ledger
            .ingest_verified(receipt("r2", "mir2", &["node-b", "node-c"], 100))
            .unwrap());

        let batch = ledger.finalize_epoch("mir2", 7, 11).unwrap();
        assert_eq!(batch.total_reward, 1_000);
        assert_eq!(
            batch
                .allocations
                .iter()
                .map(|item| item.amount)
                .sum::<u64>(),
            1_000
        );
        assert_eq!(batch.allocation_count, 3);
        assert_eq!(ledger.finalize_epoch("mir2", 7, 11).unwrap(), batch);
    }

    #[test]
    fn games_are_isolated_and_unfinalized_control_work_is_rejected() {
        let mut ledger = MultiGameRewardLedger::default();
        ledger.register_policy(policy("mir2", 500)).unwrap();
        ledger.register_policy(policy("other-game", 900)).unwrap();
        ledger
            .ingest_verified(receipt("same-id", "mir2", &["a", "b"], 10))
            .unwrap();
        ledger
            .ingest_verified(receipt("same-id", "other-game", &["a", "b"], 10))
            .unwrap();
        assert!(ledger.finalize_epoch("mir2", 7, 10).is_err());
        let mir2 = ledger.finalize_epoch("mir2", 7, 11).unwrap();
        let other = ledger.finalize_epoch("other-game", 7, 11).unwrap();
        assert_ne!(mir2.batch_id, other.batch_id);
        assert_ne!(mir2.merkle_root, other.merkle_root);
    }

    #[test]
    fn every_claim_proof_verifies_and_settlement_is_idempotent() {
        let mut ledger = MultiGameRewardLedger::default();
        ledger.register_policy(policy("mir2", 1_000)).unwrap();
        ledger
            .ingest_verified(receipt("r1", "mir2", &["a", "b"], 20))
            .unwrap();
        ledger
            .ingest_verified(receipt("r2", "mir2", &["b", "c"], 10))
            .unwrap();
        let batch = ledger.finalize_epoch("mir2", 7, 11).unwrap();
        for allocation in &batch.allocations {
            let proof = ledger.claim_proof("mir2", 7, &allocation.node_id).unwrap();
            assert!(proof.verify(&batch.merkle_root));
        }
        ledger.mark_submitted(&batch.batch_id, "tx-1").unwrap();
        ledger.mark_submitted(&batch.batch_id, "tx-1").unwrap();
        assert!(ledger.mark_submitted(&batch.batch_id, "tx-2").is_err());
        ledger.mark_finalized(&batch.batch_id, "tx-1", 99).unwrap();
        assert!(matches!(
            ledger.settlement_status(&batch.batch_id),
            Some(SettlementStatus::Finalized { checkpoint: 99, .. })
        ));
    }
}
