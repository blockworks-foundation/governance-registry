use crate::error::*;
use crate::state::voting_mint_config::VotingMintConfig;
use anchor_lang::prelude::*;
use anchor_spl::token::Mint;

/// Instance of a voting rights distributor.
#[account(zero_copy)]
#[derive(Default)]
pub struct Registrar {
    pub governance_program_id: Pubkey,
    pub realm: Pubkey,
    pub realm_governing_token_mint: Pubkey,
    pub realm_authority: Pubkey,
    pub clawback_authority: Pubkey,
    // The length should be adjusted for one's use case.
    pub voting_mints: [VotingMintConfig; 2],

    /// Debug only: time offset, to allow tests to move forward in time.
    pub time_offset: i64,
    pub bump: u8,
    pub padding: [u8; 31],
}
const_assert!(std::mem::size_of::<Registrar>() == 5 * 32 + 2 * 120 + 8 + 1 + 31);

impl Registrar {
    pub fn clock_unix_timestamp(&self) -> i64 {
        Clock::get().unwrap().unix_timestamp + self.time_offset
    }

    pub fn voting_mint_config_index(&self, mint: Pubkey) -> Result<usize> {
        self.voting_mints
            .iter()
            .position(|r| r.mint == mint)
            .ok_or(Error::ErrorCode(ErrorCode::VotingMintNotFound))
    }

    pub fn max_vote_weight(&self, mint_accounts: &[AccountInfo]) -> Result<u64> {
        self.voting_mints
            .iter()
            .try_fold(0u64, |mut sum, voting_mint_config| -> Result<u64> {
                if !voting_mint_config.in_use() {
                    return Ok(sum);
                }
                let mint_account = mint_accounts
                    .iter()
                    .find(|a| a.key() == voting_mint_config.mint)
                    .ok_or(Error::ErrorCode(ErrorCode::VotingMintNotFound))?;
                let mint = Account::<Mint>::try_from(mint_account)?;
                sum = sum
                    .checked_add(voting_mint_config.deposit_vote_weight(mint.supply)?)
                    .ok_or(Error::ErrorCode(ErrorCode::VoterWeightOverflow))?;
                sum = sum
                    .checked_add(voting_mint_config.max_lockup_vote_weight(mint.supply)?)
                    .ok_or(Error::ErrorCode(ErrorCode::VoterWeightOverflow))?;
                Ok(sum)
            })
    }
}

#[macro_export]
macro_rules! registrar_seeds {
    ( $registrar:expr ) => {
        &[
            $registrar.realm.as_ref(),
            b"registrar".as_ref(),
            $registrar.realm_governing_token_mint.as_ref(),
            &[$registrar.bump],
        ]
    };
}

pub use registrar_seeds;
