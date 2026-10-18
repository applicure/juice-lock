//! `lock`: invariants 6 (refusals, measured amount), 8 (exact unlock times,
//! overflow), 10 (fee, pause) and 11 (PDA uniqueness), on the recorded
//! mainnet pools and synthetic ones.
#![allow(clippy::result_large_err)]
mod common;
use common::*;
use juice_lock::events::Locked;

#[test]
fn inv06_lock_records_the_measured_amount_and_every_field() {
    let mut env = Env::new();
    let p = env.troll;
    let who = env.locker(&p, 5 * LP);
    let (r, l) = env.lock(&who, &p, 2, 3 * LP, 42);
    expect_ok(&r);
    let rec = env.record(&l);
    assert_eq!(rec.version, 1);
    assert_eq!(rec.tier, 2);
    assert!(!rec.forever);
    assert_eq!(rec.lp_decimals, 9);
    assert_eq!(rec.token_program, TOKEN_PROGRAM_2022);
    assert_eq!(rec.owner, who.key());
    assert_eq!(rec.lp_mint, p.lp_mint);
    assert_eq!(rec.pump_pool, p.pool);
    assert_eq!(rec.vault, l.vault);
    assert_eq!(rec.amount, 3 * LP);
    assert_eq!(rec.start_ts, NOW);
    assert_eq!(rec.unlock_ts, NOW + DURATIONS[2]);
    assert_eq!(rec.nonce, 42);
    assert_eq!(rec.extended_at, 0);
    assert_eq!(rec.fee_paid, FEE);
    // The vault: a 165-byte token account of the LP mint, authority the record.
    let v = env.svm.get_account(&l.vault).unwrap();
    assert_eq!(v.owner, TOKEN_2022);
    assert_eq!(v.data.len(), 165);
    assert_eq!(v.data[0..32], *p.lp_mint.as_ref());
    assert_eq!(v.data[32..64], *l.record.as_ref());
    assert_eq!(env.token_amount(&l.vault), 3 * LP);
    assert_eq!(env.token_amount(&who.lp), 2 * LP);
    // Rents at mainnet rates: 192 + 128 and 165 + 128 bytes at 5,080 lamports per byte.
    assert_eq!(env.lamports(&l.record), (192 + 128) * 5_080);
    assert_eq!(env.lamports(&l.vault), (165 + 128) * 5_080);
    // `lock` writes nothing to the config: it is read-only in the instruction.
    let ix = env.lock_ix(env.lock_accounts(&who.key(), &who.lp, &p, 43), 0, 1, 43);
    assert!(!ix.accounts[1].is_writable);
    let ev = events::<Locked>(&r);
    assert_eq!(ev.len(), 1);
    let e = &ev[0];
    assert_eq!(
        (e.lock, e.owner, e.lp_mint, e.pump_pool),
        (l.record, who.key(), p.lp_mint, p.pool)
    );
    assert_eq!(
        (e.amount, e.tier, e.start_ts, e.unlock_ts, e.nonce, e.fee),
        (3 * LP, 2, NOW, NOW + DURATIONS[2], 42, FEE)
    );
}

#[test]
fn inv06_every_recorded_mainnet_pool_can_be_locked() {
    let mut env = Env::new();
    for p in [env.juice_pump, env.troll, env.usdp, env.user] {
        let (_, l) = env.new_lock(&p, 0, LP);
        assert_eq!(env.record(&l).amount, LP);
    }
}

#[test]
fn inv06_tier_out_of_range_refused() {
    let mut env = Env::new();
    let p = env.troll;
    let who = env.locker(&p, LP);
    for tier in [6u8, 7, 100, 255] {
        expect_err(&env.lock(&who, &p, tier, LP, 1).0, "InvalidTier");
    }
}

#[test]
fn inv06_zero_amount_refused() {
    let mut env = Env::new();
    let p = env.troll;
    let who = env.locker(&p, LP);
    expect_err(&env.lock(&who, &p, 0, 0, 1).0, "ZeroAmount");
}

#[test]
fn inv06_more_than_held_refused() {
    let mut env = Env::new();
    let p = env.troll;
    let who = env.locker(&p, LP);
    expect_fail(&env.lock(&who, &p, 0, LP + 1, 1).0);
    assert!(!env.exists(&lock_pda(&who.key(), &p.lp_mint, 1)));
}

