use crate::error::*;
use crate::state::*;
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct InternalTransfer<'info> {
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
pub fn internal_transfer(
    ctx: Context<InternalTransfer>,
    source_deposit_entry_index: u8,
    target_deposit_entry_index: u8,
    amount: u64,
) -> Result<()> {
    let registrar = &ctx.accounts.registrar.load()?;
    let voter = &mut ctx.accounts.voter.load_mut()?;
    let curr_ts = registrar.clock_unix_timestamp();

    let source = voter.active_deposit_mut(source_deposit_entry_index)?;
    source.resolve_vesting(curr_ts)?;
    let source_seconds_left = source.lockup.seconds_left(curr_ts);
    let source_strictness = source.lockup.kind.strictness();
    let source_mint_idx = source.voting_mint_config_idx;

    // Allowing transfers from clawback-enabled deposits could be used to avoid
    // clawback by making proposal instructions target the wrong entry index.
    require!(!source.allow_clawback, InvalidDays);

    // Reduce source amounts
    require!(
        amount <= source.amount_deposited_native,
        InsufficientDepositedTokens
    );
    source.amount_deposited_native = source.amount_deposited_native.checked_sub(amount).unwrap();
    source.amount_initially_locked_native =
        source.amount_initially_locked_native.saturating_sub(amount);

    // Check target compatibility
    let target = voter.active_deposit_mut(target_deposit_entry_index)?;
    target.resolve_vesting(curr_ts)?;
    require!(
        target.voting_mint_config_idx == source_mint_idx,
        InvalidMint
    );
    require!(
        target.lockup.seconds_left(curr_ts) >= source_seconds_left,
        InvalidLockupPeriod
    );
    require!(
        target.lockup.kind.strictness() >= source_strictness,
        InvalidLockupKind
    );

    // Add target amounts
    target.amount_deposited_native = target.amount_deposited_native.checked_add(amount).unwrap();
    target.amount_initially_locked_native = target
        .amount_initially_locked_native
        .checked_add(amount)
        .unwrap();

    Ok(())
}
