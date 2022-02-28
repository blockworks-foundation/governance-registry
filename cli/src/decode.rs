use anchor_lang::Discriminator;
use anyhow::{anyhow, bail, Result};
use serde::Serialize;
use std::collections::HashMap;
use std::io;
use std::io::BufRead;
use voter_stake_registry::state::*;

//
// Output format declarations. These are built from the decoded
// accounts and then converted to JSON.
//

#[derive(Serialize)]
struct DisplayDepositEntry {
    allow_clawback: bool,
    mint_index: u8,
    unlocked_now: u64,
    locked_now: u64,
    locked_1y: u64,
    locked_2y: u64,
    locked_3y: u64,
    locked_4y: u64,
    locked_5y: u64,
}

#[derive(Serialize)]
struct DisplayVoter {
    voter_authority: String,
    registrar: String,
    deposit_entries: Vec<DisplayDepositEntry>,
}

/// Decode a Voter account and print its JSON to stdout
fn decode_voter(data: &[u8]) -> Result<()> {
    let mut data = data;
    let voter: Voter = anchor_lang::AccountDeserialize::try_deserialize(&mut data)?;
    let now_ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs() as i64;
    let year = 365 * 24 * 60 * 60;
    let ser = DisplayVoter {
        voter_authority: voter.voter_authority.to_string(),
        registrar: voter.registrar.to_string(),
        deposit_entries: voter
            .deposits
            .iter()
            .filter(|d| d.is_used)
            .map(|d| DisplayDepositEntry {
                allow_clawback: d.allow_clawback,
                mint_index: d.voting_mint_config_idx,
                unlocked_now: d.amount_unlocked(now_ts),
                locked_now: d.amount_locked(now_ts),
                locked_1y: d.amount_locked(now_ts + year),
                locked_2y: d.amount_locked(now_ts + 2 * year),
                locked_3y: d.amount_locked(now_ts + 3 * year),
                locked_4y: d.amount_locked(now_ts + 4 * year),
                locked_5y: d.amount_locked(now_ts + 5 * year),
            })
            .collect(),
    };
    println!("{}", serde_json::to_string(&ser)?);
    Ok(())
}

// Read a sequence of base64 encoded accounts from stdin
// and write their decoded versions back out as JSON.
pub fn decode_account() -> Result<()> {
    let account_types = HashMap::from([(Voter::discriminator(), &decode_voter)]);

    for line in io::stdin().lock().lines() {
        let data = base64::decode(line?)?;

        if data.len() < 8 {
            bail!("data length {} too small for discriminator", data.len());
        }
        let discr = &data[0..8];
        let handler = account_types
            .get(discr)
            .ok_or_else(|| anyhow!("discriminator {:?} not recognized", discr))?;

        handler(&data)?;
    }
    Ok(())
}
