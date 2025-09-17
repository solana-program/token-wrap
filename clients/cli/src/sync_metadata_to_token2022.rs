use {
    crate::{
        common::{parse_pubkey, process_transaction},
        config::Config,
        output::{format_output, println_display},
        CommandResult, Error,
    },
    clap::{ArgMatches, Args},
    mpl_token_metadata::accounts::Metadata as MetaplexMetadata,
    serde_derive::{Deserialize, Serialize},
    serde_with::{serde_as, DisplayFromStr},
    solana_cli_output::{display::writeln_name_value, QuietDisplay, VerboseDisplay},
    solana_client::nonblocking::rpc_client::RpcClient,
    solana_pubkey::Pubkey,
    solana_remote_wallet::remote_wallet::RemoteWalletManager,
    solana_signature::Signature,
    solana_signer::Signer,
    solana_transaction::Transaction,
    spl_token_2022::{
        extension::{
            metadata_pointer::MetadataPointer, BaseStateWithExtensions, PodStateWithExtensions,
        },
        pod::PodMint,
    },
    spl_token_wrap::{
        get_wrapped_mint_address, get_wrapped_mint_authority,
        instruction::sync_metadata_to_token_2022,
    },
    std::{
        fmt::{Display, Formatter},
        rc::Rc,
    },
};

#[derive(Clone, Debug, Args)]
pub struct SyncMetadataToToken2022Args {
    /// The address of the unwrapped mint whose metadata will be synced from
    #[clap(value_parser = parse_pubkey)]
    pub unwrapped_mint: Pubkey,

    /// Optional source metadata account. If not provided, it will be derived
    /// automatically. For SPL Token mints, this will be the `Metaplex`
    /// Metadata PDA. For Token-2022 mints, the metadata pointer extension
    /// is checked first, falling back to the `Metaplex` PDA if the pointer is
    /// not set.
    #[clap(long, value_parser = parse_pubkey)]
    pub metadata_account: Option<Pubkey>,

    /// Optional owner program for the source metadata account, when owned by a
    /// third-party program
    #[clap(long, value_parser = parse_pubkey, requires = "metadata-account")]
    pub metadata_program_id: Option<Pubkey>,
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
    pub metadata_program_id: Option<Pubkey>,

    pub signatures: Vec<Signature>,
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
        if let Some(id) = self.metadata_program_id {
            writeln_name_value(f, "Metadata program id:", &id.to_string())?;
        }

        writeln!(f, "Signers:")?;
        for signature in &self.signatures {
            writeln!(f, "  {signature}")?;
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

    let wrapped_mint = get_wrapped_mint_address(&args.unwrapped_mint, &spl_token_2022::id());
    let wrapped_mint_authority = get_wrapped_mint_authority(&wrapped_mint);

    let source_metadata = if let Some(metadata_account) = args.metadata_account {
        Some(metadata_account)
    } else {
        resolve_source_metadata_account(&config.rpc_client, &args.unwrapped_mint).await?
    };

    println_display(
        config,
        format!(
            "Syncing metadata to Token-2022 mint {} from {}",
            wrapped_mint, args.unwrapped_mint
        ),
    );

    let instruction = sync_metadata_to_token_2022(
        &spl_token_wrap::id(),
        &wrapped_mint,
        &wrapped_mint_authority,
        &args.unwrapped_mint,
        source_metadata.as_ref(),
        args.metadata_program_id.as_ref(),
    );

    let blockhash = config.rpc_client.get_latest_blockhash().await?;

    let mut transaction = Transaction::new_with_payer(&[instruction], Some(&payer.pubkey()));
    transaction.partial_sign(&[payer.clone()], blockhash);

    process_transaction(config, transaction.clone()).await?;

    let output = SyncMetadataToToken2022Output {
        unwrapped_mint: args.unwrapped_mint,
        wrapped_mint,
        wrapped_mint_authority,
        source_metadata,
        metadata_program_id: args.metadata_program_id,
        signatures: transaction.signatures,
    };

    Ok(format_output(config, output))
}

pub async fn resolve_source_metadata_account(
    rpc_client: &RpcClient,
    unwrapped_mint: &Pubkey,
) -> Result<Option<Pubkey>, Error> {
    let acct = rpc_client.get_account(unwrapped_mint).await?;
    let owner = acct.owner;

    let metaplex_pda = Some(MetaplexMetadata::find_pda(unwrapped_mint).0);

    if owner == spl_token::id() {
        return Ok(metaplex_pda);
    }

    if owner == spl_token_2022::id() {
        let mint_state = PodStateWithExtensions::<PodMint>::unpack(&acct.data)?;

        let resolved = match mint_state.get_extension::<MetadataPointer>() {
            Ok(pointer) => match Option::from(pointer.metadata_address) {
                Some(addr) if addr == *unwrapped_mint => None,
                Some(addr) => Some(addr),
                None => metaplex_pda, // unset pointer → fallback
            },
            Err(_) => metaplex_pda, // no extension → fallback
        };

        return Ok(resolved);
    }

    Err(format!(
        "Unwrapped mint {} is not an SPL Token or SPL Token-2022 mint",
        unwrapped_mint
    )
    .into())
}
