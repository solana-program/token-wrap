//! Program state processor

use solana_program::account_info::next_account_info;
use solana_program::program_error::ProgramError;
use solana_program::rent::Rent;
use solana_program::{msg, system_instruction};
use solana_program::{program::invoke_signed, program_pack::Pack};
use {
    crate::instruction::TokenWrapInstruction,
    solana_program::{account_info::AccountInfo, entrypoint::ProgramResult, pubkey::Pubkey},
};

/// TODO: Add docs
pub fn process_create_mint(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    idempotent: bool,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();

    let wrapped_mint_account = next_account_info(account_info_iter)?;
    let wrapped_backpointer_account = next_account_info(account_info_iter)?;
    let unwrapped_mint_account = next_account_info(account_info_iter)?;
    let system_program_account = next_account_info(account_info_iter)?;
    let wrapped_token_program_account = next_account_info(account_info_iter)?;

    // TODO: Add account validation (has correct permissions, is passed in correct order, etc)

    // // --- Mint Existence Check and Idempotency ---
    // if wrapped_mint_account.data_len() > 0 {
    //     msg!("Wrapped mint account already exists");
    //     return if !idempotent {
    //         Err(ProgramError::AccountAlreadyInitialized)
    //     } else {
    //         msg!("Idempotent creation requested, skipping account creation and initialization.");
    //         Ok(()) // Succeed silently as idempotent creation requested
    //     };
    // }

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
