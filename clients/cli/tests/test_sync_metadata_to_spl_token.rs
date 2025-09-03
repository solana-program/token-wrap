use {
    crate::helpers::{extract_signers, setup_test_env, TOKEN_WRAP_CLI_BIN},
    mpl_token_metadata::accounts::Metadata as MetaplexMetadata,
    serde_json::Value,
    serial_test::serial,
    solana_signer::Signer,
    solana_transaction::Transaction,
    spl_token_2022::extension::{metadata_pointer, ExtensionType},
    spl_token_metadata_interface::instruction as tmi_ix,
    spl_token_wrap::get_wrapped_mint_address,
    std::process::Command,
};

mod helpers;

async fn create_token2022_mint_with_metadata(env: &crate::helpers::TestEnv) -> solana_pubkey::Pubkey {
    use solana_keypair::Keypair;
    use solana_system_interface::instruction::create_account;
    use spl_token_2022::{instruction::initialize_mint2, state::Mint};

    let payer = &env.payer;
    let mint = Keypair::new();

    let mint_len = ExtensionType::try_calculate_account_len::<Mint>(&[ExtensionType::MetadataPointer])
        .unwrap();
    let rent = env
        .rpc_client
        .get_minimum_balance_for_rent_exemption(mint_len)
        .await
        .unwrap();

    let ix_create = create_account(
        &payer.pubkey(),
        &mint.pubkey(),
        rent,
        mint_len as u64,
        &spl_token_2022::id(),
    );
    let ix_pointer = metadata_pointer::instruction::initialize(
        &spl_token_2022::id(),
        &mint.pubkey(),
        Some(payer.pubkey()),
        Some(mint.pubkey()),
    )
    .unwrap();
    let ix_init_mint = initialize_mint2(
        &spl_token_2022::id(),
        &mint.pubkey(),
        &payer.pubkey(),
        None,
        9,
    )
    .unwrap();
    let ix_init_meta = tmi_ix::initialize(
        &spl_token_2022::id(),
        &mint.pubkey(),
        &payer.pubkey(),
        &mint.pubkey(),
        &payer.pubkey(),
        "Name".to_string(),
        "SYM".to_string(),
        "uri".to_string(),
    );
    let bh = env.rpc_client.get_latest_blockhash().await.unwrap();
    let tx = Transaction::new_signed_with_payer(
        &[ix_create, ix_pointer, ix_init_mint, ix_init_meta],
        Some(&payer.pubkey()),
        &[payer, &mint],
        bh,
    );
    env.rpc_client
        .send_and_confirm_transaction(&tx)
        .await
        .unwrap();
    mint.pubkey()
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn test_sync_metadata_to_spl_token_sign_only_builds_correct_pdas() {
    let env = setup_test_env().await;

    // Choose a random unwrapped mint and derive PDAs; no on-chain accounts required for sign-only
    let unwrapped_mint = solana_pubkey::Pubkey::new_unique();
    let wrapped_mint = get_wrapped_mint_address(&unwrapped_mint, &spl_token::id());
    let (metaplex_pda, _) = MetaplexMetadata::find_pda(&wrapped_mint);

    let blockhash = env.rpc_client.get_latest_blockhash().await.unwrap();
    let output = Command::new(TOKEN_WRAP_CLI_BIN)
        .args([
            "sync-metadata-to-spl-token",
            "-C",
            &env.config_file_path,
            &unwrapped_mint.to_string(),
            "--sign-only",
            "--blockhash",
            &blockhash.to_string(),
            "--output",
            "json",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());

    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        json["wrappedMint"].as_str().unwrap(),
        &wrapped_mint.to_string()
    );
    assert_eq!(
        json["metaplexMetadata"].as_str().unwrap(),
        &metaplex_pda.to_string()
    );

    // Ensure payer signer is present in sign-only data
    let signers = extract_signers(&output.stdout);
    assert!(signers
        .iter()
        .any(|s| s.starts_with(&env.payer.pubkey().to_string())));
}
