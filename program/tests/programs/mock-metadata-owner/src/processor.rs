//! This mock program simulates a third-party metadata program that chooses to
//! store its metadata directly within the `TokenMetadata` extension of a
//! Token-2022 mint account.
//!
//! This mock does not implement other instructions like `UpdateField`, as it's
//! only intended for testing the `Emit` functionality from a caller's
//! perspective.

use {
    crate::NO_RETURN,
    solana_account_info::{next_account_info, AccountInfo},
    solana_cpi::set_return_data,
    solana_program_error::ProgramResult,
    solana_pubkey::Pubkey,
    spl_token_2022::{
        extension::{BaseStateWithExtensions, PodStateWithExtensions},
        pod::PodMint,
    },
    spl_token_metadata_interface::{instruction::TokenMetadataInstruction, state::TokenMetadata},
};

pub fn process_instruction(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    ix: TokenMetadataInstruction,
) -> ProgramResult {
    let TokenMetadataInstruction::Emit(emit) = ix else {
        unimplemented!("Instruction not implemented")
    };

    let account_info_iter = &mut accounts.iter();
    let metadata_info = next_account_info(account_info_iter)?;

    if *metadata_info.key == NO_RETURN {
        return Ok(());
    }

    let data = metadata_info.try_borrow_data()?;
    let state = PodStateWithExtensions::<PodMint>::unpack(&data)?;
    let metadata_bytes = state.get_extension_bytes::<TokenMetadata>()?;

    let range = TokenMetadata::get_slice(metadata_bytes, emit.start, emit.end).unwrap();

    set_return_data(range);

    Ok(())
}
