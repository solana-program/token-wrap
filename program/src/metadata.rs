//! Metadata resolution helpers for pointer-aware metadata sync

use {
    crate::{error::TokenWrapError, metaplex::metaplex_to_token_2022_metadata},
    mpl_token_metadata::accounts::Metadata as MetaplexMetadata,
    solana_account_info::AccountInfo,
    solana_cpi::{get_return_data, invoke},
    solana_program_error::ProgramError,
    spl_token_2022::{
        extension::{
            metadata_pointer::MetadataPointer, BaseStateWithExtensions, PodStateWithExtensions,
        },
        pod::PodMint,
    },
    spl_token_metadata_interface::{instruction::emit, state::TokenMetadata},
    spl_type_length_value::variable_len_pack::VariableLenPack,
};

/// Fetches metadata from a third-party program implementing
/// `TokenMetadataInstruction` by invoking its `Emit` instruction and decoding
/// the `TokenMetadata` struct from the return data.
pub fn cpi_emit_and_decode<'a>(
    owner_program_info: &AccountInfo<'a>,
    metadata_info: &AccountInfo<'a>,
) -> Result<TokenMetadata, ProgramError> {
    invoke(
        &emit(owner_program_info.key, metadata_info.key, None, None),
        &[metadata_info.clone()],
    )?;

    if let Some((program_key, data)) = get_return_data() {
        // This check ensures this data comes from the program we just called
        if program_key == *owner_program_info.key {
            return TokenMetadata::unpack_from_slice(&data);
        }
    }

    Err(TokenWrapError::ExternalProgramReturnedNoData.into())
}

fn read_metaplex_for_mint(
    mint_info: &AccountInfo,
    source_metadata_info: Option<&AccountInfo>,
) -> Result<TokenMetadata, ProgramError> {
    let metadata_info = source_metadata_info.ok_or(ProgramError::NotEnoughAccountKeys)?;
    if metadata_info.owner != &mpl_token_metadata::ID {
        return Err(ProgramError::InvalidAccountOwner);
    }
    let (expected_pda, _) = MetaplexMetadata::find_pda(mint_info.key);
    if *metadata_info.key != expected_pda {
        return Err(TokenWrapError::MetaplexMetadataMismatch.into());
    }
    metaplex_to_token_2022_metadata(mint_info, metadata_info)
}

/// Resolve the canonical metadata source for an unwrapped Token-2022 mint
/// by following its `MetadataPointer`.
///
/// Supported pointer targets:
/// - Self
/// - Token-2022 account
/// - `Metaplex` PDA
/// - Third-party program
///
/// If no pointer is on the mint, attempt to read the `Metaplex` PDA directly.
pub fn resolve_token_2022_source_metadata<'a>(
    unwrapped_mint_info: &AccountInfo<'a>,
    maybe_source_metadata_info: Option<&AccountInfo<'a>>,
    maybe_owner_program_info: Option<&AccountInfo<'a>>,
) -> Result<TokenMetadata, ProgramError> {
    let data = unwrapped_mint_info.try_borrow_data()?;
    let mint_state = PodStateWithExtensions::<PodMint>::unpack(&data)?;

    let Ok(pointer) = mint_state.get_extension::<MetadataPointer>() else {
        // No pointer? Fall back to Metaplex PDA.
        return read_metaplex_for_mint(unwrapped_mint_info, maybe_source_metadata_info);
    };

    // Pointer present, get set address on extension
    let Some(metadata_addr) = Option::from(pointer.metadata_address) else {
        return Err(TokenWrapError::MetadataPointerUnset.into());
    };

    // Pointer points to self, read off unwrapped mint metadata extension
    if metadata_addr == *unwrapped_mint_info.key {
        return mint_state.get_variable_len_extension::<TokenMetadata>();
    }

    // Metadata account must be passed by this point
    let metadata_info = maybe_source_metadata_info.ok_or(ProgramError::NotEnoughAccountKeys)?;
    if metadata_info.key != &metadata_addr {
        return Err(TokenWrapError::MetadataPointerMismatch.into());
    }

    if metadata_info.owner == &spl_token_2022::id() {
        // This is explicitly unsupported. A metadata pointer should not point to
        // another mint account.
        Err(ProgramError::InvalidAccountData)
    } else if metadata_info.owner == &mpl_token_metadata::ID {
        // Scenario 2: points to a Metaplex PDA
        metaplex_to_token_2022_metadata(unwrapped_mint_info, metadata_info)
    } else {
        // Scenario 3: points to an external program
        let owner_program_info =
            maybe_owner_program_info.ok_or(ProgramError::NotEnoughAccountKeys)?;
        if owner_program_info.key != metadata_info.owner {
            return Err(ProgramError::InvalidAccountOwner);
        }
        cpi_emit_and_decode(owner_program_info, metadata_info)
    }
}

/// Extracts the token metadata from the unwrapped mint
pub fn extract_token_metadata<'a>(
    unwrapped_mint_info: &AccountInfo<'a>,
    source_metadata_info: Option<&AccountInfo<'a>>,
    owner_program_info: Option<&AccountInfo<'a>>,
) -> Result<TokenMetadata, ProgramError> {
    if *unwrapped_mint_info.owner == spl_token_2022::id() {
        // Source is Token-2022: resolve metadata pointer
        resolve_token_2022_source_metadata(
            unwrapped_mint_info,
            source_metadata_info,
            owner_program_info,
        )
    } else if *unwrapped_mint_info.owner == spl_token::id() {
        // Source is spl-token: read from Metaplex PDA
        read_metaplex_for_mint(unwrapped_mint_info, source_metadata_info)
    } else {
        Err(ProgramError::IncorrectProgramId)
    }
}
