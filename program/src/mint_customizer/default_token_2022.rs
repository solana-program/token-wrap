use {
    crate::{get_wrapped_mint_authority, mint_customizer::interface::MintCustomizer},
    solana_account_info::AccountInfo,
    solana_cpi::invoke,
    solana_program_error::{ProgramError, ProgramResult},
    solana_pubkey::Pubkey,
    spl_token_2022::{
        extension::{
            confidential_transfer::instruction::initialize_mint as initialize_confidential_transfer_mint,
            metadata_pointer::instruction::initialize as initialize_metadata_pointer,
            ExtensionType::{self},
            PodStateWithExtensions,
        },
        pod::PodMint,
        state::Mint,
    },
};

/// This implementation adds the `ConfidentialTransferMint` & `MetadataPointer`
/// extensions by default.
pub struct DefaultToken2022Customizer;

impl MintCustomizer for DefaultToken2022Customizer {
    fn get_token_2022_mint_space() -> Result<usize, ProgramError> {
        // Calculate space for all extensions that are initialized *before* the base
        // mint. The TokenMetadata extension is initialized *after* and its
        // `initialize` instruction handles its own reallocation.
        ExtensionType::try_calculate_account_len::<Mint>(&[
            ExtensionType::ConfidentialTransferMint,
            ExtensionType::MetadataPointer,
        ])
    }

    fn initialize_extensions(
        wrapped_mint_account: &AccountInfo,
        wrapped_token_program_account: &AccountInfo,
    ) -> ProgramResult {
        // Initialize confidential transfer ext
        invoke(
            &initialize_confidential_transfer_mint(
                wrapped_token_program_account.key,
                wrapped_mint_account.key,
                None, // Immutable. No one can later change privacy settings.
                true, // No approvals necessary to use.
                None, // No auditor can decrypt transaction amounts.
            )?,
            &[wrapped_mint_account.clone()],
        )?;

        // Initialize metadata pointer
        let wrapped_mint_authority = get_wrapped_mint_authority(wrapped_mint_account.key);
        invoke(
            &initialize_metadata_pointer(
                wrapped_token_program_account.key,
                wrapped_mint_account.key,
                Some(wrapped_mint_authority),
                Some(*wrapped_mint_account.key),
            )?,
            &[wrapped_mint_account.clone()],
        )?;

        Ok(())
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
