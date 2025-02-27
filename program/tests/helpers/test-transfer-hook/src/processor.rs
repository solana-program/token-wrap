//! Program state processor

use {
    crate::state::Counter,
    solana_account_info::{next_account_info, AccountInfo},
    solana_program_error::ProgramResult,
    solana_pubkey::Pubkey,
};

pub fn process_instruction(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    _instruction_data: &[u8],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();

    let _source = next_account_info(account_info_iter)?;
    let _mint = next_account_info(account_info_iter)?;
    let _destination = next_account_info(account_info_iter)?;
    let _authority = next_account_info(account_info_iter)?;
    let _validation_state = next_account_info(account_info_iter)?;
    let counter_account = next_account_info(account_info_iter)?;

    // Increment the counter
    let mut counter_data = counter_account.try_borrow_mut_data()?;
    let counter_size = std::mem::size_of::<Counter>();
    let counter_slice = &mut counter_data[..counter_size];

    let counter = bytemuck::from_bytes_mut::<Counter>(counter_slice);
    counter.count = counter.count.checked_add(1).unwrap();

    Ok(())
}
