//! Test harness for `juice_lock`: LiteSVM with mainnet's Token-2022 binary,
//! mainnet's rent (5,080 lamports per byte-year, threshold 1.0, from the
//! recorded Rent sysvar), and recorded public mainnet accounts
//! (tests/fixtures/mainnet.json: four PumpSwap pools and their LP mints, the
//! same bytes juice_boost records). Recorded accounts are never patched;
//! tests that need a wrong mint or pool build their own account at another
//! address. Wallets and token accounts are synthetic: no customer wallet or
//! transaction is a fixture.
//!
//! The LP mints' authority is their PumpSwap pool, so test wallets get LP as
//! token accounts written straight into the bank.
#![allow(dead_code, clippy::result_large_err, clippy::too_many_arguments)]

use anchor_lang::solana_program::instruction::{AccountMeta, Instruction};
pub use anchor_lang::{prelude::Pubkey, InstructionData, ToAccountMetas};
use anchor_lang::{AccountDeserialize, AnchorDeserialize, Discriminator};
use base64::Engine;
pub use juice_lock::state::*;
use litesvm::types::{FailedTransactionMetadata, TransactionMetadata};
use litesvm::LiteSVM;
use sha2::{Digest, Sha256};
pub use solana_account::Account;
pub use solana_keypair::Keypair;
use solana_message::{Message, VersionedMessage};
pub use solana_signer::Signer;
use solana_transaction::versioned::VersionedTransaction;
use std::str::FromStr;

pub const LOCK: Pubkey = juice_lock::ID;
pub const PUMP_AMM: Pubkey = juice_lock::pump::PUMP_AMM_ID;
pub const TOKEN: Pubkey = juice_lock::pump::TOKEN_ID;
pub const TOKEN_2022: Pubkey = juice_lock::pump::TOKEN_2022_ID;
pub const SYSTEM: Pubkey = anchor_lang::solana_program::system_program::ID;
pub const NOW: i64 = 1_790_000_000;
/// The program ids of the builds: mainnet (default and `mainnet-test`,
/// .secrets/mainnet/lock-program.json) and devnet.
pub const MAINNET_PROGRAM_ID: Pubkey =
    anchor_lang::pubkey!("544vBp5c5GHJwNDm4YmS4FK24to9YP17iJLSXmcjFtmG");
pub const DEVNET_PROGRAM_ID: Pubkey =
    anchor_lang::pubkey!("A6uz54MqeiS3VW6GTpKMUswyiTPAzaNpfnRZvANLr9sU");
/// The id the default build carried before the mainnet key pair existed
/// (sha256 of "juice_lock mainnet program id placeholder"); in no binary.
pub const PLACEHOLDER_PROGRAM_ID: Pubkey =
    anchor_lang::pubkey!("BbHMWyGpX1FBxcV31yMeVA4CHqXvDvu91cuyDFrVGfqK");
/// The duration table of the FINAL (default) and the MAINNET-TEST build.
pub const FINAL_DURATIONS: [i64; 5] = [604_800, 2_592_000, 7_776_000, 15_552_000, 31_536_000];
pub const TEST_DURATIONS: [i64; 5] = [86_400, 2_592_000, 7_776_000, 15_552_000, 31_536_000];
/// The test-only CPI owner program (test-programs/lock-cpi-owner).
pub const CPI_OWNER: Pubkey =
    anchor_lang::pubkey!("DHuNSqgQeoBt6P2ZdFLL5ipWP2hzCCqd5D6fVof3UmLQ");
pub const SOL: u64 = 1_000_000_000;
/// One LP token in base units (9 decimals).
pub const LP: u64 = 1_000_000_000;
/// The launch fee the owner decided (0.01 SOL).
pub const FEE: u64 = 10_000_000;
pub const RENT_SYSVAR: &str = "SysvarRent111111111111111111111111111111111";

pub type TxResult = Result<TransactionMetadata, FailedTransactionMetadata>;

pub fn pk(s: &str) -> Pubkey {
    Pubkey::from_str(s).unwrap()
}

// ---------------------------------------------------------------- fixtures

pub struct Fixture {
    pub v: serde_json::Value,
    pub slot: u64,
    pub rent_lamports_per_byte_year: u64,
    pub rent_exemption_threshold: f64,
    pub token_2022_sha256: String,
    pub accounts: Vec<(Pubkey, Account)>,
}

