// SPDX-License-Identifier: MIT
pragma solidity ^0.8.26;

/// @title  OrderTypes
/// @notice EIP-712 typed-data schema for Tangent orders.
/// @dev    Canonical public schema. Designed to be a stable baseline future
///         Arc perp builders can extend rather than reinvent. Mirrors the
///         Shapeshifter CMDT ClearingHouse Order struct shape (which is
///         already on-chain verifiable via their verified OrderTypes.sol)
///         under a distinct EIP-712 domain so signatures are not portable
///         between the two systems.
library OrderTypes {
    /// @notice Single order placed by an account against a market.
    /// @param  accountId    AccountManager-assigned identifier of the trader.
    /// @param  marketId     MarketRegistry-assigned identifier of the perp market.
    /// @param  isBuy        true = long entry / short close, false = short entry / long close.
    /// @param  limitPrice   Worst-acceptable price in PRICE_SCALE units (1e8 = $1).
    /// @param  size         Base quantity in 1e18 units.
    /// @param  nonce        Monotonic per-account counter; settled orders consume their nonce.
    /// @param  expiry       block.timestamp cutoff. Orders past expiry are rejected at submit.
    /// @param  reduceOnly   true = order may only reduce an existing position, never flip or open.
    struct Order {
        uint256 accountId;
        uint256 marketId;
        bool isBuy;
        uint256 limitPrice;
        uint256 size;
        uint256 nonce;
        uint256 expiry;
        bool reduceOnly;
    }

    /// @notice EIP-712 typeHash for Order. Computed once at deploy + cached.
    bytes32 internal constant ORDER_TYPEHASH = keccak256(
        "Order(uint256 accountId,uint256 marketId,bool isBuy,uint256 limitPrice,uint256 size,uint256 nonce,uint256 expiry,bool reduceOnly)"
    );

    /// @notice Hash a single Order for EIP-712 signing.
    function hash(Order memory o) internal pure returns (bytes32) {
        return keccak256(
            abi.encode(
                ORDER_TYPEHASH,
                o.accountId,
                o.marketId,
                o.isBuy,
                o.limitPrice,
                o.size,
                o.nonce,
                o.expiry,
                o.reduceOnly
            )
        );
    }

    /// @notice EIP-712 domain separator construction. Domain name is Tangent
    ///         v1; chainId and verifyingContract are bound at deploy.
    function domainSeparator(uint256 chainId, address verifyingContract) internal pure returns (bytes32) {
        return keccak256(
            abi.encode(
                keccak256("EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)"),
                keccak256(bytes("Tangent")),
                keccak256(bytes("v1")),
                chainId,
                verifyingContract
            )
        );
    }

    /// @notice Compose the final digest a wallet must sign for an Order.
    function digest(Order memory o, bytes32 domainSep) internal pure returns (bytes32) {
        return keccak256(abi.encodePacked("\x19\x01", domainSep, hash(o)));
    }
}
