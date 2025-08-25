//! A reference mint customizer for compliance-focused use cases.
//! This is NOT enabled by default. To use it, the processor must be
//! modified to use this instead of `DefaultToken2022Customizer`.

use {
    crate::{get_wrapped_mint_authority, mint_customizer::interface::MintCustomizer},
    solana_account_info::AccountInfo,
    solana_cpi::invoke,
    solana_program_error::{ProgramError, ProgramResult},
    solana_pubkey::Pubkey,
    spl_token_2022::{
        extension::{
            confidential_transfer::instruction::initialize_mint as initialize_confidential_transfer_mint,
            default_account_state::instruction::initialize_default_account_state,
            // permanent_delegate init helper is not available under extension::permanent_delegate
            // in this workspace version; we'll use the top-level instruction helper instead.
            ExtensionType,
            PodStateWithExtensions,
        },
        pod::PodMint,
        state::{AccountState, Mint},
    },
};

/// A reference implementation for a mint customizer that adds several
/// extensions to showcase advanced capabilities of SPL Token 2022 for
/// compliance-focused use cases.
///
/// This customizer enables:
/// - A permanent delegate.
/// - Default account state set to `Frozen` to support sRFC-37 style
///   allow-lists.
/// - Pausable transfers by setting the mint's freeze authority.
/// - Confidential transfers with a designated auditor.
pub struct ComplianceMintCustomizer;

/// Permanent delegate address for the reference customizer.
/// Exposed for tests to assert correct initialization.
pub static PERMANENT_DELEGATE_ADDRESS_RAW: Pubkey = Pubkey::new_from_array([0xAA; 32]);
/// Public static reference used by tests
pub static PERMANENT_DELEGATE_ADDRESS: &Pubkey = &PERMANENT_DELEGATE_ADDRESS_RAW;

/// Auditor ElGamal public key used by the reference customizer.
/// Exposed for tests to assert correct initialization.
pub const AUDITOR_ELGAMAL_PUBKEY: Pubkey = Pubkey::new_from_array([0x11; 32]);

impl MintCustomizer for ComplianceMintCustomizer {
    fn get_token_2022_mint_space() -> Result<usize, ProgramError> {
        // Calculate space for all extensions that will be initialized. A mint's size
        // is immutable, so all desired extensions must be included at creation.
        ExtensionType::try_calculate_account_len::<Mint>(&[
            ExtensionType::PermanentDelegate,
            ExtensionType::DefaultAccountState,
            ExtensionType::ConfidentialTransferMint,
        ])
    }

    fn initialize_extensions(
        wrapped_mint_account: &AccountInfo,
        wrapped_token_program_account: &AccountInfo,
    ) -> ProgramResult {
        // All extension initialization instructions must be invoked *before* the
        // `InitializeMint` instruction.

        // Permanent Delegate: This delegate can burn or transfer tokens from any
        // account for this mint, even without an explicit approval. This authority is
        // irrevocable.
        invoke(
            &spl_token_2022::instruction::initialize_permanent_delegate(
                wrapped_token_program_account.key,
                wrapped_mint_account.key,
                PERMANENT_DELEGATE_ADDRESS,
            )?,
            &[wrapped_mint_account.clone()],
        )?;

        // Default Account State: New token accounts will be initialized in the `Frozen`
        // state by default. This is a key part of the sRFC-37 allow-list standard,
        // where a freeze authority must explicitly thaw accounts before use.
        invoke(
            &initialize_default_account_state(
                wrapped_token_program_account.key,
                wrapped_mint_account.key,
                &AccountState::Frozen,
            )?,
            &[wrapped_mint_account.clone()],
        )?;

        // Confidential Transfer with Auditor: Enables private transactions and
        // specifies an auditor that can decrypt transaction amounts for compliance.
        let wrapped_mint_authority = get_wrapped_mint_authority(wrapped_mint_account.key);
        invoke(
            &initialize_confidential_transfer_mint(
                wrapped_token_program_account.key,
                wrapped_mint_account.key,
                Some(wrapped_mint_authority), // Authority to manage settings
                true,                         /* Auto-approve new accounts for confidential
                                               * transfers */
                Some(AUDITOR_ELGAMAL_PUBKEY.to_bytes().into()),
            )?,
            &[wrapped_mint_account.clone()],
        )?;

        Ok(())
    }

    fn get_freeze_auth_and_decimals(
        unwrapped_mint_account: &AccountInfo,
    ) -> Result<(Option<Pubkey>, u8), ProgramError> {
        // Pausable Tokens: By setting a freeze authority, we enable "pausable"
        // functionality. The freeze authority can freeze all token accounts,
        // effectively pausing transfers. For a token wrap, setting this to the
        // program-derived authority is a sensible default.
        let wrapped_mint_authority = get_wrapped_mint_authority(unwrapped_mint_account.key);
        let freeze_authority = Some(wrapped_mint_authority);

        // Copy decimals from the original unwrapped mint.
        let unwrapped_mint_data = unwrapped_mint_account.try_borrow_data()?;
        let pod_mint = PodStateWithExtensions::<PodMint>::unpack(&unwrapped_mint_data)?.base;
        let decimals = pod_mint.decimals;

        Ok((freeze_authority, decimals))
    }
}
