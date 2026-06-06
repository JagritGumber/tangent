//! # tangent-sdk
//!
//! Rust SDK for [Tangent](https://github.com/JagritGumber/tangent).
//!
//! Designed to be the low-level dependency a downstream agent (Selbo,
//! CapitalArc, future Arc-native agents) can use to integrate against the
//! on-chain Solidity primitives without copying Tangent ABI details. The
//! Solidity side lives at `../../src/`; this crate wraps the current raw
//! integration surface in typed Rust APIs.
//!
//! ## Current status (v0.1 of the parent repo)
//!
//! Pre-1.0. This crate currently ships the canonical EIP-712 [`Order`] type
//! mirroring `OrderTypes.sol`, signed-order calldata helpers, deployment
//! manifest parsing, primitive contract calldata helpers, and minimal ABI
//! return decoders. It does not yet open RPC connections, sign with Circle Dev
//! Wallets, estimate gas, or broadcast transactions. The full RPC client
//! (`TangentClient`), Circle Dev Wallet signing backend, and broadcast helpers
//! land at v0.8 of the parent repo, alongside the keeper daemon.
//!
//! See [`ARCHITECTURE.md`](https://github.com/JagritGumber/tangent/blob/main/ARCHITECTURE.md)
//! for the full system design and roadmap.

#![doc(html_root_url = "https://docs.rs/tangent-sdk")]

pub mod abi;
pub mod contracts;
pub mod domain;
mod eip712;
pub mod manifest;
pub mod order;
pub mod orderbook;
pub mod signing;

pub use abi::AbiDecodeError;
pub use contracts::{AccountManagerCalls, ERC20Calls, MarketRegistryCalls, USDCVaultCalls};
pub use domain::DomainSeparatorInput;
pub use manifest::{ContractAddresses, DeploymentManifest, ManifestError, NetworkConstants};
pub use order::{
    Order, OrderBuilder, OrderConstraints, OrderError, OrderParams, Side, BASE_SCALE, PRICE_SCALE,
};
pub use orderbook::OrderBookCalls;
pub use signing::{OrderSignature, PreparedOrder, SignatureError, SignedOrder};
