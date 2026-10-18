//! Invariant 1: no instruction except `unlock` moves vault tokens. Shown
//! twice: over the IDL (only `lock` and `unlock` take a token program, so no
//! other instruction can make a token CPI at all; the program reads no
//! remaining accounts), and by trying every instruction with an existing
//! lock's vault in every non-signer account position, before and after its
//! unlock time.
#![allow(clippy::result_large_err)]
mod common;
use anchor_lang::solana_program::instruction::Instruction;
use common::*;

fn idl() -> serde_json::Value {
    let path = format!("{}/idl/juice_lock.json", env!("CARGO_MANIFEST_DIR"));
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn idl_accounts(ix: &serde_json::Value) -> Vec<String> {
    ix["accounts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["name"].as_str().unwrap().to_string())
        .collect()
}

#[test]
fn inv01_idl_only_lock_and_unlock_can_reach_a_token_program() {
    let v = idl();
    let ixs = v["instructions"].as_array().unwrap();
    let mut names: Vec<&str> = ixs.iter().map(|i| i["name"].as_str().unwrap()).collect();
    names.sort();
    assert_eq!(
        names,
        ["extend", "init_config", "lock", "set_admin", "set_config", "unlock"],
        "the instruction set is fixed"
    );
    for ix in ixs {
        let name = ix["name"].as_str().unwrap();
        let accs = idl_accounts(ix);
        let token = accs.iter().any(|a| a == "token_program");
        let vault = accs.iter().any(|a| a == "vault");
        let touches = matches!(name, "lock" | "unlock");
        assert_eq!(token, touches, "{name}: token program");
        assert_eq!(vault, touches, "{name}: vault");
        match name {
            // Reads nothing but the record.
            "extend" => assert_eq!(accs, ["owner", "lock_record"]),
            // No config, no pool.
            "unlock" => assert_eq!(
                accs,
                ["owner", "lock_record", "vault", "lp_mint", "destination", "token_program"]
            ),
            // Admin instructions never name a lock, a vault or a token account.
            "set_config" => assert_eq!(accs, ["admin", "config"]),
            "set_admin" => assert_eq!(accs, ["admin", "config", "new_admin"]),
            "init_config" => assert!(!accs.iter().any(|a| a.contains("lock") || a.contains("token"))),
            _ => {}
        }
    }
    // Every IDL error is the program's, in order.
    let errs: Vec<u64> = v["errors"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["code"].as_u64().unwrap())
        .collect();
    assert_eq!(errs, (6000..6020).collect::<Vec<u64>>());
}

#[test]
fn inv01_no_instruction_but_unlock_moves_vault_tokens() {
    let mut env = Env::new();
    let p = env.troll;
    let (owner, l) = env.new_lock(&p, 0, 3 * LP);
    let other = env.locker(&p, 10 * LP);
    let (r, lo) = env.lock(&other, &p, 1, LP, 99);
    expect_ok(&r);
    let new_admin = env.wallet(SOL);
    let upgrade = env.upgrade.insecure_clone();
    let admin = env.admin.insecure_clone();

    let check = |env: &Env| {
        assert!(env.exists(&l.vault), "vault closed");
        assert_eq!(env.token_amount(&l.vault), 3 * LP, "vault moved");
        assert!(env.exists(&l.record), "record closed");
    };

    for phase in 0..2 {
        if phase == 1 {
            env.warp(DURATIONS[1] + 1);
        }
        // (instruction, payer, extra signers)
        let cases: Vec<(&str, Instruction, Keypair, Vec<Keypair>)> = vec![
            (
                "init_config",
                env.init_config_ix(&upgrade.pubkey(), &upgrade.pubkey(), admin.pubkey(), env.treasury.pubkey(), FEE),
                upgrade.insecure_clone(),
                vec![],
            ),
            (
                "set_config",
                env.set_config_ix(&admin.pubkey(), env.treasury.pubkey(), FEE, false),
                admin.insecure_clone(),
                vec![],
            ),
            (
                "set_admin",
                env.set_admin_ix(&admin.pubkey(), &new_admin.pubkey()),
                admin.insecure_clone(),
                vec![new_admin.insecure_clone()],
            ),
            (
                "lock",
                env.lock_ix(env.lock_accounts(&other.key(), &other.lp, &p, 1_000 + phase), 0, LP, 1_000 + phase),
                other.kp.insecure_clone(),
                vec![],
            ),
            (
                "extend",
                env.extend_ix(&owner.key(), &l.record, 4),
                owner.kp.insecure_clone(),
                vec![],
            ),
            (
                "extend (other lock)",
                env.extend_ix(&other.key(), &lo.record, 4),
                other.kp.insecure_clone(),
                vec![],
            ),
            (
                "unlock (other lock)",
                env.unlock_ix(env.unlock_accounts(&other.key(), &lo, &other.lp)),
                other.kp.insecure_clone(),
                vec![],
            ),
            (
                "unlock (stranger)",
                env.unlock_ix(env.unlock_accounts(&other.key(), &l, &other.lp)),
                other.kp.insecure_clone(),
                vec![],
            ),
        ];
        let mut tried = 0;
        for (name, ix, payer, extra) in cases {
            for i in 0..ix.accounts.len() {
                if ix.accounts[i].is_signer || ix.accounts[i].pubkey == l.vault {
                    continue;
                }
                let mut bad = ix.clone();
                bad.accounts[i].pubkey = l.vault;
                bad.accounts[i].is_writable = true;
                let extra_refs: Vec<&Keypair> = extra.iter().collect();
                let _ = env.send(&[bad], &payer, &extra_refs);
                check(&env);
                tried += 1;
                let _ = name;
            }
            // And the instruction as built.
            let extra_refs: Vec<&Keypair> = extra.iter().collect();
            let _ = env.send(&[ix], &payer, &extra_refs);
            check(&env);
        }
        assert_eq!(tried, 26, "every non-signer position of every instruction");
        // Unlock by the owner with the vault in every other position.
        let ix = env.unlock_ix(env.unlock_accounts(&owner.key(), &l, &owner.lp));
        for i in 0..ix.accounts.len() {
            if ix.accounts[i].is_signer || ix.accounts[i].pubkey == l.vault {
                continue;
            }
            let mut bad = ix.clone();
            bad.accounts[i].pubkey = l.vault;
            bad.accounts[i].is_writable = true;
            expect_fail(&env.send(&[bad], &owner.kp, &[]));
            check(&env);
        }
    }
    // The owner's unlock is the one way out (the extend above made it 365 days).
    let unlock_ts = env.record(&l).unlock_ts;
    env.set_time(unlock_ts);
    expect_ok(&env.unlock(&owner.kp, &l, &owner.lp));
    assert_eq!(env.token_amount(&owner.lp), 3 * LP);
}

#[test]
fn inv01_a_lock_never_takes_from_an_existing_vault() {
    // `lock` with another lock's vault as its source: the source must be the
    // signer's own token account, and only the record can sign for a vault.
    let mut env = Env::new();
    let p = env.troll;
    let (_, l) = env.new_lock(&p, 0, 2 * LP);
    let who = env.locker(&p, LP);
    let mut a = env.lock_accounts(&who.key(), &who.lp, &p, 3);
    a.owner_token = l.vault;
    let ix = env.lock_ix(a, 0, LP, 3);
    expect_err(&env.send(&[ix], &who.kp, &[]), "WrongTokenOwner");
    assert_eq!(env.token_amount(&l.vault), 2 * LP);
}
