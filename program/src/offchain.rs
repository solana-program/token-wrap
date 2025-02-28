//! Program instructions

use {
    crate::instruction::TokenWrapInstruction,
    solana_instruction::{AccountMeta, Instruction},
    solana_pubkey::Pubkey,
};

/// Creates `CreateMint` instruction.
pub fn create_mint(
    program_id: &Pubkey,
    wrapped_mint_address: &Pubkey,
    wrapped_backpointer_address: &Pubkey,
    unwrapped_mint_address: &Pubkey,
    wrapped_token_program_id: &Pubkey,
    idempotent: bool,
) -> Instruction {
    let accounts = vec![
        AccountMeta::new(*wrapped_mint_address, false),
        AccountMeta::new(*wrapped_backpointer_address, false),
        AccountMeta::new_readonly(*unwrapped_mint_address, false),
        AccountMeta::new_readonly(solana_system_interface::program::id(), false),
        AccountMeta::new_readonly(*wrapped_token_program_id, false),
    ];
    let data = TokenWrapInstruction::CreateMint { idempotent }.pack();
    Instruction::new_with_bytes(*program_id, &data, accounts)
}

/// Creates `Wrap` instruction.
#[allow(clippy::too_many_arguments)]
pub fn wrap(
    program_id: &Pubkey,
    recipient_wrapped_token_account_address: &Pubkey,
    wrapped_mint_address: &Pubkey,
    wrapped_mint_authority_address: &Pubkey,
    unwrapped_token_program_id: &Pubkey,
    wrapped_token_program_id: &Pubkey,
    unwrapped_token_account_address: &Pubkey,
    unwrapped_mint_address: &Pubkey,
    unwrapped_escrow_address: &Pubkey,
    transfer_authority_address: &Pubkey,
    multisig_signer_pubkeys: &[&Pubkey],
    transfer_hook_metas: Vec<AccountMeta>,
    amount: u64,
) -> Instruction {
    let mut accounts = vec![
        AccountMeta::new(*recipient_wrapped_token_account_address, false),
        AccountMeta::new(*wrapped_mint_address, false),
        AccountMeta::new_readonly(*wrapped_mint_authority_address, false),
        AccountMeta::new_readonly(*unwrapped_token_program_id, false),
        AccountMeta::new_readonly(*wrapped_token_program_id, false),
        AccountMeta::new(*unwrapped_token_account_address, false),
        AccountMeta::new_readonly(*unwrapped_mint_address, false),
        AccountMeta::new(*unwrapped_escrow_address, false),
        AccountMeta::new_readonly(
            *transfer_authority_address,
            multisig_signer_pubkeys.is_empty(),
        ),
    ];
    for signer_pubkey in multisig_signer_pubkeys.iter() {
        accounts.push(AccountMeta::new_readonly(**signer_pubkey, true));
    }

    for meta in transfer_hook_metas.into_iter() {
        accounts.push(meta);
    }

    let data = TokenWrapInstruction::Wrap { amount }.pack();
    Instruction::new_with_bytes(*program_id, &data, accounts)
}

/// Creates `Unwrap` instruction.
#[allow(clippy::too_many_arguments)]
pub fn unwrap(
    program_id: &Pubkey,
    unwrapped_escrow_address: &Pubkey,
    recipient_unwrapped_token_account_address: &Pubkey,
    wrapped_mint_authority_address: &Pubkey,
    unwrapped_mint_address: &Pubkey,
    wrapped_token_program_id: &Pubkey,
    unwrapped_token_program_id: &Pubkey,
    wrapped_token_account_address: &Pubkey,
    wrapped_mint_address: &Pubkey,
    transfer_authority_address: &Pubkey,
    multisig_signer_pubkeys: &[&Pubkey],
    transfer_hook_metas: Vec<AccountMeta>,
    amount: u64,
) -> Instruction {
    let mut accounts = vec![
        AccountMeta::new(*unwrapped_escrow_address, false),
        AccountMeta::new(*recipient_unwrapped_token_account_address, false),
        AccountMeta::new_readonly(*wrapped_mint_authority_address, false),
        AccountMeta::new_readonly(*unwrapped_mint_address, false),
        AccountMeta::new_readonly(*wrapped_token_program_id, false),
        AccountMeta::new_readonly(*unwrapped_token_program_id, false),
        AccountMeta::new(*wrapped_token_account_address, false),
        AccountMeta::new(*wrapped_mint_address, false),
        AccountMeta::new_readonly(
            *transfer_authority_address,
            multisig_signer_pubkeys.is_empty(),
        ),
    ];
    for signer_pubkey in multisig_signer_pubkeys.iter() {
        accounts.push(AccountMeta::new_readonly(**signer_pubkey, true));
    }

    for meta in transfer_hook_metas.into_iter() {
        accounts.push(meta);
    }

    let data = TokenWrapInstruction::Unwrap { amount }.pack();
    Instruction::new_with_bytes(*program_id, &data, accounts)
}
