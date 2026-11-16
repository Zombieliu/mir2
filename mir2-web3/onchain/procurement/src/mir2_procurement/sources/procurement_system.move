// SPDX-License-Identifier: Apache-2.0
module mir2_procurement::procurement_system;

use dubhe::dapp_service::{Self, DappHub, DappStorage, ObjectStorage};
use dubhe::dapp_system;
use mir2_procurement::dapp_key::{Self, DappKey};
use mir2_procurement::intent::{Self, IntentV1, OracleProofV1};
use sui::clock::{Self, Clock};
use sui::event;

const EInvalid: u64 = 101;
const EState: u64 = 102;
const EPermission: u64 = 103;
const EDisabled: u64 = 104;
const OPEN: u8 = 1;
const FILLED: u8 = 2;
const CANCELLED: u8 = 3;
const EXPIRED: u8 = 4;

/// Phantom marker used by the real Dubhe typed shared object implementation.
public struct Order has copy, drop {}
public struct AdminCap has key, store { id: UID }
public struct Config has key {
    id: UID, network_hash: vector<u8>, dapp_storage_id: address,
    oracle_public_key: vector<u8>, oracle_key_id: u64, publishing_enabled: bool,
}

public struct OrderTransition has copy, drop {
    order_id: vector<u8>, object_id: address, status: u8, fill_id: vector<u8>,
    buyer_ref: vector<u8>, seller_ref: vector<u8>, template_id: u32, conditions_hash: vector<u8>,
    quantity: u64, price_gold: u64, rule_version: u64, asset_instance_hash: vector<u8>, fence: u64,
}
public struct VerifierChanged has copy, drop { config_id: ID, key_id: u64, public_key: vector<u8> }
public struct PublicationChanged has copy, drop { config_id: ID, enabled: bool }
public struct FenceRevoked has copy, drop { order_id: vector<u8>, object_id: address, fence: u64 }
public struct RevocationV1 has copy, drop {
    domain: vector<u8>, network_hash: vector<u8>, package_id: address, dapp_storage_id: address,
    order_id: vector<u8>, next_fence: u64, valid_until_ms: u64, nonce: vector<u8>, oracle_key_id: u64,
}

fun init(ctx: &mut TxContext) {
    transfer::transfer(AdminCap { id: object::new(ctx) }, ctx.sender());
}

/// First deployment only. It cannot be called without the package's deployment capability.
/// There are no gold/points tokens, equipment NFTs or marketplace payments.
/// create_dapp creates ds in this transaction, so sharing this fresh return is safe.
#[allow(lint(share_owned))]
public fun genesis(_cap: &AdminCap, hub: &mut DappHub, clock: &Clock,
    network_hash: vector<u8>, oracle_public_key: vector<u8>, oracle_key_id: u64, ctx: &mut TxContext) {
    assert!(network_hash.length() == 32 && oracle_public_key.length() == 32 && oracle_key_id > 0, EInvalid);
    let ds = dapp_system::create_dapp(dapp_key::new(), hub,
        std::ascii::string(b"mir2_procurement"), std::ascii::string(b"Server-custodied personal procurement"),
        0, clock, ctx);
    let config = Config { id: object::new(ctx), network_hash,
        dapp_storage_id: object::uid_to_address(dapp_service::dapp_storage_id(&ds)),
        oracle_public_key, oracle_key_id, publishing_enabled: false };
    transfer::share_object(config);
    transfer::public_share_object(ds);
}

public fun set_publishing_enabled(_cap: &AdminCap, config: &mut Config, enabled: bool) {
    config.publishing_enabled = enabled;
    event::emit(PublicationChanged { config_id: object::id(config), enabled });
}

/// Rotation invalidates proofs under the old key; the confirmed config state is a release fence.
/// This grants no ability to edit server balances or transfer any game asset.
public fun rotate_verifier(_cap: &AdminCap, config: &mut Config, public_key: vector<u8>, key_id: u64) {
    assert!(public_key.length() == 32 && key_id > config.oracle_key_id, EInvalid);
    config.oracle_public_key = public_key;
    config.oracle_key_id = key_id;
    event::emit(VerifierChanged { config_id: object::id(config), key_id, public_key });
}

