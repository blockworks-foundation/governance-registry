use std::ops::DerefMut;

use crate::error::*;
use crate::state::*;
use anchor_lang::prelude::*;
use anchor_spl::token::{self, CloseAccount, Token, TokenAccount, Transfer};
use bytemuck::bytes_of_mut;

// Remaining accounts must be all the token token accounts owned by voter, he wants to close,
// they should be writable so that they can be closed and sol required for rent
// can then be sent back to the sol_destination
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
        has_one = registrar,
        close = sol_destination
    )]
    pub voter: AccountLoader<'info, Voter>,

    pub voter_authority: Signer<'info>,

    #[account(mut)]
    /// CHECK: Destination may be any address.
    pub sol_destination: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token>,
}

/// Closes the voter account (Optionally, also token vaults, as part of remaining_accounts),
/// allowing one to retrieve rent exemption SOL.
/// Only accounts with no remaining deposits can be closed.
pub fn close_voter<'key, 'accounts, 'remaining, 'info>(
    ctx: Context<'key, 'accounts, 'remaining, 'info, CloseVoter<'info>>,
) -> Result<()> {
    {
        let voter = ctx.accounts.voter.load()?;
        let amount = voter.deposits.iter().fold(0u64, |sum, d| {
            sum.checked_add(d.amount_deposited_native).unwrap()
        });
        require_eq!(amount, 0, VsrError::VotingTokenNonZero);

        let voter_seeds = voter_seeds!(voter);
        for account in &mut ctx.remaining_accounts.iter() {
            let token = Account::<TokenAccount>::try_from(&account.clone()).unwrap();
            require_keys_eq!(
                token.owner,
                ctx.accounts.voter.key(),
                VsrError::InvalidAuthority
            );
            require_eq!(token.amount, 0, VsrError::VaultTokenNonZero);

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
    }

    // zero out voter account to prevent reinit attacks
    // appease rust borrow checker
    {
        let mut voter = ctx.accounts.voter.load_mut()?;
        let voter_dereffed = voter.deref_mut();
        let voter_bytes = bytes_of_mut(voter_dereffed);
        voter_bytes.fill(0);
    }

    Ok(())
}

/// Closes the voter account, transfers all funds from token accounts and closes vaults.
/// Only accounts with no remaining lockups can be closed.
/// remaining_accounts: All voter vaults followed by target token accounts, in order.
pub fn close_voter_v2<'key, 'accounts, 'remaining, 'info>(
    ctx: Context<'key, 'accounts, 'remaining, 'info, CloseVoter<'info>>,
) -> Result<()> {
    let registrar = ctx.accounts.registrar.load()?;
    let curr_ts = registrar.clock_unix_timestamp();

    {
        let voter = ctx.accounts.voter.load()?;

        let active_deposit_entries = voter.deposits.iter().filter(|d| d.is_used).count();
        require_eq!(ctx.remaining_accounts.len(), active_deposit_entries * 2);

        let any_locked = voter.deposits.iter().any(|d| d.amount_locked(curr_ts) > 0);
        require!(!any_locked, VsrError::DepositStillLocked);

        let voter_seeds = voter_seeds!(voter);

        let active_deposits = voter.deposits.iter().filter(|d| d.is_used);
        let deposit_vaults = &ctx.remaining_accounts[..active_deposit_entries];
        let target_accounts = &ctx.remaining_accounts[active_deposit_entries..];

        for ((deposit, deposit_vault), target_account) in
            active_deposits.zip(deposit_vaults).zip(target_accounts)
        {
            let mint = &registrar.voting_mints[deposit.voting_mint_config_idx as usize].mint;

            let token = Account::<TokenAccount>::try_from(&deposit_vault.clone()).unwrap();
            require_keys_eq!(
                token.owner,
                ctx.accounts.voter.key(),
                VsrError::InvalidAuthority
            );
            require_keys_eq!(token.mint, *mint);

            // transfer to target_account
            let cpi_transfer_accounts = Transfer {
                from: deposit_vault.to_account_info(),
                to: target_account.to_account_info(),
                authority: ctx.accounts.voter.to_account_info(),
            };
            token::transfer(
                CpiContext::new_with_signer(
                    ctx.accounts.token_program.to_account_info(),
                    cpi_transfer_accounts,
                    &[voter_seeds],
                ),
                token.amount,
            )?;

            // close vault
            let cpi_close_accounts = CloseAccount {
                account: deposit_vault.to_account_info(),
                destination: ctx.accounts.sol_destination.to_account_info(),
                authority: ctx.accounts.voter.to_account_info(),
            };
            token::close_account(CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                cpi_close_accounts,
                &[voter_seeds],
            ))?;

            deposit_vault.exit(ctx.program_id)?;
        }
    }

    // zero out voter account to prevent reinit attacks
    // appease rust borrow checker
    {
        let mut voter = ctx.accounts.voter.load_mut()?;
        let voter_dereffed = voter.deref_mut();
        let voter_bytes = bytes_of_mut(voter_dereffed);
        voter_bytes.fill(0);
    }

    Ok(())
}