impl Fixture {
    pub fn pool(&self, name: &str) -> Pubkey {
        pk(self.v["pools"][name].as_str().unwrap())
    }
    pub fn lp_mint(&self, name: &str) -> Pubkey {
        pk(self.v["lpMints"][name].as_str().unwrap())
    }
}

pub fn fixture() -> Fixture {
    let path = format!("{}/tests/fixtures/mainnet.json", env!("CARGO_MANIFEST_DIR"));
    let v: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let accounts = v["accounts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| {
            (
                pk(a["key"].as_str().unwrap()),
                Account {
                    lamports: a["lamports"].as_u64().unwrap(),
                    data: base64::engine::general_purpose::STANDARD
                        .decode(a["data"].as_str().unwrap())
                        .unwrap(),
                    owner: pk(a["owner"].as_str().unwrap()),
                    executable: false,
                    rent_epoch: 0,
                },
            )
        })
        .collect();
    Fixture {
        slot: v["recordedAtSlot"].as_u64().unwrap(),
        rent_lamports_per_byte_year: v["rentSysvar"]["lamportsPerByteYear"]
            .as_str()
            .unwrap()
            .parse()
            .unwrap(),
        rent_exemption_threshold: v["rentSysvar"]["exemptionThreshold"].as_f64().unwrap(),
        token_2022_sha256: v["token2022"]["sha256"].as_str().unwrap().to_string(),
        accounts,
        v,
    }
}

fn sha256_hex(b: &[u8]) -> String {
    Sha256::digest(b)
        .iter()
        .map(|x| format!("{x:02x}"))
        .collect()
}

/// Refuse a program binary older than its sources (`cargo test` doesn't rebuild the .so).
fn assert_program_fresh(so: &str, sources_dir: &str) {
    let built = std::fs::metadata(so)
        .and_then(|m| m.modified())
        .unwrap_or_else(|_| panic!("{so} missing: run ./test-lock.sh (or cargo build-sbf) first"));
    let mut sources = vec![format!("{sources_dir}/Cargo.toml")];
    for dir in ["src", "src/handlers"] {
        let Ok(rd) = std::fs::read_dir(format!("{sources_dir}/{dir}")) else {
            continue;
        };
        for e in rd {
            let p = e.unwrap().path();
            if p.is_file() {
                sources.push(p.to_string_lossy().into_owned());
            }
        }
    }
    for src in sources {
        let changed = std::fs::metadata(&src).and_then(|m| m.modified()).unwrap();
        assert!(
            changed <= built,
            "stale program binary: {src} is newer than {so}. Run ./test-lock.sh first."
        );
    }
}

/// The program binary, refused when stale or of the other build.
pub fn program_bytes() -> Vec<u8> {
    let root = env!("CARGO_MANIFEST_DIR");
    let so = format!("{root}/../../target/deploy/juice_lock.so");
    assert_program_fresh(&so, root);
    let bytes = std::fs::read(&so).unwrap();
    // The binary must be the build these tests are for (devnet feature or
    // not): its program id appears in it, the other build's doesn't.
    let other = if cfg!(feature = "devnet") {
        MAINNET_PROGRAM_ID
    } else {
        DEVNET_PROGRAM_ID
    };
    assert!(
        bytes.windows(32).any(|w| w == LOCK.as_ref())
            && !bytes.windows(32).any(|w| w == other.as_ref())
            && !bytes.windows(32).any(|w| w == PLACEHOLDER_PROGRAM_ID.as_ref()),
        "{so} is not the {} build (program id {LOCK} not in it, or {other} or the placeholder in it): rebuild with ./test-lock.sh",
        if cfg!(feature = "devnet") { "devnet" } else { "mainnet" }
    );
    // FINAL and MAINNET-TEST share the program id; their duration tables
    // (state::DURATIONS, kept whole in the binary's read-only data) tell them
    // apart: this build's table is in it, the other mainnet build's isn't.
    if !cfg!(feature = "devnet") {
        let table = |d: &[i64; 5]| d.iter().flat_map(|x| x.to_le_bytes()).collect::<Vec<u8>>();
        let (mine, theirs) = if cfg!(feature = "mainnet-test") {
            (table(&TEST_DURATIONS), table(&FINAL_DURATIONS))
        } else {
            (table(&FINAL_DURATIONS), table(&TEST_DURATIONS))
        };
        assert!(
            bytes.windows(40).any(|w| w == mine.as_slice())
                && !bytes.windows(40).any(|w| w == theirs.as_slice()),
            "{so} is not the {} build (its duration table): rebuild with ./test-lock.sh",
            if cfg!(feature = "mainnet-test") { "mainnet-test" } else { "default" }
        );
    }
    bytes
}

