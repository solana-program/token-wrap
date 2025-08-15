//! Program instructions

use {
    solana_instruction::{AccountMeta, Instruction},
    solana_program_error::ProgramError,
    solana_pubkey::Pubkey,
    std::convert::TryInto,
};

/// Instructions supported by the Token Wrap program
#[derive(Clone, Debug, PartialEq)]
#[repr(u8)]
pub enum TokenWrapInstruction {
    /// Create a wrapped token mint. Assumes caller has pre-funded wrapped mint
    /// and backpointer account. Supports both directions:
    /// - spl-token to token-2022
    /// - token-2022 to spl-token
    /// - token-2022 to token-2022 w/ new extensions
    ///
    /// Accounts expected by this instruction:
    ///
    /// 0. `[w]` Unallocated wrapped mint account to create (PDA), address must
    ///    be: `get_wrapped_mint_address(unwrapped_mint_address,
    ///    wrapped_token_program_id)`
    /// 1. `[w]` Unallocated wrapped backpointer account to create (PDA)
    ///    `get_wrapped_mint_backpointer_address(wrapped_mint_address)`
    /// 2. `[]` Existing unwrapped mint
    /// 3. `[]` System program
    /// 4. `[]` SPL Token program for wrapped mint
    CreateMint {
        /// If true, idempotent creation. If false, fail if the mint already
        /// exists.
        idempotent: bool,
    },

    /// Wrap tokens
    ///
    /// Move a user's unwrapped tokens into an escrow account and mint the same
    /// number of wrapped tokens into the provided account.
    ///
    /// Accounts expected by this instruction:
    ///
    /// 0. `[w]` Recipient wrapped token account
    /// 1. `[w]` Wrapped mint, must be initialized, address must be:
    ///    `get_wrapped_mint_address(unwrapped_mint_address,
    ///    wrapped_token_program_id)`
    /// 2. `[]` Wrapped mint authority, address must be:
    ///    `get_wrapped_mint_authority(wrapped_mint)`
    /// 3. `[]` SPL Token program for unwrapped mint
    /// 4. `[]` SPL Token program for wrapped mint
    /// 5. `[w]` Unwrapped token account to wrap
    ///    `get_wrapped_mint_authority(wrapped_mint_address)`
    /// 6. `[]` Unwrapped token mint
    /// 7. `[w]` Escrow of unwrapped tokens, address must be an `ATA`:
    ///    `get_escrow_address(unwrapped_mint, unwrapped_token_program,
    ///    wrapped_token_program)`
    /// 8. `[s]` Transfer authority on unwrapped token account. Not required to
    ///    be a signer if it's a multisig.
    /// 9. `..8+M` `[s]` (Optional) M multisig signers on unwrapped token
    ///    account.
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
    /// 0. `[w]` Escrow of unwrapped tokens, address must be an `ATA`:
    ///    `get_escrow_address(unwrapped_mint, unwrapped_token_program,
    ///    wrapped_token_program)`
    /// 1. `[w]` Recipient unwrapped tokens
    /// 2. `[]` Wrapped mint authority, address must be:
    ///    `get_wrapped_mint_authority(wrapped_mint)`
    /// 3. `[]` Unwrapped token mint
    /// 4. `[]` SPL Token program for wrapped mint
    /// 5. `[]` SPL Token program for unwrapped mint
    /// 6. `[w]` Wrapped token account to unwrap
    /// 7. `[w]` Wrapped mint, address must be:
    ///    `get_wrapped_mint_address(unwrapped_mint_address,
    ///    wrapped_token_program_id)`
    /// 8. `[s]` Transfer authority on wrapped token account
    /// 9. `..8+M` `[s]` (Optional) M multisig signers on wrapped token account
    Unwrap {
        /// little-endian `u64` representing the amount to unwrap
        amount: u64,
    },

    /// Closes a stuck escrow `ATA`. This is for the edge case where an
    /// unwrapped mint with a close authority is closed and then a new mint
    /// is created at the same address but with a different size, leaving
    /// the escrow `ATA` in a bad state.
    ///
    /// This instruction will close the old escrow `ATA`, returning the lamports
    /// to the destination account. It will only work if the current escrow has
    /// different extensions than the mint. The client is then responsible
    /// for calling `create_associated_token_account` to recreate it.
    ///
    /// Accounts expected by this instruction:
    ///
    /// 0. `[w]` Escrow account to close (`ATA`)
    /// 1. `[w]` Destination for lamports from closed account
    /// 2. `[]` Unwrapped mint
    /// 3. `[]` Wrapped mint
    /// 4. `[]` Wrapped mint authority (PDA)
    /// 5. `[]` Token-2022 program
    CloseStuckEscrow,

