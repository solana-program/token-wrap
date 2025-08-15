//! `Metaplex` related helpers

use {
    mpl_token_metadata::accounts::Metadata as MetaplexMetadata, solana_account_info::AccountInfo,
    solana_program_error::ProgramError, spl_pod::optional_keys::OptionalNonZeroPubkey,
    spl_token_metadata_interface::state::TokenMetadata,
};

fn extract_additional_metadata(
    metaplex_metadata: &MetaplexMetadata,
) -> Result<Vec<(String, String)>, ProgramError> {
    let mut additional_metadata = vec![
        (
            "key".to_string(),
            serde_json::to_string(&metaplex_metadata.key)
                .map_err(|_| ProgramError::InvalidAccountData)?,
        ),
        (
            "seller_fee_basis_points".to_string(),
            metaplex_metadata.seller_fee_basis_points.to_string(),
        ),
        (
            "primary_sale_happened".to_string(),
            metaplex_metadata.primary_sale_happened.to_string(),
        ),
        (
            "is_mutable".to_string(),
            metaplex_metadata.is_mutable.to_string(),
        ),
    ];

    if let Some(creators) = &metaplex_metadata.creators {
        if !creators.is_empty() {
            additional_metadata.push((
                "creators".to_string(),
                serde_json::to_string(creators).map_err(|_| ProgramError::InvalidAccountData)?,
            ));
        }
    }
    if let Some(edition_nonce) = metaplex_metadata.edition_nonce {
        additional_metadata.push(("edition_nonce".to_string(), edition_nonce.to_string()));
    }
    if let Some(token_standard) = &metaplex_metadata.token_standard {
        additional_metadata.push((
            "token_standard".to_string(),
            serde_json::to_string(token_standard).map_err(|_| ProgramError::InvalidAccountData)?,
        ));
    }
    if let Some(collection) = &metaplex_metadata.collection {
        additional_metadata.push((
            "collection".to_string(),
            serde_json::to_string(collection).map_err(|_| ProgramError::InvalidAccountData)?,
        ));
    }
    if let Some(uses) = &metaplex_metadata.uses {
        additional_metadata.push((
            "uses".to_string(),
            serde_json::to_string(uses).map_err(|_| ProgramError::InvalidAccountData)?,
        ));
    }
    if let Some(collection_details) = &metaplex_metadata.collection_details {
        additional_metadata.push((
            "collection_details".to_string(),
            serde_json::to_string(collection_details)
                .map_err(|_| ProgramError::InvalidAccountData)?,
        ));
    }
    if let Some(programmable_config) = &metaplex_metadata.programmable_config {
        additional_metadata.push((
            "programmable_config".to_string(),
            serde_json::to_string(programmable_config)
                .map_err(|_| ProgramError::InvalidAccountData)?,
        ));
    }

    Ok(additional_metadata)
}

/// Converts `Metaplex` metadata to the Token-2022 `TokenMetadata` format.
pub fn metaplex_to_token_2022_metadata(
    unwrapped_mint_info: &AccountInfo,
    metaplex_metadata_info: &AccountInfo,
) -> Result<TokenMetadata, ProgramError> {
    let metaplex_data = metaplex_metadata_info.try_borrow_data()?;
    let metaplex_metadata = MetaplexMetadata::safe_deserialize(&metaplex_data)
        .map_err(|_| ProgramError::InvalidAccountData)?;

    let additional_metadata = extract_additional_metadata(&metaplex_metadata)?;

    Ok(TokenMetadata {
        update_authority: OptionalNonZeroPubkey(metaplex_metadata.update_authority),
        mint: *unwrapped_mint_info.key,
        name: metaplex_metadata.name,
        symbol: metaplex_metadata.symbol,
        uri: metaplex_metadata.uri,
        additional_metadata,
    })
}
