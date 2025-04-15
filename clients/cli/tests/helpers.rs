#![allow(dead_code)]

use {
    serde_json::Value,
    solana_cli_config::Config as SolanaConfig,
    solana_client::nonblocking::rpc_client::RpcClient,
    solana_keypair::{write_keypair_file, Keypair},
    solana_program_pack::Pack,
    solana_pubkey::Pubkey,
    solana_sdk_ids::bpf_loader_upgradeable,
    solana_signer::Signer,
    solana_system_interface::instruction::create_account,
    solana_test_validator::{TestValidator, TestValidatorGenesis, UpgradeableProgramInfo},
    solana_transaction::Transaction,
    spl_associated_token_account_client::address::get_associated_token_address_with_program_id,
    spl_token::{self, state::Mint as SplTokenMint},
    spl_token_2022::instruction::initialize_mint,
    std::{error::Error, path::PathBuf, process::Command, sync::Arc},
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

pub async fn create_unwrapped_mint(env: &TestEnv, token_program_addr: &Pubkey) -> Pubkey {
    let mint_account = Keypair::new();
    let rent = env
        .rpc_client
        .get_minimum_balance_for_rent_exemption(SplTokenMint::LEN)
        .await
        .unwrap();

    let blockhash = env.rpc_client.get_latest_blockhash().await.unwrap();

    let transaction = Transaction::new_signed_with_payer(
        &[
            solana_system_interface::instruction::create_account(
                &env.payer.pubkey(),
                &mint_account.pubkey(),
                rent,
                SplTokenMint::LEN as u64,
                token_program_addr,
            ),
            initialize_mint(
                token_program_addr,
                &mint_account.pubkey(),
                &env.payer.pubkey(),
                None,
                9,
            )
            .unwrap(),
        ],
        Some(&env.payer.pubkey()),
        &[env.payer.insecure_clone(), mint_account.insecure_clone()],
        blockhash,
    );

    env.rpc_client
        .send_and_confirm_transaction(&transaction)
        .await
        .unwrap();
    mint_account.pubkey()
}

pub async fn execute_create_mint(
    env: &TestEnv,
    unwrapped_mint: &Pubkey,
    unwrapped_token_program: &Pubkey,
    wrapped_token_program: &Pubkey,
) {
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
}

pub async fn create_associated_token_account(
    env: &TestEnv,
    token_program: &Pubkey,
    mint: &Pubkey,
    wallet_addr: &Pubkey,
) -> Pubkey {
    let ata = get_associated_token_address_with_program_id(wallet_addr, mint, token_program);

    let ata_account = env.rpc_client.get_account(&ata).await;
    if ata_account.is_ok() {
        return ata; // Return early if it exists
    }

    let instruction =
        spl_associated_token_account_client::instruction::create_associated_token_account(
            &env.payer.pubkey(),
            wallet_addr,
            mint,
            token_program,
        );

    let tx = Transaction::new_signed_with_payer(
        &[instruction],
        Some(&env.payer.pubkey()),
        &[&env.payer],
        env.rpc_client.get_latest_blockhash().await.unwrap(),
    );

    env.rpc_client
        .send_and_confirm_transaction(&tx)
        .await
        .unwrap();

    ata
}

pub async fn create_token_account(
    env: &TestEnv,
    token_program: &Pubkey,
    mint: &Pubkey,
    owner: &Pubkey,
) -> Pubkey {
    let token_account = Keypair::new();
    let account_size = spl_token::state::Account::LEN;

    let tx = Transaction::new_signed_with_payer(
        &[
            solana_system_interface::instruction::create_account(
                &env.payer.pubkey(),
                &token_account.pubkey(),
                env.rpc_client
                    .get_minimum_balance_for_rent_exemption(account_size)
                    .await
                    .unwrap(),
                account_size as u64,
                token_program,
            ),
            spl_token_2022::instruction::initialize_account(
                token_program,
                &token_account.pubkey(),
                mint,
                owner,
            )
            .unwrap(),
        ],
        Some(&env.payer.pubkey()),
        &[&env.payer, &token_account],
        env.rpc_client.get_latest_blockhash().await.unwrap(),
    );
    env.rpc_client
        .send_and_confirm_transaction(&tx)
        .await
        .unwrap();

    token_account.pubkey()
}

pub async fn mint_to(
    env: &TestEnv,
    token_program: &Pubkey,
    mint: &Pubkey,
    token_account: &Pubkey,
    amount: u64,
) {
    let tx = Transaction::new_signed_with_payer(
        &[spl_token::instruction::mint_to(
            token_program,
            mint,
            token_account,
            &env.payer.pubkey(),
            &[&env.payer.pubkey()],
            amount,
        )
        .unwrap()],
        Some(&env.payer.pubkey()),
        &[&env.payer],
        env.rpc_client.get_latest_blockhash().await.unwrap(),
    );
    env.rpc_client
        .send_and_confirm_transaction(&tx)
        .await
        .unwrap();
}

// Creates 2 of 3 multisig
pub async fn create_test_multisig(
    env: &mut TestEnv,
    token_program: &Pubkey,
) -> Result<(Pubkey, Vec<Keypair>), Box<dyn Error>> {
    let multisig_keypair = Keypair::new();
    let multisig_pubkey = multisig_keypair.pubkey();
    let multisig_member1 = Keypair::new();
    let multisig_member2 = Keypair::new();
    let multisig_member3 = Keypair::new();
    let multisig_member_pubkeys = [
        &multisig_member1.pubkey(),
        &multisig_member2.pubkey(),
        &multisig_member3.pubkey(),
    ];

    let rent = env
        .rpc_client
        .get_minimum_balance_for_rent_exemption(spl_token::state::Multisig::LEN)
        .await?;

    let create_account_instruction = create_account(
        &env.payer.pubkey(),
        &multisig_pubkey,
        rent,
        spl_token::state::Multisig::LEN as u64,
        token_program,
    );

    let initialize_multisig_instruction = spl_token_2022::instruction::initialize_multisig(
        token_program,
        &multisig_pubkey,
        &multisig_member_pubkeys,
        2,
    )?;
    let mut transaction = Transaction::new_with_payer(
        &[create_account_instruction, initialize_multisig_instruction],
        Some(&env.payer.pubkey()),
    );
    let recent_blockhash = env.rpc_client.get_latest_blockhash().await?;
    transaction.sign(&[&env.payer, &multisig_keypair], recent_blockhash);

    env.rpc_client
        .send_and_confirm_transaction(&transaction)
        .await?;

    Ok((
        multisig_pubkey,
        vec![multisig_member1, multisig_member2, multisig_member3],
    ))
}

// Used to get signers vec from CLI output
pub fn extract_signers(std_out: &[u8]) -> Vec<String> {
    let json_output1 = String::from_utf8(std_out.to_vec()).unwrap();
    let parsed_value: Value = serde_json::from_str(&json_output1).unwrap();
    parsed_value
        .get("signOnlyData")
        .unwrap()
        .get("signers")
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect()
}