fun guard(ds: &DappStorage, config: &Config) {
    // Deliberately do not enforce publication pause here: existing orders must remain recoverable.
    dapp_system::ensure_latest_version<DappKey>(ds, 1);
    assert!(object::uid_to_address(dapp_service::dapp_storage_id(ds)) == config.dapp_storage_id, EInvalid);
}

fun verify(config: &Config, i: &IntentV1, p: &OracleProofV1, actor_sig: &vector<u8>,
    oracle_sig: &vector<u8>, clock: &Clock) {
    intent::verify(i, p, actor_sig, oracle_sig, &config.oracle_public_key, config.oracle_key_id,
        &config.network_hash, dapp_key::package_id(), config.dapp_storage_id, clock::timestamp_ms(clock));
}

/// Phase 1 registers uniqueness and shares an empty ObjectStorage. It is NOT yet an advertised order.
/// The persistent outbox must retain the escrow and quota until activation or a confirmed terminal result.
public fun create_order(ds: &mut DappStorage, config: &Config, clock: &Clock, i: IntentV1,
    p: OracleProofV1, actor_sig: vector<u8>, oracle_sig: vector<u8>, ctx: &mut TxContext) {
    guard(ds, config);
    dapp_system::ensure_not_paused<DappKey>(ds);
    assert!(config.publishing_enabled, EDisabled);
    verify(config, &i, &p, &actor_sig, &oracle_sig, clock);
    assert!(intent::operation(&i) == 1 && intent::fill_id(&i) == intent::zero(), EInvalid);
    assert!(intent::seller_ref(&i) == intent::zero() && intent::instance_hash(&p) == intent::zero() && intent::fence(&p) == 0, EInvalid);
    let oid = intent::order_id(&i);
    // Duplicate transaction bytes are replay-safe at Sui. A different create transaction
    // for the same order aborts in Dubhe's entity registry; recovery reads that registry.
    dapp_system::create_and_share_typed_object<DappKey, Order>(dapp_key::new(), ds, b"order", oid, ctx);
}

/// Phase 2 writes all conditions atomically and then emits OPEN. It also succeeds while publication is paused.
public fun activate_order(ds: &DappStorage, config: &Config, order: &mut ObjectStorage<Order>, clock: &Clock,
    i: IntentV1, p: OracleProofV1, actor_sig: vector<u8>, oracle_sig: vector<u8>, conditions_bcs: vector<u8>) {
    guard(ds, config);
    verify(config, &i, &p, &actor_sig, &oracle_sig, clock);
    assert!(intent::operation(&i) == 1 && intent::fill_id(&i) == intent::zero() && intent::seller_ref(&i) == intent::zero(), EInvalid);
    assert!(intent::instance_hash(&p) == intent::zero() && intent::fence(&p) == 0, EInvalid);
    assert!(conditions_bcs.length() >= 5 && conditions_bcs.length() <= 512
        && std::hash::sha2_256(conditions_bcs) == intent::conditions_hash(&i), EInvalid);
    assert_order_id(order, intent::order_id(&i));
    if (has<u8>(order, b"status")) {
        assert!(get<vector<u8>>(order, b"publish_intent_hash") == intent::intent_hash(&i), EInvalid);
        emit_transition(order);
        return
    };
    set(order, b"publish_intent_hash", intent::intent_hash(&i));
    set(order, b"buyer_ref", intent::buyer_ref(&i));
    set(order, b"buyer_public_key", intent::actor_key(&i));
    set(order, b"template_id", intent::template_id(&i));
    set(order, b"conditions_hash", intent::conditions_hash(&i));
    set(order, b"conditions_bcs", conditions_bcs);
    set(order, b"quantity", intent::quantity(&i));
    set(order, b"price_gold", intent::price(&i));
    set(order, b"expires_at_ms", intent::expires(&i));
    set(order, b"rule_version", intent::rule_version(&i));
    set(order, b"escrow_lock_id", intent::lock_id(&p));
    set(order, b"status", OPEN);
    set(order, b"fence", 0u64);
    set(order, b"fill_id", intent::zero());
    set(order, b"seller_ref", intent::zero());
    set(order, b"asset_instance_hash", intent::zero());
    emit_transition(order);
}

