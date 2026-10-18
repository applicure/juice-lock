use anchor_lang::prelude::*;

/// Error codes: 6000 + position, fixed by `tests/layout.rs`. The program is
/// immutable after the revoke, so these never change.
#[error_code]
pub enum LockError {
    #[msg("Only the program's upgrade authority can do that")]
    NotUpgradeAuthority, // 6000
    #[msg("Only the lock admin can do that")]
    NotAdmin,
    #[msg("New locks are paused; extend and unlock always work")]
    Paused,
    #[msg("Unsupported account layout version")]
    BadVersion,
    #[msg("Config value outside its bounds")]
    InvalidConfigValue,
    #[msg("The mint has a freeze authority or an extension this program refuses")]
    MintNotAllowed, // 6005
    #[msg("Not a PumpSwap pool, or the LP mint is not that pool's")]
    PumpPoolMismatch,
    #[msg("Wrong token program for this mint")]
    WrongTokenProgram,
    #[msg("The token account belongs to someone else")]
    WrongTokenOwner,
    #[msg("Wrong mint for this lock")]
    WrongMint,
    #[msg("Not the treasury fixed in the config")]
    WrongTreasury, // 6010
    #[msg("Amount must be positive")]
    ZeroAmount,
    #[msg("Tier out of range")]
    InvalidTier,
    #[msg("Only the lock's owner can do that")]
    NotOwner,
    #[msg("The lock has not reached its unlock time")]
    StillLocked,
    #[msg("A Forever lock can never be unlocked or changed")]
    ForeverLock, // 6015
    #[msg("The new unlock time must be later than the current one")]
    NotLater,
    #[msg("Wrong vault for this lock")]
    WrongVault,
    #[msg("Arithmetic overflow")]
    MathOverflow,
    #[msg("The lock fee is higher than the maximum the owner accepted")]
    FeeTooHigh, // 6019
}
