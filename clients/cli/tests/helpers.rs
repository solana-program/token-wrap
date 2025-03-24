#![allow(dead_code)]

use {
    solana_cli_config::Config as SolanaConfig,
    solana_client::nonblocking::rpc_client::RpcClient,
    solana_keypair::{write_keypair_file, Keypair},
    solana_program_pack::Pack,
    solana_pubkey::Pubkey,
    solana_sdk_ids::bpf_loader_upgradeable,
    solana_signer::Signer,
    solana_test_validator::{TestValidator, TestValidatorGenesis, UpgradeableProgramInfo},
    solana_transaction::Transaction,
    spl_token::{self, instruction::initialize_mint, state::Mint as SplTokenMint},
    spl_token_wrap::{get_wrapped_mint_address, get_wrapped_mint_authority},
    std::{path::PathBuf, process::Command, sync::Arc},
    tempfile::NamedTempFile,
};

pub const TOKEN_WRAP_CLI_BIN: &str = "../../target/debug/spl-token-wrap";

pub struct TestEnv {
    pub rpc_client: Arc<RpcClient>,
    pub payer: Keypair,
    pub config_file_path: String,
    // Persist these to keep them in scope
    _validator: TestValidator,
    _keypair_file: NamedTempFile,
    _config_file: NamedTempFile,
}

pub async fn start_validator() -> (TestValidator, Keypair) {
    solana_logger::setup();
    let mut test_validator_genesis = TestValidatorGenesis::default();

    test_validator_genesis.add_upgradeable_programs_with_path(&[UpgradeableProgramInfo {
        program_id: spl_token_wrap::id(),
        loader: bpf_loader_upgradeable::id(),
        program_path: PathBuf::from("../../target/deploy/spl_token_wrap.so"),
        upgrade_authority: Pubkey::default(),
    }]);

    test_validator_genesis.start_async().await
}

pub async fn setup_test_env() -> TestEnv {
    let (validator, payer) = start_validator().await;
    let rpc_client = Arc::new(validator.get_async_rpc_client());

    // Write payer keypair to a temporary file
    let keypair_file = NamedTempFile::new().unwrap();
    write_keypair_file(&payer, &keypair_file).unwrap();
    let keypair_file_path = keypair_file.path().to_str().unwrap().to_string();

    // Create and save CLI configuration file
    let config_file = NamedTempFile::new().unwrap();
    let config_file_path = config_file.path().to_str().unwrap().to_string();
    let solana_config = SolanaConfig {
        json_rpc_url: validator.rpc_url(),
        websocket_url: validator.rpc_pubsub_url(),
        keypair_path: keypair_file_path,
        ..SolanaConfig::default()
    };
    solana_config.save(&config_file_path).unwrap();

    TestEnv {
        payer,
        rpc_client,
        config_file_path,
        _keypair_file: keypair_file,
        _config_file: config_file,
        _validator: validator,
    }
}

pub async fn create_unwrapped_mint(
    rpc_client: &Arc<RpcClient>,
    payer: &Keypair,
    token_program_addr: &Pubkey,
) -> Pubkey {
    let mint_account = Keypair::new();
    let rent = rpc_client
        .get_minimum_balance_for_rent_exemption(SplTokenMint::LEN)
        .await
        .unwrap();

    let blockhash = rpc_client.get_latest_blockhash().await.unwrap();

    let transaction = Transaction::new_signed_with_payer(
        &[
            solana_system_interface::instruction::create_account(
                &payer.pubkey(),
                &mint_account.pubkey(),
                rent,
                SplTokenMint::LEN as u64,
                token_program_addr,
            ),
            initialize_mint(
                token_program_addr,
                &mint_account.pubkey(),
                &payer.pubkey(),
                None,
                9,
            )
            .unwrap(),
        ],
        Some(&payer.pubkey()),
        &[payer, &mint_account],
        blockhash,
    );

    rpc_client
        .send_and_confirm_transaction(&transaction)
        .await
        .unwrap();
    mint_account.pubkey()
}

pub struct CreateMintResult {
    pub unwrapped_token_program: Pubkey,
    pub wrapped_token_program: Pubkey,
    pub unwrapped_mint: Pubkey,
}

pub async fn execute_create_mint(env: &TestEnv) -> CreateMintResult {
    let unwrapped_token_program = spl_token::id();
    let wrapped_token_program = spl_token_2022::id();
    let unwrapped_mint =
        create_unwrapped_mint(&env.rpc_client, &env.payer, &unwrapped_token_program).await;

    let status = Command::new(TOKEN_WRAP_CLI_BIN)
        .args([
            "create-mint",
            "-C",
            &env.config_file_path,
            &unwrapped_mint.to_string(),
            &unwrapped_token_program.to_string(),
            &wrapped_token_program.to_string(),
            "--idempotent",
        ])
        .status()
        .unwrap();
    assert!(status.success());

    CreateMintResult {
        unwrapped_token_program,
        wrapped_token_program,
        unwrapped_mint,
    }
}

