//! Invariant 13: the layout is frozen (plan section 1.1). Account sizes and
//! byte offsets, seeds, discriminators, error numbers, durations and bounds.
//! The program is immutable after the revoke, and phase 2 (juice_boost lock
//! positions) reads `LockRecord` at these offsets, so nothing here may change.
use anchor_lang::prelude::Pubkey;
use anchor_lang::{AccountSerialize, Discriminator, Space};
use juice_lock::error::LockError;
use juice_lock::state::*;
use sha2::{Digest, Sha256};

fn disc(prefix: &str, name: &str) -> [u8; 8] {
    Sha256::digest(format!("{prefix}:{name}").as_bytes())[..8]
        .try_into()
        .unwrap()
}

fn bytes<T: AccountSerialize>(v: &T) -> Vec<u8> {
    let mut out = vec![];
    v.try_serialize(&mut out).unwrap();
    out
}

fn k(b: u8) -> Pubkey {
    Pubkey::new_from_array([b; 32])
}

#[test]
fn inv13_program_id_per_build() {
    let want = if cfg!(feature = "devnet") {
        "A6uz54MqeiS3VW6GTpKMUswyiTPAzaNpfnRZvANLr9sU"
    } else {
        // Default and `mainnet-test` builds: .secrets/mainnet/lock-program.json.
        "544vBp5c5GHJwNDm4YmS4FK24to9YP17iJLSXmcjFtmG"
    };
    assert_eq!(juice_lock::ID.to_string(), want);
}

#[test]
fn inv13_account_sizes() {
    assert_eq!(8 + LockConfig::INIT_SPACE, 88);
    assert_eq!(8 + LockRecord::INIT_SPACE, 192);
}

#[test]
fn inv13_config_offsets() {
    let c = LockConfig {
        version: 1,
        bump: 0xfe,
        paused_new: true,
        _pad: [0; 5],
        admin: k(0x11),
        treasury: k(0x22),
        fee_lamports: 0x0102_0304_0506_0708,
    };
    let b = bytes(&c);
    assert_eq!(b.len(), 88);
    assert_eq!(b[..8], LockConfig::DISCRIMINATOR[..]);
    assert_eq!(b[8], 1);
    assert_eq!(b[9], 0xfe);
    assert_eq!(b[10], 1);
    assert_eq!(b[11..16], [0; 5]);
    assert_eq!(b[16..48], [0x11; 32]);
    assert_eq!(b[48..80], [0x22; 32]);
    assert_eq!(b[80..88], 0x0102_0304_0506_0708u64.to_le_bytes());
}

#[test]
fn inv13_record_offsets() {
    let r = LockRecord {
        version: 1,
        bump: 0xfd,
        vault_bump: 0xfc,
        tier: 5,
        forever: true,
        lp_decimals: 9,
        token_program: 1,
        _pad: 0,
        owner: k(0x31),
        lp_mint: k(0x32),
        pump_pool: k(0x33),
        vault: k(0x34),
        amount: 0x2122_2324_2526_2728,
        start_ts: 0x3132_3334_3536_3738,
        unlock_ts: i64::MAX,
        nonce: 0x4142_4344_4546_4748,
        extended_at: 0x5152_5354_5556_5758,
        fee_paid: 0x6162_6364_6566_6768,
    };
    let b = bytes(&r);
    assert_eq!(b.len(), 192);
    assert_eq!(b[..8], LockRecord::DISCRIMINATOR[..]);
    assert_eq!(b[8..16], [1, 0xfd, 0xfc, 5, 1, 9, 1, 0]);
    assert_eq!(b[16..48], [0x31; 32]);
    assert_eq!(b[48..80], [0x32; 32]);
    assert_eq!(b[80..112], [0x33; 32]);
    assert_eq!(b[112..144], [0x34; 32]);
    assert_eq!(b[144..152], 0x2122_2324_2526_2728u64.to_le_bytes());
    assert_eq!(b[152..160], 0x3132_3334_3536_3738i64.to_le_bytes());
    assert_eq!(b[160..168], i64::MAX.to_le_bytes());
    assert_eq!(b[168..176], 0x4142_4344_4546_4748u64.to_le_bytes());
    assert_eq!(b[176..184], 0x5152_5354_5556_5758i64.to_le_bytes());
    assert_eq!(b[184..192], 0x6162_6364_6566_6768u64.to_le_bytes());
}

