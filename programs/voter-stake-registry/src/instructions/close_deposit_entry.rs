use crate::error::*;
use crate::state::*;
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct CloseDepositEntry<'info> {
    #[account(mut, has_one = voter_authority)]
    pub voter: AccountLoader<'info, Voter>,
    pub voter_authority: Signer<'info>,
}

/// Close an empty deposit entry, allowing it to be reused in the future.
///
/// Deposit entries can only be closed when they don't hold any tokens.
///
/// If the deposit entry has `allow_clawback` set, it can only be closed once
/// the lockup period has expired.
pub fn close_deposit_entry(ctx: Context<CloseDepositEntry>, deposit_entry_index: u8) -> Result<()> {
    msg!("--------close_deposit_entry--------");
    let voter = &mut ctx.accounts.voter.load_mut()?;

    require!(
        voter.deposits.len() > deposit_entry_index as usize,
        DepositEntryIndexOutOfBounds
    );
    let d = &mut voter.deposits[deposit_entry_index as usize];
    require!(d.is_used, DepositEntryIndexOutOfBounds);
    require!(d.amount_deposited_native == 0, VotingTokenNonZero);

    // Deposits that have clawback enabled are guaranteed to live until the end
    // of their locking period. That ensures a deposit can't be closed and reopenend
    // with a different locking kind or locking end time before funds are deposited.
    if d.allow_clawback {
        require!(
            d.lockup.end_ts < Clock::get()?.unix_timestamp,
            DepositStillLocked
        );
    }

    *d = DepositEntry::default();
    d.is_used = false;

    Ok(())
}