// ---------------------------------------------------------------- PDAs

pub fn config_pda() -> Pubkey {
    Pubkey::find_program_address(&[CONFIG_SEED], &LOCK).0
}
pub fn lock_pda(owner: &Pubkey, lp_mint: &Pubkey, nonce: u64) -> Pubkey {
    Pubkey::find_program_address(
        &[LOCK_SEED, owner.as_ref(), lp_mint.as_ref(), &nonce.to_le_bytes()],
        &LOCK,
    )
    .0
}
pub fn vault_pda(record: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[VAULT_SEED, record.as_ref()], &LOCK).0
}
pub fn program_data_pda() -> Pubkey {
    Pubkey::find_program_address(
        &[LOCK.as_ref()],
        &anchor_lang::solana_program::bpf_loader_upgradeable::ID,
    )
    .0
}
/// The CPI owner program's owner PDA.
pub fn cpi_owner_pda() -> Pubkey {
    Pubkey::find_program_address(&[b"owner"], &CPI_OWNER).0
}

/// A PumpSwap pool and its LP mint, with the mint's token program.
#[derive(Clone, Copy, Debug)]
pub struct P {
    pub pool: Pubkey,
    pub lp_mint: Pubkey,
    pub program: Pubkey,
}

/// A wallet holding LP of one pool.
pub struct Locker {
    pub kp: Keypair,
    pub lp: Pubkey,
}

impl Locker {
    pub fn key(&self) -> Pubkey {
        self.kp.pubkey()
    }
}

/// One lock's keys.
#[derive(Clone, Copy, Debug)]
pub struct L {
    pub record: Pubkey,
    pub vault: Pubkey,
    pub owner: Pubkey,
    pub pool: P,
    pub nonce: u64,
}

// ---------------------------------------------------------------- environment

pub struct Env {
    pub svm: LiteSVM,
    pub fx: Fixture,
    /// The program's upgrade authority (the deployer).
    pub upgrade: Keypair,
    /// The `lock-admin` stored in the config.
    pub admin: Keypair,
    pub treasury: Keypair,
    /// Canonical, quoted in PUMP (JUICE/PUMP).
    pub juice_pump: P,
    /// Canonical, quoted in WSOL (TROLL/SOL).
    pub troll: P,
    /// Canonical, quoted in USDC (USDP/USDC).
    pub usdp: P,
    /// User-created (USDC/SOL).
    pub user: P,
}

impl Env {
    /// Program deployed, config initialised (0.01 SOL fee), no lock.
    pub fn new() -> Env {
        let mut env = Env::new_uninit();
        expect_ok(&env.init_config(FEE));
        env
    }

