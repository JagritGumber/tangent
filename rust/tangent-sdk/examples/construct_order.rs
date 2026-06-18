//! Minimal example: construct an `Order` and print it.
//!
//! v0.8 will expand this into a full `place_order.rs` that signs via a
//! Circle Dev Wallet and submits through the on-chain `OrderBook`.
//!
//! Run with:
//!   cargo run --example construct_order -p tangent-sdk

use alloy_primitives::Address;
use tangent_sdk::{
    DomainSeparatorInput, MarketDetails, MarketReadSummary, Order, OrderBookMaintenancePlan,
    OrderLifecycleState, OrderLifecycleStatus, OrderParams, OrderPlacementPlan, OrderSignature,
    OrderSigner, OrderSigningRequest, Side, SignedOrder, SignerBackendConfig, SignerBackendKind,
    TxRequestMetadata, UnsignedCall, BASE_SCALE, PRICE_SCALE,
};

#[derive(Debug, Default)]
struct MockOrderSigner;

impl OrderSigner for MockOrderSigner {
    type Error = &'static str;

    fn sign_order(
        &mut self,
        _request: &OrderSigningRequest,
    ) -> Result<OrderSignature, Self::Error> {
        OrderSignature::from_bytes([0u8; OrderSignature::LEN]).map_err(|_| "bad signature")
    }
}

fn main() {
    // A long BTC order: 1 BTC base quantity at $65k limit price.
    // accountId is assigned by AccountManager.registerAccount; marketId by
    // MarketRegistry.registerMarket. limitPrice is in PRICE_SCALE (1e8) units
    // ($65,000 -> 6_500_000_000_000) and size is in 1e18 base units
    // (1 BTC -> 1_000_000_000_000_000_000). nonce is per-account monotonic.
    let btc_market = MarketDetails {
        symbol: "BTC".to_owned(),
        price_feed: Address::repeat_byte(0x11),
        initial_margin_bps: 1_000,
        maint_margin_bps: 500,
        max_leverage: 10,
        tick_size: 100,
        lot_size: 1_000_000_000_000_000,
        max_price_age: 60,
        paused: false,
    };
    let btc_market_summary = MarketReadSummary {
        total_markets: 1,
        mark_price: 65_000 * PRICE_SCALE,
        market: Some(btc_market.clone()),
    };
    let btc_constraints = btc_market.order_constraints();
    let order_params = OrderParams {
        account_id: 7,
        market_id: 1,
        side: Side::Buy,
        limit_price: 65_000 * PRICE_SCALE,
        size: BASE_SCALE,
        nonce: 1,
        expiry: 1_717_000_000,
        reduce_only: false,
    };

    let chain_id = 11111;
    let verifying_contract = Address::ZERO;
    let placement_plan = OrderPlacementPlan::new(
        verifying_contract,
        Address::repeat_byte(0x20),
        chain_id,
        order_params,
        1_716_999_000,
    );
    let order = placement_plan
        .build_order(&btc_market_summary)
        .expect("order matches decoded market reads");
    let constraints_accept_order = btc_constraints.accepts_price(order.limit_price)
        && btc_constraints.accepts_size(order.size);
    let market_is_tradable = placement_plan
        .market_plan
        .is_tradable_market(&btc_market_summary);
    let domain = placement_plan.domain.clone();
    let domain_separator = domain.separator();
    let prepared = placement_plan
        .prepare(&btc_market_summary)
        .expect("order prepares");
    let signing_request = prepared.signing_request();
    let signer_backend =
        SignerBackendConfig::new(SignerBackendKind::CircleDevWallet, "sample-wallet")
            .expect("sample signer backend");
    let external_signing = prepared
        .external_signing_request("order-7-1", signer_backend)
        .expect("external signing request");
    let external_signing_report = external_signing.report();
    let digest_hex = prepared.digest_hex();

    let placement = placement_plan
        .sign_with(&btc_market_summary, &mut MockOrderSigner)
        .expect("mock signer returns a valid EVM signature");
    let lifecycle = placement.lifecycle.clone();
    let signed_order = placement.signed_order().clone();
    let maintenance = OrderBookMaintenancePlan::new(verifying_contract);
    let submit_tx = lifecycle.submit_tx();
    let submit_request = submit_tx.to_request();
    let submit_tx_roundtrip =
        UnsignedCall::from_hex_data(submit_tx.to, &submit_request.data).expect("submit calldata");
    let submit_tx_request = submit_tx.to_tx_request();
    let submit_tx_envelope = submit_tx.to_tx_request_with_metadata(
        TxRequestMetadata::new()
            .with_from(Address::repeat_byte(0x30))
            .with_nonce(7)
            .with_gas(250_000)
            .with_eip1559_fees(2_000_000_000, 1_000_000_000)
            .with_chain_id(chain_id),
    );
    let cancel_tx = lifecycle.cancel_tx();
    let is_live_call = lifecycle.is_live_call();
    let order_of_call = lifecycle.order_of_call();
    let tick_tx = maintenance.tick_tx();
    let sample_lifecycle_status = OrderLifecycleStatus {
        is_live: true,
        stored_order: Some(signed_order.order.clone()),
    };

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
    println!("  side        : {:?}", signed_order.order.side());
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
    println!("  constraints : {constraints_accept_order}");
    println!("  market live : {market_is_tradable}");
    println!("  market check: {}", btc_market.symbol);
    println!();
    println!("EIP-712 type string:");
    println!("  {}", Order::EIP712_TYPE_STRING);
    println!("Order hash:");
    println!("  {}", signed_order.order_hash_hex());
    println!("Signing digest:");
    println!("  {digest_hex}");
    println!("Signing request digest:");
    println!("  {}", signing_request.digest);
    println!("External signing request:");
    println!("  {}", external_signing.request_id);
    println!("External signing report:");
    println!(
        "  {:?} via {:?} ({})",
        external_signing_report.payload_kind,
        external_signing_report.backend_kind,
        external_signing_report.backend_key_id
    );
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
    println!("submitOrder request data:");
    println!("  {}", submit_request.data);
    println!("submitOrder tx value:");
    println!("  {}", submit_tx_request.value);
    println!("submitOrder tx envelope:");
    println!(
        "  from {:?}, nonce {:?}, gas {:?}, maxFee {:?}",
        submit_tx_envelope.from,
        submit_tx_envelope.nonce,
        submit_tx_envelope.gas,
        submit_tx_envelope.max_fee_per_gas
    );
    println!("submitOrder selector match:");
    println!(
        "  {}",
        submit_tx.has_selector(SignedOrder::submit_order_selector())
    );
    println!("submitOrder data roundtrip:");
    println!("  {}", submit_tx_roundtrip == submit_tx);
    println!("submitOrder argument bytes:");
    println!(
        "  {}",
        submit_tx
            .arguments()
            .expect("submitOrder has selector and args")
            .len()
    );
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
    println!("order lifecycle state:");
    println!("  {:?}", sample_lifecycle_status.state());
    println!(
        "  cancel ready: {}",
        sample_lifecycle_status.state() == OrderLifecycleState::Live
            && sample_lifecycle_status.can_cancel()
    );
    println!("tick selector:");
    println!("  {}", tick_tx.selector_hex().expect("tick has selector"));
    println!("maintenance txs:");
    println!("  {} tx", maintenance.transactions().len());
    println!();
    println!(
        "(sign this digest with the account owner; RPC submission lands after full-stack deployment.)"
    );
}
