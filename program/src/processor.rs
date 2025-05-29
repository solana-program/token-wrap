//! Program state processor

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
    solana_program_error::{ProgramError, ProgramResult},
    solana_program_pack::Pack,
    solana_pubkey::Pubkey,
    solana_rent::Rent,
    solana_system_interface::instruction::{allocate, assign},
    solana_sysvar::Sysvar,
    spl_token_2022::{
        extension::PodStateWithExtensions,
        instruction::initialize_mint2,
        onchain::{extract_multisig_accounts, invoke_transfer_checked},
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

    let escrow_data = unwrapped_escrow.try_borrow_data()?;
    let escrow_account = PodStateWithExtensions::<PodAccount>::unpack(&escrow_data)?;
    if escrow_account.base.owner != expected_authority {
        Err(TokenWrapError::EscrowOwnerMismatch)?
    }
    drop(escrow_data);

    // Transfer unwrapped tokens from user to escrow

    let unwrapped_mint_data = unwrapped_mint.try_borrow_data()?;
    let unwrapped_mint_state = PodStateWithExtensions::<PodMint>::unpack(&unwrapped_mint_data)?;
    invoke_transfer_checked(
        unwrapped_token_program.key,
        unwrapped_token_account.clone(),
        unwrapped_mint.clone(),
        unwrapped_escrow.clone(),
        transfer_authority.clone(),
        &accounts[9..],
        amount,
        unwrapped_mint_state.base.decimals,
        &[],
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
    )
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
    let additional_accounts = account_info_iter.as_slice();

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

    let multisig_signer_keys = extract_multisig_accounts(transfer_authority, additional_accounts)?
        .iter()
        .map(|a| a.key)
        .collect::<Vec<_>>();

    invoke(
        &spl_token_2022::instruction::burn(
            wrapped_token_program.key,
            wrapped_token_account.key,
            wrapped_mint.key,
            transfer_authority.key,
            &multisig_signer_keys,
            amount,
        )?,
        &accounts[6..],
    )?;

    // Transfer unwrapped tokens from escrow to recipient

    let unwrapped_mint_data = unwrapped_mint.try_borrow_data()?;
    let unwrapped_mint_state = PodStateWithExtensions::<PodMint>::unpack(&unwrapped_mint_data)?;
    let bump_seed = [bump];
    let signer_seeds = get_wrapped_mint_authority_signer_seeds(wrapped_mint.key, &bump_seed);

    invoke_transfer_checked(
        unwrapped_token_program.key,
        unwrapped_escrow.clone(),
        unwrapped_mint.clone(),
        recipient_unwrapped_token.clone(),
        wrapped_mint_authority.clone(),
        additional_accounts,
        amount,
        unwrapped_mint_state.base.decimals,
        &[&signer_seeds],
    )
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
