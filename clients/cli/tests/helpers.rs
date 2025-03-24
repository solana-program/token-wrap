use {
    solana_account::{Account, AccountSharedData},
    solana_cli_config::Config as SolanaConfig,
    solana_client::nonblocking::rpc_client::RpcClient,
    solana_keypair::{write_keypair_file, Keypair},
    solana_program_pack::Pack,
    solana_pubkey::Pubkey,
    solana_sdk_ids::{bpf_loader_upgradeable, system_program},
    solana_signer::Signer,
    solana_test_validator::{TestValidator, TestValidatorGenesis, UpgradeableProgramInfo},
    spl_token::{
        self,
        solana_program::{program_option::COption, rent::Rent},
    },
    spl_token_wrap::{self},
    std::{path::PathBuf, sync::Arc},
    tempfile::NamedTempFile,
};

pub const TOKEN_WRAP_CLI_BIN: &str = "../../target/debug/spl-token-wrap";
const UNWRAPPED_TOKEN_PROGRAM: Pubkey = spl_token::id();
const WRAPPED_TOKEN_PROGRAM: Pubkey = spl_token_2022::id();

pub async fn start_validator(starting_accounts: Vec<(Pubkey, AccountSharedData)>) -> TestValidator {
    solana_logger::setup();
    let mut test_validator_genesis = TestValidatorGenesis::default();

    test_validator_genesis.add_upgradeable_programs_with_path(&[UpgradeableProgramInfo {
        program_id: spl_token_wrap::id(),
        loader: bpf_loader_upgradeable::id(),
        program_path: PathBuf::from("../../target/deploy/spl_token_wrap.so"),
        upgrade_authority: Pubkey::default(),
    }]);

    test_validator_genesis.add_accounts(starting_accounts);

    test_validator_genesis.start_async().await.0
}

pub struct Env {
    pub rpc_client: Arc<RpcClient>,
    pub config_file_path: String,
    pub unwrapped_mint: Pubkey,
    pub unwrapped_token_program: Pubkey,
    pub wrapped_token_program: Pubkey,
    // Persist these to keep them in scope
    _validator: TestValidator,
    _keypair_file: NamedTempFile,
    _config_file: NamedTempFile,
}

pub async fn setup() -> Env {
    // Setup starting accounts
    let payer = Keypair::new();
    let unwrapped_mint = setup_mint(&payer.pubkey());
    let payer_account = AccountSharedData::new(1_000_000_000, 0, &system_program::id());
    let starting_accounts = vec![(payer.pubkey(), payer_account), unwrapped_mint.clone()];

    // Start the test validator with necessary programs
    let validator = start_validator(starting_accounts).await;

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
        rpc_client: Arc::new(validator.get_async_rpc_client()),
        config_file_path,
        unwrapped_mint: unwrapped_mint.0,
        unwrapped_token_program: UNWRAPPED_TOKEN_PROGRAM,
        wrapped_token_program: WRAPPED_TOKEN_PROGRAM,
        _validator: validator,
        _keypair_file: keypair_file,
        _config_file: config_file,
    }
}

pub fn setup_mint(mint_authority: &Pubkey) -> (Pubkey, AccountSharedData) {
    let state = spl_token::state::Mint {
        decimals: 8,
        is_initialized: true,
        supply: 1_000_000_000,
        mint_authority: COption::Some(*mint_authority),
        freeze_authority: COption::None,
    };
    let mut data = vec![0u8; spl_token::state::Mint::LEN];
    state.pack_into_slice(&mut data);

    let lamports = Rent::default().minimum_balance(data.len());

    let account = Account {
        lamports,
        data,
        owner: UNWRAPPED_TOKEN_PROGRAM,
        ..Default::default()
    };
    (Pubkey::new_unique(), account.into())
}
