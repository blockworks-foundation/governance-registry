use crate::error::*;
use crate::state::*;
use anchor_lang::prelude::*;
use anchor_spl::token::Mint;

#[derive(Accounts)]
pub struct CreateDepositEntry<'info> {
    pub registrar: Box<Account<'info, Registrar>>,

    #[account(mut, has_one = registrar, has_one = voter_authority)]
    pub voter: AccountLoader<'info, Voter>,
    pub voter_authority: Signer<'info>,

    pub deposit_mint: Box<Account<'info, Mint>>,
}

/// Creates a new deposit entry.
///
/// Initializes a deposit entry with the requested settings.
/// Will error if the deposit entry is already in use.
///
/// `deposit_entry_index`: deposit entry to use
/// `kind`: Type of lockup to use.
/// `period`: How long to lock up, depending on `kind`. See LockupKind::period_secs()
/// `allow_clawback`: When enabled, the the clawback_authority is allowed to
///                   unilaterally claim locked tokens.
pub fn create_deposit_entry(
    ctx: Context<CreateDepositEntry>,
    deposit_entry_index: u8,
    kind: LockupKind,
    periods: i32,
    allow_clawback: bool,
) -> Result<()> {
    msg!("--------create_deposit_entry--------");

    // Load accounts.
    let registrar = &ctx.accounts.registrar;
    let voter = &mut ctx.accounts.voter.load_mut()?;

    // Set the lockup start timestamp.
    let start_ts = registrar.clock_unix_timestamp();

    // Get the exchange rate entry associated with this deposit.
    let er_idx = registrar
        .rates
        .iter()
        .position(|r| r.mint == ctx.accounts.deposit_mint.key())
        .ok_or(ErrorCode::ExchangeRateEntryNotFound)?;

    // Get and set up the deposit entry.
    require!(
        voter.deposits.len() > deposit_entry_index as usize,
        DepositEntryIndexOutOfBounds
    );
    let d_entry = &mut voter.deposits[deposit_entry_index as usize];
    require!(!d_entry.is_used, DepositEntryIndexOutOfBounds);

    *d_entry = DepositEntry::default();
    d_entry.is_used = true;
    d_entry.rate_idx = er_idx as u8;
    d_entry.amount_deposited_native = 0;
    d_entry.amount_initially_locked_native = 0;
    d_entry.allow_clawback = allow_clawback;
    d_entry.lockup = Lockup {
        kind,
        start_ts,
        end_ts: start_ts
            .checked_add(i64::from(periods).checked_mul(kind.period_secs()).unwrap())
            .unwrap(),
        padding: [0u8; 16],
    };

    Ok(())
}
