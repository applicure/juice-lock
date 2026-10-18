//! Juice LP locks. A user locks PumpSwap LP tokens for a fixed time (7, 30,
//! 90, 180 or 365 days, or Forever); nobody can take them out early: not the
//! owner, not Juice, not an admin. After the unlock time only the owner can
//! withdraw them, to a token account it owns. Plan: `docs/plans/lp-lock.md`;
//! the program as built: `docs/lock-program.md`. Handlers live in
//! `handlers/`, the arithmetic in `math.rs`; this file holds the instruction
//! entry points and their account lists.
//!
//! Custody: each lock has one vault, a token account at the PDA ["vault",
//! record] whose authority is the record PDA. Only `unlock` signs for the
//! record: it pays the vault's whole balance to a token account the record's
//! owner owns, then closes the vault and the record. `extend` and `unlock`
//! don't take the config, the PumpSwap pool or any account an admin names.
//! There is no admin instruction over locks, vaults or token accounts. The
//! upgrade authority is revoked after the audits.

use anchor_lang::prelude::*;
use anchor_spl::token_interface::{TokenAccount, TokenInterface};

pub mod error;
pub mod events;
mod handlers;
pub mod math;
pub mod pump;
pub mod state;

use error::LockError;
use state::*;

// Program id per cluster, chosen at build time by the `devnet` feature.
// `--features devnet`: .secrets/devnet/lock-program.json. The default build,
// and the `mainnet-test` build (state.rs), carry the mainnet id:
// .secrets/mainnet/lock-program.json.
#[cfg(feature = "devnet")]
declare_id!("A6uz54MqeiS3VW6GTpKMUswyiTPAzaNpfnRZvANLr9sU");
#[cfg(not(feature = "devnet"))]
declare_id!("544vBp5c5GHJwNDm4YmS4FK24to9YP17iJLSXmcjFtmG");

// `mainnet-test` shortens only the mainnet "1 week" tier (state.rs); the
// devnet build has its own soak durations. The two never combine.
#[cfg(all(feature = "devnet", feature = "mainnet-test"))]
compile_error!("the features `devnet` and `mainnet-test` exclude each other");

// Embedded security contact (solana-security-txt). It becomes permanent with
// the revoke. No `source_code`: the public repository for this program's
// source doesn't exist yet (docs/lock-program.md section 13).
#[cfg(not(feature = "no-entrypoint"))]
solana_security_txt::security_txt! {
    name: "Juice LP locks",
    project_url: "https://juicepools.app",
    contacts: "email:security@juicepools.app",
    policy: "https://juicepools.app/security",
    preferred_languages: "en,de",
    auditors: "Two independent AI-agent security audits (Claude agents, Oct 2026); no external audit firm"
}

#[program]
pub mod juice_lock {
    use super::*;

    // ------------------------------------------------------------ admin

    /// Once, before the revoke: the upgrade authority creates the config.
    pub fn init_config(
        ctx: Context<InitConfig>,
        admin: Pubkey,
        treasury: Pubkey,
        fee_lamports: u64,
    ) -> Result<()> {
        handlers::admin::init_config(ctx, admin, treasury, fee_lamports)
    }

    /// Admin: the treasury, the fee (0 or within `[rent minimum, MAX_FEE_LAMPORTS]`)
    /// and the pause of new locks. Touches nothing but the config.
    pub fn set_config(
        ctx: Context<SetConfig>,
        treasury: Pubkey,
        fee_lamports: u64,
        paused_new: bool,
    ) -> Result<()> {
        handlers::admin::set_config(ctx, treasury, fee_lamports, paused_new)
    }

    /// Admin, with the new admin's signature.
    pub fn set_admin(ctx: Context<SetAdmin>) -> Result<()> {
        handlers::admin::set_admin(ctx)
    }

    // ------------------------------------------------------------ locks

    /// The owner locks `amount` LP of a PumpSwap pool for `tier`
    /// (0..=4: 7, 30, 90, 180, 365 days; 5: Forever), paying the fee to the
    /// treasury. `nonce` (chosen by the client) tells the owner's locks of one
    /// mint apart. `max_fee`: the most the owner accepts to pay (the fee it
    /// was shown), so a fee raised before the transaction lands is refused.
    pub fn lock(ctx: Context<Lock>, tier: u8, amount: u64, nonce: u64, max_fee: u64) -> Result<()> {
        handlers::lock::lock(ctx, tier, amount, nonce, max_fee)
    }

    /// The owner lengthens a lock: the new unlock time (`now + DURATIONS[new_tier]`,
    /// or Forever) must be later than the current one. Also after expiry.
    pub fn extend(ctx: Context<Extend>, new_tier: u8) -> Result<()> {
        handlers::lock::extend(ctx, new_tier)
    }

    /// The owner, from the unlock time on: the vault's whole balance to a
    /// token account the owner owns; vault and record closed, rent to the owner.
    pub fn unlock(ctx: Context<Unlock>) -> Result<()> {
        handlers::lock::unlock(ctx)
    }
}

// ---------------------------------------------------------------- account lists

