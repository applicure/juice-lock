//! `unlock`: invariants 2 (who and when; never Forever), 3 (whole balance,
//! everything closed, rents back), 4 (independent of the config and the
//! pool) and 12 (destination rules).
#![allow(clippy::result_large_err)]
mod common;
use common::*;
use juice_lock::events::Unlocked;

#[test]
fn inv02_unlock_only_by_the_records_owner() {
    let mut env = Env::new();
    let p = env.troll;
    let (who, l) = env.new_lock(&p, 0, LP);
    env.warp(DURATIONS[0]);
    let thief = env.locker(&p, 0);
    // A stranger naming its own destination.
    expect_err(&env.unlock(&thief.kp, &l, &thief.lp), "NotOwner");
    // A stranger naming the owner's destination.
    expect_err(&env.unlock(&thief.kp, &l, &who.lp), "NotOwner");
    // The owner's key as a non-signer: the transaction can't be built
    // without its signature, so a stranger pays and signs alone.
    let mut ix = env.unlock_ix(env.unlock_accounts(&who.key(), &l, &who.lp));
    ix.accounts[0].is_signer = false;
    expect_fail(&env.send(&[ix], &thief.kp, &[]));
    assert!(env.exists(&l.record));
    expect_ok(&env.unlock(&who.kp, &l, &who.lp));
}

#[test]
fn inv02_unlock_only_from_the_unlock_time() {
    let mut env = Env::new();
    let p = env.troll;
    for tier in 0..5u8 {
        let (who, l) = env.new_lock(&p, tier, LP);
        let unlock_ts = env.record(&l).unlock_ts;
        expect_err(&env.unlock(&who.kp, &l, &who.lp), "StillLocked");
        env.set_time(unlock_ts - 1);
        expect_err(&env.unlock(&who.kp, &l, &who.lp), "StillLocked");
        env.set_time(unlock_ts);
        expect_ok(&env.unlock(&who.kp, &l, &who.lp));
        env.set_time(NOW);
    }
}

#[test]
fn inv02_a_forever_lock_never_unlocks() {
    let mut env = Env::new();
    let p = env.troll;
    let (who, l) = env.new_lock(&p, FOREVER_TIER, LP);
    for t in [NOW, NOW + 100 * 365 * 86_400, i64::MAX - 1, i64::MAX] {
        env.set_time(t);
        expect_err(&env.unlock(&who.kp, &l, &who.lp), "ForeverLock");
    }
    // A Forever lock reached by extend is just as final.
    env.set_time(NOW);
    let (who2, l2) = env.new_lock(&p, 0, LP);
    expect_ok(&env.extend(&who2.kp, &l2, FOREVER_TIER));
    env.set_time(i64::MAX - 1);
    expect_err(&env.unlock(&who2.kp, &l2, &who2.lp), "ForeverLock");
    assert_eq!(env.token_amount(&l.vault), LP);
    assert_eq!(env.token_amount(&l2.vault), LP);
}

#[test]
fn inv03_unlock_pays_the_whole_balance_and_closes_everything() {
    let mut env = Env::new();
    let p = env.troll;
    let (who, l) = env.new_lock(&p, 0, 3 * LP);
    // Donations: tokens and lamports sent to the vault, lamports to the record.
    env.donate(&l, 2 * LP + 17);
    env.svm.airdrop(&l.vault, 1_000_000).unwrap();
    env.svm.airdrop(&l.record, 333).unwrap();
    env.warp(DURATIONS[0]);
    let rents = env.lamports(&l.record) + env.lamports(&l.vault);
    let w0 = env.lamports(&who.key());
    let r = env.unlock(&who.kp, &l, &who.lp);
    expect_ok(&r);
    assert_eq!(env.token_amount(&who.lp), 5 * LP + 17);
    // Both accounts are gone; every lamport they held went to the owner.
    assert!(!env.exists(&l.record));
    assert!(!env.exists(&l.vault));
    assert_eq!(env.lamports(&who.key()), w0 + rents - 5_000);
    let ev = events::<Unlocked>(&r);
    assert_eq!(ev.len(), 1);
    assert_eq!(
        (ev[0].lock, ev[0].owner, ev[0].amount, ev[0].destination),
        (l.record, who.key(), 5 * LP + 17, who.lp)
    );
    // Nothing of the lock remains: a second unlock and an extend fail.
    expect_fail(&env.unlock(&who.kp, &l, &who.lp));
    expect_fail(&env.extend(&who.kp, &l, 4));
    // The same nonce can be used again for a new lock.
    let more = env.token_account(&who.key(), &p.lp_mint, LP);
    let w = Locker { kp: who.kp.insecure_clone(), lp: more };
    let (r, l2) = env.lock(&w, &p, 0, LP, l.nonce);
    expect_ok(&r);
    assert_eq!(l2.record, l.record);
    assert_eq!(env.record(&l2).amount, LP);
}

