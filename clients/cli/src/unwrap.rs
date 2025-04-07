use {
    crate::{
        common::{
            get_account_owner, get_mint_for_token_account, parse_presigner, parse_pubkey,
            parse_token_program, process_transaction,
        },
        config::Config,
        output::{format_output, println_display},
        CommandResult, Error,
    },
    clap::{value_parser, ArgMatches, Args},
    serde_derive::{Deserialize, Serialize},
    serde_with::{serde_as, DisplayFromStr},
    solana_clap_v3_utils::{
        input_parsers::signer::{SignerSource, SignerSourceParserBuilder},
        keypair::{signer_from_source_with_config, SignerFromPathConfig},
    },
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
    spl_associated_token_account_client::address::get_associated_token_address_with_program_id,
    spl_token_wrap::{get_wrapped_mint_address, get_wrapped_mint_authority, instruction::unwrap},
    std::{
        fmt::{Display, Formatter},
        rc::Rc,
        sync::Arc,
    },
};

#[derive(Clone, Debug, Args)]
pub struct UnwrapArgs {
    /// The address of the wrapped token account to unwrap from
    #[clap(value_parser = parse_pubkey)]
    pub wrapped_token_account: Pubkey,

    /// The address of the token account to receive the unwrapped tokens
    #[clap(value_parser = parse_pubkey)]
    pub unwrapped_token_recipient: Pubkey,

    /// The amount of tokens to unwrap
    #[clap(value_parser)]
    pub amount: u64,

    /// The address of the escrow account holding the unwrapped tokens.
    /// If not provided, defaults to the Associated Token Account (`ATA`) for
    /// the wrapped mint authority PDA on the unwrapped mint.
    #[clap(long, value_parser = parse_pubkey)]
    pub escrow_account: Option<Pubkey>,

    /// Signer source of transfer authority (to burn wrapped tokens)
    /// if different from fee payer
    #[clap(
        long,
        value_parser = SignerSourceParserBuilder::default().allow_all().build()
    )]
    pub transfer_authority: Option<SignerSource>,

    /// The address of the unwrapped mint, queried if not provided
    #[clap(long, value_parser = parse_pubkey)]
    pub unwrapped_mint: Option<Pubkey>,

    /// The address of the token program for the wrapped mint,
    /// queried if not provided.
    #[clap(long, value_parser = parse_token_program)]
    pub wrapped_token_program: Option<Pubkey>,

    /// The address of the token program for the unwrapped mint,
    /// queried if not provided.
    #[clap(long, value_parser = parse_token_program)]
    pub unwrapped_token_program: Option<Pubkey>,

    /// Member signer of a multisig account.
    /// Use this argument multiple times for each signer.
    #[clap(
        long,
        multiple = true,
        value_parser = SignerSourceParserBuilder::default().allow_all().build(),
        requires = "blockhash"
    )]
    pub multisig_signer: Option<Vec<SignerSource>>,

    #[clap(long, value_parser = value_parser!(Hash))]
    pub blockhash: Option<Hash>,

    /// Signatures to add to transaction.
    /// Often the `PUBKEY=SIGNATURE` output from a multisig --sign-only signer.
    #[clap(
        long,
        multiple = true,
        value_parser = parse_presigner,
        requires = "blockhash"
    )]
    pub signer: Option<Vec<Presigner>>,

    /// Do not broadcast signed transaction, just sign
    #[clap(long)]
    pub sign_only: bool,
}

#[serde_as]
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnwrapOutput {
    #[serde_as(as = "DisplayFromStr")]
    pub unwrapped_mint_address: Pubkey,

    #[serde_as(as = "DisplayFromStr")]
    pub unwrapped_token_program: Pubkey,

    #[serde_as(as = "DisplayFromStr")]
    pub wrapped_mint_address: Pubkey,

    #[serde_as(as = "DisplayFromStr")]
    pub wrapped_token_account: Pubkey,

    #[serde_as(as = "DisplayFromStr")]
    pub escrow_account: Pubkey,

    #[serde_as(as = "DisplayFromStr")]
    pub recipient_token_account: Pubkey,

    pub amount: u64,

    pub signatures: Vec<Signature>,

    pub sign_only_data: Option<CliSignOnlyData>,
}

