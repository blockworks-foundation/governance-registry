use anchor_lang::__private::bytemuck::Zeroable;
use anchor_lang::prelude::*;

/// Exchange rate for an asset that can be used to mint voting rights.
#[zero_copy]
#[derive(AnchorSerialize, AnchorDeserialize, Default)]
pub struct VotingMintConfig {
    /// Mint for this entry.
    pub mint: Pubkey,

    /// Mint decimals.
    pub mint_decimals: u8,

    /// Exchange rate for 1.0 decimal-respecting unit of mint currency
    /// into the common vote currency.
    ///
    /// Example: If rate=2, then 1.000 of mint currency has a vote weight
    /// of 2.000000 in common vote currency. In the example mint decimals
    /// was 3 and common_decimals was 6.
    pub rate: u64,

    /// Factor for converting mint native currency to common vote currency,
    /// including decimal handling.
    ///
    /// Examples:
    /// - if common and mint have the same number of decimals, this is the same as 'rate'
    /// - common decimals = 6, mint decimals = 3, rate = 5 -> 500
    pub conversion_factor: u64,

    /// The authority that is allowed to push grants into voters
    pub grant_authority: Pubkey,
}

impl VotingMintConfig {
    /// Converts an amount in this voting mints's native currency
    /// to the equivalent common registrar vote currency amount.
    pub fn convert(&self, amount_native: u64) -> u64 {
        amount_native.checked_mul(self.conversion_factor).unwrap()
    }
}

unsafe impl Zeroable for VotingMintConfig {}
