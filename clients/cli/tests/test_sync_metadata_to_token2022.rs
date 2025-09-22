use {
    crate::helpers::{
        create_metaplex_metadata, create_unwrapped_mint, execute_create_mint, setup_test_env,
        TestEnv, TOKEN_WRAP_CLI_BIN,
    },
    mpl_token_metadata::utils::clean,
    serde_json::Value,
    serial_test::serial,
    solana_keypair::Keypair,
    solana_pubkey::Pubkey,
    solana_sdk_ids::system_program,
    solana_signer::Signer,
    solana_system_interface::instruction::{create_account, transfer},
    solana_transaction::Transaction,
    spl_token_2022::{
        extension::{
            metadata_pointer, BaseStateWithExtensions, ExtensionType, PodStateWithExtensions,
        },
        pod::PodMint,
        state::Mint,
    },
    spl_token_metadata_interface::state::TokenMetadata,
    spl_token_wrap::get_wrapped_mint_address,
    std::process::Command,
};

mod helpers;

const NAME: &str = "xyz-dex";
const SYMBOL: &str = "XYZ";
const URI: &str = "http://test.com";

async fn create_token2022_mint(
    env: &TestEnv,
    pointer_address: Option<Pubkey>,
    has_pointer_extension: bool,
    has_token_metadata: bool,
) -> Keypair {
    let unwrapped_mint_kp = Keypair::new();

    let mut extensions = vec![];
    if has_pointer_extension {
        extensions.push(ExtensionType::MetadataPointer);
    }

    let mint_size = ExtensionType::try_calculate_account_len::<Mint>(&extensions).unwrap();
    let rent = env
        .rpc_client
        .get_minimum_balance_for_rent_exemption(mint_size)
        .await
        .unwrap();

    let mut ixs = vec![create_account(
        &env.payer.pubkey(),
        &unwrapped_mint_kp.pubkey(),
        rent,
        mint_size as u64,
        &spl_token_2022::id(),
    )];

    if has_pointer_extension {
        ixs.push(
            metadata_pointer::instruction::initialize(
                &spl_token_2022::id(),
                &unwrapped_mint_kp.pubkey(),
                Some(env.payer.pubkey()),
                pointer_address,
            )
            .unwrap(),
        );
    }

    ixs.push(
        spl_token_2022::instruction::initialize_mint(
            &spl_token_2022::id(),
            &unwrapped_mint_kp.pubkey(),
            &env.payer.pubkey(),
            None,
            9,
        )
        .unwrap(),
    );

    let blockhash = env.rpc_client.get_latest_blockhash().await.unwrap();
    let init_tx = Transaction::new_signed_with_payer(
        &ixs,
        Some(&env.payer.pubkey()),
        &[&env.payer, &unwrapped_mint_kp],
        blockhash,
    );
    env.rpc_client
        .send_and_confirm_transaction(&init_tx)
        .await
        .unwrap();

    if has_token_metadata {
        // Fund for realloc
        let topup_tx = Transaction::new_signed_with_payer(
            &[transfer(
                &env.payer.pubkey(),
                &unwrapped_mint_kp.pubkey(),
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

        let update_authority = env.payer.pubkey();
        let meta_tx = Transaction::new_signed_with_payer(
            &[
                spl_token_metadata_interface::instruction::initialize(
                    &spl_token_2022::id(),
                    &unwrapped_mint_kp.pubkey(),
                    &update_authority,
                    &unwrapped_mint_kp.pubkey(),
                    &update_authority,
                    NAME.to_string(),
                    SYMBOL.to_string(),
                    URI.to_string(),
                ),
                spl_token_metadata_interface::instruction::update_field(
                    &spl_token_2022::id(),
                    &unwrapped_mint_kp.pubkey(),
                    &update_authority,
                    spl_token_metadata_interface::state::Field::Name,
                    NAME.to_string(),
                ),
                spl_token_metadata_interface::instruction::update_field(
                    &spl_token_2022::id(),
                    &unwrapped_mint_kp.pubkey(),
                    &update_authority,
                    spl_token_metadata_interface::state::Field::Symbol,
                    SYMBOL.to_string(),
                ),
                spl_token_metadata_interface::instruction::update_field(
                    &spl_token_2022::id(),
                    &unwrapped_mint_kp.pubkey(),
                    &update_authority,
                    spl_token_metadata_interface::state::Field::Uri,
                    URI.to_string(),
                ),
            ],
            Some(&env.payer.pubkey()),
            &[&env.payer],
            env.rpc_client.get_latest_blockhash().await.unwrap(),
        );
        env.rpc_client
            .send_and_confirm_transaction(&meta_tx)
            .await
            .unwrap();
    }

    unwrapped_mint_kp
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn test_sync_metadata_from_spl_token_to_token2022() {
    let env = setup_test_env().await;

    // 1. Create a standard SPL Token mint
    let unwrapped_mint = create_unwrapped_mint(&env, &spl_token::id()).await;

    // 2. Create Metaplex metadata for the SPL Token mint
    let name = "Test Token".to_string();
    let symbol = "TEST".to_string();
    let uri = "http://test.com".to_string();
    create_metaplex_metadata(
        &env,
        &unwrapped_mint,
        spl_token::id(),
        name.clone(),
        symbol.clone(),
        uri.clone(),
    )
    .await;

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

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn test_sync_from_token2022_with_self_referential_pointer() {
    let env = setup_test_env().await;

    // 1. Create unwrapped T22 mint with self-referential pointer and metadata
    let unwrapped_mint_kp = create_token2022_mint(
        &env,
        Some(Pubkey::new_unique()), // placeholder, will be updated
        true,
        true,
    )
    .await;
    let unwrapped_mint = unwrapped_mint_kp.pubkey();

    // Update pointer to be self-referential
    let update_pointer_ix = metadata_pointer::instruction::update(
        &spl_token_2022::id(),
        &unwrapped_mint,
        &env.payer.pubkey(),
        &[],
        Some(unwrapped_mint),
    )
    .unwrap();
    let tx = Transaction::new_signed_with_payer(
        &[update_pointer_ix],
        Some(&env.payer.pubkey()),
        &[&env.payer],
        env.rpc_client.get_latest_blockhash().await.unwrap(),
    );
    env.rpc_client
        .send_and_confirm_transaction(&tx)
        .await
        .unwrap();

    // 2. Create and fund wrapped mint
    execute_create_mint(&env, &unwrapped_mint, &spl_token_2022::id()).await;
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

    // 3. Execute sync command
    let output = Command::new(TOKEN_WRAP_CLI_BIN)
        .args([
            "sync-metadata-to-token2022",
            "-C",
            &env.config_file_path,
            &unwrapped_mint.to_string(),
        ])
        .output()
        .unwrap();
    assert!(output.status.success());

    // 4. Verify synced metadata
    let wrapped_mint_account = env
        .rpc_client
        .get_account(&wrapped_mint_address)
        .await
        .unwrap();
    let wrapped_mint_state =
        PodStateWithExtensions::<PodMint>::unpack(&wrapped_mint_account.data).unwrap();
    let token_metadata = wrapped_mint_state
        .get_variable_len_extension::<TokenMetadata>()
        .unwrap();

    assert_eq!(clean(token_metadata.name), NAME);
    assert_eq!(clean(token_metadata.symbol), SYMBOL);
    assert_eq!(clean(token_metadata.uri), URI);
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn test_sync_from_token2022_with_external_metaplex_pointer() {
    let env = setup_test_env().await;

    // 1. Create an external Metaplex metadata account
    let dummy_mint = create_unwrapped_mint(&env, &spl_token::id()).await;
    let name = "External Metaplex".to_string();
    let symbol = "EXTM".to_string();
    let uri = "http://external.com".to_string();
    let metaplex_pda = create_metaplex_metadata(
        &env,
        &dummy_mint,
        spl_token::id(),
        name.clone(),
        symbol.clone(),
        uri.clone(),
    )
    .await;

    // 2. Create unwrapped T22 mint pointing to the external metadata
    let unwrapped_mint_kp = create_token2022_mint(
        &env,
        Some(metaplex_pda),
        true,  // has_pointer_extension
        false, // has_token_metadata
    )
    .await;
    let unwrapped_mint = unwrapped_mint_kp.pubkey();

    // 3. Create and fund wrapped mint
    execute_create_mint(&env, &unwrapped_mint, &spl_token_2022::id()).await;
    let wrapped_mint_address = get_wrapped_mint_address(&unwrapped_mint, &spl_token_2022::id());

    let funding_amount = 1_000_000_000;
    let fund_tx = Transaction::new_signed_with_payer(
        &[transfer(
            &env.payer.pubkey(),
            &wrapped_mint_address,
            funding_amount,
        )],
        Some(&env.payer.pubkey()),
        &[&env.payer],
        env.rpc_client.get_latest_blockhash().await.unwrap(),
    );
    env.rpc_client
        .send_and_confirm_transaction(&fund_tx)
        .await
        .unwrap();

    // 4. Execute sync command
    let output = Command::new(TOKEN_WRAP_CLI_BIN)
        .args([
            "sync-metadata-to-token2022",
            "-C",
            &env.config_file_path,
            &unwrapped_mint.to_string(),
            "--metadata-account",
            &metaplex_pda.to_string(),
        ])
        .output()
        .unwrap();

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        panic!(
            "sync-metadata-to-token2022 command failed with status: {}\nStdout: {}\nStderr: {}",
            output.status, stdout, stderr
        );
    }

    // 5. Verify synced metadata
    let wrapped_mint_account = env
        .rpc_client
        .get_account(&wrapped_mint_address)
        .await
        .unwrap();
    let wrapped_mint_state =
        PodStateWithExtensions::<PodMint>::unpack(&wrapped_mint_account.data).unwrap();
    let token_metadata = wrapped_mint_state
        .get_variable_len_extension::<TokenMetadata>()
        .unwrap();

    assert_eq!(clean(token_metadata.name), name);
    assert_eq!(clean(token_metadata.symbol), symbol);
    assert_eq!(clean(token_metadata.uri), uri);
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn test_sync_from_token2022_without_pointer_fallback() {
    let env = setup_test_env().await;

    // 1. Create unwrapped T22 mint with no pointer extension
    let unwrapped_mint_kp = create_token2022_mint(&env, None, false, false).await;
    let unwrapped_mint = unwrapped_mint_kp.pubkey();

    // 2. Create Metaplex metadata for the unwrapped mint (the fallback)
    let name = "No Pointer Fallback".to_string();
    let symbol = "NPF".to_string();
    let uri = "http://npf.com".to_string();
    create_metaplex_metadata(
        &env,
        &unwrapped_mint,
        spl_token_2022::id(),
        name.clone(),
        symbol.clone(),
        uri.clone(),
    )
    .await;

    // 3. Create and fund wrapped mint
    execute_create_mint(&env, &unwrapped_mint, &spl_token_2022::id()).await;
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

    // 4. Execute sync command
    let output = Command::new(TOKEN_WRAP_CLI_BIN)
        .args([
            "sync-metadata-to-token2022",
            "-C",
            &env.config_file_path,
            &unwrapped_mint.to_string(),
        ])
        .output()
        .unwrap();
    assert!(output.status.success());

    // 5. Verify synced metadata
    let wrapped_mint_account = env
        .rpc_client
        .get_account(&wrapped_mint_address)
        .await
        .unwrap();
    let wrapped_mint_state =
        PodStateWithExtensions::<PodMint>::unpack(&wrapped_mint_account.data).unwrap();
    let token_metadata = wrapped_mint_state
        .get_variable_len_extension::<TokenMetadata>()
        .unwrap();

    assert_eq!(clean(token_metadata.name), name);
    assert_eq!(clean(token_metadata.symbol), symbol);
    assert_eq!(clean(token_metadata.uri), uri);
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn test_fail_sync_from_invalid_mint_owner() {
    let env = setup_test_env().await;
    let invalid_account = Keypair::new();

    let tx = Transaction::new_signed_with_payer(
        &[create_account(
            &env.payer.pubkey(),
            &invalid_account.pubkey(),
            env.rpc_client
                .get_minimum_balance_for_rent_exemption(0)
                .await
                .unwrap(),
            0,
            &system_program::id(),
        )],
        Some(&env.payer.pubkey()),
        &[&env.payer, &invalid_account],
        env.rpc_client.get_latest_blockhash().await.unwrap(),
    );
    env.rpc_client
        .send_and_confirm_transaction(&tx)
        .await
        .unwrap();

    let output = Command::new(TOKEN_WRAP_CLI_BIN)
        .args([
            "sync-metadata-to-token2022",
            "-C",
            &env.config_file_path,
            &invalid_account.pubkey().to_string(),
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("is not an SPL Token or SPL Token-2022 mint"));
}
