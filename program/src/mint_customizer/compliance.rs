use {
    crate::mint_customizer::interface::MintCustomizer,
    solana_account_info::AccountInfo,
    solana_cpi::invoke,
    solana_program_error::{ProgramError, ProgramResult},
    solana_pubkey::Pubkey,
    spl_token_2022::{
        extension::{
            confidential_transfer::instruction::initialize_mint as initialize_confidential_transfer_mint,
            default_account_state::instruction::initialize_default_account_state, ExtensionType,
            PodStateWithExtensions,
        },
        instruction::initialize_permanent_delegate,
        pod::PodMint,
        state::{AccountState, Mint},
    },
};

/// A reference implementation for a mint `customizer` that adds the following
/// extensions for compliance-focused use cases:
/// - A permanent delegate
/// - `Pausable` transfers by setting the mint's freeze authority
/// - Confidential transfers with a designated auditor
///
/// In the future, can support sRFC-37: https://github.com/solana-foundation/SRFCs/discussions/2
pub struct ComplianceMintCustomizer;

/// Permanent delegate that can transfer/burn from any account of this mint
pub const PERMANENT_DELEGATE_ADDRESS: Pubkey =
    solana_pubkey::pubkey!("deLpBmD7UP27BHTuhnxR7mBE9rEV6mWUnwWsXMXTFwR");

/// Authority that manages Confidential Transfer mint settings
pub const CONFIDENTIAL_EXT_AUTHORITY: Pubkey =
    solana_pubkey::pubkey!("con2YXp7bKscyhzJzbSQgwz6RFcXqe6otUGK5Rr8saK");

/// Auditor public key for Confidential Transfer amount visibility
pub const AUDITOR_ELGAMAL_PUBKEY: Pubkey =
    solana_pubkey::pubkey!("audrNg32Nfg6YnJmU9dfzgLUXjxr2AqehdFsSbyz4Kp");

/// Mint freeze authority enabling freezable tokens
pub const FREEZE_AUTHORITY: Pubkey =
    solana_pubkey::pubkey!("freTRAXwCVELv5k7V6UobnCiG1hmhnj79AezxRwAR3h");

impl MintCustomizer for ComplianceMintCustomizer {
    fn get_token_2022_mint_space() -> Result<usize, ProgramError> {
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
        // This delegate can burn or transfer tokens from any account for this mint,
        // even without an explicit approval
        invoke(
            &initialize_permanent_delegate(
                wrapped_token_program_account.key,
                wrapped_mint_account.key,
                &PERMANENT_DELEGATE_ADDRESS,
            )?,
            &[wrapped_mint_account.clone()],
        )?;

        // Enables private transactions and specifies an auditor that can decrypt
        // transaction amounts for compliance
        invoke(
            &initialize_confidential_transfer_mint(
                wrapped_token_program_account.key,
                wrapped_mint_account.key,
                Some(CONFIDENTIAL_EXT_AUTHORITY), // Authority to manage settings
                true,
                // Enable compliance monitoring by allowing auditor to decrypt confidential
                // transfer amounts
                Some(AUDITOR_ELGAMAL_PUBKEY.to_bytes().into()),
            )?,
            &[wrapped_mint_account.clone()],
        )?;

        // By default, new accounts are initialized. The freeze authority can freeze
        // them individually.
        invoke(
            &initialize_default_account_state(
                wrapped_token_program_account.key,
                wrapped_mint_account.key,
                &AccountState::Initialized,
            )?,
            &[wrapped_mint_account.clone()],
        )?;

        Ok(())
    }

    fn get_freeze_auth_and_decimals(
        unwrapped_mint_account: &AccountInfo,
    ) -> Result<(Option<Pubkey>, u8), ProgramError> {
        // Copy decimals from the original unwrapped mint.
        let unwrapped_mint_data = unwrapped_mint_account.try_borrow_data()?;
        let pod_mint = PodStateWithExtensions::<PodMint>::unpack(&unwrapped_mint_data)?.base;
        let decimals = pod_mint.decimals;

        // By setting a freeze authority, we enable "pausable" functionality. The freeze
        // authority can freeze all token accounts, effectively pausing transfers.
        Ok((Some(FREEZE_AUTHORITY), decimals))
    }
}
