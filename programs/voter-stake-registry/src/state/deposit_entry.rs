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

    pub reserved: [u8; 29],
}
const_assert!(std::mem::size_of::<DepositEntry>() == 32 + 2 * 8 + 3 + 29);
const_assert!(std::mem::size_of::<DepositEntry>() % 8 == 0);

impl DepositEntry {
    /// # Voting Power Caclulation
    ///
    /// Returns the voting power for the deposit, giving locked tokens boosted
    /// voting power that scales linearly with the lockup time.
    ///
    /// For each cliff-locked token, the vote weight is:
    ///
    /// ```
    ///    voting_power = baseline_vote_weight
    ///                   + lockup_duration_factor * max_extra_lockup_vote_weight
    /// ```
    ///
    /// with
    ///   - lockup_duration_factor = min(lockup_time_remaining / lockup_saturation_secs, 1)
    ///   - the VotingMintConfig providing the values for
    ///     baseline_vote_weight, max_extra_lockup_vote_weight, lockup_saturation_secs
    ///
    /// Linear vesting schedules can be thought of as a sequence of cliff-
    /// locked tokens and have the matching voting weight.
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
        let baseline_vote_weight =
            voting_mint_config.baseline_vote_weight(self.amount_deposited_native)?;
        let max_locked_vote_weight =
            voting_mint_config.max_extra_lockup_vote_weight(self.amount_initially_locked_native)?;
        let locked_vote_weight = self.voting_power_locked(
            curr_ts,
            max_locked_vote_weight,
            voting_mint_config.lockup_saturation_secs,
        )?;
        require_gte!(
            max_locked_vote_weight,
            locked_vote_weight,
            VsrError::InternalErrorBadLockupVoteWeight
        );
        baseline_vote_weight
            .checked_add(locked_vote_weight)
            .ok_or_else(|| error!(VsrError::VoterWeightOverflow))
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
            LockupKind::Constant => {
                self.voting_power_cliff(curr_ts, max_locked_vote_weight, lockup_saturation_secs)
            }
        }
    }

    /// Vote power contribution from locked funds only at `at_ts`, assuming the user does everything
    /// they can to unlock as quickly as possible at `curr_ts`.
    ///
    /// Currently that means that Constant lockups get turned into Cliff lockups.
    pub fn voting_power_locked_guaranteed(
        &self,
        curr_ts: i64,
        at_ts: i64,
        max_locked_vote_weight: u64,
        lockup_saturation_secs: u64,
    ) -> Result<u64> {
        let mut altered = self.clone();

        // Trigger the unlock phase for constant lockups
        if self.lockup.kind == LockupKind::Constant {
            altered.lockup.kind = LockupKind::Cliff;
            altered.lockup.start_ts = curr_ts;
            altered.lockup.end_ts = curr_ts
                .checked_add(i64::try_from(self.lockup.seconds_left(curr_ts)).unwrap())
                .unwrap();
        }

        // Other lockup types don't need changes, because the user
        // cannot reduce their lockup strength.

        altered.voting_power_locked(at_ts, max_locked_vote_weight, lockup_saturation_secs)
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
        // If secs_to_closest_cliff < lockup_saturation_secs, we can split the sum
        //    \sum_p secs_left_for_cliff_p
        // into the part before saturation and the part after:
        // Let q be the largest integer 1 <= q <= periods_left where
        //        secs_to_closest_cliff + (q-1) * period_secs < lockup_saturation_secs
        //    =>  q = (lockup_saturation_secs - secs_to_closest_cliff + period_secs) / period_secs
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

        let secs_to_closest_cliff = self
            .lockup
            .seconds_left(curr_ts)
            .checked_sub(
                period_secs
                    .checked_mul(periods_left.saturating_sub(1))
                    .unwrap(),
            )
            .unwrap();

        if secs_to_closest_cliff >= lockup_saturation_secs {
            return Ok(max_locked_vote_weight);
        }

        // In the example above, periods_total was 5.
        let denominator = periods_total.checked_mul(lockup_saturation_secs).unwrap();

        let lockup_saturation_periods = (lockup_saturation_secs
            .saturating_sub(secs_to_closest_cliff)
            .checked_add(period_secs)
            .unwrap())
        .checked_div(period_secs)
        .unwrap();
        let q = min(lockup_saturation_periods, periods_left);
        let r = periods_left.saturating_sub(q);

        // Sum of the full periods left for all remaining vesting cliffs.
        //
        // Examples:
        // - if there are 3 periods left, meaning three vesting cliffs in the future:
        //   one has only a fractional period left and contributes 0
        //   the next has one full period left
        //   and the next has two full periods left
        //   so sums to 3 = 3 * 2 / 2
        // - if there's only one period left, the sum is 0
        let sum_full_periods = q.checked_mul(q.saturating_sub(1)).unwrap() / 2;

        // Total number of seconds left over all periods_left remaining vesting cliffs
        let lockup_secs_fractional = q.checked_mul(secs_to_closest_cliff).unwrap();
        let lockup_secs_full = sum_full_periods.checked_mul(period_secs).unwrap();
        let lockup_secs_saturated = r.checked_mul(lockup_saturation_secs).unwrap();
        let lockup_secs = lockup_secs_fractional as u128
            + lockup_secs_full as u128
            + lockup_secs_saturated as u128;

        Ok(u64::try_from(
            (max_locked_vote_weight as u128)
                .checked_mul(lockup_secs)
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
            LockupKind::Constant => Ok(0),
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

    /// Returns native tokens that are unlocked given current vesting
    /// and previous withdraws.
    #[inline(always)]
    pub fn amount_unlocked(&self, curr_ts: i64) -> u64 {
        self.amount_deposited_native
            .checked_sub(self.amount_locked(curr_ts))
            .unwrap()
    }

    /// Adjusts the deposit and remaining lockup periods such that
    /// no parts of amount_initially_locked_native have vested.
    ///
    /// That makes it easier to deal with changes to the locked
    /// amount because amount_initially_locked_native represents
    /// exactly the amount that is locked.
    ///
    /// Example:
    ///   If 30 tokens are locked up over 3 months, vesting each month,
    ///   then after month 2:
    ///      amount_initially_locked_native = 30
    ///      amount_deposited_native = 30
    ///      vested() = 20
    ///      period_current() = 2
    ///      periods_total() = 3
    ///   And after this function was called:
    ///      amount_initially_locked_native = 10
    ///      amount_deposited_native = 30
    ///      vested() = 0
    ///      period_current() = 0
    ///      periods_total() = 1
    pub fn resolve_vesting(&mut self, curr_ts: i64) -> Result<()> {
        let vested_amount = self.vested(curr_ts)?;
        require_gte!(
            self.amount_initially_locked_native,
            vested_amount,
            VsrError::InternalProgramError
        );
        self.amount_initially_locked_native = self
            .amount_initially_locked_native
            .checked_sub(vested_amount)
            .unwrap();
        self.lockup.remove_past_periods(curr_ts)?;
        require_eq!(self.vested(curr_ts)?, 0, VsrError::InternalProgramError);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::LockupKind::{Constant, Daily};

    #[test]
    pub fn resolve_vesting() -> Result<()> {
        let mut deposit = DepositEntry {
            amount_deposited_native: 35,
            amount_initially_locked_native: 30,
            lockup: Lockup::new_from_periods(LockupKind::Monthly, 1000, 1000, 3).unwrap(),
            is_used: true,
            allow_clawback: false,
            voting_mint_config_idx: 0,
            reserved: [0; 29],
        };
        let initial_deposit = deposit.clone();
        let month = deposit.lockup.kind.period_secs() as i64;
        // function to avoid unaligned references when used with assert!()
        let amount_initially_locked =
            |deposit: &DepositEntry| deposit.amount_initially_locked_native;

        let mut time = 1001;
        assert_eq!(deposit.vested(time).unwrap(), 0);
        assert_eq!(deposit.amount_unlocked(time), 5);
        deposit.resolve_vesting(time).unwrap(); // no effect
        assert_eq!(deposit.vested(time).unwrap(), 0);
        assert_eq!(deposit.amount_unlocked(time), 5);
        assert_eq!(
            deposit.lockup.seconds_left(time),
            initial_deposit.lockup.seconds_left(time)
        );
        assert_eq!(deposit.lockup.period_current(time).unwrap(), 0);
        assert_eq!(deposit.lockup.periods_total().unwrap(), 3);
        assert_eq!(amount_initially_locked(&deposit), 30);

        time = 1001 + month;
        assert_eq!(deposit.vested(time).unwrap(), 10);
        assert_eq!(deposit.lockup.period_current(time).unwrap(), 1);
        assert_eq!(deposit.lockup.periods_total().unwrap(), 3);
        deposit.resolve_vesting(time).unwrap();
        assert_eq!(deposit.vested(time).unwrap(), 0);
        assert_eq!(deposit.amount_unlocked(time), 15);
        assert_eq!(
            deposit.lockup.seconds_left(time),
            initial_deposit.lockup.seconds_left(time)
        );
        assert_eq!(deposit.lockup.period_current(time).unwrap(), 0);
        assert_eq!(deposit.lockup.periods_total().unwrap(), 2);
        assert_eq!(amount_initially_locked(&deposit), 20);

        time = 1001 + 3 * month;
        assert_eq!(deposit.vested(time).unwrap(), 20);
        assert_eq!(deposit.lockup.period_current(time).unwrap(), 2);
        assert_eq!(deposit.lockup.periods_total().unwrap(), 2);
        deposit.resolve_vesting(time).unwrap();
        assert_eq!(deposit.vested(time).unwrap(), 0);
        assert_eq!(deposit.amount_unlocked(time), 35);
        assert_eq!(
            deposit.lockup.seconds_left(time),
            initial_deposit.lockup.seconds_left(time)
        );
        assert_eq!(deposit.lockup.period_current(time).unwrap(), 0);
        assert_eq!(deposit.lockup.periods_total().unwrap(), 0);
        assert_eq!(amount_initially_locked(&deposit), 0);

        Ok(())
    }

    #[test]
    pub fn far_future_lockup_start_test() -> Result<()> {
        // Check that voting power stays correct even if the lockup is very far in the
        // future, or at least more than lockup_saturation_secs in the future.
        let day: i64 = 86_400;
        let saturation: i64 = 5 * day;
        let lockup_start = 10_000_000_000; // arbitrary point
        let deposit = DepositEntry {
            amount_deposited_native: 10_000,
            amount_initially_locked_native: 10_000,
            lockup: Lockup {
                start_ts: lockup_start,
                end_ts: lockup_start + 2 * day,
                kind: Daily,
                reserved: [0; 15],
            },
            is_used: true,
            allow_clawback: false,
            voting_mint_config_idx: 0,
            reserved: [0; 29],
        };
        let voting_mint_config = VotingMintConfig {
            mint: Pubkey::default(),
            grant_authority: Pubkey::default(),
            baseline_vote_weight_scaled_factor: 1_000_000_000, // 1x
            max_extra_lockup_vote_weight_scaled_factor: 1_000_000_000, // 1x
            lockup_saturation_secs: saturation as u64,
            digit_shift: 0,
            reserved1: [0; 7],
            reserved2: [0; 7],
        };

        let baseline_vote_weight =
            voting_mint_config.baseline_vote_weight(deposit.amount_deposited_native)?;
        assert_eq!(baseline_vote_weight, 10_000);
        let max_locked_vote_weight = voting_mint_config
            .max_extra_lockup_vote_weight(deposit.amount_initially_locked_native)?;
        assert_eq!(max_locked_vote_weight, 10_000);

        // The timestamp 100_000 is very far before the lockup_start timestamp
        let withdrawable = deposit.amount_unlocked(100_000);
        assert_eq!(withdrawable, 0);
        let voting_power = deposit.voting_power(&voting_mint_config, 100_000).unwrap();
        assert_eq!(voting_power, 20_000);

        let voting_power = deposit
            .voting_power(&voting_mint_config, lockup_start - saturation)
            .unwrap();
        assert_eq!(voting_power, 20_000);

        let voting_power = deposit
            .voting_power(&voting_mint_config, lockup_start - saturation + day)
            .unwrap();
        assert_eq!(voting_power, 20_000);

        let voting_power = deposit
            .voting_power(&voting_mint_config, lockup_start - saturation + day + 1)
            .unwrap();
        assert_eq!(voting_power, 19_999);

        let voting_power = deposit
            .voting_power(&voting_mint_config, lockup_start - saturation + 2 * day)
            .unwrap();
        assert_eq!(voting_power, 19_000); // the second cliff has only 4/5th of lockup period left

        let voting_power = deposit
            .voting_power(&voting_mint_config, lockup_start - saturation + 2 * day + 1)
            .unwrap();
        assert_eq!(voting_power, 18_999);

        Ok(())
    }

    #[test]
    pub fn guaranteed_lockup_test() -> Result<()> {
        // Check that constant lockups are handled correctly.
        let day: i64 = 86_400;
        let saturation = (10 * day) as u64;
        let start = 10_000_000_000; // arbitrary point
        let deposit = DepositEntry {
            amount_deposited_native: 10_000,
            amount_initially_locked_native: 10_000,
            lockup: Lockup {
                start_ts: start,
                end_ts: start + 5 * day,
                kind: Constant,
                reserved: [0; 15],
            },
            is_used: true,
            allow_clawback: false,
            voting_mint_config_idx: 0,
            reserved: [0; 29],
        };

        let v = |curr_offset, at_offset| {
            deposit
                .voting_power_locked_guaranteed(
                    start + curr_offset,
                    start + at_offset,
                    100,
                    saturation,
                )
                .unwrap()
        };

        assert_eq!(v(0, 0), 50);
        assert_eq!(v(-day, 0), 40);
        assert_eq!(v(-100 * day, 0), 0);
        assert_eq!(v(-100 * day, -98 * day), 30);
        assert_eq!(v(0, day), 40);
        assert_eq!(v(0, 5 * day), 0);
        assert_eq!(v(0, 50 * day), 0);
        assert_eq!(v(day, day), 50);
        assert_eq!(v(day, 2 * day,), 40);
        assert_eq!(v(day, 20 * day), 0);
        assert_eq!(v(50 * day, 50 * day), 50);
        assert_eq!(v(50 * day, 51 * day), 40);
        assert_eq!(v(50 * day, 80 * day), 0);

        Ok(())
    }
}
