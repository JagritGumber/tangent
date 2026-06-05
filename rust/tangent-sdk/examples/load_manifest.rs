//! Minimal example: load the checked-in Arc Testnet deployment manifest.
//!
//! Run with:
//!   cargo run --example load_manifest -p tangent-sdk

use tangent_sdk::{
    AccountManagerCalls, DeploymentManifest, ERC20Calls, MarketRegistryCalls, USDCVaultCalls,
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
        "USDC approve selector   : 0x{}",
        hex::encode(ERC20Calls::approve_selector())
    );
    println!(
        "registerAccount selector: 0x{}",
        hex::encode(AccountManagerCalls::register_account_selector())
    );
    println!(
        "deposit selector        : 0x{}",
        hex::encode(USDCVaultCalls::deposit_selector())
    );
    println!(
        "market selector         : 0x{}",
        hex::encode(MarketRegistryCalls::market_selector())
    );
    let mut sample_balance_return = [0u8; 32];
    sample_balance_return[31] = 7;
    println!(
        "sample balance decode   : {}",
        USDCVaultCalls::decode_free_balance_of_return(&sample_balance_return)
            .expect("sample return")
    );

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
