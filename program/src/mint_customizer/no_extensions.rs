use {
    crate::mint_customizer::interface::MintCustomizer,
    solana_account_info::AccountInfo,
    solana_program_error::ProgramError,
    solana_pubkey::Pubkey,
    spl_token_2022::{
        extension::{ExtensionType, PodStateWithExtensions},
        pod::PodMint,
        state::Mint,
    },
};

/// This implementation does not add any extensions.
pub struct NoExtensionCustomizer;

impl MintCustomizer for NoExtensionCustomizer {
    fn get_token_2022_mint_space() -> Result<usize, ProgramError> {
        let extensions = vec![];
        ExtensionType::try_calculate_account_len::<Mint>(&extensions)
    }

    fn get_freeze_auth_and_decimals(
        unwrapped_mint_account: &AccountInfo,
    ) -> Result<(Option<Pubkey>, u8), ProgramError> {
        // Copy fields over from original mint
        let unwrapped_mint_data = unwrapped_mint_account.try_borrow_data()?;
        let pod_mint = PodStateWithExtensions::<PodMint>::unpack(&unwrapped_mint_data)?.base;
        let freeze_authority = pod_mint.freeze_authority.ok_or(()).ok();
        let decimals = pod_mint.decimals;
        Ok((freeze_authority, decimals))
    }
}
