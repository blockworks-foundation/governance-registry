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
    periods: u32,
) -> Result<()> {
    let registrar = &ctx.accounts.registrar.load()?;
    let voter = &mut ctx.accounts.voter.load_mut()?;
    let d = voter.active_deposit_mut(deposit_entry_index)?;

    // The lockup period can only be increased.
    let curr_ts = registrar.clock_unix_timestamp();
    require!(
        periods as u64 >= d.lockup.periods_left(curr_ts)?,
        InvalidDays
    );
    require!(periods > 0, InvalidDays);

    // Lock up every deposited token again
    d.amount_initially_locked_native = d.amount_deposited_native;
    d.lockup = Lockup::new_from_periods(d.lockup.kind, curr_ts, periods)?;

    Ok(())
}
