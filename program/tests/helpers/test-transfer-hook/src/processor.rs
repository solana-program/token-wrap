//! Program state processor

use crate::state::Counter;
use solana_account_info::{next_account_info, AccountInfo};
use solana_msg::msg;
use solana_program_error::{ProgramError, ProgramResult};
use solana_pubkey::Pubkey;
use spl_pod::bytemuck::pod_from_bytes_mut;

pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    _instruction_data: &[u8],
) -> ProgramResult {
    msg!("Transfer Hook Entrypoint");

    // Get the counter account
    let accounts_iter = &mut accounts.iter();
    let counter_account = next_account_info(accounts_iter)?;

    // Check that the account is owned by the program
    if counter_account.owner != program_id {
        return Err(ProgramError::IncorrectProgramId);
    }

    // Increment the counter
    let mut counter_data = counter_account.try_borrow_mut_data()?;
    let counter = pod_from_bytes_mut::<Counter>(&mut counter_data)?;
    counter.count = counter.count.checked_add(1).unwrap();

    Ok(())
}
