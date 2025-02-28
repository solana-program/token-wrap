//! Program instructions

use {solana_program_error::ProgramError, std::convert::TryInto};

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
    /// 7. `[w]` Escrow of unwrapped tokens, must be owned by:
    ///    `get_wrapped_mint_authority(wrapped_mint_address)`
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
    ///
    /// 0. `[writeable]` Escrow of unwrapped tokens, must be owned by:
    ///    `get_wrapped_mint_authority(wrapped_mint_address)`
    /// 1. `[writeable]` Recipient unwrapped tokens
    /// 2. `[]` Wrapped mint authority, address must be:
    ///    `get_wrapped_mint_authority(wrapped_mint)`
    /// 3. `[]` Unwrapped token mint
    /// 4. `[]` SPL Token program for wrapped mint
    /// 5. `[]` SPL Token program for unwrapped mint
    /// 6. `[writeable]` Wrapped token account to unwrap
    /// 7. `[writeable]` Wrapped mint, address must be:
    ///    `get_wrapped_mint_address(unwrapped_mint_address,
    ///    wrapped_token_program_id)`
    /// 8. `[signer]` Transfer authority on wrapped token account
    /// 9. `..8+M` `[signer]` (Optional) M multisig signers on wrapped token
    ///    account
    Unwrap {
        /// little-endian `u64` representing the amount to unwrap
        amount: u64,
    },
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
            _ => Err(ProgramError::InvalidInstructionData),
        }
    }
}
