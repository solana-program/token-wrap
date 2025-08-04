use {
    crate::{get_wrapped_mint_authority, mint_customizer::interface::MintCustomizer},
    solana_account_info::AccountInfo,
    solana_cpi::{invoke, invoke_signed},
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
    spl_token_metadata_interface::{
        instruction::initialize as initialize_token_metadata, state::TokenMetadata,
    },
};

/// This implementation adds the `ConfidentialTransferMint` & `TokenMetadata`
/// extensions by default.
pub struct DefaultToken2022Customizer;

impl MintCustomizer for DefaultToken2022Customizer {
    fn get_token_2022_mint_initialization_space() -> Result<usize, ProgramError> {
        // Calculate space for all extensions that are initialized *before* the base
        // mint. The TokenMetadata extension is initialized *after* and its
        // `initialize` instruction handles its own reallocation.
        ExtensionType::try_calculate_account_len::<Mint>(&[
            ExtensionType::ConfidentialTransferMint,
            ExtensionType::MetadataPointer,
        ])
    }

    fn get_token_2022_total_space() -> Result<usize, ProgramError> {
        let base_size = Self::get_token_2022_mint_initialization_space()?;
        let metadata_size = TokenMetadata::default().tlv_size_of()?;
        base_size
            .checked_add(metadata_size)
            .ok_or(ProgramError::ArithmeticOverflow)
    }

    fn pre_initialize_extensions(
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

    fn post_initialize_extensions<'a>(
        wrapped_mint_account: &AccountInfo<'a>,
        wrapped_token_program_account: &AccountInfo,
        wrapped_mint_authority_account: &AccountInfo<'a>,
        mint_authority_signer_seeds: &[&[u8]],
    ) -> ProgramResult {
        // Initialize metadata ext (must be done after mint initialization)
        let wrapped_mint_authority = get_wrapped_mint_authority(wrapped_mint_account.key);

        let cpi_accounts = [
            wrapped_mint_account.clone(),
            wrapped_mint_authority_account.clone(),
            wrapped_mint_account.clone(),
            wrapped_mint_authority_account.clone(),
        ];

        invoke_signed(
            &initialize_token_metadata(
                wrapped_token_program_account.key,
                wrapped_mint_account.key,
                &wrapped_mint_authority,
                wrapped_mint_account.key,
                &wrapped_mint_authority,
                // Initialized as empty, but separate instructions are available
                // to update these fields
                "".to_string(),
                "".to_string(),
                "".to_string(),
            ),
            &cpi_accounts,
            &[mint_authority_signer_seeds],
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
