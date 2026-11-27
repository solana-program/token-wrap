use {
    crate::common::helpers::{
        create_unwrapped_mint, execute_create_mint, TestEnv, TOKEN_WRAP_CLI_BIN,
    },
    solana_keypair::{write_keypair_file, Keypair},
    solana_program_pack::Pack,
    solana_pubkey::Pubkey,
    spl_token::{self, state::Mint as SplTokenMint},
    spl_token_2022_interface::{
        extension::{
            confidential_transfer::ConfidentialTransferMint, metadata_pointer::MetadataPointer,
            BaseStateWithExtensions, PodStateWithExtensions,
        },
        pod::PodMint,
    },
    spl_token_wrap::{
        self, get_wrapped_mint_address, get_wrapped_mint_authority,
        get_wrapped_mint_backpointer_address, state::Backpointer,
    },
    std::process::Command,
    tempfile::NamedTempFile,
};

pub async fn test_create_mint(env: &TestEnv) {
    let unwrapped_token_program = spl_token::id();
    let wrapped_token_program = spl_token_2022_interface::id();
    let unwrapped_mint = create_unwrapped_mint(env, &unwrapped_token_program).await;
    execute_create_mint(env, &unwrapped_mint, &wrapped_token_program).await;

    // Derive expected account addresses
    let wrapped_mint_address = get_wrapped_mint_address(&unwrapped_mint, &wrapped_token_program);
    let backpointer_address = get_wrapped_mint_backpointer_address(&wrapped_mint_address);

    // Fetch created accounts
    let wrapped_mint_account = env
        .rpc_client
        .get_account(&wrapped_mint_address)
        .await
        .unwrap();
    let backpointer_account = env
        .rpc_client
        .get_account(&backpointer_address)
        .await
        .unwrap();

    // Verify owners
    assert_eq!(wrapped_mint_account.owner, wrapped_token_program);
    assert_eq!(backpointer_account.owner, spl_token_wrap::id());

    // Verify mint properties
    let unwrapped_mint_account = env.rpc_client.get_account(&unwrapped_mint).await.unwrap();
    let unwrapped_mint_data = SplTokenMint::unpack(&unwrapped_mint_account.data).unwrap();
    let wrapped_mint_state =
        PodStateWithExtensions::<PodMint>::unpack(&wrapped_mint_account.data).unwrap();
    assert_eq!(
        wrapped_mint_state.base.decimals,
        unwrapped_mint_data.decimals
    );

    // Verify backpointer data
    let backpointer = *bytemuck::from_bytes::<Backpointer>(&backpointer_account.data);
    assert_eq!(backpointer.unwrapped_mint, unwrapped_mint);

    // Verify extension state
    assert_eq!(wrapped_mint_state.get_extension_types().unwrap().len(), 2);

    assert!(wrapped_mint_state
        .get_extension::<ConfidentialTransferMint>()
        .is_ok());

    // Verify MetadataPointer content
    let pointer_ext = wrapped_mint_state
        .get_extension::<MetadataPointer>()
        .unwrap();
    let expected_mint_authority = get_wrapped_mint_authority(&wrapped_mint_address);
    assert_eq!(
        Option::<Pubkey>::from(pointer_ext.authority).unwrap(),
        expected_mint_authority
    );
    assert_eq!(
        Option::<Pubkey>::from(pointer_ext.metadata_address).unwrap(),
        wrapped_mint_address
    );
}

pub async fn test_create_mint_dry_run_fails_on_simulation_error(env: &TestEnv) {
    let unwrapped_token_program = spl_token::id();
    let wrapped_token_program = spl_token_2022_interface::id();
    let unwrapped_mint = create_unwrapped_mint(env, &unwrapped_token_program).await;

    // 1. Unfunded fee payer fails simulation
    let unfunded_payer = Keypair::new();
    let unfunded_payer_file = NamedTempFile::new().unwrap();
    write_keypair_file(&unfunded_payer, &unfunded_payer_file).unwrap();

    let output = Command::new(TOKEN_WRAP_CLI_BIN)
        .args([
            "create-mint",
            "-C",
            &env.config_file_path,
            "--fee-payer",
            unfunded_payer_file.path().to_str().unwrap(),
            "--dry-run",
            &unwrapped_mint.to_string(),
            &wrapped_token_program.to_string(),
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(!stdout.contains("Simulation succeeded"));
    assert!(
        stderr.contains("Simulation failed:"),
        "unexpected stderr: {stderr}"
    );

    // 2. On-chain program error with --verbose prints logs before failing
    let invalid_mint = Pubkey::new_unique();
    let output = Command::new(TOKEN_WRAP_CLI_BIN)
        .args([
            "create-mint",
            "-C",
            &env.config_file_path,
            "--dry-run",
            "--verbose",
            &invalid_mint.to_string(),
            &wrapped_token_program.to_string(),
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stdout.contains("Program log: Instruction: CreateMint"),
        "expected program logs in stdout: {stdout}"
    );
    assert!(!stdout.contains("Simulation succeeded"));
    assert!(
        stderr.contains("Simulation failed:"),
        "unexpected stderr: {stderr}"
    );
}
