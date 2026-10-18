//! The config. The admin sets the treasury, the fee (within a hard bound)
//! and the pause of new locks, and hands the role on with the new admin's
//! signature. Nothing here names a lock, a vault or a token account.
use anchor_lang::prelude::*;

use crate::error::LockError;
use crate::events::*;
use crate::math;
use crate::state::*;
use crate::{InitConfig, SetAdmin, SetConfig};

/// 0, or within `[rent minimum of an empty account, MAX_FEE_LAMPORTS]`.
fn fee_ok(fee: u64) -> Result<bool> {
    Ok(math::fee_ok(
        fee,
        Rent::get()?.minimum_balance(0),
        MAX_FEE_LAMPORTS,
    ))
}

pub fn init_config(
    ctx: Context<InitConfig>,
    admin: Pubkey,
    treasury: Pubkey,
    fee_lamports: u64,
) -> Result<()> {
    require!(
        admin != Pubkey::default() && treasury != Pubkey::default() && fee_ok(fee_lamports)?,
        LockError::InvalidConfigValue
    );
    let c = &mut ctx.accounts.config;
    c.version = VERSION;
    c.bump = ctx.bumps.config;
    c.paused_new = false;
    c._pad = [0; 5];
    c.admin = admin;
    c.treasury = treasury;
    c.fee_lamports = fee_lamports;
    emit!(ConfigInitialized {
        admin,
        treasury,
        fee_lamports,
    });
    Ok(())
}

pub fn set_config(
    ctx: Context<SetConfig>,
    treasury: Pubkey,
    fee_lamports: u64,
    paused_new: bool,
) -> Result<()> {
    require!(
        treasury != Pubkey::default() && fee_ok(fee_lamports)?,
        LockError::InvalidConfigValue
    );
    let c = &mut ctx.accounts.config;
    let ev = ConfigChanged {
        old_treasury: c.treasury,
        treasury,
        old_fee_lamports: c.fee_lamports,
        fee_lamports,
        old_paused_new: c.paused_new,
        paused_new,
    };
    c.treasury = treasury;
    c.fee_lamports = fee_lamports;
    c.paused_new = paused_new;
    emit!(ev);
    Ok(())
}

pub fn set_admin(ctx: Context<SetAdmin>) -> Result<()> {
    let c = &mut ctx.accounts.config;
    let old = c.admin;
    c.admin = ctx.accounts.new_admin.key();
    emit!(AdminChanged { old, new: c.admin });
    Ok(())
}
