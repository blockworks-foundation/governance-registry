use crate::error::*;
use crate::instructions::create_voter::is_freshly_initialized;
use crate::state::*;
use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount};
use spl_governance::addins::voter_weight::VoterWeightAccountType;
use std::mem::size_of;

#[derive(Accounts)]
#[instruction(
    voter_bump: u8,
    voter_weight_record_bump: u8,
    kind: LockupKind,
    periods: u32,
    allow_clawback: bool,
    amount: u64,
)]
pub struct Grant<'info> {
    pub registrar: AccountLoader<'info, Registrar>,

    #[account(
        init_if_needed,
        seeds = [registrar.key().as_ref(), b"voter".as_ref(), voter_authority.key().as_ref()],
        bump = voter_bump,
        payer = authority,
        space = 8 + size_of::<Voter>(),
    )]
    pub voter: AccountLoader<'info, Voter>,

    /// The account of the grantee / the address controlling the voter
    /// that the grant is going to.
    pub voter_authority: UncheckedAccount<'info>,

    /// The voter weight record is the account that will be shown to spl-governance
    /// to prove how much vote weight the voter has. See update_voter_weight_record.
    #[account(
        init_if_needed,
        seeds = [registrar.key().as_ref(), b"voter-weight-record".as_ref(), voter_authority.key().as_ref()],
        bump = voter_weight_record_bump,
        payer = authority,
        space = 150,
    )]
    pub voter_weight_record: Account<'info, VoterWeightRecord>,

    #[account(
        mut,
        associated_token::authority = registrar,
        associated_token::mint = deposit_token.mint,
    )]
    pub vault: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        constraint = deposit_token.owner == authority.key(),
    )]
    pub deposit_token: Box<Account<'info, TokenAccount>>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub rent: Sysvar<'info, Rent>,
}

impl<'info> Grant<'info> {
    pub fn transfer_ctx(&self) -> CpiContext<'_, '_, '_, 'info, token::Transfer<'info>> {
        let program = self.token_program.to_account_info();
        let accounts = token::Transfer {
            from: self.deposit_token.to_account_info(),
            to: self.vault.to_account_info(),
            authority: self.authority.to_account_info(),
        };
        CpiContext::new(program, accounts)
    }
}

/// Gives a grant to a voter.
///
/// The voter may or may not exist in advance.
/// Creates a new deposit entry -- errors if no free ones are available.
pub fn grant(
    ctx: Context<Grant>,
    voter_bump: u8,
    voter_weight_record_bump: u8,
    kind: LockupKind,
    periods: u32,
    allow_clawback: bool,
    amount: u64,
) -> Result<()> {
    // Load accounts.
    let registrar = &ctx.accounts.registrar.load()?;
    let voter_authority = ctx.accounts.voter_authority.key();

    // Get the exchange rate entry associated with this deposit.
    let mint_idx = registrar.voting_mint_config_index(ctx.accounts.deposit_token.mint)?;
    let mint_config = &registrar.voting_mints[mint_idx];

    let authority = ctx.accounts.authority.key();
    require!(
        authority == registrar.realm_authority || authority == mint_config.grant_authority,
        InvalidAuthority
    );

    // Init the voter if it hasn't been already.
    let new_voter = is_freshly_initialized(ctx.accounts.voter.as_ref())?;
    let mut voter = if new_voter {
        ctx.accounts.voter.load_init()?
    } else {
        ctx.accounts.voter.load_mut()?
    };
    if new_voter {
        voter.voter_bump = voter_bump;
        voter.voter_weight_record_bump = voter_weight_record_bump;
        voter.voter_authority = voter_authority;
        voter.registrar = ctx.accounts.registrar.key();

        // Initializing the voter weight record exactly when setting up the voter is fine.
        // Note that vote_weight_record is not an Anchor account, is_freshly_initialized()
        // would not work.
        let voter_weight_record = &mut ctx.accounts.voter_weight_record;
        voter_weight_record.account_type = VoterWeightAccountType::VoterWeightRecord;
        voter_weight_record.realm = registrar.realm;
        voter_weight_record.governing_token_mint = registrar.realm_governing_token_mint;
        voter_weight_record.governing_token_owner = voter_authority;
    }

    // Get and init the first free deposit entry.
    let free_entry_idx = voter
        .deposits
        .iter()
        .position(|d_entry| !d_entry.is_used)
        .ok_or(ErrorCode::DepositEntryFull)?;
    let d_entry = &mut voter.deposits[free_entry_idx];

    // Set up a deposit.
    *d_entry = DepositEntry::default();
    d_entry.is_used = true;
    d_entry.voting_mint_config_idx = mint_idx as u8;
    d_entry.allow_clawback = allow_clawback;
    d_entry.lockup = Lockup::new_from_periods(kind, registrar.clock_unix_timestamp(), periods)?;

    // Deposit tokens, locking them all.
    token::transfer(ctx.accounts.transfer_ctx(), amount)?;
    d_entry.amount_deposited_native = amount;
    d_entry.amount_initially_locked_native = amount;

    let start_ts = d_entry.lockup.start_ts;
    let end_ts = d_entry.lockup.end_ts;
    msg!(
        "Granted amount {} at deposit index {} with lockup kind {:?}, and start ts {}, end ts {}",
        amount,
        free_entry_idx,
        d_entry.lockup.kind,
        start_ts,
        end_ts,
    );

    Ok(())
}
