// SPDX-License-Identifier: MIT
pragma solidity ^0.8.26;

import {Test} from "forge-std/Test.sol";
import {AccountManager} from "../src/AccountManager.sol";
import {IAccountManager} from "../src/interfaces/IAccountManager.sol";

contract AccountManagerTest is Test {
    AccountManager internal mgr;

    address internal alice = address(0xA11CE);
    address internal bob = address(0xB0B);

    event AccountRegistered(uint256 indexed accountId, address indexed owner, uint64 registeredAt);

    function setUp() public {
        mgr = new AccountManager();
    }

    function test_registerAccount_emitsAndAssignsMonotonicIds() public {
        vm.expectEmit(true, true, false, true);
        emit AccountRegistered(1, alice, uint64(block.timestamp));

        vm.prank(alice);
        uint256 aliceId = mgr.registerAccount();
        assertEq(aliceId, 1, "first account is id 1, not 0");

        vm.prank(bob);
        uint256 bobId = mgr.registerAccount();
        assertEq(bobId, 2, "second account is id 2");

        assertEq(mgr.totalAccounts(), 2, "two accounts total");
        assertEq(mgr.ownerOf(1), alice);
        assertEq(mgr.ownerOf(2), bob);
        assertEq(mgr.accountIdOf(alice), 1);
        assertEq(mgr.accountIdOf(bob), 2);
    }

    function test_registerAccount_revertsOnDoubleRegister() public {
        vm.prank(alice);
        uint256 first = mgr.registerAccount();

        vm.expectRevert(abi.encodeWithSelector(AccountManager.AlreadyRegistered.selector, alice, first));
        vm.prank(alice);
        mgr.registerAccount();
    }

    function test_ownerOf_revertsOnUnknownAccount() public {
        vm.expectRevert(abi.encodeWithSelector(AccountManager.UnknownAccount.selector, uint256(999)));
        mgr.ownerOf(999);
    }

    function testFuzz_registerAccount_assignsUniqueIds(address[8] memory eoas) public {
        // Sanity: each distinct caller (with a non-zero address that hasn't
        // already registered) gets a unique monotonic id.
        uint256 expectedId = 0;
        for (uint256 i = 0; i < eoas.length; i++) {
            address eoa = eoas[i];
            if (eoa == address(0)) continue;
            if (mgr.accountIdOf(eoa) != 0) continue; // dup in fuzz input

            unchecked {
                expectedId++;
            }
            vm.prank(eoa);
            uint256 id = mgr.registerAccount();
            assertEq(id, expectedId, "id mismatches monotonic expectation");
            assertEq(mgr.ownerOf(id), eoa);
            assertEq(mgr.accountIdOf(eoa), id);
        }
        assertEq(mgr.totalAccounts(), expectedId, "totalAccounts tracks unique registrations");
    }
}
