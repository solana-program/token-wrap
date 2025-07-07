use {
    crate::{
        common::{parse_pubkey, parse_token_program, process_transaction},
        config::Config,
        output::{format_output, println_display},
        CommandResult,
    },
    clap::Args,
    serde_derive::{Deserialize, Serialize},
    serde_with::{serde_as, DisplayFromStr},
    solana_cli_output::{display::writeln_name_value, QuietDisplay, VerboseDisplay},
    solana_instruction::Instruction,
    solana_program_pack::Pack,
    solana_pubkey::Pubkey,
    solana_signature::Signature,
    solana_system_interface::instruction::transfer,
    solana_transaction::Transaction,
    spl_token_2022::{extension::ExtensionType, state::Mint},
    spl_token_wrap::{
        get_wrapped_mint_address, get_wrapped_mint_backpointer_address, id,
        instruction::create_mint,
    },
    std::fmt::{Display, Formatter},
};

#[derive(Clone, Debug, Args)]
pub struct CreateMintArgs {
    /// The address of the mint to wrap
    #[clap(value_parser = parse_pubkey)]
    pub unwrapped_mint: Pubkey,

    /// The address of the token program that the wrapped mint should belong to
    #[clap(value_parser = parse_token_program)]
    pub wrapped_token_program: Pubkey,

    /// Do not err if account already created
    #[clap(long)]
    pub idempotent: bool,
}

#[serde_as]
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateMintOutput {
    #[serde_as(as = "DisplayFromStr")]
    pub unwrapped_mint_address: Pubkey,
    #[serde_as(as = "DisplayFromStr")]
    pub wrapped_mint_address: Pubkey,
    #[serde_as(as = "DisplayFromStr")]
    pub wrapped_backpointer_address: Pubkey,
    pub funded_wrapped_mint_lamports: u64,
    pub funded_backpointer_lamports: u64,
    #[serde_as(as = "Option<DisplayFromStr>")]
    pub signature: Option<Signature>,
}

impl Display for CreateMintOutput {
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
            "Wrapped backpointer address:",
            &self.wrapped_backpointer_address.to_string(),
        )?;
        writeln_name_value(
            f,
            "Funded wrapped mint lamports:",
            &self.funded_wrapped_mint_lamports.to_string(),
        )?;
        writeln_name_value(
            f,
            "Funded backpointer lamports:",
            &self.funded_backpointer_lamports.to_string(),
        )?;

        if let Some(signature) = self.signature {
            writeln_name_value(f, "Signature:", &signature.to_string())?;
        }

        Ok(())
    }
}

impl QuietDisplay for CreateMintOutput {
    fn write_str(&self, _: &mut dyn std::fmt::Write) -> std::fmt::Result {
        Ok(())
    }
}
impl VerboseDisplay for CreateMintOutput {}

pub async fn command_create_mint(config: &Config, args: CreateMintArgs) -> CommandResult {
    let payer = config.fee_payer()?;
    let rpc_client = config.rpc_client.clone();

    let wrapped_mint_address =
        get_wrapped_mint_address(&args.unwrapped_mint, &args.wrapped_token_program);
    let wrapped_backpointer_address = get_wrapped_mint_backpointer_address(&wrapped_mint_address);

    println_display(
        config,
        format!("Creating wrapped mint for {}", args.unwrapped_mint),
    );

    let mut instructions: Vec<Instruction> = Vec::new();

    // Fund the wrapped mint account if it doesn't exist or is insufficiently funded
    let wrapped_mint_account = rpc_client.get_account(&wrapped_mint_address).await;
    let wrapped_mint_lamports = match wrapped_mint_account {
        Ok(account) => account.lamports,
        Err(_) => 0,
    };

    // For token-2022, confidential transfers extension added by default
    let mint_size = if args.wrapped_token_program == spl_token_2022::id() {
        ExtensionType::try_calculate_account_len::<Mint>(&[
            ExtensionType::ConfidentialTransferMint,
        ])?
    } else {
        spl_token::state::Mint::LEN
    };

    let mint_rent = rpc_client
        .get_minimum_balance_for_rent_exemption(mint_size)
        .await?;

    let funded_wrapped_mint_lamports = mint_rent.saturating_sub(wrapped_mint_lamports);
    if funded_wrapped_mint_lamports > 0 {
        println_display(
            config,
            format!(
                "Funding wrapped_mint_account {wrapped_mint_address} with \
                 {funded_wrapped_mint_lamports} lamports for rent"
            ),
        );
        instructions.push(transfer(
            &payer.pubkey(),
            &wrapped_mint_address,
            funded_wrapped_mint_lamports,
        ));
    }

    // Fund the backpointer account if it doesn't exist or is insufficiently funded
    let backpointer_account = rpc_client.get_account(&wrapped_backpointer_address).await;
    let backpointer_lamports = match backpointer_account {
        Ok(account) => account.lamports,
        Err(_) => 0,
    };

    let backpointer_rent = rpc_client
        .get_minimum_balance_for_rent_exemption(std::mem::size_of::<
            spl_token_wrap::state::Backpointer,
        >())
        .await?;

    let funded_backpointer_lamports = backpointer_rent.saturating_sub(backpointer_lamports);
    if funded_backpointer_lamports > 0 {
        println_display(
            config,
            format!(
                "Funding backpointer_account {wrapped_backpointer_address} with \
                 {funded_backpointer_lamports} lamports for rent"
            ),
        );
        instructions.push(transfer(
            &payer.pubkey(),
            &wrapped_backpointer_address,
            funded_backpointer_lamports,
        ));
    }

    // Add the create_mint instruction
    instructions.push(create_mint(
        &id(),
        &wrapped_mint_address,
        &wrapped_backpointer_address,
        &args.unwrapped_mint,
        &args.wrapped_token_program,
        args.idempotent,
    ));

    let latest_blockhash = rpc_client.get_latest_blockhash().await?;
    let transaction = Transaction::new_signed_with_payer(
        &instructions,
        Some(&payer.pubkey()),
        &[&*payer],
        latest_blockhash,
    );

    let signature = process_transaction(config, transaction).await?;

    Ok(format_output(
        config,
        CreateMintOutput {
            unwrapped_mint_address: args.unwrapped_mint,
            wrapped_mint_address,
            wrapped_backpointer_address,
            funded_wrapped_mint_lamports,
            funded_backpointer_lamports,
            signature,
        },
    ))
}