#[test]
fn inv06_wrong_token_program_refused() {
    let mut env = Env::new();
    let p = env.troll;
    let who = env.locker(&p, LP);
    let mut a = env.lock_accounts(&who.key(), &who.lp, &p, 1);
    a.token_program = TOKEN;
    let ix = env.lock_ix(a, 0, LP, 1);
    expect_err(&env.send(&[ix], &who.kp, &[]), "WrongTokenProgram");
    // Not a token program at all.
    let mut a = env.lock_accounts(&who.key(), &who.lp, &p, 1);
    a.token_program = SYSTEM;
    let ix = env.lock_ix(a, 0, LP, 1);
    expect_fail(&env.send(&[ix], &who.kp, &[]));
}

#[test]
fn inv06_wrong_treasury_refused() {
    let mut env = Env::new();
    let p = env.troll;
    let who = env.locker(&p, LP);
    let mut a = env.lock_accounts(&who.key(), &who.lp, &p, 1);
    a.treasury = who.key();
    let ix = env.lock_ix(a, 0, LP, 1);
    expect_err(&env.send(&[ix], &who.kp, &[]), "WrongTreasury");
}

#[test]
fn inv06_owner_token_of_another_wallet_or_mint_refused() {
    let mut env = Env::new();
    let p = env.troll;
    let who = env.locker(&p, LP);
    let other = env.locker(&p, LP);
    let mut a = env.lock_accounts(&who.key(), &other.lp, &p, 1);
    a.owner_token = other.lp;
    let ix = env.lock_ix(a, 0, LP, 1);
    expect_err(&env.send(&[ix], &who.kp, &[]), "WrongTokenOwner");
    let q = env.usdp;
    let wrong = env.token_account(&who.key(), &q.lp_mint, LP);
    let a = env.lock_accounts(&who.key(), &wrong, &p, 1);
    let ix = env.lock_ix(a, 0, LP, 1);
    expect_err(&env.send(&[ix], &who.kp, &[]), "WrongMint");
}

/// Builds the lock of a (possibly fake) pool/mint pair with LP of `lp_mint`.
fn try_pool(env: &mut Env, pool: Pubkey, lp_mint: Pubkey, program: Pubkey) -> TxResult {
    let p = P {
        pool,
        lp_mint,
        program,
    };
    let who = env.locker(&p, LP);
    env.lock(&who, &p, 0, LP, 1).0
}

#[test]
fn inv06_fake_pools_refused() {
    let mut env = Env::new();
    let real = env.troll;
    let t0 = env.lamports(&env.treasury.pubkey());

    // A pool account owned by another program (same bytes as the real pool).
    let fake = Pubkey::new_unique();
    let mut acc = env.svm.get_account(&real.pool).unwrap();
    acc.owner = Pubkey::new_unique();
    env.svm.set_account(fake, acc).unwrap();
    let mint = lp_mint_pda(&fake);
    env.set_mint(mint, TOKEN_2022, 9, Some(fake), None, &[]);
    expect_err(&try_pool(&mut env, fake, mint, TOKEN_2022), "PumpPoolMismatch");

    // Owned by PumpSwap but another discriminator (e.g. a GlobalConfig).
    let fake = Pubkey::new_unique();
    let mut d = env.data(&real.pool);
    d[0] ^= 1;
    env.svm.set_account(fake, account(PUMP_AMM, d)).unwrap();
    let mint = lp_mint_pda(&fake);
    env.set_mint(mint, TOKEN_2022, 9, Some(fake), None, &[]);
    expect_err(&try_pool(&mut env, fake, mint, TOKEN_2022), "PumpPoolMismatch");

    // Shorter than a discriminator.
    let fake = Pubkey::new_unique();
    env.svm.set_account(fake, account(PUMP_AMM, vec![241, 154, 109])).unwrap();
    let mint = lp_mint_pda(&fake);
    env.set_mint(mint, TOKEN_2022, 9, Some(fake), None, &[]);
    expect_err(&try_pool(&mut env, fake, mint, TOKEN_2022), "PumpPoolMismatch");

    // A real pool with another pool's LP mint.
    let other_mint = env.usdp.lp_mint;
    expect_err(
        &try_pool(&mut env, real.pool, other_mint, TOKEN_2022),
        "PumpPoolMismatch",
    );

    // A mint at a non-PDA address whose authority is the real pool.
    let mint = Pubkey::new_unique();
    env.set_mint(mint, TOKEN_2022, 9, Some(real.pool), None, &[]);
    expect_err(&try_pool(&mut env, real.pool, mint, TOKEN_2022), "PumpPoolMismatch");

    // The right PDA address, but the mint's authority isn't the pool.
    let s = env.synthetic_pool(TOKEN_2022);
    env.set_mint(s.lp_mint, TOKEN_2022, 9, Some(Pubkey::new_unique()), None, &[]);
    expect_err(&try_pool(&mut env, s.pool, s.lp_mint, TOKEN_2022), "PumpPoolMismatch");
    // ... or has no mint authority at all.
    env.set_mint(s.lp_mint, TOKEN_2022, 9, None, None, &[]);
    expect_err(&try_pool(&mut env, s.pool, s.lp_mint, TOKEN_2022), "PumpPoolMismatch");
    // ... and a tag byte other than Some (the key bytes are the pool's).
    let mut d = mint_data(9, Some(s.pool), None, &[]);
    d[0] = 2;
    let acc = Account {
        lamports: env.rent(82),
        data: d,
        owner: TOKEN_2022,
        executable: false,
        rent_epoch: 0,
    };
    env.svm.set_account(s.lp_mint, acc).unwrap();
    expect_err(&try_pool(&mut env, s.pool, s.lp_mint, TOKEN_2022), "PumpPoolMismatch");

    // None of these went through: no fee reached the treasury.
    assert_eq!(env.lamports(&env.treasury.pubkey()), t0);
}

