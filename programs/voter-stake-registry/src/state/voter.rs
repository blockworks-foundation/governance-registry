use crate::error::*;
use crate::state::deposit_entry::DepositEntry;
use crate::state::registrar::Registrar;
use anchor_lang::prelude::*;
use spl_governance::state::token_owner_record;

/// User account for minting voting rights.
#[account(zero_copy)]
pub struct Voter {
    pub voter_authority: Pubkey,
    pub registrar: Pubkey,
    pub deposits: [DepositEntry; 32],
    pub voter_bump: u8,
    pub voter_weight_record_bump: u8,
    pub padding: [u8; 30],
}
const_assert!(std::mem::size_of::<Voter>() == 2 * 32 + 32 * 64 + 2 + 30);

impl Voter {
    pub fn weight(&self, registrar: &Registrar) -> Result<u64> {
        let curr_ts = registrar.clock_unix_timestamp();
        self.deposits
            .iter()
            .filter(|d| d.is_used)
            .try_fold(0, |sum, d| {
                d.voting_power(
                    &registrar.voting_mints[d.voting_mint_config_idx as usize],
                    curr_ts,
                )
                .map(|vp| sum + vp)
            })
    }

    pub fn active_deposit_mut(&mut self, index: u8) -> Result<&mut DepositEntry> {
        let index = index as usize;
        require!(index < self.deposits.len(), OutOfBoundsDepositEntryIndex);
        let d = &mut self.deposits[index];
        require!(d.is_used, UnusedDepositEntryIndex);
        Ok(d)
    }

    pub fn load_token_owner_record(
        &self,
        account_info: &AccountInfo,
        registrar: &Registrar,
    ) -> Result<token_owner_record::TokenOwnerRecord> {
        let record = token_owner_record::get_token_owner_record_data_for_realm_and_governing_mint(
            &registrar.governance_program_id,
            account_info,
            &registrar.realm,
            &registrar.realm_governing_token_mint,
        )?;
        require!(
            record.governing_token_owner == self.voter_authority,
            InvalidTokenOwnerRecord
        );
        Ok(record)
    }
}
