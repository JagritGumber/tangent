// SPDX-License-Identifier: MIT
pragma solidity ^0.8.26;

/// @title  ISettlement
/// @notice Settlement of matched orders produced by the bound OrderBook.
///         System-level permissionlessness comes from OrderBook.tick(), which
///         anyone can call; direct settlement is restricted to the book so
///         fills and book state remain atomic.
interface ISettlement {
    /// @notice Account position in a single market.
    /// @param  size          Signed base size. Positive = long, negative = short.
    /// @param  entryPrice    Average entry price in PRICE_SCALE units.
    /// @param  lockedMargin  USDC margin locked against the position.
    struct Position {
        int256 size;
        uint256 entryPrice;
        uint256 lockedMargin;
    }

    /// @notice A single matched fill between two opposing orders.
    /// @param  buyOrderHash    Hash of the buy-side order (long entry or short close).
    /// @param  sellOrderHash   Hash of the sell-side order (short entry or long close).
    /// @param  buyAccountId    Account opening/adjusting the long side of the fill.
    /// @param  sellAccountId   Account opening/adjusting the short side of the fill.
    /// @param  marketId        Market the fill executed in.
    /// @param  size            Fill size in 1e18 base units.
    /// @param  price           Fill price in PRICE_SCALE units (1e8 = $1).
    struct Match {
        bytes32 buyOrderHash;
        bytes32 sellOrderHash;
        uint256 buyAccountId;
        uint256 sellAccountId;
        uint256 marketId;
        uint256 size;
        uint256 price;
    }

    /// @notice Emitted per match successfully settled.
    event Settled(
        bytes32 indexed buyOrderHash,
        bytes32 indexed sellOrderHash,
        uint256 indexed marketId,
        uint256 size,
        uint256 price
    );

    /// @notice Settle a batch of matches from the bound OrderBook. Reverts the
    ///         whole batch on any invalid match or margin failure.
    function settleBatch(Match[] calldata matches) external;

    /// @notice Current position for an account in a market.
    function positionOf(uint256 accountId, uint256 marketId) external view returns (Position memory);

    /// @notice Liquidation hook callable only by the bound LiquidationKeeper.
    ///         Closes the entire position at mark price and realizes PnL.
    function forceClose(uint256 accountId, uint256 marketId, uint256 price) external returns (int256 pnl);

    /// @notice Reverts if withdrawing `amount` would leave the account below
    ///         aggregate maintenance margin across open positions.
    function validateWithdrawal(uint256 accountId, uint256 amount) external view;
}
