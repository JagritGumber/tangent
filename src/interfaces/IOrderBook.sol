// SPDX-License-Identifier: MIT
pragma solidity ^0.8.26;

import {OrderTypes} from "../types/OrderTypes.sol";

/// @title  IOrderBook
/// @notice On-chain CLOB with deterministic end-of-block batched matching.
///         Orders accumulate in contract state during the block; matching
///         runs at end-of-block via a permissionless tick. The batching
///         window eliminates intra-block MEV between order placement and
///         match.
interface IOrderBook {
    /// @notice Emitted when an order is accepted into the live book.
    event OrderSubmitted(
        bytes32 indexed orderHash,
        uint256 indexed accountId,
        uint256 indexed marketId,
        bool isBuy,
        uint256 limitPrice,
        uint256 size
    );

    /// @notice Emitted when an order is cancelled (by owner or expiry sweep).
    event OrderCancelled(bytes32 indexed orderHash, uint256 indexed accountId, string reason);

    /// @notice Emitted by tick() for each match produced during the matching pass.
    event Matched(
        bytes32 indexed buyOrderHash,
        bytes32 indexed sellOrderHash,
        uint256 indexed marketId,
        uint256 size,
        uint256 price
    );

    /// @notice Submit an EIP-712 signed order. Signature recovered against the
    ///         account's registered owner. Reverts on stale/duplicate nonce,
    ///         expired order, or invalid signature. Permissionless.
    function submitOrder(OrderTypes.Order calldata order, bytes calldata signature) external;

    /// @notice Cancel a previously submitted order. Only the account owner can
    ///         cancel. Idempotent against orders that already matched or expired.
    function cancelOrder(bytes32 orderHash) external;

    /// @notice Permissionless end-of-block tick. Walks the book once and emits
    ///         Matched events for the batch. Settlement handoff happens via
    ///         the one-shot-bound ISettlement contract.
    function tick() external;

    /// @notice True if the order with the given hash is still resting on the book.
    function isLive(bytes32 orderHash) external view returns (bool);

    /// @notice Full stored order metadata for settlement validation.
    function orderOf(bytes32 orderHash) external view returns (OrderTypes.Order memory order, bool exists);
}
