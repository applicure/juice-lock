//! Invariant 11, PDA owners: the test program `lock-cpi-owner` locks,
//! extends and unlocks with its PDA (a plain system account, as a Squads
//! vault) as the owner, through juice_lock's `cpi` crate. The PDA pays the
//! fee and the rents and gets the rents back, exactly as a wallet.
#![allow(clippy::result_large_err)]
mod common;
use anchor_lang::solana_program::instruction::{AccountMeta, Instruction};
use common::*;

/// Anchor's instruction discriminator.
fn disc(name: &str) -> Vec<u8> {
    use sha2::{Digest, Sha256};
    Sha256::digest(format!("global:{name}").as_bytes())[..8].to_vec()
}

/// The CPI owner program's instruction: user, owner PDA, then juice_lock's
/// own accounts without the owner, then the juice_lock program.
fn forward(user: &Pubkey, name: &str, arg: Vec<u8>, inner: Vec<AccountMeta>) -> Instruction {
    let mut accounts = vec![
        AccountMeta::new(*user, true),
        AccountMeta::new(cpi_owner_pda(), false),
    ];
    accounts.extend(inner.into_iter().skip(1));
    accounts.push(AccountMeta::new_readonly(LOCK, false));
    let mut data = disc(name);
    data.extend(arg);
    Instruction {
        program_id: CPI_OWNER,
        accounts,
        data,
    }
}

#[test]
fn inv11_a_pda_owner_locks_extends_and_unlocks_through_cpi() {
    let mut env = Env::new();
    env.load_cpi_owner();
    let p = env.troll;
    let user = env.wallet(SOL);
    let o = cpi_owner_pda();
    env.svm.airdrop(&o, SOL).unwrap();
    let o_lp = env.token_account(&o, &p.lp_mint, 2 * LP);

    // Lock: the PDA owns the lock and pays.
    let t0 = env.lamports(&env.treasury.pubkey());
    let o0 = env.lamports(&o);
    let a = env.lock_accounts(&o, &o_lp, &p, 77);
    let (record, vault) = (a.lock_record, a.vault);
    let mut arg = vec![0u8];
    arg.extend((2 * LP).to_le_bytes());
    arg.extend(77u64.to_le_bytes());
    arg.extend(FEE.to_le_bytes()); // max_fee
    let ix = forward(&user.pubkey(), "lock", arg, a.to_account_metas(None));
    expect_ok(&env.send(&[ix], &user, &[]));
    let l = L {
        record,
        vault,
        owner: o,
        pool: p,
        nonce: 77,
    };
    let rec = env.record(&l);
    assert_eq!(rec.owner, o);
    assert_eq!(rec.amount, 2 * LP);
    assert_eq!(env.lamports(&env.treasury.pubkey()) - t0, FEE);
    let rents = env.lamports(&record) + env.lamports(&vault);
    assert_eq!(o0 - env.lamports(&o), FEE + rents);

    // Nobody else can act for the PDA: a wallet naming it as owner can't sign.
    let mut ix = env.extend_ix(&o, &record, 1);
    ix.accounts[0].is_signer = false;
    expect_fail(&env.send(&[ix], &user, &[]));

    // Extend through CPI.
    let ix = forward(&user.pubkey(), "extend", vec![1], env.extend_ix(&o, &record, 1).accounts);
    expect_ok(&env.send(&[ix], &user, &[]));
    assert_eq!(env.record(&l).tier, 1);

    // Unlock through CPI after the time, to the PDA's own token account.
    let unlock_metas = env.unlock_accounts(&o, &l, &o_lp).to_account_metas(None);
    let ix = forward(&user.pubkey(), "unlock", vec![], unlock_metas.clone());
    expect_err(&env.send(&[ix], &user, &[]), "StillLocked");
    env.warp(DURATIONS[1]);
    let o1 = env.lamports(&o);
    let ix = forward(&user.pubkey(), "unlock", vec![], unlock_metas);
    expect_ok(&env.send(&[ix], &user, &[]));
    assert_eq!(env.token_amount(&o_lp), 2 * LP);
    assert!(!env.exists(&record) && !env.exists(&vault));
    assert_eq!(env.lamports(&o) - o1, rents);
}