    /// Program deployed (upgrade authority = `upgrade`), recorded accounts loaded, no config yet.
    pub fn new_uninit() -> Env {
        let fx = fixture();
        let mut svm = LiteSVM::new();
        // Mainnet's Token-2022, not the build LiteSVM ships.
        let path = format!(
            "{}/tests/fixtures/token_2022.so",
            env!("CARGO_MANIFEST_DIR")
        );
        let t22 = std::fs::read(&path)
            .unwrap_or_else(|_| panic!("{path} missing: run scripts/dump-lock-programs.sh"));
        assert_eq!(
            sha256_hex(&t22),
            fx.token_2022_sha256,
            "token_2022.so is not the recorded mainnet binary"
        );
        svm.add_program(TOKEN_2022, &t22).unwrap();
        svm.add_program(LOCK, &program_bytes()).unwrap();
        for (k, a) in &fx.accounts {
            // The Rent sysvar is set from its recorded values below.
            if *k != pk(RENT_SYSVAR) {
                svm.set_account(*k, a.clone()).unwrap();
            }
        }
        let mut rent: anchor_lang::prelude::Rent = svm.get_sysvar();
        #[allow(deprecated)]
        {
            rent.lamports_per_byte_year = fx.rent_lamports_per_byte_year;
            rent.exemption_threshold = fx.rent_exemption_threshold;
        }
        svm.set_sysvar(&rent);
        let mut clock: anchor_lang::prelude::Clock = svm.get_sysvar();
        clock.slot = fx.slot;
        clock.unix_timestamp = NOW;
        svm.set_sysvar(&clock);
        let p = |name: &str| P {
            pool: fx.pool(name),
            lp_mint: fx.lp_mint(name),
            program: TOKEN_2022,
        };
        let (juice_pump, troll, usdp, user) =
            (p("juicePump"), p("trollSol"), p("usdpUsdc"), p("usdcSolUser"));
        let mut env = Env {
            svm,
            fx,
            upgrade: Keypair::new(),
            admin: Keypair::new(),
            treasury: Keypair::new(),
            juice_pump,
            troll,
            usdp,
            user,
        };
        for k in [env.upgrade.pubkey(), env.admin.pubkey()] {
            env.svm.airdrop(&k, 10 * SOL).unwrap();
        }
        let a = env.upgrade.pubkey();
        env.set_upgrade_authority(Some(a));
        env
    }

    /// Rewrites the ProgramData upgrade authority (LiteSVM deploys with none).
    pub fn set_upgrade_authority(&mut self, authority: Option<Pubkey>) {
        let pd = program_data_pda();
        let mut a = self.svm.get_account(&pd).unwrap();
        match authority {
            Some(k) => {
                a.data[12] = 1;
                a.data[13..45].copy_from_slice(k.as_ref());
            }
            None => {
                a.data[12] = 0;
                a.data[13..45].fill(0);
            }
        }
        self.svm.set_account(pd, a).unwrap();
    }

    /// Loads the test-only CPI owner program (built by ./test-lock.sh).
    pub fn load_cpi_owner(&mut self) {
        let root = env!("CARGO_MANIFEST_DIR");
        let so = format!("{root}/../../target/deploy/lock_cpi_owner.so");
        assert_program_fresh(&so, &format!("{root}/../../test-programs/lock-cpi-owner"));
        let bytes = std::fs::read(&so).unwrap();
        self.svm.add_program(CPI_OWNER, &bytes).unwrap();
    }

    // ------------------------------------------------------------ plumbing

    pub fn wallet(&mut self, sol: u64) -> Keypair {
        let k = Keypair::new();
        self.svm.airdrop(&k.pubkey(), sol).unwrap();
        k
    }

