//! Raw calldata helpers for Tangent's currently deployed primitive contracts.
//!
//! These helpers produce transaction or `eth_call` input bytes plus minimal
//! single-word return decoders. They do not perform RPC or assume a contract is
//! deployed.

use alloy_primitives::{keccak256, Address};

use crate::AbiDecodeError;

fn selector(signature: &str) -> [u8; 4] {
    let hash = keccak256(signature.as_bytes());
    [hash[0], hash[1], hash[2], hash[3]]
}

fn no_arg_call(signature: &str) -> Vec<u8> {
    selector(signature).to_vec()
}

fn u128_call(signature: &str, value: u128) -> Vec<u8> {
    let mut out = Vec::with_capacity(36);
    out.extend_from_slice(&selector(signature));
    crate::eip712::encode_u128(&mut out, value);
    out
}

fn two_u128_call(signature: &str, first: u128, second: u128) -> Vec<u8> {
    let mut out = Vec::with_capacity(68);
    out.extend_from_slice(&selector(signature));
    crate::eip712::encode_u128(&mut out, first);
    crate::eip712::encode_u128(&mut out, second);
    out
}

fn address_call(signature: &str, value: Address) -> Vec<u8> {
    let mut out = Vec::with_capacity(36);
    out.extend_from_slice(&selector(signature));
    crate::eip712::encode_address(&mut out, value);
    out
}

/// ABI helpers for `LiquidationKeeper` calls.
pub struct LiquidationKeeperCalls;

impl LiquidationKeeperCalls {
    pub const LIQUIDATE_SIGNATURE: &'static str = "liquidate(uint256,uint256)";
    pub const IS_LIQUIDATABLE_SIGNATURE: &'static str = "isLiquidatable(uint256,uint256)";
    pub const LIQUIDATION_STATE_SIGNATURE: &'static str = "liquidationState(uint256,uint256)";

    #[must_use]
    pub fn liquidate_selector() -> [u8; 4] {
        selector(Self::LIQUIDATE_SIGNATURE)
    }

    #[must_use]
    pub fn liquidate_calldata(account_id: u128, market_id: u128) -> Vec<u8> {
        two_u128_call(Self::LIQUIDATE_SIGNATURE, account_id, market_id)
    }

    #[must_use]
    pub fn is_liquidatable_selector() -> [u8; 4] {
        selector(Self::IS_LIQUIDATABLE_SIGNATURE)
    }

    #[must_use]
    pub fn is_liquidatable_calldata(account_id: u128, market_id: u128) -> Vec<u8> {
        two_u128_call(Self::IS_LIQUIDATABLE_SIGNATURE, account_id, market_id)
    }

    #[must_use]
    pub fn liquidation_state_selector() -> [u8; 4] {
        selector(Self::LIQUIDATION_STATE_SIGNATURE)
    }

    #[must_use]
    pub fn liquidation_state_calldata(account_id: u128, market_id: u128) -> Vec<u8> {
        two_u128_call(Self::LIQUIDATION_STATE_SIGNATURE, account_id, market_id)
    }

    pub fn decode_is_liquidatable_return(data: &[u8]) -> Result<bool, AbiDecodeError> {
        crate::abi::decode_bool(data)
    }

    pub fn decode_liquidation_state_return(
        data: &[u8],
    ) -> Result<(bool, i128, u128), AbiDecodeError> {
        if data.len() != 96 {
            return Err(AbiDecodeError::InvalidLength {
                expected: 96,
                actual: data.len(),
            });
        }

        Ok((
            crate::abi::decode_bool(&data[0..32])?,
            crate::abi::decode_i128(&data[32..64])?,
            crate::abi::decode_u128(&data[64..96])?,
        ))
    }
}

/// ABI helpers for standard ERC-20 calls used by the USDC collateral flow.
pub struct ERC20Calls;

impl ERC20Calls {
    pub const APPROVE_SIGNATURE: &'static str = "approve(address,uint256)";
    pub const ALLOWANCE_SIGNATURE: &'static str = "allowance(address,address)";
    pub const BALANCE_OF_SIGNATURE: &'static str = "balanceOf(address)";
    pub const TRANSFER_SIGNATURE: &'static str = "transfer(address,uint256)";

