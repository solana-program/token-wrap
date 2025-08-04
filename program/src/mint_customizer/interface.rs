use {
    solana_account_info::AccountInfo,
    solana_program_error::{ProgramError, ProgramResult},
    solana_pubkey::Pubkey,
};

/// The interface for customizing attributes of the new wrapped mint.
pub trait MintCustomizer {
    /// Calculates the space required for a new spl-token-2022 mint
    /// account, including any custom extensions
    fn get_token_2022_mint_initialization_space() -> Result<usize, ProgramError>;

    /// Calculates the total space required for a new spl-token-2022
    /// mint, after `post_initialize_extensions()` is called. This is useful in
    /// calculating rent requirements if something like `TokenMetadata` does a
    /// realloc after the mint is created. If not implemented, defaults to
    /// `get_token_2022_mint_initialization_space()` result.
    fn get_token_2022_total_space() -> Result<usize, ProgramError> {
        Self::get_token_2022_mint_initialization_space()
    }

    /// Customizes extensions for the wrapped mint *before* the base mint is
    /// initialized. This is for extensions that must be initialized on an
    /// uninitialized mint account, like `ConfidentialTransferMint`.
    fn pre_initialize_extensions(
        wrapped_mint_account: &AccountInfo,
        wrapped_token_program_account: &AccountInfo,
    ) -> ProgramResult;

    /// Customizes extensions for the wrapped mint *after* the base mint is
    /// initialized. This is for extensions that require the mint to be
    /// initialized, like `TokenMetadata`.
    fn post_initialize_extensions<'a>(
        wrapped_mint_account: &AccountInfo<'a>,
        wrapped_token_program_account: &AccountInfo,
        wrapped_mint_authority_account: &AccountInfo<'a>,
        mint_authority_signer_seeds: &[&[u8]],
    ) -> ProgramResult;

    /// Customize the freeze authority and decimals for the wrapped mint
    fn get_freeze_auth_and_decimals(
        unwrapped_mint_account: &AccountInfo,
    ) -> Result<(Option<Pubkey>, u8), ProgramError>;
}
