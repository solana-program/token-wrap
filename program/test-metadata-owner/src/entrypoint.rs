//! Program entrypoint

#![cfg(all(target_os = "solana", not(feature = "no-entrypoint")))]

use {
    solana_account_info::AccountInfo, solana_program_error::ProgramResult, solana_pubkey::Pubkey,
    spl_token_metadata_interface::instruction::TokenMetadataInstruction,
};

solana_program_entrypoint::entrypoint!(process_instruction);

fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    let ix = TokenMetadataInstruction::unpack(instruction_data)?;
    crate::processor::process_instruction(program_id, accounts, ix)
}
