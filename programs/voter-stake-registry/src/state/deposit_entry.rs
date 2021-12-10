use crate::error::*;
use crate::state::lockup::{Lockup, LockupKind};
use crate::state::voting_mint_config::VotingMintConfig;
use anchor_lang::prelude::*;
use std::cmp::min;
use std::convert::TryFrom;

/// Bookkeeping for a single deposit for a given mint and lockup schedule.
#[zero_copy]
#[derive(Default)]
pub struct DepositEntry {
    // Locked state.
    pub lockup: Lockup,

    /// Amount in deposited, in native currency. Withdraws of vested tokens
    /// directly reduce this amount.
    ///
    /// This directly tracks the total amount added by the user. They may
    /// never withdraw more than this amount.
    pub amount_deposited_native: u64,

    /// Amount in locked when the lockup began, in native currency.
    ///
    /// Note that this is not adjusted for withdraws. It is possible for this
    /// value to be bigger than amount_deposited_native after some vesting
    /// and withdrawals.
    ///
    /// This value is needed to compute the amount that vests each peroid,
    /// which should not change due to withdraws.
    pub amount_initially_locked_native: u64,

    // True if the deposit entry is being used.
    pub is_used: bool,

    /// If the clawback authority is allowed to extract locked tokens.
    pub allow_clawback: bool,

    // Points to the VotingMintConfig this deposit uses.
    pub voting_mint_config_idx: u8,

    pub padding: [u8; 13],
}
const_assert!(std::mem::size_of::<DepositEntry>() == 32 + 2 * 8 + 3 + 13);

impl DepositEntry {
    /// # Voting Power Caclulation
    ///
    /// Returns the voting power for the deposit, giving locked tokens boosted
    /// voting power that scales linearly with the lockup time.
    ///
    /// For each cliff-locked token, the vote weight is:
    ///
    /// ```
    ///    voting_power = deposit_vote_weight
    ///                   + lockup_duration_factor * max_lockup_vote_weight
    /// ```
    ///
    /// with
    ///    deposit_vote_weight and max_lockup_vote_weight from the
    ///        VotingMintConfig
    ///    lockup_duration_factor = lockup_time_remaining / max_lockup_time
    ///
    /// Linear vesting schedules can be thought of as a sequence of cliff-
    /// locked tokens and have the matching voting weight.
    ///
    /// To achieve this with the SPL governance program--which requires a "max
    /// vote weight"--we attach what amounts to a scalar multiplier between 0
    /// and 1 to normalize voting power. This multiplier is a function of
    /// the lockup schedule. Here we will describe two, a one time
    /// cliff and a linear vesting schedule unlocking daily.
    ///
    /// ## Cliff Lockup
    ///
    /// The cliff lockup allows one to lockup their tokens for a set period
    /// of time, unlocking all at once on a given date.
    ///
    /// The calculation for this is straightforward and is detailed above.
    ///
    /// ### Decay
    ///
    /// As time passes, the voting power decays until it's back to just
    /// fixed_factor when the cliff has passed. This is important because at
    /// each point in time the lockup should be equivalent to a new lockup
    /// made for the remaining time period.
    ///
    /// ## Linear Vesting Lockup
    ///
    /// Daily/monthly linear vesting can be calculated with series sum, see
    /// voting_power_linear_vesting() below.
    ///
    pub fn voting_power(&self, voting_mint_config: &VotingMintConfig, curr_ts: i64) -> Result<u64> {
        let deposit_vote_weight =
            voting_mint_config.deposit_vote_weight(self.amount_deposited_native)?;
        let max_locked_vote_weight =
            voting_mint_config.max_lockup_vote_weight(self.amount_initially_locked_native)?;
        deposit_vote_weight
            .checked_add(self.voting_power_locked(
                curr_ts,
                max_locked_vote_weight,
                voting_mint_config.lockup_saturation_secs,
            )?)
            .ok_or(Error::ErrorCode(ErrorCode::VoterWeightOverflow))
    }

    /// Vote power contribution from locked funds only.
    pub fn voting_power_locked(
        &self,
        curr_ts: i64,
        max_locked_vote_weight: u64,
        lockup_saturation_secs: u64,
    ) -> Result<u64> {
        if self.lockup.expired(curr_ts) || max_locked_vote_weight == 0 {
            return Ok(0);
        }
        match self.lockup.kind {
            LockupKind::None => Ok(0),
            LockupKind::Daily => self.voting_power_linear_vesting(
                curr_ts,
                max_locked_vote_weight,
                lockup_saturation_secs,
            ),
            LockupKind::Monthly => self.voting_power_linear_vesting(
                curr_ts,
                max_locked_vote_weight,
                lockup_saturation_secs,
            ),
            LockupKind::Cliff => {
                self.voting_power_cliff(curr_ts, max_locked_vote_weight, lockup_saturation_secs)
            }
        }
    }

    /// Vote power contribution from funds with linear vesting.
    fn voting_power_cliff(
        &self,
        curr_ts: i64,
        max_locked_vote_weight: u64,
        lockup_saturation_secs: u64,
    ) -> Result<u64> {
        let remaining = min(self.lockup.seconds_left(curr_ts), lockup_saturation_secs);
        Ok(u64::try_from(
            (max_locked_vote_weight as u128)
                .checked_mul(remaining as u128)
                .unwrap()
                .checked_div(lockup_saturation_secs as u128)
                .unwrap(),
        )
        .unwrap())
    }

