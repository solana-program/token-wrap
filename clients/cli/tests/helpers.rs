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
    spl_token::{self, instruction::initialize_mint, state::Mint as SplTokenMin},
    spl_token_client::client::{ProgramClient, ProgramRpcClient, ProgramRpcClientSendTransaction},
    spl_token_wrap::{self},
    std::{path::PathBuf, sync::Arc},
    tempfile::NamedTempFile,
};

pub const TOKEN_WRAP_CLI_BIN: &str = "../../target/debug/spl-token-wrap";

pub type PClient = Arc<dyn ProgramClient<ProgramRpcClientSendTransaction>>;

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

pub struct Env {
    pub rpc_client: Arc<RpcClient>,
    pub program_client: PClient,
    pub payer: Keypair,
    pub config_file_path: String,
    // Persist these to keep them in scope
    _validator: TestValidator,
    _keypair_file: NamedTempFile,
    _config_file: NamedTempFile,
}

pub async fn setup() -> Env {
    // Start the test validator with necessary programs
    let (validator, payer) = start_validator().await;

    // Create RPC and program clients
    let rpc_client = Arc::new(validator.get_async_rpc_client());
    let program_client = Arc::new(ProgramRpcClient::new(
        rpc_client.clone(),
        ProgramRpcClientSendTransaction,
    ));

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

    Env {
        rpc_client,
        program_client,
        payer,
        config_file_path,
        _validator: validator,
        _keypair_file: keypair_file,
        _config_file: config_file,
    }
}

pub async fn create_unwrapped_mint(
    program_client: PClient,
    payer: &Keypair,
    token_program_addr: &Pubkey,
) -> Pubkey {
    let mint_account = Keypair::new();
    let rent = program_client
        .get_minimum_balance_for_rent_exemption(SplTokenMin::LEN)
        .await
        .unwrap();

    let blockhash = program_client.get_latest_blockhash().await.unwrap();

    let transaction = Transaction::new_signed_with_payer(
        &[
            solana_system_interface::instruction::create_account(
                &payer.pubkey(),
                &mint_account.pubkey(),
                rent,
                SplTokenMin::LEN as u64,
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

    program_client.send_transaction(&transaction).await.unwrap();
    mint_account.pubkey()
}
