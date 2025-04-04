use {
    crate::common::get_account_owner, solana_keypair::Keypair,
    spl_token_2022::instruction::initialize_account,
};
use {
    crate::{
        common::{parse_pubkey, parse_token_program, process_transaction}, /* Keep parse_token_program if we add an override flag later */
        config::Config,
        output::{format_output, println_display},
        CommandResult,
    },
    clap::Args,
    serde_derive::{Deserialize, Serialize},
    serde_with::{serde_as, DisplayFromStr},
    solana_cli_output::{display::writeln_name_value, QuietDisplay, VerboseDisplay},
    solana_program_pack::Pack,
    solana_pubkey::Pubkey,
    solana_signature::Signature,
    solana_signer::Signer,
    solana_system_interface::instruction::create_account,
    solana_transaction::Transaction,
    spl_token_wrap::{get_wrapped_mint_address, get_wrapped_mint_authority},
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
    pub escrow_token_program_id: Pubkey,

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
            "Escrow Account Owner (PDA):",
            &self.escrow_account_owner.to_string(),
        )?;
        writeln_name_value(
            f,
            "Escrow Token Program ID:",
            &self.escrow_token_program_id.to_string(),
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

    // --- Determine Unwrapped Token Program ---
    let unwrapped_token_program_id =
        get_account_owner(&args.unwrapped_mint, rpc_client.clone()).await?;

    // --- Derive PDAs ---
    let wrapped_mint_address =
        get_wrapped_mint_address(&args.unwrapped_mint, &args.wrapped_token_program);
    let wrapped_mint_authority = get_wrapped_mint_authority(&wrapped_mint_address);

    println_display(
        config,
        format!(
            "Creating escrow account under program {} for unwrapped mint {} owned by PDA {}",
            unwrapped_token_program_id, args.unwrapped_mint, wrapped_mint_authority
        ),
    );

    // --- Prepare Escrow Creation ---
    let escrow_keypair = Keypair::new();
    let escrow_account_address = escrow_keypair.pubkey();

    let account_len = spl_token_2022::state::Account::LEN;
    let rent = rpc_client
        .get_minimum_balance_for_rent_exemption(account_len)
        .await?;
    let create_account_instruction = create_account(
        &payer.pubkey(),
        &escrow_account_address,
        rent,
        account_len as u64,
        &unwrapped_token_program_id,
    );

    let initialize_instruction = initialize_account(
        &unwrapped_token_program_id,
        &escrow_account_address,
        &args.unwrapped_mint,
        &wrapped_mint_authority,
    )?;

    // --- Build and Send Transaction ---
    let latest_blockhash = rpc_client.get_latest_blockhash().await?;
    let transaction = Transaction::new_signed_with_payer(
        &[create_account_instruction, initialize_instruction],
        Some(&payer.pubkey()),
        &[&*payer, &escrow_keypair],
        latest_blockhash,
    );

    let signature = process_transaction(config, transaction).await?;

    Ok(format_output(
        config,
        CreateEscrowAccountOutput {
            escrow_account_address,
            escrow_account_owner: wrapped_mint_authority,
            escrow_token_program_id: unwrapped_token_program_id,
            signature,
        },
    ))
}
