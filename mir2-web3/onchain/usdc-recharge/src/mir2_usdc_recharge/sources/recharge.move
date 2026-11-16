// SPDX-License-Identifier: Apache-2.0
module mir2_usdc_recharge::recharge;

use std::ascii::{Self, String};
use std::type_name;
use sui::clock::{Self, Clock};
use sui::coin::{Self, Coin};
use sui::event;

const EInvalid: u64 = 101;
const EPermission: u64 = 102;
const EExpired: u64 = 103;
const EPaid: u64 = 104;
const EDisabled: u64 = 105;
const EWrongCoin: u64 = 106;
const EAmount: u64 = 107;
// Circle-issued native Sui Testnet USDC, not a symbol or bridged lookalike.
const USDC_TYPE: vector<u8> = b"a1ec7fc00a6f40db9693ad1415d0c193ad3906494428cf252621037bd7117e29::usdc::USDC";
// SHA256(UTF8("4c78adac")); the driver also verifies the fullnode chain identifier.
const NETWORK_HASH: vector<u8> = x"b55d4afda0318ec0a264cc5f46c4e61c96da8000e9ea1bdf04878266867c6246";
const MAX_LIFETIME_MS: u64 = 604800000;

public struct AdminCap has key, store { id: UID, issuer: address }
public struct Config has key {
    id: UID, admin_cap_id: ID, issuer: address, network_hash: vector<u8>,
    coin_type: String, merchant: address, publishing_enabled: bool,
}
/// One mutable object per order; no global payment registry is mutated by pay.
public struct Invoice has key {
    id: UID, config_id: ID, network_hash: vector<u8>, order_ref: vector<u8>,
    account_ref: vector<u8>, snapshot_hash: vector<u8>, payer: address, merchant: address,
    coin_type: String, amount_micros: u64, expires_at_ms: u64, paid: bool, paid_at_ms: u64,
}
public struct InvoiceCreated has copy, drop {
    invoice_id: ID, config_id: ID, network_hash: vector<u8>, order_ref: vector<u8>,
    account_ref: vector<u8>, snapshot_hash: vector<u8>, payer: address, merchant: address,
    coin_type: String, amount_micros: u64, expires_at_ms: u64, issuer: address, created_at_ms: u64,
}
public struct PaymentReceived has copy, drop {
    invoice_id: ID, config_id: ID, network_hash: vector<u8>, order_ref: vector<u8>,
    account_ref: vector<u8>, snapshot_hash: vector<u8>, payer: address, merchant: address,
    coin_type: String, amount_micros: u64, expires_at_ms: u64, paid_at_ms: u64,
}
public struct PublicationChanged has copy, drop { config_id: ID, enabled: bool }

fun init(ctx: &mut TxContext) {
    transfer::transfer(AdminCap { id: object::new(ctx), issuer: ctx.sender() }, ctx.sender());
}

public fun create_config(cap: &AdminCap, network_hash: vector<u8>, merchant: address, ctx: &mut TxContext) {
    assert!(cap.issuer == ctx.sender(), EPermission);
    assert!(network_hash == NETWORK_HASH && merchant != @0x0, EInvalid);
    transfer::share_object(Config { id: object::new(ctx), admin_cap_id: object::id(cap),
        issuer: cap.issuer, network_hash, coin_type: ascii::string(USDC_TYPE), merchant,
        publishing_enabled: false });
}
public fun set_publishing_enabled(cap: &AdminCap, config: &mut Config, enabled: bool, ctx: &TxContext) {
    assert!(config.admin_cap_id == object::id(cap) && config.issuer == ctx.sender(), EPermission);
    config.publishing_enabled = enabled;
    event::emit(PublicationChanged { config_id: object::id(config), enabled });
}

