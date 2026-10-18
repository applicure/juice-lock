#!/usr/bin/env bash
# Build juice_lock (and the test-only CPI owner program), then run every test
# of the program, for each build: the devnet build (`--features devnet`:
# devnet program id, soak durations), the mainnet-test build (`--features
# mainnet-test`: mainnet id, "1 week" = 1 day) and the default build (mainnet
# program id, the FINAL build). The default build runs last, so target/deploy
# ends with it. `cargo test` alone doesn't rebuild the SBF binaries the
# LiteSVM tests load; the harness refuses a stale binary and a binary of
# another build.
# Token-2022 binary: programs/juice-lock/tests/fixtures/token_2022.so
# (git-ignored; scripts/dump-lock-programs.sh; the harness checks its sha256
# against tests/fixtures/mainnet.json).
#   ./test-lock.sh                         all three builds
#   LOCK_BUILDS=default ./test-lock.sh     one build ("devnet", "mainnet-test" or "default")
set -euo pipefail
cd "$(dirname "$0")"
[ -f programs/juice-lock/tests/fixtures/token_2022.so ] || scripts/dump-lock-programs.sh
for build in ${LOCK_BUILDS:-devnet mainnet-test default}; do
  echo "=== juice_lock build: $build"
  case "$build" in
    devnet | mainnet-test) feat=(--features "$build") ;;
    default) feat=() ;;
    *) echo "unknown build: $build" >&2; exit 2 ;;
  esac
  cargo build-sbf --manifest-path programs/juice-lock/Cargo.toml ${feat[@]+"${feat[@]}"}
  cargo build-sbf --manifest-path test-programs/lock-cpi-owner/Cargo.toml ${feat[@]+"${feat[@]}"}
  # cargo-build-sbf writes throwaway program key pairs next to the binaries;
  # they are not the program's keys and are removed at once.
  rm -f target/deploy/juice_lock-keypair.json target/deploy/lock_cpi_owner-keypair.json
  # build-sbf leaves an up-to-date .so untouched; mark it as built now so the
  # harness's stale-binary guard (sources newer than the .so) stays meaningful.
  touch target/deploy/juice_lock.so target/deploy/lock_cpi_owner.so
  cargo test -p juice-lock ${feat[@]+"${feat[@]}"} "$@"
done
