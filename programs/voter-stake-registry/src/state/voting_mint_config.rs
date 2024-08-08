use crate::error::*;
use anchor_lang::prelude::*;
use std::convert::TryFrom;

const SCALED_FACTOR_BASE: u64 = 1_000_000_000;

/// Exchange rate for an asset that can be used to mint voting rights.
///
/// See documentation of configure_voting_mint for details on how
/// native token amounts convert to vote weight.
#[zero_copy]
#[derive(Default)]
pub struct VotingMintConfig {
    /// Mint for this entry.
    pub mint: Pubkey,

    /// The authority that is allowed to push grants into voters
    pub grant_authority: Pubkey,

    /// Vote weight factor for all funds in the account, no matter if locked or not.
    ///
    /// In 1/SCALED_FACTOR_BASE units.
    pub baseline_vote_weight_scaled_factor: u64,

    /// Maximum extra vote weight factor for lockups.
    ///
    /// This is the extra votes gained for lockups lasting lockup_saturation_secs or
    /// longer. Shorter lockups receive only a fraction of the maximum extra vote weight,
    /// based on lockup_time divided by lockup_saturation_secs.
    ///
    /// In 1/SCALED_FACTOR_BASE units.
    pub max_extra_lockup_vote_weight_scaled_factor: u64,

    /// Number of seconds of lockup needed to reach the maximum lockup bonus.
    pub lockup_saturation_secs: u64,

    /// Number of digits to shift native amounts, applying a 10^digit_shift factor.
    pub digit_shift: i8,

    // Empty bytes for future upgrades.
    pub reserved1: [u8; 7],
    pub reserved2: [u64; 7], // split because `Default` does not support [u8; 63]
}
const_assert!(std::mem::size_of::<VotingMintConfig>() == 2 * 32 + 3 * 8 + 1 + 63);
const_assert!(std::mem::size_of::<VotingMintConfig>() % 8 == 0);

impl VotingMintConfig {
    /// Converts an amount in this voting mints's native currency
    /// to the base vote weight (without the deposit or lockup scalings)
    /// by applying the digit_shift factor.
    fn digit_shift_native(&self, amount_native: u64) -> Result<u64> {
        let compute = || -> Option<u64> {
            let val = if self.digit_shift < 0 {
                (amount_native as u128).checked_div(10u128.pow((-self.digit_shift) as u32))?
            } else {
                (amount_native as u128).checked_mul(10u128.pow(self.digit_shift as u32))?
            };
            u64::try_from(val).ok()
        };
        compute().ok_or_else(|| error!(VsrError::VoterWeightOverflow))
    }

    /// Apply a factor in SCALED_FACTOR_BASE units.
    fn apply_factor(base: u64, factor: u64) -> Result<u64> {
        let compute = || -> Option<u64> {
            u64::try_from(
                (base as u128)
                    .checked_mul(factor as u128)?
                    .checked_div(SCALED_FACTOR_BASE as u128)?,
            )
            .ok()
        };
        compute().ok_or_else(|| error!(VsrError::VoterWeightOverflow))
    }

    /// The vote weight a deposit of a number of native tokens should have.
    ///
    /// This vote_weight is a component for all funds in a voter account, no
    /// matter if locked up or not.
    pub fn baseline_vote_weight(&self, amount_native: u64) -> Result<u64> {
        Self::apply_factor(
            self.digit_shift_native(amount_native)?,
            self.baseline_vote_weight_scaled_factor,
        )
    }

    /// The maximum extra vote weight a number of locked up native tokens can have.
    /// Will be multiplied with a factor between 0 and 1 for the lockup duration.
    pub fn max_extra_lockup_vote_weight(&self, amount_native: u64) -> Result<u64> {
        Self::apply_factor(
            self.digit_shift_native(amount_native)?,
            self.max_extra_lockup_vote_weight_scaled_factor,
        )
    }

    /// Whether this voting mint is configured.
    pub fn in_use(&self) -> bool {
        self.mint != Pubkey::default()
    }

    /// Do tokens of this mint contribute to voting weight?
    ///
    /// DAOs may configure mints without any vote weight contributions if they
    /// want to use the grant / vesting / clawback functionality for non-voting
    /// tokens like USDC.
    pub fn grants_vote_weight(&self) -> bool {
        self.baseline_vote_weight_scaled_factor > 0
            || self.max_extra_lockup_vote_weight_scaled_factor > 0
    }
}