#[test]
fn inv06_pool_layout_is_not_read() {
    // A PumpSwap pool of any length and content after the discriminator is
    // accepted: the binding is the LP mint's address and authority alone.
    let mut env = Env::new();
    for len in [8usize, 301, 1_000] {
        let s = env.synthetic_pool(TOKEN_2022);
        let mut d = vec![0xabu8; len];
        d[..8].copy_from_slice(&juice_lock::pump::POOL_DISCRIMINATOR);
        env.svm.set_account(s.pool, account(PUMP_AMM, d)).unwrap();
        let (_, l) = env.new_lock(&s, 0, LP);
        assert_eq!(env.record(&l).pump_pool, s.pool);
    }
}

#[test]
fn inv08_unlock_ts_is_start_plus_duration_exactly() {
    let mut env = Env::new();
    let p = env.troll;
    for tier in 0..=5u8 {
        env.warp(1_234);
        let (_, l) = env.new_lock(&p, tier, LP);
        let r = env.record(&l);
        assert_eq!(r.start_ts, env.now());
        assert_eq!(r.tier, tier);
        if tier == FOREVER_TIER {
            assert!(r.forever);
            assert_eq!(r.unlock_ts, i64::MAX);
        } else {
            assert!(!r.forever);
            assert_eq!(r.unlock_ts, r.start_ts + DURATIONS[tier as usize]);
        }
    }
}

#[test]
fn inv08_overflow_returns_an_error_never_a_panic() {
    let mut env = Env::new();
    let p = env.troll;
    let who = env.locker(&p, 10 * LP);
    // One second too late for tier 0: start + duration overflows i64.
    env.set_time(i64::MAX - DURATIONS[0] + 1);
    let r = env.lock(&who, &p, 0, LP, 1).0;
    expect_err(&r, "MathOverflow");
    assert!(!logs(&r).contains("panicked"));
    // The longest tier overflows earlier.
    expect_err(&env.lock(&who, &p, 4, LP, 1).0, "MathOverflow");
    // Forever needs no sum and works at any time.
    expect_ok(&env.lock(&who, &p, 5, LP, 1).0);
    // Exactly at the edge, tier 0 lands on i64::MAX - and is still not Forever.
    env.set_time(i64::MAX - DURATIONS[0]);
    let (r, l) = env.lock(&who, &p, 0, LP, 2);
    expect_ok(&r);
    let rec = env.record(&l);
    assert_eq!(rec.unlock_ts, i64::MAX);
    assert!(!rec.forever);
}

#[test]
fn inv10_fee_goes_exactly_to_the_treasury() {
    let mut env = Env::new();
    let p = env.troll;
    let who = env.locker(&p, LP);
    let t = env.treasury.pubkey();
    let (t0, w0) = (env.lamports(&t), env.lamports(&who.key()));
    let (r, l) = env.lock(&who, &p, 0, LP, 1);
    expect_ok(&r);
    assert_eq!(env.lamports(&t) - t0, FEE);
    // The owner paid the fee, both rents and the 5,000-lamport signature.
    let rents = env.lamports(&l.record) + env.lamports(&l.vault);
    assert_eq!(w0 - env.lamports(&who.key()), FEE + rents + 5_000);
    assert_eq!(env.record(&l).fee_paid, FEE);
    // An empty treasury account (0 lamports) still receives the fee.
    assert_eq!(t0, 0);
}

