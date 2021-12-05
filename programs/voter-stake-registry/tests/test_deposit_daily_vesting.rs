use anchor_spl::token::TokenAccount;
use program_test::*;
use solana_program_test::*;
use solana_sdk::{pubkey::Pubkey, signature::Keypair, signer::Signer, transport::TransportError};

mod program_test;

#[derive(PartialEq, Debug)]
struct Balances {
    token: u64,
    vault: u64,
    deposit: u64,
    voter_weight: u64,
}

async fn balances(
    context: &TestContext,
    registrar: &RegistrarCookie,
    address: Pubkey,
    voter: &VoterCookie,
    voting_mint: &VotingMintConfigCookie,
    deposit_id: u8,
) -> Balances {
    // Advance slots to avoid caching of the UpdateVoterWeightRecord call
    // TODO: Is this something that could be an issue on a live node?
    context.solana.advance_clock_by_slots(2).await;

    let token = context.solana.token_account_balance(address).await;
    let vault = voting_mint.vault_balance(&context.solana).await;
    let deposit = voter.deposit_amount(&context.solana, deposit_id).await;
    let vwr = context
        .addin
        .update_voter_weight_record(&registrar, &voter)
        .await
        .unwrap();
    Balances {
        token,
        vault,
        deposit,
        voter_weight: vwr.voter_weight,
    }
}

#[allow(unaligned_references)]
#[tokio::test]
async fn test_deposit_daily_vesting() -> Result<(), TransportError> {
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
            1,
            None,
        )
        .await;

    let voter = addin
        .create_voter(&registrar, &token_owner_record, &voter_authority, &payer)
        .await;

    let reference_account = context.users[1].token_accounts[0];
    let get_balances = |depot_id| {
        balances(
            &context,
            &registrar,
            reference_account,
            &voter,
            &mngo_voting_mint,
            depot_id,
        )
    };
    let withdraw = |amount: u64| {
        addin.withdraw(
            &registrar,
            &voter,
            &mngo_voting_mint,
            &voter_authority,
            reference_account,
            0,
            amount,
        )
    };
    let deposit = |amount: u64| {
        addin.deposit(
            &registrar,
            &voter,
            &mngo_voting_mint,
            &voter_authority,
            reference_account,
            0,
            amount,
        )
    };

    // test deposit and withdraw

    let initial = get_balances(0).await;
    assert_eq!(initial.vault, 0);
    assert_eq!(initial.deposit, 0);

    addin
        .create_deposit_entry(
            &registrar,
            &voter,
            &voter_authority,
            &mngo_voting_mint,
            0,
            voter_stake_registry::state::LockupKind::Daily,
            3,
            false,
        )
        .await
        .unwrap();
    deposit(9000).await.unwrap();

    let after_deposit = get_balances(0).await;
    assert_eq!(initial.token, after_deposit.token + after_deposit.vault);
    assert_eq!(after_deposit.voter_weight, after_deposit.vault);
    assert_eq!(after_deposit.vault, 9000);
    assert_eq!(after_deposit.deposit, 9000);

    // cannot withdraw yet, nothing is vested
    withdraw(1).await.expect_err("nothing vested yet");

    // advance a day
    addin
        .set_time_offset(&registrar, &realm_authority, 25 * 60 * 60)
        .await;
    context.solana.advance_clock_by_slots(2).await;

    withdraw(3001).await.expect_err("withdrew too much");
    withdraw(3000).await.unwrap();

    let after_withdraw = get_balances(0).await;
    assert_eq!(initial.token, after_withdraw.token + after_withdraw.vault);
    assert_eq!(after_withdraw.voter_weight, after_withdraw.vault);
    assert_eq!(after_withdraw.vault, 6000);
    assert_eq!(after_withdraw.deposit, 6000);

    // There are two vesting periods left, if we add 5000 to the deposit,
    // half of that should vest each day.
    deposit(5000).await.unwrap();

    let after_deposit = get_balances(0).await;
    assert_eq!(initial.token, after_deposit.token + after_deposit.vault);
    assert_eq!(after_deposit.voter_weight, after_deposit.vault);
    assert_eq!(after_deposit.vault, 11000);
    assert_eq!(after_deposit.deposit, 11000);

    withdraw(1).await.expect_err("nothing vested yet");

    // advance another day
    addin
        .set_time_offset(&registrar, &realm_authority, 49 * 60 * 60)
        .await;
    context.solana.advance_clock_by_slots(2).await;

    // There is just one period left, should be fully withdrawable after
    deposit(1000).await.unwrap();

    context.solana.advance_clock_by_slots(2).await;

    // can withdraw 3000 (original deposit) plus 2500 (second deposit)
    // nothing from the third deposit is vested
    withdraw(5501).await.expect_err("withdrew too much");
    withdraw(5500).await.unwrap();

    let after_withdraw = get_balances(0).await;
    assert_eq!(initial.token, after_withdraw.token + after_withdraw.vault);
    assert_eq!(after_withdraw.voter_weight, after_withdraw.vault);
    assert_eq!(after_withdraw.vault, 6500);
    assert_eq!(after_withdraw.deposit, 6500);

    // check that creating the voter again has no effect on balances
    addin
        .create_voter(&registrar, &token_owner_record, &voter_authority, &payer)
        .await;

    let after_recreate = get_balances(0).await;
    assert_eq!(after_withdraw, after_recreate);

    // advance another day
    addin
        .set_time_offset(&registrar, &realm_authority, 73 * 60 * 60)
        .await;
    context.solana.advance_clock_by_slots(2).await;

    // can withdraw the rest
    withdraw(6500).await.unwrap();

    let after_withdraw = get_balances(0).await;
    assert_eq!(initial.token, after_withdraw.token + after_withdraw.vault);
    assert_eq!(after_withdraw.voter_weight, after_withdraw.vault);
    assert_eq!(after_withdraw.vault, 0);
    assert_eq!(after_withdraw.deposit, 0);

    // if we deposit now, we can immediately withdraw
    deposit(1000).await.unwrap();

    let after_deposit = get_balances(0).await;
    assert_eq!(initial.token, after_deposit.token + after_deposit.vault);
    assert_eq!(after_deposit.voter_weight, after_deposit.vault);
    assert_eq!(after_deposit.vault, 1000);
    assert_eq!(after_deposit.deposit, 1000);

    withdraw(1000).await.unwrap();

    let after_withdraw = get_balances(0).await;
    assert_eq!(initial.token, after_withdraw.token + after_withdraw.vault);
    assert_eq!(after_withdraw.voter_weight, after_withdraw.vault);
    assert_eq!(after_withdraw.vault, 0);
    assert_eq!(after_withdraw.deposit, 0);

    addin
        .close_deposit_entry(&voter, &voter_authority, 0)
        .await
        .unwrap();

    Ok(())
}