#[test]
fn inv03_unlock_spl_token_lock() {
    let mut env = Env::new();
    let s = env.synthetic_pool(TOKEN);
    let (who, l) = env.new_lock(&s, 0, 4 * LP);
    assert_eq!(env.record(&l).token_program, TOKEN_PROGRAM_SPL);
    assert_eq!(env.svm.get_account(&l.vault).unwrap().owner, TOKEN);
    env.donate(&l, 1);
    env.warp(DURATIONS[0]);
    // The Token-2022 program for an SPL lock is refused.
    let mut a = env.unlock_accounts(&who.key(), &l, &who.lp);
    a.token_program = TOKEN_2022;
    let ix = env.unlock_ix(a);
    expect_err(&env.send(&[ix], &who.kp, &[]), "WrongTokenProgram");
    expect_ok(&env.unlock(&who.kp, &l, &who.lp));
    assert_eq!(env.token_amount(&who.lp), 4 * LP + 1);
    assert!(!env.exists(&l.vault) && !env.exists(&l.record));
}

#[test]
fn inv03_unlock_refuses_another_locks_vault_or_mint() {
    let mut env = Env::new();
    let p = env.troll;
    let (who, l) = env.new_lock(&p, 0, LP);
    let (_, other) = env.new_lock(&p, 0, 2 * LP);
    env.warp(DURATIONS[0]);
    let mut a = env.unlock_accounts(&who.key(), &l, &who.lp);
    a.vault = other.vault;
    let ix = env.unlock_ix(a);
    expect_err(&env.send(&[ix], &who.kp, &[]), "WrongVault");
    let mut a = env.unlock_accounts(&who.key(), &l, &who.lp);
    a.lp_mint = env.usdp.lp_mint;
    let ix = env.unlock_ix(a);
    expect_err(&env.send(&[ix], &who.kp, &[]), "WrongMint");
    assert_eq!(env.token_amount(&other.vault), 2 * LP);
}

#[test]
fn inv04_extend_and_unlock_ignore_the_config() {
    // Paused, set to other values, zeroed, and deleted.
    for case in 0..4 {
        let mut env = Env::new();
        let p = env.troll;
        let (who, l) = env.new_lock(&p, 0, LP);
        match case {
            0 => expect_ok(&env.set_paused(true)),
            1 => expect_ok(&env.set_config(Pubkey::new_unique(), MAX_FEE_LAMPORTS, true)),
            2 => {
                let mut a = env.svm.get_account(&config_pda()).unwrap();
                a.data.fill(0);
                env.svm.set_account(config_pda(), a).unwrap();
            }
            _ => {
                env.svm
                    .set_account(
                        config_pda(),
                        Account {
                            lamports: 0,
                            data: vec![],
                            owner: SYSTEM,
                            executable: false,
                            rent_epoch: 0,
                        },
                    )
                    .unwrap();
                assert!(!env.exists(&config_pda()));
            }
        }
        expect_ok(&env.extend(&who.kp, &l, 1));
        env.warp(DURATIONS[1]);
        expect_ok(&env.unlock(&who.kp, &l, &who.lp));
        assert_eq!(env.token_amount(&who.lp), LP, "case {case}");
    }
}

#[test]
fn inv04_extend_and_unlock_ignore_the_pool() {
    // The PumpSwap pool closed, re-laid out, or handed to another program.
    for case in 0..3 {
        let mut env = Env::new();
        let s = env.synthetic_pool(TOKEN_2022);
        let (who, l) = env.new_lock(&s, 0, LP);
        match case {
            0 => env
                .svm
                .set_account(
                    s.pool,
                    Account {
                        lamports: 0,
                        data: vec![],
                        owner: SYSTEM,
                        executable: false,
                        rent_epoch: 0,
                    },
                )
                .unwrap(),
            1 => env
                .svm
                .set_account(s.pool, account(PUMP_AMM, vec![0x5a; 777]))
                .unwrap(),
            _ => env
                .svm
                .set_account(s.pool, account(Pubkey::new_unique(), vec![1; 301]))
                .unwrap(),
        }
        expect_ok(&env.extend(&who.kp, &l, 1));
        env.warp(DURATIONS[1]);
        expect_ok(&env.unlock(&who.kp, &l, &who.lp));
        assert_eq!(env.token_amount(&who.lp), LP, "case {case}");
    }
}

