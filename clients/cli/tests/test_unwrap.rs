use {
    crate::helpers::{
        create_token_account, create_unwrapped_mint, execute_create_mint, mint_to, setup_test_env,
        TestEnv, TOKEN_WRAP_CLI_BIN,
    },
    serial_test::serial,
    solana_keypair::{write_keypair_file, Keypair},
    solana_pubkey::Pubkey,
    solana_signer::Signer,
    spl_token::{self},
    spl_token_2022::{extension::PodStateWithExtensions, pod::PodAccount},
    spl_token_wrap::{get_wrapped_mint_address, get_wrapped_mint_authority},
    std::process::Command,
    tempfile::NamedTempFile,
};

mod helpers;

pub struct UnwrapSetup {
    pub wrapped_token_account: Pubkey,
    pub escrow_account: Pubkey,
    pub unwrapped_token_recipient: Pubkey,
    pub unwrapped_mint: Pubkey,
    pub unwrapped_token_program: Pubkey,
    pub wrapped_token_program: Pubkey,
}

/// Sets up everything needed before calling `unwrap`:
/// 1) Creates the unwrapped mint
/// 2) Wraps `wrap_amount` of the unwrapped token
/// 3) Returns accounts and program IDs needed by the tests.
///
/// If `maybe_wrapped_owner` is provided, that pubkey will own the wrapped-token
/// account; otherwise it defaults to the `env.payer.pubkey()`.
async fn setup_for_unwrap(
    env: &TestEnv,
    initial_unwrapped_balance: u64,
    wrap_amount: u64,
    maybe_wrapped_owner: Option<Pubkey>,
) -> UnwrapSetup {
    // --- Create Mints ---
    let unwrapped_token_program = spl_token::id();
    let wrapped_token_program = spl_token_2022::id();
    let unwrapped_mint = create_unwrapped_mint(env, &unwrapped_token_program).await;
    execute_create_mint(
        env,
        &unwrapped_mint,
        &unwrapped_token_program,
        &wrapped_token_program,
    )
    .await;

    // --- Setup Accounts for Initial Wrap ---
    // 1) Source account for unwrapped tokens (owned by payer)
    let source_unwrapped_account = create_token_account(
        env,
        &unwrapped_token_program,
        &unwrapped_mint,
        &env.payer.pubkey(),
    )
    .await;
    mint_to(
        env,
        &unwrapped_token_program,
        &unwrapped_mint,
        &source_unwrapped_account,
        initial_unwrapped_balance,
    )
    .await;

    // 2) Escrow account (owned by PDA)
    let wrapped_mint = get_wrapped_mint_address(&unwrapped_mint, &wrapped_token_program);
    let wrapped_mint_authority = get_wrapped_mint_authority(&wrapped_mint);
    let escrow_account = create_token_account(
        env,
        &unwrapped_token_program,
        &unwrapped_mint,
        &wrapped_mint_authority,
    )
    .await;

    // 3) Target account for wrapped tokens
    let wrapped_owner = maybe_wrapped_owner.unwrap_or(env.payer.pubkey());
    let wrapped_token_account =
        create_token_account(env, &wrapped_token_program, &wrapped_mint, &wrapped_owner).await;

    // Perform the initial "wrap"
    Command::new(TOKEN_WRAP_CLI_BIN)
        .args(vec![
            "wrap".to_string(),
            "-C".to_string(),
            env.config_file_path.clone(),
            source_unwrapped_account.to_string(),
            escrow_account.to_string(),
            wrapped_token_program.to_string(),
            wrap_amount.to_string(),
            "--recipient-token-account".to_string(),
            wrapped_token_account.to_string(),
        ])
        .status()
        .unwrap();

    // 4) Setup the final recipient for the future "unwrap"
    let unwrapped_token_recipient = create_token_account(
        env,
        &unwrapped_token_program,
        &unwrapped_mint,
        &env.payer.pubkey(),
    )
    .await;

    UnwrapSetup {
        wrapped_token_account,
        escrow_account,
        unwrapped_token_recipient,
        unwrapped_mint,
        unwrapped_token_program,
        wrapped_token_program,
    }
}

