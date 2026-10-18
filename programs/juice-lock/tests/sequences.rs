//! Invariant 14: random sequences of lock / extend / unlock / donate / warp
//! across several owners and both token programs. After every step: each
//! open vault holds at least its record's amount; the records' amounts sum
//! to what was locked minus what was unlocked; the vaults' balances sum to
//! what went in (locks and donations) minus what came out (unlocks); LP is
//! conserved per mint; and each instruction succeeds exactly when the model
//! says it should.
#![allow(clippy::result_large_err)]
mod common;
use common::*;
use proptest::prelude::*;

#[derive(Clone, Debug)]
enum Op {
    Lock { who: usize, pool: usize, tier: u8, amount: u64 },
    Extend { lock: usize, tier: u8, by_owner: bool },
    Unlock { lock: usize, by_owner: bool },
    Donate { lock: usize, amount: u64 },
    Warp { secs: i64 },
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        3 => (0..3usize, 0..2usize, 0..7u8, 1..5_000u64)
            .prop_map(|(who, pool, tier, amount)| Op::Lock { who, pool, tier, amount: amount * 1_000_000 }),
        2 => (0..16usize, 0..6u8, prop::bool::weighted(0.85))
            .prop_map(|(lock, tier, by_owner)| Op::Extend { lock, tier, by_owner }),
        3 => (0..16usize, prop::bool::weighted(0.85))
            .prop_map(|(lock, by_owner)| Op::Unlock { lock, by_owner }),
        1 => (0..16usize, 1..1_000u64).prop_map(|(lock, amount)| Op::Donate { lock, amount }),
        3 => prop_oneof![
            Just(1i64),
            Just(DURATIONS[0]),
            Just(DURATIONS[1]),
            Just(DURATIONS[2]),
            0..DURATIONS[4],
        ]
        .prop_map(|secs| Op::Warp { secs }),
    ]
}

/// The model of one lock.
#[derive(Clone)]
struct M {
    l: L,
    owner: usize,
    amount: u64,
    unlock_ts: i64,
    forever: bool,
    open: bool,
}

const HOLD: u64 = 10_000 * LP;

