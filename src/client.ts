import { Program, Provider } from '@project-serum/anchor';
import { PublicKey } from '@solana/web3.js';
import { VoterStakeRegistry, IDL } from './voter_stake_registry';

export const VSR_ID = new PublicKey(
  '4Q6WW2ouZ6V3iaNm56MTd5n2tnTm4C5fiH8miFHnAFHo',
);

export class VsrClient {
  constructor(
    public program: Program<VoterStakeRegistry>,
    public devnet?: boolean,
  ) {}

  static async connect(
    provider: Provider,
    devnet?: boolean,
  ): Promise<VsrClient> {
    // alternatively we could fetch from chain
    // const idl = await Program.fetchIdl(VSR_ID, provider);
    const idl = IDL;

    return new VsrClient(
      new Program<VoterStakeRegistry>(
        idl as VoterStakeRegistry,
        VSR_ID,
        provider,
      ),
      devnet,
    );
  }
}