impl Display for UnwrapOutput {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        writeln_name_value(
            f,
            "Unwrapped token program:",
            &self.unwrapped_token_program.to_string(),
        )?;
        writeln_name_value(
            f,
            "Unwrapped mint address:",
            &self.unwrapped_mint_address.to_string(),
        )?;
        writeln_name_value(
            f,
            "Wrapped mint address:",
            &self.wrapped_mint_address.to_string(),
        )?;
        writeln_name_value(
            f,
            "Wrapped token account:",
            &self.wrapped_token_account.to_string(),
        )?;
        writeln_name_value(f, "Escrow account:", &self.escrow_account.to_string())?;
        writeln_name_value(
            f,
            "Recipient unwrapped token account:",
            &self.recipient_token_account.to_string(),
        )?;
        writeln_name_value(f, "Amount unwrapped:", &self.amount.to_string())?;

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

impl QuietDisplay for UnwrapOutput {
    fn write_str(&self, _: &mut dyn std::fmt::Write) -> std::fmt::Result {
        Ok(())
    }
}
impl VerboseDisplay for UnwrapOutput {}

pub async fn command_unwrap(
    config: &Config,
    args: UnwrapArgs,
    matches: &ArgMatches,
    wallet_manager: &mut Option<Rc<RemoteWalletManager>>,
) -> CommandResult {
    let ResolvedAddrs {
        wrapped_token_program,
        unwrapped_mint_address,
        wrapped_mint_address,
        wrapped_mint_authority_address,
        unwrapped_token_program,
        escrow_account,
        transfer_authority_signer,
    } = resolve_addresses(config, &args, matches, wallet_manager).await?;

    let mut multisig_signers: Vec<Arc<dyn Signer>> = vec![];
    if let Some(sources) = &args.multisig_signer {
        for source in sources {
            let signer = signer_from_source_with_config(
                matches,
                source,
                "multisig_signer",
                wallet_manager,
                &SignerFromPathConfig {
                    allow_null_signer: true,
                },
            )
            .map_err(|e| e.to_string())?;
            multisig_signers.push(Arc::from(signer));
        }
    }

    let multisig_pubkeys = multisig_signers
        .iter()
        .map(|s| s.pubkey())
        .collect::<Vec<Pubkey>>();

    let instruction = unwrap(
        &spl_token_wrap::id(),
        &escrow_account,
        &args.unwrapped_token_recipient,
        &wrapped_mint_authority_address,
        &unwrapped_mint_address,
        &wrapped_token_program,
        &unwrapped_token_program,
        &args.wrapped_token_account,
        &wrapped_mint_address,
        &transfer_authority_signer.pubkey(),
        &multisig_pubkeys.iter().collect::<Vec<&Pubkey>>(),
        args.amount,
    );

    let blockhash = if let Some(hash) = args.blockhash {
        hash
    } else {
        config.rpc_client.get_latest_blockhash().await?
    };

    let payer = config.fee_payer()?;

    // Payer will always be a signer
    let mut signers = vec![payer.clone()];

    // In the case that a transfer_authority is passed (otherwise defaults to
    // payer), it needs to be added to signers if it isn't a multisig.
    if payer.pubkey() != transfer_authority_signer.pubkey() && multisig_signers.is_empty() {
        signers.push(transfer_authority_signer);
    }

    for signer in &multisig_signers {
        signers.push(signer.clone());
    }

    // Pre-signed transactions can be passed as --signer `PUBKEY=SIGNATURE`
    if let Some(pre_signers) = &args.signer {
        for signer in pre_signers {
            signers.push(Arc::from(signer));
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

    let output = UnwrapOutput {
        unwrapped_token_program,
        unwrapped_mint_address,
        wrapped_mint_address,
        escrow_account,
        wrapped_token_account: args.wrapped_token_account,
        recipient_token_account: args.unwrapped_token_recipient,
        amount: args.amount,
        signatures: transaction.signatures,
        sign_only_data,
    };

    Ok(format_output(config, output))
}

struct ResolvedAddrs {
    wrapped_token_program: Pubkey,
    unwrapped_mint_address: Pubkey,
    wrapped_mint_address: Pubkey,
    wrapped_mint_authority_address: Pubkey,
    unwrapped_token_program: Pubkey,
    escrow_account: Pubkey,
    transfer_authority_signer: Arc<dyn Signer>,
}

// Validates optional fields passed, or if not passed queries for them
async fn resolve_addresses(
    config: &Config,
    args: &UnwrapArgs,
    matches: &ArgMatches,
    wallet_manager: &mut Option<Rc<RemoteWalletManager>>,
) -> Result<ResolvedAddrs, Error> {
    let payer = config.fee_payer()?;

    // Validate `wrapped_token_program` governs `wrapped_token_account`
    let queried_wrapped_token_program =
        get_account_owner(&config.rpc_client, &args.wrapped_token_account).await?;
    let wrapped_token_program = if let Some(program_id) = args.wrapped_token_program {
        if program_id != queried_wrapped_token_program {
            return Err(format!(
                "Provided wrapped token program ID {program_id} does not match actual owner \
                 {queried_wrapped_token_program} of account {}",
                args.wrapped_token_account
            )
            .into());
        }
        program_id
    } else {
        queried_wrapped_token_program
    };

    // Validate `unwrapped_token_recipient` account matches the `unwrapped_mint`
    let queried_mint =
        get_mint_for_token_account(&config.rpc_client, &args.unwrapped_token_recipient).await?;
    let unwrapped_mint_address = if let Some(mint) = args.unwrapped_mint {
        if mint != queried_mint {
            return Err(format!(
                "Provided unwrapped mint {mint} does not match actual mint {queried_mint} of \
                 recipient account {}",
                args.unwrapped_token_recipient
            )
            .into());
        }
        mint
    } else {
        queried_mint
    };

    // Validate `unwrapped_mint_address` matches the `unwrapped_token_program`
    let queried_unwrapped_token_program =
        get_account_owner(&config.rpc_client, &unwrapped_mint_address).await?;
    let unwrapped_token_program = if let Some(program_id) = args.unwrapped_token_program {
        if program_id != queried_unwrapped_token_program {
            return Err(format!(
                "Provided unwrapped token program ID {program_id} does not match actual owner \
                 {queried_unwrapped_token_program} of unwrapped mint {unwrapped_mint_address}",
            )
            .into());
        }
        program_id
    } else {
        queried_unwrapped_token_program
    };

    let wrapped_mint_address =
        get_wrapped_mint_address(&unwrapped_mint_address, &wrapped_token_program);
    let wrapped_mint_authority_address = get_wrapped_mint_authority(&wrapped_mint_address);

    // If not passed, default to the ATA of the `wrapped_mint_authority`
    let escrow_account = args.escrow_account.unwrap_or_else(|| {
        println_display(
            config,
            "Escrow account not provided, defaulting to `wrapped_mint_authority` ATA.".to_string(),
        );
        get_associated_token_address_with_program_id(
            &wrapped_mint_authority_address,
            &unwrapped_mint_address,
            &unwrapped_token_program,
        )
    });

    if !config.dry_run {
        println_display(
            config,
            format!(
                "Unwrapping {} tokens from mint {wrapped_mint_address} to {unwrapped_mint_address}",
                args.amount,
            ),
        );
    }

    let transfer_authority_signer = if let Some(authority_source) = &args.transfer_authority {
        let signer = signer_from_source_with_config(
            matches,
            authority_source,
            "transfer_authority",
            wallet_manager,
            &SignerFromPathConfig {
                allow_null_signer: true,
            },
        )
        .map_err(|e| e.to_string())?;
        Arc::from(signer)
    } else {
        payer.clone() // Default to payer
    };

    Ok(ResolvedAddrs {
        wrapped_token_program,
        unwrapped_mint_address,
        wrapped_mint_address,
        wrapped_mint_authority_address,
        unwrapped_token_program,
        escrow_account,
        transfer_authority_signer,
    })
}
