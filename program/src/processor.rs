//! Program state processor

use {
    crate::{
        get_escrow_address, get_wrapped_mint_address, get_wrapped_mint_address_with_seed,
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
            return Err(ProgramError::InvalidAccountOwner);
        }
        if wrapped_backpointer_account.owner != program_id {
            msg!("Wrapped backpointer account owner is not the expected token wrap program");
            return Err(ProgramError::InvalidAccountOwner);
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
        return Err(ProgramError::AccountNotRentExempt);
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

/// Processes [`Wrap`](enum.TokenWrapInstruction.html) instruction.
pub fn process_wrap(_program_id: &Pubkey, accounts: &[AccountInfo], amount: u64) -> ProgramResult {
    if amount == 0 {
        msg!("Wrap amount should be positive");
        return Err(ProgramError::InvalidArgument);
    }

    let account_info_iter = &mut accounts.iter();

    let transfer_authority = next_account_info(account_info_iter)?;
    let unwrapped_escrow = next_account_info(account_info_iter)?;
    let unwrapped_token_account = next_account_info(account_info_iter)?;
    let recipient_wrapped_token_account = next_account_info(account_info_iter)?;
    let wrapped_mint = next_account_info(account_info_iter)?;
    let unwrapped_token_program = next_account_info(account_info_iter)?;
    let wrapped_token_program = next_account_info(account_info_iter)?;
    let unwrapped_mint = next_account_info(account_info_iter)?;
    let wrapped_mint_authority = next_account_info(account_info_iter)?;
    let _signer_accounts = account_info_iter.as_slice();

    // Validate accounts

    let expected_wrapped_mint =
        get_wrapped_mint_address(unwrapped_mint.key, wrapped_token_program.key);
    if expected_wrapped_mint != *wrapped_mint.key {
        msg!("Wrapped mint address does not match the derived address");
        return Err(ProgramError::InvalidAccountData);
    }

    let expected_authority = get_wrapped_mint_authority(wrapped_mint.key);
    if *wrapped_mint_authority.key != expected_authority {
        msg!("Wrapped mint authority does not match the derived address");
        return Err(ProgramError::IncorrectAuthority);
    }

    let expected_escrow = get_escrow_address(transfer_authority.key, unwrapped_mint.key);
    if expected_escrow != *unwrapped_escrow.key {
        msg!("Escrow address does not match the derived address");
        return Err(ProgramError::InvalidAccountData);
    }

    {
        let escrow_data = unwrapped_escrow.try_borrow_data()?;
        let escrow_account = spl_token::state::Account::unpack(&escrow_data)?;
        if escrow_account.owner != expected_authority {
            msg!("Unwrapped escrow token owner is not set to token-wrap program");
            return Err(ProgramError::InvalidAccountOwner);
        }
    }

    // Transfer unwrapped tokens from user to escrow

    let unwrapped_mint_data = unwrapped_mint.try_borrow_data()?;
    let unwrapped_mint_state = PodStateWithExtensions::<PodMint>::unpack(&unwrapped_mint_data)?;
    invoke_signed(
        &spl_token_2022::instruction::transfer_checked(
            unwrapped_token_program.key,
            unwrapped_token_account.key,
            unwrapped_mint.key,
            unwrapped_escrow.key,
            transfer_authority.key,
            &[],
            amount,
            unwrapped_mint_state.base.decimals,
        )?,
        &[
            unwrapped_token_account.clone(),
            unwrapped_mint.clone(),
            unwrapped_escrow.clone(),
            transfer_authority.clone(),
        ],
        &[],
    )?;

    // Mint wrapped tokens to recipient
    let bump = get_wrapped_mint_authority_with_seed(wrapped_mint.key).1;
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
            process_wrap(program_id, accounts, amount)
        }
        TokenWrapInstruction::Unwrap { .. } => {
            msg!("Instruction: Unwrap");
            unimplemented!();
        }
    }
}
