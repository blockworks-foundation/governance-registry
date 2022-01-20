use anchor_lang::prelude::*;

#[event]
#[derive(Debug)]
pub struct VoterInfo {
    pub voting_power: u64,
    pub voting_power_deposit_only: u64,
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
    /// Amount that can be withdrawn directly
    pub withdrawable: u64,
    /// Voting power implied by this deposit entry
    pub voting_power: u64,
    /// Voting power that is not based on lockup
    pub voting_power_deposit_only: u64,
    /// Information about locking, if any
    pub locking: Option<LockingInfo>,
}
