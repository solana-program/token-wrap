use {
    crate::{
        common::{
            get_account_owner, get_mint, parse_pubkey, parse_token_program, process_transaction,
        },
        config::Config,
        output::{format_output, println_display},
        CommandResult,
    },
    clap::{value_parser, Args},
    serde_derive::{Deserialize, Serialize},
    serde_with::{serde_as, DisplayFromStr},
    solana_clap_v3_utils::{
        input_parsers::signer::{SignerSource, SignerSourceParserBuilder},
        keypair::signer_from_source,
    },
    solana_cli_output::{display::writeln_name_value, QuietDisplay, VerboseDisplay},
    solana_hash::Hash,
    solana_pubkey::Pubkey,
    solana_remote_wallet::remote_wallet::RemoteWalletManager,
    solana_signature::Signature,
    solana_signer::Signer,
    solana_transaction::Transaction,
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

    /// The address of the escrow account holding the unwrapped tokens
    #[clap(value_parser = parse_pubkey)]
    pub escrow_account: Pubkey,

    /// The address of the token account to receive the unwrapped tokens
    #[clap(value_parser = parse_pubkey)]
    pub unwrapped_token_recipient: Pubkey,

    /// The amount of tokens to unwrap
    #[clap(value_parser)]
    pub amount: u64,

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

    #[clap(long, value_parser = value_parser!(Hash))]
    pub blockhash: Option<Hash>,
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
    pub recipient_token_account: Pubkey,

    pub amount: u64,

    pub signatures: Vec<Signature>,
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
            "Recipient unwrapped token account:",
            &self.recipient_token_account.to_string(),
        )?;
        writeln_name_value(f, "Amount unwrapped:", &self.amount.to_string())?;

        writeln!(f, "Signers:")?;
        for signature in &self.signatures {
            writeln!(f, "  {signature}")?;
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
    matches: &clap::ArgMatches,
    wallet_manager: &mut Option<Rc<RemoteWalletManager>>,
) -> CommandResult {
    let payer = config.fee_payer()?;

    // Get wrapped token program ID from the owner of the wrapped token account
    // if not provided
    let wrapped_token_program_id = if let Some(program_id) = args.wrapped_token_program {
        program_id
    } else {
        get_account_owner(&config.rpc_client, &args.wrapped_token_account).await?
    };

    // Get unwrapped mint from the recipient account if not provided
    let unwrapped_mint_address = if let Some(mint) = args.unwrapped_mint {
        mint
    } else {
        get_mint(&config.rpc_client, &args.unwrapped_token_recipient).await?
    };

    // Get unwrapped token program ID from the owner of the escrow account
    // if not provided
    let unwrapped_token_program = if let Some(program_id) = args.unwrapped_token_program {
        program_id
    } else {
        get_account_owner(&config.rpc_client, &args.escrow_account).await?
    };

    println_display(
        config,
        format!(
            "Unwrapping {} tokens from mint {}",
            args.amount, unwrapped_mint_address
        ),
    );

    // Derive wrapped mint address and mint authority
    let wrapped_mint_address =
        get_wrapped_mint_address(&unwrapped_mint_address, &wrapped_token_program_id);
    let wrapped_mint_authority_address = get_wrapped_mint_authority(&wrapped_mint_address);

    // If transfer_authority is provided, use it as a signer,
    // else default to fee payer
    let transfer_authority_signer = if let Some(authority_keypair_path) = &args.transfer_authority {
        let signer = signer_from_source(
            matches,
            authority_keypair_path,
            "transfer_authority",
            wallet_manager,
        )
        .map_err(|e| e.to_string())?;
        Arc::from(signer)
    } else {
        payer.clone()
    };

    let instruction = unwrap(
        &spl_token_wrap::id(),
        &args.escrow_account,
        &args.unwrapped_token_recipient,
        &wrapped_mint_authority_address,
        &unwrapped_mint_address,
        &wrapped_token_program_id,
        &unwrapped_token_program,
        &args.wrapped_token_account,
        &wrapped_mint_address,
        &transfer_authority_signer.pubkey(),
        &[], // TODO: add multisig support
        args.amount,
    );

    let blockhash = if let Some(hash) = args.blockhash {
        hash
    } else {
        config.rpc_client.get_latest_blockhash().await?
    };

    // Payer will always be a signer
    let mut signers: Vec<&dyn Signer> = vec![payer.as_ref()];

    // Add transfer_authority if it's not the payer
    if payer.pubkey() != transfer_authority_signer.pubkey() {
        signers.push(transfer_authority_signer.as_ref());
    }

    let mut transaction = Transaction::new_with_payer(&[instruction], Some(&payer.pubkey()));
    transaction.partial_sign(&signers, blockhash);

    process_transaction(config, transaction.clone()).await?;

    let output = UnwrapOutput {
        unwrapped_token_program,
        unwrapped_mint_address,
        recipient_token_account: args.unwrapped_token_recipient,
        amount: args.amount,
        signatures: transaction.signatures,
    };

    Ok(format_output(config, output))
}