/// The backend must durably save the exact signed create transaction before broadcasting.
/// A database order binds the resulting invoice ID exactly once. Duplicate invoice issuance
/// is not prevented by a global onchain registry and is therefore an issuer trust boundary.
public fun create_invoice(cap: &AdminCap, config: &Config, clock: &Clock,
    order_ref: vector<u8>, account_ref: vector<u8>, snapshot_hash: vector<u8>, payer: address,
    amount_micros: u64, expires_at_ms: u64, ctx: &mut TxContext) {
    assert!(config.admin_cap_id == object::id(cap) && config.issuer == ctx.sender(), EPermission);
    assert!(config.publishing_enabled, EDisabled);
    let now = clock::timestamp_ms(clock);
    assert!(order_ref.length() == 32 && account_ref.length() == 32 && snapshot_hash.length() == 32
        && payer != @0x0 && payer != config.merchant && amount_micros > 0, EInvalid);
    assert!(expires_at_ms > now && expires_at_ms - now <= MAX_LIFETIME_MS, EExpired);
    let invoice = Invoice { id: object::new(ctx), config_id: object::id(config),
        network_hash: config.network_hash, order_ref, account_ref, snapshot_hash, payer,
        merchant: config.merchant, coin_type: config.coin_type, amount_micros,
        expires_at_ms, paid: false, paid_at_ms: 0 };
    event::emit(InvoiceCreated { invoice_id: object::id(&invoice), config_id: invoice.config_id,
        network_hash: invoice.network_hash, order_ref: invoice.order_ref, account_ref: invoice.account_ref,
        snapshot_hash: invoice.snapshot_hash, payer: invoice.payer, merchant: invoice.merchant,
        coin_type: invoice.coin_type, amount_micros, expires_at_ms, issuer: config.issuer, created_at_ms: now });
    transfer::share_object(invoice);
}

/// Pausing new invoices does not strand an existing payment or its later DB delivery.
/// The wallet signs a normal Sui transaction and pays SUI gas in this first version.
public fun pay<T>(config: &Config, invoice: &mut Invoice, payment: Coin<T>, clock: &Clock, ctx: &TxContext) {
    assert!(invoice.config_id == object::id(config) && invoice.network_hash == config.network_hash
        && invoice.merchant == config.merchant && invoice.coin_type == config.coin_type, EInvalid);
    assert!(ctx.sender() == invoice.payer, EPermission);
    assert!(!invoice.paid, EPaid);
    let now = clock::timestamp_ms(clock);
    assert!(now < invoice.expires_at_ms, EExpired);
    assert!(type_name::with_original_ids<T>().into_string() == invoice.coin_type, EWrongCoin);
    assert!(coin::value(&payment) == invoice.amount_micros, EAmount);
    transfer::public_transfer(payment, invoice.merchant);
    invoice.paid = true;
    invoice.paid_at_ms = now;
    event::emit(PaymentReceived { invoice_id: object::id(invoice), config_id: invoice.config_id,
        network_hash: invoice.network_hash, order_ref: invoice.order_ref, account_ref: invoice.account_ref,
        snapshot_hash: invoice.snapshot_hash, payer: invoice.payer, merchant: invoice.merchant,
        coin_type: invoice.coin_type, amount_micros: invoice.amount_micros,
        expires_at_ms: invoice.expires_at_ms, paid_at_ms: now });
}

#[test_only]
fun pay_case(sender: address, payer: address, paid: bool, now: u64, value: u64,
    wrong_coin: bool, wrong_config: bool) {
    use sui::test_scenario;
    use sui::sui::SUI;
    let mut scenario = test_scenario::begin(sender);
    let ctx = scenario.ctx();
    let coin_type = if (wrong_coin) ascii::string(USDC_TYPE)
        else type_name::with_original_ids<SUI>().into_string();
    let config = Config { id: object::new(ctx), admin_cap_id: object::id_from_address(@0x777),
        issuer: @0x111, network_hash: NETWORK_HASH, coin_type, merchant: @0x888,
        publishing_enabled: false };
    let mut invoice = Invoice { id: object::new(ctx),
        config_id: if (wrong_config) object::id_from_address(@0x999) else object::id(&config),
        network_hash: NETWORK_HASH,
        order_ref: x"1111111111111111111111111111111111111111111111111111111111111111",
        account_ref: x"2222222222222222222222222222222222222222222222222222222222222222",
        snapshot_hash: x"3333333333333333333333333333333333333333333333333333333333333333",
        payer, merchant: @0x888, coin_type, amount_micros: 1234567,
        expires_at_ms: 20000, paid, paid_at_ms: 0 };
    let mut clock = clock::create_for_testing(ctx);
    clock::set_for_testing(&mut clock, now);
    let coin = coin::mint_for_testing<SUI>(value, ctx);
    pay(&config, &mut invoice, coin, &clock, ctx);
    assert!(invoice.paid && invoice.paid_at_ms == now, 900);
    transfer::share_object(invoice);
    transfer::share_object(config);
    clock::destroy_for_testing(clock);
    scenario.next_tx(@0x888);
    let paid_coin = scenario.take_from_sender<Coin<SUI>>();
    assert!(coin::value(&paid_coin) == 1234567, 901);
    coin::burn_for_testing(paid_coin);
    scenario.end();
}