    pub fn send(&mut self, ixs: &[Instruction], payer: &Keypair, extra: &[&Keypair]) -> TxResult {
        self.svm.expire_blockhash();
        let bh = self.svm.latest_blockhash();
        let msg = Message::new_with_blockhash(ixs, Some(&payer.pubkey()), &bh);
        let mut signers = vec![payer];
        signers.extend_from_slice(extra);
        let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &signers).unwrap();
        self.svm.send_transaction(tx)
    }

    pub fn warp(&mut self, secs: i64) {
        let mut clock: anchor_lang::prelude::Clock = self.svm.get_sysvar();
        clock.slot += (secs.max(1) as u64).min(1 << 40);
        clock.unix_timestamp = clock.unix_timestamp.checked_add(secs).unwrap();
        self.svm.set_sysvar(&clock);
    }
    pub fn set_time(&mut self, ts: i64) {
        let mut clock: anchor_lang::prelude::Clock = self.svm.get_sysvar();
        clock.slot += 1;
        clock.unix_timestamp = ts;
        self.svm.set_sysvar(&clock);
    }
    pub fn now(&self) -> i64 {
        self.svm
            .get_sysvar::<anchor_lang::prelude::Clock>()
            .unix_timestamp
    }
    pub fn rent(&self, len: usize) -> u64 {
        self.svm.minimum_balance_for_rent_exemption(len)
    }

    // ------------------------------------------------------------ mints, pools, token accounts

    /// A mint at `key` under `program`: `decimals`, supply 1e15, optional
    /// mint authority, optional freeze authority, and (Token-2022) extensions
    /// as (type, value bytes).
    pub fn set_mint(
        &mut self,
        key: Pubkey,
        program: Pubkey,
        decimals: u8,
        authority: Option<Pubkey>,
        freeze: Option<Pubkey>,
        exts: &[(u16, Vec<u8>)],
    ) {
        let data = mint_data(decimals, authority, freeze, exts);
        let lamports = self.rent(data.len());
        self.svm
            .set_account(
                key,
                Account {
                    lamports,
                    data,
                    owner: program,
                    executable: false,
                    rent_epoch: 0,
                },
            )
            .unwrap();
    }

    /// A synthetic, well-formed PumpSwap pool (discriminator, 301 bytes) with
    /// its LP mint at PumpSwap's PDA under `program`, mint authority the pool
    /// (what PumpSwap itself would create), 9 decimals, no freeze authority,
    /// with `exts` (Token-2022 only).
    pub fn synthetic_pool_with(&mut self, program: Pubkey, exts: &[(u16, Vec<u8>)]) -> P {
        let pool = Pubkey::new_unique();
        let lp_mint = lp_mint_pda(&pool);
        let mut d = vec![0u8; 301];
        d[..8].copy_from_slice(&juice_lock::pump::POOL_DISCRIMINATOR);
        self.svm.set_account(pool, account(PUMP_AMM, d)).unwrap();
        self.set_mint(lp_mint, program, 9, Some(pool), None, exts);
        P {
            pool,
            lp_mint,
            program,
        }
    }
    pub fn synthetic_pool(&mut self, program: Pubkey) -> P {
        self.synthetic_pool_with(program, &[])
    }

    /// A plain 165-byte token account of `mint` for `owner` holding `amount`,
    /// at a fresh address, under the mint's own token program.
    pub fn token_account(&mut self, owner: &Pubkey, mint: &Pubkey, amount: u64) -> Pubkey {
        let key = Pubkey::new_unique();
        self.set_token_account(key, owner, mint, amount);
        key
    }
    pub fn set_token_account(&mut self, key: Pubkey, owner: &Pubkey, mint: &Pubkey, amount: u64) {
        let program = self.svm.get_account(mint).expect("mint").owner;
        let mut data = vec![0u8; 165];
        data[0..32].copy_from_slice(mint.as_ref());
        data[32..64].copy_from_slice(owner.as_ref());
        data[64..72].copy_from_slice(&amount.to_le_bytes());
        data[108] = 1; // initialized
        let lamports = self.rent(165);
        self.svm
            .set_account(
                key,
                Account {
                    lamports,
                    data,
                    owner: program,
                    executable: false,
                    rent_epoch: 0,
                },
            )
            .unwrap();
    }

    /// A funded wallet (10 SOL) with `amount` of the pool's LP.
    pub fn locker(&mut self, p: &P, amount: u64) -> Locker {
        let kp = self.wallet(10 * SOL);
        let lp = self.token_account(&kp.pubkey(), &p.lp_mint, amount);
        Locker { kp, lp }
    }

    pub fn token_amount(&self, k: &Pubkey) -> u64 {
        let d = self.data(k);
        u64::from_le_bytes(d[64..72].try_into().unwrap())
    }
    /// A plain token transfer by the owner (not through the program): a donation.
    pub fn raw_transfer(
        &mut self,
        from: &Pubkey,
        to: &Pubkey,
        mint: &Pubkey,
        owner: &Keypair,
        amount: u64,
        decimals: u8,
    ) -> TxResult {
        let mut data = vec![12u8]; // TransferChecked
        data.extend_from_slice(&amount.to_le_bytes());
        data.push(decimals);
        let program = self.svm.get_account(mint).unwrap().owner;
        let ix = Instruction {
            program_id: program,
            accounts: vec![
                AccountMeta::new(*from, false),
                AccountMeta::new_readonly(*mint, false),
                AccountMeta::new(*to, false),
                AccountMeta::new_readonly(owner.pubkey(), true),
            ],
            data,
        };
        self.send(&[ix], owner, &[])
    }
    /// Someone else sends `amount` LP of the lock's mint into its vault.
    pub fn donate(&mut self, l: &L, amount: u64) {
        let d = self.locker(&l.pool, amount);
        expect_ok(&self.raw_transfer(&d.lp, &l.vault, &l.pool.lp_mint, &d.kp, amount, 9));
    }

    // ------------------------------------------------------------ reads

    pub fn data(&self, k: &Pubkey) -> Vec<u8> {
        self.svm.get_account(k).map(|a| a.data).unwrap_or_default()
    }
    pub fn exists(&self, k: &Pubkey) -> bool {
        self.svm
            .get_account(k)
            .map(|a| a.lamports > 0)
            .unwrap_or(false)
    }
    pub fn lamports(&self, k: &Pubkey) -> u64 {
        self.svm.get_account(k).map(|a| a.lamports).unwrap_or(0)
    }
    pub fn config(&self) -> LockConfig {
        LockConfig::try_deserialize(&mut self.data(&config_pda()).as_slice()).unwrap()
    }
    pub fn record(&self, l: &L) -> LockRecord {
        LockRecord::try_deserialize(&mut self.data(&l.record).as_slice()).unwrap()
    }
    /// Writes a changed account back (a test's way to model a doctored state).
    pub fn write<T: anchor_lang::AccountSerialize>(&mut self, k: &Pubkey, v: &T) {
        let mut a = self.svm.get_account(k).unwrap();
        let mut out = Vec::new();
        v.try_serialize(&mut out).unwrap();
        a.data[..out.len()].copy_from_slice(&out);
        self.svm.set_account(*k, a).unwrap();
    }

    // ------------------------------------------------------------ admin instructions

    pub fn init_config_ix(
        &self,
        authority: &Pubkey,
        payer: &Pubkey,
        admin: Pubkey,
        treasury: Pubkey,
        fee: u64,
    ) -> Instruction {
        Instruction {
            program_id: LOCK,
            accounts: juice_lock::accounts::InitConfig {
                authority: *authority,
                program: LOCK,
                program_data: program_data_pda(),
                payer: *payer,
                config: config_pda(),
                system_program: SYSTEM,
            }
            .to_account_metas(None),
            data: juice_lock::instruction::InitConfig {
                admin,
                treasury,
                fee_lamports: fee,
            }
            .data(),
        }
    }
    pub fn init_config(&mut self, fee: u64) -> TxResult {
        let u = self.upgrade.insecure_clone();
        let ix = self.init_config_ix(
            &u.pubkey(),
            &u.pubkey(),
            self.admin.pubkey(),
            self.treasury.pubkey(),
            fee,
        );
        self.send(&[ix], &u, &[])
    }
    pub fn set_config_ix(&self, admin: &Pubkey, treasury: Pubkey, fee: u64, paused: bool) -> Instruction {
        Instruction {
            program_id: LOCK,
            accounts: juice_lock::accounts::SetConfig {
                admin: *admin,
                config: config_pda(),
            }
            .to_account_metas(None),
            data: juice_lock::instruction::SetConfig {
                treasury,
                fee_lamports: fee,
                paused_new: paused,
            }
            .data(),
        }
    }
    pub fn set_config(&mut self, treasury: Pubkey, fee: u64, paused: bool) -> TxResult {
        let a = self.admin.insecure_clone();
        let ix = self.set_config_ix(&a.pubkey(), treasury, fee, paused);
        self.send(&[ix], &a, &[])
    }
    pub fn set_paused(&mut self, paused: bool) -> TxResult {
        let c = self.config();
        self.set_config(c.treasury, c.fee_lamports, paused)
    }
    pub fn set_admin_ix(&self, admin: &Pubkey, new_admin: &Pubkey) -> Instruction {
        Instruction {
            program_id: LOCK,
            accounts: juice_lock::accounts::SetAdmin {
                admin: *admin,
                config: config_pda(),
                new_admin: *new_admin,
            }
            .to_account_metas(None),
            data: juice_lock::instruction::SetAdmin {}.data(),
        }
    }

    // ------------------------------------------------------------ lock instructions

    pub fn lock_accounts(&self, owner: &Pubkey, owner_token: &Pubkey, p: &P, nonce: u64) -> juice_lock::accounts::Lock {
        let record = lock_pda(owner, &p.lp_mint, nonce);
        juice_lock::accounts::Lock {
            owner: *owner,
            config: config_pda(),
            treasury: self.config_treasury(),
            pump_pool: p.pool,
            lp_mint: p.lp_mint,
            owner_token: *owner_token,
            lock_record: record,
            vault: vault_pda(&record),
            token_program: p.program,
            system_program: SYSTEM,
        }
    }
    /// The config's treasury, or the env's treasury before the config exists.
    pub fn config_treasury(&self) -> Pubkey {
        if self.exists(&config_pda()) {
            self.config().treasury
        } else {
            self.treasury.pubkey()
        }
    }
    /// `lock` with `max_fee` = the config's current fee (what a client shows).
    pub fn lock_ix(&self, accounts: juice_lock::accounts::Lock, tier: u8, amount: u64, nonce: u64) -> Instruction {
        let fee = if self.exists(&config_pda()) {
            self.config().fee_lamports
        } else {
            0
        };
        self.lock_ix_fee(accounts, tier, amount, nonce, fee)
    }
    pub fn lock_ix_fee(
        &self,
        accounts: juice_lock::accounts::Lock,
        tier: u8,
        amount: u64,
        nonce: u64,
        max_fee: u64,
    ) -> Instruction {
        Instruction {
            program_id: LOCK,
            accounts: accounts.to_account_metas(None),
            data: juice_lock::instruction::Lock {
                tier,
                amount,
                nonce,
                max_fee,
            }
            .data(),
        }
    }
    /// The locker locks `amount` of its LP of `p` for `tier` with `nonce`.
    pub fn lock(&mut self, who: &Locker, p: &P, tier: u8, amount: u64, nonce: u64) -> (TxResult, L) {
        let accounts = self.lock_accounts(&who.key(), &who.lp, p, nonce);
        let l = L {
            record: accounts.lock_record,
            vault: accounts.vault,
            owner: who.key(),
            pool: *p,
            nonce,
        };
        let ix = self.lock_ix(accounts, tier, amount, nonce);
        (self.send(&[ix], &who.kp, &[]), l)
    }
    /// A fresh locker locks `amount` (all its LP) of `p` for `tier`; must succeed.
    pub fn new_lock(&mut self, p: &P, tier: u8, amount: u64) -> (Locker, L) {
        let who = self.locker(p, amount);
        let (r, l) = self.lock(&who, p, tier, amount, 7);
        expect_ok(&r);
        (who, l)
    }

    pub fn extend_ix(&self, owner: &Pubkey, record: &Pubkey, new_tier: u8) -> Instruction {
        Instruction {
            program_id: LOCK,
            accounts: juice_lock::accounts::Extend {
                owner: *owner,
                lock_record: *record,
            }
            .to_account_metas(None),
            data: juice_lock::instruction::Extend { new_tier }.data(),
        }
    }
    pub fn extend(&mut self, who: &Keypair, l: &L, new_tier: u8) -> TxResult {
        let ix = self.extend_ix(&who.pubkey(), &l.record, new_tier);
        self.send(&[ix], who, &[])
    }

    pub fn unlock_accounts(&self, owner: &Pubkey, l: &L, destination: &Pubkey) -> juice_lock::accounts::Unlock {
        juice_lock::accounts::Unlock {
            owner: *owner,
            lock_record: l.record,
            vault: l.vault,
            lp_mint: l.pool.lp_mint,
            destination: *destination,
            token_program: l.pool.program,
        }
    }
    pub fn unlock_ix(&self, accounts: juice_lock::accounts::Unlock) -> Instruction {
        Instruction {
            program_id: LOCK,
            accounts: accounts.to_account_metas(None),
            data: juice_lock::instruction::Unlock {}.data(),
        }
    }
    pub fn unlock(&mut self, who: &Keypair, l: &L, destination: &Pubkey) -> TxResult {
        let ix = self.unlock_ix(self.unlock_accounts(&who.pubkey(), l, destination));
        self.send(&[ix], who, &[])
    }
}

