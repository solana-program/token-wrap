//! Program instructions

use solana_program::instruction::{AccountMeta, Instruction};
use solana_program::program_error::ProgramError;
use solana_program::pubkey::Pubkey;
use std::convert::TryInto;

/// Instructions supported by the Token Wrap program
#[derive(Clone, Debug, PartialEq)]
#[repr(u8)]
pub enum TokenWrapInstruction {
    /// Create a wrapped token mint. Assumes caller has pre-funded wrapped mint
    /// and backpointer account. Supports both directions:
    /// - spl-token to token-2022
    /// - token-2022 to spl-token
    ///
    /// Accounts expected by this instruction:
    ///
    /// 0. `[writeable]` Unallocated wrapped mint account to create (PDA),
    ///    address must be: `get_wrapped_mint_address(unwrapped_mint_address,
    ///    wrapped_token_program_id)`
    /// 1. `[writeable]` Unallocated wrapped backpointer account to create (PDA)
    ///    `get_wrapped_mint_backpointer_address(wrapped_mint_address)`
    /// 2. `[]` Existing unwrapped mint
    /// 3. `[]` System program
    /// 4. `[]` SPL Token program for wrapped mint
    CreateMint {
        /// If true, idempotent creation. If false, fail if the mint already exists.
        idempotent: bool,
    },

    /// Wrap tokens
    ///
    /// Move a user's unwrapped tokens into an escrow account and mint the same
    /// number of wrapped tokens into the provided account.
    ///
    /// Accounts expected by this instruction:
    ///
    /// 0. `[writeable]` Unwrapped token account to wrap
    /// 1. `[writeable]` Escrow of unwrapped tokens, must be owned by:
    ///    `get_wrapped_mint_authority(wrapped_mint_address)`
    /// 2. `[]` Unwrapped token mint
    /// 3. `[writeable]` Wrapped mint, must be initialized, address must be:
    ///    `get_wrapped_mint_address(unwrapped_mint_address,
    ///    wrapped_token_program_id)`
    /// 4. `[writeable]` Recipient wrapped token account
    /// 5. `[]` Escrow mint authority, address must be:
    ///    `get_wrapped_mint_authority(wrapped_mint)`
    /// 6. `[]` SPL Token program for unwrapped mint
    /// 7. `[]` SPL Token program for wrapped mint
    /// 8. `[signer]` Transfer authority on unwrapped token account
    /// 9. `..8+M` `[signer]` (Optional) M multisig signers on unwrapped token
    ///    account
    Wrap {
        /// little-endian `u64` representing the amount to wrap
        amount: u64,
    },

    /// Unwrap tokens
    ///
    /// Burn user wrapped tokens and transfer the same amount of unwrapped
    /// tokens from the escrow account to the provided account.
    ///
    /// Accounts expected by this instruction:
    ///
    /// 0. `[writeable]` Wrapped token account to unwrap
    /// 1. `[writeable]` Wrapped mint, address must be:
    ///    `get_wrapped_mint_address(unwrapped_mint_address,
    ///    wrapped_token_program_id)`
    /// 2. `[writeable]` Escrow of unwrapped tokens, must be owned by:
    ///    `get_wrapped_mint_authority(wrapped_mint_address)`
    /// 3. `[writeable]` Recipient unwrapped tokens
    /// 4. `[]` Unwrapped token mint
    /// 5. `[]` Escrow unwrapped token authority
    ///    `get_wrapped_mint_authority(wrapped_mint)`
    /// 6. `[]` SPL Token program for wrapped mint
    /// 7. `[]` SPL Token program for unwrapped mint
    /// 8. `[signer]` Transfer authority on wrapped token account
    /// 9. `..8+M` `[signer]` (Optional) M multisig signers on wrapped token
    ///    account
    UnWrap {
        /// little-endian `u64` representing the amount to unwrap
        amount: u64,
    },
}

impl TokenWrapInstruction {
    /// Packs a [`TokenWrapInstruction`](enum.TokenWrapInstruction.html) into a byte array.
    pub fn pack(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        match self {
            TokenWrapInstruction::CreateMint { idempotent } => {
                buf.push(0);
                buf.push(if *idempotent { 1 } else { 0 });
            }

            TokenWrapInstruction::Wrap { amount } => {
                buf.push(1);
                buf.extend_from_slice(&amount.to_le_bytes());
            }
            TokenWrapInstruction::UnWrap { amount } => {
                buf.push(2);
                buf.extend_from_slice(&amount.to_le_bytes());
            }
        }
        buf
    }

