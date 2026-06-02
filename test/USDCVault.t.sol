// SPDX-License-Identifier: MIT
pragma solidity ^0.8.26;

import {Test} from "forge-std/Test.sol";
import {USDCVault, IERC20} from "../src/USDCVault.sol";
import {AccountManager} from "../src/AccountManager.sol";
import {IAccountManager} from "../src/interfaces/IAccountManager.sol";

/// @notice Minimal in-test ERC-20 used as the USDC stand-in. Mintable for
///         test setup, otherwise standard transfer / transferFrom semantics.
contract MockUSDC is IERC20 {
    string public name = "Mock USDC";
    string public symbol = "USDC";
    uint8 public decimals = 6;
    uint256 public totalSupply;

    mapping(address => uint256) public balanceOf;
    mapping(address => mapping(address => uint256)) public allowance;

    function mint(address to, uint256 amount) external {
        balanceOf[to] += amount;
        totalSupply += amount;
    }

    function approve(address spender, uint256 amount) external returns (bool) {
        allowance[msg.sender][spender] = amount;
        return true;
    }

    function transfer(address to, uint256 amount) external override returns (bool) {
        require(balanceOf[msg.sender] >= amount, "balance");
        balanceOf[msg.sender] -= amount;
        balanceOf[to] += amount;
        return true;
    }

    function transferFrom(address from, address to, uint256 amount) external override returns (bool) {
        require(balanceOf[from] >= amount, "balance");
        require(allowance[from][msg.sender] >= amount, "allowance");
        allowance[from][msg.sender] -= amount;
        balanceOf[from] -= amount;
        balanceOf[to] += amount;
        return true;
    }
}

