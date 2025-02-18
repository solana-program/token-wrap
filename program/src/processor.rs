//! Program state processor

use spl_transfer_hook_interface::get_extra_account_metas_address;
use spl_transfer_hook_interface::instruction::execute_with_extra_account_metas;
use {
    crate::{
        error::TokenWrapError, get_wrapped_mint_address, get_wrapped_mint_address_with_seed,
        get_wrapped_mint_authority, get_wrapped_mint_authority_signer_seeds,
        get_wrapped_mint_authority_with_seed, get_wrapped_mint_backpointer_address_signer_seeds,
        get_wrapped_mint_backpointer_address_with_seed, get_wrapped_mint_signer_seeds,
        instruction::TokenWrapInstruction, state::Backpointer,
    },
    solana_account_info::{next_account_info, AccountInfo},
    solana_cpi::{invoke, invoke_signed},
    solana_msg::msg,
    solana_program::sysvar::Sysvar,
    solana_program_error::{ProgramError, ProgramResult},
    solana_program_pack::Pack,
    solana_pubkey::Pubkey,
    solana_rent::Rent,
    solana_system_interface::instruction::{allocate, assign},
    spl_token_2022::{
        extension::PodStateWithExtensions,
        instruction::initialize_mint2,
        pod::{PodAccount, PodMint},
    },
};

/// Processes [`CreateMint`](enum.TokenWrapInstruction.html) instruction.
pub fn process_create_mint(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    idempotent: bool,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();

    let wrapped_mint_account = next_account_info(account_info_iter)?;
    let wrapped_backpointer_account = next_account_info(account_info_iter)?;
    let unwrapped_mint_account = next_account_info(account_info_iter)?;
    let _system_program_account = next_account_info(account_info_iter)?;
    let wrapped_token_program_account = next_account_info(account_info_iter)?;

    let (wrapped_mint_address, mint_bump) = get_wrapped_mint_address_with_seed(
        unwrapped_mint_account.key,
        wrapped_token_program_account.key,
    );

    let (wrapped_backpointer_address, backpointer_bump) =
        get_wrapped_mint_backpointer_address_with_seed(wrapped_mint_account.key);

    // PDA derivation validation

    if *wrapped_mint_account.key != wrapped_mint_address {
        Err(TokenWrapError::WrappedMintMismatch)?
    }

    if *wrapped_backpointer_account.key != wrapped_backpointer_address {
        Err(TokenWrapError::BackpointerMismatch)?
    }

    // Idempotency checks

    if wrapped_mint_account.data_len() > 0 || wrapped_backpointer_account.data_len() > 0 {
        msg!("Wrapped mint or backpointer account already initialized");
        if !idempotent {
            Err(ProgramError::AccountAlreadyInitialized)?
        }
        if wrapped_mint_account.owner != wrapped_token_program_account.key {
            Err(TokenWrapError::InvalidWrappedMintOwner)?
        }
        if wrapped_backpointer_account.owner != program_id {
            Err(TokenWrapError::InvalidBackpointerOwner)?
        }
        return Ok(());
    }

    // Initialize wrapped mint PDA

    let bump_seed = [mint_bump];
    let signer_seeds = get_wrapped_mint_signer_seeds(
        unwrapped_mint_account.key,
        wrapped_token_program_account.key,
        &bump_seed,
    );
    let space = spl_token_2022::state::Mint::get_packed_len();

    let rent = Rent::get()?;
    let mint_rent_required = rent.minimum_balance(space);
    if wrapped_mint_account.lamports() < mint_rent_required {
        msg!(
            "Error: wrapped_mint_account requires pre-funding of {} lamports",
            mint_rent_required
        );
        Err(ProgramError::AccountNotRentExempt)?
    }

    // Initialize the wrapped mint

    invoke_signed(
        &allocate(&wrapped_mint_address, space as u64),
        &[wrapped_mint_account.clone()],
        &[&signer_seeds],
    )?;
    invoke_signed(
        &assign(&wrapped_mint_address, wrapped_token_program_account.key),
        &[wrapped_mint_account.clone()],
        &[&signer_seeds],
    )?;

    // New wrapped mint matches decimals & freeze authority of unwrapped mint
    let unwrapped_mint_data = unwrapped_mint_account.try_borrow_data()?;
    let unpacked_unwrapped_mint =
        PodStateWithExtensions::<PodMint>::unpack(&unwrapped_mint_data)?.base;
    let decimals = unpacked_unwrapped_mint.decimals;
    let freeze_authority = unpacked_unwrapped_mint
        .freeze_authority
        .ok_or(ProgramError::InvalidArgument)
        .ok();

    let wrapped_mint_authority = get_wrapped_mint_authority(wrapped_mint_account.key);

    invoke(
        &initialize_mint2(
            wrapped_token_program_account.key,
            wrapped_mint_account.key,
            &wrapped_mint_authority,
            freeze_authority.as_ref(),
            decimals,
        )?,
        &[wrapped_mint_account.clone()],
    )?;

    // Initialize backpointer PDA

    let backpointer_space = std::mem::size_of::<Backpointer>();
    let backpointer_rent_required = rent.minimum_balance(space);
    if wrapped_backpointer_account.lamports() < rent.minimum_balance(backpointer_space) {
        msg!(
            "Error: wrapped_backpointer_account requires pre-funding of {} lamports",
            backpointer_rent_required
        );
        Err(ProgramError::AccountNotRentExempt)?
    }

    let bump_seed = [backpointer_bump];
    let backpointer_signer_seeds =
        get_wrapped_mint_backpointer_address_signer_seeds(wrapped_mint_account.key, &bump_seed);
    invoke_signed(
        &allocate(&wrapped_backpointer_address, backpointer_space as u64),
        &[wrapped_backpointer_account.clone()],
        &[&backpointer_signer_seeds],
    )?;
    invoke_signed(
        &assign(&wrapped_backpointer_address, program_id),
        &[wrapped_backpointer_account.clone()],
        &[&backpointer_signer_seeds],
    )?;

    // Set data within backpointer PDA

    let mut backpointer_account_data = wrapped_backpointer_account.try_borrow_mut_data()?;
    let backpointer = bytemuck::from_bytes_mut::<Backpointer>(&mut backpointer_account_data[..]);
    backpointer.unwrapped_mint = *unwrapped_mint_account.key;

    Ok(())
}

