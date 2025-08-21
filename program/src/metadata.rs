//! Metadata resolution helpers for pointer-aware metadata sync

use {
    crate::{error::TokenWrapError, metaplex::metaplex_to_token_2022_metadata},
    mpl_token_metadata::ID as MPL_TOKEN_METADATA_ID,
    solana_account_info::AccountInfo,
    solana_cpi::{get_return_data, invoke},
    solana_program_error::ProgramError,
    spl_token_2022::{
        extension::{
            metadata_pointer::MetadataPointer, BaseStateWithExtensions, PodStateWithExtensions,
        },
        id as token_2022_id,
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

/// Resolve the canonical metadata source for an unwrapped Token-2022 mint
/// by following its `MetadataPointer`.
///
/// Supported pointer targets:
/// - Self
/// - Token-2022 account
/// - `Metaplex` PDA
/// - Third-party program
pub fn resolve_token_2022_source_metadata<'a>(
    unwrapped_mint_info: &AccountInfo<'a>,
    maybe_source_metadata_info: Option<&AccountInfo<'a>>,
    maybe_owner_program_info: Option<&AccountInfo<'a>>,
) -> Result<TokenMetadata, ProgramError> {
    let data = unwrapped_mint_info.try_borrow_data()?;
    let mint_state = PodStateWithExtensions::<PodMint>::unpack(&data)?;
    let pointer = mint_state
        .get_extension::<MetadataPointer>()
        .map_err(|_| TokenWrapError::MetadataPointerMissing)?;
    let metadata_addr =
        Option::from(pointer.metadata_address).ok_or(TokenWrapError::MetadataPointerUnset)?;

    // Scenario 1: points to self, read off unwrapped mint
    if metadata_addr == *unwrapped_mint_info.key {
        return mint_state.get_variable_len_extension::<TokenMetadata>();
    }

    // Metadata account must be passed by this point
    let metadata_info = maybe_source_metadata_info.ok_or(ProgramError::NotEnoughAccountKeys)?;
    if metadata_info.key != &metadata_addr {
        return Err(TokenWrapError::MetadataPointerMismatch.into());
    }

    if metadata_info.owner == &token_2022_id() {
        // This is explicitly unsupported. A metadata pointer should not point to
        // another mint account.
        Err(ProgramError::InvalidAccountData)
    } else if metadata_info.owner == &MPL_TOKEN_METADATA_ID {
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