contract USDCVaultTest is Test {
    MockUSDC internal usdc;
    AccountManager internal accounts;
    USDCVault internal vault;

    address internal alice = address(0xA11CE);
    address internal bob = address(0xB0B);
    address internal settlementEngine = address(0x5E771);

    uint256 internal aliceId;
    uint256 internal bobId;

    function setUp() public {
        usdc = new MockUSDC();
        accounts = new AccountManager();
        vault = new USDCVault(IERC20(address(usdc)), IAccountManager(address(accounts)));

        vm.prank(alice);
        aliceId = accounts.registerAccount();
        vm.prank(bob);
        bobId = accounts.registerAccount();

        usdc.mint(alice, 1_000_000_000); // 1000 USDC (6 decimals)
        usdc.mint(bob, 500_000_000);

        vm.prank(alice);
        usdc.approve(address(vault), type(uint256).max);
        vm.prank(bob);
        usdc.approve(address(vault), type(uint256).max);
    }

    // -- constructor + bind --

    function test_bindSettlementEngine_revertsOnNonBinder() public {
        vm.expectRevert(abi.encodeWithSelector(USDCVault.OnlySettlementBinder.selector, alice, address(this)));
        vm.prank(alice);
        vault.bindSettlementEngine(settlementEngine);
    }

    function test_bindSettlementEngine_revertsOnSecondBind() public {
        vault.bindSettlementEngine(settlementEngine);
        vm.expectRevert(
            abi.encodeWithSelector(USDCVault.SettlementEngineAlreadyBound.selector, settlementEngine)
        );
        vault.bindSettlementEngine(address(0xBEEF));
    }

    function test_bindSettlementEngine_revertsOnZeroAddress() public {
        vm.expectRevert(USDCVault.ZeroAddress.selector);
        vault.bindSettlementEngine(address(0));
    }

    // -- deposit --

    function test_deposit_creditsFreeBalance() public {
        vm.prank(alice);
        vault.deposit(aliceId, 100_000_000); // 100 USDC

        assertEq(vault.freeBalanceOf(aliceId), 100_000_000);
        assertEq(vault.lockedBalanceOf(aliceId), 0);
        assertEq(vault.totalBalanceOf(aliceId), 100_000_000);
        assertEq(usdc.balanceOf(address(vault)), 100_000_000);
        assertEq(usdc.balanceOf(alice), 900_000_000);
    }

    function test_deposit_allowsThirdPartyFunder() public {
        // Bob funds Alice's account. Vault doesn't require depositor == owner.
        vm.prank(bob);
        vault.deposit(aliceId, 50_000_000);

        assertEq(vault.freeBalanceOf(aliceId), 50_000_000);
        assertEq(vault.freeBalanceOf(bobId), 0);
    }

    function test_deposit_revertsOnUnknownAccount() public {
        vm.prank(alice);
        vm.expectRevert(abi.encodeWithSelector(AccountManager.UnknownAccount.selector, uint256(999)));
        vault.deposit(999, 1_000_000);
    }

    // -- withdraw --

    function test_withdraw_debitsFreeBalanceAndTransfers() public {
        vm.prank(alice);
        vault.deposit(aliceId, 100_000_000);

        vm.prank(alice);
        vault.withdraw(aliceId, 40_000_000, alice);

        assertEq(vault.freeBalanceOf(aliceId), 60_000_000);
        assertEq(usdc.balanceOf(alice), 940_000_000);
    }

    function test_withdraw_revertsOnNonOwner() public {
        vm.prank(alice);
        vault.deposit(aliceId, 100_000_000);

        vm.expectRevert(abi.encodeWithSelector(USDCVault.NotAccountOwner.selector, bob, aliceId, alice));
        vm.prank(bob);
        vault.withdraw(aliceId, 1_000_000, bob);
    }

    function test_withdraw_revertsOnInsufficientFree() public {
        vm.prank(alice);
        vault.deposit(aliceId, 10_000_000);
        vm.expectRevert(
            abi.encodeWithSelector(
                USDCVault.InsufficientFree.selector, aliceId, uint256(50_000_000), uint256(10_000_000)
            )
        );
        vm.prank(alice);
        vault.withdraw(aliceId, 50_000_000, alice);
    }

    // -- margin hooks gated until binding --

    function test_marginHooks_revertWhenSettlementNotBound() public {
        vm.expectRevert(USDCVault.SettlementEngineNotBound.selector);
        vm.prank(settlementEngine);
        vault.lockMargin(aliceId, 1_000_000);

        vm.expectRevert(USDCVault.SettlementEngineNotBound.selector);
        vm.prank(settlementEngine);
        vault.releaseMargin(aliceId, 1_000_000);

        vm.expectRevert(USDCVault.SettlementEngineNotBound.selector);
        vm.prank(settlementEngine);
        vault.applyPnL(aliceId, 1_000_000);
    }

    function test_marginHooks_revertWhenCallerIsNotSettlement() public {
        vault.bindSettlementEngine(settlementEngine);

        vm.expectRevert(abi.encodeWithSelector(USDCVault.OnlySettlementEngine.selector, alice));
        vm.prank(alice);
        vault.lockMargin(aliceId, 1_000_000);
    }

    // -- margin hooks happy paths (after binding) --

    function test_lockMargin_movesFreeToLocked() public {
        vault.bindSettlementEngine(settlementEngine);
        vm.prank(alice);
        vault.deposit(aliceId, 100_000_000);

        vm.prank(settlementEngine);
        vault.lockMargin(aliceId, 30_000_000);

        assertEq(vault.freeBalanceOf(aliceId), 70_000_000);
        assertEq(vault.lockedBalanceOf(aliceId), 30_000_000);
        assertEq(vault.totalBalanceOf(aliceId), 100_000_000);
    }

    function test_lockMargin_revertsOnInsufficientFree() public {
        vault.bindSettlementEngine(settlementEngine);
        vm.prank(alice);
        vault.deposit(aliceId, 10_000_000);

        vm.expectRevert(
            abi.encodeWithSelector(
                USDCVault.InsufficientFree.selector, aliceId, uint256(50_000_000), uint256(10_000_000)
            )
        );
        vm.prank(settlementEngine);
        vault.lockMargin(aliceId, 50_000_000);
    }

    function test_releaseMargin_movesLockedToFree() public {
        vault.bindSettlementEngine(settlementEngine);
        vm.prank(alice);
        vault.deposit(aliceId, 100_000_000);
        vm.prank(settlementEngine);
        vault.lockMargin(aliceId, 60_000_000);

        vm.prank(settlementEngine);
        vault.releaseMargin(aliceId, 40_000_000);

        assertEq(vault.freeBalanceOf(aliceId), 80_000_000);
        assertEq(vault.lockedBalanceOf(aliceId), 20_000_000);
    }

    function test_applyPnL_positiveAddsToFree() public {
        vault.bindSettlementEngine(settlementEngine);
        vm.prank(alice);
        vault.deposit(aliceId, 100_000_000);

        vm.prank(settlementEngine);
        vault.applyPnL(aliceId, 25_000_000);

        assertEq(vault.freeBalanceOf(aliceId), 125_000_000);
    }

    function test_applyPnL_negativeAbsorbsFromFreeFirst() public {
        vault.bindSettlementEngine(settlementEngine);
        vm.prank(alice);
        vault.deposit(aliceId, 100_000_000);
        vm.prank(settlementEngine);
        vault.lockMargin(aliceId, 30_000_000); // 70 free / 30 locked

        vm.prank(settlementEngine);
        vault.applyPnL(aliceId, -50_000_000); // loss < free

        assertEq(vault.freeBalanceOf(aliceId), 20_000_000);
        assertEq(vault.lockedBalanceOf(aliceId), 30_000_000);
    }

    function test_applyPnL_negativeOverflowsIntoLocked() public {
        vault.bindSettlementEngine(settlementEngine);
        vm.prank(alice);
        vault.deposit(aliceId, 100_000_000);
        vm.prank(settlementEngine);
        vault.lockMargin(aliceId, 60_000_000); // 40 free / 60 locked

        vm.prank(settlementEngine);
        vault.applyPnL(aliceId, -75_000_000); // free=40, then 35 from locked

        assertEq(vault.freeBalanceOf(aliceId), 0);
        assertEq(vault.lockedBalanceOf(aliceId), 25_000_000);
    }

    function test_applyPnL_negativeBeyondTotalZerosOut() public {
        vault.bindSettlementEngine(settlementEngine);
        vm.prank(alice);
        vault.deposit(aliceId, 100_000_000);
        vm.prank(settlementEngine);
        vault.lockMargin(aliceId, 60_000_000);

        vm.prank(settlementEngine);
        vault.applyPnL(aliceId, -250_000_000); // larger than total balance

        assertEq(vault.freeBalanceOf(aliceId), 0);
        assertEq(vault.lockedBalanceOf(aliceId), 0);
    }

    // -- fuzz --

    function testFuzz_depositThenWithdrawIsConserved(uint128 depositAmt, uint128 withdrawAmt) public {
        vm.assume(depositAmt > 0);
        vm.assume(depositAmt <= 1_000_000_000); // <= mint
        uint256 actualWithdraw =
            uint256(withdrawAmt) > uint256(depositAmt) ? uint256(depositAmt) : uint256(withdrawAmt);
        vm.assume(actualWithdraw > 0);

        vm.prank(alice);
        vault.deposit(aliceId, depositAmt);
        vm.prank(alice);
        vault.withdraw(aliceId, actualWithdraw, alice);

        assertEq(vault.freeBalanceOf(aliceId), uint256(depositAmt) - actualWithdraw);
        assertEq(usdc.balanceOf(alice), 1_000_000_000 - (uint256(depositAmt) - actualWithdraw));
    }

    function testFuzz_lockReleaseConservesTotal(uint128 deposit, uint128 lockAmt) public {
        vm.assume(deposit > 0 && deposit <= 1_000_000_000);
        uint256 lockable = lockAmt > deposit ? deposit : lockAmt;
        vm.assume(lockable > 0);

        vault.bindSettlementEngine(settlementEngine);
        vm.prank(alice);
        vault.deposit(aliceId, deposit);

        uint256 totalBefore = vault.totalBalanceOf(aliceId);
        vm.prank(settlementEngine);
        vault.lockMargin(aliceId, lockable);
        assertEq(vault.totalBalanceOf(aliceId), totalBefore);

        vm.prank(settlementEngine);
        vault.releaseMargin(aliceId, lockable);
        assertEq(vault.totalBalanceOf(aliceId), totalBefore);
        assertEq(vault.freeBalanceOf(aliceId), deposit);
        assertEq(vault.lockedBalanceOf(aliceId), 0);
    }
}
