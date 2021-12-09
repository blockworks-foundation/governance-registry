use crate::error::*;
use crate::state::*;
use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount};
use std::convert::TryFrom;

#[derive(Accounts)]
pub struct Deposit<'info> {
    pub registrar: AccountLoader<'info, Registrar>,

    // checking the PDA address it just an extra precaution,
    // the other constraints must be exhaustive
    #[account(
        mut,
        seeds = [registrar.key().as_ref(), b"voter".as_ref(), voter.load()?.voter_authority.key().as_ref()],
        bump = voter.load()?.voter_bump,
        has_one = registrar)]
    pub voter: AccountLoader<'info, Voter>,

    #[account(
        mut,
        associated_token::authority = registrar,
        associated_token::mint = deposit_token.mint,
    )]
    pub vault: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        constraint = deposit_token.owner == deposit_authority.key(),
    )]
    pub deposit_token: Box<Account<'info, TokenAccount>>,
    pub deposit_authority: Signer<'info>,

    pub token_program: Program<'info, Token>,
}

impl<'info> Deposit<'info> {
    pub fn transfer_ctx(&self) -> CpiContext<'_, '_, '_, 'info, token::Transfer<'info>> {
        let program = self.token_program.to_account_info();
        let accounts = token::Transfer {
            from: self.deposit_token.to_account_info(),
            to: self.vault.to_account_info(),
            authority: self.deposit_authority.to_account_info(),
        };
        CpiContext::new(program, accounts)
    }
}

/// Adds tokens to a deposit entry.
///
/// Tokens will be transfered from deposit_token to vault using the deposit_authority.
///
/// The deposit entry must have been initialized with create_deposit_entry.
///
/// `deposit_entry_index`: Index of the deposit entry.
/// `amount`: Number of native tokens to transfer.
pub fn deposit(ctx: Context<Deposit>, deposit_entry_index: u8, amount: u64) -> Result<()> {
    if amount == 0 {
        return Ok(());
    }

    let registrar = &ctx.accounts.registrar.load()?;
    let voter = &mut ctx.accounts.voter.load_mut()?;

    let d_entry = voter.active_deposit_mut(deposit_entry_index)?;

    // Get the exchange rate entry associated with this deposit.
    let mint_idx = registrar.voting_mint_config_index(ctx.accounts.deposit_token.mint)?;
    require!(
        mint_idx == d_entry.voting_mint_config_idx as usize,
        InvalidMint
    );

    // Adding funds to a lockup that is already in progress can be complicated
    // for linear vesting schedules because all added funds should be paid out
    // gradually over the remaining lockup duration.
    // The logic used is to:
    // - realize the vesting by reducing the locked amount and moving the start
    //   if the lockup forward by the number of expired vesting periods
    // - add the new funds to the locked up token count, so they will vest over
    //   the remaining periods.
    let curr_ts = registrar.clock_unix_timestamp();
    let vested_amount = d_entry.vested(curr_ts)?;
    assert!(vested_amount <= d_entry.amount_initially_locked_native);
    d_entry.amount_initially_locked_native -= vested_amount;
    d_entry.lockup.start_ts = d_entry
        .lockup
        .start_ts
        .checked_add(
            i64::try_from(d_entry.lockup.period_current(curr_ts)?)
                .unwrap()
                .checked_mul(d_entry.lockup.kind.period_secs())
                .unwrap(),
        )
        .unwrap();
    assert!(d_entry.lockup.start_ts <= d_entry.lockup.end_ts);
    assert!(d_entry.lockup.period_current(curr_ts)? == 0);

    // Deposit tokens into the registrar and increase the lockup amount too.
    token::transfer(ctx.accounts.transfer_ctx(), amount)?;
    d_entry.amount_deposited_native += amount;
    d_entry.amount_initially_locked_native += amount;

    let start_ts = d_entry.lockup.start_ts;
    let end_ts = d_entry.lockup.end_ts;
    msg!(
        "Deposited amount {} at deposit index {} with lockup kind {:?}, and start ts {}, end ts {}",
        amount,
        deposit_entry_index,
        d_entry.lockup.kind,
        start_ts,
        end_ts,
    );

    Ok(())
}
