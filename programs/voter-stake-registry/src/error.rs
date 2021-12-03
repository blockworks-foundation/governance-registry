use anchor_lang::prelude::*;

#[error]
pub enum ErrorCode {
    #[msg("Exchange rate must be greater than zero")]
    InvalidRate, // 300
    #[msg("")]
    RatesFull,
    #[msg("")]
    ExchangeRateEntryNotFound, // 302
    #[msg("")]
    DepositEntryNotFound,
    #[msg("")]
    DepositEntryFull, // 304
    #[msg("")]
    VotingTokenNonZero,
    #[msg("")]
    DepositEntryIndexOutOfBounds, // 306
    #[msg("")]
    DepositEntryIndexAlreadInUse, // 307
    #[msg("")]
    UnusedDepositEntryIndex, // 308
    #[msg("")]
    InsufficientVestedTokens, // 309
    #[msg("")]
    UnableToConvert,
    #[msg("")]
    InvalidLockupPeriod,
    #[msg("")]
    InvalidEndTs,
    #[msg("")]
    InvalidDays,
    #[msg("")]
    RateAtIndexAlreadySet,
    #[msg("")]
    InvalidIndex,
    #[msg("Exchange rate decimals cannot be larger than registrar decimals")]
    InvalidDecimals,
    #[msg("")]
    InvalidToDepositAndWithdrawInOneSlot,
    #[msg("")]
    ForbiddenCpi,
    #[msg("")]
    InvalidMint,
    #[msg("")]
    DebugInstruction,
    #[msg("")]
    ClawbackNotAllowedOnDeposit, // 319
    #[msg("")]
    DepositStillLocked, // 320
    #[msg("")]
    InvalidAuthority, // 321
    #[msg("")]
    InvalidTokenOwnerRecord,
    #[msg("")]
    InvalidRealmAuthority,
}
