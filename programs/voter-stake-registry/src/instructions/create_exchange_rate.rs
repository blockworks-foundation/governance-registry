use crate::error::*;
use crate::state::*;
use anchor_lang::prelude::*;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token::{Mint, Token, TokenAccount};

#[derive(Accounts)]
#[instruction(idx: u16, rate: u64, decimals: u8)]
pub struct CreateExchangeRate<'info> {
    #[account(mut, has_one = realm_authority)]
    pub registrar: Box<Account<'info, Registrar>>,
    pub realm_authority: Signer<'info>,

    /// Token account that all funds for this mint will be stored in
    #[account(
        init,
        payer = payer,
        associated_token::authority = registrar,
        associated_token::mint = mint,
    )]
    pub vault: Account<'info, TokenAccount>,
    /// Tokens of this mint will produce vote weight
    pub mint: Account<'info, Mint>,

    #[account(mut)]
    pub payer: Signer<'info>,

    pub rent: Sysvar<'info, Rent>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

/// Creates a new exchange rate for a given mint. This allows a voter to
/// deposit the mint in exchange for vote weight. There can only be a single
/// exchange rate per mint.
///
/// `idx`: index of the rate to be set
/// `rate`: multiplier to apply for converting tokens to vote weight
/// `decimals`: number of decimals of mint that make one unit of token
///
/// The vote weight for one native token will be:
/// ```
/// rate * 10^vote_weight_decimals / 10^decimals
/// ```
pub fn create_exchange_rate(
    ctx: Context<CreateExchangeRate>,
    idx: u16,
    rate: u64,
    decimals: u8,
) -> Result<()> {
    msg!("--------create_exchange_rate--------");
    require!(rate > 0, InvalidRate);
    let registrar = &mut ctx.accounts.registrar;
    require!((idx as usize) < registrar.rates.len(), InvalidIndex);
    require!(
        registrar.rates[idx as usize].rate == 0,
        RateAtIndexAlreadySet
    );
    registrar.rates[idx as usize] = registrar.new_rate(ctx.accounts.mint.key(), decimals, rate)?;

    // TODO housekeeping check that rate for a particular mint is set only once

    Ok(())
}
