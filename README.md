# Description

Voter-stake-registry is a voter weight addin for Solana's
[spl-governance program](https://github.com/solana-labs/solana-program-library/tree/master/governance).

With the addin enabled, the governance realm authority can:

- Control which token mints can be used to vote, and at what scaling factor.

  That means that tokens from mints other than the governing mint can be used
  to vote and that their relative weight can be set.

- Claw back locked tokens from user deposits where the user has enabled it.

  This is intended for use with token grants. Users would not enable clawback
  for normal deposits.

Users can:

- Deposit and withdraw tokens of the chosen mints to gain voting weight.

  When an addin is enabled, the default deposit/withdraw flow of the governing
  token mints is disabled in spl-governance. The addin adds back the ability
  to deposit and withdraw without lockup.

- Lock up tokens with different vesting schedules.

  The tokens will only be withdrawable once vested or the lock up has expired.
  Locked up tokens may have extra voting weight.

- Use their voting weight to vote on spl-governance proposals.

# Development

## Rust
* Built and developed using - rust stable(`rustc 1.79.0 (129f3b996 2024-06-10)`)
* Run rust based tests - `cargo test-sbf`
* `run-generate-anchor-types.sh` generates latest anchor types file and writes to `./voter_stake_registry.ts`
* To install the typescript client, do - `yarn add @blockworks-foundation/voter-stake-registry-client`
* usage

## Node/Typescript
* Built and developed using - node (`v16.13.1`)
* Usage
```
import { Provider, Wallet } from '@project-serum/anchor';
import { Connection, Keypair } from '@solana/web3.js';
import { VsrClient } from '@blockworks-foundation/voter-stake-registry-client';

async function main() {
  const options = Provider.defaultOptions();
  const connection = new Connection('https://api.devnet.solana.com', options);
  const wallet = new Wallet(Keypair.generate());
  const provider = new Provider(connection, wallet, options);
  const client = await VsrClient.connect(provider, true);
```

<img width="708" alt="image" src="https://user-images.githubusercontent.com/89031858/148725266-29459e80-623e-45c4-952d-5d9d1f0f15bc.png">


# Deployment

Users will likely want to compile their own voter-stake-registry and deploy it to an address they control.

Before compiling, look at:
- `Registrar::voting_mints`: The length of this array defines the number of configurable voting mints. Adjust as needed.

## Devnet

For testing purposes, an instance of voter-stake-registry is deployed on devnet:
```
voter-stake-registry:  4Q6WW2ouZ6V3iaNm56MTd5n2tnTm4C5fiH8miFHnAFHo
spl-governance master: i7BqPFNUvB7yqwVeCRJHrtZVwRsZZNUJTdBm7Vg2cDb
```

# Usage Scenarios

## Setup

To start using the addin, make a governance proposal with the spl-governance
realm authority to:
1. Deploy an instance of the voter-stake-registry.
2. Create a registrar for the realm with the `CreateRegistrar` instruction.
3. Add voting token mints to the registrar by calling the `ConfigureVotingMint`
   instruction as often as desired.
4. Call the `SetRealmConfig` instruction on spl-governance to set the
   voter-weight-addin program id and thereby enable the addin.

## Deposit and Vote Without Lockup

1. Call `CreateVoter` on the addin (first time only). Use the same
   voter_authority that was used for registering with spl-governance.
2. Call `CreateDepositEntry` for the voter with `LockupKind::None`
   and the token mint for that tokens are to be deposited. (first time only)

   This creates a new deposit entry that can be used for depositing and
   withdrawing funds without lockup.
3. Call `Deposit` for the voter and same deposit entry id to deposit funds.
4. To vote, call `UpdateVoterWeightRecord` on the addin and then call `CastVote`
   on spl-governance in the same transaction, passing the voter weight record
   to both.
5. Withdraw funds with `Withdraw` once proposals have resolved.

## Give Grants of Locked Tokens

1. Ask the recepient for their desired address.
2. Make a proposal to call `Grant` for depositing tokens into a new locked
   deposit entry for their address. Use a governance that either is the realm
   authority or the token mint's grant authority.
3. If necessary, later make a proposal to call `Clawback` on their deposit to
   retrieve all remaining locked tokens.

## Manage Constant Maturity Deposits

Constant maturity deposits are useful when there's a vote weight bonus for
locking up tokens: With cliff or daily/monthly vested deposits the remaining
lockup period decreases as the time of maturity approaches and thus the vote
weight decreases over time as well.

Constant maturity lockup keeps tokens at a fixed maturity. That guarantees a
fixed vote weight, but also means they need to be manually transitioned to a
different lockup type before they can eventually be withdrawn.

Setting up a constant maturity lockup is easy:

1. Create a deposit entry of `Constant` lockup type with the chosen number of
   days.
2. `Deposit` tokens into it.
3. Use it to vote.

If you want access to the tokens again, you need to start the unlocking process
by either
- changing the whole deposit entry to `Cliff` with `ResetLockup`, or
- creating a new `Cliff` deposit entry and transfering some locked tokens from
  your `Constant` deposit entry over with `InternalTransferLocked`.

In both cases you'll need to wait for the cliff to be reached before being able
to access the tokens again.


# Instruction Overview

## Setup

- [`CreateRegistrar`](programs/voter-stake-registry/src/instructions/create_registrar.rs)

  Creates a Registrar account for a governance realm.

- [`ConfigureVotingMint`](programs/voter-stake-registry/src/instructions/configure_voting_mint.rs)

  Enables voting with tokens from a mint and sets the exchange rate for vote weight.

## Usage

- [`CreateVoter`](programs/voter-stake-registry/src/instructions/create_voter.rs)

  Create a new voter account for a user.

- [`CreateDepositEntry`](programs/voter-stake-registry/src/instructions/create_deposit_entry.rs)

  Create a deposit entry on a voter. A deposit entry is where tokens from a voting mint
  are deposited, and which may optionally have a lockup period and vesting schedule.

  Each voter can have multiple deposit entries.

- [`Deposit`](programs/voter-stake-registry/src/instructions/deposit.rs)

  Add tokens to a deposit entry.

- [`Withdraw`](programs/voter-stake-registry/src/instructions/withdraw.rs)

  Remove tokens from a deposit entry, either unlocked or vested.

- [`ResetLockup`](programs/voter-stake-registry/src/instructions/reset_lockup.rs)

  Re-lock tokens where the lockup has expired, or increase the duration of the lockup or
  change the lockup kind.

- [`InternalTransferLocked`](programs/voter-stake-registry/src/instructions/internal_transfer_locked.rs)

  Transfer locked tokens from one deposit entry to another. Useful for splitting off a
  chunk of a "constant" lockup deposit entry that you want to start the unlock process on.

- [`InternalTransferUnocked`](programs/voter-stake-registry/src/instructions/internal_transfer_unlocked.rs)

  Transfer unlocked tokens from one deposit entry to another. Useful for splitting off a
  chunk to be locked again in a different deposit entry without having to withdraw and redeposit.

- [`UpdateVoterWeightRecord`](programs/voter-stake-registry/src/instructions/update_voter_weight_record.rs)

  Write the current voter weight to the account that spl-governance can read to
  prepare for voting.

- [`CloseDepositEntry`](programs/voter-stake-registry/src/instructions/close_deposit_entry.rs)

  Close an empty deposit entry, so it can be reused for a different mint or lockup type.

- [`CloseVoter`](programs/voter-stake-registry/src/instructions/close_voter.rs)

  Close an empty voter, reclaiming rent.

## Special

- [`Grant`](programs/voter-stake-registry/src/instructions/grant.rs)

  As the realm authority or mint's grant authority: create a voter (if needed), create a
  new deposit and fund it. This instruction is intended for use with DAO proposals.

- [`Clawback`](programs/voter-stake-registry/src/instructions/clawback.rs)

  As the clawback authority, claim locked tokens from a voter's deposit entry that
  has opted-in to clawback.

- [`UpdateMaxVoteWeight`](programs/voter-stake-registry/src/instructions/update_max_vote_weight.rs)

  Unfinished instruction for telling spl-governance about the total maximum vote weight.

- [`SetTimeOffset`](programs/voter-stake-registry/src/instructions/set_time_offset.rs)

  Debug instruction for advancing time in tests. Not usable.


# License

This code is currently not free to use while in development.


# References:
* [spl-governance](https://github.com/solana-labs/solana-program-library/tree/master/governance)
