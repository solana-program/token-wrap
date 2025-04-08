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
    solana_clap_v3_utils::{
        input_parsers::signer::{SignerSource, SignerSourceParserBuilder},
        keypair::signer_from_source,
    },
    solana_cli_output::{display::writeln_name_value, QuietDisplay, VerboseDisplay},
    solana_program_pack::Pack,
    solana_pubkey::Pubkey,
    solana_remote_wallet::remote_wallet::RemoteWalletManager,
    solana_signature::Signature,
    solana_signer::Signer,
    solana_system_interface::instruction::create_account,
    solana_transaction::Transaction,
    spl_associated_token_account_client::{
        address::get_associated_token_address_with_program_id,
        instruction::{
            create_associated_token_account, create_associated_token_account_idempotent,
        },
    },
    spl_token_2022::instruction::initialize_account,
    spl_token_wrap::{get_wrapped_mint_address, get_wrapped_mint_authority},
    std::{
        fmt::{Display, Formatter},
        rc::Rc,
    },
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

    /// Keypair source for the escrow account itself.
    /// If not provided, the Associated Token Account (`ATA`) for the wrapped
    /// mint authority PDA will be used or created.
    #[clap(long, value_parser = SignerSourceParserBuilder::default().allow_all().build())]
    pub escrow_account_signer: Option<SignerSource>,

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
    matches: &clap::ArgMatches,
    wallet_manager: &mut Option<Rc<RemoteWalletManager>>,
) -> CommandResult {
    let payer = config.fee_payer()?;
    let rpc_client = config.rpc_client.clone();

    // --- Validate Unwrapped Mint ---
    assert_mint_account(&rpc_client, &args.unwrapped_mint).await?;

    // --- Determine Unwrapped Token Program ---
    let unwrapped_token_program_id = get_account_owner(&args.unwrapped_mint, &rpc_client).await?;

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
    let mut signers = vec![payer.clone()];
    let escrow_account_address: Pubkey;

    // --- Decide How to Create Escrow Account ---
    if let Some(signer_source) = &args.escrow_account_signer {
        // --- Case 1: User Supplied a Signer for the Escrow Account ---
        let escrow_signer = signer_from_source(
            matches,
            signer_source,
            "escrow_account_signer",
            wallet_manager,
        )
        .map_err(|e| format!("Failed to load escrow account signer: {}", e))?;
        escrow_account_address = escrow_signer.pubkey();
        signers.push(std::sync::Arc::from(escrow_signer));

        // Check whether this account already exists.
        match rpc_client.get_account(&escrow_account_address).await {
            // Account exists
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
                    return Err(format!(
                        "Escrow account {} already exists",
                        escrow_account_address
                    )
                    .into());
                }
            }
            // Account does not exist, create it
            Err(_) => {
                let account_len = spl_token_2022::state::Account::LEN;
                let rent_exempt_min = rpc_client
                    .get_minimum_balance_for_rent_exemption(account_len)
                    .await?;

                instructions.push(create_account(
                    &payer.pubkey(),
                    &escrow_account_address,
                    rent_exempt_min,
                    account_len as u64,
                    &unwrapped_token_program_id,
                ));

                instructions.push(initialize_account(
                    &unwrapped_token_program_id,
                    &escrow_account_address,
                    &args.unwrapped_mint,
                    &wrapped_mint_authority, // The PDA must be the owner
                )?);
            }
        }
    } else {
        // --- Case 2: Default to Associated Token Account (ATA) ---
        escrow_account_address = get_associated_token_address_with_program_id(
            &wrapped_mint_authority,
            &args.unwrapped_mint,
            &unwrapped_token_program_id,
        );

        println_display(
            config,
            format!("Using ATA {} for escrow account", escrow_account_address),
        );

        let create_ata_instruction = if args.idempotent {
            create_associated_token_account_idempotent
        } else {
            create_associated_token_account
        };

        instructions.push(create_ata_instruction(
            &payer.pubkey(),
            &wrapped_mint_authority,
            &args.unwrapped_mint,
            &unwrapped_token_program_id,
        ));
    }

    // --- Build and Send Transaction if Needed ---
    let signature = if instructions.is_empty() {
        None
    } else {
        let latest_blockhash = rpc_client.get_latest_blockhash().await?;
        let transaction = Transaction::new_signed_with_payer(
            &instructions,
            Some(&payer.pubkey()),
            &signers,
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
