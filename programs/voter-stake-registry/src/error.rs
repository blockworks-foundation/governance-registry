use anchor_lang::prelude::*;

#[error]
pub enum ErrorCode {
    #[msg("Exchange rate must be greater than zero")]
    InvalidRate, // 300
    #[msg("")]
    RatesFull,
    #[msg("")]
    VotingMintNotFound, // 302
    #[msg("")]
    DepositEntryNotFound,
    #[msg("")]
    DepositEntryFull, // 304
    #[msg("")]
    VotingTokenNonZero,
    #[msg("")]
    OutOfBoundsDepositEntryIndex, // 306
    #[msg("")]
    UnusedDepositEntryIndex, // 307
    #[msg("")]
    InsufficientVestedTokens, // 308
    #[msg("")]
    UnableToConvert,
    #[msg("")]
    InvalidLockupPeriod,
    #[msg("")]
    InvalidEndTs,
    #[msg("")]
    InvalidDays,
    #[msg("")]
    VotingMintConfigIndexAlreadyInUse,
    #[msg("")]
    OutOfBoundsVotingMintConfigIndex,
    #[msg("Exchange rate decimals cannot be larger than registrar decimals")]
    InvalidDecimals,
    #[msg("")]
    InvalidToDepositAndWithdrawInOneSlot,
    #[msg("")]
    ShouldBeTheFirstIxInATx,
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
    #[msg("")]
    VoterWeightOverflow,
    #[msg("")]
    LockupSaturationMustBePositive,
    #[msg("")]
    VotingMintConfiguredWithDifferentIndex,
    #[msg("")]
    InternalProgramError,
    #[msg("")]
    InsufficientLockedTokens,
    #[msg("")]
    MustKeepTokensLocked,
    #[msg("")]
    InvalidLockupKind,
    #[msg("")]
    InvalidChangeToClawbackDepositEntry,
    #[msg("")]
    InternalErrorBadLockupVoteWeight,
    #[msg("")]
    DepositStartTooFarInFuture,
}