#[test]
fn inv12_destination_of_another_wallet_refused() {
    let mut env = Env::new();
    let p = env.troll;
    let (who, l) = env.new_lock(&p, 0, LP);
    env.warp(DURATIONS[0]);
    let stranger = Pubkey::new_unique();
    let theirs = env.token_account(&stranger, &p.lp_mint, 0);
    expect_err(&env.unlock(&who.kp, &l, &theirs), "WrongTokenOwner");
    // The vault itself (owned by the record) is no destination either.
    expect_fail(&env.unlock(&who.kp, &l, &l.vault));
    assert_eq!(env.token_amount(&l.vault), LP);
}

#[test]
fn inv12_destination_of_another_mint_refused() {
    let mut env = Env::new();
    let p = env.troll;
    let (who, l) = env.new_lock(&p, 0, LP);
    env.warp(DURATIONS[0]);
    let other_mint = env.usdp.lp_mint;
    let wrong = env.token_account(&who.key(), &other_mint, 0);
    expect_err(&env.unlock(&who.kp, &l, &wrong), "WrongMint");
    // A non-token account.
    let junk = Pubkey::new_unique();
    env.svm.set_account(junk, account(SYSTEM, vec![])).unwrap();
    expect_fail(&env.unlock(&who.kp, &l, &junk));
    assert_eq!(env.token_amount(&l.vault), LP);
}

#[test]
fn inv12_any_token_account_the_owner_owns_is_a_destination() {
    let mut env = Env::new();
    let p = env.troll;
    let (who, l) = env.new_lock(&p, 0, LP);
    env.warp(DURATIONS[0]);
    let second = env.token_account(&who.key(), &p.lp_mint, 5);
    expect_ok(&env.unlock(&who.kp, &l, &second));
    assert_eq!(env.token_amount(&second), LP + 5);
    assert_eq!(env.token_amount(&who.lp), 0);
}

#[test]
fn l2_unlock_and_relock_with_the_same_nonce_in_one_transaction() {
    // The record's address is reused: a fresh record (new start, amount,
    // tier, no extension) at the same address. Readers must re-read it.
    let mut env = Env::new();
    let p = env.troll;
    let (who, l) = env.new_lock(&p, 0, 3 * LP);
    expect_ok(&env.extend(&who.kp, &l, 1));
    let old = env.record(&l);
    env.warp(DURATIONS[1] + 10);
    let unlock = env.unlock_ix(env.unlock_accounts(&who.key(), &l, &who.lp));
    let relock = env.lock_ix(env.lock_accounts(&who.key(), &who.lp, &p, l.nonce), 2, 2 * LP, l.nonce);
    expect_ok(&env.send(&[unlock, relock], &who.kp, &[]));
    let new = env.record(&l);
    assert_eq!(lock_pda(&who.key(), &p.lp_mint, l.nonce), l.record);
    assert_eq!((new.amount, new.tier, new.extended_at), (2 * LP, 2, 0));
    assert!(new.start_ts > old.start_ts);
    assert_eq!(new.unlock_ts, new.start_ts + DURATIONS[2]);
    assert_eq!(env.token_amount(&l.vault), 2 * LP);
    assert_eq!(env.token_amount(&who.lp), LP);
}

#[test]
fn info_memo_transfer_destination_is_refused_and_another_account_works() {
    // A destination requiring incoming memos refuses our CPI transfer; the
    // owner names another account it owns and the lock is not stuck.
    let mut env = Env::new();
    let p = env.troll;
    let (who, l) = env.new_lock(&p, 0, LP);
    env.warp(DURATIONS[0]);
    let memo = env.token_account_with_ext(&who.key(), &p.lp_mint, 0, 8);
    let r = env.unlock(&who.kp, &l, &memo);
    expect_fail(&r);
    assert!(logs(&r).contains("No memo in previous instruction"), "{}", logs(&r));
    assert_eq!(env.token_amount(&l.vault), LP);
    assert!(env.exists(&l.record));
    expect_ok(&env.unlock(&who.kp, &l, &who.lp));
    assert_eq!(env.token_amount(&who.lp), LP);
}
