use crate::account::*;
use anchor_lang::prelude::*;
use anchor_lang::solana_program::sysvar::instructions as tx_instructions;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token::{self, Mint, Token, TokenAccount};
use std::mem::size_of;

pub const VOTER_WEIGHT_RECORD: [u8; 19] = *b"voter-weight-record";

#[derive(Accounts)]
#[instruction(vote_weight_decimals: u8, registrar_bump: u8)]
pub struct CreateRegistrar<'info> {
    #[account(
        init,
        seeds = [realm.key().as_ref()],
        bump = registrar_bump,
        payer = payer,
        space = 8 + size_of::<Registrar>()
    )]
    /// a voting registrar. There can only be a single registrar
    /// per governance realm.
    pub registrar: AccountLoader<'info, Registrar>,
    pub governance_program_id: UncheckedAccount<'info>,
    // Unsafe and untrusted. This instruction needs to be invoked immediately
    // after the realm is created.
    // TODO can't we ensure that a realm owner can call this instruction?
    pub realm: UncheckedAccount<'info>,
    // TODO what about council mint?
    pub realm_community_mint: Account<'info, Mint>,
    // TODO: can't this be the realm?
    pub registrar_authority: UncheckedAccount<'info>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
#[instruction(voter_bump: u8, voter_weight_record_bump: u8)]
pub struct CreateVoter<'info> {
    #[account(
        init,
        seeds = [registrar.key().as_ref(), voter_authority.key().as_ref()],
        bump = voter_bump,
        payer = voter_authority,
        space = 8 + size_of::<Voter>(),
    )]
    pub voter: AccountLoader<'info, Voter>,
    #[account(
        init,
        seeds = [VOTER_WEIGHT_RECORD.as_ref(), registrar.key().as_ref(), voter_authority.key().as_ref()],
        bump = voter_weight_record_bump,
        payer = payer,
        space = 150,
    )]
    pub voter_weight_record: Account<'info, VoterWeightRecord>,
    pub registrar: AccountLoader<'info, Registrar>,

    // TODO: Why is authority and payer different? Is it necessary that Voter and VoterWeightRecord are paid for differently?
    #[account(mut)]
    pub voter_authority: Signer<'info>,
    #[account(mut)]
    pub payer: Signer<'info>,

    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
    #[account(address = tx_instructions::ID)]
    pub instructions: UncheckedAccount<'info>,
}

#[derive(Accounts)]
#[instruction(idx: u16, mint: Pubkey, rate: u64, decimals: u8)]
pub struct CreateExchangeRate<'info> {
    #[account(
        init,
        payer = payer,
        associated_token::authority = registrar,
        associated_token::mint = deposit_mint,
    )]
    pub exchange_vault: Account<'info, TokenAccount>,
    #[account(
        init,
        seeds = [registrar.key().as_ref(), deposit_mint.key().as_ref()],
        bump,
        payer = payer,
        mint::authority = registrar,
        mint::freeze_authority = registrar,
        mint::decimals = deposit_mint.decimals,
    )]
    pub voting_mint: Account<'info, Mint>,
    pub deposit_mint: Account<'info, Mint>,
    #[account(mut, has_one = registrar_authority)]
    pub registrar: AccountLoader<'info, Registrar>,
    #[account(mut)]
    pub registrar_authority: Signer<'info>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub rent: Sysvar<'info, Rent>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct CreateDeposit<'info> {
    pub deposit: UpdateDeposit<'info>,
}

#[derive(Accounts)]
pub struct UpdateDeposit<'info> {
    pub registrar: AccountLoader<'info, Registrar>,
    #[account(mut, has_one = voter_authority, has_one = registrar)]
    pub voter: AccountLoader<'info, Voter>,
    #[account(
        mut,
        associated_token::authority = registrar,
        associated_token::mint = deposit_mint,
    )]
    pub exchange_vault: Account<'info, TokenAccount>,
    #[account(
        mut,
        constraint = deposit_token.mint == deposit_mint.key(),
    )]
    pub deposit_token: Account<'info, TokenAccount>,
    #[account(
        init_if_needed,
        payer = voter_authority,
        associated_token::authority = voter_authority,
        associated_token::mint = voting_mint,
    )]
    pub voting_token: Account<'info, TokenAccount>,
    #[account(mut)]
    pub voter_authority: Signer<'info>,
    pub deposit_mint: Account<'info, Mint>,
    #[account(
        mut,
        seeds = [registrar.key().as_ref(), deposit_token.mint.as_ref()],
        bump,
    )]
    pub voting_mint: Account<'info, Mint>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

impl<'info> UpdateDeposit<'info> {
    pub fn transfer_ctx(&self) -> CpiContext<'_, '_, '_, 'info, token::Transfer<'info>> {
        let program = self.token_program.to_account_info();
        let accounts = token::Transfer {
            from: self.deposit_token.to_account_info(),
            to: self.exchange_vault.to_account_info(),
            authority: self.voter_authority.to_account_info(),
        };
        CpiContext::new(program, accounts)
    }

    pub fn thaw_ctx(&self) -> CpiContext<'_, '_, '_, 'info, token::ThawAccount<'info>> {
        let program = self.token_program.to_account_info();
        let accounts = token::ThawAccount {
            account: self.voting_token.to_account_info(),
            mint: self.voting_mint.to_account_info(),
            authority: self.registrar.to_account_info(),
        };
        CpiContext::new(program, accounts)
    }

    pub fn mint_to_ctx(&self) -> CpiContext<'_, '_, '_, 'info, token::MintTo<'info>> {
        let program = self.token_program.to_account_info();
        let accounts = token::MintTo {
            mint: self.voting_mint.to_account_info(),
            to: self.voting_token.to_account_info(),
            authority: self.registrar.to_account_info(),
        };
        CpiContext::new(program, accounts)
    }

    pub fn freeze_ctx(&self) -> CpiContext<'_, '_, '_, 'info, token::FreezeAccount<'info>> {
        let program = self.token_program.to_account_info();
        let accounts = token::FreezeAccount {
            account: self.voting_token.to_account_info(),
            mint: self.voting_mint.to_account_info(),
            authority: self.registrar.to_account_info(),
        };
        CpiContext::new(program, accounts)
    }
}