    #[must_use]
    pub fn approve_selector() -> [u8; 4] {
        selector(Self::APPROVE_SIGNATURE)
    }

    #[must_use]
    pub fn approve_calldata(spender: Address, amount: u128) -> Vec<u8> {
        let mut out = Vec::with_capacity(68);
        out.extend_from_slice(&Self::approve_selector());
        crate::eip712::encode_address(&mut out, spender);
        crate::eip712::encode_u128(&mut out, amount);
        out
    }

    #[must_use]
    pub fn allowance_selector() -> [u8; 4] {
        selector(Self::ALLOWANCE_SIGNATURE)
    }

    #[must_use]
    pub fn allowance_calldata(owner: Address, spender: Address) -> Vec<u8> {
        let mut out = Vec::with_capacity(68);
        out.extend_from_slice(&Self::allowance_selector());
        crate::eip712::encode_address(&mut out, owner);
        crate::eip712::encode_address(&mut out, spender);
        out
    }

    #[must_use]
    pub fn balance_of_selector() -> [u8; 4] {
        selector(Self::BALANCE_OF_SIGNATURE)
    }

    #[must_use]
    pub fn balance_of_calldata(account: Address) -> Vec<u8> {
        address_call(Self::BALANCE_OF_SIGNATURE, account)
    }

    #[must_use]
    pub fn transfer_selector() -> [u8; 4] {
        selector(Self::TRANSFER_SIGNATURE)
    }

    #[must_use]
    pub fn transfer_calldata(to: Address, amount: u128) -> Vec<u8> {
        let mut out = Vec::with_capacity(68);
        out.extend_from_slice(&Self::transfer_selector());
        crate::eip712::encode_address(&mut out, to);
        crate::eip712::encode_u128(&mut out, amount);
        out
    }

    pub fn decode_bool_return(data: &[u8]) -> Result<bool, AbiDecodeError> {
        crate::abi::decode_bool(data)
    }

    pub fn decode_allowance_return(data: &[u8]) -> Result<u128, AbiDecodeError> {
        crate::abi::decode_u128(data)
    }

    pub fn decode_balance_of_return(data: &[u8]) -> Result<u128, AbiDecodeError> {
        crate::abi::decode_u128(data)
    }
}

/// ABI helpers for `IAccountManager`.
pub struct AccountManagerCalls;

impl AccountManagerCalls {
    pub const REGISTER_ACCOUNT_SIGNATURE: &'static str = "registerAccount()";
    pub const OWNER_OF_SIGNATURE: &'static str = "ownerOf(uint256)";
    pub const ACCOUNT_ID_OF_SIGNATURE: &'static str = "accountIdOf(address)";
    pub const TOTAL_ACCOUNTS_SIGNATURE: &'static str = "totalAccounts()";

    #[must_use]
    pub fn register_account_selector() -> [u8; 4] {
        selector(Self::REGISTER_ACCOUNT_SIGNATURE)
    }

    #[must_use]
    pub fn register_account_calldata() -> Vec<u8> {
        no_arg_call(Self::REGISTER_ACCOUNT_SIGNATURE)
    }

    #[must_use]
    pub fn owner_of_selector() -> [u8; 4] {
        selector(Self::OWNER_OF_SIGNATURE)
    }

    #[must_use]
    pub fn owner_of_calldata(account_id: u128) -> Vec<u8> {
        u128_call(Self::OWNER_OF_SIGNATURE, account_id)
    }

    #[must_use]
    pub fn account_id_of_selector() -> [u8; 4] {
        selector(Self::ACCOUNT_ID_OF_SIGNATURE)
    }

    #[must_use]
    pub fn account_id_of_calldata(owner: Address) -> Vec<u8> {
        address_call(Self::ACCOUNT_ID_OF_SIGNATURE, owner)
    }

    #[must_use]
    pub fn total_accounts_selector() -> [u8; 4] {
        selector(Self::TOTAL_ACCOUNTS_SIGNATURE)
    }

    #[must_use]
    pub fn total_accounts_calldata() -> Vec<u8> {
        no_arg_call(Self::TOTAL_ACCOUNTS_SIGNATURE)
    }

    pub fn decode_register_account_return(data: &[u8]) -> Result<u128, AbiDecodeError> {
        crate::abi::decode_u128(data)
    }