#[test]
fn inv10_fee_follows_the_config_and_zero_works() {
    let mut env = Env::new();
    let p = env.troll;
    let new_t = Pubkey::new_unique();
    expect_ok(&env.set_config(new_t, 0, false));
    let who = env.locker(&p, 2 * LP);
    let w0 = env.lamports(&who.key());
    let (r, l) = env.lock(&who, &p, 0, LP, 1);
    expect_ok(&r);
    assert_eq!(env.lamports(&new_t), 0);
    assert_eq!(env.record(&l).fee_paid, 0);
    let rents = env.lamports(&l.record) + env.lamports(&l.vault);
    assert_eq!(w0 - env.lamports(&who.key()), rents + 5_000);
    assert_eq!(events::<Locked>(&r)[0].fee, 0);
    // The maximum fee, to the new treasury.
    expect_ok(&env.set_config(new_t, MAX_FEE_LAMPORTS, false));
    expect_ok(&env.lock(&who, &p, 0, LP, 2).0);
    assert_eq!(env.lamports(&new_t), MAX_FEE_LAMPORTS);
    // The old treasury is refused now.
    let mut a = env.lock_accounts(&who.key(), &who.lp, &p, 3);
    a.treasury = env.treasury.pubkey();
    let ix = env.lock_ix(a, 0, 1, 3);
    expect_err(&env.send(&[ix], &who.kp, &[]), "WrongTreasury");
}

#[test]
fn inv10_paused_blocks_lock_only() {
    let mut env = Env::new();
    let p = env.troll;
    let (a, la) = env.new_lock(&p, 0, LP);
    expect_ok(&env.set_paused(true));
    let who = env.locker(&p, LP);
    expect_err(&env.lock(&who, &p, 0, LP, 1).0, "Paused");
    // Extend and unlock still work while paused.
    expect_ok(&env.extend(&a.kp, &la, 1));
    env.warp(DURATIONS[1]);
    expect_ok(&env.unlock(&a.kp, &la, &a.lp));
    // Unpaused, lock works again.
    expect_ok(&env.set_paused(false));
    expect_ok(&env.lock(&who, &p, 0, LP, 1).0);
}

#[test]
fn inv11_same_owner_mint_and_nonce_twice_fails() {
    let mut env = Env::new();
    let p = env.troll;
    let who = env.locker(&p, 2 * LP);
    let (r, l) = env.lock(&who, &p, 0, LP, 9);
    expect_ok(&r);
    let before = env.record(&l);
    expect_fail(&env.lock(&who, &p, 3, LP, 9).0);
    // The first lock is untouched.
    let after = env.record(&l);
    assert_eq!((after.amount, after.tier, after.unlock_ts), (before.amount, before.tier, before.unlock_ts));
    assert_eq!(env.token_amount(&l.vault), LP);
    assert_eq!(env.token_amount(&who.lp), LP);
}

#[test]
fn inv11_other_nonces_owners_and_mints_coexist() {
    let mut env = Env::new();
    let p = env.troll;
    let q = env.usdp;
    let a = env.locker(&p, 10 * LP);
    let b = env.locker(&p, 10 * LP);
    let a_q = env.token_account(&a.key(), &q.lp_mint, 10 * LP);
    let mut seen = std::collections::HashSet::new();
    for nonce in [0u64, 1, 2, u64::MAX] {
        let (r, l) = env.lock(&a, &p, 0, LP, nonce);
        expect_ok(&r);
        assert!(seen.insert(l.record) && seen.insert(l.vault));
        // Another owner with the same nonce.
        let (r, l) = env.lock(&b, &p, 1, LP, nonce);
        expect_ok(&r);
        assert!(seen.insert(l.record) && seen.insert(l.vault));
        // The same owner and nonce on another mint.
        let qa = Locker { kp: a.kp.insecure_clone(), lp: a_q };
        let (r, l) = env.lock(&qa, &q, 2, LP, nonce);
        expect_ok(&r);
        assert!(seen.insert(l.record) && seen.insert(l.vault));
    }
    assert_eq!(seen.len(), 24);
}

