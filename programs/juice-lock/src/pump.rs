//! The mint rule and the PumpSwap (pump_amm) facts `lock` relies on. The
//! program never calls PumpSwap, and only `lock` reads a pool.
//!
//! Origin: `check_mint` is `programs/juice-boost/src/pump.rs` (`check_mint`
//! and `check_mint_inner` with `freeze_allowed = false`) unchanged apart from
//! the error type. `check_pool` is new (plan section 1.4): it reads no field
//! offset of the pool, so a PumpSwap re-layout can't make it read a wrong key.
use anchor_lang::prelude::*;

use crate::error::LockError;

pub const PUMP_AMM_ID: Pubkey = pubkey!("pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA");
pub const TOKEN_2022_ID: Pubkey = pubkey!("TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb");
pub const TOKEN_ID: Pubkey = pubkey!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");

/// Anchor discriminator of PumpSwap's `Pool` (IDL, @pump-fun/pump-swap-sdk 1.20), as in juice_boost.
pub const POOL_DISCRIMINATOR: [u8; 8] = [241, 154, 109, 4, 17, 177, 109, 188];
pub const POOL_LP_MINT_SEED: &[u8] = b"pool_lp_mint";

/// Token-2022 mint extensions the check accepts: display-only metadata.
const EXT_METADATA_POINTER: u16 = 18;
const EXT_TOKEN_METADATA: u16 = 19;

/// Mint risk check for LP mints, fail closed (juice_boost's `check_mint`).
/// Refused: an owner other than the two token programs, a freeze authority,
/// and for Token-2022 every extension outside {MetadataPointer,
/// TokenMetadata}, including any extension this program doesn't know.
pub fn check_mint(mint: &AccountInfo) -> Result<()> {
    require!(
        *mint.owner == TOKEN_ID || *mint.owner == TOKEN_2022_ID,
        LockError::MintNotAllowed
    );
    let d = mint.try_borrow_data()?;
    require!(d.len() >= 82, LockError::MintNotAllowed);
    // Mint layout: mint_authority COption(36) supply(8) decimals(1)
    // is_initialized(1) freeze_authority COption(4 + 32) at 46.
    require!(d[45] == 1, LockError::MintNotAllowed);
    require!(d[46..50] == [0, 0, 0, 0], LockError::MintNotAllowed);
    if d.len() == 82 {
        return Ok(());
    }
    // Longer than a plain mint: only Token-2022 has that, as account type 1
    // (Mint) at byte 165 followed by type(2) length(2) value entries.
    require!(
        *mint.owner == TOKEN_2022_ID && d.len() >= 166 && d[165] == 1,
        LockError::MintNotAllowed
    );
    let mut at = 166;
    while at + 4 <= d.len() {
        let ty = u16::from_le_bytes([d[at], d[at + 1]]);
        let len = u16::from_le_bytes([d[at + 2], d[at + 3]]) as usize;
        require!(at + 4 + len <= d.len(), LockError::MintNotAllowed);
        match ty {
            0 => break, // uninitialized: end of extensions
            EXT_METADATA_POINTER | EXT_TOKEN_METADATA => {}
            _ => return err!(LockError::MintNotAllowed),
        }
        at += 4 + len;
    }
    Ok(())
}

/// Whether `pump_pool` is a PumpSwap pool and `lp_mint` its LP mint
/// (plan section 1.4), reading as little of PumpSwap as possible:
/// 1. the pool is owned by PumpSwap and starts with PumpSwap's `Pool` discriminator;
/// 2. `lp_mint` is PumpSwap's PDA ["pool_lp_mint", pool] (binds the mint to
///    the pool's address without reading a field offset);
/// 3. the mint's `mint_authority` is set and is the pool.
pub fn check_pool(pump_pool: &AccountInfo, lp_mint: &AccountInfo) -> Result<()> {
    require_keys_eq!(*pump_pool.owner, PUMP_AMM_ID, LockError::PumpPoolMismatch);
    {
        let d = pump_pool.try_borrow_data()?;
        require!(
            d.len() >= 8 && d[..8] == POOL_DISCRIMINATOR,
            LockError::PumpPoolMismatch
        );
    }
    let (pda, _) =
        Pubkey::find_program_address(&[POOL_LP_MINT_SEED, pump_pool.key.as_ref()], &PUMP_AMM_ID);
    require_keys_eq!(pda, *lp_mint.key, LockError::PumpPoolMismatch);
    let m = lp_mint.try_borrow_data()?;
    require!(
        m.len() >= 36 && m[0..4] == [1, 0, 0, 0] && m[4..36] == *pump_pool.key.as_ref(),
        LockError::PumpPoolMismatch
    );
    Ok(())
}
