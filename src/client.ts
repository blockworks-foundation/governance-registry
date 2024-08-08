import { Program, Provider } from '@coral-xyz/anchor';
import { PublicKey } from '@solana/web3.js';
import { VoterStakeRegistry } from './voter_stake_registry';
import VoterStakeRegistryIDL from './voter_stake_registry.json';

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
    const idl = VoterStakeRegistryIDL;

    return new VsrClient(
      new Program<VoterStakeRegistry>(
        idl as VoterStakeRegistry,
        provider,
      ),
      devnet,
    );
  }
}
