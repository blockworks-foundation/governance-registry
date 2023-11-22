use anchor_spl::token::TokenAccount;
use program_test::*;
use solana_program_test::*;
use solana_sdk::{signature::Keypair, signer::Signer, transport::TransportError};
use std::cell::RefCell;
use std::sync::Arc;
use voter_stake_registry::state::LockupKind;

mod program_test;

#[allow(unaligned_references)]
#[tokio::test]
async fn test_unlock_deposit() -> Result<(), TransportError> {
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

    let reference_account = context.users[1].token_accounts[0];
    let withdraw = |index: u8, amount: u64| {
        addin.withdraw(
            &registrar,
            &voter,
            &mngo_voting_mint,
            &voter_authority,
            reference_account,
            index,
            amount,
        )
    };
    let deposit = |index: u8, amount: u64| {
        addin.deposit(
            &registrar,
            &voter,
            &mngo_voting_mint,
            &voter_authority,
            reference_account,
            index,
            amount,
        )
    };

    let unlock_deposit = |index: u8| {
        addin.unlock_deposit(
            &registrar,
            &voter,
            &voter_authority,
            &realm_authority,
            index,
        )
    };
    let time_offset = Arc::new(RefCell::new(0i64));
    let lockup_status =
        |index: u8| get_lockup_data(&context.solana, voter.address, index, *time_offset.borrow());

    let day = 24 * 60 * 60;

    // Test bad grant_authority
    let deposit_entry_index = 1;
    addin
        .create_deposit_entry(
            &registrar,
            &voter,
            &voter_authority,
            &mngo_voting_mint,
            deposit_entry_index,
            LockupKind::Daily,
            None,
            3,
            false,
        )
        .await
        .unwrap();
    deposit(deposit_entry_index, 80).await.unwrap();
    assert_eq!(
        lockup_status(deposit_entry_index).await,
        LockupData {
            time_passed: 0,
            duration: 3 * day,
            amount_initially_locked_native: 80,
            amount_deposited_native: 80,
            amount_unlocked: 0
        }
    );
    addin
        .unlock_deposit(
            &registrar,
            &voter,
            &voter_authority,
            &voter_authority,
            deposit_entry_index,
        )
        .await
        .expect_err("BadUnlockDepositAuthority");

    // tests for daily vesting
    let deposit_entry_index = 2;
    addin
        .create_deposit_entry(
            &registrar,
            &voter,
            &voter_authority,
            &mngo_voting_mint,
            deposit_entry_index,
            LockupKind::Daily,
            None,
            3,
            false,
        )
        .await
        .unwrap();
    deposit(deposit_entry_index, 80).await.unwrap();
    assert_eq!(
        lockup_status(deposit_entry_index).await,
        LockupData {
            time_passed: 0,
            duration: 3 * day,
            amount_initially_locked_native: 80,
            amount_deposited_native: 80,
            amount_unlocked: 0
        }
    );
    deposit(deposit_entry_index, 10).await.unwrap();
    assert_eq!(
        lockup_status(deposit_entry_index).await,
        LockupData {
            time_passed: 0,
            duration: 3 * day,
            amount_initially_locked_native: 90,
            amount_deposited_native: 90,
            amount_unlocked: 0
        }
    );

    unlock_deposit(deposit_entry_index).await?;
    assert_eq!(
        lockup_status(deposit_entry_index).await,
        LockupData {
            time_passed: 0,
            duration: 0,
            amount_initially_locked_native: 90,
            amount_deposited_native: 90,
            amount_unlocked: 90
        }
    );

    context.solana.advance_clock_by_slots(2).await; // avoid deposit and withdraw in one slot

    withdraw(deposit_entry_index, 90).await.unwrap(); // withdraw all previously locked tokens
    assert_eq!(
        lockup_status(deposit_entry_index).await,
        LockupData {
            time_passed: 0,
            duration: 0,
            amount_initially_locked_native: 90,
            amount_deposited_native: 0,
            amount_unlocked: 0
        }
    );

    Ok(())
}
