use {
    solana_account_info::AccountInfo,
    solana_program_error::{ProgramError, ProgramResult},
    solana_pubkey::Pubkey,
};

/// The interface for customizing attributes of the new wrapped mint.
pub trait MintCustomizer {
    /// Calculates the total space required for a new spl-token-2022 mint
    /// account, including any custom extensions
    fn get_token_2022_mint_space(
        &self,
        unwrapped_mint_account: &AccountInfo,
        all_accounts: &[AccountInfo],
    ) -> Result<usize, ProgramError>;

    /// Customizes initialization for the extensions for the wrapped mint
    /// (only relevant if creating spl-token-2022 mint)
    fn initialize_extensions(
        &self,
        wrapped_mint_account: &AccountInfo,
        unwrapped_mint_account: &AccountInfo,
        wrapped_token_program_account: &AccountInfo,
        all_accounts: &[AccountInfo],
    ) -> ProgramResult;

    /// Customize the freeze authority and decimals for the wrapped mint
    fn get_freeze_auth_and_decimals(
        &self,
        unwrapped_mint_account: &AccountInfo,
        all_accounts: &[AccountInfo],
    ) -> Result<(Option<Pubkey>, u8), ProgramError>;
}
