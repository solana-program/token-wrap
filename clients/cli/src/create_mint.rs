use {
    crate::{config::Config, CommandResult},
    clap::{ArgMatches, Args},
    solana_pubkey::Pubkey,
    solana_remote_wallet::remote_wallet::RemoteWalletManager,
    spl_token_client::spl_token_2022,
    spl_token_wrap::get_wrapped_mint_address,
    std::{rc::Rc, str::FromStr},
};

#[derive(Clone, Debug, Args)]
pub struct CreateMintArgs {
    /// The address of the SPL Token mint to wrap.
    #[clap(value_parser = parse_pubkey)]
    pub unwrapped_mint_address: Pubkey,

    /// The address of the token program that the unwrapped mint belongs to.
    #[clap(value_parser = parse_token_program)]
    pub unwrapped_token_program_id: Pubkey,

    /// The address of the token program that the wrapped mint should belong to.
    #[clap(value_parser = parse_token_program)]
    pub wrapped_token_program_id: Pubkey,

    /// Only perform checks, do not create account
    #[clap(long)]
    pub idempotent: bool,
}

fn parse_pubkey(value: &str) -> Result<Pubkey, String> {
    Pubkey::from_str(value).map_err(|e| format!("Invalid Pubkey: {e}"))
}

fn parse_token_program(value: &str) -> Result<Pubkey, String> {
    let pubkey = Pubkey::from_str(value).map_err(|e| format!("Invalid Pubkey: {e}"))?;
    if pubkey == spl_token::id() || pubkey == spl_token_2022::id() {
        Ok(pubkey)
    } else {
        Err("Invalid token program. Must be spl-token or spl-token-2022".to_string())
    }
}

pub async fn command_create_mint(
    config: &Config,
    command_config: CreateMintArgs,
    matches: &ArgMatches,
    wallet_manager: &mut Option<Rc<RemoteWalletManager>>,
) -> CommandResult {
    let payer = config.fee_payer()?;

    let wrapped_mint_address = get_wrapped_mint_address(
        &command_config.unwrapped_mint_address,
        &command_config.wrapped_token_program_id,
    );
    let wrapped_backpointer_address =
        spl_token_wrap::get_wrapped_mint_backpointer_address(&wrapped_mint_address);

    println!(
        "Creating wrapped mint for {} at {} with backpointer at {}",
        command_config.unwrapped_mint_address, wrapped_mint_address, wrapped_backpointer_address
    );

    Ok("Success".to_string())
}
