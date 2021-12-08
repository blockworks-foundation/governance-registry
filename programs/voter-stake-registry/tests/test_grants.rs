use anchor_spl::token::TokenAccount;
use program_test::*;
use solana_program_test::*;
use solana_sdk::{signature::Keypair, signer::Signer, transport::TransportError};
use voter_stake_registry::state::LockupKind;

mod program_test;

#[allow(unaligned_references)]
#[tokio::test]
async fn test_grants() -> Result<(), TransportError> {
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
    let voter2_authority = &context.users[2].key;
    let token_owner_record = realm
        .create_token_owner_record(voter_authority.pubkey(), &payer)
        .await;

    let registrar = addin
        .create_registrar(&realm, &realm_authority, payer)
        .await;

    let grant_authority = &context.users[3].key;
    let grant_funds = context.users[3].token_accounts[0];

    let mngo_voting_mint = addin
        .configure_voting_mint(
            &registrar,
            &realm_authority,
            payer,
            0,
            &context.mints[0],
            0,
            2.0,
            0.0,
            5 * 365 * 24 * 60 * 60,
            Some(grant_authority.pubkey()),
        )
        .await;

    let voter = addin
        .create_voter(&registrar, &token_owner_record, &voter_authority, &payer)
        .await;

    // use up entry 0
    addin
        .create_deposit_entry(
            &registrar,
            &voter,
            voter_authority,
            &mngo_voting_mint,
            0,
            LockupKind::None,
            0,
            false,
        )
        .await
        .unwrap();

    // grant funds to voter (existing)
    let voter_grant = addin
        .grant(
            &registrar,
            voter_authority.pubkey(),
            &mngo_voting_mint,
            LockupKind::Monthly,
            12,
            true,
            12000,
            grant_funds,
            &grant_authority,
        )
        .await
        .unwrap();

    // grant funds to voter2 (new)
    let voter2_grant = addin
        .grant(
            &registrar,
            voter2_authority.pubkey(),
            &mngo_voting_mint,
            LockupKind::Monthly,
            12,
            true,
            24000,
            grant_funds,
            &grant_authority,
        )
        .await
        .unwrap();

    assert_eq!(mngo_voting_mint.vault_balance(&context.solana).await, 36000);
    assert_eq!(voter.deposit_amount(&context.solana, 0).await, 0);
    assert_eq!(voter.deposit_amount(&context.solana, 1).await, 12000);
    assert_eq!(voter.address, voter_grant.address);
    assert_eq!(voter2_grant.deposit_amount(&context.solana, 0).await, 24000);

    let voter_data = context
        .solana
        .get_account::<voter_stake_registry::state::Voter>(voter.address)
        .await;
    let deposit = &voter_data.deposits[1];
    assert_eq!(deposit.is_used, true);
    assert_eq!(deposit.amount_deposited_native, 12000);
    assert_eq!(deposit.amount_initially_locked_native, 12000);
    assert_eq!(deposit.allow_clawback, true);
    assert_eq!(deposit.lockup.kind, LockupKind::Monthly);
    assert_eq!(deposit.lockup.periods_total().unwrap(), 12);

    Ok(())
}