// ---------------------------------------------------------------- synthetic accounts

pub fn lp_mint_pda(pool: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[b"pool_lp_mint", pool.as_ref()], &PUMP_AMM).0
}

/// A mint's bytes: `decimals`, supply 1e15, optional mint and freeze
/// authorities, and (Token-2022) extensions as (type, value bytes).
pub fn mint_data(
    decimals: u8,
    authority: Option<Pubkey>,
    freeze: Option<Pubkey>,
    extensions: &[(u16, Vec<u8>)],
) -> Vec<u8> {
    let mut d = vec![0u8; 82];
    if let Some(a) = authority {
        d[0] = 1;
        d[4..36].copy_from_slice(a.as_ref());
    }
    d[36..44].copy_from_slice(&1_000_000_000_000_000u64.to_le_bytes());
    d[44] = decimals;
    d[45] = 1;
    if let Some(f) = freeze {
        d[46] = 1;
        d[50..82].copy_from_slice(f.as_ref());
    }
    if extensions.is_empty() {
        return d;
    }
    d.resize(165, 0);
    d.push(1); // AccountType::Mint
    for (ty, value) in extensions {
        d.extend_from_slice(&ty.to_le_bytes());
        d.extend_from_slice(&(value.len() as u16).to_le_bytes());
        d.extend_from_slice(value);
    }
    d
}