pub struct WrapResult {
    pub unwrapped_token_account: Keypair,
    pub recipient_token_account: Keypair,
    pub escrow_account: Keypair,
}

pub async fn execute_wrap(env: &TestEnv, create_mint_result: CreateMintResult) -> WrapResult {
    let CreateMintResult {
        unwrapped_token_program,
        wrapped_token_program,
        unwrapped_mint,
    } = create_mint_result;

    // Create unwrapped_token_account with 100 tokens
    let unwrapped_token_account = Keypair::new();
    let mint_authority = env.payer.pubkey();
    let recipient = Keypair::new();

    // Create token account for unwrapped tokens
    let unwrapped_account_size = spl_token::state::Account::LEN;
    let tx = Transaction::new_signed_with_payer(
        &[
            solana_system_interface::instruction::create_account(
                &env.payer.pubkey(),
                &unwrapped_token_account.pubkey(),
                env.rpc_client
                    .get_minimum_balance_for_rent_exemption(unwrapped_account_size)
                    .await
                    .unwrap(),
                unwrapped_account_size as u64,
                &unwrapped_token_program,
            ),
            spl_token::instruction::initialize_account(
                &unwrapped_token_program,
                &unwrapped_token_account.pubkey(),
                &unwrapped_mint,
                &env.payer.pubkey(),
            )
            .unwrap(),
            spl_token::instruction::mint_to(
                &unwrapped_token_program,
                &unwrapped_mint,
                &unwrapped_token_account.pubkey(),
                &mint_authority,
                &[&env.payer.pubkey()],
                100,
            )
            .unwrap(),
        ],
        Some(&env.payer.pubkey()),
        &[&env.payer, &unwrapped_token_account],
        env.rpc_client.get_latest_blockhash().await.unwrap(),
    );
    env.rpc_client
        .send_and_confirm_transaction(&tx)
        .await
        .unwrap();

    // Create recipient token account (with nothing in it)
    let wrapped_mint_address = get_wrapped_mint_address(&unwrapped_mint, &wrapped_token_program);
    let recipient_token_account = Keypair::new();
    let wrapped_account_size = spl_token_2022::state::Account::LEN;

    let tx = Transaction::new_signed_with_payer(
        &[
            solana_system_interface::instruction::create_account(
                &env.payer.pubkey(),
                &recipient_token_account.pubkey(),
                env.rpc_client
                    .get_minimum_balance_for_rent_exemption(wrapped_account_size)
                    .await
                    .unwrap(),
                wrapped_account_size as u64,
                &wrapped_token_program,
            ),
            spl_token_2022::instruction::initialize_account(
                &wrapped_token_program,
                &recipient_token_account.pubkey(),
                &wrapped_mint_address,
                &recipient.pubkey(),
            )
            .unwrap(),
        ],
        Some(&env.payer.pubkey()),
        &[&env.payer, &recipient_token_account],
        env.rpc_client.get_latest_blockhash().await.unwrap(),
    );
    env.rpc_client
        .send_and_confirm_transaction(&tx)
        .await
        .unwrap();

    // Create escrow token account with wrapped_mint_authority as owner
    let wrapped_mint_authority = get_wrapped_mint_authority(&wrapped_mint_address);
    let escrow_account = Keypair::new();

    let tx = Transaction::new_signed_with_payer(
        &[
            solana_system_interface::instruction::create_account(
                &env.payer.pubkey(),
                &escrow_account.pubkey(),
                env.rpc_client
                    .get_minimum_balance_for_rent_exemption(unwrapped_account_size)
                    .await
                    .unwrap(),
                unwrapped_account_size as u64,
                &unwrapped_token_program,
            ),
            spl_token::instruction::initialize_account(
                &unwrapped_token_program,
                &escrow_account.pubkey(),
                &unwrapped_mint,
                &wrapped_mint_authority,
            )
            .unwrap(),
        ],
        Some(&env.payer.pubkey()),
        &[&env.payer, &escrow_account],
        env.rpc_client.get_latest_blockhash().await.unwrap(),
    );
    env.rpc_client
        .send_and_confirm_transaction(&tx)
        .await
        .unwrap();

    let status = Command::new(TOKEN_WRAP_CLI_BIN)
        .args([
            "wrap",
            "-C",
            &env.config_file_path,
            &unwrapped_token_program.to_string(),
            &unwrapped_mint.to_string(),
            &unwrapped_token_account.pubkey().to_string(),
            &escrow_account.pubkey().to_string(),
            &wrapped_token_program.to_string(),
            &recipient_token_account.pubkey().to_string(),
            "100", // Amount to wrap
        ])
        .status()
        .unwrap();
    assert!(status.success());

    WrapResult {
        unwrapped_token_account,
        recipient_token_account,
        escrow_account,
    }
}
