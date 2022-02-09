use anchor_spl::token::TokenAccount;
use solana_program_test::*;
use solana_sdk::{signer::Signer, transport::TransportError};

use program_test::*;

mod program_test;

#[allow(unaligned_references)]
#[tokio::test]
async fn test_clawback() -> Result<(), TransportError> {
    let context = TestContext::new().await;

    let community_token_mint = &context.mints[0];

    let realm_authority = &context.users[0].key;
    let realm_authority_ata = context.users[0].token_accounts[0];

    let voter_authority = &context.users[1].key;
    let voter_authority_ata = context.users[1].token_accounts[0];

    println!("create_realm");
    let realm = context
        .governance
        .create_realm(
            "testrealm",
            realm_authority.pubkey(),
            community_token_mint,
            &realm_authority,
            &context.addin.program_id,
        )
        .await;

    let token_owner_record = realm
        .create_token_owner_record(voter_authority.pubkey(), &realm_authority)
        .await;

    let registrar = context
        .addin
        .create_registrar(&realm, realm_authority, realm_authority)
        .await;

    println!("configure_voting_mint");
    let mngo_voting_mint = context
        .addin
        .configure_voting_mint(
            &registrar,
            &realm_authority,
            realm_authority,
            0,
            community_token_mint,
            0,
            1.0,
            0.0,
            5 * 365 * 24 * 60 * 60,
            None,
            None,
        )
        .await;

    println!("create_voter");
    let voter = context
        .addin
        .create_voter(
            &registrar,
            &token_owner_record,
            &voter_authority,
            &realm_authority,
        )
        .await;

    let realm_ata_initial = context
        .solana
        .token_account_balance(realm_authority_ata)
        .await;
    let voter_ata_initial = context
        .solana
        .token_account_balance(voter_authority_ata)
        .await;
    let voter_balance_initial = voter.deposit_amount(&context.solana, 0).await;
    assert_eq!(voter_balance_initial, 0);

    println!("create_deposit");
    context
        .addin
        .create_deposit_entry(
            &registrar,
            &voter,
            voter_authority,
            &mngo_voting_mint,
            0,
            voter_stake_registry::state::LockupKind::Daily,
            None,
            10,
            true,
        )
        .await?;
    context
        .addin
        .deposit(
            &registrar,
            &voter,
            &mngo_voting_mint,
            &realm_authority,
            realm_authority_ata,
            0,
            10000,
        )
        .await?;

    let realm_ata_after_deposit = context
        .solana
        .token_account_balance(realm_authority_ata)
        .await;
    assert_eq!(realm_ata_initial, realm_ata_after_deposit + 10000);
    let vault_after_deposit = mngo_voting_mint
        .vault_balance(&context.solana, &voter)
        .await;
    assert_eq!(vault_after_deposit, 10000);
    let voter_balance_after_deposit = voter.deposit_amount(&context.solana, 0).await;
    assert_eq!(voter_balance_after_deposit, 10000);

    println!("withdraw");
    context
        .addin
        .withdraw(
            &registrar,
            &voter,
            &mngo_voting_mint,
            &voter_authority,
            voter_authority_ata,
            0,
            10000,
        )
        .await
        .expect_err("fails because nothing is vested");

    // Advance almost three days for some vesting to kick in
    context
        .addin
        .set_time_offset(&registrar, &realm_authority, (3 * 24 - 1) * 60 * 60)
        .await;
    context.solana.advance_clock_by_slots(2).await;

    println!("withdraw");
    context
        .addin
        .withdraw(
            &registrar,
            &voter,
            &mngo_voting_mint,
            &voter_authority,
            voter_authority_ata,
            0,
            999,
        )
        .await?;

    println!("clawback");
    context
        .addin
        .clawback(
            &registrar,
            &voter,
            &mngo_voting_mint,
            &voter_authority,
            realm_authority_ata,
            0,
        )
        .await
        .expect_err("fails because realm_authority is invalid");

    println!("clawback");
    context
        .addin
        .clawback(
            &registrar,
            &voter,
            &mngo_voting_mint,
            &realm_authority,
            realm_authority_ata,
            0,
        )
        .await?;

    println!("withdraw");
    context
        .addin
        .withdraw(
            &registrar,
            &voter,
            &mngo_voting_mint,
            &voter_authority,
            voter_authority_ata,
            0,
            1001,
        )
        .await?;

    let realm_after_clawback = context
        .solana
        .token_account_balance(realm_authority_ata)
        .await;
    assert_eq!(realm_ata_initial - 2000, realm_after_clawback);
    let voter_after_withdraw = context
        .solana
        .token_account_balance(voter_authority_ata)
        .await;
    assert_eq!(voter_after_withdraw, voter_ata_initial + 2000);
    let vault_after_withdraw = mngo_voting_mint
        .vault_balance(&context.solana, &voter)
        .await;
    assert_eq!(vault_after_withdraw, 0);
    let voter_balance_after_withdraw = voter.deposit_amount(&context.solana, 0).await;
    assert_eq!(voter_balance_after_withdraw, 0);

    Ok(())
}