#[test]
fn inv13_seeds() {
    assert_eq!(CONFIG_SEED, b"lock_config");
    assert_eq!(LOCK_SEED, b"lock");
    assert_eq!(VAULT_SEED, b"vault");
    assert_eq!(VERSION, 1);
}

#[test]
fn inv13_discriminators() {
    assert_eq!(LockConfig::DISCRIMINATOR, disc("account", "LockConfig"));
    assert_eq!(LockRecord::DISCRIMINATOR, disc("account", "LockRecord"));
    use juice_lock::instruction as ix;
    for (name, d) in [
        ("init_config", ix::InitConfig::DISCRIMINATOR),
        ("set_config", ix::SetConfig::DISCRIMINATOR),
        ("set_admin", ix::SetAdmin::DISCRIMINATOR),
        ("lock", ix::Lock::DISCRIMINATOR),
        ("extend", ix::Extend::DISCRIMINATOR),
        ("unlock", ix::Unlock::DISCRIMINATOR),
    ] {
        assert_eq!(d, disc("global", name), "{name}");
    }
    use juice_lock::events as ev;
    for (name, d) in [
        ("ConfigInitialized", ev::ConfigInitialized::DISCRIMINATOR),
        ("ConfigChanged", ev::ConfigChanged::DISCRIMINATOR),
        ("AdminChanged", ev::AdminChanged::DISCRIMINATOR),
        ("Locked", ev::Locked::DISCRIMINATOR),
        ("Extended", ev::Extended::DISCRIMINATOR),
        ("Unlocked", ev::Unlocked::DISCRIMINATOR),
    ] {
        assert_eq!(d, disc("event", name), "{name}");
    }
}

#[test]
fn inv13_error_numbers() {
    let all = [
        (LockError::NotUpgradeAuthority, 6000),
        (LockError::NotAdmin, 6001),
        (LockError::Paused, 6002),
        (LockError::BadVersion, 6003),
        (LockError::InvalidConfigValue, 6004),
        (LockError::MintNotAllowed, 6005),
        (LockError::PumpPoolMismatch, 6006),
        (LockError::WrongTokenProgram, 6007),
        (LockError::WrongTokenOwner, 6008),
        (LockError::WrongMint, 6009),
        (LockError::WrongTreasury, 6010),
        (LockError::ZeroAmount, 6011),
        (LockError::InvalidTier, 6012),
        (LockError::NotOwner, 6013),
        (LockError::StillLocked, 6014),
        (LockError::ForeverLock, 6015),
        (LockError::NotLater, 6016),
        (LockError::WrongVault, 6017),
        (LockError::MathOverflow, 6018),
        (LockError::FeeTooHigh, 6019),
    ];
    for (e, n) in all {
        assert_eq!(u32::from(e), n);
    }
}

#[test]
fn inv13_durations_and_bounds() {
    let days = [7i64, 30, 90, 180, 365];
    for (t, d) in days.iter().enumerate() {
        let real = d * 86_400;
        if cfg!(feature = "devnet") {
            assert_eq!(DURATIONS[t] * 1_440, real, "tier {t}");
        } else if cfg!(feature = "mainnet-test") && t == 0 {
            assert_eq!(DURATIONS[t], 86_400, "tier 0 of the mainnet-test build: 1 day");
        } else {
            assert_eq!(DURATIONS[t], real, "tier {t}");
        }
    }
    assert_eq!(FOREVER_TIER, 5);
    assert_eq!(NUM_TIERS, 6);
    assert_eq!(MAX_FEE_LAMPORTS, 50_000_000);
    assert_eq!(TOKEN_PROGRAM_SPL, 0);
    assert_eq!(TOKEN_PROGRAM_2022, 1);
    assert_eq!(juice_lock::math::FOREVER_TS, i64::MAX);
    assert_eq!(
        juice_lock::pump::POOL_DISCRIMINATOR,
        [241, 154, 109, 4, 17, 177, 109, 188]
    );
    assert_eq!(
        juice_lock::pump::PUMP_AMM_ID.to_string(),
        "pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA"
    );
}