#[test]
fun exact_coin_delivered_once_even_when_publication_paused() { pay_case(@0x111, @0x111, false, 10000, 1234567, false, false); }
#[test]
#[expected_failure(abort_code=102,location=mir2_usdc_recharge::recharge)]
fun foreign_wallet_cannot_pay_bound_invoice() { pay_case(@0x112, @0x111, false, 10000, 1234567, false, false); }
#[test]
#[expected_failure(abort_code=104,location=mir2_usdc_recharge::recharge)]
fun concurrent_or_repeated_payment_cannot_charge_twice() { pay_case(@0x111, @0x111, true, 10000, 1234567, false, false); }
#[test]
#[expected_failure(abort_code=103,location=mir2_usdc_recharge::recharge)]
fun deadline_is_exclusive() { pay_case(@0x111, @0x111, false, 20000, 1234567, false, false); }
#[test]
#[expected_failure(abort_code=107,location=mir2_usdc_recharge::recharge)]
fun underpayment_rejected() { pay_case(@0x111, @0x111, false, 10000, 1234566, false, false); }
#[test]
#[expected_failure(abort_code=107,location=mir2_usdc_recharge::recharge)]
fun overpayment_rejected() { pay_case(@0x111, @0x111, false, 10000, 1234568, false, false); }
#[test]
#[expected_failure(abort_code=106,location=mir2_usdc_recharge::recharge)]
fun wrong_coin_type_rejected() { pay_case(@0x111, @0x111, false, 10000, 1234567, true, false); }
#[test]
#[expected_failure(abort_code=101,location=mir2_usdc_recharge::recharge)]
fun foreign_config_rejected() { pay_case(@0x111, @0x111, false, 10000, 1234567, false, true); }

#[test_only]
fun issue_case(sender: address, enabled: bool, payer: address, expires: u64, wrong_cap: bool) {
    use sui::test_scenario;
    let mut scenario = test_scenario::begin(sender);
    let ctx = scenario.ctx();
    let cap = AdminCap { id: object::new(ctx), issuer: @0x111 };
    let config = Config { id: object::new(ctx),
        admin_cap_id: if (wrong_cap) object::id_from_address(@0x999) else object::id(&cap),
        issuer: @0x111, network_hash: NETWORK_HASH, coin_type: ascii::string(USDC_TYPE),
        merchant: @0x888, publishing_enabled: enabled };
    let mut clock = clock::create_for_testing(ctx);
    clock::set_for_testing(&mut clock, 10000);
    create_invoice(&cap, &config, &clock,
        x"1111111111111111111111111111111111111111111111111111111111111111",
        x"2222222222222222222222222222222222222222222222222222222222222222",
        x"3333333333333333333333333333333333333333333333333333333333333333",
        payer, 1234567, expires, ctx);
    transfer::public_transfer(cap, @0x111);
    transfer::share_object(config);
    clock::destroy_for_testing(clock);
    scenario.next_tx(@0x111);
    let invoice = scenario.take_shared<Invoice>();
    assert!(!invoice.paid && invoice.amount_micros == 1234567 && invoice.payer == payer
        && invoice.expires_at_ms == expires && invoice.merchant == @0x888, 902);
    test_scenario::return_shared(invoice);
    scenario.end();
}
#[test]
fun authorized_issuer_freezes_invoice_and_shares_per_order() { issue_case(@0x111, true, @0x222, 20000, false); }
#[test]
#[expected_failure(abort_code=102,location=mir2_usdc_recharge::recharge)]
fun foreign_issuer_rejected() { issue_case(@0x112, true, @0x222, 20000, false); }
#[test]
#[expected_failure(abort_code=102,location=mir2_usdc_recharge::recharge)]
fun foreign_capability_rejected() { issue_case(@0x111, true, @0x222, 20000, true); }
#[test]
#[expected_failure(abort_code=105,location=mir2_usdc_recharge::recharge)]
fun disabled_publication_rejected() { issue_case(@0x111, false, @0x222, 20000, false); }
#[test]
#[expected_failure(abort_code=103,location=mir2_usdc_recharge::recharge)]
fun expired_creation_rejected() { issue_case(@0x111, true, @0x222, 10000, false); }
#[test]
#[expected_failure(abort_code=101,location=mir2_usdc_recharge::recharge)]
fun merchant_cannot_create_self_payment() { issue_case(@0x111, true, @0x888, 20000, false); }
