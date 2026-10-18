//! Invariant 9: the admin's powers and their limits. `init_config` only by
//! the upgrade authority and only once; `set_config` within the fee bounds
//! and touching only the config; `set_admin` needs both signatures.
#![allow(clippy::result_large_err)]
mod common;
use common::*;
use juice_lock::events::{AdminChanged, ConfigChanged, ConfigInitialized};

/// Rent minimum of an empty account at mainnet rates: 128 bytes x 5,080.
const RENT_MIN: u64 = 128 * 5_080;

#[test]
fn inv09_init_config_by_the_upgrade_authority_once() {
    let mut env = Env::new_uninit();
    let (a, t) = (env.admin.pubkey(), env.treasury.pubkey());
    // A stranger (also as payer).
    let s = env.wallet(SOL);
    let ix = env.init_config_ix(&s.pubkey(), &s.pubkey(), a, t, FEE);
    expect_err(&env.send(&[ix], &s, &[]), "NotUpgradeAuthority");
    // The stored admin is not the upgrade authority either.
    let admin = env.admin.insecure_clone();
    let ix = env.init_config_ix(&admin.pubkey(), &admin.pubkey(), a, t, FEE);
    expect_err(&env.send(&[ix], &admin, &[]), "NotUpgradeAuthority");
    // A fake ProgramData naming the stranger as authority.
    let mut ix = env.init_config_ix(&s.pubkey(), &s.pubkey(), a, t, FEE);
    let fake = Pubkey::new_unique();
    let mut pd = env.svm.get_account(&program_data_pda()).unwrap();
    pd.data[13..45].copy_from_slice(s.pubkey().as_ref());
    env.svm.set_account(fake, pd).unwrap();
    ix.accounts[2].pubkey = fake;
    expect_err(&env.send(&[ix], &s, &[]), "NotUpgradeAuthority");
    // After a revoke (no upgrade authority) nobody can.
    env.set_upgrade_authority(None);
    let u = env.upgrade.insecure_clone();
    let ix = env.init_config_ix(&u.pubkey(), &u.pubkey(), a, t, FEE);
    expect_err(&env.send(&[ix], &u, &[]), "NotUpgradeAuthority");
    env.set_upgrade_authority(Some(u.pubkey()));
    // The upgrade authority, with a separate payer.
    let ix = env.init_config_ix(&u.pubkey(), &s.pubkey(), a, t, FEE);
    let r = env.send(&[ix], &s, &[&u]);
    expect_ok(&r);
    let c = env.config();
    assert_eq!(
        (c.version, c.paused_new, c.admin, c.treasury, c.fee_lamports),
        (1, false, a, t, FEE)
    );
    assert_eq!(env.lamports(&config_pda()), (88 + 128) * 5_080);
    let ev = events::<ConfigInitialized>(&r);
    assert_eq!((ev[0].admin, ev[0].treasury, ev[0].fee_lamports), (a, t, FEE));
    // Only once.
    expect_fail(&env.init_config(FEE));
    let ix = env.init_config_ix(&u.pubkey(), &u.pubkey(), s.pubkey(), s.pubkey(), 0);
    expect_fail(&env.send(&[ix], &u, &[]));
    assert_eq!(env.config().admin, a);
}

#[test]
fn inv09_init_config_bounds() {
    let mut env = Env::new_uninit();
    let u = env.upgrade.insecure_clone();
    let (a, t) = (env.admin.pubkey(), env.treasury.pubkey());
    for (admin, treasury, fee) in [
        (Pubkey::default(), t, FEE),
        (a, Pubkey::default(), FEE),
        (a, t, MAX_FEE_LAMPORTS + 1),
        (a, t, 1),
        (a, t, RENT_MIN - 1),
    ] {
        let ix = env.init_config_ix(&u.pubkey(), &u.pubkey(), admin, treasury, fee);
        expect_err(&env.send(&[ix], &u, &[]), "InvalidConfigValue");
    }
    for fee in [0, RENT_MIN, MAX_FEE_LAMPORTS] {
        let mut env = Env::new_uninit();
        expect_ok(&env.init_config(fee));
        assert_eq!(env.config().fee_lamports, fee);
    }
}

