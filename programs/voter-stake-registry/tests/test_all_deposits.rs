#![cfg(feature = "test-sbf")]

use anchor_spl::token::TokenAccount;
use program_test::*;
use solana_program_test::*;
use solana_sdk::{signature::Keypair, signer::Signer, transport::TransportError};
use voter_stake_registry::state::LockupKind;

mod program_test;

#[allow(unaligned_references)]
#[tokio::test]
async fn test_all_deposits() -> Result<(), TransportError> {
    let context = TestContext::new().await;
    let addin = &context.addin;

    let payer = &context.users[0].key;
    let realm_authority = Keypair::new();
    let realm = context
        .governance
        .create_realm(
            "testrealm",
            realm_authority.pubkey(),
            &context.mints[0],
            &payer,
            &context.addin.program_id,
        )
        .await;

    let voter_authority = &context.users[1].key;
    let voter_mngo = context.users[1].token_accounts[0];
    let token_owner_record = realm
        .create_token_owner_record(voter_authority.pubkey(), &payer)
        .await;

    let registrar = addin
        .create_registrar(&realm, &realm_authority, payer)
        .await;
    let mngo_voting_mint = addin
        .configure_voting_mint(
            &registrar,
            &realm_authority,
            payer,
            0,
            &context.mints[0],
            0,
            1.0,
            0.0,
            5 * 365 * 24 * 60 * 60,
            None,
            None,
        )
        .await;

    let voter = addin
        .create_voter(&registrar, &token_owner_record, &voter_authority, &payer)
        .await;

    for i in 0..32 {
        addin
            .create_deposit_entry(
                &registrar,
                &voter,
                voter_authority,
                &mngo_voting_mint,
                i,
                LockupKind::Monthly,
                None,
                12,
                false,
            )
            .await
            .unwrap();
        addin
            .deposit(
                &registrar,
                &voter,
                &mngo_voting_mint,
                voter_authority,
                voter_mngo,
                i,
                12000,
            )
            .await
            .unwrap();
    }

    // advance time, to be in the middle of all deposit lockups
    addin
        .set_time_offset(&registrar, &realm_authority, 32 * 24 * 60 * 60)
        .await;
    context.solana.advance_clock_by_slots(2).await;

    // the two most expensive calls which scale with number of deposts
    // are update_voter_weight_record and withdraw - both compute the vote weight

    let vwr = addin
        .update_voter_weight_record(&registrar, &voter)
        .await
        .unwrap();
    assert_eq!(vwr.voter_weight, 12000 * 32);

    // make sure withdrawing works with all deposits filled
    addin
        .withdraw(
            &registrar,
            &voter,
            &mngo_voting_mint,
            voter_authority,
            voter_mngo,
            0,
            1000,
        )
        .await
        .unwrap();

    // logging can take a lot of cu/mem
    addin.log_voter_info(&registrar, &voter, 0).await;

    Ok(())
}
