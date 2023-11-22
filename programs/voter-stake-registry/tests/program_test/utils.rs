use bytemuck::{bytes_of, Contiguous};
use solana_program::program_error::ProgramError;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Keypair;

use super::SolanaCookie;

#[allow(dead_code)]
pub fn gen_signer_seeds<'a>(nonce: &'a u64, acc_pk: &'a Pubkey) -> [&'a [u8]; 2] {
    [acc_pk.as_ref(), bytes_of(nonce)]
}

#[allow(dead_code)]
pub fn gen_signer_key(
    nonce: u64,
    acc_pk: &Pubkey,
    program_id: &Pubkey,
) -> Result<Pubkey, ProgramError> {
    let seeds = gen_signer_seeds(&nonce, acc_pk);
    Ok(Pubkey::create_program_address(&seeds, program_id)?)
}

#[allow(dead_code)]
pub fn create_signer_key_and_nonce(program_id: &Pubkey, acc_pk: &Pubkey) -> (Pubkey, u64) {
    for i in 0..=u64::MAX_VALUE {
        if let Ok(pk) = gen_signer_key(i, acc_pk, program_id) {
            return (pk, i);
        }
    }
    panic!("Could not generate signer key");
}

#[allow(dead_code)]
pub fn clone_keypair(keypair: &Keypair) -> Keypair {
    Keypair::from_base58_string(&keypair.to_base58_string())
}

#[derive(Debug, PartialEq)]
pub struct LockupData {
    /// time since lockup start (saturating at "duration")
    pub time_passed: u64,
    /// duration of lockup
    pub duration: u64,
    pub amount_initially_locked_native: u64,
    pub amount_deposited_native: u64,
    pub amount_unlocked: u64,
}

#[allow(dead_code)]
pub async fn get_lockup_data(
    solana: &SolanaCookie,
    voter: Pubkey,
    index: u8,
    time_offset: i64,
) -> LockupData {
    let now = solana.get_clock().await.unix_timestamp + time_offset;
    let voter = solana
        .get_account::<voter_stake_registry::state::Voter>(voter)
        .await;
    let d = voter.deposits[index as usize];
    let duration = d.lockup.periods_total().unwrap() * d.lockup.kind.period_secs();
    LockupData {
        time_passed: (duration - d.lockup.seconds_left(now)) as u64,
        duration,
        amount_initially_locked_native: d.amount_initially_locked_native,
        amount_deposited_native: d.amount_deposited_native,
        amount_unlocked: d.amount_unlocked(now),
    }
}