pub fn account(owner: Pubkey, data: Vec<u8>) -> Account {
    Account {
        lamports: 10 * SOL,
        data,
        owner,
        executable: false,
        rent_epoch: 0,
    }
}

// ---------------------------------------------------------------- results

/// The Anchor error name from a failed transaction's logs ("ok" on success).
pub fn err_name(r: &TxResult) -> String {
    match r {
        Ok(_) => "ok".into(),
        Err(e) => e
            .meta
            .logs
            .iter()
            .rev()
            .find_map(|l| {
                l.split("Error Code: ")
                    .nth(1)
                    .map(|s| s.split('.').next().unwrap().to_string())
            })
            .unwrap_or_else(|| format!("{:?}", e.err)),
    }
}

pub fn logs(r: &TxResult) -> String {
    match r {
        Ok(m) => m.logs.join("\n"),
        Err(e) => e.meta.logs.join("\n"),
    }
}

/// Assert a transaction failed with the named error.
#[track_caller]
pub fn expect_err(r: &TxResult, name: &str) {
    let got = err_name(r);
    assert_eq!(got, name, "expected {name}, got {got}\n{}", logs(r));
}

/// Assert a transaction failed, whatever the error.
#[track_caller]
pub fn expect_fail(r: &TxResult) {
    assert!(r.is_err(), "expected a failure, got ok\n{}", logs(r));
}

