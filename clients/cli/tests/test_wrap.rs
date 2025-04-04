use {
    crate::helpers::{
        create_associated_token_account, create_test_multisig, create_token_account,
        create_unwrapped_mint, execute_create_mint, extract_signers, mint_to, setup_test_env,
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

#[tokio::test]
#[serial]
async fn test_wrap_single_signer_with_defaults() {
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
    let wrap_amount = 50;

    let status = Command::new(TOKEN_WRAP_CLI_BIN)
        .args(vec![
            "wrap".to_string(),
            "-C".to_string(),
            env.config_file_path.clone(),
            unwrapped_token_account.to_string(),
            escrow_account.to_string(),
            wrapped_token_program.to_string(),
            wrap_amount.to_string(),
        ])
        .status()
        .unwrap();
    assert!(status.success());

    assert_result(
        env,
        &unwrapped_token_account,
        starting_amount,
        &recipient_account,
        &escrow_account,
        wrap_amount,
    )
    .await;
}

#[tokio::test]
#[serial]
async fn test_wrap_single_signer_with_optional_flags() {
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

    let transfer_authority = Keypair::new();
    let authority_keypair_file = NamedTempFile::new().unwrap();
    write_keypair_file(&transfer_authority, &authority_keypair_file).unwrap();

    // Fund initial unwrapped token account
    let unwrapped_token_account = create_token_account(
        &env,
        &unwrapped_token_program,
        &unwrapped_mint,
        &transfer_authority.pubkey(),
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
    let wrap_amount = 50;

    let status = Command::new(TOKEN_WRAP_CLI_BIN)
        .args(vec![
            "wrap".to_string(),
            "-C".to_string(),
            env.config_file_path.clone(),
            unwrapped_token_account.to_string(),
            escrow_account.to_string(),
            wrapped_token_program.to_string(),
            wrap_amount.to_string(),
            "--unwrapped-token-program".to_string(),
            unwrapped_token_program.to_string(),
            "--unwrapped-mint".to_string(),
            unwrapped_mint.to_string(),
            "--recipient-token-account".to_string(),
            recipient_account.to_string(),
            "--transfer-authority".to_string(),
            authority_keypair_file.path().to_str().unwrap().to_string(),
        ])
        .status()
        .unwrap();
    assert!(status.success());

    assert_result(
        env,
        &unwrapped_token_account,
        starting_amount,
        &recipient_account,
        &escrow_account,
        wrap_amount,
    )
    .await;
}

async fn assert_result(
    env: TestEnv,
    unwrapped_token_account: &Pubkey,
    starting_amount: u64,
    recipient_account: &Pubkey,
    escrow_account: &Pubkey,
    wrap_amount: u64,
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
        starting_amount.checked_sub(wrap_amount).unwrap()
    );

    // Escrow account should have the tokens
    let escrow_account_data = env
        .rpc_client
        .get_account_data(escrow_account)
        .await
        .unwrap();
    let escrow_token_state =
        PodStateWithExtensions::<PodAccount>::unpack(&escrow_account_data).unwrap();
    assert_eq!(u64::from(escrow_token_state.base.amount), wrap_amount);

    // Recipient should have wrapped tokens
    let wrapped_account = env.rpc_client.get_account(recipient_account).await.unwrap();
    assert_eq!(wrapped_account.owner, spl_token_2022::id());
    let wrapped_token_state =
        PodStateWithExtensions::<PodAccount>::unpack(&wrapped_account.data).unwrap();
    assert_eq!(u64::from(wrapped_token_state.base.amount), wrap_amount);
}

#[tokio::test]
#[serial]
async fn test_wrap_with_multisig() {
    let mut env = setup_test_env().await;

    let (multisig_pubkey, multisig_members) = create_test_multisig(&mut env).await.unwrap();

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

    let unwrapped_token_account = create_token_account(
        &env,
        &unwrapped_token_program,
        &unwrapped_mint,
        &multisig_pubkey,
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

    let wrapped_mint = get_wrapped_mint_address(&unwrapped_mint, &wrapped_token_program);
    let recipient_account =
        create_associated_token_account(&env, &wrapped_token_program, &wrapped_mint).await;

    let wrapped_mint_authority = get_wrapped_mint_authority(&wrapped_mint);
    let escrow_account = create_token_account(
        &env,
        &unwrapped_token_program,
        &unwrapped_mint,
        &wrapped_mint_authority,
    )
    .await;

    let wrap_amount = 50;

    let multisig_member_1 = multisig_members.first().unwrap();
    let member_1_keypair_file = NamedTempFile::new().unwrap();
    write_keypair_file(multisig_member_1, &member_1_keypair_file).unwrap();

    let multisig_member_2 = multisig_members.get(1).unwrap();
    let member_2_keypair_file = NamedTempFile::new().unwrap();
    write_keypair_file(multisig_member_2, &member_2_keypair_file).unwrap();

    let blockhash = env.rpc_client.get_latest_blockhash().await.unwrap();

    // --- Signer 1 ---
    // multisig member #1 passes their keypair and the pubkeys
    // for multisig member #2 and fee payer
    let output1 = Command::new(TOKEN_WRAP_CLI_BIN)
        .args(vec![
            "wrap".to_string(),
            "-C".to_string(),
            env.config_file_path.clone(),
            unwrapped_token_account.to_string(),
            escrow_account.to_string(),
            wrapped_token_program.to_string(),
            wrap_amount.to_string(),
            "--fee-payer".to_string(),
            env.payer.pubkey().to_string(),
            "--recipient-token-account".to_string(),
            recipient_account.to_string(),
            "--transfer-authority".to_string(),
            multisig_pubkey.to_string(),
            "--multisig-signer".to_string(),
            member_1_keypair_file.path().to_str().unwrap().to_string(),
            "--multisig-signer".to_string(),
            multisig_member_2.pubkey().to_string(),
            "--blockhash".to_string(),
            blockhash.to_string(),
            "--sign-only".to_string(),
            "--output".to_string(),
            "json".to_string(),
        ])
        .output()
        .unwrap();

    if !output1.status.success() {
        let stderr = String::from_utf8_lossy(&output1.stderr);
        let stdout = String::from_utf8_lossy(&output1.stdout);
        panic!(
            "output1 failed with status: {}\nStdout: {}\nStderr: {}",
            output1.status, stdout, stderr
        );
    }

    let signers1 = extract_signers(&output1.stdout);
    assert_eq!(signers1.len(), 1);
    let member_1_signature = signers1.first().cloned().unwrap();

    // --- Signer 2 ---
    // multisig member #2 passes their keypair and the pubkeys
    // for multisig member #1 and fee payer
    let output2 = Command::new(TOKEN_WRAP_CLI_BIN)
        .args(vec![
            "wrap".to_string(),
            "-C".to_string(),
            env.config_file_path.clone(),
            unwrapped_token_account.to_string(),
            escrow_account.to_string(),
            wrapped_token_program.to_string(),
            wrap_amount.to_string(),
            "--fee-payer".to_string(),
            env.payer.pubkey().to_string(),
            "--recipient-token-account".to_string(),
            recipient_account.to_string(),
            "--transfer-authority".to_string(),
            multisig_pubkey.to_string(),
            "--multisig-signer".to_string(),
            multisig_member_1.pubkey().to_string(),
            "--multisig-signer".to_string(),
            member_2_keypair_file.path().to_str().unwrap().to_string(),
            "--blockhash".to_string(),
            blockhash.to_string(),
            "--sign-only".to_string(),
            "--output".to_string(),
            "json".to_string(),
        ])
        .output()
        .unwrap();

    if !output2.status.success() {
        let stderr = String::from_utf8_lossy(&output1.stderr);
        let stdout = String::from_utf8_lossy(&output1.stdout);
        panic!(
            "output2 failed with exit code: {}\nStdout: {}\nStderr: {}",
            output2.status, stdout, stderr
        );
    }

    let signers2 = extract_signers(&output2.stdout);
    assert_eq!(signers2.len(), 1);
    let member_2_signature = signers2.first().cloned().unwrap();

    // --- Final Broadcaster ---
    // Passes the keypair for feepayer (default behavior)
    // and the signatures for member #1 & #2
    let status = Command::new(TOKEN_WRAP_CLI_BIN)
        .args(vec![
            "wrap".to_string(),
            "-C".to_string(),
            env.config_file_path.clone(),
            unwrapped_token_account.to_string(),
            escrow_account.to_string(),
            wrapped_token_program.to_string(),
            wrap_amount.to_string(),
            "--recipient-token-account".to_string(),
            recipient_account.to_string(),
            "--transfer-authority".to_string(),
            multisig_pubkey.to_string(),
            "--multisig-signer".to_string(),
            multisig_member_1.pubkey().to_string(),
            "--multisig-signer".to_string(),
            multisig_member_2.pubkey().to_string(),
            "--blockhash".to_string(),
            blockhash.to_string(),
            "--signer".to_string(),
            member_1_signature,
            "--signer".to_string(),
            member_2_signature,
        ])
        .status()
        .unwrap();
    assert!(status.success());

    // Verify the results of the wrap transaction
    assert_result(
        env,
        &unwrapped_token_account,
        starting_amount,
        &recipient_account,
        &escrow_account,
        wrap_amount,
    )
    .await;
}
