// SPDX-License-Identifier: Apache-2.0
module mir2_procurement::intent;

use sui::ed25519;
use std::hash::sha2_256;

const EInvalid: u64 = 100;

/// Frozen v1 wire order. Addresses are fixed 32-byte BCS addresses; byte vectors carry ULEB lengths.
public struct IntentV1 has copy, drop, store {
    domain: vector<u8>, network_hash: vector<u8>, package_id: address, dapp_storage_id: address,
    operation: u8, order_id: vector<u8>, fill_id: vector<u8>, buyer_ref: vector<u8>, seller_ref: vector<u8>,
    template_id: u32, conditions_hash: vector<u8>, quantity: u64, price_gold: u64,
    expires_at_ms: u64, rule_version: u64, nonce: vector<u8>, valid_until_ms: u64,
    actor_public_key: vector<u8>,
}

public struct OracleProofV1 has copy, drop, store {
    domain: vector<u8>, intent_hash: vector<u8>, asset_lock_id: vector<u8>,
    asset_instance_hash: vector<u8>, oracle_key_id: u64, valid_until_ms: u64, fence: u64,
}

public fun new_intent(
    network_hash: vector<u8>, package_id: address, dapp_storage_id: address, operation: u8,
    order_id: vector<u8>, fill_id: vector<u8>, buyer_ref: vector<u8>, seller_ref: vector<u8>,
    template_id: u32, conditions_hash: vector<u8>, quantity: u64, price_gold: u64,
    expires_at_ms: u64, rule_version: u64, nonce: vector<u8>, valid_until_ms: u64,
    actor_public_key: vector<u8>,
): IntentV1 {
    IntentV1 { domain: b"MIR2_PROCUREMENT_INTENT_V1", network_hash, package_id, dapp_storage_id,
        operation, order_id, fill_id, buyer_ref, seller_ref, template_id, conditions_hash, quantity,
        price_gold, expires_at_ms, rule_version, nonce, valid_until_ms, actor_public_key }
}

public fun new_proof(intent_hash: vector<u8>, asset_lock_id: vector<u8>, asset_instance_hash: vector<u8>,
    oracle_key_id: u64, valid_until_ms: u64, fence: u64): OracleProofV1 {
    OracleProofV1 { domain: b"MIR2_PROCUREMENT_ORACLE_V1", intent_hash, asset_lock_id,
        asset_instance_hash, oracle_key_id, valid_until_ms, fence }
}

public fun intent_hash(i: &IntentV1): vector<u8> { sha2_256(sui::bcs::to_bytes(i)) }
public fun proof_hash(p: &OracleProofV1): vector<u8> { sha2_256(sui::bcs::to_bytes(p)) }

public fun verify(i: &IntentV1, p: &OracleProofV1, actor_signature: &vector<u8>,
    oracle_signature: &vector<u8>, oracle_public_key: &vector<u8>, oracle_key_id: u64,
    network_hash: &vector<u8>, package_id: address, dapp_storage_id: address, now: u64) {
    assert!(i.domain == b"MIR2_PROCUREMENT_INTENT_V1" && p.domain == b"MIR2_PROCUREMENT_ORACLE_V1", EInvalid);
    assert!(i.network_hash == *network_hash && i.package_id == package_id && i.dapp_storage_id == dapp_storage_id, EInvalid);
    assert!(i.network_hash.length() == 32 && i.order_id.length() == 32 && i.fill_id.length() == 32, EInvalid);
    assert!(i.buyer_ref.length() == 32 && i.seller_ref.length() == 32 && i.conditions_hash.length() == 32, EInvalid);
    assert!(i.nonce.length() == 32 && i.actor_public_key.length() == 32 && p.asset_lock_id.length() == 32, EInvalid);
    assert!(p.asset_instance_hash.length() == 32 && p.intent_hash == intent_hash(i), EInvalid);
    assert!(i.quantity == 1 && i.price_gold > 0 && i.price_gold <= 4_294_967_295 && i.template_id == 245, EInvalid);
    assert!(i.rule_version > 0 && now < i.valid_until_ms && now < p.valid_until_ms, EInvalid);
    assert!(p.valid_until_ms <= i.valid_until_ms && i.valid_until_ms <= i.expires_at_ms, EInvalid);
    assert!(p.oracle_key_id == oracle_key_id && oracle_public_key.length() == 32, EInvalid);
    let ih = intent_hash(i);
    let ph = proof_hash(p);
    assert!(ed25519::ed25519_verify(actor_signature, &i.actor_public_key, &ih), EInvalid);
    assert!(ed25519::ed25519_verify(oracle_signature, oracle_public_key, &ph), EInvalid);
}

public fun network(i: &IntentV1): vector<u8> { i.network_hash }
public fun operation(i: &IntentV1): u8 { i.operation }
public fun order_id(i: &IntentV1): vector<u8> { i.order_id }
public fun fill_id(i: &IntentV1): vector<u8> { i.fill_id }
public fun buyer_ref(i: &IntentV1): vector<u8> { i.buyer_ref }
public fun seller_ref(i: &IntentV1): vector<u8> { i.seller_ref }
public fun template_id(i: &IntentV1): u32 { i.template_id }
public fun conditions_hash(i: &IntentV1): vector<u8> { i.conditions_hash }
public fun quantity(i: &IntentV1): u64 { i.quantity }
public fun price(i: &IntentV1): u64 { i.price_gold }
public fun expires(i: &IntentV1): u64 { i.expires_at_ms }
public fun rule_version(i: &IntentV1): u64 { i.rule_version }
public fun actor_key(i: &IntentV1): vector<u8> { i.actor_public_key }
public fun lock_id(p: &OracleProofV1): vector<u8> { p.asset_lock_id }
public fun instance_hash(p: &OracleProofV1): vector<u8> { p.asset_instance_hash }
public fun fence(p: &OracleProofV1): u64 { p.fence }
public fun zero(): vector<u8> { vector[0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0] }
