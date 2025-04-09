use {
    crate::helpers::{create_unwrapped_mint, setup_test_env, TestEnv, TOKEN_WRAP_CLI_BIN},
    serde_json::Value,
    serial_test::serial,
    solana_keypair::{write_keypair_file, Keypair},
    solana_program_pack::IsInitialized,
    solana_pubkey::Pubkey,
    solana_signer::Signer,
    solana_transaction::Transaction,
    spl_token::{self},
    spl_token_2022::{
        extension::PodStateWithExtensions,
        pod::PodAccount,
        {self},
    },
    spl_token_wrap::{get_wrapped_mint_address, get_wrapped_mint_authority},
    std::{process::Command, str::FromStr},
    tempfile::NamedTempFile,
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

    let cli_reported_token_program_str = cli_output["unwrappedTokenProgramId"].as_str().unwrap();
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
async fn test_create_ata_escrow_account_for_spl_token_mint() {
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
async fn test_create_ata_escrow_account_for_token2022_mint() {
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

#[tokio::test]
#[serial]
async fn test_create_escrow_account_with_signer() {
    let env = setup_test_env().await;
    let unwrapped_token_program_id = spl_token::id();
    let wrapped_token_program_id = spl_token_2022::id();

    let unwrapped_mint = create_unwrapped_mint(&env, &unwrapped_token_program_id).await;

    // Create a keypair for the escrow account
    let escrow_keypair = Keypair::new();
    let escrow_keypair_file = NamedTempFile::new().unwrap();
    write_keypair_file(&escrow_keypair, &escrow_keypair_file).unwrap();

    let mut command = Command::new(TOKEN_WRAP_CLI_BIN);
    let output = command
        .args([
            "create-escrow-account",
            "-C",
            &env.config_file_path,
            &unwrapped_mint.to_string(),
            &wrapped_token_program_id.to_string(),
            "--escrow-account-signer",
            escrow_keypair_file.path().to_str().unwrap(),
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        panic!(
            "Creation with signer failed with status: {}\nStdout: {}\nStderr: {}",
            output.status, stdout, stderr
        );
    }

    let output_str = String::from_utf8(output.stdout).unwrap();
    let json_result: Value = serde_json::from_str(&output_str).unwrap();

    let wrapped_mint_address = get_wrapped_mint_address(&unwrapped_mint, &wrapped_token_program_id);
    let expected_owner_pda = get_wrapped_mint_authority(&wrapped_mint_address);

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
async fn test_create_escrow_account_signer_idempotent() {
    let env = setup_test_env().await;
    let unwrapped_token_program_id = spl_token::id();
    let wrapped_token_program_id = spl_token_2022::id();

    let unwrapped_mint = create_unwrapped_mint(&env, &unwrapped_token_program_id).await;

    let escrow_keypair = Keypair::new();
    let escrow_keypair_file = NamedTempFile::new().unwrap();
    write_keypair_file(&escrow_keypair, &escrow_keypair_file).unwrap();

    let args = [
        "create-escrow-account",
        "-C",
        &env.config_file_path,
        &unwrapped_mint.to_string(),
        &wrapped_token_program_id.to_string(),
        "--escrow-account-signer",
        escrow_keypair_file.path().to_str().unwrap(),
        "--idempotent",
    ];
    let mut command_a = Command::new(TOKEN_WRAP_CLI_BIN);
    let status_a = command_a.args(args).status().unwrap();
    assert!(status_a.success());

    // Second time, same arguments w/ idempotent successful
    let mut command_b = Command::new(TOKEN_WRAP_CLI_BIN);
    let status_b = command_b.args(args).status().unwrap();
    assert!(status_b.success());

    // Running without idempotent flag will raise an error on subsequent run
    let mut command_c = Command::new(TOKEN_WRAP_CLI_BIN);
    let output = command_c
        .args([
            "create-escrow-account",
            "-C",
            &env.config_file_path,
            &unwrapped_mint.to_string(),
            &wrapped_token_program_id.to_string(),
            "--escrow-account-signer",
            escrow_keypair_file.path().to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr
        .contains(format!("Escrow account {} already exists", escrow_keypair.pubkey()).as_str()));
}

#[tokio::test]
#[serial]
async fn test_create_escrow_account_ata_idempotent() {
    let env = setup_test_env().await;
    let unwrapped_token_program_id = spl_token::id();
    let wrapped_token_program_id = spl_token_2022::id();

    let unwrapped_mint = create_unwrapped_mint(&env, &unwrapped_token_program_id).await;

    let args = [
        "create-escrow-account",
        "-C",
        &env.config_file_path,
        &unwrapped_mint.to_string(),
        &wrapped_token_program_id.to_string(),
        "--idempotent",
    ];
    let mut command_a = Command::new(TOKEN_WRAP_CLI_BIN);
    let status_a = command_a.args(args).status().unwrap();
    assert!(status_a.success());

    // Second time, same arguments w/ idempotent successful
    let mut command_b = Command::new(TOKEN_WRAP_CLI_BIN);
    let status_b = command_b.args(args).status().unwrap();
    assert!(status_b.success());

    // Running without idempotent flag will raise an error on subsequent run
    let mut command_c = Command::new(TOKEN_WRAP_CLI_BIN);
    let output_c = command_c
        .args([
            "create-escrow-account",
            "-C",
            &env.config_file_path,
            &unwrapped_mint.to_string(),
            &wrapped_token_program_id.to_string(),
        ])
        .output()
        .unwrap();
    assert!(!output_c.status.success());
    let stderr_c = String::from_utf8(output_c.stderr).unwrap();
    assert!(stderr_c.contains("already exists"));
}

#[tokio::test]
#[serial]
async fn test_create_escrow_account_with_wrong_mint_owner() {
    let env = setup_test_env().await;

    let keypair = Keypair::new();
    let wrong_owner = Pubkey::new_unique();
    create_account(&env, &keypair, &wrong_owner).await;

    let mut command = Command::new(TOKEN_WRAP_CLI_BIN);
    let output = command
        .args([
            "create-escrow-account",
            "-C",
            &env.config_file_path,
            &keypair.pubkey().to_string(),
            &spl_token_2022::id().to_string(),
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("is not owned by a token program"));
}

#[tokio::test]
#[serial]
async fn test_create_escrow_account_with_wrong_account_type() {
    let env = setup_test_env().await;

    let keypair = Keypair::new();

    // note no data in account
    create_account(&env, &keypair, &spl_token_2022::id()).await;

    let mut command = Command::new(TOKEN_WRAP_CLI_BIN);
    let output = command
        .args([
            "create-escrow-account",
            "-C",
            &env.config_file_path,
            &keypair.pubkey().to_string(),
            &spl_token_2022::id().to_string(),
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("Failed to unpack as spl token mint:"));
}

async fn create_account(env: &TestEnv, key_pair: &Keypair, owner: &Pubkey) {
    let tx = Transaction::new_signed_with_payer(
        &[solana_system_interface::instruction::create_account(
            &env.payer.pubkey(),
            &key_pair.pubkey(),
            env.rpc_client
                .get_minimum_balance_for_rent_exemption(100)
                .await
                .unwrap(),
            100,
            owner,
        )],
        Some(&env.payer.pubkey()),
        &[&env.payer, key_pair],
        env.rpc_client.get_latest_blockhash().await.unwrap(),
    );
    env.rpc_client
        .send_and_confirm_transaction(&tx)
        .await
        .unwrap();
}