    /// Vote power contribution from cliff-locked funds.
    fn voting_power_linear_vesting(
        &self,
        curr_ts: i64,
        max_locked_vote_weight: u64,
        lockup_saturation_secs: u64,
    ) -> Result<u64> {
        let periods_left = self.lockup.periods_left(curr_ts)?;
        let periods_total = self.lockup.periods_total()?;
        let period_secs = self.lockup.kind.period_secs() as u64;

        if periods_left == 0 {
            return Ok(0);
        }

        // This computes the voting power by considering the linear vesting as a
        // sequence of vesting cliffs.
        //
        // For example, if there were 5 vesting periods, with 3 of them left
        // (i.e. two have already vested and their tokens are no longer locked)
        // we'd have (max_locked_vote_weight / 5) weight in each of them, and the
        // voting power would be:
        //    (max_locked_vote_weight/5) * secs_left_for_cliff_1 / lockup_saturation_secs
        //  + (max_locked_vote_weight/5) * secs_left_for_cliff_2 / lockup_saturation_secs
        //  + (max_locked_vote_weight/5) * secs_left_for_cliff_3 / lockup_saturation_secs
        //
        // Or more simply:
        //    max_locked_vote_weight * (\sum_p secs_left_for_cliff_p) / (5 * lockup_saturation_secs)
        //  = max_locked_vote_weight * lockup_secs                    / denominator
        //
        // The value secs_left_for_cliff_p splits up as
        //    secs_left_for_cliff_p = min(
        //        secs_to_closest_cliff + (p-1) * period_secs,
        //        lockup_saturation_secs)
        //
        // We can split the sum into the part before saturation and the part after:
        // Let q be the largest integer <= periods_left where
        //        secs_to_closest_cliff + (q-1) * period_secs < lockup_saturation_secs
        //    =>  q < (lockup_saturation_secs + period_secs - secs_to_closest_cliff) / period_secs
        // and r be the integer where q + r = periods_left, then:
        //    lockup_secs := \sum_p secs_left_for_cliff_p
        //                 = \sum_{p<=q} secs_left_for_cliff_p
        //                   + r * lockup_saturation_secs
        //                 = q * secs_to_closest_cliff
        //                   + period_secs * \sum_0^q (p-1)
        //                   + r * lockup_saturation_secs
        //
        // Where the sum can be expanded to:
        //
        //    sum_full_periods := \sum_0^q (p-1)
        //                      = q * (q - 1) / 2
        //

        // In the example above, periods_total was 5.
        let denominator = periods_total * lockup_saturation_secs;

        let secs_to_closest_cliff = self
            .lockup
            .seconds_left(curr_ts)
            .checked_sub(period_secs * (periods_left - 1))
            .unwrap();

        let lockup_saturation_periods =
            (lockup_saturation_secs + period_secs - secs_to_closest_cliff) / period_secs;
        let q = min(lockup_saturation_periods, periods_left);
        let r = if q < periods_left {
            periods_left - q
        } else {
            0
        };

        // Sum of the full periods left for all remaining vesting cliffs.
        //
        // Examples:
        // - if there are 3 periods left, meaning three vesting cliffs in the future:
        //   one has only a fractional period left and contributes 0
        //   the next has one full period left
        //   and the next has two full periods left
        //   so sums to 3 = 3 * 2 / 2
        // - if there's only one period left, the sum is 0
        let sum_full_periods = q * (q - 1) / 2;

        // Total number of seconds left over all periods_left remaining vesting cliffs
        let lockup_secs =
            q * secs_to_closest_cliff + sum_full_periods * period_secs + r * lockup_saturation_secs;

        Ok(u64::try_from(
            (max_locked_vote_weight as u128)
                .checked_mul(lockup_secs as u128)
                .unwrap()
                .checked_div(denominator as u128)
                .unwrap(),
        )
        .unwrap())
    }

    /// Returns the amount of unlocked tokens for this deposit--in native units
    /// of the original token amount (not scaled by the exchange rate).
    pub fn vested(&self, curr_ts: i64) -> Result<u64> {
        if self.lockup.expired(curr_ts) {
            return Ok(self.amount_initially_locked_native);
        }
        match self.lockup.kind {
            LockupKind::None => Ok(self.amount_initially_locked_native),
            LockupKind::Daily => self.vested_linearly(curr_ts),
            LockupKind::Monthly => self.vested_linearly(curr_ts),
            LockupKind::Cliff => Ok(0),
        }
    }

    fn vested_linearly(&self, curr_ts: i64) -> Result<u64> {
        let period_current = self.lockup.period_current(curr_ts)?;
        let periods_total = self.lockup.periods_total()?;
        if period_current == 0 {
            return Ok(0);
        }
        if period_current >= periods_total {
            return Ok(self.amount_initially_locked_native);
        }
        let vested = self
            .amount_initially_locked_native
            .checked_mul(period_current)
            .unwrap()
            .checked_div(periods_total)
            .unwrap();
        Ok(vested)
    }

    /// Returns native tokens still locked.
    #[inline(always)]
    pub fn amount_locked(&self, curr_ts: i64) -> u64 {
        self.amount_initially_locked_native
            .checked_sub(self.vested(curr_ts).unwrap())
            .unwrap()
    }

    /// Returns the amount that may be withdrawn given current vesting
    /// and previous withdraws.
    #[inline(always)]
    pub fn amount_withdrawable(&self, curr_ts: i64) -> u64 {
        self.amount_deposited_native
            .checked_sub(self.amount_locked(curr_ts))
            .unwrap()
    }
}
