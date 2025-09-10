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
    solana_keypair::Keypair,
    solana_sdk_ids::system_program,
    solana_signer::Signer,
    solana_system_interface::instruction::transfer,
    solana_transaction::Transaction,
    spl_token_2022::extension::ExtensionType,
    spl_token_wrap::{get_wrapped_mint_address, get_wrapped_mint_authority},
    std::process::Command,
};

mod helpers;

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn test_sync_metadata_from_token2022_to_spl_token() {
    let env = setup_test_env().await;

    // 1. Create an unwrapped Token-2022 mint with MetadataPointer and TokenMetadata
    let unwrapped_mint_kp = Keypair::new();
    let unwrapped_mint = unwrapped_mint_kp.pubkey();
    let name = "Test Token 2022".to_string();
    let symbol = "T22".to_string();
    let uri = "http://test2022.com".to_string();

    // Allocate and initialize the mint
    let mint_size = ExtensionType::try_calculate_account_len::<spl_token_2022::state::Mint>(&[
        ExtensionType::MetadataPointer,
    ])
    .unwrap();
    let rent = env
        .rpc_client
        .get_minimum_balance_for_rent_exemption(mint_size)
        .await
        .unwrap();
    let blockhash = env.rpc_client.get_latest_blockhash().await.unwrap();
    let init_tx = Transaction::new_signed_with_payer(
        &[
            solana_system_interface::instruction::create_account(
                &env.payer.pubkey(),
                &unwrapped_mint,
                rent,
                mint_size as u64,
                &spl_token_2022::id(),
            ),
            spl_token_2022::extension::metadata_pointer::instruction::initialize(
                &spl_token_2022::id(),
                &unwrapped_mint,
                Some(env.payer.pubkey()),
                Some(unwrapped_mint),
            )
            .unwrap(),
            spl_token_2022::instruction::initialize_mint(
                &spl_token_2022::id(),
                &unwrapped_mint,
                &env.payer.pubkey(),
                None,
                9,
            )
            .unwrap(),
        ],
        Some(&env.payer.pubkey()),
        &[&env.payer, &unwrapped_mint_kp],
        blockhash,
    );
    env.rpc_client
        .send_and_confirm_transaction(&init_tx)
        .await
        .unwrap();

    let update_authority = env.payer.pubkey();

    // Ensure mint has enough lamports for TokenMetadata reallocation
    let topup_tx = Transaction::new_signed_with_payer(
        &[solana_system_interface::instruction::transfer(
            &env.payer.pubkey(),
            &unwrapped_mint,
            1_000_000_000,
        )],
        Some(&env.payer.pubkey()),
        &[&env.payer],
        env.rpc_client.get_latest_blockhash().await.unwrap(),
    );

    env.rpc_client
        .send_and_confirm_transaction(&topup_tx)
        .await
        .unwrap();

    let init_meta_ix = spl_token_metadata_interface::instruction::initialize(
        &spl_token_2022::id(),
        &unwrapped_mint,
        &update_authority,
        &unwrapped_mint,
        &update_authority,
        name.clone(),
        symbol.clone(),
        uri.clone(),
    );
    let update_name_ix = spl_token_metadata_interface::instruction::update_field(
        &spl_token_2022::id(),
        &unwrapped_mint,
        &update_authority,
        spl_token_metadata_interface::state::Field::Name,
        name.clone(),
    );
    let update_symbol_ix = spl_token_metadata_interface::instruction::update_field(
        &spl_token_2022::id(),
        &unwrapped_mint,
        &update_authority,
        spl_token_metadata_interface::state::Field::Symbol,
        symbol.clone(),
    );
    let update_uri_ix = spl_token_metadata_interface::instruction::update_field(
        &spl_token_2022::id(),
        &unwrapped_mint,
        &update_authority,
        spl_token_metadata_interface::state::Field::Uri,
        uri.clone(),
    );

    let meta_tx = Transaction::new_signed_with_payer(
        &[
            init_meta_ix,
            update_name_ix,
            update_symbol_ix,
            update_uri_ix,
        ],
        Some(&env.payer.pubkey()),
        &[&env.payer],
        env.rpc_client.get_latest_blockhash().await.unwrap(),
    );
    env.rpc_client
        .send_and_confirm_transaction(&meta_tx)
        .await
        .unwrap();

    // 2. Create the corresponding wrapped SPL Token mint
    execute_create_mint(&env, &unwrapped_mint, &spl_token::id()).await;
    let wrapped_mint_address = get_wrapped_mint_address(&unwrapped_mint, &spl_token::id());

    // 3. Fund the wrapped mint authority PDA so it can pay for the Metaplex CPI
    let wrapped_mint_authority = get_wrapped_mint_authority(&wrapped_mint_address);
    let fund_tx = Transaction::new_signed_with_payer(
        &[transfer(
            &env.payer.pubkey(),
            &wrapped_mint_authority,
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

    // 4. Execute the sync-metadata-to-spl-token command
    let output = Command::new(TOKEN_WRAP_CLI_BIN)
        .args([
            "sync-metadata-to-spl-token",
            "-C",
            &env.config_file_path,
            &unwrapped_mint.to_string(),
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        panic!(
            "sync-metadata-to-spl-token command failed:\nSTDOUT:\n{}\nSTDERR:\n{}",
            stdout, stderr
        );
    }

    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    let metaplex_metadata_address_str = json["metaplexMetadata"].as_str().unwrap();

    // 5. Verify the metadata was written correctly to the Metaplex account
    let metaplex_metadata_account = env
        .rpc_client
        .get_account(&metaplex_metadata_address_str.parse().unwrap())
        .await
        .unwrap();
    let metaplex_metadata = MetaplexMetadata::from_bytes(&metaplex_metadata_account.data).unwrap();

    assert_eq!(clean(metaplex_metadata.name), name);
    assert_eq!(clean(metaplex_metadata.symbol), symbol);
    assert_eq!(clean(metaplex_metadata.uri), uri);
    assert_eq!(metaplex_metadata.update_authority, wrapped_mint_authority);
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn test_sync_metadata_from_spl_token_to_spl_token() {
    let env = setup_test_env().await;

    // 1. Create an unwrapped SPL Token mint
    let unwrapped_mint = create_unwrapped_mint(&env, &spl_token::id()).await;

    // 2. Create Metaplex metadata for the unwrapped mint
    let (metaplex_pda, _) = MetaplexMetadata::find_pda(&unwrapped_mint);
    let name = "Unwrapped SPL".to_string();
    let symbol = "USPL".to_string();
    let uri = "http://uspl.com".to_string();

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

    // 3. Create the corresponding wrapped SPL Token mint
    execute_create_mint(&env, &unwrapped_mint, &spl_token::id()).await;
    let wrapped_mint_address = get_wrapped_mint_address(&unwrapped_mint, &spl_token::id());

    // 4. Fund the wrapped mint authority PDA
    let wrapped_mint_authority = get_wrapped_mint_authority(&wrapped_mint_address);
    let fund_tx = Transaction::new_signed_with_payer(
        &[transfer(
            &env.payer.pubkey(),
            &wrapped_mint_authority,
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

    // 5. Execute the sync command, providing the source metadata account
    let output = Command::new(TOKEN_WRAP_CLI_BIN)
        .args([
            "sync-metadata-to-spl-token",
            "-C",
            &env.config_file_path,
            &unwrapped_mint.to_string(),
            "--source-metadata",
            &metaplex_pda.to_string(),
            "--output",
            "json",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());

    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    let metaplex_metadata_address_str = json["metaplexMetadata"].as_str().unwrap();

    // 6. Verify the metadata was written correctly to the new Metaplex account
    let metaplex_metadata_account = env
        .rpc_client
        .get_account(&metaplex_metadata_address_str.parse().unwrap())
        .await
        .unwrap();
    let metaplex_metadata = MetaplexMetadata::from_bytes(&metaplex_metadata_account.data).unwrap();

    assert_eq!(clean(metaplex_metadata.name), name);
    assert_eq!(clean(metaplex_metadata.symbol), symbol);
    assert_eq!(clean(metaplex_metadata.uri), uri);
    assert_eq!(metaplex_metadata.update_authority, wrapped_mint_authority);
}
