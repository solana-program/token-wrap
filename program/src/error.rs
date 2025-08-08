//! Error types

use {
    num_derive::FromPrimitive,
    num_traits::FromPrimitive,
    solana_msg::msg,
    solana_program_error::{ProgramError, ToStr},
    std::convert::TryFrom,
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
    /// Escrow account address does not match expected `ATA`
    #[error("Escrow account address does not match expected ATA")]
    EscrowMismatch,
    /// The escrow account is in a good state and cannot be recreated
    #[error("The escrow account is in a good state and cannot be recreated")]
    EscrowInGoodState,
    /// Unwrapped mint does not have the `TokenMetadata` extension
    #[error("Unwrapped mint does not have the TokenMetadata extension")]
    UnwrappedMintHasNoMetadata,
}

impl From<TokenWrapError> for ProgramError {
    fn from(e: TokenWrapError) -> Self {
        ProgramError::Custom(e as u32)
    }
}

impl TryFrom<u32> for TokenWrapError {
    type Error = ProgramError;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        TokenWrapError::from_u32(value).ok_or(ProgramError::InvalidArgument)
    }
}

impl ToStr for TokenWrapError {
    fn to_str<E>(&self) -> &'static str
    where
        E: 'static + ToStr + TryFrom<u32>,
    {
        match self {
            TokenWrapError::WrappedMintMismatch => "Error: WrappedMintMismatch",
            TokenWrapError::BackpointerMismatch => "Error: BackpointerMismatch",
            TokenWrapError::ZeroWrapAmount => "Error: ZeroWrapAmount",
            TokenWrapError::MintAuthorityMismatch => "Error: MintAuthorityMismatch",
            TokenWrapError::EscrowOwnerMismatch => "Error: EscrowOwnerMismatch",
            TokenWrapError::InvalidWrappedMintOwner => "Error: InvalidWrappedMintOwner",
            TokenWrapError::InvalidBackpointerOwner => "Error: InvalidBackpointerOwner",
            TokenWrapError::EscrowMismatch => "Error: EscrowMismatch",
            TokenWrapError::EscrowInGoodState => "Error: EscrowInGoodState",
            TokenWrapError::UnwrappedMintHasNoMetadata => "Error: UnwrappedMintHasNoMetadata",
        }
    }
}

/// Logs program errors
pub fn log_error(err: &ProgramError) {
    msg!(err.to_str::<TokenWrapError>());
}
