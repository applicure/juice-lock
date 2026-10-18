//! `extend`: invariant 5. It strictly raises `unlock_ts` or sets Forever,
//! never lowers it, can't change a Forever lock, and only the owner may.
#![allow(clippy::result_large_err)]
mod common;
use common::*;
use juice_lock::events::Extended;

#[test]
fn inv05_extend_strictly_raises_the_unlock_time() {
    let mut env = Env::new();
    let p = env.troll;
    let (who, l) = env.new_lock(&p, 2, LP);
    let start = env.record(&l);
    // The same or a shorter tier at once: not later, refused.
    for t in 0..=2u8 {
        expect_err(&env.extend(&who.kp, &l, t), "NotLater");
    }
    // A day later, tier 2 again ends a day later: allowed.
    env.warp(86_400);
    let r = env.extend(&who.kp, &l, 2);
    expect_ok(&r);
    let rec = env.record(&l);
    assert_eq!(rec.unlock_ts, start.unlock_ts + 86_400);
    assert_eq!(rec.extended_at, env.now());
    assert_eq!(rec.start_ts, start.start_ts);
    assert_eq!(rec.amount, start.amount);
    let ev = events::<Extended>(&r);
    assert_eq!(
        (ev[0].lock, ev[0].old_tier, ev[0].new_tier, ev[0].old_unlock_ts, ev[0].new_unlock_ts),
        (l.record, 2, 2, start.unlock_ts, start.unlock_ts + 86_400)
    );
    // A longer tier.
    expect_ok(&env.extend(&who.kp, &l, 4));
    let rec = env.record(&l);
    assert_eq!(rec.tier, 4);
    assert_eq!(rec.unlock_ts, env.now() + DURATIONS[4]);
    // Shorter again: refused, nothing changes.
    for t in 0..=4u8 {
        expect_err(&env.extend(&who.kp, &l, t), "NotLater");
    }
    assert_eq!(env.record(&l).unlock_ts, rec.unlock_ts);
}

#[test]
fn inv05_extend_never_lowers_the_unlock_time() {
    // Exhaustive over (tier, new tier, elapsed): success iff the new unlock
    // time is strictly later; the unlock time never goes down.
    for tier in 0..5u8 {
        for new_tier in 0..=5u8 {
            for elapsed in [0i64, 1, DURATIONS[0], DURATIONS[2], DURATIONS[4] + 5] {
                let mut env = Env::new();
                let p = env.troll;
                let (who, l) = env.new_lock(&p, tier, LP);
                let old = env.record(&l).unlock_ts;
                env.warp(elapsed);
                let r = env.extend(&who.kp, &l, new_tier);
                let want = if new_tier == FOREVER_TIER {
                    i64::MAX
                } else {
                    env.now() + DURATIONS[new_tier as usize]
                };
                let now_ts = env.record(&l).unlock_ts;
                if want > old {
                    expect_ok(&r);
                    assert_eq!(now_ts, want);
                } else {
                    expect_err(&r, "NotLater");
                    assert_eq!(now_ts, old);
                }
                assert!(now_ts >= old);
            }
        }
    }
}

#[test]
fn inv05_extend_after_expiry_relocks() {
    let mut env = Env::new();
    let p = env.troll;
    let (who, l) = env.new_lock(&p, 0, LP);
    env.warp(DURATIONS[0] + 3_600);
    expect_ok(&env.extend(&who.kp, &l, 0));
    assert_eq!(env.record(&l).unlock_ts, env.now() + DURATIONS[0]);
    expect_err(&env.unlock(&who.kp, &l, &who.lp), "StillLocked");
}

#[test]
fn inv05_forever_is_final() {
    let mut env = Env::new();
    let p = env.troll;
    let (who, l) = env.new_lock(&p, 1, LP);
    expect_ok(&env.extend(&who.kp, &l, FOREVER_TIER));
    let rec = env.record(&l);
    assert!(rec.forever);
    assert_eq!(rec.tier, FOREVER_TIER);
    assert_eq!(rec.unlock_ts, i64::MAX);
    for t in 0..=5u8 {
        expect_err(&env.extend(&who.kp, &l, t), "ForeverLock");
    }
    let (who2, l2) = env.new_lock(&p, FOREVER_TIER, LP);
    for t in 0..=5u8 {
        expect_err(&env.extend(&who2.kp, &l2, t), "ForeverLock");
    }
    assert_eq!(env.record(&l2).unlock_ts, i64::MAX);
}

#[test]
fn inv05_extend_by_a_non_owner_refused() {
    let mut env = Env::new();
    let p = env.troll;
    let (_, l) = env.new_lock(&p, 0, LP);
    let stranger = env.wallet(SOL);
    expect_err(&env.extend(&stranger, &l, 4), "NotOwner");
    expect_err(&env.extend(&stranger, &l, FOREVER_TIER), "NotOwner");
    assert_eq!(env.record(&l).tier, 0);
}

#[test]
fn inv05_extend_tier_out_of_range_and_overflow() {
    let mut env = Env::new();
    let p = env.troll;
    let (who, l) = env.new_lock(&p, 0, LP);
    for t in [6u8, 200, 255] {
        expect_err(&env.extend(&who.kp, &l, t), "InvalidTier");
    }
    env.set_time(i64::MAX - 10);
    let r = env.extend(&who.kp, &l, 0);
    expect_err(&r, "MathOverflow");
    assert!(!logs(&r).contains("panicked"));
    // Forever needs no sum.
    expect_ok(&env.extend(&who.kp, &l, FOREVER_TIER));
}

#[test]
fn inv05_extend_takes_only_the_record() {
    let ix = Env::new().extend_ix(&Pubkey::new_unique(), &Pubkey::new_unique(), 1);
    assert_eq!(ix.accounts.len(), 2);
    let mut env = Env::new();
    let p = env.troll;
    let (who, l) = env.new_lock(&p, 0, LP);
    // A doctored account of another owner at another address is refused (seeds).
    let mut fake = env.record(&l);
    fake.unlock_ts = 0;
    let addr = Pubkey::new_unique();
    let acc = env.svm.get_account(&l.record).unwrap();
    env.svm.set_account(addr, acc).unwrap();
    env.write(&addr, &fake);
    let ix = env.extend_ix(&who.key(), &addr, 1);
    expect_fail(&env.send(&[ix], &who.kp, &[]));
}
