use crate::error::*;
use crate::state::*;
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct ResetLockup<'info> {
    // checking the PDA address it just an extra precaution,
    // the other constraints must be exhaustive
    pub registrar: AccountLoader<'info, Registrar>,
    #[account(
        mut,
        seeds = [registrar.key().as_ref(), b"voter".as_ref(), voter_authority.key().as_ref()],
        bump = voter.load()?.voter_bump,
        has_one = voter_authority,
        has_one = registrar)]
    pub voter: AccountLoader<'info, Voter>,
    pub voter_authority: Signer<'info>,
}

/// Resets a lockup to start at the current slot timestamp and to last for
/// `periods`, which must be >= the number of periods left on the lockup.
/// This will re-lock any non-withdrawn vested funds.
pub fn reset_lockup(
    ctx: Context<ResetLockup>,
    deposit_entry_index: u8,
    target_deposit_entry_index: u8,
    kind: LockupKind,
    periods: u32,
    amount: u64,
) -> Result<()> {
    let registrar = &ctx.accounts.registrar.load()?;
    let voter = &mut ctx.accounts.voter.load_mut()?;
    let curr_ts = registrar.clock_unix_timestamp();
    let target_index = target_deposit_entry_index as usize;

    // Common checks on the source deposit
    {
        let source = voter.active_deposit_mut(deposit_entry_index)?;
        require!(
            amount <= source.amount_deposited_native,
            InsufficientDepositedTokens
        );

        // Must not decrease duration or strictness
        require!(
            (periods as u64).checked_mul(kind.period_secs()).unwrap()
                >= source.lockup.seconds_left(curr_ts),
            InvalidLockupPeriod
        );
        require!(
            kind.strictness() >= source.lockup.kind.strictness(),
            InvalidLockupKind
        );

        // Allowing changes to clawback-enabled deposits could be used to avoid
        // clawback by making proposal instructions target the wrong entry index.
        require!(!source.allow_clawback, InvalidDays);
    }

    if deposit_entry_index == target_deposit_entry_index {
        // Change the deposit entry internally.
        let d_entry = voter.active_deposit_mut(deposit_entry_index)?;

        // Cannot unlock tokens that way.
        require!(
            amount >= d_entry.amount_locked(curr_ts),
            MustKeepTokensLocked
        );

        d_entry.amount_initially_locked_native = amount;
        d_entry.lockup = Lockup::new_from_periods(kind, curr_ts, periods)?;
    } else {
        // Move partially to a new deposit entry
        let source = voter.active_deposit_mut(deposit_entry_index)?;
        source.amount_initially_locked_native =
            source.amount_initially_locked_native.saturating_sub(amount);
        source.amount_deposited_native -= amount;
        let mint_idx = source.voting_mint_config_idx;

        require!(
            voter.deposits.len() > target_index,
            OutOfBoundsDepositEntryIndex
        );
        let target = &mut voter.deposits[target_index];
        require!(!target.is_used, DepositEntryFull);

        *target = DepositEntry::default();
        target.is_used = true;
        target.voting_mint_config_idx = mint_idx;
        target.amount_deposited_native = amount;
        target.amount_initially_locked_native = amount;
        target.allow_clawback = false;
        target.lockup = Lockup::new_from_periods(kind, curr_ts, periods)?;
    }

    Ok(())
}
