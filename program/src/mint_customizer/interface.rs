use {
    solana_account_info::AccountInfo,
    solana_program_error::{ProgramError, ProgramResult},
    solana_pubkey::Pubkey,
    spl_token_2022::extension::ExtensionType,
};

/// The interface for customizing attributes of the new wrapped mint.
pub trait MintCustomizer {
    /// Returns the extensions to be included in the new wrapped mint
    /// (only relevant if creating spl-token-2022 mint)
    fn get_extension_types(&self) -> Vec<ExtensionType>;

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