/// The signer must be the upgrade authority in this program's ProgramData
/// (as juice_boost's `init_config`). After the revoke nobody is.
#[derive(Accounts)]
pub struct InitConfig<'info> {
    pub authority: Signer<'info>,
    #[account(constraint = program.programdata_address()? == Some(program_data.key()) @ LockError::NotUpgradeAuthority)]
    pub program: Program<'info, crate::program::JuiceLock>,
    #[account(constraint = program_data.upgrade_authority_address == Some(authority.key()) @ LockError::NotUpgradeAuthority)]
    pub program_data: Account<'info, ProgramData>,
    #[account(mut)]
    pub payer: Signer<'info>,
    #[account(init, payer = payer, space = 8 + LockConfig::INIT_SPACE, seeds = [CONFIG_SEED], bump)]
    pub config: Account<'info, LockConfig>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct SetConfig<'info> {
    #[account(address = config.admin @ LockError::NotAdmin)]
    pub admin: Signer<'info>,
    #[account(mut, seeds = [CONFIG_SEED], bump = config.bump, constraint = config.version == VERSION @ LockError::BadVersion)]
    pub config: Account<'info, LockConfig>,
}

/// Both sign, so a typo can't orphan the config (as boost's `set_guardian`).
#[derive(Accounts)]
pub struct SetAdmin<'info> {
    #[account(address = config.admin @ LockError::NotAdmin)]
    pub admin: Signer<'info>,
    #[account(mut, seeds = [CONFIG_SEED], bump = config.bump, constraint = config.version == VERSION @ LockError::BadVersion)]
    pub config: Account<'info, LockConfig>,
    pub new_admin: Signer<'info>,
}

#[derive(Accounts)]
#[instruction(tier: u8, amount: u64, nonce: u64, max_fee: u64)]
pub struct Lock<'info> {
    /// Pays the fee and both rents. Any signer, a PDA included.
    #[account(mut)]
    pub owner: Signer<'info>,
    /// Read only: `lock` writes nothing to the config.
    #[account(seeds = [CONFIG_SEED], bump = config.bump, constraint = config.version == VERSION @ LockError::BadVersion)]
    pub config: Account<'info, LockConfig>,
    /// CHECK: receives the fee in lamports; must be the config's treasury.
    #[account(mut, address = config.treasury @ LockError::WrongTreasury)]
    pub treasury: UncheckedAccount<'info>,
    /// CHECK: a PumpSwap pool, checked by `pump::check_pool` (owner,
    /// discriminator, LP mint PDA, mint authority).
    pub pump_pool: UncheckedAccount<'info>,
    /// CHECK: checked by `pump::check_mint` and `pump::check_pool`; its owner
    /// is the token program (constraint on `token_program`).
    pub lp_mint: UncheckedAccount<'info>,
    #[account(mut,
        constraint = owner_token.mint == lp_mint.key() @ LockError::WrongMint,
        constraint = owner_token.owner == owner.key() @ LockError::WrongTokenOwner)]
    pub owner_token: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(init, payer = owner, space = 8 + LockRecord::INIT_SPACE,
        seeds = [LOCK_SEED, owner.key().as_ref(), lp_mint.key().as_ref(), &nonce.to_le_bytes()], bump)]
    pub lock_record: Box<Account<'info, LockRecord>>,
    /// CHECK: created by the handler at this PDA as a token account of
    /// `lp_mint` under `token_program`, authority `lock_record`. Never an ATA.
    #[account(mut, seeds = [VAULT_SEED, lock_record.key().as_ref()], bump)]
    pub vault: UncheckedAccount<'info>,
    #[account(constraint = token_program.key() == *lp_mint.owner @ LockError::WrongTokenProgram)]
    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

/// Reads nothing but the record: no config, no pool, no fee.
#[derive(Accounts)]
pub struct Extend<'info> {
    pub owner: Signer<'info>,
    #[account(mut, has_one = owner @ LockError::NotOwner,
        seeds = [LOCK_SEED, lock_record.owner.as_ref(), lock_record.lp_mint.as_ref(), &lock_record.nonce.to_le_bytes()],
        bump = lock_record.bump,
        constraint = lock_record.version == VERSION @ LockError::BadVersion)]
    pub lock_record: Box<Account<'info, LockRecord>>,
}

/// No config, no pool: nothing but the record, its vault and mint, and the
/// owner's destination.
#[derive(Accounts)]
pub struct Unlock<'info> {
    /// Receives both rents.
    #[account(mut)]
    pub owner: Signer<'info>,
    #[account(mut, close = owner,
        has_one = owner @ LockError::NotOwner,
        has_one = vault @ LockError::WrongVault,
        has_one = lp_mint @ LockError::WrongMint,
        seeds = [LOCK_SEED, lock_record.owner.as_ref(), lock_record.lp_mint.as_ref(), &lock_record.nonce.to_le_bytes()],
        bump = lock_record.bump,
        constraint = lock_record.version == VERSION @ LockError::BadVersion)]
    pub lock_record: Box<Account<'info, LockRecord>>,
    /// CHECK: the record's vault (`has_one`).
    #[account(mut)]
    pub vault: UncheckedAccount<'info>,
    /// CHECK: the record's mint (`has_one`); `transfer_checked` checks it too.
    pub lp_mint: UncheckedAccount<'info>,
    /// A token account of the lock's mint that the lock's owner owns.
    #[account(mut,
        constraint = destination.mint == lock_record.lp_mint @ LockError::WrongMint,
        constraint = destination.owner == lock_record.owner @ LockError::WrongTokenOwner)]
    pub destination: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(constraint = token_program.key() == lock_record.token_program_id() @ LockError::WrongTokenProgram)]
    pub token_program: Interface<'info, TokenInterface>,
}
