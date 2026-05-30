// SPDX-License-Identifier: MIT
pragma solidity ^0.8.26;

import {IUSDCVault} from "./interfaces/IUSDCVault.sol";
import {IAccountManager} from "./interfaces/IAccountManager.sol";

/// @notice Minimal ERC-20 interface for the USDC token on Arc Testnet. We
///         depend only on transferFrom + transfer to keep the surface small
///         and forkable for non-USDC collateral tokens.
interface IERC20 {
    function transferFrom(address from, address to, uint256 amount) external returns (bool);
    function transfer(address to, uint256 amount) external returns (bool);
    function balanceOf(address account) external view returns (uint256);
}

/// @title  USDCVault
/// @notice Per-account USDC collateral for Tangent. Tracks free + locked
///         balances per accountId, exposes deposit/withdraw to account owners,
///         and exposes lockMargin/releaseMargin/applyPnL hooks gated to the
///         deploy-time-bound SettlementEngine.
///
///         Contrast with Shapeshifter's USDCCollateralVault: that vault binds
///         collateral to Fireblocks-custodied accounts via off-chain
///         provisioning. This vault is non-custodial - the only contract that
///         can lock or release margin is the SettlementEngine bound once by
///         the immutable settlementBinder. There is no admin upgrade path
///         after the one-shot binding.
///
/// @dev    v0.1 ships the deposit/withdraw path fully working. The margin
///         hooks (lockMargin/releaseMargin/applyPnL) revert until the
///         SettlementEngine is bound, which lets v0.1 be deployed and used
///         today while keeping the v0.3-v0.5 wiring frozen ahead of time.
contract USDCVault is IUSDCVault {
    /// @notice The USDC ERC-20 contract on Arc Testnet. Immutable: a fork
    ///         that needs a different collateral token deploys a new vault.
    IERC20 public immutable usdc;

    /// @notice The AccountManager that owns the canonical `ownerOf` mapping.
    IAccountManager public immutable accounts;

    /// @notice Address permitted to perform the one-shot settlement-engine
    ///         binding. Usually the deployment EOA or deployment coordinator.
    ///         Has no power after settlementEngine is set.
    address public immutable settlementBinder;

    /// @notice The SettlementEngine permitted to call margin hooks. Set once
    ///         via `bindSettlementEngine`; zero address until then.
    address public settlementEngine;

    /// @notice Per-account free (withdrawable) USDC balance, in 6-decimal units.
    mapping(uint256 accountId => uint256) private _free;

    /// @notice Per-account locked-as-margin USDC balance, in 6-decimal units.
    mapping(uint256 accountId => uint256) private _locked;

    error UnknownAccount(uint256 accountId);
    error NotAccountOwner(address caller, uint256 accountId, address owner);
    error InsufficientFree(uint256 accountId, uint256 requested, uint256 available);
    error InsufficientLocked(uint256 accountId, uint256 requested, uint256 available);
    error TransferFailed();
    error SettlementEngineNotBound();
    error OnlySettlementEngine(address caller);
    error OnlySettlementBinder(address caller, address binder);
    error SettlementEngineAlreadyBound(address current);
    error ZeroAmount();
    error ZeroAddress();

    constructor(IERC20 _usdc, IAccountManager _accounts) {
        if (address(_usdc) == address(0)) revert ZeroAddress();
        if (address(_accounts) == address(0)) revert ZeroAddress();
        usdc = _usdc;
        accounts = _accounts;
        settlementBinder = msg.sender;
    }

    /// @notice One-shot wiring of the SettlementEngine. Reverts on second
    ///         call so the binding is immutable in practice without an admin
    ///         upgrade path. Called by the deployment coordinator.
    function bindSettlementEngine(address engine) external {
        if (msg.sender != settlementBinder) revert OnlySettlementBinder(msg.sender, settlementBinder);
        if (engine == address(0)) revert ZeroAddress();
        if (settlementEngine != address(0)) revert SettlementEngineAlreadyBound(settlementEngine);
        settlementEngine = engine;
    }

    /// @inheritdoc IUSDCVault
    function deposit(uint256 accountId, uint256 amount) external override {
        if (amount == 0) revert ZeroAmount();
        // Trip a clear revert before transferFrom so the failure mode is
        // legible. We do NOT require the depositor to be the account owner -
        // a delegated funder can top up someone else's vault.
        address owner = accounts.ownerOf(accountId);
        if (owner == address(0)) revert UnknownAccount(accountId);

        if (!usdc.transferFrom(msg.sender, address(this), amount)) revert TransferFailed();
        _free[accountId] += amount;
        emit Deposited(accountId, msg.sender, amount);
    }

    /// @inheritdoc IUSDCVault
    function withdraw(uint256 accountId, uint256 amount, address to) external override {
        if (amount == 0) revert ZeroAmount();
        if (to == address(0)) revert ZeroAddress();
        address owner = accounts.ownerOf(accountId);
        if (msg.sender != owner) revert NotAccountOwner(msg.sender, accountId, owner);

        uint256 free = _free[accountId];
        if (amount > free) revert InsufficientFree(accountId, amount, free);
        _free[accountId] = free - amount;

        if (!usdc.transfer(to, amount)) revert TransferFailed();
        emit Withdrawn(accountId, to, amount);
    }

    /// @inheritdoc IUSDCVault
    function freeBalanceOf(uint256 accountId) external view override returns (uint256) {
        return _free[accountId];
    }

    /// @inheritdoc IUSDCVault
    function lockedBalanceOf(uint256 accountId) external view override returns (uint256) {
        return _locked[accountId];
    }

    /// @inheritdoc IUSDCVault
    function totalBalanceOf(uint256 accountId) external view override returns (uint256) {
        return _free[accountId] + _locked[accountId];
    }

    /// @inheritdoc IUSDCVault
    function lockMargin(uint256 accountId, uint256 amount) external override onlySettlement {
        if (amount == 0) revert ZeroAmount();
        uint256 free = _free[accountId];
        if (amount > free) revert InsufficientFree(accountId, amount, free);
        unchecked {
            _free[accountId] = free - amount;
            _locked[accountId] += amount;
        }
        emit MarginLocked(accountId, amount);
    }

    /// @inheritdoc IUSDCVault
    function releaseMargin(uint256 accountId, uint256 amount) external override onlySettlement {
        if (amount == 0) revert ZeroAmount();
        uint256 locked = _locked[accountId];
        if (amount > locked) revert InsufficientLocked(accountId, amount, locked);
        unchecked {
            _locked[accountId] = locked - amount;
            _free[accountId] += amount;
        }
        emit MarginReleased(accountId, amount);
    }

    /// @inheritdoc IUSDCVault
    /// @dev Negative PnL is absorbed by free balance first, then by locked
    ///      balance (the underwater path - if locked is also exhausted the
    ///      account is bad-debt and a future SettlementEngineV2 will route
    ///      the residual to an insurance fund per ADR 0005, v0.6 TBD).
    function applyPnL(uint256 accountId, int256 pnl) external override onlySettlement {
        if (pnl > 0) {
            _free[accountId] += uint256(pnl);
        } else if (pnl < 0) {
            uint256 loss = uint256(-pnl);
            uint256 free = _free[accountId];
            if (loss <= free) {
                unchecked {
                    _free[accountId] = free - loss;
                }
            } else {
                uint256 remainder = loss - free;
                _free[accountId] = 0;
                uint256 locked = _locked[accountId];
                _locked[accountId] = remainder >= locked ? 0 : locked - remainder;
            }
        }
        emit PnLApplied(accountId, pnl);
    }

    modifier onlySettlement() {
        if (settlementEngine == address(0)) revert SettlementEngineNotBound();
        if (msg.sender != settlementEngine) revert OnlySettlementEngine(msg.sender);
        _;
    }
}
