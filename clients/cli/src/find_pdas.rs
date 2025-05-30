use {
    crate::{
        common::{get_account_owner, parse_pubkey, parse_token_program},
        config::Config,
        output::format_output,
        CommandResult,
    },
    clap::Args,
    serde_derive::{Deserialize, Serialize},
    serde_with::{serde_as, DisplayFromStr},
    solana_cli_output::{display::writeln_name_value, QuietDisplay, VerboseDisplay},
    solana_pubkey::Pubkey,
    spl_token_wrap::{
        get_escrow_address, get_wrapped_mint_address, get_wrapped_mint_authority,
        get_wrapped_mint_backpointer_address,
    },
    std::fmt::{Display, Formatter},
};

#[derive(Clone, Debug, Args)]
pub struct FindPdasArgs {
    /// The address of the mint to wrap
    #[clap(value_parser = parse_pubkey)]
    pub unwrapped_mint: Pubkey,

    /// The address of the token program that the wrapped mint should belong to
    #[clap(value_parser = parse_token_program)]
    pub wrapped_token_program: Pubkey,
}

#[serde_as]
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PdasOutput {
    #[serde_as(as = "DisplayFromStr")]
    pub wrapped_mint_address: Pubkey,
    #[serde_as(as = "DisplayFromStr")]
    pub wrapped_mint_authority: Pubkey,
    #[serde_as(as = "DisplayFromStr")]
    pub wrapped_backpointer_address: Pubkey,
    #[serde_as(as = "DisplayFromStr")]
    pub unwrapped_escrow: Pubkey,
}

impl Display for PdasOutput {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        writeln_name_value(
            f,
            "Wrapped mint address:",
            &self.wrapped_mint_address.to_string(),
        )?;
        writeln_name_value(
            f,
            "Wrapped mint authority:",
            &self.wrapped_mint_authority.to_string(),
        )?;
        writeln_name_value(
            f,
            "Wrapped backpointer address:",
            &self.wrapped_backpointer_address.to_string(),
        )?;
        writeln_name_value(
            f,
            "Unwrapped escrow address:",
            &self.unwrapped_escrow.to_string(),
        )?;

        Ok(())
    }
}

impl QuietDisplay for PdasOutput {
    fn write_str(&self, _: &mut dyn std::fmt::Write) -> std::fmt::Result {
        Ok(())
    }
}
impl VerboseDisplay for PdasOutput {}

pub async fn command_get_pdas(config: &Config, args: FindPdasArgs) -> CommandResult {
    let wrapped_mint_address =
        get_wrapped_mint_address(&args.unwrapped_mint, &args.wrapped_token_program);
    let wrapped_backpointer_address = get_wrapped_mint_backpointer_address(&wrapped_mint_address);
    let wrapped_mint_authority = get_wrapped_mint_authority(&wrapped_mint_address);

    let unwrapped_token_program =
        get_account_owner(&config.rpc_client, &args.unwrapped_mint).await?;

    let unwrapped_escrow = get_escrow_address(
        &args.unwrapped_mint,
        &unwrapped_token_program,
        &args.wrapped_token_program,
    );

    Ok(format_output(
        config,
        PdasOutput {
            wrapped_mint_address,
            wrapped_mint_authority,
            wrapped_backpointer_address,
            unwrapped_escrow,
        },
    ))
}
