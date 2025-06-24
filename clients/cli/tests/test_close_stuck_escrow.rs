use {
    crate::helpers::{
        create_associated_token_account, create_unwrapped_mint, execute_create_mint,
        setup_test_env, TOKEN_WRAP_CLI_BIN,
    },
    serial_test::serial,
    solana_keypair::Keypair,
    solana_signer::Signer,
    solana_system_interface::instruction::create_account,
    solana_transaction::Transaction,
    spl_token_2022::{
        extension::{transfer_fee::instruction::initialize_transfer_fee_config, ExtensionType},
        instruction::{initialize_mint2, initialize_mint_close_authority},
        pod::PodMint,
    },
    spl_token_wrap::{get_wrapped_mint_address, get_wrapped_mint_authority},
    std::process::Command,
};

mod helpers;

#[tokio::test]
#[serial]
async fn test_only_token_2022_allowed() {
    let env = setup_test_env().await;

    // Create unwrapped mint with spl-token (not spl-token-2022)
    let unwrapped_token_program = spl_token::id();
    let wrapped_token_program = spl_token_2022::id();
    let unwrapped_mint = create_unwrapped_mint(&env, &unwrapped_token_program).await;

    let output = Command::new(TOKEN_WRAP_CLI_BIN)
        .args(vec![
            "close-stuck-escrow".to_string(),
            "-C".to_string(),
            env.config_file_path.clone(),
            unwrapped_mint.to_string(),
            env.payer.pubkey().to_string(),
            wrapped_token_program.to_string(),
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("CloseStuckEscrow only works with spl-token-2022 unwrapped mints"));
}

#[tokio::test]
#[serial]
async fn test_create_mint_close_stuck_escrow_fails() {
    let env = setup_test_env().await;

    // 1. Create unwrapped mint with spl-token-2022
    let unwrapped_token_program = spl_token_2022::id();
    let wrapped_token_program = spl_token_2022::id();
    let unwrapped_mint = create_unwrapped_mint(&env, &unwrapped_token_program).await;

    // 2. Create wrapped mint
    execute_create_mint(&env, &unwrapped_mint, &wrapped_token_program).await;

    // 3. Create escrow account (the ATA for the wrapped mint authority)
    let wrapped_mint = get_wrapped_mint_address(&unwrapped_mint, &wrapped_token_program);
    let wrapped_mint_authority = get_wrapped_mint_authority(&wrapped_mint);
    create_associated_token_account(
        &env,
        &unwrapped_token_program,
        &unwrapped_mint,
        &wrapped_mint_authority,
    )
    .await;

    // 4. Try to close stuck escrow (should fail because escrow is not stuck)
    let output = Command::new(TOKEN_WRAP_CLI_BIN)
        .args(vec![
            "close-stuck-escrow".to_string(),
            "-C".to_string(),
            env.config_file_path.clone(),
            unwrapped_mint.to_string(),
            env.payer.pubkey().to_string(),
            wrapped_token_program.to_string(),
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
}

#[tokio::test]
#[serial]
async fn test_successful_close() {
    let env = setup_test_env().await;
    let unwrapped_token_program = spl_token_2022::id();
    let wrapped_token_program = spl_token_2022::id();
    let close_authority = env.payer.pubkey();
    let unwrapped_mint_keypair = Keypair::new();
    let unwrapped_mint = unwrapped_mint_keypair.pubkey();

    // 1. Create unwrapped mint with spl-token-2022 with Close authority extension
    let extensions = vec![ExtensionType::MintCloseAuthority];
    let space = ExtensionType::try_calculate_account_len::<PodMint>(&extensions).unwrap();
    let rent = env
        .rpc_client
        .get_minimum_balance_for_rent_exemption(space)
        .await
        .unwrap();
    let blockhash = env.rpc_client.get_latest_blockhash().await.unwrap();

    let create_account_ix = create_account(
        &env.payer.pubkey(),
        &unwrapped_mint,
        rent,
        space as u64,
        &unwrapped_token_program,
    );

    let init_close_authority_ix = initialize_mint_close_authority(
        &unwrapped_token_program,
        &unwrapped_mint,
        Some(&close_authority),
    )
    .unwrap();

    let init_mint_ix = initialize_mint2(
        &unwrapped_token_program,
        &unwrapped_mint,
        &env.payer.pubkey(),
        None,
        0,
    )
    .unwrap();

    let tx = Transaction::new_signed_with_payer(
        &[create_account_ix, init_close_authority_ix, init_mint_ix],
        Some(&env.payer.pubkey()),
        &[&env.payer, &unwrapped_mint_keypair],
        blockhash,
    );
    env.rpc_client
        .send_and_confirm_transaction(&tx)
        .await
        .unwrap();

    // 2. Create wrapped mint
    execute_create_mint(&env, &unwrapped_mint, &wrapped_token_program).await;

    // 3. Create the initial escrow account
    let wrapped_mint = get_wrapped_mint_address(&unwrapped_mint, &wrapped_token_program);
    let wrapped_mint_authority = get_wrapped_mint_authority(&wrapped_mint);
    let escrow_address = create_associated_token_account(
        &env,
        &unwrapped_token_program,
        &unwrapped_mint,
        &wrapped_mint_authority,
    )
    .await;

    // 4. Close the unwrapped mint
    let blockhash = env.rpc_client.get_latest_blockhash().await.unwrap();
    let close_ix = spl_token_2022::instruction::close_account(
        &unwrapped_token_program,
        &unwrapped_mint,
        &env.payer.pubkey(),
        &env.payer.pubkey(),
        &[],
    )
    .unwrap();

    let tx = Transaction::new_signed_with_payer(
        &[close_ix],
        Some(&env.payer.pubkey()),
        &[&env.payer],
        blockhash,
    );
    env.rpc_client
        .send_and_confirm_transaction(&tx)
        .await
        .unwrap();
    let mint_account = env.rpc_client.get_account(&unwrapped_mint).await;
    assert!(mint_account.is_err()); // Confirm it's closed

    // 5. Re-create the unwrapped mint at the same address with a transfer fee
    //    extension
    let extensions = vec![ExtensionType::TransferFeeConfig];
    let space = ExtensionType::try_calculate_account_len::<PodMint>(&extensions).unwrap();
    let rent = env
        .rpc_client
        .get_minimum_balance_for_rent_exemption(space)
        .await
        .unwrap();
    let blockhash = env.rpc_client.get_latest_blockhash().await.unwrap();

    let create_account_ix = create_account(
        &env.payer.pubkey(),
        &unwrapped_mint,
        rent,
        space as u64,
        &unwrapped_token_program,
    );

    let init_mint_ix = initialize_mint2(
        &unwrapped_token_program,
        &unwrapped_mint,
        &env.payer.pubkey(),
        None,
        0,
    )
    .unwrap();

    let init_transfer_fee_ix = initialize_transfer_fee_config(
        &unwrapped_token_program,
        &unwrapped_mint,
        Some(&env.payer.pubkey()),
        Some(&env.payer.pubkey()),
        100,
        1_000,
    )
    .unwrap();

    let tx = Transaction::new_signed_with_payer(
        &[create_account_ix, init_transfer_fee_ix, init_mint_ix],
        Some(&env.payer.pubkey()),
        &[&env.payer, &unwrapped_mint_keypair],
        blockhash,
    );
    env.rpc_client
        .send_and_confirm_transaction(&tx)
        .await
        .unwrap();

    // 6. Close the stuck escrow account
    let output = Command::new(TOKEN_WRAP_CLI_BIN)
        .args([
            "close-stuck-escrow",
            "-C",
            &env.config_file_path,
            &unwrapped_mint.to_string(),
            &env.payer.pubkey().to_string(),
            &wrapped_token_program.to_string(),
        ])
        .output()
        .unwrap();

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        panic!(
            "close-stuck-escrow failed with status: {}\nStdout: {}\nStderr:
    {}",
            output.status, stdout, stderr
        );
    }

    // 7. Verify the old escrow account is closed
    let escrow_account_after_close = env.rpc_client.get_account(&escrow_address).await;
    assert!(escrow_account_after_close.is_err());
}
