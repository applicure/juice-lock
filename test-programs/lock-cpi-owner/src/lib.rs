//! Test-only owner program: its PDA ["owner"] (a plain system account, like
//! a Squads vault) owns juice_lock locks and signs `lock`, `extend` and
//! `unlock` through `invoke_signed`, using juice_lock's `cpi` crate. Every
//! juice_lock account is passed through unchecked: juice_lock does all the
//! checking. Never deployed; the id is a placeholder (sha256 of "juice_lock
//! test cpi owner program id").
use anchor_lang::prelude::*;
use juice_lock::cpi::accounts as jl;

declare_id!("DHuNSqgQeoBt6P2ZdFLL5ipWP2hzCCqd5D6fVof3UmLQ");

pub const OWNER_SEED: &[u8] = b"owner";

#[program]
pub mod lock_cpi_owner {
    use super::*;

    pub fn lock<'info>(
        ctx: Context<'info, Forward<'info>>,
        tier: u8,
        amount: u64,
        nonce: u64,
        max_fee: u64,
    ) -> Result<()> {
        let r = ctx.remaining_accounts;
        require!(r.len() == 10, OwnerError::Accounts);
        let seeds: &[&[u8]] = &[OWNER_SEED, &[ctx.bumps.owner]];
        juice_lock::cpi::lock(
            CpiContext::new_with_signer(
                juice_lock::ID,
                jl::Lock {
                    owner: ctx.accounts.owner.to_account_info(),
                    config: r[0].clone(),
                    treasury: r[1].clone(),
                    pump_pool: r[2].clone(),
                    lp_mint: r[3].clone(),
                    owner_token: r[4].clone(),
                    lock_record: r[5].clone(),
                    vault: r[6].clone(),
                    token_program: r[7].clone(),
                    system_program: r[8].clone(),
                },
                &[seeds],
            ),
            tier,
            amount,
            nonce,
            max_fee,
        )
    }

    pub fn extend<'info>(ctx: Context<'info, Forward<'info>>, new_tier: u8) -> Result<()> {
        let r = ctx.remaining_accounts;
        require!(r.len() == 2, OwnerError::Accounts);
        let seeds: &[&[u8]] = &[OWNER_SEED, &[ctx.bumps.owner]];
        juice_lock::cpi::extend(
            CpiContext::new_with_signer(
                juice_lock::ID,
                jl::Extend {
                    owner: ctx.accounts.owner.to_account_info(),
                    lock_record: r[0].clone(),
                },
                &[seeds],
            ),
            new_tier,
        )
    }

    pub fn unlock<'info>(ctx: Context<'info, Forward<'info>>) -> Result<()> {
        let r = ctx.remaining_accounts;
        require!(r.len() == 6, OwnerError::Accounts);
        let seeds: &[&[u8]] = &[OWNER_SEED, &[ctx.bumps.owner]];
        juice_lock::cpi::unlock(CpiContext::new_with_signer(
            juice_lock::ID,
            jl::Unlock {
                owner: ctx.accounts.owner.to_account_info(),
                lock_record: r[0].clone(),
                vault: r[1].clone(),
                lp_mint: r[2].clone(),
                destination: r[3].clone(),
                token_program: r[4].clone(),
            },
            &[seeds],
        ))
    }
}

/// The user (pays the transaction), the owner PDA; juice_lock's own accounts
/// (without its owner) follow as remaining accounts in its order, the last
/// one being the juice_lock program.
#[derive(Accounts)]
pub struct Forward<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    /// CHECK: the PDA that owns the lock; signs through invoke_signed.
    #[account(mut, seeds = [OWNER_SEED], bump)]
    pub owner: UncheckedAccount<'info>,
}

#[error_code]
pub enum OwnerError {
    #[msg("Wrong number of accounts")]
    Accounts,
}