    /// This instruction copies the metadata fields from an unwrapped mint to
    /// its wrapped mint `TokenMetadata` extension.
    ///
    /// Supports (unwrapped to wrapped):
    /// - Token-2022 to Token-2022
    /// - SPL-token to Token-2022
    ///
    /// If the `TokenMetadata` extension on the wrapped mint if not present, it
    /// will initialize it. The client is responsible for funding the wrapped
    /// mint account with enough lamports to cover the rent for the
    /// additional space required by the `TokenMetadata` extension and/or
    /// metadata sync.
    ///
    /// Accounts expected by this instruction:
    ///
    /// 0. `[w]` Wrapped mint
    /// 1. `[]` Wrapped mint authority PDA
    /// 2. `[]` Unwrapped mint
    /// 3. `[]` Token-2022 program
    /// 4. `[]` (Optional) `Metaplex` Metadata PDA. Required if the unwrapped
    ///    mint is an `spl-token` mint.
    SyncMetadataToToken2022,
}

impl TokenWrapInstruction {
    /// Packs a [`TokenWrapInstruction`](enum.TokenWrapInstruction.html) into a
    /// byte array.
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
            TokenWrapInstruction::Unwrap { amount } => {
                buf.push(2);
                buf.extend_from_slice(&amount.to_le_bytes());
            }
            TokenWrapInstruction::CloseStuckEscrow => {
                buf.push(3);
            }
            TokenWrapInstruction::SyncMetadataToToken2022 => {
                buf.push(4);
            }
        }
        buf
    }

    /// Unpacks a byte array into a
    /// [`TokenWrapInstruction`](enum.TokenWrapInstruction.html).
    pub fn unpack(input: &[u8]) -> Result<Self, ProgramError> {
        match input.split_first() {
            Some((&0, rest)) if rest.len() == 1 => {
                let idempotent = match rest[0] {
                    0 => false,
                    1 => true,
                    _ => return Err(ProgramError::InvalidInstructionData),
                };
                Ok(TokenWrapInstruction::CreateMint { idempotent })
            }
            Some((&1, rest)) if rest.len() == 8 => {
                let amount = u64::from_le_bytes(rest.try_into().unwrap());
                Ok(TokenWrapInstruction::Wrap { amount })
            }
            Some((&2, rest)) if rest.len() == 8 => {
                let amount = u64::from_le_bytes(rest.try_into().unwrap());
                Ok(TokenWrapInstruction::Unwrap { amount })
            }
            Some((&3, [])) => Ok(TokenWrapInstruction::CloseStuckEscrow),
            Some((&4, [])) => Ok(TokenWrapInstruction::SyncMetadataToToken2022),
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

    let data = TokenWrapInstruction::Unwrap { amount }.pack();
    Instruction::new_with_bytes(*program_id, &data, accounts)
}

/// Creates `CloseStuckEscrow` instruction.
pub fn close_stuck_escrow(
    program_id: &Pubkey,
    escrow_address: &Pubkey,
    destination_address: &Pubkey,
    unwrapped_mint_address: &Pubkey,
    wrapped_mint_address: &Pubkey,
    wrapped_mint_authority_address: &Pubkey,
) -> Instruction {
    let accounts = vec![
        AccountMeta::new(*escrow_address, false),
        AccountMeta::new(*destination_address, false),
        AccountMeta::new_readonly(*unwrapped_mint_address, false),
        AccountMeta::new_readonly(*wrapped_mint_address, false),
        AccountMeta::new_readonly(*wrapped_mint_authority_address, false),
        AccountMeta::new_readonly(spl_token_2022::id(), false),
    ];
    let data = TokenWrapInstruction::CloseStuckEscrow.pack();
    Instruction::new_with_bytes(*program_id, &data, accounts)
}

/// Creates `SyncMetadataToToken2022` instruction.
pub fn sync_metadata_to_token_2022(
    program_id: &Pubkey,
    wrapped_mint: &Pubkey,
    wrapped_mint_authority: &Pubkey,
    unwrapped_mint: &Pubkey,
    metaplex_metadata: Option<&Pubkey>,
) -> Instruction {
    let mut accounts = vec![
        AccountMeta::new(*wrapped_mint, false),
        AccountMeta::new_readonly(*wrapped_mint_authority, false),
        AccountMeta::new_readonly(*unwrapped_mint, false),
        AccountMeta::new_readonly(spl_token_2022::id(), false),
    ];

    if let Some(pubkey) = metaplex_metadata {
        accounts.push(AccountMeta::new_readonly(*pubkey, false));
    }

    let data = TokenWrapInstruction::SyncMetadataToToken2022.pack();
    Instruction::new_with_bytes(*program_id, &data, accounts)
}
