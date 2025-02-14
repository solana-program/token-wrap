//! Error types

use {
    num_derive::FromPrimitive,
    solana_program::{
        decode_error::DecodeError,
        msg,
        program_error::{PrintProgramError, ProgramError},
    },
    std::error::Error,
    thiserror::Error,
};

/// Errors that may be returned by the Token Wrap program.
#[derive(Clone, Debug, Eq, Error, PartialEq, FromPrimitive)]
pub enum TokenWrapError {
    // 0
    /// Wrapped mint account address does not match expected PDA
    #[error("Wrapped mint account address does not match expected PDA")]
    WrappedMintMismatch,
    /// Wrapped backpointer account address does not match expected PDA
    #[error("Wrapped backpointer account address does not match expected PDA")]
    BackpointerMismatch,
    /// Wrap amount should be positive
    #[error("Wrap amount should be positive")]
    ZeroWrapAmount,
    /// Wrapped mint authority does not match expected PDA
    #[error("Wrapped mint authority does not match expected PDA")]
    MintAuthorityMismatch,
    /// Unwrapped escrow token owner is not set to expected PDA
    #[error("Unwrapped escrow token owner is not set to expected PDA")]
    EscrowOwnerMismatch,

    // 5
    /// Wrapped mint account owner is not the expected token program
    #[error("Wrapped mint account owner is not the expected token program")]
    InvalidWrappedMintOwner,
    /// Wrapped backpointer account owner is not the expected token wrap program
    #[error("Wrapped backpointer account owner is not the expected token wrap program")]
    InvalidBackpointerOwner,
}

impl From<TokenWrapError> for ProgramError {
    fn from(e: TokenWrapError) -> Self {
        ProgramError::Custom(e as u32)
    }
}

impl<T> DecodeError<T> for TokenWrapError {
    fn type_of() -> &'static str {
        "TokenWrapError"
    }
}

impl PrintProgramError for TokenWrapError {
    fn print<E>(&self)
    where
        E: 'static + Error + DecodeError<E> + PrintProgramError + num_traits::FromPrimitive,
    {
        msg!(&self.to_string());
    }
}
