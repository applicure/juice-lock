//! Compute units per instruction (printed with `-- --nocapture`), with a
//! bound each so a regression shows. Measured on the recorded TROLL/SOL pool
//! (Token-2022) and a synthetic SPL Token pool.
#![allow(clippy::result_large_err)]
mod common;
use common::*;

#[test]
fn compute_units_per_instruction() {
    let mut env = Env::new();
    for p in [env.troll, env.synthetic_pool(TOKEN)] {
        let who = env.locker(&p, 2 * LP);
        let (r, l) = env.lock(&who, &p, 0, LP, 1);
        expect_ok(&r);
        let lock = units(&r);
        let r = env.extend(&who.kp, &l, 1);
        expect_ok(&r);
        let extend = units(&r);
        env.warp(DURATIONS[1]);
        let r = env.unlock(&who.kp, &l, &who.lp);
        expect_ok(&r);
        let unlock = units(&r);
        let name = if p.program == TOKEN { "SPL Token" } else { "Token-2022" };
        println!("{name}: lock {lock} CU, extend {extend} CU, unlock {unlock} CU");
        assert!(lock < 80_000, "lock {lock}");
        assert!(extend < 15_000, "extend {extend}");
        assert!(unlock < 40_000, "unlock {unlock}");
    }
    let r = env.set_paused(true);
    expect_ok(&r);
    println!("set_config {} CU", units(&r));
}