    pub fn decode_owner_of_return(data: &[u8]) -> Result<Address, AbiDecodeError> {
        crate::abi::decode_address(data)
    }

    pub fn decode_account_id_of_return(data: &[u8]) -> Result<u128, AbiDecodeError> {
        crate::abi::decode_u128(data)
    }

    pub fn decode_total_accounts_return(data: &[u8]) -> Result<u128, AbiDecodeError> {
        crate::abi::decode_u128(data)
    }
}

/// ABI helpers for `IUSDCVault` user-facing calls.
pub struct USDCVaultCalls;

impl USDCVaultCalls {
    pub const DEPOSIT_SIGNATURE: &'static str = "deposit(uint256,uint256)";
    pub const WITHDRAW_SIGNATURE: &'static str = "withdraw(uint256,uint256,address)";
    pub const FREE_BALANCE_OF_SIGNATURE: &'static str = "freeBalanceOf(uint256)";
    pub const LOCKED_BALANCE_OF_SIGNATURE: &'static str = "lockedBalanceOf(uint256)";
    pub const TOTAL_BALANCE_OF_SIGNATURE: &'static str = "totalBalanceOf(uint256)";

    #[must_use]
    pub fn deposit_selector() -> [u8; 4] {
        selector(Self::DEPOSIT_SIGNATURE)
    }

    #[must_use]
    pub fn deposit_calldata(account_id: u128, amount: u128) -> Vec<u8> {
        let mut out = Vec::with_capacity(68);
        out.extend_from_slice(&Self::deposit_selector());
        crate::eip712::encode_u128(&mut out, account_id);
        crate::eip712::encode_u128(&mut out, amount);
        out
    }

    #[must_use]
    pub fn withdraw_selector() -> [u8; 4] {
        selector(Self::WITHDRAW_SIGNATURE)
    }

    #[must_use]
    pub fn withdraw_calldata(account_id: u128, amount: u128, to: Address) -> Vec<u8> {
        let mut out = Vec::with_capacity(100);
        out.extend_from_slice(&Self::withdraw_selector());
        crate::eip712::encode_u128(&mut out, account_id);
        crate::eip712::encode_u128(&mut out, amount);
        crate::eip712::encode_address(&mut out, to);
        out
    }

    #[must_use]
    pub fn free_balance_of_selector() -> [u8; 4] {
        selector(Self::FREE_BALANCE_OF_SIGNATURE)
    }

    #[must_use]
    pub fn free_balance_of_calldata(account_id: u128) -> Vec<u8> {
        u128_call(Self::FREE_BALANCE_OF_SIGNATURE, account_id)
    }

    #[must_use]
    pub fn locked_balance_of_selector() -> [u8; 4] {
        selector(Self::LOCKED_BALANCE_OF_SIGNATURE)
    }

    #[must_use]
    pub fn locked_balance_of_calldata(account_id: u128) -> Vec<u8> {
        u128_call(Self::LOCKED_BALANCE_OF_SIGNATURE, account_id)
    }

    #[must_use]
    pub fn total_balance_of_selector() -> [u8; 4] {
        selector(Self::TOTAL_BALANCE_OF_SIGNATURE)
    }

    #[must_use]
    pub fn total_balance_of_calldata(account_id: u128) -> Vec<u8> {
        u128_call(Self::TOTAL_BALANCE_OF_SIGNATURE, account_id)
    }

    pub fn decode_free_balance_of_return(data: &[u8]) -> Result<u128, AbiDecodeError> {
        crate::abi::decode_u128(data)
    }

    pub fn decode_locked_balance_of_return(data: &[u8]) -> Result<u128, AbiDecodeError> {
        crate::abi::decode_u128(data)
    }

    pub fn decode_total_balance_of_return(data: &[u8]) -> Result<u128, AbiDecodeError> {
        crate::abi::decode_u128(data)
    }
}

/// ABI helpers for `IMarketRegistry` read calls.
pub struct MarketRegistryCalls;

impl MarketRegistryCalls {
    pub const MARKET_SIGNATURE: &'static str = "market(uint256)";
    pub const MARK_PRICE_SIGNATURE: &'static str = "markPrice(uint256)";
    pub const TOTAL_MARKETS_SIGNATURE: &'static str = "totalMarkets()";