#[test]
fn inv11_prefunded_record_or_vault_address_does_not_block_a_lock() {
    let mut env = Env::new();
    let p = env.troll;
    let who = env.locker(&p, LP);
    let record = lock_pda(&who.key(), &p.lp_mint, 5);
    let vault = vault_pda(&record);
    // Someone sends lamports to both addresses before the lock.
    env.svm.airdrop(&record, 1_000).unwrap();
    env.svm.airdrop(&vault, 7_777_777).unwrap();
    let (r, l) = env.lock(&who, &p, 0, LP, 5);
    expect_ok(&r);
    assert_eq!(env.token_amount(&l.vault), LP);
    assert_eq!(env.svm.get_account(&vault).unwrap().owner, TOKEN_2022);
    // An over-funded vault keeps its extra lamports (they go to the owner at unlock).
    assert_eq!(env.lamports(&vault), 7_777_777);
    // A vault address funded below its rent is topped up by the owner.
    let record = lock_pda(&who.key(), &p.lp_mint, 6);
    let vault = vault_pda(&record);
    env.svm.airdrop(&vault, 1_000).unwrap();
    let more = env.token_account(&who.key(), &p.lp_mint, LP);
    let w = Locker { kp: who.kp.insecure_clone(), lp: more };
    let (r, l) = env.lock(&w, &p, 0, LP, 6);
    expect_ok(&r);
    assert_eq!(env.lamports(&l.vault), (165 + 128) * 5_080);
    assert_eq!(env.token_amount(&l.vault), LP);
}

#[test]
fn inv10_max_fee_bounds_the_fee_charged() {
    let mut env = Env::new();
    let p = env.troll;
    let who = env.locker(&p, 10 * LP);
    let t = env.treasury.pubkey();
    // The fee above the owner's maximum: refused, nothing charged or created.
    let t0 = env.lamports(&t);
    let ix = env.lock_ix_fee(env.lock_accounts(&who.key(), &who.lp, &p, 1), 0, LP, 1, FEE - 1);
    expect_err(&env.send(&[ix], &who.kp, &[]), "FeeTooHigh");
    assert_eq!(env.lamports(&t), t0);
    assert!(!env.exists(&lock_pda(&who.key(), &p.lp_mint, 1)));
    // Equal: passes. Above the fee: passes, and only the fee is charged.
    let ix = env.lock_ix_fee(env.lock_accounts(&who.key(), &who.lp, &p, 1), 0, LP, 1, FEE);
    expect_ok(&env.send(&[ix], &who.kp, &[]));
    let ix = env.lock_ix_fee(env.lock_accounts(&who.key(), &who.lp, &p, 2), 0, LP, 2, u64::MAX);
    expect_ok(&env.send(&[ix], &who.kp, &[]));
    assert_eq!(env.lamports(&t) - t0, 2 * FEE);
    // A front-run: the owner signed for the shown fee, the admin raises it first.
    let signed = env.lock_ix_fee(env.lock_accounts(&who.key(), &who.lp, &p, 3), 0, LP, 3, FEE);
    expect_ok(&env.set_config(t, MAX_FEE_LAMPORTS, false));
    expect_err(&env.send(&[signed], &who.kp, &[]), "FeeTooHigh");
    // Zero fee with a zero maximum passes.
    expect_ok(&env.set_config(t, 0, false));
    let t1 = env.lamports(&t);
    let ix = env.lock_ix_fee(env.lock_accounts(&who.key(), &who.lp, &p, 4), 0, LP, 4, 0);
    expect_ok(&env.send(&[ix], &who.kp, &[]));
    assert_eq!(env.lamports(&t), t1);
}

#[test]
fn info_cpi_guard_source_is_refused_and_another_account_works() {
    // A source with CPI Guard refuses the owner-signed transfer inside our CPI;
    // the owner recovers by locking from an account without the guard.
    let mut env = Env::new();
    let p = env.troll;
    let who = env.locker(&p, LP);
    let guarded = env.token_account_with_ext(&who.key(), &p.lp_mint, LP, 11);
    let g = Locker { kp: who.kp.insecure_clone(), lp: guarded };
    let r = env.lock(&g, &p, 0, LP, 1).0;
    assert!(logs(&r).contains("CPI Guard is enabled"), "{}", logs(&r));
    expect_fail(&r);
    assert_eq!(env.token_amount(&guarded), LP);
    assert!(!env.exists(&lock_pda(&who.key(), &p.lp_mint, 1)));
    expect_ok(&env.lock(&who, &p, 0, LP, 1).0);
}
