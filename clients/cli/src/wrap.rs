use {
    crate::{
        common::{parse_pubkey, parse_token_program, process_transaction},
        config::Config,
        output::{format_output, println_display},
        CommandResult, Error,
    },
    clap::Args,
    serde_derive::{Deserialize, Serialize},
    serde_with::{serde_as, DisplayFromStr},
    solana_cli_output::{display::writeln_name_value, QuietDisplay, VerboseDisplay},
    solana_client::nonblocking::rpc_client::RpcClient,
    solana_pubkey::Pubkey,
    solana_remote_wallet::remote_wallet::RemoteWalletManager,
    solana_signature::Signature,
    solana_signer::Signer,
    solana_transaction::Transaction,
    spl_associated_token_account_client::address::get_associated_token_address_with_program_id,
    spl_token_2022::{extension::PodStateWithExtensions, pod::PodAccount},
    spl_token_wrap::{get_wrapped_mint_address, get_wrapped_mint_authority, instruction::wrap},
    std::{
        fmt::{Display, Formatter},
        rc::Rc,
        sync::Arc,
    },
};

#[derive(Clone, Debug, Args)]
pub struct WrapArgs {
    /// The address of the unwrapped token account to wrap from
    #[clap(value_parser = parse_pubkey)]
    pub unwrapped_token_account: Pubkey,

    /// The address of the escrow account that will hold the unwrapped tokens
    #[clap(value_parser = parse_pubkey)]
    pub escrow_account: Pubkey,

    /// The address of the token program that the wrapped mint should belong to
    #[clap(value_parser = parse_token_program)]
    pub wrapped_token_program: Pubkey,

    /// The amount of tokens to wrap
    #[clap(value_parser)]
    pub amount: u64,

    /// Path to the signer for the transfer authority if different from
    /// fee payer
    #[clap(long, value_name = "PATH")]
    pub transfer_authority: Option<String>,

    /// The address of the mint to wrap, queried if not provided
    #[clap(long, value_parser = parse_pubkey)]
    pub unwrapped_mint: Option<Pubkey>,

    /// The address of the token account to receive wrapped tokens.
    /// If not provided, defaults to fee payer associated token account
    #[clap(long, value_parser = parse_pubkey)]
    pub recipient_token_account: Option<Pubkey>,

    /// The address of the token program that the unwrapped mint belongs to.
    /// Queries account for `unwrapped_token_account` if not provided.
    #[clap(long, value_parser = parse_token_program)]
    pub unwrapped_token_program: Option<Pubkey>,
}

#[serde_as]
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WrapOutput {
    #[serde_as(as = "DisplayFromStr")]
    pub unwrapped_mint_address: Pubkey,
    #[serde_as(as = "DisplayFromStr")]
    pub wrapped_mint_address: Pubkey,
    #[serde_as(as = "DisplayFromStr")]
    pub unwrapped_token_account: Pubkey,
    #[serde_as(as = "DisplayFromStr")]
    pub recipient_token_account: Pubkey,
    #[serde_as(as = "DisplayFromStr")]
    pub escrow_account: Pubkey,
    pub amount: u64,
    #[serde_as(as = "Option<DisplayFromStr>")]
    pub signature: Option<Signature>,
}

impl Display for WrapOutput {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
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
            "Unwrapped token account:",
            &self.unwrapped_token_account.to_string(),
        )?;
        writeln_name_value(
            f,
            "Recipient wrapped token account:",
            &self.recipient_token_account.to_string(),
        )?;
        writeln_name_value(f, "Escrow account:", &self.escrow_account.to_string())?;
        writeln_name_value(f, "Amount:", &self.amount.to_string())?;
        if let Some(signature) = self.signature {
            writeln_name_value(f, "Signature:", &signature.to_string())?;
        }
        Ok(())
    }
}

impl QuietDisplay for WrapOutput {
    fn write_str(&self, _: &mut dyn std::fmt::Write) -> std::fmt::Result {
        Ok(())
    }
}
impl VerboseDisplay for WrapOutput {}

async fn get_unwrapped_mint(
    rpc_client: &RpcClient,
    unwrapped_token_account: &Pubkey,
) -> Result<Pubkey, Error> {
    let token_account_info = rpc_client.get_account(unwrapped_token_account).await?;
    let unpacked_account = PodStateWithExtensions::<PodAccount>::unpack(&token_account_info.data)?;
    Ok(unpacked_account.base.mint)
}

pub async fn command_wrap(
    config: &Config,
    args: WrapArgs,
    matches: &clap::ArgMatches,
    wallet_manager: &mut Option<Rc<RemoteWalletManager>>,
) -> CommandResult {
    let payer = config.fee_payer()?;

    let unwrapped_mint = if let Some(mint) = args.unwrapped_mint {
        mint
    } else {
        get_unwrapped_mint(&config.rpc_client, &args.unwrapped_token_account).await?
    };

    println_display(
        config,
        format!(
            "Wrapping {} tokens from mint {}",
            args.amount, unwrapped_mint
        ),
    );

    // Derive wrapped mint address and mint authority
    let wrapped_mint_address =
        get_wrapped_mint_address(&unwrapped_mint, &args.wrapped_token_program);
    let wrapped_mint_authority = get_wrapped_mint_authority(&wrapped_mint_address);

    // If no recipient passed, get ATA of payer
    let recipient_token_account = args.recipient_token_account.unwrap_or_else(|| {
        get_associated_token_address_with_program_id(
            &payer.pubkey(),
            &wrapped_mint_address,
            &args.wrapped_token_program,
        )
    });

    // If transfer_authority is provided, use it as a signer,
    // else default to fee payer
    let transfer_authority_signer = if let Some(authority_keypair_path) = &args.transfer_authority {
        let signer = solana_clap_v3_utils::keypair::signer_from_path(
            matches,
            authority_keypair_path,
            "transfer-authority",
            wallet_manager,
        )
        .map_err(|e| e.to_string())?;
        Arc::from(signer)
    } else {
        payer.clone()
    };

    let unwrapped_token_program = if let Some(pubkey) = args.unwrapped_token_program {
        pubkey
    } else {
        config
            .rpc_client
            .get_account(&args.unwrapped_token_account)
            .await?
            .owner
    };

    let instruction = wrap(
        &spl_token_wrap::id(),
        &recipient_token_account,
        &wrapped_mint_address,
        &wrapped_mint_authority,
        &unwrapped_token_program,
        &args.wrapped_token_program,
        &args.unwrapped_token_account,
        &unwrapped_mint,
        &args.escrow_account,
        &transfer_authority_signer.pubkey(),
        &[], // TODO: Add multisig support
        args.amount,
    );

    let latest_blockhash = config.rpc_client.get_latest_blockhash().await?;
    let mut signers = vec![payer.as_ref()];

    if payer.pubkey() != transfer_authority_signer.pubkey() {
        signers.push(transfer_authority_signer.as_ref());
    }

    let transaction = Transaction::new_signed_with_payer(
        &[instruction],
        Some(&payer.pubkey()),
        &signers,
        latest_blockhash,
    );

    let signature = process_transaction(config, transaction).await?;

    let output = WrapOutput {
        unwrapped_mint_address: unwrapped_mint,
        wrapped_mint_address,
        unwrapped_token_account: args.unwrapped_token_account,
        recipient_token_account,
        escrow_account: args.escrow_account,
        amount: args.amount,
        signature,
    };

    Ok(format_output(config, output))
}
