use {
    crate::helpers::{execute_create_mint, execute_wrap, setup_test_env, WrapResult},
    serial_test::serial,
    solana_program_pack::Pack,
    solana_signer::Signer,
    spl_token::{self},
};

mod helpers;

#[tokio::test]
#[serial]
async fn test_wrap_single_signer() {
    let env = setup_test_env().await;
    let create_mint_result = execute_create_mint(&env).await;
    let WrapResult {
        unwrapped_token_account,
        recipient_token_account,
        escrow_account,
    } = execute_wrap(&env, create_mint_result).await;

    let unwrapped_account_data = env
        .rpc_client
        .get_account_data(&unwrapped_token_account.pubkey())
        .await
        .unwrap();
    let unwrapped_token_state = spl_token::state::Account::unpack(&unwrapped_account_data).unwrap();

    // Unwrapped token account should be empty now
    assert_eq!(unwrapped_token_state.amount, 0);

    // Escrow account should have the tokens
    let escrow_account_data = env
        .rpc_client
        .get_account_data(&escrow_account.pubkey())
        .await
        .unwrap();
    let escrow_token_state = spl_token::state::Account::unpack(&escrow_account_data).unwrap();
    assert_eq!(escrow_token_state.amount, 100);

    // Recipient should have wrapped tokens
    let wrapped_account = env
        .rpc_client
        .get_account(&recipient_token_account.pubkey())
        .await
        .unwrap();
    assert_eq!(wrapped_account.owner, spl_token_2022::id());
    let wrapped_token_state =
        spl_token_2022::state::Account::unpack(&wrapped_account.data).unwrap();
    assert_eq!(wrapped_token_state.amount, 100);
}
