//! Minimal example: construct an `Order` and print it.
//!
//! v0.8 will expand this into a full `place_order.rs` that signs via a
//! Circle Dev Wallet and submits through the on-chain `OrderBook`.
//!
//! Run with:
//!   cargo run --example construct_order -p tangent-sdk

use alloy_primitives::Address;
use tangent_sdk::{
    DomainSeparatorInput, Order, OrderBookMaintenancePlan, OrderConstraints, OrderLifecyclePlan,
    OrderParams, OrderSignature, Side, BASE_SCALE, PRICE_SCALE,
};

fn main() {
    // A long BTC order: 1 BTC base quantity at $65k limit price.
    // accountId is assigned by AccountManager.registerAccount; marketId by
    // MarketRegistry.registerMarket. limitPrice is in PRICE_SCALE (1e8) units
    // ($65,000 -> 6_500_000_000_000) and size is in 1e18 base units
    // (1 BTC -> 1_000_000_000_000_000_000). nonce is per-account monotonic.
    let btc_constraints = OrderConstraints::new(100, 1_000_000_000_000_000);
    let order = OrderParams {
        account_id: 7,
        market_id: 1,
        side: Side::Buy,
        limit_price: 65_000 * PRICE_SCALE,
        size: BASE_SCALE,
        nonce: 1,
        expiry: 1_717_000_000,
        reduce_only: false,
    }
    .build(btc_constraints, 1_716_999_000)
    .expect("valid order");

    let chain_id = 11111;
    let verifying_contract = Address::ZERO;
    let domain = DomainSeparatorInput::new(chain_id, verifying_contract);
    let domain_separator = domain.separator();
    let order_hash = order.order_hash();
    let prepared = order.prepare(domain);
    let digest = prepared.digest;

    // Placeholder signature shape. In production this comes from the account
    // owner wallet after signing `prepared.digest`.
    let signature = OrderSignature::from_bytes([0u8; OrderSignature::LEN]).expect("valid shape");
    let signed_order = prepared.attach_signature(signature);
    let lifecycle = OrderLifecyclePlan::new(verifying_contract, signed_order.clone());
    let maintenance = OrderBookMaintenancePlan::new(verifying_contract);
    let submit_tx = lifecycle.submit_tx();
    let cancel_tx = lifecycle.cancel_tx();
    let is_live_call = lifecycle.is_live_call();
    let order_of_call = lifecycle.order_of_call();
    let tick_tx = maintenance.tick_tx();

    println!("=== tangent-sdk example: constructed order ===");
    println!("EIP-712 domain:");
    println!("  name             : {}", DomainSeparatorInput::NAME);
    println!("  version          : {}", DomainSeparatorInput::VERSION);
    println!("  chainId          : {chain_id}");
    println!("  verifyingContract: {verifying_contract}");
    println!("  domainSeparator  : 0x{}", hex::encode(domain_separator));
    println!();
    println!("Order:");
    println!("  accountId   : {}", signed_order.order.account_id);
    println!("  marketId    : {}", signed_order.order.market_id);
    println!("  isBuy       : {}", signed_order.order.is_buy);
    println!(
        "  limitPrice  : {} (PRICE_SCALE=1e8)",
        signed_order.order.limit_price
    );
    println!(
        "  size        : {} (1e18 base units)",
        signed_order.order.size
    );
    println!("  nonce       : {}", signed_order.order.nonce);
    println!("  expiry      : {}", signed_order.order.expiry);
    println!("  reduceOnly  : {}", signed_order.order.reduce_only);
    println!();
    println!("EIP-712 type string:");
    println!("  {}", Order::EIP712_TYPE_STRING);
    println!("Order hash:");
    println!("  0x{}", hex::encode(order_hash));
    println!("Signing digest:");
    println!("  0x{}", hex::encode(digest));
    println!("Signature bytes:");
    println!("  {}", signed_order.signature.to_hex());
    println!("submitOrder selector:");
    println!(
        "  {}",
        submit_tx.selector_hex().expect("submitOrder has selector")
    );
    println!("submitOrder calldata bytes:");
    println!("  {}", submit_tx.data_len());
    println!("submitOrder tx target:");
    println!("  {}", submit_tx.to);
    println!("cancelOrder selector:");
    println!(
        "  {}",
        cancel_tx.selector_hex().expect("cancelOrder has selector")
    );
    println!("cancelOrder calldata:");
    println!("  {}", cancel_tx.data_hex());
    println!("isLive selector:");
    println!(
        "  {}",
        is_live_call.selector_hex().expect("isLive has selector")
    );
    println!("orderOf selector:");
    println!(
        "  {}",
        order_of_call.selector_hex().expect("orderOf has selector")
    );
    println!("order lifecycle reads:");
    println!("  {} calls", lifecycle.calls().len());
    println!("tick selector:");
    println!("  {}", tick_tx.selector_hex().expect("tick has selector"));
    println!("maintenance txs:");
    println!("  {} tx", maintenance.transactions().len());
    println!();
    println!(
        "(sign this digest with the account owner; RPC submission lands after full-stack deployment.)"
    );
}
