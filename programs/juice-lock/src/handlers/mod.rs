pub mod admin;
pub mod lock;

use anchor_lang::prelude::*;

use crate::error::LockError;
use crate::pump::{TOKEN_2022_ID, TOKEN_ID};

pub fn ovf() -> Error {
    error!(LockError::MathOverflow)
}

/// The balance of a token account (SPL Token or Token-2022): the u64 at
/// byte 64. The caller has checked the account is the vault it expects.
pub fn token_amount(account: &AccountInfo) -> Result<u64> {
    require!(
        *account.owner == TOKEN_ID || *account.owner == TOKEN_2022_ID,
        LockError::WrongVault
    );
    let d = account.try_borrow_data()?;
    require!(d.len() >= 165, LockError::WrongVault);
    Ok(u64::from_le_bytes(d[64..72].try_into().unwrap()))
}

/// Creates the account at a PDA of this program (`seeds`, with bump) with
/// `space` bytes, owned by `owner`, rent paid by `payer`. Like Anchor's
/// `init`, it also works when someone sent lamports to the address first.
/// (juice_boost's `create_pda`, with this program's error type.)
pub fn create_pda<'info>(
    payer: &AccountInfo<'info>,
    account: &AccountInfo<'info>,
    space: usize,
    owner: &Pubkey,
    system_program: &AccountInfo<'info>,
    seeds: &[&[u8]],
) -> Result<()> {
    use anchor_lang::system_program as sys;
    let space_u64 = space as u64;
    let rent = Rent::get()?.minimum_balance(space);
    let have = account.lamports();
    if have == 0 {
        sys::create_account(
            CpiContext::new_with_signer(
                system_program.key(),
                sys::CreateAccount {
                    from: payer.clone(),
                    to: account.clone(),
                },
                &[seeds],
            ),
            rent,
            space_u64,
            owner,
        )?;
    } else {
        // Pre-funded address: it can only be a plain system account (only
        // this program can sign for the PDA). Top up, allocate, assign.
        require!(
            *account.owner == sys::ID && account.data_is_empty(),
            LockError::WrongVault
        );
        if have < rent {
            sys::transfer(
                CpiContext::new(
                    system_program.key(),
                    sys::Transfer {
                        from: payer.clone(),
                        to: account.clone(),
                    },
                ),
                rent - have,
            )?;
        }
        sys::allocate(
            CpiContext::new_with_signer(
                system_program.key(),
                sys::Allocate {
                    account_to_allocate: account.clone(),
                },
                &[seeds],
            ),
            space_u64,
        )?;
        sys::assign(
            CpiContext::new_with_signer(
                system_program.key(),
                sys::Assign {
                    account_to_assign: account.clone(),
                },
                &[seeds],
            ),
            owner,
        )?;
    }
    Ok(())
}