#[derive(Accounts)]
pub struct Withdraw<'info> {
    pub registrar: AccountLoader<'info, Registrar>,
    #[account(mut, has_one = registrar, has_one = voter_authority)]
    pub voter: AccountLoader<'info, Voter>,
    pub token_owner_record: UncheckedAccount<'info>,
    #[account(
        mut,
        associated_token::authority = registrar,
        associated_token::mint = withdraw_mint,
    )]
    pub exchange_vault: Account<'info, TokenAccount>,
    pub withdraw_mint: Account<'info, Mint>,
    #[account(
        mut,
        associated_token::authority = voter_authority,
        associated_token::mint = voting_mint,
    )]
    pub voting_token: Account<'info, TokenAccount>,
    #[account(
        mut,
        seeds = [registrar.key().as_ref(), withdraw_mint.key().as_ref()],
        bump,
    )]
    pub voting_mint: Account<'info, Mint>,
    #[account(mut)]
    pub destination: Account<'info, TokenAccount>,
    pub voter_authority: Signer<'info>,
    pub token_program: Program<'info, Token>,
}

impl<'info> Withdraw<'info> {
    pub fn transfer_ctx(&self) -> CpiContext<'_, '_, '_, 'info, token::Transfer<'info>> {
        let program = self.token_program.to_account_info();
        let accounts = token::Transfer {
            from: self.exchange_vault.to_account_info(),
            to: self.destination.to_account_info(),
            authority: self.registrar.to_account_info(),
        };
        CpiContext::new(program, accounts)
    }

    pub fn thaw_ctx(&self) -> CpiContext<'_, '_, '_, 'info, token::ThawAccount<'info>> {
        let program = self.token_program.to_account_info();
        let accounts = token::ThawAccount {
            account: self.voting_token.to_account_info(),
            mint: self.voting_mint.to_account_info(),
            authority: self.registrar.to_account_info(),
        };
        CpiContext::new(program, accounts)
    }

    pub fn burn_ctx(&self) -> CpiContext<'_, '_, '_, 'info, token::Burn<'info>> {
        let program = self.token_program.to_account_info();
        let accounts = token::Burn {
            mint: self.voting_mint.to_account_info(),
            to: self.voting_token.to_account_info(),
            authority: self.voter_authority.to_account_info(),
        };
        CpiContext::new(program, accounts)
    }

    pub fn freeze_ctx(&self) -> CpiContext<'_, '_, '_, 'info, token::FreezeAccount<'info>> {
        let program = self.token_program.to_account_info();
        let accounts = token::FreezeAccount {
            account: self.voting_token.to_account_info(),
            mint: self.voting_mint.to_account_info(),
            authority: self.registrar.to_account_info(),
        };
        CpiContext::new(program, accounts)
    }
}

#[derive(Accounts)]
pub struct CloseDeposit<'info> {
    #[account(mut, has_one = voter_authority)]
    pub voter: AccountLoader<'info, Voter>,
    pub voter_authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct UpdateSchedule<'info> {
    pub registrar: AccountLoader<'info, Registrar>,
    #[account(mut, has_one = voter_authority, has_one = registrar)]
    pub voter: AccountLoader<'info, Voter>,
    pub voter_authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct UpdateVoterWeightRecord<'info> {
    pub registrar: AccountLoader<'info, Registrar>,
    #[account(
        has_one = registrar,
        has_one = voter_authority,
    )]
    pub voter: AccountLoader<'info, Voter>,
    #[account(
        mut,
        seeds = [VOTER_WEIGHT_RECORD.as_ref(), registrar.key().as_ref(), voter_authority.key().as_ref()],
        bump = voter.load()?.voter_weight_record_bump,
        constraint = voter_weight_record.realm == registrar.load()?.realm,
        constraint = voter_weight_record.governing_token_owner == voter.load()?.voter_authority,
    )]
    pub voter_weight_record: Account<'info, VoterWeightRecord>,
    pub voter_authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

// Remaining accounts should all the token mints that have registered
// exchange rates.
#[derive(Accounts)]
pub struct UpdateMaxVoteWeight<'info> {
    pub registrar: AccountLoader<'info, Registrar>,
    // TODO: SPL governance has not yet implemented this.
    pub max_vote_weight_record: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct CloseVoter<'info> {
    #[account(mut, has_one = voter_authority, close = sol_destination)]
    pub voter: AccountLoader<'info, Voter>,
    pub voter_authority: Signer<'info>,
    pub sol_destination: UncheckedAccount<'info>,
}

#[derive(Accounts)]
#[instruction(time_offset: i64)]
pub struct SetTimeOffset<'info> {
    #[account(mut, has_one = registrar_authority)]
    pub registrar: AccountLoader<'info, Registrar>,
    pub registrar_authority: Signer<'info>,
}
