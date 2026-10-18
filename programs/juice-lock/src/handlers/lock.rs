//! The owner's instructions. `unlock` is the only code that signs for a
//! record, i.e. the only code that can move tokens out of a vault or close
//! it; it pays only a token account the record's owner owns. `lock` moves
//! tokens into a fresh vault; `extend` moves nothing.
use anchor_lang::prelude::*;
use anchor_lang::system_program;
use anchor_spl::token_interface::{self, CloseAccount, TransferChecked};

use super::{create_pda, ovf, token_amount};
use crate::error::LockError;
use crate::events::*;
use crate::math;
use crate::pump::{check_mint, check_pool, TOKEN_2022_ID};
use crate::state::*;
use crate::{Extend, Lock, Unlock};

pub fn lock(ctx: Context<Lock>, tier: u8, amount: u64, nonce: u64, max_fee: u64) -> Result<()> {
    let a = ctx.accounts;
    require!(!a.config.paused_new, LockError::Paused);
    // The fee is charged as the config says at execution; the owner's bound
    // keeps a fee raised in between (by the admin or a stolen admin key) out.
    let fee = a.config.fee_lamports;
    require!(fee <= max_fee, LockError::FeeTooHigh);
    require!(tier < NUM_TIERS, LockError::InvalidTier);
    require!(amount > 0, LockError::ZeroAmount);
    let mint = a.lp_mint.to_account_info();
    check_mint(&mint)?;
    check_pool(&a.pump_pool.to_account_info(), &mint)?;
    let decimals = mint.try_borrow_data()?[44];
    let now = Clock::get()?.unix_timestamp;
    let unlock_ts = math::unlock_ts(now, tier, &DURATIONS, FOREVER_TIER).ok_or_else(ovf)?;

    // The fee, by system transfer to the config's treasury.
    if fee > 0 {
        system_program::transfer(
            CpiContext::new(
                a.system_program.key(),
                system_program::Transfer {
                    from: a.owner.to_account_info(),
                    to: a.treasury.to_account_info(),
                },
            ),
            fee,
        )?;
    }

    // The vault: sized by the token program, created at its PDA, authority the record.
    let token_program = a.token_program.to_account_info();
    let record_key = a.lock_record.key();
    let vault_bump = ctx.bumps.vault;
    let vault_seeds: &[&[u8]] = &[VAULT_SEED, record_key.as_ref(), &[vault_bump]];
    let size = token_interface::get_account_data_size(
        CpiContext::new(
            token_program.key(),
            token_interface::GetAccountDataSize { mint: mint.clone() },
        ),
        &[],
    )?;
    let size = usize::try_from(size).map_err(|_| ovf())?;
    let vault = a.vault.to_account_info();
    create_pda(
        &a.owner.to_account_info(),
        &vault,
        size,
        token_program.key,
        &a.system_program.to_account_info(),
        vault_seeds,
    )?;
    token_interface::initialize_account3(CpiContext::new(
        token_program.key(),
        token_interface::InitializeAccount3 {
            account: vault.clone(),
            mint: mint.clone(),
            authority: a.lock_record.to_account_info(),
        },
    ))?;

    // The deposit; the record keeps what the vault measurably received.
    let before = token_amount(&vault)?;
    token_interface::transfer_checked(
        CpiContext::new(
            token_program.key(),
            TransferChecked {
                from: a.owner_token.to_account_info(),
                mint: mint.clone(),
                to: vault.clone(),
                authority: a.owner.to_account_info(),
            },
        ),
        amount,
        decimals,
    )?;
    let got = math::received(before, token_amount(&vault)?).ok_or_else(ovf)?;
    require!(got > 0, LockError::ZeroAmount);

    let r = &mut a.lock_record;
    r.version = VERSION;
    r.bump = ctx.bumps.lock_record;
    r.vault_bump = vault_bump;
    r.tier = tier;
    r.forever = tier == FOREVER_TIER;
    r.lp_decimals = decimals;
    r.token_program = if *token_program.key == TOKEN_2022_ID {
        TOKEN_PROGRAM_2022
    } else {
        TOKEN_PROGRAM_SPL
    };
    r._pad = 0;
    r.owner = a.owner.key();
    r.lp_mint = mint.key();
    r.pump_pool = a.pump_pool.key();
    r.vault = vault.key();
    r.amount = got;
    r.start_ts = now;
    r.unlock_ts = unlock_ts;
    r.nonce = nonce;
    r.extended_at = 0;
    r.fee_paid = fee;
    emit!(Locked {
        lock: record_key,
        owner: r.owner,
        lp_mint: r.lp_mint,
        pump_pool: r.pump_pool,
        amount: got,
        tier,
        start_ts: now,
        unlock_ts,
        nonce,
        fee,
    });
    Ok(())
}

pub fn extend(ctx: Context<Extend>, new_tier: u8) -> Result<()> {
    let r = &mut ctx.accounts.lock_record;
    require!(!r.forever, LockError::ForeverLock);
    require!(new_tier < NUM_TIERS, LockError::InvalidTier);
    let now = Clock::get()?.unix_timestamp;
    let new_unlock = math::unlock_ts(now, new_tier, &DURATIONS, FOREVER_TIER).ok_or_else(ovf)?;
    require!(new_unlock > r.unlock_ts, LockError::NotLater);
    let ev = Extended {
        lock: r.key(),
        old_tier: r.tier,
        new_tier,
        old_unlock_ts: r.unlock_ts,
        new_unlock_ts: new_unlock,
    };
    r.tier = new_tier;
    r.forever = new_tier == FOREVER_TIER;
    r.unlock_ts = new_unlock;
    r.extended_at = now;
    emit!(ev);
    Ok(())
}

pub fn unlock(ctx: Context<Unlock>) -> Result<()> {
    let a = ctx.accounts;
    let r = &a.lock_record;
    require!(!r.forever, LockError::ForeverLock);
    let now = Clock::get()?.unix_timestamp;
    require!(now >= r.unlock_ts, LockError::StillLocked);

    let nonce = r.nonce.to_le_bytes();
    let seeds: &[&[u8]] = &[
        LOCK_SEED,
        r.owner.as_ref(),
        r.lp_mint.as_ref(),
        &nonce,
        &[r.bump],
    ];
    let token_program = a.token_program.key();
    let vault = a.vault.to_account_info();
    let record = a.lock_record.to_account_info();
    // The whole balance, so a donation to the vault never blocks the close.
    let balance = token_amount(&vault)?;
    if balance > 0 {
        token_interface::transfer_checked(
            CpiContext::new_with_signer(
                token_program,
                TransferChecked {
                    from: vault.clone(),
                    mint: a.lp_mint.to_account_info(),
                    to: a.destination.to_account_info(),
                    authority: record.clone(),
                },
                &[seeds],
            ),
            balance,
            r.lp_decimals,
        )?;
    }
    token_interface::close_account(CpiContext::new_with_signer(
        token_program,
        CloseAccount {
            account: vault,
            destination: a.owner.to_account_info(),
            authority: record,
        },
        &[seeds],
    ))?;
    emit!(Unlocked {
        lock: a.lock_record.key(),
        owner: r.owner,
        amount: balance,
        destination: a.destination.key(),
    });
    // The record is closed to the owner by the `close = owner` constraint.
    Ok(())
}
