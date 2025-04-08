use {
    crate::{config::Config, output::println_display, Error},
    clap::ArgMatches,
    solana_clap_v3_utils::keypair::pubkey_from_path,
    solana_client::nonblocking::rpc_client::RpcClient,
    solana_presigner::Presigner,
    solana_program_pack::Pack,
    solana_pubkey::Pubkey,
    solana_signature::Signature,
    solana_transaction::Transaction,
    spl_token_2022::{extension::PodStateWithExtensions, pod::PodAccount},
    std::str::FromStr,
};

pub fn parse_pubkey(value: &str) -> Result<Pubkey, String> {
    parse_address(value, "pubkey")
}

fn parse_address(path: &str, name: &str) -> Result<Pubkey, String> {
    let mut wallet_manager = None;
    pubkey_from_path(&ArgMatches::default(), path, name, &mut wallet_manager)
        .map_err(|_| format!("Failed to load pubkey {} at {}", name, path))
}

pub fn parse_token_program(value: &str) -> Result<Pubkey, String> {
    let pubkey = parse_pubkey(value)?;
    if pubkey == spl_token::id() || pubkey == spl_token_2022::id() {
        Ok(pubkey)
    } else {
        Err("Invalid token program. Must be spl-token or spl-token-2022".to_string())
    }
}

pub fn parse_presigner(value: &str) -> Result<Presigner, String> {
    let (pubkey_string, sig_string) = value
        .split_once('=')
        .ok_or("failed to split `pubkey=signature` pair")?;
    let pubkey = Pubkey::from_str(pubkey_string)
        .map_err(|_| "Failed to parse pubkey from string".to_string())?;
    let sig = Signature::from_str(sig_string)
        .map_err(|_| "Failed to parse signature from string".to_string())?;
    Ok(Presigner::new(&pubkey, &sig))
}

pub async fn process_transaction(
    config: &Config,
    transaction: Transaction,
) -> Result<Option<Signature>, Error> {
    if config.dry_run {
        let simulation_data = config.rpc_client.simulate_transaction(&transaction).await?;

        if config.verbose() {
            if let Some(logs) = simulation_data.value.logs {
                for log in logs {
                    println!("    {}", log);
                }
            }

            println!(
                "\nSimulation succeeded, consumed {} compute units",
                simulation_data.value.units_consumed.unwrap()
            );
        } else {
            println_display(config, "Simulation succeeded".to_string());
        }

        Ok(None)
    } else {
        Ok(Some(
            config
                .rpc_client
                .send_and_confirm_transaction_with_spinner(&transaction)
                .await?,
        ))
    }
}

pub async fn get_mint_for_token_account(
    rpc_client: &RpcClient,
    token_account_address: &Pubkey,
) -> Result<Pubkey, Error> {
    let token_account_info = rpc_client.get_account(token_account_address).await?;
    let unpacked_account = PodStateWithExtensions::<PodAccount>::unpack(&token_account_info.data)?;
    Ok(unpacked_account.base.mint)
}

pub async fn get_account_owner(rpc_client: &RpcClient, account: &Pubkey) -> Result<Pubkey, Error> {
    let owner = rpc_client.get_account(account).await?.owner;
    Ok(owner)
}

pub async fn assert_mint_account(
    rpc_client: &RpcClient,
    account_key: &Pubkey,
) -> Result<(), String> {
    let account_info = rpc_client
        .get_account(account_key)
        .await
        .map_err(|e| format!("Failed to fetch account {}: {}", account_key, e))?;

    let owner = account_info.owner;
    if owner != spl_token::id() && owner != spl_token_2022::id() {
        return Err(format!(
            "Account {} is not owned by a token program. Owner: {}",
            account_key, owner
        ));
    }

    // Attempt to deserialize the data as a mint account
    let _ = Mint::unpack(&account_info.data)
        .map_err(|e| format!("Failed to unpack as spl token mint: {:?}", e))?;

    Ok(())
}