/// The MAINNET-TEST build (`--features mainnet-test`) is the FINAL build with
/// "1 week" = 1 day and nothing else. Two halves: (1) in this build, the
/// duration table equals FINAL's (or TEST's) entry by entry, so across the
/// default and mainnet-test runs of `./test-lock.sh` only tier 0 differs;
/// (2) in the sources, the feature gates exactly the two `TIER0_SECS`
/// constants (and the compile error that keeps it apart from `devnet`), so it
/// can't change anything else: not the id, a layout, a bound or a check.
#[test]
fn inv13_mainnet_test_changes_only_the_1_week_tier() {
    const FINAL: [i64; 5] = [604_800, 2_592_000, 7_776_000, 15_552_000, 31_536_000];
    const TEST: [i64; 5] = [86_400, 2_592_000, 7_776_000, 15_552_000, 31_536_000];
    assert_eq!(FINAL[1..], TEST[1..]);
    assert_ne!(FINAL[0], TEST[0]);
    if !cfg!(feature = "devnet") {
        let want = if cfg!(feature = "mainnet-test") { TEST } else { FINAL };
        assert_eq!(DURATIONS, want);
        assert_eq!(TIER0_SECS, want[0]);
    }

    let src = concat!(env!("CARGO_MANIFEST_DIR"), "/src");
    let mut files = vec![];
    for dir in [src.to_string(), format!("{src}/handlers")] {
        for e in std::fs::read_dir(&dir).unwrap() {
            let p = e.unwrap().path();
            if p.extension().is_some_and(|x| x == "rs") {
                files.push(p);
            }
        }
    }
    // Every line naming the feature, with the item it gates (the next line).
    let mut gates = vec![];
    for p in &files {
        let text = std::fs::read_to_string(p).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        for (i, l) in lines.iter().enumerate() {
            if l.contains("feature = \"mainnet-test\"") {
                let name = p.file_name().unwrap().to_string_lossy().into_owned();
                gates.push((name, l.trim().to_string(), lines[i + 1].trim().to_string()));
            }
        }
    }
    gates.sort();
    let g = |f: &str, a: &str, b: &str| (f.to_string(), a.to_string(), b.to_string());
    assert_eq!(
        gates,
        vec![
            g(
                "lib.rs",
                "#[cfg(all(feature = \"devnet\", feature = \"mainnet-test\"))]",
                "compile_error!(\"the features `devnet` and `mainnet-test` exclude each other\");"
            ),
            g("state.rs", "#[cfg(feature = \"mainnet-test\")]", "pub const TIER0_SECS: i64 = 86_400;"),
            g("state.rs", "#[cfg(not(feature = \"mainnet-test\"))]", "pub const TIER0_SECS: i64 = 604_800;"),
        ],
        "the mainnet-test feature must gate only TIER0_SECS"
    );
    // And TIER0_SECS feeds only the first entry of the mainnet duration table.
    let state = std::fs::read_to_string(format!("{src}/state.rs")).unwrap();
    assert!(state.contains(
        "pub const DURATIONS: [i64; 5] = [TIER0_SECS, 2_592_000, 7_776_000, 15_552_000, 31_536_000];"
    ));
    for p in &files {
        let uses = std::fs::read_to_string(p).unwrap().matches("TIER0_SECS").count();
        let want = if p.ends_with("state.rs") { 3 } else { 0 };
        assert_eq!(uses, want, "{} names TIER0_SECS {uses} times", p.display());
    }
}
