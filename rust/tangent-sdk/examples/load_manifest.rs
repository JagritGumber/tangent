//! Minimal example: load the checked-in Arc Testnet deployment manifest.
//!
//! Run with:
//!   cargo run --example load_manifest -p tangent-sdk

use tangent_sdk::{
    AccountOnboardingPlan, AccountStatusPlan, CollateralDepositPlan, CollateralStatusPlan,
    CollateralWithdrawPlan, DeploymentManifest, LiquidationReadPlan, MarketReadPlan,
    SettlementReadPlan, USDCVaultCalls,
};

fn main() {
    let manifest =
        DeploymentManifest::from_json(include_str!("../../../docs/deployments/arc-testnet.json"))
            .expect("valid deployment manifest");

    println!("=== tangent-sdk example: deployment manifest ===");
    println!("project : {}", manifest.project);
    println!("network : {}", manifest.network);
    println!("chainId : {}", manifest.chain_id);
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
    let deposit_plan = CollateralDepositPlan::from_manifest(&manifest, 1, 1_000_000);
    let withdraw_plan =
        CollateralWithdrawPlan::from_manifest(&manifest, 1, 500_000, manifest.deployer);
    let status_plan = CollateralStatusPlan::from_manifest(&manifest, manifest.deployer, 1);
    let onboarding_plan = AccountOnboardingPlan::from_manifest(&manifest, manifest.deployer);
    let account_status_plan = AccountStatusPlan::from_manifest(&manifest, manifest.deployer, 1);
    let market_plan = MarketReadPlan::from_manifest(&manifest, 1);
    let settlement_plan = SettlementReadPlan::from_manifest(&manifest, 1, 1);
    let liquidation_plan = LiquidationReadPlan::from_manifest(&manifest, 1, 1);
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
    println!(
        "sample balance decode   : {}",
        USDCVaultCalls::decode_free_balance_of_return(&sample_balance_return)
            .expect("sample return")
    );
    println!(
        "sample register tx to   : {}",
        onboarding_plan.register_tx().to
    );
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
    println!(
        "market status reads     : {} calls",
        market_plan.calls().len()
    );
    println!("sample deposit tx to    : {}", deposit_plan.deposit_tx().to);
    println!(
        "collateral deposit txs  : {} txs",
        deposit_plan.transactions().len()
    );
    println!(
        "sample withdraw tx to   : {}",
        withdraw_plan.withdraw_tx().to
    );
    println!(
        "collateral withdraw txs : {} tx",
        withdraw_plan.transactions().len()
    );

    match settlement_plan {
        Some(plan) => {
            println!("sample position call to : {}", plan.position_of_call().to);
            println!("sample margin call to   : {}", plan.margin_state_call().to);
            println!("settlement status reads : {} calls", plan.calls().len());
        }
        None => {
            println!("sample settlement reads : unavailable in this v0.1 manifest");
        }
    }

    match liquidation_plan {
        Some(plan) => {
            println!(
                "sample liq state call to: {}",
                plan.liquidation_state_call().to
            );
            println!("sample liquidate tx to  : {}", plan.liquidate_tx().to);
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
}
