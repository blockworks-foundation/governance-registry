#!/usr/bin/env bash

set -euo pipefail

if [[ -z "${PROVIDER_WALLET}" ]]; then
  echo "Please provide path to a provider wallet keypair."
  exit -1
fi

if [[ -z "${VERSION_MANUALLY_BUMPED}" ]]; then
  echo "Please bump versions in package.json and in cargo.toml."
  exit -1
fi

anchor build

anchor deploy --provider.cluster devnet --provider.wallet ${PROVIDER_WALLET}
anchor idl upgrade --provider.cluster devnet --provider.wallet ${PROVIDER_WALLET}\
 --filepath target/idl/voter_stake_registry.json 4Q6WW2ouZ6V3iaNm56MTd5n2tnTm4C5fiH8miFHnAFHo

cp ./target/types/voter_stake_registry.ts src/voter_stake_registry.ts
yarn clean && yarn build && cp package.json ./dist/ && yarn publish dist

