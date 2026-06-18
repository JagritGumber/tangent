//! Minimal keeper polling preview/report example.
//!
//! Run with:
//!   cargo run --example keeper_polling -p tangent-sdk

use alloy_primitives::{Address, B256};
use tangent_sdk::{
    DeploymentManifest, KeeperPollingOutcome, KeeperPollingPolicy, KeeperPollingSnapshot,
    LiquidationStatus, OrderSubmittedEvent, RpcEndpointConfig, TangentClientPlan, TangentEvent,
    TangentEventProjection, TangentKeeperLiquidationCandidate, TangentKeeperLiquidationScanResult,
    TangentKeeperPollingExecution, TangentKeeperPollingState, TangentKeeperPollingStateExecution,
};

fn main() {
    let mut manifest =
        DeploymentManifest::from_json(include_str!("../../../docs/deployments/arc-testnet.json"))
            .expect("valid deployment manifest");
    manifest.contracts.settlement_engine = Some(Address::repeat_byte(0x51));
    manifest.contracts.liquidation_keeper = Some(Address::repeat_byte(0x52));

    let policies = tangent_sdk::TangentClientPolicies {
        keeper_polling: KeeperPollingPolicy::new(100, 1, 1),
        ..tangent_sdk::TangentClientPolicies::default()
    };
    let client_plan = TangentClientPlan::with_policies(
        manifest,
        RpcEndpointConfig::new("https://rpc.arc.example").expect("sample endpoint"),
        policies,
    )
    .expect("client plan");

    let order_hash = B256::repeat_byte(0x7a);
    let mut projection = TangentEventProjection::default();
    projection
        .apply_event(&TangentEvent::OrderSubmitted(OrderSubmittedEvent {
            order_hash,
            account_id: 7,
            market_id: 1,
            is_buy: true,
            limit_price: 65_000_000_000,
            size: 1_000_000_000,
        }))
        .expect("seed projection");

    let snapshot = KeeperPollingSnapshot::at_block(130)
        .with_event_from_block(120)
        .with_last_liquidation_scan_block(120);
    let state = TangentKeeperPollingState::new(snapshot, projection.clone());
    let explicit_candidates = [TangentKeeperLiquidationCandidate::new(9, 1)];
    let preview = client_plan
        .keeper_polling_preview(&state, &explicit_candidates)
        .expect("keeper preview");
    let preview_summary = preview.summary();

    println!("=== tangent-sdk example: keeper polling ===");
    println!(
        "event query chunks       : {}",
        preview_summary.event_query_count
    );
    println!(
        "derived scan candidates : {}",
        preview_summary.derived_liquidation_candidates
    );
    println!(
        "scan candidates         : {}",
        preview_summary.scan_candidates
    );

    let plan = client_plan
        .keeper_polling_plan(snapshot)
        .expect("keeper polling plan");
    let execution = TangentKeeperPollingExecution {
        plan,
        event_records: Default::default(),
        events: Default::default(),
        projection,
        derived_liquidation_candidates: preview.derived_liquidation_candidates,
        maintenance_submission: None,
        liquidation_results: preview
            .scan_candidates
            .iter()
            .copied()
            .map(|candidate| TangentKeeperLiquidationScanResult {
                candidate,
                status: LiquidationStatus {
                    is_liquidatable: false,
                    equity: 25,
                    maintenance_margin: 100,
                },
                submission: None,
            })
            .collect(),
        outcome: KeeperPollingOutcome::at_block(snapshot.current_block)
            .with_completed_liquidation_scan(),
    };
    let state_execution = TangentKeeperPollingStateExecution {
        next_state: execution.next_state(snapshot),
        execution,
    };
    let report = state_execution.report();
    let resume_report = state_execution.next_state.resume_report_at(135);
    let report_json = serde_json::to_string_pretty(&report).expect("report serializes");
    let resume_report_json =
        serde_json::to_string_pretty(&resume_report).expect("resume report serializes");

    println!("ready liquidations      : {}", report.ready_liquidations);
    println!(
        "next scan checkpoint    : {:?}",
        report.checkpoint.snapshot.last_liquidation_scan_block
    );
    println!(
        "resume event cursor     : {:?}",
        resume_report.effective_event_cursor
    );
    println!("report json             : {report_json}");
    println!("resume report json      : {resume_report_json}");
}
