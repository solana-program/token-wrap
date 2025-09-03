use {
    crate::{
        common::{parse_presigner, parse_pubkey, process_transaction},
        config::Config,
        output::{format_output, println_display},
        CommandResult,
    },
    clap::{value_parser, ArgMatches, Args},
    serde_derive::{Deserialize, Serialize},
    serde_with::{serde_as, DisplayFromStr},
    solana_cli_output::{
        display::writeln_name_value, return_signers_data, CliSignOnlyData, QuietDisplay,
        ReturnSignersConfig, VerboseDisplay,
    },
    solana_hash::Hash,
    solana_presigner::Presigner,
    solana_pubkey::Pubkey,
    solana_remote_wallet::remote_wallet::RemoteWalletManager,
    solana_signature::Signature,
    solana_signer::Signer,
    solana_transaction::Transaction,
    spl_token_wrap::{
        get_wrapped_mint_address, get_wrapped_mint_authority,
        instruction::sync_metadata_to_token_2022,
    },
    std::fmt::{Display, Formatter},
    std::rc::Rc,
    std::sync::Arc,
};

#[derive(Clone, Debug, Args)]
#[clap(
    about = "Sync metadata from the unwrapped mint to the wrapped Token-2022 mint's TokenMetadata extension"
)]
pub struct SyncMetadataToToken2022Args {
    /// The address of the unwrapped mint whose metadata will be synced from
    #[clap(value_parser = parse_pubkey)]
    pub unwrapped_mint: Pubkey,

    /// Optional source metadata account when the unwrapped mint's metadata pointer
    /// points to an external account (or when sourcing from a third-party program)
    #[clap(long, value_parser = parse_pubkey)]
    pub source_metadata: Option<Pubkey>,

    /// Optional owner program for the source metadata account, when owned by a third-party program
    #[clap(long, value_parser = parse_pubkey)]
    pub owner_program: Option<Pubkey>,

    #[clap(long, value_parser = value_parser!(Hash))]
    pub blockhash: Option<Hash>,

    /// Signatures to add to transaction (PUBKEY=SIGNATURE)
    #[clap(long, multiple = true, value_parser = parse_presigner, requires = "blockhash")]
    pub signer: Option<Vec<Presigner>>,

    /// Do not broadcast signed transaction, just sign
    #[clap(long)]
    pub sign_only: bool,
}

#[serde_as]
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncMetadataToToken2022Output {
    #[serde_as(as = "DisplayFromStr")]
    pub unwrapped_mint: Pubkey,

    #[serde_as(as = "DisplayFromStr")]
    pub wrapped_mint: Pubkey,

    #[serde_as(as = "DisplayFromStr")]
    pub wrapped_mint_authority: Pubkey,

    #[serde_as(as = "Option<DisplayFromStr>")]
    pub source_metadata: Option<Pubkey>,

    #[serde_as(as = "Option<DisplayFromStr>")]
    pub owner_program: Option<Pubkey>,

    pub signatures: Vec<Signature>,

    pub sign_only_data: Option<CliSignOnlyData>,
}

impl Display for SyncMetadataToToken2022Output {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        writeln_name_value(f, "Unwrapped mint:", &self.unwrapped_mint.to_string())?;
        writeln_name_value(f, "Wrapped mint:", &self.wrapped_mint.to_string())?;
        writeln_name_value(
            f,
            "Wrapped mint authority:",
            &self.wrapped_mint_authority.to_string(),
        )?;
        if let Some(src) = self.source_metadata {
            writeln_name_value(f, "Source metadata:", &src.to_string())?;
        }
        if let Some(owner) = self.owner_program {
            writeln_name_value(f, "Owner program:", &owner.to_string())?;
        }

        if let Some(data) = &self.sign_only_data {
            writeln!(f, "{}", data)?;
        } else {
            writeln!(f, "Signers:")?;
            for signature in &self.signatures {
                writeln!(f, "  {signature}")?;
            }
        }

        Ok(())
    }
}

impl QuietDisplay for SyncMetadataToToken2022Output {
    fn write_str(&self, _: &mut dyn std::fmt::Write) -> std::fmt::Result {
        Ok(())
    }
}
impl VerboseDisplay for SyncMetadataToToken2022Output {}

pub async fn command_sync_metadata_to_token2022(
    config: &Config,
    args: SyncMetadataToToken2022Args,
    _matches: &ArgMatches,
    _wallet_manager: &mut Option<Rc<RemoteWalletManager>>,
) -> CommandResult {
    let payer = config.fee_payer()?;

    // Derive destination wrapped mint and authority for Token-2022
    let wrapped_mint = get_wrapped_mint_address(&args.unwrapped_mint, &spl_token_2022::id());
    let wrapped_mint_authority = get_wrapped_mint_authority(&wrapped_mint);

    if !args.sign_only {
        println_display(
            config,
            format!(
                "Syncing metadata to Token-2022 mint {} from {}",
                wrapped_mint, args.unwrapped_mint
            ),
        );
    }

    let instruction = sync_metadata_to_token_2022(
        &spl_token_wrap::id(),
        &wrapped_mint,
        &wrapped_mint_authority,
        &args.unwrapped_mint,
        args.source_metadata.as_ref(),
        args.owner_program.as_ref(),
    );

    let blockhash = if let Some(hash) = args.blockhash {
        hash
    } else {
        config.rpc_client.get_latest_blockhash().await?
    };

    let mut signers: Vec<Arc<dyn Signer>> = vec![payer.clone()];
    if let Some(pre_signers) = &args.signer {
        for s in pre_signers {
            signers.push(Arc::from(s));
        }
    }

    let mut transaction = Transaction::new_with_payer(&[instruction], Some(&payer.pubkey()));
    transaction.partial_sign(&signers, blockhash);

    if !args.sign_only {
        process_transaction(config, transaction.clone()).await?;
    }

    let sign_only_data = args.sign_only.then(|| {
        return_signers_data(
            &transaction,
            &ReturnSignersConfig {
                dump_transaction_message: true,
            },
        )
    });

    let output = SyncMetadataToToken2022Output {
        unwrapped_mint: args.unwrapped_mint,
        wrapped_mint,
        wrapped_mint_authority,
        source_metadata: args.source_metadata,
        owner_program: args.owner_program,
        signatures: transaction.signatures,
        sign_only_data,
    };

    Ok(format_output(config, output))
}
