//! Invariant 7: the mint rules (juice_boost's `check_mint`). SPL Token and
//! Token-2022 LP mints are accepted; a freeze authority and every Token-2022
//! extension other than MetadataPointer and TokenMetadata are refused.
#![allow(clippy::result_large_err)]
mod common;
use common::*;

/// Token-2022 extension types (spl-token-2022 `ExtensionType`) and a value
/// length each, for every type a mint can carry or that is unknown (the
/// list juice_boost's mint tests use).
const EXTENSIONS: &[(u16, usize, &str)] = &[
    (1, 108, "TransferFeeConfig"),
    (2, 8, "TransferFeeAmount"),
    (3, 32, "MintCloseAuthority"),
    (4, 65, "ConfidentialTransferMint"),
    (5, 295, "ConfidentialTransferAccount"),
    (6, 1, "DefaultAccountState"),
    (7, 0, "ImmutableOwner"),
    (8, 1, "MemoTransfer"),
    (9, 0, "NonTransferable"),
    (10, 52, "InterestBearingConfig"),
    (11, 1, "CpiGuard"),
    (12, 32, "PermanentDelegate"),
    (13, 0, "NonTransferableAccount"),
    (14, 64, "TransferHook"),
    (15, 1, "TransferHookAccount"),
    (16, 129, "ConfidentialTransferFeeConfig"),
    (17, 32, "ConfidentialTransferFeeAmount"),
    (20, 64, "GroupPointer"),
    (21, 80, "TokenGroup"),
    (22, 64, "GroupMemberPointer"),
    (23, 72, "TokenGroupMember"),
    (24, 196, "ConfidentialMintBurn"),
    (25, 56, "ScaledUiAmount"),
    (26, 33, "Pausable"),
    (27, 1, "PausableAccount"),
    (28, 32, "unknown 28"),
    (999, 16, "unknown 999"),
    (u16::MAX, 4, "unknown 65535"),
];

fn try_lock(env: &mut Env, p: &P) -> TxResult {
    let who = env.locker(p, LP);
    env.lock(&who, p, 0, LP, 1).0
}

/// A synthetic pool whose LP mint (at the PDA, authority the pool) has
/// `exts`, or a freeze authority.
fn pool_with(env: &mut Env, program: Pubkey, freeze: Option<Pubkey>, exts: &[(u16, Vec<u8>)]) -> P {
    let p = env.synthetic_pool(program);
    env.set_mint(p.lp_mint, program, 9, Some(p.pool), freeze, exts);
    p
}

#[test]
fn inv07_recorded_token_2022_lp_mints_accepted() {
    let mut env = Env::new();
    for p in [env.juice_pump, env.troll, env.usdp, env.user] {
        assert_eq!(env.svm.get_account(&p.lp_mint).unwrap().owner, TOKEN_2022);
        expect_ok(&try_lock(&mut env, &p));
    }
}

#[test]
fn inv07_spl_token_lp_mint_accepted() {
    let mut env = Env::new();
    let p = env.synthetic_pool(TOKEN);
    let (_, l) = env.new_lock(&p, 0, LP);
    let r = env.record(&l);
    assert_eq!(r.token_program, TOKEN_PROGRAM_SPL);
    assert_eq!(env.svm.get_account(&l.vault).unwrap().data.len(), 165);
}

#[test]
fn inv07_metadata_extensions_accepted() {
    let mut env = Env::new();
    for exts in [
        vec![(18u16, vec![0u8; 64])],
        vec![(19, vec![7; 120])],
        vec![(18, vec![0; 64]), (19, vec![1; 90])],
    ] {
        let p = pool_with(&mut env, TOKEN_2022, None, &exts);
        let (_, l) = env.new_lock(&p, 0, LP);
        // Metadata needs no room in a token account: the vault stays 165 bytes.
        assert_eq!(env.svm.get_account(&l.vault).unwrap().data.len(), 165);
    }
}

#[test]
fn inv07_freeze_authority_refused() {
    let mut env = Env::new();
    for program in [TOKEN, TOKEN_2022] {
        let p = pool_with(&mut env, program, Some(Pubkey::new_unique()), &[]);
        expect_err(&try_lock(&mut env, &p), "MintNotAllowed");
        // The pool is fine; only the freeze authority is refused.
        let p = pool_with(&mut env, program, None, &[]);
        expect_ok(&try_lock(&mut env, &p));
    }
}

#[test]
fn inv07_every_refused_extension_refused() {
    let mut env = Env::new();
    for (ty, len, name) in EXTENSIONS {
        let p = pool_with(&mut env, TOKEN_2022, None, &[(*ty, vec![0; *len])]);
        let r = try_lock(&mut env, &p);
        assert_eq!(err_name(&r), "MintNotAllowed", "{name}\n{}", logs(&r));
        // With an accepted extension in front, still refused.
        let p = pool_with(
            &mut env,
            TOKEN_2022,
            None,
            &[(18, vec![0; 64]), (*ty, vec![0; *len])],
        );
        let r = try_lock(&mut env, &p);
        assert_eq!(err_name(&r), "MintNotAllowed", "{name} after MetadataPointer");
    }
}

#[test]
fn inv07_malformed_mints_refused() {
    let mut env = Env::new();
    // Uninitialized.
    let p = env.synthetic_pool(TOKEN_2022);
    let mut d = mint_data(9, Some(p.pool), None, &[]);
    d[45] = 0;
    env.svm.set_account(p.lp_mint, account(TOKEN_2022, d)).unwrap();
    expect_err(&try_lock(&mut env, &p), "MintNotAllowed");
    // Extensions on an SPL Token mint (longer than 82 bytes).
    let p = env.synthetic_pool(TOKEN);
    let d = mint_data(9, Some(p.pool), None, &[(18, vec![0; 64])]);
    env.svm.set_account(p.lp_mint, account(TOKEN, d)).unwrap();
    expect_err(&try_lock(&mut env, &p), "MintNotAllowed");
    // A Token-2022 extension entry running past the end.
    let p = env.synthetic_pool(TOKEN_2022);
    let mut d = mint_data(9, Some(p.pool), None, &[(18, vec![0; 64])]);
    d.truncate(d.len() - 1);
    env.svm.set_account(p.lp_mint, account(TOKEN_2022, d)).unwrap();
    expect_err(&try_lock(&mut env, &p), "MintNotAllowed");
    // A "mint" owned by neither token program.
    let p = env.synthetic_pool(TOKEN_2022);
    let d = mint_data(9, Some(p.pool), None, &[]);
    env.svm.set_account(p.lp_mint, account(Pubkey::new_unique(), d)).unwrap();
    expect_fail(&try_lock(&mut env, &p));
}
