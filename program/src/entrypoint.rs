//! Program entrypoint

#![cfg(all(target_os = "solana", not(feature = "no-entrypoint")))]

use {
    crate::error::log_error, solana_account_info::AccountInfo, solana_program_error::ProgramResult,
    solana_pubkey::Pubkey,
};

solana_program_entrypoint::entrypoint!(process_instruction);

fn process_instruction<'a>(
    program_id: &Pubkey,
    accounts: &'a [AccountInfo<'a>],
    instruction_data: &[u8],
) -> ProgramResult {
    crate::processor::process_instruction(program_id, accounts, instruction_data)
        .inspect_err(log_error)
}
