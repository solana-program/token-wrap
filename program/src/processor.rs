//! Program state processor

use {
    crate::{
        get_wrapped_mint_address_with_seed, get_wrapped_mint_authority,
        get_wrapped_mint_backpointer_address_signer_seeds,
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
        extension::PodStateWithExtensions, instruction::initialize_mint2, pod::PodMint,
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
        msg!("Wrapped mint account address does not match expected PDA");
        return Err(ProgramError::InvalidAccountData);
    }

    if *wrapped_backpointer_account.key != wrapped_backpointer_address {
        msg!("Error: wrapped_backpointer_account address is not as expected");
        return Err(ProgramError::InvalidSeeds);
    }

    // Idempotency checks

    if wrapped_mint_account.data_len() > 0 || wrapped_backpointer_account.data_len() > 0 {
        msg!("Wrapped mint or backpointer account already initialized");
        if !idempotent {
            return Err(ProgramError::AccountAlreadyInitialized);
        }
        if wrapped_mint_account.owner != wrapped_token_program_account.key {
            msg!("Wrapped mint account owner is not the expected token program");
            return Err(ProgramError::InvalidAccountData);
        }
        if wrapped_backpointer_account.owner != program_id {
            msg!("Wrapped backpointer account owner is not the expected token wrap program");
            return Err(ProgramError::InvalidAccountData);
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
        return Err(ProgramError::InsufficientFunds);
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
        return Err(ProgramError::InsufficientFunds);
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
        TokenWrapInstruction::Wrap { .. } => {
            msg!("Instruction: Wrap");
            unimplemented!();
        }
        TokenWrapInstruction::Unwrap { .. } => {
            msg!("Instruction: UnWrap");
            unimplemented!();
        }
    }
}
