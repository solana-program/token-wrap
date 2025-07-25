use {
    crate::mint_customizer::interface::MintCustomizer,
    solana_account_info::AccountInfo,
    solana_cpi::invoke,
    solana_program_error::{ProgramError, ProgramResult},
    solana_pubkey::Pubkey,
    spl_token_2022::{
        extension::{
            confidential_transfer::instruction::initialize_mint as initialize_confidential_transfer_mint,
            ExtensionType, PodStateWithExtensions,
        },
        pod::PodMint,
        state::Mint,
    },
};

/// This implementation adds the `ConfidentialTransferMint` extension by
/// default.
pub struct ConfidentialTransferCustomizer;

impl MintCustomizer for ConfidentialTransferCustomizer {
    fn get_token_2022_mint_space(
        &self,
        _unwrapped_mint_account: &AccountInfo,
        _all_accounts: &[AccountInfo],
    ) -> Result<usize, ProgramError> {
        let extensions = vec![ExtensionType::ConfidentialTransferMint];
        ExtensionType::try_calculate_account_len::<Mint>(&extensions)
    }

    fn initialize_extensions(
        &self,
        wrapped_mint_account: &AccountInfo,
        _unwrapped_mint_account: &AccountInfo,
        wrapped_token_program_account: &AccountInfo,
        _all_accounts: &[AccountInfo],
    ) -> ProgramResult {
        invoke(
            &initialize_confidential_transfer_mint(
                wrapped_token_program_account.key,
                wrapped_mint_account.key,
                None, // Immutable. No one can later change privacy settings.
                true, // No approvals necessary to use.
                None, // No auditor can decrypt transaction amounts.
            )?,
            &[wrapped_mint_account.clone()],
        )
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
