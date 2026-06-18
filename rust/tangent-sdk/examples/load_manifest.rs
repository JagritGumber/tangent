//! Minimal example: load the checked-in Arc Testnet deployment manifest.
//!
//! Run with:
//!   cargo run --example load_manifest -p tangent-sdk

use alloy_primitives::{Address, B256};
use tangent_sdk::{
    AccountOnboardingNextStep, AccountRegisteredEvent, CallReturn, CallReturnBatch,
    CollateralDepositNextStep, CollateralStatus, CollateralWithdrawNextStep, DeploymentManifest,
    EventFilterSet, JsonRpcRequest, JsonRpcResponse, KeeperPollingOutcome, KeeperPollingPolicy,
    KeeperPollingSnapshot, LiquidationNextStep, LiquidationReadPlan, LiquidationStatus,
    MarginStatus, PositionStatus, RawLog, RawLogMetadata, RpcBlockTag, RpcEndpointConfig,
    SettlementReadPlan, SettlementStatus, SettlementWithdrawalNextStep, SignedRawTransaction,
    TangentClientPlan, TangentContext, TangentEvent, TxBatchRequestMetadata, TxConfirmationPlan,
    TxConfirmationPolicy, TxHash, TxPreflight, TxReceipt, TxSubmissionPlan, USDCVaultCalls,
    UnsignedCall,
};

