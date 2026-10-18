//! Accounts, seeds and constants of `juice_lock` (docs/plans/lp-lock.md
//! section 1.1). Byte offsets are fixed by `tests/layout.rs`. The program is
//! made immutable after its audits, so there is no `reserved` space and no
//! layout version but 1: a later program gets a new id.
use anchor_lang::prelude::*;

pub const CONFIG_SEED: &[u8] = b"lock_config";
pub const LOCK_SEED: &[u8] = b"lock";
pub const VAULT_SEED: &[u8] = b"vault";

/// Layout version, the first byte after the discriminator of every account.
pub const VERSION: u8 = 1;

/// Tiers 0..=4 have a duration; tier 5 is Forever.
pub const FOREVER_TIER: u8 = 5;
pub const NUM_TIERS: u8 = 6;

/// Seconds of tier 0, "1 week": 7 days.
#[cfg(not(feature = "mainnet-test"))]
pub const TIER0_SECS: i64 = 604_800;
/// MAINNET-TEST build only (`--features mainnet-test`, the 1-day test on
/// mainnet before the upgrade to the final build): "1 week" lasts 1 day. This
/// is the one thing the feature changes (tests/layout.rs).
#[cfg(feature = "mainnet-test")]
pub const TIER0_SECS: i64 = 86_400;

/// Seconds per tier: 7, 30, 90, 180 and 365 days.
#[cfg(not(feature = "devnet"))]
pub const DURATIONS: [i64; 5] = [TIER0_SECS, 2_592_000, 7_776_000, 15_552_000, 31_536_000];
/// Devnet soak build: every duration divided by 1,440 (7 minutes for "1 week").
#[cfg(feature = "devnet")]
pub const DURATIONS: [i64; 5] = [420, 1_800, 5_400, 10_800, 21_900];

/// Hard bound on the lock fee: 0.05 SOL. `set_config` refuses more.
pub const MAX_FEE_LAMPORTS: u64 = 50_000_000;

/// `LockRecord.token_program` values: the LP mint's owner at lock time.
pub const TOKEN_PROGRAM_SPL: u8 = 0;
pub const TOKEN_PROGRAM_2022: u8 = 1;

/// Global settings, PDA ["lock_config"], created once by the upgrade
/// authority (before the revoke). Neither `extend` nor `unlock` takes this
/// account, so nothing here can stop them. Read-only in `lock`; there is no
/// on-chain lock counter (the indexer counts locks off-chain).
#[account]
#[derive(Debug, InitSpace)]
pub struct LockConfig {
    pub version: u8,
    pub bump: u8,
    /// Blocks `lock` only.
    pub paused_new: bool,
    pub _pad: [u8; 5],
    /// A stored key (the upgrade authority won't exist after the revoke).
    pub admin: Pubkey,
    /// Receives the fee in lamports.
    pub treasury: Pubkey,
    /// 0, or within `[rent minimum of an empty account, MAX_FEE_LAMPORTS]`.
    pub fee_lamports: u64,
}

/// One lock, PDA ["lock", owner, lp_mint, nonce (u64 LE)], rent paid by the
/// owner. Only `unlock` closes it, so a record that disappears was unlocked.
#[account]
#[derive(Debug, InitSpace)]
pub struct LockRecord {
    pub version: u8,
    pub bump: u8,
    pub vault_bump: u8,
    /// 0..=5: 1 w, 1 m, 3 m, 6 m, 1 y, Forever (of the last `lock` or `extend`).
    pub tier: u8,
    /// `tier == FOREVER_TIER`; then `unlock_ts == i64::MAX`.
    pub forever: bool,
    pub lp_decimals: u8,
    /// `TOKEN_PROGRAM_SPL` or `TOKEN_PROGRAM_2022`.
    pub token_program: u8,
    pub _pad: u8,
    /// Any signer, a PDA included (multisig vaults).
    pub owner: Pubkey,
    pub lp_mint: Pubkey,
    /// The PumpSwap pool the mint was proven against at `lock`.
    pub pump_pool: Pubkey,
    /// PDA ["vault", this record]: a token account of `lp_mint`, authority this record.
    pub vault: Pubkey,
    /// What the vault received at `lock` (measured).
    pub amount: u64,
    /// `lock`'s clock time; never changes.
    pub start_ts: i64,
    /// `start_ts + DURATIONS[tier]`, raised by `extend`; `i64::MAX` for Forever.
    pub unlock_ts: i64,
    pub nonce: u64,
    /// 0 until the first `extend`, then the time of the last one.
    pub extended_at: i64,
    /// Fee charged at `lock`.
    pub fee_paid: u64,
}

impl LockRecord {
    pub fn token_program_id(&self) -> Pubkey {
        if self.token_program == TOKEN_PROGRAM_2022 {
            crate::pump::TOKEN_2022_ID
        } else {
            crate::pump::TOKEN_ID
        }
    }
}
