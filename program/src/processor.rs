//! Program state processor

use crate::state::Backpointer;
use crate::{
    _get_wrapped_mint_authority_signer_seeds, _get_wrapped_mint_backpointer_address_signer_seeds,
    _get_wrapped_mint_signer_seeds, get_wrapped_mint_address_with_seed,
    get_wrapped_mint_authority_with_seed, get_wrapped_mint_backpointer_address_with_seed,
};
use solana_program::account_info::next_account_info;
use solana_program::program::invoke_signed;
use solana_program::program_error::ProgramError;
use solana_program::program_pack::Pack;
use solana_program::rent::Rent;
use solana_program::sysvar::Sysvar;
use solana_program::{msg, system_instruction};
use spl_token_2022::instruction::initialize_mint2;
use {
    crate::instruction::TokenWrapInstruction,
    solana_program::{account_info::AccountInfo, entrypoint::ProgramResult, pubkey::Pubkey},
};

/// Processes [CreateMint](enum.TokenWrapInstruction.html) instruction.
pub fn process_create_mint(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    idempotent: bool,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();

    let wrapped_mint_account = next_account_info(account_info_iter)?;
    let wrapped_backpointer_account = next_account_info(account_info_iter)?;
    let unwrapped_mint_account = next_account_info(account_info_iter)?;
    let system_program_account = next_account_info(account_info_iter)?; // TODO: This is not used 🤔
    let wrapped_token_program_account = next_account_info(account_info_iter)?;

    // Idempotency checks

    if wrapped_mint_account.data_len() > 0 || wrapped_backpointer_account.data_len() > 0 {
        msg!("Wrapped mint or backpointer account already initialized");
        return if !idempotent {
            Err(ProgramError::AccountAlreadyInitialized)
        } else {
            Ok(())
        };
    }

    // Initialize wrapped mint PDA

    let (wrapped_mint_address, bump) = get_wrapped_mint_address_with_seed(
        unwrapped_mint_account.key,
        wrapped_token_program_account.key,
    );
    let bump_seed = [bump];
    let signer_seeds = _get_wrapped_mint_signer_seeds(
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
        &system_instruction::allocate(&wrapped_mint_address, space as u64),
        &[wrapped_mint_account.clone()],
        &[&signer_seeds],
    )?;
    invoke_signed(
        &system_instruction::assign(&wrapped_mint_address, wrapped_token_program_account.key),
        &[wrapped_mint_account.clone()],
        &[&signer_seeds],
    )?;

    // New wrapped mint matches decimals of unwrapped mint
    let unwrapped_mint_data = unwrapped_mint_account.try_borrow_data()?;
    let unpacked_unwrapped_mint = spl_token_2022::state::Mint::unpack(&unwrapped_mint_data)?;
    let decimals = unpacked_unwrapped_mint.decimals;

    let (wrapped_mint_authority, authority_bump_seed) =
        get_wrapped_mint_authority_with_seed(wrapped_mint_account.key);
    let authority_bump_seeds = [authority_bump_seed];
    let authority_signer_seeds =
        _get_wrapped_mint_authority_signer_seeds(wrapped_mint_account.key, &authority_bump_seeds);

    invoke_signed(
        &initialize_mint2(
            wrapped_token_program_account.key,
            wrapped_mint_account.key,
            &wrapped_mint_authority,
            None,
            decimals,
        )?,
        &[wrapped_mint_account.clone()],
        &[&authority_signer_seeds],
    )?;

    // Initialize backpointer PDA

    let (wrapped_backpointer_address, bump) =
        get_wrapped_mint_backpointer_address_with_seed(wrapped_mint_account.key);
    if *wrapped_backpointer_account.key != wrapped_backpointer_address {
        msg!("Error: wrapped_backpointer_account address is not as expected");
        return Err(ProgramError::InvalidSeeds);
    }

    let backpointer_space = std::mem::size_of::<Backpointer>();
    let backpointer_rent_required = rent.minimum_balance(space);
    if wrapped_backpointer_account.lamports() < rent.minimum_balance(backpointer_space) {
        msg!(
            "Error: wrapped_backpointer_account requires pre-funding of {} lamports",
            backpointer_rent_required
        );
        return Err(ProgramError::InsufficientFunds);
    }

    let bump_seed = [bump];
    let backpointer_signer_seeds =
        _get_wrapped_mint_backpointer_address_signer_seeds(wrapped_mint_account.key, &bump_seed);
    invoke_signed(
        &system_instruction::allocate(&wrapped_backpointer_address, backpointer_space as u64),
        &[wrapped_backpointer_account.clone()],
        &[&backpointer_signer_seeds],
    )?;
    invoke_signed(
        &system_instruction::assign(&wrapped_backpointer_address, program_id),
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
        TokenWrapInstruction::UnWrap { .. } => {
            msg!("Instruction: UnWrap");
            unimplemented!();
        }
    }
}