fun assert_terms(order: &ObjectStorage<Order>, i: &IntentV1) {
    assert_order_id(order, intent::order_id(i));
    assert!(get<vector<u8>>(order, b"buyer_ref") == intent::buyer_ref(i), EInvalid);
    assert!(get<u32>(order, b"template_id") == intent::template_id(i), EInvalid);
    assert!(get<vector<u8>>(order, b"conditions_hash") == intent::conditions_hash(i), EInvalid);
    assert!(get<u64>(order, b"quantity") == intent::quantity(i) && get<u64>(order, b"price_gold") == intent::price(i), EInvalid);
    assert!(get<u64>(order, b"expires_at_ms") == intent::expires(i) && get<u64>(order, b"rule_version") == intent::rule_version(i), EInvalid);
}

public fun fill_order(ds: &DappStorage, config: &Config, order: &mut ObjectStorage<Order>, clock: &Clock,
    i: IntentV1, p: OracleProofV1, actor_sig: vector<u8>, oracle_sig: vector<u8>) {
    guard(ds, config);
    verify(config, &i, &p, &actor_sig, &oracle_sig, clock);
    assert!(intent::operation(&i) == 2 && intent::fill_id(&i) != intent::zero(), EInvalid);
    assert!(intent::seller_ref(&i) != intent::buyer_ref(&i) && intent::seller_ref(&i) != intent::zero(), EInvalid);
    assert!(intent::instance_hash(&p) != intent::zero(), EInvalid);
    assert_terms(order, &i);
    if (get<u8>(order, b"status") == FILLED) {
        assert!(get<vector<u8>>(order, b"fill_id") == intent::fill_id(&i)
            && get<vector<u8>>(order, b"seller_ref") == intent::seller_ref(&i)
            && get<vector<u8>>(order, b"asset_instance_hash") == intent::instance_hash(&p), EState);
        emit_transition(order);
        return
    };
    assert_can_decide(get<u8>(order, b"status"), FILLED, clock::timestamp_ms(clock), get<u64>(order, b"expires_at_ms"));
    assert!(intent::fence(&p) == get<u64>(order, b"fence"), EState);
    set(order, b"status", FILLED);
    set(order, b"fill_id", intent::fill_id(&i));
    set(order, b"seller_ref", intent::seller_ref(&i));
    set(order, b"asset_instance_hash", intent::instance_hash(&p));
    emit_transition(order);
}

public fun cancel_order(ds: &DappStorage, config: &Config, order: &mut ObjectStorage<Order>, clock: &Clock,
    i: IntentV1, p: OracleProofV1, actor_sig: vector<u8>, oracle_sig: vector<u8>) {
    guard(ds, config);
    verify(config, &i, &p, &actor_sig, &oracle_sig, clock);
    assert!(intent::operation(&i) == 3 && intent::fill_id(&i) == intent::zero() && intent::seller_ref(&i) == intent::zero(), EInvalid);
    assert_terms(order, &i);
    // The Oracle attests the authenticated buyer_ref -> this confirmation key binding.
    // A fresh purpose key after a client restart must not strand the buyer's order.
    assert!(intent::lock_id(&p) == get<vector<u8>>(order, b"escrow_lock_id"), EInvalid);
    if (get<u8>(order, b"status") == CANCELLED) { emit_transition(order); return };
    assert_can_decide(get<u8>(order, b"status"), CANCELLED, clock::timestamp_ms(clock), get<u64>(order, b"expires_at_ms"));
    assert!(intent::fence(&p) == get<u64>(order, b"fence"), EState);
    set(order, b"status", CANCELLED);
    emit_transition(order);
}

