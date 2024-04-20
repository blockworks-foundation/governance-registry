import * as os from 'os';
import * as fs from 'fs';
import * as anchor from "@project-serum/anchor";
import {
  VsrClient,
} from '../src';
import { Account, Commitment, Connection, PublicKey } from '@solana/web3.js';
import { BN } from '@project-serum/anchor';

async function main() {
  // set ANCHOR_PROVIDER_URL and ANCHOR_WALLET
  // and VSR_REGISTRAR
  const provider = anchor.AnchorProvider.env();
  const registrar = new PublicKey(process.env.VSR_REGISTRAR as String);

  let vsr = await VsrClient.connect(provider);
  const voters = await vsr.program.account['voter'].all();

  for (const voter of voters) {
    if (voter.account.registrar.toString() != registrar.toString()) {
      continue;
    }
    let deposited = new BN(0);
    // @ts-ignore
    for (const entry of voter.account.deposits) {
      if (entry.isUsed && entry.votingMintConfigIdx == 0) {
        deposited.iadd(entry.amountDepositedNative);
      }
    }
    if (deposited.gt(new BN(0))) {
      console.log(voter.publicKey.toString(), voter.account.voterAuthority.toString(), deposited.toString());
    }
  }
}

main();