#[track_caller]
pub fn expect_ok(r: &TxResult) {
    if r.is_err() {
        panic!("expected ok, got {}\n{}", err_name(r), logs(r));
    }
}

pub fn units(r: &TxResult) -> u64 {
    match r {
        Ok(m) => m.compute_units_consumed,
        Err(e) => e.meta.compute_units_consumed,
    }
}

/// Every event of type `T` in a successful transaction's logs, in order.
pub fn events<T: anchor_lang::Event + AnchorDeserialize + Discriminator>(r: &TxResult) -> Vec<T> {
    let Ok(m) = r else { return vec![] };
    m.logs
        .iter()
        .filter_map(|l| l.strip_prefix("Program data: "))
        .filter_map(|b| base64::engine::general_purpose::STANDARD.decode(b).ok())
        .filter(|d| d.len() >= 8 && d[..8] == *T::DISCRIMINATOR)
        .map(|d| T::deserialize(&mut &d[8..]).unwrap())
        .collect()
}

impl Env {
    /// A Token-2022 token account of `mint` for `owner` holding `amount`,
    /// with one account extension enabled (type, 1-byte value 1): e.g.
    /// MemoTransfer (8) or CpiGuard (11).
    pub fn token_account_with_ext(&mut self, owner: &Pubkey, mint: &Pubkey, amount: u64, ext: u16) -> Pubkey {
        let key = Pubkey::new_unique();
        let mut data = vec![0u8; 165];
        data[0..32].copy_from_slice(mint.as_ref());
        data[32..64].copy_from_slice(owner.as_ref());
        data[64..72].copy_from_slice(&amount.to_le_bytes());
        data[108] = 1; // initialized
        data.push(2); // AccountType::Account
        data.extend_from_slice(&ext.to_le_bytes());
        data.extend_from_slice(&1u16.to_le_bytes());
        data.push(1);
        let lamports = self.rent(data.len());
        self.svm
            .set_account(
                key,
                Account {
                    lamports,
                    data,
                    owner: TOKEN_2022,
                    executable: false,
                    rent_epoch: 0,
                },
            )
            .unwrap();
        key
    }
}
