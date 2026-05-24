// SPDX-License-Identifier: MIT
pragma solidity ^0.8.26;

/// @title  IPriceFeed
/// @notice Minimal oracle adapter interface used by MarketRegistry.markPrice.
///         Designed to be implementable against any underlying feed (Pyth,
///         Chainlink, Redstone, custom) without leaking the underlying
///         oracle's quirks into MarketRegistry. The MarketRegistry stores
///         an `address priceFeed` per market; the adapter contract is the
///         per-oracle integration.
///
///         Price is in PRICE_SCALE = 1e8 units (i.e. 1e8 == $1). All ArcPerpRef
///         math is in this scale; adapters are responsible for normalizing.
///         publishedAt is a unix timestamp; consumers check staleness against
///         their own max-age policy.
interface IPriceFeed {
    /// @notice Latest published price for the asset.
    /// @return price       in PRICE_SCALE (1e8) units
    /// @return publishedAt unix timestamp the price was attested at
    function latestPrice() external view returns (uint256 price, uint256 publishedAt);
}
