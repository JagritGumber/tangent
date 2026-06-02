// SPDX-License-Identifier: MIT
pragma solidity ^0.8.26;

import {IMarketRegistry} from "./interfaces/IMarketRegistry.sol";
import {IPriceFeed} from "./interfaces/IPriceFeed.sol";

/// @title  MarketRegistry
/// @notice Catalogue of tradeable perp markets. Each market binds a base
///         symbol (BTC, ETH, SOL...) to an IPriceFeed adapter + risk params.
///         Admin-curated in v0.3 to keep launch hands careful; permissionless
///         with a bond + slashing model in v0.9. The admin set at deploy
///         time can be a single multisig or any contract; this contract
///         does not care about the admin's identity, only that
///         risk-param mutations come from it.
///
/// @dev    Staleness checking is enforced at the registry level so every
///         mark-price consumer fails closed under the same per-market policy.
///         Oracle adapters remain responsible for normalizing their native
///         feed data into (PRICE_SCALE price, publishedAt timestamp).
contract MarketRegistry is IMarketRegistry {
    /// @notice Privileged address allowed to register, update, and pause
    ///         markets. Typically a governance multisig at production
    ///         deployments; the deployer for hackathon-scope deployments.
    address public admin;

    /// @notice Total number of registered markets. Also the next marketId
    ///         to be assigned (after pre-increment). MarketId 0 is reserved
    ///         as the unregistered sentinel.
    uint256 public override totalMarkets;

    /// @notice marketId -> full Market struct.
    mapping(uint256 marketId => Market) private _markets;

    error NotAdmin(address caller);
    error UnknownMarket(uint256 marketId);
    error MarketPausedError(uint256 marketId);
    error InvalidPriceFeed();
    error InvalidMarginParams(uint16 initialBps, uint16 maintBps);
    error InvalidLeverage(uint8 maxLeverage);
    error InitialMarginBelowMaxLeverage(uint16 initialBps, uint8 maxLeverage, uint16 requiredInitialBps);
    error InvalidTickOrLot(uint256 tickSize, uint256 lotSize);
    error InvalidMaxPriceAge();
    error InvalidPrice(uint256 marketId, uint256 price);
    error InvalidPriceTimestamp(uint256 marketId, uint256 publishedAt);
    error StalePrice(uint256 marketId, uint256 publishedAt, uint256 maxAge);
    error EmptySymbol();
    error ZeroAddress();

    constructor(address _admin) {
        if (_admin == address(0)) revert ZeroAddress();
        admin = _admin;
    }

    modifier onlyAdmin() {
        if (msg.sender != admin) revert NotAdmin(msg.sender);
        _;
    }

    /// @inheritdoc IMarketRegistry
    function registerMarket(Market calldata params) external override onlyAdmin returns (uint256 marketId) {
        _validateMarketParams(params);
        unchecked {
            marketId = ++totalMarkets; // 1-based; 0 reserved as unregistered
        }
        _markets[marketId] = params;
        emit MarketRegistered(marketId, params.symbol, params.priceFeed);
    }

    /// @inheritdoc IMarketRegistry
    function updateMarketParams(uint256 marketId, Market calldata params) external override onlyAdmin {
        _requireKnownMarket(marketId);
        _validateMarketParams(params);
        _markets[marketId] = params;
        emit MarketParamsUpdated(marketId);
    }

    /// @inheritdoc IMarketRegistry
    function setPaused(uint256 marketId, bool paused) external override onlyAdmin {
        _requireKnownMarket(marketId);
        _markets[marketId].paused = paused;
        emit MarketPaused(marketId, paused);
    }

    /// @inheritdoc IMarketRegistry
    function market(uint256 marketId) external view override returns (Market memory) {
        _requireKnownMarket(marketId);
        return _markets[marketId];
    }

    /// @inheritdoc IMarketRegistry
    /// @dev Reverts when the oracle price is zero, timestamped in the
    ///      future, or older than the market's maxPriceAge.
    function markPrice(uint256 marketId) external view override returns (uint256) {
        _requireKnownMarket(marketId);
        Market storage m = _markets[marketId];
        IPriceFeed feed = IPriceFeed(m.priceFeed);
        (uint256 price, uint256 publishedAt) = feed.latestPrice();
        if (price == 0) revert InvalidPrice(marketId, price);
        // Oracle freshness is intentionally timestamp-bound.
        // forge-lint: disable-next-line(block-timestamp)
        if (publishedAt == 0 || publishedAt > block.timestamp) {
            revert InvalidPriceTimestamp(marketId, publishedAt);
        }
        // forge-lint: disable-next-line(block-timestamp)
        if (block.timestamp - publishedAt > m.maxPriceAge) {
            revert StalePrice(marketId, publishedAt, m.maxPriceAge);
        }
        return price;
    }

    function _requireKnownMarket(uint256 marketId) internal view {
        if (marketId == 0 || marketId > totalMarkets) revert UnknownMarket(marketId);
    }

    function _validateMarketParams(Market calldata params) internal pure {
        if (bytes(params.symbol).length == 0) revert EmptySymbol();
        if (params.priceFeed == address(0)) revert InvalidPriceFeed();
        // initialMarginBps must be at least maintMarginBps (you can't open a
        // position that's already eligible for liquidation), and both must
        // fit within the 0-10000 bps range.
        if (
            params.initialMarginBps == 0 || params.initialMarginBps > 10000
                || params.maintMarginBps == 0 || params.maintMarginBps > 10000
                || params.maintMarginBps > params.initialMarginBps
        ) {
            revert InvalidMarginParams(params.initialMarginBps, params.maintMarginBps);
        }
        if (params.maxLeverage == 0 || params.maxLeverage > 100) {
            revert InvalidLeverage(params.maxLeverage);
        }
        // Keep the human-readable maxLeverage in sync with the margin math
        // SettlementEngine actually enforces.
        uint16 requiredInitialBps = uint16((10_000 + params.maxLeverage - 1) / params.maxLeverage);
        if (params.initialMarginBps < requiredInitialBps) {
            revert InitialMarginBelowMaxLeverage(
                params.initialMarginBps, params.maxLeverage, requiredInitialBps
            );
        }
        if (params.tickSize == 0 || params.lotSize == 0) {
            revert InvalidTickOrLot(params.tickSize, params.lotSize);
        }
        if (params.maxPriceAge == 0) revert InvalidMaxPriceAge();
    }
}