/// Anyone may finalize expiry; trusted Clock and the same mutable order arbitrate against fill/cancel.
public fun expire_order(ds: &DappStorage, config: &Config, order: &mut ObjectStorage<Order>, clock: &Clock) {
    guard(ds, config);
    if (get<u8>(order, b"status") == EXPIRED) { emit_transition(order); return };
    assert_can_decide(get<u8>(order, b"status"), EXPIRED, clock::timestamp_ms(clock), get<u64>(order, b"expires_at_ms"));
    set(order, b"status", EXPIRED);
    emit_transition(order);
}

/// Confirm this transaction before unlocking any still-pending seller proof. Never use local timeout.
public fun revoke_fence(ds: &DappStorage, config: &Config, order: &mut ObjectStorage<Order>, clock: &Clock,
    next_fence: u64, valid_until_ms: u64, nonce: vector<u8>, oracle_sig: vector<u8>) {
    guard(ds, config);
    assert!(nonce.length() == 32 && clock::timestamp_ms(clock) < valid_until_ms, EInvalid);
    let revocation = RevocationV1 { domain: b"MIR2_PROCUREMENT_REVOKE_V1", network_hash: config.network_hash,
        package_id: dapp_key::package_id(), dapp_storage_id: config.dapp_storage_id,
        order_id: *dapp_service::object_storage_entity_id(order),
        next_fence, valid_until_ms, nonce, oracle_key_id: config.oracle_key_id };
    let bytes = sui::bcs::to_bytes(&revocation);
    let message_hash = std::hash::sha2_256(bytes);
    assert!(sui::ed25519::ed25519_verify(&oracle_sig, &config.oracle_public_key, &message_hash), EPermission);
    assert!(get<u8>(order, b"status") == OPEN && next_fence == get<u64>(order, b"fence") + 1, EState);
    set(order, b"fence", next_fence);
    event::emit(FenceRevoked { order_id: *dapp_service::object_storage_entity_id(order),
        object_id: object::uid_to_address(dapp_service::object_storage_id(order)), fence: next_fence });
}

fun assert_order_id(order: &ObjectStorage<Order>, oid: vector<u8>) {
    assert!(dapp_service::object_storage_dapp_key(order) == dapp_system::dapp_key<DappKey>(), EInvalid);
    assert!(*dapp_service::object_storage_type(order) == b"order", EInvalid);
    assert!(*dapp_service::object_storage_entity_id(order) == oid, EInvalid);
}
public(package) fun assert_can_decide(current: u8, decision: u8, now: u64, expires: u64) {
    assert!(current == OPEN, EState);
    assert!((decision == EXPIRED && now >= expires)
        || ((decision == FILLED || decision == CANCELLED) && now < expires), EState);
}
fun set<T: copy + drop + store>(o: &mut ObjectStorage<Order>, k: vector<u8>, v: T) {
    dapp_system::set_object_field<DappKey, Order, T>(dapp_key::new(), o, k, v);
}
fun get<T: copy + drop + store>(o: &ObjectStorage<Order>, k: vector<u8>): T {
    dapp_system::get_object_field<Order, T>(o, k)
}
fun has<T: copy + drop + store>(o: &ObjectStorage<Order>, k: vector<u8>): bool {
    dapp_system::has_object_field<Order, T>(o, k)
}
public fun status(o: &ObjectStorage<Order>): u8 { if (has<u8>(o, b"status")) { get(o, b"status") } else { 0 } }
public fun fence(o: &ObjectStorage<Order>): u64 { if (has<u64>(o, b"fence")) { get(o, b"fence") } else { 0 } }

fun emit_transition(o: &ObjectStorage<Order>) {
    event::emit(OrderTransition { order_id: *dapp_service::object_storage_entity_id(o),
        object_id: object::uid_to_address(dapp_service::object_storage_id(o)),
        status: get(o, b"status"), fill_id: get(o, b"fill_id"), buyer_ref: get(o, b"buyer_ref"),
        seller_ref: get(o, b"seller_ref"), template_id: get(o, b"template_id"),
        conditions_hash: get(o, b"conditions_hash"), quantity: get(o, b"quantity"),
        price_gold: get(o, b"price_gold"), rule_version: get(o, b"rule_version"),
        asset_instance_hash: get(o, b"asset_instance_hash"), fence: get(o, b"fence") });
}
