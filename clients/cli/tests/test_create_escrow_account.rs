use {
    crate::helpers::{create_unwrapped_mint, setup_test_env, TestEnv, TOKEN_WRAP_CLI_BIN},
    serde_json::Value,
    serial_test::serial,
    solana_program_pack::IsInitialized,
    solana_pubkey::Pubkey,
    spl_token::{self},
    spl_token_2022::{
        extension::PodStateWithExtensions,
        pod::PodAccount,
        {self},
    },
    spl_token_wrap::{get_wrapped_mint_address, get_wrapped_mint_authority},
    std::{process::Command, str::FromStr},
};

mod helpers;

async fn assert_escrow_creation(
    env: &TestEnv,
    cli_output: &Value,
    expected_owner_pda: &Pubkey,
    unwrapped_mint: &Pubkey,
    expected_token_program_id: &Pubkey,
) {
    let escrow_account_address_str = cli_output["escrowAccountAddress"].as_str().unwrap();
    let escrow_account_address = Pubkey::from_str(escrow_account_address_str).unwrap();

    let cli_reported_owner_pda_str = cli_output["escrowAccountOwner"].as_str().unwrap();
    let cli_reported_owner_pda = Pubkey::from_str(cli_reported_owner_pda_str).unwrap();

    let cli_reported_token_program_str = cli_output["escrowTokenProgramId"].as_str().unwrap();
    let cli_reported_token_program_id = Pubkey::from_str(cli_reported_token_program_str).unwrap();

    let escrow_account = env
        .rpc_client
        .get_account(&escrow_account_address)
        .await
        .unwrap();

    // --- Assertions ---

    // 1. Verify the owner program ID reported by CLI and on-chain matches expected
    assert_eq!(cli_reported_token_program_id, *expected_token_program_id);
    assert_eq!(escrow_account.owner, *expected_token_program_id);

    // 2. Verify the PDA owner reported by CLI and expected matches
    assert_eq!(cli_reported_owner_pda, *expected_owner_pda);

    // 3. Verify the on-chain account state
    let account_state = PodStateWithExtensions::<PodAccount>::unpack(&escrow_account.data).unwrap();
    assert!(account_state.base.is_initialized());
    assert_eq!(account_state.base.mint, *unwrapped_mint);
    assert_eq!(account_state.base.owner, *expected_owner_pda);
    assert_eq!(account_state.base.amount, 0.into());
}

#[tokio::test]
#[serial]
async fn test_create_escrow_account_for_spl_token_mint() {
    let env = setup_test_env().await;
    let unwrapped_token_program_id = spl_token::id();
    let wrapped_token_program_id = spl_token_2022::id();

    let unwrapped_mint = create_unwrapped_mint(&env, &unwrapped_token_program_id).await;

    let wrapped_mint_address = get_wrapped_mint_address(&unwrapped_mint, &wrapped_token_program_id);
    let expected_owner_pda = get_wrapped_mint_authority(&wrapped_mint_address);

    let mut command = Command::new(TOKEN_WRAP_CLI_BIN);
    let output = command
        .args([
            "create-escrow-account",
            "-C",
            &env.config_file_path,
            &unwrapped_mint.to_string(),
            &wrapped_token_program_id.to_string(),
            "--output",
            "json",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());

    let output_str = String::from_utf8(output.stdout).unwrap();
    let json_result: Value = serde_json::from_str(&output_str).unwrap();

    assert_escrow_creation(
        &env,
        &json_result,
        &expected_owner_pda,
        &unwrapped_mint,
        &unwrapped_token_program_id,
    )
    .await;
}

#[tokio::test]
#[serial]
async fn test_create_escrow_account_for_token2022_mint() {
    let env = setup_test_env().await;
    let unwrapped_token_program_id = spl_token_2022::id();
    let wrapped_token_program_id = spl_token::id();

    let unwrapped_mint = create_unwrapped_mint(&env, &unwrapped_token_program_id).await;

    let wrapped_mint_address = get_wrapped_mint_address(&unwrapped_mint, &wrapped_token_program_id);
    let expected_owner_pda = get_wrapped_mint_authority(&wrapped_mint_address);

    let mut command = Command::new(TOKEN_WRAP_CLI_BIN);
    let output = command
        .args([
            "create-escrow-account",
            "-C",
            &env.config_file_path,
            &unwrapped_mint.to_string(),
            &wrapped_token_program_id.to_string(),
            "--output",
            "json",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());

    let output_str = String::from_utf8(output.stdout).unwrap();
    let json_result: Value = serde_json::from_str(&output_str).unwrap();

    assert_escrow_creation(
        &env,
        &json_result,
        &expected_owner_pda,
        &unwrapped_mint,
        &unwrapped_token_program_id,
    )
    .await;
}
