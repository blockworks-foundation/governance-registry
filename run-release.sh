#!/usr/bin/env bash

set -euo pipefail

anchor build
cp ./target/idl/voter_stake_registry.json src/voter_stake_registry.json
cp ./target/types/voter_stake_registry.ts src/voter_stake_registry.ts
yarn clean && yarn build && cp package.json ./dist/ && yarn publish dist

