use crate::error::*;
use crate::state::*;
use anchor_lang::prelude::*;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token::{Mint, Token, TokenAccount};

// Remaining accounts must be all the token mints that have registered
// as voting mints, including the newly registered one.
#[derive(Accounts)]
pub struct ConfigureVotingMint<'info> {
    #[account(mut, has_one = realm_authority)]
    pub registrar: AccountLoader<'info, Registrar>,
    pub realm_authority: Signer<'info>,

    /// Token account that all funds for this mint will be stored in
    #[account(
        init_if_needed,
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
/// * `idx`: index of the rate to be set
/// * `digit_shift`: how many digits to shift the native token amount, see below
/// * `deposit_scaled_factor`: vote weight factor for deposits, in 1/1e9 units
/// * `lockup_scaled_factor`: max extra weight for lockups, in 1/1e9 units
/// * `lockup_saturation_secs`: lockup duration at which the full vote weight
///   bonus is given to locked up deposits
///
/// This instruction can be called several times for the same mint and index to
/// change the voting mint configuration.
///
/// The vote weight for `amount` of native tokens will be
/// ```
/// vote_weight =
///     amount * 10^(digit_shift)
///            * (deposit_scaled_factor/1e9
///               + lockup_duration_factor * lockup_scaled_factor/1e9)
/// ```
/// where lockup_duration_factor is a value between 0 and 1, depending on how long
/// the amount is locked up. It is 1 when the lockup duration is greater or equal
/// lockup_saturation_secs.
///
/// Warning: Choose values that ensure that the vote weight will not overflow the
/// u64 limit! There is a check based on the supply of all configured mints, but
/// do your own checking too.
///
/// If you use a single mint, prefer digit_shift=0 and deposit_scaled_factor +
/// lockup_scaled_factor <= 1e9. That way you won't have issues with overflow no
/// matter the size of the mint's supply.
///
/// Digit shifting is particularly useful when using several voting token mints
/// that have a different number of decimals. It can be used to align them to
/// a common number of decimals.
///
/// Example: If you have token A with 6 decimals and token B with 9 decimals, you
/// could set up:
///    * A with digit_shift=0,  deposit_scaled_factor=2e9, lockup_scaled_factor=0
///    * B with digit_shift=-3, deposit_scaled_factor=1e9, lockup_scaled_factor=1e9
///
/// That would make 1.0 decimaled tokens of A as valuable as 2.0 decimaled tokens
/// of B when unlocked. B tokens could be locked up to double their vote weight. As
/// long as A's and B's supplies are below 2^63, there could be no overflow.
///
/// Note that in this example, you need 1000 native B tokens before receiving 1
/// unit of vote weight. If the supplies were significantly lower, you could use
///    * A with digit_shift=3, deposit_scaled_factor=2e9, lockup_scaled_factor=0
///    * B with digit_shift=0, deposit_scaled_factor=1e9, lockup_scaled_factor=1e9
/// to not lose precision on B tokens.
///
pub fn configure_voting_mint(
    ctx: Context<ConfigureVotingMint>,
    idx: u16,
    digit_shift: i8,
    deposit_scaled_factor: u64,
    lockup_scaled_factor: u64,
    lockup_saturation_secs: u64,
    grant_authority: Option<Pubkey>,
) -> Result<()> {
    require!(lockup_saturation_secs > 0, LockupSaturationMustBePositive);
    let registrar = &mut ctx.accounts.registrar.load_mut()?;
    let mint = ctx.accounts.mint.key();
    let existing_idx = registrar.voting_mint_config_index(mint);
    let idx = idx as usize;
    require!(
        idx < registrar.voting_mints.len(),
        OutOfBoundsVotingMintConfigIndex
    );
    let voting_mint_config = &mut registrar.voting_mints[idx];

    // Either it's a new mint for an unused index, or it's the correct old index.
    match existing_idx {
        Ok(existing_idx) => require!(existing_idx == idx, VotingMintConfiguredWithDifferentIndex),
        Err(_) => require!(
            !voting_mint_config.in_use(),
            VotingMintConfigIndexAlreadyInUse
        ),
    };

    *voting_mint_config = VotingMintConfig {
        mint,
        digit_shift,
        deposit_scaled_factor,
        lockup_scaled_factor,
        lockup_saturation_secs,
        grant_authority: grant_authority.unwrap_or(Pubkey::new_from_array([0; 32])),
        padding: [0; 31],
    };

    // Check for overflow in vote weight
    registrar.max_vote_weight(ctx.remaining_accounts)?;

    Ok(())
}
