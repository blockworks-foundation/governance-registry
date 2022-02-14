use anchor_lang::prelude::*;

#[event]
#[derive(Debug)]
pub struct VoterInfo {
    /// Voter's total voting power
    pub voting_power: u64,
    /// Voter's total voting power, when ignoring any effects from lockup
    pub voting_power_baseline: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Debug)]
pub struct VestingInfo {
    /// Amount of tokens vested each period
    pub rate: u64,
    /// Time of the next upcoming vesting
    pub next_timestamp: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Debug)]
pub struct LockingInfo {
    /// Amount of locked tokens
    pub amount: u64,
    /// Time at which the lockup fully ends (None for Constant lockup)
    pub end_timestamp: Option<u64>,
    /// Information about vesting, if any
    pub vesting: Option<VestingInfo>,
}

#[event]
#[derive(Debug)]
pub struct DepositEntryInfo {
    pub deposit_entry_index: u8,
    pub voting_mint_config_index: u8,
    /// Amount that is unlocked
    pub unlocked: u64,
    /// Voting power implied by this deposit entry
    pub voting_power: u64,
    /// Voting power without any adjustments for lockup
    pub voting_power_baseline: u64,
    /// Information about locking, if any
    pub locking: Option<LockingInfo>,
}
