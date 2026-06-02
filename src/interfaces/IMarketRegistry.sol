// SPDX-License-Identifier: MIT
pragma solidity ^0.8.26;

/// @title  IMarketRegistry
/// @notice Catalogue of tradeable perp markets. Each market binds a base
///         symbol (BTC, ETH, SOL, ...) to its price oracle and risk
///         parameters. Admin-curated in v0.1 to keep the surface small;
///         permissionless with bond + slashing in v1.1.
interface IMarketRegistry {
    /// @notice Per-market risk + execution parameters.
    /// @param  symbol             Human-readable base symbol (e.g. "BTC").
    /// @param  priceFeed          Oracle address (Pyth / Chainlink on Arc).
    /// @param  initialMarginBps   Initial margin in basis points (e.g. 1000 = 10%).
    /// @param  maintMarginBps     Maintenance margin in basis points (e.g. 500 = 5%).
    /// @param  maxLeverage        Maximum leverage implied by initial margin (e.g. 10x).
    /// @param  tickSize           Minimum price increment in PRICE_SCALE units.
    /// @param  lotSize            Minimum size increment in 1e18 base units.
    /// @param  maxPriceAge        Maximum accepted oracle age in seconds.
    /// @param  paused             Emergency pause flag; halts new orders but allows close.
    struct Market {
        string symbol;
        address priceFeed;
        uint16 initialMarginBps;
        uint16 maintMarginBps;
        uint8 maxLeverage;
        uint256 tickSize;
        uint256 lotSize;
        uint32 maxPriceAge;
        bool paused;
    }

    event MarketRegistered(uint256 indexed marketId, string symbol, address priceFeed);
    event MarketParamsUpdated(uint256 indexed marketId);
    event MarketPaused(uint256 indexed marketId, bool paused);

    /// @notice Register a new market. Admin-only in v0.1.
    function registerMarket(Market calldata params) external returns (uint256 marketId);

    /// @notice Update mutable risk params for an existing market. Admin-only.
    function updateMarketParams(uint256 marketId, Market calldata params) external;

    /// @notice Toggle the pause flag on a market. Admin-only emergency control.
    function setPaused(uint256 marketId, bool paused) external;

    /// @notice Fetch a market by id.
    function market(uint256 marketId) external view returns (Market memory);

    /// @notice Current mark price from the market's oracle, in PRICE_SCALE units.
    function markPrice(uint256 marketId) external view returns (uint256);

    /// @notice Total number of registered markets.
    function totalMarkets() external view returns (uint256);
}
