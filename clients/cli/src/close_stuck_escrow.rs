use {
    crate::{
        common::{get_account_owner, parse_pubkey, parse_token_program, process_transaction},
        config::Config,
        output::{format_output, println_display},
        CommandResult,
    },
    clap::Args,
    serde_derive::{Deserialize, Serialize},
    serde_with::{serde_as, DisplayFromStr},
    solana_cli_output::{display::writeln_name_value, QuietDisplay, VerboseDisplay},
    solana_pubkey::Pubkey,
    solana_signature::Signature,
    solana_signer::Signer,
    solana_transaction::Transaction,
    spl_token_wrap::{
        get_escrow_address, get_wrapped_mint_address, get_wrapped_mint_authority,
        instruction::close_stuck_escrow,
    },
    std::fmt::{Display, Formatter},
};

#[derive(Clone, Debug, Args)]
pub struct CloseStuckEscrowArgs {
    /// The address of the unwrapped mint
    #[clap(value_parser = parse_pubkey)]
    pub unwrapped_mint: Pubkey,

    /// The address of the account to send lamports to
    #[clap(value_parser = parse_pubkey)]
    pub destination: Pubkey,

    /// The address of the token program for the wrapped mint
    #[clap(value_parser = parse_token_program)]
    pub wrapped_token_program: Pubkey,
}

#[serde_as]
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloseStuckEscrowOutput {
    #[serde_as(as = "DisplayFromStr")]
    pub unwrapped_mint: Pubkey,

    #[serde_as(as = "DisplayFromStr")]
    pub escrow_account: Pubkey,

    pub signatures: Vec<Signature>,
}

impl Display for CloseStuckEscrowOutput {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        writeln_name_value(f, "Unwrapped mint:", &self.unwrapped_mint.to_string())?;
        writeln_name_value(f, "Escrow account:", &self.escrow_account.to_string())?;

        writeln!(f, "Signers:")?;
        for signature in &self.signatures {
            writeln!(f, "  {signature}")?;
        }

        Ok(())
    }
}

impl QuietDisplay for CloseStuckEscrowOutput {
    fn write_str(&self, _: &mut dyn std::fmt::Write) -> std::fmt::Result {
        Ok(())
    }
}
impl VerboseDisplay for CloseStuckEscrowOutput {}

pub async fn command_close_stuck_escrow(
    config: &Config,
    args: CloseStuckEscrowArgs,
) -> CommandResult {
    let unwrapped_token_program =
        get_account_owner(&config.rpc_client, &args.unwrapped_mint).await?;

    // CloseStuckEscrow only works with spl-token-2022 unwrapped mints due to
    // extension requirements
    if unwrapped_token_program != spl_token_2022::id() {
        return Err(format!(
            "CloseStuckEscrow only works with spl-token-2022 unwrapped mints. Unwrapped mint {} \
             uses program {}",
            args.unwrapped_mint, unwrapped_token_program
        )
        .into());
    }

    let wrapped_mint = get_wrapped_mint_address(&args.unwrapped_mint, &args.wrapped_token_program);
    let wrapped_mint_authority = get_wrapped_mint_authority(&wrapped_mint);
    let escrow_account = get_escrow_address(
        &args.unwrapped_mint,
        &unwrapped_token_program,
        &args.wrapped_token_program,
    );

    if !config.dry_run {
        println_display(
            config,
            format!(
                "Closing stuck escrow account {} for unwrapped mint {}",
                escrow_account, args.unwrapped_mint,
            ),
        );
    }

    let instruction = close_stuck_escrow(
        &spl_token_wrap::id(),
        &escrow_account,
        &args.destination,
        &args.unwrapped_mint,
        &wrapped_mint,
        &wrapped_mint_authority,
    );

    let latest_blockhash = config.rpc_client.get_latest_blockhash().await?;
    let payer = config.fee_payer()?;

    let transaction = Transaction::new_signed_with_payer(
        &[instruction],
        Some(&payer.pubkey()),
        &[&*payer],
        latest_blockhash,
    );

    process_transaction(config, transaction.clone()).await?;

    let output = CloseStuckEscrowOutput {
        unwrapped_mint: args.unwrapped_mint,
        escrow_account,
        signatures: transaction.signatures,
    };

    Ok(format_output(config, output))
}
