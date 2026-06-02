// SPDX-License-Identifier: MIT
pragma solidity ^0.8.26;

import {Test} from "forge-std/Test.sol";
import {OrderTypes} from "../src/types/OrderTypes.sol";

/// @notice Tests for the canonical EIP-712 Order schema. These pin the wire
///         format. Any change here is a signature-breaking change and the
///         downstream Rust SDK (v0.8) must rev its embedded type hash to match.
///         The CHANGELOG must note the bump explicitly.
contract OrderTypesTest is Test {
    address internal constant TEST_VERIFYING_CONTRACT = address(0xC0DE);

    /// @notice The frozen typeHash for Order. If this assertion ever fails,
    ///         it means the Order struct shape was changed without
    ///         coordinating the wire bump. Hardcoded so a drift is loud.
    bytes32 internal constant EXPECTED_ORDER_TYPEHASH = keccak256(
        "Order(uint256 accountId,uint256 marketId,bool isBuy,uint256 limitPrice,uint256 size,uint256 nonce,uint256 expiry,bool reduceOnly)"
    );

    function test_typeHash_isFrozen() public pure {
        assertEq(OrderTypes.ORDER_TYPEHASH, EXPECTED_ORDER_TYPEHASH, "ORDER_TYPEHASH drift");
    }

    function test_hash_changesWithEveryField() public pure {
        OrderTypes.Order memory base = _sampleOrder();
        bytes32 baseHash = OrderTypes.hash(base);

        OrderTypes.Order memory mutated = _sampleOrder();
        mutated.accountId = 99;
        assertTrue(OrderTypes.hash(mutated) != baseHash, "accountId must be in hash");

        mutated = _sampleOrder();
        mutated.marketId = 99;
        assertTrue(OrderTypes.hash(mutated) != baseHash, "marketId must be in hash");

        mutated = _sampleOrder();
        mutated.isBuy = !base.isBuy;
        assertTrue(OrderTypes.hash(mutated) != baseHash, "isBuy must be in hash");

        mutated = _sampleOrder();
        mutated.limitPrice = base.limitPrice + 1;
        assertTrue(OrderTypes.hash(mutated) != baseHash, "limitPrice must be in hash");

        mutated = _sampleOrder();
        mutated.size = base.size + 1;
        assertTrue(OrderTypes.hash(mutated) != baseHash, "size must be in hash");

        mutated = _sampleOrder();
        mutated.nonce = base.nonce + 1;
        assertTrue(OrderTypes.hash(mutated) != baseHash, "nonce must be in hash");

        mutated = _sampleOrder();
        mutated.expiry = base.expiry + 1;
        assertTrue(OrderTypes.hash(mutated) != baseHash, "expiry must be in hash");

        mutated = _sampleOrder();
        mutated.reduceOnly = !base.reduceOnly;
        assertTrue(OrderTypes.hash(mutated) != baseHash, "reduceOnly must be in hash");
    }

    function test_digest_signRecoverRoundtrip() public pure {
        OrderTypes.Order memory o = _sampleOrder();
        bytes32 domainSep = OrderTypes.domainSeparator(uint256(11111), TEST_VERIFYING_CONTRACT);
        bytes32 d = OrderTypes.digest(o, domainSep);

        uint256 pk = 0xA11CE;
        address signer = vm.addr(pk);
        (uint8 v, bytes32 r, bytes32 s) = vm.sign(pk, d);
        address recovered = ecrecover(d, v, r, s);

        assertEq(recovered, signer, "signed digest must recover to signer address");
    }

    function _sampleOrder() internal pure returns (OrderTypes.Order memory) {
        return OrderTypes.Order({
            accountId: 7,
            marketId: 1,
            isBuy: true,
            limitPrice: 6500000000000, // $65,000 in 1e8 scale
            size: 1e18, // 1 BTC notional
            nonce: 42,
            expiry: 1717000000,
            reduceOnly: false
        });
    }
}
