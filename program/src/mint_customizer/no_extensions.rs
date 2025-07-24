use {
    crate::mint_customizer::interface::MintCustomizer,
    solana_account_info::AccountInfo,
    solana_program_error::{ProgramError, ProgramResult},
    solana_pubkey::Pubkey,
    spl_token_2022::{
        extension::{ExtensionType, PodStateWithExtensions},
        pod::PodMint,
    },
};

/// This implementation does not add any extensions.
pub struct NoExtensionCustomizer;

impl MintCustomizer for NoExtensionCustomizer {
    fn get_extension_types(&self) -> Vec<ExtensionType> {
        vec![]
    }

    fn initialize_extensions(
        &self,
        _wrapped_mint_account: &AccountInfo,
        _unwrapped_mint_account: &AccountInfo,
        _wrapped_token_program_account: &AccountInfo,
        _all_accounts: &[AccountInfo],
    ) -> ProgramResult {
        Ok(())
    }

    fn get_freeze_auth_and_decimals(
        &self,
        unwrapped_mint_account: &AccountInfo,
        _all_accounts: &[AccountInfo],
    ) -> Result<(Option<Pubkey>, u8), ProgramError> {
        // Copy fields over from original mint
        let unwrapped_mint_data = unwrapped_mint_account.try_borrow_data()?;
        let pod_mint = PodStateWithExtensions::<PodMint>::unpack(&unwrapped_mint_data)?.base;
        let freeze_authority = pod_mint.freeze_authority.ok_or(()).ok();
        let decimals = pod_mint.decimals;
        Ok((freeze_authority, decimals))
    }
}