    #[must_use]
    pub fn market_selector() -> [u8; 4] {
        selector(Self::MARKET_SIGNATURE)
    }

    #[must_use]
    pub fn market_calldata(market_id: u128) -> Vec<u8> {
        u128_call(Self::MARKET_SIGNATURE, market_id)
    }

    #[must_use]
    pub fn mark_price_selector() -> [u8; 4] {
        selector(Self::MARK_PRICE_SIGNATURE)
    }

    #[must_use]
    pub fn mark_price_calldata(market_id: u128) -> Vec<u8> {
        u128_call(Self::MARK_PRICE_SIGNATURE, market_id)
    }

    #[must_use]
    pub fn total_markets_selector() -> [u8; 4] {
        selector(Self::TOTAL_MARKETS_SIGNATURE)
    }

    #[must_use]
    pub fn total_markets_calldata() -> Vec<u8> {
        no_arg_call(Self::TOTAL_MARKETS_SIGNATURE)
    }

    pub fn decode_mark_price_return(data: &[u8]) -> Result<u128, AbiDecodeError> {
        crate::abi::decode_u128(data)
    }

    pub fn decode_total_markets_return(data: &[u8]) -> Result<u128, AbiDecodeError> {
        crate::abi::decode_u128(data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn addr() -> Address {
        Address::repeat_byte(0x11)
    }

    #[test]
    fn account_manager_selectors_match_foundry_fixtures() {
        assert_eq!(
            hex::encode(AccountManagerCalls::register_account_selector()),
            "d9f226e9"
        );
        assert_eq!(
            hex::encode(AccountManagerCalls::owner_of_selector()),
            "6352211e"
        );
        assert_eq!(
            hex::encode(AccountManagerCalls::account_id_of_selector()),
            "ad93b0ab"
        );
        assert_eq!(
            hex::encode(AccountManagerCalls::total_accounts_selector()),
            "58451f97"
        );
    }

    #[test]
    fn erc20_selectors_match_foundry_fixtures() {
        assert_eq!(hex::encode(ERC20Calls::approve_selector()), "095ea7b3");
        assert_eq!(hex::encode(ERC20Calls::allowance_selector()), "dd62ed3e");
        assert_eq!(hex::encode(ERC20Calls::balance_of_selector()), "70a08231");
        assert_eq!(hex::encode(ERC20Calls::transfer_selector()), "a9059cbb");
    }

    #[test]
    fn vault_selectors_match_foundry_fixtures() {
        assert_eq!(hex::encode(USDCVaultCalls::deposit_selector()), "e2bbb158");
        assert_eq!(hex::encode(USDCVaultCalls::withdraw_selector()), "0ad58d2f");
        assert_eq!(
            hex::encode(USDCVaultCalls::free_balance_of_selector()),
            "ddcc2289"
        );
        assert_eq!(
            hex::encode(USDCVaultCalls::locked_balance_of_selector()),
            "58bc8e22"
        );
        assert_eq!(
            hex::encode(USDCVaultCalls::total_balance_of_selector()),
            "6663b4a4"
        );
    }

    #[test]
    fn market_registry_selectors_match_foundry_fixtures() {
        assert_eq!(
            hex::encode(MarketRegistryCalls::market_selector()),
            "28861d22"
        );
        assert_eq!(
            hex::encode(MarketRegistryCalls::mark_price_selector()),
            "ddc04609"
        );
        assert_eq!(
            hex::encode(MarketRegistryCalls::total_markets_selector()),
            "8162486b"
        );
    }

    #[test]
    fn no_arg_calls_are_selector_only() {
        assert_eq!(
            AccountManagerCalls::register_account_calldata(),
            AccountManagerCalls::register_account_selector()
        );
        assert_eq!(
            AccountManagerCalls::total_accounts_calldata(),
            AccountManagerCalls::total_accounts_selector()
        );
        assert_eq!(
            MarketRegistryCalls::total_markets_calldata(),
            MarketRegistryCalls::total_markets_selector()
        );
    }

    #[test]
    fn uint_calls_encode_selector_plus_word() {
        let owner_of = AccountManagerCalls::owner_of_calldata(7);
        assert_eq!(owner_of.len(), 36);
        assert_eq!(&owner_of[..4], &AccountManagerCalls::owner_of_selector());
        assert_eq!(hex::encode(&owner_of[4..]), format!("{:064x}", 7));

        let market = MarketRegistryCalls::market_calldata(1);
        assert_eq!(market.len(), 36);
        assert_eq!(&market[..4], &MarketRegistryCalls::market_selector());
        assert_eq!(hex::encode(&market[4..]), format!("{:064x}", 1));
    }

    #[test]
    fn liquidation_calls_encode_two_uint_words() {
        let liquidate = LiquidationKeeperCalls::liquidate_calldata(7, 1);
        assert_eq!(liquidate.len(), 68);
        assert_eq!(
            &liquidate[..4],
            &LiquidationKeeperCalls::liquidate_selector()
        );
        assert_eq!(hex::encode(&liquidate[4..36]), format!("{:064x}", 7));
        assert_eq!(hex::encode(&liquidate[36..68]), format!("{:064x}", 1));

        let state = LiquidationKeeperCalls::liquidation_state_calldata(7, 1);
        assert_eq!(state.len(), 68);
        assert_eq!(
            &state[..4],
            &LiquidationKeeperCalls::liquidation_state_selector()
        );
    }

    #[test]
    fn address_and_multi_arg_calls_encode_expected_shape() {
        let account_id = AccountManagerCalls::account_id_of_calldata(addr());
        assert_eq!(account_id.len(), 36);
        assert_eq!(
            &account_id[..4],
            &AccountManagerCalls::account_id_of_selector()
        );
        assert_eq!(&account_id[16..36], addr().as_slice());

        let deposit = USDCVaultCalls::deposit_calldata(7, 1_000_000);
        assert_eq!(deposit.len(), 68);
        assert_eq!(&deposit[..4], &USDCVaultCalls::deposit_selector());

        let withdraw = USDCVaultCalls::withdraw_calldata(7, 1_000_000, addr());
        assert_eq!(withdraw.len(), 100);
        assert_eq!(&withdraw[..4], &USDCVaultCalls::withdraw_selector());
        assert_eq!(&withdraw[80..100], addr().as_slice());

        let approve = ERC20Calls::approve_calldata(addr(), 1_000_000);
        assert_eq!(approve.len(), 68);
        assert_eq!(&approve[..4], &ERC20Calls::approve_selector());
        assert_eq!(&approve[16..36], addr().as_slice());

        let allowance = ERC20Calls::allowance_calldata(addr(), addr());
        assert_eq!(allowance.len(), 68);
        assert_eq!(&allowance[..4], &ERC20Calls::allowance_selector());
        assert_eq!(&allowance[16..36], addr().as_slice());
        assert_eq!(&allowance[48..68], addr().as_slice());
    }

    #[test]
    fn decodes_single_word_contract_returns() {
        let mut seven = [0u8; 32];
        seven[31] = 7;

        let mut address = [0u8; 32];
        address[12..].fill(0x11);

        let mut yes = [0u8; 32];
        yes[31] = 1;

        assert_eq!(
            AccountManagerCalls::decode_register_account_return(&seven).expect("account id"),
            7
        );
        assert_eq!(
            AccountManagerCalls::decode_owner_of_return(&address).expect("owner"),
            addr()
        );
        assert_eq!(
            USDCVaultCalls::decode_free_balance_of_return(&seven).expect("balance"),
            7
        );
        assert_eq!(
            MarketRegistryCalls::decode_mark_price_return(&seven).expect("price"),
            7
        );
        assert!(ERC20Calls::decode_bool_return(&yes).expect("bool"));
        assert_eq!(
            ERC20Calls::decode_allowance_return(&seven).expect("allowance"),
            7
        );
    }

    #[test]
    fn decodes_liquidation_state_return() {
        let mut yes = [0u8; 32];
        yes[31] = 1;

        let mut equity = [0xffu8; 32];
        equity[16..].copy_from_slice(&(-7i128).to_be_bytes());

        let mut maintenance = [0u8; 32];
        maintenance[31] = 9;

        let mut data = Vec::new();
        data.extend_from_slice(&yes);
        data.extend_from_slice(&equity);
        data.extend_from_slice(&maintenance);

        assert_eq!(
            LiquidationKeeperCalls::decode_liquidation_state_return(&data).expect("state decodes"),
            (true, -7, 9)
        );
    }
}
