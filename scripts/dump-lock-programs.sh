#!/usr/bin/env bash
# Read-only: download the mainnet Token-2022 program binary the juice_lock
# tests run against (git-ignored). The harness refuses any binary whose
# sha256 differs from tests/fixtures/mainnet.json (the same binary juice_boost
# records). If Token-2022 was upgraded since, the printed hash differs: then
# re-record the fixtures and update the hash in both programs' fixtures.
#   SOLANA_RPC=<mainnet rpc> scripts/dump-lock-programs.sh
set -euo pipefail
cd "$(dirname "$0")/../programs/juice-lock/tests/fixtures"
solana program dump -u "${SOLANA_RPC:-https://api.mainnet-beta.solana.com}" TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb token_2022.so
shasum -a 256 token_2022.so
