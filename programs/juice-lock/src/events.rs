//! Events (docs/plans/lp-lock.md section 1.8), read from the logs by the
//! indexer; the hourly account read is the truth. Fixed with the program.
use anchor_lang::prelude::*;

#[event]
pub struct ConfigInitialized {
    pub admin: Pubkey,
    pub treasury: Pubkey,
    pub fee_lamports: u64,
}

/// `set_config`: the values before and after.
#[event]
pub struct ConfigChanged {
    pub old_treasury: Pubkey,
    pub treasury: Pubkey,
    pub old_fee_lamports: u64,
    pub fee_lamports: u64,
    pub old_paused_new: bool,
    pub paused_new: bool,
}

#[event]
pub struct AdminChanged {
    pub old: Pubkey,
    pub new: Pubkey,
}

#[event]
pub struct Locked {
    pub lock: Pubkey,
    pub owner: Pubkey,
    pub lp_mint: Pubkey,
    pub pump_pool: Pubkey,
    /// Measured: what the vault received.
    pub amount: u64,
    pub tier: u8,
    pub start_ts: i64,
    /// `i64::MAX` for Forever.
    pub unlock_ts: i64,
    pub nonce: u64,
    /// Lamports paid to the treasury.
    pub fee: u64,
}

#[event]
pub struct Extended {
    pub lock: Pubkey,
    pub old_tier: u8,
    pub new_tier: u8,
    pub old_unlock_ts: i64,
    pub new_unlock_ts: i64,
}

#[event]
pub struct Unlocked {
    pub lock: Pubkey,
    pub owner: Pubkey,
    /// The vault's whole balance, paid to `destination` (donations included).
    pub amount: u64,
    pub destination: Pubkey,
}
