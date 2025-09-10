use {
    crate::{
        common::{parse_pubkey, process_transaction},
        config::Config,
        output::{format_output, println_display},
        CommandResult,
    },
    clap::{ArgMatches, Args},
    mpl_token_metadata::accounts::Metadata as MetaplexMetadata,
    serde_derive::{Deserialize, Serialize},
    serde_with::{serde_as, DisplayFromStr},
    solana_cli_output::{display::writeln_name_value, QuietDisplay, VerboseDisplay},
    solana_pubkey::Pubkey,
    solana_remote_wallet::remote_wallet::RemoteWalletManager,
    solana_signature::Signature,
    solana_signer::Signer,
    solana_transaction::Transaction,
    spl_token_wrap::{
        get_wrapped_mint_address, get_wrapped_mint_authority,
        instruction::sync_metadata_to_spl_token,
    },
    std::{
        fmt::{Display, Formatter},
        rc::Rc,
    },
};

#[derive(Clone, Debug, Args)]
pub struct SyncMetadataToSplTokenArgs {
    /// The address of the unwrapped mint whose metadata will be synced from
    #[clap(value_parser = parse_pubkey)]
    pub unwrapped_mint: Pubkey,

    /// Optional source metadata account when the unwrapped mint's metadata
    /// pointer points to an external account or third-party program
    #[clap(long, value_parser = parse_pubkey)]
    pub source_metadata: Option<Pubkey>,

    /// Optional owner program for the source metadata account, when owned by a
    /// third-party program
    #[clap(long, value_parser = parse_pubkey)]
    pub owner_program: Option<Pubkey>,
}

#[serde_as]
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncMetadataToSplTokenOutput {
    #[serde_as(as = "DisplayFromStr")]
    pub unwrapped_mint: Pubkey,

    #[serde_as(as = "DisplayFromStr")]
    pub wrapped_mint: Pubkey,

    #[serde_as(as = "DisplayFromStr")]
    pub wrapped_mint_authority: Pubkey,

    #[serde_as(as = "DisplayFromStr")]
    pub metaplex_metadata: Pubkey,

    #[serde_as(as = "Option<DisplayFromStr>")]
    pub source_metadata: Option<Pubkey>,

    #[serde_as(as = "Option<DisplayFromStr>")]
    pub owner_program: Option<Pubkey>,

    #[serde_as(as = "Option<DisplayFromStr>")]
    pub signature: Option<Signature>,
}

impl Display for SyncMetadataToSplTokenOutput {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        writeln_name_value(f, "Unwrapped mint:", &self.unwrapped_mint.to_string())?;
        writeln_name_value(f, "Wrapped mint:", &self.wrapped_mint.to_string())?;
        writeln_name_value(
            f,
            "Wrapped mint authority:",
            &self.wrapped_mint_authority.to_string(),
        )?;
        writeln_name_value(
            f,
            "Metaplex metadata account:",
            &self.metaplex_metadata.to_string(),
        )?;
        if let Some(src) = self.source_metadata {
            writeln_name_value(f, "Source metadata:", &src.to_string())?;
        }
        if let Some(owner) = self.owner_program {
            writeln_name_value(f, "Owner program:", &owner.to_string())?;
        }
        if let Some(signature) = self.signature {
            writeln_name_value(f, "Signature:", &signature.to_string())?;
        }
        Ok(())
    }
}

impl QuietDisplay for SyncMetadataToSplTokenOutput {
    fn write_str(&self, _: &mut dyn std::fmt::Write) -> std::fmt::Result {
        Ok(())
    }
}
impl VerboseDisplay for SyncMetadataToSplTokenOutput {}

pub async fn command_sync_metadata_to_spl_token(
    config: &Config,
    args: SyncMetadataToSplTokenArgs,
    _matches: &ArgMatches,
    _wallet_manager: &mut Option<Rc<RemoteWalletManager>>,
) -> CommandResult {
    let payer = config.fee_payer()?;
    let wrapped_token_program = spl_token::id();

    let wrapped_mint = get_wrapped_mint_address(&args.unwrapped_mint, &wrapped_token_program);
    let wrapped_mint_authority = get_wrapped_mint_authority(&wrapped_mint);
    let (metaplex_metadata, _) = MetaplexMetadata::find_pda(&wrapped_mint);

    println_display(
        config,
        format!(
            "Syncing metadata to SPL Token mint {} from {}",
            wrapped_mint, args.unwrapped_mint
        ),
    );

    let instruction = sync_metadata_to_spl_token(
        &spl_token_wrap::id(),
        &metaplex_metadata,
        &wrapped_mint_authority,
        &wrapped_mint,
        &args.unwrapped_mint,
        args.source_metadata.as_ref(),
        args.owner_program.as_ref(),
    );

    let blockhash = config.rpc_client.get_latest_blockhash().await?;

    let mut transaction = Transaction::new_with_payer(&[instruction], Some(&payer.pubkey()));
    transaction.partial_sign(&[payer.clone()], blockhash);

    let signature = process_transaction(config, transaction).await?;

    let output = SyncMetadataToSplTokenOutput {
        unwrapped_mint: args.unwrapped_mint,
        wrapped_mint,
        wrapped_mint_authority,
        metaplex_metadata,
        source_metadata: args.source_metadata,
        owner_program: args.owner_program,
        signature,
    };

    Ok(format_output(config, output))
}
