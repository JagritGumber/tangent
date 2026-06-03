//! Minimal example: construct an `Order` and print it.
//!
//! v0.8 will expand this into a full `place_order.rs` that signs via a
//! Circle Dev Wallet and submits through the on-chain `OrderBook`.
//!
//! Run with:
//!   cargo run --example construct_order -p tangent-sdk

use tangent_sdk::{DomainSeparatorInput, Order, OrderConstraints, Side, BASE_SCALE, PRICE_SCALE};

fn main() {
    // A long BTC order: 1 BTC notional at $65k limit price.
    // accountId is assigned by AccountManager.registerAccount; marketId by
    // MarketRegistry.registerMarket. limitPrice is in PRICE_SCALE (1e8) units
    // ($65,000 -> 6_500_000_000_000) and size is in 1e18 base units
    // (1 BTC -> 1_000_000_000_000_000_000). nonce is per-account monotonic.
    let btc_constraints = OrderConstraints::new(100, 1_000_000_000_000_000);
    let order = Order::builder()
        .account_id(7)
        .market_id(1)
        .side(Side::Buy)
        .limit_price(65_000 * PRICE_SCALE)
        .size(BASE_SCALE)
        .nonce(1)
        .expiry(1_717_000_000)
        .build(btc_constraints, 1_716_999_000)
        .expect("valid order");

    let domain = DomainSeparatorInput::new(11111, [0u8; 20]);
    let verifying_hex = hex::encode(domain.verifying_contract);

    println!("=== tangent-sdk example: constructed order ===");
    println!("EIP-712 domain:");
    println!("  name             : {}", DomainSeparatorInput::NAME);
    println!("  version          : {}", DomainSeparatorInput::VERSION);
    println!("  chainId          : {}", domain.chain_id);
    println!("  verifyingContract: 0x{verifying_hex}");
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
    println!();
    println!(
        "(v0.8 will add signing + RPC submission; this example only constructs the typed payload.)"
    );
}
