use {
    crate::{config::Config, output::println_display, Error},
    clap::ArgMatches,
    solana_clap_v3_utils::keypair::pubkey_from_path,
    solana_pubkey::Pubkey,
    solana_signature::Signature,
    solana_transaction::Transaction,
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
