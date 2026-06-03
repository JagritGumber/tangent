//! # tangent-sdk
//!
//! Rust SDK for [Tangent](https://github.com/JagritGumber/tangent).
//!
//! Designed to be the single dependency a downstream agent (Selbo, CapitalArc,
//! future Arc-native agents) needs to integrate against the on-chain Solidity
//! primitives. The Solidity side lives at `../../src/`; this crate wraps it
//! in typed Rust APIs.
//!
//! ## Current status (v0.1 of the parent repo)
//!
//! Pre-1.0. This crate currently ships the canonical EIP-712 [`Order`] type
//! mirroring `OrderTypes.sol` so off-chain code can construct orders with
//! the same shape the on-chain `OrderBook` will accept. The full RPC client
//! (`TangentClient`), Circle Dev Wallet signing backend, and broadcast
//! helpers land at v0.8 of the parent repo, alongside the keeper daemon.
//!
//! See [`ARCHITECTURE.md`](https://github.com/JagritGumber/tangent/blob/main/ARCHITECTURE.md)
//! for the full system design and roadmap.

#![doc(html_root_url = "https://docs.rs/tangent-sdk")]

pub mod domain;
pub mod order;

pub use domain::DomainSeparatorInput;
pub use order::{Order, OrderBuilder, OrderConstraints, OrderError, Side, BASE_SCALE, PRICE_SCALE};
