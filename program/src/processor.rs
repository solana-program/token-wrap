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
    solana_pubkey::Pubkey,
    solana_rent::Rent,
    solana_system_interface::instruction::{allocate, assign},
    solana_sysvar::{clock::Clock, Sysvar},
    spl_associated_token_account_client::address::get_associated_token_address_with_program_id,
    spl_token_2022::{
        extension::{
            confidential_transfer::instruction::initialize_mint as initialize_confidential_transfer_mint,
            transfer_fee::TransferFeeConfig, BaseStateWithExtensions, ExtensionType,
            PodStateWithExtensions,
        },
        instruction::initialize_mint2,
        onchain::{
            extract_multisig_accounts, invoke_transfer_checked, invoke_transfer_checked_with_fee,
        },
        pod::{PodAccount, PodMint},
        state::{AccountState, Mint},
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

    // The *unwrapped mint* must itself be a real SPL‑Token mint
    if unwrapped_mint_account.owner != &spl_token::id()
        && unwrapped_mint_account.owner != &spl_token_2022::id()
    {
        Err(ProgramError::InvalidAccountOwner)?
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

    // ConfidentialTransferMint extension added by default for Token-2022 wrapped
    // mints. Existing extensions from the unwrapped mint are not preserved.
    let is_token_2022 = wrapped_token_program_account.key == &spl_token_2022::id();
    let extensions = if is_token_2022 {
        vec![ExtensionType::ConfidentialTransferMint]
    } else {
        vec![]
    };
    let space = ExtensionType::try_calculate_account_len::<Mint>(&extensions)?;

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

    // Initialize confidential transfer extension if this is a token-2022 mint
    if is_token_2022 {
        invoke(
            &initialize_confidential_transfer_mint(
                wrapped_token_program_account.key,
                wrapped_mint_account.key,
                None, // Immutable. No one can later change privacy settings.
                true, // No approvals necessary to use.
                None, // No auditor can decrypt transaction amounts.
            )?,
            &[wrapped_mint_account.clone()],
        )?;
    }

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
    let backpointer_rent_required = rent.minimum_balance(backpointer_space);
    if wrapped_backpointer_account.lamports() < backpointer_rent_required {
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

    let expected_escrow = get_associated_token_address_with_program_id(
        wrapped_mint_authority.key,
        unwrapped_mint.key,
        unwrapped_token_program.key,
    );

    if *unwrapped_escrow.key != expected_escrow {
        Err(TokenWrapError::EscrowMismatch)?
    }

    {
        let escrow_data = unwrapped_escrow.try_borrow_data()?;
        let escrow_account = PodStateWithExtensions::<PodAccount>::unpack(&escrow_data)?;
        if escrow_account.base.owner != expected_authority {
            Err(TokenWrapError::EscrowOwnerMismatch)?
        }
    }

    // Transfer unwrapped tokens from user to escrow

    let unwrapped_mint_data = unwrapped_mint.try_borrow_data()?;
    let unwrapped_mint_state = PodStateWithExtensions::<PodMint>::unpack(&unwrapped_mint_data)?;

    // Calculate amount to mint (subtracting for possible transfer fee)
    let clock = Clock::get()?.epoch;
    let fee = unwrapped_mint_state
        .get_extension::<TransferFeeConfig>()
        .ok()
        .and_then(|cfg| cfg.calculate_epoch_fee(clock, amount))
        .unwrap_or(0);
    let net_amount = amount
        .checked_sub(fee)
        .ok_or(ProgramError::ArithmeticOverflow)?;

    if unwrapped_token_program.key == &spl_token_2022::id() {
        // This invoke fn does extra validation on calculated fee
        invoke_transfer_checked_with_fee(
            unwrapped_token_program.key,
            unwrapped_token_account.clone(),
            unwrapped_mint.clone(),
            unwrapped_escrow.clone(),
            transfer_authority.clone(),
            &accounts[9..],
            amount,
            unwrapped_mint_state.base.decimals,
            fee,
            &[],
        )?;
    } else {
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
    }

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
            net_amount,
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

    let expected_escrow = get_associated_token_address_with_program_id(
        wrapped_mint_authority.key,
        unwrapped_mint.key,
        unwrapped_token_program.key,
    );
    if *unwrapped_escrow.key != expected_escrow {
        Err(TokenWrapError::EscrowMismatch)?
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
    )?;

    Ok(())
}

/// Processes [`CloseStuckEscrow`](enum.TokenWrapInstruction.html) instruction.
pub fn process_close_stuck_escrow(accounts: &[AccountInfo]) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();

    let escrow_account = next_account_info(account_info_iter)?;
    let destination_account = next_account_info(account_info_iter)?;
    let unwrapped_mint = next_account_info(account_info_iter)?;
    let wrapped_mint = next_account_info(account_info_iter)?;
    let wrapped_mint_authority = next_account_info(account_info_iter)?;
    let _token_2022_program = next_account_info(account_info_iter)?;

    // This instruction is only for spl-token-2022 accounts because only they
    // can have extensions that lead to size changes.
    if *escrow_account.owner != spl_token_2022::id()
        || unwrapped_mint.owner != &spl_token_2022::id()
    {
        return Err(ProgramError::IncorrectProgramId);
    }

    let expected_wrapped_mint_pubkey =
        get_wrapped_mint_address(unwrapped_mint.key, wrapped_mint.owner);
    if *wrapped_mint.key != expected_wrapped_mint_pubkey {
        Err(TokenWrapError::WrappedMintMismatch)?
    }

    let (expected_authority, bump) = get_wrapped_mint_authority_with_seed(wrapped_mint.key);
    if *wrapped_mint_authority.key != expected_authority {
        Err(TokenWrapError::MintAuthorityMismatch)?
    }

    let expected_escrow_address = get_associated_token_address_with_program_id(
        wrapped_mint_authority.key,
        unwrapped_mint.key,
        unwrapped_mint.owner,
    );

    if *escrow_account.key != expected_escrow_address {
        return Err(TokenWrapError::EscrowMismatch.into());
    }

    let escrow_data = escrow_account.try_borrow_data()?;
    let escrow_state = PodStateWithExtensions::<PodAccount>::unpack(&escrow_data)?;

    if escrow_state.base.owner != *wrapped_mint_authority.key {
        return Err(TokenWrapError::EscrowOwnerMismatch.into());
    }

    // Closing only works when the token balance is zero
    if u64::from(escrow_state.base.amount) != 0 {
        return Err(ProgramError::InvalidAccountData);
    }

    // Ensure the account is in the initialized state
    if escrow_state.base.state != (AccountState::Initialized as u8) {
        return Err(ProgramError::InvalidAccountData);
    }

    let current_account_extensions = escrow_state.get_extension_types()?;
    drop(escrow_data);

    let mint_data = unwrapped_mint.try_borrow_data()?;
    let mint_state = PodStateWithExtensions::<PodMint>::unpack(&mint_data)?;
    let mint_extensions = mint_state.get_extension_types()?;
    let mut required_account_extensions =
        ExtensionType::get_required_init_account_extensions(&mint_extensions);

    // ATAs always have the ImmutableOwner extension
    required_account_extensions.push(ExtensionType::ImmutableOwner);

    // If the token account already shares the same extensions as the mint,
    // it does not need to be re-created
    let in_good_state = current_account_extensions.len() == required_account_extensions.len()
        && required_account_extensions
            .iter()
            .all(|item| current_account_extensions.contains(item));

    if in_good_state {
        return Err(TokenWrapError::EscrowInGoodState.into());
    }

    // Close old escrow account
    let bump_seed = [bump];
    let signer_seeds = get_wrapped_mint_authority_signer_seeds(wrapped_mint.key, &bump_seed);

    invoke_signed(
        &spl_token_2022::instruction::close_account(
            escrow_account.owner,
            escrow_account.key,
            destination_account.key,
            wrapped_mint_authority.key,
            &[],
        )?,
        &[
            escrow_account.clone(),
            destination_account.clone(),
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
        TokenWrapInstruction::CloseStuckEscrow => {
            msg!("Instruction: CloseStuckEscrow");
            process_close_stuck_escrow(accounts)
        }
    }
}