fn main() {
    let manifest =
        DeploymentManifest::from_json(include_str!("../../../docs/deployments/arc-testnet.json"))
            .expect("valid deployment manifest");
    let context = TangentContext::new(manifest.clone());
    let client_plan = TangentClientPlan::new(
        manifest.clone(),
        RpcEndpointConfig::new("https://rpc.arc.example").expect("sample endpoint"),
    )
    .expect("client plan");

    println!("=== tangent-sdk example: deployment manifest ===");
    println!("project : {}", manifest.project);
    println!("network : {}", manifest.network);
    println!("chainId : {}", manifest.chain_id);
    println!("context chainId : {}", context.chain_id());
    println!(
        "client chain match : {}",
        client_plan.chain_id_matches_manifest()
    );
    println!(
        "client endpoint secure : {}",
        client_plan.config.endpoint.is_secure()
    );
    println!("client fee policy : {:?}", client_plan.config.policies.fee);
    let client_config_report = client_plan.config.report();
    println!(
        "client config report : {:?}, {} signer backends",
        client_config_report.endpoint.scheme, client_config_report.signer_backend_count
    );
    println!(
        "client full stack ready : {}",
        client_plan.startup_report().readiness.full_perp_stack
    );
    let perp_stack = manifest.perp_stack_availability();
    println!("perp stack: {}", perp_stack.is_complete());
    println!("  orderBook      : {}", perp_stack.order_book);
    println!("  settlement     : {}", perp_stack.settlement_engine);
    println!("  liquidation    : {}", perp_stack.liquidation_keeper);
    println!("  missing        : {:?}", perp_stack.missing_contracts());
    println!("USDC    : {}", manifest.constants.usdc);
    println!("AccountManager : {}", manifest.contracts.account_manager);
    println!("USDCVault      : {}", manifest.contracts.usdc_vault);
    println!("MarketRegistry : {}", manifest.contracts.market_registry);
    println!(
        "OrderBook      : {}",
        manifest
            .contracts
            .order_book
            .map_or_else(|| "not present".to_owned(), |address| address.to_string())
    );
    println!(
        "Settlement     : {}",
        manifest
            .contracts
            .settlement_engine
            .map_or_else(|| "not present".to_owned(), |address| address.to_string())
    );
    println!(
        "Liquidation    : {}",
        manifest
            .contracts
            .liquidation_keeper
            .map_or_else(|| "not present".to_owned(), |address| address.to_string())
    );
    let context_event_filters = context.event_filters();
    let keeper_runtime = context.keeper_runtime();
    let keeper_polling = keeper_runtime
        .polling_plan(
            KeeperPollingSnapshot::at_block(123).with_event_from_block(100),
            KeeperPollingPolicy::new(50, 1, 1),
        )
        .expect("keeper polling plan");
    let deposit_plan = context.collateral_deposit(1, 1_000_000);
    let deposit_txs = deposit_plan.transactions();
    let deposit_tx_requests = UnsignedCall::to_tx_requests(&deposit_txs);
    let deposit_tx_envelopes = UnsignedCall::to_tx_requests_with_batch_metadata(
        &deposit_txs,
        TxBatchRequestMetadata::new()
            .with_from(manifest.deployer)
            .with_start_nonce(7)
            .with_gas(120_000)
            .with_eip1559_fees(2_000_000_000, 1_000_000_000)
            .with_chain_id(manifest.chain_id),
    )
    .expect("deposit transaction envelopes");
    let chain_id_request = JsonRpcRequest::eth_chain_id(20);
    let nonce_request =
        JsonRpcRequest::eth_get_transaction_count(21, manifest.deployer, RpcBlockTag::Pending);
    let gas_estimate_request = JsonRpcRequest::eth_estimate_gas(22, &deposit_tx_envelopes[0]);
    let gas_price_request = JsonRpcRequest::eth_gas_price(23);
    let max_priority_fee_request = JsonRpcRequest::eth_max_priority_fee_per_gas(24);
    let withdraw_plan = context.collateral_withdraw(1, 500_000, manifest.deployer);
    let status_plan = context.collateral_status(manifest.deployer, 1);
    let onboarding_plan = context.account_onboarding(manifest.deployer);
    let account_status_plan = context.account_status(manifest.deployer, 1);
    let market_plan = context.market(1);
    let market_calls = market_plan.calls();
    let market_requests = UnsignedCall::to_requests(&market_calls);
    let market_queries = UnsignedCall::to_queries_at(&market_calls, RpcBlockTag::Finalized);
    let market_rpc_calls = JsonRpcRequest::eth_call_batch(&market_calls, RpcBlockTag::Finalized, 1)
        .expect("sample market eth_call batch");
    let sample_market_rpc_responses: Vec<JsonRpcResponse<String>> = vec![
        JsonRpcResponse {
            jsonrpc: "2.0".to_owned(),
            id: 3,
            result: Some("0x03".to_owned()),
            error: None,
        },
        JsonRpcResponse {
            jsonrpc: "2.0".to_owned(),
            id: 1,
            result: Some("0x01".to_owned()),
            error: None,
        },
        JsonRpcResponse {
            jsonrpc: "2.0".to_owned(),
            id: 2,
            result: Some("0x02".to_owned()),
            error: None,
        },
    ];
    let sample_market_rpc_returns = JsonRpcResponse::into_call_return_batch_for_requests(
        sample_market_rpc_responses,
        &market_rpc_calls,
    )
    .expect("sample ordered market RPC returns");
    let settlement_plan = SettlementReadPlan::from_manifest(&manifest, 1, 1);
    let liquidation_plan = LiquidationReadPlan::from_manifest(&manifest, 1, 1);
    let event_filter_set = EventFilterSet::from_manifest(&manifest);
    let event_filter_request = event_filter_set.to_request();
    let event_log_query = event_filter_set.to_query(Some(0), None);
    let event_rpc_query = event_log_query.to_rpc_query();
    let event_rpc_request = JsonRpcRequest::eth_get_logs(10, &event_rpc_query);
    let event_query_chunks = event_filter_set
        .chunked_queries(0, 250, 100)
        .expect("sample event query chunks");
    let event_rpc_query_chunks = event_filter_set
        .chunked_rpc_queries(0, 250, 100)
        .expect("sample RPC event query chunks");
    let sample_account_registered_data = topic_u128(123);
    let sample_log_metadata =
        RawLogMetadata::new(Some(123), Some(B256::repeat_byte(0xaa)), Some(0));
    let sample_account_registered_log = RawLog::from_hex_data_with_metadata(
        manifest.contracts.account_manager,
        vec![
            AccountRegisteredEvent::topic0(),
            topic_u128(1),
            topic_address(manifest.deployer),
        ],
        format!("0x{}", hex::encode(sample_account_registered_data)),
        sample_log_metadata,
    )
    .expect("sample event log data");
    let sample_account_registered =
        AccountRegisteredEvent::decode(&sample_account_registered_log).expect("sample event log");
    let sample_known_event =
        TangentEvent::decode_known(&sample_account_registered_log).expect("known event");
    let sample_log_batch = event_filter_set
        .decode_logs([
            &sample_account_registered_log,
            &RawLog::new(Address::ZERO, vec![B256::repeat_byte(0xee)], vec![]),
        ])
        .expect("sample log batch");
    let sample_cursor = sample_account_registered_log
        .cursor()
        .expect("sample log cursor");
    let sample_resume_query = event_filter_set.resume_query(sample_cursor, Some(250));
    let sample_resume_rpc_query = event_filter_set.resume_rpc_query(sample_cursor, Some(250));
    let sample_log_batch_after_cursor = event_filter_set
        .decode_logs_after_cursor([&sample_account_registered_log], sample_cursor)
        .expect("sample post-cursor batch");
    let sample_keeper_next_snapshot = KeeperPollingOutcome::at_block(123)
        .with_latest_event_cursor(sample_cursor)
        .with_completed_maintenance()
        .with_completed_liquidation_scan()
        .next_snapshot(KeeperPollingSnapshot::at_block(122).with_event_from_block(100));
    let sample_tx_hash =
        TxHash::from_hex(format!("0x{}", "11".repeat(32))).expect("sample tx hash");
    let sample_receipt = TxReceipt::from_rpc_fields(
        sample_tx_hash.to_hex(),
        Some("0x7b"),
        Some("0x1"),
        Some("0x5208"),
        Some("0x77359400"),
        vec![sample_account_registered_log.clone()],
    )
    .expect("sample RPC receipt fields");
    let sample_receipt_request =
        JsonRpcRequest::eth_get_transaction_receipt(11, sample_receipt.transaction_hash);
    let sample_receipt_logs = sample_receipt
        .decode_logs(&event_filter_set)
        .expect("sample receipt logs");
    let sample_confirmation_plan = TxConfirmationPlan::new(
        sample_receipt.transaction_hash,
        TxConfirmationPolicy::new(2).with_timeout_blocks(20),
    )
    .with_submitted_at_block(100);
    let sample_confirmation_requests = sample_confirmation_plan.requests(11, 12);
    let sample_current_block_response: JsonRpcResponse<String> =
        serde_json::from_str("{\"jsonrpc\":\"2.0\",\"id\":12,\"result\":\"0x7c\"}")
            .expect("sample block number response");
    let sample_current_block = sample_current_block_response
        .into_quantity_u64()
        .expect("sample current block");
    let sample_confirmation =
        sample_confirmation_plan.classify(Some(&sample_receipt), Some(sample_current_block));
    println!(
        "USDC approve selector   : {}",
        deposit_plan
            .approve_tx()
            .selector_hex()
            .expect("approve has selector")
    );
    println!(
        "registerAccount selector: {}",
        onboarding_plan
            .register_tx()
            .selector_hex()
            .expect("registerAccount has selector")
    );
    println!(
        "AccountRegistered topic : 0x{}",
        hex::encode(AccountRegisteredEvent::topic0())
    );
    println!("event filters           : {}", event_filter_set.len());
    println!("context event filters   : {}", context_event_filters.len());
    println!(
        "keeper capabilities     : {:?}",
        keeper_runtime.capabilities()
    );
    println!(
        "keeper can tick         : {}",
        keeper_runtime.can_tick_orderbook()
    );
    println!(
        "keeper poll queries     : {}, maintenance txs {}, liquidation scan {}",
        keeper_polling.event_queries.len(),
        keeper_polling.maintenance_transactions.len(),
        keeper_polling.should_scan_liquidations
    );
    println!(
        "event request           : {} addresses, {} topics",
        event_filter_request.addresses.len(),
        event_filter_request.topic0.len()
    );
    println!(
        "event query range       : {:?}..{:?}",
        event_log_query.from_block, event_log_query.to_block
    );
    println!(
        "event rpc query         : {} addresses, {} topic slots, {:?}..{:?}",
        event_rpc_query.addresses.len(),
        event_rpc_query.topics.len(),
        event_rpc_query.from_block,
        event_rpc_query.to_block
    );
    println!("event query chunks      : {}", event_query_chunks.len());
    println!("event rpc query chunks  : {}", event_rpc_query_chunks.len());
    println!("event rpc method        : {}", event_rpc_request.method);
    println!(
        "deposit selector        : {}",
        deposit_plan
            .deposit_tx()
            .selector_hex()
            .expect("deposit has selector")
    );
    println!(
        "market selector         : {}",
        market_plan
            .market_call()
            .selector_hex()
            .expect("market has selector")
    );
    let mut sample_balance_return = [0u8; 32];
    sample_balance_return[31] = 7;
    let sample_balance_rpc_return =
        CallReturn::from_hex(format!("0x{}", hex::encode(sample_balance_return)))
            .expect("sample RPC return");
    let sample_balance_rpc_response: JsonRpcResponse<String> = serde_json::from_str(&format!(
        "{{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":\"{}\"}}",
        sample_balance_rpc_return.data_hex()
    ))
    .expect("sample JSON-RPC response");
    let sample_balance_response_return = sample_balance_rpc_response
        .into_call_return()
        .expect("sample response return");
    let sample_preflight = TxPreflight::from_rpc_responses(
        Some(
            serde_json::from_str(&format!(
                "{{\"jsonrpc\":\"2.0\",\"id\":20,\"result\":\"{}\"}}",
                RpcBlockTag::Number(manifest.chain_id).to_rpc_param()
            ))
            .expect("sample chain id response"),
        ),
        Some(
            serde_json::from_str("{\"jsonrpc\":\"2.0\",\"id\":21,\"result\":\"0x7\"}")
                .expect("sample nonce response"),
        ),
        Some(
            serde_json::from_str("{\"jsonrpc\":\"2.0\",\"id\":22,\"result\":\"0x3d090\"}")
                .expect("sample gas estimate response"),
        ),
        Some(
            serde_json::from_str("{\"jsonrpc\":\"2.0\",\"id\":23,\"result\":\"0x77359400\"}")
                .expect("sample gas price response"),
        ),
        Some(
            serde_json::from_str("{\"jsonrpc\":\"2.0\",\"id\":24,\"result\":\"0x77359400\"}")
                .expect("sample max fee response"),
        ),
        Some(
            serde_json::from_str("{\"jsonrpc\":\"2.0\",\"id\":25,\"result\":\"0x3b9aca00\"}")
                .expect("sample max priority fee response"),
        ),
    )
    .expect("sample preflight responses");
    let sample_preflight_request =
        sample_preflight.apply_to(&deposit_plan.approve_tx(), Some(manifest.deployer));
    let sample_submission_plan = TxSubmissionPlan::from_unsigned_tx(
        &deposit_plan.approve_tx(),
        Some(manifest.deployer),
        sample_preflight,
        TxConfirmationPolicy::new(2).with_timeout_blocks(20),
    )
    .with_submitted_at_block(100);
    let sample_submit_estimate_request = sample_submission_plan.estimate_gas_request(28);
    let sample_submit_send_request = sample_submission_plan.send_transaction_request(29);
    let sample_signed_tx =
        SignedRawTransaction::from_hex("0x02abcd").expect("sample externally signed transaction");
    let sample_raw_send_request =
        sample_submission_plan.send_raw_transaction_request(30, &sample_signed_tx);
    let sample_submit_confirmation_plan =
        sample_submission_plan.confirmation_plan(sample_receipt.transaction_hash);
    let sample_account_status_returns = CallReturnBatch::from_hex([
        format!("0x{}", hex::encode(topic_address(manifest.deployer))),
        format!("0x{}", hex::encode(topic_u128(1))),
        format!("0x{}", hex::encode(topic_u128(1))),
    ])
    .expect("sample account status return batch");
    let sample_account_status = account_status_plan
        .decode_return_batch(&sample_account_status_returns)
        .expect("sample account status batch");
    let account_next_step = onboarding_plan.next_step(&sample_account_status, 1);
    let sample_collateral_status = CollateralStatus {
        usdc_balance: 1_000_000,
        vault_allowance: 0,
        free_balance: 500_000,
        locked_balance: 100_000,
        total_balance: 600_000,
    };
    println!(
        "sample balance decode   : {}",
        USDCVaultCalls::decode_free_balance_of_return(sample_balance_rpc_return.as_ref())
            .expect("sample return")
    );
    println!(
        "sample return bytes     : {}",
        sample_balance_rpc_return.data_len()
    );
    println!(
        "sample response bytes   : {}",
        sample_balance_response_return.data_len()
    );
    println!(
        "sample return batch     : {} returns, registered {}",
        sample_account_status_returns.len(),
        account_status_plan.is_registered_binding(&sample_account_status)
    );
    println!(
        "account binding status  : {:?}",
        account_status_plan.binding_status(&sample_account_status)
    );
    println!(
        "account next step       : {}",
        match &account_next_step {
            AccountOnboardingNextStep::Register(tx) => format!("register via {}", tx.to),
            AccountOnboardingNextStep::UseAccount(account_id) => {
                format!("use account {account_id}")
            }
            AccountOnboardingNextStep::Blocked(status) => format!("blocked: {status:?}"),
        }
    );
    println!(
        "sample register tx to   : {}",
        onboarding_plan.register_tx().to
    );
    println!(
        "sample registered owner : {}",
        sample_account_registered.owner
    );
    println!("sample known event      : {}", sample_known_event.is_some());
    println!(
        "sample decoded logs     : {} known, {} unknown",
        sample_log_batch.known_logs(),
        sample_log_batch.unknown_logs
    );
    println!(
        "sample log batch total  : {}",
        sample_log_batch.total_logs()
    );
    println!(
        "sample event kinds      : {}",
        sample_log_batch.nonzero_kind_counts().len()
    );
    println!(
        "sample log data bytes   : {}",
        sample_account_registered_log.data.len()
    );
    println!(
        "sample log source       : block {:?}, index {:?}",
        sample_account_registered_log
            .metadata
            .and_then(|metadata| metadata.block_number),
        sample_account_registered_log
            .metadata
            .and_then(|metadata| metadata.log_index)
    );
    println!(
        "sample log cursor       : resume block {}, next block {}",
        sample_cursor.resume_from_block(),
        sample_cursor.next_block()
    );
    println!(
        "sample resume query     : {:?}..{:?}",
        sample_resume_query.from_block, sample_resume_query.to_block
    );
    println!(
        "sample resume rpc query : {:?}..{:?}",
        sample_resume_rpc_query.from_block, sample_resume_rpc_query.to_block
    );
    println!(
        "sample after cursor     : {} known",
        sample_log_batch_after_cursor.known_logs()
    );
    println!(
        "keeper next checkpoint  : cursor {:?}, tick {:?}, scan {:?}",
        sample_keeper_next_snapshot.event_cursor,
        sample_keeper_next_snapshot.last_tick_block,
        sample_keeper_next_snapshot.last_liquidation_scan_block
    );
    println!("sample tx hash          : {}", sample_tx_hash.to_hex());
    println!(
        "sample receipt status   : mined {}, success {}",
        sample_receipt.is_mined(),
        sample_receipt.is_success()
    );
    println!(
        "sample receipt method   : {}",
        sample_receipt_request.method
    );
    println!(
        "confirmation rpc methods: {}, {}",
        sample_confirmation_requests[0].method, sample_confirmation_requests[1].method
    );
    println!(
        "sample receipt logs     : {} known, last cursor {:?}",
        sample_receipt_logs.known_logs(),
        sample_receipt.last_cursor()
    );
    println!(
        "sample receipt fee      : {:?}",
        sample_receipt.execution_fee_paid()
    );
    println!("sample confirmation     : {:?}", sample_confirmation);
    println!(
        "account onboarding txs  : {} tx",
        onboarding_plan.transactions().len()
    );
    println!(
        "sample accountId call to: {}",
        onboarding_plan.account_id_of_call().to
    );
    println!(
        "sample ownerOf call to  : {}",
        account_status_plan.owner_of_call().to
    );
    println!("sample approve tx to    : {}", deposit_plan.approve_tx().to);
    println!(
        "sample allowance call to: {}",
        status_plan.vault_allowance_call().to
    );
    println!(
        "sample markPrice call to: {}",
        market_plan.mark_price_call().to
    );
    println!(
        "account status reads    : {} calls",
        account_status_plan.calls().len()
    );
    println!(
        "collateral status reads : {} calls",
        status_plan.calls().len()
    );
    println!("market status reads     : {} calls", market_calls.len());
    println!(
        "market request payloads : {} requests",
        market_requests.len()
    );
    println!(
        "market finalized queries: {} queries at {}",
        market_queries.len(),
        RpcBlockTag::Finalized.to_rpc_param()
    );
    println!(
        "market JSON-RPC calls   : first id {}, method {}",
        market_rpc_calls[0].id, market_rpc_calls[0].method
    );
    println!(
        "market RPC returns      : {} ordered",
        sample_market_rpc_returns.len()
    );
    println!("sample deposit tx to    : {}", deposit_plan.deposit_tx().to);
    let deposit_next_step = deposit_plan.next_step(&sample_collateral_status);
    println!(
        "deposit readiness       : {:?}",
        deposit_plan.readiness(&sample_collateral_status)
    );
    println!(
        "deposit next tx to      : {}",
        match &deposit_next_step {
            CollateralDepositNextStep::Approve(tx) | CollateralDepositNextStep::Deposit(tx) =>
                tx.to.to_string(),
            CollateralDepositNextStep::Blocked(_) => "blocked".to_owned(),
        }
    );
    println!("collateral deposit txs  : {} txs", deposit_txs.len());
    println!(
        "deposit tx requests     : {} requests",
        deposit_tx_requests.len()
    );
    println!(
        "deposit tx envelopes    : first nonce {:?}, second nonce {:?}",
        deposit_tx_envelopes[0].nonce, deposit_tx_envelopes[1].nonce
    );
    println!(
        "preflight rpc methods   : {}, {}, {}, {}, {}",
        chain_id_request.method,
        nonce_request.method,
        gas_estimate_request.method,
        gas_price_request.method,
        max_priority_fee_request.method
    );
    println!(
        "sample preflight tx     : chain {:?}, nonce {:?}, gas {:?}, maxFee {:?}",
        sample_preflight_request.chain_id,
        sample_preflight_request.nonce,
        sample_preflight_request.gas,
        sample_preflight_request.max_fee_per_gas
    );
    println!(
        "submission rpc methods  : {}, {}, {}",
        sample_submit_estimate_request.method,
        sample_submit_send_request.method,
        sample_submit_confirmation_plan.receipt_request(31).method
    );
    println!(
        "sample raw send         : {} bytes via {}",
        sample_signed_tx.len(),
        sample_raw_send_request.method
    );
    println!(
        "sample withdraw tx to   : {}",
        withdraw_plan.withdraw_tx().to
    );
    let withdraw_next_step = withdraw_plan.next_step(&sample_collateral_status, None);
    println!(
        "withdraw readiness      : {:?}",
        withdraw_plan.readiness(&sample_collateral_status, None)
    );
    println!(
        "withdraw next tx to     : {}",
        match &withdraw_next_step {
            CollateralWithdrawNextStep::Withdraw(tx) => tx.to.to_string(),
            CollateralWithdrawNextStep::Blocked(_) => "blocked".to_owned(),
        }
    );
    println!(
        "collateral withdraw txs : {} tx",
        withdraw_plan.transactions().len()
    );

    match settlement_plan {
        Some(plan) => {
            let sample_settlement_status = SettlementStatus {
                position: PositionStatus {
                    size: 1,
                    entry_price: 65_000,
                    locked_margin: 500_000,
                },
                margin: MarginStatus {
                    equity: 1_000_000,
                    maintenance_margin: 500_000,
                },
            };
            println!("sample position call to : {}", plan.position_of_call().to);
            println!("sample margin call to   : {}", plan.margin_state_call().to);
            println!(
                "settlement withdraw ok  : {:?}",
                plan.withdrawal_readiness(&sample_settlement_status, 500_000)
            );
            println!(
                "settlement validate call: {}",
                match plan.withdrawal_next_step(&sample_settlement_status, 500_000) {
                    SettlementWithdrawalNextStep::Validate(call) => call.to.to_string(),
                    SettlementWithdrawalNextStep::Blocked(_) => "blocked".to_owned(),
                }
            );
            println!("settlement status reads : {} calls", plan.calls().len());
        }
        None => {
            println!("sample settlement reads : unavailable in this v0.1 manifest");
        }
    }

    match liquidation_plan {
        Some(plan) => {
            let sample_liquidation_status = LiquidationStatus {
                is_liquidatable: true,
                equity: -1,
                maintenance_margin: 1,
            };
            let liquidation_next_step = plan.next_step(&sample_liquidation_status);
            println!(
                "sample liq state call to: {}",
                plan.liquidation_state_call().to
            );
            println!("sample liquidate tx to  : {}", plan.liquidate_tx().to);
            println!(
                "liquidation readiness   : {:?}",
                plan.readiness(&sample_liquidation_status)
            );
            println!(
                "liquidation next tx to  : {}",
                match &liquidation_next_step {
                    LiquidationNextStep::Liquidate(tx) => tx.to.to_string(),
                    LiquidationNextStep::Blocked(_) => "blocked".to_owned(),
                }
            );
            println!("liquidation txs         : {} tx", plan.transactions().len());
            println!("liquidation reads       : {} calls", plan.calls().len());
        }
        None => {
            println!("sample liquidation reads: unavailable in this v0.1 manifest");
        }
    }

    match manifest.order_book_domain() {
        Some(domain) => {
            println!("OrderBook       : {}", domain.verifying_contract);
            println!("Domain separator: 0x{}", hex::encode(domain.separator()));
        }
        None => {
            println!("OrderBook       : not present in this v0.1 manifest");
            println!("Domain separator: unavailable until full-stack deployment is published");
        }
    }

    match manifest.require_perp_stack() {
        Ok(stack) => {
            println!("full stack orderBook: {}", stack.order_book);
        }
        Err(err) => {
            println!("full stack gate : {err}");
        }
    }
}

fn topic_u128(value: u128) -> B256 {
    let mut out = [0u8; 32];
    out[16..].copy_from_slice(&value.to_be_bytes());
    B256::from(out)
}

fn topic_address(value: Address) -> B256 {
    let mut out = [0u8; 32];
    out[12..].copy_from_slice(value.as_slice());
    B256::from(out)
}