fn run(ops: Vec<Op>) {
    let mut env = Env::new();
    let pools = [env.troll, env.synthetic_pool(TOKEN)];
    // owners[i] holds LP of both pools; one account per (owner, pool).
    let owners: Vec<Keypair> = (0..3).map(|_| env.wallet(100 * SOL)).collect();
    let lp: Vec<Vec<Pubkey>> = owners
        .iter()
        .map(|o| pools.iter().map(|p| env.token_account(&o.pubkey(), &p.lp_mint, HOLD)).collect())
        .collect();
    let stranger = env.wallet(100 * SOL);
    let stranger_lp: Vec<Pubkey> = pools
        .iter()
        .map(|p| env.token_account(&stranger.pubkey(), &p.lp_mint, HOLD))
        .collect();
    let mut locks: Vec<M> = vec![];
    let (mut locked_in, mut donated, mut paid_out, mut unlocked_amount) = ([0u64; 2], [0u64; 2], [0u64; 2], [0u64; 2]);
    let mut nonce = 0u64;
    let pool_ix = |p: &P| if p.program == TOKEN { 1 } else { 0 };

    for op in ops {
        match op {
            Op::Lock { who, pool, tier, amount } => {
                let p = pools[pool];
                nonce += 1;
                let w = Locker { kp: owners[who].insecure_clone(), lp: lp[who][pool] };
                let have = env.token_amount(&w.lp);
                let (r, l) = env.lock(&w, &p, tier, amount, nonce);
                let ok = tier <= 5 && amount <= have;
                assert_eq!(r.is_ok(), ok, "lock {tier} {amount}: {}", err_name(&r));
                if ok {
                    let rec = env.record(&l);
                    assert_eq!(rec.amount, amount);
                    locked_in[pool] += amount;
                    locks.push(M { l, owner: who, amount, unlock_ts: rec.unlock_ts, forever: tier == 5, open: true });
                }
            }
            Op::Extend { lock, tier, by_owner } => {
                let Some(m) = locks.get(lock).cloned() else { continue };
                let signer = if by_owner { owners[m.owner].insecure_clone() } else { stranger.insecure_clone() };
                let r = env.extend(&signer, &m.l, tier);
                let new = if tier == 5 { i64::MAX } else { env.now() + DURATIONS[tier as usize] };
                let ok = m.open && by_owner && !m.forever && new > m.unlock_ts;
                assert_eq!(r.is_ok(), ok, "extend: {}", err_name(&r));
                if ok {
                    locks[lock].unlock_ts = new;
                    locks[lock].forever = tier == 5;
                }
            }
            Op::Unlock { lock, by_owner } => {
                let Some(m) = locks.get(lock).cloned() else { continue };
                let pi = pool_ix(&m.l.pool);
                let (signer, dest) = if by_owner {
                    (owners[m.owner].insecure_clone(), lp[m.owner][pi])
                } else {
                    (stranger.insecure_clone(), stranger_lp[pi])
                };
                let bal = if m.open { env.token_amount(&m.l.vault) } else { 0 };
                let d0 = env.token_amount(&dest);
                let r = env.unlock(&signer, &m.l, &dest);
                let ok = m.open && by_owner && !m.forever && env.now() >= m.unlock_ts;
                assert_eq!(r.is_ok(), ok, "unlock: {}", err_name(&r));
                if ok {
                    assert_eq!(env.token_amount(&dest) - d0, bal);
                    assert!(!env.exists(&m.l.vault) && !env.exists(&m.l.record));
                    paid_out[pi] += bal;
                    unlocked_amount[pi] += m.amount;
                    locks[lock].open = false;
                }
            }
            Op::Donate { lock, amount } => {
                let Some(m) = locks.get(lock).cloned() else { continue };
                if !m.open {
                    continue;
                }
                let pi = pool_ix(&m.l.pool);
                expect_ok(&env.raw_transfer(&stranger_lp[pi], &m.l.vault, &m.l.pool.lp_mint, &stranger, amount, 9));
                donated[pi] += amount;
            }
            Op::Warp { secs } => env.warp(secs),
        }

        // Invariants after every step.
        let (mut vaults, mut records) = ([0u64; 2], [0u64; 2]);
        for m in locks.iter().filter(|m| m.open) {
            let pi = pool_ix(&m.l.pool);
            let rec = env.record(&m.l);
            let bal = env.token_amount(&m.l.vault);
            assert!(bal >= rec.amount, "vault below its record");
            assert_eq!(rec.amount, m.amount);
            assert_eq!(rec.unlock_ts, m.unlock_ts);
            assert_eq!(rec.forever, m.forever);
            vaults[pi] += bal;
            records[pi] += rec.amount;
        }
        for pi in 0..2 {
            assert_eq!(records[pi], locked_in[pi] - unlocked_amount[pi]);
            assert_eq!(vaults[pi], locked_in[pi] + donated[pi] - paid_out[pi]);
            let wallets: u64 = lp.iter().map(|v| env.token_amount(&v[pi])).sum::<u64>()
                + env.token_amount(&stranger_lp[pi]);
            assert_eq!(wallets + vaults[pi], 4 * HOLD, "LP conserved");
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 48, ..ProptestConfig::default() })]
    #[test]
    fn inv14_random_sequences_keep_custody(ops in prop::collection::vec(op(), 1..40)) {
        run(ops);
    }
}

/// A fixed sequence that takes every path the model knows (both token
/// programs, donations, extends, early and stranger unlocks, Forever).
#[test]
fn inv14_fixed_sequence_takes_every_path() {
    use Op::*;
    run(vec![
        Lock { who: 0, pool: 0, tier: 0, amount: 5 * LP },
        Lock { who: 1, pool: 1, tier: 1, amount: 7 * LP },
        Lock { who: 2, pool: 0, tier: 5, amount: LP },
        Lock { who: 2, pool: 1, tier: 6, amount: LP },
        Donate { lock: 0, amount: 123 },
        Donate { lock: 1, amount: 9 },
        Unlock { lock: 0, by_owner: true },
        Warp { secs: DURATIONS[0] },
        Unlock { lock: 0, by_owner: false },
        Extend { lock: 1, tier: 0, by_owner: true },
        Extend { lock: 1, tier: 2, by_owner: false },
        Extend { lock: 1, tier: 2, by_owner: true },
        Extend { lock: 2, tier: 4, by_owner: true },
        Unlock { lock: 0, by_owner: true },
        Unlock { lock: 0, by_owner: true },
        Warp { secs: DURATIONS[2] },
        Unlock { lock: 1, by_owner: true },
        Unlock { lock: 2, by_owner: true },
        Donate { lock: 2, amount: 5 },
        Lock { who: 0, pool: 0, tier: 0, amount: HOLD + 1 },
    ]);
}
