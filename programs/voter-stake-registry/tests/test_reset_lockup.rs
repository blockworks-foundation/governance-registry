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
async fn test_reset_lockup() -> Result<(), TransportError> {
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
    let reset_lockup = |index: u8, periods: u32, kind: LockupKind| {
        addin.reset_lockup(&registrar, &voter, &voter_authority, index, kind, periods)
    };
    let time_offset = Arc::new(RefCell::new(0i64));
    let advance_time = |extra: u64| {
        *time_offset.borrow_mut() += extra as i64;
        addin.set_time_offset(&registrar, &realm_authority, *time_offset.borrow())
    };
    let lockup_status =
        |index: u8| get_lockup_data(&context.solana, voter.address, index, *time_offset.borrow());

    let month = LockupKind::Monthly.period_secs();
    let day = 24 * 60 * 60;
    let hour = 60 * 60;

    // tests for daily vesting
    addin
        .create_deposit_entry(
            &registrar,
            &voter,
            &voter_authority,
            &mngo_voting_mint,
            7,
            LockupKind::Daily,
            None,
            3,
            false,
        )
        .await
        .unwrap();
    deposit(7, 80).await.unwrap();
    assert_eq!(
        lockup_status(7).await,
        LockupData {
            time_passed: 0,
            duration: 3 * day,
            amount_initially_locked_native: 80,
            amount_deposited_native: 80,
            amount_unlocked: 0
        }
    );
    deposit(7, 10).await.unwrap();
    assert_eq!(
        lockup_status(7).await,
        LockupData {
            time_passed: 0,
            duration: 3 * day,
            amount_initially_locked_native: 90,
            amount_deposited_native: 90,
            amount_unlocked: 0
        }
    );
    reset_lockup(7, 2, LockupKind::Daily)
        .await
        .expect_err("can't relock for less periods");
    reset_lockup(7, 3, LockupKind::Daily).await.unwrap(); // just resets start to current timestamp
    assert_eq!(
        lockup_status(7).await,
        LockupData {
            time_passed: 0,
            duration: 3 * day,
            amount_initially_locked_native: 90,
            amount_deposited_native: 90,
            amount_unlocked: 0
        }
    );

    // advance more than a day
    advance_time(day + hour).await;
    context.solana.advance_clock_by_slots(2).await;

    assert_eq!(
        lockup_status(7).await,
        LockupData {
            time_passed: day + hour,
            duration: 3 * day,
            amount_initially_locked_native: 90,
            amount_deposited_native: 90,
            amount_unlocked: 30
        }
    );
    deposit(7, 10).await.unwrap();
    assert_eq!(
        lockup_status(7).await,
        LockupData {
            time_passed: hour,
            duration: 2 * day,
            amount_initially_locked_native: 70,
            amount_deposited_native: 100,
            amount_unlocked: 30
        }
    );
    reset_lockup(7, 10, LockupKind::Daily).await.unwrap();
    assert_eq!(
        lockup_status(7).await,
        LockupData {
            time_passed: 0,
            duration: 10 * day,
            amount_initially_locked_native: 100,
            amount_deposited_native: 100,
            amount_unlocked: 0
        }
    );

    // advance four more days
    advance_time(4 * day + hour).await;
    context.solana.advance_clock_by_slots(2).await;

    assert_eq!(
        lockup_status(7).await,
        LockupData {
            time_passed: 4 * day + hour,
            duration: 10 * day,
            amount_initially_locked_native: 100,
            amount_deposited_native: 100,
            amount_unlocked: 40
        }
    );
    withdraw(7, 20).await.unwrap(); // partially withdraw vested
    assert_eq!(
        lockup_status(7).await,
        LockupData {
            time_passed: 4 * day + hour,
            duration: 10 * day,
            amount_initially_locked_native: 100,
            amount_deposited_native: 80,
            amount_unlocked: 20
        }
    );
    reset_lockup(7, 5, LockupKind::Daily)
        .await
        .expect_err("can't relock for less periods");
    reset_lockup(7, 6, LockupKind::Daily).await.unwrap();
    assert_eq!(
        lockup_status(7).await,
        LockupData {
            time_passed: 0,
            duration: 6 * day,
            amount_initially_locked_native: 80,
            amount_deposited_native: 80,
            amount_unlocked: 0
        }
    );
    reset_lockup(7, 8, LockupKind::Daily).await.unwrap();
    assert_eq!(
        lockup_status(7).await,
        LockupData {
            time_passed: 0,
            duration: 8 * day,
            amount_initially_locked_native: 80,
            amount_deposited_native: 80,
            amount_unlocked: 0
        }
    );

    // advance three more days
    advance_time(3 * day + hour).await;
    context.solana.advance_clock_by_slots(2).await;

    assert_eq!(
        lockup_status(7).await,
        LockupData {
            time_passed: 3 * day + hour,
            duration: 8 * day,
            amount_initially_locked_native: 80,
            amount_deposited_native: 80,
            amount_unlocked: 30
        }
    );
    deposit(7, 10).await.unwrap();
    assert_eq!(
        lockup_status(7).await,
        LockupData {
            time_passed: hour,
            duration: 5 * day,
            amount_initially_locked_native: 60,
            amount_deposited_native: 90,
            amount_unlocked: 30
        }
    );

    context.solana.advance_clock_by_slots(2).await; // avoid deposit and withdraw in one slot

    withdraw(7, 20).await.unwrap(); // partially withdraw vested
    assert_eq!(
        lockup_status(7).await,
        LockupData {
            time_passed: hour,
            duration: 5 * day,
            amount_initially_locked_native: 60,
            amount_deposited_native: 70,
            amount_unlocked: 10
        }
    );
    reset_lockup(7, 10, LockupKind::Daily).await.unwrap();
    assert_eq!(
        lockup_status(7).await,
        LockupData {
            time_passed: 0,
            duration: 10 * day,
            amount_initially_locked_native: 70,
            amount_deposited_native: 70,
            amount_unlocked: 0
        }
    );

    reset_lockup(7, 1, LockupKind::Monthly).await.unwrap();
    assert_eq!(
        lockup_status(7).await,
        LockupData {
            time_passed: 0,
            duration: 1 * month,
            amount_initially_locked_native: 70,
            amount_deposited_native: 70,
            amount_unlocked: 0
        }
    );

    reset_lockup(7, 31, LockupKind::Daily)
        .await
        .expect_err("decreasing strictness");
    reset_lockup(7, 31, LockupKind::None)
        .await
        .expect_err("decreasing strictness");
    reset_lockup(7, 30, LockupKind::Cliff)
        .await
        .expect_err("period shortnend");
    reset_lockup(7, 31, LockupKind::Cliff).await.unwrap();
    assert_eq!(
        lockup_status(7).await,
        LockupData {
            time_passed: 0,
            duration: 31 * day,
            amount_initially_locked_native: 70,
            amount_deposited_native: 70,
            amount_unlocked: 0
        }
    );

    // tests for cliff vesting
    addin
        .create_deposit_entry(
            &registrar,
            &voter,
            &voter_authority,
            &mngo_voting_mint,
            5,
            LockupKind::Cliff,
            None,
            3,
            false,
        )
        .await
        .unwrap();
    deposit(5, 80).await.unwrap();
    assert_eq!(
        lockup_status(5).await,
        LockupData {
            time_passed: 0,
            duration: 3 * day,
            amount_initially_locked_native: 80,
            amount_deposited_native: 80,
            amount_unlocked: 0
        }
    );
    reset_lockup(5, 2, LockupKind::Cliff)
        .await
        .expect_err("can't relock for less periods");
    reset_lockup(5, 3, LockupKind::Cliff).await.unwrap(); // just resets start to current timestamp
    assert_eq!(
        lockup_status(5).await,
        LockupData {
            time_passed: 0,
            duration: 3 * day,
            amount_initially_locked_native: 80,
            amount_deposited_native: 80,
            amount_unlocked: 0
        }
    );
    reset_lockup(5, 4, LockupKind::Cliff).await.unwrap();
    assert_eq!(
        lockup_status(5).await,
        LockupData {
            time_passed: 0,
            duration: 4 * day,
            amount_initially_locked_native: 80,
            amount_deposited_native: 80,
            amount_unlocked: 0
        }
    );

    // advance to end of cliff
    advance_time(4 * day + hour).await;
    context.solana.advance_clock_by_slots(2).await;

    assert_eq!(
        lockup_status(5).await,
        LockupData {
            time_passed: 4 * day,
            duration: 4 * day,
            amount_initially_locked_native: 80,
            amount_deposited_native: 80,
            amount_unlocked: 80
        }
    );
    reset_lockup(5, 1, LockupKind::Cliff).await.unwrap();
    assert_eq!(
        lockup_status(5).await,
        LockupData {
            time_passed: 0,
            duration: 1 * day,
            amount_initially_locked_native: 80,
            amount_deposited_native: 80,
            amount_unlocked: 0
        }
    );
    withdraw(5, 10).await.expect_err("nothing unlocked");

    // advance to end of cliff again
    advance_time(day + hour).await;
    context.solana.advance_clock_by_slots(2).await;

    assert_eq!(
        lockup_status(5).await,
        LockupData {
            time_passed: day,
            duration: day,
            amount_initially_locked_native: 80,
            amount_deposited_native: 80,
            amount_unlocked: 80
        }
    );
    withdraw(5, 10).await.unwrap();
    assert_eq!(
        lockup_status(5).await,
        LockupData {
            time_passed: day,
            duration: day,
            amount_initially_locked_native: 80,
            amount_deposited_native: 70,
            amount_unlocked: 70
        }
    );
    deposit(5, 5).await.unwrap();
    assert_eq!(
        lockup_status(5).await,
        LockupData {
            time_passed: 0,
            duration: 0,
            amount_initially_locked_native: 5,
            amount_deposited_native: 75,
            amount_unlocked: 75
        }
    );
    reset_lockup(5, 1, LockupKind::Cliff).await.unwrap();
    assert_eq!(
        lockup_status(5).await,
        LockupData {
            time_passed: 0,
            duration: 1 * day,
            amount_initially_locked_native: 75,
            amount_deposited_native: 75,
            amount_unlocked: 0
        }
    );
    deposit(5, 15).await.unwrap();
    assert_eq!(
        lockup_status(5).await,
        LockupData {
            time_passed: 0,
            duration: 1 * day,
            amount_initially_locked_native: 90,
            amount_deposited_native: 90,
            amount_unlocked: 0
        }
    );

    Ok(())
}
