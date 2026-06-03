//! Minimal example: construct an `Order` and print it.
//!
//! v0.8 will expand this into a full `place_order.rs` that signs via a
//! Circle Dev Wallet and submits through the on-chain `OrderBook`.
//!
//! Run with:
//!   cargo run --example construct_order -p tangent-sdk

use alloy_primitives::Address;
use tangent_sdk::{
    DomainSeparatorInput, Order, OrderConstraints, OrderParams, Side, BASE_SCALE, PRICE_SCALE,
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

    let domain = DomainSeparatorInput::new(11111, Address::ZERO);
    let domain_separator = domain.separator();
    let order_hash = order.struct_hash();
    let digest = order.digest(&domain);

    println!("=== tangent-sdk example: constructed order ===");
    println!("EIP-712 domain:");
    println!("  name             : {}", DomainSeparatorInput::NAME);
    println!("  version          : {}", DomainSeparatorInput::VERSION);
    println!("  chainId          : {}", domain.chain_id);
    println!("  verifyingContract: {}", domain.verifying_contract);
    println!("  domainSeparator  : 0x{}", hex::encode(domain_separator));
    println!();
    println!("Order:");
    println!("  accountId   : {}", order.account_id);
    println!("  marketId    : {}", order.market_id);
    println!("  isBuy       : {}", order.is_buy);
    println!("  limitPrice  : {} (PRICE_SCALE=1e8)", order.limit_price);
    println!("  size        : {} (1e18 base units)", order.size);
    println!("  nonce       : {}", order.nonce);
    println!("  expiry      : {}", order.expiry);
    println!("  reduceOnly  : {}", order.reduce_only);
    println!();
    println!("EIP-712 type string:");
    println!("  {}", Order::EIP712_TYPE_STRING);
    println!("Order hash:");
    println!("  0x{}", hex::encode(order_hash));
    println!("Signing digest:");
    println!("  0x{}", hex::encode(digest));
    println!();
    println!(
        "(sign this digest with the account owner; RPC submission lands after full-stack deployment.)"
    );
}