#[test]
fn inv09_set_config_bounds() {
    let mut env = Env::new();
    let t = env.treasury.pubkey();
    for fee in [MAX_FEE_LAMPORTS + 1, u64::MAX, 1, RENT_MIN - 1] {
        expect_err(&env.set_config(t, fee, false), "InvalidConfigValue");
    }
    expect_err(&env.set_config(Pubkey::default(), FEE, false), "InvalidConfigValue");
    assert_eq!(env.config().fee_lamports, FEE);
    for fee in [0, RENT_MIN, FEE, MAX_FEE_LAMPORTS] {
        expect_ok(&env.set_config(t, fee, false));
        assert_eq!(env.config().fee_lamports, fee);
    }
}

#[test]
fn inv09_set_config_admin_only_and_touches_only_the_config() {
    let mut env = Env::new();
    let p = env.troll;
    let (who, l) = env.new_lock(&p, 0, LP);
    let rec0 = env.data(&l.record);
    let vault0 = env.data(&l.vault);
    // Neither a stranger nor the upgrade authority is the admin.
    for k in [env.wallet(SOL), env.upgrade.insecure_clone()] {
        let ix = env.set_config_ix(&k.pubkey(), k.pubkey(), 0, true);
        expect_err(&env.send(&[ix], &k, &[]), "NotAdmin");
    }
    let new_t = Pubkey::new_unique();
    let r = env.set_config(new_t, 2 * FEE, true);
    expect_ok(&r);
    let c = env.config();
    assert_eq!((c.treasury, c.fee_lamports, c.paused_new), (new_t, 2 * FEE, true));
    assert_eq!((c.admin, c.version), (env.admin.pubkey(), 1));
    let ev = events::<ConfigChanged>(&r);
    assert_eq!(
        (ev[0].old_treasury, ev[0].treasury, ev[0].old_fee_lamports, ev[0].fee_lamports, ev[0].old_paused_new, ev[0].paused_new),
        (env.treasury.pubkey(), new_t, FEE, 2 * FEE, false, true)
    );
    // The lock didn't change by a byte.
    assert_eq!(env.data(&l.record), rec0);
    assert_eq!(env.data(&l.vault), vault0);
    // The set_config account list is the admin and the config: nothing else can be named.
    assert_eq!(env.set_config_ix(&who.key(), new_t, 0, false).accounts.len(), 2);
    // A config elsewhere (not the PDA) is refused.
    let fake = Pubkey::new_unique();
    let acc = env.svm.get_account(&config_pda()).unwrap();
    env.svm.set_account(fake, acc).unwrap();
    let mut ix = env.set_config_ix(&env.admin.pubkey(), new_t, 0, false);
    ix.accounts[1].pubkey = fake;
    let admin = env.admin.insecure_clone();
    expect_fail(&env.send(&[ix], &admin, &[]));
}

#[test]
fn inv09_set_admin_needs_both_signatures() {
    let mut env = Env::new();
    let admin = env.admin.insecure_clone();
    let next = env.wallet(SOL);
    // Without the new admin's signature.
    let mut ix = env.set_admin_ix(&admin.pubkey(), &next.pubkey());
    ix.accounts[2].is_signer = false;
    expect_fail(&env.send(&[ix], &admin, &[]));
    // Without the admin's signature (the new admin alone).
    let mut ix = env.set_admin_ix(&admin.pubkey(), &next.pubkey());
    ix.accounts[0].is_signer = false;
    expect_fail(&env.send(&[ix], &next, &[]));
    // A stranger with the new admin.
    let s = env.wallet(SOL);
    let ix = env.set_admin_ix(&s.pubkey(), &next.pubkey());
    expect_err(&env.send(&[ix], &s, &[&next]), "NotAdmin");
    assert_eq!(env.config().admin, admin.pubkey());
    // Both.
    let ix = env.set_admin_ix(&admin.pubkey(), &next.pubkey());
    let r = env.send(&[ix], &admin, &[&next]);
    expect_ok(&r);
    assert_eq!(env.config().admin, next.pubkey());
    let ev = events::<AdminChanged>(&r);
    assert_eq!((ev[0].old, ev[0].new), (admin.pubkey(), next.pubkey()));
    // The old admin is out, the new one is in.
    let ix = env.set_config_ix(&admin.pubkey(), admin.pubkey(), 0, true);
    expect_err(&env.send(&[ix], &admin, &[]), "NotAdmin");
    let ix = env.set_config_ix(&next.pubkey(), next.pubkey(), 0, true);
    expect_ok(&env.send(&[ix], &next, &[]));
}