    /// Unpacks a byte array into a [`TokenWrapInstruction`](enum.TokenWrapInstruction.html).
    pub fn unpack(input: &[u8]) -> Result<Self, ProgramError> {
        let (&tag, rest) = input
            .split_first()
            .ok_or(ProgramError::InvalidInstructionData)?;
        match tag {
            0 => {
                if rest.len() != 1 {
                    return Err(ProgramError::InvalidInstructionData);
                }
                let idempotent = rest[0] != 0;
                Ok(TokenWrapInstruction::CreateMint { idempotent })
            }
            1 => {
                if rest.len() != 8 {
                    return Err(ProgramError::InvalidInstructionData);
                }
                let amount = u64::from_le_bytes(
                    rest.try_into()
                        .map_err(|_| ProgramError::InvalidInstructionData)?,
                );
                Ok(TokenWrapInstruction::Wrap { amount })
            }
            2 => {
                if rest.len() != 8 {
                    return Err(ProgramError::InvalidInstructionData);
                }
                let amount = u64::from_le_bytes(
                    rest.try_into()
                        .map_err(|_| ProgramError::InvalidInstructionData)?,
                );
                Ok(TokenWrapInstruction::UnWrap { amount })
            }
            _ => Err(ProgramError::InvalidInstructionData),
        }
    }
}

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
        AccountMeta::new_readonly(solana_program::system_program::id(), false),
        AccountMeta::new_readonly(*wrapped_token_program_id, false),
    ];
    let data = TokenWrapInstruction::CreateMint { idempotent }.pack();
    Instruction::new_with_bytes(*program_id, &data, accounts)
}

/// Creates `UnWrap` instruction.
#[allow(clippy::too_many_arguments)]
pub fn unwrap(
    program_id: &Pubkey,
    wrapped_token_account_address: &Pubkey,
    wrapped_mint_address: &Pubkey,
    wrapped_escrow_address: &Pubkey,
    recipient_unwrapped_token_account_address: &Pubkey,
    unwrapped_mint_address: &Pubkey,
    wrapped_mint_authority_address: &Pubkey,
    wrapped_token_program_id: &Pubkey,
    unwrapped_token_program_id: &Pubkey,
    transfer_authority_address: &Pubkey,
    amount: u64,
) -> Instruction {
    let accounts = vec![
        AccountMeta::new(*wrapped_token_account_address, false),
        AccountMeta::new(*wrapped_mint_address, false),
        AccountMeta::new(*wrapped_escrow_address, false),
        AccountMeta::new(*recipient_unwrapped_token_account_address, false),
        AccountMeta::new_readonly(*unwrapped_mint_address, false),
        AccountMeta::new_readonly(*wrapped_mint_authority_address, false),
        AccountMeta::new_readonly(*wrapped_token_program_id, false),
        AccountMeta::new_readonly(*unwrapped_token_program_id, false),
        AccountMeta::new_readonly(*transfer_authority_address, true),
    ];
    let data = TokenWrapInstruction::UnWrap { amount }.pack();
    Instruction::new_with_bytes(*program_id, &data, accounts)
}

/// Creates `Wrap` instruction.
#[allow(clippy::too_many_arguments)]
pub fn wrap(
    program_id: &Pubkey,
    unwrapped_token_account_address: &Pubkey,
    wrapped_escrow_address: &Pubkey,
    unwrapped_mint_address: &Pubkey,
    wrapped_mint_address: &Pubkey,
    recipient_wrapped_token_account_address: &Pubkey,
    wrapped_mint_authority_address: &Pubkey,
    unwrapped_token_program_id: &Pubkey,
    wrapped_token_program_id: &Pubkey,
    transfer_authority_address: &Pubkey,
    amount: u64,
) -> Instruction {
    let accounts = vec![
        AccountMeta::new(*unwrapped_token_account_address, false),
        AccountMeta::new(*wrapped_escrow_address, false),
        AccountMeta::new_readonly(*unwrapped_mint_address, false),
        AccountMeta::new(*wrapped_mint_address, false),
        AccountMeta::new(*recipient_wrapped_token_account_address, false),
        AccountMeta::new_readonly(*wrapped_mint_authority_address, false),
        AccountMeta::new_readonly(*unwrapped_token_program_id, false),
        AccountMeta::new_readonly(*wrapped_token_program_id, false),
        AccountMeta::new_readonly(*transfer_authority_address, true),
    ];
    let data = TokenWrapInstruction::Wrap { amount }.pack();
    Instruction::new_with_bytes(*program_id, &data, accounts)
}