#[allow(clippy::too_many_arguments)]
async fn assert_unwrap_result(
    env: &TestEnv,
    wrapped_token_account_addr: &Pubkey,
    wrapped_start_bal: u64, // balance after initial wrap, before unwrap
    unwrapped_token_addr_recipient: &Pubkey,
    recipient_start_bal: u64, // balance before unwrap
    escrow_account_addr: &Pubkey,
    escrow_start_bal: u64, // balance after initial wrap, before unwrap
    unwrap_amount: u64,
) {
    // 1) Wrapped token account should have been debited (tokens burned)
    let wrapped_account_data = env
        .rpc_client
        .get_account_data(wrapped_token_account_addr)
        .await
        .unwrap();
    let wrapped_token_state =
        PodStateWithExtensions::<PodAccount>::unpack(&wrapped_account_data).unwrap();
    assert_eq!(
        u64::from(wrapped_token_state.base.amount),
        wrapped_start_bal.checked_sub(unwrap_amount).unwrap(),
    );

    // 2) Unwrapped token recipient should have been credited
    let recipient_account_data = env
        .rpc_client
        .get_account_data(unwrapped_token_addr_recipient)
        .await
        .unwrap();
    let recipient_token_state =
        PodStateWithExtensions::<PodAccount>::unpack(&recipient_account_data).unwrap();
    assert_eq!(
        u64::from(recipient_token_state.base.amount),
        recipient_start_bal.checked_add(unwrap_amount).unwrap(),
    );

    // 3) Escrow account should have transferred out tokens
    let escrow_account_data = env
        .rpc_client
        .get_account_data(escrow_account_addr)
        .await
        .unwrap();
    let escrow_token_state =
        PodStateWithExtensions::<PodAccount>::unpack(&escrow_account_data).unwrap();
    assert_eq!(
        u64::from(escrow_token_state.base.amount),
        escrow_start_bal.checked_sub(unwrap_amount).unwrap(),
    );
}

#[tokio::test]
#[serial]
async fn test_unwrap_single_signer_with_defaults() {
    let env = setup_test_env().await;

    let initial_unwrapped_balance = 200;
    let setup_wrap_amount = 100;
    let unwrap_amount = 50;
    let setup = setup_for_unwrap(&env, initial_unwrapped_balance, setup_wrap_amount, None).await;

    Command::new(TOKEN_WRAP_CLI_BIN)
        .args(vec![
            "unwrap".to_string(),
            "-C".to_string(),
            env.config_file_path.clone(),
            setup.wrapped_token_account.to_string(),
            setup.escrow_account.to_string(),
            setup.unwrapped_token_recipient.to_string(),
            unwrap_amount.to_string(),
        ])
        .status()
        .unwrap();

    assert_unwrap_result(
        &env,
        &setup.wrapped_token_account,
        setup_wrap_amount,
        &setup.unwrapped_token_recipient,
        0,
        &setup.escrow_account,
        setup_wrap_amount,
        unwrap_amount,
    )
    .await;
}

#[tokio::test]
#[serial]
async fn test_unwrap_single_signer_with_optional_flags() {
    let env = setup_test_env().await;

    // Create a separate Keypair to be the transfer authority
    let transfer_authority = Keypair::new();
    let authority_keypair_file = NamedTempFile::new().unwrap();
    write_keypair_file(&transfer_authority, &authority_keypair_file).unwrap();

    let initial_unwrapped_balance = 300;
    let setup_wrap_amount = 150;
    let unwrap_amount = 75;

    let setup = setup_for_unwrap(
        &env,
        initial_unwrapped_balance,
        setup_wrap_amount,
        Some(transfer_authority.pubkey()),
    )
    .await;

    let blockhash = env.rpc_client.get_latest_blockhash().await.unwrap();

    // Adding all optional flags to pass
    Command::new(TOKEN_WRAP_CLI_BIN)
        .args(vec![
            "unwrap".to_string(),
            "-C".to_string(),
            env.config_file_path.clone(),
            setup.wrapped_token_account.to_string(),
            setup.escrow_account.to_string(),
            setup.unwrapped_token_recipient.to_string(),
            unwrap_amount.to_string(),
            "--unwrapped-mint".to_string(),
            setup.unwrapped_mint.to_string(),
            "--wrapped-token-program".to_string(),
            setup.wrapped_token_program.to_string(),
            "--unwrapped-token-program".to_string(),
            setup.unwrapped_token_program.to_string(),
            "--transfer-authority".to_string(),
            authority_keypair_file.path().to_str().unwrap().to_string(),
            "--blockhash".to_string(),
            blockhash.to_string(),
        ])
        .status()
        .unwrap();

    // Confirm the final balances after unwrap
    assert_unwrap_result(
        &env,
        &setup.wrapped_token_account,
        setup_wrap_amount,
        &setup.unwrapped_token_recipient,
        0,
        &setup.escrow_account,
        setup_wrap_amount,
        unwrap_amount,
    )
    .await;
}