/// Processes [`Wrap`](enum.TokenWrapInstruction.html) instruction.
pub fn process_wrap(accounts: &[AccountInfo], amount: u64) -> ProgramResult {
    if amount == 0 {
        Err(TokenWrapError::ZeroWrapAmount)?
    }

    let account_info_iter = &mut accounts.iter();

    let recipient_wrapped_token_account = next_account_info(account_info_iter)?;
    let wrapped_mint = next_account_info(account_info_iter)?;
    let wrapped_mint_authority = next_account_info(account_info_iter)?;
    let unwrapped_token_program = next_account_info(account_info_iter)?;
    let wrapped_token_program = next_account_info(account_info_iter)?;
    let unwrapped_token_account = next_account_info(account_info_iter)?;
    let unwrapped_mint = next_account_info(account_info_iter)?;
    let unwrapped_escrow = next_account_info(account_info_iter)?;
    let transfer_authority = next_account_info(account_info_iter)?;

    // The remaining accounts can include:
    //  - Optional multisig signers for the transfer authority, and
    //  - Transfer hook extra validation accounts.
    let remaining_accounts = account_info_iter.as_slice();
    // --- Account Splitting: Divide remaining accounts into multisig signers and transfer hook accounts ---
    let mut multisig_signers = Vec::new();
    let mut transfer_hook_accounts = Vec::new();

    // For example, any account flagged as a signer we treat as a multisig signer.
    // The remaining ones may be used in the transfer hook CPI.
    for acct in remaining_accounts.iter() {
        if acct.is_signer {
            multisig_signers.push(acct.clone());
        } else {
            transfer_hook_accounts.push(acct.clone());
        }
    }

    // Validate accounts

    let expected_wrapped_mint =
        get_wrapped_mint_address(unwrapped_mint.key, wrapped_token_program.key);
    if expected_wrapped_mint != *wrapped_mint.key {
        Err(TokenWrapError::WrappedMintMismatch)?
    }

    let (expected_authority, bump) = get_wrapped_mint_authority_with_seed(wrapped_mint.key);
    if *wrapped_mint_authority.key != expected_authority {
        Err(TokenWrapError::MintAuthorityMismatch)?
    }

    {
        let escrow_data = unwrapped_escrow.try_borrow_data()?;
        let escrow_account = PodStateWithExtensions::<PodAccount>::unpack(&escrow_data)?;
        if escrow_account.base.owner != expected_authority {
            Err(TokenWrapError::EscrowOwnerMismatch)?
        }
    }

    if !transfer_hook_accounts.is_empty() {
        // Find the transfer hook validation state account, which is expected when hooks are configured
        let validation_state_pubkey =
            get_extra_account_metas_address(unwrapped_mint.key, unwrapped_token_program.key);

        // Build the hook CPI instruction.
        let hook_instruction = execute_with_extra_account_metas(
            unwrapped_token_program.key, // Hook program ID (if unwrapped token program is also the hook program)
            unwrapped_token_account.key, // Source token account
            unwrapped_mint.key,          // Unwrapped mint
            unwrapped_escrow.key,        // Destination: escrow account
            transfer_authority.key,      // Transfer authority
            &validation_state_pubkey,    // PDA holding extra hook configuration
            &[],                         // Provide extra AccountMeta if needed
            amount,
        );

        // Construct an account list for hook CPI.
        // Make sure the first element is the hook validation state account.
        // (If multiple accounts are provided, ensure they’re in the order expected by the hook program.)
        let mut hook_account_infos: Vec<AccountInfo> = vec![
            unwrapped_token_account.clone(),
            unwrapped_mint.clone(),
            unwrapped_escrow.clone(),
            transfer_authority.clone(),
            // Use the first non-signer account as the validation state account.
            transfer_hook_accounts.first().cloned().unwrap(),
        ];
        hook_account_infos.extend(transfer_hook_accounts);
        hook_account_infos.extend(multisig_signers.clone());

        msg!("Invoking transfer hook CPI call...");
        invoke(&hook_instruction, &hook_account_infos)?;
    } else {
        msg!("No transfer hook accounts provided, skipping hook CPI call.");
    }

    let multisig_signer_pubkeys = multisig_signers
        .iter()
        .map(|account| account.key)
        .collect::<Vec<_>>();

    let unwrapped_mint_data = unwrapped_mint.try_borrow_data()?;
    let unwrapped_mint_state = PodStateWithExtensions::<PodMint>::unpack(&unwrapped_mint_data)?;

    // Transfer unwrapped tokens from user to escrow

    invoke(
        &spl_token_2022::instruction::transfer_checked(
            unwrapped_token_program.key,
            unwrapped_token_account.key,
            unwrapped_mint.key,
            unwrapped_escrow.key,
            transfer_authority.key,
            &multisig_signer_pubkeys,
            amount,
            unwrapped_mint_state.base.decimals,
        )?,
        &accounts[5..],
    )?;

    // Mint wrapped tokens to recipient
    let bump_seed = [bump];
    let signer_seeds = get_wrapped_mint_authority_signer_seeds(wrapped_mint.key, &bump_seed);

    invoke_signed(
        &spl_token_2022::instruction::mint_to(
            wrapped_token_program.key,
            wrapped_mint.key,
            recipient_wrapped_token_account.key,
            wrapped_mint_authority.key,
            &[],
            amount,
        )?,
        &[
            wrapped_mint.clone(),
            recipient_wrapped_token_account.clone(),
            wrapped_mint_authority.clone(),
        ],
        &[&signer_seeds],
    )?;

    Ok(())
}

