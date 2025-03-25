use {
    crate::helpers::{
        create_associated_token_account, create_token_account, create_unwrapped_mint,
        execute_create_mint, execute_wrap, mint_to, setup_test_env, TestEnv,
    },
    serial_test::serial,
    solana_pubkey::Pubkey,
    solana_signer::Signer,
    spl_token::{self},
    spl_token_2022::{extension::PodStateWithExtensions, pod::PodAccount},
    spl_token_wrap::{get_wrapped_mint_address, get_wrapped_mint_authority},
};

mod helpers;

#[tokio::test]
#[serial]
async fn test_wrap_single_signer_with_no_extra_flags() {
    let env = setup_test_env().await;

    // Create Mint
    let unwrapped_token_program = spl_token::id();
    let wrapped_token_program = spl_token_2022::id();
    let unwrapped_mint = create_unwrapped_mint(&env, &unwrapped_token_program).await;
    execute_create_mint(
        &env,
        &unwrapped_mint,
        &unwrapped_token_program,
        &wrapped_token_program,
    )
    .await;

    // Fund initial unwrapped token account
    let unwrapped_token_account = create_token_account(
        &env,
        &unwrapped_token_program,
        &unwrapped_mint,
        &env.payer.pubkey(),
    )
    .await;
    let starting_amount = 100;
    mint_to(
        &env,
        &unwrapped_token_program,
        &unwrapped_mint,
        &unwrapped_token_account,
        starting_amount,
    )
    .await;

    // Setup recipient account with zero balance
    let wrapped_mint = get_wrapped_mint_address(&unwrapped_mint, &wrapped_token_program);
    let recipient_account =
        create_associated_token_account(&env, &wrapped_token_program, &wrapped_mint).await;

    // Setup escrow with mint_authority as owner
    let wrapped_mint_authority = get_wrapped_mint_authority(&wrapped_mint);
    let escrow_account = create_token_account(
        &env,
        &unwrapped_token_program,
        &unwrapped_mint,
        &wrapped_mint_authority,
    )
    .await;

    // Execute wrap instruction
    let unwrap_amount = 50;
    execute_wrap(
        &env,
        &unwrapped_token_program,
        &unwrapped_token_account,
        &escrow_account,
        &wrapped_token_program,
        unwrap_amount,
        None,
        None,
    )
    .await;

    assert_result(
        env,
        &unwrapped_token_account,
        starting_amount,
        &recipient_account,
        &escrow_account,
        unwrap_amount,
    )
    .await;
}

#[tokio::test]
#[serial]
async fn test_wrap_single_signer_with_recipient_and_mint_flags() {
    let env = setup_test_env().await;

    // Create Mint
    let unwrapped_token_program = spl_token::id();
    let wrapped_token_program = spl_token_2022::id();
    let unwrapped_mint = create_unwrapped_mint(&env, &unwrapped_token_program).await;
    execute_create_mint(
        &env,
        &unwrapped_mint,
        &unwrapped_token_program,
        &wrapped_token_program,
    )
    .await;

    // Fund initial unwrapped token account
    let unwrapped_token_account = create_token_account(
        &env,
        &unwrapped_token_program,
        &unwrapped_mint,
        &env.payer.pubkey(),
    )
    .await;
    let starting_amount = 100;
    mint_to(
        &env,
        &unwrapped_token_program,
        &unwrapped_mint,
        &unwrapped_token_account,
        starting_amount,
    )
    .await;

    // Setup recipient account with zero balance
    // This time it is not an ATA, but a fresh token account
    let wrapped_mint = get_wrapped_mint_address(&unwrapped_mint, &wrapped_token_program);
    let recipient_account = create_token_account(
        &env,
        &wrapped_token_program,
        &wrapped_mint,
        &env.payer.pubkey(),
    )
    .await;

    // Setup escrow with mint_authority as owner
    let wrapped_mint_authority = get_wrapped_mint_authority(&wrapped_mint);
    let escrow_account = create_token_account(
        &env,
        &unwrapped_token_program,
        &unwrapped_mint,
        &wrapped_mint_authority,
    )
    .await;

    // Execute wrap instruction
    let unwrap_amount = 50;
    execute_wrap(
        &env,
        &unwrapped_token_program,
        &unwrapped_token_account,
        &escrow_account,
        &wrapped_token_program,
        unwrap_amount,
        Some(&unwrapped_mint),
        Some(&recipient_account),
    )
    .await;

    assert_result(
        env,
        &unwrapped_token_account,
        starting_amount,
        &recipient_account,
        &escrow_account,
        unwrap_amount,
    )
    .await;
}

async fn assert_result(
    env: TestEnv,
    unwrapped_token_account: &Pubkey,
    starting_amount: u64,
    recipient_account: &Pubkey,
    escrow_account: &Pubkey,
    unwrap_amount: u64,
) {
    let unwrapped_account_data = env
        .rpc_client
        .get_account_data(unwrapped_token_account)
        .await
        .unwrap();
    let unwrapped_token_state =
        PodStateWithExtensions::<PodAccount>::unpack(&unwrapped_account_data).unwrap();

    // Unwrapped token account should be lower
    assert_eq!(
        u64::from(unwrapped_token_state.base.amount),
        starting_amount.checked_sub(unwrap_amount).unwrap()
    );

    // Escrow account should have the tokens
    let escrow_account_data = env
        .rpc_client
        .get_account_data(escrow_account)
        .await
        .unwrap();
    let escrow_token_state =
        PodStateWithExtensions::<PodAccount>::unpack(&escrow_account_data).unwrap();
    assert_eq!(u64::from(escrow_token_state.base.amount), unwrap_amount);

    // Recipient should have wrapped tokens
    let wrapped_account = env.rpc_client.get_account(recipient_account).await.unwrap();
    assert_eq!(wrapped_account.owner, spl_token_2022::id());
    let wrapped_token_state =
        PodStateWithExtensions::<PodAccount>::unpack(&wrapped_account.data).unwrap();
    assert_eq!(u64::from(wrapped_token_state.base.amount), unwrap_amount);
}
