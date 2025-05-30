use {
    crate::{
        common::{
            assert_mint_account, get_account_owner, parse_pubkey, parse_token_program,
            process_transaction,
        },
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
    spl_associated_token_account_client::instruction::create_associated_token_account,
    spl_token_wrap::{get_escrow_address, get_wrapped_mint_address, get_wrapped_mint_authority},
    std::fmt::{Display, Formatter},
};

#[derive(Clone, Debug, Args)]
#[clap(about = "Creates an escrow token account for holding unwrapped tokens")]
pub struct CreateEscrowAccountArgs {
    /// The address of the mint for the unwrapped tokens the escrow will hold
    #[clap(value_parser = parse_pubkey)]
    pub unwrapped_mint: Pubkey,

    /// The address of the token program for the *wrapped* mint
    #[clap(value_parser = parse_token_program)]
    pub wrapped_token_program: Pubkey,

    /// Do not error if the escrow account already exists and is initialized
    #[clap(long)]
    pub idempotent: bool,
}

#[serde_as]
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateEscrowAccountOutput {
    #[serde_as(as = "DisplayFromStr")]
    pub escrow_account_address: Pubkey,

    #[serde_as(as = "DisplayFromStr")]
    pub escrow_account_owner: Pubkey, // This is the wrapped_mint_authority PDA

    #[serde_as(as = "DisplayFromStr")]
    pub unwrapped_token_program_id: Pubkey,

    #[serde_as(as = "Option<DisplayFromStr>")]
    pub signature: Option<Signature>,
}

impl Display for CreateEscrowAccountOutput {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        writeln_name_value(
            f,
            "Escrow Account Address:",
            &self.escrow_account_address.to_string(),
        )?;
        writeln_name_value(
            f,
            "Escrow Account Owner (wrapped mint authority):",
            &self.escrow_account_owner.to_string(),
        )?;
        writeln_name_value(
            f,
            "Unwrapped Token Program ID:",
            &self.unwrapped_token_program_id.to_string(),
        )?;
        if let Some(signature) = self.signature {
            writeln_name_value(f, "Signature:", &signature.to_string())?;
        }
        Ok(())
    }
}

impl QuietDisplay for CreateEscrowAccountOutput {
    fn write_str(&self, _: &mut dyn std::fmt::Write) -> std::fmt::Result {
        Ok(())
    }
}
impl VerboseDisplay for CreateEscrowAccountOutput {}

pub async fn command_create_escrow_account(
    config: &Config,
    args: CreateEscrowAccountArgs,
) -> CommandResult {
    let payer = config.fee_payer()?;
    let rpc_client = config.rpc_client.clone();

    // --- Validate Unwrapped Mint ---
    assert_mint_account(&rpc_client, &args.unwrapped_mint).await?;

    // --- Determine Unwrapped Token Program ---
    let unwrapped_token_program_id = get_account_owner(&rpc_client, &args.unwrapped_mint).await?;

    // --- Derive PDAs ---
    let wrapped_mint_address =
        get_wrapped_mint_address(&args.unwrapped_mint, &args.wrapped_token_program);
    let wrapped_mint_authority = get_wrapped_mint_authority(&wrapped_mint_address);

    println_display(
        config,
        format!(
            "Creating escrow account under program {} for unwrapped mint {} owned by wrapped mint \
             authority {}",
            unwrapped_token_program_id, args.unwrapped_mint, wrapped_mint_authority
        ),
    );

    let mut instructions = Vec::new();
    let escrow_account_address = get_escrow_address(
        &args.unwrapped_mint,
        &unwrapped_token_program_id,
        &args.wrapped_token_program,
    );

    match rpc_client.get_account(&escrow_account_address).await {
        Ok(_) => {
            if args.idempotent {
                println_display(
                    config,
                    format!(
                        "Escrow account {} already exists, skipping creation",
                        escrow_account_address
                    ),
                );
            } else {
                return Err(
                    format!("Escrow account {} already exists", escrow_account_address).into(),
                );
            }
        }
        Err(_) => {
            instructions.push(create_associated_token_account(
                &payer.pubkey(),
                &wrapped_mint_authority,
                &args.unwrapped_mint,
                &unwrapped_token_program_id,
            ));
        }
    }

    // --- Build and Send Transaction if Needed ---
    let signature = if instructions.is_empty() {
        None
    } else {
        let latest_blockhash = rpc_client.get_latest_blockhash().await?;
        let transaction = Transaction::new_signed_with_payer(
            &instructions,
            Some(&payer.pubkey()),
            &[payer.clone()],
            latest_blockhash,
        );
        process_transaction(config, transaction).await?
    };

    Ok(format_output(
        config,
        CreateEscrowAccountOutput {
            escrow_account_address,
            escrow_account_owner: wrapped_mint_authority,
            unwrapped_token_program_id,
            signature,
        },
    ))
}
