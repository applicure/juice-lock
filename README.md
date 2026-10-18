# juice_lock

Source of **Juice LP locks**, the Solana program behind the LP lock on [juicepools.app](https://juicepools.app).
It is published so anyone can rebuild it and check that it is exactly the program running on mainnet.

All rights reserved; published for verification.

| | |
|---|---|
| Program id (mainnet) | `544vBp5c5GHJwNDm4YmS4FK24to9YP17iJLSXmcjFtmG` |
| Upgrade authority | none. Immutable since 2026-10-17, revoke transaction [`2oQmpb7G…ZuZ`](https://solscan.io/tx/2oQmpb7GTe1PwCxLbktdZ4PfN9mPWMn3tdoC3VgHWfSt3kL1ToWFtpsQJohcSvfyQnNBX2gmPnxE9sxpQYKpZDuZ) |
| Executable hash (`solana-verify get-program-hash`) | `e46610defebfe99228e3828e96e740268d7ecce549e5451bfdb9794a993bb91b` |
| Binary | 345,192 bytes, sha256 `1b3c8ac05d7826674b61e23d8b809df803cbd5d0d2fe957b6d93f1b3e9c2c1b2` |
| Stack | Anchor 1.1.2, Solana 3.1.10 (platform-tools v1.52), Rust 1.89.0 |

## What it does

A user locks PumpSwap LP tokens for a fixed time: **7, 30, 90, 180 or 365 days, or Forever**. Nobody can take them
out early: not the owner, not Juice, not an admin. After the unlock time only the lock's owner can withdraw them, to
a token account it owns. Forever locks can never be withdrawn.

- **Custody.** Each lock has its own vault, a token account at the PDA `["vault", record]` whose authority is the
  lock record PDA. Only `unlock` signs for the record: it pays the vault's whole balance to the owner, then closes
  the vault and the record.
- **Instructions.** `lock`, `extend` (lengthen only, never shorten), `unlock`, and the admin's `init_config`,
  `set_config`, `set_admin`. `extend` and `unlock` don't read the config, so no admin setting can stop, delay or
  redirect them.
- **Accepted LP.** The pool must be owned by the PumpSwap AMM, the LP mint must be that pool's mint (the pool is its
  mint authority), and the mint must not have a freeze authority or a refused Token-2022 extension.
- **Fee.** A SOL fee per new lock, paid to the treasury in the config. It is 0 or between the rent minimum and
  **0.05 SOL** (hard cap in the program, `MAX_FEE_LAMPORTS`). The user signs a `max_fee`; a higher fee is refused.

**What the admin can do:** change the fee (within the cap), change the treasury, pause and unpause **new** locks,
and hand the admin role on (with the new admin's signature). **What it can't:** touch any existing lock. No admin
instruction takes a lock, a vault or a token account; the admin can't move, shorten, extend, redirect or block
the unlock of anyone's LP.

## Reproduce the build and compare

Requirements: Docker, [`solana-verify`](https://github.com/Ellipsis-Labs/solana-verifiable-build) (tested with
0.5.2), and Rust (for `solana-verify` itself).

```sh
git clone https://github.com/applicure/juice-lock && cd juice-lock
solana-verify build --library-name juice_lock
solana-verify get-executable-hash target/deploy/juice_lock.so
solana-verify get-program-hash -um 544vBp5c5GHJwNDm4YmS4FK24to9YP17iJLSXmcjFtmG
```

Both hashes must be `e46610defebfe99228e3828e96e740268d7ecce549e5451bfdb9794a993bb91b`.

- `--library-name juice_lock` is required. It builds only `programs/juice-lock`. A build of the whole workspace
  would also compile the test-only CPI program, which turns on `juice-lock`'s `cpi` feature and gives a different
  binary.
- The image is `solanafoundation/solana-verifiable-build:3.1.10`
  (`sha256:f71be5ca7620b7e40933b7f1294fa44e01d08c1fc5ba1f375a2478f5a01580d3`). `solana-verify` picks it from
  `[workspace.metadata.cli]` in `Cargo.toml`.
- The deployed program is the **default build** (no features). The `devnet` feature changes the program id and the
  durations. The `mainnet-test` feature (deployed for a one-day test before the final upgrade) changes only the
  "7 days" tier to 1 day. Neither feature is in the mainnet binary.

Or in one step, which clones, builds and compares against the chain:

```sh
solana-verify verify-from-repo -um --program-id 544vBp5c5GHJwNDm4YmS4FK24to9YP17iJLSXmcjFtmG \
  https://github.com/applicure/juice-lock --commit-hash <commit> --library-name juice_lock
```

(Answer `n` when asked to upload verification data. That step writes a transaction and is not needed to compare.)

## Tests

66 LiteSVM integration tests plus unit tests, run against the mainnet Token-2022 binary and recorded public
mainnet accounts (four PumpSwap pools, their LP mints, the Rent sysvar) in `programs/juice-lock/tests/fixtures/`.

```sh
./test-lock.sh                       # devnet, mainnet-test and default builds, every test
LOCK_BUILDS=default ./test-lock.sh   # the mainnet build only
```

`test-lock.sh` downloads the Token-2022 program once, read-only (`scripts/dump-lock-programs.sh`, set `SOLANA_RPC`
for another RPC). The harness checks its sha256 against the fixture. `test-programs/lock-cpi-owner` is a test-only
program (a PDA that owns locks through CPI). It is never deployed.

Source comments refer to internal design documents (`docs/...`) that are not part of this repository. The program
key pairs are not in this repository either.

## Audits

Two independent AI-agent security audits (Claude agents, October 2026). No external audit firm. This is also
stated in the program's embedded `security.txt`.

## Security

Report vulnerabilities to **security@juicepools.app**. Policy: <https://juicepools.app/security>.
The program is immutable, so a bug can't be patched. The admin can pause new locks; existing locks are not
affected by a pause.
