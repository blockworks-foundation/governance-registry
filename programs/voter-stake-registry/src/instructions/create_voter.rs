use crate::error::*;
use crate::state::*;
use anchor_lang::prelude::*;
use anchor_lang::solana_program::sysvar::instructions as tx_instructions;
use spl_governance::addins::voter_weight::VoterWeightAccountType;
use spl_governance::state::token_owner_record;
use std::mem::size_of;

pub const VOTER_WEIGHT_RECORD: [u8; 19] = *b"voter-weight-record";

#[derive(Accounts)]
#[instruction(voter_bump: u8, voter_weight_record_bump: u8)]
pub struct CreateVoter<'info> {
    pub registrar: Box<Account<'info, Registrar>>,

    #[account(
        init,
        seeds = [registrar.key().as_ref(), b"voter".as_ref(), voter_authority.key().as_ref()],
        bump = voter_bump,
        payer = payer,
        space = 8 + size_of::<Voter>(),
    )]
    pub voter: AccountLoader<'info, Voter>,
    // TODO would be this different than payer?
    pub voter_authority: Signer<'info>,

    /// The token_owner_record for the voter_authority.
    ///
    /// FIXME the instruction does read and verify it
    /// The instruction does not read it, but providing it here ensures that
    /// users don't accidentally create voters and deposit funds without having
    /// a valid token_owner_record. It is impossible to withdraw funds without
    /// it.
    ///
    /// token_owner_record is validated in the instruction:
    /// - owned by registrar.governance_program_id
    /// - for the registrar.realm
    /// - for the registrar.realm_governing_token_mint
    /// - governing_token_owner is voter_authority
    pub token_owner_record: UncheckedAccount<'info>,

    /// The voter weight record is the account that will be shown to spl-governance
    /// to prove how much vote weight the voter has. See update_voter_weight_record.
    #[account(
        init,
        seeds = [VOTER_WEIGHT_RECORD.as_ref(), registrar.key().as_ref(), voter_authority.key().as_ref()],
        bump = voter_weight_record_bump,
        payer = payer,
        space = 150,
    )]
    // TODO why not space = 8 + size_of::<VoterWeightRecord>(),
    pub voter_weight_record: Account<'info, VoterWeightRecord>,

    #[account(mut)]
    pub payer: Signer<'info>,

    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,

    #[account(address = tx_instructions::ID)]
    pub instructions: UncheckedAccount<'info>,
}

/// Creates a new voter account. There can only be a single voter per
/// voter_authority.
///
/// The user must register with spl-governance using the same voter_authority.
/// Their token owner record will be required for withdrawing funds later.
pub fn create_voter(
    ctx: Context<CreateVoter>,
    voter_bump: u8,
    voter_weight_record_bump: u8,
) -> Result<()> {
    msg!("--------create_voter--------");
    // Forbid creating voter accounts from CPI. The goal is to make automation
    // impossible that weakens some of the limitations intentionally imposed on
    // locked tokens.
    // TODO e.g. secondary market for locked tokens
    {
        let ixns = ctx.accounts.instructions.to_account_info();
        let current_index = tx_instructions::load_current_index_checked(&ixns)? as usize;
        let current_ixn = tx_instructions::load_instruction_at_checked(current_index, &ixns)?;
        require!(
            current_ixn.program_id == *ctx.program_id,
            ErrorCode::ForbiddenCpi
        );
    }

    // Load accounts.
    let registrar = &ctx.accounts.registrar;
    let voter = &mut ctx.accounts.voter.load_init()?;
    let voter_authority = ctx.accounts.voter_authority.key();
    let voter_weight_record = &mut ctx.accounts.voter_weight_record;

    // Validate the token owner record.
    let token_owner_record_data =
        token_owner_record::get_token_owner_record_data_for_realm_and_governing_mint(
            &registrar.governance_program_id,
            &ctx.accounts.token_owner_record.to_account_info(),
            &registrar.realm,
            &registrar.realm_governing_token_mint,
        )?;
    require!(
        token_owner_record_data.governing_token_owner == voter_authority,
        InvalidTokenOwnerRecord
    );

    // Init the voter.
    voter.voter_bump = voter_bump;
    voter.voter_weight_record_bump = voter_weight_record_bump;
    voter.voter_authority = voter_authority;
    voter.registrar = ctx.accounts.registrar.key();

    // Init the voter weight record.
    voter_weight_record.account_type = VoterWeightAccountType::VoterWeightRecord;
    voter_weight_record.realm = registrar.realm;
    voter_weight_record.governing_token_mint = registrar.realm_governing_token_mint;
    voter_weight_record.governing_token_owner = voter_authority;

    Ok(())
}
