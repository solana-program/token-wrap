use {
    crate::helpers::{
        create_unwrapped_mint, execute_create_mint, setup_test_env, TOKEN_WRAP_CLI_BIN,
    },
    mpl_token_metadata::{
        accounts::Metadata as MetaplexMetadata,
        instructions::{CreateMetadataAccountV3, CreateMetadataAccountV3InstructionArgs},
        types::DataV2,
        utils::clean,
    },
    serde_json::Value,
    serial_test::serial,
    solana_sdk_ids::system_program,
    solana_signer::Signer,
    solana_system_interface::instruction::transfer,
    solana_transaction::Transaction,
    spl_token_2022::{
        extension::{BaseStateWithExtensions, PodStateWithExtensions},
        pod::PodMint,
    },
    spl_token_metadata_interface::state::TokenMetadata,
    spl_token_wrap::get_wrapped_mint_address,
    std::process::Command,
};

mod helpers;

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn test_sync_metadata_from_spl_token_to_token2022() {
    let env = setup_test_env().await;

    // 1. Create a standard SPL Token mint
    let unwrapped_mint = create_unwrapped_mint(&env, &spl_token::id()).await;

    // 2. Create Metaplex metadata for the SPL Token mint
    let (metaplex_pda, _) = MetaplexMetadata::find_pda(&unwrapped_mint);
    let name = "Test Token".to_string();
    let symbol = "TEST".to_string();
    let uri = "http://test.com".to_string();

    let create_meta_ix = CreateMetadataAccountV3 {
        metadata: metaplex_pda,
        mint: unwrapped_mint,
        mint_authority: env.payer.pubkey(),
        payer: env.payer.pubkey(),
        update_authority: (env.payer.pubkey(), true),
        system_program: system_program::id(),
        rent: None,
    }
    .instruction(CreateMetadataAccountV3InstructionArgs {
        data: DataV2 {
            name: name.clone(),
            symbol: symbol.clone(),
            uri: uri.clone(),
            seller_fee_basis_points: 0,
            creators: None,
            collection: None,
            uses: None,
        },
        is_mutable: true,
        collection_details: None,
    });

    let meta_tx = Transaction::new_signed_with_payer(
        &[create_meta_ix],
        Some(&env.payer.pubkey()),
        &[&env.payer],
        env.rpc_client.get_latest_blockhash().await.unwrap(),
    );
    env.rpc_client
        .send_and_confirm_transaction(&meta_tx)
        .await
        .unwrap();

    // 3. Create the corresponding wrapped Token-2022 mint for the SPL Token mint
    execute_create_mint(&env, &unwrapped_mint, &spl_token_2022::id()).await;

    // 4. Fund the wrapped mint account for the extra space needed for the metadata
    //    extension
    let wrapped_mint_address = get_wrapped_mint_address(&unwrapped_mint, &spl_token_2022::id());
    let fund_tx = Transaction::new_signed_with_payer(
        &[transfer(
            &env.payer.pubkey(),
            &wrapped_mint_address,
            1_000_000_000,
        )],
        Some(&env.payer.pubkey()),
        &[&env.payer],
        env.rpc_client.get_latest_blockhash().await.unwrap(),
    );
    env.rpc_client
        .send_and_confirm_transaction(&fund_tx)
        .await
        .unwrap();

    // 5. Execute the sync-metadata-to-token2022 command
    let output = Command::new(TOKEN_WRAP_CLI_BIN)
        .args([
            "sync-metadata-to-token2022",
            "-C",
            &env.config_file_path,
            &unwrapped_mint.to_string(),
            "--metaplex",
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        panic!(
            "sync-metadata-to-token2022 command failed:\nSTDOUT:\n{}\nSTDERR:\n{}",
            stdout, stderr
        );
    }
    assert!(output.status.success());

    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        json["wrappedMint"].as_str().unwrap(),
        &wrapped_mint_address.to_string()
    );

    // 6. Verify the metadata was written correctly to the wrapped mint
    let wrapped_mint_account_after = env
        .rpc_client
        .get_account(&wrapped_mint_address)
        .await
        .unwrap();
    let wrapped_mint_state =
        PodStateWithExtensions::<PodMint>::unpack(&wrapped_mint_account_after.data).unwrap();
    let token_metadata = wrapped_mint_state
        .get_variable_len_extension::<TokenMetadata>()
        .unwrap();

    assert_eq!(clean(token_metadata.name), name);
    assert_eq!(clean(token_metadata.symbol), symbol);
    assert_eq!(clean(token_metadata.uri), uri);
    assert_eq!(token_metadata.mint, wrapped_mint_address);
}