/// Processes [`Unwrap`](enum.TokenWrapInstruction.html) instruction.
pub fn process_unwrap(accounts: &[AccountInfo], amount: u64) -> ProgramResult {
    if amount == 0 {
        Err(TokenWrapError::ZeroWrapAmount)?
    }

    let account_info_iter = &mut accounts.iter();

    let unwrapped_escrow = next_account_info(account_info_iter)?;
    let recipient_unwrapped_token = next_account_info(account_info_iter)?;
    let wrapped_mint_authority = next_account_info(account_info_iter)?;
    let unwrapped_mint = next_account_info(account_info_iter)?;
    let wrapped_token_program = next_account_info(account_info_iter)?;
    let unwrapped_token_program = next_account_info(account_info_iter)?;
    let wrapped_token_account = next_account_info(account_info_iter)?;
    let wrapped_mint = next_account_info(account_info_iter)?;
    let transfer_authority = next_account_info(account_info_iter)?;
    let multisig_signer_accounts = account_info_iter.as_slice();

    // Validate accounts

    let expected_wrapped_mint =
        get_wrapped_mint_address(unwrapped_mint.key, wrapped_token_program.key);
    if expected_wrapped_mint != *wrapped_mint.key {
        Err(TokenWrapError::WrappedMintMismatch)?
    }

    let (expected_authority, bump) = get_wrapped_mint_authority_with_seed(wrapped_mint.key);
    if *wrapped_mint_authority.key != expected_authority {
        Err(TokenWrapError::MintAuthorityMismatch)?
    }

    // Burn wrapped tokens

    let multisig_signer_pubkeys = multisig_signer_accounts
        .iter()
        .map(|account| account.key)
        .collect::<Vec<_>>();

    invoke(
        &spl_token_2022::instruction::burn(
            wrapped_token_program.key,
            wrapped_token_account.key,
            wrapped_mint.key,
            transfer_authority.key,
            &multisig_signer_pubkeys,
            amount,
        )?,
        &accounts[6..],
    )?;

    // Transfer unwrapped tokens from escrow to recipient

    let unwrapped_mint_data = unwrapped_mint.try_borrow_data()?;
    let unwrapped_mint_state = PodStateWithExtensions::<PodMint>::unpack(&unwrapped_mint_data)?;
    let bump_seed = [bump];
    let signer_seeds = get_wrapped_mint_authority_signer_seeds(wrapped_mint.key, &bump_seed);

    invoke_signed(
        &spl_token_2022::instruction::transfer_checked(
            unwrapped_token_program.key,
            unwrapped_escrow.key,
            unwrapped_mint.key,
            recipient_unwrapped_token.key,
            wrapped_mint_authority.key,
            &[],
            amount,
            unwrapped_mint_state.base.decimals,
        )?,
        &[
            unwrapped_escrow.clone(),
            unwrapped_mint.clone(),
            recipient_unwrapped_token.clone(),
            wrapped_mint_authority.clone(),
        ],
        &[&signer_seeds],
    )?;

    Ok(())
}

/// Instruction processor
pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    input: &[u8],
) -> ProgramResult {
    match TokenWrapInstruction::unpack(input)? {
        TokenWrapInstruction::CreateMint { idempotent } => {
            msg!("Instruction: CreateMint");
            process_create_mint(program_id, accounts, idempotent)
        }
        TokenWrapInstruction::Wrap { amount } => {
            msg!("Instruction: Wrap");
            process_wrap(accounts, amount)
        }
        TokenWrapInstruction::Unwrap { amount } => {
            msg!("Instruction: Unwrap");
            process_unwrap(accounts, amount)
        }
    }
}
