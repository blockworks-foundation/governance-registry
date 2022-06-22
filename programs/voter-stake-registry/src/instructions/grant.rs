use crate::error::*;
use crate::state::*;
use anchor_lang::prelude::*;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token::{self, Mint, Token, TokenAccount};
use std::convert::TryFrom;
use std::mem::size_of;

#[derive(Accounts)]
pub struct Grant<'info> {
    pub registrar: AccountLoader<'info, Registrar>,

    #[account(
        init_if_needed,
        seeds = [registrar.key().as_ref(), b"voter".as_ref(), voter_authority.key().as_ref()],
        bump,
        payer = payer,
        space = 8 + size_of::<Voter>(),
    )]
    pub voter: AccountLoader<'info, Voter>,

    /// The account of the grantee / the address controlling the voter
    /// that the grant is going to.
    /// CHECK: May be any address
    pub voter_authority: UncheckedAccount<'info>,

    /// The voter weight record is the account that will be shown to spl-governance
    /// to prove how much vote weight the voter has. See update_voter_weight_record.
    #[account(
        init_if_needed,
        seeds = [registrar.key().as_ref(), b"voter-weight-record".as_ref(), voter_authority.key().as_ref()],
        bump,
        payer = payer,
        space = size_of::<VoterWeightRecord>(),
    )]
    pub voter_weight_record: Account<'info, VoterWeightRecord>,

    #[account(
        init_if_needed,
        associated_token::authority = voter,
        associated_token::mint = deposit_mint,
        payer = payer
    )]
    pub vault: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        constraint = deposit_token.owner == token_authority.key(),
        constraint = deposit_token.mint == deposit_mint.key(),
    )]
    pub deposit_token: Box<Account<'info, TokenAccount>>,

    /// Authority for transfering tokens away from deposit_token
    pub token_authority: Signer<'info>,

    /// Authority for making a grant to this voter account
    ///
    /// Verification inline in instruction
    pub grant_authority: Signer<'info>,

    /// Rent payer if a new account is to be created
    #[account(mut)]
    pub payer: Signer<'info>,

    pub deposit_mint: Box<Account<'info, Mint>>,

    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub rent: Sysvar<'info, Rent>,
}

impl<'info> Grant<'info> {
    pub fn transfer_ctx(&self) -> CpiContext<'_, '_, '_, 'info, token::Transfer<'info>> {
        let program = self.token_program.to_account_info();
        let accounts = token::Transfer {
            from: self.deposit_token.to_account_info(),
            to: self.vault.to_account_info(),
            authority: self.token_authority.to_account_info(),
        };
        CpiContext::new(program, accounts)
    }
}

/// Returns if the anchor discriminator on the account is still unset
pub fn is_freshly_initialized(account_info: &AccountInfo) -> Result<bool> {
    let data = account_info.try_borrow_data()?;
    let mut disc_bytes = [0u8; 8];
    disc_bytes.copy_from_slice(&data[..8]);
    let discriminator = u64::from_le_bytes(disc_bytes);
    Ok(discriminator == 0)
}

/// Gives a grant to a voter.
///
/// The voter may or may not exist in advance.
/// Creates a new deposit entry -- errors if no free ones are available.
#[allow(clippy::too_many_arguments)]
pub fn grant(
    ctx: Context<Grant>,
    voter_bump: u8,
    voter_weight_record_bump: u8,
    kind: LockupKind,
    start_ts: Option<u64>,
    periods: u32,
    allow_clawback: bool,
    amount: u64,
) -> Result<()> {
    require_eq!(voter_bump, *ctx.bumps.get("voter").unwrap());
    require_eq!(
        voter_weight_record_bump,
        *ctx.bumps.get("voter_weight_record").unwrap()
    );

    // Load accounts.
    let registrar = &ctx.accounts.registrar.load()?;
    let voter_authority = ctx.accounts.voter_authority.key();

    // Get the exchange rate entry associated with this deposit.
    let mint_idx = registrar.voting_mint_config_index(ctx.accounts.deposit_token.mint)?;
    let mint_config = &registrar.voting_mints[mint_idx];

    // The grant instruction creates a new deposit entry for the target voter. This is a
    // limited resource. If anyone could call "grant" then it could be used for denial of
    // service by filling all deposit entries with tiny amounts and long lockup times.
    let grant_authority = ctx.accounts.grant_authority.key();
    require!(
        grant_authority == registrar.realm_authority
            || grant_authority == mint_config.grant_authority
            || grant_authority == voter_authority,
        VsrError::InvalidAuthority
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
        voter_weight_record.account_discriminator =
            spl_governance_addin_api::voter_weight::VoterWeightRecord::ACCOUNT_DISCRIMINATOR;
        voter_weight_record.realm = registrar.realm;
        voter_weight_record.governing_token_mint = registrar.realm_governing_token_mint;
        voter_weight_record.governing_token_owner = voter_authority;
    }

    // Get and init the first free deposit entry.
    let free_entry_idx = voter
        .deposits
        .iter()
        .position(|d_entry| !d_entry.is_used)
        .ok_or(VsrError::DepositEntryFull)?;
    let d_entry = &mut voter.deposits[free_entry_idx];

    let curr_ts = registrar.clock_unix_timestamp();
    let start_ts = if let Some(v) = start_ts {
        i64::try_from(v).unwrap()
    } else {
        curr_ts
    };

    // Set up a deposit.
    *d_entry = DepositEntry::default();
    d_entry.is_used = true;
    d_entry.voting_mint_config_idx = mint_idx as u8;
    d_entry.allow_clawback = allow_clawback;
    d_entry.lockup = Lockup::new_from_periods(kind, curr_ts, start_ts, periods)?;

    // Deposit tokens, locking them all.
    token::transfer(ctx.accounts.transfer_ctx(), amount)?;
    d_entry.amount_deposited_native = amount;
    d_entry.amount_initially_locked_native = amount;

    msg!(
        "Granted amount {} at deposit index {} with lockup kind {:?} for {} periods",
        amount,
        free_entry_idx,
        d_entry.lockup.kind,
        periods,
    );

    Ok(())
}
