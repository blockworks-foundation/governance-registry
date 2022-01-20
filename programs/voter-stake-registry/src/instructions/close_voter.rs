use crate::error::*;
use crate::state::*;
use anchor_lang::prelude::*;
use anchor_spl::token::{self, CloseAccount, Token, TokenAccount};

#[derive(Accounts)]
pub struct CloseVoter<'info> {
    pub registrar: AccountLoader<'info, Registrar>,

    // checking the PDA address it just an extra precaution,
    // the other constraints must be exhaustive
    #[account(
        mut,
        seeds = [voter.load()?.registrar.key().as_ref(), b"voter".as_ref(), voter_authority.key().as_ref()],
        bump = voter.load()?.voter_bump,
        has_one = voter_authority,
        close = sol_destination)]
    pub voter: AccountLoader<'info, Voter>,

    #[account(mut)]
    pub voter_authority: Signer<'info>,

    #[account(mut)]
    pub sol_destination: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token>,
}

/// Closes the voter account (Optionally, also token vaults, as part of remaining_accounts),
/// allowing one to retrieve rent exemption SOL.
/// Only accounts with no remaining deposits can be closed.
pub fn close_voter<'key, 'accounts, 'remaining, 'info>(
    ctx: Context<'key, 'accounts, 'remaining, 'info, CloseVoter<'info>>,
) -> Result<()> {
    let voter = &ctx.accounts.voter.load()?;
    let amount = voter.deposits.iter().fold(0u64, |sum, d| {
        sum.checked_add(d.amount_deposited_native).unwrap()
    });
    require!(amount == 0, VotingTokenNonZero);

    let voter_seeds = voter_seeds!(voter);
    for account in &mut ctx.remaining_accounts.iter() {
        let token = Account::<TokenAccount>::try_from(&account.clone()).unwrap();
        require!(token.owner == ctx.accounts.voter.key(), InvalidAuthority);
        require!(token.amount == 0, VaultTokenNonZero);

        let cpi_accounts = CloseAccount {
            account: account.to_account_info(),
            destination: ctx.accounts.sol_destination.to_account_info(),
            authority: ctx.accounts.voter.to_account_info(),
        };
        let cpi_program = ctx.accounts.token_program.to_account_info();
        token::close_account(CpiContext::new_with_signer(
            cpi_program,
            cpi_accounts,
            &[voter_seeds],
        ))?;

        account.exit(ctx.program_id)?;
    }

    Ok(())
}
